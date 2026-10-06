"""Chapter 4 (opus take) — paint the FIGBY title as a lit 3D block with the Marker brush.

Approach (differs from ch4.py's horizontal airbrush bands):
  1. Type FIGBY in the bob FIGfont (reuses ch4.title) and duplicate it into three layers:
     Shadow (bottom), Extrude (middle), Face (top).
  2. Move tool: Shadow slides +2,+2 and Extrude +1,+1, so the word gets a stepped 3D extrusion.
  3. Marker recolours each back layer with its own 2-colour ramp: slate -> black cast shadow,
     bright magenta -> magenta extrusion side walls.
  4. Face: light comes from the upper-left. Vertical "drip" strokes (2-wide round brush, +1 ramp
     step per stroke) run down every letter, one pass per ramp step; each pass stops earlier, and
     earlier still towards each letter's right edge, so a diagonal terminator forms per letter:
     white top-left -> cyan -> lavender bottom-right.
  5. Bounce light: one stroke along each letter's bottom edge lifts it a step (reflected light).
  6. Export PNG / TXT / ANSI to opus-named copies of the title assets.

Usage (repo root, daemon.py running):
  python3 assets/e2e-art/timelapse/driver/ch4_opus.py --dry          # drive without recording
  python3 assets/e2e-art/timelapse/driver/ch4_opus.py               # record recordings/04-title-shading-opus-raw.cast
  python3 assets/e2e-art/timelapse/driver/ch4_opus.py --render      # -> timelapse/04-title-shading-opus.cast + .gif
"""
import sys
import ui
from ui import *
import ch4

OUT = "assets/e2e-art/timelapse/title"
# ANSI swatches as (palette group, swatch index); see palette.rs hue_group_for_ansi
FACE = (("blue", 0), ("blue", 1), ("cyan", 0), ("cyan", 1), ("neutral", 3))   # lavender .. white
EXTRUDE = (("purple", 1), ("purple", 0))                                        # bright magenta, magenta
SHADOW = (("neutral", 2), ("neutral", 0))                                       # slate, black
LETTERS = ((4, 13, 6, 17), (15, 17, 6, 17), (19, 28, 6, 17), (30, 39, 6, 17), (41, 50, 6, 20))  # x0 x1 y0 y1
BOUNCE = ((3, 14, 17), (14, 18, 17), (18, 29, 17), (29, 40, 17), (40, 46, 17), (45, 51, 20))
LAYER_ROW = {"Face": 9, "Extrude": 11, "Shadow": 13}      # layer panel rows (top of stack first)


def layer(name):
    side_tab("layers"); click(118, LAYER_ROW[name]); time.sleep(.25)
    assert f"› {name}" in screen(), name


def layers():
    """Duplicate the rasterised title twice and name the three layers."""
    for _ in range(2):
        alt("l"); key("Down"); key("Enter"); time.sleep(.5)     # Layers > Duplicate Layer
    side_tab("layers"); time.sleep(.3)
    for name in ("Shadow", "Extrude", "Face"):
        click(118, LAYER_ROW[name]); time.sleep(.2)
        key("F2"); time.sleep(.1); bs(24); text(name); key("Enter"); time.sleep(.25)
    for name in LAYER_ROW: assert name in screen(), name


def nudge(name, dx, dy):
    """Move tool: drag the whole layer by (dx, dy) cells, one cell at a time."""
    layer(name)
    tool("move"); time.sleep(.2)
    x, y = 24, 11
    pts = [cell(x, y)] + [cell(x + min(i, dx), y + min(i, dy)) for i in range(1, max(dx, dy) + 1)]
    drag(pts); time.sleep(.5)


def ramp(colours):
    """Load a Marker ramp: Normal mode first (clears leftover accumulation), multi-select, Marker."""
    tool("brush"); side_tab("props")
    if "Mode: Marker" in screen():
        key("M"); time.sleep(.3)
    key("j"); time.sleep(.4)
    for grp, idx in colours:
        click(2 + 2 * idx, GROUP_ROW[grp] + 1); time.sleep(.25)
    key("j"); time.sleep(.3)
    key("M"); time.sleep(.4)
    assert "Mode: Marker" in screen(), "marker mode not active"


def shape(name):
    for _ in range(6):
        if f"Shape: {name}" in screen(): return
        key("\\"); time.sleep(.2)
    raise AssertionError(("brush shape", name))


