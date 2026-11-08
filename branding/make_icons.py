#!/usr/bin/env python3
"""Draws the ClearMic mark and writes every icon the apps need.

A microphone whose capsule holds a clean waveform: flat deep teal, warm paper white. The proportions are measured from
the approved ChatGPT artwork, redrawn as vectors so every size is crisp.
Run from anywhere:  python3 branding/make_icons.py
Needs Pillow and numpy; the .icns step needs macOS (iconutil).
"""
import os, shutil, subprocess, sys
import numpy as np
from PIL import Image, ImageDraw, ImageFilter

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
TAURI_ICONS = os.path.join(ROOT, "desktop/app/src-tauri/icons")
ANDROID_RES = os.path.join(ROOT, "android/app/src/main/res")

TEAL = (20, 107, 103)      # #146B67
PAPER = (251, 249, 244)    # #FBF9F4

S = 4                      # supersampling
N = 1024 * S


def mark_layer(T, color=PAPER, bars=TEAL, cut_bars=False):
    """The mic on a transparent 1024 canvas, sized as if drawn on a tile T px wide (the mark is 0.707 T tall)."""
    img = Image.new("RGBA", (N, N), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    cx = cy = 512

    def X(f): return (cx + f * T) * S           # fractions of the tile, from its centre
    def Y(f): return (cy + f * T) * S
    def W(f): return f * T * S

    half_w = 0.1495
    stroke = 0.055
    # capsule
    d.rounded_rectangle([X(-half_w), Y(-0.349), X(half_w), Y(0.159)], radius=W(half_w), fill=color)
    # holder: lower half circle with round caps, centre line radius 0.2085
    arc_cy, r_mid = 0.007, 0.2085
    d.arc([X(-(r_mid + stroke / 2)), Y(arc_cy - r_mid - stroke / 2), X(r_mid + stroke / 2), Y(arc_cy + r_mid + stroke / 2)],
          0, 180, fill=color, width=int(W(stroke)))
    for ex in (-r_mid, r_mid):
        d.ellipse([X(ex - stroke / 2), Y(arc_cy - stroke / 2), X(ex + stroke / 2), Y(arc_cy + stroke / 2)], fill=color)
    # stem and base
    base_y = 0.330
    d.rectangle([X(-0.0282), Y(arc_cy + r_mid), X(0.0282), Y(base_y)], fill=color)
    d.rounded_rectangle([X(-0.127), Y(base_y - stroke / 2), X(0.127), Y(base_y + stroke / 2)], radius=W(stroke / 2), fill=color)
    # waveform: a tall centre bar and two short ones
    bw = 0.043
    bar_layer = Image.new("RGBA", (N, N), (0, 0, 0, 0))
    bd = ImageDraw.Draw(bar_layer)
    for bx, top, bottom in ((-0.0797, -0.130, -0.0033), (0.0, -0.201, 0.0449), (0.0797, -0.130, -0.0033)):
        bd.rounded_rectangle([X(bx - bw / 2), Y(top), X(bx + bw / 2), Y(bottom)], radius=W(bw / 2), fill=bars + (255,))
    if cut_bars:
        alpha = np.asarray(img.getchannel("A"), dtype=np.int16) - np.asarray(bar_layer.getchannel("A"), dtype=np.int16)
        img.putalpha(Image.fromarray(np.clip(alpha, 0, 255).astype(np.uint8)))
    else:
        img = Image.alpha_composite(img, bar_layer)
    return img


def tile(inset_frac, radius_frac, shadow=False):
    """Flat teal rounded square with the mark, 1024 px."""
    inset = int(N * inset_frac)
    mask = Image.new("L", (N, N), 0)
    ImageDraw.Draw(mask).rounded_rectangle([inset, inset, N - inset, N - inset], radius=int((N - 2 * inset) * radius_frac), fill=255)
    out = Image.new("RGBA", (N, N), (0, 0, 0, 0))
    if shadow:
        a = mask.filter(ImageFilter.GaussianBlur(14 * S)).point(lambda v: int(v * 0.30))
        sh = Image.new("RGBA", (N, N), (8, 40, 38, 255))
        sh.putalpha(a)
        out.alpha_composite(sh, (0, 10 * S))
    out.paste(Image.new("RGBA", (N, N), TEAL + (255,)), (0, 0), mask)
    out.alpha_composite(mark_layer((N - 2 * inset) / S))
    return out.resize((1024, 1024), Image.LANCZOS)


def save(img, path, px=None):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    (img.resize((px, px), Image.LANCZOS) if px else img).save(path)


def main():
    mac = tile(0.098, 0.215, shadow=True)    # macOS: 824 px tile inside the standard margin
    win = tile(0.04, 0.215)                  # Windows, Linux, Tauri
    save(mac, os.path.join(HERE, "icon-macos-1024.png"))
    save(win, os.path.join(HERE, "icon-1024.png"))
    save(win, os.path.join(HERE, "icon-512.png"), 512)
    mark_layer(1024, cut_bars=False).resize((1024, 1024), Image.LANCZOS).save(os.path.join(HERE, "mark-paper.png"))
    mark_layer(1024, color=(0, 0, 0), bars=(0, 0, 0), cut_bars=True).resize((1024, 1024), Image.LANCZOS).save(os.path.join(HERE, "mark-black.png"))

    # Tauri: png sizes, ico, icns
    for name, px in (("32x32.png", 32), ("128x128.png", 128), ("128x128@2x.png", 256), ("icon.png", 512)):
        save(win, os.path.join(TAURI_ICONS, name), px)
    win.resize((256, 256), Image.LANCZOS).save(os.path.join(TAURI_ICONS, "icon.ico"), sizes=[(s, s) for s in (16, 24, 32, 48, 64, 128, 256)])
    iconset = os.path.join(HERE, "icon.iconset")
    shutil.rmtree(iconset, ignore_errors=True)
    os.makedirs(iconset)
    for s in (16, 32, 128, 256, 512):
        save(mac, os.path.join(iconset, f"icon_{s}x{s}.png"), s)
        save(mac, os.path.join(iconset, f"icon_{s}x{s}@2x.png"), s * 2)
    if sys.platform == "darwin":
        subprocess.run(["iconutil", "-c", "icns", iconset, "-o", os.path.join(TAURI_ICONS, "icon.icns")], check=True)
    shutil.rmtree(iconset, ignore_errors=True)

    # macOS menu bar: a black template image, the system tints it
    tray = mark_layer(1150, color=(0, 0, 0), bars=(0, 0, 0), cut_bars=True).resize((1024, 1024), Image.LANCZOS)
    save(tray, os.path.join(TAURI_ICONS, "tray-template.png"), 44)

    # Android adaptive icon: 108 dp canvas, the mark (0.707 T tall) kept inside the 72 dp safe zone
    fg = mark_layer(840).resize((1024, 1024), Image.LANCZOS)
    mono = mark_layer(840, color=(0, 0, 0), bars=(0, 0, 0), cut_bars=True).resize((1024, 1024), Image.LANCZOS)
    for dpi, px in (("mdpi", 108), ("hdpi", 162), ("xhdpi", 216), ("xxhdpi", 324), ("xxxhdpi", 432)):
        save(fg, os.path.join(ANDROID_RES, f"mipmap-{dpi}", "ic_launcher_foreground.png"), px)
        save(mono, os.path.join(ANDROID_RES, f"mipmap-{dpi}", "ic_launcher_monochrome.png"), px)
    save(Image.new("RGBA", (64, 64), TEAL + (255,)), os.path.join(ANDROID_RES, "drawable-nodpi", "ic_launcher_background.png"))
    print("icons written")


if __name__ == "__main__":
    main()
