"""Chapter 4b v3 (opus take) — v2's travelling pool of light, with only the Face layer lit.

In v2 the Extrude layer (the ░/▒ side walls showing through gaps such as the space between
F's bars) accepted lighting, so the pool lit those gaps as if they were face. v3 keeps both
back layers out of the lighting pass (L in the layer panel on Shadow and Extrude), so the
extrusion keeps its painted magenta and the cast shadow stays dark; only the Face is lit.
Everything else (ambient 0.4, the left-to-right drift, the loop back over the upper half,
the ambient 0.5 + directional 0.3 fill) is v2's choreography.

Usage (repo root, daemon.py running; rebuild figby-rs/target/debug/figby first):
  python3 assets/e2e-art/timelapse/driver/ch4_opus_lighting_v3.py --dry      # no recording / saves
  python3 assets/e2e-art/timelapse/driver/ch4_opus_lighting_v3.py            # record recordings/04-title-lighting-opus-v3-raw.cast
  python3 assets/e2e-art/timelapse/driver/ch4_opus_lighting_v3.py --render   # -> timelapse/04-title-lighting-opus-v3.cast + .gif
"""
import sys
from ui import *
import ch2, ch4
import ch4_opus_lighting as v1
import ch4_opus_lighting_v2 as v2

LIT = v1.TITLE + "/figby-title-opus-lit-v3.figmap"
TIMELAPSE = "assets/e2e-art/timelapse/04-title-lighting-opus-v3"
RAW = "assets/e2e-art/recordings/04-title-lighting-opus-v3-raw.cast"


def face_only():
    """Shadow and Extrude out of the lighting pass; Face stays the active layer."""
    side_tab("layers")
    for name in ("Shadow", "Extrude"):
        click(118, v1.LAYER_ROW[name]); time.sleep(.2); key("L"); time.sleep(.3)
    click(118, v1.LAYER_ROW["Face"]); time.sleep(.3)


def save():
    key("Escape"); time.sleep(.6)            # leave lighting mode; the scene stays on
    ch2.save_as(LIT)
    time.sleep(1.5)


STEPS = (v1.swatches, face_only, v2.lights_up, v2.drift, v2.loop_back, v2.fill, save)


def render():
    v1.TIMELAPSE = TIMELAPSE                 # v1's pipeline, pointed at the v3 outputs
    v1.render(raw=RAW, speed="2", hold=2.0)


if __name__ == "__main__":
    if sys.argv[1:2] == ["--render"]:
        render(); sys.exit()
    DRY = sys.argv[1:2] == ["--dry"]
    v1.prepare()
    args = {"cols": 140, "rows": 50, "figmap": v1.PAINTED}
    if not DRY: args["record"] = "04-title-lighting-opus-v3-raw"
    call("stop")
    r = call("launch", args); assert "error" not in r, r
    time.sleep(2.0); ch4.calibrate(); time.sleep(1.0)
    for step in STEPS:
        if DRY and step is save: continue
        step(); print(step.__name__, snap(), flush=True)
    time.sleep(1.0)
    print(snap())
    if not DRY: print(call("record_stop", {"make_gif": False}))
