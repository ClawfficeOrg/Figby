//! The "Run effect…" prompt (F7): one line of tachyonfx DSL, or the name of
//! a built-in preset, run over the screen in the `manual` slot.
//!
//! Keys: type to edit; Left/Right/Home/End move; Backspace/Delete erase;
//! Up/Down pick a preset (its name fills the line); Tab expands a preset
//! name into its DSL so it can be tweaked; Enter runs; Esc closes.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;

use super::presets;
use super::runtime::one_line;
use crate::tui::theme::Theme;

/// A preset as the prompt lists it.
#[derive(Debug, Clone, PartialEq)]
pub struct PromptPreset {
    pub name: &'static str,
    pub description: &'static str,
    /// The preset graph's DSL on one line.
    pub dsl: String,
}

/// What a key did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FxPromptOutcome {
    /// Nothing changed.
    None,
    /// The line or the preset highlight changed; redraw.
    Changed,
    /// Enter: run this text (DSL, or a preset name).
    Submit(String),
    /// Esc: the prompt closed.
    Cancel,
}

/// Names the prompt lists under the input, for the help line.
pub const TARGET_NAMES: &str = "canvas screen selection layer_N toolbox side_panel status";

pub struct FxPrompt {
    pub open: bool,
    input: String,
    /// Cursor position as a char index into `input`.
    cursor: usize,
    selected: Option<usize>,
    presets: Vec<PromptPreset>,
}

impl Default for FxPrompt {
    fn default() -> Self {
        Self::new()
    }
}

impl FxPrompt {
    pub fn new() -> Self {
        let presets = presets::RUNNABLE
            .iter()
            .filter_map(|name| presets::find(name))
            .filter_map(|p| {
                Some(PromptPreset {
                    name: p.name,
                    description: p.description,
                    dsl: one_line(&p.graph.to_dsl().ok()?),
                })
            })
            .collect();
        Self {
            open: false,
            input: String::new(),
            cursor: 0,
            selected: None,
            presets,
        }
    }

    /// Open with the last line kept, so a failed run can be fixed.
    pub fn open(&mut self) {
        self.open = true;
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn input(&self) -> &str {
        &self.input
    }

    /// Cursor as a 0-based char index.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn presets(&self) -> &[PromptPreset] {
        &self.presets
    }

    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    pub fn set_input(&mut self, text: &str) {
        self.input = text.to_string();
        self.cursor = self.input.chars().count();
    }

    /// Put the cursor on a 1-based column of the line (an error position).
    pub fn set_cursor_column(&mut self, column: usize) {
        self.cursor = column.saturating_sub(1).min(self.input.chars().count());
    }

    /// The preset whose name is `text` (trimmed), if any.
    pub fn preset_named(&self, text: &str) -> Option<&PromptPreset> {
        let text = text.trim();
        self.presets.iter().find(|p| p.name == text)
    }

    fn byte_at(&self, char_idx: usize) -> usize {
        self.input
            .char_indices()
            .nth(char_idx)
            .map_or(self.input.len(), |(b, _)| b)
    }

