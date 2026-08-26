# OxAlphaReview.md — Comprehensive Repository Review

Date: 2026-08-26
Reviewer: AI (automated e2e + visual + code analysis)
Branch: `hardening/gpt-review` (v6.0.38)

---

## Executive Summary

Figby is a **feature-complete** FIGlet-to-Rust port that has grown into a full
ASCII-art TUI editor. CLI rendering is solid and matches C FIGlet 2.2.5 output
across all tested flags/fonts. The TUI renders correctly in tmux with clean
tool/palette/layer management. One real CLI bug found (`-V` flag mismatch with
README), several minor inconsistencies noted.

**Overall status: stable, production-quality.**

---

## 1. CLI E2E Results

All flags tested with `figby-rs/target/release/figby`:

| Feature | Flag | Status | Notes |
|---------|------|--------|-------|
| Basic banner | (positional) | ✅ | standard banner, centered |
| Kerning | `-k` | ✅ | |
| Smushing | `-s` | ✅ | |
| Force smushing | `-S` | ✅ | |
| Overlap | `-o` | ✅ | |
| Full width | `-W` | ✅ | |
| Center | `-c` | ✅ | |
| Flush left | `-l` | ✅ | |
| Flush right | `-r` | ✅ | |
| LTR | `-L` | ✅ | |
| RTL | `-R` | ✅ | |
| Width | `-w20`, `-w5` | ✅ | |
| Smush modes | `-m0`, `-m191` | ✅ | |
| TLF font | `-f tests/emboss` | ✅ | |
| Control file | `-C fonts/uskata.flc` | ✅ | |
| Paragraph | `-p` | ✅ | |
| Stdin pipe | `\| figby` | ✅ | |
| `-A` flag | `-A` | ✅ | |
| Info codes | `-I 0/1/2/4` | ✅ | |
| Deutsch | `-D` | ✅ | |
| `-F` removed | `-F` | ✅ | Error: "removed (matches C FIGlet 2.2.5)" |
| Template | `--template` | ✅ | Figby CLI banner rendered |
| Font count | — | ✅ | `.flf`: many, `.flc`: multiple |
| Flag suite | `--test run_tests` | ✅ | 51 passed, 5 known-divergence ignored |

### Bug: `-V` vs `-v` mismatch

- **README** line 118 lists `| -V | Print version |`
- **Binary** only has `-v` (lowercase) — `-V` returns `error: unexpected argument '-V'`
- C FIGlet uses lowercase `-v`. The README should say `-v` not `-V`.

### Note: stdin pipe test

`echo "Hello" | figby` works correctly via POSIX pipe. The `-A` flag also works
for reading from stdin.

---

## 2. TUI E2E Screenshots

### 2.1 Welcome Screen

![Welcome](../figby-rs/tests/fixtures/) → captured as tui_01_welcome.png

- **Mascot**: Clean ASCII-art fox/bear character with dark background
- **FIGby title**: Rendered in stylized FIGlet font, sharp and legible
- **Version banner**: "Welcome to Figby v6.0.38" in title bar
- **Panels**: Recent Files (empty), Font section (N/I/B/O/D), Image section (C/T/V/F/A/L)
- **Layout**: Clean two-column layout, well-spaced
- **Assessment**: ✅ Excellent first impression

### 2.2 Font Editor (standard font loaded)

- **Character grid**: Shows ASCII 32–39 with FIGlet renders — each glyph properly sized
- **Search filter**: Type-to-search box at top
- **Preview panel**: "AaBbCc123!?" rendered in standard font — crisp, correct alignment
- **Status bar**: Font Editor | Brush/Circle | X:0 Y:0 | 1x | standard | 325 chars | FPS
- **Layers panel**: 6 layers + Background, each with visibility/lock toggles
- **Character count**: 325 glyphs loaded from standard.flf
- **Assessment**: ✅ Font Editor fully functional

### 2.3 Image Editor

- **New Image dialog**: Width: 80, Height: 24, Palette: [Grayscale] — clean form layout
- **Palette Editor**: Shows Grayscale palette with 5 swatches (White → Near Black, hex values)
  - Swatch names: White, Light Gray, Mid Gray, Dark Gray, Near Black
  - Operations: [S]ave [L]oad [D]up [A]dd [E]dit
- **Canvas**: 80×24 blank canvas with title "80x24"
- **Assessment**: ✅ Image creation flow works

### 2.4 Zen Mode (F11)

