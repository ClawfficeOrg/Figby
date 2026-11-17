//! A reusable multi-line text area for ratatui.
//!
//! Self-contained on purpose: it depends only on ratatui (and its crossterm
//! events) and `unicode-width` (which ratatui uses too), with no Figby
//! types, so another app can lift the file as is.
//!
//! - **Model.** Lines of text with a cursor and an optional selection
//!   anchor. Every position is `(row, col)` in *chars*, never bytes, so
//!   non-ASCII text edits and highlights correctly.
//! - **Display width.** Layout counts terminal cells: a wide CJK or emoji
//!   char takes two ([`cell_width`]). Horizontal scroll, the cursor cell,
//!   mouse hits and highlights all use display columns, and a wide char is
//!   never split at the viewport edge (the visible half is drawn blank).
//! - **Editing.** Insert, Enter (optional auto-indent), Backspace and Delete
//!   across line breaks, Tab as spaces, paste, selection replace.
//! - **Undo.** Snapshot undo and redo; runs of typed chars coalesce into
//!   one step.
//! - **Clipboard.** The widget owns none. Copy and cut return the text in
//!   [`TextAreaEvent::Copy`] / [`TextAreaEvent::Cut`]; Ctrl+V returns
//!   [`TextAreaEvent::PasteRequested`] and the host answers with
//!   [`TextArea::insert_str`]. That lets the host route through whatever
//!   clipboard it already has.
//! - **View.** Vertical and horizontal scroll follow the cursor; optional
//!   line numbers; host-supplied [`Highlight`] spans (diagnostics, search
//!   hits); the cursor cell is drawn with its own style, and
//!   [`TextArea::render_frame`] also places the terminal cursor. Mouse
//!   click places the cursor, drag selects, the wheel scrolls.
//!
//! Keys: arrows, Home/End, PageUp/PageDown, Ctrl+Home/End, Ctrl+Left/Right
//! (by word) move, and Shift extends the selection; Ctrl+Backspace and
//! Ctrl+Delete delete a word; Ctrl+A selects all; Ctrl+C/X/V copy, cut,
//! paste; Ctrl+Z undo; Ctrl+Y or Ctrl+Shift+Z redo. A word is a run of
//! alphanumerics and `_`, or a run of other non-space chars.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::Frame;
use unicode_width::UnicodeWidthChar;

/// A text position: `(row, col)`, 0-based, col in chars.
pub type TextPos = (usize, usize);

/// How many undo steps are kept.
const UNDO_LIMIT: usize = 200;

/// Lines the mouse wheel moves.
const WHEEL_LINES: usize = 3;

/// What an input did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextAreaEvent {
    /// Not handled; the host may use it.
    Ignored,
    /// Cursor, selection or scroll changed, the text did not.
    Moved,
    /// The text changed.
    Edited,
    /// Ctrl+C: put this text on the clipboard.
    Copy(String),
    /// Ctrl+X: this text was removed; put it on the clipboard.
    Cut(String),
    /// Ctrl+V: call [`TextArea::insert_str`] with the clipboard text.
    PasteRequested,
}

impl TextAreaEvent {
    /// Whether the text changed.
    pub fn is_edit(&self) -> bool {
        matches!(self, Self::Edited | Self::Cut(_))
    }
}

/// A span the host wants marked: `row`, chars `start..end`. A span may
/// extend past the line end (one cell is drawn there).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Highlight {
    pub row: usize,
    pub start: usize,
    pub end: usize,
}

/// Styles for [`TextArea::render`]. Highlight, selection and cursor are
/// patched over `text` in that order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextAreaStyle {
    pub text: Style,
    pub cursor: Style,
    pub selection: Style,
    pub highlight: Style,
    pub line_number: Style,
    /// Line number of a row that has a highlight.
    pub line_number_marked: Style,
}

