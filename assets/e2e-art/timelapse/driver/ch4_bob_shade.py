"""Take 1 (bob / space-bunny) — build the FIGBY title in the bob FIGfont and shade it
with the Marker brush, recorded as a timelapse.

What makes this take different from the existing ch4/ch4_opus ones: instead of
building a 3-layer stepped extrusion, it shades one flat layer with a single
Marker ramp, sweeping row-runs bottom-up in four passes:

  base Red -> Yellow -> BrightYellow -> BrightWhite

The level function is a diagonal light falloff (5 o'clock, upper-left), so each
pass paints a smaller, higher and further-left region and the terminator walks
diagonally across the word.  Two extra passes give it a chrome edge: the topmost
painted cell of every column is pushed to white (rim) and the bottommost cell of
every column gets one step (bounce light off an unseen floor).  The result stays
one flat layer, which is what video 2 lights.

Steps (each is a visible beat in the recording):
  stage  new 76x26 canvas, calibrate the canvas origin from the status bar
  title  Text tool -> "FIGBY" -> bob FIGfont -> Red -> rasterise
  ramp   Marker mode on a Red/Yellow/BrightYellow/BrightWhite ramp, size 1
  shade  four bottom-up sweeps: Yellow, BrightYellow, white, then the edges
  save   Save as Figmap (video 2 starts from this file)

Usage (repo root, daemon.py running, binary freshly built):
  python3 assets/e2e-art/timelapse/driver/ch4_bob_shade.py --dry     # drive only
  python3 assets/e2e-art/timelapse/driver/ch4_bob_shade.py           # record the raw cast
  python3 assets/e2e-art/timelapse/driver/ch4_bob_shade.py --render  # -> timelapse/*.cast+.gif
"""
import json
import os
import subprocess
import sys

import bob
import ch2
import ui
from bob import alt, brush_mode, brush_shape, brush_size, calib, cal, click, key
from bob import ramp, runs_by_row, screen, sweep, tab, text, time, wait
from ui import pick

TITLE_DIR = "assets/e2e-art/timelapse/title"
PAINTED = TITLE_DIR + "/figby-title-bob-v1.figmap"
TIMELAPSE = "assets/e2e-art/timelapse/04-title-bob-shading-v1"
RAW = "assets/e2e-art/recordings/04-title-bob-shading-v1-raw.cast"

W, H = 76, 26
ORIGIN = (14, 3)                      # canvas cell the text block's top-left lands on

# Marker ramp, click order. Verified by hovering each swatch before clicking:
# (palette flat order is Black, White, BrightBlack, BrightWhite, Red, BrightRed,
# Yellow, BrightYellow, ...).  The title is placed in Red so sweep 1 == Yellow.
RAMP = (("red", 0, "Red"), ("yellow", 0, "Yellow"),
        ("yellow", 1, "Bright Yellow"), ("neutral", 3, "Bright White"))
BASE = RAMP[0]                        # the title is placed in Red

# bob FIGBY, exactly as `figby -f fonts/bob FIGBY` renders it: 18 rows x 48 cols,
# glyph body in rows 3..17. Kept here so the sweeps can be computed, not guessed.
GLYPH = [
    "                                                ",
    "                                                ",
    "                                                ",
    "█████████░ ██▓ █████████░ █████████░ ██▓   ▓██░ ",
    "█████████▒ ███ █████████▒ █████████▒ ███   ███▒ ",
    "█████████░ ███ █████████░ █████████▒ ███   ███▒ ",
    "███▓░░░░   ███ ███▒░░░░   ███▒░▒███▒ ███   ███▒ ",
    "████▓▓▓▓▓  ███ ███        ███▓░▓███▒ ███   ███▒ ",
    "█████████▒ ███ ███   ░▒▒  █████████▒ ███   ███▒ ",
    "█████████▒ ███ ███   ███▒ █████████▒ ███   ███▒ ",
    "███▓▒▒▒▒▒  ███ ███░  ███▒ ███▒ ▒███▒ ███░  ███▒ ",
    "███        ███ ███▓▒▒███▒ ███▓▒▓███▒ ███▓▒▓███▒ ",
    "███        ███ █████████▒ █████████▒ █████████▒ ",
    "███        ███ █████████▒ █████████▒ █████████▒ ",
    "▓█▓        ▓█▓ ▓███████▓  ▓███████▓  ▓████████▒ ",
    "                                           ███▒ ",
    "                                           ███▒ ",
    "                                           ▒██░ ",
]
GH, GW = len(GLYPH), max(len(r) for r in GLYPH)
TOP, BOT = 3, 17                       # first / last glyph body row
STEPS = len(RAMP) - 1                  # three Marker passes


def painted():
    return {(j, i) for j in range(GH) for i in range(GW) if GLYPH[j][i] != " "}


CELLS = painted()
COL_TOP = {i: min(j for j, k in CELLS if k == i) for j, i in CELLS}
COL_BOT = {i: max(j for j, k in CELLS if k == i) for j, i in CELLS}


def level(j, i):
    """Diagonal light level, capped at the top of the ramp: light from the upper
    left, falling off 70% down the letter body and 25% along the word. A cell
    ends up `floor(level)` steps up the ramp, so the three sweeps split the glyph
    roughly 5/32/38/24% over Red / Yellow / BrightYellow / white and the
    terminator walks down-left to up-right."""
    v = (j - TOP) / (BOT - TOP)
    u = i / (GW - 1)
    return min(float(STEPS), 3.9 * (1.0 - 0.70 * v - 0.25 * u))


