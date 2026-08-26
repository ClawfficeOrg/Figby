# Snap packaging — quarantined

This directory holds a **quarantined** Snap definition. It is NOT published
and should not be used as-is.

## What it is

`snapcraft.yaml` packages the **legacy C `figlet`** source under
`c-figlet/` (a `make` plugin build), not the Rust `figby` application this
repository is about. It targets `base: core18`, which is end-of-life.

## Why it is quarantined

- It builds and ships the C FIGlet binary/app name (`figlet`, `figlist`,
  `chkfont`, `showfigfonts`), not the Rust `figby` binary.
- Presenting that result as the stable product would mislead users about
  what this project ships.
- `base: core18` is unsupported.

## To ship Figby as a Snap

Write a new `snapcraft.yaml` that:

- Builds the Rust crate: `figby-rs/` (part plugin `rust`, or
  `cargo`), not `c-figlet/`.
- Declares the app as `figby` (not `figlet`).
- Uses a supported base (e.g. `core22`/`core24`).
- Bundles fonts (default font dir) and exposes `FIGLET_FONTDIR`.

Until that definition exists, Snap packaging is considered
**not-yet-available**.