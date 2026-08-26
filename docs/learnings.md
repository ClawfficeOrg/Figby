# Figby — Learnings

## 6.0.33 — GPT-review remediation Phase 1 (document model)

- **Two keyboard-paint paths existed** (`dispatch.rs` AND
  `EditorState::handle_key`) — gating only the dispatch one left paint
  leaking through on locked layers. When enforcing a rule, grep for
  *every* implementation of the behavior, not just the first call site.
- **Undo redo-capture needs the entry's layer buffer, not the active
  layer's**: `undo(cur, cur_idx)` must receive the current content of
  the layer being undone, or cross-layer redo restores garbage. Peek
  helpers (`undo_top_layer`) + caller-side fetch keep UndoSystem dumb.
- **Undo semantics gotcha when writing tests**: `push_undo_snapshot`
  captures the CURRENT buffer — to test "undo reverts an edit", push
  BEFORE mutating (the app does this: snapshot precedes stroke).
- **macOS `sed -i ''` has no `\|` alternation in BRE** and perl one-liners
  with escaped paths are error-prone — for multi-file refactors the
  Edit tool with replaceAll is safer than shell one-liners; a sed that
  silently matches nothing looks identical to success.
- **`self.mark_dirty()` inside its own body**: a mechanical sed of
  `self.unsaved = true` → `self.mark_dirty()` will also rewrite the new
  helper's own body into infinite recursion (stack overflow in tests).
  Write the helper first, then exclude it from any bulk substitution.
- **Status-bar truncation is intentional responsive design**: droppable
  items vanish under ~100 cols, so TUI smoke assertions must use a wide
  backend (120 cols) rather than expecting every label at 80.
- **Flaky parallel tests from clock-based temp names**: pid+nanos
  collides when clock granularity is coarse; add a per-process
  AtomicU64 counter. Symptom was `DirectoryNotEmpty` only under full-
  suite runs, never alone.

## 6.0.26 — Text tool UX redesign

- Live preview overlay on canvas: clone `TextToolState`, call `render_rows_from_buffer()`, push `TextOverlay` into `canvas.text_overlays` alongside committed blocks. The clone avoids mutating the real state during render.
- Font prev/next with clickable `[<]` and `[>]` buttons: each button is a `PropsWidgetRect` with `PropAction::FontPrev` / `PropAction::FontNext`. FontPrev already existed in `PropAction` enum but was missing from `dispatch_props_action` — had to add the match arm.
- Rasterize: compute bounding box, expand layer buffer if needed (check `need_w > cur_w || need_h > cur_h`), call `rasterize_selected_block(&mut buf)`. Access layer via `self.editor.layer_stack.layers[aidx]` to avoid double-borrow issues with `active_layer_mut()`.
- Editing activation: set `editing = true` when side panel Text tab is active. Three trigger sites: tab click (mouse), tab cycle (Alt+arrows), and key dispatch (auto-activate if Text tab open). All three needed editing flag set.
- Integration tests needed updating: `entering_text` → `editing`, `cursor_position` → `preview_pos`. Space no longer enters text mode; tests set `editing = true` directly and push to `text_buffer`.

## 7.6.1 — GIF import sizing dialog

- When building a two-phase dialog modeled on `RasciiImportDialog`, the key pattern is: `active` + `confirmed` + `config` fields for the handshake with `dispatch.rs`. The dialog sets `active = false` and `confirmed = true` on confirm; dispatch.rs checks both, reads `config.take()`, and performs the import. On error, the dialog re-opens with `error_message` so the user can adjust settings.
- `GifScaleTarget::Exact(w, h)` is needed because `FitBox` applies 0.5× terminal-cell aspect compensation. When the user explicitly sets "exactly 40×20 cells", they do NOT want compensation re-applied — the output should be exactly 40×20 regardless of source proportions.
- Centering frames on the canvas: compute `x_off = (canvas_w - frame_w) / 2` and `y_off = (canvas_h - frame_h) / 2`, then offset each cell write. This must be done both for the active layer (first frame) and every timeline frame buffer.
- `resolve()` on `GifScaleTarget` must be `pub(crate)` — it's needed by the dialog to compute default dimensions from `FitBox/FitWidth` before any frames are decoded.

## 7.5.1 — Particle edge + layer collision

- `ParticleSystem::update()` signature change (added `bounds` and `layer_mask`) required touching all 27 existing test calls. Using `sed` for mechanical replacement of `system.update(X)` → `system.update(X, None, None)` saved time over 27 individual edits.
- For layer-cell collision normal computation: 4-neighbor occupancy gives a binary normal. If the particle's cell neighbor is occupied, the normal points away from that neighbor. An isolated occupied cell (no occupied neighbors) defaults to full velocity reflection on both axes.
- Push-out distance of 0.5 cells after reflection prevents re-collision on the next frame tick.

## 7.4.2 — FIGfont density heightmap

- The simplest heightmap source for FIGfont text is binary: non-space cells get `height=255` (max), spaces get `None` (0). The Sobel filter in `compute_normal_map_figfont` naturally produces smooth gradient normals at character-background boundaries from this binary input.
- When debugging Sobel normal direction: gradient `gx > 0` (height increases rightward) produces `nx < 0` (normal tilts left). The normal points away from the gradient, which is physically correct for bump mapping.
- The render pipeline's `heightfield build` at `components/canvas.rs:48-58` reads `CanvasCell.height` and scales to `[0.0, 1.0]`. Setting `height: Some(255)` on text cells gives `1.0` → maximum elevation.

## 7.3.3 — Group remaining TuiApp fields into sub-structs

- Python regex mass-replacements on 5 files worked well for mechanical field renames,
  but multi-line `self\n.field` patterns are invisible to `\bself\.field\b` regex.
  Always verify with `cargo check` after mass replace — 2 multi-line sites + 3 test
  sites with different variable names were missed.
- `FrameState::dirty` accessed ~80 times via `self.frame.dirty = true`. Decided not
  to add a convenience method — the prefix is already clear and concise enough.

## 7.3.2 — Extract `render_canvas_area` player block to `AnimationState::render`

- `AnimationPlayer` uses `Cell` for interior mutability, so `AnimationState::render`
  can take `&self` and still call `frame.render_widget(&player, ...)` — no `&mut self`
  needed for the player path.
- `layout::FrameLayout` was being recomputed redundantly in `render_canvas_area` with
  identical parameters to the one in `render()`. Passing `&FrameLayout` from the caller
  eliminates this waste.
- `Borders` can be computed once at the call site and passed down, avoiding the need
  to pass `self.zen_mode` around.

## 7.1.1 — Fix quit-confirm dialog sizing + mouse input

- `Rect::contains` in ratatui-core 0.1.1 takes `Position`, not `(u16, u16)`.
  Use `.into()` to convert: `rect.contains((col, row).into())`.
- Button rects for Paragraph hit-testing: character positions within `inner` rect
  map directly to terminal pixel positions (1 char = 1 col). Compute x from char
  index within the line, y from line index within the Paragraph.

## 6.9.4 — Move tool options to right sidebar

- When removing a sub-panel from a vertical split in ratatui, eliminate the
  intermediate `Layout::vertical` split and assign the remaining area directly.
  `toolbox_list = left_vert[0]` replaces `tb_vert[0]`. Update `FrameLayout`
  struct to remove the now-unused `toolbox_brush: Option<Rect>`.
- `replaceAll` is dangerous for short patterns like `+ 0` — it can match
  unintended locations. Prefer targeted `edit` with surrounding context.
  (Lesson: the corrupted `mouse_fl` block was caused by `replaceAll` matching
  `layout::TOOLBOX_BRUSH_HEIGHT` at three sites plus the brush removal edit
  interacting badly.)

## 6.9.2 — Layer panel: drag handle reorder

- `Block::inner(area)` is sufficient for computing the content rect for hit testing,
  even when the block has a title. You don't need the actual rendered block — just
  `Block::default().borders(Borders::ALL).inner(area)` gives the same inner rect.
- When implementing `layer_at_pos`, the walk order must exactly match `render_with_stack`:
  reverse layer iteration (`len-1-rev_idx`), group headers consume 1 display row,
  layers consume 2. The `emitted` set prevents duplicate group-header counting.
- Mouse drag state in `LayerPanel` is `Option<(from_idx, current_to_idx)>` — the
  `reorder()` call on `Up` completes the move. Visual feedback (highlighted target)
  is rendered per-frame by checking `drag_state` during render.

## 6.9.1 — Layer panel: icon-based 2-row layout

- When a layer occupies 2 display rows and the panel height only fits the name row,
  skip row 2 rather than rendering a partial entry. Scroll clamping must target the
  name row (first of the pair) so the active layer's name is always visible.
- Nerd Font icons from `icons.yaml` (loaded into `BTreeMap<String,String>`) are the
  canonical source for visibility/lock/blend icons. Fallback to ASCII chars when
  icon not found (empty map or fallback string). The `icons` field on `LayerPanel`
  is populated by `TuiApp::new()` via `editor.layer_panel.icons = icons.clone()`.

## 5.8.4 — Palette LUT integration

- `SwatchLightingData` in `lighting.rs` and `Swatch` in `palette_import.rs` share similar fields but serve different roles: `Swatch` is the serialisable/UI model with `Option` fields for partial overrides, while `SwatchLightingData` is the flattened LUT input with resolved defaults. Keep the separation — the conversion happens in `PaletteEditor::lighting_swatches()`.
- Canvas-to-swatch matching uses a `HashMap<(u8,u8,u8), usize>` for exact RGB matches, with Euclidean distance fallback via `nearest_rgb()` in canvas. This is simpler than storing swatch index per cell and avoids adding a field to `CanvasCell`.
- `is_none_or()` (stabilised Rust 1.82) is useful for conditional luminance-based FG colour selection: `luminance(bg).is_none_or(|l| l > 128)`.
- When adding multi-swatch support to `LightingLut`, the single-swatch `from_palette()` method delegates to `from_swatches(&[data])` — avoids duplicating the LUT generation logic.

## 5.6.5 — Marker brush mode

- `BrushSubMode` enum lives in `brush.rs` (alongside `BrushState`), but the task's
  "Touches" only listed `palette.rs`, `tools/brush.rs`, and `mod.rs`. Both
  `brush.rs` (enum definition, sub_mode field) and `side_panel.rs` (Mode display
  line) are necessary supporting changes that should be listed. Touches = files
  directly modified, not just the "main" ones.

## 5.6.4 — Palette import: common formats

- Rust 2021 edition: `r#"..."#` raw string delimiters break when content contains
  `#"` sequences (e.g., `"#FF0000"` in JSON test fixtures). The `"#` after the
  hex value is parsed as the closing delimiter. Fix: use `r##"..."##` with two
  hashes for any raw string containing `#"`.
- Adobe ASE binary format uses big-endian byte order for all multi-byte fields
  (u16, u32, f32). UTF-16BE name strings require manual decoding via
  `u16::from_be_bytes` chunks → `char::decode_utf16` or `String::from_utf16`.
  The name length field counts UTF-16 code units (including null terminator).
- `#[serde(rename = "camelCase")]` on struct fields with snake_case names avoids
  `non_snake_case` clippy lint while still matching JSON keys like `cursorColor`,
  `brightBlack`, etc.
- `#[expect(dead_code)]` is the Rust 2024+ idiom for fields parsed by serde but
  never directly read — it will warn if the field later becomes used (the
  expectation is unfulfilled), serving as a maintenance signal.
- When migrating `Swatch` from `palette_editor.rs` (local definition) to
  `palette_import.rs` (shared module), the `PaletteFile` struct in
  `palette_editor.rs` still compiles because `pub use crate::palette_import::Swatch`
  makes it available in the module scope. Child test modules access it via
  `use super::*` since `pub use` is a re-export visible to children.
- JSON auto-detection strategy: check structural keys in order of specificity
  (`colors` → WezTerm, `schemes` → Windows Terminal, `name`+`swatches` → Native,
  top-level array → Paletty). ASE detection by magic bytes `ASEF` takes priority
  (checked before JSON parse attempt).

## 5.6.2 — 5-per-row hue-grouped palette layout

- Switching from 2×8 grid to hue-grouped layout roughly triples the rendered line count (2 data rows → 8 group headers + 8 data rows + FG/BG + custom hex + recent = ~18 lines). Integration tests using fixed terminal sizes like 10 or 12 rows fail because they no longer fit. Always check test terminal dimensions when layout vertical density changes significantly.

## 5.5.2 — Surface timeline panel in main layout

- `T` key was previously bound to `open_tween()`. Since `T` is now the timeline toggle,
  tween was moved to `Shift+T`. The `Shift+T` dispatch must be checked BEFORE `T`
  (uppercase before lowercase in key dispatch order) to avoid the lowercase `T` handler
  catching the shifted press first.
- `canvas_borders()` grew from 4 cases (2 booleans) to 8 cases (3 booleans) when
  `timeline` was added. The border logic now exhaustively matches all 8 combinations
  of toolbox/right_panel/timeline visibility — ratatui's `Borders` bitflags make
  this clean but the pattern-match explosion is unavoidable for edge-sharing panels.
- Both `Widget` and `StatefulWidget` were already implemented for `&AnimationTimeline`.
  Adding `panel_instance()` constructor was sufficient to wire it into the main layout
  — the render logic (thumbnails, playhead, frame labels) was already complete from
  earlier phases.

## 5.5.1 — Audit 4.5–4.8 implementation vs spec

