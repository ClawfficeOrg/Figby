#!/usr/bin/env python3
"""Tasteful acceleration for asciicast v3. Event deltas are divided by SPEED and idle gaps capped at
MAX_GAP; events between the first and last output containing REALTIME_MARKER (e.g. in-app playback)
keep their original timing so animation speed is honest.
Usage: speedup.py in.cast out.cast [speed=13] [max_gap=0.7] [realtime_marker]"""
import json, sys
src, dst = sys.argv[1], sys.argv[2]
speed = float(sys.argv[3]) if len(sys.argv) > 3 else 13.0
max_gap = float(sys.argv[4]) if len(sys.argv) > 4 else 0.7
marker = sys.argv[5] if len(sys.argv) > 5 else None
with open(src) as f:
    header = next(f); events = [json.loads(l) for l in f]
first = last = None
if marker:
    for i, e in enumerate(events):
        if e[1] == "o" and marker in e[2]:
            first = i if first is None else first; last = i
with open(dst, "w") as out:
    out.write(header)
    for i, e in enumerate(events):
        if first is not None and first < i <= last:
            pass                                   # real time
        else:
            e[0] = min(max_gap, e[0] / speed)
        out.write(json.dumps(e, ensure_ascii=False) + "\n")
