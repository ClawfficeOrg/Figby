import time, json, sys
from fig import call
def key(k): call("keypress", {"key": k}); time.sleep(0.05)
def text(t): call("type_text", {"text": t})
def alt(c): text("\x1b" + c); time.sleep(0.25)
def bs(n=1):
    for _ in range(n): text("\x7f")
def mouse(x, y, a, b=None):
    args = {"x": x, "y": y, "action": a}
    if b: args["button"] = b
    call("mouse", args)
def click(x, y, b="left"):
    mouse(x, y, "move"); mouse(x, y, "down", b); time.sleep(0.05); mouse(x, y, "up", b); time.sleep(0.15)
def drag(pts, b="left"):
    x, y = pts[0]; mouse(x, y, "move"); mouse(x, y, "down", b); time.sleep(0.04)
    for x, y in pts[1:]: mouse(x, y, "move", b); time.sleep(0.03)
    mouse(x, y, "up", b); time.sleep(0.15)
def line_pts(x0, y0, x1, y1):
    n = max(abs(x1-x0), abs(y1-y0), 1)
    return [(round(x0+(x1-x0)*i/n), round(y0+(y1-y0)*i/n)) for i in range(n+1)]
def screen():
    return call("snapshot").get("screen", "")
def snap():
    r = call("snapshot"); return r.get("png")
def row(n): return screen().split("\n")[n]
def wait(t, to=5): return call("wait_for_text", {"text": t, "timeout_seconds": to}).get("found")
def new_image(w, h):
    alt("f"); key("Enter"); wait("New Image")
    bs(6); text(str(w)); key("Down"); bs(6); text(str(h)); key("Enter"); time.sleep(0.5)
