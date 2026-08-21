# GPT Comprehensive Review — 2026-08-20

Repository: `ClawfficeOrg/Figby`

Reviewed branch: `master`

Reviewed commit: `53d39b9362c79e3d4732e7bad73647684f636dc1`

Review type: source, architecture, security, tests, packaging, release, and documentation

Build constraint: all Cargo work was limited to four jobs

## Executive summary

Figby has an unusually broad feature set, substantial test coverage, a useful
module split, and several good defensive controls. The native crate builds, the
source is formatted, and the WASM target checks successfully. The earlier
template command-substitution vulnerability has also been removed.

The current `master` should nevertheless **not be released, published to
crates.io, or described as green**. A clean checkout cannot compile the exact
Clippy and test targets used by CI, a packaged crate cannot compile because
required assets live outside the crate, and the production GIF/APNG timeline
export path discards saved frame snapshots. The latter can silently produce an
animation containing repeated copies of the current canvas.

Several high-impact correctness and safety problems remain behind those
blockers: timeline navigation can flatten state into the wrong layer, rendering
can overwrite image edits, save completion is not revision-aware, some edits do
not mark the document dirty, and multiple untrusted-input paths can panic or
allocate far beyond their nominal limits. Dependency resolution is not
reproducible, and the current graph includes an unsound `lru` advisory in the
WASM dependency chain.

Recommended release decision: **hold** until at least F-01 through F-12 are
resolved and enforced in clean-checkout CI.

## Scope and method

The review covered:

- Rust CLI, library, TUI, animation/export, template, parser, and WASM paths;
- tests and ignored compatibility tests;
- GitHub Actions, Cargo packaging, dependencies, release metadata, and docs;
- prior reviews and milestone documents, to distinguish fixed findings from
  regressions or incomplete remediations;
- static adversarial analysis of files, templates, fonts, GIFs, terminal text,
  and automation scripts.

No deliberate out-of-memory, stack-overflow, symlink-clobber, or credential
exfiltration proof was run. Those findings were established from reachable code
paths and should be reproduced only with bounded fixtures in an isolated test
environment.

## Severity model

- **Blocker**: prevents the claimed build, test, package, or core output from
  working correctly.
- **High**: likely data loss, security boundary failure, panic, denial of
  service, or a materially false core-product guarantee.
- **Medium**: important reliability, supply-chain, portability, or maintenance
  problem that should be scheduled before a stable release.
- **Low**: cleanup or defense-in-depth work with limited immediate impact.

## Release blockers

### F-01 — Blocker — Clean-checkout CI targets do not compile

`.github/workflows/ci.yml:35-39` runs Clippy with `--all-targets` and then the
test suite. Test code in `figby-rs/src/font_gen.rs:1116-1150` uses
`include_bytes!("../../assets/fonts/to-convert/Lixdu.ttf")`, but that file is
absent and the containing directory is ignored by `.gitignore:35-37`.

Both of these commands stop at compile time:

```text
cargo clippy --manifest-path figby-rs/Cargo.toml --all-targets --all-features -- -D warnings
cargo test --manifest-path figby-rs/Cargo.toml --no-fail-fast
```

The errors are at `font_gen.rs:1119` and `font_gen.rs:1143`. This is not a local
environment issue: a clean clone lacks the compile-time fixture by definition.

Recommended fix:

1. Add a small, redistribution-compatible font fixture under
   `figby-rs/tests/fixtures/`.
2. Reference that tracked fixture from the tests.
3. Add a CI assertion that required fixtures and compile-time assets are
   tracked.
4. Run the gates from a fresh checkout, not a working tree containing ignored
   development files.

### F-02 — Blocker — The test suite remains red after the fixture is supplied

In an isolated audit copy, the missing font reference was temporarily replaced
with the tracked `Fattern.otf` solely to reveal the next failure. Compilation
then succeeded and most of the suite passed: 1,161 library tests, 67 binary
tests, and the other integration suites were green. However,
`figby-rs/tests/tui.rs:23-50` failed at line 49 because
`test_tui_smoke_all_panels_render` expects a status-bar `FPS:` label that the
current UI does not render.

