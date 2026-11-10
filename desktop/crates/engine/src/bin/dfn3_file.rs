//! Offline check of the denoiser alone: wav in -> DeepFilterNet3 (frame by frame) -> wav out.
//!   dfn3_file in.wav out.wav [strength 0..1]
#[cfg(feature = "dfn3")]
fn main() {
    use sonivane_dsp::{denoise::DeepFilter, Denoiser};
    let a: Vec<String> = std::env::args().collect();
    let mut rd = hound::WavReader::open(&a[1]).unwrap();
    let m = (1i64 << (rd.spec().bits_per_sample - 1)) as f32;
    let x: Vec<f32> = rd.samples::<i32>().map(|s| s.unwrap() as f32 / m).collect();
    let mut d = DeepFilter::new().unwrap();
    d.set_strength(a.get(3).and_then(|s| s.parse().ok()).unwrap_or(1.0));
    let spec = hound::WavSpec { channels: 1, sample_rate: 48_000, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
    let mut w = hound::WavWriter::create(&a[2], spec).unwrap();
    for ch in x.chunks_exact(480) {
        let mut f = ch.to_vec();
        d.process(&mut f);
        for s in f { w.write_sample((s.clamp(-1.0, 1.0) * 32767.0) as i16).unwrap(); }
    }
    w.finalize().unwrap();
}
#[cfg(not(feature = "dfn3"))]
fn main() { eprintln!("build with --features dfn3"); }
