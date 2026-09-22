# E2E Testing Plan — Figby TUI

## Overview

Automated end-to-end testing for the Figby TUI application using three complementary strategies:
1. **tmux send-keys** — keystroke automation in a virtual terminal
2. **Visual diffing** — terminal screenshot comparison
3. **macOS Accessibility API** — native computer-use automation (CUA)

---

## Strategy 1: tmux + send-keys Automation

### Setup

```bash
# Create named session with fixed size
tmux new-session -d -s figby-test -x 120 -y 40

# Launch figby in the session
tmux send-keys -t figby-test 'cargo run --manifest-path figby-rs/Cargo.toml' Enter

# Wait for TUI to initialize
sleep 2
```

### Keystroke Simulation

```bash
# Send single key
tmux send-keys -t figby-test 'q'

# Send Ctrl+O (open file)
tmux send-keys -t figby-test C-o

# Send F11 (zen mode)
tmux send-keys -t figby-test F11

# Send Escape
tmux send-keys -t figby-test Escape

# Send arrow keys
tmux send-keys -t figby-test Up
tmux send-keys -t figby-test Down

# Send text input
tmux send-keys -t figby-test 'Hello World'
```

### Capture Terminal State

```bash
# Capture pane content (text)
tmux capture-pane -t figby-test -p > /tmp/figby-output.txt

# Capture pane as PNG (requires tmux 3.2+)
tmux capture-pane -t figby-test -p -e | head -40  # text fallback
```

### Test Workflow

```bash
#!/bin/bash
# scripts/e2e-tmux-test.sh

SESSION="figby-e2e"
tmux new-session -d -s $SESSION -x 120 -y 40

# Launch app
tmux send-keys -t $SESSION 'cargo run --manifest-path figby-rs/Cargo.toml' Enter
sleep 3

# Test 1: Welcome screen appears
tmux capture-pane -t $SESSION -p | grep -q "Figby" && echo "PASS: Welcome" || echo "FAIL: Welcome"

# Test 2: Dismiss welcome with Enter
tmux send-keys -t $SESSION Enter
sleep 1
tmux capture-pane -t $SESSION -p | grep -q "Brush" && echo "PASS: Main UI" || echo "FAIL: Main UI"

# Test 3: Zen mode toggle
tmux send-keys -t $SESSION F11
sleep 1
tmux capture-pane -t $SESSION -p > /tmp/zen-mode.txt
# Verify zen mode UI elements

# Test 4: Open file dialog
tmux send-keys -t $SESSION C-o
sleep 1
tmux capture-pane -t $SESSION -p | grep -q "Open" && echo "PASS: Open dialog" || echo "FAIL: Open dialog"

# Test 5: Escape closes dialog
tmux send-keys -t $SESSION Escape
sleep 1

# Test 6: Draw on canvas
tmux send-keys -t $SESSION 'b'  # brush tool
sleep 0.5
tmux send-keys -t $SESSION Enter  # paint
sleep 0.5

# Cleanup
tmux kill-session -t $SESSION
```

---

## Strategy 2: Visual Diff Testing

### Concept

Compare terminal buffer snapshots against known-good baselines.

### Implementation

