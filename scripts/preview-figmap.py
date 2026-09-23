#!/usr/bin/env python3
"""Render a .figmap scene as ANSI truecolor to stdout (for demo/recording).

Usage:
  preview-figmap.py <file.figmap> [--frames N] [--hold SECS] [--no-loop]

Static (kind=Image): composites visible layers bottom-to-top, prints once,
holds SECS. Animation: plays timeline frames using each frame's own delay
(centiseconds, clamped to >=0.05s), looping N times (default 2).
"""
import json
import sys
import time

NAMED = {
    "Black": 30, "Red": 31, "Green": 32, "Yellow": 33, "Blue": 34,
    "Magenta": 35, "Cyan": 36, "Gray": 37, "DarkGray": 90,
    "LightRed": 91, "LightGreen": 92, "LightYellow": 93, "LightBlue": 94,
    "LightMagenta": 95, "LightCyan": 96, "White": 97,
}


def esc(fg, bg):
    parts = []
    if fg:
        parts.append(fg)
    if bg:
        parts.append(bg)
    return f"\x1b[{';'.join(parts)}m" if parts else ""


def color_code(c, bg=False):
    if c is None or c == "Reset":
        return None
    base = 40 if bg else 30
    if isinstance(c, str) and c.startswith("#") and len(c) == 7:
        r, g, b = int(c[1:3], 16), int(c[3:5], 16), int(c[5:7], 16)
        return f"{base + 8};2;{r};{g};{b}"
    if isinstance(c, str) and c in NAMED:
        return str(NAMED[c] + (base - 30))
    return None


def paint(cell):
    ch = cell.get("ch", " ")
    seq = esc(color_code(cell.get("fg")), color_code(cell.get("bg"), True))
    return f"{seq}{ch}\x1b[0m" if seq else ch


def composite(doc):
    w, h = doc["width"], doc["height"]
    grid = [[{"ch": " "} for _ in range(w)] for _ in range(h)]
    for layer in doc["layers"]:
        if not layer.get("visible", True):
            continue
        buf = layer["buffer"]
        for y in range(min(h, buf["height"])):
            for x in range(min(w, buf["width"])):
                top = buf["cells"][y][x]
                if top.get("ch", " ") == " " and not top.get("fg") and not top.get("bg"):
                    continue
                grid[y][x] = top
    return grid


def show(grid):
    out = []
    for row in grid:
        out.append("".join(paint(c) for c in row).rstrip())
    sys.stdout.write("\n".join(out) + "\n")
    sys.stdout.flush()


def main():
    args = sys.argv[1:]
    path = args[0]
    loops = 2
    hold = 3.0
    i = 1
    while i < len(args):
        if args[i] == "--frames":
            loops = int(args[i + 1]); i += 2
        elif args[i] == "--hold":
            hold = float(args[i + 1]); i += 2
        else:
            i += 1
    with open(path) as f:
        doc = json.load(f)
    tl = doc.get("timeline")
    if tl and tl.get("frames"):
        for _ in range(loops):
            for fr in tl["frames"]:
                ds = fr.get("document_state")
                if ds:
                    buf = ds[0]
                    grid = buf["cells"]
                else:
                    grid = composite(doc)
                sys.stdout.write("\x1b[2J\x1b[H")
                show(grid)
                time.sleep(max(fr.get("delay", 12) / 100.0, 0.05))
    else:
        show(composite(doc))
        time.sleep(hold)


if __name__ == "__main__":
    main()
