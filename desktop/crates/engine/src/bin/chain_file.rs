//! Offline check of the whole voice chain: wav in -> input gain -> chain with DPDFNet -> wav out.
//!   chain_file model.onnx in.wav out.wav input_gain_db [natural|discord|podcast]
//! Prints the loudness of the louder half of the frames, which is where the voice is.
#[cfg(feature = "dpdfnet")]
fn main() {
    use clearmic_dsp::{denoise::DpdfNet, Chain, ChainConfig, Preset};
    let a: Vec<String> = std::env::args().collect();
    let mut rd = hound::WavReader::open(&a[2]).unwrap();
    let m = (1i64 << (rd.spec().bits_per_sample - 1)) as f32;
    let g = 10f32.powf(a[4].parse::<f32>().unwrap() / 20.0);
    let x: Vec<f32> = rd.samples::<i32>().map(|s| s.unwrap() as f32 / m * g).collect();
    let preset = match a.get(5).map(String::as_str) { Some("natural") => Preset::Natural, Some("podcast") => Preset::Podcast, _ => Preset::Discord };
    let mut chain = Chain::new(Box::new(DpdfNet::from_file(&a[1]).unwrap()), ChainConfig::preset(preset));
    let spec = hound::WavSpec { channels: 1, sample_rate: 48_000, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
    let mut w = hound::WavWriter::create(&a[3], spec).unwrap();
    let mut levels = Vec::new();
    for ch in x.chunks_exact(480) {
        let mut f = ch.to_vec();
        chain.process(&mut f);
        levels.push(20.0 * ((f.iter().map(|v| v * v).sum::<f32>() / 480.0).sqrt() + 1e-9).log10());
        for s in &f { w.write_sample((s.clamp(-1.0, 1.0) * 32767.0) as i16).unwrap(); }
    }
    w.finalize().unwrap();
    levels.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let loud = &levels[levels.len() / 2..];
    eprintln!("input gain {} dB -> median level of the louder half: {:.1} dBFS", a[4], loud[loud.len() / 2]);
}
#[cfg(not(feature = "dpdfnet"))]
fn main() { eprintln!("build with --features dpdfnet"); }
