#!/usr/bin/env python3
"""Generate the 12-species launch cast sprite sheets (docs/PRODUCT.md §2).

Same stdlib-only pipeline as gen-placeholder-assets.py: each species is a
16x16 character grid + palette, scaled x2 to the shipping 32x32 frames. Dance
strips are 4 frames built from the idle grid by generic transforms (shear /
squash / bob), plus hand-authored signature frames where a species earns one
(Thundercloud's bolt).

Outputs (committed; re-run to regenerate):
  Sources/Scoot/Resources/Sprites/buddy-<id>.png / .json   for all 12 species
  docs/assets/cast-preview.png                             art-direction sheet

Species ids and sheet names must match Sources/ScootCore/Resources/
buddy-catalog.json — the manifest is the contract, this script is content.
"""

import json
import os
import sys
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pixellib import (blank, blit, grid_to_pixels, hstack, pad_h,
                      recolor_template, shear, shift_y, validate, vsquash,
                      vstack, vstretch, write_png)

REPO = Path(__file__).resolve().parents[1]
SPRITES = REPO / "Sources" / "Scoot" / "Resources" / "Sprites"
MENUBAR = REPO / "Sources" / "Scoot" / "Resources" / "MenuBar"

# Dance frames carry a transparent apron so the shear lean never truncates
# art at the canvas edge (a full-width sprite used to lose its outermost
# pixels mid-dance). 16px art + 2px each side = 20px frames, 40px shipped.
APRON = 2

OUTLINE = (59, 42, 40, 255)      # shared warm-dark outline (design tokens)
EYE = (43, 27, 24, 255)
WHITE = (255, 255, 255, 255)


def palette(**colors):
    base = {".": (0, 0, 0, 0), "o": OUTLINE, "E": EYE, "W": WHITE}
    base.update(colors)
    return base


# ---------------------------------------------------------------- the cast ---
# Each species: id, palette, idle grid, animation style.
# Styles: "bounce" = lean/squash/lean/idle (grounded), "float" = bob + sway
# (airborne), "storm" = thundercloud's bolt beat.

ROUND_BLOB = {
    "id": "round-blob",
    "style": "bounce",
    "menubar": [
        "................",
        "................",
        "................",
        "....oooooooo....",
        "...oBBBBBBBBo...",
        "..oBHBBBBBBBBo..",
        "..oBWEBBBBWEBo..",
        "..oBEEBBBBEEBo..",
        "..oBKBBBBBKBBo..",
        "..oBBBMMBBBBBo..",
        "..oSBBBBBBBBSo..",
        "...oSSSSSSSSo...",
        "....oooooooo....",
        "................",
        "................",
        "................",
    ],
    "palette": palette(
        B=(255, 138, 112, 255),  # body coral
        S=(224, 107, 82, 255),   # shade
        H=(255, 196, 176, 255),  # highlight
        K=(255, 158, 134, 255),  # cheek
        M=(122, 46, 31, 255),    # mouth
    ),
    "idle": [
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
    ],
}

BEAN_CAT = {
    "id": "bean-cat",
    "style": "bounce",
    "menubar": [
        "................",
        "................",
        "...oo.....oo....",
        "...oPo...oPo....",
        "..oBBBo.oBBBo...",
        "..oBBBBoBBBBo...",
        ".oBBBBBBBBBBBo..",
        ".oBEBBBBBBEBBo..",
        ".oBBBBNBBBBBBo..",
        ".oBKBBBBBBKBBo..",
        ".oSBBBBBBBBBSo..",
        "..oSSSSSSSSSo...",
        "...ooooooooo....",
        "................",
        "................",
        "................",
    ],
    "palette": palette(
        B=(239, 207, 165, 255),  # loaf
        S=(214, 171, 120, 255),  # shade
        P=(255, 183, 160, 255),  # inner ear
        K=(255, 158, 134, 255),  # cheek
        N=(150, 84, 54, 255),    # nose
    ),
    "idle": [
        "................",
        "................",
        "..oo......oo....",
        "..oPo....oPo....",
        ".oBBBo..oBBBo...",
        ".oBBBBooBBBBo...",
        ".oBBBBBBBBBBo...",
        "oBBBBBBBBBBBBo..",
        "oBEBBBBBBBEBBo..",
        "oBBBBNBBBBBBBo..",
        "oBKBBBBBBBKBBoo.",
        "oBBBBBBBBBBBoBBo",
        "oBBBBBBBBBBBBSo.",
        "oSBBBBBBBBBBSo..",
        ".oSSSSSSSSSSo...",
        "..oooooooooo....",
    ],
}

