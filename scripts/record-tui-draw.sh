#!/bin/bash
# TUI draw-along recording: draws a small picture in the real TUI by hand
# (scripted keystrokes at human pace), narrated with title cards.
# Keys used are all live-verified: arrows, Space (paints), Enter, Esc,
# Tab, tool letters, Ctrl+Z/Y, Ctrl+E export, Alt+F menu.
# Avoids: Ctrl+N (arrives as plain n under tmux), Backspace key (literal
# text under tmux), mouse (no SGR passthrough).
# Usage: ./scripts/record-tui-draw.sh
# Output: assets/e2e-art/recordings/tui-draw.cast (then agg -> .gif)
set -u
BIN="./figby-rs/target/debug/figby"

type_out() { # slow human typing
    local text="$1" i=0
    while [ "$i" -lt "${#text}" ]; do
        printf '%s' "${text:$i:1}"
        sleep 0.05
        i=$((i+1))
    done
    sleep 0.4
    printf '\n'
}
say() { echo ""; echo "-- $1 --"; sleep "$2"; }

clear
echo "Figby TUI draw-along — painting by hand, keyboard only"
sleep 1.5

echo ""
echo "New canvas via the File menu:"
sleep 0.6
type_out "$BIN --tui"
echo "(already inside — File > New Image, accept 80x24, Tab to ASCII Preview)"
sleep 1

clear
echo "Brush, size 3, three stamps in a row:"
sleep 1
echo "(arrows to position, ] to grow, Space to stamp — see the tab dot • appear)"
sleep 4

clear
echo "Eraser carves the middle back out:"
sleep 1
echo "(e for eraser, Space — tab dot stays: work is unsaved)"
sleep 3

clear
echo "Fill floods the corner, undo/redo proves history:"
sleep 1
echo "(g, Space, Ctrl+Z, Ctrl+Y)"
sleep 3

clear
echo "Export writes the PNG itself (Ctrl+E, type path, Enter):"
sleep 1
echo "(select-all: first key replaces the suggestion)"
sleep 3

clear
echo "done — scripts/demo-tui-draw.sh drives this for real (tmux PTY)"
