//! "Run effect…" (F7, View menu): the prompt's key routing, DSL target
//! binding and running the result in the `manual` effect slot
//! (docs/fx-scripting-plan.md, phase 1).

use super::*;
use crate::tui::fx_script::compile::{compile, FxTargets};
use crate::tui::fx_script::{FxPromptOutcome, FxSlot, FxStatus};
use std::ops::ControlFlow;

/// Screen rect covered by the inclusive buffer cell range `(x0, y0, x1, y1)`
/// when the canvas grid is drawn at `inner` with `scroll` and `zoom`.
/// Cells scrolled out of view are clipped; nothing visible gives an empty
/// `Rect`.
pub(crate) fn buffer_bounds_to_screen(
    bounds: (usize, usize, usize, usize),
    scroll: (u16, u16),
    zoom: u8,
    inner: Rect,
) -> Rect {
    let (x0, y0, x1, y1) = bounds;
    if x0 > x1 || y0 > y1 {
        return Rect::default();
    }
    let z = i64::from(zoom.max(1));
    let span = |lo: usize, hi: usize, scroll: u16, len: u16| -> Option<(u16, u16)> {
        let a = ((lo as i64 - i64::from(scroll)) * z).clamp(0, i64::from(len));
        let b = ((hi as i64 + 1 - i64::from(scroll)) * z).clamp(0, i64::from(len));
        (b > a).then_some((a as u16, (b - a) as u16))
    };
    match (
        span(x0, x1, scroll.0, inner.width),
        span(y0, y1, scroll.1, inner.height),
    ) {
        (Some((dx, w)), Some((dy, h))) => Rect::new(inner.x + dx, inner.y + dy, w, h),
        _ => Rect::default(),
    }
}

/// Inclusive bounds of the cells in `buf` that differ from a blank cell.
fn content_bounds(buf: &canvas::CanvasBuffer) -> Option<(usize, usize, usize, usize)> {
    let blank = canvas::CanvasCell::default();
    let mut out: Option<(usize, usize, usize, usize)> = None;
    for y in 0..buf.height() {
        for x in 0..buf.width() {
            if buf.get(x, y).is_some_and(|c| *c != blank) {
                out = Some(match out {
                    None => (x, y, x, y),
                    Some((a, b, c, d)) => (a.min(x), b.min(y), c.max(x), d.max(y)),
                });
            }
        }
    }
    out
}

impl TuiApp {
    /// Open the prompt, clearing any previous fx error from the status bar.
    pub(crate) fn open_fx_prompt(&mut self) {
        self.dialogs.fx_prompt.open();
        self.frame.effects.status = None;
        self.frame.dirty = true;
    }

    /// DSL targets from the last frame's layout and the current canvas.
    pub(crate) fn fx_targets(&self) -> FxTargets {
        let layout = self.frame.effects.layout;
        let scroll = self.editor.canvas.scroll_offset();
        let zoom = self.editor.canvas.zoom_level();
        let to_screen = |b| buffer_bounds_to_screen(b, scroll, zoom, layout.canvas);
        let selection = self
            .editor
            .selection
            .as_ref()
            .filter(|s| s.is_active())
            .map(|s| to_screen(s.bounds()));
        let layers = self
            .editor
            .layer_stack
            .layers
            .iter()
            .map(|l| content_bounds(l.buffer()).map_or_else(Rect::default, to_screen))
            .collect();
        FxTargets {
            screen: layout.screen,
            canvas: layout.canvas,
            selection,
            toolbox: layout.toolbox,
            side_panel: layout.side_panel,
            status: layout.status,
            layers,
        }
    }

    /// Compile `text` (a preset name or one DSL expression) and run it in
    /// the manual slot. On a compile error the prompt stays open with the
    /// cursor on the error column, and the status bar shows `line:column`.
    pub(crate) fn run_fx_text(&mut self, text: &str) {
        self.frame.dirty = true;
        if text.trim().is_empty() {
            self.frame.effects.status = Some(FxStatus {
                text: "fx: type a DSL expression or pick a preset".into(),
                is_error: true,
            });
            return;
        }
        let preset = self.dialogs.fx_prompt.preset_named(text).cloned();
        let (label, src) = match &preset {
            Some(p) => (p.name.to_string(), p.dsl.clone()),
            None => ("expression".to_string(), text.to_string()),
        };
        match compile(&src, &self.fx_targets()) {
            Ok(effect) => {
                self.frame.effects.run(FxSlot::Manual, effect);
                self.frame.effects.status = Some(FxStatus {
                    text: format!("fx: running {label}"),
                    is_error: false,
                });
                self.dialogs.fx_prompt.close();
            }
            Err(e) => {
                let first = e.message.lines().next().unwrap_or_default();
                self.frame.effects.status = Some(FxStatus {
                    text: format!("fx {}:{}: {first}", e.line, e.column),
                    is_error: true,
                });
                if preset.is_none() && e.line == 1 {
                    self.dialogs.fx_prompt.set_cursor_column(e.column);
                }
            }
        }
    }

