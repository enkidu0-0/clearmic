#!/usr/bin/env python3
"""ClearMic denoiser A/B bench.

Runs every clip in a folder through several denoisers, writes the results,
scores them with DNSMOS (if available), and builds a blind listening page.

    cd clearmic/spike
    .venv/bin/python ../tools/bench/bench.py ../tools/test-clips

Outputs go to tools/bench/out/:
    <clip>/<system>.wav   every system's result (not blind)
    blind.html            blind A/B page (labels hidden, open in a browser)
    key.json              label -> system mapping (blind.html reveals it too)
    scores.csv            DNSMOS + real-time factor per clip and system
"""
import argparse
import csv
import json
import random
import subprocess
import sys
import tempfile
import time
import warnings
from pathlib import Path

import numpy as np
import soundfile as sf

warnings.filterwarnings("ignore")

HERE = Path(__file__).resolve().parent
RNNOISE_MODEL = HERE / "models" / "std.rnnn"
SR = 48_000
AUDIO_EXT = {".wav", ".flac", ".mp3", ".m4a", ".ogg", ".aac", ".opus", ".mp4"}


def load_48k_mono(path: Path) -> np.ndarray:
    """Decode anything ffmpeg can read to 48 kHz mono float32."""
    with tempfile.TemporaryDirectory() as td:
        tmp = Path(td) / "in.wav"
        subprocess.run(
            ["ffmpeg", "-y", "-loglevel", "error", "-i", str(path),
             "-ac", "1", "-ar", str(SR), "-c:a", "pcm_f32le", str(tmp)],
            check=True)
        audio, _ = sf.read(tmp, dtype="float32")
    return audio


# ---- systems: each is f(audio48k) -> audio48k ------------------------------

def sys_raw(x):
    return x


def make_dpdfnet(model):
    def run(x):
        import dpdfnet
        return np.asarray(dpdfnet.enhance(x, SR, model=model), dtype=np.float32)
    return run


_dfn = {}


def sys_dfn3(x):
    import torch
    from df.enhance import enhance, init_df
    if not _dfn:
        _dfn["m"], _dfn["s"], _ = init_df(log_level="ERROR")
    out = enhance(_dfn["m"], _dfn["s"], torch.from_numpy(x).unsqueeze(0))
    return out.squeeze(0).numpy().astype(np.float32)


def sys_rnnoise(x):
    with tempfile.TemporaryDirectory() as td:
        a, b = Path(td) / "a.wav", Path(td) / "b.wav"
        sf.write(a, x, SR, subtype="FLOAT")
        subprocess.run(
            ["ffmpeg", "-y", "-loglevel", "error", "-i", str(a),
             "-af", f"arnndn=m={RNNOISE_MODEL}", "-c:a", "pcm_f32le", str(b)],
            check=True)
        y, _ = sf.read(b, dtype="float32")
    return y


SYSTEMS = {
    "raw": sys_raw,
    "rnnoise": sys_rnnoise,
    "dfn3": sys_dfn3,
    "dpdfnet2": make_dpdfnet("dpdfnet2_48khz_hr"),
    "dpdfnet8": make_dpdfnet("dpdfnet8_48khz_hr"),
}


def match_level(y, ref):
    """Match loudness to the raw clip so louder != 'better' in blind tests."""
    n = min(len(y), len(ref))
    ry = np.sqrt(np.mean(y[:n] ** 2)) + 1e-9
    rr = np.sqrt(np.mean(ref[:n] ** 2)) + 1e-9
    y = y * (rr / ry)
    peak = np.max(np.abs(y))
    return y / peak * 0.98 if peak > 0.98 else y