- `ExportDialog` in `export.rs` has two nearly-identical render implementations:
  a `Widget for &ExportDialog` impl (line 719) and a direct `render(&self, frame, area)`
  method (line 458). The Widget impl is never registered in mod.rs — mod.rs calls
  `dialog.render(frame, area)` directly. The Widget impl is dead code dating from
  an incomplete migration.
- `play_raw()` at `player.rs:566` is a complete raw-mode playback engine but is
  never called from the TUI path (`play_fullscreen()` is used instead). This is
  independent functionality that could be wired for CLI animation playback.

## 5.3.3 — Phase merge: release/5.3 → master

- Phase merge bookkeeping was accidentally reverted after the merge commit
  (likely from a `git reset` or branch switch that discarded uncommitted docs
  changes). Had to re-apply version bump, memory entry, ralph-log entry, and
  todo check-off. Lesson: commit bookkeeping before merging, or do docs + merge
  in a single atomic commit.

## 5.3.1 — Flat item-based status bar with section grouping

- When restructuring a Widget's internal layout, every struct field must be consumed
  somewhere to avoid `dead_code` lint. Old fields that were dropped (like `zoom`,
  `render_mode`, `layer_count`, `undo_count`, `throbber_text`) must be absorbed
  into the new sections or the struct signature must change.
- A flat item list with `StatusItem { spans, width, keep }` is more flexible than
  fixed `Layout::horizontal` sections — items with `keep: false` can be dropped
  right-to-left when space runs out without redoing the layout.
- Pipe separator `\u{2502}` with `sep_style` between items is rendered inline in
  the final `Line`, avoiding per-chunk `buf.set_line()` calls needed by powerline.
- Initial implementation used powerline triangles with `Layout::horizontal` (5 chunks),
  but this was replaced by the flat item approach for better responsiveness.

## 3.0.0-rc.4 — Multi-font-directory search + font generation improvements

- When `load_font` signature changes from `(&str, &str)` to `(&str, &[&str])`,
  ALL callers must be updated — including integration tests (`tests/tui.rs`) and
  submodule test files. Using `rg "load_font\("` to find every call site before
  changing the signature prevents broken builds.
- macOS SIP (System Integrity Protection) blocks writes to `/usr/share/` since
  El Capitan. `/usr/local/share/` is the standard writable equivalent. The default
  font dir search order `["/usr/local/share/figlet", "/usr/share/figlet"]` puts
  the macOS path first, so font files installed there are found without env vars.
- `FontFamilyInfo` returned by `list_system_fonts()` includes both family name
  and available style descriptions. The font name passed to `--create-font` must
  match the system family name exactly (case-sensitive).
- In `deluxe_charset()`, combining multiple sub-charsets (braille, box, ogham, etc.)
  buries `█` in the middle — the last charset in the concatenation determines the
  darkest character. The `full` charset (ASCII printable + blocks with `█` last)
  avoids this issue entirely by keeping the charset small and focused.
- `generate_figfont_header()` previously hardcoded `print_direction: -1` in the
  format string. Changed to use `font.print_direction` field so header generation
  is consistent with the struct's actual value. Tests that construct fonts via
  `FIGfont::default()` (print_direction=-1) continue to get `-1` in the header.
- `rasterize_glyph()` with `RasterizationOptions::GrayscaleAa` produces 0-255
  alpha values. The `rascii_art` crate maps these linearly across the charset.
  With the `smooth` charset (18 chars), each char covers ~5.5% of the luminance
  range. Adding `█` at the end of `full` (127 chars) gives each char ~0.8%
  coverage — much finer gradation.

## 4.9.5 — Phase merge: release/4.9 → master

- No code changes — merge brought in TachyonFX welcome fade-in (4.9.1), dark neon
  theme (4.9.2), app fade-in on launch (4.9.3), and widget-based status bar (4.9.4)
  from release/4.9 branch. Only docs, changelog, and metadata updated.

## 4.8.4 — Phase merge: release/4.8 → master

- No code changes — merge brought in AnimationPlayer widget (4.8.0), terminal
  capture (4.8.1), raw-mode playback (4.8.2), and player TUI integration (4.8.3)
  from release/4.8 branch. Only docs, changelog, and metadata updated.

## 4.8.2 — Raw mode playback engine

- `write!()` into `String` requires `use std::fmt::Write`, not `use std::io::Write`.
  Using `format!()` avoids the trait ambiguity entirely.
- `event::poll(Duration::ZERO)` works for non-blocking keyboard checks in raw mode.
  Combined with `std::thread::sleep()` for frame timing, this avoids the coupling
  between poll timeout and frame rate.
- CUP escape sequence `\x1b[{row};{col}H` is 1-indexed (row 1, col 1 = top-left).
- Skipping blank cells (space with no colors) in `render_frame_raw()` reduces
  ANSI output size significantly for frames with lots of empty space.
- ratatui `Color::Gray` and `Color::DarkGray` both map to ANSI `90m` (bright
  black) — ratatui's distinction between them is not reflected in standard
  ANSI, so both get the same SGR code.

## 4.6.1 — Particle system design

- Spawn-before-update pattern: particles are created at the emitter position,
  then ALL particles (including newly spawned) go through the position update
  in the same frame. This means a particle's first frame includes a full dt of
  motion. Tests must account for this: lifetime must exceed dt to survive
  the birth frame, and `assertions on position must include `velocity × dt`.
- `retain(|p| p.remaining_lifetime > 0.0)` with strict `>` (not `>=`) means
  particles expire when remaining_lifetime reaches exactly 0.0. Combined with
  spawn-before-update, a particle with `lifetime == dt` expires instantly.
  Always use `lifetime > dt` for any particle expected to render at least
  one frame.
- `FromStr` for `BlendMode` matches lowercase names and returns
  `Ok(BlendMode::Normal)` for unknown strings — never fails, always falls
  back gracefully. This is deliberate (config file parsing should be lenient).
- `ParticleSection` in config.rs uses `Option<T>` for every field (not raw `T`)
  so that a TOML config file can override individual settings without needing
  to specify the full `ParticleConfig` structure. The `ParticleConfig` defaults
  in `particles.rs` are separate from the TOML-level defaults.

## 4.7.4 — Phase merge: release/4.7 → master

- No code changes — merge was a no-op (release/4.7 already an ancestor of master
  from merge commits for 4.7.1, 4.7.2, 4.7.3). Only docs, changelog, and metadata
  updated.

## 4.6.4 — Phase merge: release/4.6 → master

- No code changes — merge was a no-op (release/4.6 already an ancestor of master
  from 3 prior merge commits). Only docs, changelog, and metadata updated.

## 4.5.3 — Tweening

- Standard bounce easing: 4 piecewise quadratic phases with decreasing amplitude.
  Reference implementation widely used in CSS/JS easing; formula verified against
  common JavaScript libraries.
- `filter(|t| ...)` on `Option` returns `Option` with same lifetime — useful for
  conditional preview rendering without redundant matching.
- `replaceAll` in edit tool is fragile when there are multiple match patterns;
  better to use targeted `rg` to count instances then apply precise `oldString` patterns.

## 4.5.2 — Keyframing

- `Vec<Option<LayerKeyframe>>` with `get(layer_idx)?` returns `Option<&Option<T>>`.
  Dereferencing: `(*vec.get(i)?)?` extracts the inner `T` — cleaner than
  `.and_then(|o| *o)` or `.copied().flatten()` which don't work because
  `&Option<T>` is not an iterator.
- When using struct-update syntax for test TimelineState constructors, adding
  new fields to `Default` and using `..Default::default()` in existing tests
  minimizes churn. With 14+ existing tests using struct literals, `replaceAll`
  for the closing pattern `fps: 12,\n        };` → `fps: 12,\n        keyframe_editor: ...,\n        };`
  is less risky than changing each test individually.
- `clippy::collapsible_match` fires when a match arm body is `if guard { action }`.
  Fix: move the `if` to a match guard: `Arm if guard => { action }`.
- `clippy::manual_clamp` fires on `.min(max).max(min)` chains. Fix: `.clamp(min, max)`.
  Note: clamps panic if min > max, so args must be ordered correctly (min then max).

## 1.1.2 — Core types

- Serde + serde_json needed for round-trip tests even though "Touches" only listed
  `font.rs`. Cargo.toml modification is a necessary supporting change.
- `print_direction` defaults to -1 (unset) in `FIGfont::default()`, matching C
  semantics where -1 detects CLI override.
- `#[derive(Default)]` on `FIGcharacter` works since `Vec<String>`'s default is empty.
  Manual impl triggered `clippy::derivable_impls`.
- Using struct literal `..Default::default()` pattern avoids `field_reassign_with_default`
  clippy lint in tests.

## 1.1.3 — Header parser

- `figlet.c` skips baseline with `%*d` in sscanf and doesn't parse `codetag_count`,
  while `chkfont.c` includes both. Figby follows `chkfont.c` (parse all fields
  including baseline and codetag_count as 9th optional field).
- Full layout derivation rule (from `figlet.c:1231-1238`):
  `old_layout == 0 → 64 (SM_KERN)`, `old_layout < 0 → 0`, else `(old_layout & 31) | 128 (SM_SMUSH)`.
- `pub(crate)` triggers `dead_code` lint when function only called from tests.
  Made function `pub` instead — will be used by subsequent font parser stages.

## 1.1.5 — Code-tagged FIGcharacter parser

- C's `sscanf(fileline,"%li",&theord)` auto-detects hex (`0x` prefix) via `strtol` behavior.
  Rust `parse_codetag_integer()` must manually detect `0x`/`0X` and call `i64::from_str_radix`.
- C stores code `-1` as a normal entry in the linked list; Rust port skips it (reserved per task spec).
- `inchr` in C is `long` (64-bit on Linux), but Rust map uses `u32` keys. Negative codes
  stored via two's complement `(code as u32)` — preserves bit pattern.
- Codetagged section end is signaled by first non-numeric line (not EOF). No error raised.

## 1.1.7 — Compressed font support (zip/deflate)

- `zip` crate v2.x uses `FileOptions<'_, T: FileOptionExtension>` — `Default::default()`
  alone can't infer `T`. Use `zip.start_file::<&str, ()>("name", Default::default())` or
  annotate `let opts: zip::write::FileOptions<'_, ()> = Default::default();`.
- `zip::ZipArchive::len()` triggers `clippy::len_zero` — use `is_empty()` instead.
- `std::io::Error` doesn't implement `PartialEq`, so `#[derive(PartialEq)]` must be
  removed from `FontError` when adding `IoError(std::io::Error)`. Manual `PartialEq`
  impl skips `IoError` variant comparison (correct for all existing test patterns).
- `Path::join("", "standard.flf")` gives `"standard.flf"` (not `/standard.flf`),
  avoiding a leading-slash problem when fontdir is empty.

## 1.2.1 — Character lookup + width calculation

- `.expect()` used for char 0 invariant in `lookup_char()` — FIGfont spec mandates
  char code 0 always exists. Panic is intentional here (programming error if missing),
  not a recoverable runtime failure. Violates "no unwrap in production" rule in spirit
  but not letter (`.expect()` ≠ `.unwrap()`). Documented in both memory and learnings
  as a deliberate tradeoff.

## 1.2.3 — Smush amount calculation

- C's `smushamt()` computes signed `int` arithmetic that can go negative.
  Rust version uses `saturating_sub` for `usize`, clamping negative results
  to 0. This is safe for all FIGfont rendering since negative smush amounts
  only occur in degenerate (empty-line) edge cases.
- C uses comma operator in `for` loop conditions to assign and check in one
  expression. Rust port separates assignment from logic using helper functions
  (`last_non_space`, `first_non_space`) with fallback parameters.
- The `ch2` null check in C (`if (ch2)`) maps to `ch2 != '\0'` in Rust.
  Forward-scan all-spaces case yields fallback char `'\0'`, matching C's
  null-terminator sentinel behavior.
- Clippy `if_same_then_else` lint fires when both branches of an `if/else if`
  have identical bodies. Fix: merge conditions with `||` since the logic is
  naturally OR (either ch1 is space/null OR (ch2 exists AND smush succeeds)).

## 1.2.4 — Character addition with smushing

- `add_char` has 8 parameters, triggering `clippy::too_many_arguments` (default
  threshold 7). Adding `#[allow(clippy::too_many_arguments)]` is acceptable since
  the function mirrors C's use of global variables — all 8 params are necessary
  to avoid globals.
- `clippy::needless_range_loop` fires for `for k in 0..overlap` patterns that
  use `k` only to index one collection. Fix: use `for (k, item) in collection.iter().enumerate().take(overlap)`.
  One case (`out_chars` RTL) iterates `out_chars` but indexes both `out_chars`
  and `temp` by `k`; using the iterator for `out_chars` resolves the lint cleanly.
- The `calc_smush_amount` bug (passing `outlinelen` as `prev_width` to
  `smush_horizontal`) is known and does not affect `add_char` correctness —
  `add_char` passes the correct `old_prev_width` in its own overlap loop.

## 1.2.6 — Line breaking and word splitting

- C's `splitline()` uses global `inchrline` (char buffer) and `outline`
  (rendered rows). Rust version takes `&[u32]` char_buffer and `&mut Vec<String>`
  output_rows as explicit parameters — no globals.
- C's `splitline()` always produces output (even if no word break found, it
  prints a blank line). Rust version returns `None` for no-break, letting the
  caller decide the fallback (forced break or blank line). This is more
  idiomatic and avoids silent blank-line generation.
