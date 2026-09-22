# Figby v8 — Part Twah

Milestone goal: clear every unchecked finding in the "Part Twah" section of
`docs/manual-testing-v7.md` (lines 34-41). Pt deux (the previous unchecked
section) is done; this is the next backlog batch. Two items in this batch —
the multi-document tab strip and the figmap file format — are architectural,
not polish, so this milestone is ordered to land small independent wins
first, the document-model foundation next as its own phase, and the format
work last since it depends on that foundation.

Source: `docs/manual-testing-v7.md` lines 34-41.

**Fix order is intentional.** Confirm `cargo build`, `cargo test`, `cargo
clippy`, `cargo fmt` clean after every task (per CLAUDE.md's after-every-task
checklist) — don't batch multiple tasks into one unverified pass.

---

## Phase 8.0 — Quick, independent fixes

- [x] `8.0.1` Default brush size → 1 (manual-note #4)
  - **Touches:** `figby-rs/src/tui/brush.rs` — `BrushState::new()` (`:89`)
    `size: 3` → `size: 1`; update `test_brush_default_size` (`:385-386`) to
    expect `1`. `MIN_SIZE = 1` already permits this; leave the config
    override path (`app_state.rs:1061-1073`) alone.
  - **Difficulty:** Low

- [x] `8.0.2` Space starts/stops playback instead of Enter (manual-note #2)
  - **Goal:** `tui/dispatch.rs:1424` triggers in-canvas playback on
    `KeyCode::Enter`; Space is already bound to "paint" for Brush/Eraser/
    Line/Fill/Spray at `dispatch.rs:~1747` and `app_state.rs:549`, so a naive
    swap would make Space always start playback instead of painting whenever
    the timeline has frames.
  - **Touches:** `figby-rs/src/tui/player.rs:239-242` — delete the redundant
    `KeyCode::Enter => { self.play(); ... }` arm (Space already calls
    `toggle_play()` there). `figby-rs/src/tui/dispatch.rs:1424` — change the
    playback trigger to `KeyCode::Char(' ')`, gated on the active tool NOT
    being a paint tool. `figby-rs/src/tui/toolbox.rs` — extract the existing
    `Tool::Brush | Tool::Eraser | Tool::Line | Tool::Fill | Tool::Spray`
    match into a shared `fn is_paint_tool(tool: Tool) -> bool`, used by both
    the new playback gate and the existing paint-dispatch site.
  - **Difficulty:** Low

---

## Phase 8.1 — File dialog overhaul (manual-note #3)

- [x] `8.1.1` Filesystem `..` entry + Left/Right navigation
  - **Touches:** `figby-rs/src/tui/file_ops.rs` — `refresh_directory`'s
    normal filesystem branch (`:260-313`) never inserts `..` (only the
    zip-browsing branch does, `:250`); add it, skipping at filesystem root.
    Add `KeyCode::Right` → reuse existing `select_entry()` (`:347-404`).
    Add `KeyCode::Left` → new `fn go_to_parent(&mut self)` modeled on the
    existing zip `..` handling (`:350-363`), generalized to plain
    directories. Keep Tab working too (lower regression risk than removing
    it).
  - **Difficulty:** Medium

- [x] `8.1.2` Enter only activates selectable entries
  - **Touches:** `figby-rs/src/tui/file_ops.rs` — add shared
    `fn entry_is_selectable(&self, entry: &str) -> bool` consolidating the
    extension-matching logic currently duplicated across
    `refresh_directory`'s filters and each `Enter` arm; gate the
    non-navigation branch of every `Enter` handler on it.
  - **Difficulty:** Low

- [x] `8.1.3` Mouse support in file dialogs
  - **Touches:** `figby-rs/src/tui/file_ops.rs` — add `entry_rects`
    (parallel to `directory_entries`, populated during render), following
    the existing hit-testing idiom (`DialogState.quit_confirm_buttons`,
    `AnimationState.transport_rects`). Add
    `FileOpsDialog::handle_mouse(col, row, kind) -> bool` (click
    selects/activates, wheel scrolls). `figby-rs/src/tui/dispatch.rs:381-383`
    currently unconditionally drops mouse events while any file dialog is
    open — wire the new handler in instead.
  - **Difficulty:** Medium

- [x] `8.1.4` Show zip bundles in the font-import dialog
  - **Goal:** "I thought we added zip support" — it exists (`Open` mode
    already allows `.zip`, `file_ops.rs:304`) but `ImportFont` mode's filter
    (`:300-301`) only allows dirs/`.ttf`/`.otf`, so FIGlet zip bundles
    (`.flf`/`.tlf` inside a `.zip`) aren't reachable from that flow.
  - **Touches:** `figby-rs/src/tui/file_ops.rs` — add `.zip` to
    `ImportFont`'s visibility filter; once inside a zip in this mode, reuse
    the exact same listing/selection path as `Open` mode's zip browsing
    (`OpenTarget::ZipEntry`, `dispatch.rs:2075-2091`). True `.ttf`-inside-
    `.zip` support (a different, unrelated zip convention) is out of scope —
    flag if that turns out to be what's actually wanted.
  - **Difficulty:** Medium

- [x] `8.1.5` (opportunistic) Dedup the five near-identical `handle_key_*` dialog handlers
  - **Goal:** `handle_key_open`/`handle_key_import_font`/
    `handle_key_import_gif`/`handle_key_open_image`/`handle_key_save_as`
    (`file_ops.rs:417-819`) are ~90% duplicated. Since 8.1.1-8.1.4 already
    touch all of them, collapse to one `handle_key` parameterized by a small
    filter enum/closure shared with `refresh_directory`'s visibility filter.
  - **Touches:** `figby-rs/src/tui/file_ops.rs`. Before editing render
    logic, confirm whether the second `impl Widget for &FileOpsDialog`
    (`:1331+`) is dead code or a live duplicate of `render_open`.
  - **Difficulty:** Medium

---

## Phase 8.2 — Braille sub-cell brush tool (manual-note #7)

- [x] `8.2.1` Add `Tool::Braille` as a standalone toolbox entry
  - **Touches:** `figby-rs/src/tui/toolbox.rs` — `Tool::Braille` variant +
    `display_name()`/`key_shortcut()` (`k` is free)/`icon_key()` (use the
    literal glyph `⣿`, no icon asset needed)/`Tool::all()` (`:30-128`).
  - **Difficulty:** Low

- [x] `8.2.2` Braille dot-toggle logic + keyboard sub-cursor
  - **Goal:** Terminal mice report whole-cell coordinates only, not sub-cell
    pixel position, so true sub-cell mouse painting isn't achievable without
    separate SGR-pixel terminal work. Scope v1 as a keyboard sub-cursor:
    arrow keys move a 2×4 dot-resolution cursor within the current cell,
    Space/Enter toggles the dot under it; mouse clicks select the whole cell
    only. Call this reduced scope out explicitly when shipped.
  - **Touches:** new `figby-rs/src/tui/tools/braille.rs` (`BrailleBrushState`
    on `EditorState`, next to `brush: BrushState`). Reuse
    `image_input.rs`'s existing bit-packing (`pixels_to_braille_char`,
    `:396-419`, `BRAILLE_BASE` + `position_bits`) via a new shared
    `pub fn toggle_braille_dot(ch: char, dx: u8, dy: u8) -> char` in
    `image_input.rs`, called by both the whole-image converter and this
    tool. Wire into paint-dispatch and mouse-paint match arms in
    `dispatch.rs` (compiler will flag any exhaustive `match Tool` missing
    the new arm).
  - **Difficulty:** Medium

- [x] `8.2.3` Props panel for the braille tool
  - **Touches:** `figby-rs/src/tui/props_panel.rs` — extend `PropAction`
    (`:5-39`) with radius up/down, following the existing per-tool props
    pattern, so this tool isn't another stub panel.
  - **Difficulty:** Low

---

## Phase 8.3 — Font creation dialog parity with CLI (manual-note #1)

- [x] `8.3.1` Charset field on "New Font from System"
  - **Goal:** CLI already supports this (`main.rs:279-298`
    `--font-size`/`--create-font-charset`, resolved via
    `font_gen::resolve_charset()` at `font_gen.rs:346-365` — names:
    `default/slight/smooth/block/blocks/box/braille/ogham/dithered/
    geometric/deluxe/full` or a custom comma list). The TUI's
    `SystemFontPickerDialog` (`tui/dialogs/system_font.rs`) has
    `Field::List`/`Field::Size` (`:14-17`) but charset is hardcoded to
    `"smooth"` at the call site (`dispatch.rs:2136-2138`).
  - **Touches:** `figby-rs/src/tui/dialogs/system_font.rs` — add
    `Field::Charset` + `charset_index`/`result_charset`, include in the
    Tab-cycle and render. `figby-rs/src/tui/dispatch.rs:2136-2138` — read
    the result instead of the literal.
  - **Difficulty:** Medium

- [x] `8.3.2` Options dialog for "New Font from File" (currently has none)
  - **Goal:** `perform_import_font` (`dispatch.rs:2095-2115`) hardcodes both
    size (`12.0`) and charset (`rascii_art::charsets::DEFAULT`) today — no
    dialog step exists at all.
  - **Touches:** new `figby-rs/src/tui/dialogs/font_charset.rs` (or extend
    `system_font.rs` for a shared component) modeled on `NewImageDialog`'s
    `Field`/buffer/`confirmed` pattern (`new_image.rs:14-32`); shown after
    file selection, before conversion runs. Keep both flows' charset
    list/cycling UX identical. `figby-rs/src/tui/dispatch.rs` —
    `perform_import_font` reads size/charset from the new dialog's result.
  - **Difficulty:** Medium

---

## Phase 8.4 — Save dialog filename field (manual-note #5)

- [x] `8.4.1` Split the save dialog's path field into directory + filename
  - **Goal:** `enter_save_as` (`file_ops.rs:208-217`) seeds one
    `path_buffer` with the whole path (dir+name together), so browsing and
    typing fight over the same field.
  - **Touches:** `figby-rs/src/tui/file_ops.rs` — add `filename_buffer`
    (name only), `path_buffer` becomes directory-only.
    `handle_key_save_as` (`:776-819`): typed chars/Backspace go to
    `filename_buffer`; Up/Down/Tab only navigate/select directories; Enter
    combines dir + filename (`.flf` auto-appended if missing).
    `render_save_as` (`:1230-1319`): add a second labeled field with a
    focus indicator (reuse `system_font.rs`'s `Field::List`/`Field::Size`
    toggle idiom for what's focused).
  - **Difficulty:** Medium

- [x] `8.4.2` Default filename from the current font's name
  - **Touches:** `figby-rs/src/tui/dispatch.rs` — default
    `filename_buffer` to `"{font_storage_name}.flf"`
    (`font_editor.rs:235`, already used for the tab/window title), falling
    back to `"untitled"` when empty (mirrors `selected_path()`'s existing
    fallback, `file_ops.rs:332-345`). Companion fix: `perform_import_font`'s
    success path doesn't currently set `font_storage_name` from the
    imported file's stem — fix so this default works after a TTF/OTF import
    too.
  - **Difficulty:** Low

---

## Phase 8.5 — Multi-document tab strip (manual-note #8, architectural)

> `TuiApp` already keeps `editor: EditorState`, `animation: AnimationState`,
> `lighting: LightingState` as single named fields (`app_state.rs:1040-1053`),
> referenced ~680 times total across the codebase (`self.editor.` 559×,
> `self.animation.` 96×, `self.lighting.` 25×). A naive `Vec<Document>` +
> index-everywhere refactor would touch all of them. Use a hot-swap shim
> instead of an index-everywhere rewrite.

- [x] `8.5.1` Introduce `Document`/`DocumentKind` + hot-swap on `TuiApp` — SHIPPED
  - **Goal:** Keep `editor`/`animation`/`lighting` as real fields on
    `TuiApp` (so existing call sites compile unchanged); add
    `documents: Vec<Document>` + `active_doc: usize`. Tab switch =
    `std::mem::swap` between `self.editor` and
    `self.documents[active_doc].editor` (same for `animation`/`lighting`,
    and `self.ui.mode`/`prev_mode`) — legal disjoint-field borrows, same
    idea already used for timeline-frame swap
    (`EditorState::load_timeline_frame`).
    ```rust
    pub struct Document {
        pub kind: DocumentKind,   // Font | Image
        pub title: String,
        pub path: Option<PathBuf>,
        pub unsaved: bool,
        pub mode: AppMode,        // per-doc last-viewed mode
        pub editor: EditorState,
        pub animation: AnimationState,
        pub lighting: LightingState,
    }
    ```
  - **Touches:** new `figby-rs/src/tui/documents.rs`,
    `figby-rs/src/tui/app_state.rs`. `BrushState`/undo history/`LayerStack`
    become per-document automatically as a consequence (expected/correct
    behavior, just unobservable today with a single document).
    `SessionType` (`app_state.rs:31-40`) collapses into `DocumentKind` —
    derive it from the active document instead of tracking separately.
    `SidePanel`/`props_panel`/`palette_editor` stay global UI chrome (not
    per-document) — only their data is per-document via the swap. `mode`/
    `prev_mode` move from `UiState` into the swap set (3 fields, much
    cheaper than moving editor/animation/lighting themselves).
  - **Difficulty:** High

- [x] `8.5.2` Replace the static mode strip with a clickable document tab strip — SHIPPED
  - **Goal:** `tui/mod.rs:210-241` currently builds a hardcoded, 3-entry,
    non-interactive `Tabs` widget (no click handling exists today).
  - **Touches:** `figby-rs/src/tui/mod.rs` — new tab strip (title, kind
    icon, unsaved-dot) reusing `SidePanel`'s recently-fixed
    `tab_rects`/`tab_at_pos` hit-testing pattern (`side_panel.rs:118-142`).
    `figby-rs/src/tui/dispatch.rs:327+` — handle tab-strip clicks early in
    `handle_mouse_event`, same early-return pattern as the menu-bar check.
  - **Difficulty:** Medium

- [x] `8.5.3` Keybindings + menu entries for document tabs — SHIPPED
  - **Touches:** `figby-rs/src/tui/keymap.rs` — next/prev document
    (`Ctrl+PageUp`/`Ctrl+PageDown`, more portable than `Ctrl+Tab`), close
    document (`Ctrl+W`). Menu: File > New Tab (Font) / New Tab (Image) /
    Close Tab.
  - **Difficulty:** Low

- [x] `8.5.4` Test fallout — NO CHANGES NEEDED: no `TuiApp {}` struct
  literals exist (all construction via `TuiApp::new()`); full suite green
  across all 12 test targets, 0 failures.
  - **Touches:** ~5 call sites construct `TuiApp` directly; each needs
    `documents: vec![initial_doc], active_doc: 0` added.
  - **Difficulty:** Low

---

## Phase 8.6 — figmap file format (manual-note #6, architectural, depends on 8.5)

> No existing format to extend — `.ftmp` (`template.rs`) is a text-render-
> macro format, not a raster/layer/animation project format. This is
> genuinely new, and needs `Document`/`DocumentKind` (phase 8.5) to
> serialize into.

- [x] `8.6.1` Enable serde on the raster data types — SHIPPED as planned
  (`4a4bc5d` range): ratatui `serde` cargo feature enabled
  (`figby-rs/Cargo.toml:25`), `CanvasCell` derives
  `Serialize`/`Deserialize` (`lib.rs:5-11`), `Color`/`Style` free via
  ratatui.
  - **Difficulty:** Medium

- [x] `8.6.2` Define the figmap schema + save/load — SHIPPED with deltas
  (`4a4bc5d`, `1c1d61b`, `b7f15ce`): `figby-rs/src/figmap.rs` with JSON +
  `version` field, `save_figmap`/`load_figmap`/`into_runtime`, zero-dim +
  version validation, round-trip tests (image, animation, light
  keyframes). DELTAS vs plan: no `Document` (8.5 not done — serializes
  `LayerStack` + `TimelineState` + lights + palette instead);
  `FigmapKind` dropped (timeline `None` = static image); lights +
  `light_keyframes` + CLI `--play` support are supersets of the plan.
  - **Difficulty:** High

- [x] `8.6.3` Wire figmap into file dialogs + menus — SHIPPED, gap closed
  (`9424224` + follow-up): Open/SaveAs visibility filters list `.figmap`,
  Save/Open route by extension (`dispatch.rs:perform_save`/
  `perform_open_figmap`), CLI plays `.figmap`. GAP FOUND & FIXED: no menu
  entry reached the save path outside font mode (`start_save_as` is
  font-gated) — added File > Save as Figmap (`MenuAction::FileSaveAsFigmap`
  → `start_save_as_figmap`, any mode, seeds `untitled.figmap` via
  `enter_save_as_with_extension`). File > New Animation landed after
  8.5 unblocked it: fresh image tab with the timeline already open.
  - **Difficulty:** Medium

---

## Verification

- Run `cargo build`, `cargo test`, `cargo clippy`, `cargo fmt` after every
  task, not batched across a whole phase.
- 8.0.x: manually exercise in the TUI (`cargo run -- --tui`) — brush size on
  a new canvas; Space starting/stopping playback without breaking paint-on-
  Space.
- 8.1.x: exercise the file dialog with mouse clicks, `..` navigation,
  Left/Right, and a zip containing `.flf`/`.tlf` fonts from the ImportFont
  flow specifically.
- 8.2.x: exercise the braille tool's keyboard sub-cursor painting; confirm
  no regression to other tools' Space/Enter paint behavior (shared dispatch
  code).
- 8.3.x/8.4.x: exercise both font-creation dialogs end-to-end (system font +
  file import) and the save dialog's filename field + default name.
- 8.5.x: open two documents (one font, one image), switch tabs, confirm
  each keeps independent undo history, brush state, and mode; confirm
  existing single-document flows are unaffected.
- 8.6.x: round-trip save→load a `.figmap` file for both a static image and
  an animation, confirm layers/groups/links/timeline frames are
  byte-identical after reload.
