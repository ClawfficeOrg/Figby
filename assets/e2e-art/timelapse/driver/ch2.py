import sys
from ui import *

def ship():
    new_layer("Ship")
    tool("brush"); set_char("█"); set_size(1)
    pick("neutral", 1)
    for y, (a, b) in {21:(6,10), 22:(3,14), 23:(2,17), 24:(3,14), 25:(6,10)}.items():
        stroke(a, y, b, y)
    pick("red", 1); stroke(0, 22, 1, 22); stroke(0, 24, 1, 24); stroke(0, 23, 2, 23)       # engine bell
    set_char("o"); pick("cyan", 1)
    for x in (6, 8, 10): dab(x, 23)                                                         # windows
    set_char("▶"); pick("yellow", 1); dab(17, 23)                                           # nose light

def save_as(path):
    alt("f"); [key("Down") for _ in range(4)]; key("Enter"); wait("Save Figmap As")
    bs(60); text(path); time.sleep(.6); key("Enter"); time.sleep(.8)
    if "Overwrite?" in screen(): key("Enter"); time.sleep(.8)     # confirm replacing our own checkpoint

def menu(item):                      # Animation menu: 0 Add Frame, 1 Delete, 2 Play/Pause, 3 Toggle Timeline
    alt("a"); [key("Down") for _ in range(item)]; key("Enter"); time.sleep(.5)

def kf_editor(layer, x, frame_hdr):
    text("K"); time.sleep(.3)
    [key("Up") for _ in range(30)]; [key("Down") for _ in range(layer)]
    assert f"Frame: {frame_hdr}" in screen(), screen()
    key("Enter"); bs(4); text(str(x)); key("Enter"); time.sleep(.5)
    key("Escape"); time.sleep(.3)

def keyframes():
    menu(3); time.sleep(.5)                      # show timeline
    menu(0); menu(0)                             # capture frame 0 and frame 1
    key("Left"); time.sleep(.3)                  # frame 0: ship at left edge
    kf_editor(8, 0, 0)
    key("Right"); time.sleep(.3)                 # frame 1: ship at right side
    kf_editor(8, 70, 1)
    key("Left"); time.sleep(.3)

def tween():
    text("T"); time.sleep(.4)                    # Shift+T tween panel
    key("Down"); key("Down")
    for _ in range(10): key("Right")             # 15 frames
    key("Down"); key("Right"); key("Right")      # Ease Out
    key("Enter"); time.sleep(.8); assert "Generated" in screen(), screen()
    time.sleep(1.0); key("Enter"); time.sleep(.8)

def lighting():
    side_tab("layers")
    for k in range(1, 9): click(118, 9 + 2*k); key("L")      # only the Ship layer accepts lighting
    click(118, 9)
    text("G"); time.sleep(.6); key("P"); time.sleep(.4)
    for _ in range(45): key("Left")
    for _ in range(7): text("\x1b[1;2B"); time.sleep(.05)   # Shift+Down: v-move
    for _ in range(3): text("+")
    time.sleep(1.5); key("Escape"); time.sleep(.6)

def playback():
    key("Home"); key("F8"); time.sleep(.6)
    text("l")                                    # loop
    time.sleep(7.0)
    key("Escape"); time.sleep(.8)

if __name__ == "__main__":
    for st in sys.argv[1:]: globals()[st]()
    print(snap())
