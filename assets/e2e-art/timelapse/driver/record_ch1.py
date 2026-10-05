"""Chapter 1 — build the space scene from scratch in the live TUI while asciinema records."""
import sys
from ui import *
import ch1
NAME = sys.argv[1] if len(sys.argv) > 1 else "timelapse-01-space-scene"
call("stop")
r = call("launch", {"cols": 140, "rows": 50, "record": NAME}); assert "error" not in r, r
time.sleep(1.5)
new_image(92, 34)
for step in (ch1.background, ch1.stars, ch1.nebula, ch1.planet, ch1.title, ch1.glow):
    step(); print(step.__name__, snap(), flush=True)
ch1.save("assets/e2e-art/timelapse", "space-scene-01-build-v3.figmap")
time.sleep(2.5)
print(snap())
print(call("record_stop", {"make_gif": False}))