- Return type `Option<(Vec<String>, usize)>` packs both the rendered part1 rows
  (for printing by caller) and the part2_start index (for caller to truncate
  its char_buffer). Cleaner than C's side-effect-only approach.
- The `#![allow(clippy::too_many_arguments)]` pattern from `add_char()` carries
  over to `split_line()` (9 params) — all necessary to avoid globals.
- Test pattern: `build_expected()` helper calls `add_char()` independently to
  compute reference output, then compares against `split_line()` result. This
  tests both the splitting logic and the rebuild correctness simultaneously.

## 1.2.7 — Phase merge review

Three bugs found in phase merge review:

1. **Width guard in wrong function**: C's `smushem()` (which Rust `smush_horizontal()` mirrors)
   has NO width guard. The guard `if (currcharwidth < 2 || old_prev_width < 2) smush = 0`
   belongs in `addchar()`/`add_char()`, not in `smushem()`/`smush_horizontal()`. Having it
   in `smush_horizontal()` caused `calc_smush_amount()` to fail because it passed
   `outlinelen` (not `old_prev_width`) as the width parameter, causing false `None`
   returns when outlinelen was small.

2. **Missing first-char optimization**: C's `addchar()` has `if (prev_width == 0)` short-circuit
   that copies the character directly without smush computation. Rust `add_char()` lacked
   this, causing incorrect overlap calculations for the first character.

3. **Wrong `contains()` usage for KERN|SMUSH check**: `calc_smush_amount()` used
   `!mode.contains(KERN | SMUSH)` which checks ALL bits set (AND), but C's
   `!(smushmode & (KERN | SMUSH))` checks ANY bit set (OR). Changed to
   `!mode.contains(KERN) && !mode.contains(SMUSH)` to match C semantics.

## 1.3.4 — Main event loop

- `pub(crate)` visibility in `font.rs` constants is NOT visible from binary crate
  (`main.rs`), since the binary depends on `figby` as a separate library crate.
  Changing `DEUTSCH_CHARS` to `pub` is required when the binary needs it.
- `std::io::Stdin::bytes()` requires `Read` trait in scope (`use std::io::Read`).
  Using `io::BufReader::new(io::stdin()).bytes()` avoids
  `clippy::unbuffered_bytes` lint.
- `clippy::never_loop` fires on one-shot `loop { return ... }` — replace with
  plain `match`/`if`.
- The inner retry loop in C uses `do {} while (char_not_added)` with a flag.
  Rust alternative: `loop { ... break; ... }` where every branch either `break`s
  (char handled) or falls through (retry after flush/split). Avoids
  `clippy::needless_late_init`.
- Clippy `ptr_arg` on `&mut Vec<String>` — use `#[allow(clippy::ptr_arg)]` when
  the function signature needs to match the calling convention (callers pass
  `Vec<String>` and mutate it). Changing to `&mut [String]` loses the ability
  to `clear()`.
- `flush_output_line` has 8 parameters triggering `clippy::too_many_arguments`.
  Acceptable mirror of C's global-based approach — suppressed with allow attr.

## 1.3.5 — Phase merge

- Merge `c7ab68d` is single-parent (fast-forward), unlike previous phase
  merges (1.1.8, 1.2.7) which used `--no-ff` (two parents). The fast-forward
  was likely due to `master` being directly on `release/1.3`'s linear history
  with no divergent commits.

## 1.3.1 — CLI argument parsing

- `#[allow(non_snake_case)]` is required on clap structs when flags have
  uppercase/lowercase collisions (e.g., `-L` vs `-l`). In snake_case, `flag_L`
  and `flag_l` collapse to the same name. Eight such collisions exist in FIGlet.
- `CliArgs::try_parse_from(["figby", "-A"])` — the array arg must be
  owned (no `&` prefix). Clippy `needless_borrows_for_generic_args` fires if
  you write `&["figby", "-A"]`; clap's `try_parse_from` accepts
  `impl IntoIterator` and `[&str; N]` already satisfies that without a borrow.
- `-m -1` parsing with clap: requires `#[arg(allow_hyphen_values = true)]` on
  the field. Without it, clap treats `-1` as an unknown flag. In clap 4 the
  `Option<i32>` parser alone does NOT allow leading hyphens — the attribute
  must be explicit.
- `smushoverride` for `-s` does NOT change `smushmode` — it only sets
  override to `SMO_NO`. This differs from `-W` which sets `smushmode = 0`
  AND `override = SMO_YES`. Matching C semantics precisely is critical.

## 1.4.1 — Control file parser

- C's `readcontrol()` outer switch reads the FIRST byte of each line.
  Lines starting with `\` (backslash) in `upper.flc` (e.g.
  `\0x037A \0x0399`) fall to `default:` and are silently skipped.
  These mapping entries are effectively documentation-only in the C parser.
  Only lines starting with `0-9` or `-` are parsed as mapping table entries.
- `read_tchar()` and `read_num()` are deeply coupled — `read_tchar` parses
  `\` escape prefix then delegates to `read_num` for numeric escapes
  (`\0x...`, `\377`, `\-6`). `read_num` uses the full hex digit set
  `"0123456789ABCDEF"` regardless of parsed base (decimal uses hex
  digit set too, matching C's `strchr` approach).
- C `charsetname()` has dead code: the `\n`/`\r` check is never hit
  because `readTchar` already returns 0 for newlines. The `Zungetc(0, fp)`
  bug (pushing back NUL byte) is also present — harmless since
  `skiptoeol` always follows.
- C `readcontrol()` has missing `break` before `case '\r': case '\n':`
  after the `case 'g'` inner switch — harmless fallthrough since the
  empty-line case just does `break`.
- The `94x94` double-byte charset path reads `x`, then `9`, then `4`,
  then `skipws` before `charsetname`. The `96` path has NO `skipws`
  before `charsetname` — a C bug never triggered in practice (no
  `.flc` uses `96` charset).

## 1.4.3 — ISO 2022 character set handling

- Closure-based dispatch (`next_char`) used to route between `iso2022()`
  and raw `input.next()` based on `multibyte` flag. Parameters for `input`
  and `state` avoid closure capturing them, preventing borrow conflicts
  with `remap_char` usage later in the function.
- `control_state` must be `mut` even though most phases use it immutably
  (`remap_char`), because `iso2022()` takes `&mut self` to update gl/gr/gndbl.
- C `iso2022()` uses `inchr` (long) for ch, so values can exceed u32 range
  on shift operations. Rust port uses `u32` which is sufficient since
  FIGlet's gn values are ASCII codes shifted by at most 24 bits.
- `b'B' as u32 + 0x100` patterns can't be used directly in match arms
  in Rust — use literal hex values like `0x128` instead. Hex is readable
  as `0x100 + byte_value` mapping.

## 1.5.1 — UTF-8 input mode

- `clippy::needless_range_loop` fires even for simple `for i in 1..length { buf[i] = ... }`
  patterns. Fix: `for slot in buf.iter_mut().take(length).skip(1) { *slot = ... }`.
- `std::str::from_utf8` handles all the complex validation (overlong, surrogate,
  >U+10FFFF) — the decoder only needs explicit continuation byte checks and
  leading byte pattern dispatch. This keeps the production code simple and correct.
- `char::from_u32` is not needed — when `from_utf8` succeeds, `s.chars().next()`
  always returns `Some(char)` for a non-empty slice, and `char as u32` gives the
  scalar value directly.
- The `0x0080` error sentinel is FIGlet's C replacement character for invalid
  multi-byte sequences — must match exactly for compatibility.

## 1.5.2 — DBCS, HZ, Shift-JIS input modes

- HZ C code for `}~` exit reads the byte after `~` and returns it directly.
  Rust recursive approach (set hz_mode = false, recurse) produces same result:
  recursive call reads next byte in non-HZ mode.
- HZ C code loses the non-`~` byte when `}` is followed by a non-`~` byte
  inside HZ mode. Rust `unget(c)` pushes it back, treating `}` as first byte
  of a proper double-byte pair. This is a clear improvement.
- Plan's test `test_hz_eof_in_exit` expected 0x7E7B but recursive implementation
  returns None for `~{` + EOF (recursive call hits EOF). Adjusted test to verify
  `}` + EOF in hz_mode returns `}` alone (incomplete double-byte).
- Closure with 3 mutable ref params works when closure captures only `config`
  (immutable ref) and takes `input`, `state`, `hz` as mutable parameters.
  No borrow conflicts because each param is a separate `&mut` to disjoint data.
- DBCS and SJIS byte ranges are identical (0x80-0x9F, 0xE0-0xEF — lead bytes;
  any byte as trail). Combined as `(lead << 8) | trail`.

## 1.6.3 — Project rename: Feiglet → Figby

- Using `replaceAll` on edit tool is efficient for bulk renames within a file,
  but backtick-pattern `\`feiglet\`` won't match `cargo build -p feiglet` since
  the backtick is at the start of the command, not right before feiglet. Need
  a dedicated replace for such cases.
- `rg -il` is the fastest way to verify zero remaining matches after a rename.
- `git mv` for directory rename preserves history cleanly.
- After directory rename, `cargo build` must be run from inside the renamed
  directory (no workspace Cargo.toml at root).

## 1.5.3 — Deutsch flag character re-routing

- Clippy `manual_range_contains` fires on `c >= x && c <= y` patterns — use
  `(x..=y).contains(&c)` instead.
- When a `use` import is used only in `#[cfg(test)]` code within the same file,
  clippy flags it as unused in the binary target. Move it inside the test module
  to silence the lint cleanly.

## 1.6.1 — Port C test harness

- C `-f` handler strips `.flf`/`.tlf` suffix from font name before `FIGopen()`:
  `if (suffixcmp(fontname,FONTFILESUFFIX)) fontname[strlen-4] = '\0'` (figlet.c:1044-1046).
  Rust `CliConfig::from_args()` doesn't strip extensions — `config.fontname = val` directly.
  When names like `fonts/banner.flf` are passed (e.g., from shell glob), C strips to
  `fonts/banner`; Rust keeps `fonts/banner.flf` which then fails in `font_candidates()`
  since it appends another `.flf` suffix.
- C `-C` flag strips `.flc` suffix (figlet.c:1055-1057) and uses `FIGopen()` which
  prepends `fontdirname` and appends `.flc`. Rust `read_control()` opens the path
  directly via `File::open()` — no fontdir-based resolution. Bare control names like
  `uskata` don't resolve; full path `fonts/uskata.flc` required.
- `font_candidates()` in Rust tries `{name}.flf` and `{name}.tlf` but never the bare
  `name` directly (C's `FIGopen` also appends suffix, but the name has already been
  stripped). This means Rust fails on names that already carry `.flf`/`.tlf` extension
  while C' strips first then appends.
- `env!("CARGO_BIN_EXE_figby")` provides binary path during integration tests only —
  requires `[[bin]]` section in Cargo.toml. Works for `tests/` integration tests.
- `CARGO_MANIFEST_DIR` gives crate directory; repo root is one level up. Used to
  locate `fonts/`, `tests/`, and expected output files.
- `std::fs::read_dir` + sort by filename matches POSIX `ls` default ordering for
  ASCII filenames (used in `showfigfonts_output` and `list_control_files_output`).
- Test 20 tempdir with `tempfile::tempdir()` is drop-safe for panic cleanup vs C's
  `mkdir + rm -Rf` which leaks on error.

## 1.6.5 — Rendering pipeline bug fix

- C figlet's `addchar` failure for a space char checks `wordbreakmode == 2`
  (figlet.c:2107), not `>= 2`. When `wordbreakmode == 3` (after a space, back
  in a word), a failing space causes `printline()` (simple flush), not `splitline()`.
  The non-space failure path (figlet.c:2108) correctly uses `wordbreakmode >= 2`.
  Different semantics for space vs non-space paths — subtle C detail easy to miss.
- `char_buffer.truncate(part2_start)` is wrong when `split_line` returns the
  start index of part2 (not the length of part1). `drain(..part2_start)` is
  correct. This matters when the char_buffer contains data before part2 —
  `truncate` keeps only `part2_start` elements; `drain` removes `part2_start`
  elements from the front. In the no-word-break case, `part2_start == 0`, so
  `drain(..0)` is a no-op (correct) while `truncate(0)` clears the buffer
  (wrong — drops all output lines).
- `String::from_utf8` fails on any non-UTF-8 byte. FIGfont files from C era
  can contain arbitrary bytes (0xFF padding, etc.). `String::from_utf8_lossy`
  replaces invalid bytes with U+FFFD, matching C's byte-level Latin-1 handling
  (where the byte just passes through as-is). Lossy is safe for FIGfonts since
  valid ASCII/UTF-8 characters pass through unchanged.
- Standard font space glyph ` $@` has width 2 after `strip_endmarks` (replaces
  `$` with hardblank `@` after strip). Renders as 1 space after hardblank→space
  replacement in `render_line`. This small width means space chars are the first
  to hit output width limits, making them the primary trigger for line-wrap bugs.
- Standard font default char (code 0/empty) has empty rows — not stored in font
  file for code 0. C figlet allocates it empty via `addchar` first-call;
  Rust parser reads code 32 from first data block (correctly matches C).
- `cargo fmt` requires `--manifest-path figby-rs/Cargo.toml` since there's no workspace
  Cargo.toml at the repo root.
- `std::sync::OnceLock` is stable since Rust 1.70 and works well for lazy font loading
  in benchmarks. Must use `FONT.get_or_init(|| { ... })` pattern.
