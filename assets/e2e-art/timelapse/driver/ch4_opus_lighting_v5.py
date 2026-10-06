"""Chapter 4b v5 (opus take) — two point lights roam the title like searchlights.

Setup as v4: the painted Shadow/Extrude/Face project, the title's colours loaded as lighting
swatches, Shadow kept out of lighting (Extrude and Face lit), ambient 0.4. The TUI's light
panel has no light-colour control (only position, intensity, add/remove), so both lights
stay white.

Choreography (light 1 = "A", light 2 = "B"; both added with P, both at intensity 1.0):
  1. Both spawn at the canvas centre, one stacked on the other (an overlap flare on G/B),
     then split apart: A to the left edge, B to the right edge.
  2. A sweeps left to right through the letters on a slow wave. B, moving about twice as fast,
     traces a full figure-eight across the word starting from the right, so the two pools
     cross in the middle (overlapping and brightening), part, and cross again.
  3. Both swing up and park over the upper-left of the word, side by side; ambient goes back
     to 0.5 and a head-on directional fill (D) arrives at 0.8 and eases to 0.3 (v4's settle).
  4. Saves title/figby-title-opus-lit-v5.figmap.

The lighting panel moves only the selected light, so each tick selects A (Up/Down), steps it,
selects B, steps it. Per-tick step limits give the two lights different speeds.

Usage (repo root, daemon.py running; rebuild figby-rs/target/debug/figby first):
  python3 assets/e2e-art/timelapse/driver/ch4_opus_lighting_v5.py --dry      # no recording / saves
  python3 assets/e2e-art/timelapse/driver/ch4_opus_lighting_v5.py            # record recordings/04-title-lighting-opus-v5-raw.cast
  python3 assets/e2e-art/timelapse/driver/ch4_opus_lighting_v5.py --render   # -> timelapse/04-title-lighting-opus-v5.cast + .gif
"""
import math, os, sys
from ui import *
import ch2, ch4
import ch4_opus_lighting as v1
from ch4_opus_lighting import intensity, W, H

LIT = v1.TITLE + "/figby-title-opus-lit-v5.figmap"
TIMELAPSE = "assets/e2e-art/timelapse/04-title-lighting-opus-v5"
RAW = "assets/e2e-art/recordings/04-title-lighting-opus-v5-raw.cast"
DEBUG = os.environ.get("OPUS_DEBUG")

AMB, A, B = 0, 1, 2                          # scene light indices
pos = {A: [W // 2, H // 2], B: [W // 2, H // 2]}
sel = [AMB]                                  # currently selected light index


def select(i):
    while sel[0] < i: key("Down"); sel[0] += 1
    while sel[0] > i: key("Up"); sel[0] -= 1


def nudge(i, tx, ty, max_step):
    """Move light i up to max_step cells per axis towards (tx, ty)."""
    tx, ty = max(0, min(W - 1, round(tx))), max(0, min(H - 1, round(ty)))
    p = pos[i]
    if p == [tx, ty]: return
    select(i)
    for _ in range(min(max_step, abs(tx - p[0]))):
        key("Right" if tx > p[0] else "Left"); p[0] += 1 if tx > p[0] else -1
    for _ in range(min(max_step, abs(ty - p[1]))):
        text("\x1b[1;2B" if ty > p[1] else "\x1b[1;2A"); p[1] += 1 if ty > p[1] else -1


def tick(targets, dt):
    """targets: {light: (x, y, max_step)}; one movement step for every light, then a pause."""
    for i, (x, y, s) in targets.items(): nudge(i, x, y, s)
    time.sleep(dt)


def travel(targets, dt, limit=80):
    """Tick until every light has reached its target."""
    for _ in range(limit):
        if all(pos[i] == [max(0, min(W - 1, round(x))), max(0, min(H - 1, round(y)))]
               for i, (x, y, _) in targets.items()):
            return
        tick(targets, dt)


def lights_up():
    side_tab("props"); time.sleep(.3)        # keep the Layers tab from catching +/-, D, Shift+arrows
    text("G"); time.sleep(.8)
    assert "LIGHTING" in screen(), "lighting mode not active"
    ch4.calibrate()
    sel[0] = AMB
    time.sleep(.8)
    intensity(-1)                            # ambient 0.5 -> 0.4
    assert "Amb  0.40" in screen(), screen().split("\n")[5:8]
    time.sleep(.5)
    key("P"); sel[0] = A; time.sleep(.4); intensity(+2)     # A: 0.8 -> 1.0
    key("P"); sel[0] = B; time.sleep(.4); intensity(+2)     # B: stacked on A at the centre
    assert screen().count("Pnt") >= 2, screen().split("\n")[5:9]
    time.sleep(1.0)                                          # the doubled flare on G/B
    travel({A: (2, 10, 1), B: (52, 11, 1)}, .1)              # split: A left, B right
    time.sleep(.5)


def searchlights():
    """A: left to right on a slow wave. B: a figure-eight from the right, ~2x faster."""
    n = 54
    for k in range(1, n + 1):
        t = k / n
        ax, ay = 2 + 53 * t, 11 - 2 * math.sin(2 * math.pi * 2 * t)
        th = math.pi / 2 + 2 * math.pi * t
        bx, by = 27 + 25 * math.sin(th), 11 + 4 * math.sin(2 * th)
        tick({A: (ax, ay, 2), B: (bx, by, 4)}, .14)
        if DEBUG and k % 6 == 0: print("search", k, pos[A], pos[B], snap(), flush=True)
    time.sleep(.4)


def settle():
    """Swing both up over the word and park them side by side above the upper-left."""
    travel({A: (40, 4, 2), B: (30, 3, 2)}, .08)
    travel({A: (15, 6, 2), B: (8, 6, 2)}, .1)
    time.sleep(.8)
    select(AMB); time.sleep(.3)
    intensity(+1)                            # ambient 0.4 -> 0.5
    key("D"); sel[0] = 3; time.sleep(.6)     # head-on directional fill arrives at 0.8 ...
    intensity(-5)                            # ... and eases back to 0.3
    time.sleep(3.0)                          # hold the final lit title


def save():
    key("Escape"); time.sleep(.6)            # leave lighting mode; the scene stays on
    ch2.save_as(LIT)
    time.sleep(1.5)


STEPS = (v1.swatches, v1.keep_shadow_dark, lights_up, searchlights, settle, save)


def render():
    v1.TIMELAPSE = TIMELAPSE                 # v1's pipeline, pointed at the v5 outputs
    v1.render(raw=RAW, speed="3", hold=2.0)   # two lights = twice the keystrokes per tick


if __name__ == "__main__":
    if sys.argv[1:2] == ["--render"]:
        render(); sys.exit()
    DRY = sys.argv[1:2] == ["--dry"]
    v1.prepare()
    args = {"cols": 140, "rows": 50, "figmap": v1.PAINTED}
    if not DRY: args["record"] = "04-title-lighting-opus-v5-raw"
    call("stop")
    r = call("launch", args); assert "error" not in r, r
    time.sleep(2.0); ch4.calibrate(); time.sleep(1.0)
    for step in STEPS:
        if DRY and step is save: continue
        step(); print(step.__name__, snap(), flush=True)
    time.sleep(1.0)
    print(snap())
    if not DRY: print(call("record_stop", {"make_gif": False}))
