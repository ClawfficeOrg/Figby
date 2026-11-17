//! The FX script editor (F6, View > FX Script Editor…): a multi-line DSL
//! expression with live preview (docs/fx-scripting-plan.md, phase 3).
//!
//! Text editing is the generic [`TextArea`] widget
//! (`components/multiline.rs`); this module adds only the FX parts:
//!
//! - Every edit restarts a [`DEBOUNCE`] timer. Once the text has been idle
//!   that long, [`FxEditor::take_due`] hands it back for one compile; the
//!   caller binds the same targets as the F7 prompt, runs the effect in the
//!   manual slot and reports the result with [`FxEditor::set_result`].
//! - A compile error is kept here and drawn inline: its span (char columns,
//!   from `FxCompileError::span_ranges`) becomes a [`Highlight`] in the text
//!   area, and the message sits under the text.
//!
//! The clock is injected: key handling and polling take `now`, so tests
//! drive the debounce with fixed instants.
//!
//! Keys: the text area's (arrows, Home/End, PageUp/PageDown, Shift to
//! select, Ctrl+A/C/X/V, Ctrl+Z/Y), plus Ctrl+R to compile and replay now
//! and Esc to close (the text is kept for next time).

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;
use std::time::{Duration, Instant};

use super::compile::FxCompileError;
use super::prompt::TARGET_NAMES;
use super::syntax;
use crate::tui::components::multiline::{Highlight, TextArea, TextAreaEvent, TextAreaStyle};
use crate::tui::theme::Theme;

/// Idle time after the last edit before the preview recompiles.
pub const DEBOUNCE: Duration = Duration::from_millis(300);

/// What the editor starts with the first time it opens.
pub const STARTER: &str = "// Live preview: edits recompile after a short pause.\n\
fx::sequence(&[\n    fx::coalesce((500, QuadOut)),\n    fx::glitch(300),\n])\n.with_area(canvas)";

/// Rows the editor uses for its message lines under the text.
const MESSAGE_ROWS: u16 = 2;

/// What an input did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FxEditorOutcome {
    /// Nothing changed.
    None,
    /// The text, cursor or selection changed; redraw.
    Changed,
    /// Ctrl+R: compile and replay now, without waiting for the debounce.
    RunNow,
    /// Esc: the editor closed.
    Close,
    /// Copy or cut: put this text on the clipboard.
    Copy(String),
    /// Ctrl+V: call [`FxEditor::insert_str`] with the clipboard text.
    Paste,
}

/// The last compile, as the editor shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FxEditorResult {
    /// Running in the manual slot.
    Ok,
    Error(FxCompileError),
}

pub struct FxEditor {
    pub open: bool,
    area: TextArea,
    /// When the text last changed, while a recompile is pending.
    edited_at: Option<Instant>,
    result: Option<FxEditorResult>,
    /// The box drawn last, for mouse hit-testing.
    rect: Rect,
}

impl Default for FxEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl FxEditor {
    pub fn new() -> Self {
        let mut area = TextArea::from_text(STARTER);
        area.line_numbers = true;
        Self {
            open: false,
            area,
            edited_at: None,
            result: None,
            rect: Rect::default(),
        }
    }

    /// Open, keeping the text. A first preview is due right away, so the
    /// effect plays as soon as the editor shows.
    pub fn open(&mut self, now: Instant) {
        self.open = true;
        self.edited_at = now.checked_sub(DEBOUNCE).or(Some(now));
    }

    /// Close; a pending recompile is dropped.
    pub fn close(&mut self) {
        self.open = false;
        self.edited_at = None;
    }

    /// The whole text, lines joined with `\n`.
    pub fn text(&self) -> String {
        self.area.text()
    }

    /// Replace the text (clears its undo history); the cursor goes to the
    /// end. Does not arm the debounce (callers that want a preview call
    /// [`Self::touch`]).
    pub fn set_text(&mut self, text: &str) {
        self.area.set_text(text);
        self.result = None;
    }

    /// The text area, for its cursor and selection.
    pub fn text_area(&self) -> &TextArea {
        &self.area
    }

