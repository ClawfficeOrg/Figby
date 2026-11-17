//! The FX script editor (F6, View > FX Script Editor…): a multi-line DSL
//! expression with live preview (docs/fx-scripting-plan.md, phase 3).
//!
//! Every edit restarts a [`DEBOUNCE`] timer. Once the text has been idle
//! that long, [`FxEditor::take_due`] hands it back for one compile; the
//! caller binds the same targets as the F7 prompt, runs the effect in the
//! manual slot and reports the result with [`FxEditor::set_result`]. A
//! compile error is kept here and drawn inline: the span is highlighted in
//! the text and the message sits under it.
//!
//! The clock is injected: key handling and polling take `now`, so tests
//! drive the debounce with fixed instants.
//!
//! Keys follow the F7 prompt: type to edit; arrows, Home/End and
//! PageUp/PageDown move; Backspace/Delete erase; Enter breaks the line
//! (keeping the indent); Tab inserts four spaces; Ctrl+R compiles and
//! replays now; Esc closes (the text is kept for next time).

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;
use std::time::{Duration, Instant};

use super::compile::FxCompileError;
use super::prompt::TARGET_NAMES;
use crate::tui::theme::Theme;

/// Idle time after the last edit before the preview recompiles.
pub const DEBOUNCE: Duration = Duration::from_millis(300);

/// What the editor starts with the first time it opens.
pub const STARTER: &str = "// Live preview: edits recompile after a short pause.\n\
fx::sequence(&[\n    fx::coalesce((500, QuadOut)),\n    fx::glitch(300),\n])\n.with_area(canvas)";

/// Rows the editor uses for its message lines under the text.
const MESSAGE_ROWS: u16 = 2;

/// What a key did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FxEditorOutcome {
    /// Nothing changed.
    None,
    /// The text or the cursor changed; redraw.
    Changed,
    /// Ctrl+R: compile and replay now, without waiting for the debounce.
    RunNow,
    /// Esc: the editor closed.
    Close,
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
    /// Never empty: an empty text is one empty line.
    lines: Vec<String>,
    /// Cursor line (0-based).
    row: usize,
    /// Cursor position as a char index into `lines[row]`.
    col: usize,
    /// First text line shown.
    scroll_row: usize,
    /// First char shown on every line.
    scroll_col: usize,
    /// When the text last changed, while a recompile is pending.
    edited_at: Option<Instant>,
    result: Option<FxEditorResult>,
}

impl Default for FxEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl FxEditor {
    pub fn new() -> Self {
        let mut ed = Self {
            open: false,
            lines: vec![String::new()],
            row: 0,
            col: 0,
            scroll_row: 0,
            scroll_col: 0,
            edited_at: None,
            result: None,
        };
        ed.set_text(STARTER);
        ed
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
        self.lines.join("\n")
    }

    /// Replace the text; the cursor goes to the end. Does not arm the
    /// debounce (callers that want a preview call [`Self::touch`]).
    pub fn set_text(&mut self, text: &str) {
        self.lines = text
            .replace("\r\n", "\n")
            .split('\n')
            .map(String::from)
            .collect();
        self.row = self.lines.len() - 1;
        self.col = self.line_len(self.row);
        self.result = None;
    }

