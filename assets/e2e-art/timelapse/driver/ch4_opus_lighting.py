"""Chapter 4b (opus take) — light the finished 3D FIGBY title with Figby's lighting engine.

Continues from ch4_opus.py: opens the painted Shadow/Extrude/Face project
(title/figby-title-opus.figmap; rebuilt with ch4_opus's steps if missing), then:
  1. Loads the title's ANSI ramp colours as lighting swatches (pick -> Recent ->
     View > Palette Editor), so the lit render keeps the painted hues instead of greys.
  2. Keeps the Shadow layer out of the lighting pass (L in the layer panel): a cast
     shadow should stay dark whatever the light does.
  3. Lighting mode (G), with the side panel on Props (the Layers tab would swallow
     +/-, D and Shift+arrows): ambient drops 0.5 -> 0.2 and a point light (P, 1.0)
     rises up the word's left side and arcs over the top. Figby's lights cast 2D
     shadows in the canvas plane, so a point light reads as a rim light: only the
     outer edges facing it (F's left side, then each letter's top) flare to full colour.
  4. The light swings back to rest up and to the left (the direction the hand-painted
     highlights imply); ambient goes to 0.3 and a head-on directional fill (D) arrives
     at 0.8 and eases back to 0.5, which also bevels the letter edges.
  5. Saves the lit result as title/figby-title-opus-lit.figmap.

Usage (repo root, daemon.py running):
  python3 assets/e2e-art/timelapse/driver/ch4_opus_lighting.py --dry      # drive, no recording / saves
  python3 assets/e2e-art/timelapse/driver/ch4_opus_lighting.py            # record recordings/04-title-lighting-opus-raw.cast
  python3 assets/e2e-art/timelapse/driver/ch4_opus_lighting.py --render   # -> timelapse/04-title-lighting-opus.cast + .gif
"""
import math, os, sys
import ui
from ui import *
import ch2, ch4

TITLE = "assets/e2e-art/timelapse/title"
PAINTED = TITLE + "/figby-title-opus.figmap"
LIT = TITLE + "/figby-title-opus-lit.figmap"
TIMELAPSE = "assets/e2e-art/timelapse/04-title-lighting-opus"
RAW = "assets/e2e-art/recordings/04-title-lighting-opus-raw.cast"
# Recent holds 8 colours: the face ramp, the extrusion pair and the shadow slate (black needs no swatch:
# the Shadow layer is excluded from lighting).
SWATCHES = (("neutral", 2), ("purple", 0), ("purple", 1), ("blue", 0), ("blue", 1),
            ("cyan", 0), ("cyan", 1), ("neutral", 3))
