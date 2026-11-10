# Denoiser A/B bench

Compares RNNoise, DeepFilterNet3, DPDFNet-2 and DPDFNet-8 (plus the raw clip as a hidden control) on your own recordings.

## Run

Put real noisy recordings (phone voice memos are fine: wav, m4a, mp3...) in `tools/test-clips/`.
Aim for 8-10 clips of 10-30 s: fan, keyboard, street, TV, kids, echoey room, quiet room.
`fan_keyboard_synthetic.wav` is only a smoke test; delete it for the real test.

```bash
cd sonivane/spike
.venv/bin/python ../tools/bench/bench.py ../tools/test-clips
open ../tools/bench/out/blind.html
```

## Output (`tools/bench/out/`)

- `blind.html`: shuffled, unlabeled players with 1-5 ratings. "Reveal and tally" shows which system each letter was and the average rating per system.
- `scores.csv`: DNSMOS (overall, speech, background; higher is better) and offline real-time factor (below 1 = faster than real time).
- `<clip>/<system>.wav`: each result, loudness-matched to the original so louder does not win.

## Notes

- DNSMOS is a guide. The blind listening result decides.
- RTF is for whole-file offline processing on this Mac, not streaming latency on a Windows PC.
- Models: DPDFNet from Hugging Face (cached in `~/Library/Caches/dpdfnet`), DeepFilterNet3 weights download on first use, RNNoise model in `models/std.rnnn`.
