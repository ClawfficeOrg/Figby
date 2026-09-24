#!/usr/bin/env python3
"""Assemble assets/e2e-art/recordings/tui-draw.cast from live tmux frames.

Runs the real TUI in a tmux PTY on the figbyrev socket, drives the same
hand-verified keystrokes as scripts/demo-tui-draw.sh, and records every
step as a cast event with human pacing. Frame content comes from
`tmux capture-pane -p` (plain text, no ANSI) so the cast renders
identically everywhere agg runs.

Then: agg assets/e2e-art/recordings/tui-draw.cast
          assets/e2e-art/recordings/tui-draw.gif

Requires: tmux, a built ./figby-rs/target/debug/figby.
"""
import json
import subprocess
import sys
import time

REPO = "/Users/hippo/git_repos/Figby"
SOCK = "figbyrev"
SESSION = "tuidraw-cast"
BIN = "./figby-rs/target/debug/figby"
OUT = "assets/e2e-art/recordings/tui-draw.cast"
W, H = 120, 40

events = []
clock = 0.0


def emit(text, pause=0.0):
    """Append one output event holding the full screen text."""
    global clock
    events.append([round(clock, 3), "o", text])
    clock += pause


def tmux(*args):
    return subprocess.run(
        ["tmux", "-L", SOCK, *args],
        cwd=REPO, capture_output=True, text=True, timeout=30,
    )


def keys(*args):
    tmux("send-keys", "-t", SESSION, *args)


def shot():
    r = tmux("capture-pane", "-t", SESSION, "-p")
    text = r.stdout
    if not text.endswith("\n"):
        text += "\n"
    return text


def step(pause=0.5):
    time.sleep(pause)
    emit("\x1b[2J\x1b[H" + shot())


def type_slow(text, per=0.05):
    for ch in text:
        keys("-l", ch)
        time.sleep(per)
    time.sleep(0.4)
    emit("\x1b[2J\x1b[H" + shot())


def main():
    tmux("kill-session", "-t", SESSION)
    tmux("new-session", "-d", "-s", SESSION, "-x", str(W), "-y", str(H),
         "-c", REPO)
    keys(BIN + " --tui", "Enter")
    time.sleep(3)
    keys("Escape")
    time.sleep(1)
    emit("\x1b[2J\x1b[H" + shot(), 1.0)

    # New image via File menu (tmux-safe Ctrl+N equivalent)
    keys("M-f")
    time.sleep(0.8)
    step(0.5)
    keys("Enter")
    time.sleep(1)
    step(0.5)
    keys("Enter")
    time.sleep(1)
    step(0.5)
    # To AsciiPreview
    keys("Tab")
    time.sleep(0.5)
    keys("Tab")
    time.sleep(0.5)
    emit("\x1b[2J\x1b[H" + shot(), 1.0)

    # cards
    emit("Figby TUI draw-along — painting by hand, keyboard only\r\n", 1.5)

    # Walk out, grow brush, three stamps (dirty dot • appears in tab bar)
    for _ in range(10):
        keys("Right")
        time.sleep(0.03)
    for _ in range(5):
        keys("Down")
        time.sleep(0.03)
    keys("]")
    time.sleep(0.3)
    keys("]")
    time.sleep(0.3)
    for _ in range(3):
        keys(" ")
        time.sleep(0.4)
        step(0.3)
        keys("Right")
        time.sleep(0.2)
    emit("Brush, size 3, three stamps — tab dot \u2022 means unsaved\r\n", 1.0)

    # Eraser carve
    keys("e")
    time.sleep(0.4)
    step(0.4)
    keys(" ")
    time.sleep(0.5)
    step(0.5)
    emit("Eraser carves the middle back out\r\n", 1.0)

    # Fill corner
    for _ in range(10):
        keys("Left")
        time.sleep(0.02)
    for _ in range(5):
        keys("Up")
        time.sleep(0.02)
    keys("g")
    time.sleep(0.4)
    step(0.4)
    keys(" ")
    time.sleep(0.6)
    step(0.6)
    emit("Fill floods the corner\r\n", 1.0)

    # Undo / redo
    keys("C-z")
    time.sleep(0.5)
    step(0.4)
    keys("C-y")
    time.sleep(0.5)
    step(0.4)
    emit("Undo, redo — history holds\r\n", 1.0)

    # Export PNG (select-all: first key replaces suggestion)
    keys("C-e")
    time.sleep(0.8)
    step(0.4)
    type_slow("/tmp/tui-draw.png")
    keys("Enter")
    time.sleep(1.5)
    step(0.5)
    emit("Export writes the PNG itself\r\n", 1.0)

    # File menu peek, quit
    keys("M-f")
    time.sleep(0.8)
    step(0.5)
    keys("Escape")
    time.sleep(0.4)
    emit("done — scripts/demo-tui-draw.sh drives this (tmux PTY)\r\n", 1.0)

    tmux("kill-session", "-t", SESSION)

    header = {"version": 3, "term": {"cols": W, "rows": H},
              "timestamp": int(time.time()),
              "command": "./scripts/demo-tui-draw.sh"}
    with open(f"{REPO}/{OUT}", "w") as f:
        f.write(json.dumps(header) + "\n")
        for ev in events:
            f.write(json.dumps(ev) + "\n")
    print(f"wrote {OUT}: {len(events)} events, {clock:.1f}s")


if __name__ == "__main__":
    sys.exit(main())
