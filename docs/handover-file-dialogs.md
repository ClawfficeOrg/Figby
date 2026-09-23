# Handover — File Dialog UX + Animation/Open Follow-ups

Date: 2026-09-23. Branch: `hardening/gpt-review` (pushed, ahead of `master`;
`master` undrifted → still fast-forwardable). Snapshots:
`snapshot/hardening-gpt-review-2026-09-22`,
`snapshot/hardening-gpt-review-2026-09-22b` (covers through v6.0.41;
post-dates the two fix commits below — re-snapshot after next push).

## Session context

Two user complaints were investigated and fixed (commit `e6cf6fb`, pushed):

1. **Open replaced the current document** — File>Open, image open, figmap
   open, New Image, and welcome New now all land in a **fresh tab**
   (`TuiApp::new_document`). The unsaved-changes prompt on open is gone
   (nothing is destroyed). Async font opens record `pending_open_tab`
   for the OpenComplete handler, with fresh-tab fallback. The
   `PendingDocAction::OpenFont` flow was removed as unreachable.
2. **Animation model confusion** — captured frames are now plain snapshots
   (`has_keyframe: false`, `layer_keyframes: all None`); keyframes are
   opt-in via the tween editor only. Fixed en route: `commit-on-navigate`
   minting keyframe flags, GIF import mixing `has_keyframe: false` with
   `Some()` markers, first capture auto-shows the timeline.
   - Trap for future spelunkers: `AnimationState::handle_key` is called
     from `dispatch.rs:1674` ("Animation state dispatch") — the call spans
     multiple lines, so line-based grep for `animation.handle_key` misses
     it. It is NOT dead code.

Verify baseline before starting (all green at handover):
```bash
cargo build --manifest-path figby-rs/Cargo.toml
cargo test --manifest-path figby-rs/Cargo.toml
cargo clippy --manifest-path figby-rs/Cargo.toml --all-targets -- -D warnings
cargo fmt --manifest-path figby-rs/Cargo.toml --check
```

## File dialog defect list (static review, `figby-rs/src/tui/file_ops.rs`)

None of this has been fixed yet — the user was about to add their own
notes. Live tmux driving from the dev sandbox is broken (fresh panes
never emit output, even `/bin/sh` + `cat` — infra issue, not figby), so
verify visually with computer control (`tmux send-keys` + `capture-pane`;
the committed `scripts/e2e-tmux-test.sh` is a starter harness).

1. **Typing fights the path buffer** (`handle_key_browse` Char arm ~607;
   `refresh_directory:299` resets `selected_entry = 0` per keystroke).
   Every printable char appends to `path_buffer` and re-lists, so
   type-to-find and the path field are one buffer fighting itself, and
   the highlight jumps to `..` on each keystroke. Consider: separate
   filter buffer, or only treat typing as path input with an explicit
   focus/mode.
2. **Highlight vs Enter disagree** (`handle_key_browse` Enter arm ~670).
   A typed file path wins over the highlighted entry with no visual
   indication of precedence. Type `x`, arrow to a valid file, Enter —
   which one opens depends on hidden priority rules.
3. **Digits untypeable in Open mode** (recent-shortcut arm ~612). `1-9`
   jump to recents, so paths containing digits can't be typed. Other
   modes type them normally — inconsistent.
4. **Recent list lies past 9 entries** (render ~982 vs handler ~619).
   Labels show continuing numbers (10+) for the last-9 slice, but keys
   `1-9` index the first nine of the full list. Cap display at 9 or fix
   numbering.
5. **SaveAs Enter saves instantly, no overwrite confirm**
   (`handle_key_save_as` Enter; `perform_save` in `dispatch.rs`). Stray
   Enter on the default `untitled.flf` silently replaces it. Add
   exists-check + confirm.
6. **SaveAs highlight is decorative.** Up/Down move `selected_entry` but
   Enter finalizes from the buffers regardless; Right on a file loads
   the filename (then Enter overwrites it — see 5).
7. **Stale chrome in figmap flows.** Title `" Open Font "` (:898) also
   serves figmap opens; `" Save Font As "` (:1192) and
   `"(no .flf/.tlf files…)"` (:1244) show when saving `.figmap`.
   Titles/empty-text should follow the mode/extension.
8. **SaveAs can browse into zips** (`allowed()` lists `.zip` for SaveAs).
   Saving into an archive is broken downstream — disallow or implement.
9. **Mouse gaps.** Recent rows are keyboard-only (hit rects cover
   directory entries only), and single-click opens immediately
   (deliberate in 8.1.3, but users expect select-then-confirm).

## Suggested order

5 (data loss) → 1+2 (core wonkiness) → 4, 7 (small, mechanical) →
6, 8, 9 (judgment calls — confirm with user). Re-verify with the
four commands above per change; bump `figby-rs/Cargo.toml` +
`CHANGELOG.md` per repo convention (`AGENTS.md` checklist).

## Open questions for the user

- Their own dialog notes (pending at handover).
- Merge `hardening/gpt-review` → `master`: code-ready, awaiting
  human TUI QA + merge style (fast-forward vs merge commit).
