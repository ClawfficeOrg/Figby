# Figby — Claude Code Context

See [README.md](README.md) for full project overview, CLI usage, feature matrix, and build instructions.

## Source layout

- `figby-rs/src/` — Rust source
  - `main.rs` — CLI entry point + TUI launch
  - `lib.rs` — Crate root (re-exports public API)
  - `font.rs` — FIGfont/TLF parser + ZIP font support
  - `font_gen.rs` — TTF/OTF → FIGfont conversion
  - `render.rs` — FIGlet render engine (kerning, smushing)
  - `smush.rs` — Smushing rules engine
  - `control.rs` — FIGlet control file parser (.flc)
  - `image_input.rs` — Image → ASCII luminance/RGB matrix
  - `gif_import.rs` — Animated GIF import
  - `palette_import.rs` — Palette import (Paletty/ASE/WezTerm)
  - `template.rs` — .ftmp template engine
  - `tui/mod.rs` — TUI app state, event loop, render
  - `tui/canvas.rs` — ASCII canvas buffer
  - `tui/layers.rs` — Layer stack + compositing
  - `tui/palette.rs` — Color palette widget
  - `tui/toolbox.rs` — Tool selection widget
  - `tui/font_editor.rs` — FIGfont glyph editor
  - `tui/file_ops.rs` — File open/save dialogs + recent files
  - `tui/layout.rs` — Layout computation
  - `tui/theme.rs` — Color theme system
  - `tui/welcome.rs` — Welcome screen
  - `tui/components/` — Reusable widgets (`canvas.rs`, `status_bar.rs`)
  - `tui/tools/` — Tool implementations (brush, fill, selection, etc.)
- `docs/` — Task lists (`todo-v6.md` is current), audit, learnings
- `fonts/` — Bundled `.flf` fonts
- `assets/` — Icons, images, template examples

## Build & run

```bash
cargo build --manifest-path figby-rs/Cargo.toml
cargo run --manifest-path figby-rs/Cargo.toml -- --tui
```

## Current milestone

v6 — Pre-Release Hardening & Polish. See `docs/todo-v6.md`.

## After every task

1. Summarize what changed.
2. Run `cargo build`, `cargo test`, `cargo clippy` (fix failures before committing), `cargo fmt` (auto-fix then re-check) — all with `--manifest-path figby-rs/Cargo.toml` (the repo root has no `Cargo.toml`; see `AGENTS.md`).
3. Ask (interactive) or auto-do (auto mode): conventional-commit message, bump `figby-rs/Cargo.toml` version + README + CHANGELOG.md (create if absent).

See `AGENTS.md` §Post-Task Checklist for full procedure.
