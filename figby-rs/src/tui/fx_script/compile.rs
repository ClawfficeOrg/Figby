//! Compile tachyonfx DSL text with Figby's named target regions bound.
//!
//! Both script mode and the node graph go through here: the graph emits DSL
//! text (`FxGraph::to_dsl`) and this compiles it, so both share one
//! compiler, one set of errors and one runtime. Works on `&str` only (no
//! filesystem), per the wasm note in docs/fx-scripting-plan.md section 6.

use ratatui::layout::Rect;
use tachyonfx::dsl::EffectDsl;
use tachyonfx::Effect;

/// Regions bound as DSL variables before compiling. A region Figby does not
/// have right now (no selection, say) is bound as an empty `Rect`, so a
/// script that names it still compiles and simply touches no cells.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FxTargets {
    pub screen: Rect,
    pub canvas: Rect,
    pub selection: Option<Rect>,
    pub toolbox: Rect,
    pub side_panel: Rect,
    pub status: Rect,
    /// Layer bounds; `layers[0]` is bound as `layer_1`.
    pub layers: Vec<Rect>,
}

impl FxTargets {
    /// Every target filled with `area`; handy for previews and tests.
    pub fn uniform(area: Rect, layers: usize) -> Self {
        Self {
            screen: area,
            canvas: area,
            selection: Some(area),
            toolbox: area,
            side_panel: area,
            status: area,
            layers: vec![area; layers],
        }
    }

    /// `(name, rect)` pairs in binding order.
    pub fn bindings(&self) -> Vec<(String, Rect)> {
        let mut out = vec![
            ("screen".to_string(), self.screen),
            ("canvas".to_string(), self.canvas),
            ("selection".to_string(), self.selection.unwrap_or_default()),
            ("toolbox".to_string(), self.toolbox),
            ("side_panel".to_string(), self.side_panel),
            ("status".to_string(), self.status),
        ];
        out.extend(
            self.layers
                .iter()
                .enumerate()
                .map(|(i, r)| (format!("layer_{}", i + 1), *r)),
        );
        out
    }
}

/// A compile failure, reduced to what the TUI shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FxCompileError {
    /// 1-based line of the error start.
    pub line: usize,
    /// 1-based column of the error start.
    pub column: usize,
    pub message: String,
}

impl std::fmt::Display for FxCompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}: {}", self.line, self.column, self.message)
    }
}

impl std::error::Error for FxCompileError {}

/// Compile one DSL expression with `targets` bound.
pub fn compile(src: &str, targets: &FxTargets) -> Result<Effect, FxCompileError> {
    let dsl = EffectDsl::new();
    let mut compiler = dsl.compiler();
    for (name, rect) in targets.bindings() {
        compiler = compiler.bind(name, rect);
    }
    compiler.compile(src).map_err(|e| FxCompileError {
        line: e.start_line(),
        column: e.start_column(),
        message: e.source.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::fx_script::graph::FxInterp;

    #[test]
    fn layer_bindings_are_one_based() {
        let t = FxTargets::uniform(Rect::new(0, 0, 4, 4), 2);
        let names: Vec<String> = t.bindings().into_iter().map(|(n, _)| n).collect();
        assert!(names.contains(&"layer_1".to_string()));
        assert!(names.contains(&"layer_2".to_string()));
        assert!(!names.contains(&"layer_0".to_string()));
    }

    #[test]
    fn bound_target_compiles() {
        let t = FxTargets::uniform(Rect::new(0, 0, 10, 4), 1);
        let fx = compile("fx::dissolve((300, Linear)).with_area(layer_1)", &t).unwrap();
        assert_eq!(fx.area(), Some(Rect::new(0, 0, 10, 4)));
    }

    #[test]
    fn missing_selection_binds_empty_rect() {
        let t = FxTargets::default();
        let fx = compile("fx::dissolve(300).with_area(selection)", &t).unwrap();
        assert_eq!(fx.area(), Some(Rect::default()));
    }

    #[test]
    fn errors_carry_line_and_column() {
        let t = FxTargets::default();
        let err = compile("fx::dissolve(300)\n    .with_area(nowhere)", &t)
            .expect_err("unknown variable must fail");
        assert_eq!(err.line, 2);
        assert!(err.column >= 1);
        assert!(err.message.contains("nowhere"), "{err}");
    }

    #[test]
    fn unknown_effect_is_an_error() {
        let err = compile("fx::no_such_effect(1)", &FxTargets::default()).expect_err("must fail");
        assert_eq!(err.line, 1);
    }

    #[test]
    fn every_interpolation_name_compiles() {
        let t = FxTargets::default();
        for interp in FxInterp::ALL {
            let src = format!("fx::dissolve((100, {}))", interp.dsl_name());
            assert!(compile(&src, &t).is_ok(), "{src}");
        }
        assert_eq!(FxInterp::ALL.len(), 34);
    }
}
