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
from pixellib import (blank, blit, grid_to_pixels, hstack, shear, shift_y,
                      validate, vsquash, vstack, write_png)

REPO = Path(__file__).resolve().parents[1]
SPRITES = REPO / "Sources" / "Scoot" / "Resources" / "Sprites"

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
    idle = species["idle"]
    style = species["style"]
    if style == "bounce":
        return [shear(idle, "left"), vsquash(idle), shear(idle, "right"), idle]
    if style == "float":
        return [idle, shift_y(shear(idle, "left"), -1),
                idle, shift_y(shear(idle, "right"), -1)]
    if style == "storm":
        return [idle, shift_y(idle, -1), species["alt"], shift_y(idle, -1)]
    raise ValueError(f"unknown style {style}")


def fps_for(style):
    return 10.0 if style == "bounce" else 6.0


def main():
    print("Cast sprites:")
    preview_rows = []
    for species in CAST:
        pal = species["palette"]
        validate(species["idle"], pal)
        if "alt" in species:
            validate(species["alt"], pal)
        frames = dance_frames(species)

        strip = hstack([grid_to_pixels(g, pal, scale=2) for g in frames])
        name = f"buddy-{species['id']}"
        write_png(SPRITES / f"{name}.png", strip, repo_root=REPO)
        manifest = {
            "name": name,
            "frameWidth": 32,
            "frameHeight": 32,
            "frameCount": len(frames),
            "fps": fps_for(species["style"]),
        }
        mpath = SPRITES / f"{name}.json"
        mpath.write_text(json.dumps(manifest, indent=2) + "\n")
        print(f"  wrote {mpath.relative_to(REPO)}")

        preview_rows.append(hstack(
            [grid_to_pixels(g, pal, scale=8) for g in [species["idle"]] + frames]
        ))

    print("Preview:")
    write_png(REPO / "docs" / "assets" / "cast-preview.png",
              vstack(preview_rows), repo_root=REPO)
    print("Done.")


if __name__ == "__main__":
    main()
