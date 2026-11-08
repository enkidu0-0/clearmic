# ClearMic: plan for a WO Mic replacement with Krisp-style noise removal

Working name: **ClearMic**. The phone is the microphone, the PC shows it as a normal mic
("ClearMic Microphone") in Discord, OBS, Zoom and games, and the audio is cleaned
with an AI noise-removal model before it reaches those apps.

---

## 1. Why WO Mic sounds bad and what we do differently

| Problem in WO Mic | What ClearMic does |
|---|---|
| Sends raw phone mic audio, or uses Android's built-in `NoiseSuppressor`, which is weak and differs from phone to phone | Captures clean raw audio, then runs a **neural noise-suppression model** (DeepFilterNet3) |
| No vocal processing, so it sounds thin and "phone-y" | Adds a **podcast voice chain**: high-pass, EQ, compressor, de-esser, limiter |
| Jitter and dropouts on Wi-Fi | **Opus** codec with FEC (forward error correction), a jitter buffer and packet-loss concealment |
| Keyboard clicks and breaths between words | **Voice activity detection** (Silero VAD) drives a smooth gate |

### What "Krisp" and "Adobe Podcast" mean technically

- **Discord Krisp / NVIDIA Broadcast** are *real-time* neural noise suppression with about 20–40 ms delay.
  The best open-source equivalent found (Oct 2026) is **DPDFNet** by Ceva (Apache-2.0, 48 kHz, an improved DeepFilterNet).
  It beats **DeepFilterNet3** on every published metric, and DeepFilterNet3 stays as the low-latency fallback. See section 2.5.
- **Adobe Podcast Enhance** is *not* real-time. It regenerates or re-synthesizes the voice offline, which is why it sounds like a studio mic.
  You cannot get exactly that live. The realistic approach:
  1. **Live mode**: DeepFilterNet3 plus a podcast EQ and compressor chain gets you about 80–90% of the way.
  2. **Studio mode (optional, later)**: for *recordings* only, run a heavier generative enhancer offline
     (for example Resemble Enhance or VoiceFixer) on the saved file. This is the "Adobe-like" button.

---

## 2. The big decisions

### 2.1 Phone app: Kotlin or Flutter?

**Use Kotlin (Jetpack Compose UI) with a small C++ part through the NDK.**

| | Kotlin + NDK | Flutter |
|---|---|---|
| Low-latency audio capture (Oboe/AAudio) | Native, direct | Still needs a Kotlin/C++ plugin |
| Foreground service (keeps the mic running with the screen off) | Native | Plugin plus platform code |
| Real-time audio thread without GC pauses | Yes (C++) | Dart isolates are not real-time safe |
| iOS later | Separate Swift app | One UI codebase |

Audio code in Flutter ends up being Kotlin and C++ anyway, so Flutter only helps the UI.
**Choose Flutter only if iOS support is a hard requirement from day one.** In that case use
Flutter for the UI and settings and write the audio engine as a native plugin (Kotlin/C++ on Android, Swift on iOS).

### 2.2 Where does the noise removal run? On the PC.

| Option | Pros | Cons |
|---|---|---|
| **PC side (recommended)** | Same quality on every phone, can use the big DeepFilterNet3 model, no phone battery or heat cost, easy to update | PC uses about 3–8% of one CPU core |
| Phone side | PC app stays tiny | Cheap phones struggle with it, heat, battery drain |

Plan: **DPDFNet on the PC by default (DeepFilterNet3 for low-latency mode).** Add an optional "Phone denoise" toggle that runs a
tiny model (RNNoise or GTCRN) on the phone for weak PCs, or for people who use the phone
mic with other apps.

### 2.3 PC app: what language for the .exe?

**Use Rust, with Tauri for the UI.**
- DeepFilterNet's real-time engine (`libDF`, `DfTract`) is **written in Rust**, so there is no glue layer.
- Gives you a single small `.exe` (about 10–15 MB) with no .NET or Python runtime.
- `cpal` (WASAPI audio), `opus` / `audiopus` and `tokio` (networking) are all mature.
- Tauri gives a nice web-tech UI (React or Svelte) with a tiny binary. Use `egui` instead if you want pure Rust.

Alternative: C++ with JUCE. Fine too, but you would use DeepFilterNet through ONNX Runtime.

### 2.4 How the PC sees a "microphone" (the hard part)

