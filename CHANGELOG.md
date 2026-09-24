# Changelog
## [6.0.46] - 2026-09-24

### Added
- TUI draw-along recording: `assets/e2e-art/recordings/tui-draw.cast` +
  `.gif` — scripted hand-drawing in the real TUI (brush stamps, eraser
  carve, fill, undo/redo, PNG export), driven by
  `scripts/demo-tui-draw.sh` + `scripts/make-tui-draw-cast.py`.
- E2E art suite: tribute sprites (plumber/quest/invader), castle layers
  backdrop, animated FIGBY banner finale under `assets/e2e-art/` with
  `scripts/gen-e2e-art.py` generator. Headless contract
  (`figby-rs/tests/e2e_art.rs`, 4 tests) plus non-headless PTY suite
  (`scripts/e2e-art-tmux.sh`, 19 checks: `--play` still display, real
  Open dialog, tool keys, timeline/export dialogs, `--play --loop`).
  Narrated demo recording: `assets/e2e-art/recordings/e2e-art.cast` +
  `.gif` (`scripts/demo-e2e-art.sh`, `scripts/preview-figmap.py`).
- Manual E2E plan: `docs/e2e-tui-art-tests.md` (tools → layers →
  animation → lighting/banner → file-ops sweep, issues #1–11 audited).
- `--play` on a static figmap (no timeline frames) now displays the
  still inline via stdout (no screen clear, no raw mode) instead of
  erroring; animated figmaps play as before.

### Fixed
- Text tool no longer traps all keys: `Esc` exits to Brush (was: every
  keystroke swallowed with no visible editing state); Alt+arrows cycle
  the drawer without re-arming the capture.
- Export dialog select-all: first key replaces the `export.png`
  suggestion (was: appended, `export.png/m/hand-ain.ng`); pasted paths
  replace wholesale via `handle_paste_event`.
- FontEditor Overview search no longer swallows `m`/`n`: Emitter and
  Lighting tool keys reach the toolbox catch-all (was: glyph search
  started instead of selecting the tool).
- Open dialog: typed/pasted full paths now open on Enter (FontEditor
  Overview search no longer swallows `/`, `.`, letters; dialog dispatches
  first when open). Recent-file digits 1-9 only fire on an empty Path;
  stale (deleted) recents filtered at dialog entry.
- Export dialog: Esc on an empty path closes without firing a doomed
  async "no path specified" export that reopened the dialog.
- Figmap open now recomposites layers to the canvas (was blank with only
  layer names visible).

## [6.0.41] - 2026-09-22

### Added
- File > New Animation: fresh image tab with the timeline already open
  (8.6.3 leftover, unblocked by the document model).

### Fixed
- Canvas content verified to survive a tab switch round-trip
  (regression test on the hot-swap).

## [6.0.40] - 2026-09-22

### Added (todo-v8 8.5 — document tabs, 8.6.3 gap)
- Multi-document tabs: the static mode strip is now one tab per open
  document (kind icon + title + unsaved dot). Backed by a hot-swapped
  `Document` model — each tab keeps independent undo, brush, layers,
  timeline, and lighting state.
- `Ctrl+PageUp`/`Ctrl+PageDown` cycle tabs, `Ctrl+W` closes (unsaved tabs
  prompt through the existing Save/Discard/Cancel dialog). File menu
  gains New Tab (Font), New Tab (Image), Close Tab, and Save as Figmap
  (previously unreachable outside font mode).

### Fixed
- Font overview search no longer swallows Ctrl/Alt combos (`Ctrl+W` typed
  `w` into search instead of closing the tab).

## [6.0.39] - 2026-09-22

### Fixed
- `--play-inline` now renders at the cursor position instead of the
  top-left: the old code reused the fullscreen renderer's absolute CUP
  sequences, and smaller frames ghosted over larger ones. New
  `render_frame_inline` uses DECSC save/restore plus cursor-relative moves
  only, and writes the full animation bounding box (spaces included) so
  frames fully erase each other. Cursor parks below the animation on exit.

### Added
- `--play-timeline` flag (default off): shows a one-row cursor-relative
  playback timeline under `--play --play-inline` animations. Fullscreen
  playback keeps its always-on progress bar.

## [6.0.38] - 2026-08-26

Remaining verified GPT-review bugs closed: F-12, F-15, F-17.

### Fixed
- **F-12** — new-image and GIF-import dialogs can no longer allocate
  ~4.3-billion-cell canvases from the 65,535 u16 cap: both `confirm()`
  paths now reject dimensions beyond a shared `canvas::MAX_CANVAS_CELLS`
  (1,000,000) budget, and `apply_settings` refuses an oversized resize.
  Template rendering already had its own zero-dim/checked-arithmetic guard.
- **F-15** — WASM editor cursor is a character index, not a byte offset
  (already fixed in 2a55c2f); verified with multibyte edit tests covering
  é, CJK, emoji, and combining marks.
- **F-17** — palette hex parsing already validates ASCII hex digits before
  slicing (46ca98b); added proptest no-panic fuzz for every palette format
  (Paletty/WezTerm/WindowsTerminal/ASE) plus `parse_hex_rgb`, and fixed the
  same byte-slice-on-multibyte pattern in the rascii-import preview
  truncation (`&s[..n]` → char-boundary truncation).

## [6.0.37] - 2026-08-26

### Changed
- README notes that Figby is feature-complete / bug-chasing and points ralph
  skill/loop discussions to the `~/Zoidot` repo; the ralph sandboxing policy
  is linked from the README.

## [6.0.36] - 2026-08-26

Phase A of F-20 (ralph sandboxing), per the approved policy at
`docs/f-20-ralph-sandbox-policy.md` (GPT review F-20, High). The policy's
design stance: never rely on LLM judgment as a security boundary — enforce
with process and environment, so even a fully compromised agent prompt
cannot reach git write ops, credentials, or network exfiltration.

### Changed
- **F-20 R1/R2** — sandboxed agents: `invoke_agent` refuses `claude`/`copilot`
  (which would need `--dangerously-skip-permissions` / `--allow-all-tools`)
  unless `RALPH_ALLOW_UNSANDBOXED_CLI=1`; the opencode path now injects a
  generated permissions profile (file read/write/edit + the four cargo verify
  commands allowed; `git`, `gh`, `curl`, `ssh`, `sudo`, package managers,
  credential-file reads, `webfetch`/`websearch` denied; `external_directory`
  denied) and scrubs credential-shaped environment variables (provider model
  keys are the documented retained minimum needed to run the model).
- **F-20 R4** — explicit-path staging: `stage_explicit` replaces every
  `git add -A`; it stages exactly the working-tree delta and refuses
  credential/`.git` paths.
- **F-20 R6** — real gates: `.githooks/pre-commit` (fmt/clippy/build/test,
  docs-only fast path) plus `run_gates()` invoked explicitly before every
  commit; `core.hooksPath` is auto-set to `.githooks`. The false claim that a
  pre-commit hook ran the gates is removed from `ralph.sh` prompts and
  `skills/ralph.md`.

## [6.0.35] - 2026-08-26

Phase 3 of the GPT-review remediation (`docs/GPT-review-8-20-26.md`, tracked
in `docs/review-remediation-checklist.md`) — distribution, claims, CI, and
developer commands (F-27, F-28, F-29, F-30).

### Changed
- **F-27** Qualified README parity claims: full-flag-parity and
  "bit-identical output" claims now say "coverage of every implemented flag"
  and "byte-identical across active differential tests"; a new
  "Known divergences from C FIGlet" section documents the five failing
  scenarios (tests 21, 23, 26, 35, 37).
- **F-27** `-F` is now documented (README + `--help`) as removed, matching C
  FIGlet 2.2.5 (which dropped it for `figlist`); the binary's error message
  says so.
- **F-27** The C expected-output generator and the Rust differential tests
  are now driven by one authoritative table
  (`figby-rs/tests/cparity/scenarios.tsv`); a sync-guard test fails if the
  table and the `#[ignore]` statuses drift apart. Two paragraph tests
  (45, 54) now pass and are enabled.
- **F-28** Owner URLs and Cargo metadata point at `ClawfficeOrg/Figby`
  (the canonical owner) instead of `DoseOfGose`; README Project Status
  clarifies that the `6.0.x` package version is canonical while `7.0.x` git
  tags are milestone markers; the milestone index gains v8.
- **F-28** The legacy `snapcraft.yaml` (packaged the C `figlet`, not the
  Rust app, on EOL `core18`) is quarantined under `packaging/snap-legacy/`
  with a README; Snap shipping is declared not-yet-available.
- **F-29** CI expanded to a real matrix: Linux fmt/clippy/test, strict
  rustdoc, `cargo package` + tag-gated publish dry-run, MSRV check,
  Windows/macOS build + focused tests, WASM check + clippy, and a `cargo
  audit` gate. Action versions are pinned to release tags, and permissions
  are least-privilege.
- **F-29** Fixed strict-rustdoc private-link errors
  (`gif_import.rs` `MAX_TOTAL_CELLS`, `tui/mod.rs` module links).
- **F-30** All developer commands standardized on explicit
  `--manifest-path figby-rs/Cargo.toml` in AGENTS.md, README, CLAUDE.md, and
  `scripts/ralph.sh` (the repo root has no `Cargo.toml`).

## [6.0.34] - 2026-08-26

Phase 3 of the GPT-review remediation (`docs/GPT-review-8-20-26.md`, tracked
in `docs/review-remediation-checklist.md`) — animation timing and transform
behavior (F-26).

### Changed
- **F-26** Per-frame hold time now lives inside `TimelineFrame::delay`
  (centiseconds), so insert/delete/reorder/duplicate carry timing with the
  content instead of detaching it from a side-channel delays vector; export
  timing and inline/standalone playback all derive from the frames.

### Fixed
- **F-26** Signed keyframe offsets: the shared timeline compositor now crops
  negative-position layers against the canvas instead of clamping the offset
  to zero, so left/up keyframe motion renders correctly.
- **F-26** Frame reorder no longer leaves `current_frame` pointing at the
  wrong logical frame when the current frame is moved right.
- **F-26** Production ANSI animation export now receives all timeline frames
  (via the same shared compositor as GIF/APNG) instead of just the current
  frame.
- **F-26** `--play`, in-canvas playback, and the fullscreen preview honor
  per-frame GIF timing instead of flattening it to a uniform FPS.
- **F-26** The export dialog's animation preview is scheduler-driven (elapsed
  time in the event loop, per-frame delays) and can no longer stall when
  redraws are suppressed.

