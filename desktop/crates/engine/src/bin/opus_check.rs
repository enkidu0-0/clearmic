//! Conformance check: decode an Ogg Opus file made by libopus (ffmpeg) with our decoder.
//!   opus_check in.opus out.wav     (the file must use 10 ms frames, mono, 48 kHz)
use sonivane_engine::codec::Decoder;
use sonivane_protocol::FRAME_SAMPLES;

fn packets(data: &[u8]) -> Vec<Vec<u8>> {
    let (mut out, mut cur, mut i) = (Vec::new(), Vec::new(), 0);
    while i + 27 <= data.len() && &data[i..i + 4] == b"OggS" {
        let nseg = data[i + 26] as usize;
        let lacing = &data[i + 27..i + 27 + nseg];
        let mut p = i + 27 + nseg;
        for &l in lacing {
            cur.extend_from_slice(&data[p..p + l as usize]);
            p += l as usize;
            if l < 255 {
                out.push(std::mem::take(&mut cur));
            }
        }
        i = p;
    }
    out
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let pk = packets(&std::fs::read(&a[1]).unwrap());
    let spec = hound::WavSpec { channels: 1, sample_rate: 48_000, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
    let mut w = hound::WavWriter::create(&a[2], spec).unwrap();
    let mut dec = Decoder::new();
    let mut f = [0f32; FRAME_SAMPLES];
    let mut bad = 0;
    for p in pk.iter().skip(2) {
        let mut data = vec![0u8]; // flags: opus
        data.extend_from_slice(p);
        if !dec.decode(&data, &mut f) { bad += 1; }
        for s in f { w.write_sample((s.clamp(-1.0, 1.0) * 32767.0) as i16).unwrap(); }
    }
    w.finalize().unwrap();
    eprintln!("{} packets, {} failed", pk.len() - 2, bad);
}
