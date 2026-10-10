"""Take 2 (bob / space-bunny) — pick up the shaded bob title and light it, then add embers.

Starts from ch4_bob_shade.py's saved figmap (title/figby-title-bob-v1.figmap), so
this is the second half of the same piece:

  swatches  the title's four ramp colours become the lighting LUT swatches, so the
            lit render keeps gold/pink/white instead of going grey
  embers    lighting mode (G): ambient drops 0.5 -> 0.2 and two point lights come up
              A "red"   sinks below the word and sweeps left to right underneath it
                        (the furnace the pink came from)
              B "cyan"  arcs over the top and sweeps left to right (a cold rim),
                        crossing A once on the way
            then both park over the upper-left, ambient back to 0.45 and a head-on
            directional fill arrives at 0.8 and eases to 0.4
  particles Emitter tool clicked at the foot of the Y: a Circle(5) emitter of
            orange sparks, upward with a 25 degree spread, short lifetimes, Despawn
  hold      the lit title under the rising embers

Usage (repo root, daemon.py running, binary freshly built):
  python3 assets/e2e-art/timelapse/driver/ch4_bob_litfx.py --dry
  python3 assets/e2e-art/timelapse/driver/ch4_bob_litfx.py
  python3 assets/e2e-art/timelapse/driver/ch4_bob_litfx.py --render
"""
import json
import math
import os
import subprocess
import sys

import bob
import ch2
import ui
from bob import alt, cal, calib, click, key, screen, tab, text, time, wait
from ui import pick

TITLE_DIR = "assets/e2e-art/timelapse/title"
PAINTED = TITLE_DIR + "/figby-title-bob-v1.figmap"
LIT = TITLE_DIR + "/figby-title-bob-v1-lit.figmap"
TIMELAPSE = "assets/e2e-art/timelapse/05-title-bob-lightfx-v1"
RAW = "assets/e2e-art/recordings/05-title-bob-lightfx-v1-raw.cast"