DUMPLING_DOG = {
    "id": "dumpling-dog",
    "style": "bounce",
    "menubar": [
        "................",
        "................",
        "......oo........",
        ".....oRRo.......",
        "....oRRRRo......",
        "...ooBBBBoo.....",
        "..oBBBBBBBBBo...",
        ".oRBBEBBBEBBRo..",
        ".oRBBBNNBBBBRo..",
        ".oBBKBNNBKBBBo..",
        "..oBBBBBBBBBo...",
        "..oSSoBBBoSSo...",
        "...ooooooooo....",
        "................",
        "................",
        "................",
    ],
    "palette": palette(
        B=(242, 227, 201, 255),  # dumpling cream
        S=(216, 195, 160, 255),  # shade
        R=(169, 116, 74, 255),   # ears / topknot brown
        K=(255, 158, 134, 255),  # cheek
        N=(96, 62, 40, 255),     # nose
    ),
    "idle": [
        "................",
        "................",
        ".......oo.......",
        "......oRRo......",
        ".....oRRRRo.....",
        "....ooBBBBoo....",
        "..ooBBBBBBBBoo..",
        ".oRBBBBBBBBBBRo.",
        "oRRBEBBBBBEBBRRo",
        "oRRBBBNNBBBBBRRo",
        ".oRBKBNNBBKBBRo.",
        ".oBBBBBBBBBBBBo.",
        "..oBBBBBBBBBBo..",
        "..oBBoBBBBoBBo..",
        "..oSSoSSSSoSSo..",
        "...oooooooooo...",
    ],
}

TRAFFIC_CONE = {
    "id": "traffic-cone",
    "style": "bounce",
    "menubar": [
        "................",
        "................",
        "......oo........",
        ".....oOOo.......",
        ".....oOOo.......",
        "....oBBBBo......",
        "....oEBBEo......",
        "...oOOOOOOo.....",
        "...oOOOOOOo.....",
        "..oBBBBBBBBo....",
        ".oOOOOOOOOOOo...",
        ".oooooooooooo...",
        ".oDDDDDDDDDDo...",
        "..oooooooooo....",
        "................",
        "................",
    ],
    "palette": palette(
        O=(255, 122, 51, 255),   # cone orange
        D=(217, 91, 31, 255),    # dark orange / base
        B=(255, 255, 255, 255),  # band white
        K=(255, 170, 140, 255),  # cheek
    ),
    "idle": [
        "................",
        ".......oo.......",
        "......oOOo......",
        "......oOOo......",
        ".....oOOOOo.....",
        ".....oOOOOo.....",
        "....oBBBBBBo....",
        "....oBEBBEBo....",
        "...oOOOOOOOOo...",
        "...oOKOOOOKOo...",
        "..oBBBBBBBBBBo..",
        "..oOOOOOOOOOOo..",
        ".oOOOOOOOOOOOOo.",
        "oooooooooooooooo",
        "oDDDDDDDDDDDDDDo",
        ".oooooooooooooo.",
    ],
}

