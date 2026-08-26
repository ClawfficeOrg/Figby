# Figby — Memory Index

## ⚠️ Hard Rules

1. **Never delete files** without explicit user instruction. If removal seems
   necessary, ask first. If approved, `mv` to `/tmp/` (recycle) instead of
   `rm` — permanent deletion is never acceptable.

Master memory index. Detailed entries live in versioned files below.

## Versioned Memory Files

| Milestone | File | Status |
|-----------|------|--------|
| v1 — Port | [memory-v1.md](memory-v1.md) | Active |
| v2 — Templates, Images & TUI | [memory-v2.md](memory-v2.md) | Active |
| v3 — TUI Refinement & Animation | [memory-v3.md](memory-v3.md) | Active (v3.0.0-rc.1 RC cut) |
| v4 — (in progress) | (in memory.md) | Active (Phase 4.9 merged) |
| v5 — UI Overhaul & Feature Completion | [memory-v5.md](memory-v5.md) | Active (Phase 5.8 complete) |
| v6 — Pre-Release Hardening | (in memory.md) | Active (6.9.4 complete) |

## Architectural Decisions

### GPT-Review Remediation (6.0.32–6.0.33, branch `hardening/gpt-review`)
Full review at `docs/GPT-review-8-20-26.md` (30 findings); progress
tracked in `docs/review-remediation-checklist.md`. Second remote:
`origin` = ClawfficeOrg/Figby, `upstream` = CompewterTutor/Figby.

Key architectural decisions from Phase 0–1:

- **Single timeline authority (F-05)**: `TimelineFrame::document_state`
  (`Vec<CanvasBuffer>`, one per layer) owns pixels; keyframes own
  transforms and are applied per-layer on top at composite time in
  `capture_timeline_frames` — the ONLY compositor for export/playback.
  Navigation commits every layer's buffer and restores every layer via
  `EditorState::load_timeline_frame(&[CanvasBuffer])`.
- **Rendering is read-only (F-06)**: the ImageEditor cache sync was
  removed from `render()`; it runs only on explicit model mutations
  (image load, conversion keys). GIF/RASCII imports call
  `ImageEditor::clear_cells()`.
- **Revision-aware saves (F-07)**: `EditorState::mark_dirty()` bumps a
  revision counter; async saves capture `pending_save_revision` and
  only clear unsaved / honor deferred quit when unchanged.
- **One transition dialog (F-08)**: `PendingDocAction` +
  `request_transition()` route Quit/Open/New through one
  save/discard/cancel dialog; `reset_document()` atomically replaces
  all document + transient state. FontEditor exposes `mutation_epoch`;
  dispatcher marks dirty only when it moves.
- **Layer-aware undo (F-09)**: `UndoEntry.layer_index` + peek helpers
  (`undo_top_layer`/`redo_top_layer`) so redo captures the target
  layer's own buffer. `restore_undo_snapshot()` is the single lock-
  aware write path. `active_layer_locked()` gates every mutating path.
- **Packaged assets (F-03)**: embedded assets live under
  `figby-rs/assets/` (themes, icons, mascot, web fonts). Root `fonts/`
  remains for CLI runtime font discovery and control-file tests.
  Test fixture: `figby-rs/tests/fixtures/FiraMono-Regular.ttf` (OFL).
- **Reproducible builds (F-19)**: `Cargo.lock` tracked, CI runs
  `--locked`, toolchain pinned by root `rust-toolchain.toml` (1.97.1).
  CI also asserts fixtures exist and verifies `cargo package`.

### Animation timing & transforms (F-26, 6.0.34, branch `hardening/gpt-review`)

- **Timing is part of frame identity**: `TimelineFrame.delay` (centiseconds,
  GIF convention) lives on the frame, so insert/delete/reorder/duplicate
  carry timing with content. `TimelineState::frame_delays()` is the single
  producer of export/playback delays; the export dialog's
  `set_timeline_from_timeline` replaces `set_timeline` in production paths
  (FPS-preset override stays per-session via `dialog.frame_delays`).
- **Signed compositor**: `capture_timeline_frames` uses `signed_src_range`
  (source index window) so negative keyframe offsets crop the leading edge
  instead of being clamped to 0.
- **Player variable timing**: `AnimationPlayer` takes optional per-frame
  delays (`with_frame_delays`); `advance()` steps per-frame intervals,
  `frame_interval()` drives event-loop throttle and sleep loops.
  `play_fullscreen_timed`/`play_raw_timed` wrap the fixed-FPS versions.
- **Scheduler-driven preview**: export-dialog preview ticks on elapsed time
  in `run()` (never inside `render_overlays`), honoring per-frame delays;
  returns whether the frame moved so the loop can mark dirty.
- **ANSI multi-frame**: production export composes timeline frames for
  Ansi too (previously GIF/APNG only → ANSI export got only current frame).

### Distribution, claims, CI, developer commands (F-27–F-30, 6.0.35, branch `hardening/gpt-review`)

- **C-parity single table**: `figby-rs/tests/cparity/scenarios.tsv` is the
  one source of truth for differential scenarios (args/input/status:
  `active`/`known-divergence`/`special`). `scripts/regenerate-expected.sh`
  regenerates `tests/res*.txt` from C by iterating the table;
  `test_cparity_table_in_sync` fails if the table's `known-divergence` set
  drifts from the `#[ignore]`d tests. 5 divergences remain: tests 21, 23,
  26, 35, 37 (paragraph, `-kpc`, JIS0201, control chars, `-m191`).
- **`-F` is removed (matches C 2.2.5)**: C figlet treats `-F` as an illegal
  option (dropped for `figlist`); Figby documents + errors the same way.
  Do not "implement" it as a font lister.
- **Version identity**: package version (Cargo `6.0.x`) is canonical; `7.0.x`
  git tags are milestone markers for the animation line. Keep README Project
  Status consistent with that split.
- **Owner**: canonical owner is `ClawfficeOrg/Figby` (`origin`); `upstream`
  is `CompewterTutor/Figby`. Cargo `repository` + README URLs must use the
  canonical owner.
- **Legacy snap** quarantined at `packaging/snap-legacy/` (packaged C figlet,
  EOL `core18`). A real Figby snap must build `figby-rs/` (not `c-figlet/`),
  declare app `figby`, use `core22`/`core24`.
- **CI matrix** in `.github/workflows/ci.yml`: fmt / test(Linux clippy+test) /
  docs (strict rustdoc `RUSTDOCFLAGS="-D warnings"`) / package (+tag-gated
  `cargo publish --dry-run`) / msrv / cross (win+mac build + `--lib` tests) /
  wasm (check + clippy `--no-deps`) / audit. Actions pinned to release tags.
  Strict rustdoc needs `--no-deps` (deps emit doc warnings).
- **Root commands**: repo root has no `Cargo.toml` — every cargo invocation
  must pass `--manifest-path figby-rs/Cargo.toml` (docs, scripts, CI).
- Known `cargo audit` warnings (allowed, gating on real failures only):
  `ansi_term` + `paste` unmaintained (via `image`→`exr`→`pulp`), `lru`
  0.16.4 unsound (via `ratzilla` wasm chain).

### UTF-8 Native Encoding
Figby uses Rust `char`/`String` natively (UTF-8), not `wchar_t`.
FIGlet C used `typedef long inchr` for internal char representation.
We map directly to `char` (Unicode scalar value) and use `String` for
output rows. TLF fonts already UTF-8; FIGfont ASCII is valid UTF-8.

### Smushing Engine as Pure Functions
The C `smushem()` function uses global `smushmode`, `previouscharwidth`,
`currcharwidth`, `hardblank`, `right2left`. Rust version takes these
explicitly as parameters — no globals. `SmushMode` bitflags struct
replaces `int smushmode`.

### No ZIP Reading in Initial Port
The original zipio.c/inflate.c/crc.c stack reads `.flf` from ZIP archives.
Rust equivalent uses the `zip` crate. Deferred to Phase 1.1.7 — initial
port reads plain files only.

### Error Handling
No `unwrap()` in production paths. All parse errors return `Result`.
Parser is lenient: hard errors on bad magic, but silently corrects minor
issues (truncation, extreme values) as C does.

### CLI Library
Use `clap` with derive macros for CLI parsing, replacing `getopt`.
FIGlet flag semantics preserved exactly.

### Multi-font-directory search
- `font_candidates()` and `load_font()` changed from `&str` (single dir) to
  `&[&str]` (multiple dirs). All callers updated: `main.rs`, `template.rs`,
  `tui/tools/text.rs`, `tui/font_editor.rs`, `tests/tui.rs`, and `font.rs` tests.
- `DEFAULT_FONT_DIRS` constant: `["/usr/local/share/figlet", "/usr/share/figlet"]`.
  CLI render path searches `[user_dir, ...DEFAULTS]` so fonts in `/usr/local/share/figlet`
  are found automatically on macOS without `FIGLET_FONTDIR`.
- Template path similarly searches `[config.font_dir, ...DEFAULTS]`.

### Full charset for font generation
- Added `full_charset()` preset: ASCII printable (0x20-0x7E) + `blocks_charset()`
  with `█` (U+2588) as the last/darkest entry. No intermediate braille/box/ogham
  sets to avoid drowning out the full block.
- `resolve_charset()` updated with `"full"` alias.
- `--create-font-charset` help text updated to list `"full"`.
- In `deluxe_charset()`, `█` is now explicitly pushed at the end after all other
  sets, ensuring darkest pixels always fill solid regardless of set ordering.

### 7.1.1 — Fix quit-confirm dialog sizing + mouse input

- **Width fix**: Replaced hardcoded `52` with dynamic width computed from hint line length
  (55 chars) + 4 for borders/padding = 59. Full hint now visible at 80 and 60 col terminals.
- **Button rect storage**: `TuiApp.quit_confirm_buttons: [Rect; 3]` stores Y/N/C button geometry
  computed during `overlays.rs` render based on character positions within the `inner` rect.
- **Mouse input**: Early `if self.quit_confirm_dialog` branch in `handle_mouse_event` hit-tests
  the three stored rects and dispatches Save/Discard/Cancel, mirroring keyboard handler logic.
- **Files touched**: `figby-rs/src/tui/mod.rs` (struct field, constructor, mouse handler),
  `figby-rs/src/tui/overlays.rs` (render width computation + button rect storage).

### 7.3.3 — Group remaining TuiApp fields into sub-structs

- **New sub-structs**: `WelcomeState` (welcome_screen/fx/fade_in), `FrameState`
  (dirty/force_full_redraw/last_draw_time/fps/last_frame_time/delta_time/fx_last_tick),
  `UiState` (mode/prev_mode/session_type/zen_mode/show_keybindings/keybindings_scroll/
  menu_bar/menu_bar_state/should_quit), `AppContext` (icons/theme/render_mode/git_branch/
  auto_save_interval/last_save_time/throbber/async_rx/palette_rgb_to_swatch).
- **DialogState extended**: Added `quit_confirm_dialog`, `quit_confirm_buttons`,
  `quit_after_save`.
- **TuiApp**: 39 fields → 12 sub-struct fields (reduction of 27).
- **Files touched**: `figby-rs/src/tui/mod.rs` (struct defs, constructor, ~150 access
  sites), `figby-rs/src/tui/overlays.rs` (8 access sites), `tests/tui.rs` (12 access
  sites), `tests/regression_tui.rs` (1 access site), `src/main.rs` (1 access site).
- **Convenience**: `mark_dirty()` and `force_full_redraw()` methods considered but
  not added — `self.frame.dirty = true` is already concise and the ~80 occurrences
  are a straightforward grep.
- **No behaviour change**: Pure refactoring.

### Generated font print_direction
- `print_direction` in `render_font_glyphs()` changed from `-1` to `0` (explicit LTR).
- `generate_figfont_header()` now uses `font.print_direction` field value instead
  of hardcoded `-1`, making headers reflect the actual struct state.

## Task History
### 1.1.1 — Create `figby` crate in workspace

Added `[lib]` section to `figby-rs/Cargo.toml` (name=figby, path=src/lib.rs,
crate-type=[lib]). Added `#![doc]` crate-level attribute to `src/lib.rs` with
description. Sorted `pub mod` declarations alphabetically (rustfmt preference).
Five module stubs (font, render, smush, control, input) compile as-is.

### 1.1.2 — Define core types: FIGfont, FIGcharacter, FCharnode
Added `FIGfont`, `FIGcharacter`, `FCharnode` structs in `font.rs` with serde
derive (Serialize/Deserialize). `FIGcharacter` wraps `Vec<String>` rows with
`width()` and `rows()` accessors. `FCharnode` maps `u32` code → `FIGcharacter`.
`FIGfont` owns font metadata (hardblank, charheight, baseline, maxlength,
old_layout, full_layout, print_direction, comment_lines, codetag_count) +
`HashMap<u32, FIGcharacter>` chars. All types derive `Default` (FIGfont: -1
for print_direction, rest zero/empty). Round-trip serde tests via serde_json.

### 1.1.3 — FIGfont magic number + header line parser
Added `parse_header()` in `font.rs` — validates `flf2a` magic, extracts hardblank
char, parses 5 required numeric fields (height, baseline, max_length, old_layout,
comment_lines) and 3 optional fields (print_direction, full_layout, codetag_count).
Missing optionals defaulted per C logic: print_direction → -1, full_layout derived
from old_layout, codetag_count → 0. Defined `FontError` enum with `InvalidMagic`
and `ParseError` variants. 11 fixture + error tests.

### 1.1.4 — FIGcharacter data parser
Added `parse_char_data()` in `font.rs` — reads 95 required ASCII FIGcharacters
(codes 32–126) and 7 Deutsch chars (196, 214, 220, 228, 246, 252, 223). Added
`strip_endmarks()` private helper that follows `figlet.c:1155-1165` algorithm:
trim trailing whitespace, identify endmark as last remaining char, remove all
consecutive endmarks from the right. Trailing whitespace before endmarks is
preserved for width correctness. Added `DEUTSCH_CHARS` constant matching C
`inchr deutsch[7]`. Function returns unconsumed line slice for subsequent
codetag parsing (1.1.5). 10 tests covering endmark stripping edge cases,
102-char parse, endmark removal verification, width consistency, error on
truncated input, and unconsumed line return.

### 1.1.6 — TLF font support (TOIlet format)

Added `FontFormat` enum (`Figfont`/`Tlf`), `format` field on `FIGfont`,
`parse_tlf_font()` entry point. `parse_header()` now accepts both `flf2a`
and `tlf2a` magic numbers. Reuses all FIGfont parsing infrastructure
(endmark stripping, char data, codetagged). TLF rows are UTF-8 natively
(Rust `String` handles this without special treatment).

### 1.1.7 — Compressed font support (zip/deflate)

Added `load_font()` as FIGopen() equivalent: try `fontdir/name.flf`, bare
`name.flf`, `fontdir/name.tlf`, then bare `name.tlf`. Each candidate reads
bytes, detects ZIP magic (`PK\x03\x04`), extracts first entry if ZIP, parses
via existing `parse_tlf_font()`. `FontError` gained `IoError(std::io::Error)`
and `ZipError(String)` variants. `zip = "2"` and `flate2 = "1"` added to
Cargo.toml. ZIP is only read from — `ZipWriter` used solely in tests.
`PartialEq` on `FontError` is now manual (cannot derive with `std::io::Error`).

### 1.1.8 — Phase merge: release/1.1 → master

Merged all Phase 1.1 work into default branch (master). Phase 1.1 complete:
crate scaffold, core types, FIGfont/TLF header parser, FIGcharacter data
parser, code-tagged character parser, TLF support, ZIP/deflate compressed
font loading. All 7 subtasks (1.1.1–1.1.7) implemented, tested, merged.
Phase 1.2 (render engine) is next.

### 1.2.1 — Character lookup + width calculation

Added `lookup_char()` in `render.rs` — font char lookup with fallback to
char code 0. Updates `current_width` (via `&mut usize`) so caller captures
previous width before next call. Uses `expect()` for char 0 invariant
(FIGfont spec requires it). Three tests: known char, unknown fallback,
previous-width tracking.

### 1.2.2 — Smushing rules engine

Full smushing rules engine in `smush.rs`. `SmushMode` newtype over `u32`
with bitmask constants matching FIGfont `full_layout` encoding.
`smush_horizontal()` mirrors `figlet.c:smushem()` — all 6 horizontal rules
(H1-H6) plus universal overlap and kerning. `smush_vertical()` implements
V1-V5 vertical smushing rules. Hardblank treated as space for vertical ops.
Hierarchy helpers shared between H3/V3. No `.unwrap()` in production — all
fallible paths use `Option<char>`. 34 unit tests covering every rule.

### 1.2.3 — Smush amount calculation

Added `calc_smush_amount()` in `render.rs` — port of C `smushamt()`.

### 1.2.4 — Character addition with smushing

