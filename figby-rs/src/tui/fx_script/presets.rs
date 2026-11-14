//! Built-in effect graphs shown in the FX Graph view.
//!
//! `app-fade-in` and `welcome-fade` are the two effects Figby already runs
//! (`tui::fx::AppFadeIn` and `tui::fx::WelcomeFx`) expressed as graphs; the
//! tests check the graph versions draw the same cells as the originals.

use super::graph::{
    FxColor, FxEffect, FxFilter, FxGraph, FxGraphBuilder, FxInterp, FxMotion, FxNodeKind, FxTarget,
    FxTimer,
};

/// A named built-in graph.
#[derive(Debug, Clone, PartialEq)]
pub struct FxPreset {
    pub name: &'static str,
    pub description: &'static str,
    pub graph: FxGraph,
}

/// All presets, in display order.
pub fn all() -> Vec<FxPreset> {
    vec![
        FxPreset {
            name: "intro",
            description: "fade in, then sweep and hue-shift the text",
            graph: intro(),
        },
        FxPreset {
            name: "app-fade-in",
            description: "launch fade from black (built in)",
            graph: app_fade_in(),
        },
        FxPreset {
            name: "welcome-fade",
            description: "welcome dialog text fade (built in)",
            graph: welcome_fade(),
        },
        FxPreset {
            name: "sweep",
            description: "left-to-right wipe over the canvas",
            graph: single_on_canvas(FxEffect::SweepIn {
                motion: FxMotion::LeftToRight,
                gradient_len: 12,
                randomness: 0,
                color: FxColor::Cyan,
                timer: FxTimer::new(500, FxInterp::SineOut),
            }),
        },
        FxPreset {
            name: "dissolve",
            description: "canvas breaks up into random cells",
            graph: single_on_canvas(FxEffect::Dissolve {
                timer: FxTimer::new(500, FxInterp::QuadOut),
            }),
        },
        FxPreset {
            name: "coalesce",
            description: "canvas assembles from random cells",
            graph: single_on_canvas(FxEffect::Coalesce {
                timer: FxTimer::new(500, FxInterp::QuadOut),
            }),
        },
        FxPreset {
            name: "pulse",
            description: "text pulses cyan three times",
            graph: pulse(),
        },
    ]
}

fn single_on_canvas(effect: FxEffect) -> FxGraph {
    let mut b = FxGraphBuilder::new();
    let e = b.effect(effect);
    let root = b.add(
        FxNodeKind::Area {
            target: FxTarget::Canvas,
        },
        &[e],
    );
    b.build(root)
}

/// Same as `tui::fx::AppFadeIn`.
fn app_fade_in() -> FxGraph {
    let mut b = FxGraphBuilder::new();
    let root = b.effect(FxEffect::FadeFrom {
        bg: FxColor::Black,
        fg: FxColor::Black,
        timer: FxTimer::new(600, FxInterp::QuadOut),
    });
    b.build(root)
}

/// Same as `tui::fx::WelcomeFx`.
fn welcome_fade() -> FxGraph {
    let mut b = FxGraphBuilder::new();
    let root = b.effect(FxEffect::FadeFromFg {
        color: FxColor::DarkGray,
        timer: FxTimer::new(400, FxInterp::QuadOut),
    });
    b.build(root)
}

/// The worked example from docs/fx-scripting-plan.md section 1.6.
fn intro() -> FxGraph {
    let mut b = FxGraphBuilder::new();
    let fade = b.effect(FxEffect::FadeFrom {
        bg: FxColor::Black,
        fg: FxColor::Black,
        timer: FxTimer::new(600, FxInterp::QuadOut),
    });
    let sweep = b.effect(FxEffect::SweepIn {
        motion: FxMotion::LeftToRight,
        gradient_len: 12,
        randomness: 0,
        color: FxColor::Cyan,
        timer: FxTimer::new(500, FxInterp::SineOut),
    });
    let hue = b.effect(FxEffect::HslShiftFg {
        hsl: [40.0, 0.0, 0.0],
        timer: FxTimer::new(800, FxInterp::Linear),
    });
    let text_only = b.add(
        FxNodeKind::Filter {
            filter: FxFilter::Text,
        },
        &[hue],
    );
    let both = b.add(FxNodeKind::Parallel, &[sweep, text_only]);
    let seq = b.add(FxNodeKind::Sequence, &[fade, both]);
    let root = b.add(
        FxNodeKind::Area {
            target: FxTarget::Canvas,
        },
        &[seq],
    );
    b.build(root)
}