BABY_GHOST = {
    "id": "baby-ghost",
    "style": "float",
    "menubar": [
        "................",
        "................",
        "................",
        "....oooooo......",
        "...oBBBBBBo.....",
        "..oBBBBBBBBo....",
        "..oBEEBBEEBo....",
        "..oBEEBBEEBo....",
        "..oBBBMMBBBo....",
        "..oBBBBBBBBo....",
        "..oBSBBSBBSo....",
        "...oo.oo.oo.....",
        "................",
        "................",
        "................",
        "................",
    ],
    "palette": {
        ".": (0, 0, 0, 0),
        "o": (74, 84, 120, 255),     # cool outline
        "B": (234, 242, 255, 255),   # sheet
        "S": (199, 215, 240, 255),   # hem shade
        "E": (43, 27, 24, 255),
        "M": (110, 120, 155, 255),   # little o mouth
        "K": (255, 183, 197, 255),   # cheek
    },
    "idle": [
        "................",
        "................",
        ".....oooooo.....",
        "....oBBBBBBo....",
        "...oBBBBBBBBo...",
        "..oBBBBBBBBBBo..",
        "..oBEEBBBBEEBo..",
        "..oBEEBBBBEEBo..",
        "..oBKBBMMBBKBo..",
        "..oBBBBMMBBBBo..",
        "..oBBBBBBBBBBo..",
        "..oBBBBBBBBBBo..",
        "..oBSBBSSBBSBo..",
        "...oo.oo.oo.oo..",
        "................",
        "................",
    ],
}

SUNGLASSES_FROG = {
    "id": "sunglasses-frog",
    "style": "bounce",
    "menubar": [
        "................",
        "................",
        "................",
        "..ooo.....ooo...",
        ".oBBBo...oBBBo..",
        ".oBBBBoooBBBBo..",
        ".oGGGGGGGGGGGo..",
        ".oGLGGGGGGLGGo..",
        ".oBBBBBBBBBBBo..",
        ".oBBMMMMMMBBBo..",
        ".oSBBBBBBBBBSo..",
        "..oSSSSSSSSSo...",
        "...ooooooooo....",
        "................",
        "................",
        "................",
    ],
    "palette": palette(
        B=(107, 191, 89, 255),   # frog green
        S=(76, 158, 74, 255),    # shade
        T=(168, 224, 99, 255),   # throat light
        G=(24, 24, 28, 255),     # glasses
        L=(120, 130, 150, 255),  # glare
        M=(60, 110, 50, 255),    # mouth line
    ),
    "idle": [
        "................",
        "................",
        "................",
        "..ooo......ooo..",
        ".oBBBo....oBBBo.",
        ".oBBBBooooBBBBo.",
        "oBGGGGGGGGGGGGo.",
        "oBGLLGGGGGLLGGo.",
        "oBBGGGBBBBGGGBo.",
        "oBBBBBBBBBBBBBo.",
        "oBTTBBBBBBBTTBo.",
        "oBBBMMMMMMBBBBo.",
        ".oBBBBBBBBBBBo..",
        ".oSBBBBBBBBBSo..",
        "..oSSSSSSSSSo...",
        "...ooooooooo....",
    ],
}

HOUSEPLANT = {
    "id": "houseplant",
    "style": "bounce",
    "menubar": [
        "................",
        "................",
        "...oo...oo......",
        "..oGGo.oGGo.....",
        "..oGLGoGLGo.....",
        "...oGLGLGo......",
        "....oLLLo.......",
        "...ooooooo......",
        "...oPPPPPo......",
        "...oPEPPEPo.....",
        "...oQPPPPQo.....",
        "....ooooo.......",
        "................",
        "................",
        "................",
        "................",
    ],
    "palette": palette(
        L=(76, 158, 94, 255),    # leaf
        G=(123, 196, 127, 255),  # leaf light
        V=(47, 122, 68, 255),    # vein
        P=(201, 111, 74, 255),   # pot terracotta
        Q=(165, 82, 47, 255),    # pot shade
        K=(255, 158, 134, 255),  # cheek
    ),
    "idle": [
        "................",
        "....oo...oo.....",
        "...oGGo.oGGo....",
        "..oGLLGoGLLGo...",
        "..oGLVLGLVLGo...",
        "...oGLLGLLGo....",
        ".oo.oGLGLGo.oo..",
        "oGGooGLLGooGGo..",
        "oGLLGoLLoGLLGo..",
        ".ooGGoLLoGGoo...",
        "...ooooooooo....",
        "...oPPPPPPPPo...",
        "...oPEPPPPEPo...",
        "...oPKPPPPKPo...",
        "...oQPPPPPPQo...",
        "....oooooooo....",
    ],
}