## [6.0.33] - 2026-08-25

Phase 1 of the GPT-review remediation (`docs/GPT-review-8-20-26.md`,
tracked in `docs/review-remediation-checklist.md`) — core document
correctness: F-04 through F-09 all closed.

### Fixed
- **F-04** Production GIF/APNG export now uses the shared snapshot-aware
  timeline compositor instead of re-deriving every frame from the live
  layer stack; end-to-end test decodes the GIF and asserts distinct
  frame pixels.
- **F-05** Single timeline authority: `TimelineFrame::document_state`
  (per-layer buffers) owns pixels, keyframes own transforms applied on
  top at composite time; navigation commits/restores every layer so
  multilayer documents survive frame changes losslessly.
- **F-06** Rendering is read-only against document state — the
  image-editor cache sync no longer runs in the render path and cannot
  overwrite paint operations; GIF/RASCII imports clear the stale cache.
- **F-07** Revision-aware saves: edits made while an async save runs
  keep the document dirty and cancel deferred quit.
- **F-08** Uniform dirty tracking (font-editor mutation epoch) plus
  guarded Open/New transitions sharing one save/discard/cancel dialog;
  atomic document reset covers undo/selection/timeline/export state;
  Save option hidden where no save path exists.
- **F-09** Layer-aware undo: entries carry their target layer and
  restore into it (with lock checks); per-layer locks are enforced on
  every mutating path (mouse tools, text placement, keyboard paint,
  braille dots, cut/paste).
- **F-16** ISO-2022 and HZ parsers converted from recursion to
  iterative state machines; adversarial tests drive 500k-element
  control/escape runs through both.

### Security
- **F-10** Template images resolve against the template's base dir with
  canonicalization immediately before open — relative symlink escapes
  rejected.
- **F-11** `${VAR}` env expansion in templates is opt-in (default off);
  template image dimensions defaulted when absent and capped at 2000.
- **F-12** Template canvas rejects zero dimensions; padding/margin
  counted via checked arithmetic before allocation.
- **F-13** GIF import streams decode→composite in one pass instead of
  retaining all native-resolution frames; 10k frame-count cap added.
- **F-15** WASM editor cursor is a char index; multibyte editing
  (accents, CJK, emoji, combining marks) no longer panics.
- **F-17** One ASCII-validating `parse_hex_rgb` replaces seven unsafe
  byte-offset slicing sites across palette/theme parsing.

## [6.0.32] - 2026-08-25

