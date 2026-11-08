//! Plays a 2 second beep through the same speaker path the app uses for "Hear my voice".
use clearmic_engine::{MonitorGate, MonitorSink, Sink};
use std::sync::{atomic::Ordering::Relaxed, Arc};
use std::time::{Duration, Instant};

fn main() {
    println!("outputs: {:?}", clearmic_engine::CpalSink::list_outputs());
    let gate = Arc::new(MonitorGate::default());
    gate.switch.store(true, Relaxed);
    let mut sink = MonitorSink::new(gate);
    let mut next = Instant::now();
    for n in 0..200u32 {
        let frame: Vec<f32> = (0..480).map(|i| 0.2 * (2.0 * std::f32::consts::PI * 440.0 * (n * 480 + i) as f32 / 48_000.0).sin()).collect();
        sink.write(&frame);
        next += Duration::from_millis(10);
        std::thread::sleep(next.saturating_duration_since(Instant::now()));
    }
    println!("done");
}
