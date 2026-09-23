#!/bin/bash
# Non-headless (real PTY) E2E for the e2e-art scenes. Requires tmux.
# Unlike figby-rs/tests/e2e_art.rs (headless structural checks only), every
# test here drives the real TUI in a tmux PTY: welcome dismiss, Open dialog
# typing + Tab-arm + Enter, canvas content via capture-pane, tool switching
# via the status bar, timeline/export dialogs, and --play --loop playback.
#
# Usage (from repo root):
#   ./scripts/e2e-art-tmux.sh
# Exit 0 = all pass, 1 = any failure. Slow (~1 min): one TUI launch per scene
# so every open starts from FontEditor mode, where Ctrl+O routes to the
# .figmap-capable Open dialog (in ImageEditor mode Ctrl+O opens the image
# dialog instead — see docs/e2e-tui-art-tests.md issue #5).

set -u

BIN="figby-rs/target/debug/figby"
ART="assets/e2e-art"
PASS=0
FAIL=0

if ! command -v tmux >/dev/null; then
    echo "FAIL: tmux not installed"
    exit 1
fi
if [ ! -x "$BIN" ]; then
    echo "Building figby..."
    cargo build --manifest-path figby-rs/Cargo.toml || exit 1
fi
check_scene() { # $1=file (basename) $2=marker-grep $3=label
    local S="figby-art-$RANDOM"
    tmux new-session -d -s "$S" -x 120 -y 40
    tmux send-keys -t "$S" "./$BIN --tui" Enter
    sleep 4
    tmux send-keys -t "$S" Escape   # dismiss welcome (Enter does NOT dismiss)
    sleep 0.8
    # Type the full relative path; disarmed-Enter finalizes existing files
    # directly (digits mid-path are literal — recents only fire on an empty
    # Path). Launched from FontEditor mode so Ctrl+O reaches the
    # .figmap-capable Open dialog.
    tmux send-keys -t "$S" C-o      # Open dialog (FontEditor mode)
    sleep 0.8
    tmux send-keys -t "$S" -l "$ART/$1"
    sleep 0.8
    tmux send-keys -t "$S" Enter    # open (buffer names an existing file)
    sleep 1.5
    local cap="/tmp/e2e-art-$1.txt"
    tmux capture-pane -t "$S" -p > "$cap" 2>/dev/null
    if grep -aq "$2" "$cap"; then
        echo "  [PASS] $3"
        PASS=$((PASS+1))
    else
        echo "  [FAIL] $3 (marker '$2' missing)"
        FAIL=$((FAIL+1))
    fi
    tmux send-keys -t "$S" C-c; sleep 0.3
    tmux send-keys -t "$S" q; sleep 0.5
    tmux kill-session -t "$S" 2>/dev/null || true
}
check_tools_and_dialogs() {
    local S="figby-art-tools-$RANDOM"
    tmux new-session -d -s "$S" -x 120 -y 40
    tmux send-keys -t "$S" "./$BIN --tui" Enter
    sleep 4
    tmux send-keys -t "$S" Escape; sleep 0.8
    # Reach AsciiPreview: Tab cycles FontEditor -> ImageEditor ->
    # AsciiPreview (Any session). ImageEditor swallows i/d/c/k/r into its
    # own adjustments, so tools assert in AsciiPreview where all 16 keys
    # dispatch (docs/e2e-tui-art-tests.md issue #2).
    tmux send-keys -t "$S" Tab; sleep 0.6
    tmux send-keys -t "$S" Tab; sleep 0.8
    # Tool keys assert via the status bar (lowercase).
    for spec in "b:Brush" "e:Eraser" "g:Fill" "a:Spray" "u:Move" "v:Select" "i:Line" "t:Text"; do
        local k="${spec%%:*}" want="${spec##*:}"
        tmux send-keys -t "$S" "$k"; sleep 0.4
        # Escape Text-tool capture so the next key dispatches as a tool key.
        if [ "$k" = "t" ]; then tmux send-keys -t "$S" Escape; sleep 0.3; fi
        tmux capture-pane -t "$S" -p 2>/dev/null | tail -1 | grep -aq "$want" \
            && { echo "  [PASS] tool $k -> $want"; PASS=$((PASS+1)); } \
            || { echo "  [FAIL] tool $k -> $want"; FAIL=$((FAIL+1)); }
    done
    # Timeline toggle shows the timeline panel ("No frames in timeline" when
    # empty); second T closes it.
    tmux send-keys -t "$S" T; sleep 0.8
    tmux capture-pane -t "$S" -p 2>/dev/null | grep -aq -i "timeline" \
    # Export dialog opens; Esc closes it (no Enter: that would fire async export).
    tmux send-keys -t "$S" C-e; sleep 0.8
    tmux capture-pane -t "$S" -p 2>/dev/null | grep -aq "Format:" \
        && { echo "  [PASS] export opens"; PASS=$((PASS+1)); } \
        || { echo "  [FAIL] export opens"; FAIL=$((FAIL+1)); }
    tmux send-keys -t "$S" Escape; sleep 0.8
    tmux capture-pane -t "$S" -p 2>/dev/null | grep -aq "Format:" \
        && { echo "  [FAIL] export closes"; FAIL=$((FAIL+1)); } \
        || { echo "  [PASS] export closes"; PASS=$((PASS+1)); }
    tmux send-keys -t "$S" C-c; sleep 0.3
    tmux send-keys -t "$S" q; sleep 0.5
    tmux kill-session -t "$S" 2>/dev/null || true
}

check_banner_play() {
    local S="figby-art-play-$RANDOM"
    tmux new-session -d -s "$S" -x 80 -y 30
    tmux send-keys -t "$S" "./$BIN --play $ART/banner-figby-finale.figmap --loop" Enter
    sleep 5
    local cap="/tmp/e2e-art-play.txt"
    # Progress row renders a few seconds in (frames cycle first): poll up
    # to ~10s for the "1/6" marker instead of single-shot capture.
    local tries=0
    while [ "$tries" -lt 5 ]; do
        tmux capture-pane -t "$S" -p > "$cap" 2>/dev/null
        grep -aq "1/6" "$cap" && break
        sleep 2
        tries=$((tries+1))
    done
    if grep -aq "1/6" "$cap"; then
        echo "  [PASS] banner --play renders + progress 1/6"
        PASS=$((PASS+1))
    else
        echo "  [FAIL] banner --play renders + progress 1/6"
        FAIL=$((FAIL+1))
    fi
    tmux send-keys -t "$S" x; sleep 1  # any key exits --loop
    tmux kill-session -t "$S" 2>/dev/null || true
}
echo "-- scene opens (real Open dialog) --"
check_scene "sprite-plumber-tribute.figmap" "RRRR" "plumber tribute opens, hero visible"
check_scene "sprite-quest-tribute.figmap" "TTTT" "quest tribute opens, hero visible"
check_scene "sprite-invader-tribute.figmap" "WWWWWWWW" "invader tribute opens, hero visible"
check_scene "layers-castle-tribute.figmap" "Castle" "castle opens, Castle layer visible"
echo ""
echo "-- tools + dialogs (real key dispatch) --"
check_tools_and_dialogs
echo ""
echo "-- banner playback (real PTY) --"
check_banner_play
echo ""
echo "=== Results: passed $PASS, failed $FAIL ==="
[ "$FAIL" -eq 0 ]