The temporary substitution was not made in this branch. Decide whether the
status label or the assertion represents intended behavior, repair it, and
keep the smoke test as a required gate.

### F-03 — Blocker — `cargo package` produces a crate that cannot compile

The crate compiles in the repository partly because production source embeds
files outside `figby-rs/`. Cargo packaging includes only the crate directory,
so verification fails on at least:

- `figby-rs/src/tui/theme.rs:4` → `assets/tui/themes/default.yaml`;
- `figby-rs/src/tui/welcome.rs:13` → the welcome ASCII image;
- `figby-rs/src/tui/app_state.rs:20` → `assets/tui/icons.yaml`.

The WASM code at `figby-rs/src/web.rs:134-139` similarly embeds repository-level
fonts that are not part of the package. The root license is also outside the
crate package. `figby-rs/Cargo.toml:1-9` has no package layout that resolves
this.

Move every required runtime and compile-time asset under the crate package,
declare package inclusion explicitly, and gate both `cargo package` and
`cargo publish --dry-run`. The README's Cargo installation instructions should
remain qualified until a package made from a clean tag verifies and runs.

### F-04 — Blocker — Production GIF/APNG export ignores saved frame pixels

Timeline frames can contain a saved `layer_state` (`tui/timeline.rs:30-37`), and
import/manual frame paths store those snapshots
(`tui/dispatch.rs:2250-2276`, `tui/app_state.rs:931-949`). The production export
path at `tui/dispatch.rs:2389-2467`, however, constructs each frame from the
current live `layer_stack`; it does not use that frame's saved layer state.

This can export repeated copies of the current canvas, with only transform
differences, instead of the user's actual timeline. A tested helper in
`tui/export.rs:663-733` does prioritize `layer_state`, but production bypasses
that helper.

Use one compositor/capture function for preview, tests, GIF, APNG, and ANSI
animation. Add an end-to-end test with two visually different saved frames and
assert distinct decoded export frames.

## High-severity findings

### F-05 — High — Timeline state has two contradictory authorities

`TimelineFrame` contains both `layer_state` and keyframes. The export helper
prefers snapshots and ignores keyframes when a snapshot is present, while the
production exporter behaves in the opposite direction. Playback does not
consistently commit current edits before changing frames
(`tui/dispatch.rs:2618-2635`).

Navigation is also destructive: the commit path stores a composite image
(`tui/app_state.rs:869-878`), while the load path writes that flattened image
into only the active layer and leaves other layers intact
(`tui/app_state.rs:132-140,903-928`). A multilayer document can therefore be
duplicated or corrupted merely by moving through the timeline.

Choose a single frame authority. Prefer a versioned full-document snapshot or a
well-defined base document plus deterministic deltas. Navigation should restore
the same structural state that was saved.

### F-06 — High — Rendering can overwrite image edits

Every render synchronizes the image editor into the canvas
(`tui/mod.rs:480-486`). That synchronization copies cached `ImageEditor` cells
into the active layer (`tui/app_state.rs:219-235`), while drawing tools mutate
the layer directly (`tui/dispatch.rs:762-875`). If the editor cache is stale,
an ordinary redraw can overwrite a paint operation. GIF import also does not
reliably reset that cache.

Rendering must be read-only. Move synchronization to explicit model mutation
events, establish one source of truth, and add a regression test that paints,
redraws, and verifies the paint survives.

### F-07 — High — Save completion can clear newer unsaved changes

Font save clones a snapshot and writes asynchronously
(`tui/dispatch.rs:1896-1916,2065-2084`). Completion unconditionally marks the
application clean and may satisfy a pending quit
(`tui/event_loop.rs:138-147`), even if the user edited the document after the
save began.

Assign every document mutation a monotonically increasing revision. Capture
the saved revision with the job, and clear dirty state or complete a deferred
quit only if the current revision still matches it.

### F-08 — High — Dirty-state and destructive-transition handling can lose work

