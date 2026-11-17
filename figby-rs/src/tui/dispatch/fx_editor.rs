//! The FX script editor (F6, View menu): key routing, the debounced live
//! preview and running it in the manual slot (docs/fx-scripting-plan.md,
//! phase 3). Targets are bound exactly as for the F7 prompt
//! ([`TuiApp::fx_targets`], [`compile`]).
//!
//! The editor never touches the document or the canvas tools: while it is
//! open it takes every key, mouse and paste event, and it is drawn after
//! the effects tick, so a preview never paints over the editor itself.

use super::*;
use crate::tui::fx_script::compile::compile;
use crate::tui::fx_script::{FxEditorOutcome, FxSlot, FxStatus};
use std::ops::ControlFlow;
use std::time::Instant;

impl TuiApp {
    /// Open the editor. Its text is kept between openings; the first
    /// preview plays at the next poll.
    pub(crate) fn open_fx_editor(&mut self) {
        self.open_fx_editor_at(Instant::now());
    }

    pub(crate) fn open_fx_editor_at(&mut self, now: Instant) {
        self.dialogs.fx_prompt.close();
        self.dialogs.fx_editor.open(now);
        self.frame.effects.status = None;
        self.frame.dirty = true;
    }

    /// Event-loop hook: recompile and replay once the text has been idle
    /// for the debounce. `now` is injected so tests are deterministic.
    pub(crate) fn poll_fx_editor(&mut self, now: Instant) {
        if let Some(src) = self.dialogs.fx_editor.take_due(now) {
            self.run_fx_editor_text(&src);
        }
    }

    /// Compile `src` against the current targets and replay it in the
    /// manual slot. Errors go to the editor (inline) and to the status bar
    /// as `fx line:column: message`.
    pub(crate) fn run_fx_editor_text(&mut self, src: &str) {
        self.frame.dirty = true;
        match compile(src, &self.fx_targets()) {
            Ok(effect) => {
                self.frame.effects.run(FxSlot::Manual, effect);
                self.dialogs.fx_editor.set_result(Ok(()));
                self.frame.effects.status = Some(FxStatus {
                    text: "fx: previewing script".into(),
                    is_error: false,
                });
            }
            Err(e) => {
                // A broken edit stops the old preview, so what plays always
                // matches the text.
                if self.frame.effects.slot_running(FxSlot::Manual) {
                    self.frame.effects.cancel(FxSlot::Manual);
                }
                self.frame.effects.status = Some(FxStatus {
                    text: e.status_text(),
                    is_error: true,
                });
                self.dialogs.fx_editor.set_result(Err(e));
            }
        }
    }

    /// Act on what an editor input did: replay, or move text through the
    /// document clipboard (the same one the canvas selection uses).
    fn apply_fx_editor_outcome(&mut self, outcome: FxEditorOutcome, now: Instant) {
        match outcome {
            FxEditorOutcome::RunNow => {
                let src = self.dialogs.fx_editor.text();
                self.run_fx_editor_text(&src);
            }
            FxEditorOutcome::Copy(text) => {
                self.editor.clipboard = Some(clipboard_from_text(&text));
                self.frame.dirty = true;
            }
            FxEditorOutcome::Paste => {
                if let Some(clip) = self.editor.clipboard.as_ref() {
                    let text = text_from_clipboard(clip);
                    self.dialogs.fx_editor.insert_str(&text, now);
                    self.frame.dirty = true;
                }
            }
            FxEditorOutcome::Changed | FxEditorOutcome::Close => self.frame.dirty = true,
            FxEditorOutcome::None => {}
        }
    }

    /// The editor is modal while open: it takes every key.
    pub(super) fn key_fx_editor(&mut self, key: KeyEvent) -> ControlFlow<Option<AppEvent>> {
        if !self.dialogs.fx_editor.open {
            return ControlFlow::Continue(());
        }
        let now = Instant::now();
        let outcome = self.dialogs.fx_editor.handle_key(key, now);
        self.apply_fx_editor_outcome(outcome, now);
        ControlFlow::Break(None)
    }