    fn pick(&mut self, idx: usize) {
        self.selected = Some(idx);
        let name = self.presets[idx].name;
        self.set_input(name);
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> FxPromptOutcome {
        let len = self.input.chars().count();
        match key.code {
            KeyCode::Esc => {
                self.close();
                FxPromptOutcome::Cancel
            }
            KeyCode::Enter => FxPromptOutcome::Submit(self.input.trim().to_string()),
            KeyCode::Up if !self.presets.is_empty() => {
                let n = self.presets.len();
                let idx = self.selected.map_or(n - 1, |i| (i + n - 1) % n);
                self.pick(idx);
                FxPromptOutcome::Changed
            }
            KeyCode::Down if !self.presets.is_empty() => {
                let n = self.presets.len();
                let idx = self.selected.map_or(0, |i| (i + 1) % n);
                self.pick(idx);
                FxPromptOutcome::Changed
            }
            KeyCode::Tab => match self.preset_named(&self.input).map(|p| p.dsl.clone()) {
                Some(dsl) => {
                    self.set_input(&dsl);
                    FxPromptOutcome::Changed
                }
                None => FxPromptOutcome::None,
            },
            KeyCode::Left if self.cursor > 0 => {
                self.cursor -= 1;
                FxPromptOutcome::Changed
            }
            KeyCode::Right if self.cursor < len => {
                self.cursor += 1;
                FxPromptOutcome::Changed
            }
            KeyCode::Home => {
                self.cursor = 0;
                FxPromptOutcome::Changed
            }
            KeyCode::End => {
                self.cursor = len;
                FxPromptOutcome::Changed
            }
            KeyCode::Backspace if self.cursor > 0 => {
                let at = self.byte_at(self.cursor - 1);
                self.input.remove(at);
                self.cursor -= 1;
                self.selected = None;
                FxPromptOutcome::Changed
            }
            KeyCode::Delete if self.cursor < len => {
                let at = self.byte_at(self.cursor);
                self.input.remove(at);
                self.selected = None;
                FxPromptOutcome::Changed
            }
            KeyCode::Char(c)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                let at = self.byte_at(self.cursor);
                self.input.insert(at, c);
                self.cursor += 1;
                self.selected = None;
                FxPromptOutcome::Changed
            }
            _ => FxPromptOutcome::None,
        }
    }

    /// The box the prompt draws in, for a frame of `area`.
    pub fn rect(&self, area: Rect) -> Rect {
        let width = area.width.saturating_sub(4).min(76);
        let height = (self.presets.len() as u16 + 4).min(area.height);
        Rect {
            x: area.x + (area.width.saturating_sub(width)) / 2,
            y: area.y + (area.height.saturating_sub(height)) / 3,
            width,
            height,
        }
    }

    pub fn render(&self, frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
        let rect = self.rect(area);
        if rect.width < 12 || rect.height < 3 {
            return;
        }
        frame.render_widget(Clear, rect);
        let dim = Style::default().fg(theme.general.secondary);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(theme.general.primary))
            .title(Span::styled(
                " Run effect ",
                Style::default()
                    .fg(theme.general.primary)
                    .add_modifier(Modifier::BOLD),
            ))
            .title_bottom(Line::styled(
                " Enter run  ↑↓ preset  Tab expand  Esc close ",
                dim,
            ))
            .style(
                Style::default()
                    .bg(theme.menu.dropdown_bg)
                    .fg(theme.menu.fg),
            );
        let inner = block.inner(rect);
        frame.render_widget(block, rect);
        if inner.height == 0 || inner.width < 3 {
            return;
        }

        // Input line: "> " then the text, scrolled so the cursor shows.
        let room = inner.width.saturating_sub(2) as usize;
        let skip = self.cursor.saturating_sub(room.saturating_sub(1));
        let shown: String = self.input.chars().skip(skip).take(room).collect();
        let input_line = Line::from(vec![
            Span::styled("> ", Style::default().fg(theme.general.primary)),
            Span::raw(shown),
        ]);
        let input_rect = Rect { height: 1, ..inner };
        frame.render_widget(Paragraph::new(input_line), input_rect);
        frame.set_cursor_position(Position {
            x: inner.x + 2 + (self.cursor - skip) as u16,
            y: inner.y,
        });

        let mut lines = vec![Line::styled(format!("targets: {TARGET_NAMES}"), dim)];
        for (i, p) in self.presets.iter().enumerate() {
            let style = if self.selected == Some(i) {
                Style::default()
                    .fg(theme.general.primary)
                    .add_modifier(Modifier::BOLD | Modifier::REVERSED)
            } else {
                Style::default()
            };
            lines.push(Line::from(vec![
                Span::styled(format!(" {:<9}", p.name), style),
                Span::styled(format!(" {}", p.description), dim),
            ]));
        }
        let rest = Rect {
            y: inner.y + 1,
            height: inner.height.saturating_sub(1),
            ..inner
        };
        frame.render_widget(Paragraph::new(lines), rest);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::fx_script::compile::{compile, FxTargets};