Font editor mutations in `tui/font_editor.rs:850-883,947-968,1134-1403` are not
consistently propagated to the application's unsaved flag. Several layer,
timeline, tween, and resize mutations also omit dirty-state updates. Conversely,
some mouse or redraw activity can mark the application dirty without a model
change.

Open and New replace significant application state without a uniform unsaved
guard (`tui/dispatch.rs:36-48,94-107,1182-1207,2093-2101`). Their reset logic is
partial, leaving possible undo, timeline, selection, or export state from the
previous document. In image mode, “Save and Quit” sets a pending flag, but the
save implementation returns early unless the app is in Font Editor mode.

Centralize all document mutations behind a document revision. Make Open, New,
Quit, and mode changes use the same save/discard/cancel state machine, and reset
the entire document aggregate atomically.

### F-09 — High — Undo and layer locks do not protect document structure

Undo entries store only a `CanvasBuffer` (`tui/undo.rs:3-14`) and snapshot only
the active layer (`tui/app_state.rs:142-146`). Undo then applies the buffer to
whichever layer is active at undo time (`tui/dispatch.rs:1817-1833,2751-2769`).
Editing layer A, switching to layer B, and undoing can therefore corrupt layer B.
Layer creation/deletion/reorder, timeline state, selection, and font state are
outside the same transaction model.

Layer lock is similarly cosmetic: the flag lives in `tui/layers.rs:130-159`,
but several editing paths mutate `active_layer` without a centralized
editability check.

Undo commands should carry target identity and inverse structural operations,
or snapshot a bounded document aggregate. All writes must pass one lock-aware
mutation API.

### F-10 — High — Template image containment can be bypassed

`RenderConfig` does not carry the template's base directory
(`template.rs:132-138`). Image rendering rejects absolute paths and lexical
`..` components, then opens the source relative to the process working
directory (`template.rs:583-598`). A relative symlink can escape the intended
template boundary. This does not meet the containment promised in
`docs/todo-v6.md:32-39`.

Pass the template base explicitly, canonicalize both base and target, require
containment immediately before opening, and use no-follow/open-at-style handling
where the platform permits it. Define clearly whether remote, absolute, and
symlinked sources are supported.

### F-11 — High — Template images and environment expansion expose data and resources

Template text expands arbitrary `${VAR}` values through `std::env::var`
(`template.rs:140-160,642-643`). A shared template can encode CI tokens or
other process secrets into rendered output. Environment expansion should be
off by default or limited to an explicit caller-provided allowlist.

Image tags forward attacker-controlled optional dimensions to
`rascii_art::render_to` (`template.rs:566-598`). The resolved dependency uses an
unrestricted image open, and it panics if both dimensions are absent. Huge
dimensions and decompression-bomb inputs bypass Figby's limited image loader.
Require a bounded nonzero dimension and route decoding through the same limited
reader as CLI/TUI image import.

The previous `$(command)` template RCE appears fixed and should stay covered by
a test proving that shell-like text remains literal.

### F-12 — High — Template and dialog dimension caps have allocation bypasses

The template canvas guard checks `width.saturating_mul(height)` at
`template.rs:490-521`. `width = 0, height = u32::MAX` passes the cell check and
then attempts to allocate billions of empty row vectors. Padding and margin are
added later (`template.rs:679-695`) and are not included in the budget.

New-image and GIF-canvas dialogs allow dimensions up to 65,535 per side
(`tui/dialogs/new_image.rs:82-140`, `tui/dialogs/gif_import.rs:263-325`) before
constructing nested vectors in `tui/canvas.rs:47-53`.

Reject zero dimensions and use checked arithmetic for final padded dimensions,
cell counts, row overhead, and estimated bytes before any allocation. Prefer a
fallible flat buffer to nested row vectors.

### F-13 — High — Scaled GIF import retains unbounded native frame data

`gif_import.rs:209-225` clones each decoded native frame into `raw_frames`.
The guard counts scaled output cells (`out_width × out_height × frame_count`),
not native decoded bytes. A GIF with very large native frames scaled to 1×1 can
therefore pass the nominal limit while retaining all full-resolution frame
buffers.