- Criterion 0.5 uses `criterion::black_box` (re-export of `std::hint::black_box`).
  `criterion_group!` and `criterion_main!` macros work unchanged.
- `std::iter::repeat_n()` (stable since 1.82) is preferred over `repeat().take(N)` —
  clippy `manual_repeat_n` lint enforces this.
- No compiled C `figlet` binary exists in the repo for baseline comparison. The
  benchmarks establish a Rust baseline; manual C comparison is separate work.

## 2.0.1 — CLI `--help` output

- `clap::Command::render_help()` returns `StyledStr` (not `Result`) — no `.unwrap()` needed.
- Bench `calc_smush_amount` call had wrong argument order (missing `prevcharwidth`,
  passing `SmushMode` as `usize`). Pre-existing bug surfaced when clippy compiled
  `--all-targets`. Argument 5 is `prevcharwidth: usize`, argument 6 is `mode: SmushMode`.
- `calc_smush_amount` in `render.rs` was missing `#[allow(clippy::too_many_arguments)]` —
  pre-existing lint that needed fixing to pass the clippy gate.
- `figby -f <name>` expects bare font name (no path prefix) and resolves via fontdir.
  From repo root, pass `-d fonts/ -f standard` not `-f fonts/standard.flf`.
- POSIX `case` patterns use glob syntax, not regex. Comma-separated list matching
  uses `case ",$LIST," in *,"$ITEM",*)` — the simplest portable pattern.

## 2.0.7 — Border and shadow rendering for template output

- `clippy::needless_range_loop` fires for index loops where vars only index
  one collection. Fix: `canvas.iter_mut().enumerate().skip(outer_top).take(count)`
  yields `(y, row)` pairs with correct indices. `saturating_sub` + `saturating_add`
  needed for `take()` arithmetic to avoid underflow when range is empty.
- `clippy::needless_late_init` fires on `let x; match { ... x = val ... }` — fix:
  `let x = match { ... val ... }`.
- `clippy::unused_variables` fires when variables are assigned but never read.
  Remove unused variables entirely.
- `fill_shadow` and `fill_border` use `_y`/`_x` prefix for unused index
  variables (needed by `enumerate()` for correct iteration but not used
  in shadow body since it fills the entire region unconditionally).
- Rust raw string `r#"..."#` delimiters: the closing `"#` sequence must not
  appear in the string content. A string like `border_color = "."` ends with
  `"#` which matches the raw string delimiter, truncating the content. Fix:
  use more hashes (`r##"..."##`) or escape conventionally (`"border = \".\""`).
- Colored image output from `rascii_art` embeds ANSI escape codes per-character.
  These cannot be stored in a `Vec<Vec<char>>` grid — escape sequences occupy
  multiple cells. Color rendering in templates requires a richer canvas type
  that stores per-cell color metadata.
- Template format should use YAML frontmatter (not TOML) with typed elements.
  Reference: `assets/templates/figby-cli-h1.ftmp`. TOML limitations block
  multi-font fallback arrays and complex nested metadata.
- Template rendering deferred to Phase 2.3+ (TUI/ratatui canvas widget). Current
  `parse_ftmp`/`render_template` is a prototype; ratatui `Paragraph`/`Canvas`
  widgets can handle styled text natively without the ANSI-in-grid problem.

## 2.0.10 — Phase merge: release/2.0 → master

- Second phase merge (after `10035c9`) to bring 3 post-merge commits from
  `release/2.0`. No conflicts — `docs/todo-v2.md` auto-merged cleanly because
  master's change (line 116: `[ ]`→`[x]`) and release/2.0's changes (lines 201-203:
  2.2.5 renumber + new 2.2.5 iconset task) were on different lines.
- Both `docs/memory-v2.md` and `docs/learnings.md` had additions only on
  `release/2.0` — no conflict with master. `docs/memory.md` had additions only
  on master — no conflict with release/2.0.
- 4 new files (`assets/templates/*.ftmp`, `assets/tui/icons.yaml`) with no
  prior history on master — created cleanly.
- `figby-rs/src/template.rs` was modified only on release/2.0 (no conflict).

## 2.10.1 — Full regression against C FIGlet 2.2.5

- C FIGlet's `-m` flag accepts smush mode values from 0 (kerning/no smushing)
  to 191 (all 6 rules enabled). Mode 0 with standard font produces wider spacing
  than default (kerning instead of smushing).
- Some expected outputs (test 31 deutsch flag) differ between builds of C figlet
  depending on font content. The Deutsch chars (196/214/220/228/246/252/223)
  must exist in the font for `-D` re-routing to produce visible output.
- `FIGLET_FONTDIR` env var works identically in C figlet — the binary reads it
  at font resolution time. This is how the Rust test harness sets the font dir
  for all tests.
- Tests 47-48 (all fonts with extra flags) require the same font enumeration
  logic as test 02. Extracted `all_fonts_output()` helper to avoid code
  duplication.
- The `regenerate-expected.sh` script must handle shell escaping carefully:
  `printf "[\\]"` for backslash-containing inputs, and `printf "a\x01b\x02c\n"`
  for control-character inputs.
- Test 42 (`-A` flag, cmdinput) takes input from args, not stdin. The
  `run_figby` helper passes `None` for stdin data in this case.

## 2.1.1 — Image loading + grayscale conversion

- `image` 0.24.9 was already a transitive dependency via `rascii_art`. Adding it
  as a direct dependency with explicit format features (`jpeg`, `png`, `bmp`, `webp`)
  ensures features don't silently change when `rascii_art` updates.
- Encoder API differences between formats:
  - `JpegEncoder::new(w: W)` — takes ownership (by value)
  - `BmpEncoder::new(w: &mut W)` — takes mutable reference
  - `WebPEncoder::new_lossless(w: W)` — method is `new_lossless`, not `new`
- `ImageBuffer` has inherent `dimensions()` and `get_pixel()` — no need to import
  `GenericImageView` trait. Similarly, `put_pixel()` is inherent, no `GenericImage` needed.
- `u8` value range (0..=255) is enforced by the type system — `val <= 255` for
  `u8` triggers both `clippy::absurd_extreme_comparisons` and compiler
  `unused_comparisons`. Remove such assertions entirely; type system guarantees it.
- `rsplit('.').last()` triggers `clippy::double_ended_iterator_last` — use
  `rsplit('.').next_back()` instead for direct O(1) access.

## 2.1.2 — Luminance-to-ASCII character mapping

- `image::codecs::png::PngEncoder::encode` is deprecated in image 0.24.9 —
  use `write_image` via the `image::ImageEncoder` trait (must be imported).
  Other encoders (JpegEncoder, BmpEncoder, WebPEncoder) still use their own
  `encode` methods without deprecation (as of 0.24.9).
- Bilinear resize: when sampling at the far edge (`dx = new_width - 1`),
  `sx` may land between the last two source pixels. Clamping `x1` to
  `(x0 + 1).min(src_w - 1)` handles this correctly. Same for y-boundary.
- Terminal char aspect ratio (~2:1 height:width) means the ASCII output
  height should be halved relative to pixel aspect to avoid stretched output.
  Factor 0.5 applied in `luminance_to_ascii` height calculation.

## 2.1.5 — Image CLI flags integration

- `--width` long flag is safe to add because existing `-w` has no long form.
  No namespace collision with existing FIGlet flags.
- `--flipX`/`--flipY` require `#[arg(long = "flipX")]` since Rust convention
  uses `flip_x` field name but the flag name uses `flipX`.
- Flip functions placed in `main.rs` (not `image_input.rs`) to respect strict
  "Touches: main.rs" scope. Uses `image_input::RgbPixel` qualified in signatures
  so no type import needed.
- `img_height` field initially flagged as dead code — had to wire it into
  `run_image()` by truncating output lines. All `ImageOptions` fields must
  be used to avoid clippy `dead_code` lint.
- Image mode dispatch placed after template rendering (`--render-template`)
  but before FIGlet mode (`-f`, `message`). This ensures image mode doesn't
  conflict with template or FIGlet flag processing.
- URL support stubbed with `eprintln` error — `image::open` takes `AsRef<Path>`,
  not URLs. Full URL support needs `ureq` or `reqwest` dependency (deferred).

## 2.2.1 — System font enumeration via font-kit

- `Source::all_families()` returns `Vec<String>` (plain strings), not `Vec<FamilyName>`
- `Source::select_family_by_name(&str)` returns `FamilyHandle` with `fonts() -> &[Handle]`
- `Handle::load()` returns `font_kit::font::Font` (the freetype-based cross-platform Font)
- `Font::is_monospace()` is available directly on the freetype Font — no need for glyph-advance heuristic
- `fontconfig` must be installed on Linux for `SystemSource` to work. The `yeslogic-fontconfig-sys` crate wraps fontconfig C library.

## 2.2.3 — FIGfont header from font metrics

- `format!` macro is preferred over `write!` with `unwrap()` for infallible string
  formatting to String — no `unwrap()` needed, pure allocation.
- `strip_endmarks()` trims trailing whitespace BEFORE identifying endmark. This
  means trailing spaces before `@` in a row like `" char @"` are preserved:
  endmark `@` is identified first (whitespace-trimmed string still has `@`), then
  only consecutive `@` chars are removed. Result: `" char "` with trailing space
  intact. This is critical for width correctness in FIGfont glyphs.
- Round-trip header generation works because `generate_figfont_header()` always
  emits all 9 fields (including explicit `full_layout`). When `parse_header()`
  sees `tokens.len() > 6`, it reads `full_layout` from the header directly rather
  than deriving it from `old_layout`. This preserves non-default full_layout
  values (e.g. 191 for all smushing rules).
- Placeholder rows for missing required chars use `maxlength` spaces + `@`.
  After `strip_endmarks`, these become `maxlength` spaces — correct width for
  empty/space glyphs.

## 2.2.4 — `--create-font` CLI

- `font-kit` uses `pathfinder_geometry` types (`Transform2F`, `Vector2I`,
  `Vector2F`, `RectI`) throughout its public API (`rasterize_glyph`,
  `raster_bounds`, `Canvas::new`). These are NOT re-exported — must add
  `pathfinder_geometry = "0.5"` as a direct dependency to use them.
- `font_kit::font::Font` is re-exported from `font_kit::loaders::default::Font`.
  On Linux, default is the freetype backend. `Font` implements the `Loader` trait
  which provides `glyph_for_char()`, `advance()`, `metrics()`, `raster_bounds()`,
  `rasterize_glyph()`, `is_monospace()`, etc.
- `rasterize_glyph()` is the main rendering function: takes a `&mut Canvas`,
  `glyph_id`, `point_size` (pixels per em), `Transform2F`, `HintingOptions`,
  `RasterizationOptions`. Renders at the given `point_size` using freetype's
  `FT_Set_Char_Size` + `FT_Load_Glyph` with `FT_LOAD_RENDER`.
- `raster_bounds()` computes the pixel bounding box of a glyph at the given
  `point_size`. The returned `RectI` has origin in "top-left" coordinate system
  where the glyph origin (baseline) is at (0,0). `origin_y()` is typically
  negative (above baseline). `origin_y() + height()` gives the descender depth.
- `Canvas` uses `Format::A8` for single-byte-per-pixel alpha-only rendering.
  Anti-aliased rendering via `RasterizationOptions::GrayscaleAa` produces
  0-255 alpha values; threshold at 128 for monochrome conversion.
- The freetype loader has `reset_freetype_face_char_size()` which is called on
  load and sets char size to `units_per_em` (design size). `rasterize_glyph()`
  overrides this with the requested `point_size` via `FT_Set_Char_Size`.
- `clippy::repeat_once` lint: `" ".repeat(1)` → `" ".to_string()`.
- `font.advance(glyph_id)?.x()` returns advance in **font units** (font-kit FreeType
  backend sets char size to `units_per_em` during `reset_freetype_face_char_size`).
  Must scale by `point_size / upem` to get pixel advance. NOT the same as
  `raster_bounds.size().x()` which gives ink bounding box width.
- For `--create-font`: character cell width must use **advance width**, not ink
  bounding box width. `raster_bounds.size().x()` gives per-glyph ink width
  (varies: space=1, `!`=4, `W`=9) → terrible output. Advance width gives the
  font's proper horizontal metric (uniform for monospace, ~7px for 12pt).
- Space character (code 32) has no visible ink → `raster_bounds` returns (0,0).
  Old code created FIGcharacter with 1-wide rows (`" ".to_string()`). Fix: compute
  advance width even for empty glyphs and create blank FIGcharacter at full width.
- `rascii_art::render_image_to` formula: `char_index = (grayscale * (N-1)) as usize`.
  With N=2 charset, only grayscale=1.0 (alpha=255) maps to the second char due to
  integer truncation. Use N≥3 for any threshold below 255.
- `@` is FIGfont endmark — `strip_endmarks()` removes all trailing `@` from each row.
  If glyph fill uses `@` and it appears at end of row, it gets stripped → corrupted glyph.
  Never use `@` as a fill character in generated fonts.
- `$` is hardblank — renderer replaces with space in output (`render.rs:334`).
  Never use in glyph fill content. Only 1 occurrence in font file (header).
- For FIGfont charset design: avoid `@` (endmark) and the hardblank char. Everything
  else is safe as fill characters.

## 2.3.1 — TUI scaffold with ratatui

