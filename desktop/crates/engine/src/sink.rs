use std::io::BufWriter;
use std::fs::File;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

/// Streaming linear resampler from 48 kHz to the device rate (cables are often set to 44.1 kHz).
pub struct Resampler {
    step: f64, // input samples consumed per output sample
    pos: f64,  // position inside the current input pair
    prev: f32,
}

impl Resampler {
    pub fn new(to_rate: u32) -> Self {
        Self { step: 48_000.0 / to_rate as f64, pos: 0.0, prev: 0.0 }
    }

    pub fn process(&mut self, input: &[f32], out: &mut Vec<f32>) {
        for &x in input {
            while self.pos < 1.0 {
                out.push(self.prev + (x - self.prev) * self.pos as f32);
                self.pos += self.step;
            }
            self.pos -= 1.0;
            self.prev = x;
        }
    }
}

pub trait Sink: Send {
    fn write(&mut self, frame: &[f32]);
    fn finish(&mut self) {}
}

/// Plays nothing. The receiver still runs, so the phone connects and the window shows levels.
pub struct NullSink;

impl Sink for NullSink {
    fn write(&mut self, _frame: &[f32]) {}
}

/// When the voice is played on this computer: the "hear my voice" switch, or while the compare button is held.
#[derive(Default)]
pub struct MonitorGate {
    pub switch: AtomicBool,
    pub hold: AtomicBool,
    /// What the speakers are doing, for the window: which device is playing, or why it could not open.
    pub info: Mutex<String>,
}

/// The computer's speakers or headphones, opened only the first time they are wanted, and fed only while the gate is on.
pub struct MonitorSink {
    gate: Arc<MonitorGate>,
    sink: Option<CpalSink>,
    failed: bool,
}

impl MonitorSink {
    pub fn new(gate: Arc<MonitorGate>) -> Self {
        Self { gate, sink: None, failed: false }
    }
}

impl Sink for MonitorSink {
    fn write(&mut self, frame: &[f32]) {
        if !(self.gate.switch.load(Relaxed) || self.gate.hold.load(Relaxed)) {
            return;
        }
        if self.sink.is_none() && !self.failed {
            match CpalSink::open(None) {
                Ok(s) => {
                    *self.gate.info.lock().unwrap() = format!("Playing on {}", s.device_name);
                    self.sink = Some(s);
                }
                Err(e) => {
                    *self.gate.info.lock().unwrap() = format!("The speakers could not be opened: {e}");
                    self.failed = true;
                }
            }
        }
        if let Some(s) = &mut self.sink {
            s.write(frame);
        }
    }
}

/// Sends every frame to each sink, for example the virtual cable and the speakers at once.
pub struct TeeSink(pub Vec<Box<dyn Sink>>);

impl Sink for TeeSink {
    fn write(&mut self, frame: &[f32]) {
        for s in &mut self.0 {
            s.write(frame);
        }
    }
    fn finish(&mut self) {
        for s in &mut self.0 {
            s.finish();
        }
    }
}

/// Writes the processed audio to a 48 kHz mono 16-bit wav (testing, A/B).
pub struct WavSink {
    w: Option<hound::WavWriter<BufWriter<File>>>,
}

impl WavSink {
    pub fn create(path: &str) -> Result<Self, String> {
        let spec = hound::WavSpec {
            channels: 1, sample_rate: 48_000, bits_per_sample: 16, sample_format: hound::SampleFormat::Int,
        };
        let w = hound::WavWriter::create(path, spec).map_err(|e| e.to_string())?;
        Ok(Self { w: Some(w) })
    }
}

impl Sink for WavSink {
    fn write(&mut self, frame: &[f32]) {
        if let Some(w) = self.w.as_mut() {
            for s in frame {
                let _ = w.write_sample((s.clamp(-1.0, 1.0) * 32767.0) as i16);
            }
        }
    }
    fn finish(&mut self) {
        if let Some(w) = self.w.take() {
            let _ = w.finalize();
        }
    }
}

/// Plays into an output device, for example the virtual cable ("CABLE Input").
/// The cpal stream lives on its own thread (streams are not `Send` everywhere);
/// this struct only holds the ring buffer's producer side.
pub struct CpalSink {
    tx: rtrb::Producer<f32>,
    _stop: mpsc::Sender<()>,
    pub device_name: String,
    pub dropped: u64,
    resampler: Option<Resampler>,
    scratch: Vec<f32>,
}

