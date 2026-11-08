//! Stands in for the phone: sends a 48 kHz mono wav as encrypted packets in real time.
//!   clearmic_sim --key <64 hex> --wav noisy.wav [--to 127.0.0.1:47100] [--loss 5] [--jitter 40]
use clearmic_protocol::{parse_key_hex, Cipher, FLAG_RAW16, FRAME_SAMPLES};
use std::net::UdpSocket;
use std::time::{Duration, Instant};

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let key = arg(&args, "--key").and_then(|k| parse_key_hex(&k)).expect("--key <64 hex>");
    let path = arg(&args, "--wav").expect("--wav <file>");
    let to = arg(&args, "--to").unwrap_or_else(|| "127.0.0.1:47100".into());
    let loss: f32 = arg(&args, "--loss").and_then(|v| v.parse().ok()).unwrap_or(0.0) / 100.0;
    let jitter_ms: f32 = arg(&args, "--jitter").and_then(|v| v.parse().ok()).unwrap_or(0.0);

    let mut rd = hound::WavReader::open(&path).expect("open wav");
    let spec = rd.spec();
    assert!(spec.sample_rate == 48_000 && spec.channels == 1, "need 48 kHz mono wav, got {spec:?}");
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => rd.samples::<f32>().map(|s| s.unwrap()).collect(),
        hound::SampleFormat::Int => {
            let m = (1i64 << (spec.bits_per_sample - 1)) as f32;
            rd.samples::<i32>().map(|s| s.unwrap() as f32 / m).collect()
        }
    };

    let cipher = Cipher::new(&key);
    let mut rng = Rng(0x9E3779B97F4A7C15);
    // (send time in ms from start, datagram)
    let mut plan: Vec<(f32, Vec<u8>)> = Vec::new();
    for (seq, chunk) in samples.chunks_exact(FRAME_SAMPLES).enumerate() {
        if rng.next() < loss {
            continue;
        }
        let mut pcm = Vec::with_capacity(FRAME_SAMPLES * 2);
        for s in chunk {
            pcm.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
        }
        let t = seq as f32 * 10.0 + rng.next() * jitter_ms;
        plan.push((t, cipher.seal(FLAG_RAW16, seq as u32, (seq as u64) * 10_000, &pcm)));
    }
    plan.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

    let sock = UdpSocket::bind("0.0.0.0:0").unwrap();
    let t0 = Instant::now();
    let n = plan.len();
    for (t, d) in plan {
        let due = Duration::from_secs_f32(t / 1000.0);
        if let Some(w) = due.checked_sub(t0.elapsed()) {
            std::thread::sleep(w);
        }
        sock.send_to(&d, &to).unwrap();
    }
    eprintln!("sent {n} packets in {:.1} s", t0.elapsed().as_secs_f32());
}