    /// Cursor as `(line, char)`, 0-based.
    pub fn cursor(&self) -> (usize, usize) {
        (self.row, self.col)
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

    fn line_len(&self, row: usize) -> usize {
        self.lines[row].chars().count()
    }

    fn byte_at(&self, row: usize, char_idx: usize) -> usize {
        let line = &self.lines[row];
        line.char_indices()
            .nth(char_idx)
            .map_or(line.len(), |(b, _)| b)
    }

    fn insert_char(&mut self, c: char) {
        let at = self.byte_at(self.row, self.col);
        self.lines[self.row].insert(at, c);
        self.col += 1;
    }

    fn break_line(&mut self) {
        let at = self.byte_at(self.row, self.col);
        let rest = self.lines[self.row].split_off(at);
        let indent: String = self.lines[self.row]
            .chars()
            .take_while(|c| *c == ' ')
            .collect();
        self.col = indent.chars().count();
        self.lines.insert(self.row + 1, indent + &rest);
        self.row += 1;
    }

    /// Insert pasted text at the cursor (`\r\n` and `\r` become line
    /// breaks; tabs become four spaces) and restart the debounce.
    pub fn insert_str(&mut self, text: &str, now: Instant) {
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        for c in text.chars() {
            match c {
                '\n' => {
                    // A paste keeps its own indentation.
                    let at = self.byte_at(self.row, self.col);
                    let rest = self.lines[self.row].split_off(at);
                    self.lines.insert(self.row + 1, rest);
                    self.row += 1;
                    self.col = 0;
                }
                '\t' => (0..4).for_each(|_| self.insert_char(' ')),
                c if c.is_control() => {}
                c => self.insert_char(c),
            }
        }
        self.touch(now);
    }

    pub fn handle_key(&mut self, key: KeyEvent, now: Instant) -> FxEditorOutcome {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        let len = self.line_len(self.row);
        let last = self.lines.len() - 1;
        let moved = |ed: &mut Self| {
            ed.col = ed.col.min(ed.line_len(ed.row));
            FxEditorOutcome::Changed
        };
        let edited = |ed: &mut Self| {
            ed.touch(now);
            FxEditorOutcome::Changed
        };
        match key.code {
            KeyCode::Esc => {
                self.close();
                FxEditorOutcome::Close
            }
            KeyCode::Char('r') | KeyCode::Char('R') if ctrl => {
                self.edited_at = None;
                FxEditorOutcome::RunNow
            }
            KeyCode::Char(c) if !ctrl && !alt => {
                self.insert_char(c);
                edited(self)
            }
            KeyCode::Enter => {
                self.break_line();
                edited(self)
            }
            KeyCode::Tab => {
                (0..4).for_each(|_| self.insert_char(' '));
                edited(self)
            }
            KeyCode::Backspace if self.col > 0 => {
                let at = self.byte_at(self.row, self.col - 1);
                self.lines[self.row].remove(at);
                self.col -= 1;
                edited(self)
            }
            KeyCode::Backspace if self.row > 0 => {
                let line = self.lines.remove(self.row);
                self.row -= 1;
                self.col = self.line_len(self.row);
                self.lines[self.row].push_str(&line);
                edited(self)
            }
            KeyCode::Delete if self.col < len => {
                let at = self.byte_at(self.row, self.col);
                self.lines[self.row].remove(at);
                edited(self)
            }
            KeyCode::Delete if self.row < last => {
                let next = self.lines.remove(self.row + 1);
                self.lines[self.row].push_str(&next);
                edited(self)
            }
            KeyCode::Left if self.col > 0 => {
                self.col -= 1;
                FxEditorOutcome::Changed
            }
            KeyCode::Left if self.row > 0 => {
                self.row -= 1;
                self.col = self.line_len(self.row);
                FxEditorOutcome::Changed
            }
            KeyCode::Right if self.col < len => {
                self.col += 1;
                FxEditorOutcome::Changed
            }
            KeyCode::Right if self.row < last => {
                self.row += 1;
                self.col = 0;
                FxEditorOutcome::Changed
            }
            KeyCode::Up if self.row > 0 => {
                self.row -= 1;
                moved(self)
            }
            KeyCode::Down if self.row < last => {
                self.row += 1;
                moved(self)
            }
            KeyCode::PageUp => {
                self.row = self.row.saturating_sub(10);
                moved(self)
            }
            KeyCode::PageDown => {
                self.row = (self.row + 10).min(last);
                moved(self)
            }
            KeyCode::Home => {
                self.col = 0;
                FxEditorOutcome::Changed
            }
            KeyCode::End => {
                self.col = len;
                FxEditorOutcome::Changed
            }
            _ => FxEditorOutcome::None,
        }
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

    /// Scroll so the cursor stays inside a text area of `rows` x `cols`.
    fn follow_cursor(&mut self, rows: usize, cols: usize) {
        if rows > 0 {
            if self.row < self.scroll_row {
                self.scroll_row = self.row;
            } else if self.row >= self.scroll_row + rows {
                self.scroll_row = self.row + 1 - rows;
            }
        }
        if cols > 0 {
            if self.col < self.scroll_col {
                self.scroll_col = self.col;
            } else if self.col >= self.scroll_col + cols {
                self.scroll_col = self.col + 1 - cols;
            }
        }
    }

    pub fn render(&mut self, frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
        let rect = self.rect(area);
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
                " Ctrl+R replay  Esc close  (preview after a pause) ",
                dim,
            ))
            .style(
                Style::default()
                    .bg(theme.menu.dropdown_bg)
                    .fg(theme.menu.fg),
            );
        let inner = block.inner(rect);
        frame.render_widget(block, rect);
        let text_rows = inner.height.saturating_sub(MESSAGE_ROWS) as usize;
        let gutter = self.lines.len().to_string().len() + 1;
        let text_cols = (inner.width as usize).saturating_sub(gutter + 1);
        if text_rows == 0 || text_cols == 0 {
            return;
        }
        self.follow_cursor(text_rows, text_cols);

        let text = self.text();
        let spans = self
            .error()
            .map(|e| e.span_ranges(&text))
            .unwrap_or_default();
        let hl = Style::default()
            .fg(theme.general.error)
            .add_modifier(Modifier::REVERSED | Modifier::UNDERLINED);
        let mut out = Vec::with_capacity(text_rows);
        for (i, line) in self
            .lines
            .iter()
            .enumerate()
            .skip(self.scroll_row)
            .take(text_rows)
        {
            let marked = spans.iter().find(|(r, _, _)| *r == i);
            let num_style = if marked.is_some() { err_style } else { dim };
            let mut parts = vec![Span::styled(
                format!("{:>w$} ", i + 1, w = gutter - 1),
                num_style,
            )];
            // Pad past the end so a span after the last char still shows.
            let chars: Vec<char> = line.chars().chain(std::iter::once(' ')).collect();
            let (a, b) = marked.map_or((usize::MAX, usize::MAX), |(_, a, b)| (*a, *b));
            let visible = self.scroll_col..(self.scroll_col + text_cols).min(chars.len().max(b));
            let mut run = String::new();
            let mut run_hl = false;
            for x in visible {
                let c = chars.get(x).copied().unwrap_or(' ');
                let in_span = x >= a && x < b;
                if in_span != run_hl && !run.is_empty() {
                    parts.push(Span::styled(
                        std::mem::take(&mut run),
                        if run_hl { hl } else { Style::default() },
                    ));
                }
                run_hl = in_span;
                run.push(c);
            }
            if !run.is_empty() {
                let trimmed = if run_hl {
                    run
                } else {
                    run.trim_end().to_string()
                };
                parts.push(Span::styled(
                    trimmed,
                    if run_hl { hl } else { Style::default() },
                ));
            }
            out.push(Line::from(parts));
        }
        let text_rect = Rect {
            height: text_rows as u16,
            ..inner
        };
        frame.render_widget(Paragraph::new(out), text_rect);

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
            y: inner.y + text_rows as u16,
            height: inner.height - text_rows as u16,
            ..inner
        };
        frame.render_widget(Paragraph::new(message), msg_rect);