    /// The editor is modal while open: it swallows the mouse, and clicks,
    /// drags and the wheel go to its text area. Returns whether it was open.
    pub(super) fn mouse_fx_editor(&mut self, mouse: MouseEvent) -> bool {
        if !self.dialogs.fx_editor.open {
            return false;
        }
        let now = Instant::now();
        let outcome = self.dialogs.fx_editor.handle_mouse(mouse, now);
        self.apply_fx_editor_outcome(outcome, now);
        true
    }
}

/// Editor text as a clipboard block: one row per line, one cell per char.
fn clipboard_from_text(text: &str) -> tools::selection::Clipboard {
    text.split('\n')
        .map(|line| {
            line.chars()
                .map(|ch| {
                    Some(canvas::CanvasCell {
                        ch,
                        ..Default::default()
                    })
                })
                .collect()
        })
        .collect()
}

/// A clipboard block as text: rows become lines, transparent cells spaces,
/// trailing blanks trimmed.
fn text_from_clipboard(clip: &tools::selection::Clipboard) -> String {
    clip.iter()
        .map(|row| {
            let line: String = row
                .iter()
                .map(|c| c.as_ref().map_or(' ', |c| c.ch))
                .collect();
            line.trim_end().to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::fx_script::editor::{FxEditorResult, DEBOUNCE};
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::style::Color;
    use ratatui::Terminal;
    use std::time::Duration;

    fn app() -> TuiApp {
        let mut app = TuiApp::new();
        app.welcome.screen.show = false;
        app.welcome.fx = None;
        app.welcome.fade_in = None;
        app
    }

    fn draw(app: &mut TuiApp, w: u16, h: u16) -> Buffer {
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        terminal.draw(|f| app.render(f)).unwrap();
        terminal.backend().buffer().clone()
    }

    fn key(app: &mut TuiApp, code: KeyCode) {
        let _ = app.handle_key_event(KeyEvent::new(code, KeyModifiers::NONE));
    }

    fn type_str(app: &mut TuiApp, s: &str) {
        for c in s.chars() {
            key(app, KeyCode::Char(c));
        }
    }

    fn canvas_rows(app: &TuiApp) -> Vec<String> {
        let buf = &app.editor.canvas.buffer;
        (0..buf.height())
            .map(|y| {
                (0..buf.width())
                    .map(|x| buf.get(x, y).map_or(' ', |c| c.ch))
                    .collect()
            })
            .collect()
    }

    #[test]
    fn f6_and_the_view_menu_open_the_editor() {
        let mut app = app();
        key(&mut app, KeyCode::F(6));
        assert!(app.dialogs.fx_editor.open);
        key(&mut app, KeyCode::Esc);
        assert!(!app.dialogs.fx_editor.open);
        app.handle_menu_action(menu::MenuAction::ViewFxEditor);
        assert!(app.dialogs.fx_editor.open);
    }

    #[test]
    fn typing_goes_to_the_editor_and_never_paints_the_canvas() {
        let mut app = app();
        draw(&mut app, 100, 30);
        let before = canvas_rows(&app);
        let undo_before = app.editor.undo.history_len();
        key(&mut app, KeyCode::F(6));
        // 'q' would quit, Space and letters would paint or switch tools.
        type_str(&mut app, "q x#b ");
        key(&mut app, KeyCode::Enter);
        assert!(!app.ui.should_quit);
        assert!(app.dialogs.fx_editor.text().ends_with("q x#b \n"));
        assert_eq!(canvas_rows(&app), before);
        assert_eq!(app.editor.undo.history_len(), undo_before);
        assert!(!app.editor.unsaved);
        // Paste lands in the editor too.
        app.handle_paste_event("zz".into());
        assert!(app.dialogs.fx_editor.text().ends_with("\nzz"));
        assert_eq!(canvas_rows(&app), before);
    }

    #[test]
    fn debounced_recompile_replays_in_the_manual_slot() {
        let mut app = app();
        draw(&mut app, 100, 30);
        let t0 = Instant::now();
        let ms = |n| t0 + Duration::from_millis(n);
        app.open_fx_editor_at(t0);
        app.dialogs
            .fx_editor
            .set_text("fx::dissolve(200).with_area(canvas)");
        app.dialogs.fx_editor.touch(ms(0));
        app.poll_fx_editor(ms(100));
        assert!(!app.frame.effects.is_running(), "still inside the debounce");
        app.poll_fx_editor(ms(0) + DEBOUNCE);
        assert!(app.frame.effects.slot_running(FxSlot::Manual));
        assert_eq!(app.dialogs.fx_editor.result(), Some(&FxEditorResult::Ok));
        assert!(app.frame.effects.needs_redraw());
        // The bound target is the drawn canvas, as with F7.
        let canvas = app.fx_targets().canvas;
        assert!(canvas.width > 0);
    }

    #[test]
    fn compile_error_shows_inline_and_in_the_status_bar() {
        let mut app = app();
        draw(&mut app, 120, 30);
        let t0 = Instant::now();
        app.open_fx_editor_at(t0);
        app.dialogs
            .fx_editor
            .set_text("fx::dissolve(200)\n    .with_area(nowhere)");
        app.dialogs.fx_editor.touch(t0);
        app.poll_fx_editor(t0 + DEBOUNCE);
        assert!(!app.frame.effects.slot_running(FxSlot::Manual));
        let err = app.dialogs.fx_editor.error().cloned().expect("error kept");
        assert_eq!(err.line, 2);
        let status = app.frame.effects.status.clone().unwrap();
        assert!(status.is_error);
        assert!(status.text.starts_with("fx 2:"), "{}", status.text);
        assert!(status.text.contains("nowhere"), "{}", status.text);

        let buf = draw(&mut app, 120, 30);
        let row = |y: u16| -> String { (0..120).map(|x| buf[(x, y)].symbol()).collect() };
        assert!(row(29).contains("fx 2:"), "status bar: {}", row(29));
        let screen: Vec<String> = (0..30).map(row).collect();
        assert!(
            screen.iter().any(|l| l.contains("FX Script: error")),
            "{screen:#?}"
        );
        // The error span is drawn reversed in the error colour.
        let theme_err = app.ctx.theme.general.error;
        let marked = (0..30u16).any(|y| {
            (0..120u16).any(|x| {
                let c = &buf[(x, y)];
                c.fg == theme_err && c.modifier.contains(ratatui::style::Modifier::REVERSED)
            })
        });
        assert!(marked);

        // Fixing it replays.
        app.dialogs
            .fx_editor
            .set_text("fx::dissolve(200)\n    .with_area(canvas)");
        app.dialogs.fx_editor.touch(t0);
        app.poll_fx_editor(t0 + DEBOUNCE);
        assert!(app.frame.effects.slot_running(FxSlot::Manual));
        assert_eq!(app.dialogs.fx_editor.error(), None);

        // Breaking it again stops the running preview.
        app.dialogs.fx_editor.set_text("fx::nope(1)");
        app.dialogs.fx_editor.touch(t0);
        app.poll_fx_editor(t0 + DEBOUNCE);
        draw(&mut app, 120, 30);
        assert!(!app.frame.effects.slot_running(FxSlot::Manual));
    }

    #[test]
    fn ctrl_r_replays_without_waiting() {
        let mut app = app();
        draw(&mut app, 100, 30);
        key(&mut app, KeyCode::F(6));
        app.dialogs.fx_editor.set_text("fx::coalesce(300)");
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
        assert!(app.frame.effects.slot_running(FxSlot::Manual));
    }

    fn ctrl(app: &mut TuiApp, c: char) {
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL));
    }

    #[test]
    fn copy_cut_paste_use_the_document_clipboard() {
        let mut app = app();
        draw(&mut app, 100, 30);
        key(&mut app, KeyCode::F(6));
        app.dialogs.fx_editor.set_text("ab\ncd");
        ctrl(&mut app, 'a');
        ctrl(&mut app, 'c');
        let clip = app.editor.clipboard.clone().expect("copied");
        assert_eq!(clip.len(), 2);
        assert_eq!(clip[1][1].as_ref().map(|c| c.ch), Some('d'));
        assert_eq!(app.dialogs.fx_editor.text(), "ab\ncd");
        ctrl(&mut app, 'x');
        assert_eq!(app.dialogs.fx_editor.text(), "");
        ctrl(&mut app, 'v');
        ctrl(&mut app, 'v');
        assert_eq!(app.dialogs.fx_editor.text(), "ab\ncdab\ncd");
        ctrl(&mut app, 'z');
        assert_eq!(app.dialogs.fx_editor.text(), "ab\ncd");
        ctrl(&mut app, 'y');
        assert_eq!(app.dialogs.fx_editor.text(), "ab\ncdab\ncd");
        // A canvas clipboard (transparent cells) pastes as text.
        app.editor.clipboard = Some(vec![vec![
            Some(canvas::CanvasCell {
                ch: '#',
                ..Default::default()
            }),
            None,
            Some(canvas::CanvasCell {
                ch: '*',
                ..Default::default()
            }),
            None,
        ]]);
        app.dialogs.fx_editor.set_text("");
        ctrl(&mut app, 'v');
        assert_eq!(app.dialogs.fx_editor.text(), "# *");
    }

    #[test]
    fn mouse_click_in_the_editor_places_the_cursor_only() {
        let mut app = app();
        draw(&mut app, 100, 30);
        key(&mut app, KeyCode::F(6));
        app.dialogs.fx_editor.set_text("first\nsecond");
        draw(&mut app, 100, 30);
        let before = canvas_rows(&app);
        let rect = app.dialogs.fx_editor.last_rect();
        // Border, then the "1 " gutter: row 2 col 3 of the text.
        let click = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: rect.x + 1 + 2 + 3,
            row: rect.y + 1 + 1,
            modifiers: KeyModifiers::NONE,
        };
        app.handle_mouse_event(click);
        assert_eq!(app.dialogs.fx_editor.text_area().cursor(), (1, 3));
        // A click above the editor (on the canvas) is swallowed.
        app.handle_mouse_event(MouseEvent { row: 5, ..click });
        assert_eq!(canvas_rows(&app), before);
        assert!(!app.editor.unsaved);
    }

    #[test]
    fn the_editor_is_drawn_over_a_running_preview() {
        // A screen-wide effect must not paint the editor's own cells.
        let mut app = app();
        draw(&mut app, 100, 30);
        key(&mut app, KeyCode::F(6));
        app.dialogs.fx_editor.set_text(
            "fx::never_complete(fx::fade_to_fg(Color::Red, (10, Linear))).with_area(screen)",
        );
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
        assert!(
            app.frame.effects.is_running(),
            "{:?}",
            app.frame.effects.status
        );
        // First frame steps by zero; the next by a fixed 50 ms, which ends
        // the fade (the effect then holds red forever).
        draw(&mut app, 100, 30);
        app.frame.fx_last_tick = Instant::now() - Duration::from_millis(50);
        let buf = draw(&mut app, 100, 30);
        let rect = app.dialogs.fx_editor.rect(Rect::new(0, 0, 100, 30));
        let title: String = (0..100).map(|x| buf[(x, rect.y)].symbol()).collect();
        assert!(title.contains("FX Script"), "{title}");
        let border = &buf[(rect.x, rect.y + 1)];
        assert_eq!(border.fg, app.ctx.theme.general.primary);
        // The effect did paint the screen outside the editor.
        let painted = (0..rect.y).any(|y| (0..100).any(|x| buf[(x, y)].fg == Color::Red));
        assert!(painted, "the preview runs over the rest of the screen");
    }
}