    fn key(p: &mut FxPrompt, code: KeyCode) -> FxPromptOutcome {
        p.handle_key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn type_str(p: &mut FxPrompt, s: &str) {
        for c in s.chars() {
            key(p, KeyCode::Char(c));
        }
    }

    #[test]
    fn lists_every_runnable_preset_in_order() {
        let p = FxPrompt::new();
        let names: Vec<_> = p.presets().iter().map(|p| p.name).collect();
        assert_eq!(names, presets::RUNNABLE);
    }

    #[test]
    fn every_listed_preset_compiles_on_one_line() {
        let t = FxTargets::uniform(Rect::new(0, 0, 20, 5), 1);
        for preset in FxPrompt::new().presets() {
            assert!(!preset.dsl.contains('\n'));
            compile(&preset.dsl, &t).unwrap_or_else(|e| panic!("{}: {e}", preset.name));
        }
    }

    #[test]
    fn typing_editing_and_submit() {
        let mut p = FxPrompt::new();
        p.open();
        type_str(&mut p, "fx::dissolve(30)");
        assert_eq!(p.input(), "fx::dissolve(30)");
        key(&mut p, KeyCode::Left);
        key(&mut p, KeyCode::Backspace);
        type_str(&mut p, "5");
        assert_eq!(p.input(), "fx::dissolve(35)");
        key(&mut p, KeyCode::Home);
        key(&mut p, KeyCode::Delete);
        assert_eq!(p.input(), "x::dissolve(35)");
        key(&mut p, KeyCode::End);
        type_str(&mut p, "  ");
        assert_eq!(
            key(&mut p, KeyCode::Enter),
            FxPromptOutcome::Submit("x::dissolve(35)".into())
        );
        assert!(p.open, "Enter leaves closing to the caller");
    }

    #[test]
    fn control_chords_are_not_typed() {
        let mut p = FxPrompt::new();
        let out = p.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert_eq!(out, FxPromptOutcome::None);
        assert_eq!(p.input(), "");
    }

    #[test]
    fn esc_cancels_and_closes() {
        let mut p = FxPrompt::new();
        p.open();
        assert_eq!(key(&mut p, KeyCode::Esc), FxPromptOutcome::Cancel);
        assert!(!p.open);
    }

    #[test]
    fn arrows_cycle_presets_and_tab_expands() {
        let mut p = FxPrompt::new();
        key(&mut p, KeyCode::Down);
        assert_eq!(p.input(), "fade-in");
        key(&mut p, KeyCode::Up);
        assert_eq!(p.input(), "glitch", "Up wraps to the last preset");
        assert_eq!(p.selected(), Some(4));
        key(&mut p, KeyCode::Tab);
        assert_eq!(p.input(), "fx::glitch(600).with_area(canvas)");
        assert_eq!(p.cursor(), p.input().chars().count());
        assert_eq!(key(&mut p, KeyCode::Tab), FxPromptOutcome::None);
    }

    #[test]
    fn multibyte_input_edits_by_char() {
        let mut p = FxPrompt::new();
        type_str(&mut p, "a…b");
        key(&mut p, KeyCode::Left);
        key(&mut p, KeyCode::Backspace);
        assert_eq!(p.input(), "ab");
    }

    #[test]
    fn error_column_moves_the_cursor() {
        let mut p = FxPrompt::new();
        p.set_input("fx::dissolve(300).with_area(nowhere)");
        p.set_cursor_column(29);
        assert_eq!(p.cursor(), 28);
        p.set_cursor_column(500);
        assert_eq!(p.cursor(), p.input().chars().count());
    }
}
