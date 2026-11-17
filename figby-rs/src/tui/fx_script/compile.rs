//! Compile tachyonfx DSL text with Figby's named target regions bound.
//!
//! Both script mode and the node graph go through here: the graph emits DSL
//! text (`FxGraph::to_dsl`) and this compiles it, so both share one
//! compiler, one set of errors and one runtime. Works on `&str` only (no
//! filesystem), per the wasm note in docs/fx-scripting-plan.md section 6.

use ratatui::layout::Rect;
use tachyonfx::Effect;

use super::runtime::figby_dsl;

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
    /// 1-based line of the error end.
    pub end_line: usize,
    /// 1-based column just past the error end (exclusive), as tachyonfx
    /// counts it: in chars, so it indexes the line by char, not byte.
    pub end_column: usize,
    pub message: String,
    /// `DslParseError::context()`: the lines around the error with the span
    /// underlined.
    pub context: String,
}

impl FxCompileError {
    /// The status-bar form, `fx line:column: message` (first message line
    /// only). Shared by the F7 prompt and the script editor.
    pub fn status_text(&self) -> String {
        let first = self.message.lines().next().unwrap_or_default();
        format!("fx {}:{}: {first}", self.line, self.column)
    }

    /// The error span as `(line_idx, start_char, end_char)` ranges over
    /// `src`, 0-based, end exclusive, one per line the error covers. Every
    /// range is at least one char wide (it may sit just past the line end,
    /// where the error is "expected more here"). Char-based on purpose:
    /// `DslParseError::error_text()` slices bytes with char columns, which
    /// panics on non-ASCII text and on a span that ends at the line end.
    pub fn span_ranges(&self, src: &str) -> Vec<(usize, usize, usize)> {
        let lines: Vec<&str> = src.split('\n').collect();
        let first = self.line.max(1);
        let last = self.end_line.max(first);
        (first..=last)
            .filter_map(|n| {
                let len = lines.get(n - 1)?.chars().count();
                let start = if n == first {
                    self.column.saturating_sub(1)
                } else {
                    0
                };
                let end = if n == last {
                    self.end_column.saturating_sub(1)
                } else {
                    len
                };
                let start = start.min(len);
                Some((n - 1, start, end.max(start + 1)))
            })
            .collect()
    }

    /// The source text the error covers (lines joined with `\n`), clipped
    /// to the text that exists. A char-safe stand-in for
    /// `DslParseError::error_text()`.
    pub fn error_text(&self, src: &str) -> String {
        let lines: Vec<&str> = src.split('\n').collect();
        self.span_ranges(src)
            .into_iter()
            .map(|(i, a, b)| lines[i].chars().skip(a).take(b - a).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl std::fmt::Display for FxCompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}: {}", self.line, self.column, self.message)
    }
}

impl std::error::Error for FxCompileError {}