impl Default for TextAreaStyle {
    fn default() -> Self {
        Self {
            text: Style::default(),
            cursor: Style::default().add_modifier(Modifier::REVERSED),
            selection: Style::default().add_modifier(Modifier::REVERSED | Modifier::DIM),
            highlight: Style::default().add_modifier(Modifier::UNDERLINED),
            line_number: Style::default().add_modifier(Modifier::DIM),
            line_number_marked: Style::default().add_modifier(Modifier::BOLD),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Snapshot {
    lines: Vec<String>,
    cursor: TextPos,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditKind {
    None,
    Typing,
    Other,
}

#[derive(Debug, Clone)]
pub struct TextArea {
    /// Never empty: an empty text is one empty line.
    lines: Vec<String>,
    cursor: TextPos,
    /// Selection start; the selection runs from here to the cursor.
    anchor: Option<TextPos>,
    /// First row and first display column shown.
    scroll: TextPos,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    last_edit: EditKind,
    highlights: Vec<Highlight>,
    /// A left-button drag is selecting.
    dragging: bool,
    /// Display column Up/Down aim for (see `move_vertical`).
    goal: Option<usize>,
    /// Where the text (not the gutter) was drawn last; mouse hit-testing.
    text_rect: Rect,
    pub line_numbers: bool,
    /// Spaces a Tab inserts.
    pub tab_width: usize,
    /// Enter copies the current line's leading spaces.
    pub auto_indent: bool,
}

impl Default for TextArea {
    fn default() -> Self {
        Self::new()
    }
}

fn char_len(s: &str) -> usize {
    s.chars().count()
}

/// Byte offset of char `ci` in `s` (the end if past it).
fn byte_at(s: &str, ci: usize) -> usize {
    s.char_indices().nth(ci).map_or(s.len(), |(b, _)| b)
}

/// Word classes for word-wise movement: 0 space, 1 alphanumeric or `_`,
/// 2 any other char (punctuation runs are words of their own).
fn word_class(c: char) -> u8 {
    if c.is_whitespace() {
        0
    } else if c.is_alphanumeric() || c == '_' {
        1
    } else {
        2
    }
}

/// Terminal cells `c` takes: 2 for wide CJK and emoji, else 1. Zero-width
/// and control chars count as 1 so every char keeps a cell of its own.
pub fn cell_width(c: char) -> usize {
    c.width().unwrap_or(1).max(1)
}

/// Display column where char `ci` of `s` starts (the sum of the widths of
/// the chars before it).
pub fn display_col(s: &str, ci: usize) -> usize {
    s.chars().take(ci).map(cell_width).sum()
}

/// The char of `s` drawn at display column `x` (a wide char owns both its
/// cells); the line length when `x` is past the end.
pub fn char_at_display(s: &str, x: usize) -> usize {
    let mut acc = 0;
    for (i, c) in s.chars().enumerate() {
        let w = cell_width(c);
        if x < acc + w {
            return i;
        }
        acc += w;
    }
    char_len(s)
}

impl TextArea {
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
            cursor: (0, 0),
            anchor: None,
            scroll: (0, 0),
            undo: Vec::new(),
            redo: Vec::new(),
            last_edit: EditKind::None,
            highlights: Vec::new(),
            dragging: false,
            goal: None,
            text_rect: Rect::default(),
            line_numbers: false,
            tab_width: 4,
            auto_indent: true,
        }
    }

    pub fn from_text(text: &str) -> Self {
        let mut t = Self::new();
        t.set_text(text);
        t
    }

    // ---- model -------------------------------------------------------

    /// The whole text, lines joined with `\n`.
    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    /// Replace the text. Clears undo history and selection; the cursor
    /// goes to the end.
    pub fn set_text(&mut self, text: &str) {
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        self.lines = text.split('\n').map(String::from).collect();
        let last = self.lines.len() - 1;
        self.cursor = (last, char_len(&self.lines[last]));
        self.anchor = None;
        self.undo.clear();
        self.redo.clear();
        self.last_edit = EditKind::None;
        self.goal = None;
        self.scroll = (0, 0);
    }

    pub fn cursor(&self) -> TextPos {
        self.cursor
    }

    fn clamp(&self, (row, col): TextPos) -> TextPos {
        let row = row.min(self.lines.len() - 1);
        (row, col.min(char_len(&self.lines[row])))
    }

    /// Move the cursor (clamped) and drop the selection.
    pub fn set_cursor(&mut self, pos: TextPos) {
        self.cursor = self.clamp(pos);
        self.anchor = None;
        self.last_edit = EditKind::None;
        self.goal = None;
    }

    /// The selection as ordered `(start, end)`, end exclusive; `None` when
    /// nothing is selected.
    pub fn selection(&self) -> Option<(TextPos, TextPos)> {
        let a = self.anchor?;
        if a == self.cursor {
            return None;
        }
        Some(if a < self.cursor {
            (a, self.cursor)
        } else {
            (self.cursor, a)
        })
    }

    /// Select from `from` to `to` (the cursor ends at `to`).
    pub fn select(&mut self, from: TextPos, to: TextPos) {
        self.anchor = Some(self.clamp(from));
        self.cursor = self.clamp(to);
        self.last_edit = EditKind::None;
        self.goal = None;
    }

    pub fn select_all(&mut self) {
        let last = self.lines.len() - 1;
        self.select((0, 0), (last, char_len(&self.lines[last])));
    }

    pub fn clear_selection(&mut self) {
        self.anchor = None;
    }

    /// The text between two ordered positions.
    fn text_between(&self, (a, b): (TextPos, TextPos)) -> String {
        if a.0 == b.0 {
            return self.lines[a.0].chars().skip(a.1).take(b.1 - a.1).collect();
        }
        let mut out: String = self.lines[a.0].chars().skip(a.1).collect();
        for line in &self.lines[a.0 + 1..b.0] {
            out.push('\n');
            out.push_str(line);
        }
        out.push('\n');
        out.extend(self.lines[b.0].chars().take(b.1));
        out
    }

    pub fn selected_text(&self) -> Option<String> {
        self.selection().map(|s| self.text_between(s))
    }

    /// Record an undo step before an edit of `kind`.
    fn begin_edit(&mut self, kind: EditKind) {
        let coalesce = kind == EditKind::Typing && self.last_edit == EditKind::Typing;
        if !coalesce {
            self.undo.push(Snapshot {
                lines: self.lines.clone(),
                cursor: self.cursor,
            });
            if self.undo.len() > UNDO_LIMIT {
                self.undo.remove(0);
            }
        }
        self.redo.clear();
        self.last_edit = kind;
        self.goal = None;
    }

    /// Remove the selected text without recording undo.
    fn remove_selection(&mut self) -> Option<String> {
        let (a, b) = self.selection()?;
        let removed = self.text_between((a, b));
        let tail: String = self.lines[b.0].chars().skip(b.1).collect();
        let head = byte_at(&self.lines[a.0], a.1);
        self.lines[a.0].truncate(head);
        self.lines[a.0].push_str(&tail);
        self.lines.drain(a.0 + 1..=b.0);
        self.cursor = a;
        self.anchor = None;
        Some(removed)
    }

    /// Delete the selection (one undo step). Returns the removed text.
    pub fn delete_selection(&mut self) -> Option<String> {
        self.selection()?;
        self.begin_edit(EditKind::Other);
        self.remove_selection()
    }

    /// Insert raw text at the cursor (no undo, no conversion).
    fn insert_raw(&mut self, text: &str) {
        let (row, col) = self.cursor;
        let at = byte_at(&self.lines[row], col);
        let rest = self.lines[row].split_off(at);
        let mut parts = text.split('\n');
        let first = parts.next().unwrap_or_default();
        self.lines[row].push_str(first);
        let mut r = row;
        for part in parts {
            r += 1;
            self.lines.insert(r, part.to_string());
        }
        let col = char_len(&self.lines[r]);
        self.lines[r].push_str(&rest);
        self.cursor = (r, col);
    }

    /// Insert text at the cursor, replacing any selection (one undo step).
    /// `\r\n` and `\r` become line breaks, Tab becomes spaces, other
    /// control chars are dropped.
    pub fn insert_str(&mut self, text: &str) {
        let tab = " ".repeat(self.tab_width);
        let clean: String = text
            .replace("\r\n", "\n")
            .replace('\r', "\n")
            .chars()
            .filter(|c| *c == '\n' || *c == '\t' || !c.is_control())
            .collect::<String>()
            .replace('\t', &tab);
        self.begin_edit(EditKind::Other);
        self.remove_selection();
        self.insert_raw(&clean);
    }

    /// Type one char (consecutive typing is one undo step).
    pub fn insert_char(&mut self, c: char) {
        if c == '\n' {
            self.newline();
            return;
        }
        if c.is_control() {
            return;
        }
        self.begin_edit(EditKind::Typing);
        self.remove_selection();
        let (row, col) = self.cursor;
        let at = byte_at(&self.lines[row], col);
        self.lines[row].insert(at, c);
        self.cursor.1 += 1;
    }

    /// Break the line at the cursor, keeping the indent if `auto_indent`.
    pub fn newline(&mut self) {
        self.begin_edit(EditKind::Other);
        self.remove_selection();
        let indent: String = if self.auto_indent {
            let line = &self.lines[self.cursor.0];
            line.chars()
                .take(self.cursor.1)
                .take_while(|c| *c == ' ')
                .collect()
        } else {
            String::new()
        };
        self.insert_raw(&format!("\n{indent}"));
    }

    /// Erase the selection, or the char before the cursor (joining with the
    /// previous line at column 0). Returns whether anything changed.
    pub fn backspace(&mut self) -> bool {
        if self.selection().is_some() {
            return self.delete_selection().is_some();
        }
        let (row, col) = self.cursor;
        if col > 0 {
            self.begin_edit(EditKind::Other);
            let at = byte_at(&self.lines[row], col - 1);
            self.lines[row].remove(at);
            self.cursor.1 -= 1;
        } else if row > 0 {
            self.begin_edit(EditKind::Other);
            let line = self.lines.remove(row);
            let prev = char_len(&self.lines[row - 1]);
            self.lines[row - 1].push_str(&line);
            self.cursor = (row - 1, prev);
        } else {
            return false;
        }
        true
    }

    /// Erase the selection, or the char under the cursor (joining the next
    /// line at the line end). Returns whether anything changed.
    pub fn delete(&mut self) -> bool {
        if self.selection().is_some() {
            return self.delete_selection().is_some();
        }
        let (row, col) = self.cursor;
        if col < char_len(&self.lines[row]) {
            self.begin_edit(EditKind::Other);
            let at = byte_at(&self.lines[row], col);
            self.lines[row].remove(at);
        } else if row + 1 < self.lines.len() {
            self.begin_edit(EditKind::Other);
            let next = self.lines.remove(row + 1);
            self.lines[row].push_str(&next);
        } else {
            return false;
        }
        true
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            lines: self.lines.clone(),
            cursor: self.cursor,
        }
    }

    fn restore(&mut self, s: Snapshot) {
        self.lines = s.lines;
        self.cursor = self.clamp(s.cursor);
        self.anchor = None;
        self.last_edit = EditKind::None;
        self.goal = None;
    }

    pub fn undo(&mut self) -> bool {
        let Some(s) = self.undo.pop() else {
            return false;
        };
        self.redo.push(self.snapshot());
        self.restore(s);
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(s) = self.redo.pop() else {
            return false;
        };
        self.undo.push(self.snapshot());
        self.restore(s);
        true
    }

    /// Replace the highlighted spans (drawn with `TextAreaStyle::highlight`).
    pub fn set_highlights(&mut self, highlights: Vec<Highlight>) {
        self.highlights = highlights;
    }

    pub fn highlights(&self) -> &[Highlight] {
        &self.highlights
    }

    /// Where Ctrl+Right lands from `pos`: past any spaces, then past one
    /// word (an alphanumeric/underscore run, or a run of other non-space
    /// chars). At a line end it moves to the next line start.
    pub fn word_right(&self, (row, col): TextPos) -> TextPos {
        let chars: Vec<char> = self.lines[row].chars().collect();
        if col >= chars.len() {
            return if row + 1 < self.lines.len() {
                (row + 1, 0)
            } else {
                (row, chars.len())
            };
        }
        let mut i = col;
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        if let Some(&c) = chars.get(i) {
            let class = word_class(c);
            while i < chars.len() && word_class(chars[i]) == class {
                i += 1;
            }
        }
        (row, i)
    }

    /// Where Ctrl+Left lands from `pos`: back over any spaces, then to the
    /// start of the word before. At a line start it moves to the previous
    /// line end.
    pub fn word_left(&self, (row, col): TextPos) -> TextPos {
        if col == 0 {
            return if row > 0 {
                (row - 1, char_len(&self.lines[row - 1]))
            } else {
                (0, 0)
            };
        }
        let chars: Vec<char> = self.lines[row].chars().collect();
        let mut i = col.min(chars.len());
        while i > 0 && chars[i - 1].is_whitespace() {
            i -= 1;
        }
        if i > 0 {
            let class = word_class(chars[i - 1]);
            while i > 0 && word_class(chars[i - 1]) == class {
                i -= 1;
            }
        }
        (row, i)
    }

    /// Delete from the cursor to `to` (either side), one undo step.
    fn delete_to(&mut self, to: TextPos) -> bool {
        if to == self.cursor {
            return false;
        }
        self.anchor = Some(to);
        self.delete_selection().is_some()
    }

    /// Ctrl+Backspace: the selection, or back to the previous word start.
    pub fn delete_word_left(&mut self) -> bool {
        if self.selection().is_some() {
            return self.delete_selection().is_some();
        }
        self.delete_to(self.word_left(self.cursor))
    }

    /// Ctrl+Delete: the selection, or up to the next word end.
    pub fn delete_word_right(&mut self) -> bool {
        if self.selection().is_some() {
            return self.delete_selection().is_some();
        }
        self.delete_to(self.word_right(self.cursor))
    }

    // ---- input -------------------------------------------------------

    /// Move the cursor to `to`, extending the selection when `extend`.
    fn move_to(&mut self, to: TextPos, extend: bool) -> TextAreaEvent {
        if extend {
            self.anchor.get_or_insert(self.cursor);
        } else {
            self.anchor = None;
        }
        self.cursor = self.clamp(to);
        self.last_edit = EditKind::None;
        self.goal = None;
        TextAreaEvent::Moved
    }

    /// Move to `row`, keeping the goal column: the display column the
    /// cursor had when this run of vertical moves began, so passing a
    /// shorter line does not lose it. Any other move or edit resets it.
    fn move_vertical(&mut self, row: usize, extend: bool) -> TextAreaEvent {
        let (r, c) = self.cursor;
        let goal = self.goal.unwrap_or_else(|| display_col(&self.lines[r], c));
        let row = row.min(self.lines.len() - 1);
        let col = char_at_display(&self.lines[row], goal);
        let ev = self.move_to((row, col), extend);
        self.goal = Some(goal);
        ev
    }

    /// The goal column of the current run of vertical moves, if any.
    pub fn goal_column(&self) -> Option<usize> {
        self.goal
    }

    /// Rows PageUp/PageDown move: the rendered height, or 10 before the
    /// first render.
    fn page(&self) -> usize {
        match self.text_rect.height {
            0 => 10,
            h => h as usize,
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> TextAreaEvent {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
        let (row, col) = self.cursor;
        let len = char_len(&self.lines[row]);
        let last = self.lines.len() - 1;
        let edited = |changed: bool| {
            if changed {
                TextAreaEvent::Edited
            } else {
                TextAreaEvent::Ignored
            }
        };
        match key.code {
            KeyCode::Char(c) if ctrl && !alt => match c.to_ascii_lowercase() {
                'a' => {
                    self.select_all();
                    TextAreaEvent::Moved
                }
                'c' => self
                    .selected_text()
                    .map_or(TextAreaEvent::Ignored, TextAreaEvent::Copy),
                'x' => self
                    .delete_selection()
                    .map_or(TextAreaEvent::Ignored, TextAreaEvent::Cut),
                'v' => TextAreaEvent::PasteRequested,
                'z' if shift => edited(self.redo()),
                'z' => edited(self.undo()),
                'y' => edited(self.redo()),
                _ => TextAreaEvent::Ignored,
            },
            KeyCode::Char(c) if !ctrl && !alt => {
                self.insert_char(c);
                TextAreaEvent::Edited
            }
            KeyCode::Enter if !ctrl && !alt => {
                self.newline();
                TextAreaEvent::Edited
            }
            KeyCode::Tab if !ctrl && !alt => {
                self.insert_str("\t");
                TextAreaEvent::Edited
            }
            KeyCode::Backspace if ctrl => edited(self.delete_word_left()),
            KeyCode::Delete if ctrl => edited(self.delete_word_right()),
            KeyCode::Backspace => edited(self.backspace()),
            KeyCode::Delete => edited(self.delete()),
            KeyCode::Left if ctrl => self.move_to(self.word_left(self.cursor), shift),
            KeyCode::Right if ctrl => self.move_to(self.word_right(self.cursor), shift),
            KeyCode::Left => {
                if !shift && self.selection().is_some() {
                    let (a, _) = self.selection().unwrap_or_default();
                    return self.move_to(a, false);
                }
                let to = if col > 0 {
                    (row, col - 1)
                } else if row > 0 {
                    (row - 1, char_len(&self.lines[row - 1]))
                } else {
                    (row, col)
                };
                self.move_to(to, shift)
            }
            KeyCode::Right => {
                if !shift && self.selection().is_some() {
                    let (_, b) = self.selection().unwrap_or_default();
                    return self.move_to(b, false);
                }
                let to = if col < len {
                    (row, col + 1)
                } else if row < last {
                    (row + 1, 0)
                } else {
                    (row, col)
                };
                self.move_to(to, shift)
            }
            KeyCode::Up if row == 0 => self.move_to((0, 0), shift),
            KeyCode::Up => self.move_vertical(row - 1, shift),
            KeyCode::Down if row == last => self.move_to((row, len), shift),
            KeyCode::Down => self.move_vertical(row + 1, shift),
            KeyCode::PageUp => self.move_vertical(row.saturating_sub(self.page()), shift),
            KeyCode::PageDown => self.move_vertical((row + self.page()).min(last), shift),
            KeyCode::Home if ctrl => self.move_to((0, 0), shift),
            KeyCode::End if ctrl => self.move_to((last, usize::MAX), shift),
            KeyCode::Home => self.move_to((row, 0), shift),
            KeyCode::End => self.move_to((row, len), shift),
            _ => TextAreaEvent::Ignored,
        }
    }

    /// The text position drawn at screen cell `(x, y)` in the last render,
    /// clamped to the text. `None` outside the text rect.
    pub fn pos_at(&self, x: u16, y: u16) -> Option<TextPos> {
        let r = self.text_rect;
        if !r.contains(Position { x, y }) {
            return None;
        }
        Some(self.pos_at_clamped(x, y))
    }

    fn pos_at_clamped(&self, x: u16, y: u16) -> TextPos {
        let r = self.text_rect;
        let dy = y
            .clamp(r.y, (r.y + r.height).saturating_sub(1))
            .saturating_sub(r.y);
        let dx = x.clamp(r.x, r.x + r.width).saturating_sub(r.x);
        let row = (self.scroll.0 + dy as usize).min(self.lines.len() - 1);
        let col = char_at_display(&self.lines[row], self.scroll.1 + dx as usize);
        (row, col)
    }

    /// Left click places the cursor (Shift+click extends), drag selects,
    /// the wheel moves three lines. Needs a previous render for the layout.
    pub fn handle_mouse(&mut self, mouse: MouseEvent) -> TextAreaEvent {
        let shift = mouse.modifiers.contains(KeyModifiers::SHIFT);
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let Some(pos) = self.pos_at(mouse.column, mouse.row) else {
                    return TextAreaEvent::Ignored;
                };
                self.dragging = true;
                if shift {
                    return self.move_to(pos, true);
                }
                self.move_to(pos, false);
                self.anchor = Some(pos);
                TextAreaEvent::Moved
            }
            MouseEventKind::Drag(MouseButton::Left) if self.dragging => {
                let pos = self.pos_at_clamped(mouse.column, mouse.row);
                self.cursor = pos;
                self.goal = None;
                TextAreaEvent::Moved
            }
            MouseEventKind::Up(MouseButton::Left) if self.dragging => {
                self.dragging = false;
                TextAreaEvent::Moved
            }
            MouseEventKind::ScrollUp => {
                let (row, col) = self.cursor;
                self.move_to((row.saturating_sub(WHEEL_LINES), col), false)
            }
            MouseEventKind::ScrollDown => {
                let (row, col) = self.cursor;
                self.move_to((row + WHEEL_LINES, col), false)
            }
            _ => TextAreaEvent::Ignored,
        }
    }

    // ---- view --------------------------------------------------------

    /// First row and first display column shown (as of the last render).
    pub fn scroll(&self) -> TextPos {
        self.scroll
    }

    fn gutter_width(&self) -> u16 {
        if self.line_numbers {
            self.lines.len().to_string().len() as u16 + 1
        } else {
            0
        }
    }

    /// Where the cursor was drawn by the last render, if it was visible.
    pub fn cursor_screen_position(&self) -> Option<Position> {
        let r = self.text_rect;
        let (row, col) = self.cursor;
        let dy = row.checked_sub(self.scroll.0)?;
        let dx = display_col(&self.lines[row], col).checked_sub(self.scroll.1)?;
        (dy < r.height as usize && dx < r.width as usize).then(|| Position {
            x: r.x + dx as u16,
            y: r.y + dy as u16,
        })
    }

    /// Whether `(row, col)` is selected. `col == line len` is the line
    /// break, selected when the selection continues onto the next row.
    fn selected(sel: Option<(TextPos, TextPos)>, row: usize, col: usize) -> bool {
        sel.is_some_and(|(a, b)| (row, col) >= a && (row, col) < b)
    }

    /// Draw into `buf` at `area`, scrolling so the cursor is visible.
    pub fn render(&mut self, area: Rect, buf: &mut Buffer, style: &TextAreaStyle) {
        let gutter = self.gutter_width().min(area.width);
        self.text_rect = Rect {
            x: area.x + gutter,
            width: area.width - gutter,
            ..area
        };
        let (rows, cols) = (
            self.text_rect.height as usize,
            self.text_rect.width as usize,
        );
        if rows == 0 || cols == 0 {
            return;
        }
        // Follow the cursor, in display columns. The cursor cell is as wide
        // as the char under it (one cell at the line end).
        let (crow, ccol) = self.cursor;
        if crow < self.scroll.0 {
            self.scroll.0 = crow;
        } else if crow >= self.scroll.0 + rows {
            self.scroll.0 = crow + 1 - rows;
        }
        let cline = &self.lines[crow];
        let cx = display_col(cline, ccol);
        let cw = cline.chars().nth(ccol).map_or(1, cell_width);
        if cx < self.scroll.1 {
            self.scroll.1 = cx;
        } else if cx + cw > self.scroll.1 + cols {
            self.scroll.1 = (cx + cw).saturating_sub(cols);
        }
        self.scroll.0 = self.scroll.0.min(self.lines.len() - 1);

        let sel = self.selection();
        let (sx, x0) = (self.scroll.1, self.text_rect.x);
        for dy in 0..rows {
            let y = area.y + dy as u16;
            let row = self.scroll.0 + dy;
            let line = self.lines.get(row);
            if gutter > 0 {
                let label = match line {
                    Some(_) => format!("{:>w$} ", row + 1, w = gutter as usize - 1),
                    None => " ".repeat(gutter as usize),
                };
                let marked = self.highlights.iter().any(|h| h.row == row);
                let st = if marked {
                    style.line_number_marked
                } else {
                    style.line_number
                };
                buf.set_stringn(area.x, y, &label, gutter as usize, st);
            }
            for dx in 0..cols {
                buf[(x0 + dx as u16, y)].set_char(' ').set_style(style.text);
            }
            let Some(line) = line else {
                continue;
            };
            let chars: Vec<char> = line.chars().collect();
            // Walk the chars, then one-cell blanks past the end (for the
            // cursor, the selected line break and highlights past the end),
            // until the viewport is full.
            let mut disp = 0;
            for i in 0.. {
                if disp >= sx + cols {
                    break;
                }
                let (c, w) = chars.get(i).map_or((' ', 1), |&c| (c, cell_width(c)));
                let st = self.cell_style(style, sel, row, i, chars.len());
                if disp >= sx && disp + w <= sx + cols {
                    let x = x0 + (disp - sx) as u16;
                    buf[(x, y)].set_char(c).set_style(st);
                    for k in 1..w as u16 {
                        // Covered by the wide char, as ratatui's own
                        // set_stringn leaves them.
                        buf[(x + k, y)].reset();
                        buf[(x + k, y)].set_style(st);
                    }
                } else if disp + w > sx {
                    // A wide char cut by the viewport edge: never draw half
                    // of it, blank the visible cells instead.
                    for cell in disp.max(sx)..(disp + w).min(sx + cols) {
                        buf[(x0 + (cell - sx) as u16, y)]
                            .set_char(' ')
                            .set_style(st);
                    }
                }
                disp += w;
            }
        }
    }

    /// The style of char `col` on `row`: text, then highlight, selection
    /// and cursor patched over it.
    fn cell_style(
        &self,
        style: &TextAreaStyle,
        sel: Option<(TextPos, TextPos)>,
        row: usize,
        col: usize,
        len: usize,
    ) -> Style {
        let mut st = style.text;
        if self
            .highlights
            .iter()
            .any(|h| h.row == row && col >= h.start && col < h.end.max(h.start + 1))
        {
            st = st.patch(style.highlight);
        }
        if col <= len && Self::selected(sel, row, col) {
            st = st.patch(style.selection);
        }
        if (row, col) == self.cursor {
            st = st.patch(style.cursor);
        }
        st
    }

    /// [`Self::render`] into the frame, and put the terminal cursor on the
    /// cursor cell.
    pub fn render_frame(&mut self, frame: &mut Frame<'_>, area: Rect, style: &TextAreaStyle) {
        self.render(area, frame.buffer_mut(), style);
        if let Some(pos) = self.cursor_screen_position() {
            frame.set_cursor_position(pos);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::style::Color;
    use ratatui::Terminal;

    fn k(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    fn shift(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::SHIFT)
    }

    fn typed(t: &mut TextArea, s: &str) {
        for c in s.chars() {
            t.handle_key(k(KeyCode::Char(c)));
        }
    }

    // ---- edit model ----

    #[test]
    fn insert_and_newline_with_auto_indent() {
        let mut t = TextArea::new();
        typed(&mut t, "  ab");
        t.handle_key(k(KeyCode::Left));
        t.handle_key(k(KeyCode::Enter));
        assert_eq!(t.text(), "  a\n  b");
        assert_eq!(t.cursor(), (1, 2));
        t.auto_indent = false;
        t.handle_key(k(KeyCode::Enter));
        assert_eq!(t.text(), "  a\n  \nb");
        assert_eq!(t.cursor(), (2, 0));
    }

    #[test]
    fn backspace_and_delete_join_lines() {
        let mut t = TextArea::from_text("ab\ncd");
        t.set_cursor((1, 0));
        assert!(t.backspace());
        assert_eq!((t.text().as_str(), t.cursor()), ("abcd", (0, 2)));
        t.set_cursor((0, 1));
        t.insert_str("x\ny");
        assert_eq!(t.text(), "ax\nybcd");
        t.set_cursor((0, 2));
        assert!(t.delete());
        assert_eq!(t.text(), "axybcd");
        t.set_cursor((0, 6));
        assert!(!t.delete(), "nothing after the last char");
        t.set_cursor((0, 0));
        assert!(!t.backspace(), "nothing before the first char");
    }

    #[test]
    fn indexing_is_by_char_not_byte() {
        let mut t = TextArea::from_text("é…ü");
        t.set_cursor((0, 2));
        typed(&mut t, "x");
        assert_eq!(t.text(), "é…xü");
        assert!(t.backspace() && t.backspace());
        assert_eq!(t.text(), "éü");
        t.select((0, 0), (0, 1));
        assert_eq!(t.selected_text().as_deref(), Some("é"));
        t.handle_key(k(KeyCode::End));
        assert_eq!(t.cursor(), (0, 2));
    }

    #[test]
    fn selection_text_delete_and_replace() {
        let mut t = TextArea::from_text("one\ntwo\nthree");
        t.select((0, 1), (2, 2));
        assert_eq!(t.selected_text().as_deref(), Some("ne\ntwo\nth"));
        // Reversed anchor gives the same ordered selection.
        t.select((2, 2), (0, 1));
        assert_eq!(t.selection(), Some(((0, 1), (2, 2))));
        typed(&mut t, "X");
        assert_eq!(t.text(), "oXree");
        assert_eq!(t.cursor(), (0, 2));
        t.select_all();
        assert_eq!(t.delete_selection().as_deref(), Some("oXree"));
        assert_eq!(t.text(), "");
        assert_eq!(t.selection(), None);
    }

    #[test]
    fn undo_redo_and_typing_coalesces() {
        let mut t = TextArea::new();
        typed(&mut t, "abc");
        t.handle_key(k(KeyCode::Enter));
        typed(&mut t, "de");
        assert_eq!(t.text(), "abc\nde");
        assert_eq!(t.handle_key(ctrl('z')), TextAreaEvent::Edited);
        assert_eq!(t.text(), "abc\n", "one step undoes the typed run");
        t.handle_key(ctrl('z'));
        assert_eq!(t.text(), "abc");
        t.handle_key(ctrl('z'));
        assert_eq!(t.text(), "");
        assert_eq!(t.handle_key(ctrl('z')), TextAreaEvent::Ignored);
        t.handle_key(ctrl('y'));
        assert_eq!(t.text(), "abc");
        t.handle_key(KeyEvent::new(
            KeyCode::Char('Z'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        ));
        assert_eq!(t.text(), "abc\n");
        // A new edit clears redo.
        typed(&mut t, "q");
        assert!(!t.can_redo());
        // Moving the cursor breaks a typing run.
        let mut t = TextArea::new();
        typed(&mut t, "ab");
        t.handle_key(k(KeyCode::Left));
        typed(&mut t, "c");
        t.undo();
        assert_eq!(t.text(), "ab");
    }

    #[test]
    fn set_text_resets_history() {
        let mut t = TextArea::new();
        typed(&mut t, "x");
        t.set_text("a\r\nb");
        assert_eq!(t.lines(), ["a", "b"]);
        assert!(!t.can_undo());
    }

    // ---- keys ----

    #[test]
    fn movement_keys() {
        let mut t = TextArea::from_text("abcdef\nx\nlast line");
        t.set_cursor((0, 5));
        t.handle_key(k(KeyCode::Down));
        assert_eq!(t.cursor(), (1, 1), "clamped to the shorter line");
        t.handle_key(k(KeyCode::Right));
        assert_eq!(t.cursor(), (2, 0), "wraps to the next line");
        t.handle_key(k(KeyCode::Left));
        assert_eq!(t.cursor(), (1, 1), "wraps back");
        t.handle_key(k(KeyCode::Home));
        assert_eq!(t.cursor(), (1, 0));
        t.handle_key(k(KeyCode::End));
        assert_eq!(t.cursor(), (1, 1));
        t.handle_key(k(KeyCode::PageDown));
        assert_eq!(t.cursor().0, 2);
        t.handle_key(k(KeyCode::PageUp));
        assert_eq!(t.cursor().0, 0);
        t.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::CONTROL));
        assert_eq!(t.cursor(), (2, 9));
        t.handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::CONTROL));
        assert_eq!(t.cursor(), (0, 0));
        assert_eq!(t.handle_key(k(KeyCode::F(3))), TextAreaEvent::Ignored);
    }