def pass_cells(p):
    """Cells the p-th sweep should step once. p is 1-based over STEPS, plus the
    two edge passes (STEPS + 1 = white rim, STEPS + 2 = bottom bounce)."""
    out = set()
    for j, i in CELLS:
        if p <= STEPS:
            if level(j, i) >= p:
                out.add((j, i))
        elif p == STEPS + 1:
            if j == COL_TOP[i]:
                out.add((j, i))
        else:
            if j == COL_BOT[i] and level(j, i) < 2:
                out.add((j, i))
    return out


# ------------------------------------------------------------------ steps ----
def stage():
    alt("f")
    key("Enter")
    wait("New Image")
    text("\x7f" * 8 + str(W))
    key("Down")
    time.sleep(0.2)
    text("\x7f" * 8 + str(H))
    key("Enter")
    time.sleep(0.8)
    calib()
    assert f"{W}x{H}" in screen(), "canvas size wrong"


def title():
    click(5, 17, 0.2)                       # Text tool
    time.sleep(0.3)
    tab("text")
    assert "Text Tool" in screen(), "Text Tool panel not open"
    text("FIGBY")
    time.sleep(0.3)
    bob.pick_font("bob")
    pick(*BASE[:2])                            # place the block in the ramp's base colour
    time.sleep(0.3)
    x, y = cal(*ORIGIN)
    ui.mouse(x, y, "move")
    time.sleep(0.9)                          # hover preview
    click(x, y, 0.8)
    assert "Rasterize" in screen(), "text block not placed"
    key("C-r")                               # rasterise so it survives the tool change
    time.sleep(0.8)
    key("Escape")
    time.sleep(0.4)
    assert " Brush " in screen(), "tool did not fall back to Brush"


def load_ramp():
    click(5, 5, 0.2)                         # Brush tool
    time.sleep(0.2)
    brush_shape("Square")
    brush_size(1)
    ramp(RAMP)
    assert "Mode: Marker" in screen()


def shade():
    """Four bottom-up sweeps. Each one steps its cell set up the ramp, so the
    light front visibly climbs the letters."""
    for p in range(1, STEPS + 3):
        cells = pass_cells(p)
        rows = runs_by_row(cells)
        for n, (j, runs) in enumerate(rows):
            for c0, c1 in runs:
                x0, x1 = ORIGIN[0] + c0, ORIGIN[0] + c1
                if (n + p) % 2:
                    sweep(x0, ORIGIN[1] + j, x1, ORIGIN[1] + j)
                else:
                    sweep(x1, ORIGIN[1] + j, x0, ORIGIN[1] + j)
            time.sleep(0.35)                  # let each row's sweep read on screen
        time.sleep(0.9)                       # beat between passes
    tab("layers")
    time.sleep(1.6)                          # let the finished title sit


def save():
    ch2.save_as(PAINTED)
    time.sleep(1.5)
    root = os.path.join(os.path.dirname(os.path.abspath(__file__)), *[".."] * 4)
    assert os.path.exists(os.path.join(root, PAINTED)), "figmap not written"


STEPS_LIST = (stage, title, load_ramp, shade, save)


def render(speed="6", hold=2.5, theme="asciinema"):
    """raw cast -> speedup.py -> trim before the quit prompt -> hold -> agg.

    `theme=asciinema` is deliberate: agg's default (dracula) maps ANSI 3 and ANSI
    11 to two near-identical pale yellows, which collapses this ramp's middle two
    steps into one flat cream. The asciinema theme keeps gold / white / deep gold
    separate, so the sunset ramp reads as four tones.
    """
    here = os.path.dirname(os.path.abspath(__file__))
    out = TIMELAPSE + ".cast"
    subprocess.run([sys.executable, os.path.join(here, "speedup.py"), RAW, out, speed, "0.7"], check=True)
    with open(out) as f:
        header, events = next(f), [json.loads(l) for l in f]
    cut = next((i for i, e in enumerate(events)
                if e[1] == "o" and ("Unsaved Changes" in e[2] or "\x1b[?1049l" in e[2])), len(events))
    events = events[:cut] + [[hold, "o", ""]]
    with open(out, "w") as f:
        f.write(header)
        for e in events:
            f.write(json.dumps(e, ensure_ascii=False) + "\n")
    subprocess.run(["agg", "--quiet", "--theme", theme, "--fps-cap", "24",
                    "--idle-time-limit", str(hold + 0.5), out, TIMELAPSE + ".gif"], check=True)


if __name__ == "__main__":
    if sys.argv[1:2] == ["--render"]:
        render()
        sys.exit()
    if sys.argv[1:2] == ["--stats"]:
        for p in range(1, STEPS + 3):
            print(p, len(pass_cells(p)))
        sys.exit()
    DRY = sys.argv[1:2] == ["--dry"]
    bob.call("stop")
    args = {"cols": 140, "rows": 50}
    if not DRY:
        args["record"] = "04-title-bob-shading-v1-raw"
    r = bob.call("launch", args)
    assert "error" not in r, r
    time.sleep(2.0)
    for step in STEPS_LIST:
        if DRY and step is save:
            continue
        step()
        print(step.__name__, ui.snap(), flush=True)
    time.sleep(1.5)
    print(ui.snap())
    if not DRY:
        print(bob.call("record_stop", {"make_gif": False}))