```rust
// figby-rs/tests/tui_visual.rs

use std::process::{Command, Stdio};
use std::io::Write;

struct TuiTestHarness {
    session: String,
}

impl TuiTestHarness {
    fn new(session_name: &str) -> Self {
        // Create tmux session
        Command::new("tmux")
            .args(["new-session", "-d", "-s", session_name, "-x", "120", "-y", "40"])
            .spawn()
            .expect("failed to create tmux session");
        
        Self { session: session_name.to_string() }
    }
    
    fn launch(&self) {
        let root = std::env::var("CARGO_MANIFEST_DIR")
            .map(std::path::PathBuf::from)
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        
        Command::new("tmux")
            .args(["send-keys", "-t", &self.session,
                   &format!("cargo run --manifest-path {}/figby-rs/Cargo.toml", root.display()),
                   "Enter"])
            .spawn()
            .expect("failed to send launch command");
        
        std::thread::sleep(std::time::Duration::from_secs(3));
    }
    
    fn send_key(&self, key: &str) {
        Command::new("tmux")
            .args(["send-keys", "-t", &self.session, key])
            .spawn()
            .expect("failed to send key");
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    
    fn send_ctrl(&self, ch: char) {
        Command::new("tmux")
            .args(["send-keys", "-t", &self.session, &format!("C-{}", ch)])
            .spawn()
            .expect("failed to send ctrl key");
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    
    fn capture(&self) -> String {
        let output = Command::new("tmux")
            .args(["capture-pane", "-t", &self.session, "-p"])
            .output()
            .expect("failed to capture pane");
        String::from_utf8_lossy(&output.stdout).to_string()
    }
    
    fn capture_lines(&self) -> Vec<String> {
        self.capture().lines().map(String::from).collect()
    }
    
    fn kill(&self) {
        let _ = Command::new("tmux")
            .args(["kill-session", "-t", &self.session])
            .spawn();
    }
}

#[test]
fn test_welcome_screen() {
    let harness = TuiTestHarness::new("e2e-welcome");
    harness.launch();
    
    let output = harness.capture();
    
    // Baseline: welcome screen shows version
    assert!(output.contains("6.0"), "Welcome should show version");
    
    // Baseline: recent files section exists
    // (capture and compare against stored baseline)
    
    harness.kill();
}

#[test]
fn test_main_ui_layout() {
    let harness = TuiTestHarness::new("e2e-layout");
    harness.launch();
    
    // Dismiss welcome
    harness.send_key("Enter");
    std::thread::sleep(std::time::Duration::from_millis(500));
    
    let lines = harness.capture_lines();
    
    // Verify toolbox visible (left column)
    // Verify canvas area present
    // Verify status bar at bottom
    
    let has_toolbox = lines.iter().any(|l| l.contains("Brush") || l.contains("Toolbox"));
    assert!(has_toolbox, "Toolbox should be visible");
    
    let has_status = lines.last().map(|l| l.contains("Ln") || l.contains("Col")).unwrap_or(false);
    assert!(has_status, "Status bar should be visible");
    
    harness.kill();
}

#[test]
fn test_tool_switching() {
    let harness = TuiTestHarness::new("e2e-tools");
    harness.launch();
    harness.send_key("Enter"); // dismiss welcome
    std::thread::sleep(std::time::Duration::from_millis(500));
    
    // Switch to eraser
    harness.send_key("e");
    let output = harness.capture();
    assert!(output.contains("Eraser"), "Eraser tool should be active");
    
    // Switch to fill
    harness.send_key("f");
    let output = harness.capture();
    assert!(output.contains("Fill"), "Fill tool should be active");
    
    // Switch to line
    harness.send_key("l");
    let output = harness.capture();
    assert!(output.contains("Line"), "Line tool should be active");
    
    harness.kill();
}
```

---

## Strategy 3: macOS Accessibility API (CUA)

### Approach

Use Apple's Accessibility APIs via the `accessibility` crate (or raw FFI) to:
- Query TUI element tree
- Send synthetic events
- Verify UI state

### Dependencies

```toml
[dev-dependencies]
# macOS accessibility bindings
core-foundation = "0.9"
core-graphics = "0.22"
# Optional: higher-level wrapper
# accessibility = "0.1"  # if available
```

### Implementation Sketch

