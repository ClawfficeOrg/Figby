//! Running compiled effects: one tachyonfx `EffectManager` keyed by
//! [`FxSlot`], plus Figby's custom DSL effects (`glitch`).
//!
//! Phase 0/1 of docs/fx-scripting-plan.md. The manager runs alongside the
//! older ad-hoc `WelcomeFx`/`AppFadeIn` fields; `TuiApp::render` ticks it
//! after everything else has drawn, and the event loop keeps redrawing while
//! [`FxRuntime::needs_redraw`] is true.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use std::time::Duration;
use tachyonfx::dsl::EffectDsl;
use tachyonfx::{fx, Effect, EffectManager, EffectTimer, Interpolation};

/// Longest step fed to an effect in one frame. A stall (the terminal
/// blocked, the machine slept) slows the effect down instead of skipping it.
const MAX_STEP: Duration = Duration::from_millis(100);

/// `EffectManager` key: starting an effect in a slot replaces whatever ran
/// there before.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum FxSlot {
    /// Effects started by hand ("Run effect…", F7).
    #[default]
    Manual,
}

/// A one-line message for the status bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FxStatus {
    pub text: String,
    pub is_error: bool,
}

/// Screen regions from the last frame, used to bind DSL targets.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FxScreenLayout {
    pub screen: Rect,
    /// The drawn canvas grid (inside its border), or the whole canvas
    /// panel when no grid was drawn.
    pub canvas: Rect,
    pub toolbox: Rect,
    pub side_panel: Rect,
    pub status: Rect,
}

/// The effect manager plus the bookkeeping the render loop needs.
#[derive(Default)]
pub struct FxRuntime {
    manager: EffectManager<FxSlot>,
    /// An effect started since the last tick: its first step is zero, so
    /// time spent idle before it started does not count against it.
    fresh: bool,
    /// The last effect ended during the previous tick; one more frame is
    /// needed to draw the plain UI over its final state.
    settle: bool,
    pub status: Option<FxStatus>,
    pub layout: FxScreenLayout,
}

impl FxRuntime {
    pub fn new() -> Self {
        Self::default()
    }

    /// Start `effect` in `slot`, replacing what ran there.
    pub fn run(&mut self, slot: FxSlot, effect: Effect) {
        self.manager.add_unique_effect(slot, effect);
        self.fresh = true;
    }

    /// Stop whatever runs in `slot`.
    pub fn cancel(&mut self, slot: FxSlot) {
        self.manager.cancel_unique_effect(slot);
    }

    pub fn is_running(&self) -> bool {
        self.manager.is_running()
    }

    /// True while an effect runs, and for one frame after the last ends.
    pub fn needs_redraw(&self) -> bool {
        self.is_running() || self.settle
    }

    /// Step every effect by `dt` over `buf`. Call once per frame, after
    /// the widgets have drawn.
    pub fn tick(&mut self, dt: Duration, buf: &mut Buffer, area: Rect) {
        if !self.is_running() {
            self.settle = false;
            return;
        }
        let dt = if std::mem::take(&mut self.fresh) {
            Duration::ZERO
        } else {
            dt.min(MAX_STEP)
        };
        self.manager.process_effects(dt, buf, area);
        if !self.is_running() {
            self.settle = true;
            if self.status.as_ref().is_some_and(|s| !s.is_error) {
                self.status = None;
            }
        }
    }
}

/// The DSL with Figby's custom effects registered.
pub fn figby_dsl() -> EffectDsl {
    EffectDsl::new().register("glitch", |args| {
        let ms = args.duration()?.as_millis().min(u128::from(u32::MAX)) as u32;
        Ok(glitch(ms))
    })
}

/// Glyphs a glitched cell may show.
const GLITCH_GLYPHS: &[char] = &['░', '▒', '▓', '█', '▚', '▞', '#', '%', '/', '\\', '?', '!'];

/// How often (ms of effect time) the glitch pattern changes.
const GLITCH_FRAME_MS: u64 = 40;

/// Figby's `glitch(ms)`: for `ms`, random cells (fewer as it nears the end)
/// swap to noise glyphs with inverted or neon colours. The pattern depends
/// only on cell position and elapsed effect time, so a fixed `dt` always
/// gives the same frames (needed for headless tests and rasterising).
pub fn glitch(ms: u32) -> Effect {
    fx::effect_fn(
        0u64,
        EffectTimer::from_ms(ms, Interpolation::Linear),
        |elapsed_ms, ctx, cells| {
            *elapsed_ms += ctx.last_tick.as_millis() as u64;
            let alpha = ctx.alpha();
            if alpha >= 1.0 {
                return;
            }
            let frame = *elapsed_ms / GLITCH_FRAME_MS;
            // Strong at the start, fading out.
            let threshold = ((1.0 - alpha) * 0.18 * u32::MAX as f32) as u32;
            for (pos, cell) in cells {
                let h = hash3(u32::from(pos.x), u32::from(pos.y), frame as u32);
                if h >= threshold {
                    continue;
                }
                let glyph = GLITCH_GLYPHS[(h as usize >> 3) % GLITCH_GLYPHS.len()];
                cell.set_char(glyph);
                match h & 3 {
                    0 => {
                        let (fg, bg) = (cell.fg, cell.bg);
                        cell.set_fg(bg);
                        cell.set_bg(fg);
                    }
                    1 => {
                        cell.set_fg(Color::Magenta);
                    }
                    2 => {
                        cell.set_fg(Color::Cyan);
                    }
                    _ => {
                        cell.set_fg(Color::LightGreen);
                    }
                }
            }
        },
    )
}