LAYER_ROW = {"Face": 9, "Extrude": 11, "Shadow": 13}
W, H = 64, 24
pos = [W // 2, H // 2]                     # a new point light starts at the canvas centre


def prepare():
    """Rebuild the painted project with ch4_opus if it is missing (not recorded)."""
    if os.path.exists(PAINTED): return
    import ch4_opus
    call("stop"); r = call("launch", {"cols": 140, "rows": 50}); assert "error" not in r, r
    time.sleep(1.5)
    for st in (ch4_opus.title, ch4_opus.layers, ch4_opus.depth, ch4_opus.light): st()
    ch2.save_as(PAINTED)
    call("stop")


def swatches():
    for grp, idx in SWATCHES:
        pick(grp, idx); time.sleep(.15)
    alt("v"); [key("Down") for _ in range(6)]; key("Enter"); time.sleep(.6)   # View > Palette Editor
    assert "Palette Editor" in screen(), "palette editor did not open"
    time.sleep(1.2)                                                          # let the swatch list read
    key("Escape"); time.sleep(.5)


def keep_shadow_dark():
    side_tab("layers")
    click(118, LAYER_ROW["Shadow"]); time.sleep(.2); key("L"); time.sleep(.3)
    click(118, LAYER_ROW["Face"]); time.sleep(.3)


def step_to(x, y, dt=.09):
    """Walk the selected point light to (x, y) one cell at a time, diagonally where possible."""
    while pos[0] != x or pos[1] != y:
        if pos[0] != x:
            key("Right" if x > pos[0] else "Left"); pos[0] += 1 if x > pos[0] else -1
        if pos[1] != y:
            text("\x1b[1;2B" if y > pos[1] else "\x1b[1;2A"); pos[1] += 1 if y > pos[1] else -1
        time.sleep(dt)


def light_pos():
    for yy, l in enumerate(screen().split("\n")):
        if "✦" in l:
            return l.index("✦") - ui.CX0, yy - ui.CY0
    return None


def intensity(delta):
    """+/- 0.1 steps on the selected light (side panel must not be on Layers: it eats +/-)."""
    for _ in range(abs(delta)):
        text("+" if delta > 0 else "-"); time.sleep(.18)


def lights_up():
    side_tab("props"); time.sleep(.3)        # the Layers tab would swallow +/-, D, Shift+arrows
    text("G"); time.sleep(.8)
    assert "LIGHTING" in screen(), "lighting mode not active"
    ch4.calibrate()
    time.sleep(.8)
    intensity(-3)                            # ambient (selected on entry) 0.5 -> 0.2: lights down
    assert "Amb  0.20" in screen(), screen().split("\n")[5:8]
    time.sleep(.8)
    key("P"); time.sleep(.5)                 # point light at the centre, selected
    intensity(+2)                            # 0.8 -> 1.0
    pos[:] = [W // 2, H // 2]
    step_to(pos[0], 21, .05)                 # duck under the word
    step_to(1, 21, .03)
    step_to(1, 6, .1)                        # rise up its left side: F's left edge catches it
    time.sleep(.4)


def over():
    """Arc over the top, left to right. Figby's lights cast 2D shadows in the canvas plane, so
    the light reads as a rim light: each letter's top edge flares as it passes overhead."""
    step_to(3, 3, .1)
    for x in range(4, W - 6):
        y = round(3 - 1.5 * math.sin(math.pi * (x - 4) / (W - 10)))
        step_to(x, y, .15)
        if os.environ.get("OPUS_DEBUG") and x % 12 == 0: print("over", x, snap(), flush=True)
    step_to(W - 8, 9, .1)                    # dip down past Y's right side
    time.sleep(.4)


def settle():
    """Swing back to rest up and to the left of the word (the direction the hand-painted
    highlights imply), then bring the scene up: a little ambient and a head-on directional fill
    that also bevels the letter edges."""
    step_to(W - 12, 1, .05)
    step_to(8, 2, .05)
    p = light_pos()
    assert p is None or p == (8, 2), ("light drifted", p, pos)
    time.sleep(.6)
    key("Up"); time.sleep(.3)                # select the ambient light
    intensity(+1)                            # 0.2 -> 0.3
    key("D"); time.sleep(.6)                 # directional fill, (0, 0, 1), arrives at 0.8
    intensity(-3)                            # ... and eases back to 0.5
    time.sleep(3.0)                          # hold the final lit title


def save():
    key("Escape"); time.sleep(.6)            # leave lighting mode; the scene stays on
    ch2.save_as(LIT)
    time.sleep(1.5)


STEPS = (swatches, keep_shadow_dark, lights_up, over, settle, save)


def render(raw=RAW, speed="2", hold=2.0):
    """Same pipeline as ch4_opus.render: speedup.py, trim before the quit prompt, hold, agg."""
    import json, subprocess
    here = os.path.dirname(os.path.abspath(__file__))
    out = TIMELAPSE + ".cast"
    subprocess.run([sys.executable, os.path.join(here, "speedup.py"), raw, out, speed, "0.7"], check=True)
    with open(out) as f:
        header, events = next(f), [json.loads(l) for l in f]
    # end before the quit prompt / alt-screen exit; skip the welcome screen flashed before the figmap loads
    cut = next((i for i, e in enumerate(events) if e[1] == "o" and
                ("Unsaved Changes" in e[2] or "\x1b[?1049l" in e[2])), len(events))
    first = next((i for i, e in enumerate(events) if e[1] == "o" and "64x24" in e[2]), 0)
    for e in events[:first]: e[0] = 0.0
    events = events[:cut] + [[hold, "o", ""]]
    with open(out, "w") as f:
        f.write(header)
        for e in events: f.write(json.dumps(e, ensure_ascii=False) + "\n")
    subprocess.run(["agg", "--quiet", "--fps-cap", "24", "--idle-time-limit", str(hold + .5),
                    out, TIMELAPSE + ".gif"], check=True)


if __name__ == "__main__":
    if sys.argv[1:2] == ["--render"]:
        render(); sys.exit()
    DRY = sys.argv[1:2] == ["--dry"]
    prepare()
    args = {"cols": 140, "rows": 50, "figmap": PAINTED}
    if not DRY: args["record"] = "04-title-lighting-opus-raw"
    call("stop")
    r = call("launch", args); assert "error" not in r, r
    time.sleep(2.0); ch4.calibrate(); time.sleep(1.0)
    for step in STEPS:
        if DRY and step is save: continue
        step(); print(step.__name__, snap(), flush=True)
    time.sleep(1.0)
    print(snap())
    if not DRY: print(call("record_stop", {"make_gif": False}))
