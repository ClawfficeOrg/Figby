//! Binary-only CLI modules (not part of the `figby` library).
//!
//! - `subcommands`: `figby play|bake|image|font|tui` and their mapping onto
//!   the legacy long flags.
//! - `playback`: the bake / play / daemon / tmux / detach programs.
//! - `create_font`: `--create-font` / `--create-font-path` (FIGfont generation).
//! - `template`: `--render-template` (`.ftmp` rendering).
//! - `image`: `--image` conversion of image files to ASCII / braille.

// Font generation (`figby::font_gen`) and the terminal player (`figby::tui`)
// do not exist on wasm32; the wasm binary only runs `figby::web`.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod create_font;
pub(crate) mod image;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod playback;
pub(crate) mod subcommands;
pub(crate) mod template;
