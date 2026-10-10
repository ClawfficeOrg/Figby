"""Shared helpers for the bob-title takes (ch4_bob_shade.py / ch4_bob_litfx.py).

Everything here was re-derived against the live 6.0.55 TUI at 140x50, because the
plain-text screen capture from the driver is NOT column-accurate in the top-left
region (the toolbox/palette rows are off by one) — only the right-hand side panel
lines up. So:

  * `calibrate()` finds the canvas origin from the status bar's "X:n Y:n" readout
    (two probes, asserted to agree) instead of trusting a `╔` column index.
  * Panel coordinates are PNG-derived for 140x50 and asserted by content, not by
    index: toolbox rows 5..20, palette swatch rows 25..37, side-panel tab icons on
    row 5 (props/text 121, layers 113), Text-tool rows 7..12 (Font 10),
    Brush props rows 7..12 (Size 8, Shape 9, Mode 10, Density 11, Char 12).
"""
import re
import time

import ui
from fig import call
from ui import cell, click, drag, key, line_pts, mouse, screen, snap, stroke, text, time  # noqa: F401

# ---------------------------------------------------------------- geometry ----
# Verified at 140x50 for a 76x26 canvas: mouse (30,20) -> cell (4,7) and
# (60,30) -> cell (34,17).  The screen text capture cannot be indexed for these.
CX0, CY0 = 26, 13

PROPS_X = 121          # "props"/"text" side-panel tab icon (row 5)
LAYERS_X = 113         # "layers" tab icon
LIBS_X = 125           # effects/library tab icon (4th tab)

TEXT_ROW = {"text": 8, "mode": 9, "font": 10, "just": 11, "scale": 12,
            "rasterize": 14}
BRUSH_ROW = {"size": 8, "shape": 9, "mode": 10, "density": 11, "char": 12}
LAYER_ROW0 = 9         # first layer row in the Layers tab


def calib(probe=((60, 30), (40, 20))):
    """Derive (CX0, CY0) from the status bar; both probes must agree, and the
    result is re-checked by hovering the origin (must read 0,0) and origin+3 (3,3).
    The readout only refreshes on a canvas move, so a stale value would otherwise
    sail through the agreement check."""
    global CX0, CY0
    got = set()
    for px, py in probe:
        mouse(px + 1, py, "move")
        time.sleep(0.15)
        mouse(px, py, "move")
        time.sleep(0.25)
        bx, by = cursor()
        got.add((px - bx, py - by))
    assert len(got) == 1, f"canvas origin probes disagree: {got}"
    ui.CX0, ui.CY0 = got.pop()
    CX0, CY0 = ui.CX0, ui.CY0
    for want, at in (((0, 0), (CX0, CY0)), ((3, 3), (CX0 + 3, CY0 + 3))):
        mouse(at[0] + 1, at[1], "move")
        time.sleep(0.15)
        mouse(*at, "move")
        time.sleep(0.25)
        assert cursor() == want, f"origin check failed: hover {at} read {cursor()} not {want}"
    return CX0, CY0


def cursor():
    m = re.search(r"X:(\d+) Y:(\d+)", screen())
    assert m, "no cursor readout in status bar"
    return int(m.group(1)), int(m.group(2))


def cal(sx, sy):
    return ui.CX0 + sx, ui.CY0 + sy


def sweep(x0, y0, x1, y1, hold=0.03):
    """One drag stroke: down at A, move to B (Bresenham fills the gap), up at B.

    A single move event is enough for the app to paint the whole segment, so a
    whole 48-cell row costs three driver calls.
    """
    a, b = cal(x0, y0), cal(x1, y1)
    mouse(*a, "move")
    time.sleep(hold)
    mouse(*a, "down", "left")
    time.sleep(hold)
    mouse(*b, "move", "left")
    time.sleep(hold)
    mouse(*b, "up", "left")
    time.sleep(hold)


def runs_by_row(cells):
    """[(row, [(c0, c1), ...])] — group a cell set into contiguous horizontal runs."""
    byrow = {}
    for j, i in cells:
        byrow.setdefault(j, []).append(i)
    out = []
    for j in sorted(byrow):
        cols = sorted(byrow[j])
        runs, start, prev = [], cols[0], cols[0]
        for c in cols[1:]:
            if c == prev + 1:
                prev = c
                continue
            runs.append((start, prev))
            start = prev = c
        runs.append((start, prev))
        out.append((j, runs))
    return out


def font_name():
    for line in screen().split("\n"):
        if "Font:" in line:
            return line.split("[<]")[1].split("[>]")[0].strip()
    raise AssertionError("Text Tool panel not open")


