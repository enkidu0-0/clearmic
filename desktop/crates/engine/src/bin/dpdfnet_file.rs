//! Offline check of the denoiser alone: wav in -> DPDFNet (frame by frame) -> wav out.
//!   dpdfnet_file model.onnx in.wav out.wav [strength 0..1]
#[cfg(feature = "dpdfnet")]
fn main() {
    use sonivane_dsp::{denoise::DpdfNet, Denoiser};
    let a: Vec<String> = std::env::args().collect();
    let mut rd = hound::WavReader::open(&a[2]).unwrap();
    let m = (1i64 << (rd.spec().bits_per_sample - 1)) as f32;
    let x: Vec<f32> = rd.samples::<i32>().map(|s| s.unwrap() as f32 / m).collect();
    let mut d = DpdfNet::from_file(&a[1]).unwrap();
    d.set_strength(a.get(4).and_then(|s| s.parse().ok()).unwrap_or(1.0));
    let spec = hound::WavSpec { channels: 1, sample_rate: 48_000, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
    let mut w = hound::WavWriter::create(&a[3], spec).unwrap();
    let t = std::time::Instant::now();
    let mut out = Vec::with_capacity(x.len());
    for ch in x.chunks_exact(480) {
        let mut f = ch.to_vec();
        d.process(&mut f);
        out.extend_from_slice(&f);
    }
    let rtf = t.elapsed().as_secs_f32() / (x.len() as f32 / 48000.0);
    // drop the algorithmic delay so the output lines up with the input
    let delay = (d.latency_ms() as usize - 10) * 48;
    for s in out.iter().skip(delay) { w.write_sample((s.clamp(-1.0, 1.0) * 32767.0) as i16).unwrap(); }
    w.finalize().unwrap();
    eprintln!("real-time factor {rtf:.3}");
}
#[cfg(not(feature = "dpdfnet"))]
fn main() { eprintln!("build with --features dpdfnet"); }