Windows apps only list **audio devices that come from a driver**. You need a virtual audio device.

| Phase | Approach | Notes |
|---|---|---|
| **MVP** | Output to **VB-Audio Virtual Cable** ("CABLE Input"), and the user selects "CABLE Output" as their mic | Free for personal use. Bundling it in *your* installer needs a license from VB-Audio. For the MVP, ask the user to install it. |
| **v1.0** | **Your own virtual mic driver** based on Microsoft's **SysVAD** sample, or the MIT-licensed **VirtualDrivers/Virtual-Audio-Driver** project (which is itself based on SysVAD) | Must be signed. Needs an **EV code-signing certificate** (about $250–500 a year) and **Microsoft Partner Center attestation signing**. Without it, Windows refuses to load the driver unless test-signing mode is on. |

The PC app writes processed audio into the virtual device's "speaker" side, and apps read it from the "mic" side.
That is what WO Mic does with its own kernel driver.

### 2.5 Which noise-removal model? (research, Oct 2026)

**Benchmark 1: DPDFNet paper (arXiv 2512.16420), low-SNR multilingual test set.**
These are causal (real-time capable) models only. Higher is better, and the scores are the authors' own.

| Model | Params | GMACs | PESQ | SI-SNR | DNSMOS OVL | NISQA |
|---|---|---|---|---|---|---|
| RNNoise | 0.09M | 0.04 | 2.36 | 8.97 | 2.51 | 2.20 |
| GTCRN | 0.02M | 0.04 | 2.21 | 9.11 | 2.25 | 2.53 |
| DTLN | 0.99M | 0.12 | 2.14 | 10.83 | 2.15 | 2.13 |
| NSNet2 (Microsoft) | 2.6M | 0.26 | 2.35 | 8.63 | 2.36 | 2.63 |
| Demucs (Meta) | 33.5M | 7.7 | 2.57 | 12.36 | 2.69 | 2.70 |
| aTENNuate (BrainChip) | 0.8M | 0.33 | 2.80 | 9.65 | 2.74 | 3.90 |
| CleanUNet (NVIDIA) | 46M | 15.4 | 2.82 | 12.50 | 2.77 | 2.96 |
| DeepFilterNet3 | 2.1M | 0.35 | 2.76 | 12.10 | 2.72 | 3.99 |
| **DPDFNet-2** | 2.5M | 1.35 | 3.14 | 13.72 | 2.85 | 4.23 |
| **DPDFNet-4** | 2.8M | 2.36 | 3.18 | 14.11 | 2.87 | 4.30 |
| **DPDFNet-8** | 3.5M | 4.37 | **3.20** | **14.47** | **2.89** | **4.34** |

**Benchmark 2: HushMic wiki (3 demo clips, DNSMOS P.835 overall). It is informal but includes Krisp.**
DPDFNet 3.20 > DeepFilterNet3 2.97 > **Krisp 2.57** > GTCRN 2.42 > RNNoise 2.01 > raw input 1.49.

**Decision:**

| Where | Model | Why |
|---|---|---|
| **PC, default "Quality"** | **DPDFNet `dpdfnet2_48khz_hr`** (or `dpdfnet8_48khz_hr` on fast CPUs) | Best open-source quality. On these clips it even scores above Krisp. 48 kHz, Apache-2.0, ONNX and TFLite models on Hugging Face. |
| **PC, "Low latency" mode** | DeepFilterNet3 | About 40 ms instead of about 80 ms. 7x less compute. Native Rust (`libDF`). |
| **Phone, optional** | GTCRN or UL-UNAS (or RNNoise) | Tiny (about 0.04 GMACs). Good enough for weak PCs. |
| **Offline "Studio mode"** | ClearerVoice **MossFormer2_SE_48K** plus **Resemble Enhance** (MIT) | Not real-time. This is the open "Adobe Podcast" recipe that CrispVoice uses. |

