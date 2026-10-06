"""Chapter 4b v4 (opus take) — the travelling pool of light with the glyph cap (6.0.54).

With lit glyphs now capped at their painted density (lighting::cap_lit_char), a lit ░ stays
░: F's bar gaps and the ░/▒ side walls of the extrusion only change colour, never fill in.
That removes v3's reason to keep the Extrude layer out of lighting, so v4 lights it again
(only the cast Shadow stays excluded). The extrusion now behaves like part of the solid:
it sinks to a deep purple in the dim surround, warms to magenta where the pool passes, and
comes back to full magenta when the fill light comes up at the end.

Choreography is v2's: ambient 0.5 -> 0.4, a point light (1.0) drifts left to right through
the letters on a slow wave, loops back across the upper half, settles over the upper-left;
then ambient 0.5 and a head-on directional fill (0.8 easing to 0.3).

Usage (repo root, daemon.py running; rebuild figby-rs/target/debug/figby first):
  python3 assets/e2e-art/timelapse/driver/ch4_opus_lighting_v4.py --dry      # no recording / saves
  python3 assets/e2e-art/timelapse/driver/ch4_opus_lighting_v4.py            # record recordings/04-title-lighting-opus-v4-raw.cast
  python3 assets/e2e-art/timelapse/driver/ch4_opus_lighting_v4.py --render   # -> timelapse/04-title-lighting-opus-v4.cast + .gif
"""
import sys
from ui import *
import ch2, ch4
import ch4_opus_lighting as v1
import ch4_opus_lighting_v2 as v2

LIT = v1.TITLE + "/figby-title-opus-lit-v4.figmap"
TIMELAPSE = "assets/e2e-art/timelapse/04-title-lighting-opus-v4"
RAW = "assets/e2e-art/recordings/04-title-lighting-opus-v4-raw.cast"


def save():
    key("Escape"); time.sleep(.6)            # leave lighting mode; the scene stays on
    ch2.save_as(LIT)
    time.sleep(1.5)


# Shadow out of lighting (v1.keep_shadow_dark); Face and Extrude lit.
STEPS = (v1.swatches, v1.keep_shadow_dark, v2.lights_up, v2.drift, v2.loop_back, v2.fill, save)


def render():
    v1.TIMELAPSE = TIMELAPSE                 # v1's pipeline, pointed at the v4 outputs
    v1.render(raw=RAW, speed="2", hold=2.0)


if __name__ == "__main__":
    if sys.argv[1:2] == ["--render"]:
        render(); sys.exit()
    DRY = sys.argv[1:2] == ["--dry"]
    v1.prepare()
    args = {"cols": 140, "rows": 50, "figmap": v1.PAINTED}
    if not DRY: args["record"] = "04-title-lighting-opus-v4-raw"
    call("stop")
    r = call("launch", args); assert "error" not in r, r
    time.sleep(2.0); ch4.calibrate(); time.sleep(1.0)
    for step in STEPS:
        if DRY and step is save: continue
        step(); print(step.__name__, snap(), flush=True)
    time.sleep(1.0)
    print(snap())
    if not DRY: print(call("record_stop", {"make_gif": False}))