fn pulse() -> FxGraph {
    let mut b = FxGraphBuilder::new();
    let fade = b.effect(FxEffect::FadeToFg {
        color: FxColor::Cyan,
        timer: FxTimer::new(300, FxInterp::SineInOut),
    });
    let pp = b.add(FxNodeKind::PingPong, &[fade]);
    let rep = b.add(FxNodeKind::Repeat { times: 3 }, &[pp]);
    let root = b.add(
        FxNodeKind::Filter {
            filter: FxFilter::Text,
        },
        &[rep],
    );
    b.build(root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::fx::{AppFadeIn, WelcomeFx};
    use crate::tui::fx_script::compile::{compile, FxTargets};
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::style::{Color, Style};
    use std::time::Duration;

    fn sample_buffer(area: Rect) -> Buffer {
        let mut buf = Buffer::empty(area);
        for y in 0..area.height {
            buf.set_string(
                0,
                y,
                "Figby fx".repeat(4),
                Style::default().fg(Color::White).bg(Color::Blue),
            );
        }
        buf
    }

    #[test]
    fn every_preset_validates_and_compiles() {
        let targets = FxTargets::uniform(Rect::new(0, 0, 20, 5), 1);
        for p in all() {
            let dsl = p
                .graph
                .to_dsl()
                .unwrap_or_else(|e| panic!("{}: {e}", p.name));
            compile(&dsl, &targets).unwrap_or_else(|e| panic!("{}: {e}\n{dsl}", p.name));
        }
    }

    #[test]
    fn preset_names_are_unique() {
        let mut names: Vec<_> = all().iter().map(|p| p.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), all().len());
    }

    #[test]
    fn intro_emits_golden_dsl() {
        let expected = "\
fx::sequence(&[
    fx::fade_from(Color::Black, Color::Black, (600, QuadOut)),
    fx::parallel(&[
        fx::sweep_in(LeftToRight, 12, 0, Color::Cyan, (500, SineOut)),
        fx::hsl_shift_fg([40.0, 0.0, 0.0], (800, Linear))
        .with_filter(CellFilter::Text)
    ])
])
.with_area(canvas)";
        assert_eq!(intro().to_dsl().unwrap(), expected);
        assert_eq!(intro().duration_ms(), Some(1400));
    }

    /// Step both effects with the same fixed `dt` and compare every cell.
    fn assert_same_cells(
        mut graph_fx: tachyonfx::Effect,
        mut step_builtin: impl FnMut(Duration, &mut Buffer, Rect) -> bool,
    ) {
        let area = Rect::new(0, 0, 32, 3);
        let mut a = sample_buffer(area);
        let mut b = sample_buffer(area);
        let dt = Duration::from_millis(1000 / 30);
        for frame in 0..40 {
            graph_fx.process(dt, &mut a, area);
            let builtin_done = step_builtin(dt, &mut b, area);
            assert_eq!(a, b, "frame {frame}");
            assert_eq!(graph_fx.done(), builtin_done, "frame {frame}");
        }
        assert!(graph_fx.done());
    }

    #[test]
    fn app_fade_in_graph_matches_builtin_effect() {
        let dsl = app_fade_in().to_dsl().unwrap();
        let fx = compile(&dsl, &FxTargets::default()).unwrap();
        let mut builtin = AppFadeIn::new();
        assert_same_cells(fx, |dt, buf, area| {
            builtin.process(dt, buf, area);
            builtin.done()
        });
    }

    #[test]
    fn welcome_fade_graph_matches_builtin_effect() {
        let dsl = welcome_fade().to_dsl().unwrap();
        let fx = compile(&dsl, &FxTargets::default()).unwrap();
        let mut builtin = WelcomeFx::new();
        assert_same_cells(fx, |dt, buf, area| {
            builtin.process(dt, buf, area);
            builtin.done()
        });
    }
}
