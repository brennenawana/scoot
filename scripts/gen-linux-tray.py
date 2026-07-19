#!/usr/bin/env python3
"""Generate Scoot's Linux tray icon atlases (docs/PORTS.md §8).

StatusNotifierItem hands the host raw ARGB32 pixmaps and lets it choose which
one to draw, so we ship the sizes tray hosts actually ask for instead of
rescaling at runtime. That is not a performance choice: DESIGN.md §2's
"integer scale factors only, nearest-neighbor only" is constitutional, and a
panel's smooth downscale of one oversized pixmap would blur every edge.

Two things differ from the macOS menu bar atlas, both forced by the platform:

  * **Full colour, no template.** macOS tints an `isTemplate` image to match
    the menu bar. Linux panels do no such thing, so a black-and-alpha resting
    icon would disappear into Ubuntu's dark top bar. Frame 0 ships coloured.
  * **Centred in a square, not an 18px frame.** Tray hosts ask for concrete
    pixel sizes (16/22/24/…), few of which are integer multiples of the 16px
    art. The art is drawn at the largest integer scale that fits and centred
    on an integral origin — PORTS.md §13's rule, applied to the panel.

Frame layout matches the menu bar atlas so the shell's animation code is the
same shape everywhere: frame 0 resting, frames 1-4 the bounce.

Outputs (committed, per PHILOSOPHY.md §3 — regenerate, don't build):
  shells/linux/assets/tray/tray-classic-<size>.png   5-frame strip

v0.1 parity ships the classic buddy only; the collection's 12 species get
their tray art when the collection itself lands on Linux (M4).
"""

import os
import sys
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from pixellib import blank, blit, grid_to_pixels, hstack, write_png

REPO = Path(__file__).resolve().parents[1]
OUT = REPO / "shells" / "linux" / "assets" / "tray"

# What tray hosts ask for in practice: 16/22/24 are the common panel sizes
# (GNOME's appindicator extension, KDE's systemtray, XFCE's statusnotifier),
# 32/48/64 cover HiDPI panels and the 2x/3x scale factors.
TRAY_SIZES = (16, 22, 24, 32, 48, 64)

ART = 16  # the character grids are 16x16


def _load_art():
    """Import the v0.1 grids from the placeholder generator.

    Its filename has hyphens, so it is not a normal importable module name;
    load it by path. Importing is safe — the module only acts under __main__.
    """
    import importlib.util

    path = Path(__file__).parent / "gen-placeholder-assets.py"
    spec = importlib.util.spec_from_file_location("scoot_placeholder", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def tray_atlas(art, size):
    """A 5-frame strip of `size`x`size` cells: resting, then the 4-beat bounce.

    The scale is the largest integer that fits the 16px art in the cell, and
    the origin is integral — so every edge lands on a pixel boundary at every
    size. `dy` is in grid units and scales with the art, keeping the bounce's
    proportions identical across sizes.
    """
    scale = max(1, size // ART)
    origin = (size - ART * scale) // 2

    beats = [
        (art.IDLE, 0),      # 0: resting
        (art.SQUASH, 0),    # 1: squash
        (art.IDLE, -1),     # 2: up
        (art.TALL, -2),     # 3: stretch
        (art.IDLE, -1),     # 4: up
    ]

    frames = []
    for index, (grid, dy) in enumerate(beats):
        pixels = grid_to_pixels(grid, art.PALETTE, scale)
        oy = origin + dy * scale
        canvas = blit(blank(size, size), pixels, origin, oy)

        # A bounce beat may only shed *empty* rows off the top of the cell:
        # clipping real art would make the buddy lose its head at 16px.
        if oy < 0:
            clipped_rows = grid[: (-oy + scale - 1) // scale]
            opaque = sum(
                1 for row in clipped_rows for ch in row if art.PALETTE[ch][3] != 0
            )
            assert opaque == 0, (
                f"tray frame {index} at {size}px clips {opaque} opaque pixels"
            )
        frames.append(canvas)

    return hstack(frames)


def main():
    art = _load_art()
    art.validate(art.IDLE)
    art.validate(art.SQUASH)
    art.validate(art.TALL)

    print("Linux tray atlases:")
    for size in TRAY_SIZES:
        write_png(OUT / f"tray-classic-{size}.png", tray_atlas(art, size),
                  repo_root=REPO)
    print("Done.")


if __name__ == "__main__":
    main()