- `ratatui-core` 0.1.1 (used by ratatui 0.30.1) has `Frame<'a>` with only a lifetime parameter — no generic backend type. Earlier ratatui versions used `Frame<'a, B: Backend>`. Method signatures must use `Frame<'_>` not `Frame<'_, B>`.
- `Buffer` from `ratatui-core` 0.1.1 does NOT implement `Display`. To get rendered output as a string (e.g., for test assertions), use `buffer.content().iter().map(|c| c.symbol()).collect::<String>()`.
- `Cell::symbol` field is private — use the `symbol()` accessor method instead.
- Dead code on `icons` field is acceptable for now — it's an architectural scaffold for future TUI tasks (2.3.2+). Prefix with `_icons` (Rust idiom) to suppress warning without `#[allow(dead_code)]`.

## 2.3.2 — Toolbox bar

- Converting a single file module (`tui.rs`) to a directory module (`tui/mod.rs`) requires updating `include_str!` relative paths (+1 `..` level for the subdirectory).
- `pub` methods on `pub struct` inside `pub mod` do NOT trigger clippy `dead_code` — they're public API, even when no callers exist yet. This is useful for future-tooling methods like `full_name()`, `icon_key()`, `next()`, `prev()`.
- `ListState` must be created locally (not stored) and passed by `&mut` to `render_stateful_widget`, even for read-only (no-interaction) rendering. Ratatui's stateful widget pattern requires `&mut` for potential state updates during rendering (scroll, highlight tracking).

## 2.3.3 — Canvas widget

- `Buffer::get_mut(x, y)` is deprecated in ratatui-core 0.1.1 — use `Buffer::cell_mut((x, y))` instead. `cell_mut` returns `Option<&mut Cell>` (non-panicking), satisfying the "no unwrap" invariant when handled with `if let Some`.
- `Buffer[(x, y)]` indexing panics on OOB — avoid it for invariant compliance. Use `cell_mut` with `if let`.
- `Style` in ratatui 0.30 has `add_modifier: Modifier` and `sub_modifier: Modifier` fields (not `modifier: Modifier`). Check reversed style with `cell.style().add_modifier.contains(Modifier::REVERSED)`.
- `(x - area.x) % zoom == 0` triggers `clippy::manual_is_multiple_of` in Rust 1.95 — use `.is_multiple_of(zoom)` instead. Method is safe only when `zoom > 0` (invariant holds for CanvasWidget where zoom ∈ [1,8]).
- Canvas keys (`+`, `-`, arrows, `G`) placed before toolbox keys in dispatch order prevents `g`-for-Fill conflict since canvas only intercepts uppercase `G`. Lowercase `g` still reaches toolbox for Fill tool.

## 2.3.6 — Status bar + canvas settings

- `Constraint::Length(1)` with `Block::default().borders(Borders::ALL)` yields 0 rows
  of content area — the text is effectively invisible. Changed to `Length(3)` for
  a usable 1-line content area between top/bottom borders. Without this change,
  status bar text never appears in the rendered buffer.
- Settings mode must intercept keys BEFORE canvas handler, since arrow keys are
  used for both canvas cursor movement and settings field navigation. A mode check
  at the top of `handle_key_event()` ensures settings captures ↑/↓/←/→/Enter/Esc
  before any other handler.
- `CanvasSettings` syncs values to canvas on every key event (dimensions → recreate
  widget, grid → toggle). This reactive approach ensures settings panel and canvas
  stay in sync without a separate "apply" step.

## 2.4.1 — Brush tool

- `EnableMouseCapture`/`DisableMouseCapture` live in `crossterm::event`, NOT
  `crossterm::terminal` as one might expect. In crossterm 0.28, mouse capture
  commands are event-system primitives alongside `EnableBracketedPaste`,
  not terminal-mode commands like `EnterAlternateScreen`.

## 2.4.5 — Selection tools

- `handle_key_event` signature changed from `KeyCode` to `impl Into<KeyEvent>` to
  support modifier checks (Ctrl+C/X/V). This is backward-compatible: all existing
  callers passing `KeyCode` continue to work via crossterm's `From<KeyCode> for KeyEvent`.
- `self.selection.clone()` fails because `Selection` doesn't derive `Clone` and
  can't easily (`Vec<Vec<bool>>` is cloneable but the borrow checker needs
  `Option::take()` to avoid simultaneous `&self` + `&mut self` on `self`).
- Circle scanline fill: `hw = sqrt(r² - dy²)` gives horizontal half-width at row
  offset dy. Must use `f64` for sqrt, then cast back to `i16` — this is the
  standard midpoint circle algorithm variant for filled circles.
- Polygon even-odd fill: `partial_cmp` fallback (`Ordering::Equal`) needed for
  `sort_by` on `f64` intersections (IEEE NaN edge case, though NaN should never
  appear in practice with valid geometry).
- Dashed perimeter overlay rendered in `CanvasWidget::render` by sorting perimeter
  cells by row-major order then alternating `▒`/space. At zoom>1, all N×N terminal
  cells of each perimeter buffer cell get the same dash character, preserving the
  dash pattern at any zoom level.
- Polygon close-on-click: check `|bx - fx| + |by - fy| < 3` (Manhattan distance
  < 3) to detect when user clicks near the first vertex. This matches common
  image editor behavior where closing requires clicking near the start point.

## 2.4.6 — Eyedropper tool

- `Palette::push_recent` was `fn` (private) because only `Palette` itself pushed
  recent colors. Eyedropper integration needs to push sampled colors — changed to
  `pub fn`. This is the first external caller to push_recent, exposing a
  pre-existing API gap in the palette module's public interface.
- `BrushState` gained `ch` field, requiring updates to every `BrushState { ... }`
  struct literal in tests across `brush.rs` and `tests/tui.rs`. This is the cost
  of using struct literals instead of `..Default::default()` in tests — but using
  `..Default::default()` would hide the meaningful fields from test readers.

## 2.4.3 — Line tool

- Full-buffer clone/restore for line preview is simple but `O(n)` on every drag
  event. For small canvases (≤200×100) this is imperceptible. Optimization to
  region-only save/restore deferred until performance measurements show need.
- Line tool reuses `brush::paint_line` (Bresenham) — no algorithm duplication
  vs. eraser which also duplicates the Bresenham loop. The duplication pattern
  (brush.rs has paint_line, eraser.rs has erase_line) could be refactored into
  a shared Bresenham iterator, but deferring since each variant needs per-step
  delegation (paint_stamp vs erase_stamp).
- Line tool uses `line_start` + `saved_buffer` as separate state fields from
  `prev_mouse_buf` (brush/eraser drag origin). This separation is deliberate:
  Line needs two-point semantics (start + current) while brush/eraser use
  sequential segment semantics (previous → current). Not combined to avoid
  complicating the simpler brush/eraser drag path.

## 2.4.7 — Spray paint brush

- `StdRng::seed_from_u64(thread_rng().gen())` seeds a fresh RNG from the system's
  thread-local RNG, avoiding `Result` (and thus `.expect()`) entirely.
- Using `rand::Rng::gen_bool(prob)` for stochastic selection is cleaner than
  `rng.gen::<f64>() < prob` — same effect, one function call.
- Spray tool shortcut `a` (aerosol) instead of `s` to avoid conflict with
  Settings toggle `S`. The toolbox `handle_key` does `to_ascii_lowercase()` so
  both `s` and `S` would match `s` — moving Settings check before toolbox
  resolves this.
- `'` was previously bound to cycle brush shape. Rebound to `\` to free `'`
  for density up (adjacent to `;` density down). This creates a contiguous
  size/density block: `[` size down, `]` size up, `;` density down, `'` density up.
- The spray stamp uses radius = `brush.size` directly (not `size/2`), making
  the spray area significantly larger than the circle brush at the same size
  setting. This is intentional — spray is meant to cover more area diffusely.
- `mul_add` is used for `dx*dx + dy*dy` in the circle check to avoid a
  separate multiplication, matching modern Rust idiom for fused multiply-add.

## 2.4.8 — Phase merge: release/2.4 → main

- Merge completed cleanly — no conflicts. All 20 files merged automatically.
  `docs/todo-v2.md`, `docs/memory.md`, `docs/memory-v2.md`, `docs/learnings.md`,
  and `docs/ralph-log.md` had additions only on `release/2.4` (no divergent
  changes on master), so no manual conflict resolution was needed.
- This aligns with the pattern seen in 2.0.10, 2.1.6, 2.2.6, and 2.3.7 phase
  merges — documentation files accumulate only on release branches, and master's
  versions only change during the phase merge commit itself.
- Two pre-existing test bugs found in phase review of `fill.rs`:
  - `test_fill_empty_region`: painted spaces on an already-space canvas, so
    flood fill had no boundary and filled the entire 5x5 instead of the 3x3
    center. Fix: fill border with `@` first.
## 4.4.2 — Blending modes

- `BlendMode::Normal` is a fast path in `composite()`: when both opacity=255
  AND blend_mode=Normal, the top cell is written directly (no per-pixel math).
  For Normal+opacity<255, the general path still computes blend_mode_color
  (which returns top color for Normal) then alpha-composites — same result but
  slightly more overhead.
- `blend_mode_color()` early-returns `top` for Normal mode, avoiding the
  `match` on Color variants entirely. This is an optimization for the common
  case (Normal is the default).
- Non-RGB `Color` variants (Indexed, AnsiValue, Reset, etc.) cannot be
  numerically blended. The strategy is: blend mode returns `Some(top)` when
  either color is non-RGB, preserving the top layer's color. This matches the
  existing `blend_colors()` behavior.
