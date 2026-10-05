"""Chapter 4 — FIGBY title: text, Marker-brush shading, static text exports, light sweep animation + GIF export.
Usage: record_ch4.py <cast-name>   |   record_ch4.py --dry   (no recording)"""
import sys
from ui import *
import ch4, ch2
DRY = len(sys.argv) > 1 and sys.argv[1] == "--dry"
NAME = None if DRY else (sys.argv[1] if len(sys.argv) > 1 else "timelapse-04-title-shading-lighting")
call("stop")
args = {"cols": 140, "rows": 50}
if NAME: args["record"] = NAME
r = call("launch", args); assert "error" not in r, r
time.sleep(1.5)
for step in (ch4.title, ch4.shade, ch4.export_static, ch4.lighting, ch4.animate, ch4.export_animation):
    step(); print(step.__name__, snap(), flush=True)
ch2.save_as("assets/e2e-art/timelapse/title/figby-title-shading-lighting.figmap") if False else None
time.sleep(2.0)
print(snap())
if NAME: print(call("record_stop", {"make_gif": False}))
