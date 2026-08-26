# Figby

[![CI](https://github.com/ClawfficeOrg/figby/actions/workflows/ci.yml/badge.svg)](https://github.com/ClawfficeOrg/figby/actions)
[![Crates.io](https://img.shields.io/crates/v/figby)](https://crates.io/crates/figby)
[![License](https://img.shields.io/badge/license-BSD--3--Clause-blue.svg)](LICENSE)

> Rust port of FIGlet 2.2.5 — ASCII art banner generator

Figby renders text as large ASCII art characters using FIGfont (`.flf`) and
TOIlet (`.tlf`) font files. It is a **safe, modern Rust rewrite** of FIGlet
2.2.5 preserving all features: kerning, smushing, multi-byte character support,
control files, and the full CLI interface.

Original C source lives in `c-figlet/` for reference; the Rust port lives in
`figby-rs/`.

## Features

- Full FIGlet 2.2.5 CLI flag coverage (27 flags; `-F` is removed, matching C
  FIGlet — see [Known divergences](#known-divergences-from-c-figlet))
- FIGfont (`.flf`) and TOIlet (`.tlf`) font support
- Kerning and smushing (all 11 rules: H1–H6, V1–V5)
- Multi-byte input: UTF-8, DBCS, Shift-JIS, HZ
- Control files (`.flc`) with translate, freeze, ISO 2022 charset handling
- Deutsch flag character re-routing (`-D`/`-E`)
- Compressed font support (ZIP/deflate)
- UTF-8 native — no `wchar_t` hacks
- Full-screen TUI editor (`--tui`): drawing tools, layers (grouping,
  linking, mouse toolbar), palette/font editing, image import, and an
  **animation timeline** — keyframing, tweening (linear/ease-in/ease-out/
  bounce), onion skinning, animated GIF import with real per-frame timing,
  a clickable play/pause/stop/loop transport bar, and GIF/APNG/ANSI
  export.
- `--play <file.gif>`: play an animated GIF fullscreen in the terminal, then
  exit — no TUI required. Scales to fit the terminal by default (or to
  `--play-width <N>` columns), so GIFs larger than the terminal — or larger
  than would otherwise fit the animation import size cap — still play. Honors
  the GIF's real per-frame timing. Add `--loop` to repeat until any key is
  pressed instead of playing once. See
  [docs/sonnet5-review.md](docs/sonnet5-review.md) for current known
  limitations of the animation subsystem.

## Installation

### From source (cargo)

```bash
cargo install --path figby-rs
```

### Build from git

```bash
git clone https://github.com/ClawfficeOrg/figby.git
cd figby
cargo build --manifest-path figby-rs/Cargo.toml --release
```

The binary is at `figby-rs/target/release/figby`.

### Pre-built binaries

Not yet available. Track progress via [GitHub Releases](https://github.com/ClawfficeOrg/figby/releases).

### System package managers

Not yet packaged. Package manager contributions welcome.

## Quick Start

```bash
# Run from repo root with bundled fonts
cargo run --manifest-path figby-rs/Cargo.toml -- -d fonts -f standard "Hello, world!"

# After installing: pipe input
figby -d fonts < tests/input.txt

# Pick a different font
figby -d fonts -f banner "Figby"
```

Default font directory is `/usr/share/figlet`. Use `-d <path>` or
`FIGLET_FONTDIR` to override without system install.

## CLI Usage

```
figby [OPTIONS] [MESSAGE]
```

| Flag | Description |
|------|-------------|
| `-f <font>` | Font name (without `.flf`/`.tlf` suffix) |
| `-d <dir>` | Font directory path |
| `-k` | Kerning (default for most fonts) |
| `-s` | Smushing |
| `-S` | Force smushing |
| `-o` | No smushing (overlap only) |
| `-W` | No width handling (no smushing, no kerning) |
| `-m <mode>` | Smush mode bitmask (0–255) |
| `-c` | Center justification |
| `-l` | Left justification |
| `-r` | Right justification |
| `-x` | Default justification |
| `-L` | Left-to-right text direction |
| `-R` | Right-to-left text direction |
| `-X` | Default text direction |
| `-t` | Use terminal width for output |
| `-w <width>` | Output width (default 80) |
| `-I <code>` | Display info (0=usage, 1=version, 2=fontdir, 3=font, 4=width, 5=formats) |
| `-D` | Deutsch mode (map `[\]` to Ä/Ö/Ü, `{|}~` to ä/ö/ü/ß) |
| `-E` | Disable Deutsch mode |
| `-C <file>` | Control file (`.flc`) for character mapping |
| `-p` | Paragraph mode |
| `-n` | No paragraph mode |
| `-A` | Read input from stdin (positional args also work) |
| `-F` | Removed (matches C FIGlet 2.2.5) — list fonts with `figlist` |
| `-h` | Print help |
| `-V` | Print version |
| `--tui` | Launch the full-screen TUI editor (drawing, layers, animation timeline) |
| `--play <file.gif>` | Play an animated GIF fullscreen in the terminal, then exit |
| `--play-width <N>` | Scale playback to N columns [default: fit to terminal] |
| `--loop` | With `--play`: repeat until any key is pressed, instead of playing once |

### Examples

```bash
# Basic usage
figby -f standard "Hello"

# Smushing with a smush-friendly font
figby -f smush -s "FIGBY"

# Center-justified output at terminal width
figby -f standard -c -t "Welcome to Figby"

# Right-to-left text
figby -f standard -R "reverse"

# Pipe input
echo "Hello from pipe" | figby

# Deutsch mode
figby -D -f standard "\]

# Load font from custom directory
figby -d ~/my-fonts -f custom "Figby"

# Show version info
figby -I 1
```

## Font Directory Setup

Figby looks for fonts in the following order:

1. **`-d <path>`** flag — overrides the default search directory
2. **`FIGLET_FONTDIR`** environment variable
3. **`/usr/share/figlet`** — default system-wide path

When loading a font named `standard`, Figby searches for:
- `<fontdir>/standard.flf`
- `<fontdir>/standard.tlf`
- `./standard.flf`
- `./standard.tlf`

Each candidate is checked for ZIP magic bytes; compressed fonts are
decompressed automatically.

## Getting Fonts

Bundled fonts are in the `fonts/` directory at the repo root. Hundreds more
are available from the FIGlet font archive:

- [FIGlet font collection](http://www.figlet.org/fontdb.cgi) (official)
- [Patriotic fonts](http://patorjk.com/figlet/fonts/) (patched for modern FIGlet)
- [FIGlet font archive](https://github.com/xero/figlet-fonts) (community)

To install fonts system-wide:

```bash
sudo mkdir -p /usr/share/figlet
sudo cp path/to/fonts/*.flf /usr/share/figlet/
```

Or use `-d` per session:

```bash
figby -d /path/to/fonts -f myfont "Hello"
```

## Comparison with C FIGlet

| Feature | C FIGlet 2.2.5 | Figby (Rust) |
|---------|----------------|--------------|
| FIGfont parser | sscanf-based | Type-safe parser |
| TLF support | Via code path | Native |
| Kerning | Yes | Yes |
| Smushing (H1–H6) | Yes | Yes |
| Smushing (V1–V5) | Yes | Yes |
| Multi-byte (UTF-8) | Optional flag | Native |
| Multi-byte (DBCS/SJIS/HZ) | Yes | Yes |
| Control files | Yes | Yes |
| ISO 2022 | Yes | Yes |
| Deutsch mode | Yes | Yes |
| ZIP fonts | Yes | Yes (`zip` crate) |
| CLI flags | 27 flags | Coverage of every implemented flag |
| Output compatibility | Baseline | Byte-identical across active differential tests |
| Memory safety | Manual | Guaranteed (Rust) |
| Error handling | Silent fallbacks | Explicit `Result` |
| Internal encoding | `wchar_t` / `inchr` | `char` (Unicode scalar) |
| Global state | 20+ globals | None (encapsulated) |

Figby's output is byte-identical to C FIGlet 2.2.5 across the active
differential test suite (`figby-rs/tests/run_tests.rs`, fixtures regenerated
from C by `scripts/regenerate-expected.sh`). A small set of known divergences
remains intentionally ignored — see below. All other differences are
intentional improvements in safety, clarity, and maintainability.

## Known divergences from C FIGlet

These behaviors differ from C FIGlet 2.2.5 and are tracked as `#[ignore]`d
differential tests (test numbers refer to `figby-rs/tests/run_tests.rs`).
They are real gaps to close, not intentional departures:

| Test | Scenario | Status |
|------|----------|--------|
| 21 | Paragraph mode (`-p -w250`) | Known divergence |
| 23 | Combined `-kpc` flags | Known divergence |
| 26 | JIS0201 control file | Known divergence |
| 35 | Control-character skipping | Known divergence |
| 37 | Smush all rules (`-m191`) | Known divergence |

## Project Status

Active development — a safe, idiomatic Rust FIGlet port that has grown into
a full ASCII-art TUI editor.

**Version identity:** the Cargo package version (currently `6.0.x`, see
`CHANGELOG.md`) is the canonical release version. `7.0.x` git tags are
milestone markers for the animation-editor line, not package versions — the
package continued on the `6.0.x` hardening line after `7.0.0-rc.1`.

- **v1** — C-to-Rust port (complete): parser, render engine, CLI, control
  files, multi-byte input, test suite. See [docs/todo-v1.md](docs/todo-v1.md).
- **v2–v5** — Polish & extensions (complete): templates, image-to-ASCII, font
  creation, and the full TUI editor (drawing tools, layers, palette editor,
  animation timeline/keyframes/GIF import-export).
- **v6** — Pre-release hardening & polish (complete): security fixes, green
  test suite, CI gate, parser hardening, architecture cleanup. See
  [docs/todo-v6.md](docs/todo-v6.md).
- **v7.0/v7.1** — Animation editor, playback & architecture (v7.0 complete,
  v7.1 active): usable animation editor, keymap/props overhaul, module
  architecture split. See [docs/todo-v7.md](docs/todo-v7.md).
- **v8** — Next backlog batch ("Part Twah"): multi-document tab strip, figmap
  file format, and smaller wins. See [docs/todo-v8.md](docs/todo-v8.md).

## Roadmap

Not yet done (tracked in [docs/todo-v6.md](docs/todo-v6.md)'s deferred
section unless noted):

- Reduced-motion (`--no-anim`) and color-depth fallback
- Panic-hook terminal restore + autosave
- Additional export formats: SVG, asciinema, sixel
- Template starter library
- Release tooling (cargo-dist / release-plz / VHS demos)
- Onboarding (`?`-help, which-key, tutorial)
- Figby → rename/de-brand (flagged as a copyrighted name in the v6 audit)

See [docs/todo.md](docs/todo.md) for the full milestone index.

## Contributing

Contributions welcome! Here's how to get started:

### Setup

```bash
git clone https://github.com/ClawfficeOrg/figby.git
cd figby
cargo build --manifest-path figby-rs/Cargo.toml -p figby
cargo test --manifest-path figby-rs/Cargo.toml -p figby
```

### Quality gates

Before committing, ensure (all from the repo root, using the crate's
`--manifest-path` — the root has no `Cargo.toml`):

```bash
# Check formatting
cargo fmt --manifest-path figby-rs/Cargo.toml -- --check

# Run linter (deny all warnings)
cargo clippy --manifest-path figby-rs/Cargo.toml --all-targets --all-features -- -D warnings

# Run tests
cargo test --manifest-path figby-rs/Cargo.toml
```

### Conventions

- **No `unwrap()` in production** — use proper error handling (`Result`, `Option`)
- **FIGfont spec compliance** is non-negotiable — test against original C output
- **UTF-8 native** — no `wchar_t` hacks
- **No global state** — all state passed explicitly as function parameters
- Tasks tracked in `docs/todo-*.md` — each task maps to a `task-X.Y.Z` branch

### Pull requests

1. Fork the repo
2. Create a feature branch off `main`
3. Implement your changes
4. Ensure `cargo fmt --manifest-path figby-rs/Cargo.toml -- --check` and
   `cargo clippy --manifest-path figby-rs/Cargo.toml --all-targets --all-features -- -D warnings` pass
5. Open a pull request

File issues at [github.com/ClawfficeOrg/figby/issues](https://github.com/ClawfficeOrg/figby/issues).

## License

New BSD License (same as FIGlet 2.2.5)