**Reuse:** [HushMic](https://github.com/Fovty/HushMic) (MIT/Apache) is a Linux virtual mic built on DPDFNet.
Its `hushmic-denoiser` crate is **Rust** and wraps a fast native C engine that uses less than half the CPU of ONNX Runtime,
which fits our Rust PC app directly. Study it or reuse it before writing our own DPDFNet wrapper.

**Not chosen:**
- Demucs and CleanUNet are too heavy (8–15 GMACs) and score worse than DPDFNet.
- aTENNuate has good PESQ, but its repo is no longer reachable on GitHub (404 in Oct 2026).
- FRCRN and MossFormerGAN are 16 kHz only.
- Tencent AuK (1.5B parameters, Sept 2026) and VoiceFixer are generative and offline only. AuK is worth testing later for Studio mode.

**Before committing, run your own A/B test.** Record 10 noisy clips on your phone (fan, keyboard, street, TV, kids).
Run them through DPDFNet-2, DPDFNet-8, DeepFilterNet3 and RNNoise with `tools/bench`, then score with DNSMOS and listen blind.
Published numbers come from the authors' own test sets.

---

## 3. System architecture

```
┌──────────────────────── ANDROID PHONE (Kotlin + C++) ─────────────────────────┐
│                                                                               │
│  Mic ─► Oboe/AAudio capture (48 kHz, mono, float, 10 ms frames)               │
│          AudioSource.UNPROCESSED (fallback: VOICE_RECOGNITION)                │
│        ─► [optional] phone-side RNNoise/GTCRN                                 │
│        ─► Opus encoder (VOIP mode, 48 kHz, 10 ms, 64–96 kbps, in-band FEC)    │
│        ─► Packetizer (seq number + timestamp) ─► UDP (Wi-Fi) or TCP (USB)     │
│                                                                               │
│  Foreground Service (keeps running with the screen off)  ·  Compose UI        │
│  QR pairing · level meter · mute · gain · denoise toggle                      │
└──────────────────────────────────┬────────────────────────────────────────────┘
                                   │  Wi-Fi UDP   /  USB (adb reverse or USB tethering)
┌──────────────────────────────────▼──────────────── WINDOWS PC (Rust .exe) ────┐
│                                                                               │
│  Receiver ─► Jitter buffer (adaptive 20–60 ms) ─► Opus decode (+PLC/FEC)      │
│           ─► DSP chain:                                                       │
│               1. High-pass 80 Hz (rumble, handling noise)                     │
│               2. DPDFNet-48k (main noise removal) / DFN3 in low-latency mode │
│               3. Silero VAD ─► soft gate / expander (kills residual noise)    │
│               4. EQ: -2 dB @ 250 Hz (boxiness), +3 dB shelf @ 4 kHz (clarity) │
│               5. Compressor (3:1, -18 dB threshold, auto make-up gain)        │
│               6. De-esser (5–8 kHz)                                           │
│               7. Limiter (ceiling -1 dBFS)                                    │
│           ─► WASAPI output ─► Virtual audio device ─► "ClearMic Microphone"   │
│                                                                               │
│  Tauri UI: device list, pairing QR, A/B bypass, noise-strength slider,       │
│  presets (Natural / Discord / Podcast), latency readout, tray icon            │
└───────────────────────────────────────────────────────────────────────────────┘
                                   │
                        Discord / OBS / Zoom / games
```

### Latency budget (target under 100 ms, similar to Discord Krisp plus a USB mic)

| Stage | ms |
|---|---|
| Phone capture buffer | 10 |
| Opus frame | 10 |
| Network (Wi-Fi / USB) | 5–20 / 1–3 |
| Jitter buffer | 20–40 / 10 |
| Denoiser: DeepFilterNet3 ~40 / DPDFNet ~80 (HushMic's measured figure) | 40–80 |
| Output buffer (WASAPI) | 10 |
| **Total** | **Low-latency (DFN3): ~85–130 Wi-Fi / ~70 USB · Quality (DPDFNet): ~125–170 Wi-Fi / ~110 USB** |

Over 150 ms, people notice the lag on calls, so for Wi-Fi gaming voice chat, offer "Low latency" mode.

---

## 4. Transport and protocol

### Connection modes
1. **Wi-Fi (default)**: UDP for audio, TCP for control. Discovery with mDNS (`_clearmic._udp.local`) or a UDP broadcast.
2. **USB via ADB**: the PC runs `adb reverse tcp:47100 tcp:47100`, and the phone connects to `127.0.0.1`.
   This is what WO Mic does, and it needs USB debugging turned on. Ship `adb.exe` from platform-tools.
3. **USB tethering (no ADB)**: turn on USB tethering and the phone becomes a network card. Then Wi-Fi mode works over the cable with about 1 ms latency.
   This is easier for normal users than ADB.
4. Bluetooth: skip it. The latency and bandwidth are bad.

### Pairing and security
- The PC shows a **QR code** containing `clearmic://<ip>:<port>?key=<32-byte base64>&name=<pc-name>`.
- The phone scans it, and both sides use that key for **ChaCha20-Poly1305** (libsodium secretbox).
  This stops other people on café Wi-Fi from listening.

### Audio packet (UDP), little-endian
```
offset size field
0      2    magic 0xC1 0xEA
2      1    version (1)
3      1    flags (bit0 = muted, bit1 = phone-denoise-on, bit2 = FEC present)
4      4    sequence number (u32)
8      8    capture timestamp (u64, microseconds, phone clock)
16     2    payload length
18     N    Opus packet (encrypted together with the header as AAD)
```

### Control channel (TCP, length-prefixed JSON)
`hello`, `pair`, `set_gain`, `set_bitrate`, `mute`, `ping/pong` (to measure RTT), `stats`.

---

## 5. Project structure (monorepo)

```
clearmic/
├── ARCHITECTURE.md                ← this file
├── protocol/
│   └── PROTOCOL.md                ← packet + control spec (shared contract)
│
├── android/                       ← Kotlin app (Android Studio)
│   ├── settings.gradle.kts
│   └── app/
│       ├── build.gradle.kts       ← minSdk 26, NDK, CMake, Compose
│       └── src/main/
│           ├── AndroidManifest.xml          ← RECORD_AUDIO, FOREGROUND_SERVICE_MICROPHONE,
│           │                                   INTERNET, CAMERA (QR), POST_NOTIFICATIONS
│           ├── java/dev/clearmic/
│           │   ├── MainActivity.kt
│           │   ├── ui/                      ← Compose screens
│           │   │   ├── HomeScreen.kt        ← big Connect/Mute button, level meter
│           │   │   ├── PairScreen.kt        ← QR scanner (CameraX + ML Kit)
│           │   │   └── SettingsScreen.kt    ← bitrate, gain, phone denoise, mic source
│           │   ├── service/
│           │   │   └── MicStreamService.kt  ← foreground service, owns the engine
│           │   ├── net/
│           │   │   ├── Discovery.kt         ← mDNS (NsdManager)
│           │   │   ├── ControlClient.kt     ← TCP JSON control
│           │   │   └── PairingInfo.kt
│           │   ├── engine/
│           │   │   └── NativeEngine.kt      ← JNI bridge: start/stop/setGain/levels
│           │   └── data/SettingsStore.kt    ← DataStore prefs
│           └── cpp/
│               ├── CMakeLists.txt
│               ├── engine.cpp               ← Oboe input stream callback
│               ├── encoder.cpp              ← libopus encoder
│               ├── sender.cpp               ← UDP/TCP socket + packetizer + crypto
│               ├── denoise_rnnoise.cpp      ← optional phone-side denoise
│               └── third_party/ (oboe, opus, rnnoise, libsodium)
│
├── desktop/                       ← Rust workspace (Windows exe)
│   ├── Cargo.toml                 ← [workspace]
│   ├── crates/
│   │   ├── protocol/              ← packet structs, (de)serialize, crypto
│   │   ├── net/                   ← UDP/TCP server, mDNS advertise, adb helper
│   │   ├── jitter/                ← adaptive jitter buffer
│   │   ├── dsp/
│   │   │   ├── src/denoise.rs     ← trait Denoiser { fn process(&mut self, &mut [f32]) }
│   │   │   ├── src/dpdfnet.rs     ← DPDFNet (hushmic-denoiser or ONNX via `ort`)
│   │   │   ├── src/dfn3.rs        ← DeepFilterNet3 (libDF / DfTract), low-latency mode
│   │   │   ├── src/vad.rs         ← Silero VAD (ONNX via `ort`)
│   │   │   ├── src/eq.rs          ← biquad filters
│   │   │   ├── src/dynamics.rs    ← compressor, de-esser, limiter, gate
│   │   │   └── src/chain.rs       ← preset pipeline (Natural/Discord/Podcast)
│   │   ├── audio_out/             ← cpal/WASAPI writer to virtual device
│   │   └── engine/                ← glues it all together, lock-free ring buffers
│   ├── app/                       ← Tauri app (UI + tray)
│   │   ├── src-tauri/             ← Rust commands → engine
│   │   └── ui/                    ← built: index.html, app.css, app.js, tokens.css, base.css
│   │                                (plain web files, "Ultraviolet night" theme; talks to Rust via Tauri commands,
│   │                                 see the header of app.js. Preview: python3 -m http.server in this folder)
│   └── models/                    ← DeepFilterNet3 + Silero VAD weights
│
├── driver/                        ← v1.0: virtual mic driver (C++, WDK)
│   └── clearmic-vad/              ← fork of SysVAD / Virtual-Audio-Driver
│
├── installer/
│   └── clearmic.iss               ← Inno Setup: exe + adb + driver install
│
└── tools/
    ├── test-clips/                ← noisy recordings: fan, keyboard, street, kids, TV
    ├── bench/                     ← offline runner: wav in → chain → wav out
    └── dnsmos/                    ← score quality (Microsoft DNSMOS) for A/B tests
```

---

## 6. Key code sketches

### Android: foreground service (Kotlin)
```kotlin
class MicStreamService : Service() {
    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        startForeground(
            NOTIF_ID, buildNotification(),
            ServiceInfo.FOREGROUND_SERVICE_TYPE_MICROPHONE // required on Android 14+
        )
        val target = intent!!.getParcelableExtra<PairingInfo>("target")!!
        NativeEngine.start(target.host, target.port, target.key, sampleRate = 48_000)
        return START_STICKY
    }
    override fun onDestroy() { NativeEngine.stop(); super.onDestroy() }
    override fun onBind(i: Intent?) = null
}
```

### Android: capture callback (C++, Oboe)
```cpp
oboe::DataCallbackResult Engine::onAudioReady(oboe::AudioStream*, void* data, int32_t frames) {
    auto* in = static_cast<float*>(data);          // 480 frames = 10 ms @ 48 kHz
    if (phoneDenoise) rnnoise.process(in, frames);
    int bytes = opus_encode_float(enc, in, frames, pkt, sizeof(pkt));
    sender.send(pkt, bytes, seq++, nowMicros());   // non-blocking, lock-free queue
    return oboe::DataCallbackResult::Continue;
}
// Stream: Direction::Input, InputPreset::Unprocessed, PerformanceMode::LowLatency,
//         SharingMode::Exclusive, SampleRate 48000, ChannelCount 1, Format::Float
```

### PC: processing chain (Rust)
```rust
pub struct Chain { hpf: Biquad, df: DfTract, vad: SileroVad, gate: Gate,
                   eq: [Biquad; 2], comp: Compressor, deess: DeEsser, lim: Limiter }

impl Chain {
    pub fn process(&mut self, buf: &mut [f32]) {          // one 10 ms frame
        self.hpf.run(buf);
        self.df.process_frame(buf);                       // the "Krisp" part
        let speech = self.vad.prob(buf) > 0.5;
        self.gate.run(buf, speech);
        for f in &mut self.eq { f.run(buf); }
        self.comp.run(buf); self.deess.run(buf); self.lim.run(buf);
    }
}
```
Rule: the audio thread never allocates, locks or logs. Use `rtrb` ring buffers between the network thread, the DSP thread and the cpal output callback.

---

## 7. Roadmap

| Phase | Time (solo dev) | Result |
|---|---|---|
| **0. Spike** | 3–4 days | Python test: record a noisy wav on the phone, run DeepFilterNet3 and the chain, *listen*. Confirms the quality target before writing apps. |
| **1. MVP** | 2–3 weeks | Android app streams Opus over Wi-Fi UDP. Rust console exe receives it, denoises and plays into VB-Cable. Works in Discord. |
| **2. Usable** | 2–3 weeks | Tauri UI, QR pairing, mDNS, USB mode (adb + tethering), presets, A/B bypass, tray icon, auto-reconnect. |
| **3. Polish** | 2 weeks | Silero VAD gate, de-esser, latency stats, encryption, Inno Setup installer, auto-update. |
| **4. Own driver** | 3–6 weeks + signing | SysVAD-based "ClearMic Microphone", EV cert, attestation signing, VB-Cable no longer needed. |
| **5. Extras** | later | Studio mode (offline generative enhance), iOS app, macOS client (CoreAudio HAL plugin), multiple phones. |

---

## 8. Libraries and licenses

| Purpose | Library | License |
|---|---|---|
| Neural denoise (PC, quality) | DPDFNet (Ceva) / `hushmic-denoiser` | Apache-2.0 / MIT-Apache |
| Neural denoise (PC, low latency) | DeepFilterNet3 (`libDF`, Rust) | MIT / Apache-2.0 |
| Light denoise (phone) | GTCRN, UL-UNAS or RNNoise | MIT / BSD |
| Studio mode (offline) | ClearerVoice MossFormer2_SE_48K + Resemble Enhance | Apache-2.0 / MIT |
| VAD | Silero VAD (ONNX) via `ort` | MIT |
| Codec | libopus / `audiopus` | BSD |
| Android audio | Oboe | Apache-2.0 |
| PC audio | `cpal` (WASAPI) | Apache-2.0 |
| UI (PC) | Tauri | MIT / Apache-2.0 |
| Crypto | libsodium / `chacha20poly1305` | ISC / MIT |
| QR | ML Kit barcode (phone), `qrcode` crate (PC) | free / MIT |
| Virtual mic (MVP) | VB-Audio Virtual Cable | Donationware. Bundling requires a VB-Audio license. |
| Virtual mic (v1) | SysVAD sample / VirtualDrivers Virtual-Audio-Driver | MIT |

**Avoid** the Krisp SDK (commercial and expensive) and NVIDIA Maxine/Broadcast (RTX GPUs only). Maxine is fine as an *optional* backend if an RTX GPU is detected.

---

## 9. Tips that decide quality

1. **Capture raw audio.** Use `UNPROCESSED`. Phone NS/AGC fighting DeepFilterNet causes "underwater" artifacts.
2. **Keep 48 kHz end to end.** DeepFilterNet3 expects 48 kHz, so resampling adds latency and artifacts.
3. **Don't denoise at 100%.** Cap attenuation at about 30–40 dB ("Natural" preset). Full removal sounds robotic. Krisp also keeps a little room tone.
4. **Gain staging.** Apply phone gain before encoding so the voice peaks around -12 dBFS. Opus at 64 kbps or more is transparent for voice.
5. **Test with real noise:** a fan, mechanical keyboard, street traffic, a TV and a kids' room. Score with DNSMOS, and always do blind A/B listening.
6. **Battery:** a 10 ms Opus encode on the phone costs almost nothing. The screen-off foreground service is what keeps it alive, and some brands (Xiaomi, Oppo) need an "ignore battery optimization" prompt.

---

## Sources
- DPDFNet repo (models, sizes, real-time demo): https://github.com/ceva-ip/DPDFNet
- DPDFNet paper with the comparison table: https://arxiv.org/html/2512.16420
- HushMic (DPDFNet Linux virtual mic, Krisp comparison): https://github.com/Fovty/HushMic · https://github.com/Fovty/HushMic/wiki
- ClearerVoice-Studio (MossFormer2 48k): https://github.com/modelscope/ClearerVoice-Studio
- CrispVoice (open Adobe-Podcast-style pipeline): https://github.com/Francium-Tech/CrispVoice
- UL-UNAS: https://arxiv.org/html/2503.00340v2
- Tencent AuK: https://huggingface.co/tencent/AuK
- User report, DeepFilterNet vs NoiseTorch/RNNoise: https://discuss.getsol.us/d/10034-using-deepfilternet-with-easy-effects
- DeepFilterNet (upstream): https://github.com/rikorose/deepfilternet
- DeepFilterNet3 on Android via ONNX: https://soniqo.audio/guides/denoise/android
- GTCRN vs RNNoise numbers: https://github.com/sherryyu33/gtcrn · https://arxiv.org/html/2505.19597v1
- SysVAD virtual audio driver sample: https://learn.microsoft.com/en-us/samples/microsoft/windows-driver-samples/sysvad-virtual-audio-device-driver-sample/
- Virtual-Audio-Driver (MIT, SysVAD-based): https://kzserverkenguru.duckdns.org/VirtualDrivers/Virtual-Audio-Driver
- Micbridge (bundles VB-Cable, licensing notes): https://github.com/iAbadia/micbridge/releases/tag/v0.4.0
- How WO Mic works (third-party): https://webcammictest.com/mic/guides/phone-as-mic/ · https://sourceforge.net/p/wo-mic/
