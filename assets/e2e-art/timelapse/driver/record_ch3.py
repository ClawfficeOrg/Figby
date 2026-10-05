"""Chapter 3 — effects finale: exhaust emitter, satellite with rotate/move, title fade, light sweep."""
import sys
from ui import *
import ch2, ch3
NAME = sys.argv[1] if len(sys.argv) > 1 else "timelapse-03-effects-finale"
OUT = "assets/e2e-art/timelapse/space-scene-03-effects-v3.figmap"
call("stop")
r = call("launch", {"cols": 140, "rows": 50, "record": NAME, "figmap": "assets/e2e-art/timelapse/space-scene-02-animation-v3.figmap"}); assert "error" not in r, r
time.sleep(2.0)
ch2.save_as(OUT)
ch2.menu(3)                                   # timeline visible
print("saved-as", snap(), flush=True)
for step in (ch3.emitter, ch3.satellite, ch3.motion_keys, ch3.light_sweep, ch3.playback):
    step(); print(step.__name__, snap(), flush=True)
ch2.save_as(OUT)
time.sleep(2.0)
print(snap())
print(call("record_stop", {"make_gif": False}))
