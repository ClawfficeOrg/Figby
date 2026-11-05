use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};
use ratatui::Frame;

use super::theme::Theme;
use super::undo::UndoEntry;

pub struct UndoPanel {
    pub open: bool,
    pub scroll_offset: u16,
    pub theme: Theme,
}

impl UndoPanel {
    pub fn new() -> Self {
        Self {
            open: false,
            scroll_offset: 0,
            theme: Theme::default(),
        }
    }

    pub fn toggle(&mut self) {
        self.open = !self.open;
        self.scroll_offset = 0;
    }

    pub fn handle_key(&mut self, code: ratatui::crossterm::event::KeyCode) -> bool {
        match code {
            ratatui::crossterm::event::KeyCode::Up => {
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
                true
            }
            ratatui::crossterm::event::KeyCode::Down => {
                self.scroll_offset = self.scroll_offset.saturating_add(1);
                true
            }
            ratatui::crossterm::event::KeyCode::Esc => {
                self.open = false;
                self.scroll_offset = 0;
                true
            }
            _ => false,
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect, history: &[UndoEntry]) {
        if !self.open {
            return;
        }

        let overlay = Rect {
            x: area.width.saturating_sub(30),
            y: 0,
            width: 30.min(area.width),
            height: (history.len() as u16 + 2).min(area.height / 2).max(4),
        };

        let mut lines: Vec<Line> = Vec::new();

        let max_visible = overlay.height.saturating_sub(3) as usize;
        let start = self.scroll_offset as usize;
        let show_entries: Vec<&UndoEntry> =
            history.iter().rev().skip(start).take(max_visible).collect();

        if show_entries.is_empty() {
            lines.push(Line::from(Span::styled(
                " No undo history",
                Style::default().fg(self.theme.dialog.meta),
            )));
        } else {
            for (i, entry) in show_entries.iter().enumerate() {
                let is_current = i == 0;
                let prefix = if is_current { ">" } else { " " };
                let style = if is_current {
                    Style::default()
                        .fg(self.theme.dialog.highlight)
                        .add_modifier(Modifier::REVERSED)
                } else {
                    Style::default()
                };
                lines.push(Line::from(Span::styled(
                    format!("{} {}", prefix, entry.label),
                    style,
                )));
            }
        }

        if start + show_entries.len() < history.len() {
            lines.push(Line::from(Span::styled(
                " ... more ...",
                Style::default().fg(self.theme.dialog.meta),
            )));
        }

        let paragraph = Paragraph::new(lines)
            .block(Block::default().title("Undo History").borders(Borders::ALL));
        frame.render_widget(paragraph, overlay);
    }
}

impl Widget for &UndoPanel {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default().title("Undo History").borders(Borders::ALL);
        let inner = block.inner(area);
        Widget::render(block, area, buf);

        let paragraph = Paragraph::new(Line::from(Span::styled(
            " No undo history",
            Style::default().fg(self.theme.dialog.meta),
        )));
        Widget::render(paragraph, inner, buf);
    }
}

impl Default for UndoPanel {
    fn default() -> Self {
        Self::new()
    }
}
