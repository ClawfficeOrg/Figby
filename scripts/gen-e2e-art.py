#!/usr/bin/env python3
"""Generate tribute .figmap scenes for Figby E2E art tests.

Design goals (user request): popular-8bit-inspired sprites recreated as
*tributes* (NOT pixel copies), flair behind them, famous-8bit-inspired
backgrounds for the layer tests, and a FIGBY banner finale with animation +
lighting + particles. All scenes are generated from code (no copied ROM art).

Outputs (repo-relative `assets/e2e-art/`):
  sprite-plumber-tribute.figmap    40x20  BG gradient + hero + sparkle
  sprite-quest-tribute.figmap      40x20  BG gradient + hero + sparkle
  sprite-invader-tribute.figmap    40x20  BG gradient + hero + sparkle
  layers-castle-tribute.figmap     64x20  3 layers: sky / castle / hero
  banner-figby-finale.figmap       64x24  4 layers, 6 frames, lights + keyframes

Each file is validated by round-tripping through `figby::figmap::load_figmap`
shape checks in the companion test; run:
  python3 scripts/gen-e2e-art.py
  cargo test --manifest-path figby-rs/Cargo.toml e2e_art
"""
import json
import os
import subprocess
import sys

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(REPO, "assets", "e2e-art")
FIGBY = os.path.join(REPO, "figby-rs", "target", "debug", "figby")


def cell(ch, fg=None, bg=None):
    return {"ch": ch, "fg": fg, "bg": bg, "height": None}


def blank(w, h):
    return [[cell(" ") for _ in range(w)] for _ in range(h)]


def layer(name, grid, blend="Normal", opacity=255, visible=True):
    return {
        "buffer": {"cells": grid, "width": len(grid[0]), "height": len(grid)},
        "name": name,
        "visible": visible,
        "locked": False,
        "opacity": opacity,
        "blend_mode": blend,
        "mask": None,
        "group": None,
        "link": None,
        "accepts_lighting": True,
        "casts_shadow": True,
    }


def stamp(grid, ox, oy, art, fg, bg=None):
    """Stamp ASCII art lines onto grid. art: list of (row_string, color_override)."""
    for dy, (row, color) in enumerate(art):
        for dx, ch in enumerate(row):
            if ch == " ":
                continue
            x, y = ox + dx, oy + dy
            if 0 <= y < len(grid) and 0 <= x < len(grid[0]):
                grid[y][x] = cell(ch, color or fg, bg)


NIGHT_RAMP = [" ", "░", "░", "▒", "▒", "▓"]
NIGHT_FG = ["#0B1026", "#141B3D", "#1E2A5A", "#2A3F7A", "#3A55A0", "#4A6AC0"]
DAY_RAMP = ["░", "░", "▒", "▒", "▓", "▓"]
DAY_FG = ["#1A3A6B", "#2A5A9B", "#3A7ACB", "#5AAAEB", "#7ACAFB", "#9AEAFB"]

