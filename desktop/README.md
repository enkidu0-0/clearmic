# ClearMic for PC

Receives your phone's microphone over Wi-Fi, removes noise with DPDFNet, and plays the result into a
virtual microphone cable that Discord, OBS, Zoom and games can pick as a mic.

## Use it (Windows)

1. Install **VB-Audio Virtual Cable** (vb-audio.com/Cable, free for personal use) and restart.
2. Run the ClearMic installer, open the app. It finds "CABLE Input" by itself.
3. On your phone, open the ClearMic app, tap *scan pc code* and scan the code in the ClearMic window.
4. In Discord (or OBS, Zoom...) choose **CABLE Output** as your microphone.

## Runs in the background

- Closing the window does not quit: ClearMic keeps receiving from the tray icon (left-click to open, right-click for the menu).
  The window's web view is destroyed on close, which is what gives its memory back.
- Output tab, Background: **Start with Windows** (starts hidden in the tray with `--minimized`).
- A second launch just brings the running one forward.
- With no phone sending for two seconds the audio thread sleeps: about 0% CPU.

## Light on a gaming PC

Noise removal is the only heavy part. Pick it under **Power use** on the Live tab:

| Mode | Model | Cost (one core, Apple M1, measured) |
| --- | --- | --- |
| Efficient (default) | DPDFNet baseline, 16 kHz | about 11 to 23% |
| Balanced | DPDFNet-2, 16 kHz | about 42 to 45% |
| Quality | DPDFNet-2, 48 kHz | about 63 to 65% |
| Off | voice chain only | about 1% |

If a frame takes most of its 10 ms for two seconds, the engine moves down one step by itself and says so in the window.
On Windows the audio threads register with the multimedia scheduler ("Pro Audio") and the rest of the app runs at below-normal priority.

## Build it

The installer has to be built on Windows. Either push to GitHub and run the `windows` workflow, or:

```bash
cargo install tauri-cli --version "^2" --locked
cd desktop/app/src-tauri
cargo tauri build          # installer in desktop/target/release/bundle/nsis/
```

On a Mac or Linux you can run the same app for testing: `cargo run --release -p clearmic-app`
(set `CLEARMIC_OUTPUT=wav:/some/file.wav` to record the output to a file instead of a device).

## Layout

- `crates/protocol`: packet format and encryption
- `crates/jitter`: reorder and loss handling
- `crates/dsp`: voice chain and the DPDFNet denoiser
- `crates/engine`: network thread, playout thread, decoder, sinks
- `app/src-tauri`: the Tauri app (commands, config, pairing code)
- `app/ui`: the window (plain HTML, CSS, JS)
- `models/`: `dpdfnet2_48khz_hr.onnx` (Ceva DPDFNet, Apache-2.0), bundled into the installer

Run all tests with `cargo test --workspace`.