SKATER_ROBOT = {
    "id": "skater-robot",
    "style": "bounce",
    "menubar": [
        "................",
        ".......A........",
        ".......o........",
        "....oooooooo....",
        "...oPPPPPPPPo...",
        "...oPCCPPCCPo...",
        "...oPPPPPPPPo...",
        "...oPPGGGGPPo...",
        "....oooooooo....",
        "......oGGo......",
        ".....oGHHGo.....",
        "......oGGo......",
        "................",
        "................",
        "................",
        "................",
    ],
    "palette": palette(
        P=(201, 216, 232, 255),  # face panel
        B=(159, 180, 199, 255),  # body steel
        S=(122, 143, 166, 255),  # shade
        C=(89, 227, 255, 255),   # eye glow cyan
        A=(255, 156, 65, 255),   # antenna bobble
        G=(58, 63, 74, 255),     # grill / wheel
        H=(159, 180, 199, 255),  # wheel hub
    ),
    "idle": [
        ".......A........",
        ".......o........",
        "....oooooooo....",
        "...oPPPPPPPPo...",
        "...oPCCPPCCPo...",
        "...oPPPPPPPPo...",
        "...oPPGGGGPPo...",
        "....oooooooo....",
        "...oBBBBBBBBo...",
        "..oBBSBBBBSBBo..",
        "..oBBBBBBBBBBo..",
        "...oBBBBBBBBo...",
        "....oooooooo....",
        "......oGGo......",
        ".....oGHHGo.....",
        "......oGGo......",
    ],
}

HOODIE_FOX = {
    "id": "hoodie-fox",
    "style": "bounce",
    "menubar": [
        "................",
        "................",
        "....oooooooo....",
        "...oHHHHHHHHo...",
        "..oHHooooooHHo..",
        "..oHoFFFFFFoHo..",
        "..oHoFEFFEFoHo..",
        "..oHoFFCCFFoHo..",
        "..oHooFCCFooHo..",
        "..oHHHooooHHHo..",
        "...oHHHHHHHHo...",
        "....oooooooo....",
        "................",
        "................",
        "................",
        "................",
    ],
    "palette": palette(
        H=(142, 147, 166, 255),  # hood grey
        D=(110, 115, 133, 255),  # hood shade
        F=(242, 140, 59, 255),   # fox orange
        C=(255, 232, 202, 255),  # muzzle cream
        T=(255, 232, 202, 255),  # tail tip
    ),
    "idle": [
        "................",
        "................",
        "....oooooooo....",
        "...oHHHHHHHHo...",
        "..oHHHHHHHHHHo..",
        ".oHHooooooooHHo.",
        ".oHoFFFFFFFFoHo.",
        ".oHoFEFFFFEFoHo.",
        ".oHoFFFCCFFFoHo.",
        ".oHooFCCCCFooHo.",
        ".oHHHooooooHHHo.",
        ".oHHHHHHHHHHooo.",
        ".oHHHHHHHHHoFFo.",
        "..oHHHHHHHHoFTo.",
        "..oDDDDDDDDooo..",
        "...oooooooo.....",
    ],
}

TINY_DRAGON = {
    "id": "tiny-dragon",
    "style": "bounce",
    "menubar": [
        "................",
        "................",
        "....o.....o.....",
        "...oDo...oDo....",
        "..ooDDoooDDoo...",
        ".oGoDDDDDDDoGo..",
        ".oGoDEDDDEDoGo..",
        "..ooDDDDDDDoo...",
        "...oDBBBBBDo....",
        "...oDDBBBDDo....",
        "....oDDDDDo.....",
        "....oDo.oDo.....",
        "....oo...oo.....",
        "................",
        "................",
        "................",
    ],
    "palette": palette(
        D=(63, 167, 150, 255),   # scale teal
        S=(44, 122, 111, 255),   # shade
        G=(35, 98, 89, 255),     # wing dark
        B=(242, 227, 201, 255),  # belly cream
        N=(30, 70, 64, 255),     # nostril
        K=(255, 158, 134, 255),  # cheek
    ),
    "idle": [
        "................",
        "................",
        "....o......o....",
        "...oDo....oDo...",
        "oo.oDDooooDDo.oo",
        "oGooDDDDDDDDooGo",
        "oGGoDEDDDDEDoGGo",
        "oGGoDDDDDDDDoGGo",
        ".oGoDKDDDDKDoGo.",
        ".oGoDDBBBBDDoGo.",
        "..ooDBBBBBBDoo..",
        "...oDDBBBBDDo...",
        "...oDDDDDDDDo...",
        "....oDDooDDo....",
        "....oSSooSSo....",
        ".....oo..oo.....",
    ],
}

