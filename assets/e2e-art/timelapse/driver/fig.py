#!/usr/bin/env python3
"""Client: fig.py <tool> '<json args>'   — prints screen text (and png path)."""
import json, os, socket, sys
SOCK = os.environ.get("FIGBY_DRIVER_SOCK", "/tmp/figby-driver.sock")
def call(name, args=None):
    s = socket.socket(socket.AF_UNIX); s.connect(SOCK)
    s.sendall((json.dumps({"name": name, "args": args or {}}) + "\n").encode())
    d = b""
    while not d.endswith(b"\n"): d += s.recv(65536)
    return json.loads(d)
if __name__ == "__main__":
    r = call(sys.argv[1], json.loads(sys.argv[2]) if len(sys.argv) > 2 else {})
    scr = r.pop("screen", None)
    if scr: print(scr)
    print({k: v for k, v in r.items()})
