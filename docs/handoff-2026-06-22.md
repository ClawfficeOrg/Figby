# Figby Handoff — 2026-06-22 (v6 hardening session)

## Git state

Branch: `release/6.6` (clean, all tests pass, clippy clean)

## Completed tasks this session

| Task | Description |
|------|-------------|
| 6.7.3 | Unicode panic verified fixed by 6.5.1; added `test_text_tool_unicode_no_panic` |
| 6.7.2 | Quit-confirm dialog: `trigger_quit()` / `quit_confirm_dialog` flag / overlay |
| 6.8.5 | Built-in palettes (Grayscale/Primary/Warm/Cool) in `palette_import::builtin_palettes()` |
| 6.8.7 | Keybinds popup scrollable (↑↓/PgUp/PgDn/q); 20+ missing keybinds added |
| 6.9.3 | Layers menu + Animation menu added to menu bar with full action handlers |
| 6.6.1g | `handle_font_editor_key()` extracted from `handle_key_event` |
| 6.6.1h | `handle_image_editor_key()` extracted from `handle_key_event` |
| 6.9.6 | Visual divider: `toolbox_list_borders()` → `Borders::ALL`; `toolbox_h` +1 to preserve 12-tool inner height |
| 6.8.6 | Lighting tool placeholder: `Tool::Lighting` variant + "Press G to open lighting editor" brush panel |
| 6.8.8 | Timeline scrollable (cached_max_vis_frames); layer panel scrollable (`render_with_stack` → `&mut self`, two-pass) |
| 6.8.3 | Canvas resize: Image → Resize Canvas menu action → opens settings dialog pre-filled with current canvas size |
| 6.8.1 | Open image file dialog: `FileOpsMode::OpenImage` + `enter_open_image()` + `perform_open_image()` + welcome screen 'I' key |
| 6.8.4 | Palette editor: add/delete/edit color, inline hex/name editing, View menu entry, keymap doc |
| 6.8.2 | New image dialog: canvas size + palette selection |
| **6.9.1** | **Layer panel: 2-row icon-based layout** |
| **6.9.2** | **Layer drag handle reorder (mouse + Shift+↑/↓)** |
| **6.9.4** | **Tool options moved from left toolbox to right sidebar** |

## Files changed this session (new tasks bolded)

- `figby-rs/src/tui/mod.rs` — all the above handlers; `perform_open_image()`; `handle_image_editor_key` intercepts 'o' → `enter_open_image()`; `WelcomeAction::ImageOpen` dispatch; `ViewPaletteEditor` action handler; **layer panel mouse handler**; **removed toolbox_brush rendering**
- `figby-rs/src/tui/layers.rs` — **2-row icon-based layout; drag handle reorder (`drag_state`, `layer_at_pos`, `handle_mouse`); Shift+Up/Shift+Down; click-to-select**
- `figby-rs/src/tui/layout.rs` — **removed `TOOLBOX_BRUSH_HEIGHT`, `toolbox_brush_borders()`, `toolbox_brush` from `FrameLayout`**
- `figby-rs/src/tui/overlays.rs` — quit-confirm dialog render, scrollable keybinds overlay
- `figby-rs/src/tui/keymap.rs` — added `LayerPanel` / `TextTool` scopes; 20+ missing bindings; `Ctrl+Shift+P` keybind doc
- `figby-rs/src/tui/menu.rs` — Image menu + `ImageResizeCanvas`; Layers + Animation menus; `Tool::Lighting`; `ViewPaletteEditor` menu action
- `figby-rs/src/tui/palette_editor.rs` — add/delete/edit color operations, inline hex/name editing, palette rename, new PanelMode variants
- `figby-rs/src/palette_import.rs` — `builtin_palettes()` + test
- `figby-rs/src/tui/tools/text.rs` — `test_text_tool_unicode_no_panic`
- `figby-rs/src/tui/toolbox.rs` — `Tool::Lighting` variant (display_name "Lg", key 'n', icon "tool_lighting")
- `figby-rs/src/tui/timeline.rs` — `cached_max_vis_frames` field; updated in `StatefulWidget::render`
- `figby-rs/src/tui/side_panel.rs` — `layer_panel: Option<&mut LayerPanel>` parameter
- `figby-rs/src/tui/file_ops.rs` — `FileOpsMode::OpenImage`; `enter_open_image()`; `handle_key_open_image()`; `render_open_image()`; Widget impl arm for OpenImage
- `figby-rs/src/tui/welcome.rs` — `WelcomeAction::ImageOpen`; 'I' key; `IMAGE_ACTIONS` 6th entry; `image_action_for(5)`
- `figby-rs/tests/tui.rs` — **updated `test_brush_render_contains_shape_name` to open side panel Props tab**
- `docs/todo-v6.md` — **all tasks now checked off (v6 todo complete)**

