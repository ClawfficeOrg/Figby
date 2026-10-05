"""Chapter 2 — open the Chapter 1 checkpoint, build the ship, keyframe + tween it, light it, play it back."""
import sys
from ui import *
import ch2
NAME = sys.argv[1] if len(sys.argv) > 1 else "timelapse-02-ship-animation"
call("stop")
r = call("launch", {"cols": 140, "rows": 50, "record": NAME, "figmap": "assets/e2e-art/timelapse/space-scene-01-build-v3.figmap"}); assert "error" not in r, r
time.sleep(2.0)
ch2.save_as("assets/e2e-art/timelapse/space-scene-02-animation-v3.figmap")
print("saved-as", snap(), flush=True)
for step in (ch2.ship, ch2.keyframes, ch2.tween, ch2.lighting, ch2.playback):
    step(); print(step.__name__, snap(), flush=True)
ch2.save_as("assets/e2e-art/timelapse/space-scene-02-animation-v3.figmap")
time.sleep(2.0)
print(snap())
print(call("record_stop", {"make_gif": False}))