```rust
// figby-rs/tests/tui_accessibility.rs

#![cfg(target_os = "macos")]

use core_foundation::base::TCFType;
use core_foundation::string::CFString;
use core_graphics::geometry::CGPoint;

/// Send a synthetic keyboard event via Quartz event taps
fn send_key_event(key_code: u16, flags: u32) {
    use core_graphics::event::*;
    use core_graphics::event_source::*;
    
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState).unwrap();
    
    // Key down
    let event = CGEvent::new_keyboard_event(source.clone(), key_code, true).unwrap();
    event.set_flags(CGEventFlags::from_bits(flags).unwrap());
    event.post(CGEventTapLocation::HID);
    
    // Key up
    let event = CGEvent::new_keyboard_event(source, key_code, false).unwrap();
    event.post(CGEventTapLocation::HID);
}

/// Map tmux key names to macOS key codes
fn key_code_for(key: &str) -> u16 {
    match key {
        "a" => 0x00, "s" => 0x01, "d" => 0x02, "f" => 0x03,
        "h" => 0x04, "g" => 0x05, "z" => 0x06, "x" => 0x07,
        "c" => 0x08, "v" => 0x09, "b" => 0x0B, "q" => 0x0C,
        "w" => 0x0D, "e" => 0x0E, "r" => 0x0F, "y" => 0x10,
        "t" => 0x11, "1" => 0x12, "2" => 0x13, "3" => 0x14,
        "4" => 0x15, "6" => 0x16, "5" => 0x17, "=" => 0x18,
        "9" => 0x19, "7" => 0x1A, "-" => 0x1B, "8" => 0x1C,
        "0" => 0x1D, "]" => 0x1E, "o" => 0x1F, "u" => 0x20,
        "[" => 0x21, "i" => 0x22, "p" => 0x23, "l" => 0x25,
        "j" => 0x26, "'" => 0x27, "k" => 0x28, ";" => 0x29,
        "\\" => 0x2A, "," => 0x2B, "/" => 0x2C, "n" => 0x2D,
        "m" => 0x2E, "." => 0x2F, "return" => 0x24,
        "tab" => 0x30, "space" => 0x31, "delete" => 0x33,
        "escape" => 0x35, "up" => 0x7E, "down" => 0x7D,
        "left" => 0x7B, "right" => 0x7C,
        "f1" => 0x7A, "f2" => 0x78, "f11" => 0x67,
        _ => panic!("unknown key: {}", key),
    }
}

const CMD: u32 = 1 << 20;
const CTRL: u32 = 1 << 12;
const SHIFT: u32 = 1 << 17;

#[test]
fn test_cua_ctrl_o_opens_dialog() {
    // Launch figby first (via tmux or direct)
    // Then send Ctrl+O via CGEvent
    send_key_event(key_code_for("o"), CTRL);
    std::thread::sleep(std::time::Duration::from_millis(500));
    // Capture and verify dialog opened
}
```

### Accessibility Tree Inspection

```rust
/// Use AXUIElement to inspect app's accessibility tree
fn inspect_tui_elements() {
    use core_foundation::array::CFArrayGetCount;
    
    // Get PID of figby process
    let pid = find_process_by_name("figby").expect("figby not running");
    
    // Create AXUIElement for app
    let app = unsafe {
        let element = core_foundation::sys::AXUIElementCreateApplication(pid);
        // Query children, roles, values...
    };
    
    // Walk the element tree
    // - Verify "Brush" tool is accessible
    // - Verify canvas area has correct role
    // - Verify status bar text content
}
```

---

## Test Scenarios Matrix

| # | Scenario | Strategy | Keys | Verify |
|---|----------|----------|------|--------|
| 1 | Welcome screen | tmux + visual | none | Version text, menu items |
| 2 | Dismiss welcome | tmux | Enter | Main UI appears |
| 3 | Zen mode | tmux + visual | F11 | Toolbox hidden, hint bar |
| 4 | Open file dialog | tmux | Ctrl+O | Dialog visible |
| 5 | Close dialog | tmux | Escape | Dialog hidden |
| 6 | Tool switching | tmux + visual | b/e/f/l | Status bar shows tool |
| 7 | Draw on canvas | tmux + CGEvent | mouse | Canvas pixel changes |
| 8 | Undo | tmux | Ctrl+Z | Canvas reverts |
| 9 | Redo | tmux | Ctrl+Shift+Z | Canvas restores |
| 10 | New file | tmux | Ctrl+N | Fresh canvas |
| 11 | Save file | tmux | Ctrl+S | File written |
| 12 | Export PNG | tmux + visual | Ctrl+E | Export dialog |
| 13 | Timeline | tmux | Ctrl+T | Timeline visible |
| 14 | Brush size | tmux + props | [/] | Size changes |
| 15 | Font editor | tmux | Tab | Editor mode |
| 16 | Quit | tmux | q | Process exits |