### Fixed
- Tests embedded an untracked font (`Lixdu.ttf`), so clippy `--all-targets`
  and `cargo test` failed to compile from a clean checkout (GPT review
  F-01). Replaced with tracked, OFL-licensed Fira Mono fixture under
  `figby-rs/tests/fixtures/`.
- TUI smoke test used an 80-column backend that triggered the status
  bar's item truncation and dropped the FPS label (F-02); widened to 120.
- Terminal-capture player tests panicked in non-tty environments; they
  now skip with a message instead of unwrapping.
- De-flaked ZIP font tests: temp dir names now include a per-process
  atomic counter, preventing collisions under coarse clock granularity.

### Changed / Build
- Embedded assets (theme yaml, icons.yaml, welcome mascot, web fonts)
  moved into `figby-rs/assets/` so the packaged crate compiles;
  `cargo package` now verifies (F-03).
- `Cargo.lock` is tracked and CI runs with `--locked`; toolchain pinned
  via `rust-toolchain.toml` (1.97.1) (F-19).
- CI enforces fixture presence, locked clippy/tests with
  `-D warnings --all-features`, and package verification; workflow has
  explicit least-privilege permissions.

## [6.0.31] - 2026-07-12

### Added
- Save dialog now has a dedicated filename field, separate from the
  directory being browsed. Tab switches focus between Folder and
  Filename; typing only ever edits the filename, so browsing into a
  directory no longer clobbers whatever name you'd already typed.
- Filename defaults to the font's current name (or "untitled") plus
  `.flf` when saving a font that hasn't been saved to disk yet.

### Changed
- Clicking an existing font file in the Save dialog's listing now
  populates the filename field with that name (so you can overwrite it)
  instead of folding it into the directory path.

