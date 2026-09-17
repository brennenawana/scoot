#!/usr/bin/env python3
"""Generate the Windows tray icon strips (docs/PORTS.md §7).

macOS gets away with two exports because the status bar is always 18pt and
AppKit picks @1x or @2x for us. `Shell_NotifyIcon` does not: the tray asks for
whatever `GetSystemMetrics(SM_CXSMICON)` says — `MulDiv(16, dpi, 96)` — and it
takes a full HICON that we hand it pixel for pixel. Hand it the wrong size and
the notification area *stretches* what it got, which is the fractional resample
this whole script exists to prevent. So the shell needs a real icon at every
size Windows can ask for.

That set is every rung of the Windows display-scaling dropdown, and it is not
just the round ones: 175% asks for 28 and 225% asks for 36, both standard and
both common on 4K laptops. Missing either is not a smaller icon, it is a blurry
one.

They are generated here rather than resampled at runtime because DESIGN.md §2
and PORTS.md §9 make that constitutional: integer scale factors only,
nearest-neighbor only, integral origins. A 16x16 blob stretched to 20px on the
fly is a fractional scale and it comes out blurry, which breaks the spell. The
art therefore sits at an integer scale inside a transparent inset — the icon
looks slightly smaller on odd tray sizes and stays perfectly crisp, which is
the trade this product always makes.

The source art is the v0.1 species-agnostic stock blob owned by
gen-placeholder-assets.py. The 12-species cast and its per-species menubar
atlases are v0.2's collection feature, which PORTS.md excludes from this
milestone.

Outputs (committed; re-run to regenerate):
  shells/windows/assets/tray/tray-<S>.png   5 frames of SxS, for S in 16..48
"""

import importlib.util
import os
import sys
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pixellib import blank, blit, grid_to_pixels, hstack, validate, write_png

REPO = Path(__file__).resolve().parents[1]
TRAY = REPO / "shells" / "windows" / "assets" / "tray"

# SM_CXSMICON = MulDiv(16, dpi, 96), evaluated at every rung of the Windows
# display-scaling dropdown:
#   100% -> 16   125% -> 20   150% -> 24   175% -> 28
#   200% -> 32   225% -> 36   250% -> 40   300% -> 48   350% -> 56
# Ship all of them. A size we skip is not a size we lose — it is a size the
# shell interpolates for us, which is the one outcome PORTS.md §9 forbids.
SIZES = (16, 20, 24, 28, 32, 36, 40, 48, 56)

# The stock grids are 16x16 (DESIGN.md §2).
ART = 16


def stock_art():
    """The v0.1 blob grids and palette belong to gen-placeholder-assets.py.
    Import them instead of copying so the tray can never drift away from the
    menu bar art it is the port of. The hyphen in the filename rules out a
    plain import statement."""
    path = REPO / "scripts" / "gen-placeholder-assets.py"
    spec = importlib.util.spec_from_file_location("gen_placeholder_assets", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def geometry(size):
    """-> (scale, inset). Biggest integer scale that still fits, centered."""
    scale = size // ART
    slack = size - ART * scale
    assert scale >= 1, f"tray size {size} is smaller than the {ART}px art"
    assert slack % 2 == 0, (
        f"tray size {size} centers the art on a half pixel; integral origins "
        f"only (DESIGN.md §2)"
    )
    inset = slack // 2
    assert ART * scale + 2 * inset == size
    return scale, inset


def blit_whole(canvas, pixels, ox, oy, label):
    """blit() silently drops anything past the canvas edge. The bounce beats
    deliberately draw above the top of the art box, so prove first that only
    empty apron rows go over — a clipped outline is a chopped-off buddy."""
    height, width = len(canvas), len(canvas[0])
    lost = sum(
        1
        for y, row in enumerate(pixels)
        for x, p in enumerate(row)
        if p[3] != 0 and not (0 <= oy + y < height and 0 <= ox + x < width)
    )
    assert lost == 0, f"{label}: {lost} opaque pixels clipped at the canvas edge"
    return blit(canvas, pixels, ox, oy)


def tray_strip(art, size):
    """5-frame SxS horizontal strip: the resting icon, then the bounce.

    Beat structure is menubar_atlas's (idle, squash, up, stretch, up) drawn
    with v0.1's hand-authored SQUASH and TALL grids. Unlike macOS, frame 0 is
    *not* run through recolor_template: NSImage.isTemplate tints a silhouette
    to match the menu bar, but Shell_NotifyIcon just draws the HICON we give
    it, so a black-on-alpha frame 0 would show up as a blob-shaped hole.
    """
    scale, inset = geometry(size)
    beats = [
        (art.IDLE, 0),     # 0: calm, at rest
        (art.SQUASH, 0),   # 1: squash
        (art.IDLE, -1),    # 2: up
        (art.TALL, -2),    # 3: stretch, blits above its own box
        (art.IDLE, -1),    # 4: up
    ]
    frames = []
    for index, (grid, dy) in enumerate(beats):
        pixels = grid_to_pixels(grid, art.PALETTE, scale)
        canvas = blit_whole(
            blank(size, size), pixels, inset, inset + dy * scale,
            f"tray-{size} frame {index}",
        )
        frames.append(canvas)
    return hstack(frames)


def main():
    art = stock_art()
    for grid in (art.IDLE, art.SQUASH, art.TALL):
        validate(grid, art.PALETTE, ART)

    print("Windows tray icons:")
    for size in SIZES:
        strip = tray_strip(art, size)
        assert len(strip) == size and len(strip[0]) == 5 * size
        write_png(TRAY / f"tray-{size}.png", strip, repo_root=REPO)
    print("Done.")


if __name__ == "__main__":
    main()
