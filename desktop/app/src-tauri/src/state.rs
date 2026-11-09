//! App state: the config on disk, the running engine, and everything the UI asks for.

use crate::{Levels, Pairing, Params, Status};
use clearmic_dsp::{ChainConfig, Preset};
use clearmic_engine::{CpalSink, DenoiseKind, Engine, EngineConfig, MonitorGate, MonitorSink, NullSink, Sink, TeeSink, WavSink};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::net::UdpSocket;
use std::path::PathBuf;
use std::sync::{atomic::Ordering::Relaxed, Arc, Mutex};

const PORT: u16 = 47100;

/// How much of the PC the noise remover may use, lightest first. The names are what the window shows.
/// The 16 kHz models cost a fraction of the 48 kHz one (measured: 0.11, 0.42 and 0.63 of real time on one core).
const MODES: [(&str, &str); 3] = [
    ("efficient", "baseline.onnx"),
    ("balanced", "dpdfnet2.onnx"),
    ("quality", "dpdfnet2_48khz_hr.onnx"),
];

fn default_perf() -> String {
    "efficient".into()
}

#[derive(Serialize, Deserialize, Clone)]
struct Config {
    key_hex: String,
    port: u16,
    /// Part of an output device's name, or "wav:<path>" to record to a file (testing).
    output: Option<String>,
    /// Also play the voice on this computer's speakers, to hear yourself. Off by default.
    #[serde(default)]
    monitor: bool,
    preset: String,
    /// "efficient", "balanced", "quality" or "off"
    #[serde(default = "default_perf", alias = "engine")]
    performance: String,
}

struct Inner {
    cfg: Config,
    engine: Option<Engine>,
    error: Option<String>,
    /// Shown beside the output setting when sound is not going to a virtual cable.
    notice: Option<String>,
    /// Chain settings kept across engine restarts.
    stages: Option<ChainConfig>,
}

pub struct Shared {
    dir: PathBuf,
    models: Option<PathBuf>,
    inner: Mutex<Inner>,
    ip: Mutex<Option<(std::time::Instant, String)>>,
    /// Whether the voice is played on this computer (the switch, or the compare button held down).
    monitor: Arc<MonitorGate>,
}

fn random_key_hex() -> String {
    let mut k = [0u8; 32];
    getrandom::getrandom(&mut k).expect("system random");
    k.iter().map(|b| format!("{b:02x}")).collect()
}

/// The address a phone on the same network can reach. The UDP "connect" sends nothing; it only picks a route.
fn lan_ip() -> String {
    UdpSocket::bind("0.0.0.0:0")
        .and_then(|s| s.connect("192.0.2.1:9").map(|_| s))
        .and_then(|s| s.local_addr())
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|_| "127.0.0.1".into())
}

fn computer_name() -> String {
    let n = std::env::var("COMPUTERNAME").or_else(|_| std::env::var("HOSTNAME")).unwrap_or_else(|_| "my pc".into());
    n.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).collect()
}

fn preset_of(s: &str) -> Preset {
    match s { "natural" => Preset::Natural, "podcast" => Preset::Podcast, _ => Preset::Discord }
}

