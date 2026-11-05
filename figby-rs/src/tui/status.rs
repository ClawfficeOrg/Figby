use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget};

use super::theme::Theme;

#[derive(Debug, Clone)]
pub struct CanvasSettings {
    pub settings_open: bool,
    pub canvas_width: u16,
    pub canvas_height: u16,
    pub font_size: u8,
    pub show_grid: bool,
    pub snap_to_grid: bool,
    selected_field: usize,
    pub theme: Theme,
}

impl CanvasSettings {
    pub fn new() -> Self {
        Self {
            settings_open: false,
            canvas_width: 40,
            canvas_height: 20,
            font_size: 12,
            show_grid: false,
            snap_to_grid: false,
            selected_field: 0,
            theme: Theme::default(),
        }
    }

    pub fn handle_key(&mut self, code: KeyCode) -> bool {
        if !self.settings_open {
            return false;
        }
        match code {
            KeyCode::Up => {
                self.selected_field = self.selected_field.saturating_sub(1);
                true
            }
            KeyCode::Down => {
                if self.selected_field < 4 {
                    self.selected_field += 1;
                }
                true
            }
            KeyCode::Left => {
                match self.selected_field {
                    0 => self.canvas_width = self.canvas_width.saturating_sub(1).max(1),
                    1 => self.canvas_height = self.canvas_height.saturating_sub(1).max(1),
                    2 => self.font_size = self.font_size.saturating_sub(1).max(6),
                    _ => {}
                }
                true
            }
            KeyCode::Right => {
                match self.selected_field {
                    0 => self.canvas_width = self.canvas_width.saturating_add(1).min(200),
                    1 => self.canvas_height = self.canvas_height.saturating_add(1).min(200),
                    2 => self.font_size = self.font_size.saturating_add(1).min(72),
                    _ => {}
                }
                true
            }
            KeyCode::Enter => {
                match self.selected_field {
                    3 => self.show_grid = !self.show_grid,
                    4 => self.snap_to_grid = !self.snap_to_grid,
                    _ => {}
                }
                true
            }
            KeyCode::Esc => {
                self.settings_open = false;
                true
            }
            _ => false,
        }
    }
}

impl Widget for &CanvasSettings {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if !self.settings_open {
            return;
        }

        Widget::render(Clear, area, buf);
        let block = Block::default().title(" Settings ").borders(Borders::ALL);
        let inner = block.inner(area);
        Widget::render(block, area, buf);

        if inner.width < 10 || inner.height < 4 {
            return;
        }

        let mut lines: Vec<Line<'_>> = Vec::new();

        let fields: [(&str, String); 5] = [
            ("Canvas Width", format!("{}", self.canvas_width)),
            ("Canvas Height", format!("{}", self.canvas_height)),
            ("Font Size", format!("{}", self.font_size)),
            (
                "Grid",
                if self.show_grid {
                    "On".into()
                } else {
                    "Off".into()
                },
            ),
            (
                "Snap-to-Grid",
                if self.snap_to_grid {
                    "On".into()
                } else {
                    "Off".into()
                },
            ),
        ];

        for (i, (label, value)) in fields.iter().enumerate() {
            let line = if i == self.selected_field {
                let style = Style::default()
                    .fg(self.theme.dialog.highlight)
                    .add_modifier(Modifier::REVERSED);
                Line::from(Span::styled(format!(" {}: {} ", label, value), style))
            } else {
                Line::from(Span::raw(format!(" {}: {}", label, value)))
            };
            lines.push(line);
        }

        let paragraph = Paragraph::new(lines);
        Widget::render(paragraph, inner, buf);
    }
}

impl Default for CanvasSettings {
    fn default() -> Self {
        Self::new()
    }
}
