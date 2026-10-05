"""Chapter 4 — build a FIGBY title, shade it with the Marker brush, export it as text,
then light it and export a light-sweep animation."""
import sys
import ui
from ui import *
import ch2, ch3

OUT = "assets/e2e-art/timelapse/title"
RAMP = (("blue", 0), ("blue", 1), ("purple", 0), ("purple", 1), ("red", 0), ("red", 1),
        ("yellow", 0), ("yellow", 1), ("neutral", 2), ("neutral", 3))   # marker steps along this order (dark to bright)

def calibrate():
    for i, l in enumerate(screen().split("\n")):
        if "╔" in l:
            ui.CX0, ui.CY0 = l.index("╔") + 1, i + 1
            return
    raise AssertionError("canvas frame not found")

def title():
    new_image(64, 24); time.sleep(.5); calibrate()
    tool("text"); side_tab("props"); text("FIGBY")
    row = [i for i, l in enumerate(screen().split("\n")) if "Font:" in l][0]
    for _ in range(60):                                   # pick the bob FIGfont
        l = screen().split("\n")[row]
        if "bob" in l: break
        click(l.index("[>]") + 1, row)
    assert "bob" in screen().split("\n")[row]
    pick(*RAMP[0])                                        # ramp start; Marker only steps painted cells
    mouse(*cell(4, 3), "move"); time.sleep(.6)
    click(*cell(4, 3)); time.sleep(.8)
    key("C-r"); time.sleep(.6)                            # rasterize the text block
    key("Escape"); time.sleep(.4)

def shade():
    tool("brush"); side_tab("props")
    key("j"); time.sleep(.4)                              # palette multi-select: click the ramp in order
    for grp, idx in RAMP:
        click(2 + 2*idx, GROUP_ROW[grp] + 1); time.sleep(.3)
    key("j"); time.sleep(.3)
    key("M"); time.sleep(.4)                              # Marker (colour-stepping shading) brush
    assert "Marker" in screen(), "marker mode not active"
    key("\\"); time.sleep(.3)                             # Circle shape: soft falloff at the edges
    paint_title()

def wave(x0, x1, y, amp=1.5, period=14.0, step=1):
    """Hand-drawn looking horizontal stroke: a gentle sine wobble around row y."""
    import math
    xs = range(x0, x1 + 1, step) if x1 >= x0 else range(x0, x1 - 1, -step)
    return [cell(x, int(round(y + amp * math.sin((x - x0) / period * 2 * math.pi)))) for x in xs]

LETTERS = ((4, 13), (15, 17), (19, 28), (30, 39), (41, 50))     # F I G B Y (cell columns)

def paint_title():
    """Airbrush the lettering by hand: every pass lifts the rows it covers up the ramp, and each
    successive pass stops higher, so the top ends up brightest and the bottom stays dark blue.
    Then per-letter highlights go on with a soft round brush."""
    top, bottom = 6, 18
    levels = lambda y: int(3.0 * (bottom - y) / (bottom - top) + 0.5)    # strokes each row needs
    set_size(3)
    for p in range(1, 4):
        rows = [y for y in range(top, bottom + 1, 2) if levels(y) >= p]
        for k, y in enumerate(rows):
            pts = wave(3, 51, y, 0.0, step=1)
            drag(pts if (k + p) % 2 == 0 else pts[::-1])
        time.sleep(.5)
    # per-letter highlights: a soft bright core on each letter's upper-left
    for x0, x1 in LETTERS:
        set_size(7)
        dab(x0 + 2, 8); dab(x0 + 2, 8)
        time.sleep(.25)
    time.sleep(1.5)

def open_export():
    for _ in range(4):
        key("C-e"); time.sleep(.8)
        if "T:format" in screen(): return
        key("Escape"); time.sleep(.4)
    raise AssertionError("export dialog did not open")

def pick_format(name):
    for _ in range(6):
        if f"Format: [{name}]" in screen(): return
        key("t"); time.sleep(.25)
    raise AssertionError(("export format", name))

def export(fmt, path):
    open_export()
    pick_format(fmt)
    text(path); time.sleep(.5)
    key("Enter"); time.sleep(1.5)

def export_static():
    export("PNG", OUT + "/figby-title.png")
    export("TXT", OUT + "/figby-title.txt")
    export("ANSI", OUT + "/figby-title.ansi")

def light_pos():
    for y, l in enumerate(screen().split("\n")):
        if "✦" in l:
            return l.index("✦") - ui.CX0, y - ui.CY0
    return None

def move_light(tx, ty):
    for _ in range(80):
        p = light_pos()
        if p is None: time.sleep(.2); continue
        x, y = p
        if x == tx and y == ty: return
        if x != tx: key("Right" if x < tx else "Left")
        elif y != ty: text("\x1b[1;2B" if y < ty else "\x1b[1;2A")
    raise AssertionError(("light never reached", tx, ty, light_pos()))

def lighting():
    for grp, idx in RAMP: pick(grp, idx); time.sleep(.2)    # push the title's ramp colours into Recent
    alt("v"); [key("Down") for _ in range(6)]; key("Enter"); time.sleep(.8)   # View > Palette Editor: Recent -> lighting swatches
    key("Escape"); time.sleep(.5)
    text("G"); time.sleep(.8); key("P"); time.sleep(.6)
    move_light(4, 10); time.sleep(1.2)
    move_light(30, 10); time.sleep(1.5)
    key("Escape"); time.sleep(.8)

def animate():
    ch2.menu(3); time.sleep(.5)                           # show the timeline
    ch2.menu(0); ch2.menu(0)                              # frames 0 and 1
    key("Left"); time.sleep(.3)
    ch3.kf_set(0, 0, {2: 120})                            # title starts dim
    key("Right"); time.sleep(.3)
    ch3.kf_set(0, 1, {2: 255})                            # fades up to full
    key("Left"); time.sleep(.3)
    ch2.tween()
    ch3.seek(0)
    text("G"); time.sleep(.6); key("Up"); key("Down"); time.sleep(.2)
    move_light(2, 10)
    text("K"); time.sleep(.5); key("Escape"); time.sleep(.5)       # light keyframe, frame 0
    ch3.seek(16)
    text("G"); time.sleep(.6); key("Up"); key("Down"); time.sleep(.2)
    move_light(60, 10)
    text("K"); time.sleep(.5); key("Escape"); time.sleep(.5)       # light keyframe, frame 16
    ch3.seek(0)

def playback():
    ch2.playback()

def export_animation():
    open_export()
    pick_format("GIF")
    time.sleep(2.0)
    text(OUT + "/figby-title-light.gif"); time.sleep(.6)
    key("Enter"); time.sleep(12.0)                        # GIF encode runs async; let it finish

if __name__ == "__main__":
    for st in sys.argv[1:]: globals()[st]()
    print(snap())