def dnsmos(y):
    try:
        from speechmos import dnsmos as d
        import scipy.signal as ss
        y16 = np.clip(ss.resample_poly(y, 1, 3), -1.0, 1.0).astype(np.float32)
        r = d.run(y16, 16000)
        return r["ovrl_mos"], r["sig_mos"], r["bak_mos"]
    except Exception as e:  # scoring is optional
        print("  (DNSMOS unavailable:", type(e).__name__, e, ")", file=sys.stderr)
        return None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("clips", type=Path, help="folder with noisy recordings")
    ap.add_argument("--systems", default=",".join(SYSTEMS))
    ap.add_argument("--out", type=Path, default=HERE / "out")
    args = ap.parse_args()

    names = [s for s in args.systems.split(",") if s]
    clips = sorted(p for p in args.clips.iterdir() if p.suffix.lower() in AUDIO_EXT)
    if not clips:
        sys.exit(f"No audio files in {args.clips}")
    args.out.mkdir(parents=True, exist_ok=True)

    rows, key = [], {}
    blind = []  # (clip, [(label, relpath)])
    rng = random.Random(7)

    for clip in clips:
        print(f"\n== {clip.name}")
        x = load_48k_mono(clip)
        dur = len(x) / SR
        cdir = args.out / clip.stem
        cdir.mkdir(exist_ok=True)
        results = {}
        for name in names:
            t0 = time.perf_counter()
            y = SYSTEMS[name](x)
            rtf = (time.perf_counter() - t0) / dur
            y = match_level(np.asarray(y, dtype=np.float32), x)
            sf.write(cdir / f"{name}.wav", y, SR, subtype="PCM_16")
            results[name] = y
            m = dnsmos(y)
            rows.append([clip.name, name, f"{rtf:.3f}"] +
                        ([f"{v:.2f}" for v in m] if m else ["", "", ""]))
            print(f"  {name:9s} RTF {rtf:5.2f}" + (f"  DNSMOS ovl {m[0]:.2f} sig {m[1]:.2f} bak {m[2]:.2f}" if m else ""))

        # blind set: raw is included as a hidden control
        order = list(names)
        rng.shuffle(order)
        entries = []
        for i, name in enumerate(order):
            label = chr(ord("A") + i)
            key[f"{clip.stem}/{label}"] = name
            entries.append((label, f"{clip.stem}/{name}.wav"))
        blind.append((clip.name, entries))

    with open(args.out / "scores.csv", "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["clip", "system", "rtf", "dnsmos_ovl", "dnsmos_sig", "dnsmos_bak"])
        w.writerows(rows)
    (args.out / "key.json").write_text(json.dumps(key, indent=2))
    write_blind_page(args.out, blind, key)

    print(f"\nDone. Open {args.out / 'blind.html'} and listen. Scores: {args.out / 'scores.csv'}")


def write_blind_page(out: Path, blind, key):
    cards = []
    for clip, entries in blind:
        stem = Path(clip).stem
        rows = "".join(
            f'<div class="row"><b>{label}</b>'
            f'<audio controls preload="none" src="{rel}"></audio>'
            f'<select data-k="{stem}/{label}"><option value="">rate…</option>'
            + "".join(f'<option>{n}</option>' for n in range(1, 6)) +
            f'</select><span class="rev" data-k="{stem}/{label}"></span></div>'
            for label, rel in entries)
        cards.append(f"<section><h2>{clip}</h2>{rows}</section>")
    html = f"""<!doctype html><meta charset="utf-8"><title>ClearMic blind test</title>
<style>
body{{font:16px system-ui;max-width:760px;margin:2rem auto;padding:0 1rem}}
section{{border:1px solid #8884;border-radius:10px;padding:1rem;margin:1rem 0}}
.row{{display:flex;gap:.8rem;align-items:center;margin:.4rem 0}}
audio{{flex:1}} .rev{{min-width:7rem;font-weight:600}}
button{{padding:.6rem 1.2rem;font-size:1rem}}
</style>
<h1>Blind denoiser test</h1>
<p>Use headphones. Rate each clip 1 (bad) to 5 (great) for how clean and natural the voice sounds.
One option in each group is the untouched original. Click <i>Reveal</i> only when done.</p>
{''.join(cards)}
<button id="go">Reveal and tally</button><pre id="tally"></pre>
<script>
const KEY={json.dumps(key)};
document.getElementById('go').onclick=()=>{{
  const sums={{}},cnt={{}};
  document.querySelectorAll('select').forEach(s=>{{
    const sys=KEY[s.dataset.k];
    document.querySelector('.rev[data-k="'+s.dataset.k+'"]').textContent=sys;
    if(s.value){{sums[sys]=(sums[sys]||0)+ +s.value;cnt[sys]=(cnt[sys]||0)+1;}}
  }});
  document.getElementById('tally').textContent=Object.keys(sums)
    .map(k=>[k,sums[k]/cnt[k]]).sort((a,b)=>b[1]-a[1])
    .map(([k,v])=>k.padEnd(10)+v.toFixed(2)).join('\\n')||'No ratings entered.';
}};
</script>"""
    (out / "blind.html").write_text(html)


if __name__ == "__main__":
    main()