Part Twah 8.4 (manual-note #5).

## [6.0.30] - 2026-07-12

### Added
- "New Font from System" dialog now has a Charset field (Tab to focus,
  Left/Right to cycle through the 12 presets `font_gen::resolve_charset`
  understands — default/slight/smooth/block/blocks/box/braille/ogham/
  dithered/geometric/deluxe/full), matching the CLI's
  `--create-font-charset` flag. Previously this was hardcoded to
  `"smooth"` with no way to change it from the TUI.
- "New Font from File" (TTF/OTF import) now shows a size + charset
  options dialog after picking a file, before conversion runs. Previously
  this flow had no options step at all — size (12pt) and charset were
  silently hardcoded.

Part Twah 8.3 (manual-note #1).

## [6.0.29] - 2026-07-12

### Added
- New Braille tool (`k`), a standalone toolbox entry for painting Unicode
  braille dots at sub-cell accuracy (2x4 dots per canvas cell). Arrow keys
  move a dot cursor within the current cell, Space/Enter toggles the dot
  under it — mouse can only address whole cells, so v1 is keyboard-driven
  sub-cell precision rather than mouse-driven. Has its own Props panel
  entry showing the current dot position. Reuses the existing braille
  bit-packing convention from the image-to-braille converter
  (`image_input::BRAILLE_BASE`), via a new `toggle_braille_dot()` helper
  that flips one dot without disturbing the others already set in a cell.

### Fixed
- FontEditor's type-to-search-by-glyph feature was swallowing any tool
  shortcut letter not on its hardcoded exclusion list before it could
  reach tool selection — `k` (the new Braille shortcut) hit this; added
  it to the exclusion list alongside the other tool letters.

Part Twah 8.2 (manual-note #7).

## [6.0.28] - 2026-07-12

### Added
- File dialogs (Open/Save/Import Font/Import GIF/Open Image) now support
  mouse: click a row to select/activate it, wheel to scroll the selection.
- Filesystem `..` entry is now offered in every directory listing, not just
  while browsing inside a zip archive — no more deleting typed characters
  to go up a directory.
- Left arrow navigates up a directory (or out of a zip); Right arrow
  descends into a directory/zip, alongside the existing Tab binding.
- Font import (`ImportFont` mode) now shows `.zip` bundles and browses
  into them exactly like the Open dialog, so FIGlet's own `.flf`/`.tlf`
  zip bundle convention is reachable from the "New Font from File" flow,
  not just the generic Open dialog.

### Changed
- Enter now only finalizes a file dialog on an actual selectable target
  for that mode (directory/zip to navigate, or a matching file to open);
  previously navigating with Enter onto a non-matching entry could behave
  inconsistently since dialogs never listed non-matching files but relied
  on ad-hoc per-mode checks.
- Consolidated the file dialogs' five near-duplicate key handlers
  (Open/ImportFont/ImportGif/OpenImage) into one shared implementation,
  and deleted a ~390-line dead second render implementation
  (`impl Widget for &FileOpsDialog`) that was never actually called.

Part Twah 8.1 (manual-note #3).

## [6.0.27] - 2026-07-12

### Changed
- Default brush size is now 1 (was 3) — matches user expectation for a
  precision starting point. Part Twah 8.0.1.
- Playback now starts on Space instead of Enter, matching every other
  animation app; Enter remains the keyboard-paint key for Brush/Eraser/
  Line/Fill/Spray. Space still wins over the Layers panel's own binding
  and over painting whenever the timeline has frames, mirroring Enter's
  old priority exactly. Part Twah 8.0.2.

## [6.0.26] - 2026-07-12

### Changed
- Text tool UX redesign: sidebar text field replaces click-to-type-on-canvas.
  Font selector uses prev/next buttons instead of cycling. Hover mouse over
  canvas for live FIGlet preview; click to place editable text block.
  [Rasterize] button converts text object to layer pixels.
  Closes user-reported text-tool UX issues.

## [6.0.25] - 2026-07-12

### Added
- GIF import sizing dialog: two-phase dialog (file browser → sizing options)
  showing original GIF resolution, editable image width/height, canvas
  width/height, and keep-proportions toggle. Image auto-centers on canvas.
  Closes 7.6.1.
- `GifScaleTarget::Exact(width, height)` variant for aspect-ratio-free
  scaling.
- `probe_gif_dimensions()` for cheap header-only GIF dimension reading.

### Changed
- GIF import now uses user-configurable sizing dialog instead of
  auto-computing scale from terminal viewport. Imported GIF frames are
  centered on the canvas.

## [6.0.24] - 2026-07-11

### Added
- Per-particle keyframe tracks: new `ParticleKeyframe` type + `keyframes`
  field on `Particle` / `ParticleConfig`. `Particle::render_values()`
  interpolates colour/size/opacity linearly between adjacent keyframes
  based on lifetime progress; character picks the nearer endpoint.
  7 tests covering 25%/50%/75% interpolation, character step, empty-track
  fallback, progress clamping, and render-to-canvas integration. Closes 7.5.2.
- On-death secondary emitter: `on_death_count` + `on_death_config` on
  `ParticleConfig`. Primary particles spawn N secondaries at their
  position on death using the sub-config. Secondaries carry
  `is_secondary = true` so bursts are non-recursive. 4 tests.
- `docs/particles-design.md` design sketch covering data model,
  interpolation rules, lifecycle hooks, and deferred work.

### Changed
- `Particle` gains `total_lifetime` (spawn-time snapshot used as the
  interpolation denominator), `keyframes`, and `is_secondary` fields.
  `Particle::Default` implemented. All test literal constructions updated.
- `render_to_canvas` / `bake_to_buffer` now call `render_values()` per
  particle instead of reading static fields directly.

## [6.0.23] - 2026-07-11

### Added
- Particle edge collision response: configurable `EdgeMode` (Bounce/Wrap/Despawn)
  on `ParticleConfig` + 5 tests. Particles now bounce off, wrap around, or
  despawn at canvas edges instead of flying into dead space. Closes 7.5.1.
- Particle layer-cell collision: when `collide_with_layer` is enabled, particles
  reflect off non-blank canvas cells using a 4-neighbor normal + velocity
  reflection. 2 tests.

### Changed
- `ParticleSystem::update()` signature extended with `bounds` and `layer_mask`
  parameters. Emitter config panel gains Edge Mode + Collide w/ Layer fields.

## [6.0.22] - 2026-07-11

### Added
- Lighting help overlay + `Scope::Lighting` keybind section in keymap. One-shot
  keybinding hint shown on entering lighting mode (`light_panel.rs`).
- FIGfont density heightmap: non-space text cells now set `height: Some(255)` so
  the lighting engine receives non-flat normal maps. Lighting mode now produces
  visible shading on text canvases. Closes task 7.4.
- New test `compute_normal_map_non_empty_glyph` asserting Sobel normals tilt
  correctly at raised-block edges.

### Changed
- **Priority swap:** lighting tasks promoted to 7.4 (was 7.5), particle system
  demoted to 7.5 (was 7.4) in `docs/todo-v7.md`.
- `lighting-design.md` status updated from "Deferred to v4.x" to "Partially
  Implemented (FIGfont density path, v7.4)".

## [6.0.21] - 2026-07-11

### Changed
- Split `tui/mod.rs` (5693 LOC) into three topical submodules:
  `app_state.rs` (struct/enum defs + sub-struct impls + `TuiApp::new`),
  `event_loop.rs` (`run`/`handle_event`/tick/async completion), and
  `dispatch.rs` (key/mouse dispatch + all `perform_*`/`start_*` action
  handlers + moved tests). `mod.rs` shrank to 774 LOC (re-exports + render
  pipeline + shared helpers) — under the 1500-LOC target of task 7.3.4.
  No behaviour change; cross-module callers gained `pub(crate)` visibility
  where needed. Banner output byte-identical to system `figlet`.

## [6.0.20] - 2026-07-11

### Fixed
- GIF-imported / 'A'-key captured animation frames showed same picture
  throughout playback and export because `capture_timeline_frames` was
  re-rendering from the live (unchanging) layer stack instead of each
  frame's own `layer_state` raster snapshot. Now reads `layer_state`
  directly when present. Regression test included.

## [6.0.19] - 2026-07-11

### Fixed
- In-canvas animation playback's frame content could stay visually frozen
  on some terminals (reported: Windows Terminal / WSL) even though the
  frame counter and progress bar advanced correctly every tick — the
  same class of ratatui diff-cache staleness already fixed for the
  standalone fullscreen player in 6.0.13, just not previously applied to
  inline playback. Now sets `force_full_redraw` (bypassing the stale
  cell-diff) every time the player's current frame actually advances.

## [6.0.18] - 2026-07-11

### Fixed
- Pressing Enter to start timeline playback silently did nothing (toggled
  the active layer's visibility instead) whenever the side panel was open
  on the Layers tab — a regression exposed by the side panel now
  defaulting to open on wide terminals (6.0.17), since the Layers panel's
  own Enter binding was checked earlier in the key-dispatch chain than
  the "Enter starts playback" check. Moved the playback check ahead of
  the Layers panel dispatch (but still behind every modal dialog/overlay)
  so starting playback always wins.

## [6.0.17] - 2026-07-11

### Added
- The side panel (drawer) now opens by default at startup when the
  terminal is wide enough that opening it still leaves a reasonably
  usable canvas (toolbox width + drawer width + a 60-column canvas
  floor). Narrower terminals keep the previous closed-by-default
  behavior; `?` still toggles it manually either way.

## [6.0.16] - 2026-07-11

### Fixed
- In-canvas animation playback rendered the frame content flush against
  the panel's top-left corner instead of centered like the normal
  (non-playing) canvas, and stranded the progress bar at the bottom of
  the whole panel, disconnected from the visible frame — looked broken
  even though playback was actually advancing correctly. Now centers the
  player's content the same way `EditorState::compute_canvas_rect`
  centers the normal canvas buffer (`AnimationPlayer::content_dimensions`
  + matching centering math in `render_canvas_area`).

## [6.0.15] - 2026-07-11

### Added
- **Layer linking.** New `k`/`K` keybind and toolbar "Link" button pair up
  two layers so toggling visibility or lock on one propagates to the
  other, mirroring the existing layer-group model (`LayerStack::links`,
  `link_layers`/`unlink_layers`/`link_of_layer`/`layers_in_link`).
- **Layers panel toolbar.** Clickable New/Duplicate/Delete/Group/Link
  buttons in the panel's previously-unused header row, backed by the same
  `LayerStack` methods the existing keyboard shortcuts already call.
- **Menu shortcuts.** Every menu item now shows its keyboard shortcut,
  right-aligned in the dropdown; the ~6 that already have a global
  keybinding are derived from `keymap::GLOBAL_DISPATCH` so they can't
  drift out of sync.
- **File > New Image** (`Ctrl+N`) and **File > New Font from
  File/System**. "From system" required a real system-font picker dialog
  (`dialogs::SystemFontPickerDialog`) — `font_gen::list_system_fonts()` /
  `system_font_to_figfont()` existed but were previously only reachable
  from the `--create-font` CLI flag, not the TUI.
- **Timeline click-to-seek and mouse-wheel scroll.** The timeline grid
  never had any mouse handling at all; clicking a frame now seeks to it,
  and scrolling (Shift+scroll for the layer rows) moves the visible
  window.
- **Persistent transport bar.** The timeline's toolbar row is now a
  clickable ▶/⏸/⏹/🔁 button strip (`timeline::render_transport_bar`),
  visible whenever the timeline is shown rather than only during active
  playback, replacing the old static hint-text `Paragraph`.
- Inline playback now defaults its loop state from an imported GIF's own
  `loop_count` instead of always starting unlooped
  (`AnimationPlayer::with_loop`).

### Fixed
- Side panel tabs (Props/Text/Libraries/Effects) were completely
  unreachable: a width-calc bug in the tab bar always rendered blank
  labels, and the Layers panel's Left/Right key handling unconditionally
  swallowed the arrow keys meant to cycle tabs. Also added a real
  Lighting-tool entry to the Props tab (previously fell through to a
  generic keybind list).
- Layers panel's selected-row contrast (bright cyan bg + near-white text)
  fixed by wiring up the theme's already-defined but unused dark-navy
  `LayerTheme`, instead of reusing the menu dropdown's highlight colors.
- Clicks during in-canvas animation playback no longer leak through to
  canvas drawing tools underneath the player.
- Welcome screen's "Create Image" action (`c`) never marked the frame
  dirty and never dismissed the welcome screen, so the New Image dialog
  it opened was invisible — occluded behind the still-showing welcome
  screen with no redraw to reveal it.

## [6.0.14] - 2026-07-08

### Added
- **In-canvas animation playback.** Pressing Enter on the timeline or
  Animation > Play now plays the animation in place inside the canvas
  area — the menu bar, toolbox, palette, timeline, and status bar all stay
  visible and the app never leaves the normal editor Terminal instance.
  Previously both triggered a fullscreen takeover (`play_fullscreen`);
  `Animation > Play` was in fact a dead no-op (it toggled a
  `TimelineState::playing` field nothing ever read).
- New `AnimationState::inline_player: Option<player::AnimationPlayer>` +
  `TuiApp::play_inline()` / `start_inline_playback_from_timeline()`.
  `render_canvas_area()` renders the player into the canvas `Rect` in place
  of normal content while active; `run()`'s loop advances and
  redraw-throttles it using the same pattern already used for the throbber
  spinner (redraw at most once per the animation's own frame interval, not
  a busy loop). A non-looping playthrough auto-pauses on its last frame
  instead of redrawing forever once nothing is changing.
- Playback controls (space/seek/speed/loop-toggle/Esc/q to stop) are
  intercepted directly in `handle_key_event` and reuse
  `AnimationPlayer::handle_key()` — the exact same logic already exercised
  by the fullscreen/CLI players. The canvas border title shows the control
  legend while playing, and the progress bar now shows a 🔁 indicator when
  looping is on (previously invisible even though the underlying toggle
  already existed).
- The old fullscreen player is preserved, unchanged, as a **standalone
  preview** feature — still reachable from the Export dialog's Play
  button (`launch_player_from_export` → renamed `play_standalone_preview`
  for clarity), for previewing how a GIF/APNG export will look played back
  outside the editor.
- Verified end-to-end over a real pty: imported a GIF, played it via
  Enter, confirmed the toolbox/palette/timeline/menu/status bar all stayed
  visible around the playing canvas, toggled loop with `l` (🔁 appeared in
  the progress bar), and dismissed with `q` — canvas correctly reverted to
  normal editing with no freeze (this path never creates a second
  `Terminal` instance, so the redraw-desync bug fixed in 6.0.13 doesn't
  even apply to it).

## [6.0.13] - 2026-07-08

### Fixed
- **In-TUI animation playback left the editor UI frozen/stale after
  exiting.** `player::play_fullscreen()` renders through its own throwaway
  `ratatui::Terminal` (a separate instance from the one `TuiApp::run()`
  owns), so after playback ended, the main terminal's diff-based renderer
  still had a stale internal buffer and only repainted cells that
  genuinely differed from before playback — which was usually nothing,
  since app state hadn't changed. Symptom: screen stuck showing the last
  animation frame, except spots the user actively edited (which *did*
  differ from the cache, so *those* redrew — "I can paint" but nothing
  else updates). Added a `force_full_redraw` flag, set after
  `play_fullscreen()` returns; `run()`'s loop now calls `terminal.clear()`
  before its next `draw()` when set, forcing a full repaint instead of a
  stale diff.
- **`q` did nothing during playback (Esc was the only working exit key).**
  `play_fullscreen`'s (and `play_raw`'s) loops both gate exiting on
  `AnimationPlayer::handle_key()` reporting the key as "consumed", but
  `handle_key` had no match arm for `'q'`/`'Q'` — it fell through to
  `_ => false`, so the exit check could never fire no matter what the
  loop's own keycode comparison said. Added the missing arm (pauses and
  reports consumed, mirroring Esc's behavior minus the seek-to-0). Also
  added `'q'` to `play_raw`'s exit check for consistency (it only
  recognized Esc before). New regression test:
  `test_player_handle_key_q_is_consumed_and_pauses`.
- Both verified together end-to-end over a real pty: imported a GIF into
  the TUI, played it, exited with `q`, and confirmed the full editor UI
  (menu bar, toolbox, palette, timeline, status bar) redrew correctly
  afterward instead of staying frozen.

## [6.0.12] - 2026-07-08

### Fixed
- The TUI's own animated GIF import (File > Import GIF /
  `WelcomeAction::ImageImportGif`) imported at the GIF's native pixel
  resolution (1 pixel = 1 cell), same issue `--play` had before 6.0.10 —
  a real-world GIF either created an unusably huge canvas or got rejected
  outright by the animation import size cap. `perform_import_gif()` now
  computes the actual canvas viewport size (reusing
  `layout::FrameLayout::compute`, the same layout logic already used for
  mouse hit-testing) and scales via `gif_import::import_gif_scaled()` with
  `GifScaleTarget::FitBox`, so the imported canvas fits the visible editor
  instead of overflowing it or being rejected. Verified end-to-end by
  driving the real TUI over a pty: importing a 480x360, 90-frame GIF that
  `import_gif` alone rejects as "too large" now produces a correctly-sized
  (40x15 in a 120x40 terminal) canvas with all frames populated in the
  timeline.

## [6.0.11] - 2026-07-08

### Added
- `figby --play --loop`: repeats the animation indefinitely instead of
  playing once and auto-exiting. Any keypress dismisses it immediately
  (interactive pause/seek/speed controls are bypassed in this mode — there's
  no natural end to wait for, so any key just exits). `player::play_raw()`
  gained a `loop_playback: bool` parameter to support this. Verified
  end-to-end over a real pty: playback wraps past the last frame back to
  frame 1 repeatedly, and exits cleanly on an arbitrary keypress.

## [6.0.10] - 2026-07-08

### Added
- `gif_import::GifScaleTarget` + `import_gif_scaled()`: bilinearly scale
  every composited frame of an imported GIF to a requested size
  (`Native`, `FitWidth(cols)`, or `FitBox(max_cols, max_rows)`, the last
  preserving aspect ratio with the standard 2:1 terminal-cell compensation).
  Compositing (disposal methods, partial-frame positioning) still happens
  at native resolution for correctness; only the stored output is scaled.
- `figby --play` now scales to fit the current terminal by default, or to
  `--play-width <N>` columns explicitly. Because the cumulative memory
  guard now checks the *scaled output* size rather than native size, GIFs
  that previously failed with "too large" at native resolution (e.g. a
  480x360, 90-frame GIF — previously rejected outright) now import and
  play successfully once scaled down. Verified end-to-end over a real pty.
- 7 new tests: `GifScaleTarget::resolve()` for all three variants, a
  downscale-dimensions check, and an oversized-native-GIF-succeeds-when-
  scaled regression test mirroring the real-world case above.

## [6.0.9] - 2026-07-08

### Docs (no functional change)
- Investigated "move animation playback off the TUI event-loop thread"
  (review item 5): closed as not-a-bug rather than implemented. The TUI and
  the animation player both write directly to the same exclusive terminal
  fd via crossterm/ratatui, so a background thread would only reproduce the
  same blocking behavior via a channel, with real risk of the two writers
  racing on stdout. Added a doc comment at the `play_animation()` call site
  (`tui/mod.rs`) explaining this so it isn't "fixed" again without context.
- This closes out all 8 items from `docs/sonnet5-review.md`'s punch list
  (6.0.4–6.0.9). See that doc's closing update for a summary.

## [6.0.8] - 2026-07-08

### Docs (no functional change)
- Corrected misleading doc comments around `try_query_terminal_cells()` /
  `TerminalSession`: they suggested implementing DECRQCRA was a matter of
  future work. It isn't — DECRQCRA's response is a terminal-defined
  *checksum* of a region (used by conformance suites like vttest to verify
  known content, not to recover unknown content), so it cannot implement
  "read back the screen." There is no portable escape sequence that does;
  a couple of terminals (kitty, iTerm2) have proprietary extensions
  crossterm doesn't wrap. Returning `Unsupported` + a blank frame is the
  correct, final behavior here, not a stub. Also fixed a stale reference to
  `enter_player_mode()`/`exit_player_mode()`, which no longer exist (removed
  by the pre-6.0 alt-screen-corruption fix) but were still named in a doc
  comment.

## [6.0.7] - 2026-07-08

### Added
- `figby --play <path.gif>` — plays an animated GIF fullscreen in the
  terminal via the raw-mode player, then exits. Wires up `player::play_raw()`
  (previously fully-implemented but never called from anywhere) to a real
  entry point. FPS is approximated from the GIF's first real frame delay,
  matching the existing convention used when a GIF import seeds the TUI
  timeline's fps.

### Fixed
- `play_raw()` never auto-exited when a non-looping animation finished
  playing — the exit condition required the player to already be paused,
  but nothing ever paused it automatically, so an unattended `--play` would
  hang forever redrawing the last frame. Now exits once the final frame has
  had its full on-screen interval during natural (non-looping, still
  "playing") playback. Verified end-to-end via a real pty (tmux): a 4-frame
  and a 14-frame test GIF both play through and exit 0 on their own; an
  oversized GIF is still rejected cleanly by the existing memory guard.

## [6.0.6] - 2026-07-08

### Docs
- README: added `--tui` to the CLI flag table, added a Features bullet
  documenting the TUI's animation timeline/keyframing/GIF import-export
  (previously undocumented — only mentioned as a future "Roadmap" item
  despite being fully implemented), updated Project Status to v6, and
  rewrote Roadmap to list genuinely-outstanding post-v6 deferred work
  instead of already-shipped features.
- `docs/animation-audit.md`: added a superseded notice — several of its
  listed gaps were closed within days of it being written; points readers
  to `docs/sonnet5-review.md` for current status.

## [6.0.5] - 2026-07-08

### Added
- `gif_import.rs` unit tests (previously zero): single-frame round-trip,
  multi-frame per-frame delay preservation, `Background` disposal-method
  region clearing, malformed-file error, nonexistent-path error, oversized
  dimensions rejection, and error `Display` formatting. 7 new tests.

## [6.0.4] - 2026-07-08

### Fixed
- GIF-imported per-frame delays were silently discarded the moment the Export
  dialog was opened (`enter_export()` unconditionally called `clear_timeline()`),
  and again if the export format was cycled to GIF (`set_timeline()` always
  recomputed a uniform FPS-derived delay). Both paths now preserve real
  per-frame timing from an imported GIF instead of flattening it.
- `export_cells_to_ansi_multi()` accepted a per-frame delay parameter but
  never used it — a multi-frame ANSI export, when `cat`'d, flashed through
  every frame instantly with no timing. It now emits a self-playing POSIX
  shell script (`printf` per frame + real `sleep <delay>`); run with
  `sh export.ans`.

## [6.0.1] - 2026-06-22

### Added
- Palette editor: add (A), delete (Del), edit hex (E), rename swatch (N), rename palette (R) operations; inline hex and name editing modes; View menu entry for Palette Editor; keymap documentation for Ctrl+Shift+P (6.8.4).

## [6.0.3] - 2026-06-22

### Added (2026-06-22 session)
- Layer panel: redesigned to 2-row icon-based layout — row 1 shows layer name with active marker (›), row 2 shows compact attributes (visibility eye, lock icon, blend mode icon, opacity %) using Nerd Font icons. Removed verbose text labels and legacy 3-line help text (6.9.1).
- Layer panel: drag handle (⠿) on each layer row for mouse-drag reorder; Shift+Up/Shift+Down keyboard reorder; mouse click to select layers (6.9.2).
- Tool options (brush size/shape/opacity) moved from left toolbox column to right sidebar; left toolbox now shows tool list only. Brush info accessible via right sidebar Props tab (6.9.4).

### Added (2026-06-22 session)
- Quit-confirm dialog: pressing `q`/FileQuit when `editor.unsaved` is true now shows
  "Unsaved Changes" overlay with [Y]es save / [N]o discard / [C]ancel; `trigger_quit()`
  helper centralizes all three quit paths (6.7.2).
- Built-in palettes: `palette_import::builtin_palettes()` returns Grayscale (5),
  Primary (20), Warm (15), Cool (15) swatches; accessible via View → Palette: * menu
  entries which load into the palette editor panel (6.8.5).
- Keybinds popup now scrollable: ↑/↓/PgUp/PgDn/q all work; `keybindings_scroll` field
  on `TuiApp`; title updated to show controls (6.8.7).
- Added 20+ missing keybinds to KEYMAP: Layer Panel scope (n/d/x/l/m/M/+/-/Ctrl+G),
  Text Tool scope (↑↓/Enter/Esc/[/]), Canvas extras (Ctrl+A/X/C/V/Delete, r, H/V) (6.8.7).
- Layers menu (New/Duplicate/Delete/Merge Down/Move Up/Move Down/Toggle Visibility/
  Toggle Lock) and Animation menu (Add Frame/Delete Frame/Play/Toggle Timeline) added
  to menu bar with full action handlers (6.9.3).
- `handle_font_editor_key()` and `handle_image_editor_key()` methods extracted from
  `handle_key_event`; each mode now dispatched via a single-line call (6.6.1g, 6.6.1h).

- New image dialog: Width/Height fields (Tab/arrow navigation, numeric entry),
  palette dropdown (Left/Right to cycle), Enter confirms with canvas creation
  at specified size + selected palette (6.8.2).

### Fixed (2026-06-22 session)
- Unicode chars (Ä Ö Ü ä ö ü ß) typed in Text tool no longer panic; verified covered
  by 6.5.1 blank-glyph fallback; added `test_text_tool_unicode_no_panic` (6.7.3).

## [6.0.0] - 2026-06-22
### Security
- Remove `$(cmd)` shell command substitution from template resolver (`template.rs:160`);
  rendering a shared `.ftmp` can no longer execute arbitrary shell commands (B0/RCE).
- Sandbox `{{img:PATH}}` template image paths to template directory; absolute paths
  and `..` traversal are rejected (B0 adjacent).
- Cap template canvas dimensions: `width*height > 1_000_000` cells rejected,
  margin/padding clamped; prevents OOM from crafted frontmatter (B7/DoS).
- Validate FIGfont header numerics: `height` must be 1..=255, negative baseline/
  maxlength rejected; invalid header no longer accepted (B1).
- Cap zip decompression: `read_to_end()` replaced with size-checked read; zip-bomb
  fonts rejected before exhausting memory (B2).
- Fix GIF memory-guard timing: dimension check now runs before the frame decode loop;
  oversized GIF bails at first frame rather than after full decode (B4/DoS).
- Range-check FLC control-file group indices: `gl`/`gr` validated as `b'0'..=b'3'`
  before assignment; crafted `.flc` no longer panics (B5/panic).
- Limit image decode dimensions: `image::io::Reader` now uses `Limits::default()`;
  decompression-bomb images rejected (B6/DoS).

### Fixed
- Green test suite: 10 stale tests fixed — welcome-gate tests now dismiss welcome
  screen before key events; layer-model tests read/write active layer buffer not
  composite; palette shadow test updated to use `.round()` (B3).
- Replace `.expect()` in `render.rs:lookup_char` with blank-glyph fallback; fonts
  missing char 0 no longer panic (S1).
- Text tool: printable keys (`b`/`e`/`f` etc.) no longer switch tools while
  `entering_text=true`; `Char(c)` captured before toolbox-selector dispatch (6.7.1).

### Added
- `LightingState`, `AnimationState`, `InteractionState` sub-structs extracted from
  `TuiApp`; shrinks borrow surface and god-object field count by 20 (6.6.1a–c).
- `LightPanel::render()` method extracted from `TuiApp::render_light_panel()`;
  `tui/overlays.rs` extracted from `TuiApp::render_overlays()`; lighting key
  dispatch extracted to `LightingState::handle_key()` (6.6.1d–f).
- Compile-time test validates embedded `ICONS_YAML`; malformed YAML now fails CI
  instead of silently dropping all icons (A3/S2, 6.5.2).
- Clamp `font_gen` point_size to 4.0..=200.0; unbounded value no longer causes
  oversized canvas allocations (S5, 6.5.3).
- GitHub Actions CI: `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`
  run on push/PR; legacy `.travis.yml` removed (6.2.2).
- Hard `cargo test` gate in `scripts/ralph.sh` merge phase; LLM self-attestation
  no longer the only merge check (6.2.1).
- Brush size/shape indicator in toolbox panel; updates live on `[`/`]` (6.9.5).

### Changed
- `CLAUDE.md` and `AGENTS.md` updated to reflect current source layout and v6
  milestone (A2, 6.4.1–6.4.2).
- Shadow computation uses `.round()` instead of truncation; `default_shadow_hex`
  now produces `#4D0000` for 30% shadow (6.1.3).

### Docs
- Pre-release codebase audit: `docs/codebase-audit-2026-06-18.md` (sessions 1+2,
  complete read-through). Every finding mapped to a task in `docs/todo-v6.md`.

## [5.8.0] - 2026-06-18
### Added
- Phase merge: release/5.8 → main. Phase 5.8 (Dynamic Lighting System) complete:
  core lighting engine (5.8.1), canvas and layer integration with shading pass,
  layer lighting/shadow flags (5.8.2), light management UI with in-canvas light
  editor and light list panel (5.8.3), palette LUT integration with per-swatch
  lit/shadow colors and specular highlights (5.8.4).

## [5.6.0] - 2026-06-18
### Added
- Phase merge: release/5.6 → master. Phase 5.6 (Palette Enhancement & Marker Brush)
  complete: color name hover tooltip (5.6.1), hue-grouped palette with 5 per row
  (5.6.2), palette editor with save/load/duplicate (5.6.3), palette import from
  Paletty/ASE/WezTerm/Windows Terminal (5.6.4), marker brush mode with colour-stepping
  shading (5.6.5).
### Fixed
- Marker brush: when cell color not in selected palette, consume 1 step to enter
  the array at index 0 instead of jumping to index 1.
- Marker brush: preserve 0.0 fractional remainder entries in accum map for future
  strokes.
- TUI dispatch: only launch TUI when stdin is a terminal and no FIGlet flags are
  provided; piped stdin now correctly triggers CLI mode.
- Palette editor test: fixed race condition on XDG_CONFIG_HOME env var by
  serializing concurrent tests with a mutex.

## [5.5.0] - 2026-06-18
### Added
- Phase merge: release/5.5 → master. Phase 5.5 (Animation Audit & Surface) complete:
  animation audit (5.5.1), timeline panel surface in main layout (5.5.2), export
  end-to-end verification with 5-frame GIF/APNG/ANSI tests (5.5.3).

## [5.4.0] - 2026-06-18
### Changed
- Phase merge: release/5.4 → master. Phase 5.4 (Image Editor Fix) complete:
  mode switching fix (5.4.1), mouse event routing fix (5.4.2), rascii import
  dialog with charset/width/color options (5.4.3).

## [5.3.0] - 2026-06-18
### Added
- Phase 5.3 status bar redesign — flat item-based layout with StatusItem priority system
- Three informal sections (left/middle/right) with pipe separators
- Responsive dropping of low-priority items at narrow widths

## [5.2.0] - 2026-06-18
### Added
- Phase 5.2 layout restructure — palette under tools, tabbed right panel (Layers/Props/Text/Libraries/Effects), context-sensitive tool properties in Props tab

## [5.0.0] - 2026-06-18
### Added
- Welcome screen Phase 5.0 — complete overhaul
- Banner: Computerist-20 FIGfont title with Computerist-12 fallback; both mascot
  and title vertically centered in banner row; title horizontally centered
- Two-column content layout: Recent Files (left, scrollable, ↑↓) + Font/Image
  action panels (right)
- Font panel: 5 actions with NerdFont icons in `[K]ey` inline format
  (N/I/B/O/D shortcuts)
- Image panel: 4 actions with NerdFont icons (C/T/V/F shortcuts)
- Mouse hover highlight and click support on all welcome items via stored
  hit-test rects updated each render cycle
- `dispatch_welcome_action()` shared by keyboard and mouse paths
- Esc → dismiss/back-out only (never quits); Q / q → quit from canvas;
  Ctrl+C → quit via SIGINT

## [3.0.0-rc.4] - 2026-06-18
### Added
- Multi-directory font search: `load_font()` now accepts `&[&str]`, searches
  `DEFAULT_FONT_DIRS` (`/usr/local/share/figlet`, `/usr/share/figlet`) as
  fallback when a font is not found in the user-specified directory.
- `full` charset preset for `--create-font`: ASCII printable + block elements
  with `█` (full block) as the darkest character.
- ChicagoFLF system font generated to `figby-fonts/new_fonts/`.
### Changed
- `print_direction` in generated FIGfonts changed from `-1` to `0` (explicit LTR).
  Header generation now uses the struct's field value instead of hardcoding.
- Generated fonts default to `full` charset (from `smooth`) for richer output.

## [3.0.0-rc.2] - 2026-06-17
### Changed
- Phase merge: release/4.1 → main. Phase 4.1 complete: welcome screen (4.1.4),
  ZIP font browsing (4.1.5), various polish fixes.

## [3.0.0-rc.1] - 2026-06-16
### Added
- v3.0.0 release candidate cut. RC branch `rc/3.0.0-rc.1`, annotated tag
  `v3.0.0-rc.1`. Full Phase 3.3 regression complete.

## [2.5.4] - 2026-06-16
### Changed
- Phase merge: release/3.2 → master. Phase 3.2 complete: glyph grid mouse
  click+double-click, glyph char editor cursor+cell toggle, font preview strip.

## [2.5.3] - 2026-06-16

### Added
- Font editor overview: mouse wheel scroll through glyph grid (`handle_mouse_scroll_overview`)

## [2.5.2] - 2026-06-16

### Changed
- `keymap.rs` now owns a `GLOBAL_DISPATCH` table mapping `(KeyModifiers, KeyCode)` to
  `GlobalAction` variants; `lookup_global()` does exact-match lookup
- `TuiApp::handle_key_event` global if-chain replaced with `dispatch_global()` match arm;
  eliminates ~70 lines of repetitive modifier/key-code guards
- Undo/redo and undo-panel toggle also routed through dispatch table (early global pass)

## [2.5.1] - 2026-06-16

### Changed
- `MenuBar` refactored to `StatefulWidget for &MenuBar` with separate `MenuBarState`
- All mutable menu state (active_menu, focused_item, header/item rects, pending action)
  moved to `MenuBarState`; `MenuBar` retains only static config (menus, theme)
- Key/mouse handlers now take `&mut MenuBarState` instead of `&mut self`
- Render uses `frame.render_stateful_widget` for proper ratatui StatefulWidget pattern

## [2.5.0] - 2026-06-15

### Added
- `FrameLayout` struct: single-pass layout computation stored on `TuiApp` for mouse hit-testing
- `DrawerMode` enum: collapsible right drawer cycling Palette → BrushKeys → Closed (`?` key)
- Zen mode (`F11`): canvas expands to full frame area with dim hint overlay
- `Ctrl+K`: toggle full keybindings overlay panel
- Brush panel now shows `Shape:` label alongside Char/Size fields
- Extended keymap entries for all tool shortcuts, brush controls, and new global commands

### Changed
- Layout refactored to `tui/layout.rs`; canvas uses `Constraint::Fill(1)` instead of `Min`
- Collapsed borders between toolbox/canvas/right-panel (ratatui recipe — no double lines)
- `Tab` / `Shift+Tab` now cycle modes from any context (was `Ctrl+Tab` only)
- Font editor Overview auto-search exclusion expanded to protect all tool/global shortcuts
- Status bar zoom format changed from `Zoom:{n}x` to `{icon} {n}x`
- Settings dialog (`S`) now only opens when not in FontEditor mode (where `S` opens Smushing)

### Fixed
- Font editor Overview mode intercepting tool shortcuts (b/e/l/v/etc.) for auto-search
- Collapsed `if` blocks flagged by clippy (mod.rs mouse handler)
- Integration tests updated for new layout, status bar format, and EditorState field paths

## [Unreleased] — Rust Port

### Added

- Rust project scaffold (`figby-rs/`)
- Cargo workspace configuration
- FIGlet font submodule for test fixtures
- CI configuration (fmt + clippy + test)

### Porting Progress

- [ ] Phase 1.1 — Crate scaffold, font parser
- [ ] Phase 1.2 — Render engine (kerning + smushing)
- [ ] Phase 1.3 — CLI interface (all FIGlet flags)
- [ ] Phase 1.4 — Control files + character mapping
- [ ] Phase 1.5 — Multi-byte input (UTF-8, DBCS, Shift-JIS)
- [ ] Phase 1.6 — TLF (TOIlet) font support
- [ ] Phase 1.7 — Full test suite against original C
- [ ] Phase 1.8 — Optimization + polish