/// Compile one DSL expression with `targets` bound. Figby's custom effects
/// (`glitch`) are registered.
pub fn compile(src: &str, targets: &FxTargets) -> Result<Effect, FxCompileError> {
    let dsl = figby_dsl();
    let mut compiler = dsl.compiler();
    for (name, rect) in targets.bindings() {
        compiler = compiler.bind(name, rect);
    }
    compiler.compile(src).map_err(|e| FxCompileError {
        line: e.start_line(),
        column: e.start_column(),
        end_line: e.end_line(),
        end_column: e.end_column(),
        message: e.source.to_string(),
        context: e.context(),
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
    fn error_span_covers_the_bad_name() {
        let src = "fx::dissolve(300)\n    .with_area(nowhere)";
        let err = compile(src, &FxTargets::default()).expect_err("must fail");
        let ranges = err.span_ranges(src);
        assert_eq!(ranges.len(), 1, "{err:?}");
        let (row, a, b) = ranges[0];
        assert_eq!(row, 1);
        assert_eq!(a, err.column - 1);
        assert!(b > a);
        assert!(err.error_text(src).contains("nowhere"), "{err:?}");
        // context() underlines the error line.
        assert!(err.context.contains("> 2 |"), "{}", err.context);
        assert!(err.context.contains('^'), "{}", err.context);
        assert!(
            err.status_text().starts_with("fx 2:"),
            "{}",
            err.status_text()
        );
    }

    fn err_at(line: usize, column: usize, end_line: usize, end_column: usize) -> FxCompileError {
        FxCompileError {
            line,
            column,
            end_line,
            end_column,
            message: "boom\nmore".into(),
            context: String::new(),
        }
    }

    #[test]
    fn span_ranges_are_char_based_and_never_empty() {
        // Non-ASCII before the span: columns count chars, not bytes.
        let src = "// é…\nfx::nope(1)";
        let e = err_at(2, 5, 2, 9);
        assert_eq!(e.span_ranges(src), vec![(1, 4, 8)]);
        assert_eq!(e.error_text(src), "nope");
        // A zero-width span (an "expected more" error) still marks a cell.
        let e = err_at(1, 3, 1, 3);
        assert_eq!(e.span_ranges("ab"), vec![(0, 2, 3)]);
        assert_eq!(e.error_text("ab"), "");
        // Multi-line: the first line to its end, whole middle lines, the
        // last line up to the end column.
        let src = "abc\ndef\nghi";
        let e = err_at(1, 2, 3, 2);
        assert_eq!(e.span_ranges(src), vec![(0, 1, 3), (1, 0, 3), (2, 0, 1)]);
        assert_eq!(e.error_text(src), "bc\ndef\ng");
        // Lines past the text are skipped rather than panicking.
        assert_eq!(err_at(9, 1, 9, 4).span_ranges(src), vec![]);
        assert_eq!(e.status_text(), "fx 1:2: boom");
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

    /// The upstream `DslParseError` for `src`, bypassing our wrapper.
    fn upstream_error(src: &str) -> tachyonfx::dsl::DslParseError {
        let dsl = figby_dsl();
        match dsl.compiler().compile(src) {
            Ok(_) => panic!("{src:?} must fail to compile"),
            Err(e) => e,
        }
    }

    // tachyonfx 0.25.1 `DslParseError::error_text()` (src/dsl/parse_error.rs
    // lines 156-200) slices the line by *byte* with the 1-based *char*
    // columns: `line[start_col..end_col]` with
    // `end_col = end_column.min(line.len() + 1)`. These tests pin that it
    // panics, which is why Figby uses `FxCompileError::error_text` instead.
    // If an upstream release fixes it, they fail and can be removed.

    #[test]
    #[should_panic(expected = "end byte index 14 is out of bounds for string of length 13")]
    fn upstream_error_text_panics_on_an_error_at_line_end() {
        // "Unmatched opening '('": span 1:13..1:14, ending at the line end.
        let e = upstream_error("fx::dissolve(");
        assert_eq!(
            (
                e.start_line(),
                e.start_column(),
                e.end_line(),
                e.end_column()
            ),
            (1, 13, 1, 14)
        );
        let _ = e.error_text();
    }

    #[test]
    #[should_panic(expected = "start byte index 37 is not a char boundary")]
    fn upstream_error_text_panics_on_non_ascii_before_the_error() {
        // Five 3-byte '…' before the error shift char columns 10 bytes
        // left of the real text; the start lands inside a '…'.
        let e = upstream_error(
            "fx::dissolve(300).with_area(/*\u{2026}\u{2026}\u{2026}\u{2026}\u{2026}*/nowhere)",
        );
        let _ = e.error_text();
    }

    #[test]
    fn our_error_text_handles_both_cases() {
        let t = FxTargets::default();
        for (src, want) in [
            ("fx::dissolve(", "("),
            (
                "fx::dissolve(300).with_area(/*\u{2026}\u{2026}\u{2026}\u{2026}\u{2026}*/nowhere)",
                "nowhere",
            ),
        ] {
            let err = compile(src, &t).expect_err("must fail");
            assert_eq!(err.error_text(src), want, "{err:?}");
        }
    }
}