KNIGHT_BEETLE = {
    "id": "knight-beetle",
    "style": "bounce",
    "menubar": [
        "................",
        "................",
        ".......oo.......",
        "......oHHo......",
        "......oHHo......",
        ".....ooHHoo.....",
        "....oLLLLLLo....",
        "...oLAAAAAALo...",
        "..oAAAAAAAAAAo..",
        "..oAVVVVVVVVAo..",
        "..oAAAAAAAAAAo..",
        "...oASSSSSSAo...",
        "....oooooooo....",
        "...oSo....oSo...",
        "................",
        "................",
    ],
    "palette": palette(
        A=(74, 91, 140, 255),    # armor navy
        S=(54, 67, 107, 255),    # armor shade
        L=(143, 163, 217, 255),  # steel light
        H=(201, 211, 232, 255),  # horn silver
        V=(24, 24, 34, 255),     # visor slit
    ),
    "idle": [
        ".......oo.......",
        "......oHHo......",
        "......oHHo......",
        ".......oHHo.....",
        "....ooooHooo....",
        "...oLLLLLLLLo...",
        "..oLLAAAAAALLo..",
        ".oAAAAAAAAAAAAo.",
        ".oAVVVVVVVVVVAo.",
        ".oAAAAAAAAAAAAo.",
        ".oASAAAAAAAASAo.",
        "..oASAAAAAASAo..",
        "..oAASSSSSSAAo..",
        "..ooAoAAAAoAoo..",
        "...oSo.oo.oSo...",
        "....o......o....",
    ],
}

THUNDERCLOUD = {
    "id": "thundercloud",
    "style": "storm",
    "menubar": [
        "................",
        "................",
        "................",
        "....oooooo......",
        "...oWWWWWWoo....",
        "..oWCCCCCCCWo...",
        ".oCRRCCCCRRCo...",
        ".oCEECCCCEECo...",
        ".oCCCCDDCCCCo...",
        "..oDDDDDDDDo....",
        "...oooooooo.....",
        "................",
        "................",
        "................",
        "................",
        "................",
    ],
    "palette": {
        ".": (0, 0, 0, 0),
        "o": (60, 60, 72, 255),
        "C": (185, 194, 207, 255),   # cloud
        "D": (141, 151, 168, 255),   # underside
        "W": (230, 236, 244, 255),   # sunlit top
        "E": (43, 27, 24, 255),
        "R": (70, 74, 88, 255),      # scowl brow
        "Y": (255, 216, 77, 255),    # bolt
    },
    "idle": [
        "................",
        "................",
        "................",
        ".....oooooo.....",
        "...ooWWWWWWoo...",
        "..oWWWWWWWWWWo..",
        ".oWWCCCCCCCCWWo.",
        "oCCRRCCCCRRCCCo.",
        "oCCEECCCCEECCCo.",
        "oCCCCCDDCCCCCCo.",
        ".oDCCDDDDCCDDo..",
        "..oDDDDDDDDDo...",
        "...ooooooooo....",
        "................",
        "................",
        "................",
    ],
    # The signature frame: one lightning crack under the scowl.
    "alt": [
        "................",
        ".....oooooo.....",
        "...ooWWWWWWoo...",
        "..oWWWWWWWWWWo..",
        ".oWWCCCCCCCCWWo.",
        "oCCRRCCCCRRCCCo.",
        "oCCEECCCCEECCCo.",
        "oCCCCCDDCCCCCCo.",
        ".oDCCDDDDCCDDo..",
        "..oDDDDDDDDDo...",
        "...ooooooooo....",
        "......oYYo......",
        ".....oYYo.......",
        "......oYo.......",
        ".....oYo........",
        "......o.........",
    ],
}