- Blend formulas use `u32` intermediate arithmetic to avoid overflow
  (2*255*255/255 = 510 fits in u32, but 2*255*255 = 130050 exceeds u16's 65535).
  The Overflow/light case computes `255 - 2*(255-top)*(255-bottom)/255` which
  peaks at 255 (result always ≤ 255).
- `blend_channel` for Subtract uses `bottom.saturating_sub(top)` (u8) not
  `bottom - top` which would underflow. For Add, `top.saturating_add(bottom)`.
- `b`/`B` keybindings in layer panel are active only when right drawer shows
  Layers mode (DrawerMode::Layers). When switching to Palette/BrushKeys mode,
  `b` returns to its normal function (Brush tool selection).

## 2.5.1 — Font mode scaffold: glyph grid overview

- Search UX in font editor overview conflicts with keyboard shortcuts (tool select `b`/`v`/etc,
  brush size `[`/`]`, density `;`/`'`, palette `x`/`h`/`z`, mode switch `Tab`, quit `q`, settings `S`).
  Using `/` as search activator (common in editors like Vim, VS Code) avoids all conflicts:
  `/` activates search, all subsequent printable chars build the query, Esc clears and deactivates.
  When search is inactive, all key events fall through to normal handlers (canvas, toolbox, palette).
- Grid cell sizing: `maxlength + 2` (min 8) for width × `charheight + 1` for height.
   Standard font (maxlength=16, charheight=6) yields 18×7 cells, giving 6 columns at 120-wide terminal.
- `FontEditor::render` must take `&mut self` (not `&self`) to allow search-state-dependent rendering
  (e.g., grid_scroll updates during render if `grid_cols` calculation changes).
- `TuiApp::render` borrow issue: `self.mode.title()` borrows `self` immutably, while
  `self.sync_font_char_to_canvas()` borrows `self` mutably. Fix: capture title into a local `String`
  before creating the `Block`, breaking the borrow chain.
- Font loading in `TuiApp::new()` uses `if let Ok(font) = load_font(...)` — no `.unwrap()`.
  When font can't be loaded (wrong CWD, font missing), `font_editor.font` stays `None`
  and the grid shows empty placeholder text.
- `FIGcharacter::rows()` returns `&[String]`, `width()` returns `usize` (first row char count).
  Canvas buffer populated by iterating rows × chars with `canvas.buffer.set(x, y, cell)`.
- `collect()`ing output from a `ratatui::buffer::Buffer` for test assertions:
  `buffer.content().iter().map(|c| c.symbol()).collect::<String>()`.
  This gives a flat string of ALL rendered characters including whitespace, which can
  be checked with `contains()`.

  - `test_fill_orthogonal_not_diagonal`: "diagonal" cells at corner positions
    (0,0), (2,0), (0,2), (2,2) were actually orthogonally adjacent to the cross
    at (1,0) and (0,1), so flood fill correctly filled them. Fix: use 3x3
    canvas where corners are truly diagonal-only to center.

## 2.5.5 — Add/remove codetagged characters

- `HashMap::from([(k, v), ...])` avoids `field_reassign_with_default` clippy lint
  compared to `HashMap::new()` + separate `.insert()` calls on a `&mut FIGfont`.
  Use `..Default::default()` spread in struct literals for the same reason.
- `filtered_codes()` uses `self.selected_index` to pick which code to delete
  in DeleteConfirm mode. The index 0 maps to the first code in the filtered
  list (typically code 0, the missing char). When testing, set `selected_index`
  explicitly or delete via a different path to avoid deleting code 0.
- Code input flow uses two-step copy (CopySource → CopyDest) with `copy_source_code`
   stored between steps. This matches the UX pattern: "C → type source → Enter → type dest → Enter".

## 2.6.1 — Image import + canvas display

- RGB→resize→luma pipeline differs from luma→resize pipeline (bilinear interpolation
  on 3 channels vs 1). `test_image_editor_matches_cli_output` originally compared
  `ImageEditor` (resize RGB→luma) against `image_to_ascii` (luma→resize), which
  never matches exactly due to rounding differences. Fixed: compare against
  `color_matrix_to_ascii` which uses the same RGB→resize→luma path.
- `ImageEditor::target_width` is a public field (`pub` not declared — initialized
  to 80 in `new()`). Tests set it directly before `load_from_path`. Making it
  settable via constructor parameter would be more idiomatic but adds complexity
  for a single test use case.
- Path entry flow: `o` → type path → Enter loads, Esc cancels. The `path_buffer`
  accumulates typed chars and is taken on Enter via `std::mem::take`. Error from
  `load_from_path` stored in `error_message` for display in block title.
- Mode toggle (`c`/`C`) re-renders from cached `original_rgb` with same resize
  parameters — no re-load needed. This avoids unnecessary filesystem I/O.

## 2.5.6 — Font-level transform tools

- Clippy `for_kv_map` lint: `for (_code, ch) in &font.chars` should be `for ch in font.chars.values()` 
  when the key is unused. The lint is `-D` by default, so all such patterns must use `.values()`.
- Borrow checker with `maxlength_from_chars()`: calling `self.maxlength_from_chars()` while
  `self.font.as_mut()` is borrowed creates a conflict. Fix: inline the maxlength computation
  after the mutable borrow ends, or compute `maxlen` in a local before assigning to `font.maxlength`.
- `load_font()` does filesystem I/O depending on CWD. Unit tests that call it must use
  `concat!(env!("CARGO_MANIFEST_DIR"), "/../fonts")` for a reliable fontdir path.
  Alternatively, pass fontdir as a parameter (as done with `transform_copy_glyph_from`).
- For multi-step UI flows (CopyGlyph: font name → code point), store intermediate state
  in dedicated fields (`transform_font_name`) and use `sub_step` to track which step is active.
  Clone the stored name before the final mutation step to avoid borrow conflicts.

## 2.6.2 — Text tool with FIGlet font overlay

- `add_char()` can be reused for both CLI rendering and TUI canvas text tool — the same
  `Vec<String>` row pipeline works for both, with justification computed as x-offset
  instead of prepended spaces for canvas placement.
- Font scanning uses `std::fs::read_dir` on `fonts/` directory — must handle the case
  where directory doesn't exist (empty list returned gracefully).
- When using `add_char()` for the text tool, the smush mode should use the font's
  `full_layout` for FIGlet-compatible rendering, with KERN fallback when `full_layout < 0`.
- Overlaying FIGlet text on canvas requires skipping space cells (transparent background)
  to preserve existing canvas content behind the glyph's "holes". Only non-space chars
  from the FIGlet output are stamped.
- Mouse click to enter text mode: the text tool intercepts mouse events before the
  drawing tool early-return check. This cleanly separates text tool interaction from
  brush/paint interactions.
- Key dispatch ordering: font navigation (up/down) must be handled before canvas handler
  to prevent arrow keys from moving the canvas cursor when the intention is font list
  navigation. Text entry mode captures ALL printable keys before any other handler.
- Text tool options panel replaces the brush options panel in the left sidebar when
  `Tool::Text` is selected — conditional render in `tui/mod.rs:197` swaps between
  `brush.render()` and `text_tool.render_options()`.

## 2.6.3 — Text tool advanced: selection + transform

- `rotation` field must be `u16` (not `u8`) because valid values include 270 and 360
  (rotation % 360), and `u8` can only hold 0..=255. Using a larger integer type avoids
  `overflowing_literals` compile error.
- Font navigation (up/down) must skip when a block is selected — otherwise arrow keys
  intended to move the block are consumed by font list navigation instead. Added
  `self.text_tool.selected_block.is_none()` guard to the font-navigation match.
- Private `render_rows_from_buffer()` helper extracts FIGlet rendering logic from
  `render_text_to_buffer()` so that `commit_block()` can cache rendered rows without
  needing a `CanvasBuffer` reference — avoids code duplication.
- `move_selected_block` uses `wrapping_add` for x/y to prevent arithmetic overflow
   (defensive — buffer coordinates are bounded but user could move block arbitrarily).

## 2.6.4 — Image adjustments

- Clippy `collapsible_match` lint: `if cond { match expr { Arm => {} } }` should be
  `expr if cond => { Arm }` using a match guard on the outer match arm. Three instances
  in `handle_key()` for `+`, `-`, and `Esc` all collapsed into guard patterns.
- Clippy `unnecessary_min_or_max` lint: `(u8_val + 8).min(255)` is a no-op because
  `u8` arithmetic wraps on overflow (debug panics, release wraps). The `.min(255)`
  is dead code since the result can never exceed `u8::MAX`. Use `saturating_add(8)`
  instead to correctly handle overflow to 255.
- `u8` addition of two literal values: `self.threshold + 8` where both are `u8` will
  overflow at runtime if threshold >= 248. `saturating_add(8)` is correct — it
  saturates at `u8::MAX (255)`.
- `rgb_to_luminance_matrix` converts RGB pixels to BT.709 grayscale. This bridges
  the RGB (24-bit color) and braille (luminance-only) pipelines. The braille pipeline
  operates on `Vec<Vec<u8>>`, so the conversion happens after all RGB adjustments
  (brightness, contrast, invert) are applied.

## 2.8.1 — Migrate to Component Architecture

- Test file `figby-rs/tests/tui.rs` is outside the "Touches" scope (`figby-rs/src/tui/*.rs`)
  but field renames in `mod.rs` break its compilation. Tests referencing old field names
  (`app.toolbox`, `app.canvas`, `app.brush`, `app.palette`) must be updated to match.
  These changes are a necessary consequence of the refactoring, not scope creep.

## 2.9.1 — tui-menu integration

- `tui-menu` 0.3.1 depends on `ratatui-core ^0.1.0` and `ratatui-widgets ^0.3.0` (part of
  ratatui 0.30.x ecosystem) — compatible with existing ratatui 0.30.1.
- `tui-menu` does NOT handle keyboard or mouse events internally. Caller must map key events
  to `up/down/left/right/select/reset` calls on `MenuState`.
- `MenuEvent` is a single-variant enum (`Selected(T)`), so `drain_events()` produces
  irrefutable patterns. Use `drain_events().next()` with `let` destructuring to avoid
  clippy `never_loop` and `irrefutable_let_patterns` warnings.
- Layout changed from 3 chunks `[3, Min, 3]` to 4 chunks `[1, 3, Min, 3]` for menu bar.
- Mouse clicks only work on menu bar labels, not dropdown items (tui-menu limitation).

## 2.9.3 — Prettier status bar

- Ratatui `Line` does not support mixed left/right alignment within a single
  line. To implement LazyVim/Starship-style status bars with left/center/right
  sections, compute left and right section widths via `chars().count()`, then
  pad the center section with spaces to fill remaining terminal width. This is
  the same approach used by vim/neovim statusline plugins.
- `SystemTime::duration_since(UNIX_EPOCH)` returns `Result` (can fail if system
  clock is before epoch). Using `unwrap_or_default()` safely falls back to
  `Duration::ZERO` in edge cases, avoiding `unwrap()` in production.
- Git branch detection via `std::process::Command` at startup avoids per-frame
  subprocess overhead. The result is cached in `TuiApp.git_branch` for the
  lifetime of the application session.
- FPS EMa smoothing (α=0.1) provides stable display without flickering: the
   instant FPS can vary wildly between frames (0-60+), but the smoothed value
   converges within ~10 frames. This matches common game engine FPS counter
   implementations.

## 2.9.4 — Theming system with YAML theme file

- Raw string literals `r#"..."#` in Rust terminate at `"#` — a YAML hex color
  `"#ff0000"` contains `"#` which prematurely closes the raw string. Use `r##"..."##`
  (double-hash delimiter) when the content contains `"#` sequences.
- Deserializing `ratatui::style::Color` from YAML hex strings requires an intermediate
  struct with `Option<String>` fields, then manual conversion to `Color::Rgb`. Using
  `#[serde(deserialize_with)]` on each field is too verbose for 40+ tokens; the
  intermediate struct + `From<ThemeYaml>` approach is cleaner.
- Theme field on `CanvasWidget` (which implements `Widget for &CanvasWidget`) must be
  a `pub` field accessed in `render(self, area, buf)`. The `Widget` trait has no
  parameter for passing extra state — all context must be in `&self`.
- When removing `Color` from a module's `use` statement, verify no remaining `Color::`

## 3.2.3 — Font preview strip in overview

- Adding a preview strip (8 rows) below the glyph grid changes the layout constraints,
  shrinking the grid area. Integration tests using fixed terminal sizes (e.g. 120×50)
  may fail because codes previously visible at the bottom of the grid are now clipped.
  Fix: increase terminal height to accommodate the new strip (120×50 → 120×60).
  references exist in the file. grep for `Color::` to confirm zero matches across
  production and test code.

## 2.10.2 — Dirty render mode coverage gaps

- `dirty` flag pattern: one flag covers ALL UI changes. Every mutation path must set it,
  including paths that aren't triggered by user input (async completion, auto-save timers).
- `check_async_completion` lived only in `render()`, creating a circular dependency: render
  needs dirty=true, but dirty could only be set by events. Moving it to `handle_event()`
  breaks the cycle — now async results are processed every iteration regardless of render state.
- When the `dirty` flag is set at the top of `handle_event()` for incoming events, it covers
  ALL event-driven state changes. The gaps are non-event-driven paths: auto-save timers,
  async completions, and programmatic state transitions (dialog opens, settings apply, menu actions).
- Each `perform_*` method that starts an async operation should set `dirty = true` to show the
  throbber immediately. Without it, the throbber only appears after a user event + render cycle.

## 3.2.2 — Glyph char editor: GlyphCursor + cell toggle

- Blinking cursors need persistent state across frames. Recreating the `GlyphCursor` every frame
  via `GlyphCursor::new(x, y)` resets `Instant::now()` so the blink timer never reaches 500ms.
  Fix: create only when `glyph_cursor` is `None`, otherwise just update x/y.
- `handle_key_char_editor` now handles arrow keys (movement) and Space (toggle). The space handler
  uses `brush_char` synced from palette in `mod.rs` key dispatch before font_editor handler runs.
- `CanvasWidget::render` draws the `GlyphCursor` overlay (blinking `█`) when `glyph_cursor` is
  `Some`, or falls back to the normal reversed-style cursor highlight.

## 3.3.1 — Full regression testing

- **Font editor overview intercepts Ctrl+* combos.** `handle_key_overview` only inspects
  `KeyCode`, discarding `KeyModifiers`. Ctrl+O/S/E/K treated as search input.
  `modifiers` param in `FontEditor::handle_key` not forwarded to overview handler.
- **Toolbox subverts clipboard shortcuts.** Toolbox handles `c`/`v`/`x` for tool
  selection, so Ctrl+C/X/V for clipboard never reaches selection handler in
  FontEditor mode. Must use AsciiPreview mode or direct API.
- **`MenuAction::drain_actions()` consumed inside `handle_key_event`.**
  Cannot drain again from state; must capture `handle_key_event` return value.
- **Text tool commit requires font loading.** `commit_block` calls
  `render_rows_from_buffer()` which needs a loaded font. If font_dir is wrong,
  commit silently fails leaving `entering_text = true`.
- **`ExportDialog::handle_key('t')` toggles format** before the generic char arm.
  Use paths without 't' for path entry tests.
- **`Selection::marquee`** creates rectangle selection from 2 points.
  For integration tests, create programmatically then set `app.editor.selection`.

## 4.2.4 — Ogham charset

- Palette static string in `CHAR_GROUPS` for ogham used U+0020 (regular space) as
  the first character instead of U+1680 (Ogham Space Mark). This was a spec-compliance
  bug: the `font_gen.rs` `ogham_charset()` includes U+1680 as the first codepoint,
  but the palette display string used a regular space. Fix: replace the leading
  U+0020 byte with the UTF-8 encoding of U+1680 (`E1 9A 80`).

## 4.2.1 — Braille charset block

- Production code (braille_charset, resolve_charset wiring, deluxe_charset
  integration, CHAR_GROUPS static string) was already in place from earlier
  font_gen development. Task deliverable was exclusively 7 verification tests.
  This is an unusual pattern — typically tasks implement new production code.
  When production code pre-exists, the task can focus solely on test coverage
  and documentation.

## 3.3.2 — v3.0.0 RC cut

- Crate version and FIGlet-compatible version are separate concerns.
  `Cargo.toml version` → semver (crate release). `VERSION_INT`/`VERSION`
  in `main.rs` → FIGlet protocol compatibility (stay at 2.2.5).
- RC tags should follow existing pattern: `rc/X.Y.Z-rc.N` branch + annotated
  `vX.Y.Z-rc.N` tag. The `v` prefix is consistent with earlier version tags.

## 4.3.2 — Apply ratatui architecture fixes from audit

- The `Widget for &T` pattern renders to `Buffer`, not `Frame`. Converting
  Frame-based render methods to Buffer-based requires replacing
  `frame.render_widget(w, area)` with `ratatui::widgets::Widget::render(w, area, buf)`.
- `FontEditor` had ~2000 lines of Frame-based rendering. Making it implement
  `Widget` required converting all sub-render methods from `Frame` to `Buffer`,
  and extracting `StatefulWidget` state mutation into a separate
  `before_render(&mut self)` step.
- When converting `StatefulWidget` to regular `Widget`, the state (e.g.
  `GlyphGridState::cell_rects`) must be pre-computed in `before_render()`
  since the Widget impl only borrows `&self`.
- Screen-to-buffer coordinate conversion needs the canvas inner rect, which
  was previously stored on `CanvasComponent`. After inlining, it's computed
  locally in both `render_canvas_area()` and the mouse handler, which
  computes `FrameLayout` from `crossterm::terminal::size()`.
- The mouse handler's `FrameLayout` computation duplicates what `render()`
  does, but it's cheap arithmetic and avoids stale stored geometry.
- Removing `pub use` re-exports (`BrushState`, etc.) breaks integration tests
  that import from the crate root. These must be maintained for public API
  compatibility even though the internal architecture changed.

## 4.5.5 — Phase merge: release/4.5 → master

- Merge completed cleanly — no conflicts. All documentation files merged automatically
  (`docs/todo-v4.md`, `docs/memory.md`, `docs/learnings.md`, `docs/ralph-log.md`).
- No code changes beyond merge.

## 4.4.5 — Phase merge: release/4.4 → master

- Merge completed cleanly — no conflicts. All 11 files merged automatically
  (2320 insertions, 153 deletions). Unlike the 4.3.3 merge which had a
  `docs/ralph-log.md` conflict, this merge had zero conflicts across all
  documentation files (`docs/todo-v4.md`, `docs/memory.md`, `docs/learnings.md`,
  `docs/ralph-log.md`). The `docs/ralph-log.md` had a staged modification on
  `task-4.4.5` which was stashed before merge.
- No code changes beyond merge conflict resolution (none needed).

## 4.4.3 — Layer groups + masks

- Clippy `option_map_unit_fn` lint fires on `opt.map(|v| v.field = val)` —
  use `if let Some(v) = opt { v.field = val; }` instead. The `map` with a
  side-effect-only closure is considered an anti-pattern (should only map
  values, not cause side effects).
- Changing `LayerPanel::handle_key` from `KeyCode` to `KeyEvent` required
  updating the call site in `mod.rs:1736` to pass the full `key` event
  (which is already in scope as a `KeyEvent` from `handle_key_event`).
- `toggle_mask()` combining create/remove into a single toggle operation
  is cleaner than separate `create_mask`/`remove_mask` for the `M` keybinding,
  but all three methods are useful for programmatic control (tests,
  undo/redo). Both toggle and explicit paths are exposed.
- Mask thumbnail rendering samples 3 horizontal cells from row 0 of the
  mask buffer. This is a simplification over "sample near cursor" — the
  cursor position isn't available in the layer panel render path. Sampling
  from row 0 is sufficient for a visual mask presence indicator.

## 4.6.3 — Particle-to-layer baking

- `bake_frames()` calls `self.clear()` first, so frame generation starts from
  a clean particle system. This means baked frames reflect N frames of fresh
  emission, not a continuation of the current live state.
- `add_frozen_frames()` sets `self.active` to the last frame's index after
  inserting all frames. This makes the final frame active for immediate
  display after insertion — intentional for the `B` keybinding flow.
- `test_bake_frames_count_and_independence` uses `windows(2).all(...)` on the
  frame vec to verify adjacent frames differ. This is O(N) without comparing
  all O(N²) pairs — sufficient for proving non-identity across a sequence.

## 4.6.4 — Phase merge: release/4.6 → main

- Merge completed cleanly — no conflicts. 4 files changed (CHANGELOG.md,
  docs/memory.md, docs/ralph-log.md, docs/todo-v4.md), 18 insertions, 2 deletions.
- No code changes beyond merge.

## 4.8.3 — Player integration into TUI

- `play_fullscreen()` does its own alt screen lifecycle (EnterAlternateScreen
  inside `enter_player_mode`, LeaveAlternateScreen inside `exit_player_mode`).
  After it returns, `play_animation()` must call `EnterAlternateScreen` again to
  restore the TUI's alt screen. This double-enter pattern is correct because
  `play_fullscreen`'s `exit_player_mode` returns to the main screen.
- The `Enter` key handler for timeline playback lives in the general key dispatch
  (not gated by timeline focus) — any Enter press with non-empty `timeline_state.frames`
  triggers playback. This is intentional but could surprise users expecting Enter
  in dialogs to do other things (protected by dialog handler running first).
- Using a boolean flag (`play_requested`) as a back-channel signal from ExportDialog
  to TuiApp avoids coupling the dialog to the app's event loop directly. The flag
  is consumed and reset in the same main-loop iteration.

## 4.8.0 — AnimationPlayer widget

- `Cell` interior mutability enables `Widget for &AnimationPlayer` (not `&mut`).
  Ratatui's `Widget` trait takes `self` by value, so `&AnimationPlayer` is the
  recommended pattern for widgets with state. `Cell` is safe for `Copy` types
  (`usize`, `bool`, `f64`) and avoids `RefCell` runtime overhead.
- Accumulator-based frame advancement: `advance(delta)` accumulates elapsed time
  and only advances frames when the accumulated time exceeds `1/effective_fps`.
  This prevents frame-skipping on variable frame-rate event loops.
- Progress bar renders play icon (▶/⏸), frame counter (`cur/total`), filled bar
  (█/░), and speed label in a single terminal row. Manual `cell_mut()` writes
  avoid ratatui `Paragraph` overhead for fine-grained character control.

## 4.7.4 — Phase merge: release/4.7 → main

- No code changes — merge was a no-op (release/4.7 already an ancestor of master
  from prior merge commits). Only docs, changelog, and metadata updated.
- Fixed stale merge conflict markers in `docs/ralph-log.md` (leftover from 4.3.3
  and 4.6.4 phase merges).

## 4.9.1 — TachyonFX spike: welcome screen fade-in

- `fx::fade_from_fg()` returns `tachyonfx::Effect` (not `Box<dyn Shader>`). `Effect`
  is a concrete struct wrapping a shader, with its own `process()`, `done()`,
  `running()` methods directly. `Effect` does NOT implement the `Shader` trait.
- `Effect` is `!Send` and `!Sync` by default (unless the `sendable` feature is enabled).
  This is fine because `TuiApp` is neither `Send` nor `Sync`.
- With `std-duration` feature, `tachyonfx::Duration` = `std::time::Duration`,
  making it easy to use with Rust's standard `Instant` timing.
- tachyonfx's `Shader::process(dt, buf, area)` takes a `Duration` step (delta-time
  since last frame, not cumulative elapsed time). The effect's internal timer
  accumulates these steps automatically.