impl Shared {
    pub fn load(dir: PathBuf, models: Option<PathBuf>) -> Shared {
        let path = dir.join("config.json");
        let cfg = std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice::<Config>(&b).ok())
            .unwrap_or_else(|| Config {
                key_hex: random_key_hex(),
                port: PORT,
                output: None,
                monitor: false,
                preset: "discord".into(),
                performance: default_perf(),
            });
        let monitor = Arc::new(MonitorGate::default());
        monitor.switch.store(cfg.monitor, Relaxed);
        let s = Shared { monitor, dir, models, inner: Mutex::new(Inner { cfg, engine: None, error: None, notice: None, stages: None }), ip: Mutex::new(None) };
        s.save();
        s
    }

    fn save(&self) {
        let i = self.inner.lock().unwrap();
        let _ = std::fs::create_dir_all(&self.dir);
        let _ = std::fs::write(self.dir.join("config.json"), serde_json::to_vec_pretty(&i.cfg).unwrap());
    }

    fn model_path(&self, file: &str) -> Option<String> {
        let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../models");
        [self.models.clone(), Some(dev)]
            .into_iter()
            .flatten()
            .map(|d| d.join(file))
            .find(|p| p.is_file())
            .map(|p| p.to_string_lossy().into_owned())
    }

    /// The chosen noise remover first, then every lighter one, then none.
    fn denoise_ladder(&self, performance: &str) -> Result<(DenoiseKind, Vec<DenoiseKind>), String> {
        let Some(top) = MODES.iter().position(|(n, _)| *n == performance) else {
            return Ok((DenoiseKind::None, vec![]));
        };
        let mut ladder = Vec::new();
        for (_, file) in MODES[..=top].iter().rev() {
            match self.model_path(file) {
                Some(p) => ladder.push(DenoiseKind::Dpdfnet(p)),
                None if ladder.is_empty() => return Err(format!("noise removal model {file} is missing")),
                None => {}
            }
        }
        ladder.push(DenoiseKind::None);
        let first = ladder.remove(0);
        Ok((first, ladder))
    }

    /// The LAN address, looked up at most every 30 seconds.
    fn cached_ip(&self) -> String {
        let mut c = self.ip.lock().unwrap();
        match &*c {
            Some((t, ip)) if t.elapsed().as_secs() < 30 => ip.clone(),
            _ => {
                let ip = lan_ip();
                *c = Some((std::time::Instant::now(), ip.clone()));
                ip
            }
        }
    }

    /// The virtual cable if one is installed, otherwise nothing: playing a mic to the speakers would echo.
    fn default_output() -> Option<String> {
        let all = Self::cable_outputs();
        all.iter().find(|n| n.to_lowercase().contains("cable input")).or(all.first()).cloned()
    }

    /// Only virtual cables may carry the mic sound: a real speaker here would play your own voice back, whatever the
    /// "hear my voice" switch says. Hearing yourself has its own switch.
    fn is_virtual(name: &str) -> bool {
        let n = name.to_lowercase();
        ["cable", "vb-audio", "blackhole", "voicemeeter", "virtual", "loopback", "soundflower"].iter().any(|k| n.contains(k))
    }

    fn cable_outputs() -> Vec<String> {
        CpalSink::list_outputs().into_iter().filter(|n| Self::is_virtual(n)).collect()
    }

    /// (Re)starts the receiver with the current settings.
    pub fn restart(&self) -> Result<(), String> {
        let (cfg, old_stages) = {
            let mut i = self.inner.lock().unwrap();
            if let Some(e) = i.engine.take() {
                i.stages = Some(e.stages());
                e.stop();
            }
            (i.cfg.clone(), i.stages)
        };

        let mut notice: Option<String> = None;
        let result = (|| -> Result<Engine, String> {
            let key = clearmic_protocol::parse_key_hex(&cfg.key_hex).ok_or("the pairing key is damaged, pair again")?;
            // The receiver must always listen, or the phone gets "connection refused". With no virtual cable the sound
            // goes nowhere, and the window says so.
            let mut speakers_refused = false;
            let saved = cfg.output.clone().filter(|o| {
                let ok = o.starts_with("wav:") || Self::is_virtual(o);
                speakers_refused = !ok;
                ok
            });
            let wanted = saved.or_else(|| std::env::var("CLEARMIC_OUTPUT").ok()).or_else(Self::default_output);
            let (mut sink, mut note): (Box<dyn Sink>, Option<String>) = match wanted {
                Some(o) if o.starts_with("wav:") => match WavSink::create(&o[4..]) {
                    Ok(w) => (Box::new(w), None),
                    Err(_) => (Box::new(NullSink), Some("the saved output file cannot be written".to_string())),
                },
                Some(o) => match CpalSink::open(Some(&o.to_lowercase())) {
                    Ok(c) => (Box::new(c), None),
                    Err(_) => (Box::new(NullSink), Some(format!("the output \"{o}\" was not found"))),
                },
                None => (Box::new(NullSink), Some("no virtual microphone cable found".to_string())),
            };
            if let Some(why) = &note {
                note = Some(format!("{why}, so other apps cannot hear ClearMic yet. Install a virtual cable (VB-Cable on Windows, BlackHole on Mac) and pick it under Output."));
            }
            // the speakers are always wired in, but silent until the switch is on or the compare button is held
            sink = Box::new(TeeSink(vec![sink, Box::new(MonitorSink::new(self.monitor.clone()))]));
            if speakers_refused && note.is_some() {
                note = Some("A real speaker was selected as the output, which would play your voice out loud. ClearMic ignored it. Pick a virtual cable (VB-Cable on Windows, BlackHole on Mac) under Output. Use \"Hear my voice\" to listen to yourself.".to_string());
            }
            notice = note;
            let (denoise, fallbacks) = self.denoise_ladder(&cfg.performance)?;
            let eng = Engine::start(
                EngineConfig { port: cfg.port, key, preset: preset_of(&cfg.preset), denoise, fallbacks, chain_override: old_stages },
                sink,
            )
            .map_err(|e| format!("cannot listen on port {}: {e}", cfg.port))?;
            Ok(eng)
        })();

        let mut i = self.inner.lock().unwrap();
        match result {
            Ok(e) => {
                i.engine = Some(e);
                i.error = None;
                i.notice = notice;
                Ok(())
            }
            Err(e) => {
                i.error = Some(e.clone());
                Err(e)
            }
        }
    }

    /// Stops the engine and flushes the output (a wav output is only valid after this).
    pub fn shutdown(&self) {
        let mut i = self.inner.lock().unwrap();
        if let Some(e) = i.engine.take() {
            e.stop();
        }
    }

    pub fn new_pairing(&self) -> Result<(), String> {
        self.inner.lock().unwrap().cfg.key_hex = random_key_hex();
        self.save();
        self.restart()
    }

    pub fn status(&self) -> Status {
        let i = self.inner.lock().unwrap();
        let addr = format!("{}:{}", self.cached_ip(), i.cfg.port);
        match &i.engine {
            None => Status { state: "error", error: i.error.clone(), notice: None, monitor_info: String::new(), delay_ms: 0, nr_db: 0, loss_pct: 0.0, addr, load_pct: 0, idle: true, mode: i.cfg.performance.clone(), stepped_down: false },
            Some(e) => {
                let s = e.snapshot();
                let total = (s.received + s.lost).max(1) as f32;
                let (state, error) = match s.packet_age_ms {
                    // Packets arrive but none opens: the phone holds another key (or an old code).
                    None if s.rejected > 0 => ("error", Some(format!("the phone is sending, but its pairing code does not match this PC ({} packets refused). Scan the code on the Phone tab again.", s.rejected))),
                    None => ("waiting", None),
                    Some(a) if a < 1500 => ("connected", None),
                    Some(_) => ("error", Some(format!("the phone stopped sending after {} packets. Keep the phone screen on, set ClearMic's battery use to unrestricted, and check the Wi-Fi.", s.received))),
                };
                Status {
                    state,
                    error,
                    notice: i.notice.clone(),
                    monitor_info: self.monitor.info.lock().unwrap().clone(),
                    delay_ms: s.delay_ms.round() as u32,
                    nr_db: s.nr_db.round() as i32,
                    loss_pct: (s.lost as f32 / total * 1000.0).round() / 10.0,
                    addr,
                    load_pct: s.load_pct.round() as u32,
                    idle: s.idle,
                    mode: i.cfg.performance.clone(),
                    stepped_down: s.fell_back > 0,
                }
            }
        }
    }

    pub fn levels(&self) -> Levels {
        match &self.inner.lock().unwrap().engine {
            Some(e) => {
                let s = e.snapshot();
                Levels { input: vec![s.level_in], out: vec![s.level_out] }
            }
            None => Levels { input: vec![0.0], out: vec![0.0] },
        }
    }

    pub fn pairing(&self) -> Result<Pairing, String> {
        let i = self.inner.lock().unwrap();
        let addr = format!("{}:{}", self.cached_ip(), i.cfg.port);
        let uri = format!("clearmic://{addr}?key={}&name={}", i.cfg.key_hex, computer_name());
        let svg = qrcode::QrCode::new(uri.as_bytes())
            .map_err(|e| e.to_string())?
            .render::<qrcode::render::svg::Color>()
            .min_dimensions(220, 220)
            .quiet_zone(true)
            .dark_color(qrcode::render::svg::Color("#0b0a14"))
            .light_color(qrcode::render::svg::Color("#ffffff"))
            .build();
        Ok(Pairing { uri, addr, key: i.cfg.key_hex.clone(), qr_svg: svg })
    }

    pub fn params(&self) -> Params {
        let i = self.inner.lock().unwrap();
        let s = i.engine.as_ref().map(|e| e.stages()).or(i.stages).unwrap_or_else(|| ChainConfig::preset(preset_of(&i.cfg.preset)));
        Params {
            strength: s.strength,
            preset: i.cfg.preset.clone(),
            performance: i.cfg.performance.clone(),
            autostart: crate::autostart::is_enabled(),
            autostart_supported: crate::autostart::SUPPORTED,
            hpf: s.hpf, gate: s.gate, eq: s.eq, comp: s.comp, deess: s.deess,
            monitor: i.cfg.monitor,
            output: i.cfg.output.clone().filter(|o| o.starts_with("wav:") || Self::is_virtual(o)).or_else(Self::default_output).unwrap_or_default(),
            outputs: Self::cable_outputs(),
        }
    }

    pub fn set_bypass(&self, on: bool) {
        self.monitor.hold.store(on, Relaxed);
        if let Some(e) = &self.inner.lock().unwrap().engine {
            e.set_bypass(on);
        }
    }

    pub fn set_param(&self, name: &str, value: Value) -> Result<(), String> {
        let needs_restart = {
            let mut i = self.inner.lock().unwrap();
            let on = value.as_bool();
            match name {
                "strength" => {
                    let v = value.as_f64().ok_or("strength must be a number")? as f32;
                    if let Some(e) = &i.engine { e.set_strength(v); }
                    false
                }
                "preset" => {
                    let p = value.as_str().ok_or("preset must be text")?.to_string();
                    if let Some(e) = &i.engine { e.set_preset(preset_of(&p)); }
                    i.cfg.preset = p;
                    false
                }
                "hpf" | "gate" | "eq" | "comp" | "deess" => {
                    let on = on.ok_or("switch must be true or false")?;
                    if let Some(e) = &i.engine {
                        let mut c = e.stages();
                        match name { "hpf" => c.hpf = on, "gate" => c.gate = on, "eq" => c.eq = on, "comp" => c.comp = on, _ => c.deess = on }
                        e.set_stages(c);
                    }
                    false
                }
                "performance" => {
                    let v = value.as_str().ok_or("performance must be text")?;
                    if v != "off" && !MODES.iter().any(|(n, _)| *n == v) {
                        return Err(format!("unknown performance mode '{v}'"));
                    }
                    i.cfg.performance = v.to_string();
                    true
                }
                "autostart" => {
                    let on = on.ok_or("autostart must be true or false")?;
                    crate::autostart::set(on)?;
                    false
                }
                "monitor" => {
                    i.cfg.monitor = on.ok_or("monitor must be true or false")?;
                    self.monitor.switch.store(i.cfg.monitor, Relaxed);
                    false
                }
                "output" => {
                    let v = value.as_str().ok_or("output must be text")?;
                    if !(v.starts_with("wav:") || Self::is_virtual(v)) {
                        return Err("only a virtual cable can be the output; use \"Hear my voice\" to listen".into());
                    }
                    i.cfg.output = Some(v.to_string());
                    true
                }
                _ => return Err(format!("unknown setting '{name}'")),
            }
        };
        self.save();
        if needs_restart { self.restart() } else { Ok(()) }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn only_virtual_cables_can_carry_the_mic() {
        for ok in ["CABLE Input (VB-Audio Virtual Cable)", "BlackHole 2ch", "Voicemeeter Input"] {
            assert!(Shared::is_virtual(ok), "{ok}");
        }
        for no in ["FxSound Speakers (FxSound Audio Enhancer)", "Speakers (Realtek Audio)", "MacBook Pro Speakers"] {
            assert!(!Shared::is_virtual(no), "{no}");
        }
    }

    use super::*;
    use clearmic_protocol::{Cipher, FLAG_RAW16, FRAME_SAMPLES};
    use std::time::{Duration, Instant};

    fn send(sock: &UdpSocket, cipher: &Cipher, from: u32, count: u32) {
        let t0 = Instant::now();
        for n in 0..count {
            let seq = from + n;
            let mut pcm = Vec::new();
            for i in 0..FRAME_SAMPLES {
                let t = (seq as usize * FRAME_SAMPLES + i) as f32 / 48_000.0;
                pcm.extend_from_slice(&(((t * 220.0 * 6.2832).sin() * 6000.0) as i16).to_le_bytes());
            }
            let d = cipher.seal(FLAG_RAW16, seq, 1_000_000 + seq as u64 * 10_000, &pcm);
            sock.send_to(&d, "127.0.0.1:47100").unwrap();
            let due = t0 + Duration::from_millis(10 * (n as u64 + 1));
            std::thread::sleep(due.saturating_duration_since(Instant::now()));
        }
    }

    #[test]
    fn app_state_end_to_end() {
        let dir = std::env::temp_dir().join(format!("clearmic-test-{}", std::process::id()));
        let wav = dir.join("out.wav");
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("CLEARMIC_OUTPUT", format!("wav:{}", wav.display()));
        let shared = Shared::load(dir.clone(), None);
        shared.restart().expect("engine starts");

        // pairing: uri carries the address and the key, and the qr is an svg
        let p = shared.pairing().unwrap();
        assert!(p.uri.starts_with("clearmic://") && p.uri.contains(&format!("key={}", p.key)));
        assert!(p.qr_svg.contains("<svg"));
        let key = clearmic_protocol::parse_key_hex(&p.key).unwrap();
        assert_eq!(shared.status().state, "waiting");

        // a phone sends: status turns connected, delay and levels appear
        let sock = UdpSocket::bind("0.0.0.0:0").unwrap();
        let cipher = Cipher::new(&key);
        send(&sock, &cipher, 0, 150);
        let st = shared.status();
        assert_eq!(st.state, "connected", "{:?}", st.error);
        assert!(st.delay_ms > 0 && st.loss_pct <= 100.0);
        assert!(shared.levels().input[0] > 0.0);

        // hear my voice: the speakers open the first time they are wanted, and the window says which device plays
        assert_eq!(shared.status().monitor_info, "", "speakers stay closed until wanted");
        shared.set_param("monitor", true.into()).unwrap();
        send(&sock, &cipher, 5_000, 40);
        let info = shared.status().monitor_info;
        assert!(info.starts_with("Playing on "), "monitor said: {info:?}");
        shared.set_param("monitor", false.into()).unwrap();

        // controls
        shared.set_param("preset", "podcast".into()).unwrap();
        std::thread::sleep(Duration::from_millis(60));
        assert!(shared.params().deess, "podcast preset turns the de-esser on");
        shared.set_param("hpf", false.into()).unwrap();
        std::thread::sleep(Duration::from_millis(60));
        assert!(!shared.params().hpf);
        shared.set_param("strength", 0.5.into()).unwrap();
        assert!(shared.set_param("nonsense", 1.into()).is_err());
        assert!(shared.set_param("strength", "loud".into()).is_err());
        shared.set_bypass(true);
        shared.set_bypass(false);

        // switching the engine off restarts the receiver and keeps the chain settings
        shared.set_param("performance", "off".into()).unwrap();
        assert_eq!(shared.params().performance, "off");
        assert!(!shared.params().hpf, "stage settings survive a restart");

        // the phone going quiet is an error after 1.5 s
        std::thread::sleep(Duration::from_millis(100));
        send(&sock, &cipher, 10_000, 30);
        assert_eq!(shared.status().state, "connected");
        std::thread::sleep(Duration::from_millis(1700));
        assert_eq!(shared.status().state, "error");

        // a new pairing code changes the key, and the old key is rejected
        shared.new_pairing().unwrap();
        let p2 = shared.pairing().unwrap();
        assert_ne!(p.key, p2.key);
        send(&sock, &cipher, 20_000, 20);
        let st = shared.status();
        assert_eq!(st.state, "error", "packets under the old key must not connect");
        assert!(st.error.unwrap().starts_with("the phone is sending"), "and the app says the code does not match");
        assert!(shared.status().idle, "with no phone the receiver sleeps");

        // the new key works, and the receiver wakes up again
        let cipher2 = Cipher::new(&clearmic_protocol::parse_key_hex(&p2.key).unwrap());
        send(&sock, &cipher2, 30_000, 40);
        assert_eq!(shared.status().state, "connected");
        assert!(!shared.status().idle);

        shared.shutdown();
        let n = hound::WavReader::open(&wav).unwrap().len();
        // each restart reopens (truncates) the file, so only the last engine's audio is left
        assert!(n > 0, "wav has {n} samples");
        let _ = std::fs::remove_dir_all(dir);
    }
}
