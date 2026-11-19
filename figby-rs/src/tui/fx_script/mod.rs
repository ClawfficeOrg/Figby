//! Scripted tachyonfx effects (docs/fx-scripting-plan.md).
//!
//! - [`graph`]: `FxGraph`, Figby's serde IR for an effect tree, plus its
//!   validator and DSL emitter.
//! - [`compile`]: compile DSL text with Figby's target regions bound.
//! - [`presets`]: built-in graphs.
//! - [`runtime`]: the slot-keyed `EffectManager` and Figby's custom DSL
//!   effects (`glitch`).
//! - [`prompt`]: the one-line "Run effect…" prompt (F7).
//! - [`editor`]: the multi-line FX script editor (F6) with debounced live
//!   preview and inline error highlighting.
//! - [`syntax`]: the DSL lexer behind the editor's syntax colouring.
//! - [`edit`]: `GraphEditor`, validated structural edits with undo/redo.
//! - [`props`]: the Props panel model (a node's parameters as fields).
//! - [`graph_view`]: the FX Graph overlay with its edit mode (rataflow
//!   canvas behind the `fx-graph` feature, outline fallback without it).
//! - [`graph_file`]: saving and loading graphs (IR JSON plus emitted DSL).
//! - [`script`]: `.figfx.toml` script files (named effects, triggers,
//!   slots), parsed from `&str`.
//! - [`host`]: the loaded script's trigger queue and loops.
//! - [`panel`]: the Effects side-panel tab (script effects with play and
//!   stop buttons).
//! - [`script_file`]: reading a script and polling it for hot reload.
//!   With [`graph_file`], the only parts that touch the filesystem.
//!
//! Native-only, because it sits under `tui`.

pub mod compile;
pub mod edit;
pub mod editor;
pub mod graph;
pub mod graph_file;
pub mod graph_view;
pub mod host;
pub mod panel;
pub mod presets;
pub mod prompt;
pub mod props;
pub mod runtime;
pub mod script;
pub mod script_file;
pub mod syntax;

pub use editor::{FxEditor, FxEditorOutcome};
pub use graph_view::FxGraphView;
pub use host::FxScriptHost;
pub use prompt::{FxPrompt, FxPromptOutcome};
pub use runtime::{FxRuntime, FxSlot, FxStatus};
pub use script::{FxScript, FxTrigger};
