"""Shared pixel-art helpers for Scoot's stdlib-only asset generators.

Everything works on 16x16 character grids (one char per pixel, palette maps
char -> RGBA) so sprites stay reviewable as text diffs. PNG encoding is done
by hand — the whole pipeline must run anywhere, including bare Linux CI.
"""

import struct
import zlib


def validate(grid, palette, size=16):
    assert len(grid) == size, f"grid has {len(grid)} rows"
    for row in grid:
        assert len(row) == size, f"row '{row}' has {len(row)} chars"
        for ch in row:
            assert ch in palette, f"unknown palette char '{ch}'"
    return grid


def shear(grid, direction, upper=7, mid=11):
    """Lean a sprite by shifting upper rows sideways (dance sway)."""
    width = len(grid[0])
    out = []
    for y, row in enumerate(grid):
        shift = 2 if y <= upper else (1 if y <= mid else 0)
        if direction == "right":
            out.append(("." * shift + row)[:width])
        else:
            out.append((row + "." * shift)[shift:])
    return out


def vsquash(grid, by=2):
    """Vertically compress a sprite by `by` rows (nearest-neighbor resample,
    anchored to the ground line) — the generic squash beat of a bounce."""
    size = len(grid)
    new_h = size - by
    sampled = [grid[min(size - 1, round(i * size / new_h))] for i in range(new_h)]
    return ["." * len(grid[0])] * by + sampled


def shift_y(grid, dy):
    """Move a sprite down (dy>0) or up (dy<0), dropping rows off the edge —
    the bob of a floating species. Only safe when the vacated rows are empty."""
    blank_row = "." * len(grid[0])
    if dy > 0:
        return [blank_row] * dy + grid[:-dy]
    if dy < 0:
        return grid[-dy:] + [blank_row] * (-dy)
    return grid


def grid_to_pixels(grid, palette, scale=1):
    """-> list of rows of RGBA tuples, nearest-neighbor scaled."""
    px = []
    for row in grid:
        expanded = [palette[ch] for ch in row for _ in range(scale)]
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


def vstack(sheets):
    width = max(len(s[0]) for s in sheets)
    out = []
    for sheet in sheets:
        for row in sheet:
            out.append(list(row) + [(0, 0, 0, 0)] * (width - len(row)))
    return out


def recolor_template(pixels):
    """Menu bar resting icon: black + alpha only, so isTemplate rendering
    adapts to dark mode / menu bar tint."""
    out = []
    for row in pixels:
        out.append([(0, 0, 0, p[3]) if p[3] != 0 else p for p in row])
    return out


def write_png(path, pixels, repo_root=None):
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
    shown = path.relative_to(repo_root) if repo_root else path
    print(f"  wrote {shown}  ({width}x{height})")
