"""Chapter 4b v2 (opus take) — light the 3D FIGBY title with a pool of light that travels
across the letter faces (needs the height-aware lighting shadows from d9d2abd).

v1 (ch4_opus_lighting.py) could only skim the outer rim of the word, because every filled
cell shadowed its neighbours. With height-aware shadows a point light now lights the faces,
so this take is built around a moving pool of light:
  1. Opens title/figby-title-opus.figmap (painted Shadow/Extrude/Face stack), loads the
     title's ANSI ramp as lighting swatches, and keeps the Shadow layer out of lighting
     (a cast shadow should stay dark; layers no longer cast silhouette shadows on flat art).
  2. Lighting mode on the Props tab: ambient 0.5 -> 0.4 (dim but the extrusion still reads),
     a point light (P, 1.0) slips off to the left of the word.
  3. The light drifts left to right through the middle of the letters on a slow wave, so the
     pool washes over F, I, G, B, Y in turn, white/cyan at its core and falling off to
     ambient a few cells out.
  4. It loops back right to left across the upper half and settles over the upper-left of the
     word (the direction the painted highlights imply); ambient goes back to 0.5 and a
     head-on directional fill (D) arrives at 0.8 and eases to 0.3.
  5. Saves the lit result as title/figby-title-opus-lit-v2.figmap.

Usage (repo root, daemon.py running; rebuild figby-rs/target/debug/figby first):
  python3 assets/e2e-art/timelapse/driver/ch4_opus_lighting_v2.py --dry      # no recording / saves
  python3 assets/e2e-art/timelapse/driver/ch4_opus_lighting_v2.py            # record recordings/04-title-lighting-opus-v2-raw.cast
  python3 assets/e2e-art/timelapse/driver/ch4_opus_lighting_v2.py --render   # -> timelapse/04-title-lighting-opus-v2.cast + .gif
"""
import math, os, sys
from ui import *
import ch2, ch4
import ch4_opus_lighting as v1
from ch4_opus_lighting import step_to, intensity, light_pos, pos, W, H

PAINTED = v1.PAINTED
LIT = v1.TITLE + "/figby-title-opus-lit-v2.figmap"
TIMELAPSE = "assets/e2e-art/timelapse/04-title-lighting-opus-v2"
RAW = "assets/e2e-art/recordings/04-title-lighting-opus-v2-raw.cast"
DEBUG = os.environ.get("OPUS_DEBUG")


def lights_up():
    side_tab("props"); time.sleep(.3)        # keep the Layers tab from catching +/-, D, Shift+arrows
    text("G"); time.sleep(.8)
    assert "LIGHTING" in screen(), "lighting mode not active"
    ch4.calibrate()
    time.sleep(.8)
    intensity(-1)                            # ambient 0.5 -> 0.4
    assert "Amb  0.40" in screen(), screen().split("\n")[5:8]
    time.sleep(.6)
    key("P"); time.sleep(.5)                 # point light appears at the centre: a pool on G/B
    intensity(+2)                            # 0.8 -> 1.0
    pos[:] = [W // 2, H // 2]
    time.sleep(.8)
    step_to(W // 2, 21, .05)                 # duck below the word
    step_to(1, 21, .03)                      # round to the left
    step_to(1, 11, .07)
    time.sleep(.5)


def drift():
    """Left to right through the letters' middle on a slow wave (y 9..13)."""
    for x in range(2, 58):
        y = round(11 - 2 * math.sin(2 * math.pi * (x - 2) / 28))
        step_to(x, y, .14)
        if DEBUG and x % 8 == 2: print("drift", x, y, snap(), flush=True)
    time.sleep(.4)


def loop_back():
    """Rise on the right and come back across the upper half, slowing as it settles over the
    upper-left of the word."""
    step_to(57, 7, .09)
    for x in range(56, 10, -1):
        y = round(7 - 1.0 * math.sin(math.pi * (x - 11) / 46))
        step_to(x, y, .06 if x > 24 else .12)
        if DEBUG and x % 12 == 0: print("loop", x, y, snap(), flush=True)
    step_to(11, 7, .12)
    p = light_pos()
    assert p is None or p == (11, 7), ("light drifted", p, pos)
    time.sleep(.8)


def fill():
    key("Up"); time.sleep(.3)                # ambient
    intensity(+1)                            # 0.4 -> 0.5
    key("D"); time.sleep(.6)                 # head-on directional fill arrives at 0.8 ...
    intensity(-5)                            # ... and eases back to 0.3
    time.sleep(3.0)                          # hold the final lit title


def save():
    key("Escape"); time.sleep(.6)            # leave lighting mode; the scene stays on
    ch2.save_as(LIT)
    time.sleep(1.5)


STEPS = (v1.swatches, v1.keep_shadow_dark, lights_up, drift, loop_back, fill, save)


def render():
    """v1's pipeline (speedup.py x2, skip the welcome flash, cut before exit, hold, agg)."""
    v1.TIMELAPSE = TIMELAPSE
    v1.render(raw=RAW, speed="2", hold=2.0)


if __name__ == "__main__":
    if sys.argv[1:2] == ["--render"]:
        render(); sys.exit()
    DRY = sys.argv[1:2] == ["--dry"]
    v1.prepare()
    args = {"cols": 140, "rows": 50, "figmap": PAINTED}
    if not DRY: args["record"] = "04-title-lighting-opus-v2-raw"
    call("stop")
    r = call("launch", args); assert "error" not in r, r
    time.sleep(2.0); ch4.calibrate(); time.sleep(1.0)
    for step in STEPS:
        if DRY and step is save: continue
        step(); print(step.__name__, snap(), flush=True)
    time.sleep(1.0)
    print(snap())
    if not DRY: print(call("record_stop", {"make_gif": False}))
