# Remediation Checklist — GPT Review 2026-08-20 + Independent Verification

Verified on `review/gpt-2026-08-20` (= master @ `53d39b9` + review doc).
Every finding marked **[verified]** was reproduced by direct code inspection
and/or execution on this machine. Findings marked **[accepted]** were confirmed
by code reading of the cited locations but not executed.

Independent verification results:

- `cargo clippy --all-targets --all-features -- -D warnings` → **compile failure** (F-01 confirmed)
- Full test suite with fixture substituted → **4 failures**, not 1 as review claimed:
  - `test_tui_smoke_all_panels_render` (FPS assertion, tui.rs:49) — real, matches F-02
  - `tui::player::tests::test_terminal_session_capture` / `..._fallback_blank` —
    `WouldBlock` unwrap panics in player.rs:1107/1121, env-dependent (no pty), **missed by GPT**
  - `font::tests::test_list_zip_font_entries_rejects_path_separators` — passes alone,
    fails under full-suite parallel run → **flaky/shared state, missed by GPT**

## Phase 0 — Restore trustworthy gate (do first)

- [x] **F-01 [verified]** Blocker: add tracked font fixture under `figby-rs/tests/fixtures/`,
      replace `include_bytes!("../../assets/fonts/to-convert/Lixdu.ttf")`
      (font_gen.rs:1119,1143); delete `.gitignore` rule if no longer needed.
      CI must run gates from clean checkout.
- [x] **F-02 [verified]** Fix `test_tui_smoke_all_panels_render`: status bar does not render
      `FPS:` label. Decide: restore label or update assertion. Keep test required.
- [x] **NEW [verified]** Fix player capture tests unwrapping pty results
      (player.rs:1107,1121) — skip gracefully or handle `WouldBlock`.
- [x] **NEW [verified]** Investigate flaky `test_list_zip_font_entries_rejects_path_separators`
      (fails only in parallel full-suite run) — likely shared cwd/temp state.
- [x] **F-19 [verified]** Commit `Cargo.lock` (remove from .gitignore), use `--locked`
      in CI, pin MSRV via `rust-toolchain.toml`. Audit: patch/bump `lru`
      (RUSTSEC-2026-0253 via ratzilla chain), plan replacements for
      `ansi_term`, `paste`.
- [x] **F-03 [verified]** Move all `include_str!`/`include_bytes!` assets inside crate:
      theme.rs:4, welcome.rs:13, app_state.rs:20, web.rs:136-139 (+ LICENSE).
      Add `cargo package --allow-dirty && cargo publish --dry-run` CI gate.

## Phase 1 — Core document correctness

- [x] **F-04 [verified]** Blocker: production GIF/APNG export (dispatch.rs:~2389) builds frames
      from live layer_stack, bypassing `capture_timeline_frames` (export.rs:663)
      which correctly prefers `layer_state`. Route production through one compositor;
      add two-frame distinct-pixel round-trip test.
- [x] **F-05** Pick single timeline frame authority (snapshot vs keyframes). Fix navigation
      commit/load flattening into wrong layer (app_state.rs:869-878, 903-928,
      dispatch.rs:2618-2635). Multilayer docs corruptable by timeline nav.
- [x] **F-06** Make render read-only: sync_image_to_canvas on every redraw can overwrite paint
      ops with stale ImageEditor cache (mod.rs:480-486, app_state.rs:219-235).
      Regression test: paint → redraw → paint survives.
- [x] **F-07 [verified]** Revision-aware saves: `AsyncResult::SaveComplete`
      (event_loop.rs:138-147) unconditionally clears unsaved + may quit even if edits
      happened during async save. Add document revision counter.
- [x] **F-08** Uniform dirty tracking: font_editor mutations don't set unsaved flag;
      Open/New lack save/discard guard and partially reset state; image-mode
      "Save and Quit" early-returns unless Font Editor mode.
- [x] **F-09 [verified]** Undo entries store bare CanvasBuffer applied to whichever layer is
      active at undo time → cross-layer corruption. Carry target layer id +
      structural inverse ops. Enforce layer locks via one mutation API.

## Phase 2 — Resource & trust boundaries

- [x] **F-10** Template image containment: pass base dir in RenderConfig, canonicalize +
      re-check containment immediately before open (template.rs:583-598).
- [x] **F-11 [verified]** Disable/allowlist `${VAR}` env expansion (template.rs:156);
      bound template image dimensions; route rascii decoding through limited reader.
      Add test proving `$(...)` stays literal.