    pub fn result(&self) -> Option<&FxEditorResult> {
        self.result.as_ref()
    }

    /// Whether a recompile is waiting for the debounce.
    pub fn pending(&self) -> bool {
        self.edited_at.is_some()
    }

    /// Mark the text as edited at `now`: restarts the debounce.
    pub fn touch(&mut self, now: Instant) {
        self.edited_at = Some(now);
    }

    /// Whether the debounce has run out at `now`.
    pub fn due(&self, now: Instant) -> bool {
        self.open
            && self
                .edited_at
                .is_some_and(|t| now.saturating_duration_since(t) >= DEBOUNCE)
    }

    /// The text to compile, once per pause: `Some` when the debounce ran
    /// out at `now`, after which nothing is due until the next edit.
    pub fn take_due(&mut self, now: Instant) -> Option<String> {
        if !self.due(now) {
            return None;
        }
        self.edited_at = None;
        Some(self.text())
    }

    /// Record the outcome of compiling the text.
    pub fn set_result(&mut self, result: Result<(), FxCompileError>) {
        self.result = Some(match result {
            Ok(()) => FxEditorResult::Ok,
            Err(e) => FxEditorResult::Error(e),
        });
    }

    /// The current error, if the last compile failed.
    pub fn error(&self) -> Option<&FxCompileError> {
        match &self.result {
            Some(FxEditorResult::Error(e)) => Some(e),
            _ => None,
        }
    }

    /// Insert pasted or clipboard text at the cursor (replacing any
    /// selection) and restart the debounce.
    pub fn insert_str(&mut self, text: &str, now: Instant) {
        self.area.insert_str(text);
        self.touch(now);
    }

    fn outcome(&mut self, ev: TextAreaEvent, now: Instant) -> FxEditorOutcome {
        if ev.is_edit() {
            self.touch(now);
        }
        match ev {
            TextAreaEvent::Ignored => FxEditorOutcome::None,
            TextAreaEvent::Moved | TextAreaEvent::Edited => FxEditorOutcome::Changed,
            TextAreaEvent::Copy(s) | TextAreaEvent::Cut(s) => FxEditorOutcome::Copy(s),
            TextAreaEvent::PasteRequested => FxEditorOutcome::Paste,
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent, now: Instant) -> FxEditorOutcome {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Esc => {
                self.close();
                FxEditorOutcome::Close
            }
            KeyCode::Char('r') | KeyCode::Char('R') if ctrl => {
                self.edited_at = None;
                FxEditorOutcome::RunNow
            }
            _ => {
                let ev = self.area.handle_key(key);
                self.outcome(ev, now)
            }
        }
    }

    /// Mouse inside the text: place the cursor, select, scroll.
    pub fn handle_mouse(&mut self, mouse: MouseEvent, now: Instant) -> FxEditorOutcome {
        let ev = self.area.handle_mouse(mouse);
        self.outcome(ev, now)
    }

    /// The box the editor draws in: docked at the bottom of `area`, above
    /// the status bar row, so the canvas above stays visible for the
    /// preview.
    pub fn rect(&self, area: Rect) -> Rect {
        let avail = area.height.saturating_sub(1);
        let height = (avail * 2 / 5).clamp(8.min(avail), 18).min(avail);
        Rect {
            x: area.x,
            y: area.y + avail - height,
            width: area.width,
            height,
        }
    }

    /// The box drawn by the last render.
    pub fn last_rect(&self) -> Rect {
        self.rect
    }