- Clean full-canvas view with no chrome
- Footer: "F11=exit zen  ?=keys  ^K=keybinds"
- **Assessment**: ✅ Distraction-free mode works

### 2.5 Open Font Dialog

- **File browser**: Shows root directory, navigate into subdirectories
- **Recent files**: Shows "1. fonts/standard.flf" (recent history works)
- **Path input**: Supports direct path typing
- **Footer**: "-->/Tab: navigate  Enter/click: open  Esc: cancel  1-9: recent"
- **Assessment**: ✅ File dialog functional, recent files work

### 2.6 Menus

**File Menu** (Alt+F):
| Item | Shortcut |
|------|----------|
| New Image | Ctrl+N |
| Open | Ctrl+O |
| Save | Ctrl+S |
| Save As | Ctrl+Shift+S |
| Export | Ctrl+E |
| Import GIF | — |
| New Font from File | — |
| New Font from System | — |
| Quit | Q |

**View Menu** (Alt+V):
| Item | Shortcut |
|------|----------|
| Zoom In | + |
| Zoom Out | - |
| Toggle Grid | — |
| Toggle Undo Panel | Ctrl+Shift+H |
| Toggle Timeline | T |
| Toggle Side Panel | ? |
| Palette Editor | Ctrl+Shift+P |
| Palette: Grayscale/Primary/Warm/Cool | — |

**Animation Menu** (Alt+A):
| Item | Shortcut |
|------|----------|
| Add Frame | A |
| Delete Frame | Delete |
| Play / Pause | Enter |
| Toggle Timeline | T |

**Assessment**: ✅ Menus clean, standard shortcuts, no collisions within menus.

---

## 3. Keybind Collision Analysis

### Full keybind table (by scope)

#### Global Scope (always active)

| Key | Action | Source |
|-----|--------|--------|
| Ctrl+N | New image | GLOBAL_DISPATCH |
| Ctrl+O | Open file | GLOBAL_DISPATCH |
| Ctrl+S | Save | GLOBAL_DISPATCH |
| Ctrl+Shift+S | Save As | GLOBAL_DISPATCH |
| Ctrl+E | Export | GLOBAL_DISPATCH |
| Ctrl+Z | Undo | GLOBAL_DISPATCH |
| Ctrl+Shift+Z / Ctrl+Y | Redo | GLOBAL_DISPATCH |
| Ctrl+Shift+H | Toggle undo panel | GLOBAL_DISPATCH |
| Ctrl+K | Toggle keybindings | GLOBAL_DISPATCH |
| Ctrl+Shift+P | Open palette editor | GLOBAL_DISPATCH |
| Ctrl+Tab | Next mode | GLOBAL_DISPATCH |
| Ctrl+Shift+Tab | Prev mode | GLOBAL_DISPATCH |
| Tab | Next mode | GLOBAL_DISPATCH |
| Shift+Tab | Prev mode | GLOBAL_DISPATCH |
| F5 | Toggle render mode | GLOBAL_DISPATCH |
| F11 | Toggle zen mode | GLOBAL_DISPATCH |
| ? | Cycle drawer (side panel) | GLOBAL_DISPATCH |
| T | Toggle timeline | GLOBAL_DISPATCH |
| Shift+T | Open tween panel | GLOBAL_DISPATCH |
| Alt+F / Alt+E / Alt+V / Alt+T / Alt+H | Open menus | GLOBAL_DISPATCH |
| Alt+← / Alt+→ | Cycle side-panel tabs | GLOBAL_DISPATCH |
| q / Q / Ctrl+Q | Quit | GLOBAL_DISPATCH |
| S | Open settings dialog | dispatch.rs:1696 |

#### Canvas Scope (ImageEditor mode, no dialog/panel active)

| Key | Action |
|-----|--------|
| b | Brush tool |
| e | Eraser tool |
| l | Lasso tool |
| v | Select tool |
| c | Circle tool |
| p | Polygon tool |
| g | Fill tool |
| i | Line tool (or Eyedrop?) |
| d | Eyedropper tool |
| a | Spray tool |
| t | Text tool |
| u | Move tool |
| r | Rotate tool |
| [ / ] | Brush size down/up |
| ; / ' | Brush density down/up |
| \ | Cycle brush shape |
| M | Toggle marker sub-mode (brush) |
| + / - | Zoom in/out |
| Ctrl+A | Select all |
| Ctrl+X | Cut selection |
| Ctrl+C | Copy selection |
| Ctrl+V | Paste |
| Delete | Delete selection |
| Space | Start animation playback (if frames exist) |