def gradient_bg(w, h, ramp, fg_chain, dark=False):
    """Vertical gradient + star field; dark=True keeps playfield dim."""
    g = blank(w, h)
    hgt, wdt = len(g), len(g[0])
    for y in range(hgt):
        band = ramp[min(y * len(ramp) // hgt, len(ramp) - 1)]
        fg = fg_chain[min(y * len(fg_chain) // hgt, len(fg_chain) - 1)]
        for x in range(wdt):
            if dark and y >= 4:
                g[y][x] = cell(".", "#1A2340" if ramp is NIGHT_RAMP else "#2A5A8B")
            else:
                g[y][x] = cell(band, fg)
    for y in range(hgt):
        for x in range(wdt):
            if (x * 31 + y * 17 + x * y) % 23 == 0:
                g[y][x] = cell("*" if not dark or y < 4 else ".",
                              "#FFFFAA" if not dark or y < 4 else "#3A4A70")
    return g

def sparkle(grid, pts):
    for x, y, ch, fg in pts:
        if 0 <= y < len(grid) and 0 <= x < len(grid[0]):
            grid[y][x] = cell(ch, fg)


# --- Tribute sprites (original 8x8-ish designs, 8-bit *style*) ---
# Plumber tribute: cap, face, overalls body, boots. NOT the SMB sprite.
PLUMBER = [
    ("  RRRR  ", "#FF0000"),
    (" RRRRRR ", "#FF0000"),
    ("  FFFF  ", "#FFCC99"),
    (" FFFFFF ", "#FFCC99"),
    ("  BBBB  ", "#0000FF"),
    (" BBBBBB ", "#0000FF"),
    ("  B  B  ", "#0000FF"),
    (" KK  KK ", "#663300"),
]
# Quest tribute: hood, tunic, sword arm raised. NOT the Zelda sprite.
QUEST = [
    ("   GG   ", "#00AA00"),
    ("  GGGG  ", "#00AA00"),
    ("  FFFF  ", "#FFCC99"),
    (" GTTTTG ", "#00AA00"),
    ("  TTTT  ", "#8B5A2B"),
    ("  T  SS ", "#8B5A2B"),
    ("  L  LS ", "#663300"),
    (" LL  LL ", "#663300"),
]
# Invader tribute: symmetric bug with antennae + claws. NOT the SI crab.
INVADER = [
    ("A      A", "#00FF00"),
    (" A WW A ", "#00FF00"),
    ("  WWWW  ", "#00FF00"),
    (" WWBBWW ", "#00FF00"),
    ("WWWWWWWW", "#00FF00"),
    ("W WWWW W", "#00FF00"),
    ("  W  W  ", "#00FF00"),
    (" C    C ", "#00FF00"),
]


def sprite_scene(name, sprite, accent, ramp, fg_chain):
    W, H = 40, 20
    bg = gradient_bg(W, H, ramp, fg_chain, dark=True)
    # distant hills flair on the bottom rows
    for x in range(W):
        hill = 2 + int(1.5 * abs(((x * 7) % 11) - 5) / 5)
        for y in range(H - hill, H):
            bg[y][x] = cell("▒", "#2A6A3A" if ramp is DAY_RAMP else "#1A3A5A")
    fg = blank(W, H)
    stamp(fg, 16, 6, sprite, accent)
    sparkle(fg, [(5, 3, "+", "#FFFF00"), (33, 4, "+", "#FFFF00"),
                 (8, 14, ".", "#FFFFFF"), (30, 13, ".", "#FFFFFF")])
    doc = {
        "version": 1, "kind": "Image", "width": W, "height": H,
        "layers": [layer("Background", bg), layer("Hero", fg)],
        "groups": [], "links": [], "active_layer": 1,
        "timeline": None, "palette": [], "lights": [],
    }
    return doc


def castle_scene():
    """Famous-8bit *inspired* castle backdrop: sky, brick keep, hero."""
    W, H = 64, 20
    sky = gradient_bg(W, H, DAY_RAMP, DAY_FG)
    for x in range(W):  # sun
        pass
    stamp(sky, 52, 2, [(" \\|/ ", "#FFFF00"), ("--O--", "#FFFF00"),
                       (" /|\\ ", "#FFFF00")], "#FFFF00")
    for x in range(W):  # clouds
        if x % 13 == 0:
            stamp(sky, x, 3 + (x % 3), [(" cc ", "#FFFFFF"),
                                        ("cccc", "#FFFFFF")], "#FFFFFF")
    castle = blank(W, H)
    keep = [
        ("  ________________  ", "#AA6633"),
        (" |_|_|_|_|_|_|_|_| ", "#AA6633"),
        (" | [][][][][][][] | ", "#CC8855"),
        (" | [][][][][][][] | ", "#CC8855"),
        (" | [][][]|DD|[][] | ", "#CC8855"),
        (" | [][][]|DD|[][] | ", "#CC8855"),
        (" |_____|DD|_____| ", "#AA6633"),
    ]
    stamp(castle, 22, 9, keep, "#AA6633")
    # twin towers
    tower = [(" ___ ", "#AA6633"), ("|_|_|", "#AA6633"), ("|   |", "#CC8855"),
             ("| W |", "#CC8855"), ("|   |", "#CC8855"), ("|___|", "#AA6633")]
    stamp(castle, 12, 10, tower, "#AA6633")
    stamp(castle, 46, 10, tower, "#AA6633")
    # brick ground
    for x in range(W):
        castle[H - 2][x] = cell("=", "#774411")
        castle[H - 1][x] = cell("#", "#553311")
    hero = blank(W, H)
    stamp(hero, 4, 10, PLUMBER, "#FF0000")
    hero_l = layer("Hero", hero)
    castle_l = layer("Castle", castle)
    sky_l = layer("Sky", sky)
    return {
        "version": 1, "kind": "Image", "width": W, "height": H,
        "layers": [sky_l, castle_l, hero_l],
        "groups": [], "links": [], "active_layer": 2,
        "timeline": None, "palette": [], "lights": [],
    }


def banner_scene():
    """FIGBY banner finale: rendered straight from fonts/big via the real CLI."""
    out = subprocess.run([FIGBY, "-f", "fonts/big", "FIGBY"],
                         capture_output=True, text=True, cwd=REPO)
    if out.returncode != 0:
        sys.exit(f"figby banner render failed: {out.stderr}")
    rows = out.stdout.splitlines()
    while rows and not rows[-1].strip():
        rows.pop()
    bw = max(len(r) for r in rows)
    W, H = 64, 24
    # frame 0: banner centered with glow; frames 1..5: sparkle twinkle + drift
    ox = (W - bw) // 2
    oy = 8
    frames = []
    for f in range(6):
        sky = gradient_bg(W, H, NIGHT_RAMP, NIGHT_FG)
        banner = blank(W, H)
        # glow halo behind letters
        for dy, row in enumerate(rows):
            for dx, ch in enumerate(row):
                if ch != " ":
                    for gx in (-1, 1):
                        x, y = ox + dx + gx, oy + dy
                        if 0 <= y < H and 0 <= x < W and banner[y][x]["ch"] == " ":
                            banner[y][x] = cell("░", "#3A55A0")
        for dy, row in enumerate(rows):
            for dx, ch in enumerate(row):
                if ch != " ":
                    banner[oy + dy][ox + dx] = cell(ch, "#FFD700")
        # twinkle sparkles, deterministic per frame
        for i in range(10):
            x = (i * 13 + f * 7) % W
            y = (i * 7 + f * 3) % H
            if banner[y][x]["ch"] == " ":
                banner[y][x] = cell("+" if (i + f) % 2 else ".", "#FFFFFF")
        # rising ember particles (tribute to the particle system)
        for i in range(6):
            x = (i * 11 + f * 2) % W
            y = (H - 2 - f - i) % H
            if banner[y][x]["ch"] == " ":
                banner[y][x] = cell("*", "#FF8800")
        comp = blank(W, H)
        for y in range(H):
            for x in range(W):
                top = banner[y][x]
                comp[y][x] = top if top["ch"] != " " else sky[y][x]
        frames.append(comp)
    # document_state holds ONE layer per frame slot (banner layer snapshot)
    tl_frames = []
    for i, comp in enumerate(frames):
        thumb = [[comp[y][x]["ch"] for x in range(0, W, 8)]
                 for y in range(0, H, 8)]
        tl_frames.append({
            "thumbnail": thumb, "has_keyframe": False,
            "label": f"F{i}", "delay": 12,
            "document_state": [{
                "cells": comp, "width": W, "height": H}],
            "layer_keyframes": [None],
        })
    sky0 = gradient_bg(W, H, NIGHT_RAMP, NIGHT_FG)
    lights = [
        {"Ambient": {"intensity": 0.35, "color": "#8888FF", "target": "Both"}},
        {"Point": {"position": [32.0, 4.0, 5.0], "intensity": 1.2,
                   "color": "#FFD700", "attenuation": {"constant": 1.0,
                    "linear": 0.09, "quadratic": 0.032}, "target": "Both"}},
    ]
    light_kfs = [
        {"time": 0.0, "light_index": 1,
         "properties": {"position": [8.0, 4.0, 5.0], "intensity": 0.6,
                        "color": None, "direction": None, "attenuation": None},
         "easing": "Linear"},
        {"time": 1.0, "light_index": 1,
         "properties": {"position": [56.0, 4.0, 5.0], "intensity": 1.4,
                        "color": None, "direction": None, "attenuation": None},
         "easing": "Linear"},
    ]
    return {
        "version": 1, "kind": "Animation", "width": W, "height": H,
        "layers": [layer("Sky", sky0)],
        "groups": [], "links": [], "active_layer": 0,
        "timeline": {"fps": 8, "loop_enabled": True, "frames": tl_frames,
                     "light_keyframes": light_kfs},
        "palette": [], "lights": lights,
    }


def main():
    os.makedirs(OUT, exist_ok=True)
    scenes = {
        "sprite-plumber-tribute.figmap":
            sprite_scene("plumber", PLUMBER, "#FF0000", NIGHT_RAMP, NIGHT_FG),
        "sprite-quest-tribute.figmap":
            sprite_scene("quest", QUEST, "#00AA00", DAY_RAMP, DAY_FG),
        "sprite-invader-tribute.figmap":
            sprite_scene("invader", INVADER, "#00FF00", NIGHT_RAMP, NIGHT_FG),
        "layers-castle-tribute.figmap": castle_scene(),
        "banner-figby-finale.figmap": banner_scene(),
    }
    for name, doc in scenes.items():
        path = os.path.join(OUT, name)
        with open(path, "w") as f:
            json.dump(doc, f, indent=1)
        print(f"wrote {path} ({doc['width']}x{doc['height']}, "
              f"{len(doc['layers'])} layers)")


if __name__ == "__main__":
    main()