/// Small integer hash (lowbias32 over a mixed key).
fn hash3(x: u32, y: u32, t: u32) -> u32 {
    let mut h = x
        .wrapping_mul(0x9E37_79B1)
        .wrapping_add(y.wrapping_mul(0x85EB_CA77))
        .wrapping_add(t.wrapping_mul(0xC2B2_AE3D));
    h ^= h >> 16;
    h = h.wrapping_mul(0x7FEB_352D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846C_A68B);
    h ^= h >> 16;
    h
}

/// Join multi-line DSL into one line (each line trimmed), for the
/// single-line prompt. `FxGraph::to_dsl` puts breaks only between tokens
/// where no space is needed.
pub fn one_line(dsl: &str) -> String {
    dsl.lines().map(str::trim).collect::<Vec<_>>().join("")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::fx_script::compile::{compile, FxTargets};
    use ratatui::style::Style;

    fn sample(area: Rect) -> Buffer {
        let mut buf = Buffer::empty(area);
        for y in 0..area.height {
            buf.set_string(0, y, "x".repeat(area.width as usize), Style::default());
        }
        buf
    }

    const DT: Duration = Duration::from_millis(1000 / 30);

    #[test]
    fn manager_runs_until_the_effect_ends_then_settles() {
        let area = Rect::new(0, 0, 8, 2);
        let mut buf = sample(area);
        let mut rt = FxRuntime::new();
        assert!(!rt.is_running() && !rt.needs_redraw());
        let fx = compile("fx::dissolve(100)", &FxTargets::uniform(area, 0)).unwrap();
        rt.run(FxSlot::Manual, fx);
        assert!(rt.is_running());
        let mut frames = 0;
        while rt.is_running() {
            rt.tick(DT, &mut buf, area);
            frames += 1;
            assert!(frames < 20, "100 ms effect still running after 660 ms");
        }
        assert!(rt.needs_redraw(), "one settle frame after the end");
        rt.tick(DT, &mut buf, area);
        assert!(!rt.needs_redraw());
    }

    #[test]
    fn first_step_is_zero_after_idle() {
        let area = Rect::new(0, 0, 4, 1);
        let mut buf = sample(area);
        let mut rt = FxRuntime::new();
        rt.run(FxSlot::Manual, fx::dissolve(500));
        // A huge idle gap must not finish the effect on its first frame.
        rt.tick(Duration::from_secs(5), &mut buf, area);
        assert!(rt.is_running());
        // Later steps are capped, not skipped.
        rt.tick(Duration::from_secs(5), &mut buf, area);
        assert!(rt.is_running());
    }

    #[test]
    fn rerunning_a_slot_replaces_the_effect() {
        let area = Rect::new(0, 0, 4, 1);
        let mut buf = sample(area);
        let mut rt = FxRuntime::new();
        rt.run(FxSlot::Manual, fx::sleep(10_000));
        rt.tick(DT, &mut buf, area);
        rt.run(FxSlot::Manual, fx::sleep(50));
        for _ in 0..6 {
            rt.tick(DT, &mut buf, area);
        }
        assert!(!rt.is_running(), "the 10 s effect was not replaced");
    }

    #[test]
    fn cancel_stops_the_slot() {
        let area = Rect::new(0, 0, 4, 1);
        let mut buf = sample(area);
        let mut rt = FxRuntime::new();
        rt.run(FxSlot::Manual, fx::sleep(10_000));
        rt.tick(DT, &mut buf, area);
        rt.cancel(FxSlot::Manual);
        rt.tick(DT, &mut buf, area);
        rt.tick(DT, &mut buf, area);
        assert!(!rt.is_running());
    }

    #[test]
    fn info_status_clears_when_the_effect_ends_but_errors_stay() {
        let area = Rect::new(0, 0, 4, 1);
        let mut buf = sample(area);
        let mut rt = FxRuntime::new();
        rt.status = Some(FxStatus {
            text: "fx: running".into(),
            is_error: false,
        });
        rt.run(FxSlot::Manual, fx::sleep(10));
        rt.tick(DT, &mut buf, area);
        rt.tick(DT, &mut buf, area);
        assert_eq!(rt.status, None);
    }

    #[test]
    fn glitch_compiles_with_area_and_finishes() {
        let area = Rect::new(0, 0, 10, 3);
        let fx = compile(
            "fx::glitch(200).with_area(canvas)",
            &FxTargets::uniform(area, 0),
        )
        .unwrap();
        assert_eq!(fx.area(), Some(area));
        let mut rt = FxRuntime::new();
        rt.run(FxSlot::Manual, fx);
        let mut buf = sample(area);
        for _ in 0..12 {
            rt.tick(DT, &mut buf, area);
        }
        assert!(!rt.is_running());
    }

    #[test]
    fn glitch_is_deterministic_for_a_fixed_dt() {
        let area = Rect::new(0, 0, 16, 4);
        let run = || {
            let mut fx = glitch(300);
            let mut buf = sample(area);
            let mut frames = Vec::new();
            for _ in 0..6 {
                fx.process(DT, &mut buf, area);
                frames.push(buf.clone());
            }
            frames
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn glitch_needs_a_duration() {
        let err = compile("fx::glitch()", &FxTargets::default()).expect_err("no args");
        assert_eq!(err.line, 1);
    }

    #[test]
    fn one_line_joins_graph_dsl() {
        assert_eq!(
            one_line(
                "fx::parallel(&[\n    fx::dissolve(1),\n    fx::sleep(2)\n])\n.with_area(canvas)"
            ),
            "fx::parallel(&[fx::dissolve(1),fx::sleep(2)]).with_area(canvas)"
        );
    }
}