Decode and composite frames as a stream. At minimum, cap input bytes, frame
count, native pixels, cumulative decoded bytes, and scaled output bytes
independently.

### F-14 — High — Export can allocate multi-gigabyte transient buffers

The UI permits a 200×200 cell canvas and font size 72
(`tui/status.rs:53-64`). `output.rs:240-305` treats font size as a pixel scale,
allocates a nested RGBA matrix, and then duplicates it into a contiguous raw
byte vector. One such image can exceed practical memory; animation multiplies
the cost by frame count.

Define an explicit pixel and byte budget before allocation, use checked
arithmetic, and stream scanlines/frames directly to encoders. Clarify in the UI
whether the value is point size or integer scale.

### F-15 — High — Unicode input can panic in the WASM editor

`web.rs:39-58` stores a byte index but advances and retreats it by one for each
Unicode scalar. `String::insert` and `String::remove` require valid UTF-8 byte
boundaries. After entering a multibyte character such as `é`, a subsequent edit
can use an interior byte index and panic.

Store the cursor as a character or grapheme index and convert through validated
boundaries for mutation. Add browser tests for accented text, CJK, emoji, and
combining sequences.

### F-16 — High — Control and HZ parsers can exhaust the process stack

`control.rs:65-200` recursively re-enters the ISO-2022 parser for state/control
bytes. `input.rs:36-60` recursively skips HZ sequences. A long input containing
shift or ignored control sequences can create recursion proportional to input
length and overflow the stack.

Replace both paths with explicit state-machine loops and add bounded adversarial
tests containing long control runs.

### F-17 — High — Palette parsing can panic on valid UTF-8 JSON

`palette_import.rs:126-135` checks string byte length and then slices at fixed
byte offsets. A valid JSON value such as `"éx"` is three bytes but has no UTF-8
boundary at byte one, so slicing can panic. Related fixed-offset slicing appears
in shadow-color and palette-editor helpers.

Validate that a color is ASCII and consists only of the expected number of hex
digits, parse bytes without unchecked string slicing, and return a structured
error. Add Unicode and malformed-input property tests for every palette format.

### F-18 — High — Public and CLI image dimensions are not consistently bounded

The CLI accepts unrestricted `u32` target widths (`main.rs:267-268`), and resize
paths in `image_input.rs:82-193,318-366` allocate from caller-supplied output
dimensions. The public `image_to_ascii` path at `image_input.rs:200-209` uses
unrestricted `image::open`, bypassing the limited reader used elsewhere.

All image entry points should share one decoder limit and one checked output
cell/byte budget. Library APIs should return an error rather than trusting the
caller to choose safe dimensions.

### F-19 — High — Dependency resolution is unreproducible and includes an unsound crate

`.gitignore:1-4` excludes `Cargo.lock` even though Figby is an application. CI
hashes that absent lockfile (`.github/workflows/ci.yml:23-30`) and runs Cargo
without `--locked`. Current compatible-range resolution selected versions such
as Ratatui 0.30.2 and Clap 4.6.6, so the tested graph can change without a source
commit.

`cargo audit --no-fetch` against the audit resolution reported:

- `lru 0.16.4`, RUSTSEC-2026-0253, potential use-after-free/unsoundness through
  `ratzilla → beamterm-renderer → beamterm-core → lru`;
- unmaintained `ansi_term 0.12.1`, RUSTSEC-2021-0139;
- unmaintained `paste 1.0.15`, RUSTSEC-2024-0436.

Commit the binary lockfile, use `--locked`, update or patch the advisory path,
and add RustSec plus license/source policy gates. A lockfile should be reviewed
whenever the WASM renderer graph changes.

### F-20 — High — Autonomous Ralph workflow gives untrusted prompts broad authority

`scripts/ralph.sh` inserts repository-controlled todo, skill, diff, and review
text into agent prompts (`:582-610,666-675,740-815`), starts agents with
`--dangerously-skip-permissions` or `--allow-all-tools` (`:308-313`), stages
broadly, and can push or merge (`:617-652,836-906`). A malicious issue, source
comment, diff, or task file can therefore influence a credential-bearing agent
with write authority.