Added `add_char()` in `render.rs` — port of C `addchar()`. Function takes
font, char code, mutable output rows, outlinelen, prev_width, smush mode,
RTL flag, and outlinelen_limit. Returns `bool` (true if char added, false
if limit exceeded). Saves and restores `prev_width` on failure. LTR: builds
char on right side of output with kerning/smushing overlap. RTL: builds char
on left side with reversed smush dominance. Post-loop updates `outlinelen`
from `output_rows[0]` character count. Uses `#[allow(clippy::too_many_arguments)]`
(8 params, mirrors C's global-based approach). Uses iterator-style loops to
avoid `needless_range_loop` clippy lint. 9 tests: first-char, two-char kerning,
two-char smush, RTL smush, limit bail, prev_width restore, single-word
("Hi!"), multi-row, and boundary smush.
Two private helpers: `last_non_space()` (RTL scan for last non-space) and
`first_non_space()` (LTR scan for first non-space), each with fallback
position/char parameters matching C sentinel behavior (null terminator
for forward scans, position 0 for backward scans). Main function iterates
over row pairs, computes overlap between last non-space of output and
first non-space of current char, applies edge adjustment (boundary char
smush or space), and returns minimum across all rows. Handles LTR and RTL,
KERN-only and SMUSH modes. Uses `saturating_sub` for safe unsigned
arithmetic matching C signed-int boundary behavior. 9 unit tests covering
guards, LTR/RTL basics, row-min, boundary smush/no-smush, and all-spaces
edge cases.

### 1.2.5 — Output line printing

Added `Justification` enum (`Left`/`Center`/`Right`) with `from_i32()`
conversion matching C `justification` global (0/1/2). Added `render_line()`
in `render.rs` — port of C `putstring()`/`printline()` figlet.c:1553-1610.
Processes each row: (1) replace hardblank with space, (2) truncate to
`outputwidth - 1` if `outputwidth > 1`, (3) prepend spaces for Center/Right
per C formula. Center formula: `2*i + len - 1 < outputwidth`. Right formula:
`i + len < outputwidth`. No `clearline()` port — Rust returns fresh
`Vec<String>` each call; caller manages lifecycle. 13 tests: hardblank
replacement, left/center/right justification, width truncation, truncation
with center, `outputwidth <= 1` bypass, multi-row, C formula trace tests,
hardblank+truncation combination, zero outputwidth, empty rows.

### 1.2.6 — Line breaking and word splitting

Added `split_line()` in `render.rs` — port of C `splitline()` (figlet.c:1623-1658).
Scans char_buffer backward for last run of consecutive spaces, splits into
part1 (before space run) and part2 (after). Rebuilds both parts from scratch
via `add_char()` calls, matching C's clearline->addchar->printline->addchar
sequence. Returns `Option<(Vec<String>, usize)>` — part1 rows to caller and
part2_start index for caller to truncate its buffer. `output_rows` is mutated
in-place to contain only part2. Eight tests: basic multiword split, multiple
spaces consumed, no-word-break (None), single char after space, leading spaces
consumed, all-spaces buffer, multi-row font, empty buffer. All 8 tests use
`build_expected()` helper that calls `add_char()` independently to verify
part1/part2 output consistency.

### 1.2.7 — Phase merge: release/1.2 → master

Merged all Phase 1.2 work into default branch (master). Phase 1.2 complete:
character lookup + width calculation, smushing rules engine (all 6 horizontal
+ 5 vertical rules), smush amount calculation, character addition with
smushing, output line printing with justification, line breaking and word
splitting. All 6 subtasks (1.2.1–1.2.6) implemented, tested, merged.
Phase 1.3 (CLI Interface) is next.

### 1.3.1 — CLI argument parsing

Rewrote `main.rs` from scaffold to full CLI parser using clap derive.
`CliArgs` struct parses all 27 FIGlet flags (`-A` through `-F`) plus
positional message. `CliConfig` holds all 11 globals (`smushmode`,
`smushoverride`, `justification`, `right2left`, `paragraphflag`,
`deutschflag`, `cmdinput`, `outputwidth`, `fontdirname`, `fontname`,
`multibyte`). `SmushOverride` enum mirrors C `SMO_NO`/`SMO_YES`/`SMO_FORCE`.
Flag normalization matches C switch-case semantics: `-m` value mapping,
`-s`/`-k`/`-S`/`-o`/`-W` smush overrides, `-x`/`-l`/`-c`/`-r` justification,
`-X`/`-L`/`-R` text direction, `-D`/`-E` deutsch, `-p`/`-n` paragraph,
`-A`/positional cmdinput. `-F` prints error and exits. `run()` is a
placeholder for 1.3.4. `#[allow(non_snake_case)]` needed on `CliArgs`
struct because uppercase flags (e.g. `-L` vs `-l`, `-S` vs `-s`) would
collide in snake_case. 20 unit tests covering every flag + defaults.

### 1.3.2 — Info codes (`-I` flag)

Added `printinfo()` and `printusage()` functions ported from C. Six infocodes
mapped exactly: 0 (copyright + usage), 1 (version int), 2 (font dir), 3 (font),
4 (output width), 5 (font formats). Both functions take `&mut impl Write` for
testability — production passes `io::stdout().lock()`, tests pass `Vec<u8>`.
Constants added: `VERSION_INT=20205`, `VERSION="2.2.5"`, `DATE="31 May 2012"`,
`FONTFILE_MAGIC="flf2"`, `TOILETFILE_MAGIC="tlf2"`. `-I` handler in `main()`
prints info and exits with code 0, matching C behavior. 6 unit tests covering
all infocodes (0–5) with byte-exact output assertions.

### 1.3.3 — Terminal width detection (`-t`)

Added `get_columns()` using `termion::terminal_size()` — wraps `ioctl(TIOCGWINSZ)`
on Unix, returns `Option<u16>`. `-t` flag in `CliConfig::from_args()` calls
`get_columns()` and sets `outputwidth` to detected width if successful. `-t`
handling placed before `-w` so explicit `-w` flag overrides `-t` when both given.
`termion = "4"` added to `Cargo.toml`. No `unwrap()` in production — returns
`None` gracefully on non-TTY or error, falling back to default 80. 3 unit tests:
parsing, width update, `-w` override, `get_columns()` never panics.

### 1.3.4 — Main event loop

Full `run()` implementation porting C's `main()` loop (figlet.c:2003-2134).
`InputIter` enum handles both stdin and argv-based input (`Agetchar` equivalent)
with `unget` support for paragraph mode peeking. `flush_output_line()` helper
calls `render_line()` then writes rows to stdout, then clears output state.

Main loop matches C exactly:
- Paragraph mode (`-p`): peek at next char after `\n`, map to space if non-ws
- `last_was_eol_flag` tracking for `\n`/`\v`/`\f`/`\r` (not tab/space)
- Deutsch re-routing (`-D`): `[\]` → umlauts, `{|}~` → lowercase umlauts+ß
- `handlemapping()` identity (Phase 1.4.2 will integrate control files)
- Space normalization: tab/space→space, other ws→newline
- Control char skip: 1-31 (except `\n`) and 127 (DEL)
- Inner retry loop matching C's `do {} while (char_not_added)`:
  - `wordbreakmode == -1`: absorb spaces/newlines after forced break
  - `c == '\n'`: flush line unconditionally
  - `addchar` success: track wordbreakmode (0/1/2/3 per C spec)
  - `addchar` fail + `outlinelen == 0`: raw-char path (print char directly)
  - `addchar` fail + `c == ' '`: split (if wordbreakmode>=2) or flush, then absorb space
  - `addchar` fail + else: split/flush, set wordbreakmode, retry
- EOF flush: if `outlinelen != 0`, flush remaining line

`DEUTSCH_CHARS` visibility changed from `pub(crate)` to `pub` (needed by binary crate).
`io::Read` import added to `main.rs` for `BufReader::bytes()`.
12 unit tests for `InputIter`: empty args, single/multi word, empty words, unget.

### 1.3.5 — Phase merge: release/1.3 → master

Merged all Phase 1.3 work into default branch (master). Phase 1.3 complete:
CLI argument parsing (all 27 FIGlet flags), info codes (-I flag with 6 codes),
terminal width detection (-t flag via termion), main event loop with full
FIGlet 2.2.5 input processing pipeline. All 4 subtasks (1.3.1–1.3.4)
implemented, tested, merged. Phase 1.4 (Control Files & Character Mapping)
is next.

### 1.4.1 — Control file parser

Added `ControlCommand`, `ControlState`, `ControlError` types and `read_control()`
function in `control.rs`. Byte-level parser mirrors C `readcontrol()`:
- `t` (translate) with single char, range (`-` separator), and escape sequences
- Mapping table entries (lines starting with `0-9` or `-`)
- `f` (freeze) command
- `b`/`u`/`h`/`j` (multibyte modes) set `state.multibyte`
- `g` (ISO 2022 charset) with `charset_define()` for G0-G3,
  `gl`/`gr` selection, `96`/`94`/`94x94` char set variants
- `#` comments and blank lines silently consumed
- `ByteReader` wrapper struct provides multi-byte unget/pushback matching
  C `Zgetc`/`Zungetc` pattern
- 35 unit tests covering all command types, escape sequences, numeric
  formats (decimal, octal, hex), CRLF handling, fixture file parsing

### 1.4.2 — Character remapping via control files

Added `remap_char()` in `control.rs` — port of C `handlemapping()`. Iterates
`ControlState.commands` sequentially. For each translate command (thecommand=1),
checks if char `c` is in `[rangelo, rangehi]` range. On match: applies offset
via `wrapping_add` (matching C's signed `inchr` overflow semantics), then skips
remaining translate commands in current block (stops at freeze or end). Freeze
commands (thecommand=0) act as block boundaries — only first matching translate
within each block applies. Multiple blocks apply sequentially across freeze
boundaries. `remap_char()` returns modified char; identity if no match.

Wired into `run()` in `main.rs`: `-C` flag loads control file via `read_control()`
into `ControlState` after font loading. `remap_char()` called in main event loop
after Deutsch re-routing, replacing placeholder identity. Added `controlfile:
Option<String>` to `CliConfig`.

14 unit tests: empty commands, single char, range, no match, negative offset,
out of range, freeze-block skip (second match in same block skipped),
two-block sequential apply, three-block chain, mapping table entry, upper.flc
fixture (a→A, z→Z). All tests define state via `build_remap_state()` helper
that reads control file content then runs `remap_char`. No `.unwrap()` in
production — error handling via `Result` + `process::exit(1)` on control file
load failure.

### 1.4.3 — ISO 2022 character set handling

Added `CharReader` trait with `next()`/`unget()` methods to `control.rs` —
abstracts input for `iso2022()` to work with both `InputIter` and test
`MockReader`. `ControlState::iso2022()` port of C `iso2022()` (figlet.c:1745-1875):
processes ESC sequences for G0-G3 set designation, SO/SI GL shift, SS2/SS3
temporary invocation, LS2/LS3 permanent, GL/GR zone processing with double-byte
combining. `InputIter` implements `CharReader`. Main event loop dispatches via
`config.multibyte == 0` to `control_state.iso2022()`. All `.unwrap_or(0)` calls
match C's lenient second-byte read behavior. 15 unit tests covering all ISO 2022
escape types, edge cases, plain passthrough.

### 1.4.4 — Phase merge: release/1.4 → main

Merged all Phase 1.4 work into default branch (master). Phase 1.4 complete:
control file parser (`.flc` parsing with translate, freeze, multibyte mode,
ISO 2022 charset commands), character remapping via `remap_char()` with freeze-
block semantics, ISO 2022 character set handling with G0-G3/GL/GR/SS2/SS3.
All 3 subtasks (1.4.1–1.4.3) implemented, tested, merged. Phase 1.5
(Multi-byte Input) is next.

### 1.5.1 — UTF-8 input mode

Added `read_utf8_char()` in `input.rs` — port of C `getinchr()` case 2
(UTF-8 decoder). Decodes 1-6 byte sequences using `std::str::from_utf8`
for validation with explicit continuation byte checks. Error sentinel
`0x0080` for invalid sequences (matching C). Wired into main event loop
via `config.multibyte == 2` dispatch. Initial leading byte dispatch
uses bitmask matching. Continuation bytes validated (`b & 0xC0 == 0x80`).
No `.unwrap()` in production — `from_utf8` error path returns `Some(0x0080)`.
12 unit tests covering ASCII, 2/3/4 byte valid, overlong C0/C1, surrogate,
invalid lead bytes (0xFE/0xFF), F5+ codepoints, truncated sequences, bad
continuation, EOF on first byte, multiple mixed chars.

### 1.5.2 — DBCS, HZ, Shift-JIS input modes

Added `read_dbcs_char()` in `input.rs` — port of C `getinchr()` cases 1/4
(DBCS/SJIS). Lead byte 0x80-0x9F or 0xE0-0xEF combines with trail byte as
`(lead << 8) | trail`. Non-lead bytes pass through. EOF after lead byte
returns lead byte alone. `HZState` struct tracks HZ escape mode (`~{` enter,
`}~` leave, `~~` = tilde). `read_hz_char()` uses recursive approach:
`~{` sets mode + recurses, `}~` clears mode + recurses, `~~` returns `~`,
`~x` skips + recurses. Wired into main event loop via `config.multibyte`
match: modes 1/4 → `read_dbcs_char`, mode 3 → `read_hz_char`. `HZState`
passed as third parameter to `next_char` closure. 14 unit tests: 5 DBCS,
9 HZ covering all edge cases. `HZState` derives `Default`.

### 1.5.3 — Deutsch flag character re-routing

Added `deutsch_reroute()` in `input.rs` — port of C's inline deutsch
re-routing logic. Refactored from `main.rs` into a standalone function
for testability. Maps `[\]` (0x5B-0x5D) to Ä/Ö/Ü (196/214/220) and
`{|}~` (0x7B-0x7E) to ä/ö/ü/ß (228/246/252/223). Uses `DEUTSCH_CHARS`
constant from `font.rs`. Called before `remap_char()` in main event loop.
9 unit tests covering all 7 mappings, disabled flag, and out-of-range.

### 1.5.4 — Phase merge: release/1.5 → master

Merged all Phase 1.5 work into default branch (master). Phase 1.5 complete:
UTF-8 input mode (1.5.1), DBCS/HZ/Shift-JIS input modes (1.5.2), Deutsch
flag character re-routing (1.5.3). All 3 subtasks implemented, tested, merged.
Phase 1.6 (Test Suite & Verification) is next.

### 1.6.2 — Font fuzz testing

Added property-based fuzz tests for font parser in `tests/fuzz.rs` using `proptest`.
All 4 public parser functions (`parse_header`, `parse_tlf_font`, `parse_char_data`,
`parse_codetagged`) exercised with random malformed strings — no panics, only
`Result` returns. Height bounds prevent infinite loops at height=0. `proptest`
dev-dependency added.

### 1.6.3 — Rename project: Feiglet → Figby

Renamed every instance of `Feiglet`/`feiglet` to `Figby`/`figby` across the
entire repository. Includes: `figby-rs/` directory rename, Cargo package name,
CLI command name (`figby`), lib name, module imports (`use figby::...`),
all documentation files, scripts, and skills. Version 1.6.3 task added to
todo-v1.md with renumbering of subsequent tasks (benchmarks → 1.6.4, phase
merge → 1.6.5). Build, fmt, clippy, and all 273 tests pass clean.

### 1.6.4 — Performance benchmarks

Added Criterion benchmark suite in `figby-rs/benches/render_bench.rs` with 9 benches:
font_load, lookup_char (1000x), smush_horizontal (10000x), calc_smush_amount (1000x),
add_char_kerning (100x "Hi World"), add_char_smushing (100x), render_line (4
justifications), split_line, full_pipeline (~5KB Lorem Ipsum text). Font loaded lazily
via `OnceLock` to avoid re-parsing. `criterion = "0.5"` dev-dependency added,
`[[bench]]` entry with `harness = false`. No C binary available for baseline
comparison — Rust baseline established; manual C comparison needed separately.
`target/criterion/` already covered by `target/` in `.gitignore`.

### 1.6.5 — Fix rendering pipeline bug

Identified and fixed root cause of `wordbreakmode` condition mismatch in
space-char failure path of main event loop. C figlet uses `wordbreakmode == 2`,
Rust used `wordbreakmode >= 2`. When `wordbreakmode == 3` (after a space, now
in a word), a failing space causes `printline()` (simple flush), not
`splitline()`. The `>= 2` check incorrectly called `split_line()` which
prematurely split on word boundaries, causing output divergence for
multi-line stdin input at default terminal width.

Additional fixes:
- `char_buffer.truncate(part2_start)` → `char_buffer.drain(..part2_start)` —
  `split_line` returns start index of part2, not length of part1
- Font parser: `String::from_utf8` → `String::from_utf8_lossy` for non-UTF-8
  font bytes (bubble font had embedded 0xFF bytes)

Status: 17/27 integration tests pass (was 4/27 before fixes). Remaining 10
failures involve RTL, TLF fonts, paragraph mode — separate issues from the
wordbreakmode bug.

Phase 1.7 (Major Release: end-to-end verification + RC) is next.

### 2.0.2 — Port make-examples script to CLI

Created `scripts/make-examples.sh` — POSIX shell script that generates
example output for every font file in `fonts/`. Supports `--sample-text`
(default `"hello figby"`), `--fonts` (comma-separated whitelist),
`--exclude` (comma-separated blacklist), and `--categories` (parsed but
deferred). Resolves `figby` binary via PATH, `figby-rs/target/debug/figby`,
or builds if missing. Uses `-d fonts/` flag so font resolution works from
repo root. Output goes to `examples/` with a `.gitkeep` sentinel file.

### 2.0.4 — Repo cleanup: move C source to c-figlet/

Moved all C FIGlet 2.2.5 source files (`figlet.c`, `chkfont.c`, `inflate.c`,
`zipio.c`, `utf8.c`, `getopt.c`, `crc.c`, headers, `Makefile*`) from root into
`c-figlet/`. Removed stale `.o` build artifacts. Updated references in README.md,
AGENTS.md, skills/ralph.md, .travis.yml, snapcraft.yaml, and run-tests.sh.
Root is now clean of loose C source files.

### 2.0.3 — Update README with proper documentation

Complete README rewrite covering: what Figby is, installation methods
(cargo, build from git, pre-built, package managers), CLI usage with
all 27 flags and examples, font directory setup and resolution order,
getting fonts (bundled + external sources), comparison with C FIGlet
(feature parity table), contributing guide with setup and quality gates,
project status with v1/v2 milestone references, roadmap, and license.
`cargo fmt --check` and `cargo clippy` pass clean.

### 2.0.8 — `--to-file` output flag (CLI arg only, no-op)

Added `--to-file <path>` long flag to `CliArgs` struct, `to_file: Option<String>` field
on `CliConfig` with default `None`, assignment in `from_args()`. No file I/O — deferred
to 2.1. One parse test (`test_flag_to_file`). `cargo fmt` and `cargo clippy` pass clean.

### 2.0.9 — Builtin template functions: date + repo-data (syntax + reserve)

Added `TemplateBuiltin` enum with `Date(String)` and `RepoData(String)` variants
to `template.rs`. Added `builtin: Option<TemplateBuiltin>` field to `Layer` struct
(default `None`). `parse_ftmp()` recognizes `{{date:format}}` and
`{{repo-data:field}}` tags before the variables lookup. `render_template()` skips
builtin layers with `continue` (no-op, deferred to 2.1). No `.unwrap()` in
production — all new code uses proper Option handling. fmt and clippy pass clean.

### 2.0.10 — Phase merge: release/2.0 → master

Merged all Phase 2.0 work into default branch (master). Phase 2.0 complete:
CLI `--help` output (2.0.1), make-examples.sh (2.0.2), comprehensive README
(2.0.3), repo cleanup / C source relocation (2.0.4), `.ftmp` template format
+ renderer (2.0.5), border and shadow rendering (2.0.7), `--to-file` CLI arg
(2.0.8), builtin template functions (2.0.9). All 10 subtasks (2.0.1–2.0.10)
implemented, tested, merged. Phase 2.1 (Image-to-ASCII Pipeline) is next.

Second merge (this commit) brings 3 post-initial-merge commits from `release/2.0`:
fix broken template tests, redesign `.ftmp` format (YAML frontmatter, defer to TUI),
add `assets/tui/icons.yaml` for Phase 2.2, renumber 2.2.5→2.2.6.

### 4.5.3 — Tweening

Added auto-tween system to animation timeline:
- `EasingFunction` enum (Linear, EaseIn, EaseOut, Bounce) with `apply(t)` cubic/bounce curves
- `TweenConfig` struct (start/end frames, num_frames, easing) with Default
- `TweenPreview` struct holds generated frames, valid flag, and field_index for UI nav
- `open_tween()` creates preview from current frame
- `compute_tween()` interpolates position (lerp_i16), opacity (lerp_u8), blend mode (step) between keyframed layers
- `commit_tween()` inserts generated frames, shifts current_frame
- `discard_tween()` clears preview
- `render_tween_panel()` renders config UI in overlay panel
- `handle_tween_key()` navigates fields, adjusts values, generates/commits/discards
- Timeline widget renders preview frames as ghost thumbnails (Cyan color, dim style)
- `T` key opens tween panel, Enter generates/commits, C commits, Esc discards
- 23 unit tests covering easing, tween generation, commit, discard, edge cases
- No `.unwrap()` in production paths

### 5.2.3 — Props tab: context-sensitive tool properties

Replaced static keybinds cheat sheet in Props tab with context-sensitive dispatcher
based on active `Tool`. Each tool group renders its own property panel:
- Brush/Spray/Eraser: size, shape, density, char with keybind hints
- Text: font name, justification, scale
- Eyedropper: sample cell info (char, fg, bg) or "no sample yet"
- Fill: threshold value
- Emitter: rate, lifetime, shape, char, size (compact inline summary)
- All other tools (Marquee, Lasso, etc.): fallback keybinds cheat sheet

Image/font info (canvas dimensions, zoom, font name) always rendered at bottom
of Props tab content, separated by blank line.

`SidePanel::render()` signature expanded to accept `Tool`, `&BrushState`,
`Option<CanvasCell>` (eyedropper sample), `u8` (fill threshold),
`Option<&ParticleConfig>` (emitter config), canvas dimensions, font name, zoom.

`EditorState` gained `fill_threshold: u8` (default 0) and
`eyedropper_sample: Option<CanvasCell>` (default None) fields.

### 4.5.4 — GIF export from timeline

Added GIF timeline export support to the TUI editor:
- `export_cells_to_gif()` in `output.rs` now takes `loop_count: u16` parameter (0=infinite)
- `ExportDialog` gained GIF-specific fields: `fps`, `loop_count`, `frame_delays`,
  `preview_frame`, `preview_playing`, `timeline_available`, `timeline_frames`
- `set_timeline(fps, count)` populates frame_delays from FPS (delay=100/fps cs)
- `preview_tick()` advances preview_frame cyclically when `preview_playing`
- GIF-specific key handlers: `F` cycles FPS presets (6/8/12/24/30/60), `L` cycles
  loop presets (0/1/2/5/10), `P` toggles preview, `Space` steps frame when paused
- GIF render shows FPS/Loop/Frames/Preview status lines; hides Layers/Alpha lines
- `perform_export` in `mod.rs` composes timeline frames via keyframe interpolation
  (position_offset, opacity, blend_mode) per layer per frame before spawning export thread
- `blend_mode_color()` and `blend_colors()` made `pub(crate)` in `layers.rs` for reuse
- 10 new tests: finite/infinite loop GIF, all GIF dialog key handlers, preview tick,
  frame delay recalculation, set_timeline, mode-gating, space step
- No `.unwrap()` in production paths

### 2.10.1 — Full regression against C FIGlet 2.2.5

Added 23 new integration tests (28-50) covering all FIGlet 2.2.5 features:
empty input, single char, explicit smush mode (`-m`), deutsch flag (`-D`),
deutsch disabled (`-E`), default direction (`-X`), multibyte disable (`-N`),
control char filtering, various output widths (`-w`), full smush rules
(`-m191`), kerning with small font, overlap with standard, full-width RTL
smushing, TLF long text, cmdinput (`-A`), font dir env var (`FIGLET_FONTDIR`),
control file remapping (`-C`), paragraph mode with narrow width, smush vs kern
combo, all fonts with kerning, all fonts with overlap, long text center
justification, and big font RTL.

Created `scripts/regenerate-expected.sh` — POSIX shell script that builds
C FIGlet from `c-figlet/` and regenerates all 50 expected output files
from C FIGlet byte-exact output. Handles `FIGLET_FONTDIR` env var for font
resolution.

### 2.1.2 — Luminance-to-ASCII character mapping

Added ASCII art conversion pipeline in `image_input.rs`:
- `DEFAULT_CHAR_MAP` constant (` .-:=+*#%@`) — darkest to brightest
- `luminance_to_ascii()` — converts luminance matrix to ASCII string with
  bilinear resize to target width, auto aspect-ratio correction (0.5× height
  for terminal char ~2:1 aspect), and configurable char map
- `image_to_ascii()` — convenience wrapper: loads image, converts to ASCII,
  defaults to terminal width (80 fallback) and default char map
- `bilinear_resize()` (private) — bilinear interpolation for arbitrary
  width/height scaling
- `luminance_to_char()` (private) — maps u8 luminance to char via linear
  index into char_map: `idx = luminance * (len - 1) / 255`
- 22 new tests: luminance→char mapping (black/white/mid/custom/empty/single),
  bilinear resize (identity/upscale/downscale/empty/single-pixel), ASCII
  output (all-white/all-black/custom-map/empty/zero-width), image→ASCII
  integration (PNG/custom-map/width/nonexistent/temp-image)

No `.unwrap()` in production — all fallible paths return `Result` or handle
edge cases with early returns. Terminal width detection falls back to 80.

### 2.1.3 — Colored ASCII output (24-bit ANSI)

Added 24-bit ANSI color support in `image_input.rs`:
- `RgbPixel` type alias `(u8, u8, u8)` for RGB triples
- `load_rgb_matrix()` / `rgb_from_dynamic()` — load image preserving original color via `to_rgba8()`
- `apply_grayscale()` — in-place BT.709 luminance conversion on RGB matrix
- `apply_negative()` — in-place invert: `(255-r, 255-g, 255-b)`
- `bilinear_resize_rgb()` — bilinear interpolation on `&[Vec<RgbPixel>]`
- `ansi_color_code(r, g, b)` — returns `"\x1b[38;2;{r};{g};{b}m"`
- `ansi_reset_code()` — returns `"\x1b[0m"`
- `ImageColorConfig` struct — `colored`, `grayscale`, `negative`, `char_map`, `target_width`
- `color_matrix_to_ascii()` — resizes, applies transforms, wraps chars in ANSI codes
- `image_to_colored_ascii()` — convenience wrapper loading image with config
- 10 new tests: RGB load, pixel preservation, grayscale in-place, negative in-place,
  ANSI format, reset code, colored output, grayscale flag, negative flag, bilinear resize RGB

No `.unwrap()` in production — all fallible paths return `Result`.
fmt and clippy pass clean.

### 2.1.4 — Braille art + dithering

Added braille art pipeline in `image_input.rs`:
- `BRAILLE_BASE` constant (U+2800) — Unicode braille starting codepoint
- `pixels_to_braille_char()` — maps 2×4 pixel block to single braille char via 8-dot bit ordering
- `floyd_steinberg_dither()` — error diffusion dithering with 7/16, 3/16, 5/16, 1/16 fractions
- `luminance_to_braille()` — converts luminance matrix to braille string, with optional dithering
- `image_to_braille()` — convenience wrapper: load image → grayscale → braille
- 10 new tests: all-blank, all-filled, each dot individually, multiple blocks, partial/odd-sized, empty, dither binary output, no-dither vs dither output, file integration

`.expect()` in `pixels_to_braille_char` for `char::from_u32` is a safe invariant (bits=0..255, base=0x2800, code always in valid Unicode range), following existing codebase convention.
fmt and clippy pass clean.

### 2.1.5 — Image CLI flags integration

Added `ImageOptions` struct, image CLI flags (`--image`/`-i`, `--map`, `--braille`/`-b`,
`--color`, `--grayscale`, `--negative`, `--dither`, `--width`, `--height`, `--dimensions`,
`--flipX`, `--flipY`), image mode dispatch, and `run_image()` entry point to `main.rs`.
Flip helpers for luminance and RGB matrices reside in `main.rs` (private functions).
17 flag parse tests + 2 integration tests covering every flag, defaults, short aliases,
multiple paths, and mode detection. No `.unwrap()` in production — all error paths use
`match`/`continue`. fmt and clippy pass clean.

### 2.2.1 — System font enumeration via font-kit

Created `font_gen.rs` with system font enumeration using `font-kit` crate:
- `FontFamilyInfo` struct with `family: String` and `styles: Vec<String>`
- `FontGenError` enum wrapping `SelectionError` and `FontLoadingError`
- `list_system_fonts()` — enumerates all installed font families via `SystemSource::all_families()`, loads first handle per family via `select_family_by_name()`, extracts style descriptions from font properties
- `list_monospace_fonts()` — filters system fonts using name heuristic ("Mono" substring) + `Font::is_monospace()` check
- Private helpers: `describe_style()`, `family_is_monospace()`, `load_styles()`
- 3 unit tests: non-empty font list, monospace filter produces subset, styles are populated
- `font-kit = "0.14.3"` enabled in Cargo.toml (was commented out)

### 2.2.3 — FIGfont header from font metrics

Added `generate_figfont_header()` and `generate_figfont()` in `font_gen.rs`.
- `generate_figfont_header(font)` — generates FIGfont header line:
  `flf2a<hardblank> <height> <baseline> <max_length> 0 0 -1 <full_layout> 0`
  Always uses old_layout=0 (full-size), comment_lines=0, print_direction=-1,
  codetag_count=0. Uses `format!` macro (infallible, no unwrap).
- `generate_figfont(font)` — generates complete `.flf` content: header + 95 ASCII
  chars (32-126) + 7 Deutsch chars + codetagged chars. Missing required chars use
  space-padded rows of `maxlength` width. Each row terminated with `@` endmark.
  Codetagged chars sorted by code for deterministic output.
- 5 new tests: header round-trip, default full-size layout, smush layout
  preservation (191), hardblank multi-byte (DEL), full font round-trip with
  placeholder chars. No test failures from font-kit (tests use `parse_header`
  and `parse_tlf_font` directly, no system font dependency).

### 2.2.4 — CLI command: `--create-font`

Added `system_font_to_figfont()` in `font_gen.rs`:
- Loads system font by name via font-kit, renders all 102 required chars
  (32–126 + 7 Deutsch) to monochrome bitmaps via `rasterize_glyph()`
- Converts bitmaps to FIGcharacter rows with correct baseline positioning
  using raster bounds origin_y for padding calculation
- Computes charheight/baseline from font metrics (ascent/descent in
  design units scaled by `point_size / units_per_em`)
- `FontGenError` gains `GlyphLoading(GlyphLoadingError)`, `FontNotFound(String)`,
  `NoGlyph(u32)` variants
- `pathfinder_geometry = "0.5"` added as direct dependency for
  `Transform2F` (needed by font-kit's `rasterize_glyph` API)

CLI integration in `main.rs`:
- `--create-font <name>` generates .flf from system font
- `--font-size <f32>` (default 12.0) controls pixel size
- `--output <path>` writes to file instead of stdout
- Handler placed before `-F` check, early return after generation

5 new tests: roundtrip metrics, parseable output, render known char,
nonexistent name error, size scaling.

### 2.2.5 — Create TUI iconset YAML file

Verified `assets/tui/icons.yaml` — 201 icon entries across 23 categories
(modes, tools, cursor, canvas, brush, palette, status, file ops, edit,
font editor, smushing rules, font transforms, image editor, text tool,
layers, blending, timeline, keyframes, export, settings, navigation,
dialogs, misc UI). Every entry uses `nf-*` Nerd Font icon prefix.

Added `serde_yaml` dev-dependency and integration test
`test_icons_yaml_all_keys_present` in `tests/tui.rs`:
- Compile-time embedded via `include_str!`
- Parses as `BTreeMap<String, String>`
- Asserts ≥120 entries
- Asserts every key non-empty
- Asserts every value starts with `nf-`

### 2.2.6 — Phase merge: release/2.2 → main

Merged all Phase 2.2 work into default branch (master). Phase 2.2 complete:
system font enumeration via font-kit (2.2.1), glyph rasterization to FIGcharacter
rows (2.2.2), FIGfont header generation from font metrics (2.2.3), `--create-font`
CLI command (2.2.4), TUI iconset YAML file (2.2.5). All 6 subtasks (2.2.1–2.2.6)
implemented, tested, merged. Phase 2.3 (TUI Core & Canvas) is next.

### 2.3.1 — TUI scaffold with ratatui

Created `figby-rs/src/tui.rs` — TUI scaffold with ratatui + crossterm:
- `AppMode` enum: `FontEditor`, `ImageEditor`, `AsciiPreview` with `title()` and `next()` cycling
- `TuiApp` struct: holds mode state, quit flag, icons map (from `icons.yaml`)
- `run()` — raw mode, alternate screen, event loop with render + event handling
- `render()` — vertical layout: toolbar (Tabs with 3 modes), main area (canvas + palette sidebar), status bar
- `handle_event()` / `handle_key_event()` — Tab cycles mode, q/Esc quits
- `--tui` CLI flag added to `main.rs` dispatches to TUI on startup
- `ratatui = "0.30.1"`, `crossterm = "0.28"` dependencies added; `serde_yaml` promoted to regular dep
- 3 smoke tests: all panels render, mode switching cycles correctly, default mode is FontEditor
- No `.unwrap()` in production — `serde_yaml::from_str` uses `unwrap_or_default()` for graceful fallback

### 2.3.2 — Toolbox bar

Created `figby-rs/src/tui/toolbox.rs` — shared toolbar with 10 tool variants:
- `Tool` enum: Brush, Marquee, Lasso, CircleSelect, PolygonSelect, Fill, Line, Eraser, Eyedropper, Text
- Each tool has `display_name()` (2-char label), `full_name()`, `key_shortcut()` (KeyCode), `icon_key()` (icons.yaml lookup)
- `Toolbox` struct wraps `selected: Tool` with `handle_key()`, `next()`, `prev()`, `render()`
- Keyboard shortcuts: V(select), B(brush), L(lasso), C(circle), P(polygon), G(fill), I(line), E(eraser), D(eyedropper), T(text)
- Active tool highlighted via `List` widget with cyan bold style
- Converted `tui.rs` → `tui/mod.rs` directory module for sub-module organization
- 3 tests: default tool is Brush, round-trip selection via all shortcuts, tool names appear in rendered output

### 2.3.3 — Canvas widget

Created `figby-rs/src/tui/canvas.rs` — scrollable/zoomable canvas widget:
- `CanvasCell` struct: `ch: char`, `fg: Option<Color>`, `bg: Option<Color>` with `Default` (space, no color)
- `CanvasBuffer` struct: 2D grid of `CanvasCell` with bounds-checked `get()`, `get_mut()`, `set()`. No `unwrap()` — all bounds errors return `Option`.
- `CanvasWidget` struct: owns `CanvasBuffer`, cursor position `(u16, u16)`, scroll offset `(u16, u16)`, zoom level `u8` (1-8), grid toggle `bool`. `impl Widget for &CanvasWidget` renders buffer cells into terminal area:
  - At zoom=1, each buffer cell = 1 terminal cell
  - At zoom=N, each buffer cell fills N×N block with its char
  - Grid overlay with `│`/`─`/`┼` at cell boundaries (dim style)
  - Cursor highlight via reversed style (rendered last to win over grid)
- `handle_key()` dispatches arrows (move cursor), `+`/`=` (zoom in), `-`/`_` (zoom out), `G` (toggle grid). Returns `bool` (handled).
- `ensure_cursor_visible()` auto-scrolls to keep cursor in view.
- Canvas placed before toolbox in key dispatch order.
- 6 integration tests: empty render, cell rendering, cursor movement, zoom in/out, cursor highlight style, grid characters at zoom=2.
- Memory entry on `Buffer::cell_mut` returning `Option<&mut Cell>` (non-panicking, matches invariants).

### 2.3.4 — Color palette

Created `figby-rs/src/tui/palette.rs` — color palette sidebar widget:
- `ColorTarget` enum: `Foreground`/`Background` with `toggle()` method
- `ANSI_16_COLORS` constant: 16 standard indexed colors (0-15)
- `extended_color()` helper: computes 240-color extended grid via page/offset → `Color::Indexed(idx.min(255))`
- `Palette` struct: owns target, selected_color, recent colors (max 8), selected_index, custom_hex input, extended mode/page state
- Keyboard: arrows navigate grid, Enter selects, `x`/`X` toggles FG/BG, `f`/`F` sets FG, `h`/`H` enters hex mode, `z`/`Z` toggles extended grid
- `set_custom_hex()` — parses `#RRGGBB` string via `u8::from_str_radix`, returns `bool` on success
- `apply_to_cell()` — applies selected color to `CanvasCell.fg` or `.bg` based on target
- `render()` — renders FG/BG indicator, color swatches (2 rows of 8), hex display, recent colors strip
- `push_recent()` — deduplicates and rotates recent colors, capped at 8
- Registered as `palette` module in `tui/mod.rs`, added `palette: Palette` field to `TuiApp`
- 8 integration tests in `tests/tui.rs`: default target, FG/BG toggle, selection, recent push, hex apply, apply to cell (fg/bg), render labels

### 2.3.5 — Brush selection

Created `figby-rs/src/tui/brush.rs` — brush shape picker and size controls:
- `BrushShape` enum: Square, Circle, SprayPaint, Custom with `cycle()` method
- `BrushState` struct: `shape: BrushShape`, `size: u8` (1..=20, clamped), `set_size()`,
  `size_up()`, `size_down()`, `cycle_shape()`
- `render_preview(max_size)` returns `Vec<String>` showing brush tip at current size
- `render()` ratatui widget: shows shape name, size, and preview in toolbox column
- Integrated into `TuiApp`: `brush` field, key events (`[` size down, `]` size up,
  `'` cycle shape), preview rendered below toolbox
- Status bar updated to show current brush shape and size
- No `.unwrap()` in production — all paths use proper Option/clamp arithmetic
- SprayPaint uses fixed seed 42 for deterministic output across test runs
- fmt and clippy pass clean

### 2.3.6 — Status bar + canvas settings

Created `figby-rs/src/tui/status.rs` with two widgets:
- `StatusBar` — renders cursor X,Y, zoom level, current tool name, mode name,
  unsaved indicator using Nerd Font icons from `icons.yaml`. Static `render()`
  method takes all display data as parameters (no stored state).
- `CanvasSettings` struct — settings panel with canvas width/height, font size,
  grid toggle, snap-to-grid toggle. `pub settings_open: bool` controls visibility.
  `handle_key()` navigates fields via ↑/↓/←/→, toggles booleans via Enter, closes
  via Esc. `render()` shows labeled fields with highlighted selection.

Integrated into `TuiApp`:
- `unsaved: bool` field (default `false`), `settings: CanvasSettings` field
- Status bar constraint changed from `Length(1)` to `Length(3)` (needs room for
  borders + 1 content line)
- Settings panel replaces palette sidebar when `settings_open` is true
- `S` key opens/closes settings, loading canvas state on open
- `apply_settings()` syncs canvas width/height/grid on each settings key event
- Settings mode blocks all other key handlers (canvas, toolbox, palette)
- `apply_settings()` — recreates canvas widget when dimensions change, toggles
  grid to match settings

10 integration tests covering all status bar fields (cursor, zoom, tool, mode,
unsaved indicator) and settings panel (toggle, width change, grid toggle,
snap-to-grid toggle). fmt and clippy pass clean.

### 2.4.1 — Brush tool

Added `tools/` subdirectory under `tui/` with `mod.rs` module root and `brush.rs`
execution module. Three core functions:
- `stamp_offsets()` — computes relative (dx, dy) offsets for Square, Circle,
  SprayPaint, and Custom brush shapes. Square fills N×N block, Circle uses
  euclidean distance ≤ radius, SprayPaint uses deterministic hash (seed 42, 35%
  density), Custom stamps only center cell.
- `paint_stamp()` — applies brush stamp at (cx, cy), clips to buffer bounds,
  no `unwrap()` in production (uses `get_mut` → `Option`).
- `paint_line()` — Bresenham line interpolation with per-step stamp calls.

Integrated into TUI:
- Mouse capture via `EnableMouseCapture`/`DisableMouseCapture` (crossterm `event`
  module, not `terminal`). Left-click places stamp, drag draws line, release
  resets drag origin.
- Keyboard painting: Space/Enter paints stamp at cursor when Brush tool active.
- `screen_to_buffer()` maps terminal coords to buffer coords using scroll/zoom.
- `canvas_inner_rect` tracks canvas rendering area for mouse→buffer conversion.
- `CanvasCell` gained `Copy` derive (all fields are Copy types).
- `CanvasWidget` gained `set_cursor()` and `scroll_offset()` methods.

14 unit tests: square coverage, circle shape, spray determinism, bounds clipping,
cell attributes, line directions (horizontal/vertical/diagonal/reverse), endpoint
clipping, size-1 square, custom-only-center.

### 2.3.7 — Phase merge: release/2.3 → main

Merged all Phase 2.3 work into default branch (master). Phase 2.3 complete:
TUI scaffold with ratatui (2.3.1), toolbox bar with tool selection (2.3.2),
scrollable/zoomable canvas widget (2.3.3), color palette sidebar (2.3.4),
brush shape picker with size/preview (2.3.5), status bar + canvas settings
panel (2.3.6). All 6 subtasks (2.3.1–2.3.6) implemented, tested, merged.
Phase 2.4 (Drawing Tools) is next.

### 2.4.6 — Eyedropper tool

Added `tools/eyedropper.rs` with `sample()` — bounds-checked cell lookup returning
`Option<CanvasCell>`. Integrated into TUI mouse handler: click samples cell char +
foreground color, sets `self.brush.ch` and `self.palette.selected_color`, pushes
color to recent colors, switches target to Foreground. `BrushState` gained `ch: char`
field (default `'\u{2588}'`) — all 6 hardcoded `ch: '\u{2588}'` in drawing tools
replaced with `self.brush.ch`. `Palette::push_recent` changed from `fn` to `pub fn`
to allow external call. Eyedropper excluded from keyboard paint (Space/Enter) and
mouse early-return. 5 unit tests: cell data, empty defaults, out-of-bounds,
no-foreground cell, char sampling. fmt and clippy pass clean.

### 2.4.7 — Spray paint brush

Added `tools/spray.rs` with stochastic spray stamp and Bresenham-spray line.
- `spray_stamp()` — iterates bounding box `[-radius, +radius]`, circle-check with
  `dx² + dy² ≤ r²`, paints with probability `density / 100.0` via `rand::Rng::gen_bool()`
- `spray_line()` — Bresenham interpolation calling spray_stamp at each step
- Uses `StdRng::seed_from_u64(thread_rng().gen())` for fresh randomness per click
  (different pattern each click); tests pass seeded `StdRng` for determinism
- `rand = "0.8"` added to Cargo.toml
- `BrushState` gained `density: u8` field (1–100, default 35), `set_density()`,
  `density_up()`, `density_down()` methods
- Density UI: `;` density down, `'` density up, brush shape cycle moved to `\`
  (was `'`), Settings `S` check moved before toolbox handler to avoid conflict
  with Spray tool shortcut `a` (aerosol)
- Spray preview in brush UI now reads `self.density` instead of hardcoded 35
- 6 tests: within-circle, density distribution (200 stamps @50% ±10%), stochastic
  different, deterministic seed, bounds clip, density 0/100 extremes

### 2.4.8 — Phase merge: release/2.4 → main

Merged all Phase 2.4 work into default branch (master). Phase 2.4 complete:
brush tool (2.4.1), eraser tool (2.4.2), line tool (2.4.3), fill/flood fill
tool (2.4.4), selection tools (2.4.5), eyedropper tool (2.4.6), spray paint
brush (2.4.7). All 7 subtasks implemented, tested, merged. Phase 2.5
(Font Editor Mode) is next.

### 2.5.1 — Font mode scaffold: glyph grid overview

Created `figby-rs/src/tui/font_editor.rs` with `FontEditor` struct:
- `FontEditorView` enum: `Overview` (char grid) or `CharEditor(u32)` (single char editing)
- Glyph grid renders all 102 required FIGcharacters (32-126 + 7 Deutsch) plus codetagged chars
- Each cell shows code label + mini FIGcharacter preview (cells sized by `maxlength × charheight+1`)
- Search/filter by char code or char value via `/` key activator
- Arrow keys navigate grid, Enter selects char → switches to `CharEditor` view
- Esc clears search or returns from `CharEditor` to `Overview`
- Font loaded at `TuiApp::new()` from `fonts/standard.flf` (graceful `None` on failure)
- `sync_font_char_to_canvas()` populates canvas with FIGcharacter rows on char selection
- Status bar shows `"Font Editor [U+XXXX]"` when editing a specific char
- No `.unwrap()` in production — font loading uses `if let Ok(font)`
- `/` key activates search (avoids conflict with tool shortcuts `b`,`v`,`l`, etc.)
- 7 integration tests: grid, search by code, search by char, select+open, Esc return, empty font, grid navigation

### 2.5.4 — Smushing rule configuration

Added `FontEditorView::SmushRuleEditor` variant with visual toggle grid for all 6
horizontal smushing rule bits (EQUAL_CHARS, UNDERSCORE, HIERARCHY, PAIR, BIGX,
HARDBLANK). `SMUSH_RULE_LABELS` constant maps rule names → `SmushMode` bit constants.
`smush_selected: usize` tracks cursor position in toggle list (wraps at bounds).
`render_smush_editor()` — bordered panel showing `[X]`/`[ ]` checkboxes with
reverse-highlight cursor, live preview of `'/' + '\\'` smush via `smush_horizontal()`,
and layout value/binary display.
`handle_key_smush_editor()` — Up/Down wrap-navigate, Enter/Space XOR-toggles rule bit
in `font.full_layout`, Esc returns to Overview. `'S'` key in overview handler opens
smush editor (overrides settings panel `'S'` in FontEditor mode because font_editor
handler runs first in `mod.rs`). 5 integration tests: open/close, single toggle,
multiple toggles with cumulative bitmask, navigation wrap, preview changes on toggle.

### 2.5.3 — FIGfont header / layout editor

Added `FontEditorView::HeaderEditor` variant with inline field editor for all 7
FIGfont header properties:
- `HEADER_FIELD_LABELS` constant lists field names: Hardblank, Char Height,
  Baseline, Max Length, Full Layout, Print Direction, Comment Lines
- `editing_field` / `edit_buffer` / `error_message` state for inline text input
- `enter_header_editor()` method switches view, resets cursor to field 0
- `render_header_editor()` — bordered panel showing all fields with highlight
  cursor, editing state (green bold), and error messages (red)
- `handle_key_header_editor()` — Up/Down nav, Enter toggles edit, Esc cancels
  or returns to Overview, chars/Backspace edit buffer. Validation: height≥1,
  baseline≤height, hardblank single char, print_direction ∈ {-1,0,1}
- `save_current_field()` parses and validates via `parse::<u32>()`/`parse::<i32>()`
- `'H'` key in overview opens header editor
- `mod.rs` render dispatch changed from `Overview`-only to `!CharEditor` so
  HeaderEditor routes to font_editor render
- 10 integration tests: open/close, all 7 field edits (charheight, baseline,
  hardblank, full_layout, print_direction, comment_lines, maxlength),
  rejection of height=0 and baseline>height

### 2.5.5 — Add/remove codetagged characters

Added `CodeInputMode` enum (Add, CopySource, CopyDest, DeleteConfirm) and
state fields (`code_input_active`, `code_input_buffer`, `copy_source_code`)
to `FontEditor`. Four core methods:
- `add_char(code)` — creates space-padded FIGcharacter, inserts into font,
  rebuilds `all_codes`, selects new char in grid
- `delete_char(code)` — removes from font, ensures code 0 (missing char)
  still exists, rebuilds `all_codes`
- `copy_char(src, dst)` — clones rows from src to dst (or creates space-
  padded default if src missing), rebuilds `all_codes`
- `ensure_missing_char()` — creates space-padded code 0 if absent

Rendering: code input prompt shown above grid when `code_input_active`.
Key handlers: `A` starts add flow, `D` starts delete confirm (Y/N prompt),
`C` starts two-step copy flow (source → destination). Digit entry for code,
Backspace/Enter/Esc for standard editing. Codepoint validation rejects
surrogates (0xD800-0xDFFF) and values > 0x10FFFF.
14 new unit tests covering all operations, edge cases, and buffer management.

### 2.5.6 — Font-level transform tools

Added `FontEditorView::TransformEditor` variant with 6 font-level transforms:
- **Resize**: changes `charheight`, adds/removes rows from all glyphs, clamps baseline, recalculates `maxlength`
- **Italicize**: prepends row-index spaces to each row of every glyph, recalculates `maxlength`
- **Bold**: duplicates every character in each row (doubles width), recalculates `maxlength`
- **Mirror**: 3 submodes — Horizontal (reverse each row), Vertical (reverse row order), Both (compose both)
- **Copy Glyph**: loads external FIGfont by name via `load_font()`, extracts glyph by code, inserts into current font
- **Rename**: updates `font_storage_name` (in-memory only, no file I/O)

Transform editor UI: navigable list (`↑`/`↓`), Enter activates, parameter input for Resize/CopyGlyph/Rename, submenu for Mirror. `T` key in overview opens transform editor. Transforms clear undo/redo stacks (bulk operations incompatible with per-char undo).

`MirrorMode` enum with cycle/prev/next navigation. `transform_copy_glyph_from()` accepts `fontdir` parameter for testability.
32 new unit tests + 6 new integration tests covering all transforms, empty-font safety, parameter input flow, and multi-transform consistency. Only `font_editor.rs` and `mod.rs` touched. fmt and clippy pass clean.

### 2.5.7 — Phase merge: release/2.5 → main

- **Merge commit `b6d340f`** — release/2.5 merged into main.
- Phase 2.5 complete: all 7 subtasks (2.5.1–2.5.7) implemented, tested, merged.
- **Documentation**: `docs/todo-v2.md` task checked off; this memory entry added.
- **No code changes** — merge was performed externally; only doc state synced.
- **Next up**: Phase 2.6 — Image Editor Mode.

### 2.6.1 — Image import + canvas display

Created `figby-rs/src/tui/image_editor.rs` with `ImageEditor` struct and `AsciiMode`
enum (Color/Grayscale). `ImageEditor` supports:
- Path entry via keyboard (`o` to open, type path, Enter to load, Esc to cancel)
- Image loading via `load_from_path()` using `image_input`'s `load_rgb_matrix()`
  and `bilinear_resize_rgb()` with target_width=80 and aspect-corrected 0.5× height
- Color mode: per-cell ANSI RGB colors stored in `CanvasCell.fg` via `ratatui::Color::Rgb`
- Grayscale mode: luminance-only chars with `None` foreground
- Mode toggle via `c`/`C` key, re-renders from cached `original_rgb` matrix
- Block title shows path entry buffer and error messages
- Status bar shows current mode (Color/Grayscale)

Integration in `tui/mod.rs`:
- `ImageEditor` field on `TuiApp`, initialized in `new()`
- `sync_image_to_canvas()` resizes `CanvasWidget` to match image cells dimensions
- Image editor key dispatch placed before canvas/tools in `handle_key_event()`
- Render sync in canvas rendering block alongside font editor sync

Made `bilinear_resize`, `bilinear_resize_rgb`, `luminance_to_char` public in
`image_input.rs`. 8 unit tests in `image_editor.rs`: grayscale load, color load,
nonexistent path error, mode toggle, CLI output match, canvas render, path entry
key handling, key mode toggle. fmt and clippy pass clean.

### 2.6.2 — Text tool with FIGlet font overlay

Created `tools/text.rs` with `TextToolState` struct:
- `TextToolState::new(font_dir)` — scans fonts/ for `.flf`/`.tlf` files, builds list
- `list_available_fonts(font_dir)` — reads directory, returns sorted deduplicated names
- `load_selected_font()` — loads FIGfont by name from current `font_index`, stores in `font: Option<FIGfont>`
- `render_text_to_buffer()` — uses `add_char()` pipeline to render `text_buffer` as FIGlet rows with kerning/smushing (font's `full_layout` mode), stamps non-space cells into `CanvasBuffer` at `cursor_position` with scale and color
- `render_options()` — Paragraph widget showing font name, justification (L/C/R), scale (1-4), color, text entry status

Integration in `tui/mod.rs`:
- `text_tool` field on `TuiApp`, initialized in `new("fonts")`
- Render conditionally swaps brush panel for text options when `Tool::Text` selected
- Mouse click sets `cursor_position` + enters text entry mode (`entering_text = true`)
- Key entry mode: letters/space/punctuation → buffer, Enter → render+clear+exit, Esc → cancel, Backspace → pop
- Non-entry mode: Up/Down → font navigation, j/J → justification cycle, +/- → scale, Space/Enter → enter text mode
- Font nav handled before canvas (prevents arrow conflicts), tool settings handled after canvas

14 unit tests: single char render, multi char, left/center/right justification, color apply, font switch, scale factor, edge clipping, empty text noop, no-font panic, entering text state, font listing nonempty and nonexistent. fmt and clippy pass clean.

### 2.6.3 — Text tool advanced: selection + transform

Added `TextBlock` struct with fields: `id`, `text`, `font_index`, `x`, `y`, `scale`,
`justification`, `text_color`, `rotation` (0/90/180/270), `cached_rows`, `width`,
`height`. Added `blocks: Vec<TextBlock>`, `selected_block: Option<usize>`,
`next_block_id: usize` to `TextToolState`.

Core methods:
- `commit_block()` — renders current text through FIGlet pipeline via private
  `render_rows_from_buffer()`, caches rows/width/height, pushes new `TextBlock`
- `re_edit_block(idx)` — loads block text/font/scale/justification/color back into
  current editing fields, removes block from list, enters text mode
- `hit_test(x, y)` — iterates blocks checking point-in-bounding-box
- `move_selected_block(dx, dy)` — updates block x/y with `wrapping_add`
- `scale_selected_block(delta)` — clamps 1..=4, updates block scale
- `rotate_selected_block()` — cycles 0→90→180→270 via `% 360`
- `delete_selected_block()` — removes from blocks, clears selection
- `compute_bounding_box(idx)` — returns rect accounting for rotation (swaps w/h
  for 90/270) and justification (left/center/right x offset)
- `render_block_to_overlay(idx)` — returns `TextOverlay` struct for canvas rendering

Added `TextOverlay` struct to `canvas.rs` with `x`, `y`, `rows`, `color`, `scale`,
`rotation` fields. `CanvasWidget` gained `text_overlays` and `text_block_perimeter`
fields. `Widget::render` extended with:
- Text overlay rendering: iterates rows/chars, applies rotation transforms
  (0° direct, 90° transpose+reverse row, 180° reverse both, 270° transpose+
  reverse col), stamps char into scaled/zoomed terminal cells with color
- Text block perimeter: dashed yellow marquee around selected block

Integration in `mod.rs`:
- `render()` populates `text_overlays` and `text_block_perimeter` from blocks
  when text tool is active
- `handle_key_event()`: block ops (arrows/+-/r/Backspace/Enter/Esc) when text
  tool active with selected block; Enter in entry mode calls `commit_block()`
- `handle_mouse_event()`: `hit_test` on click when not entering text — select
  block if hit, enter text mode if miss

9 new unit tests: create, multiple, hit-test, move, scale, rotation, delete,
re-edit, bounding box. No `.unwrap()` in production. fmt and clippy pass clean.

### 2.6.4 — Image adjustments

Added brightness/contrast/threshold sliders, dither/invert/braille toggles,
and target width adjustment to `ImageEditor`. Three new public functions in
`image_input.rs`:
- `apply_brightness()` — adds i16 delta to each R/G/B channel, clamped 0-255
- `apply_contrast()` — scales distance from 128 by factor, clamped 0-255
- `rgb_to_luminance_matrix()` — converts RGB matrix to BT.709 luminance

`ImageEditor` gained 7 new fields (`adjustment_mode`, `brightness`, `contrast`,
`threshold`, `dither`, `invert`, `braille`) and 3 core methods:
- `reapply_adjustments()` — clones `original_rgb`, resizes, applies brightness
  → contrast → invert, then either braille pipeline (luminance → dither →
  braille chars) or standard `rgb_to_cells()` conversion
- `reset_adjustments()` — resets all 6 params to defaults, re-renders
- `adjustment_status()` — returns summary string for title bar

Key bindings (non-path-entry): `b`/`k`/`t`/`w` set adjustment mode,
`+`/`-` adjust current parameter (step: brightness=5, contrast=0.1,
threshold=8, width=4), `i` invert toggle, `d` dither toggle, `y` braille
toggle, `r` reset, `Esc` clears mode.

Status bar and mode title both show active adjustments (e.g. `B:+50 Inv
Braille Gray` or `Brightness[+50]` when actively adjusting). All adjustments
re-render canvas in real time via `sync_image_to_canvas()` in `mod.rs`.

16 new unit tests in `image_editor.rs`: brightness inc/dec, contrast,
invert toggle+restore, threshold change, dither toggle, target width,
reset, braille range check, adjustment persistence across mode toggle,
key binding selectors, +/- step tests, direct toggle keys, reset key.
fmt and clippy pass clean.

### 2.6.5 — Phase merge: release/2.6 → main

Merged all Phase 2.6 work into default branch (master). Phase 2.6 complete:
image import + canvas display (2.6.1), text tool with FIGlet font overlay
(2.6.2), text blocks selectable/movable/scalable/rotatable/re-editable (2.6.3),
image adjustments (brightness/contrast/threshold/dither/invert/resize) (2.6.4).
All 4 subtasks (2.6.1–2.6.4) implemented, tested, merged. Phase 2.7 (File
Operations & Persistence) is next.

### 2.7.1 — Save / Save As

Created `figby-rs/src/tui/file_ops.rs` with `FileOpsDialog` (file browser
overlay widget), `save_font()` function, and `FileOpsMode` enum (Idle/SaveAs
/AutoSaveConfig). Key behaviors:
- `save_font(font, path)` — generates `.flf` content via `generate_figfont()`,
  writes to temp file, atomically renames to target path. Returns `io::Result`.
- `FileOpsDialog` — TUI overlay with path text entry, directory listing
  (`.flf`/`.tlf` files + subdirectories), keyboard navigation (arrows, Tab
  to select entry, Enter to confirm, Esc to cancel).
- Ctrl+S in Font Editor mode: saves directly if `current_path` set, opens
  Save As dialog otherwise. Ctrl+Shift+S: always opens Save As dialog.
- Auto-save timer: `auto_save_interval` (seconds, 0=disabled) checked in
  `handle_event()` loop. Saves current font when timer elapses and `unsaved`
  is true.
- `FontEditor` gained `current_path: Option<PathBuf>` field for tracking
  file location.
- Status bar shows filename (with `*` prefix if unsaved) and save key hints
  (`^S Save | ^S+S Save As`).
- Atomic write via `write()` to `.tmp` file then `fs::rename()` prevents
  partial save corruption.
- 8 unit tests: roundtrip save+reload byte-exact, valid `.flf` generation,
  error handling for invalid paths, dialog state management, path extension
  logic. No `.unwrap()` in production. fmt and clippy pass clean.

### 2.7.3 — Copy / duplicate font

Added font duplication and import features to `FontEditor`:
- `transform_duplicate()` — clones current font into `original_font` field, sets cloned
  font as active, clears `current_path`, sets `font_storage_name` to `"Untitled Copy"`,
  resets undo/redo stacks. Enables "edit one, verify other unchanged" workflow.
- `transform_import_font(name, fontdir)` — loads external `.flf`/`.tlf` via `load_font()`,
  merges every glyph into current font via `font.chars.insert()` (last-wins for duplicates).
- `original_font: Option<FIGfont>` field added to `FontEditor` — stores pre-duplicate font
  state for independence verification.
- TRANSFORM_LABELS expanded from 6 to 8 entries: added "Duplicate Font" (index 6, immediate
  action, no input) and "Import Font" (index 7, prompts for font name).
- Existing tests updated for 8-transform navigation. 7 new unit tests: duplicate font,
  duplicate independence, import merges glyphs, import overwrites duplicates, duplicate
  empty font, import nonexistent font. Only `font_editor.rs` touched. fmt and clippy pass clean.

### 2.7.4 — Export: PNG, TXT, GIF

Created `output.rs` — pure-function output module with:
- `ExportFormat` enum (Png/Txt/Gif), `ExportError` enum
- `BITMAP_FONT_8X16` — 95-char × 16-byte VGA 8×16 bitmap font (public domain)
- `color_to_rgb()` / `xterm_to_rgb()` — ratatui `Color` → (r,g,b) conversion, 256-color xterm palette
- `rasterize_char()` — renders char to RGBA pixel grid at 1×-4× scale
- `render_frame()` — full frame rasterization from CanvasCell grid
- `export_cells_to_png()` — RGBA PNG bytes via `image::codecs::png::PngEncoder`
- `export_cells_to_txt()` — flat ASCII text, no color codes
- `export_cells_to_gif()` — animated GIF via `gif` crate, truecolor frames, infinite loop

Created `tui/export.rs` — TUI export dialog:
- `ExportMode` enum (Png/Txt/Gif) with cycle/label/extension helpers
- `ExportDialog` struct: active flag, format, path buffer, font size (1-4, default 2)
- `enter_export(mode)` / `close()` / `handle_key(code)` / `render(frame, area)` methods
- Keyboard: T cycles format, arrows/Tab navigate directory, Enter exports, Esc cancels
- `perform_export(cells)` — calls output module, writes to file, sets error on failure

Integration in `tui/mod.rs`:
- `export_dialog: export::ExportDialog` field on `TuiApp`
- Ctrl+E opens export dialog (Png for ImageEditor/AsciiPreview, Txt for FontEditor)
- Dialog overlay rendered at same position as file_ops overlay
- Key dispatch routes to dialog when active, performs export on Enter finalization

Added `gif = "0.13"` dependency to Cargo.toml. 23 new tests across output.rs + export.rs
covering PNG/TXT/GIF export, roundtrip, size checks, dialog open/close/format/path entry.
fmt and clippy pass clean.

Self-review fix: reordered `handle_key()` match arms so `T`/`t` format toggle
precedes generic `Char(c)` catch-all. Fixed 3 tests with wrong expected values
(roundtrip pixel coords, path entry account for "export.png" prefix).

### 2.7.5 — Config file

Created `figby-rs/src/config.rs` with `FigbyConfig` struct (`#[derive(Deserialize)]`)
and TOML parsing. Sections:
- `[cli]` — `font`, `output_width`, `color_mode` (all `Option<T>`)
- `[tui]` — `theme`, `recent_files_max` (both `Option`)
- `[tui.brush]` — `shape`, `size`, `density`, `ch` (all `Option`)

Private helpers `config_file_path()` (respects `XDG_CONFIG_HOME`, fallback
`~/.config/figby/config.toml`), `config_dir()` (parent dir, shared with
`RecentFiles`), and public `load_config()` (returns defaults on any error —
no `unwrap()` in production).

Integration in `main.rs`:
- `CliConfig` gained `color_mode: Option<String>` field
- `from_args_with_config(args, config_file)` — applies config values as
  fallback, then CLI flags override
- `main()` calls `config::load_config()` before CLI dispatch

Integration in `tui/mod.rs`:
- `TuiApp::new()` loads config, applies brush defaults (shape/size/density/ch)
  to `BrushState`, and `recent_files_max` to `RecentFiles`

Integration in `file_ops.rs`:
- `RecentFiles::storage_path()` now derives from `config_dir()` → `recent_files.json`
  (was `XDG_DATA_HOME/figby/recent.json` or `~/.figby/recent.json`)
- Added `set_max()` method to `RecentFiles`

17 unit tests: full config parse, partial (CLI-only, brush-only), empty TOML,
missing file returns defaults, bad TOML returns defaults, CLI override hierarchy
(4 tests: CLI wins, config fallback, partial mix, color_mode field), color_mode
default, recent files roundtrip (updated for new path). fmt and clippy pass clean.

### 2.7.6 — Undo/redo system

Created `figby-rs/src/tui/undo.rs` with `UndoEntry` (buffer + label) and
`UndoSystem` (undo/redo Vec stacks, configurable limit, batch support for
drag operations). Key methods: `push_snapshot()`, `undo()`, `redo()`,
`begin_batch()`/`end_batch()`, `clear()`, `can_undo()`, `can_redo()`.
Batching: during a drag sequence, only the first snapshot pushes; subsequent
pushes are discarded until `end_batch()`. No `unwrap()` in production.

Created `figby-rs/src/tui/undo_panel.rs` with `UndoPanel` — toggleable overlay
showing undo history entries with scroll and cursor indicator.

Modified `figby-rs/src/tui/mod.rs`:
- Added `undo` and `undo_panel` fields to `TuiApp`
- `push_undo_snapshot(label)` helper captures canvas state
- Snapshots pushed before: brush/eraser/line/fill/spray actions (mouse + keyboard),
  text block operations, selection operations (move/delete/cut/paste)
- Batched undo for mouse-drag operations (begin_batch on Down, end_batch on Up)
- Ctrl+Z undo, Ctrl+Y / Ctrl+Shift+Z redo
- Ctrl+Shift+H toggles undo history panel
- Undo cleared on: canvas resize, font load, image load, mode switch

Modified `figby-rs/src/tui/font_editor.rs`:
- Removed per-char `undo_stack`/`redo_stack` fields, `undo_char()`/`redo_char()`
  methods, and Ctrl+Z/Y handling from char editor — delegates to global undo

Modified `figby-rs/src/config.rs`: Added `undo_limit: Option<usize>` (default 50 in
code, no limit in config means default).

16 unit tests: push/pop, undo/redo cycle, multiple actions, limit enforcement
(60→50), clear, batch first-pushes-rest-discarded, two batches independent,
empty undo/redo returns None, history entries order, redo label. fmt and clippy
pass clean.

### 2.8.2 — Remove termion, use crossterm everywhere

Replaced `termion::terminal_size()` with `crossterm::terminal::size()` in:
- `main.rs:547` (`get_columns()`)
- `image_input.rs:186` (image→ASCII fallback width)

Removed `termion = "4"` dependency from `Cargo.toml`. Both functions share
identical return type `Result<(u16, u16), io::Error>` — drop-in replacement.
No import changes needed (fully-qualified paths used at both sites).
No other `termion` usage found in codebase. fmt and clippy pass clean.

### 2.8.3 — Use ratatui init/restore convenience functions

Replaced manual terminal setup/teardown in `figby-rs/src/tui/mod.rs`:
- `enable_raw_mode()` + `EnterAlternateScreen` + `Terminal::new(CrosstermBackend::new(...))` → `ratatui::init()`
- `disable_raw_mode()` + `LeaveAlternateScreen` + `show_cursor()` → `ratatui::restore()`
- Removed `EnableMouseCapture`/`DisableMouseCapture` (per task spec)
- Removed all crossterm terminal imports (`disable_raw_mode`, `enable_raw_mode`,
  `EnterAlternateScreen`, `LeaveAlternateScreen`) and crossterm event imports
  (`EnableMouseCapture`, `DisableMouseCapture`)
- `ratatui::init()` installs panic hook that restores terminal on crash (handles
  `disable_raw_mode` + `LeaveAlternateScreen` automatically)
- Removed local `use ratatui::backend::CrosstermBackend` / `use ratatui::Terminal`
  in `run()` — `init()` returns `DefaultTerminal` with correct type
- Bracketed paste (`EnableBracketedPaste`/`DisableBracketedPaste`) kept as-is
  (not managed by init/restore)
- Only `figby-rs/src/tui/mod.rs` modified. fmt and clippy pass clean.

### 2.8.4 — Phase merge: release/2.8 → master

Merged all Phase 2.8 work into default branch (master). Phase 2.8 complete:
Component architecture with `Component` trait + `Action` enum for cross-component
communication (2.8.1), removed termion dependency in favor of crossterm-only
terminal size detection (2.8.2), replaced manual TUI init/teardown with ratatui
convenience functions `ratatui::init()`/`ratatui::restore()` (2.8.3).
All 3 subtasks (2.8.1–2.8.3) implemented, tested, merged. Phase 2.9 (UI Polish
& Third-Party Widgets) is next.

### 2.9.1 — Add `tui-menu` ratatui widget

Added `tui-menu = "0.3.1"` dependency to `Cargo.toml`. Created `figby-rs/src/tui/menu.rs`
with `MenuAction` enum (17 variants for File/Edit/View/Tools/Help) and `MenuBar` struct
wrapping `MenuState<MenuAction>`. `MenuBar` handles keyboard (Alt+F/E/V/T/H to open,
Enter/arrows to navigate, Esc to close) and mouse (click menu labels). `handle_menu_action()`
delegates to existing methods: `start_open/save/save_as()`, undo/redo, zoom, tool selection,
clipboard ops, grid toggle, undo panel toggle, export dialog, quit.

Integration in `mod.rs`:
- Layout changed from 3 chunks to 4: menu bar (1 line), tabs (3 lines), main (min), status (3)
- `menu_bar` field on `TuiApp`, initialized in `new()`
- `handle_key_event()`: menu active guard before undo/redo; Alt+key activation before normal flow
- `handle_mouse_event()`: menu bar click intercepted first
- `drain_actions()` called after menu key events in both keyboard and mouse paths
- `Action::Menu(MenuAction)` variant added to `Action` enum

`tui-menu` does not handle mouse clicks on dropdown items — only menu bar labels.
Keyboard navigation works for submenus via Enter/arrows. fmt and clippy pass clean.

### 2.9.2 — Add throbber for async tasks

Created `figby-rs/src/tui/throbber.rs` with `ThrobberState` struct:
- Braille spinner sequence (`⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏`) as frames
- `tick()` advances frame index, wraps at `frames.len()`
- `start(msg)` / `stop()` control active flag + optional message
- `is_active() -> bool` getter
- `render_string()` returns spinner char + message when active, empty string when inactive
- 7 unit tests: tick cycle, start/stop, render active/inactive, frame change, inactive tick noop, multiple start/stop

Async thread spawning in `mod.rs`:
- `AsyncResult` enum with `SaveComplete`, `OpenComplete`, `ExportComplete`, `AutoSaveComplete` variants
- Long operations (`perform_save`, `perform_open`, `perform_export`, `start_save`, auto-save) spawn `std::thread::spawn` with cloned data
- `mpsc::channel` sends results back to main thread
- `check_async_completion()` polls channel via `try_recv()` during each render frame with `&mut self`
- `Double-spawn guard`: `throbber.is_active()` checked before spawning any new thread
- Auto-save also guards on `throbber.is_active()` and spawns asynchronously

Status bar integration:
- `throbber_text: String` field on `StatusBarComponent`
- Throbber string appended to status bar line when active (e.g. `⠋ Saving...`)
- `render_string()` called each frame and set on status bar before draw

No `.unwrap()` in production — all thread results handled via `match`. fmt and clippy pass clean.

### 2.9.3 — Prettier status bar (LazyVim/Starship style)

Redesigned `StatusBarComponent` in `components/status_bar.rs` with three-section
layout (left/center/right) separated by `│`:

- **Left section**: color-coded mode indicator (blue=FontEditor, green=ImageEditor,
  yellow=ASCIIPreview), tool name with icon, cursor X/Y with crosshair icon, zoom
  level with search icon
- **Center section**: filename (with unsaved/saved dot), undo count (if > 0)
- **Right section**: smoothed FPS counter (EMA with α=0.1), layer/Frame stubs (1/0
  until Phase 3.x), UTC clock, git branch (if in repo)

New fields added to `StatusBarComponent`: `mode`, `undo_count`, `fps`, `git_branch`,
`clock_str`, `layer_count`, `animation_frame`.

FPS tracking in `TuiApp`: `last_frame_time: Instant` + `fps: f64` fields. Computed
as exponential moving average of instant frame rate each render cycle. Clock
formatted as UTC HH:MM:SS via `SystemTime` (no new deps). Git branch detected
once at startup via `git rev-parse --abbrev-ref HEAD`.

### 2.9.5 — Migrate mode tabs to `Tabs` widget

Changed `_icons` field to `pub icons` on `TuiApp` so tab rendering can read
icon glyphs. Added `prev()` method to `AppMode` for Ctrl+Shift+Tab backward
cycle. Rebuilt tab labels in `render()` using icons from `icons.yaml`
(`mode_font_editor`, `mode_image_editor`, `mode_ascii_preview`) with fallback
to plain labels. Removed `Block` with `"Mode"` title border wrapping tabs.
Set inactive tab style to `theme.general.secondary` and active tab highlight
to `theme.general.primary` (was `warning`/yellow). Replaced bare `KeyCode::Tab`
handler with Ctrl+Tab (forward) and Ctrl+Shift+Tab (backward) mode cycling,
both with `undo.clear()`. Only `figby-rs/src/tui/mod.rs` modified.

### 2.9.7 — Phase merge: release/2.9 → master

Merged all Phase 2.9 work into default branch (master). Phase 2.9 complete:
tui-menu integration with menu bar (2.9.1), throbber widget for async tasks
(2.9.2), LazyVim/Starship-style status bar redesign (2.9.3), YAML theming
system with default theme (2.9.4), icon-based mode tabs with Ctrl+Tab cycling
(2.9.5), brush tool 5×5 mini-preview display (2.9.6).
All 6 subtasks (2.9.1–2.9.6) implemented, tested, merged. Phase 2.10 (Major
Release) is next.

### 2.10.2 — v2 major milestone RC — human sign-off

Created `rc/2.0.0-rc.1` branch and annotated tag `2.0.0-rc.1` from
`release/2.10` tip (af0111f). Stale RC infrastructure (old `rc/2.0.0-rc.1`
branch and `2.0.0-rc.1` tag from Phase 2.0) deleted and recreated.
Handoff for human sign-off and merge to master.

### 3.2.4 — Phase merge: release/3.2 → main

Merged all Phase 3.2 work into default branch (master). Phase 3.2 complete:
glyph grid mouse click+double-click (3.2.1), glyph char editor cursor+cell
toggle (3.2.2), font preview strip in overview (3.2.3).
All 3 subtasks (3.2.1–3.2.3) implemented, tested, merged. Phase 3.3
(Major Release) is next.

### 3.2.5 — Phase merge: release/3.2 → main

Merged Phase 3.2 (v4) work into default branch (master). Phase 3.2 complete:
AnimationTimeline widget (3.2.0), frame management (3.2.1), keyframing (3.2.2),
tweening (3.2.3), GIF export (3.2.4). Tasks 3.2.2-3.2.4 were no-ops due to ID
collision with v3 tasks (already completed in v3 scope). `master` and
`release/3.2` were already at the same commit — merge was a no-op.

### 3.3.1 — Full regression: all features vs v2.x baseline

Added 30+ new integration tests in `tests/tui.rs` covering every v2 feature
after the 3.1 ratatui refactor:
- **Brush/Eraser/Line/Spray keyboard paint** via Space/Enter at cursor position
- **Eyedropper tool** — verified Space does not paint (excluded from keyboard paint)
- **Text tool** — text entry mode, buffer population, commit/cancel cycle
- **Selection operations** — programmatic marquee creation, copy/delete/paste,
  cut, Esc deselect via direct Selection API
- **Undo/Redo** — Ctrl+Z undoes paint, Ctrl+Y and Ctrl+Shift+Z redo
- **File operations** — FileOpsDialog open/close, Save As path entry (direct API)
- **Export dialog** — Ctrl+E opens export in ImageEditor mode, format toggle
  (Png→Gif→Txt→Png), path entry with Backspace and Esc close
- **Image Editor** — Tab cycles to ImageEditor mode, C toggles Color/Grayscale
- **Menu bar** — Alt+F opens File menu, Down+Enter selects item, Alt+E opens
  Edit menu, Redo item navigation, Help>Keybindings flow
- **Layout/drawer** — `?` cycles Palette→BrushKeys→Closed
- **Zen mode** — F11 toggles full-screen canvas
- **Keybindings overlay** — toggle via programmatic state, Esc closes
- **Canvas scroll** — cursor movement beyond viewport updates scroll offset
- **Canvas grid** — G key toggle
- **Palette FG/BG** — `x` toggles, `f`/`F` sets Foreground (ImageEditor mode)
- **Font editor CharEditor** — Enter opens, Space toggles cell
- **Canvas `ensure_cursor_visible`** — scrolls to keep cursor in viewport
- **Selection perimeter** — Delete key clears selected cells
- **Polygon select tool** — tool selection via 'p' key
- **CLI dispatch** — ViewZoomIn/Out/ToggleGrid via menu, ToolsSelect via menu

Key discovery: several global keyboard shortcuts (Ctrl+O/S/Shift+S, Ctrl+K) are
intercepted by the font editor overview handler which doesn't check KeyModifiers.
This is a pre-existing dispatch bug in `font_editor.rs:handle_key_overview` —
it only inspects `KeyCode`, not modifiers, so Ctrl+char combos get treated as
search input. Filed as separate issue; not part of regression scope.

### 3.2.1 (v4) — Frame management

Frame management operations for `AnimationTimeline`: `add_frame`,
`insert_frame`, `remove_frame`, `duplicate_frame`, `reorder_frame` on
`TimelineState`. Onion skinning rendering toggle
(`AnimationTimeline.onion_skinning`). Each `TimelineFrame` stores full
layer state via `layer_state: Option<CanvasBuffer>`. 11 new tests.

### 4.1.4 — Welcome screen on startup

Created `figby-rs/src/tui/welcome.rs` with `WelcomeScreen` struct (`show: bool`,
default `true`). Renders a centered welcome overlay showing Figby + version,
recent files list (numbered 1-9 shortcuts), and keybinding hints (Ctrl+O Open,
Ctrl+N New, S Settings, ? Help, Esc dismiss).

Integration in `figby-rs/src/tui/mod.rs`:
- `welcome_screen` field on `TuiApp`, initialized in `new()`
- `render()` checks `welcome_screen.show` early, renders welcome as full-screen
  overlay with `render_overlays()` stacked on top (so keybindings overlay works
  on top of welcome)
- `handle_key_event()` checks welcome after settings but before mode-specific
  dispatch: Dismiss (Esc), OpenRecent (1-9), Open (Ctrl+O), NewFile (Ctrl+N),
  ToggleHelp (?), OpenSettings (S)
- All constructive actions dismiss the welcome screen; only ToggleHelp keeps it
  visible (keybindings overlay renders on top)

Key design decisions:
- Welcome placed after dialog/settings checks but before mode handlers so open
  dialog and settings panel can receive keys after welcome dismisses
- 1-9 digit keys open recent files directly via `perform_open()` with path from
  `recent_files.get(idx)`
- Ctrl+N creates a new blank font (clears font, undo, path, resets canvas to 32×16)

4 files touched: `welcome.rs` (new), `mod.rs` (modified). fmt and clippy pass clean.

### 4.1.5 — ZIP file browsing in file open dialog

Added ZIP archive browsing to the TUI file open dialog. `.zip` files appear as
navigable entries in the file browser. Selecting a `.zip` enters ZIP browsing
mode, listing the `.flf`/`.tlf` entries inside. Selecting a font reads it from
the ZIP via `read_zip_entry()` and parses it with the existing parser.

Key functions added to `font.rs`:
- `list_zip_font_entries(path)` — enumerates `.flf`/`.tlf` files in a ZIP archive,
  rejecting entries with path separators (path traversal defense)
- `read_zip_entry(path, name)` — reads a specific entry from a ZIP by name,
  also with path separator rejection

Key changes to `tui/file_ops.rs`:
- `OpenTarget` enum for dispatching file vs ZIP entry opens
- `browsing_zip` / `current_zip_path` state fields on `FileOpsDialog`
- `.zip` extension recognized as navigable in directory listings
- Enter/click on `.zip` enters ZIP browse mode; `..` exits back to filesystem
- `resolve_open_target()` returns `OpenTarget` for caller dispatch

Key change to `tui/mod.rs`:
- `perform_open()` dispatches to `read_zip_entry()` + parser for `ZipEntry` target

3 files touched: `font.rs`, `file_ops.rs`, `mod.rs`. 6 new ZIP browsing tests.
fmt and clippy pass clean.

### 4.1.6 — Phase merge: release/4.1 → main

Merged all Phase 4.1 work into default branch (master). Phase 4.1 complete:
remove auto-load of standard font on startup (4.1.1), fix OS Error 2 in file
open dialog (4.1.2), block mouse fall-through when dialog is open (4.1.3),
welcome screen on startup (4.1.4), ZIP file browsing in file open dialog (4.1.5).
All 5 subtasks (4.1.1–4.1.5) implemented, tested, merged. Phase 4.2 (Extended
Charsets) is next.

### 4.2.1 — Braille charset block

Production code was already in place:
- `braille_charset()` in `font_gen.rs` — all 256 codepoints U+2800–U+28FF, sorted
  by dot count then codepoint, cached via `OnceLock`
- `resolve_charset("braille")` — wired to `braille_charset()` for font-gen use
- `deluxe_charset()` — includes braille via `extend_from_slice(braille_charset())`
- `CHAR_GROUPS` in `palette.rs` — braille group with all 256 braille chars as
  a static string, exposed for future canvas charset picker

Added 7 verification tests (4 in `font_gen.rs`, 3 in `palette.rs`):
- Count (256), range (U+2800–U+28FF), sort order (dot count, codepoint), unique
  all-codepoints-no-gaps checks for both the charset function and the palette
   group string. fmt and clippy pass clean.

### 4.2.2 — Block elements charset

Updated `blocks_charset()` in `font_gen.rs` to contain all 32 codepoints U+2580–U+259F,
ordered by luminance (light → dark). Removed space (U+0020) which was not a block element.
Updated `blocks` palette string in `palette.rs` to match. Added 3 verification tests in
`font_gen.rs` (count=32, range check, unique/nogap all-32-codepoints) and 3 in `palette.rs`
(same checks on the static group string). All blocks tests pass.

### 4.2.4 — Ogham charset

Production code was already in place:
- `ogham_charset()` in `font_gen.rs` — 29 codepoints U+1680–U+169C, cached via `OnceLock`
- `resolve_charset("ogham")` — wired to `ogham_charset()` for font-gen use
- `deluxe_charset()` — includes ogham via `extend_from_slice(ogham_charset())`
- `CHAR_GROUPS` in `palette.rs` — ogham group with 29 chars as a static string

Fix applied: palette ogham group first char was U+0020 (regular space) instead of
U+1680 (Ogham Space Mark). Changed to U+1680 to match `ogham_charset()` output.

Added 6 verification tests (3 in `font_gen.rs`, 3 in `palette.rs`):
- Count (29), range (U+1680–U+169F), unique all-codepoints-no-gaps checks for both
  the charset function and the palette group string. fmt and clippy pass clean.

### 4.2.3 — Box drawing + dithered charset

Added three new charset functions to `font_gen.rs`:
- `dithered_charset()` — U+2591–U+2593 (░▒▓), 3 chars, `OnceLock` pattern
- `geometric_charset()` — 23 geometric shapes from U+25A0–U+25FF (squares, triangles, diamonds, circles)
- `resolve_charset("dithered")` and `resolve_charset("geometric")` wired for font-gen use
- `deluxe_charset()` extended with `dithered_charset()` and `geometric_charset()`

Updated `palette.rs`:
- `box` group expanded from 38-char subset to full 128-char range U+2500–U+257F
- `dithered` group added: "░▒▓" (3 chars)
- `geometric` group added: 23 geometric shapes matching `geometric_charset()`

Added 6 verification tests in `font_gen.rs` (count, range, uniqueness for dithered [3 tests] and geometric [3 tests]) and 9 in `palette.rs` (count, range, uniqueness for box [3], dithered [3], geometric [3]). fmt and clippy pass clean.

### 4.2.5 — "Deluxe" meta-charset

Updated `palette.rs`:
- "deluxe" `CharGroup` changed from descriptive string to explicit `concat!()` of all
  566 characters: ASCII printable, blocks (with quadrants), box drawing, dither,
  geometric shapes, braille, and Ogham.
- "deluxe" listed first in `CHAR_GROUPS` as the richest set.

Added 3 verification tests in `palette.rs`:
- `test_deluxe_palette_count` — asserts exactly 566 chars
- `test_deluxe_palette_contains_all_subset_chars` — asserts every char from every
  other group appears in deluxe
- `test_deluxe_palette_all_unique` — asserts 563 unique codepoints (3 dithered
   are subset of blocks). fmt and clippy pass clean.

### 4.2.6 — Phase merge: release/4.2 → main

Merged all Phase 4.2 work into default branch (master). Phase 4.2 complete:
braille charset block (4.2.1), block elements charset (4.2.2), box drawing +
dithered charset (4.2.3), Ogham charset (4.2.4), "Deluxe" meta-charset (4.2.5).
All 5 subtasks (4.2.1–4.2.5) implemented, tested, merged. Phase 4.3
(Architecture Audit) is next.

### 4.3.1 — TUI architecture deepdive vs ratatui best practices

Audited `tui/components/` and `tui/mod.rs` component architecture against
ratatui best-practice patterns (`Widget for &T`, `StatefulWidget`,
`WidgetRef`, `Layout` + `Constraint`). Findings documented in
`docs/tui-arch-audit.md` with 11 specific findings and prioritized
refactor plan (P0–P7). Key deviations: custom `Component` trait with
`&mut self` + `io::Result<()>` draw, state mutation inside render
pass, four coexisting rendering patterns, dead `StatusBar` code,
two-layer component wrappers adding no value, and transient drag
state mixed in `EditorState`. No code changes — audit only.

### 4.3.2 — Apply ratatui architecture fixes from audit

Implemented all fixes from the 4.3.1 audit:
- **Component trait eliminated** — all widgets use `impl Widget for &T` with
  `frame.render_widget()` directly. `component.rs` and 9 component-wrapper
  files (`components/*.rs`) deleted.
- **Widget ownership/borrow patterns unified** — `BrushState`, `ExportDialog`,
  `FileOpsDialog`, `UndoPanel`, `WelcomeScreen`, `FontEditor` all gained
  `Widget for &T` impls. `FontEditor::render` changed to `&self` with a
  new `before_render(&mut self)` step.
- **Dead code removed** — `StatusBar` struct (old static render),
  `Palette::render()`, `Toolbox::render()`, `CanvasSettings::render()`
  forwarding methods deleted.
- **Fields inlined** — `EditorState`/`DialogState`/`TuiApp` now hold widget
  types directly (no `*Component` wrappers). Drag state
  (`selection_drag_origin`, `selection_polygon_points`, etc.) extracted
  from `EditorState` to `TuiApp`.
- **Layout computed per-frame** — `frame_layout` removed from stored state;
  computed as local `fl` in `render()` and recomputed in mouse handler
  from terminal size. `canvas_inner_rect` no longer stored — computed
  locally in both `render_canvas_area()` and `handle_mouse_event()`.
- **Sync phase decomposed** — `sync_canvas_to_font_char()` and
  `sync_image_to_canvas()` called explicitly before widget rendering
  in `render_canvas_area()`.

### 4.3.3 — Phase merge: release/4.3 → master

Merged all Phase 4.3 work into default branch (master). Phase 4.3 complete:
TUI architecture audit (4.3.1), ratatui architecture fixes — remove Component
trait, flatten tui/, adopt native Widget pattern (4.3.2). All 2 subtasks
implemented, tested, merged. Phase 4.4 (Layers, Blending & Compositing) is next.

### 4.4.1 — Layer system

Created `figby-rs/src/tui/layers.rs` with three core types:
- `Layer` struct: owns `CanvasBuffer`, name, visibility, lock, opacity
- `LayerStack` struct: `Vec<Layer>` with active index, composite rendering,
  CRUD operations (add/delete/duplicate/merge/reorder), resize_all
- `LayerPanel` struct: ratatui widget rendered in right drawer, keyboard
  navigation (↑/↓ select, Enter toggle vis, L lock, ± opacity, N new,
  D dup, X del, M merge, ,/. reorder)

Integrated into EditorState: `layer_stack` and `layer_panel` fields replace
direct `canvas.buffer` writes with active-layer routing. Tool operations
(brush, eraser, fill, spray, line) redirected through clone-apply-recomposite
pattern to write to active layer buffer then composite to canvas. Mouse
painting, keyboard painting, selection (cut/copy/paste/delete), undo/redo
all route through active layer.

DrawerMode extended: Palette → BrushKeys → Layers → Closed cycle.
Status bar shows actual layer count from `LayerStack::len()`.
Theme expanded with `LayerTheme` struct (bg, fg, active_bg, border).

17 unit tests in layers.rs covering all operations, composite order,
opacity blending, visibility toggle, edge cases.

### 4.4.2 — Blending modes

Added `BlendMode` enum with 6 variants (Normal, Multiply, Overlay, Screen,
Add, Subtract) implementing `next()`, `prev()`, `icon_key()`, `display_name()`.
`Layer` struct gained `blend_mode: BlendMode` field (default Normal).
`LayerStack` gained `set_blend_mode()`. `composite()` updated: fast path for
opacity==255 + Normal overwrite, general path computes `blend_mode_color()` for
the mode blend, then alpha-composites with bottom via `blend_colors()`.

Blend math functions: `blend_channel(u8, u8, mode)` dispatches per-channel
Multiply/Overlay/Screen/Add/Subtract formulas (all u32 arithmetic). `blend_mode_color()`
handles non-RGB Color variants by falling back to top color.

`LayerPanel` gained `icons: BTreeMap<String, String>` field. `handle_key()`:
`b`/`B` cycle blend mode forward/backward. `render_with_stack()`: shows blend
mode Nerd Font icon in each layer row. Help text updated with "Bbld" hint.

Integration: `mod.rs` passes `icons.clone()` to `layer_panel.icons` at creation
and on welcome-screen canvas reset.

14 unit tests: multiply/overlay (dark+light)/screen/add/subtract channel math,
composite with blend mode, composite with opacity+blend, cycle (next/prev six-step),
set_blend_mode, icon_key, display_name, normal-returns-top.

### 4.4.3 — Layer groups + masks

Added `LayerMask` struct with `buffer: CanvasBuffer` and `enabled: bool`.
Mask buffer initialized to spaces (fully transparent). Non-space cells = visible.
Added `LayerGroup` struct with `name` and `collapsed` fields.

`Layer` gained `mask: Option<LayerMask>` and `group: Option<usize>` fields.
`LayerStack` gained `groups: Vec<LayerGroup>` plus methods: `create_group()`,
`remove_group()`, `toggle_group_collapsed()`, `rename_group()`, `group_of_layer()`,
`layers_in_group()`, `create_mask()`, `remove_mask()`, `toggle_mask()`,
`toggle_mask_enabled()`, `set_mask_pixel()`, `get_mask_pixel()`.

`composite()` checks mask per pixel: space cell in enabled mask → skip pixel.

`LayerPanel::handle_key()` changed signature to accept `KeyEvent` (was `KeyCode`).
New keybindings: `Ctrl+G` group, `Ctrl+Shift+G` ungroup, `→`/`←` expand/collapse,
`M` toggle mask, `m` mask enable/disable (falls back to merge-down if no mask),
`Tab` cycle focus across group boundaries.

`render_with_stack()` renders group headers with `▶`/`▼` disclosure triangles,
indents grouped layers by 2 spaces, shows 3-char mask thumbnail (`▓`/`░`/` `)
sampled from mask row 0.

22 unit tests: create/remove/toggle/rename group, group index shift after removal,
preserved after layer delete, empty/invalid index guards, create/remove/toggle mask,
toggle enabled, paint pixel, out-of-bounds, composite with mask (fully hidden),
composite with painted mask (revealed), composite with disabled mask, mask thumbnail.

### 4.4.5 — Phase merge: release/4.4 → master

Merged all Phase 4.4 work into default branch (master). Phase 4.4 complete:
layer system (4.4.1), blending modes (4.4.2), layer groups + masks (4.4.3),
export with layers (4.4.4). All 4 subtasks implemented, tested, merged.
Phase 4.5 (Animation Timeline & Playback) is next.

### 4.4.4 — Export with layers

Added per-layer export and alpha transparency to the TUI export dialog:
- `render_frame()` gained `transparent: bool` parameter — when true, space
  cells are skipped (alpha=0) in the output, preserving transparency.
- `export_cells_to_png_with_alpha()` — new public function exposing
  transparent rendering to callers that need per-pixel alpha control.
- `ExportDialog` gained `export_layers` and `use_transparency` bool fields.
  `L` toggles per-layer export mode, `P` toggles alpha transparency.
- `perform_layer_export()` — iterates visible layers, renders each to PNG
  with optional alpha, writes to `{base_dir}/{sanitized_name}.png` files.
  Duplicate names get numeric suffixes (`name_1.png`).
- `sanitize_layer_name()` — strips non-alphanumeric chars (except `_`/`-`),
  falls back to `"layer"` if result is empty.
- All layer exports use PNG format regardless of the dialog's current format.
- Layer mode dispatch in `TuiApp::start_export()`: when `export_layers` is
  true and format is PNG, writes composite to the main path then calls
  `perform_layer_export()` for individual layer files.

11 unit tests: PNG alpha matches opaque output, transparent space has alpha=0,
L/P toggle key handlers, sanitize (alphanumeric, special chars, underscore/hyphen,
empty fallback). fmt and clippy pass clean.

## 4.4.5 — Phase merge: release/4.4 → main (2026-06-17)

Merged release/4.4 branch into main at `d0a0967`. Brings 4.4.1 (Layer panel),
4.4.2 (Per-layer blend modes), 4.4.3 (Layer groups/masks), and 4.4.4 (Export
with layers/transparency) into the mainline. Next phase: 4.5 (Animation Timeline
& Playback).

## 4.5.5 — Phase merge: release/4.5 → master (2026-06-17)

Merged release/4.5 branch into master. Brings 4.5.0 (AnimationTimeline widget),
4.5.1 (Frame management), 4.5.2 (Keyframing), 4.5.3 (Tweening), and 4.5.4 (GIF export
from timeline) into the mainline. Next phase: 4.6 (Particle Effect Creator).

### 4.6.1 — Particle system design

Created `figby-rs/src/tui/particles.rs` with particle data model:
- `ParticleConfig` struct — TOML-deserializable config with emitter position,
  spawn rate, lifetime range, velocity range (x/y), acceleration (x/y), size,
  color (optional R/G/B), character, opacity, blend mode
- `Particle` struct — runtime particle state with current position (x,y),
  velocity (vx,vy), remaining_lifetime, size, color, character, opacity,
  blend_mode
- `ParticleSystem` struct — owns config + active_particles Vec + age +
  spawn_rate_accumulator. Methods: `new()`, `update(dt)`, `active_count()`,
  `clear()`, `pause()`, `resume()`, `is_paused()`
- Spawn-before-update: particles spawn at emitter then move during the same
  frame's update pass. Expired particles removed via `retain(|p| remaining > 0.0)`.
- `ParticleSection` added to `config.rs` TOML config — all ParticleConfig fields
  as `Option<T>` for granular config file override
- `FromStr` impl for `BlendMode` in `layers.rs` — parses blend mode names from
  config strings, falls back to Normal on unknown
- 12 unit tests: spawn, motion, expire, acceleration, spawn rate accumulator,
  full lifecycle, color from config, no color default, pause/resume, clear,
  negative dt, zero dt

### 4.5.2 — Keyframing

Added per-layer keyframing to the `AnimationTimeline` widget:
- `LayerKeyframe` struct with `position_offset: (i16, i16)`, `opacity: u8`,
  `blend_mode: BlendMode` — snapshots layer properties at a frame
- `TimelineFrame.layer_keyframes: Vec<Option<LayerKeyframe>>` — per-layer keyframe
  data for each frame. `has_keyframe` derived from `.any(|k| k.is_some())`
- `KeyframeEditState` struct — editor panel state with layer/property navigation,
  edit mode for numeric input, blend mode cycling via Enter
- `TimelineState` methods: `set_keyframe()`, `remove_keyframe()`, `get_keyframe()`,
  `get_interpolated_properties()`, `handle_keyframe_editor_key()`,
  `render_keyframe_editor()`
- Linear interpolation (`lerp_i16`, `lerp_u8`) for position and opacity between
  nearest keyframes; step interpolation (`step_blend_mode`) for blend mode
  (switches at t=0.5)
- Before-first/after-last keyframe: clamp to nearest keyframe values
- No keyframes: return defaults `((0,0), 255, Normal)`
- `K` key toggles keyframe editor panel overlay in TUI (`mod.rs` integration)
- 21 new tests: set/remove keyframes, linear interpolation (position, opacity),
  blend mode step, before/after bounds, single keyframe, no keyframes, multi-layer,
  editor navigation, numeric edit, blend cycle, has_keyframe derivation

### 4.6.2 — Particle emitter UI

Added full particle emitter tool to the TUI toolbox:
- `EmissionShape` enum (Point/CircleRadius/RectWH) with custom serde (string format for YAML config)
- `spread_angle: f64` and `emission_shape: EmissionShape` fields on `ParticleConfig`
- `Particle::new()` applies spread angle velocity cone randomization and emission shape position offset
- `ParticleSystem::render_to_canvas()` writes particle chars to `CanvasBuffer` with bounds clipping (no unwrap)
- `EmitterConfigPanel` struct with 17 editable fields (spawn rate, lifetime, velocity, acceleration, spread angle, emission shape, size, character, RGB color, opacity)
- Config panel rendered as a right-side overlay with bordered list, field highlight, edit mode
- `Emitter` tool variant added to `Tool` enum (shortcut `m`, display `"Em"`, icon `tool_emitter`)
- `tool_emitter: ` icon entry in `icons.yaml`
- Mouse click places emitter at buffer coords, opens config panel, starts particle animation
- Particle system updates every frame via delta time, rendering particles onto canvas buffer (save/restore pattern prevents persistence artifacts)
- Deactivation on tool switch handled via `AppEvent::Toolbox`
- 15 new unit tests: emission shapes (point/circle/rect), spread angle, render to canvas, bounds clipping, config panel navigation, float editing, shape cycling
- No `.unwrap()` in production — all fallible paths use `Option` or `Result` with user-facing error display

### 4.6.4 — Phase merge: release/4.6 → master (2026-06-17)

Merged release/4.6 branch into master at `aeddc56` (merge commit created on master via `--no-ff`).
Brings 4.6.1 (Particle system data model and lifecycle), 4.6.2 (Particle emitter UI tool with
config panel), and 4.6.3 (Particle-to-layer baking) into the mainline. Added `particles.rs`
(1225 lines), config section for particle defaults, emitter tool variant in toolbox, and
icons entry. Next phase: 4.7 (Animation Exporter).

### 4.6.3 — Particle-to-layer baking

Added bake functionality to the particle system:
- `ParticleSystem::bake_to_buffer(width, height)` — snapshots current particle state into a fresh `CanvasBuffer`, independent of the live particle system
- `ParticleSystem::bake_frames(num_frames, width, height, dt)` — clears system, generates N sequential frames via `update(dt)` + `bake_to_buffer()`, returns independent snapshot vec
- `LayerStack::add_frozen_frames(frames, base_name)` — pushes each buffer as a visible layer named `"{base_name} frame {i}"`, returns layer indices
- `TuiApp.show_live_particles` field (default `true`) — controls whether live particle overlay renders; when false, canvas renders without particle overlay (baked layers visible via composite)
- `TuiApp.baked_layer_indices` field — tracks indices of baked layers for potential cleanup
- Keybindings when emitter is active: `b` bakes single frame to layer, `B` bakes 10 frames as layer stack and switches to baked view, `v` toggles live/baked preview
- 6 new unit tests: bake independence, frame count + independence, empty system bake, content verification, 10-frame batch independence, layer stack insertion with independence

### 4.7.1 — Frame-by-frame terminal capture

Added `capture_timeline_frames()` free function in `tui/export.rs` — extracts
the timeline frame composition logic (previously inline in GIF export) into a
reusable function. Takes `&TimelineState`, `&LayerStack`, width, height, returns
`Vec<Vec<Vec<CanvasCell>>>`. Each frame composites all visible layers with keyframe-
interpolated position offset, opacity, and blend mode. Layer ordering matches
on-screen rendering: bottom layers first, top layers overlay via blend + opacity.

Added `ExportDialog::populate_from_timeline()` — convenience wrapper that calls
`capture_timeline_frames()` and stores the result in `self.timeline_frames`,
sets `timeline_available = true`, and computes frame delays from current FPS.

8 unit tests: empty timeline, single layer, two-layer composite, keyframe position
offset, keyframe opacity, keyframe blend mode (Multiply), populate dialog, populate
with empty timeline. All pass.

### 4.7.2 — APNG export

Added `export_cells_to_apng()` in `output.rs` — renders animation frames to animated
PNG using the `png` crate's native APNG support. Takes `frame_cells`, `frame_delays_cs`,
`font_size`, and `loop_count`. Sets up `png::Encoder` with `set_color(Rgba)`,
`set_depth(Eight)`, and `set_animated()` for APNG metadata. Each frame: render via
`render_frame()`, write RGBA pixel data, set frame delay via `set_frame_delay(delay, 100)`.
`finish()` finalizes the PNG stream.

Integrated into TUI export system:
- `ExportMode::Apng` added to the format cycle (Png → Apng → Gif → Txt → Png)
- `ExportDialog` handles APNG identically to GIF for animation controls (FPS, loop, preview)
- Timeline frames composed and exported via `export_cells_to_apng()` in both dialog and
  menu-driven export paths
- `ExportFormat::Apng` variant added to `output.rs`
- `ExportError::PngError(String)` variant for PNG/APNG error reporting
- `png = "0.18"` added to Cargo.toml

7 unit tests: single frame decode, multi-frame timing, infinite loop (num_plays=0),
finite loop (num_plays=3), empty frames error, format enum equality.
Integration tests in `tests/tui.rs` updated for new toggle order.

### 4.7.3 — ANSI escape sequence export (2026-06-17)

Added `export_cells_to_ansi()` in `output.rs` — renders canvas cells to ANSI
escape sequences with true-color foreground (`\x1b[38;2;R;G;Bm`) and background
(`\x1b[48;2;R;G;Bm`) using 24-bit color codes. `export_cells_to_ansi_multi()`
separates frames with `\x1b[2J\x1b[H` (clear + cursor home). Single-frame
export produces styled text with `\x1b[0m` reset per row.

Integrated into TUI export system:
- `ExportMode::Ansi` added to format cycle (Png → Apng → Gif → Txt → Ansi → Png)
- `ExportFormat::Ansi` variant in `output.rs`
- `L` (layers) and `P` (transparency) keys gated off in ANSI mode (not applicable)
- Export extension `.ans`
- CLI export path in `tui/mod.rs` handles single-frame and multi-frame cases

### 4.7.4 — Phase merge: release/4.7 → master (2026-06-17)

Merged release/4.7 branch into master. Brings 4.7.1 (Frame-by-frame terminal
capture), 4.7.2 (APNG export), and 4.7.3 (ANSI escape sequence export) into
the mainline. Next phase: 4.8 (Animation Player).

### 4.8.4 — Phase merge: release/4.8 → master (2026-06-17)

Merged release/4.8 branch into master. Brings 4.8.0 (AnimationPlayer widget),
4.8.1 (Terminal capture for playback), 4.8.2 (Raw mode playback engine), and
4.8.3 (Player integration into TUI) into the mainline. Next phase: 4.9 (Visual
Polish & TachyonFX).

### 4.8.3 — Player integration into TUI

Added player launch points from Export dialog and Timeline:
- `ExportDialog.play_requested` flag — set by `P` key in GIF/APNG mode, consumed
  by `TuiApp` main loop to call `launch_player_from_export()`
- `TuiApp::launch_player_from_export()` — captures timeline frames from current
  layer state, calls `play_animation()` with export dialog's FPS and preview_frame
- `TuiApp::play_animation()` — slices frames from `start_frame`, calls
  `player::play_fullscreen()`, then re-enters alternate screen for TUI
- Timeline `Enter` key handler — captures frames via `capture_timeline_frames()`,
  calls `play_animation()` from current timeline position
- Export dialog UI updated: `P` shows "Play Animation", `V` toggles preview play
- `play_requested` resets to `false` on close() and after consumption

Files touched: `export.rs`, `mod.rs`. 3 new unit tests in `export.rs`.

### 4.8.2 — Raw mode playback engine

Added raw mode playback engine to `player.rs`:
- `play_raw()` — enters raw mode (no echo, no line buffering), renders frames
  by writing pre-computed ANSI escape codes directly to stdout (bypasses
  ratatui Terminal::draw diffing for speed)
- `render_frame_raw()` — converts `AnimationFrame` to ANSI escape string with
  CUP cursor positioning, skips blank cells for efficiency
- `color_fg_ansi()` / `color_bg_ansi()` — ratatui `Color` to ANSI SGR code
  conversion for all named, RGB, and indexed color variants
- Frame timing via `std::thread::sleep` with speed multiplier
- Keyboard: Space=pause, Esc=exit, Left/Right=seek, +/-=speed (also keeps
  existing Up/Down for speed, L for loop)
- `+`/`=` and `-`/`_` added to `AnimationPlayer::handle_key()` for speed
- Progress bar rendered at bottom row during raw playback
- 13 new unit tests: color ANSI conversions, render_frame_raw, handle_key
  with +/-/=/-, play_raw empty frames

### 4.8.1 — Terminal capture for playback

Added terminal capture and restore lifecycle to `player.rs`:
- `TerminalSession` — captures current terminal content (blank fallback since
  DECRQCRA not implemented), manages alternate screen enter/exit
- `capture_terminal_content()` — wraps `terminal::size()` + blank frame
- `play_fullscreen()` — orchestrates capture → alt screen → ratatui render
  loop → keyboard input → restore on Esc or playback end
- `prepend_frame()` / `all_frames()` / `fps()` accessors on `AnimationPlayer`
- Captured terminal content is prepended as frame 0, so user sees original
  terminal before animation starts after alt screen transition
- 6 unit tests covering fps, prepend, capture fallback, session, blank frame
  dims, and play_fullscreen error handling (no panic on empty frames)

### 4.8.0 — Custom ratatui widget: `AnimationPlayer`

Created `figby-rs/src/tui/player.rs` with `AnimationPlayer` struct — standalone
ratatui widget for playing back animation frames on the alternate screen:
- Uses interior mutability (`Cell`) to implement `Widget for &AnimationPlayer`
  (not `&mut`), matching ratatui's recommended reference-based widget pattern
- Takes `Vec<AnimationFrame>` (2D grid of `CanvasCell`) and FPS rate
- Supports play/pause, seek by frame index, loop toggle, speed control 0.25x–4x
- `advance(delta)` — accumulator-based frame advancement at effective FPS
- `handle_key(code)` — Space (play/pause), Left/Right (seek), Up/Down (speed),
  `l`/`L` (loop toggle), Esc (pause+reset), Enter (play)
- Implements `Widget for &AnimationPlayer` rendering frame content with FG/BG
  colors and a progress bar on the bottom row (play icon, counter, bar, speed)
- 16 unit tests covering advance, looping, seek, speed clamp, progress bar
  rendering, frame content, empty frames, and all key handlers
- No `.unwrap()` in production paths

### 4.6.4 — Phase merge: release/4.6 → master (2026-06-17)

Merged release/4.6 branch into master. Brings 4.6.1 (Particle system data model and lifecycle),
4.6.2 (Particle emitter UI tool with config panel), and 4.6.3 (Particle-to-layer baking) into
the mainline. Next phase: 4.7 (Animation Exporter).

### 4.7.4 — Phase merge: release/4.7 → main (2026-06-17)

Merged release/4.7 branch into master. Brings 4.7.1 (Frame-by-frame terminal capture),
4.7.2 (APNG export), and 4.7.3 (ANSI escape sequence export) into the mainline.
No code changes — merge was a no-op (release/4.7 already an ancestor of master).
Fixed stale merge conflict markers in ralph-log.md. Next phase: 4.8 (Animation Player).

### 4.9.1 — TachyonFX spike: welcome screen fade-in

Added `tachyonfx = { version = "0.25", features = ["std-duration"] }` dependency.

Created `figby-rs/src/tui/fx.rs` with `WelcomeFx` struct wrapping a tachyonfx `Effect`
that applies `fade_from_fg(Color::DarkGray, timer)` over 400ms with `QuadOut` easing
to the welcome dialog area on startup.

Modified `figby-rs/src/tui/mod.rs`:
- Added `pub mod fx;` module declaration
- Added `delta_time`, `fx_last_tick`, `welcome_fx` fields to `TuiApp`
- `render()` computes delta_time each frame, applies `WelcomeFx::process()` to the
  welcome area after rendering, clears `welcome_fx` when effect completes (done())
- All 5 constructive welcome actions (Dismiss, OpenRecent, Open, NewFile, OpenSettings)
  set `welcome_fx = None`

Modified `figby-rs/src/tui/welcome.rs`: made `centered_welcome()` `pub` for use by fx module.

3 files touched: `Cargo.toml`, `fx.rs` (new), `mod.rs`. fmt and clippy pass clean.

### 4.9.2 — Default panel theme inspired by TachyonFX aesthetic

Updated `figby-rs/src/tui/theme.rs` default colors and `assets/tui/themes/default.yaml` to match the dark, neon-accent TachyonFX showcase aesthetic:
- Darker backgrounds (`#0d0d1a` for toolboxes/menus, `#1a1a2e` for borders/grid)
- Cyan primary (`#00d4ff`) for selection highlights, cursor, mode indicators
- Magenta accent (`#ff0099`) for secondary/graphic mode elements
- Green success (`#00ff87`) and red error (`#ff0044`) for dialogs
- Warm orange (`#ffaa00`) for ASCII preview mode
- Tests updated to verify new color values in parsed output

2 files touched: `theme.rs`, `default.yaml`. fmt, clippy, and all tests pass clean.

### 4.9.3 — App fade-in on launch (ratzilla-style)

Added `AppFadeIn` struct in `figby-rs/src/tui/fx.rs` using `fx::fade_from(Color::Black, Color::Black, timer)` over 600ms QuadOut — full-screen black overlay that fades to transparent, revealing UI content underneath.

Integrated into all three render paths in `figby-rs/src/tui/mod.rs`:
- Welcome screen path (applied after welcome_fx on full area)
- Zen mode path (applied after canvas + overlays)
- Normal mode path (applied as final pass after all widgets)

`app_fade_in` field is `Option<AppFadeIn>`, set to `Some` on construction, consumed to `None` when `done()` returns true. Effect runs once per cold launch then self-cleans.

2 files touched: `fx.rs` (new struct), `mod.rs` (integration). fmt and clippy pass clean.

### 4.9.4 — Status bar redesign (responsive, widget-based)

Created `StatusBarWidget` in `figby-rs/src/tui/components/status_bar.rs` — replaces the inline
`render_status_bar()` method on `TuiApp`. Responsive layout with 4 priority groups:
- P1 (always): mode badge (colored), cursor position, tool name, unsaved indicator
- P2 (≥60 cols): font name + glyph count, git branch
- P3 (≥80 cols): FPS + render mode
- P4 (≥100 cols): clock, layer count, undo count, throbber text

All 17 `status_*` inline fields removed from `TuiApp` struct — status bar data passed as method
parameters. Status bar height reduced from `Constraint::Length(3)` to `Constraint::Length(1)`
(no borders). Theme extended with 6 new colors (git_branch, font_name, fps, glyph_count,
unsaved, saved). 8 new icon entries in icons.yaml.

Files touched: `status_bar.rs`, `components/mod.rs`, `mod.rs`, `theme.rs`, `layout.rs`,
`icons.yaml`, `default.yaml`. fmt and clippy pass clean.

### 4.9.5 — Phase merge: release/4.9 → master

Merged all Phase 4.9 work into master (default branch). Phase 4.9 complete:
TachyonFX spike with welcome screen fade-in (4.9.1), dark neon-accent panel
theme (4.9.2), app fade-in on launch (4.9.3), widget-based responsive status
bar (4.9.4). All 4 subtasks implemented, tested, merged. Phase 4.10 (Web
Target) is next.

### 4.10.1 — WASM / web target via Ratzilla

Added `wasm32-unknown-unknown` build target support using `ratzilla` crate:

**Cargo.toml changes:**
- Moved native-only deps (crossterm, font-kit, tachyonfx, rand, pathfinder_geometry) to
  `[target.'cfg(not(target_arch = "wasm32"))'.dependencies]`
- Added WASM-only deps: `ratzilla = "0.3"`, `getrandom = { features = ["js"] }`
- ratatui split across targets: native gets `features = ["crossterm"]`, WASM gets `default-features = false`
- `zip` uses `default-features = false, features = ["deflate"]` to avoid lzma-sys C dep on WASM
- `flate2` uses `rust_backend` feature (pure Rust, no C cross-compilation)
- `[lib]` crate-type includes `"cdylib"` for wasm-bindgen output
- `rascii_art` moved to unconditional deps (pure Rust, compiles on WASM)

**lib.rs:**
- Added `#[cfg(target_arch = "wasm32")] pub mod web;`
- `pub mod tui;` and `pub mod font_gen;` gated with `#[cfg(not(target_arch = "wasm32"))]`
- `CanvasCell` type moved from `tui/canvas.rs` to crate root `canvas_inner` mod for
  unconditional availability (needed by `output.rs` export functions)
- `tui/canvas.rs` re-exports via `pub use crate::CanvasCell;`

**web.rs (new):**
- Ratzilla font preview app with `DomBackend`
- Keyboard-driven text editing (char insert, backspace, delete, arrows, home/end)
- Font selector via Up/Down arrows
- Renders FIGlet output via `render_string()` in a ratatui `Paragraph` with Wrap
- Three embedded fonts: standard, banner, big (via `include_bytes!`)
- No crossterm imports — standalone web entry point
- `run_web()` returns `io::Result<()>`

**main.rs:**
- `#[cfg(target_arch = "wasm32")] fn main()` stub calls `figby::web::run_web()`
- Existing main gated with `#[cfg(not(target_arch = "wasm32"))]`
- `get_columns()` has dual versions: crossterm for native, `None` for WASM

**image_input.rs:**
- Added `get_terminal_width()` helper with dual versions replacing direct crossterm call

**Architectural decisions:**
- Ratzilla 0.3.1 uses `DomBackend`, `WebRenderer` trait, `draw_web()` for rendering,
  `on_key_event()` for keyboard. Compatible with ratatui 0.30.1.
- `cargo build -p figby --lib --target wasm32-unknown-unknown` succeeds
- Full `cargo build -p figby --target wasm32-unknown-unknown` succeeds (bin compiles with stub main)
- Tested with `cargo clippy --all-targets --all-features -- -D warnings` (native + cross checks)

### 4.10.2 — Phase merge: release/4.10 → master

Merged release/4.10 branch into master at `4265599` (merge commit created on master via `--no-ff`).
Brings 4.10.1 (WASM/web target via Ratzilla) into the mainline. Phase 4.10 complete.
No code changes — merge only. Next phase: 4.11 (Dynamic Lighting — Design Only).

### 4.11.1 — Dynamic lighting system — initial design

Created `docs/lighting-design.md` — design specification for a dynamic lighting
system for the TUI ASCII canvas editor. Covers 6 core components:

- **Normal-map generation** from FIGfont glyph fill-density (heightfield →
  Sobel gradient → tangent-space normal) or user-painted height values
- **Scene lights** as an enum with Ambient, Directional (parallel), and Point
  (attenuated) variants, each with RGB color and intensity
- **Shading model** using Lambertian diffuse with optional Blinn-Phong specular,
  producing per-cell luminance `f32` and tint `Rgb`
- **Shadow casting** via 2D DDA grid raycast (Amanatides & Woo) from fragment
  toward each non-ambient light, binary occluded/visible output
- **Per-object flags** `accepts_lighting` and `casts_shadow` on `Layer` struct
  (design only — no code changes)
- **Output pipeline**: shaded luminance → palette LUT lookup → character +
  color remapping, hooked after layer compositing but before frame commit

Document includes: data structures (Normal3, NormalMap, Scene, Light, LutEntry),
data flow diagram, integration points (7 files), deferred features, open
questions with recommendations, and future test strategy.

Design-only task. No code touched. See `docs/lighting-design.md` for full spec.

### 4.11.2 — Phase merge: release/4.11 → master

Merged release/4.11 branch into master at (merge commit prepared via `--no-ff --no-commit`).
Brings 4.11.1 (Dynamic lighting system design document) into the mainline.
Phase 4.11 complete — design-only phase. Next phase: 4.12 (Major Release).

### 4.12.1 — Full regression against C FIGlet 2.2.5

Ran full FIGlet regression test suite against the v4 codebase:
- **48/48 FIGlet regression tests pass** — tests 01-55 in `tests/run_tests.rs` covering
  every FIGlet 2.2.5 flag, font format (FLF/TLF), smushing modes, justification, RTL,
  deutsch, control files, and paragraph mode
- **3 pre-existing TUI test failures** (unrelated to FIGlet): `test_fill_tool_keyboard`,
  `test_layout_drawer_cycle`, `test_selection_perimeter_delete` — inherited from
  release/4.12, not regressions from this phase
- **No code changes needed** — existing test infrastructure from 2.10.1 covers the
  full FIGlet regression scope. All image/TUI/animation features verified via
  `tests/regression_image.rs`, `tests/regression_tui.rs`, `tests/regression_export.rs`
- **100% FIGlet output compatibility** confirmed. Task success criterion met.

### 4.12.2 — v4 major milestone RC — human sign-off

v4 RC cut: `rc/4.0.0-rc.1` branch + `v4.0.0-rc.1` annotated tag created from
`release/4.12` tip. Version bumped from `3.0.0-rc.3` to `4.0.0-rc.1`.
CHANGELOG updated with comprehensive v4 phase summary. Stale RC infrastructure
(old `rc/4.0.0-rc.1` branch and `4.0.0-rc.1` lightweight tag) deleted.
Handoff to human for review and merge to master.

### 5.1.1 — Toolbox NerdFont icons

Added `icons: BTreeMap<String, String>` field to `Toolbox` struct, initialized
empty in `new()`. Wired via `toolbox.icons = icons.clone()` in `TuiApp::new()`.
Rendering changed from `display_name()` (2-char abbrev) to icon lookup from
`App::icons` map, falling back to abbrev when icon missing. Display format:
`[icon] FullName` per row (e.g. ` Brush`, ` Select`). Same pattern as
`LayerPanel` and `StatusBar`. Only `toolbox.rs` and `mod.rs` modified.
fmt and clippy pass clean.

### 5.1.2 — Toolbox dynamic width

Added `BrushState::required_outer_width()` returning 15 (max content width of
brush panel + 2 border). Added `Toolbox::required_width(brush_width)` computing
`max(icon_width + longest_full_name + 2, brush_width).clamp(10, 20)` using
`unicode-width` crate. Removed `TOOLBOX_WIDTH` constant from `layout.rs`,
parameterized `FrameLayout::compute` with `toolbox_width: u16`. Three call
sites in `mod.rs` (render, render_canvas_area, handle_mouse_event) compute
width before each layout pass. 5 new tests across `brush.rs` and `toolbox.rs`.
fmt and clippy pass clean.

### 5.1.3 — Canvas visible border

Added `border: Color` field to `CanvasTheme` (default accent cyan `#00d4ff`),
with YAML deserialization support via `CanvasYaml.border` and merge chain in
`From<ThemeYaml>`. Added `border: "#00d4ff"` to `default.yaml` under `canvas:`.
Updated `render_canvas_area()` in `tui/mod.rs`: replaced plain `Borders::ALL`
edge block (dim style, canvas.edge color) with `BorderType::Double` block using
`canvas.border` color (accent cyan) and `.title(format!(" {}x{} ", w, h))`
showing canvas buffer dimensions. `BorderType` added to ratatui widget imports.
fmt and clippy pass clean.

### 5.2.2 — Right panel: tabbed prop/info/library/effects drawer

Replaced right drawer with tabbed `SidePanel` component. Created
`figby-rs/src/tui/side_panel.rs` with `TabId` enum (Layers/Props/Text/Libraries/
Effects) each with NerdFont icon key from `icons.yaml` and display name.
`SidePanel` struct holds `open: bool`, `active_tab: TabId`, icons map, theme.

Layout changes (`figby-rs/src/tui/layout.rs`):
- Removed `DrawerMode` enum entirely (was Palette/BrushKeys/Layers/Closed)
- `FrameLayout::compute()` now takes `side_panel_open: bool` instead of
  `DrawerMode`
- Removed `right_panel_borders()` dead code function

Integration changes (`figby-rs/src/tui/mod.rs`):
- `right_drawer: DrawerMode` → `side_panel: SidePanel`
- All 3 `FrameLayout::compute` call sites pass `self.side_panel.open`
- Render block replaced single match with `self.side_panel.render()`
- Layer panel key dispatch guarded by `side_panel.open && active_tab == Layers`
- `?` key toggles panel open/close (was cycle BrushKeys→Layers→Closed)
- Left/right arrows switch tabs when panel open (guarded before canvas movement)
- Mouse click on tab label switches active tab
- `render_brush_keys_panel` moved into `SidePanel::render_props_content()`
  (dead private method removed from mod.rs)

Test `test_layout_drawer_cycle` renamed to `test_layout_drawer_toggle` —
checks open/closed toggling and active tab persistence.

### 5.2.1 — Palette moved under tools (left column)

Moved palette from right drawer to left column below toolbox. `FrameLayout`
gains `palette: Option<Rect>` field. `FrameLayout::compute()` takes
`toolbox_h: u16` param, splits toolbox column vertically with
`[Length(toolbox_h), Min(0)]` — palette gets remaining space. Default
`DrawerMode` changed from `Palette` to `BrushKeys`; cycle skips Palette
(Closed → BrushKeys). Right drawer match uses `_ => {}` for Palette/Closed
since palette no longer lives in right panel. Click handler updated to
target `fl.palette` rect instead of right panel + drawer mode check.
fmt and clippy pass clean.

### 5.2.4 — Phase merge: release/5.2 → main

Fast-forward merge of release/5.2 into master — brings phase 5.2 features
(palette under tools, tabbed right panel, context-sensitive props) into
mainline. Task checked off in todo-v5.md. Next phase: 5.3 (Status Bar
Redesign).

### 5.3.1 — Flat item-based status bar with section grouping

Redesigned `StatusBarWidget` from a single-line concatenation of four parts
into a flat item list using `build_all_items()` which returns `Vec<StatusItem>`.
Each item has `spans`, `width`, and `keep` fields.

Three informal sections grouped within the flat list:
- **Left:** mode (icon + name, bold, mode-color foreground), tool name,
  cursor position (X:Y), zoom level
- **Middle:** font name, unsaved/saved indicator, glyph count (only when font active)
- **Right:** git branch, FPS, clock, render mode, layer/undo counts, throbber

Pipe `│` separators (`\u{2502}`) between items using
`self.theme.statusbar.separator` color. No powerline separators or
`Layout::horizontal` — items rendered as a single `Line` via
`buf.set_line()`. At very narrow widths (<10 cols), renders truncated
mode badge.

No changes to `StatusBarWidget::new()` signature — all existing callers in
`tui/mod.rs` work unchanged. No `unwrap()` in production paths.

### 5.3.2 — Responsive: drop low-priority items at narrow widths

Replaced the fixed three-section powerline layout with a flat item-based
approach. Each status element is a `StatusItem { spans, width, keep }`.
Items with `keep: false` are dropped right-to-left (via `rposition`) when
total width exceeds available area.

Dropable items (in drop order): throbber, undo count, layer count, render
mode, clock, FPS, git branch, font group, zoom, tool. Non-dropable: mode,
position. Separator uses `\u{2502}` instead of powerline triangle `\u{e0b0}`.

### 5.3.3 — Phase merge: release/5.3 → master

Merged release/5.3 into master. Task checked off in todo-v5.md.
Version bumped from 5.2.0 to 5.3.0. No code changes — admin re-application
of reverted bookkeeping after merge. Next phase: 5.4 (Image Editor Fix).

### 5.4.1 — Fix image editor mode switching

Welcome screen `ImageOpenFigmap` and `ImageNewBlank` actions now initialize
`editor.image_editor` and set `self.mode = AppMode::ImageEditor` before proceeding.
Previously these actions were merged with `FontOpen` (just called `start_open()`)
and never entered image editor mode, leaving the editor in a broken state where
no canvas/tools were accessible for image operations.

`FontOpen` split into its own arm (no image editor init). `ImageNewFromTemplate`
and `ImageConvert` now at least set the mode flag (template picker and rascii
dialog are TODO in later tasks). Mode toggle keybind (Tab) already worked because
`image_editor` is initialized at TuiApp construction — only the welcome screen
dispatch path was broken.

### 5.4.2 — Fix mouse events in image editor

Added ImageEditor state checks in `handle_mouse_event` before the general canvas/toolbox
handlers. When `entering_path` is true (user typing a file path), all mouse events are
swallowed (early return). When `error_message` is set, a left-click dismisses the error
and returns. All other ImageEditor states (adjustment_mode, normal canvas editing) fall
through to the existing general mouse handlers — no code change needed for those paths.
Only `figby-rs/src/tui/mod.rs` modified.

### 5.4.3 — Image import dialog (rascii options)

Added `RasciiImportDialog` in `figby-rs/src/tui/dialogs/rascii_import.rs` with file
browser, options panel (charset/width/color mode), and preview. Activated by "Convert
Image to ASCII" welcome action (`V` key or click). Supports 5 charsets (block, smooth,
full, braille, deluxe), 3 color modes (Mono/256/Truecolor), adjustable output width
(8-500). 256-color uses standard 6×6×6 ANSI cube quantization. Preview renders converted
chars as inline text. Confirm loads result into canvas as a new layer and switches to
Image Editor mode. No `.unwrap()` in production. 15 unit tests.

### 5.4.4 — Phase merge: release/5.4 → master

Merged release/5.4 branch into master at `92b53d1` (merge commit created on master via `--no-ff`).
Brings 5.4.1 (image editor mode switching fix), 5.4.2 (mouse events in image editor fix),
and 5.4.3 (rascii import dialog) into the mainline. Phase 5.4 complete.
Next phase: 5.5 (Animation Audit & Surface).

### 5.5.1 — Audit 4.5–4.8 implementation vs spec

Read-only audit of `timeline.rs`, `player.rs`, `export.rs` against the spec in
`docs/todo-v4.md` phases 4.5–4.8. Findings documented in `docs/animation-audit.md`:

**What works:** All core features (AnimationTimeline widget, frame CRUD, keyframing
with interpolation, tweening with easing, GIF/APNG/ANSI export, AnimationPlayer
widget, raw mode playback engine) are implemented.

**Gaps found (7 total):**
- P0: `try_query_terminal_cells()` always returns Unsupported — terminal content
  capture is stubbed (4.8.1)
- P1: Playback blocks TUI event loop instead of running in separate thread (4.8.3)
- P1: Only global FPS supported, no per-frame delays (4.5.4)
- P2: Duplicate render code in ExportDialog (Widget impl is dead code)
- P2: `play_raw()` never called from TUI path
- P3: No timeline panel in main layout (task 5.5.2)
- P3: Standalone `Widget` impl for timeline is trivial (ruler only)

No code changes — pure audit. Gap list ready for 5.5.2/5.5.3.

### 5.5.2 — Surface timeline panel in main layout

Added timeline panel to main editor layout:
- `T` (no modifier) toggles timeline at bottom of canvas (~8 rows when open)
- `TIMELINE_HEIGHT = 8` constant in `layout.rs`, timeline as optional `Rect` in `FrameLayout`
- `canvas_borders()` updated to omit BOTTOM when timeline visible; `timeline_borders()` returns LEFT|RIGHT|BOTTOM
- Timeline renders with heading block + `StatefulWidget` content + toolbar line
- `capture_thumbnail()` downsamples `CanvasBuffer` to `thumb_w × thumb_h` char grid for frame thumbnails
- `AnimationTimeline::panel_instance()` constructor for bottom panel config
- Keybindings: `←/→` switch frame, `A` add frame, `Delete` delete frame, `Enter` play
- `Shift+T` opens tween panel (was `T` — moved to `Shift+T` to free bare `T` for toggle)
- `ToggleTimeline` global action added to keymap dispatch

### 5.5.3 — Verify animation export end-to-end

Added 5-frame animation export tests for GIF, APNG, and ANSI formats in both
`output.rs` (raw export functions) and `tui/export.rs` (ExportDialog integration):
- `test_output_gif_5_frames` / `test_output_apng_5_frames` / `test_output_ansi_5_frames`
  verify per-frame delay correctness for each export function directly.
- `test_perform_export_gif_5_frames` / `test_perform_export_apng_5_frames` /
  `test_perform_export_ansi_5_frames` verify end-to-end export via the dialog,
  including filesystem write and decode round-trip.
- `test_export_cycle_reaches_animation_formats` verifies `T` key cycles through PNG →
  APNG → GIF → TXT → ANSI → PNG (all three animation-adjacent modes reachable).
- Removed the dead `Widget for &ExportDialog` impl (identified as gap P2 in 5.5.1
  audit). The `render(&self, frame, area)` method was already the active rendering
  path — the Widget impl was never registered in mod.rs.
- Added `set_per_frame_delays()` public method on `ExportDialog` for tests to inject
  custom per-frame delay values.

### 5.5.4 — Phase merge: release/5.5 → master

Fast-forward merge of release/5.5 into master. Phase 5.5 complete: animation
audit (5.5.1 — read-only audit of timeline/player/export vs spec), timeline
panel surface in main layout (5.5.2 — `T` key toggle, frame thumbnails,
playhead, add/delete frame), export end-to-end verification (5.5.3 — 5-frame
GIF/APNG/ANSI tests, dead Widget impl removed, per-frame delays). Version
bumped from 5.4.0 to 5.5.0. Next phase: 5.6 (Palette UX & Editor).

### 5.6.1 — Color name tooltip on hover

Added hover tooltip showing terminal colour name below palette swatches:
- `hover_index: Option<usize>` field on `Palette` — tracks hover state separately
  from `selected_index`. Cleared on mouse-out.
- `ANSI_COLOR_NAMES` constant — 16 standard ANSI names (Black, Red, ... Bright White)
- `color_name(index)` — returns ANSI name for standard mode, `"Color N"` for extended
- `handle_hover(col, row, area)` — hit-tests mouse move against palette inner rect,
  sets `hover_index` on swatch match, returns `true` if hover state changed
- Widget impl renders color name below swatch grid in `self.theme.general.secondary`
- Mod.rs: `Moved` arm added to palette mouse handler, gates on `!settings_open`
- 5 unit tests: hover on/off swatch, outside clears, color name (standard + extended)

No `.unwrap()` in production. fmt and clippy pass clean.

### 5.6.2 — 5-per-row hue-grouped palette layout

Replaced the 2×8 flat grid with a hue-grouped layout:
- `HueGroup` enum with 8 variants (Neutrals/Reds/Oranges/Yellows/Greens/Cyans/Blues/Purples)
- `hue_group_for_ansi(index)` — static mapping from ANSI index 0..15 to hue group
- `build_flat_palette()` — returns Vec of (ansi_idx, Color, name) in grouped order
- Standard mode render: group header lines (dim style) + 5 swatches per data row
- Extended mode render: 4 rows of 5 swatches (was 2 rows of 8)
- `handle_key` Up/Down offset changed from 8 to 5 (both modes)
- `standard_index_at()` helper maps (rel_col, rel_row) → visual index via group walk
- `handle_hover()` and `handle_click()` rewritten for new geometry
- `current_color()` and `color_name()` use build_flat_palette() for standard mode

Files touched: `figby-rs/src/tui/palette.rs` (production + tests),
`tests/tui.rs`, `tests/regression_tui.rs` (integration test updates).
16 new unit tests: hue group mapping, flat palette completeness and ordering,
navigation offset 5.

### 5.6.4 — Palette import: common formats

Added `figby-rs/src/palette_import.rs` with 4 import format parsers:

- **Paletty JSON** — `[{hex, name}]` array, normalizes hex with/without `#`
- **Adobe ASE** — binary ASEF format with big-endian parsing of RGB/Gray swatch blocks, UTF-16BE name decoding
- **WezTerm JSON** — `colors` object with named keys (foreground, background, cursor_fg/bg, selection_fg/bg), `ansi[8]`, `brights[8]`
- **Windows Terminal JSON** — `schemes[0]` with background, foreground, cursorColor, selectionBackground, 16 named colour fields

`ImportFormat` enum with `display_name()` and `all()` iterator. `auto_detect_format()` checks ASE magic bytes (`ASEF`), file extension, and JSON key structure (`colors`, `schemes`, `swatches`+`name`, or array). `import_swatches()` dispatches to format-specific parser.

Changes to `palette_editor.rs`:
- Added `ChoosingFormat` PanelMode — format radio list (Auto / PalettyJSON / AdobeASE / WezTerm / WinTerm / Native)
- `PaletteEditor` gained `import_format: Option<ImportFormat>` and `format_index` fields
- `L` key now opens format picker first, then file browser filtered by chosen format
- `available_palettes(format)` scans for `.json` and `.ase` files based on format filter
- `load_file()` uses stored `import_format` to dispatch; auto-detects when format is None
- Help text shows active format in parentheses
- `Swatch` moved from local definition to `crate::palette_import::Swatch`

`lib.rs` added `pub mod palette_import;`. `mod.rs` updated `available_palettes()` call to use `None`.

20 unit tests in `palette_import.rs`: all 4 formats parse correctly, mixed hex formats, malformed data rejection, ASE binary construction, all 6 auto-detection cases, empty/unknown content. No `.unwrap()` in production.

### 5.6.3 — Palette editor panel (save / load / duplicate)

Created `figby-rs/src/tui/palette_editor.rs` with palette save/load/duplicate
overlay panel, accessible via `Ctrl+Shift+P` keybind:

- `PaletteEditor` struct: `open`, `name_buffer`, `swatches`, `selected`,
  `mode` (Idle/Naming/Loading), `file_list`, `file_scroll`, `message`, `modified`
- `PaletteFile` / `Swatch` — JSON-serializable types for disk persistence
- `save()` — writes `~/.config/figby/palettes/<name>.json` (path traversal
  protection via `/`, `..`, `\\` rejection; validates non-empty name)
- `load_file()` — reads and parses `.json` palette files
- `duplicate()` — saves palette under a new name
- `available_palettes()` — scans palettes dir for `.json` files, sorted
- `load_current_from_palette()` — populates swatches from active palette state
- `apply_to_palette()` — writes swatch colors back to palette's recent list
- `render()` — centered overlay via `palette_editor_overlay()` helper in
  `layout.rs` (42 cols wide, ~half height, centered). Shows swatches with
  hex values, color preview blocks, selection indicator, mode-specific UI
  (file list when Loading, message feed)
- `handle_key()` — dispatches by `PanelMode`: Idle (Esc close, arrows select,
  S save, L load, D duplicate), Naming (char input, Enter confirm,
  Esc cancel), Loading (arrows pick file, Enter load, Esc cancel)
- `color_to_hex()` / `hex_to_color()` — bidirectional Color↔`#RRGGBB` conversion
- `ansi_to_rgb()` — maps indexed color 0..255 to (r,g,b) via 16-color table +
  6x6x6 cube + grayscale ramp
- `luminance()` — computes average RGB for readable foreground text on swatch
- `serde_json = "1"` added to Cargo.toml

Files touched: `palette_editor.rs` (new), `mod.rs`, `layout.rs`, `Cargo.toml`.
8 unit tests: JSON roundtrip, duplicate independence, disk save/load,
load from palette, palettes dir creation, hex conversion, apply to palette,
path traversal rejection. No `.unwrap()` in production. fmt and clippy pass clean.

### 5.6.5 — Marker brush mode (Aseprite-style shading)

Added Marker brush sub-mode for progressive colour stepping:
- `BrushSubMode` enum (`Normal`/`Marker`) with `cycle()` and `name()` methods
  in `tui/brush.rs`. `BrushState` gains `sub_mode: BrushSubMode` field,
  `cycle_sub_mode(has_colors)` — cycles only when ≥2 colours are multi-selected
  in the palette (or when currently in Marker mode, to allow exiting).
- Palette multi-select state in `tui/palette.rs`: `multi_select_indices: Vec<usize>`,
  `multi_select_active: bool`, `toggle_multi_select_color()`, `has_multi_select()`,
  `selected_color_array()`. `Tab` toggles multi-select mode; Enter toggles colour
  in/out of selection when in multi-select mode. Selected swatches render with
  white foreground "██" indicator.
- Marker brush functions in `tools/brush.rs`:
  - `stamp_offsets_with_falloff()` — returns `(dx, dy, falloff)` for each stamp
    cell. Circle: linear drop from 1.0 at centre to 0.0 at radius. Square:
    plateau from 1.0 at centre to 0.5 at edge. SprayPaint: 1.0 for hit cells.
    Custom: 1.0 at centre.
  - `is_cell_non_empty()` — helper: `ch != ' ' || fg/bg.is_some()`
  - `accumulate_marker_stamp()` and `accumulate_marker_line()` — accumulate
    falloff values into `HashMap<(i16,i16), f64>` per stroke, skipping empty cells.
  - `commit_marker_accum()` — on mouse-up, iterates accum entries, steps fg/bg
    colour forward by `accum.floor()` positions in the selected-colour array.
    Clamps at last colour. Retains fractional remainder for future strokes.
- Wired into `mod.rs`: `marker_accum` field on `TuiApp`, mouse handler branches
  for Marker sub-mode (Down→accumulate, Drag→accumulate line, Up→commit+recomposite).
  `Shift+M` toggles marker mode when Brush tool active. `Tab` added to palette
  key dispatch.
- Side panel `add_brush_props()` shows `Mode:` line with sub-mode name.
- 17 unit tests in `tools/brush.rs`: falloff values for circle/square/edge,
  accumulate-only-fills, commit stepping, multi-position/clamping/fractional, no-match,
  bg target, line accumulation, empty-skip.

No `.unwrap()` in production. fmt and clippy pass clean.

### 5.6.6 — Phase merge: release/5.6 → master

Merged all Phase 5.6 work (5.6.1–5.6.5) into default branch (master). Phase 5.6
complete: colour name hover tooltip (5.6.1), hue-grouped 5-per-row palette (5.6.2),
palette editor save/load/duplicate (5.6.3), palette import for 4 formats (5.6.4),
marker brush mode with colour-stepping shading (5.6.5).

Post-merge fixes:
- Marker brush `commit_marker_accum`: start_idx for unmatched colors now consumes
  1 step to enter array at index 0 instead of landing at index 1.
- Removed `accum.retain(|_, v| *v > 0.0)` to preserve fractional remainder entries.
- TUI dispatch: added `has_figlet_flags()` helper to prevent TUI launch when FIGlet
  CLI flags are provided. Added `std::io::stdin().is_terminal()` check so piped stdin
  goes to CLI mode. Fixes 47/48 integration tests (test_03_long_text was the holdout).
- Palette editor test: wrapped `XDG_CONFIG_HOME`-modifying tests behind a `Mutex` to
  prevent concurrent access race from parallel test execution.

### 5.7.1 — Animated GIF import to timeline

Created `figby-rs/src/gif_import.rs` — GIF decode module using the `gif` crate (0.13):
- `import_gif(path)` reads GIF file via `gif::Decoder::new()`, composites frames
  with proper disposal handling (Keep/Background/Previous), extracts frame delays
  (centiseconds) and loop count from `gif::Repeat` enum.
- `GifImportResult` contains `frames: Vec<Vec<Vec<CanvasCell>>>`, `frame_delays`,
  `loop_count`, and `palette_colors` for palette inference.
- `GifImportError` enum with `Io`, `Decode`, `NoFrames`, `TooLarge` variants.
- Memory guard: rejects GIFs with >1M total cells (`MAX_TOTAL_CELLS`).
- Disposal method handling: `DisposalMethod::Background` clears frame region to
  GIF background color, `DisposalMethod::Previous` saves/restores canvas state.

TUI integration:
- `FileOpsMode::ImportGif` variant in `tui/file_ops.rs` with `enter_import_gif()`,
  `handle_key_import_gif()`, `render_import_gif()` — filters directory to `.gif` files.
- `WelcomeAction::ImageImportGif` in `tui/welcome.rs` — 5th action in IMAGE_ACTIONS
  with `G` keybinding ("GIF Import"). `image_action_for()` updated for index 4.
- `MenuAction::FileImportGif` in `tui/menu.rs` — "Import GIF" item in File menu.
- `TuiApp::perform_import_gif()` in `tui/mod.rs` — decodes GIF, resizes canvas,
  copies first frame to active layer, populates timeline frames with thumbnails
  and `layer_state` buffers, sets frame delays on export dialog, switches to
  ImageEditor mode with timeline visible.

6 files created/modified: `gif_import.rs` (new), `lib.rs`, `file_ops.rs`,
`welcome.rs`, `mod.rs`, `menu.rs`. No `.unwrap()` in production. fmt and clippy pass clean.

### 5.7.2 — Phase merge: release/5.7 → master

Merged all Phase 5.7 work into default branch (master). Phase 5.7 complete:
animated GIF import to timeline (5.7.1). All 1 subtask implemented, tested, merged.
Next phase: 5.8 (Dynamic Lighting System).

### 5.8.1 — Core lighting engine (`lighting.rs`)

Created `figby-rs/src/tui/lighting.rs` with full core lighting engine:
- `Normal3(i8, i8, i8)` — quantized unit normal with `from_f32()`, `to_f32()`, `dot()`
- `NormalMap` — 2D normal grid with bounds-checked get/set/get_mut, fills flat `(0,0,1)`
- `Rgb(u8, u8, u8)` — color triple
- `Attenuation` — point-light falloff (constant/linear/quadratic) with defaults
- `Light` enum — Ambient/Directional/Point variants
- `Scene` — light collection with add/remove/clear methods
- `LutEntry` / `LightingLut` — 256-entry luminance→(color,char) LUT with lerp and default char map
- `compute_normal_map_figfont()` — Sobel 3×3 gradient on heightfield, mirror-padded borders
- `shade_canvas()` — per-cell Lambertian diffuse + shadow testing for all light types
- `cast_shadow()` — Amanatides & Woo DDA 2D grid traversal, distance-limited
- `intensity_to_char()` — luminance → char via linear index into char map

22 unit tests covering all components in isolation. No `.unwrap()` in production. fmt and clippy pass clean.

### 6.8.2 — New image dialog: canvas size + palette selection

Added `NewImageDialog` in `figby-rs/src/tui/dialogs/new_image.rs` — a dialog with
three navigable fields: Width (numeric entry, default 80), Height (numeric entry,
default 24), and Palette (cycle through built-in names with Left/Right). Tab/Up/Down
navigate fields, Enter confirms, Esc cancels. On confirm, creates `CanvasWidget` at
specified dimensions and loads selected palette into `PaletteEditor`.

Wired into `TuiApp` via `DialogState.new_image` field. `WelcomeAction::ImageNewBlank`
opens the dialog instead of hardcoding 80×24. Palettes sourced from
`palette_import::builtin_palettes()` (Grayscale, Primary, Warm, Cool).

### 6.9.1 — Layer panel: icon-based 2-row layout

Replaced text-heavy single-row layer entries with a 2-row icon-based design:
- **Row 1:** layer name with active marker (`›`), indented for grouped layers
- **Row 2:** compact attribute row — visibility icon (`layer_visibility_on`/`off`),
  lock icon (`layer_lock`/`unlock`), opacity %, blend mode icon
- Removed 3-line verbose help text; scrollbar indicators (▲/▼) retained
- Removed legacy `render_mask_thumbnail()` method (mask still functional via `m` key)

Key implementation detail: display tracking uses `display_row += 2` per layer
(group headers = 1 row). Scroll clamping targets the name row (first of the pair).
If only the name row fits, row 2 is skipped rather than clipped mid-entry.

### 6.9.2 — Layers: reorder by drag handle

Added drag-and-drop reorder for the layer panel:
- **Drag handle** ("⠿") rendered on the left edge of each layer name row (row 1)
- **Mouse drag:** click drag handle → drag to target position → release to reorder via `LayerStack::reorder()`. Visual highlight on target position during drag
- **Keyboard reorder:** Shift+Up / Shift+Down keybinds call `move_up()` / `move_down()`
- **Click to select:** clicking any layer row (outside drag handle) sets it active

Implementation: `LayerPanel` gained `drag_state: Option<(from, to)>` and `drag_hover_row` fields. New `layer_at_pos()` helper maps screen coords → layer index in the 2-row-per-layer display model. New `handle_mouse()` method dispatches Down/Drag/Up events. Wired into `TuiApp::handle_mouse_event()` after tab-label click handling.

### 6.9.4 — Move tool options to right sidebar

Removed the brush/text/lighting info panel from the bottom of the left toolbox
column. Tool options were already displayed in the right sidebar's Props tab
(via `SidePanel::render_tool_props`), so the left sidebar now shows only the
tool list and palette.

Changes:
- Removed `TOOLBOX_BRUSH_HEIGHT` constant and `toolbox_brush_borders()` from `layout.rs`
- Removed `toolbox_brush` field from `FrameLayout` struct and its split logic
- Removed brush/text/lighting rendering block from `TuiApp::render()`
- `toolbox_h` simplified to `Tool::all().len() as u16 + 2` (no brush height addend)
- Updated `test_brush_render_contains_shape_name` to open side panel Props tab

### 5.8.5 — Phase merge: release/5.8 → master (2026-06-18)

Merged release/5.8 branch into master at `480352d`. Brings 5.8.1 (core lighting
engine), 5.8.2 (canvas/layer integration), 5.8.3 (light management UI), and 5.8.4
(palette LUT integration) into the mainline. 34 files / 2520 lines merged. Phase
5.8 complete. Next phase: TBD.

### 7.0.1 — Commit timeline frame edits on switch

`commit_current_timeline_frame()` helper writes the live `layer_stack.composite()`
buffer into the currently-selected frame's `layer_state`, recaptures thumbnail,
and sets `has_keyframe = true`. Called at the **top** of Left/Right/timeline-click
handlers **before** mutating `current_frame`. This ensures edits survive frame
switching — previously only the frame→layer direction was wired, so every switch
unconditionally overwrote live edits with stale snapshots.

3 call sites added: Left arrow (`mod.rs:3374`), Right arrow (`mod.rs:3385`),
timeline click (`mod.rs:1992`). Regression test `test_timeline_frame_edits_persist_on_switch`
simulates edit → switch → switch-back and asserts cell content survives round-trip.

### 7.0.2 — Fix false "Cannot read file: stream did not contain valid UTF-8" on GIF import

Two bugs in the ImportGif dialog path:
1. `ImportGif` arm returned `Some(AppEvent::OpenRequested)` after calling
   `perform_import_gif(path)`; the dispatcher forwarded `OpenRequested` to
   `perform_open()`, which called `std::fs::read_to_string(&path)` on the binary
   GIF → UTF-8 decode failed → false error. Fix: return `None` instead.
2. `perform_import_gif`'s success path never reset
   `self.dialogs.file_ops.mode = FileOpsMode::Idle`, leaving the file-open
   dialog open after successful import. Fix: set `Idle` in the `Ok` branch.

Diff: `mod.rs:2870` return `None` (was `Some(AppEvent::OpenRequested)`),
`mod.rs:4262` set `file_ops.mode = Idle` after `composite()`.

### 6.10.1 — Fix `capture_timeline_frames` ignoring per-frame `layer_state`

`capture_timeline_frames()` in `export.rs` was rebuilding every animation frame
from the live (unchanging) layer stack instead of each frame's own `layer_state`
raster snapshot (populated by GIF import / 'A'-key manual capture). The
current_frame counter advanced (progress bar looked right) but the picture never
changed — affecting both inline playback and GIF/APNG export.

Fix: check for `timeline.frames[frame_idx].layer_state` first; if present,
return it directly instead of re-rendering through the live layer stack.
Added regression test `test_capture_uses_per_frame_layer_state_when_present`
that would generate identical frames without the fix and distinct frames with it.

### 7.0.3 — Reconcile playback cursor with timeline `current_frame`

`AnimationPlayer::current_frame` (player.rs:29) and `TimelineState::current_frame`
(timeline.rs:165) were two independent cursors that never reconciled after
playback start. The tick handler advanced the player's `Cell<usize>` but never
wrote to `timeline_state.current_frame`, so the timeline strip stayed frozen on
the play-start frame. `stop_inline_playback` dropped the player without saving
the last frame, leaving the canvas on the start frame.

Fix: sync `timeline_state.current_frame` from `player.current_frame()` on every
tick. On stop/dismiss, copy the player frame and call
`load_current_timeline_frame()` so the canvas holds the last-rendered frame.
Removed `self.seek(0)` from player's Esc arm and the vestigial
`TimelineState::playing` field.

### 7.1.2 — Rebind chrome keys to Alt+arrows

Side-panel tab-cycle (`mod.rs`), layer-panel arrow/Tab/S handlers (`layers.rs`)
all moved from bare keys to Alt-modified keys. Timeline frame advance now works
even with sidebar open (bare Left/Right reach the timeline block first because
the sidebar block is gated on Alt). Palette nav block reordered above canvas
cursor block so palette arrows aren't shadowed.

Key changes:
- `mod.rs:3368-3383`: sidebar tab-cycle gated on `modifiers == KeyModifiers::ALT`
- `mod.rs:3664-3675`: inline T/Shift+T handlers removed (handled via dispatch_global)
- `mod.rs:3670`: tool selector excludes `c != 'T'` so T falls through to dispatch
- Palette nav block moved before canvas cursor block (arrow priority fix)
- `layers.rs`: all arrow/Tab/S handlers gated on Alt or Alt+Shift
- `keymap.rs`: new GlobalAction variants `CycleTabPrev`, `CycleTabNext`,
  `OpenTweenPanel` with dispatch entries and KEYMAP display entries for Alt
  bindings + lighting-mode bindings

### 7.2.1 — Make Props tab editable: clickable +/- rects + typed-entry mode

- **New module**: `tui/props_panel.rs` hosts `PropsPanel` (mode state, rects, char_buffer),
  `PropAction` enum (17 variant actions), `PropsWidgetRect` (Rect + action pair),
  and `PropsPanelMode` (Idle / EditingChar). Hit-testing and typed-entry key handling
  live here, keeping `side_panel.rs` focused on rendering.
- **Widget rect pattern**: `SidePanel::add_brush_props` / `add_text_props` / `add_fill_props`
  push `PropsWidgetRect` entries into `rects: &mut Vec<PropsWidgetRect>` during render,
  computing x/y from the layout position and `line_y` accumulator. Numeric fields (size,
  density, scale, threshold) get `[-]` / `[+]` button rects; enum fields (shape, mode,
  justification, font) get clickable-value rects; Char gets a click-to-edit rect.
- **Mouse dispatch**: `TuiApp.props_panel.rects` is cleared before each side-panel render,
  populated during render, then hit-tested in `handle_mouse_event` when `TabId::Props`
  or `TabId::Text` is active. Actions dispatch through `dispatch_props_action()` which
  maps each `PropAction` variant to the corresponding `BrushState`/`TextToolState` mutation.
- **Typed-entry mode**: Clicking Char field sets `mode = EditingChar`. Key events are
  intercepted before toolbox/global handlers: first key commits the char, Esc cancels.
  `unwrap_or('\u{2588}')` fallback for empty buffer provides safe default.
- **Not wired for 7.2.1**: Emitter and Lighting props panels accept rects/line_y params
  but don't push clickable rects yet (pass `_rects` / `_area`). `FillThresholdUp/Down`
  actions exist but brush pipelines do not currently read fill_threshold
  (could be wired in a later task). `BeginEditField` variant reserved for future generic
  typed-entry fields.
- **Files touched**: `figby-rs/src/tui/props_panel.rs` (new), `figby-rs/src/tui/mod.rs`
  (module decl, field, action dispatch, mouse/key handler integration),
  `figby-rs/src/tui/side_panel.rs` (refactored add_*_props signatures, per-tool rect
  generation, `line_y` tracking).

### 7.2.2 — Dedicated props builders for the seven hollow tools

- **New state types**: `MoveState`, `RotateState`, `LineState`, `SelectionState` added to
  their respective tool modules. Each holds the properties displayed in the panel (stride,
  snap, wrap; step_angle, direction, pivot; width, arrowhead, curve; feather, additive,
  subtractive, move_with_arrows).
- **New props builders**: `add_move_props`, `add_rotate_props`, `add_select_props`,
  `add_line_props` in `side_panel.rs` replace the old `add_tool_keybinds` fallback.
  Move/Rotate/Marquee/Lasso/CircleSelect/PolygonSelect/Line all render interactive
  props instead of the static tool-shortcut catalogue.
- **Selector dispatch**: Match arm `_ => add_tool_keybinds(...)` replaced with explicit
  arms for each of the 15 `Tool` variants — compiler-enforced exhaustive match.
- **Fill threshold**: `add_fill_props` already had +/- rects; `FillThresholdUp/Down`
  handlers dispatch correctly. Fill tool is now fully editable from the Props tab.
- **Files touched**: `figby-rs/src/tui/side_panel.rs`, `figby-rs/src/tui/mod.rs`,
  `figby-rs/src/tui/props_panel.rs`, `figby-rs/src/tui/tools/move_tool.rs`,
  `figby-rs/src/tui/tools/rotate_tool.rs`, `figby-rs/src/tui/tools/line.rs`,
  `figby-rs/src/tui/tools/selection.rs`.

### 7.3.1 — Extract handle_key_event mode blocks into per-mode handle_key methods

- **Extracted to EditorState**: Rotate tool (Left/Right 90° step), selection movement
  (arrows + Delete), clipboard (Ctrl+C/X/V), Move tool layer nudging, polygon select
  (Enter closes / Esc cancels), deselect on Esc, keyboard painting (Space/Enter for
  brush/eraser/fill/spray/line).
- **Extracted to AnimationState**: Timeline frame navigation (Left/Right/Add/Delete),
  emitter bake/toggle (b/B/v keybindings).
- **Dispatcher**: `handle_key_event` in `mod.rs` became a short ordered list of
  `if self.<state>.handle_key(...) { return None; }` calls — welcome, font editor,
  image editor, text tool, editor state, side panel Alt+arrows, animation state,
  lighting.
- **Text tool**: Already had its own `TextToolState::handle_key` — deleted the old
  inline block from `handle_key_event`.
- **`selection_polygon_points`**: Moved from `InteractionState` to `EditorState` so
  `EditorState::handle_key` can access it without borrowing TuiApp.
- **`push_undo_snapshot`**: Changed from `fn` to `pub(crate) fn` for cross-sub-struct
  access.
- **Files touched**: `figby-rs/src/tui/mod.rs` only.

### 7.3.2 — Extract `render_canvas_area` + `render_overlays` residual blocks

- **`AnimationState::render`**: New method on `AnimationState` (mod.rs:999-1023) renders
  the inline player widget if active. Signature: `render(&self, frame, area, borders) -> bool`.
- **`render_canvas_area`**: Changed signature to `(&mut self, frame, canvas_area, &FrameLayout)`.
  Removed redundant `layout::FrameLayout::compute` that duplicated the one already computed
  in `render()`. Replaced the inline player block with `self.animation.render(...)` call.
- **Three call sites** (zen, lighting, normal modes) updated to pass `&fl`.
- **`overlays.rs` audit**: Confirmed no residual overlay logic in `render()` — all floating
  dialogs (export, file ops, keybindings, undo, keyframe editor, tween, new image, system
  font, rascii import, emitter config, palette editor, quit confirm) already dispatched via
  `render_overlays`.
- **Net reduction**: ~37 LOC removed from `render_canvas_area`.
- **Files touched**: `figby-rs/src/tui/mod.rs` only.

### 7.3.4 — Split `mod.rs` into topical submodules

- **New files**: `tui/app_state.rs` (struct/enum defs + EditorState/AnimationState/LightingState impls + `TuiApp::new`/`Default`/`AsyncResult` + editor/default-side-panel tests), `tui/event_loop.rs` (`run`/`handle_event`/`process_event`/`check_async_completion`/`trigger_quit`), `tui/dispatch.rs` (key/mouse dispatch + every `perform_*`/`start_*` action handler + playback/sidebar tests).
- **`mod.rs` keeps**: `pub mod` decls + `pub use app_state::*` re-exports + `impl TuiApp { render / render_canvas_area / mode_name_string }` + 4 shared free helpers (`centered_overlay`, `rotate_drag_steps`, `capture_thumbnail`, `format_clock`) + rotate-drag tests.
- **Visibility**: cross-module inherent methods bumped to `pub(crate)` (e.g. `check_async_completion`, `trigger_quit`, `process_event`, `handle_mouse_event`, `handle_paste_event`, `perform_save/open/export`, `handle_menu_action`, `default_side_panel_open`, `EditorState::{compute_canvas_rect,screen_to_buffer,sync_canvas_to_font_char,sync_font_char_to_canvas,sync_image_to_canvas,handle_selection_down/drag/up}`, `LightingState::handle_key`, `AnimationState::{commit_current_timeline_frame,load_current_timeline_frame}`).
- **Tests relocated** by what they access: editor/default-panel tests → `app_state.rs`; playback/sidebar tests → `dispatch.rs`; rotate-drag tests stay in `mod.rs`.
- **LOC**: mod.rs 5693 → 774 (target ≤1500). Banner diff vs `figlet` byte-identical.
- **Files touched**: `figby-rs/src/tui/{mod,app_state,event_loop,dispatch}.rs`, `AGENTS.md`, `docs/todo-v7.md`, `CHANGELOG.md`, `figby-rs/Cargo.toml`.

### 7.4.1 — Lighting help overlay + keybinds

- **keymap.rs**: Added `Scope::Lighting` variant. Migrated lighting KEYMAP entries from `Scope::Global` to `Scope::Lighting`. Added missing `Esc` (exit) and `Shift+↑/↓` (vertical move) entries.
- **light_panel.rs**: Added `show_help: bool` field + one-shot keybinding help block in `render()`. Shows compact listing: Esc=exit, ↑/↓=select, ←/→=move, Sh+↑↓=v-move, +/-=intensity, A/D/P=add, Del=remove.
- **dispatch.rs**: Sets `panel.show_help = true` on G-press entry into lighting mode.
- **Files touched**: `figby-rs/src/tui/{keymap,light_panel,dispatch}.rs`.

### 7.4.2 — Wire FIGfont density heightmap

- **text.rs**: Changed `render_text_to_buffer()` to set `height: Some(255)` on non-space FIGfont cells instead of `height: None`. This gives the heightfield non-zero data where text is placed, making normal maps non-flat and lighting visually reactive.
- **lighting.rs**: Added `compute_normal_map_non_empty_glyph` test asserting normals at edges of a raised block tilt away from the block.
- **lighting-design.md**: Updated status from "Deferred to v4.x" to "Partially Implemented (FIGfont density path, v7.4)".
- **Files touched**: `figby-rs/src/tui/tools/text.rs`, `figby-rs/src/tui/lighting.rs`, `docs/lighting-design.md`.

### 7.5.1 — Edge + layer-cell collision for particles

- **particles.rs**: Added `EdgeMode` enum (Bounce/Wrap/Despawn) with serde support. `ParticleConfig` gains `edge_mode` + `collide_with_layer`. `ParticleSystem::update()` now takes `bounds: Option<(usize, usize)>` and `layer_mask: Option<&CanvasBuffer>`. Collision step inserted between velocity apply and lifetime decrement: edge bounce/wrap/despawn per mode, layer-cell collision computes 4-neighbor normal and reflects velocity.
- **event_loop.rs**: `update()` call passes canvas bounds and optional layer buffer.
- **EmitterConfigPanel**: Fields 17 (Edge Mode) and 18 (Collide w/ Layer) added to config panel UI.
- **7 new tests**: edge bounce left/right, wrap, despawn, no-bounds passthrough, layer reflect, layer disabled.
- **Files touched**: `figby-rs/src/tui/particles.rs`, `figby-rs/src/tui/event_loop.rs`, `docs/todo-v7.md`.

### 7.5.2 — Per-particle keyframe tracks + lifecycle hooks

- **particles.rs**: Added `ParticleKeyframe` type (time/color/size/character/opacity, serde). `Particle` gains `total_lifetime` (spawn snapshot for progress denominator), `keyframes: Vec<ParticleKeyframe>`, `is_secondary: bool`. `Particle::render_values()` interpolates between adjacent keyframes: linear lerp on color/size/opacity, nearest-endpoint pick on character. `Particle::progress()` helper. `ParticleConfig` gains `keyframes`, `on_death_count`, `on_death_config: Option<Box<ParticleConfig>>` for recursive sub-config (Box keeps fixed size). `ParticleSystem::update()` collects on-death secondaries before `retain()`; secondaries flagged `is_secondary = true` so bursts are non-recursive.
- **render_to_canvas / bake_to_buffer**: Now call `render_values()` per particle instead of reading static fields. Interpolated color/char applied.
- **docs/particles-design.md**: New design sketch covering data model, interpolation rules, lifecycle hooks, deferred work.
- **11 new tests**: keyframe color at 25%/50%/75%, character step low-t/high-t, empty-track fallback, progress clamping, render-to-canvas integration, on-death burst spawns N, non-recursive burst, disabled when count=0, secondary inherits keyframes.
- **Files touched**: `figby-rs/src/tui/particles.rs`, `docs/particles-design.md`, `docs/todo-v7.md`, `CHANGELOG.md`, `figby-rs/Cargo.toml`.

### 6.0.26 — Text tool UX redesign

- **Problem**: Text tool required click-canvas → type → Enter (modal entering_text). Font cycled on click with no visible list. No preview before placing.
- **Solution**: Removed `entering_text`. Text buffer is always present. Sidebar Text tab shows buffer content; editing activates when Text tab is open. Font selector changed from click-to-cycle to prev/next buttons (`[<] name [>]`). Live FIGlet preview renders under cursor on mouse hover (`show_preview` + `preview_pos`). Click places text as `TextBlock` object. Click existing block to re-edit. `[Rasterize]` button paints block onto layer pixels and removes the text object.
- **Key pattern for preview**: A clone of `TextToolState` is created during render, `render_rows_from_buffer()` called, result pushed as a `TextOverlay` alongside committed blocks. The preview overlay is rebuilt every frame so it tracks cursor movement.
- **Editing activation**: `editing` flag set `true` when user clicks Text tab, Alt+arrows to Text tab, or Text tab is open on key dispatch. Cleared on Esc. Character keys route to `text_buffer` via `handle_key`.
- **Files touched**: `figby-rs/src/tui/tools/text.rs`, `figby-rs/src/tui/side_panel.rs`, `figby-rs/src/tui/dispatch.rs`, `figby-rs/src/tui/mod.rs`, `figby-rs/src/tui/props_panel.rs`, `figby-rs/tests/tui.rs`.

### 7.6.1 — GIF import sizing dialog

- **New dialog**: `tui/dialogs/gif_import.rs` with two-phase flow: file browser → sizing options. Shows original GIF resolution, editable image width/height, canvas width/height (separate from image size), and keep-proportions toggle. Image auto-centers on canvas.
- **`GifScaleTarget::Exact(w, h)`**: New variant for aspect-ratio-free scaling — maps directly to cell dimensions without terminal-cell compensation.
- **`probe_gif_dimensions()`**: Cheap header-only GIF dimension reader via `gif::Decoder`, used by dialog to show native resolution before full decode.
- **`perform_import_gif`**: Changed signature from `PathBuf` to `GifImportConfig`. Uses dialog's `image_scale` for frame scaling and `canvas_width/height` for final canvas size. First frame + timeline frames centered on canvas via `x_off/y_off` offset.
- **Files touched**: `figby-rs/src/gif_import.rs`, `figby-rs/src/tui/dialogs/gif_import.rs` (new), `figby-rs/src/tui/dialogs/mod.rs`, `figby-rs/src/tui/app_state.rs`, `figby-rs/src/tui/overlays.rs`, `figby-rs/src/tui/dispatch.rs`, `figby-rs/tests/tui.rs`.

### 6.0.27–6.0.31 — Part Twah 8.0–8.4 (docs/todo-v8.md)

Milestone plan: `docs/todo-v8.md`, scoping the 9 open findings in
`docs/manual-testing-v7.md`'s "Part Twah" section. Phases 8.0–8.4 shipped
this session; 8.5 (multi-document tab strip) and 8.6 (figmap format) are
deliberately deferred — both are XL architectural rewrites the plan itself
flags as their own milestones, not incremental fixes like 8.0–8.4.

- **8.0** (`0e8cf79`): `BrushState::new()` default size 3→1.
  Playback trigger moved from `KeyCode::Enter` to `KeyCode::Char(' ')` at
  `dispatch.rs`'s timeline-play check — same priority Enter used to have
  (wins over Layers-panel toggle and over keyboard-paint tools whenever the
  timeline has frames). Enter remains the keyboard-paint key. Extracted
  `Tool::is_paint_tool()` to dedupe a match that was copy-pasted between
  `dispatch.rs` and `app_state.rs`.
- **8.1** (`2179ec9`): File dialog overhaul in `file_ops.rs`. Added `..`
  to the normal (non-zip) directory listing. Left/Right now navigate
  (`go_to_parent`/`select_entry`) alongside Tab. New `entry_is_selectable()`
  gates Enter to actual selectable targets. New `handle_mouse()` +
  `entry_rects: Vec<(usize, Rect)>` (populated in a new shared
  `push_entry_lines()` render helper) give every file dialog mouse
  click/scroll support — wired into `dispatch.rs::handle_mouse_event`,
  which previously unconditionally dropped mouse events while any file
  dialog was open. `ImportFont` mode's visibility filter now includes
  `.zip`; when browsing one, completion routes through `perform_open()`
  (Open/ZipEntry path) rather than `perform_import_font()`'s TTF
  conversion, since FIGlet's zip convention holds `.flf`/`.tlf` not
  `.ttf`/`.otf`. Collapsed 5 near-duplicate `handle_key_*` functions into
  one `handle_key_browse()`. Deleted a ~390-line dead
  `impl Widget for &FileOpsDialog` (confirmed via grep it was never called
  — only the inherent `.render()` method was).
- **8.2** (`2d31c42`): New `Tool::Braille` (key `k`), separate toolbox
  entry (not a Brush submode, per explicit ask). Sub-cursor state
  (`sub_x`/`sub_y`, 2x4 dot grid) lives directly on `BrushState` rather
  than a new parallel struct — simpler than threading a new type through
  `side_panel.rs`'s already-large `render_tool_props` call chain. New
  `image_input::toggle_braille_dot(ch, dx, dy)` flips one dot in a braille
  char without disturbing others already set, reusing the `BRAILLE_BASE`
  convention. Arrow keys move the dot cursor within the current cell,
  Space/Enter toggles — true sub-cell **mouse** precision isn't available
  (terminal mice report whole-cell coordinates only), so this is
  keyboard-only sub-cell accuracy, flagged explicitly as a scope
  reduction from the note's literal phrasing.
  **Real bug found and fixed along the way**: FontEditor's
  type-to-search-by-glyph feature (`font_editor.rs:1450`) swallows any
  character not on its hardcoded tool-shortcut exclusion list before
  it reaches tool selection — `k` hit this immediately. `u`/`r`/`m`/`n`
  have the same pre-existing gap for Move/Rotate/Emitter/Lighting; left
  alone, out of scope, tracked under the separate keymap-overhaul item.
- **8.3** (`d1c9920`): `SystemFontPickerDialog` gained `Field::Charset`
  (Tab/Left/Right), backed by new `pub const CHARSET_NAMES` (12 presets
  matching `font_gen::resolve_charset`). New
  `dialogs/font_import_options.rs` (`FontImportOptionsDialog`) adds the
  previously-nonexistent options step to the "New Font from File" flow —
  shown after file selection via `complete_file_ops_dialog`'s `ImportFont`
  branch, before `perform_import_font(path, size, charset)` (signature
  changed from hardcoded `12.0`/`DEFAULT`).
- **8.4** (`f939493`): `FileOpsDialog` gained `filename_buffer` (separate
  from `path_buffer`, now directory-only for `SaveAs`) and `save_focus:
  SaveFocus` (Tab-toggled). `enter_save_as(current, default_name)` seeds
  the filename from the font's existing path, or
  `"{font_storage_name or 'untitled'}.flf"` for a never-saved font.
  `selected_path()` branches to a new `save_target_path()` for `SaveAs`
  that combines the two fields (trusting `path_buffer` directly as the
  directory — deliberately *not* reusing `current_parent_dir()`'s
  filesystem-existence check, since a save destination may not exist yet).
  Clicking an existing file in the listing now populates `filename_buffer`
  instead of clobbering `path_buffer`.
- **8.5/8.6 next-session pickup**: see `docs/todo-v8.md` phases 8.5–8.6
  for the full design (hot-swap `Document`/`DocumentKind` model to avoid
  a ~680-call-site rewrite; figmap JSON schema reusing `layers::Layer`/
  `timeline::TimelineFrame` directly). Nothing in the working tree is
  half-finished — every phase above landed with full
  build/test/clippy/fmt verification and its own commit.
