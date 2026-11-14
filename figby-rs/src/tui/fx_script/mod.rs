//! Scripted tachyonfx effects (docs/fx-scripting-plan.md).
//!
//! - [`graph`]: `FxGraph`, Figby's serde IR for an effect tree, plus its
//!   validator and DSL emitter.
//! - [`compile`]: compile DSL text with Figby's target regions bound.
//! - [`presets`]: built-in graphs.
//! - [`graph_view`]: the read-only FX Graph overlay (rataflow canvas behind
//!   the `fx-graph` feature, outline fallback without it).
//!
//! Native-only, because it sits under `tui`.

pub mod compile;
pub mod graph;
pub mod graph_view;
pub mod presets;

pub use graph_view::FxGraphView;