Run autonomous agents in a secret-free, network-restricted sandbox with a tool
allowlist. Stage explicit paths, treat repository text as untrusted data, and
require a human approval boundary before commit, push, and merge.

The script also claims that a successful commit means pre-commit checks passed,
but no tracked hook or configured `core.hooksPath` was found. Some documented
root Cargo commands fail because the manifest is under `figby-rs/`. Explicit
gates must run successfully before any push.

## Medium-severity findings

### F-21 — Medium — File and archive input limits are inconsistent

Plain fonts are read fully (`font.rs:473-475`) and can be amplified during lossy
conversion and line construction. TUI font open, palettes, templates, themes,
and configs also contain unrestricted `read`/`read_to_string` paths. The ZIP
entry payload cap is valuable, but the compressed archive can still be loaded
whole and `font.rs:526-549` retains and sorts every central-directory name
without entry-count or aggregate-name limits.

Adopt bounded readers based on `take(limit + 1)`, cap archive bytes, entry count,
expanded entry size, aggregate filename bytes, and parser work. Move archive
enumeration off the UI thread.

### F-22 — Medium — Predictable temporary saves permit clobbering and exports are non-atomic

`tui/file_ops.rs:1249-1259` writes a predictable `.stem.tmp` file with
`std::fs::write`, follows an existing symlink, and then renames it. Other export
paths write directly to the final destination, so a failure can leave a partial
or corrupt output.

Use a securely created temporary file in the destination directory, write and
flush it, optionally sync as appropriate, and atomically persist it. Refuse
symlink surprises and surface replacement semantics explicitly.

### F-23 — Medium — Parser work and configurable history can exhaust resources

Control-file numeric parsing uses unchecked `u32` multiply/add
(`control.rs:291-348`), accepts unlimited commands (`:459-570`), and scans the
command collection for every input character (`:575-591`). This permits debug
panics, release wrapping, memory exhaustion, or quadratic work.

Separately, arbitrary `usize` `undo_limit` values are loaded from config
(`config.rs:20-24,76-85`) and immediately passed to `Vec::with_capacity`
(`tui/undo.rs:17-24`). Timeline snapshots and particle emitters also lack a
shared byte/work budget.

Use checked numeric parsing, parser command/work caps, lazy bounded history,
finite-value validation, and an application-wide memory budget for snapshots,
particles, and exports.

### F-24 — Medium — Imported labels and filenames can inject terminal controls

Palette names, filesystem names, ZIP entry names, and typed paths are rendered
as terminal text without consistently escaping ESC/OSC/C1 controls, bidi
controls, or zero-width characters. Examples include
`tui/palette_editor.rs:350-378` and the file dialogs.

Create one display-sanitization routine that visibly escapes controls and bidi
markers, bounds displayed length, and is used for every untrusted label. Keep
the original value separately for filesystem operations.

### F-25 — Medium — Terminal cleanup is not protected by RAII

Terminal setup occurs before multiple fallible exits, while raw-mode and
alternate-screen cleanup is performed only on the normal tail of the event loop
(`tui/event_loop.rs:13-87`). The standalone player follows the same pattern.
A read/render error or panic can leave the user's terminal in raw mode.

Wrap terminal state in a guard whose `Drop` restores the screen, cursor, and raw
mode. Add a panic hook that restores state before delegating to the previous
hook.

### F-26 — Medium — Animation timing and transform behavior are incomplete

Several smaller defects compound the primary export blocker:

- signed keyframe offsets are clamped to zero during composition, preventing
  correct negative-position cropping;
- per-frame delays live outside `TimelineFrame`, so insert/delete/reorder can
  detach timing from content;
- ANSI animation export receives only the current frame in the production path;
- standalone playback discards variable frame timing;
- frame reorder can leave `current_frame` pointing at the wrong logical frame;
- preview ticking depends on rendering and can stall when redraw is suppressed.

Make timing part of frame identity, use a signed clipped compositor, and run
preview from a scheduler rather than the render function. Reuse the same frame
sequence for all animated formats.