        frame.set_cursor_position(Position {
            x: inner.x + (gutter + self.col - self.scroll_col) as u16,
            y: inner.y + (self.row - self.scroll_row) as u16,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::fx_script::compile::{compile, FxTargets};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn key(ed: &mut FxEditor, code: KeyCode, now: Instant) -> FxEditorOutcome {
        ed.handle_key(KeyEvent::new(code, KeyModifiers::NONE), now)
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
        // Cursor moves are not edits.
        key(&mut ed, KeyCode::Left, ms(6000));
        assert_eq!(ed.take_due(ms(7000)), None);
        key(&mut ed, KeyCode::Backspace, ms(7000));
        assert_eq!(ed.take_due(ms(7300)).as_deref(), Some("fx::dissolve()"));
    }

    #[test]
    fn closed_editor_never_fires_and_open_previews_at_once() {
        let t0 = Instant::now() + Duration::from_secs(1);
        let mut ed = blank();
        ed.touch(t0);
        assert_eq!(ed.take_due(t0 + DEBOUNCE), None, "closed");
        ed.open(t0);
        assert!(ed.take_due(t0).is_some(), "opening previews right away");
        ed.handle_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE), t0);
        ed.close();
        assert!(!ed.pending());
    }

    #[test]
    fn ctrl_r_runs_now_and_cancels_the_pending_debounce() {
        let t0 = Instant::now();
        let mut ed = blank();
        ed.open = true;
        type_str(&mut ed, "a", t0);
        let out = ed.handle_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL), t0);
        assert_eq!(out, FxEditorOutcome::RunNow);
        assert_eq!(ed.text(), "a");
        assert!(!ed.pending());
        let out = ed.handle_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::ALT), t0);
        assert_eq!(out, FxEditorOutcome::None);
    }

    #[test]
    fn multi_line_editing() {
        let t = Instant::now();
        let mut ed = blank();
        type_str(&mut ed, "    a(", t);
        key(&mut ed, KeyCode::Enter, t);
        assert_eq!(ed.cursor(), (1, 4), "Enter keeps the indent");
        type_str(&mut ed, "b", t);
        assert_eq!(ed.text(), "    a(\n    b");
        key(&mut ed, KeyCode::Home, t);
        key(&mut ed, KeyCode::Backspace, t);
        assert_eq!(
            ed.text(),
            "    a(    b",
            "Backspace at column 0 joins lines"
        );
        assert_eq!(ed.cursor(), (0, 6));
        key(&mut ed, KeyCode::Enter, t);
        key(&mut ed, KeyCode::Up, t);
        key(&mut ed, KeyCode::End, t);
        key(&mut ed, KeyCode::Delete, t);
        // Enter re-indented the rest; Delete at the line end joins it back.
        assert_eq!(
            ed.text(),
            "    a(        b",
            "Delete at line end joins lines"
        );
        // Left at column 0 wraps to the previous line end.
        let mut ed = blank();
        ed.set_text("ab\ncd");
        key(&mut ed, KeyCode::Home, t);
        key(&mut ed, KeyCode::Left, t);
        assert_eq!(ed.cursor(), (0, 2));
        key(&mut ed, KeyCode::Right, t);
        assert_eq!(ed.cursor(), (1, 0));
        // Up/Down clamp the column to the shorter line.
        ed.set_text("abcdef\nx");
        key(&mut ed, KeyCode::Up, t);
        assert_eq!(ed.cursor(), (0, 1));
        key(&mut ed, KeyCode::End, t);
        key(&mut ed, KeyCode::Down, t);
        assert_eq!(ed.cursor(), (1, 1));
    }

    #[test]
    fn paste_splits_lines_and_arms_the_debounce() {
        let t = Instant::now();
        let mut ed = blank();
        ed.open = true;
        ed.insert_str("fx::dissolve(1)\r\n\t.with_area(canvas)", t);
        assert_eq!(ed.text(), "fx::dissolve(1)\n    .with_area(canvas)");
        assert!(ed.take_due(t + DEBOUNCE).is_some());
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
        let (r, a, b) = err.span_ranges(src)[0];
        assert_eq!(r, 1);
        let x0 = rect.x + 1 + 2; // border + gutter
        for x in 0..60 {
            let cell = &buf[(x, line2)];
            let in_span = x >= x0 + a as u16 && x < x0 + b as u16;
            assert_eq!(
                cell.modifier.contains(Modifier::REVERSED),
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
        // The message rows under the text: `fx line:column: message`.
        let msg = row(buf, rect.y + rect.height - 1 - MESSAGE_ROWS);
        // Clipped to the box width.
        let head: String = err.status_text().chars().take(40).collect();
        assert!(msg.contains(&head), "{msg}");
        assert!(
            msg.trim_start_matches(['│', ' ']).starts_with("fx 2:"),
            "{msg}"
        );
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
        // Inner width 38, gutter "1 ", one column kept free at the right:
        // the cursor sits on the last text column.
        terminal.backend_mut().assert_cursor_position(Position {
            x: 1 + 2 + 34,
            y: ed.rect(Rect::new(0, 0, 40, 20)).y + 1,
        });
    }
}
