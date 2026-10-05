import math, sys
from ui import *

def background():
    key("F2"); bs(24); text("Background"); key("Enter")
    tool("brush"); set_char("░"); pick_navy(); tool("fill"); click(*cell(40,15)); time.sleep(.4)

def stars():
    new_layer("Stars")
    tool("brush"); set_size(1)
    rnd = random.Random(11)
    set_char("."); pick("blue", 0)                       # dim distant stars
    for _ in range(60): dab(rnd.randrange(0,92), rnd.randrange(0,34))
    set_char("*"); pick("neutral", 3)                    # sparse bright stars
    for _ in range(22): dab(rnd.randrange(0,92), rnd.randrange(0,34))
    for grp, (cx, cy) in (("yellow",(10,29)), ("cyan",(84,6)), ("purple",(46,31))):   # coloured clusters
        pick(grp, 1)
        for _ in range(9):
            dab(max(0,min(91,cx+rnd.randrange(-6,7))), max(0,min(33,cy+rnd.randrange(-3,4))))

def nebula():
    new_layer("Nebula")
    rnd = random.Random(5)
    for grp, idx, (cx, cy), n in (("purple",0,(70,9),16), ("purple",1,(76,12),12), ("cyan",0,(64,13),10)):
        tool("braille"); pick(grp, idx)
        for _ in range(n):
            bx = int(rnd.gauss(cx, 6)); by = int(rnd.gauss(cy, 2.2))
            if not (0 <= bx < 92 and 0 <= by < 34): continue
            tool("select"); click(*cell(bx, by)); tool("braille")     # select-click parks the cell cursor
            side_tab("props")
            for k in rnd.sample(["Up","Down","Left","Right"], 3): key(k); key("Space")

def disk(cx, cy, rx, ry, row_color):
    for dy in range(-ry, ry+1):
        half = int(rx * math.sqrt(max(0.0, 1 - (dy/ry)**2)))
        y = cy + dy
        if 0 <= y < 34 and half > 0:
            row_color(dy)
            stroke(max(0, cx-half), y, min(91, cx+half), y)

def planet():
    new_layer("Planet")
    tool("brush"); set_char("█"); set_size(1)
    cx, cy, rx, ry = 70, 26, 15, 7
    def shade(dy):
        pick("blue", 0 if dy > 2 else 1)
    disk(cx, cy, rx, ry, shade)
    pick("cyan", 1); set_char("▒")                              # atmosphere band + highlight
    stroke(cx-12, cy-2, cx+6, cy-2); stroke(cx-9, cy-4, cx+2, cy-4)
    pick("neutral", 3); set_char("░"); stroke(cx-8, cy-5, cx-2, cy-5)
    ring()

def ring(layer=True):
    cx, cy = 70, 26
    if layer: new_layer("Ring")
    tool("brush"); set_char("█"); pick("yellow", 0)
    runs, cur = [], []
    for t in range(0, 361, 3):
        x = cx + int(round(22*math.cos(math.radians(t)))); y = cy - 1 + int(round(3.2*math.sin(math.radians(t))))
        front = math.sin(math.radians(t)) >= 0
        dy = y - cy                                      # back arc hides behind the planet disk
        hidden = (not front) and abs(dy) <= 7 and abs(x - cx) <= 15*math.sqrt(max(0.0, 1 - (dy/7)**2))
        if hidden or not (0 <= x < 92 and 0 <= y < 34):
            if cur: runs.append(cur); cur = []
        elif not cur or cur[-1] != cell(x, y): cur.append(cell(x, y))
    if cur: runs.append(cur)
    for r in runs:
        if len(r) > 1: drag(r)
        else: click(*r[0])
    moon()

def moon():
    new_layer("Moon")
    tool("brush"); set_char("█")
    disk(20, 28, 7, 3, lambda dy: pick("neutral", 1))
    set_char("o"); pick("neutral", 2)
    for bx, by in ((18,27),(23,29),(20,30)): dab(bx, by)

def title():
    new_layer("Title")
    tool("text"); side_tab("props"); text("FIGBY")
    for _ in range(40):                                   # pick the bob FIGfont (kerning layout)
        l = screen().split("\n")[10]
        if "bob" in l: break
        click(l.index("[>]")+1, 10)
    assert "bob" in screen().split("\n")[10]
    pick("yellow", 1)
    mouse(*cell(4,0), "move"); time.sleep(.6)
    click(*cell(4,0)); time.sleep(.8)
    key("C-r"); time.sleep(.6)                           # rasterize the text block onto the Title layer

def glow():
    new_layer("Glow")
    tool("brush"); set_char("*"); set_size(1)
    rnd = random.Random(3)
    for grp, idx in (("cyan", 1), ("neutral", 3)):
        pick(grp, idx)
        for _ in range(14):
            side = rnd.choice("tblr")
            if side == "t": bx, by = rnd.randrange(3, 58), rnd.choice((1, 2))
            elif side == "b": bx, by = rnd.randrange(3, 58), rnd.choice((20, 21))
            elif side == "l": bx, by = rnd.choice((2, 3)), rnd.randrange(3, 20)
            else: bx, by = rnd.choice((54, 55, 56)), rnd.randrange(3, 20)
            dab(bx, by)

def save(path_dir, name):
    side_tab("layers"); time.sleep(2.0)                     # show named layer stack
    alt("f"); [key("Down") for _ in range(4)]; key("Enter"); wait("Save Figmap As")
    bs(40); text(path_dir + "/" + name); time.sleep(.8); key("Enter")
    time.sleep(1.0)
    return name

if __name__ == "__main__":
    for st in sys.argv[1:]: globals()[st]()
    print(snap())
