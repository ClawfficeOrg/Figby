# Figby — Claude Code Context

See [README.md](README.md) for full project overview, CLI usage, feature matrix, and build instructions.

## Source layout

The full, current file map is in [AGENTS.md § File Structure](AGENTS.md#file-structure);
it is the only copy, so update it there. The short version:

- `figby-rs/src/` — the `figby` crate: FIGlet engine (`render.rs`, `smush.rs`,
  `font.rs`, `control.rs`, `input.rs`), CLI (`main.rs`), formats (`figmap.rs`,
  `figseq.rs`, `template.rs`), and the TUI under `tui/` (`app_state.rs`,
  `dispatch/`, `event_loop.rs`, `documents.rs`, widgets, `tools/`, `dialogs/`)
- `figby-rs/figseq/` — standalone `.figseq` crate
- `c-figlet/` — reference C FIGlet 2.2.5; `scripts/cparity-diff.sh` diffs against it
- `docs/` — see `docs/README.md`; milestones in `docs/todo.md`
- `fonts/` — bundled fonts; `assets/` — templates, demo images and video
- `figby-rs/assets/` — compiled-in assets: icons (`icons.yaml`, `icons-ascii.yaml`), themes, images

## Build & run

```bash
cargo build --manifest-path figby-rs/Cargo.toml
cargo run --manifest-path figby-rs/Cargo.toml -- --tui
```

## Current milestone

v7.1 (Dialog, Keybindings & Props Polish) and v8 (multi-document, figmap,
backlog) are active; v6 is complete. `docs/todo.md` is the source of truth.
The package version (`figby-rs/Cargo.toml`, 6.2.x) is separate from milestone
numbers.

## After every task

1. Summarize what changed.
2. Run `cargo build`, `cargo test`, `cargo clippy` (fix failures before committing), `cargo fmt` (auto-fix then re-check) — all with `--manifest-path figby-rs/Cargo.toml` (the repo root has no `Cargo.toml`; see `AGENTS.md`).
3. Ask (interactive) or auto-do (auto mode): conventional-commit message, bump `figby-rs/Cargo.toml` version + README + CHANGELOG.md (create if absent).

See `AGENTS.md` §Post-Task Checklist for full procedure.
