use crate::codec::Decoder;
use crate::sink::Sink;
use clearmic_dsp::{Chain, ChainConfig, Denoiser, Passthrough, Preset};
use clearmic_jitter::{JitterBuffer, Slot};
use clearmic_protocol::{Cipher, FRAME_SAMPLES};
use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicU8, Ordering::Relaxed};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DenoiseKind {
    None,
    /// DPDFNet 48 kHz ONNX model at this path. Needs the `dpdfnet` feature.
    Dpdfnet(String),
    /// DeepFilterNet3. Needs the `dfn3` feature.
    Dfn3,
}

#[derive(Clone)]
pub struct EngineConfig {
    pub port: u16,
    pub key: [u8; 32],
    pub preset: Preset,
    pub denoise: DenoiseKind,
    /// Lighter noise removers to fall back to, in order, if the PC cannot keep up with the first.
    pub fallbacks: Vec<DenoiseKind>,
    /// Replaces the preset's stage switches (for testing which stage helps).
    pub chain_override: Option<ChainConfig>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Snapshot {
    pub received: u64,
    pub lost: u64,
    pub late: u64,
    pub underruns: u64,
    pub rejected: u64,
    pub frames_played: u64,
    pub cushion_packets: u32,
    pub delay_ms: f32,
    pub level_in: f32,
    pub level_out: f32,
    /// Average dB the chain removed while there was sound, smoothed.
    pub nr_db: f32,
    /// Milliseconds since the last valid packet, None if none yet.
    pub packet_age_ms: Option<u64>,
    /// Share of one 10 ms frame the processing takes, smoothed, in percent.
    pub load_pct: f32,
    /// How many fallbacks have been used (0 = the first choice is running).
    pub fell_back: u32,
    /// True while the receiver is idle because no phone is sending.
    pub idle: bool,
}

#[derive(Default)]
struct Shared {
    stop: AtomicBool,
    bypass: AtomicBool,
    strength_bits: AtomicU32,
    preset: AtomicU8,
    rejected: AtomicU64,
    played: AtomicU64,
    cushion: AtomicU32,
    delay_bits: AtomicU32,
    lvl_in: AtomicU32,
    lvl_out: AtomicU32,
    stages: AtomicU8,
    nr_bits: AtomicU32,
    last_rx_ms: AtomicU64,
    load_bits: AtomicU32,
    fell_back: AtomicU32,
    idle: AtomicBool,
}

pub struct Engine {
    started: Instant,
    shared: Arc<Shared>,
    jb: Arc<Mutex<JitterBuffer>>,
    threads: Vec<JoinHandle<()>>,
    pub local_addr: SocketAddr,
}

fn preset_id(p: Preset) -> u8 {
    match p { Preset::Natural => 0, Preset::Discord => 1, Preset::Podcast => 2 }
}
fn preset_from(id: u8) -> Preset {
    match id { 0 => Preset::Natural, 2 => Preset::Podcast, _ => Preset::Discord }
}

fn make_denoiser(kind: DenoiseKind) -> Box<dyn Denoiser> {
    match kind {
        DenoiseKind::None => Box::new(Passthrough),
        #[cfg(feature = "dpdfnet")]
        DenoiseKind::Dpdfnet(path) => match clearmic_dsp::denoise::DpdfNet::from_file(&path) {
            Ok(d) => Box::new(d),
            Err(e) => {
                eprintln!("DPDFNet failed to load ({e}); running without noise removal");
                Box::new(Passthrough)
            }
        },
        #[cfg(not(feature = "dpdfnet"))]
        DenoiseKind::Dpdfnet(_) => {
            eprintln!("built without the dpdfnet feature; running without noise removal");
            Box::new(Passthrough)
        }
        #[cfg(feature = "dfn3")]
        DenoiseKind::Dfn3 => match clearmic_dsp::denoise::DeepFilter::new() {
            Ok(d) => Box::new(d),
            Err(e) => {
                eprintln!("DeepFilterNet3 failed to load ({e}); running without noise removal");
                Box::new(Passthrough)
            }
        },
        #[cfg(not(feature = "dfn3"))]
        DenoiseKind::Dfn3 => {
            eprintln!("built without the dfn3 feature; running without noise removal");
            Box::new(Passthrough)
        }
    }
}

fn stage_bits(c: &ChainConfig) -> u8 {
    c.hpf as u8 | (c.gate as u8) << 1 | (c.eq as u8) << 2 | (c.comp as u8) << 3 | (c.deess as u8) << 4
}

fn config_from(bits: u8, strength: f32) -> ChainConfig {
    ChainConfig { strength, hpf: bits & 1 != 0, gate: bits & 2 != 0, eq: bits & 4 != 0, comp: bits & 8 != 0, deess: bits & 16 != 0 }
}

fn rms(x: &[f32]) -> f32 {
    (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
}

fn peak(x: &[f32]) -> f32 {
    x.iter().fold(0.0f32, |m, v| m.max(v.abs()))
}

impl Engine {
    pub fn start(cfg: EngineConfig, mut sink: Box<dyn Sink>) -> io::Result<Engine> {
        let sock = UdpSocket::bind(("0.0.0.0", cfg.port))?;
        sock.set_read_timeout(Some(Duration::from_millis(100)))?;
        let local_addr = sock.local_addr()?;

        let shared = Arc::new(Shared::default());
        shared.preset.store(preset_id(cfg.preset), Relaxed);
        let initial = cfg.chain_override.unwrap_or_else(|| ChainConfig::preset(cfg.preset));
        shared.strength_bits.store(initial.strength.to_bits(), Relaxed);
        shared.stages.store(stage_bits(&initial), Relaxed);
        shared.last_rx_ms.store(u64::MAX, Relaxed);
        shared.idle.store(true, Relaxed);
        let started = Instant::now();
        let jb = Arc::new(Mutex::new(JitterBuffer::new(3, 8)));

        // network thread
        let rx = {
            let (shared, jb) = (shared.clone(), jb.clone());
            let cipher = Cipher::new(&cfg.key);
            thread::spawn(move || {
                crate::os::boost_audio_thread();
                let mut buf = [0u8; 2048];
                while !shared.stop.load(Relaxed) {
                    let n = match sock.recv_from(&mut buf) {
                        Ok((n, _)) => n,
                        Err(e) if matches!(e.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut) => continue,
                        Err(_) => break,
                    };
                    match cipher.open(&buf[..n]) {
                        Ok((h, payload)) => {
                            shared.last_rx_ms.store(started.elapsed().as_millis() as u64, Relaxed);
                            let mut data = Vec::with_capacity(payload.len() + 1);
                            data.push(h.flags);
                            data.extend_from_slice(&payload);
                            jb.lock().unwrap().push(h.seq, data);
                        }
                        Err(_) => {
                            shared.rejected.fetch_add(1, Relaxed);
                        }
                    }
                }
            })
        };

        // playout thread: one frame every 10 ms, and nothing at all while no phone is sending
        let play = {
            let (shared, jb) = (shared.clone(), jb.clone());
            let cfg = cfg.clone();
            thread::spawn(move || {
                crate::os::boost_audio_thread();
                let mut ladder = vec![cfg.denoise.clone()];
                ladder.extend(cfg.fallbacks.clone());
                let mut rung = 0usize;
                let mut chain = Chain::new(make_denoiser(ladder[0].clone()), cfg.chain_override.unwrap_or_else(|| ChainConfig::preset(cfg.preset)));
                let mut dec = Decoder::new();
                let mut frame = [0f32; FRAME_SAMPLES];
                let mut nr = 0f32;
                let mut load = 0f32; // smoothed processing time in ms
                let mut overloaded_ticks = 0u32;
                let mut applied_preset = preset_id(cfg.preset);
                let period = Duration::from_millis(10);
                let mut next = Instant::now();
                let mut was_idle = true;
                while !shared.stop.load(Relaxed) {
                    // No phone for two seconds: sleep, so the app costs nothing while a game runs.
                    let age = match shared.last_rx_ms.load(Relaxed) {
                        u64::MAX => None,
                        t => Some((started.elapsed().as_millis() as u64).saturating_sub(t)),
                    };
                    if age.map_or(true, |a| a > 2000) {
                        if !was_idle {
                            was_idle = true;
                            chain.reset();
                            dec = Decoder::new();
                            load = 0.0;
                            for a in [&shared.lvl_in, &shared.lvl_out, &shared.load_bits] {
                                a.store(0f32.to_bits(), Relaxed);
                            }
                            shared.idle.store(true, Relaxed);
                        }
                        thread::sleep(Duration::from_millis(100));
                        continue;
                    }
                    if was_idle {
                        was_idle = false;
                        shared.idle.store(false, Relaxed);
                        next = Instant::now();
                    }

                    next += period;
                    if let Some(wait) = next.checked_duration_since(Instant::now()) {
                        thread::sleep(wait);
                    }
                    let t_work = Instant::now();

                    // controls from the UI
                    let pid = shared.preset.load(Relaxed);
                    if pid != applied_preset {
                        let c = ChainConfig::preset(preset_from(pid));
                        shared.stages.store(stage_bits(&c), Relaxed);
                        shared.strength_bits.store(c.strength.to_bits(), Relaxed);
                        applied_preset = pid;
                    }
                    let want = config_from(shared.stages.load(Relaxed), f32::from_bits(shared.strength_bits.load(Relaxed)));
                    if want != chain.config() {
                        chain.set_config(want);
                    }
                    chain.set_bypass(shared.bypass.load(Relaxed));

                    let (slot, target) = {
                        let mut j = jb.lock().unwrap();
                        (j.pop(), j.target())
                    };
                    match slot {
                        Slot::Packet(data) => { dec.decode(&data, &mut frame); }
                        Slot::Lost | Slot::Waiting => { dec.conceal(&mut frame); }
                    }
                    shared.lvl_in.store(peak(&frame).to_bits(), Relaxed);
                    let r_in = rms(&frame);
                    chain.process(&mut frame);
                    if r_in > 1e-4 {
                        let d = 20.0 * (r_in / rms(&frame).max(1e-6)).log10();
                        nr = 0.98 * nr + 0.02 * d.clamp(-6.0, 60.0);
                        shared.nr_bits.store(nr.to_bits(), Relaxed);
                    }
                    shared.lvl_out.store(peak(&frame).to_bits(), Relaxed);
                    sink.write(&frame);

                    // load, and the overload guard: if a frame takes most of its 10 ms for two seconds,
                    // move to a lighter noise remover instead of letting the audio break up
                    let ms = t_work.elapsed().as_secs_f32() * 1000.0;
                    load = 0.98 * load + 0.02 * ms;
                    shared.load_bits.store((load * 10.0).to_bits(), Relaxed);
                    overloaded_ticks = if load > 7.0 { overloaded_ticks + 1 } else { 0 };
                    if overloaded_ticks > 200 && rung + 1 < ladder.len() {
                        rung += 1;
                        chain.set_denoiser(make_denoiser(ladder[rung].clone()));
                        shared.fell_back.store(rung as u32, Relaxed);
                        overloaded_ticks = 0;
                        load = 0.0;
                        next = Instant::now();
                    }

                    shared.played.fetch_add(1, Relaxed);
                    shared.cushion.store(target as u32, Relaxed);
                    shared.delay_bits.store((target as f32 * 10.0 + chain.latency_ms() + 10.0).to_bits(), Relaxed);
                }
                sink.finish();
            })
        };

        Ok(Engine { started, shared, jb, threads: vec![rx, play], local_addr })
    }

    pub fn snapshot(&self) -> Snapshot {
        let s = self.jb.lock().unwrap().stats;
        let f = |a: &AtomicU32| f32::from_bits(a.load(Relaxed));
        Snapshot {
            received: s.received,
            lost: s.lost,
            late: s.late,
            underruns: s.underruns,
            rejected: self.shared.rejected.load(Relaxed),
            frames_played: self.shared.played.load(Relaxed),
            cushion_packets: self.shared.cushion.load(Relaxed),
            delay_ms: f(&self.shared.delay_bits),
            level_in: f(&self.shared.lvl_in),
            level_out: f(&self.shared.lvl_out),
            nr_db: f(&self.shared.nr_bits),
            packet_age_ms: match self.shared.last_rx_ms.load(Relaxed) {
                u64::MAX => None,
                t => Some((self.started.elapsed().as_millis() as u64).saturating_sub(t)),
            },
            load_pct: f(&self.shared.load_bits), // stored as milliseconds x 10, which is percent of a 10 ms frame
            fell_back: self.shared.fell_back.load(Relaxed),
            idle: self.shared.idle.load(Relaxed),
        }
    }

    pub fn set_bypass(&self, on: bool) { self.shared.bypass.store(on, Relaxed); }
    pub fn set_preset(&self, p: Preset) { self.shared.preset.store(preset_id(p), Relaxed); }
    /// Turns the voice-chain stages on or off while running (the strength field is ignored here).
    pub fn set_stages(&self, c: ChainConfig) { self.shared.stages.store(stage_bits(&c), Relaxed); }
    pub fn stages(&self) -> ChainConfig {
        config_from(self.shared.stages.load(Relaxed), f32::from_bits(self.shared.strength_bits.load(Relaxed)))
    }
    pub fn set_strength(&self, s: f32) { self.shared.strength_bits.store(s.clamp(0.0, 1.0).to_bits(), Relaxed); }

    /// Stops both threads, flushes the sink and returns the final numbers.
    pub fn stop(mut self) -> Snapshot {
        self.shared.stop.store(true, Relaxed);
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
        self.snapshot()
    }
}