### F-27 — Medium — Compatibility and release claims exceed verified behavior

The README claims full flag parity and bit-identical output
(`README.md:9-12,19,193-215`) and says `-F` lists fonts (`README.md:116`). The
binary explicitly reports `-F` as unimplemented and exits with failure
(`main.rs:254-255,1227-1230`). Seven C-parity tests are ignored. Running them in
the isolated audit copy found five real failures: paragraph mode, combined
`-kpc`, JIS0201, control-character skipping, and `-m191`. Two ignored paragraph
tests now pass and should be enabled.

Qualify parity claims until every supported behavior is expressed in an active
differential test. The C expected-output generator and Rust scenarios should be
driven by one authoritative table; the current generator covers a different
range than the tracked fixtures.

### F-28 — Medium — Release identity and distribution metadata are contradictory

Cargo metadata, the CI badge, clone links, releases, contribution links, and
issue links still point at `DoseOfGose`, while the reviewed origin is
`ClawfficeOrg/Figby`. The Cargo/changelog version is 6.0.31, an ancestor is
tagged 7.0.0, and current work is described as v8. The milestone index omits or
mislabels several completed and active versions.

`snapcraft.yaml` packages the legacy C `figlet` source and app name rather than
the Rust Figby application, despite presenting the result as stable.

Choose the canonical owner, product name, current version, and release line;
update all metadata from that source of truth. Quarantine or port the legacy
Snap definition before publishing it.

### F-29 — Medium — CI coverage does not match supported surfaces

The workflow covers only Ubuntu stable native fmt/Clippy/test. It has no package
verification, strict rustdoc, MSRV, Windows/macOS, WASM, audit/deny, differential
C baseline, or release dry-run gate. Actions use mutable major tags and the
workflow does not declare explicit least-privilege permissions.

Strict rustdoc currently fails on private-item links in
`gif_import.rs:165-168` and `tui/mod.rs:1-5`. A normal WASM check succeeds but
emits 27 warnings; strict WASM Clippy fails. No GitHub Actions run history was
returned for this repository at review time, so the workflow's real hosted
status could not be corroborated.

Recommended minimum matrix:

1. Linux fmt, locked Clippy, and locked tests from a clean checkout.
2. Windows and macOS compile plus focused path/terminal tests.
3. WASM check/Clippy with target-appropriate features.
4. Strict rustdoc, `cargo package`, advisory/license policy, and an MSRV job.
5. Release dry-run using the exact tag artifact and packaged assets.

### F-30 — Medium — Developer commands are wrong from the repository root

The repository root has no `Cargo.toml`, but parts of `AGENTS.md`, the README,
and automation recommend root-level `cargo build`, `cargo test -p figby`, or
`cargo fmt`. Those commands fail or target the wrong path.

Standardize on either a root workspace manifest or explicit
`--manifest-path figby-rs/Cargo.toml` commands. Use the same commands in docs,
local scripts, hooks, and hosted CI.

## Positive controls and strengths

The following are worth preserving while remediating the findings:

- Native `cargo build` and formatting checks pass on the reviewed commit.
- The normal `wasm32-unknown-unknown` check completes successfully.
- The codebase has extensive unit and integration coverage once the missing
  fixture allows targets to compile.
- The earlier template shell-command substitution has been removed.
- ZIP font entries have a 10 MiB expanded-entry cap.
- Main CLI/TUI image loaders already use decoder limits in several paths.
- ASE parsing uses checked arithmetic and a block-count cap.
- GIF code defensively checks frame-buffer indexing and has a scaled-cell cap;
  the remaining issue is what that cap measures and retains.
- Template paths reject obvious absolute and lexical traversal attempts.
- No project-authored `unsafe` block was identified in the reviewed source.
- The architecture already has useful domain modules for layers, timelines,
  export, parsing, templates, palettes, and platform front ends.

## Verification results

All Cargo commands used `CARGO_BUILD_JOBS=4` and `-j 4` where supported.

