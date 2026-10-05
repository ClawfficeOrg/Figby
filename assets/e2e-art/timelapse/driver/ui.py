"""Figby TUI UI primitives (140x50 PTY). Coordinates verified against snapshots."""
import random, time
from lib import *

CX0, CY0 = 18, 9           # screen position of canvas cell (0,0) at 1x for a 92x34 image
TOOLS = {"brush":5,"move":6,"rotate":7,"select":8,"lasso":9,"circle":10,"polygon":11,"fill":12,
         "line":13,"eraser":14,"eyedrop":15,"spray":16,"text":17,"emitter":18,"lighting":19,"braille":20}
GROUP_ROW = {"neutral":25,"red":27,"yellow":29,"green":31,"cyan":33,"blue":35,"purple":37}
def cell(bx, by): return CX0 + bx, CY0 + by
def tool(name): click(5, TOOLS[name]); time.sleep(0.1)
def pick(group, idx):          # standard palette; idx = swatch index 0..3 (2 cols each)
    click(2 + 2*idx, GROUP_ROW[group]); time.sleep(0.05)
def pick_navy():
    key("z"); click(4, 25); key("z")      # extended page 1 swatch 1 = (0,0,95)
def target(which): click(2 if which == "fg" else 8, 23)
def side_tab(n): click({"layers":113,"props":121,"text":121,"libs":129}.get(n, n), 5); time.sleep(0.15)
def set_char(ch):
    side_tab("props")
    l = screen().split("\n")[12]
    assert "Char:" in l, l
    click(l.index("Char:") + 7, 12); time.sleep(0.1); text(ch); key("Enter"); time.sleep(0.15)
    got = screen().split("\n")[12]
    assert f"'{ch}'" in got, (ch, got)
    assert "1x" in screen().split("\n")[-1], "zoom changed"
def set_size(n):
    side_tab("props")
    bs_cur = int(screen().split("\n")[8].split("Size:")[1].split("[")[0])
    for _ in range(abs(n - bs_cur)): key("]" if n > bs_cur else "[")
def new_layer(name):
    alt("l"); key("Enter"); time.sleep(0.3)
    side_tab("layers"); key("F2"); time.sleep(0.1); bs(24); text(name); key("Enter"); time.sleep(0.2)
    assert name in screen(), name
def stroke(x0, y0, x1, y1): drag(line_pts(*cell(x0, y0), *cell(x1, y1)))
def dab(bx, by): x, y = cell(bx, by); click(x, y)

def emitter_field(idx, val, label):
    """Edit one Emitter Config field by absolute index; verifies the new value on screen."""
    for _ in range(20): key("Up")
    for _ in range(idx): key("Down")
    key("Enter"); bs(8); text(str(val)); key("Enter"); time.sleep(0.1)
    scr = screen()
    assert f"{label}: {val}" in scr, (label, val, [l[96:132] for l in scr.split("\n")[2:24]])