impl CpalSink {
    /// `hint`: part of the device name, case-insensitive. None picks the default output.
    pub fn open(hint: Option<&str>) -> Result<Self, String> {
        let hint = hint.map(|s| s.to_lowercase());
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(rtrb::Producer<f32>, String, u32), String>>();
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        thread::spawn(move || {
            match Self::build(hint.as_deref()) {
                Ok((stream, prod, name, rate)) => {
                    let _ = ready_tx.send(Ok((prod, name, rate)));
                    let _keep = stream;
                    let _ = stop_rx.recv(); // returns when the sink is dropped
                }
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                }
            }
        });
        let (tx, device_name, rate) = ready_rx.recv().map_err(|e| e.to_string())??;
        let resampler = (rate != 48_000).then(|| Resampler::new(rate));
        Ok(Self { tx, _stop: stop_tx, device_name, dropped: 0, resampler, scratch: Vec::with_capacity(1024) })
    }

    pub fn list_outputs() -> Vec<String> {
        use cpal::traits::{DeviceTrait, HostTrait};
        cpal::default_host()
            .output_devices()
            .map(|d| d.filter_map(|d| d.name().ok()).collect())
            .unwrap_or_default()
    }

    fn build(hint: Option<&str>) -> Result<(cpal::Stream, rtrb::Producer<f32>, String, u32), String> {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
        let host = cpal::default_host();
        let device = match hint {
            Some(h) => host
                .output_devices()
                .map_err(|e| e.to_string())?
                .find(|d| d.name().map(|n| n.to_lowercase().contains(h)).unwrap_or(false))
                .ok_or_else(|| format!("no output device matching '{h}'"))?,
            None => host.default_output_device().ok_or("no default output device")?,
        };
        let name = device.name().unwrap_or_default();
        // Prefer 48 kHz; otherwise take the device's own rate and resample.
        let ranges: Vec<_> = device
            .supported_output_configs()
            .map_err(|e| e.to_string())?
            .filter(|c| c.sample_format() == cpal::SampleFormat::F32)
            .collect();
        let config = match ranges.iter().find(|c| c.min_sample_rate().0 <= 48_000 && c.max_sample_rate().0 >= 48_000) {
            Some(r) => r.clone().with_sample_rate(cpal::SampleRate(48_000)),
            None => ranges
                .into_iter()
                .next()
                .ok_or("the output device has no f32 format")?
                .with_max_sample_rate(),
        };
        let rate = config.sample_rate().0;
        let channels = config.channels() as usize;
        let (prod, mut cons) = rtrb::RingBuffer::<f32>::new(48_000 / 4);
        let stream = device
            .build_output_stream(
                &config.into(),
                move |data: &mut [f32], _| {
                    for frame in data.chunks_mut(channels) {
                        let s = cons.pop().unwrap_or(0.0);
                        frame.iter_mut().for_each(|o| *o = s);
                    }
                },
                |e| eprintln!("audio output error: {e}"),
                None,
            )
            .map_err(|e| e.to_string())?;
        stream.play().map_err(|e| e.to_string())?;
        Ok((stream, prod, name, rate))
    }
}

impl Sink for CpalSink {
    fn write(&mut self, frame: &[f32]) {
        let data: &[f32] = match self.resampler.as_mut() {
            Some(r) => {
                self.scratch.clear();
                r.process(frame, &mut self.scratch);
                &self.scratch
            }
            None => frame,
        };
        // Keep at most about 100 ms queued: if the device runs slow, drop instead of lagging.
        if self.tx.slots() < data.len() || self.tx.buffer().capacity() - self.tx.slots() > 4800 {
            self.dropped += 1;
            return;
        }
        for s in data {
            let _ = self.tx.push(*s);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resamples_48k_to_44k1_keeping_pitch_and_length() {
        let mut r = Resampler::new(44_100);
        let input: Vec<f32> = (0..48_000).map(|i| (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48_000.0).sin()).collect();
        let mut out = Vec::new();
        for chunk in input.chunks(480) {
            r.process(chunk, &mut out);
        }
        assert!((out.len() as i32 - 44_100).abs() <= 2, "length {}", out.len());
        // count rising zero crossings: a 1 kHz tone for 1 s has about 1000
        let zc = out.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count();
        assert!((zc as i32 - 1000).abs() <= 2, "zero crossings {zc}");
    }
}