CAST = [
    ROUND_BLOB, BEAN_CAT, DUMPLING_DOG, TRAFFIC_CONE,
    BABY_GHOST, SUNGLASSES_FROG, HOUSEPLANT,
    SKATER_ROBOT, HOODIE_FOX,
    TINY_DRAGON, KNIGHT_BEETLE,
    THUNDERCLOUD,
]


def dance_frames(species):
    """4 padded frames, 20x16 each. The apron guarantees leans lose nothing."""
    idle = pad_h(species["idle"], APRON)
    style = species["style"]
    if style == "bounce":
        frames = [shear(idle, "left"), vsquash(idle), shear(idle, "right"), idle]
    elif style == "float":
        frames = [idle, shift_y(shear(idle, "left"), -1),
                  idle, shift_y(shear(idle, "right"), -1)]
    elif style == "storm":
        alt = pad_h(species["alt"], APRON)
        frames = [idle, shift_y(idle, -1), alt, shift_y(idle, -1)]
    else:
        raise ValueError(f"unknown style {style}")

    opaque = lambda g: sum(ch != "." for row in g for ch in row)
    for i, frame in enumerate(frames):
        if style == "bounce" and i == 1:
            continue  # squash resamples rows away by design
        if style == "storm" and i == 2:
            continue  # the bolt frame is different art
        assert opaque(frame) == opaque(idle), \
            f"{species['id']} frame {i} clips art ({opaque(frame)} vs {opaque(idle)})"
    return frames


def fps_for(style):
    return 10.0 if style == "bounce" else 6.0


def top_margin(grid):
    for index, row in enumerate(grid):
        if any(ch != "." for ch in row):
            return index
    return len(grid)


def celebrate_frames(species):
    """The credited-scoot moment: a 4-frame hop (or bob/wiggle where the art
    demands), padded like the dance. Loops at 10 fps for a bounded linger."""
    idle = pad_h(species["idle"], APRON)
    style = species["style"]
    if style in ("float", "storm"):
        return [idle, shift_y(idle, -1), shift_y(idle, -2), shift_y(idle, -1)]
    rise = min(2, top_margin(species["idle"]))
    if rise == 0:
        # Art touches the canvas top (robot antenna, beetle horn): an excited
        # wiggle instead of a hop.
        return [vsquash(idle), shear(idle, "left"), shear(idle, "right"), idle]
    return [vsquash(idle), shift_y(idle, -rise), shift_y(idle, -max(1, rise - 1)), idle]


