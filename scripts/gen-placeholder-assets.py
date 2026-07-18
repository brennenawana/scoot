#!/usr/bin/env python3
"""Generate Scoot's placeholder assets (pixel sprites, menu bar icons, app icon,
chime) using only the Python standard library, so the whole art pipeline runs
anywhere — including the Linux container this scaffold was authored in.

Outputs (committed to the repo; re-run to regenerate):
  Sources/Scoot/Resources/Sprites/buddy-classic.png   4-frame 32x32 dance strip
  Sources/Scoot/Resources/Sprites/buddy-classic.json  sprite sheet manifest
  Sources/Scoot/Resources/Sounds/nudge-chime.wav      gentle two-note chime
  Sources/Scoot/Resources/MenuBar/icon-idle(@2x).png  template-style resting icon
  Sources/Scoot/Resources/MenuBar/icon-frame-N(@2x).png  4 bounce frames
  Support/AppIcon.iconset/…                           10 sizes for iconutil
  docs/assets/buddy-preview.png                       8x preview strip for README

The real art pipeline (Aseprite -> atlas) replaces this in v0.2; the file formats
it emits are already the real ones (see docs/TECHNICAL.md 3d).
"""

import json
import math
import os
import struct
import wave
import zlib
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
RES = REPO / "Sources" / "Scoot" / "Resources"

# ---------------------------------------------------------------- pixel art ---

# 16x16 grids, one char per pixel. Scaled x2 to the 32x32 shipping sprites.
PALETTE = {
    ".": (0, 0, 0, 0),          # transparent
    "o": (59, 42, 40, 255),     # outline, warm dark brown
    "B": (255, 138, 112, 255),  # body coral
    "S": (224, 107, 82, 255),   # body shade
    "H": (255, 196, 176, 255),  # highlight
    "E": (43, 27, 24, 255),     # eye
    "W": (255, 255, 255, 255),  # eye shine
    "K": (255, 158, 134, 255),  # cheek
    "M": (122, 46, 31, 255),    # mouth
}

IDLE = [
    "................",
    "................",
    "................",
    "....oooooooo....",
    "...oBBBBBBBBo...",
    "..oBHHBBBBBBBo..",
    ".oBBHBBBBBBBBBo.",
    ".oBBBBBBBBBBBBo.",
    ".oBWEBBBBBWEBBo.",
    ".oBEEBBBBBEEBBo.",
    ".oBKBBBBBBBKBBo.",
    ".oBBBBMMBBBBBBo.",
    "..oBBBBBBBBBBo..",
    "..oSBBBBBBBBSo..",
    "...oSSSSSSSSo...",
    "....oooooooo....",
]

SQUASH = [
    "................",
    "................",
    "................",
    "................",
    "...oooooooooo...",
    "..oBBBBBBBBBBo..",
    ".oBBHHBBBBBBBBo.",
    "oBBBBBBBBBBBBBBo",
    "oBWEBBBBBBWEBBBo",
    "oBEEBBBBBBEEBBBo",
    "oBKBBBBBBBBKBBBo",
    "oBBBBBMMBBBBBBBo",
    ".oBBBBBBBBBBBBo.",
    ".oSBBBBBBBBBBSo.",
    "..oSSSSSSSSSSo..",
    "...oooooooooo...",
]

TALL = [
    "................",
    "................",
    "....oooooooo....",
    "...oBBBBBBBBo...",
    "..oBHHBBBBBBBo..",
    ".oBBHBBBBBBBBBo.",
    ".oBBBBBBBBBBBBo.",
    ".oBBBBBBBBBBBBo.",
    ".oBWEBBBBBWEBBo.",
    ".oBEEBBBBBEEBBo.",
    ".oBKBBBBBBBKBBo.",
    ".oBBBBMMMMBBBBo.",
    "..oBBBBBBBBBBo..",
    "..oSBBBBBBBBSo..",
    "...oSSSSSSSSo...",
    "....oooooooo....",
]


def validate(grid):
    assert len(grid) == 16, f"grid has {len(grid)} rows"
    for row in grid:
        assert len(row) == 16, f"row '{row}' has {len(row)} chars"
        for ch in row:
            assert ch in PALETTE, f"unknown palette char '{ch}'"
    return grid


def shear(grid, direction):
    """Lean the blob by shifting upper rows sideways (dance sway)."""
    out = []
    for y, row in enumerate(grid):
        shift = 2 if y <= 7 else (1 if y <= 11 else 0)
        if direction == "right":
            out.append(("." * shift + row)[:16])
        else:
            out.append((row + "." * shift)[shift:])
    return out


def grid_to_pixels(grid, scale=1):
    """-> list of rows of RGBA tuples, nearest-neighbor scaled."""
    px = []
    for row in grid:
        expanded = [PALETTE[ch] for ch in row for _ in range(scale)]
        px.extend([expanded] * scale)
    return px


def blank(width, height):
    return [[(0, 0, 0, 0)] * width for _ in range(height)]