def recolour(name, colours, rows):
    layer(name)
    ramp(colours); shape("Square"); set_size(3)
    for k, y in enumerate(rows):
        pts = [cell(x, y) for x in range(3, 56)]
        drag(pts if k % 2 == 0 else pts[::-1])
    time.sleep(.6)


def depth():
    nudge("Shadow", 2, 2)
    nudge("Extrude", 1, 1)
    recolour("Shadow", SHADOW, range(7, 24, 3))
    recolour("Extrude", EXTRUDE, range(7, 24, 3))


def face_strokes(k=5.8, ax=.45, ay=.8):
    """Per pass p (= ramp step), per letter, per 2-wide column: a top-down stroke ending at the
    deepest row whose light level k*(1 - ax*u - ay*v) still reaches p (u, v = position in the
    letter box, 0 at the upper-left)."""
    out = []
    for p in range(1, len(FACE)):
        for x0, x1, y0, y1 in LETTERS:
            w, h = x1 - x0 + 1, y1 - y0 + 1
            for x in range(x0 + 1, x1 + 2, 2):                # a 2-wide round brush covers x-1, x
                u = (x - .5 - x0) / w
                ys = [y for y in range(y0, y1 + 1) if k * (1 - ax * u - ay * (y - y0) / h) >= p - .5]
                if ys:
                    out.append((p, [(x, y) for y in range(y0 - 1, max(ys) + 1)]))
    return out


def light():
    layer("Face")
    ramp(FACE); shape("Circle"); set_size(2)
    last = 1
    for p, pts in face_strokes():
        if p != last: time.sleep(.6); last = p        # beat between passes
        drag([cell(x, y) for x, y in pts])
    time.sleep(.6)
    for x0, x1, y in BOUNCE:                          # reflected light along the bottom edges
        drag([cell(x, y) for x in range(x0, x1 + 1)])
    time.sleep(.6)
    side_tab("layers"); time.sleep(2.0)               # show the finished stack


def export_static():
    ch4.export("PNG", OUT + "/figby-title-opus.png")
    ch4.export("TXT", OUT + "/figby-title-opus.txt")
    ch4.export("ANSI", OUT + "/figby-title-opus.ansi")


def title():
    ch4.title()                                       # bob FIGfont, blue (= FACE[0]), rasterised


STEPS = (title, layers, depth, light, export_static)
TIMELAPSE = "assets/e2e-art/timelapse/04-title-shading-opus"


def render(raw="assets/e2e-art/recordings/04-title-shading-opus-raw.cast", speed="10", hold=2.5):
    """raw cast -> sped-up cast (speedup.py) -> trimmed before the quit prompt, ending on a hold
    of the finished title -> GIF via agg (same flags as figby-mcp.py)."""
    import json, os, subprocess
    here = os.path.dirname(os.path.abspath(__file__))
    out = TIMELAPSE + ".cast"
    subprocess.run([sys.executable, os.path.join(here, "speedup.py"), raw, out, speed, "0.7"], check=True)
    with open(out) as f:
        header, events = next(f), [json.loads(l) for l in f]
    cut = next((i for i, e in enumerate(events) if e[1] == "o" and "Unsaved Changes" in e[2]), len(events))
    events = events[:cut] + [[hold, "o", ""]]                # empty output keeps the last frame on screen
    with open(out, "w") as f:
        f.write(header)
        for e in events: f.write(json.dumps(e, ensure_ascii=False) + "\n")
    subprocess.run(["agg", "--quiet", "--fps-cap", "24", "--idle-time-limit", str(hold + .5),
                    out, TIMELAPSE + ".gif"], check=True)


if __name__ == "__main__":
    if sys.argv[1:2] == ["--render"]:
        render(); sys.exit()
    DRY = len(sys.argv) > 1 and sys.argv[1] == "--dry"
    if len(sys.argv) > 1 and not DRY and sys.argv[1] in globals() and callable(globals()[sys.argv[1]]):
        for st in sys.argv[1:]: globals()[st]()       # run individual steps against a live session
        print(snap()); sys.exit()
    NAME = None if DRY else (sys.argv[1] if len(sys.argv) > 1 else "04-title-shading-opus-raw")
    call("stop")
    args = {"cols": 140, "rows": 50}
    if NAME: args["record"] = NAME
    r = call("launch", args); assert "error" not in r, r
    time.sleep(1.5)
    for step in STEPS:
        if DRY and step is export_static: continue    # dry runs never touch the title assets
        step(); print(step.__name__, snap(), flush=True)
    time.sleep(2.0)
    print(snap())
    if NAME: print(call("record_stop", {"make_gif": False}))
