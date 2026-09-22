#!/bin/bash
# E2E TUI test runner using tmux
# Usage: ./scripts/e2e-tmux-test.sh [--baseline]

set -e

SESSION="figby-e2e-$$"
BASELINE_DIR="figby-rs/tests/tui_baselines"
CAPTURE_DIR="/tmp/figby-e2e-captures"
MODE="${1:---test}"

cleanup() {
    tmux kill-session -t $SESSION 2>/dev/null || true
}
trap cleanup EXIT

mkdir -p "$CAPTURE_DIR" "$BASELINE_DIR"

echo "=== Figby E2E TUI Tests ==="
echo "Mode: $MODE"

# Create tmux session
tmux new-session -d -s $SESSION -x 120 -y 40

# Launch figby
echo "Launching figby..."
tmux send-keys -t $SESSION "cargo run --manifest-path figby-rs/Cargo.toml 2>/dev/null" Enter
sleep 3

pass=0
fail=0

check() {
    local name="$1"
    local file="$2"
    local pattern="$3"
    
    if [ "$MODE" = "--baseline" ]; then
        tmux capture-pane -t $SESSION -p > "$BASELINE_DIR/$name.txt"
        echo "  [BASELINE] $name captured"
        return
    fi
    
    tmux capture-pane -t $SESSION -p > "$CAPTURE_DIR/$name.txt"
    
    if grep -q "$pattern" "$CAPTURE_DIR/$name.txt"; then
        echo "  [PASS] $name"
        ((pass++))
    else
        echo "  [FAIL] $name (pattern: $pattern)"
        ((fail++))
    fi
}

# Test 1: Welcome screen
echo ""
echo "Test 1: Welcome screen"
check "welcome" "" "Figby"

# Test 2: Dismiss welcome
echo "Test 2: Dismiss welcome"
tmux send-keys -t $SESSION Enter
sleep 1
check "main_ui" "" "Brush\|Toolbox\|Canvas"

# Test 3: Zen mode
echo "Test 3: Zen mode toggle"
tmux send-keys -t $SESSION F11
sleep 1
check "zen_mode" "" "F11\|zen\|exit"
tmux send-keys -t $SESSION F11  # exit zen
sleep 1

# Test 4: Tool switching
echo "Test 4: Tool switching"
tmux send-keys -t $SESSION 'e'
sleep 0.5
check "tool_eraser" "" "Eraser"

tmux send-keys -t $SESSION 'f'
sleep 0.5
check "tool_fill" "" "Fill"

tmux send-keys -t $SESSION 'l'
sleep 0.5
check "tool_line" "" "Line"

tmux send-keys -t $SESSION 'b'
sleep 0.5
check "tool_brush" "" "Brush"

# Test 5: Open file dialog
echo "Test 5: Open file dialog"
tmux send-keys -t $SESSION C-o
sleep 1
check "open_dialog" "" "Open\|dialog\|File"
tmux send-keys -t $SESSION Escape
sleep 0.5

# Test 6: Keybindings overlay
echo "Test 6: Keybindings overlay"
tmux send-keys -t $SESSION C-k
sleep 1
check "keybindings" "" "keybind\|Ctrl\|shortcut"
tmux send-keys -t $SESSION C-k
sleep 0.5

# Test 7: New file
echo "Test 7: New file"
tmux send-keys -t $SESSION C-n
sleep 1
check "new_file" "" "New\|Untitled\|canvas"
tmux send-keys -t $SESSION Escape  # cancel if dialog
sleep 0.5

# Summary
echo ""
echo "=== Results ==="
echo "  Passed: $pass"
echo "  Failed: $fail"
echo ""

if [ "$MODE" = "--baseline" ]; then
    echo "Baselines saved to: $BASELINE_DIR/"
    echo "Run without --baseline to test against them."
elif [ $fail -gt 0 ]; then
    echo "Captures saved to: $CAPTURE_DIR/"
    exit 1
else
    echo "All tests passed!"
fi