W, H = 76, 26
# Renderer colours, in the order they have to be pushed through Recent: the
# lighting LUT is built from the palette editor's swatch list.
SWATCHES = (("red", 0), ("yellow", 0), ("yellow", 1), ("neutral", 3))
COLOUR_A = "red"          # furnace underlight
COLOUR_B = "cyan"         # cold rim
AMB, A, B = 0, 1, 2
sel = [AMB]
pos = {A: [W // 2, H // 2], B: [W // 2, H // 2]}
EMITTER_AT = (37, 21)     # canvas cell at the foot of the Y


def prepare():
    if os.path.exists(PAINTED):
        return
    import ch4_bob_shade
    bob.call("stop")
    r = bob.call("launch", {"cols": 140, "rows": 50})
    assert "error" not in r, r
    time.sleep(1.5)
    for st in (ch4_bob_shade.stage, ch4_bob_shade.title, ch4_bob_shade.load_ramp,
               ch4_bob_shade.shade, ch4_bob_shade.save):
        st()
    bob.call("stop")


def select(i):
    while sel[0] < i:
        key("Down")
        sel[0] += 1
    while sel[0] > i:
        key("Up")
        sel[0] -= 1


def nudge(i, tx, ty, max_step):
    tx = max(0, min(W - 1, round(tx)))
    ty = max(0, min(H - 1, round(ty)))
    p = pos[i]
    if p == [tx, ty]:
        return
    select(i)
    for _ in range(min(max_step, abs(tx - p[0]))):
        key("Right" if tx > p[0] else "Left")
        p[0] += 1 if tx > p[0] else -1
    for _ in range(min(max_step, abs(ty - p[1]))):
        text("\x1b[1;2B" if ty > p[1] else "\x1b[1;2A")
        p[1] += 1 if ty > p[1] else -1


def travel(targets, dt, limit=90):
    for _ in range(limit):
        if all(pos[i] == [max(0, min(W - 1, round(x))), max(0, min(H - 1, round(y)))]
               for i, (x, y, _) in targets.items()):
            return
        for i, (x, y, s) in targets.items():
            nudge(i, x, y, s)
        time.sleep(dt)


def swatches():
    for grp, idx in SWATCHES:
        pick(grp, idx)
        time.sleep(0.15)
    alt("v")
    [key("Down") for _ in range(6)]
    key("Enter")
    time.sleep(0.8)                      # View > Palette Editor
    assert "Palette Editor" in screen(), "palette editor did not open"
    time.sleep(1.2)
    key("Escape")
    time.sleep(0.5)


def lights_up():
    tab("props")                         # the Layers tab would swallow +/-, D, Shift+arrows
    text("G")
    time.sleep(0.9)
    scr = screen()
    assert "LIGHTING" in scr and "Amb  0.50" in scr, "lighting mode not active"
    assert f"{W}x{H}" in scr, "canvas frame lost entering lighting mode"
    sel[0] = AMB
    time.sleep(0.8)
    bob.intensity(-2)                    # ambient 0.5 -> 0.3: let the lights do the work
    assert "Amb  0.30" in screen(), screen().split("\n")[5:9]
    time.sleep(0.5)
    key("P")                             # A: point light at the centre, white
    sel[0] = A
    time.sleep(0.4)
    bob.intensity(+2)                    # 0.8 -> 1.0
    bob.light_colour(COLOUR_A)
    key("P")                             # B: stacked on A
    sel[0] = B
    time.sleep(0.4)
    bob.intensity(+1)
    bob.light_colour(COLOUR_B)
    assert screen().count("Pnt") >= 2, screen().split("\n")[5:9]
    time.sleep(1.2)                      # the two tints stacked at the centre
    travel({A: (37, 24, 1), B: (37, 24, 1)}, 0.08)
    time.sleep(0.6)


def lights_sweep():
    """A: sinks under the word and runs left to right beneath it. B: climbs over the
    top on a slow arc and runs left to right, a cell behind A so the two overlap once."""
    n = 30
    for k in range(1, n + 1):
        t = k / n
        ax, ay = 2 + 70 * t, 24 - 2 * math.sin(math.pi * t)
        bx, by = 26 + 44 * t, 7 - 3 * math.sin(math.pi * min(1.0, 1.35 * t))
        for i, (x, y) in ((A, (ax, ay)), (B, (bx, by))):
            nudge(i, x, y, 4)
        time.sleep(0.16)
    time.sleep(0.5)


def settle():
    """The two lights park where they frame the word best — the cyan rim up at the
    upper-left (where the painted highlights point) and the red pool low under the
    B — and the scene comes back up: a little ambient plus a head-on directional
    fill that also bevels the letter edges."""
    travel({A: (38, 14, 2), B: (12, 4, 2)}, 0.09)
    time.sleep(0.7)
    select(AMB)
    time.sleep(0.3)
    bob.intensity(+2)                    # ambient 0.3 -> 0.5
    key("D")                             # directional fill, arrives at 0.8
    sel[0] = 3
    time.sleep(0.6)
    bob.intensity(-3)                    # ... and eases back to 0.5
    time.sleep(1.2)


def emitter_field(idx, val, label):
    """Emitter Config: walk to the field by absolute index, retype it, verify on screen."""
    for _ in range(20):
        key("Up")
    for _ in range(idx):
        key("Down")
    key("Enter")
    ui.bs(8)
    text(str(val))
    key("Enter")
    time.sleep(0.1)
    scr = screen()
    assert f"{label}: {val}" in scr, (label, val, scr.split("\n")[2:22])


def emitter_cycle(idx, times, expect):
    """Emission Shape / Edge Mode cycle on Enter: the first Enter only opens the
    field (they have no text buffer), each later Enter commits one cycle step."""
    for _ in range(20):
        key("Up")
    for _ in range(idx):
        key("Down")
    for _ in range(times):
        key("Enter")
        time.sleep(0.2)
        key("Enter")
        time.sleep(0.3)
    assert expect in screen(), (expect, screen().split("\n")[2:22])


def embers():
    """Emitter tool, clicked at the foot of the Y: sparks rise through the letters.
    Lighting mode is left first (the lit render stays baked on the canvas), then the
    canvas origin is re-calibrated — the status bar has no X/Y readout in lighting
    mode, so calib() cannot run there."""
    key("Escape")
    time.sleep(0.8)
    assert "LIGHTING" not in screen(), "still in lighting mode"
    calib()
    click(5, 18, 0.3)                    # Emitter tool
    time.sleep(0.3)
    x, y = cal(*EMITTER_AT)
    click(x, y, 1.0)
    assert "Emitter Config" in screen(), "emitter config did not open"
    for idx, val, label in ((0, 40, "Spawn Rate"), (1, 0.8, "Lifetime Min"),
                            (2, 2.2, "Lifetime Max"), (3, -2, "Vel X Min"),
                            (4, 2, "Vel X Max"), (5, -13, "Vel Y Min"),
                            (6, -5, "Vel Y Max"), (9, 22, "Spread Angle"),
                            (12, "*", "Character"), (13, 255, "Color R"),
                            (14, 170, "Color G"), (15, 60, "Color B")):
        emitter_field(idx, val, label)
    emitter_cycle(10, 1, "Emission Shape: Circle")   # sparks leave from a small disc
    emitter_cycle(17, 2, "Edge Mode: Despawn")       # ... and die rather than wrap
    key("Escape")
    time.sleep(2.5)                      # close the panel; the sparks stream upward
    time.sleep(2.0)


def save():
    ch2.save_as(LIT)
    time.sleep(1.5)
    root = os.path.join(os.path.dirname(os.path.abspath(__file__)), *[".."] * 4)
    assert os.path.exists(os.path.join(root, LIT)), "lit figmap not written"


def finish():
    tab("layers")
    time.sleep(3.0)                      # hold on the lit title under the embers


STEPS_LIST = (swatches, lights_up, lights_sweep, settle, embers, finish, save)


def render(speed="6", hold=2.5, theme="asciinema"):
    here = os.path.dirname(os.path.abspath(__file__))
    out = TIMELAPSE + ".cast"
    subprocess.run([sys.executable, os.path.join(here, "speedup.py"), RAW, out, speed, "0.7"], check=True)
    with open(out) as f:
        header, events = next(f), [json.loads(l) for l in f]
    cut = next((i for i, e in enumerate(events)
                if e[1] == "o" and ("Unsaved Changes" in e[2] or "\x1b[?1049l" in e[2])), len(events))
    first = next((i for i, e in enumerate(events) if e[1] == "o" and f"{W}x{H}" in e[2]), 0)
    for e in events[:first]:
        e[0] = 0.0
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
    DRY = sys.argv[1:2] == ["--dry"]
    prepare()
    args = {"cols": 140, "rows": 50, "figmap": PAINTED}
    if not DRY:
        args["record"] = "05-title-bob-lightfx-v1-raw"
    bob.call("stop")
    r = bob.call("launch", args)
    assert "error" not in r, r
    time.sleep(2.5)
    calib()
    time.sleep(1.0)
    for step in STEPS_LIST:
        if DRY and step is save:
            continue
        step()
        print(step.__name__, ui.snap(), flush=True)
    time.sleep(1.0)
    print(ui.snap())
    if not DRY:
        print(bob.call("record_stop", {"make_gif": False}))