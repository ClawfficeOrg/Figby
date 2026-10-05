import sys
from ui import *
import ch2

def kf_set(layer, frame, props):
    """Keyframe editor: props = {0: pos_x, 1: pos_y, 2: opacity}."""
    text("K"); time.sleep(.3)
    [key("Up") for _ in range(30)]; [key("Down") for _ in range(layer)]; [key("Left") for _ in range(3)]
    assert f"Frame: {frame}" in screen(), (frame, screen())
    cur = 0
    for p, v in sorted(props.items()):
        while cur < p: key("Right"); cur += 1
        key("Enter"); bs(5); text(str(v)); key("Enter"); time.sleep(.3)
    key("Escape"); time.sleep(.3)

def seek(frame):
    [key("Left") for _ in range(20)]
    [key("Right") for _ in range(frame)]
    time.sleep(.3)

def emitter():
    side_tab("layers"); click(118, 9)                       # Ship layer: the emitter rides on it
    tool("emitter"); side_tab("props"); click(*cell(2, 23)); time.sleep(.6)   # engine bell
    for idx, val, label in ((0,60,"Spawn Rate"),(2,0.9,"Lifetime Max"),(1,0.4,"Lifetime Min"),(3,-25,"Vel X Min"),
                            (4,-12,"Vel X Max"),(5,-4,"Vel Y Min"),(6,4,"Vel Y Max"),(12,"*","Character"),
                            (13,255,"Color R"),(14,140,"Color G"),(15,30,"Color B")):
        emitter_field(idx, val, label)
    key("Escape"); time.sleep(1.5)

def satellite():
    new_layer("Satellite")
    click(118, 9); key("L")                          # keep the satellite out of the lighting pass (glyph remap)
    tool("brush"); set_char("█"); set_size(1)
    pick("neutral", 1); stroke(62, 6, 64, 6); stroke(61, 7, 65, 7); stroke(62, 8, 64, 8)       # body
    set_char("▒"); pick("cyan", 1); stroke(54, 7, 60, 7); stroke(66, 7, 72, 7)                  # solar wings
    set_char("^"); pick("yellow", 1); dab(63, 5)                                                # dish
    # select it, rotate it a quarter turn, nudge it with the Move tool
    tool("select"); drag(line_pts(*cell(52, 3), *cell(74, 10)))
    tool("rotate"); key("Right"); time.sleep(.6)
    tool("move"); [key("Left") for _ in range(4)]; time.sleep(.4)
    tool("brush"); key("Escape")

def motion_keys():
    seek(0)
    kf_set(9, 0, {0: 0, 1: 0})                      # satellite starts in place
    kf_set(6, 0, {2: 150})                          # title starts dim
    seek(16)
    kf_set(9, 16, {0: -45, 1: 9})                   # satellite drifts across the sky
    kf_set(6, 16, {2: 255})                         # title fades up to full
    seek(0)

def light_sweep():
    seek(0)
    text("G"); time.sleep(.6)
    if "Pnt" not in screen(): key("P"); time.sleep(.4)
    key("Up"); key("Down"); time.sleep(.2)          # select the point light
    for _ in range(40): key("Left")
    for _ in range(4): text("\x1b[1;2B"); time.sleep(.05)
    text("K"); time.sleep(.5); key("Escape"); time.sleep(.5)     # light keyframe, frame 0
    seek(16)
    text("G"); time.sleep(.6); key("Up"); key("Down")
    for _ in range(80): key("Right")
    for _ in range(3): text("+")
    text("K"); time.sleep(.5); key("Escape"); time.sleep(.5)     # light keyframe, frame 16
    seek(0)

def playback():
    ch2.playback()

if __name__ == "__main__":
    for st in sys.argv[1:]: globals()[st]()
    print(snap())
