//! Console receiver. Example (writes a wav; add --device "cable" to play into a device):
//!   clearmic_rx --key <64 hex> --wav out.wav --seconds 15 --denoise dfn3 --preset discord
use clearmic_dsp::{ChainConfig, Preset};
use clearmic_engine::{CpalSink, DenoiseKind, Engine, EngineConfig, Sink, WavSink};
use clearmic_protocol::parse_key_hex;
use std::time::{Duration, Instant};

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--list-devices") {
        CpalSink::list_outputs().iter().for_each(|d| println!("{d}"));
        return;
    }
    let key = arg(&args, "--key").and_then(|k| parse_key_hex(&k)).unwrap_or_else(|| {
        eprintln!("--key <64 hex characters> is required");
        std::process::exit(2);
    });
    let port: u16 = arg(&args, "--port").and_then(|p| p.parse().ok()).unwrap_or(47100);
    let preset = match arg(&args, "--preset").as_deref() {
        Some("natural") => Preset::Natural,
        Some("podcast") => Preset::Podcast,
        _ => Preset::Discord,
    };
    let denoise = match arg(&args, "--denoise").as_deref() {
        Some("dfn3") => DenoiseKind::Dfn3,
        Some("dpdfnet") => DenoiseKind::Dpdfnet(arg(&args, "--model").unwrap_or_else(|| {
            eprintln!("--denoise dpdfnet needs --model <path to dpdfnet2_48khz_hr.onnx>");
            std::process::exit(2);
        })),
        _ => DenoiseKind::None,
    };
    // --stages hpf,gate,eq,comp,deess  (with --strength 0..1) overrides the preset
    let chain_override = arg(&args, "--stages").map(|st| {
        let on = |n: &str| st.split(',').any(|x| x == n);
        ChainConfig {
            strength: arg(&args, "--strength").and_then(|v| v.parse().ok()).unwrap_or(0.8),
            hpf: on("hpf"), gate: on("gate"), eq: on("eq"), comp: on("comp"), deess: on("deess"),
        }
    });
    let seconds: Option<f32> = arg(&args, "--seconds").and_then(|s| s.parse().ok());

    let sink: Box<dyn Sink> = if let Some(path) = arg(&args, "--wav") {
        Box::new(WavSink::create(&path).unwrap_or_else(|e| { eprintln!("{e}"); std::process::exit(1) }))
    } else {
        let s = CpalSink::open(arg(&args, "--device").as_deref()).unwrap_or_else(|e| { eprintln!("{e}"); std::process::exit(1) });
        eprintln!("playing to: {}", s.device_name);
        Box::new(s)
    };

    let eng = Engine::start(EngineConfig { port, key, preset, denoise, fallbacks: vec![], chain_override }, sink).unwrap_or_else(|e| {
        eprintln!("cannot listen on port {port}: {e}");
        std::process::exit(1);
    });
    eprintln!("listening on {}", eng.local_addr);

    let t0 = Instant::now();
    loop {
        std::thread::sleep(Duration::from_millis(1000));
        let s = eng.snapshot();
        eprintln!(
            "recv {} lost {} late {} underrun {} bad {} | cushion {} delay {:.0} ms | in {:.2} out {:.2} | load {:.1}% idle {}",
            s.received, s.lost, s.late, s.underruns, s.rejected, s.cushion_packets, s.delay_ms, s.level_in, s.level_out, s.load_pct, s.idle
        );
        if seconds.map_or(false, |s| t0.elapsed().as_secs_f32() >= s) {
            break;
        }
    }
    let f = eng.stop();
    eprintln!("done: {} frames played, {} packets received, {} lost", f.frames_played, f.received, f.lost);
}