    #[test]
    fn page_keys_use_the_rendered_height() {
        let mut t = TextArea::from_text(&"x\n".repeat(30));
        t.set_cursor((0, 0));
        let mut buf = Buffer::empty(Rect::new(0, 0, 10, 5));
        t.render(Rect::new(0, 0, 10, 5), &mut buf, &TextAreaStyle::default());
        t.handle_key(k(KeyCode::PageDown));
        assert_eq!(t.cursor().0, 5);
    }

    #[test]
    fn shift_extends_and_plain_arrow_collapses() {
        let mut t = TextArea::from_text("hello\nworld");
        t.set_cursor((0, 1));
        t.handle_key(shift(KeyCode::Right));
        t.handle_key(shift(KeyCode::Down));
        assert_eq!(t.selected_text().as_deref(), Some("ello\nwo"));
        t.handle_key(k(KeyCode::Left));
        assert_eq!(t.selection(), None);
        assert_eq!(t.cursor(), (0, 1), "Left collapses to the start");
    }

    #[test]
    fn copy_cut_paste_events() {
        let mut t = TextArea::from_text("abc def");
        assert_eq!(
            t.handle_key(ctrl('c')),
            TextAreaEvent::Ignored,
            "no selection"
        );
        t.select((0, 0), (0, 3));
        assert_eq!(t.handle_key(ctrl('c')), TextAreaEvent::Copy("abc".into()));
        assert_eq!(t.text(), "abc def");
        let ev = t.handle_key(ctrl('x'));
        assert_eq!(ev, TextAreaEvent::Cut("abc".into()));
        assert!(ev.is_edit());
        assert_eq!(t.text(), " def");
        assert_eq!(t.handle_key(ctrl('v')), TextAreaEvent::PasteRequested);
        t.insert_str("xy\r\n\tz");
        assert_eq!(t.text(), "xy\n    z def");
        t.undo();
        assert_eq!(t.text(), " def", "a paste is one undo step");
        t.handle_key(ctrl('a'));
        assert_eq!(t.selected_text().as_deref(), Some(" def"));
        assert_eq!(
            t.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::ALT)),
            TextAreaEvent::Ignored
        );
    }

    // ---- render and mouse ----

    fn draw(t: &mut TextArea, w: u16, h: u16, style: &TextAreaStyle) -> Terminal<TestBackend> {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| t.render_frame(f, f.area(), style)).unwrap();
        term
    }

    fn row_text(term: &Terminal<TestBackend>, y: u16) -> String {
        let buf = term.backend().buffer();
        (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect()
    }

    #[test]
    fn renders_text_line_numbers_and_a_visible_cursor() {
        let mut t = TextArea::from_text("alpha\nbeta");
        t.line_numbers = true;
        t.set_cursor((1, 2));
        let style = TextAreaStyle::default();
        let mut term = draw(&mut t, 12, 3, &style);
        assert_eq!(row_text(&term, 0), "1 alpha     ");
        assert_eq!(row_text(&term, 1), "2 beta      ");
        assert_eq!(row_text(&term, 2), "            ", "no number past the end");
        let buf = term.backend().buffer().clone();
        assert!(
            buf[(4, 1)].modifier.contains(Modifier::REVERSED),
            "cursor cell"
        );
        assert!(!buf[(3, 1)].modifier.contains(Modifier::REVERSED));
        term.backend_mut()
            .assert_cursor_position(Position { x: 4, y: 1 });
    }

    #[test]
    fn selection_and_highlight_styles() {
        let mut t = TextArea::from_text("abcd\nefgh");
        t.select((0, 2), (1, 1));
        t.set_highlights(vec![Highlight {
            row: 1,
            start: 2,
            end: 9,
        }]);
        let style = TextAreaStyle {
            selection: Style::default().bg(Color::Blue),
            highlight: Style::default().fg(Color::Red),
            cursor: Style::default().bg(Color::White),
            ..Default::default()
        };
        let term = draw(&mut t, 8, 2, &style);
        let buf = term.backend().buffer();
        let bg = |x, y| buf[(x, y)].bg;
        assert_eq!(bg(1, 0), Color::Reset);
        assert_eq!(bg(2, 0), Color::Blue);
        assert_eq!(bg(4, 0), Color::Blue, "the line break is selected too");
        assert_eq!(bg(5, 0), Color::Reset);
        assert_eq!(bg(0, 1), Color::Blue);
        assert_eq!(bg(1, 1), Color::White, "cursor at the selection end");
        assert_eq!(buf[(2, 1)].fg, Color::Red);
        assert_eq!(buf[(5, 1)].fg, Color::Red, "highlight past the line end");
        assert_eq!(buf[(1, 1)].fg, Color::Reset);
    }

    #[test]
    fn scrolls_both_ways_to_follow_the_cursor() {
        let text: Vec<String> = (0..20)
            .map(|i| format!("{i:02}{}", "-".repeat(30)))
            .collect();
        let mut t = TextArea::from_text(&text.join("\n"));
        t.set_cursor((15, 25));
        let mut term = draw(&mut t, 10, 4, &TextAreaStyle::default());
        assert_eq!(t.scroll(), (12, 16));
        assert_eq!(row_text(&term, 3), "-".repeat(10));
        term.backend_mut()
            .assert_cursor_position(Position { x: 9, y: 3 });
        t.set_cursor((0, 0));
        let term = draw(&mut t, 10, 4, &TextAreaStyle::default());
        assert_eq!(t.scroll(), (0, 0));
        assert_eq!(row_text(&term, 0), "00--------");
    }

    #[test]
    fn mouse_click_places_the_cursor_and_drag_selects() {
        let mut t = TextArea::from_text("hello\nworld\n!");
        t.line_numbers = true;
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 5));
        t.render(Rect::new(5, 1, 10, 3), &mut buf, &TextAreaStyle::default());
        let ev = |kind, x, y| MouseEvent {
            kind,
            column: x,
            row: y,
            modifiers: KeyModifiers::NONE,
        };
        // Gutter is 2 wide, so text starts at x = 7.
        assert_eq!(
            t.handle_mouse(ev(MouseEventKind::Down(MouseButton::Left), 9, 2)),
            TextAreaEvent::Moved
        );
        assert_eq!(t.cursor(), (1, 2));
        // Past the line end clamps to it.
        t.handle_mouse(ev(MouseEventKind::Down(MouseButton::Left), 14, 1));
        assert_eq!(t.cursor(), (0, 5));
        t.handle_mouse(ev(MouseEventKind::Up(MouseButton::Left), 14, 1));
        // Outside the text is ignored.
        assert_eq!(
            t.handle_mouse(ev(MouseEventKind::Down(MouseButton::Left), 0, 0)),
            TextAreaEvent::Ignored
        );
        // Drag from (0,1) to (1,3).
        t.handle_mouse(ev(MouseEventKind::Down(MouseButton::Left), 8, 1));
        t.handle_mouse(ev(MouseEventKind::Drag(MouseButton::Left), 10, 2));
        t.handle_mouse(ev(MouseEventKind::Up(MouseButton::Left), 10, 2));
        assert_eq!(t.selected_text().as_deref(), Some("ello\nwor"));
        t.handle_mouse(ev(MouseEventKind::ScrollDown, 8, 1));
        assert_eq!(t.cursor().0, 2);
        assert_eq!(t.selection(), None);
    }

    // ---- goal column ----

    #[test]
    fn up_down_keep_the_goal_column_across_short_lines() {
        let mut t = TextArea::from_text("abcdefgh\nab\n\nabcdefgh");
        t.set_cursor((0, 6));
        let mut seen = vec![];
        for code in [KeyCode::Down, KeyCode::Down, KeyCode::Down] {
            t.handle_key(k(code));
            seen.push(t.cursor());
        }
        assert_eq!(seen, [(1, 2), (2, 0), (3, 6)]);
        assert_eq!(t.goal_column(), Some(6));
        t.handle_key(k(KeyCode::Up));
        t.handle_key(k(KeyCode::Up));
        t.handle_key(k(KeyCode::Up));
        assert_eq!(t.cursor(), (0, 6));
        // PageUp/PageDown keep it too.
        t.handle_key(k(KeyCode::PageDown));
        assert_eq!(t.cursor(), (3, 6));
    }

    #[test]
    fn horizontal_moves_and_edits_reset_the_goal_column() {
        let mut t = TextArea::from_text("abcdefgh\nab\n\nabcdefgh");
        t.set_cursor((0, 6));
        t.handle_key(k(KeyCode::Down));
        t.handle_key(k(KeyCode::Left));
        assert_eq!(t.goal_column(), None);
        t.handle_key(k(KeyCode::Down));
        t.handle_key(k(KeyCode::Down));
        assert_eq!(t.cursor(), (3, 1), "new goal from column 1");
        t.set_cursor((0, 6));
        t.handle_key(k(KeyCode::Down));
        typed(&mut t, "x");
        assert_eq!(t.goal_column(), None);
        t.handle_key(k(KeyCode::Down));
        t.handle_key(k(KeyCode::Down));
        assert_eq!(t.cursor(), (3, 3), "goal from after the edit");
    }

    #[test]
    fn the_goal_column_is_a_display_column() {
        let mut t = TextArea::from_text("中中中\nabcdef");
        t.set_cursor((0, 2));
        t.handle_key(k(KeyCode::Down));
        assert_eq!(t.cursor(), (1, 4), "two wide chars are four cells");
        t.handle_key(k(KeyCode::Up));
        assert_eq!(t.cursor(), (0, 2));
        t.set_cursor((1, 3));
        t.handle_key(k(KeyCode::Up));
        assert_eq!(t.cursor(), (0, 1), "cell 3 is the second 中");
    }

    // ---- words ----

    fn ctrl_key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::CONTROL)
    }

    #[test]
    fn ctrl_arrows_move_by_word_and_punctuation_runs() {
        let mut t = TextArea::from_text("fx::dissolve(300)  .x\nnext_line");
        t.set_cursor((0, 0));
        let mut stops = vec![];
        for _ in 0..8 {
            t.handle_key(ctrl_key(KeyCode::Right));
            stops.push(t.cursor());
        }
        assert_eq!(
            stops,
            [
                (0, 2),
                (0, 4),
                (0, 12),
                (0, 13),
                (0, 16),
                (0, 17),
                (0, 20),
                (0, 21)
            ]
        );
        t.handle_key(ctrl_key(KeyCode::Right));
        assert_eq!(t.cursor(), (1, 0), "wraps to the next line");
        t.handle_key(ctrl_key(KeyCode::Right));
        assert_eq!(t.cursor(), (1, 9), "underscore joins a word");
        t.handle_key(ctrl_key(KeyCode::Left));
        assert_eq!(t.cursor(), (1, 0));
        t.handle_key(ctrl_key(KeyCode::Left));
        assert_eq!(t.cursor(), (0, 21), "wraps to the previous line end");
        t.handle_key(ctrl_key(KeyCode::Left));
        assert_eq!(t.cursor(), (0, 20));
        t.handle_key(ctrl_key(KeyCode::Left));
        assert_eq!(t.cursor(), (0, 19), "the '.' run");
        t.handle_key(ctrl_key(KeyCode::Left));
        assert_eq!(t.cursor(), (0, 16), "back over spaces, then ')'");
        t.set_cursor((0, 0));
        t.handle_key(ctrl_key(KeyCode::Left));
        assert_eq!(t.cursor(), (0, 0));
    }

    #[test]
    fn ctrl_shift_arrows_extend_the_selection_by_word() {
        let mut t = TextArea::from_text("let t = (600, QuadOut);");
        t.set_cursor((0, 14));
        let both = KeyModifiers::CONTROL | KeyModifiers::SHIFT;
        t.handle_key(KeyEvent::new(KeyCode::Right, both));
        assert_eq!(t.selected_text().as_deref(), Some("QuadOut"));
        t.handle_key(KeyEvent::new(KeyCode::Right, both));
        assert_eq!(t.selected_text().as_deref(), Some("QuadOut);"));
        t.set_cursor((0, 8));
        t.handle_key(KeyEvent::new(KeyCode::Left, both));
        assert_eq!(t.selected_text().as_deref(), Some("= "));
    }

    #[test]
    fn ctrl_backspace_and_delete_remove_a_word() {
        let mut t = TextArea::from_text("fx::dissolve(300)");
        t.set_cursor((0, 12));
        assert_eq!(
            t.handle_key(ctrl_key(KeyCode::Backspace)),
            TextAreaEvent::Edited
        );
        assert_eq!((t.text().as_str(), t.cursor()), ("fx::(300)", (0, 4)));
        assert_eq!(
            t.handle_key(ctrl_key(KeyCode::Delete)),
            TextAreaEvent::Edited
        );
        assert_eq!(t.text(), "fx::300)");
        t.undo();
        assert_eq!(t.text(), "fx::(300)", "one undo step per word");
        // At a line start Ctrl+Backspace joins with the line above.
        let mut t = TextArea::from_text("ab\ncd");
        t.set_cursor((1, 0));
        t.handle_key(ctrl_key(KeyCode::Backspace));
        assert_eq!(t.text(), "abcd");
        // A selection is deleted whole.
        t.select((0, 1), (0, 3));
        t.handle_key(ctrl_key(KeyCode::Delete));
        assert_eq!(t.text(), "ad");
        t.set_cursor((0, 2));
        assert_eq!(
            t.handle_key(ctrl_key(KeyCode::Delete)),
            TextAreaEvent::Ignored
        );
    }

    // ---- display width ----

    #[test]
    fn display_width_helpers() {
        assert_eq!(cell_width('a'), 1);
        assert_eq!(cell_width('中'), 2);
        assert_eq!(cell_width('🦀'), 2);
        assert_eq!(cell_width('\u{301}'), 1, "zero-width still gets a cell");
        assert_eq!(display_col("a中b", 2), 3);
        assert_eq!(display_col("a中b", 9), 4);
        let hits: Vec<usize> = (0..6).map(|x| char_at_display("a中b", x)).collect();
        assert_eq!(hits, [0, 1, 1, 2, 3, 3]);
    }

    #[test]
    fn wide_chars_take_two_cells_and_the_cursor_follows() {
        let mut t = TextArea::from_text("a中b");
        t.set_cursor((0, 2));
        let mut term = draw(&mut t, 6, 1, &TextAreaStyle::default());
        let buf = term.backend().buffer().clone();
        assert_eq!(buf[(0, 0)].symbol(), "a");
        assert_eq!(buf[(1, 0)].symbol(), "中");
        assert_eq!(buf[(3, 0)].symbol(), "b");
        assert!(
            buf[(3, 0)].modifier.contains(Modifier::REVERSED),
            "cursor on b"
        );
        term.backend_mut()
            .assert_cursor_position(Position { x: 3, y: 0 });
        // Editing stays char-indexed: Backspace removes the whole glyph.
        t.handle_key(k(KeyCode::Backspace));
        assert_eq!(t.text(), "ab");
    }

    #[test]
    fn a_selected_wide_char_colours_both_cells() {
        let mut t = TextArea::from_text("a中b");
        t.select((0, 1), (0, 2));
        let style = TextAreaStyle {
            selection: Style::default().bg(Color::Blue),
            cursor: Style::default(),
            ..Default::default()
        };
        // The frame buffer styles both cells. (A terminal, and TestBackend,
        // skip the covered cell: the glyph's own cell paints both.)
        let mut buf = Buffer::empty(Rect::new(0, 0, 6, 1));
        t.render(Rect::new(0, 0, 6, 1), &mut buf, &style);
        let bgs: Vec<Color> = (0..4).map(|x| buf[(x, 0)].bg).collect();
        assert_eq!(bgs, [Color::Reset, Color::Blue, Color::Blue, Color::Reset]);
        let term = draw(&mut t, 6, 1, &style);
        assert_eq!(term.backend().buffer()[(1, 0)].bg, Color::Blue);
    }

    #[test]
    fn wide_chars_are_never_split_at_the_viewport_edge() {
        // Right edge: 中 would need cells 2 and 3 of a 3-wide view.
        let mut t = TextArea::from_text("ab中");
        t.set_cursor((0, 0));
        let term = draw(&mut t, 3, 1, &TextAreaStyle::default());
        assert_eq!(row_text(&term, 0), "ab ");
        // Left edge: scrolled to put the cursor (after five wide chars) in
        // a 4-wide view, char 3 is cut in half and drawn blank.
        let mut t = TextArea::from_text("中中中中中");
        let mut term = draw(&mut t, 4, 1, &TextAreaStyle::default());
        assert_eq!(t.scroll(), (0, 7));
        let buf = term.backend().buffer().clone();
        assert_eq!(buf[(0, 0)].symbol(), " ");
        assert_eq!(buf[(1, 0)].symbol(), "中");
        term.backend_mut()
            .assert_cursor_position(Position { x: 3, y: 0 });
        // Scrolled so 中 straddles the left edge: blank, then 'd'.
        let mut t = TextArea::from_text("abc中def");
        t.set_cursor((0, 4));
        let term = draw(&mut t, 2, 1, &TextAreaStyle::default());
        assert_eq!(t.scroll(), (0, 4));
        assert_eq!(row_text(&term, 0), " d");
    }

    #[test]
    fn mouse_hits_count_display_columns() {
        let mut t = TextArea::from_text("a中b");
        let mut buf = Buffer::empty(Rect::new(0, 0, 6, 1));
        t.render(Rect::new(0, 0, 6, 1), &mut buf, &TextAreaStyle::default());
        let click = |x| MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: x,
            row: 0,
            modifiers: KeyModifiers::NONE,
        };
        t.handle_mouse(click(2));
        assert_eq!(t.cursor(), (0, 1), "right half of 中 is 中");
        t.handle_mouse(click(3));
        assert_eq!(t.cursor(), (0, 2));
        t.handle_mouse(click(5));
        assert_eq!(t.cursor(), (0, 3), "past the end clamps");
    }
}