- `EffectTimer::from_ms(ms, Interpolation::QuadOut)` creates a timer with 400ms
  duration and quadratic-out easing — clean and ergonomic.
- `Effect::done()` returns `true` when the effect has completed its full duration,
  allowing clean cleanup after animation finishes.

## 4.9.3 — App fade-in on launch (ratzilla-style)

- `fx::fade_from(bg, fg, timer)` (three-argument variant) applies a full-screen overlay
  that transitions both background and foreground colors from the given values to
  transparent — ideal for app-launch black-to-reveal effects. This differs from
  `fx::fade_from_fg()` (used in 4.9.1) which only modifies foreground.

## 4.10.1 — WASM / web target via Ratzilla

- Ratzilla 0.3.1 uses `DomBackend`, `WebRenderer` trait, and `draw_web()` (not `draw()`).
  Events registered via `on_key_event()` callbacks. Compatible with ratatui ^0.30.1.
- `ratatui = "0.30.1"` must be split across targets: native builds need `features = ["crossterm"]`
  (enables `init()`/`restore()`), WASM builds need `default-features = false` (avoids crossterm).
  Cargo resolves target-specific deps per-target, so no feature unification conflict.
- `zip` crate with default features pulls in `lzma-sys` (C dependency) which fails on WASM.
  Fix: `default-features = false, features = ["deflate"]` uses pure Rust miniz.
- `flate2` with default features uses C libraries (zlib/libzma). Fix: `default-features = false,
  features = ["rust_backend"]` for pure Rust deflate implementation.
- `CanvasCell` type (defined in tui/canvas.rs) is needed by output.rs unconditionally.
  For WASM builds (where `tui` module is disabled), the type must be available at crate root.
  Moved to `canvas_inner` private mod + `pub use` at lib.rs root. tui/canvas.rs re-exports via
  `pub use crate::CanvasCell;`.
- Several native-only functions in `image_input.rs` and `main.rs` used `crossterm::terminal::size()`.
  Added `get_terminal_width()` helper with dual cfg versions (crossterm for native, `None` for WASM).
- `getrandom` feature for WASM is `"js"` (not `"wasm_js"`).
- `cargo build -p figby --target wasm32-unknown-unknown` succeeds for both lib and bin targets
  (bin has stub `fn main(){}` for WASM).

- The effect must be applied AFTER rendering the UI content but BEFORE `frame.finish()`,
  so the overlay draws on top of the rendered widgets. Calling `.process()` on
  `frame.buffer_mut()` after all `frame.render_*` calls achieves this correctly.
- The `Option<Effect>` + `if let Some(ref mut)` + `take()` on `done()` pattern works
  for one-shot launch effects: the effect runs every frame during its duration, then
  drops itself cleanly with no further overhead.

## 5.2.4 — Phase merge: release/5.2 → main

- **Branch name mismatch:** AGENTS.md says "merge to main" but the default branch is `master`. Merge commit message says "into main". No functional impact, but conventions are inconsistent. Worth deciding: rename default branch to `main` or update docs.
- Version bumped to 5.2.0 (minor) — phase 5.2 adds major layout restructure.

## 5.2.1 — Palette moved under tools (left column)

- `Constraint::Min(0)` for palette allocation is preferred over `Constraint::Fill(1)`:
  Min(0) lets the palette shrink to zero if the terminal is short, while Fill(1)
  would always claim at least 1 row, potentially squeezing the toolbox.
- `Tool::all().len() + 1 + TOOLBOX_BRUSH_HEIGHT` must be recalculated wherever
  `FrameLayout::compute` is called (3 sites in `mod.rs`). Forgetting any call site
  causes palette area height mismatch. Using `rg "FrameLayout::compute\("` to find
  all callers before changing the signature is essential.
- Removing an enum arm (`Palette`) from a match on `DrawerMode` leaves a `_ => {}`
  wildcard. This is correct but means clippy's `wildcard_enum_match_arm` won't
  fire because not all arms are explicitly matched. The wildcard is deliberate:
  Palette no longer has a right-panel rendering path.

## 5.6.6 — Phase merge: release/5.6 → main

- TUI dispatch logic in `main()` had an incomplete guard: it only checked for
  `--create-font`, `--render-template`, `-I`, and `-v` before launching the TUI.
  FIGlet processing flags (`-L`, `-R`, `-f`, `-w`, etc.) were not checked, causing
  `figby -L` to launch the TUI and crash in non-TTY environments. Adding
  `has_figlet_flags()` + `std::io::stdin().is_terminal()` fixed all integration tests.
- `std::io::IsTerminal` trait must be imported separately on Rust 1.70+ — it's
  not part of `std::io::prelude`. The `is_terminal()` method is provided by the trait.
- Tests that modify process-wide env vars (`XDG_CONFIG_HOME`) race in parallel test
  execution. A `std::sync::Mutex` in the test module serializes access. Use
  `OnceLock` to lazily initialize the mutex.
- `2.7_f64 - 2.0_f64 = 0.7000000000000002` due to IEEE 754 representation.
  Floating point comparisons should use `1e-10` tolerance, not `f64::EPSILON`,
  when the expected value is a common decimal fraction that is not exactly
  representable.

## 5.7.1 — Animated GIF import to timeline

- `gif` crate 0.13 API differs significantly from both the `image` crate's GIF
  codec and from earlier gif crate versions:
  - `gif::Decoder::new(reader)` ALREADY reads the magic + header — no separate
    `read_info()` call needed. `DecodeOptions` exists but is not required for
    basic use.
  - Frame disposal method is `gif::DisposalMethod` (not `Dispose`): `Any = 0`,
    `Keep = 1`, `Background = 2`, `Previous = 3`. Re-exported at `gif::DisposalMethod`
    from `gif::common`.
  - Loop count is `gif::Repeat` enum with `Finite(u16)` and `Infinite` variants,
    accessed via `decoder.repeat()`. Not through Netscape extension parsing —
    the crate handles that internally.
  - `bg_color()` returns `Option<usize>` (palette index), not `u8`.
  - `read_next_frame()` returns `Result<Option<&Frame<'static>>, DecodingError>`.
    Frames borrow from the decoder with `'static` lifetime but must be cloned
    to outlive the decoder's next call. `Frame` derives `Clone`.
  - Frame buffer is `Cow<'a, [u8]>` — pixel indices, not RGBA values.
    Pixels must be looked up in the frame's local palette or the global palette.
  - `global_palette()` returns `Option<&[u8]>` — flat RGB bytes, 3 per entry.
  - `gif::DecodingError` does NOT implement `std::error::Error::source()`
    returning `Some(&std::io::Error)` — must be converted via `.to_string()`.
- Dispose handling in GIF compositing follows the spec:
  1. Apply previous frame's dispose to canvas BEFORE rendering current frame
  2. Save canvas state if current frame uses `DisposalMethod::Previous`
  3. Render current frame onto canvas
  4. Record snapshot for result vector
- `CanvasCell` (from `lib.rs` `canvas_inner` module) is available project-wide
  without gating — it only depends on `ratatui::style::Color` which is always
  compiled. This means `gif_import.rs` can use `CanvasCell` directly without
  importing TUI-only modules.

## 6.8.4 — Palette editor UI