## Current test state

1256 passing, 7 ignored. Clippy clean. cargo fmt applied.

## todo-v6.md status

**ALL TASKS COMPLETE. v6 hardening phase is done.**

## Recommended next steps

The v6 hardening milestone is complete. Next work could be:

1. **Release v6.0.3** — tag and push. All tasks done, tests green, clippy clean.
2. **Deferred items** from todo-v6.md "Deferred to post-v6" section:
   - Color-depth fallback (C1)
   - Reduced-motion `--no-anim` (C2)
   - Panic-hook terminal restore + autosave (C3)
   - Performance optimizations (O1/O2/O3)
   - New export formats (SVG/asciinema/sixel)
   - Template starter library
   - 3rd-party crate extraction (ratatui-paint-canvas etc.)
   - Release tooling (cargo-dist/release-plz/VHS)
   - Onboarding (?-help, which-key, tutorial)
   - DESIGN.md + zoid-ui-kit token alignment
   - Figby → rename/de-brand (copyrighted name)
3. **New feature work** — e.g. multi-touch gestures, layer blend modes UI polish, etc.

## Key decisions / non-obvious choices

- `quit_confirm_dialog Y` triggers `start_save()` (async); `quit_after_save` flag checked in `AsyncResult::SaveComplete` handler. On save failure, `quit_after_save` cleared (no quit).
- Built-in palettes load into `palette_editor.swatches` + open palette editor panel (via View menu, or selected in new-image dialog).
- Font/image editor key dispatch left as thin wrapper on `TuiApp` (not moved to `FontEditor`/`ImageEditor` structs) because both need `EditorState` mutation (`sync_font_char_to_canvas`, `undo.clear`) that isn't accessible from within those structs.
- `keybindings_scroll` resets to 0 on close.
- `AnimFrameAdd` menu handler uses hardcoded `12×6` thumbnail (matches existing code pattern).
- `6.9.6` separator: `toolbox_list_borders()` changed to `Borders::ALL`. `toolbox_h` bumped from `+1` to `+2` (12 tools × 1 row each + 2 border rows = 14 for list, preserves all 12 tools visible). Brush panel unchanged (LEFT|RIGHT|BOTTOM, no TOP).
- `6.8.1` file dialog: `Widget for &FileOpsDialog` impl renders to `&mut Buffer` directly (no Frame). OpenImage arm duplicates the render logic inline using `Widget::render(...)` calls — same pattern as ImportGif arm. `perform_open_image()` calls `image_editor.load_from_path()` + `sync_image_to_canvas()`.
- `handle_image_editor_key` intercepts 'o'/'O' before `image_editor.handle_key()` to redirect to file dialog; the old raw `entering_path` flow in `ImageEditor` is now bypassed for 'o'.
- Layer panel 2-row display: `display_row += 2` per layer, scroll targets name row (first of pair). If only name row fits, row 2 is skipped.
- Drag reorder state: `Option<(from, to)>` in `LayerPanel`. `layer_at_pos()` reverse-walks layers matching render order.
- Tool options removed from left toolbox column: `TOOLBOX_BRUSH_HEIGHT` removed, `toolbox_brush` field deleted from `FrameLayout`, brush info now only in right sidebar Props tab.