| Check | Result | Notes |
|---|---|---|
| `cargo fmt --manifest-path figby-rs/Cargo.toml -- --check` | Pass | No formatting drift |
| `cargo build --manifest-path figby-rs/Cargo.toml -j 4` | Pass | Native debug build |
| `cargo test ... -j 4 --no-fail-fast` | Compile failure | Missing ignored `Lixdu.ttf` |
| `cargo clippy ... --all-targets --all-features -- -D warnings` | Compile failure | Same missing fixture |
| Test in isolated copy with fixture substitution | Fail | One TUI smoke assertion after broad pass |
| `cargo check --target wasm32-unknown-unknown -j 4` | Pass with warnings | 27 target-specific warnings |
| strict WASM Clippy | Fail | Warnings promoted to errors |
| strict rustdoc | Fail | Four private-link errors |
| `cargo package --allow-dirty --offline` | Verification failure | Required assets outside crate |
| `cargo audit --no-fetch` | Warnings | One unsound and two unmaintained advisories |

Because `Cargo.lock` is ignored, the dependency result above is a snapshot of
the audit resolution, not a reproducible property of the reviewed commit.

## Recommended remediation order

### Phase 0 — Restore a trustworthy gate

1. Track the missing test fixture and repair the TUI smoke expectation.
2. Commit `Cargo.lock`, use `--locked`, and pin a supported Rust toolchain/MSRV.
3. Make fmt, Clippy, tests, rustdoc, WASM check, package verification, and audit
   explicit pre-push CI gates.
4. Run the gates in a clean checkout with no ignored fixtures.

### Phase 1 — Correct core document behavior

1. Introduce a single `Document` aggregate owning layers, timeline, selection,
   dirty revision, undo history, and save capability.
2. Select one timeline state model and one compositor/capture pipeline.
3. Make rendering read-only and make every model mutation revisioned.
4. Repair save/open/new/quit transitions and layer-aware undo/locking.
5. Add end-to-end export tests that decode and compare generated frames.

### Phase 2 — Enforce resource and trust boundaries

1. Add shared input-byte, decoded-pixel, output-cell, frame-count, and output-byte
   budgets used by CLI, library, TUI, template, GIF, WASM, and exporters.
2. Stream GIF import and image/animation export instead of retaining duplicate
   full-frame buffers.
3. Harden template path containment and disable ambient environment expansion.
4. Replace recursive and unchecked parsers with bounded state machines.
5. Sanitize terminal display strings and use secure atomic file replacement.

### Phase 3 — Make distribution claims verifiable

1. Move assets into the crate package and verify install/run from the `.crate`.
2. Restore active differential C-compatibility coverage before claiming parity.
3. Align owner URLs, versions, tags, changelog, milestones, and package metadata.
4. Add platform, WASM, MSRV, package, and release-artifact jobs.
5. Restrict autonomous agent authority and require human approval for writes.

## Acceptance criteria for the next release candidate

A release candidate should be cut only when:

- a fresh clone passes every required gate with `--locked`;
- `cargo package` verifies and an installed package starts without repository
  files present;
- a two-or-more-frame layered project round-trips through navigation, undo,
  save/reopen, GIF, APNG, and ANSI export without pixel or timing drift;
- dirty-state tests cover edits made during an asynchronous save;
- fuzz/property tests cover Unicode palettes and WASM editing, long control/HZ
  sequences, malformed fonts/templates/archives, and checked dimensions;
- bounded tests demonstrate rejection before large allocations;
- README compatibility, installation, version, and distribution statements are
  generated from or checked against executable tests and release metadata.

## Final assessment

Figby is a promising and ambitious application with enough existing structure
to support a focused hardening cycle. Its primary problem is not a lack of code
or tests; it is that several separate representations of the document, frame,
dirty state, and packaged application have drifted apart. Consolidating those
authorities will eliminate multiple correctness findings at once and make the
remaining safety work much easier to enforce.

The fastest safe path is: restore clean-checkout gates, fix the packaged asset
boundary, unify document/timeline/export state, then apply one shared resource
budget across every untrusted-input and output path. Until those steps are
complete, the project should be treated as pre-release despite the existing
version and compatibility claims.