---

## File Structure

```
figby-rs/tests/
├── tui_visual.rs           # Visual diff tests (baseline comparison)
├── tui_accessibility.rs    # macOS Accessibility API tests
├── tui_tmux.rs             # tmux send-keys automation
├── tui_baselines/          # Expected terminal captures
│   ├── welcome.txt
│   ├── main_ui.txt
│   ├── zen_mode.txt
│   ├── open_dialog.txt
│   └── ...
└── run_tests.rs            # Existing CLI tests

scripts/
├── e2e-tmux-test.sh        # Shell-based tmux automation
├── e2e-capture-baseline.sh # Capture baseline screenshots
└── e2e-compare.sh          # Diff captured vs baseline
```

---

## CI Integration

### GitHub Actions (Linux)

```yaml
- name: E2E Tests (tmux)
  run: |
    tmux new-session -d -s test -x 120 -y 40
    cargo test --test tui_tmux -- --test-threads=1
```

### macOS Runner (Accessibility API)

```yaml
- name: E2E Tests (macOS Accessibility)
  runs-on: macos-latest
  steps:
    - name: Grant Accessibility
      run: |
        # For CI, may need to add to Accessibility list
        sudo sqlite3 "/Library/Application Support/com.apple.TCC/TCC.db" \
          "INSERT INTO access (service, client, client_type, auth_value, auth_reason) 
           VALUES ('kTCCServiceAccessibility', 'com.github.figby', 0, 2, 2);"
    - name: Run Accessibility Tests
      run: cargo test --test tui_accessibility -- --test-threads=1
```

---

## Recommended Approach

1. **Start with tmux** — fastest to implement, works everywhere, covers most scenarios
2. **Add visual diffing** — baseline capture + comparison for regression detection
3. **Add Accessibility API** — for macOS-specific testing and precise element inspection

### Quick Win: Shell Script + tmux

```bash
# scripts/e2e-quick.sh
set -e
SESSION="figby-e2e-$$"

cleanup() { tmux kill-session -t $SESSION 2>/dev/null; }
trap cleanup EXIT

tmux new-session -d -s $SESSION -x 120 -y 40
tmux send-keys -t $SESSION "cargo run --manifest-path figby-rs/Cargo.toml" Enter
sleep 3

# Test welcome screen
tmux capture-pane -t $SESSION -p > /tmp/e2e-welcome.txt
grep -q "Figby" /tmp/e2e-welcome.txt || { echo "FAIL: Welcome"; exit 1; }

# Dismiss and test main UI
tmux send-keys -t $SESSION Enter
sleep 1
tmux capture-pane -t $SESSION -p > /tmp/e2e-main.txt
grep -q "Brush" /tmp/e2e-main.txt || { echo "FAIL: Main UI"; exit 1; }

# Zen mode
tmux send-keys -t $SESSION F11
sleep 1
tmux capture-pane -t $SESSION -p > /tmp/e2e-zen.txt
grep -q "zen" /tmp/e2e-zen.txt || { echo "FAIL: Zen mode"; exit 1; }

echo "ALL E2E TESTS PASSED"
```

---

## Next Steps

1. Create `scripts/e2e-quick.sh` with basic tmux tests
2. Add `tui_tmux.rs` integration test module
3. Capture baseline screenshots for visual diffing
4. Investigate `accessibility` crate for macOS CUA testing
5. Wire E2E tests into CI pipeline