- [x] **F-12 [verified]** Canvas guard bypass: width=0 × height=u32::MAX passes
      `saturating_mul` check then allocates billions of rows (template.rs:505-511);
      margin/padding outside budget. Same for new_image/gif_import dialogs (65,535 cap).
      Use checked arithmetic incl. padding; reject zero dims; consider flat buffer.
      (template.rs fixed in 8b4e571; dialogs now share `canvas::MAX_CANVAS_CELLS`.)
- [x] **F-13 [verified]** GIF import retains all native-resolution frames in `raw_frames`
      (gif_import.rs:213-216); scaled-cell budget doesn't count native bytes. Stream
      decode; cap native pixels/cumulative bytes independently.
- [x] **F-14** Export pixel budgets: font-size-as-scale × canvas can exceed GBs
      (output.rs:240-305). Checked pre-allocation budget; stream scanlines to encoder.
- [x] **F-15 [verified]** WASM editor cursor is byte index advanced by 1 per char
      (web.rs:39-58) → panic on multibyte edit. Switch to char/grapheme index.
      Browser tests: é, CJK, emoji, combining marks. (Fixed in 2a55c2f; verified:
      `WebApp.cursor_pos` is a char index with `test_multibyte_editing_no_panic`.)
- [x] **F-16 [verified]** Replace recursion with loops: control.rs iso2022 (recursive
      re-entry per control byte) and input.rs HZ skip. Long adversarial-run tests.
- [x] **F-17 [verified]** normalize_hex slices at byte offsets without ASCII check
      (palette_import.rs:126-135) → `"éx"` panics. Validate ASCII+hex digits, return
      structured error. Sweep shadow-color/palette-editor for same pattern.
      Property tests per palette format. (ASCII check landed in 46ca98b; added
      proptest fuzz for every palette format + fixed the rascii_import `&s[..n]`
      char-boundary truncation.)
- [x] **F-18** Shared decoder limits + checked output budgets across main.rs CLI widths,
      image_input.rs resize/public API paths (unrestricted `image::open`).
- [x] **F-21** Bounded readers everywhere (`take(limit+1)`); ZIP central-directory entry-count
      + aggregate-name caps (font.rs:526-549); move enumeration off UI thread.
- [x] **F-22** Secure atomic writes: predictable `.stem.tmp` symlink-following write
      (file_ops.rs:1249-1259); all exports write direct-to-destination. Tempfile in dest
      dir + atomic rename.
- [x] **F-23** Checked numeric parsing in control files (control.rs:291-348); command caps;
      validate config `undo_limit` before `Vec::with_capacity`; app-wide memory budget.
- [x] **F-24** One display-sanitization routine (ESC/OSC/C1/bidi/zero-width) for palette names,
      filenames, ZIP entries, typed paths.
- [x] **F-25** RAII terminal guard + panic hook restoring raw mode/alt-screen
      (event_loop.rs:13-87, player).

## Phase 3 — Distribution & claims

- [x] **F-26** Animation fixes: signed keyframe offsets, per-frame delay inside TimelineFrame,
      ANSI anim gets all frames, variable timing in playback, scheduler-driven preview.
- [x] **F-27** Qualify README parity claims; `-F` listed in README but unimplemented in binary
      (main.rs). Enable 2 passing ignored paragraph tests; drive C generator + Rust
      scenarios from one table; triage 5 failing ignored C-parity tests.
- [x] **F-28** Align owner URLs (DoseOfGose → ClawfficeOrg), version identity (6.0.31 vs tag
      7.0.0 vs v8 work), milestone index; quarantine/port snapcraft.yaml (packages C figlet).
- [x] **F-29** CI matrix: Windows/macOS, WASM check+clippy, strict rustdoc (fix private-link
      errors gif_import.rs:165, tui/mod.rs:1), MSRV, cargo-deny/audit, package verify,
      release dry-run, pinned action tags, least-privilege permissions.
- [x] **F-30** Standardize root commands: root workspace manifest or explicit
      `--manifest-path figby-rs/Cargo.toml` everywhere (AGENTS.md, README, scripts).
## Suggested task numbering

Map to `task-X.Y.Z` branches off next release line. Recommended split:
Phase 0 → 9.0.x (release-gate restoration), Phase 1 → 9.1.x (document model),
Phase 2 → 9.2.x (boundaries), Phase 3 → 9.3.x (distribution).