def write_reveal_pop(path):
    """The burst beat's sound: a soft pop (fast-decaying filtered thump) plus
    a tiny rising sparkle arpeggio. Gentle by design — the reveal is loud
    visually, not acoustically."""
    import math
    import struct as _struct
    import wave

    rate = 44100
    total = int(rate * 0.55)
    mix = [0.0] * total

    # Pop: a 60ms sine thump sliding down 220->110 Hz.
    for i in range(int(rate * 0.06)):
        t = i / rate
        freq = 220 - 1800 * t
        env = math.exp(-55 * t)
        mix[i] += 0.5 * env * math.sin(2 * math.pi * freq * t)

    # Sparkle: three quick ascending notes (E6, A6, C#7), 70ms apart.
    for n, freq in enumerate([1318.5, 1760.0, 2217.5]):
        start = int(rate * (0.10 + 0.07 * n))
        for i in range(int(rate * 0.22)):
            t = i / rate
            env = min(t / 0.008, 1.0) * math.exp(-16 * t)
            if start + i < total:
                mix[start + i] += 0.16 * env * math.sin(2 * math.pi * freq * t)

    path.parent.mkdir(parents=True, exist_ok=True)
    with wave.open(str(path), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(rate)
        w.writeframes(b"".join(
            _struct.pack("<h", max(-32767, min(32767, int(s * 32767)))) for s in mix
        ))
    print(f"  wrote {path.relative_to(REPO)}  ({total / rate:.2f}s)")


def menubar_atlas(species, scale):
    """5-frame 18px strip for the status item: frame 0 is the template
    silhouette (the resting icon), frames 1-4 the colored bounce with baked
    y offsets — same recipe as v0.1's dedicated icons, but from the species'
    simplified small-size art, not a downscale of the detailed sprite."""
    pal = species["palette"]
    grid = species["menubar"]
    side = 18 * scale
    beats = [
        (grid, 0),                 # 0: idle -> template
        (vsquash(grid, 2), 0),     # 1: squash
        (grid, -1),                # 2: up
        (vstretch(grid, 2), -2),   # 3: stretch (18 rows, blits past the top)
        (grid, -1),                # 4: up
    ]
    frames = []
    for index, (g, dy) in enumerate(beats):
        canvas = blit(blank(side, side), grid_to_pixels(g, pal, scale),
                      scale, (1 + dy) * scale)
        if index == 3:
            # The stretch frame may only shed empty apron rows off the top.
            tall = grid_to_pixels(g, pal, 1)
            clipped = sum(p[3] != 0 for row in tall[:1] for p in row)
            assert clipped == 0, f"{species['id']} menubar stretch clips art"
        frames.append(recolor_template(canvas) if index == 0 else canvas)
    return hstack(frames)


def main():
    print("Cast sprites:")
    preview_rows = []
    mb_preview = []
    for species in CAST:
        pal = species["palette"]
        validate(species["idle"], pal)
        validate(species["menubar"], pal)
        if "alt" in species:
            validate(species["alt"], pal)
        frames = dance_frames(species)

        strip = hstack([grid_to_pixels(g, pal, scale=2) for g in frames])
        name = f"buddy-{species['id']}"
        write_png(SPRITES / f"{name}.png", strip, repo_root=REPO)
        manifest = {
            "name": name,
            "frameWidth": (16 + 2 * APRON) * 2,  # 40: 20px padded art, x2
            "frameHeight": 32,
            "frameCount": len(frames),
            "fps": fps_for(species["style"]),
        }
        mpath = SPRITES / f"{name}.json"
        mpath.write_text(json.dumps(manifest, indent=2) + "\n")
        print(f"  wrote {mpath.relative_to(REPO)}")

        cele = celebrate_frames(species)
        write_png(SPRITES / f"{name}-celebrate.png",
                  hstack([grid_to_pixels(g, pal, scale=2) for g in cele]), repo_root=REPO)
        cpath = SPRITES / f"{name}-celebrate.json"
        cpath.write_text(json.dumps({
            "name": f"{name}-celebrate",
            "frameWidth": (16 + 2 * APRON) * 2,
            "frameHeight": 32,
            "frameCount": len(cele),
            "fps": 10.0,
        }, indent=2) + "\n")
        print(f"  wrote {cpath.relative_to(REPO)}")

        for scale, suffix in ((1, ""), (2, "@2x")):
            write_png(MENUBAR / f"{name}-menubar{suffix}.png",
                      menubar_atlas(species, scale), repo_root=REPO)

        preview_rows.append(hstack(
            [grid_to_pixels(g, pal, scale=8)
             for g in [pad_h(species["idle"], APRON)] + frames + cele]
        ))
        mb_preview.append(menubar_atlas(species, 4))

    print("Sound:")
    write_reveal_pop(REPO / "Sources" / "Scoot" / "Resources" / "Sounds" / "reveal-pop.wav")

    print("Previews:")
    write_png(REPO / "docs" / "assets" / "cast-preview.png",
              vstack(preview_rows), repo_root=REPO)
    write_png(REPO / "docs" / "assets" / "menubar-preview.png",
              vstack(mb_preview), repo_root=REPO)
    print("Done.")


if __name__ == "__main__":
    main()
