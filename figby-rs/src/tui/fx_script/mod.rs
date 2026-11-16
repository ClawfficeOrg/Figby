//! Scripted tachyonfx effects (docs/fx-scripting-plan.md).
//!
//! - [`graph`]: `FxGraph`, Figby's serde IR for an effect tree, plus its
//!   validator and DSL emitter.
//! - [`compile`]: compile DSL text with Figby's target regions bound.
//! - [`presets`]: built-in graphs.
//! - [`runtime`]: the slot-keyed `EffectManager` and Figby's custom DSL
//!   effects (`glitch`).
//! - [`prompt`]: the one-line "Run effect…" prompt (F7).
//! - [`graph_view`]: the read-only FX Graph overlay (rataflow canvas behind
//!   the `fx-graph` feature, outline fallback without it).
//! - [`script`]: `.figfx.toml` script files (named effects, triggers,
//!   slots), parsed from `&str`.
//! - [`host`]: the loaded script's trigger queue and loops.
//! - [`panel`]: the Effects side-panel tab (script effects with play and
//!   stop buttons).
//! - [`script_file`]: reading a script and polling it for hot reload (the
//!   only part that touches the filesystem).
//!
//! Native-only, because it sits under `tui`.

pub mod compile;
pub mod graph;
pub mod graph_view;
pub mod host;
pub mod panel;
pub mod presets;
pub mod prompt;
pub mod runtime;
pub mod script;
pub mod script_file;

pub use graph_view::FxGraphView;
pub use host::FxScriptHost;
pub use prompt::{FxPrompt, FxPromptOutcome};
pub use runtime::{FxRuntime, FxSlot, FxStatus};
pub use script::{FxScript, FxTrigger};