    pub fn render(&mut self, frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
        let rect = self.rect(area);
        self.rect = rect;
        if rect.width < 16 || rect.height < 5 {
            return;
        }
        frame.render_widget(Clear, rect);
        let dim = Style::default().fg(theme.general.secondary);
        let err_style = Style::default().fg(theme.general.error);
        let title = match &self.result {
            Some(FxEditorResult::Error(_)) => Span::styled(" FX Script: error ", err_style),
            _ if self.pending() => Span::styled(" FX Script: editing… ", dim),
            Some(FxEditorResult::Ok) => Span::styled(
                " FX Script: running ",
                Style::default().fg(theme.general.success),
            ),
            None => Span::styled(" FX Script ", dim),
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(theme.general.primary))
            .title(title.add_modifier(Modifier::BOLD))
            .title_bottom(Line::styled(
                " Ctrl+R replay  Ctrl+Z/Y undo/redo  Esc close ",
                dim,
            ))
            .style(
                Style::default()
                    .bg(theme.menu.dropdown_bg)
                    .fg(theme.menu.fg),
            );
        let inner = block.inner(rect);
        frame.render_widget(block, rect);
        let text_rows = inner.height.saturating_sub(MESSAGE_ROWS);
        if text_rows == 0 {
            return;
        }

        let text = self.text();
        let highlights = self
            .error()
            .map(|e| e.span_ranges(&text))
            .unwrap_or_default()
            .into_iter()
            .map(|(row, start, end)| Highlight { row, start, end })
            .collect();
        self.area.set_highlights(highlights);
        self.area.set_tokens(syntax::tokenize(&text));
        let style = TextAreaStyle {
            text: Style::default(),
            tokens: syntax::token_styles(theme),
            cursor: Style::default().add_modifier(Modifier::REVERSED),
            selection: Style::default()
                .bg(theme.canvas.selection)
                .fg(theme.menu.fg),
            highlight: Style::default()
                .fg(theme.general.error)
                .add_modifier(Modifier::REVERSED | Modifier::UNDERLINED),
            line_number: dim,
            line_number_marked: err_style.add_modifier(Modifier::BOLD),
        };
        let text_rect = Rect {
            height: text_rows,
            ..inner
        };
        self.area.render_frame(frame, text_rect, &style);

        let message = match &self.result {
            Some(FxEditorResult::Error(e)) => vec![
                Line::styled(e.status_text(), err_style.add_modifier(Modifier::BOLD)),
                Line::styled(
                    format!("  at `{}`", e.error_text(&text).replace('\n', "⏎")),
                    err_style,
                ),
            ],
            Some(FxEditorResult::Ok) if !self.pending() => vec![
                Line::styled("ok: running over its target (manual slot)", dim),
                Line::styled(format!("targets: {TARGET_NAMES}"), dim),
            ],
            _ => vec![
                Line::styled("", dim),
                Line::styled(format!("targets: {TARGET_NAMES}"), dim),
            ],
        };
        let msg_rect = Rect {
            y: inner.y + text_rows,
            height: inner.height - text_rows,
            ..inner
        };
        frame.render_widget(Paragraph::new(message), msg_rect);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::fx_script::compile::{compile, FxTargets};
    use ratatui::backend::TestBackend;
    use ratatui::layout::Position;
    use ratatui::Terminal;

    fn key(ed: &mut FxEditor, code: KeyCode, now: Instant) -> FxEditorOutcome {
        ed.handle_key(KeyEvent::new(code, KeyModifiers::NONE), now)
    }

    fn ctrl(ed: &mut FxEditor, c: char, now: Instant) -> FxEditorOutcome {
        ed.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL), now)
    }

    fn type_str(ed: &mut FxEditor, s: &str, now: Instant) {
        for c in s.chars() {
            key(ed, KeyCode::Char(c), now);
        }
    }

    fn blank() -> FxEditor {
        let mut ed = FxEditor::new();
        ed.set_text("");
        ed
    }

    #[test]
    fn debounce_waits_for_a_pause_and_fires_once() {
        let t0 = Instant::now();
        let ms = |n| t0 + Duration::from_millis(n);
        let mut ed = blank();
        ed.open = true;
        type_str(&mut ed, "fx::", ms(0));
        assert!(ed.pending());
        assert_eq!(ed.take_due(ms(299)), None, "not idle long enough");
        // Another edit restarts the timer.
        type_str(&mut ed, "dissolve(9)", ms(200));
        assert_eq!(ed.take_due(ms(450)), None);
        assert_eq!(ed.take_due(ms(500)).as_deref(), Some("fx::dissolve(9)"));
        // Once per pause: nothing more until the next edit.
        assert_eq!(ed.take_due(ms(5000)), None);
        // Cursor moves and copies are not edits.
        key(&mut ed, KeyCode::Left, ms(6000));
        ctrl(&mut ed, 'a', ms(6000));
        assert_eq!(
            ctrl(&mut ed, 'c', ms(6000)),
            FxEditorOutcome::Copy("fx::dissolve(9)".into())
        );
        assert_eq!(ed.take_due(ms(7000)), None);
        key(&mut ed, KeyCode::End, ms(7000));
        key(&mut ed, KeyCode::Left, ms(7000));
        key(&mut ed, KeyCode::Backspace, ms(7000));
        assert_eq!(ed.take_due(ms(7300)).as_deref(), Some("fx::dissolve()"));
        // Undo is an edit too, and re-arms the preview.
        ctrl(&mut ed, 'z', ms(8000));
        assert_eq!(ed.take_due(ms(8300)).as_deref(), Some("fx::dissolve(9)"));
    }

    #[test]
    fn closed_editor_never_fires_and_open_previews_at_once() {
        let t0 = Instant::now() + Duration::from_secs(1);
        let mut ed = blank();
        ed.touch(t0);
        assert_eq!(ed.take_due(t0 + DEBOUNCE), None, "closed");
        ed.open(t0);
        assert!(ed.take_due(t0).is_some(), "opening previews right away");
        key(&mut ed, KeyCode::Char('x'), t0);
        ed.close();
        assert!(!ed.pending());
    }

    #[test]
    fn ctrl_r_runs_now_and_cut_and_paste_are_reported() {
        let t0 = Instant::now();
        let mut ed = blank();
        ed.open = true;
        type_str(&mut ed, "ab", t0);
        assert_eq!(ctrl(&mut ed, 'r', t0), FxEditorOutcome::RunNow);
        assert!(!ed.pending());
        ctrl(&mut ed, 'a', t0);
        assert_eq!(ctrl(&mut ed, 'x', t0), FxEditorOutcome::Copy("ab".into()));
        assert_eq!(ed.text(), "");
        assert!(ed.pending(), "a cut is an edit");
        assert_eq!(ctrl(&mut ed, 'v', t0), FxEditorOutcome::Paste);
        ed.insert_str("fx::dissolve(1)\r\n\t.with_area(canvas)", t0);
        assert_eq!(ed.text(), "fx::dissolve(1)\n    .with_area(canvas)");
        assert!(ed.take_due(t0 + DEBOUNCE).is_some());
    }

    #[test]
    fn starter_text_compiles() {
        let t = FxTargets::uniform(Rect::new(0, 0, 20, 5), 1);
        compile(STARTER, &t).unwrap();
        assert_eq!(FxEditor::new().text(), STARTER);
    }

    fn row(buf: &ratatui::buffer::Buffer, y: u16) -> String {
        (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect()
    }

    #[test]
    fn renders_an_error_with_its_span_highlighted() {
        let src = "fx::dissolve(300)\n    .with_area(nowhere)";
        let err = compile(src, &FxTargets::default()).expect_err("must fail");
        let mut ed = blank();
        ed.set_text(src);
        ed.open = true;
        ed.set_result(Err(err.clone()));

        let theme = Theme::default();
        let mut terminal = Terminal::new(TestBackend::new(60, 20)).unwrap();
        terminal.draw(|f| ed.render(f, f.area(), &theme)).unwrap();
        let buf = terminal.backend().buffer();
        let rect = ed.rect(Rect::new(0, 0, 60, 20));
        assert_eq!(rect.y + rect.height, 19, "the status row stays free");
        assert!(
            row(buf, rect.y).contains("FX Script: error"),
            "{}",
            row(buf, rect.y)
        );

        // Text starts one row into the box, after a "N " gutter.
        let line2 = rect.y + 2;
        assert!(
            row(buf, line2).contains("2     .with_area(nowhere)"),
            "{}",
            row(buf, line2)
        );
        assert_eq!(
            buf[(rect.x + 1, line2)].fg,
            theme.general.error,
            "marked line number"
        );
        let (r, a, b) = err.span_ranges(src)[0];
        assert_eq!(r, 1);
        let x0 = rect.x + 1 + 2; // border + gutter
        for x in 0..60 {
            let cell = &buf[(x, line2)];
            let in_span = x >= x0 + a as u16 && x < x0 + b as u16;
            assert_eq!(
                cell.modifier.contains(Modifier::UNDERLINED),
                in_span,
                "x={x} {:?} span {a}..{b}",
                cell.symbol()
            );
            if in_span {
                assert_eq!(cell.fg, theme.general.error);
            }
        }
        let highlighted: String = (x0 + a as u16..x0 + b as u16)
            .map(|x| buf[(x, line2)].symbol())
            .collect();
        assert!(highlighted.contains("nowhere"), "{highlighted:?}");
        // The message rows under the text: `fx line:column: message`,
        // clipped to the box width.
        let msg = row(buf, rect.y + rect.height - 1 - MESSAGE_ROWS);
        let head: String = err.status_text().chars().take(40).collect();
        assert!(msg.contains(&head), "{msg}");
        assert!(
            msg.trim_start_matches(['│', ' ']).starts_with("fx 2:"),
            "{msg}"
        );
    }

    #[test]
    fn syntax_colours_render_and_the_error_highlight_wins() {
        let src = "fx::dissolve(300) // hi\n    .with_area(nowhere)";
        let err = compile(src, &FxTargets::default()).expect_err("must fail");
        let mut ed = blank();
        ed.set_text(src);
        ed.open = true;
        ed.set_result(Err(err.clone()));
        let theme = Theme::default();
        let g = &theme.general;
        let mut terminal = Terminal::new(TestBackend::new(60, 20)).unwrap();
        terminal.draw(|f| ed.render(f, f.area(), &theme)).unwrap();
        let buf = terminal.backend().buffer();
        let rect = ed.rect(Rect::new(0, 0, 60, 20));
        let x0 = rect.x + 1 + 2;
        let (y1, y2) = (rect.y + 1, rect.y + 2);
        let cell = |x: u16, y: u16| &buf[(x0 + x, y)];
        // fx::dissolve: effect colour, bold.
        assert_eq!(cell(0, y1).fg, g.primary);
        assert!(cell(0, y1).modifier.contains(Modifier::BOLD));
        assert_eq!(cell(4, y1).fg, g.primary, "dissolve");
        assert_eq!(cell(2, y1).fg, g.secondary, "::");
        assert_eq!(cell(13, y1).fg, g.warning, "300");
        assert_eq!(cell(18, y1).fg, g.secondary, "comment");
        assert!(cell(18, y1).modifier.contains(Modifier::ITALIC));
        // .with_area: a method, not bold.
        assert_eq!(cell(5, y2).fg, g.primary);
        assert!(!cell(5, y2).modifier.contains(Modifier::BOLD));
        // The error span keeps the error colour over the identifier's.
        let (_, a, b) = err.span_ranges(src)[0];
        for x in a as u16..b as u16 {
            assert_eq!(cell(x, y2).fg, g.error, "x={x}");
            assert!(cell(x, y2).modifier.contains(Modifier::UNDERLINED));
        }
    }

    #[test]
    fn long_lines_scroll_to_the_cursor() {
        let t = Instant::now();
        let mut ed = blank();
        ed.open = true;
        type_str(&mut ed, &"x".repeat(100), t);
        let theme = Theme::default();
        let mut terminal = Terminal::new(TestBackend::new(40, 20)).unwrap();
        terminal.draw(|f| ed.render(f, f.area(), &theme)).unwrap();
        // Inner width 38 minus the "1 " gutter: the cursor sits on the last
        // text column.
        terminal.backend_mut().assert_cursor_position(Position {
            x: 1 + 2 + 35,
            y: ed.rect(Rect::new(0, 0, 40, 20)).y + 1,
        });
    }
}