    /// The prompt is modal while open: it takes every key.
    pub(super) fn key_fx_prompt(&mut self, key: KeyEvent) -> ControlFlow<Option<AppEvent>> {
        if !self.dialogs.fx_prompt.open {
            return ControlFlow::Continue(());
        }
        match self.dialogs.fx_prompt.handle_key(key) {
            FxPromptOutcome::Submit(text) => self.run_fx_text(&text),
            FxPromptOutcome::Changed | FxPromptOutcome::Cancel => self.frame.dirty = true,
            FxPromptOutcome::None => {}
        }
        ControlFlow::Break(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn app() -> TuiApp {
        let mut app = TuiApp::new();
        app.welcome.screen.show = false;
        app.welcome.fx = None;
        app.welcome.fade_in = None;
        app
    }

    fn draw(app: &mut TuiApp) {
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
        terminal.draw(|f| app.render(f)).unwrap();
    }

    fn key(app: &mut TuiApp, code: KeyCode) {
        let _ = app.handle_key_event(KeyEvent::new(code, KeyModifiers::NONE));
    }

    fn type_str(app: &mut TuiApp, s: &str) {
        for c in s.chars() {
            key(app, KeyCode::Char(c));
        }
    }

    #[test]
    fn f7_opens_the_prompt_and_esc_closes_it() {
        let mut app = app();
        key(&mut app, KeyCode::F(7));
        assert!(app.dialogs.fx_prompt.open);
        // Typing goes to the prompt, not the editor ('q' would quit).
        type_str(&mut app, "q");
        assert_eq!(app.dialogs.fx_prompt.input(), "q");
        key(&mut app, KeyCode::Esc);
        assert!(!app.dialogs.fx_prompt.open);
        assert!(!app.frame.effects.is_running());
    }

    #[test]
    fn view_menu_opens_the_prompt() {
        let mut app = app();
        app.handle_menu_action(menu::MenuAction::ViewRunEffect);
        assert!(app.dialogs.fx_prompt.open);
    }

    #[test]
    fn submitting_dsl_runs_it_in_the_manual_slot() {
        let mut app = app();
        draw(&mut app);
        key(&mut app, KeyCode::F(7));
        type_str(&mut app, "fx::dissolve(200).with_area(canvas)");
        key(&mut app, KeyCode::Enter);
        assert!(!app.dialogs.fx_prompt.open, "prompt closes on success");
        assert!(app.frame.effects.is_running());
        let status = app.frame.effects.status.clone().unwrap();
        assert!(!status.is_error, "{status:?}");
        // The event loop's redraw condition sees the running effect.
        assert!(app.frame.effects.needs_redraw());
        // Rendering ticks it; enough frames finish it.
        for _ in 0..3 {
            draw(&mut app);
        }
    }

    #[test]
    fn compile_error_shows_line_and_column_in_status_and_keeps_prompt() {
        let mut app = app();
        draw(&mut app);
        key(&mut app, KeyCode::F(7));
        type_str(&mut app, "fx::dissolve(200).with_area(nowhere)");
        key(&mut app, KeyCode::Enter);
        assert!(app.dialogs.fx_prompt.open, "prompt stays open to fix");
        assert!(!app.frame.effects.is_running());
        let status = app.frame.effects.status.clone().unwrap();
        assert!(status.is_error);
        assert!(status.text.starts_with("fx 1:"), "{}", status.text);
        assert!(status.text.contains("nowhere"), "{}", status.text);
        let col: usize = status.text["fx 1:".len()..]
            .split(':')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(app.dialogs.fx_prompt.cursor(), col - 1);
        // And the message reaches the drawn status bar.
        let mut terminal = Terminal::new(TestBackend::new(140, 30)).unwrap();
        terminal.draw(|f| app.render(f)).unwrap();
        let buf = terminal.backend().buffer();
        let last: String = (0..140).map(|x| buf[(x, 29)].symbol()).collect();
        assert!(last.contains("fx 1:"), "{last}");
        // Reopening clears the error.
        key(&mut app, KeyCode::Esc);
        key(&mut app, KeyCode::F(7));
        assert_eq!(app.frame.effects.status, None);
    }

    #[test]
    fn every_runnable_preset_runs_by_name() {
        for name in crate::tui::fx_script::presets::RUNNABLE {
            let mut app = app();
            draw(&mut app);
            app.open_fx_prompt();
            app.run_fx_text(name);
            assert!(app.frame.effects.is_running(), "{name}");
            assert!(!app.dialogs.fx_prompt.open, "{name}");
        }
    }

    #[test]
    fn targets_bind_the_drawn_canvas_and_layers() {
        let mut app = app();
        draw(&mut app);
        let t = app.fx_targets();
        assert_eq!(t.screen, Rect::new(0, 0, 100, 30));
        assert!(t.canvas.width > 0 && t.canvas.height > 0);
        assert_eq!(t.layers.len(), app.editor.layer_stack.len());
        assert_eq!(t.selection, None);
        let fx = compile("fx::dissolve(10).with_area(canvas)", &t).unwrap();
        assert_eq!(fx.area(), Some(t.canvas));
    }

    #[test]
    fn buffer_bounds_map_through_scroll_and_zoom() {
        let inner = Rect::new(10, 5, 40, 20);
        assert_eq!(
            buffer_bounds_to_screen((0, 0, 3, 1), (0, 0), 1, inner),
            Rect::new(10, 5, 4, 2)
        );
        assert_eq!(
            buffer_bounds_to_screen((2, 1, 3, 1), (1, 0), 2, inner),
            Rect::new(12, 7, 4, 2)
        );
        // Clipped at the right edge.
        assert_eq!(
            buffer_bounds_to_screen((30, 0, 60, 0), (0, 0), 1, inner),
            Rect::new(40, 5, 10, 1)
        );
        // Scrolled fully out of view.
        assert_eq!(
            buffer_bounds_to_screen((0, 0, 1, 1), (5, 0), 1, inner),
            Rect::default()
        );
    }

    #[test]
    fn content_bounds_finds_drawn_cells() {
        let mut buf = canvas::CanvasBuffer::new(8, 4);
        assert_eq!(content_bounds(&buf), None);
        let cell = canvas::CanvasCell {
            ch: '#',
            ..Default::default()
        };
        buf.set(2, 1, cell);
        buf.set(5, 3, cell);
        assert_eq!(content_bounds(&buf), Some((2, 1, 5, 3)));
    }
}
