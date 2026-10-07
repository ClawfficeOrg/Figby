"""Chapter 4b v6 (opus take) — v5's two roaming searchlights, now in two colours (6.0.55).

`C` in lighting mode cycles the selected light's colour: white -> warm -> amber -> red ->
magenta -> blue -> cyan -> green. Light A (the left-to-right wave) is AMBER and light B (the
figure-eight) is MAGENTA. The tint multiplies the lit colour per channel, so a cyan light on
the already-cyan faces barely shows; amber vs magenta reads apart everywhere: peach vs pink
on the white/grey tops, jade vs azure on the cyan faces, and the magenta pool also warms the
purple extrusion. Where the pools cross the blend is a salmon/rose.
(Tried with the OPUS_COLOUR_A/B overrides: amber+cyan, magenta+cyan, warm+blue.)

Everything else is v5: the v4 setup (Shadow out of lighting, Extrude and Face lit,
ambient 0.4), the split from the centre, the wave + figure-eight crossing twice, and the
settle (both park over the upper-left, ambient 0.5, directional fill 0.8 -> 0.3). The parked
lights keep their colours, so the saved lit title is magenta-pink on F and amber-peach on I/G.

Usage (repo root, daemon.py running; rebuild figby-rs/target/debug/figby first):
  python3 assets/e2e-art/timelapse/driver/ch4_opus_lighting_v6.py --dry      # no recording / saves
  python3 assets/e2e-art/timelapse/driver/ch4_opus_lighting_v6.py            # record recordings/04-title-lighting-opus-v6-raw.cast
  python3 assets/e2e-art/timelapse/driver/ch4_opus_lighting_v6.py --render   # -> timelapse/04-title-lighting-opus-v6.cast + .gif
"""
import os, sys
from ui import *
import ch2, ch4
import ch4_opus_lighting as v1
import ch4_opus_lighting_v5 as v5
from ch4_opus_lighting import intensity

LIT = v1.TITLE + "/figby-title-opus-lit-v6.figmap"
TIMELAPSE = "assets/e2e-art/timelapse/04-title-lighting-opus-v6"
RAW = "assets/e2e-art/recordings/04-title-lighting-opus-v6-raw.cast"
PRESETS = ("white", "warm", "amber", "red", "magenta", "blue", "cyan", "green")
COLOUR_A = os.environ.get("OPUS_COLOUR_A", "amber")
COLOUR_B = os.environ.get("OPUS_COLOUR_B", "magenta")


def colour(name):
    """Cycle the selected (freshly added, so white) light to a preset."""
    for _ in range(PRESETS.index(name)):
        key("C"); time.sleep(.25)


def lights_up():
    side_tab("props"); time.sleep(.3)        # keep the Layers tab from catching +/-, D, Shift+arrows
    text("G"); time.sleep(.8)
    assert "LIGHTING" in screen(), "lighting mode not active"
    ch4.calibrate()
    v5.sel[0] = v5.AMB
    time.sleep(.8)
    intensity(-1)                            # ambient 0.5 -> 0.4
    assert "Amb  0.40" in screen(), screen().split("\n")[5:8]
    time.sleep(.5)
    key("P"); v5.sel[0] = v5.A; time.sleep(.4); intensity(+2); colour(COLOUR_A)
    time.sleep(.5)
    key("P"); v5.sel[0] = v5.B; time.sleep(.4); intensity(+2); colour(COLOUR_B)
    assert screen().count("Pnt") >= 2, screen().split("\n")[5:9]
    time.sleep(1.0)                          # stacked at the centre: the two tints blended
    v5.travel({v5.A: (2, 10, 1), v5.B: (52, 11, 1)}, .1)
    time.sleep(.5)


def save():
    key("Escape"); time.sleep(.6)            # leave lighting mode; the scene stays on
    ch2.save_as(LIT)
    time.sleep(1.5)


STEPS = (v1.swatches, v1.keep_shadow_dark, lights_up, v5.searchlights, v5.settle, save)


def render():
    v1.TIMELAPSE = TIMELAPSE                 # v1's pipeline, pointed at the v6 outputs
    v1.render(raw=RAW, speed="3", hold=2.0)  # 3x like v5 (two lights = twice the keystrokes)


if __name__ == "__main__":
    if sys.argv[1:2] == ["--render"]:
        render(); sys.exit()
    DRY = sys.argv[1:2] == ["--dry"]
    v1.prepare()
    args = {"cols": 140, "rows": 50, "figmap": v1.PAINTED}
    if not DRY: args["record"] = "04-title-lighting-opus-v6-raw"
    call("stop")
    r = call("launch", args); assert "error" not in r, r
    time.sleep(2.0); ch4.calibrate(); time.sleep(1.0)
    for step in STEPS:
        if DRY and step is save: continue
        step(); print(step.__name__, snap(), flush=True)
    time.sleep(1.0)
    print(snap())
    if not DRY: print(call("record_stop", {"make_gif": False}))