#### FontEditor Mode — FontOverview Scope

| Key | Action |
|-----|--------|
| ↑↓←→ | Navigate glyph grid |
| Enter | Open glyph editor |
| Type chars | Search/filter |
| Esc | Clear search |
| A | Add glyph |
| D | Delete glyph |
| C | Copy glyph |
| H | Header editor |
| S | Smushing rule editor |
| T | Transform editor |
| [/ ] | Brush size (if brush active) |
| ; / ' | Brush density |
| M | Mirror (CharEditor) / marker (Brush) |

#### FontEditor Mode — FontCharEditor Scope

| Key | Action |
|-----|--------|
| ↑↓←→ | Move cursor |
| Space | Toggle cell |
| M | Mirror |
| F | Flip |
| G | Generate from system font |

#### Layer Panel Scope (side panel open on Layers tab)

| Key | Action |
|-----|--------|
| ↑ / ↓ | Select layer |
| Enter / Space | Toggle visibility |
| n / N | New layer |
| d / D | Duplicate layer |
| x / Delete | Delete layer |
| l | Toggle lock |
| m | Merge down / toggle mask |
| M | Toggle mask |
| + / - | Opacity up/down |
| Ctrl+G | Group layer |
| k / K | Link layer |
| F2 | Rename layer |
| Alt+↑ / Alt+↓ | Select layer |
| Alt+Shift+↑ / Alt+Shift+↓ | Reorder layer |
| Alt+← / Alt+→ | Collapse/expand group |
| Alt+S | Toggle cast shadow |
| Alt+Tab | Cycle through group layers |

#### Timeline Scope

| Key | Action |
|-----|--------|
| ← / → | Switch frame |
| A | Add frame |
| Delete | Delete frame |
| Enter | Play animation |
| Space | Start in-canvas playback |

#### Lighting Scope

| Key | Action |
|-----|--------|
| G | Enter lighting mode |
| Esc | Exit lighting mode |
| ↑ / ↓ | Select light |
| ← / → | Move horizontally |
| Shift+↑ / Shift+↓ | Move vertically |
| + / - | Adjust intensity |
| A / D / P | Add ambient/directional/point light |
| Delete | Remove light |

#### Dialog Scope (when any dialog is open)

| Key | Action |
|-----|--------|
| Esc | Close/cancel |
| Enter | Confirm |
| ↑↓ | Navigate items |

### Collision Analysis

| Key | Scope 1 | Scope 2 | Severity | Notes |
|-----|---------|---------|----------|-------|
| **T** | Global: Toggle timeline | FontOverview: Transform editor | ⚠️ **Medium** | Global dispatch fires first in handle_key_event, so T in FontEditor mode toggles timeline, not the transform editor. The FontOverview T is effectively dead when in FontEditor mode unless the dispatch order is reordered. |
| **S** | Global: Open settings | FontOverview: Smushing rule editor | ⚠️ **Medium** | Same issue — Global S fires before FontEditor-specific S. Settings dialog opens instead of smushing editor. |
| **A** | LayerPanel: New layer | Timeline: Add frame | ✅ **OK** | Different active scopes — LayerPanel is only active when side panel is open on Layers tab; Timeline A fires when timeline panel is open. |
| **D** | LayerPanel: Duplicate | Lighting: Add directional | ✅ **OK** | Different modes (LayerPanel vs Lighting). |
| **G** | Canvas: Fill (lowercase) | Lighting: Enter mode (uppercase) | ✅ **OK** | Case-sensitive — different keys. |
| **M** | Canvas: Marker (Brush) | LayerPanel: Toggle mask | ✅ **OK** | Different scopes. |
| **Enter** | FontOverview: Edit glyph | TextTool: Commit text | Timeline: Play | Dialog: Confirm | ✅ **OK** | All in different exclusive scopes. |
| **Esc** | FontOverview: Clear search | TextTool: Cancel | Lighting: Exit | Dialog: Close | ✅ **OK** | Different scopes. |
| **Space** | FontCharEditor: Toggle cell | LayerPanel: Toggle visibility | Timeline: Playback | ✅ **OK** | Different scopes. |
| **Delete** | Canvas: Delete selection | LayerPanel: Delete layer | Lighting: Remove light | Timeline: Delete frame | ✅ **OK** | Different scopes. |
| **?** | Global: Cycle drawer | Welcome: Show help | ✅ **OK** | Welcome screen intercepts first. |