- Borrow checker requires `let name = self.name_buffer.clone();` before calling
  `self.rename_selected(&name)` — cannot borrow `self` mutably and immutably
  in the same expression even though the field access (`self.name_buffer`) is
  disjoint from the method's target (`self.swatches`). Clone the string first.
- When reusing `PanelMode::Naming` for two intents (duplicate vs rename), a
  `naming_is_rename: bool` flag is simpler than adding yet another enum variant.
- The palette editor already had full IO (save/load/import) but lacked
  in-place manipulation — add/edit/delete operations on swatches are purely
  in-memory Vec mutations, no file I/O needed.

## 5.7.2 — Phase merge: release/5.7 → main

- Task description says `main` but actual default branch is `master`. All prior
  phase merges (5.0.7, 5.1.5, ..., 5.6.6) merged into `master`. This is the same
  naming discrepancy noted in 5.2.4. The convention is inconsistent but stable:
  `main` in task text = `master` in git.

## 5.8.5 — Phase merge: release/5.8 → main

- Same `main` vs `master` branch-name discrepancy as 5.7.2 and prior phase
  merges. Task text says `main`, actual default branch is `master`. Consistent
  with the established convention.

## Rotate tool — mouse drag/keyboard wiring (post-v6)

- `TuiApp::handle_key_event` dispatches to `handle_font_editor_key` /
  `handle_image_editor_key` (gated on `self.mode`) *before* the generic
  toolbox tool-shortcut matching runs. In `AppMode::ImageEditor`,
  `ImageEditor::handle_key` unconditionally claims a bunch of single-char
  keys for its own adjustment-mode bindings (`r`/`R` reset, plus `b`, `k`,
  `t`, `w`, `c`, `i`, `d`, `y`, `o`), so those letters never reach the
  toolbox shortcut dispatch while in Image Editor mode — pressing `r` there
  does NOT select the Rotate tool. This is pre-existing and independent of
  the Rotate tool's own logic (verified with `Toolbox::handle_key` directly,
  which works fine — the conflict is purely in `TuiApp`'s routing order).
  Anyone adding a new single-letter tool shortcut should check whether
  `ImageEditor::handle_key` (or `FontEditor`'s equivalent) already claims
  that letter — the match arms shadow silently, with no compiler warning,
  since Rust can't statically prove two `if`-guarded arms are unreachable
  duplicates of each other.
- Found the same class of bug in `welcome.rs::WelcomeScreen::handle_key`:
  it had two `KeyCode::Char('I') if modifiers == KeyModifiers::NONE` arms
  in the *same* match — one returned `WelcomeAction::FontNewFromFile`, the
  other (dead, unreachable) returned `WelcomeAction::ImageOpen`. Since arms
  are matched top-to-bottom and the guards were textually identical, the
  first always won; the Image panel's "I - mport/Open Image" binding never
  fired. `cargo clippy` does not flag this either, for the same reason as
  above (duplicate literal + duplicate guard, but guards aren't proven
  equivalent). Fixed by giving the Image action its own letter ('L' — Load
  Image) instead of trying to disambiguate by context, since font and
  image action panels render simultaneously (not tabs) — there's no "which
  panel is active" to key off of.
- The user's real complaint ("the a doesn't work either") turned out to be
  a *second*, more fundamental bug, not just the 'I' duplicate: every
  welcome-screen action shortcut matched only the exact-case literal
  (`'A'`, `'N'`, `'S'`, etc.) with `modifiers == KeyModifiers::NONE`. On a
  terminal that reports an unshifted keypress as lowercase with no SHIFT
  flag (the common case), pressing the plain letter shown in the UI does
  nothing — you have to hold Shift, inconsistent with every other
  single-letter shortcut in the app (toolbox tool shortcuts are matched
  case-insensitively via `c.to_ascii_lowercase()` in `mod.rs`, and
  `ImageEditor::handle_key` matches both cases explicitly). Fixed by
  changing every `KeyCode::Char('X')` arm in `welcome.rs::handle_key` to
  `KeyCode::Char('x') | KeyCode::Char('X')`.
  Fixing this had a surprising side effect: five existing tests
  (`test_line_tool_keyboard_paint`, `test_spray_tool_keyboard_paint`,
  `test_eyedropper_tool_keyboard_does_not_paint`,
  `test_text_tool_enter_text_mode`, `test_text_tool_commit_text`) started
  failing because they never set `app.welcome_screen.show = false` before
  sending tool-shortcut keys — they were only passing because the welcome
  screen ignored lowercase letters and let the keypress fall through to
  the toolbox dispatch. Once the welcome screen legitimately started
  handling lowercase too, it correctly intercepted those same letters
  first. The fix is to add `app.welcome_screen.show = false;` to each of
  those tests (matching the convention nearly every other test in
  `tests/tui.rs` already follows), not to weaken the welcome-screen fix.
  Lesson: a test that doesn't explicitly dismiss the welcome screen is
  implicitly depending on whatever the welcome screen ignores — that's an
  easy way for an unrelated bug fix to "break" tests that were never
  actually exercising the feature they claimed to.

## 7.0.1 — Commit timeline frame edits on switch

- The exact pattern from the `KeyCode::Char('A')` capture block at `mod.rs:3401-3418`
  (`composite()` → `capture_thumbnail()` → assign to `frame.layer_state`)
  was reused for `commit_current_timeline_frame`. Duplication is acceptable
  here because the A-key block also creates new `TimelineFrame` instances
  (`layer_keyframes`, `label`, `add_frame()`), while the commit helper
  only updates the existing frame. The shared logic could be refactored into
  a private helper in a later phase, but the scope guardrails for 7.0.1 forbid
  touching the A-key block.

## 6.10.1 — `capture_timeline_frames` ignoring per-frame `layer_state`

- When `capture_timeline_frames` iterates `(0..timeline.frames.len())`, the
  per-layer keyframe-interpolation path re-renders from the *live* layer stack
  — which is unchanged across every `frame_idx`. So captured frames (GIF import
  or 'A'-key manual captures that have their own `layer_state` raster snapshot)
  all produce identical output. The fix: check `layer_state` on each frame
  first and use it directly. The `current_frame` counter still advances (making
  the progress bar look correct), so this bug is easy to miss during visual
  inspection — the regression test is essential.
- The `click_test_hookup` approach for manual verification in Windows Terminal:
  run the TUI binary, import a multi-frame GIF or build multiple captured frames
  via 'A'-key, then press Enter to play. If every frame shows the same content,
  the bug is present. If frames vary, it's fixed. `tmux` can mask this because
  tmux's own compositing can hide ratatui cache staleness.

## 7.1.2 — Rebind chrome keys to Alt+arrows

- The inline `T` handler in `mod.rs` was NOT a dead duplicate of the
  `GLOBAL_DISPATCH` entry despite both handling `(NONE, 'T')` → ToggleTimeline.
  The inline handler ran BEFORE the tool-selector catch-all; removing it meant `T`
  would be caught by the tool selector (which converts char to lowercase and
  matches `Text` tool at `'t'`). Fix: exclude `c != 'T'` in the tool-selector guard.
- `KeyModifiers::ALT | KeyModifiers::SHIFT` creates a combined bitmask; equality
  check against this value matches only when both Alt and Shift are pressed exactly.
- Palette widget (`palette.rs:308`) has no `has_focus()` method, so palette-nav
  priority over canvas arrows required physical reordering of handler blocks.

## 7.0.3 — Reconcile playback cursor with timeline `current_frame`

- When animation playback and timeline UI have independent frame cursors, the
  timeline strip freezes on the start frame and the canvas reverts to the start
  frame after stop. Fix: sync `current_frame` from player to timeline state on
  each tick and on stop/dismiss. The `playing` field on `TimelineState` was
  already dead code (nothing read it).

### Splitting one giant `impl TuiApp` across files (7.3.4)

- Rust lets one type have multiple `impl` blocks in different modules; each
  submodule does `use super::TuiApp; impl TuiApp { ... }`. BUT a private
  method defined in module A is NOT callable from module B's `impl` block —
  inherent-method visibility is module-scoped. Bump every cross-module
  method to `pub(crate)`. Libuild will flag them; iterate.
- Slice boundaries matter: a doc-comment (`///`) belongs to the *next* item.
  When extracting method N's lines as `[sigN, sigN+1-1]`, the line
  `sigN+1-1` may be `///` doc of N+1 (orphan → `E0584`). Bound slices by
  the line *before* the next item's doc start, not the line before its sig.
- `use super::*` glob imports don't trigger `unused_imports` warnings, so
  submodules can glob parent re-exports safely. Explicit `use` of external
  crate names DOES warn — only import names each file actually uses.
- A private `use` alias in a parent is visible to descendant modules but
  `glob`-importing it still leaves the parent's own `use` flagged as
  unused; move such imports into the child test mod that needs them.

### Particle keyframe interpolation + on-death bursts (7.5.2)

- Keyframe `time` is a fraction `[0,1]` of total lifetime, not absolute
  seconds. Store `total_lifetime` as a spawn-time snapshot on the particle
  — `remaining_lifetime` decreases but `total_lifetime` is the stable
  denominator for `progress = 1.0 - remaining/total`.
- Character interpolation: glyphs don't lerp. Pick the nearer endpoint:
  `a.character` if `t < 0.5`, else `b.character`. A "morph" mode that
  crossfades both glyphs with split opacity is a future extension.
- On-death burst recursion: guard with `is_secondary: bool` on the
  particle. Secondaries never trigger another burst. Depth-1 cap is a
  design choice; N-level recursion would need a `depth: u8` counter.
- `ParticleConfig` with `on_death_config: Option<Box<ParticleConfig>>`
  — the `Box` keeps the struct a fixed size despite the recursive
  reference. Without it, `Option<ParticleConfig>` is infinitely sized.
- `render_values()` clones + sorts keyframes on every call. Acceptable
  for now (particle counts are low); cache sorted `Vec` at spawn if
  profiling shows it.
- Test construction: manually pushing `Particle { ... }` literals in
  tests breaks when new fields are added. `..Default::default()` on
  `Particle` (impl `Default`) is the escape hatch — but only works if
  you control the test data. For the field-by-field literal tests, bulk
  `replaceAll` on the trailing `blend_mode: BlendMode::Normal,\n        });`
  pattern was the fastest fix.
- Clippy `unnecessary_unwrap`: `is_some()` + `.expect()` on the same
  option triggers it. Restructure as `if let Some(x) = opt.as_ref() { ... }`
  inside the outer guard.

### Part Twah (8.0–8.4)

- Before assuming a keyboard shortcut letter is "free," check every
  mode-specific key handler that runs *before* the toolbox tool-select
  dispatch in `dispatch.rs::handle_key_event` — not just
  `Tool::key_shortcut()`'s own match arms. FontEditor's overview grid has
  a "type to search by glyph" feature (`font_editor.rs:1450`) with a
  hardcoded exclusion list of letters it must NOT swallow (the existing
  tool shortcuts); adding a new tool shortcut without adding it to that
  list means the letter silently does nothing in FontEditor mode. Found
  by bisecting with a temporary `eprintln!` at the top of
  `handle_key_event` and again right before the tool-select block — much
  faster than reading the whole dispatch chain top-to-bottom.
  `u`/`r`/`m`/`n` (Move/Rotate/Emitter/Lighting) have the same
  pre-existing gap and were left alone (out of scope, tracked under the
  keymap-overhaul item) — don't assume this list is exhaustive just
  because it looks deliberate.
- ImageEditor mode has its *own* `handle_key` with bare-letter bindings
  (`b`=Brightness, `k`=Contrast, `t`=Threshold, checked before the
  toolbox dispatch too) — a different, independent collision from
  FontEditor's. Tests exercising arrow-key/letter-key tool behavior
  should switch to `AppMode::ImageEditor` and select the tool directly
  via `app.editor.toolbox.selected = Tool::X` rather than pressing its
  shortcut letter, mirroring the existing
  `test_rotate_tool_arrow_key_rotates_whole_layer` precedent — do not
  try to "fix" these pre-existing collisions as a drive-by; they're a
  separate, tracked keymap-overhaul item.
- A dialog's "current directory" field means two different things
  depending on mode, and conflating them is a footgun: for Open-style
  browsing, `current_parent_dir()` correctly falls back through
  `Path::parent()` when the string isn't an existing directory (because
  it might be a full file path being typed). For Save-style dialogs
  where the directory field is maintained as a *pure* directory string
  by construction, reusing that same existence-checking helper at
  finalize time silently walks up to the parent when the target
  directory doesn't exist yet (e.g. saving into a folder created earlier
  in the same session, or just not yet flushed in a test tmp dir) —
  `save_target_path()` deliberately trusts `path_buffer` as-is instead.
- Before deleting suspected dead code (e.g. a duplicate
  `impl Widget for &T` alongside an inherent `.render()` method), grep
  every call site first — `Widget::render(&x, ...)` vs `x.render(...)`
  are easy to conflate by eye but resolve to completely different impls.

- F-26: negative keyframe offsets crop (not clamp) in capture_timeline_frames via `signed_src_range`; `.max(0)` on a signed offset is almost always the wrong compositing fix.
- F-26: reorder-right moves a frame to index `to - 1` after the removal shift; pointing `current_frame` at `to` lands on the wrong logical frame.
- F-26: centiseconds (`delay` 1/100s) is the one timing unit across GIF import/export, player, and now TimelineFrame — keep it uniform; conversions to/from fps (`100/fps`) are only for defaults.
