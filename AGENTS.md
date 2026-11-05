# AGENTS.md

Guidance for AI agents working in the Figby repository.

## Project Overview

Figby is a Rust port of FIGlet (Frank, Ian & Glenn's Letters) — the classic
ASCII art banner generator. The original C implementation (v2.2.5) renders text
in large characters using FIGfont (.flf) and TOIlet (.tlf) font files with
kerning, smushing, and multi-byte character support.

This repo preserves the original C source in `c-figlet/` while the Rust port lives
in `figby-rs/`. The goal is a **feature-complete, safe, modern Rust rewrite**
that supports all FIGlet 2.2.5 features plus UTF-8 natively.

## Development Commands

All commands run from the repo root. The crate lives in `figby-rs/`, so every
`cargo` invocation passes `--manifest-path figby-rs/Cargo.toml` (the repo root
has no `Cargo.toml` — a bare `cargo build` fails or targets the wrong path).

```bash
# Build Rust crate
cargo build --manifest-path figby-rs/Cargo.toml

# Run tests
cargo test --manifest-path figby-rs/Cargo.toml

# Check formatting
cargo fmt --manifest-path figby-rs/Cargo.toml --check

# Run clippy
cargo clippy --manifest-path figby-rs/Cargo.toml --all-targets --all-features -- -D warnings

# Test with fonts
cargo run --manifest-path figby-rs/Cargo.toml -- -f fonts/standard "Hello"

# Run figby from repo root (binary is in figby-rs/target)
../figby-rs/target/debug/figby < tests/input.txt

# Compare against system figlet (built with TLF support)
diff <(figlet < tests/input.txt) <(figby-rs/target/debug/figby < tests/input.txt)

# Single input line test
printf "unexpected token \`}'" | figby-rs/target/debug/figby
```

### Ralph Loop

```bash
# Run autonomous task loop (multi-phase from main)
./scripts/ralph.sh

# Run single task
./scripts/ralph.sh 1.1.1

# Dry run to preview
./scripts/ralph.sh --dry-run

# Time-bounded run
./scripts/ralph.sh --minutes=30

# Run until specific task
./scripts/ralph.sh --until=1.1.3

# Log session to file
./scripts/ralph.sh --log=/tmp/figby-ralph.log

# Quiet mode (suppress agent stderr)
./scripts/ralph.sh --quiet
```

Ralph supports multi-phase loops, per-task agent selection via `Agent:`/`Difficulty:` fields in task blocks, phase completion review with auto-fix cycles, and major release (X.0) gates with RC branch creation. Stop gracefully: `kill -TERM $(cat /tmp/ralph.pid)` or `touch scripts/STOP.md`.

## Key Conventions

### Task Workflow

- Tasks defined in `docs/todo-v*.md` with format `- [ ] `X.Y.Z` description`
- Each task maps to branch `task-X.Y.Z` off `release/X.Y`
- Phase completion triggers review before merge to main
- Major versions (X.0) require human sign-off

### Post-Task Checklist

After completing any implementation work, do ALL of the following (or ask the user to confirm, unless in auto mode):

1. **Summarize** what changed — files touched, behavior added/fixed/removed.
2. **Verify** — run before committing (fail = stop, report, fix first):
   ```bash
   cargo build --manifest-path figby-rs/Cargo.toml
   cargo test --manifest-path figby-rs/Cargo.toml
   cargo clippy --manifest-path figby-rs/Cargo.toml --all-targets -- -D warnings
   cargo fmt --manifest-path figby-rs/Cargo.toml --check
   ```
   If `cargo fmt --check` fails, run `cargo fmt --manifest-path figby-rs/Cargo.toml` and re-verify before committing.
3. **Commit** — write a proper conventional-commit message (`feat:`, `fix:`, `refactor:`, etc.) with a concise subject and body that explains *why*, not just *what*. Co-author line required.
4. **Version bump** — increment the patch version (or minor/major if scope warrants) in:
   - `figby-rs/Cargo.toml` (`version = "X.Y.Z"`)
   - `README.md` (if it mentions a version)
   - Any `--version` / `--help` output strings in `main.rs`
5. **Changelog** — prepend an entry to `CHANGELOG.md` (create it if absent) in Keep a Changelog format:
   ```
   ## [X.Y.Z] - YYYY-MM-DD
   ### Added / Changed / Fixed / Removed
   - One-line description per change
   ```
6. **Check off task** in `docs/todo-v*.md`.
7. Add entries to `docs/memory.md` and `docs/learnings.md` if there's anything non-obvious to preserve.

**Auto mode:** do steps 1–7 without asking. **Interactive mode:** summarize + verify first, then ask "Commit, bump version, and update changelog?" before proceeding.

### Memory Updates

After completing any implementation work:
1. Add entries to `docs/memory.md`
2. Add insights to `docs/learnings.md`
3. Check off the task in `docs/todo-v*.md`

### Architectural Invariants

1. No `unwrap()` in production paths — use proper error handling
2. FIGfont spec compliance is non-negotiable — test against original C output
3. Support all FIGlet 2.2.5 command-line flags
4. UTF-8 is the native encoding — no wchar_t hacks

## File Structure

Checked against the tree on 2026-11-04. No line counts: they went stale.
When you add or remove a module, update this map in the same commit.

```
figby-rs/                 # Rust workspace: the figby crate (lib + binary) and figseq
├── Cargo.toml            # Package version lives here (see Post-Task Checklist)
├── figseq/               # Standalone crate: .figseq format, Player, ratatui Banner widget
├── benches/              # criterion benches (render_bench.rs)
├── tests/                # Integration tests; tests/cparity/scenarios.tsv drives C parity
└── src/
    ├── main.rs           # CLI (FIGlet flags with C getopt semantics, image/font/play/bake modes) + TUI launch
    ├── lib.rs            # Crate root (re-exports)
    ├── font.rs           # FIGfont/TLF parser (lenient like C), ZIP fonts, Latin-1 fallback
    ├── font_gen.rs       # TTF/OTF → FIGfont conversion
    ├── render.rs         # FIGlet engine: OutputLine, add_char/split_line, kerning, smushing
    ├── smush.rs          # Smushing rules
    ├── control.rs        # Control files (.flc), ISO 2022 / multibyte input decoding
    ├── input.rs          # Multibyte input readers (UTF-8, HZ, DBCS) and -D Deutsch mapping
    ├── output.rs         # Canvas export (TXT/ANSI/PNG/GIF/APNG), procedural block/braille glyphs
    ├── config.rs         # ~/.config/figby/config.toml
    ├── atomic_io.rs      # Crash- and symlink-safe file replacement (every user-facing save)
    ├── bounded_io.rs     # Size-capped reads
    ├── sanitize.rs       # Display sanitization for untrusted strings
    ├── image_input.rs    # Image → ASCII luminance/RGB matrix
    ├── gif_import.rs     # Animated GIF import
    ├── palette_import.rs # Palette import (Paletty/ASE/WezTerm)
    ├── figmap.rs         # .figmap project format (layers, timeline, lights)
    ├── figseq.rs         # Re-exports the figseq crate + bake (figmap → figseq, native only)
    ├── template.rs       # .ftmp template engine
    ├── web.rs            # WASM bindings
    └── tui/              # TUI application (ratatui)
        ├── mod.rs        # Re-exports, render pipeline, shared helpers
        ├── app_state.rs  # TuiApp, EditorState/AnimationState/LightingState, IconSet, new()
        ├── event_loop.rs # run(), handle_event(), tick, async completion
        ├── dispatch.rs   # handle_key_event/handle_mouse_event, perform_* actions (largest file)
        ├── documents.rs  # Multi-document tabs (park/unpark live state)
        ├── overlays.rs   # Floating dialogs, quit prompt, keybindings, undo panel
        ├── canvas.rs     # ASCII canvas buffer
        ├── layers.rs     # Layer stack, compositing, layer panel
        ├── brush.rs      # Brush state and shapes
        ├── palette.rs, palette_editor.rs
        ├── toolbox.rs    # Tool list widget
        ├── font_editor.rs, image_editor.rs
        ├── file_ops.rs   # Open/Save dialogs, recent files
        ├── layout.rs, theme.rs, keymap.rs, events.rs, menu.rs
        ├── welcome.rs, fx.rs        # Welcome screen and its tachyonfx fade
        ├── lighting.rs, light_panel.rs
        ├── timeline.rs, player.rs, particles.rs
        ├── side_panel.rs, props_panel.rs, status.rs (canvas settings), undo.rs, undo_panel.rs
        ├── render_mode.rs, throbber.rs, export.rs
        ├── component.rs  # Empty placeholder (Component trait was removed)
        ├── dialogs/      # new_image, gif_import, rascii_import, system_font, font_import_options
        ├── tools/        # brush, eraser, eyedropper, fill, line, move_tool, rotate_tool,
        │                 #   selection, spray, text
        └── components/   # canvas.rs, status_bar.rs
c-figlet/                 # Reference C FIGlet 2.2.5 (parity oracle)
fonts/                    # Bundled .flf/.tlf fonts
assets/                   # Templates, demo images/video (figby-rs/assets/ holds the compiled-in icons, themes)
tests/                    # Upstream-style resNNN.txt fixtures, input.txt
scripts/
├── cparity-diff.sh       # Builds C FIGlet, diffs ~460 cases against figby (CI gate)
├── regenerate-expected.sh# Regenerates tests/resNNN.txt from C FIGlet
├── ralph.sh              # Autonomous task loop (bash)
└── ralph-monitor.sh      # Rate-limit monitor cron
docs/                     # See docs/README.md for the index
├── todo.md               # Master todo index (milestones)
├── memory.md, learnings.md, ralph-log.md   # Agent memory and logs (paths used by ralph.sh)
└── full-review-2026-11-04.md               # Latest full review and fix status
```