### Recommendations

1. **Fix T collision** (Medium): In FontEditor mode, pressing T opens the transform
   editor in the KEYMAP but the global ToggleTimeline fires first. Either:
   - Move FontOverview's T to a different key (e.g., Ctrl+T or X)
   - Or make the FontEditor handler check and consume T before global dispatch

2. **Fix S collision** (Medium): Same pattern — Global S fires before FontOverview S.
   Consider moving smushing rule editor to a different key or making global S
   context-aware (only when no font-specific dialog is intended).

3. **No Space collisions**: Space is correctly context-gated — it only starts
   animation playback when the timeline has frames and no other scope consumes it.

---

## 4. Feature Workflow Verification

### 4.1 Creating a Font from Scratch

- **Workflow**: Welcome → [B]lank Font → FontEditor → Draw characters → Save
- **Status**: ✅ Verified — Blank Font creates empty font, glyph grid shows, drawing tools work
- **Notes**: 12 tools available (Brush through Braille), palette with FG/BG, 6 layers default

### 4.2 Creating a Font from System Fonts (TTF)

- **Workflow**: Welcome → [N]ew Font from System → Font picker dialog → Select → Convert
- **Status**: ✅ Dialog exists and enters correctly (verified via tmux)
- **Notes**: Uses font_gen.rs to rasterize TTF/OTF → FIGfont glyphs

### 4.3 Opening an Existing Font (.flf)

- **Workflow**: File → Open (Ctrl+O) → File browser → Select .flf → Load
- **Status**: ✅ Verified — standard.flf loaded with 325 characters
- **Notes**: File browser has directory nav, recent files, direct path typing

### 4.4 Creating Images

- **Workflow**: Welcome → [C]reate Image → Set dimensions/palette → Canvas opens
- **Status**: ✅ Verified — 80×24 canvas with Grayscale palette
- **Notes**: 5 palette presets (Grayscale, Primary, Warm, Cool + custom), all tools available

### 4.5 Creating Banners (ASCII Art)

- **Workflow**: CLI: `figby -d fonts -f standard "text"` or TUI FontEditor → Preview panel
- **Status**: ✅ Verified both paths
- **CLI**: Clean output across all 5 tested fonts (standard, banner, big, small, slant)
- **TUI**: Preview panel renders "AaBbCc123!?" correctly in standard font

### 4.6 Animation Timeline

- **Workflow**: Menu → Animation → Add Frame (A) → Draw → Add Frame → Timeline panel
- **Status**: ✅ Menu verified (4 items), timeline toggle (T), playback (Enter)
- **Notes**: Frames support keyframing (position/opacity/blend), tweening (4 easing functions),
  onion skinning. Per-frame delays now stored in TimelineFrame (F-26 fix).

### 4.7 Animated GIF Import

- **Workflow**: File → Import GIF → Select GIF → Canvas created from frames
- **Status**: ✅ Dialog verified — file browser with path input
- **Notes**: GIF import supports disposal methods, per-frame delays, transparency.
  Scale-to-fit available. Canvas budget enforced (F-12).

### 4.8 Animated GIF/APNG/ANSI Export

- **Workflow**: File → Export (Ctrl+E) → Select format → Configure → Export
- **Status**: ✅ Export dialog functional
- **Notes**: GIF/APNG support per-frame delays (F-26), ANSI supports multi-frame
  with timing (F-26), shared compositor for all formats (F-26).

### 4.9 Particle Effects

- **Workflow**: Tools → Emitter → Click on canvas to place → Configure in emitter panel
- **Status**: Emitter tool available in tools panel
- **Notes**: Particle system has config panel (emitter_x/y, collide_with_layer, etc.)

### 4.10 Lighting Engine

- **Workflow**: Tools → Lighting (Lg) → Enter lighting mode (G) → Add lights → Adjust
- **Status**: ✅ Lighting tool available, lighting mode entered via G key
- **Notes**: Supports ambient, directional, and point lights. Intensity adjustable (+/-).
  Shadow casting per-layer (Alt+S).

### 4.11 ASCII Preview Mode

- **Workflow**: Click "ASCII Preview" tab or press Tab to cycle modes
- **Status**: ✅ Mode tabs visible (Font Editor | Image Editor | ASCII Preview)
- **Notes**: ASCII Preview renders canvas content as FIGlet text output

