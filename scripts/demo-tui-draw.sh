#!/bin/bash
# TUI draw-along: a scripted "hand" that draws in the real TUI over tmux,
# captured with tmux capture-pane frames into an asciinema .cast + .gif.
# No mouse, no paste — only keys a human could press: Tab/Esc, arrows,
# Space, tool letters, menu via Alt+F. (Ctrl+<letter> arrives as the plain
# letter under tmux+crossterm, so Ctrl+N/Ctrl+H paths use menu/alt flows.
# Backspace-as-Ctrl+H DOES arrive as a key and works for path clearing.)
#
# What it draws (deliberately charming, not precise):
#   1. new image via File menu (Ctrl+N equivalent, tmux-safe)
#   2. brush dot row (size 1, then size 3 block)
#   3. eraser carve-out of the block center
#   4. fill flood of the background corner
#   5. undo/redo (Ctrl+Z / Ctrl+Y)
#   6. export to /tmp/tui-draw.png (PNG, written by the app itself)
# Usage: ./scripts/demo-tui-draw.sh
# Output: /tmp/tui-draw/frames, plus assets/e2e-art/recordings/tui-draw.cast
#   + tui-draw.gif (assembled by scripts/make-tui-draw-cast.py)
set -u
S="tuidraw-$$"
TMUX_SOCK="${TMUX_SOCK:-figbyrev}"
BIN="figby-rs/target/debug/figby"
FR=/tmp/tui-draw/frames
rm -rf /tmp/tui-draw && mkdir -p "$FR"
rm -f /tmp/tui-draw.png /tmp/tui-draw.figmap

tmux -L "$TMUX_SOCK" kill-session -t "$S" 2>/dev/null || true
tmux -L "$TMUX_SOCK" new-session -d -s "$S" -x 120 -y 40 -c "$PWD"
T() { tmux -L "$TMUX_SOCK" send-keys -t "$S" "$@"; }
snap() { # $1=name — capture current pane to a frame file
    sleep 0.4
    tmux -L "$TMUX_SOCK" capture-pane -t "$S" -p -e > "$FR/$1.txt" 2>/dev/null
}

T "./$BIN --tui" Enter
sleep 3
T Escape; sleep 1          # dismiss welcome
# New Image via File menu (Ctrl+N arrives as plain n under tmux —
# documented crossterm/tmux limitation, not app behavior)
T M-f; sleep 0.8           # open File menu
T Enter; sleep 1           # New Image is the first item
T Enter; sleep 1           # accept 80x24 defaults
# To AsciiPreview: Tab x2 (FontEditor -> ImageEditor -> AsciiPreview)
T Tab; sleep 0.5
T Tab; sleep 0.5
snap 01-ascii-preview

# Walk to mid-canvas and paint 3 dots with the brush (default tool)
for i in $(seq 1 12); do T Right; sleep 0.05; done
for i in $(seq 1 5); do T Down; sleep 0.05; done
T " "; sleep 0.2
T Right; sleep 0.15
T " "; sleep 0.2
T Right; sleep 0.15
T " "; sleep 0.2
snap 02-three-dots

# Size up (]) and stamp a 3x3 block below
T Down; sleep 0.15
T Down; sleep 0.15
T "]"; sleep 0.2
T " "; sleep 0.3
snap 03-block

# Eraser carve: select eraser, size down, erase center of block
T e; sleep 0.3
T "["; sleep 0.2
T " "; sleep 0.3
snap 04-erased-center

# Back to brush, size 1: dot the corners for flair
T b; sleep 0.3
T "["; sleep 0.2
T "["; sleep 0.2
for i in $(seq 1 6); do T Left; sleep 0.03; done
for i in $(seq 1 3); do T Up; sleep 0.03; done
T " "; sleep 0.2
snap 05-corner-dot

# Undo then redo (Ctrl+Z / Ctrl+Y arrive as keys — verified live)
T C-z; sleep 0.4
snap 06-undo
T C-y; sleep 0.4
snap 07-redo

# Export PNG: Ctrl+E opens the dialog (verified live); clear the default
# path with Ctrl+H x12 (Backspace key), type path literally, Enter.
T C-e; sleep 0.8
for i in $(seq 1 12); do T C-h; sleep 0.03; done
T -l '/tmp/tui-draw.png'; sleep 0.4
snap 08-export-dialog
T Enter; sleep 1.5
snap 09-exported

# File menu peek (Save as Figmap lives here), then quit
T M-f; sleep 0.8
snap 10-file-menu
T Escape; sleep 0.4

T C-c; sleep 0.3
T q; sleep 0.5
tmux -L "$TMUX_SOCK" kill-session -t "$S" 2>/dev/null || true

echo "frames: $(ls "$FR" | wc -l | tr -d ' ')  png: $(ls -la /tmp/tui-draw.png 2>/dev/null | awk '{print $5}')"
ls "$FR"