def font_next_col():
    """Column of the Text panel's `[>]` button (it shifts with the name length)."""
    return 113 + 11 + len(font_name()) + 1


def font_prev_col():
    return 113 + 6 + 1


def pick_font(name):
    """Cycle the Text-tool font until `name` shows (wraps both ways)."""
    for _ in range(80):
        cur = font_name()
        if cur == name:
            return
        c = font_next_col() if cur < name else font_prev_col()
        click(c, TEXT_ROW["font"], 0.25)
    raise AssertionError(f"font {name} not reachable (at {font_name()})")


def prop_col(row, label):
    """Centre column of a side-panel button, read from the live line content."""
    for line in screen().split("\n"):
        if label in line:
            i = line.index(label)
            return 113 + i + (len(label) - 1) // 2
    raise AssertionError(f"props row {label} not found")


def has(text_):
    return text_ in screen()


def wait(t, to=6):
    return call("wait_for_text", {"text": t, "timeout_seconds": to}).get("found")


def alt(c):
    text("\x1b" + c)
    time.sleep(0.25)


def tab(which):
    click({"layers": LAYERS_X, "props": PROPS_X, "text": PROPS_X, "libs": LIBS_X}[which], 5)
    time.sleep(0.25)


def brush_mode(mode):
    tab("props")
    for _ in range(3):
        if f"Mode: {mode}" in screen():
            return
        key("M")
        time.sleep(0.3)
    raise AssertionError(f"brush Mode: {mode} not reachable")


def brush_shape(name):
    tab("props")
    for _ in range(6):
        if f"Shape: {name}" in screen():
            return
        key("\\")
        time.sleep(0.25)
    raise AssertionError(f"brush Shape: {name} not reachable")


def brush_size(n):
    tab("props")
    for _ in range(24):
        cur = int(screen().split("\n")[8].split("Size:")[1].split("[")[0])
        if cur == n:
            return
        key("]" if n > cur else "[")
        time.sleep(0.08)
    raise AssertionError(f"brush size {n} not reachable")


def swatch_name(col, row):
    """Hover a palette swatch and read back its tooltip name.

    The tooltip is the palette-panel line immediately above the "Cst:" readout;
    column indices in the captured text are unreliable, so locate it by content.
    """
    mouse(col + 1, row, "move")
    time.sleep(0.15)
    mouse(col, row, "move")
    time.sleep(0.3)
    lines = screen().split("\n")
    for i, l in enumerate(lines):
        if "Cst:" in l and i:
            tip = lines[i - 1].lstrip("│ ")
            for stop in ("║", "│"):
                tip = tip.split(stop)[0]
            return tip.strip()
    raise AssertionError(f"no swatch tooltip at ({col},{row})")


def ramp(colours):
    """Load a Marker ramp: Normal mode first (clears leftover accumulation),
    multi-select the swatches in the given order, then Marker mode on.

    `colours` is ((group, swatch index, expected tooltip name), ...).  Each swatch
    is hover-verified before it is clicked — GROUP_ROW already points at the swatch
    row, not the group label, and getting that off by one silently drops entries.
    """
    tab("props")
    if "Mode: Marker" in screen():
        key("M")
        time.sleep(0.3)
    spots = []
    for group, idx, name in colours:
        # Verify unarmed (the tooltip only shows while multi-select is off) …
        col, row = 2 + 2 * idx, ui.GROUP_ROW[group]
        got = swatch_name(col, row)
        assert got == name, f"swatch ({group},{idx}) is {got!r}, expected {name!r}"
        # … but click armed: the " Sel:" row the multi-select inserts pushes every
        # swatch down by one (palette.rs handle_click row_offset).
        spots.append((col, row + 1))
    key("j")                                         # palette multi-select
    time.sleep(0.4)
    assert " Sel:" in screen(), "palette multi-select not armed"
    for col, row in spots:
        click(col, row, 0.25)
    key("j")                                         # leave the mode, keep the order
    time.sleep(0.3)
    key("M")                                         # Marker: colour-stepping shading
    time.sleep(0.4)
    assert "Mode: Marker" in screen(), "marker mode not active"


def recent_ramp():
    """Verify the ramp landed: the palette keeps a Recent strip in click order."""
    return screen()


def light_colour(preset):
    """`C` cycles white -> warm -> amber -> red -> magenta -> blue -> cyan -> green."""
    order = ("white", "warm", "amber", "red", "magenta", "blue", "cyan", "green")
    for _ in range(order.index(preset)):
        key("C")
        time.sleep(0.25)


def intensity(delta):
    """+/- 0.1 on the selected light. The side panel must be on Props, not Layers."""
    tab("props")
    for _ in range(abs(delta)):
        text("+" if delta > 0 else "-")
        time.sleep(0.18)