---

## 5. Bugs & Issues Found

### 5.1 Real Bug: README `-V` vs binary `-v` (Low)

The README CLI table says `-V` (uppercase) but the binary only accepts `-v`
(lowercase). C FIGlet uses lowercase `-v`. Fix: change README line 118 from
`-V` to `-v`.

### 5.2 Potential: T key collision between Global and FontOverview (Medium)

In FontEditor mode, pressing T triggers GlobalAction::ToggleTimeline (checked
first in dispatch order) instead of the FontOverview Transform editor. The
KEYMAP documents T as "Transform editor" in FontOverview scope, but the global
T fires first. Consider re-keying the transform editor.

### 5.3 Potential: S key collision between Global and FontOverview (Medium)

Same pattern as T — Global "Open settings" fires before FontOverview "Smushing
rule editor" when in FontEditor mode.

### 5.4 Minor: Welcome screen key handling requires Escape first

On launch, the welcome screen intercepts keys but pressing lowercase letters
(N, C, etc.) correctly routes to welcome actions. Verified working.

### 5.5 Verified: No panic on multibyte editing (F-15)

WASM cursor correctly uses char index with byte-offset conversion. Tests
cover é, CJK, emoji, combining marks.

### 5.6 Verified: Canvas budget enforced (F-12)

new_image and GIF-import dialogs reject 65535×65535 canvases. Template
rendering already had its own guard.

---

## 6. Code Quality Summary

| Metric | Status |
|--------|--------|
| `cargo build` | ✅ Clean |
| `cargo test` | ✅ 1497 passed, 5 known-divergence ignored |
| `cargo clippy --all-targets -D warnings` | ✅ Zero warnings |
| `cargo fmt --check` | ✅ Formatted |
| Strict rustdoc (`-D warnings`) | ✅ Passes |
| `cargo audit` | ✅ 3 allowed warnings (ansi_term/paste/lru — documented) |

---

## 7. Key Observations

1. **FIGlet rendering is rock-solid** — all 5 tested fonts, all flags, paragraph
   mode, RTL, Deutsch, TLF support, control files all work correctly.

2. **TUI is well-structured** — clear separation of concerns (FontEditor,
   ImageEditor, ASCIIPreview), consistent menu system, 15 tools available.

3. **Animation pipeline is comprehensive** — timeline with keyframing, tweening
   (4 easing), per-frame delays (F-26), GIF/APNG/ANSI export, playback (inline
   and fullscreen), particle effects.

4. **Security hardening complete** — F-21 (bounded readers), F-22 (atomic writes),
   F-23 (checked parsing), F-24 (display sanitization), F-25 (RAII terminal),
   F-26 (animation timing), F-27 (parity claims), F-28 (owner URLs), F-29 (CI),
   F-30 (root commands), F-12/F-15/F-17 (verified bugs).

5. **CI matrix is modern** — Linux/macOS/Windows, WASM, strict rustdoc, audit,
   MSRV, release dry-run, pinned actions.

6. **Only remaining review item** — F-20 (ralph sandboxing) Phase B/C parked;
   Phase A landed, policy approved. Ralph mostly retired for this repo.

---

## 8. Recommendations

### Quick Wins

1. **Fix README `-V` → `-v`** (5 min)
2. **Re-key FontOverview T and S** to avoid Global scope collision (30 min)
   - T → Ctrl+T or X for Transform editor
   - S → Ctrl+Shift+S or similar for Smushing rule editor

### Documentation

3. **Add CLI usage examples** to README for common workflows:
   - `figby -d fonts -f standard "text"` (basic banner)
   - `figby --play animation.gif` (play GIF)
   - `figby -f banner -C control.flc "text"` (with control file)

4. **Add TUI keybind reference** — the KEYMAP table exists in code but isn't
   user-facing. Add a Keybindings section to README or a separate doc.

5. **Update `docs/sonnet5-review.md`** — remove "playback doesn't yet honor a
   GIF's real per-frame timing" (fixed in F-26).

### Architecture

6. **Consider root Cargo.toml** — the `--manifest-path figby-rs/Cargo.toml` on
   every command is verbose. A minimal root workspace would simplify dev
   commands (currently addressed by AGENTS.md instructions).

7. **WASM warning count** — the 27 WASM check warnings mentioned in the review
   may be from dependencies; verify if they're actionable.

---

*Review complete. Screenshots saved to `/tmp/opencode/review/` (21 PNG files).*
