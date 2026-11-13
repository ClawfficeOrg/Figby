//! Binary-only CLI modules (not part of the `figby` library).
//!
//! - `subcommands`: `figby play|bake|image|font|tui` and their mapping onto
//!   the legacy long flags.
//! - `playback`: the bake / play / daemon / tmux / detach programs.

// Playback drives the native terminal player (`figby::tui`), which does not
// exist on wasm32; the wasm binary only runs `figby::web`.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod playback;
pub(crate) mod subcommands;