def blit(canvas, pixels, ox, oy):
    for y, row in enumerate(pixels):
        cy = oy + y
        if 0 <= cy < len(canvas):
            for x, p in enumerate(row):
                cx = ox + x
                if 0 <= cx < len(canvas[0]) and p[3] != 0:
                    canvas[cy][cx] = p
    return canvas


def hstack(frames):
    height = len(frames[0])
    out = []
    for y in range(height):
        row = []
        for f in frames:
            row.extend(f[y])
        out.append(row)
    return out


def recolor_template(pixels):
    """Menu bar resting icon: black + alpha only, so isTemplate rendering
    adapts to dark mode / menu bar tint."""
    out = []
    for row in pixels:
        out.append([(0, 0, 0, p[3]) if p[3] != 0 else p for p in row])
    return out


def write_png(path, pixels):
    height = len(pixels)
    width = len(pixels[0])
    raw = b""
    for row in pixels:
        raw += b"\x00" + b"".join(struct.pack("4B", *p) for p in row)

    def chunk(tag, data):
        c = tag + data
        return struct.pack(">I", len(data)) + c + struct.pack(">I", zlib.crc32(c))

    ihdr = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    png = (b"\x89PNG\r\n\x1a\n"
           + chunk(b"IHDR", ihdr)
           + chunk(b"IDAT", zlib.compress(raw, 9))
           + chunk(b"IEND", b""))
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(png)
    print(f"  wrote {path.relative_to(REPO)}  ({width}x{height})")


# ------------------------------------------------------------------- sounds ---

def write_chime(path):
    """Two soft sine notes (E5 -> A5) with attack/decay, a hint of overtone."""
    rate = 44100
    amp = 0.32

    def note(freq, dur, overlap_tail=0.10):
        n = int(rate * (dur + overlap_tail))
        samples = []
        for i in range(n):
            t = i / rate
            env = min(t / 0.02, 1.0) * math.exp(-3.2 * t)
            v = math.sin(2 * math.pi * freq * t) + 0.25 * math.sin(2 * math.pi * freq * 2 * t)
            samples.append(amp * env * v)
        return samples

    a = note(659.25, 0.30)   # E5
    b = note(880.00, 0.42)   # A5
    gap = int(rate * 0.16)
    total = max(len(a), gap + len(b))
    mix = [0.0] * total
    for i, s in enumerate(a):
        mix[i] += s
    for i, s in enumerate(b):
        mix[gap + i] += s

    path.parent.mkdir(parents=True, exist_ok=True)
    with wave.open(str(path), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(rate)
        frames = b"".join(
            struct.pack("<h", max(-32767, min(32767, int(s * 32767)))) for s in mix
        )
        w.writeframes(frames)
    print(f"  wrote {path.relative_to(REPO)}  ({len(mix) / rate:.2f}s)")


# --------------------------------------------------------------------- main ---

def main():
    idle = validate(IDLE)
    squash = validate(SQUASH)
    tall = validate(TALL)
    lean_l = shear(idle, "left")
    lean_r = shear(idle, "right")

    print("Sprites:")
    dance16 = [lean_l, squash, lean_r, tall]
    dance32 = [grid_to_pixels(g, scale=2) for g in dance16]
    write_png(RES / "Sprites" / "buddy-classic.png", hstack(dance32))
    manifest = {
        "name": "buddy-classic",
        "frameWidth": 32,
        "frameHeight": 32,
        "frameCount": 4,
        "fps": 10.0,
    }
    mpath = RES / "Sprites" / "buddy-classic.json"
    mpath.write_text(json.dumps(manifest, indent=2) + "\n")
    print(f"  wrote {mpath.relative_to(REPO)}")

    print("Menu bar icons:")
    for scale, suffix in ((1, ""), (2, "@2x")):
        side = 18 * scale
        canvas = blit(blank(side, side), grid_to_pixels(idle, scale), scale, scale)
        write_png(RES / "MenuBar" / f"icon-idle{suffix}.png", recolor_template(canvas))
        bounce = [(squash, 0), (idle, -1), (tall, -2), (idle, -1)]
        for i, (grid, dy) in enumerate(bounce, start=1):
            canvas = blit(blank(side, side), grid_to_pixels(grid, scale), scale, scale + dy * scale)
            write_png(RES / "MenuBar" / f"icon-frame-{i}{suffix}.png", canvas)

    print("App icon set:")
    iconset = REPO / "Support" / "AppIcon.iconset"
    for size, name in [
        (16, "icon_16x16"), (32, "icon_16x16@2x"),
        (32, "icon_32x32"), (64, "icon_32x32@2x"),
        (128, "icon_128x128"), (256, "icon_128x128@2x"),
        (256, "icon_256x256"), (512, "icon_256x256@2x"),
        (512, "icon_512x512"), (1024, "icon_512x512@2x"),
    ]:
        scale = size // 16
        write_png(iconset / f"{name}.png", grid_to_pixels(idle, scale))

    print("Sound:")
    write_chime(RES / "Sounds" / "nudge-chime.wav")

    print("Preview:")
    preview = hstack([grid_to_pixels(g, scale=8) for g in [idle] + dance16])
    write_png(REPO / "docs" / "assets" / "buddy-preview.png", preview)

    print("Done.")


if __name__ == "__main__":
    main()
