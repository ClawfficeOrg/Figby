use crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use super::super::theme::Theme;
use crate::palette_import::{builtin_palettes, Swatch};

const DEFAULT_WIDTH: u16 = 80;
const DEFAULT_HEIGHT: u16 = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Width,
    Height,
    Palette,
}

pub struct NewImageDialog {
    pub active: bool,
    pub width_buffer: String,
    pub height_buffer: String,
    pub selected_palette: usize,
    pub palette_names: Vec<&'static str>,
    pub selected_field: Field,
    pub error_message: String,
    pub confirmed: bool,
    pub result_width: u16,
    pub result_height: u16,
    pub result_palette_name: String,
    pub result_palette_swatches: Vec<Swatch>,
    pub theme: Theme,
}

impl NewImageDialog {
    pub fn new() -> Self {
        let palettes = builtin_palettes();
        let palette_names: Vec<&str> = palettes.iter().map(|(n, _)| *n).collect();
        Self {
            active: false,
            width_buffer: DEFAULT_WIDTH.to_string(),
            height_buffer: DEFAULT_HEIGHT.to_string(),
            selected_palette: 0,
            palette_names,
            selected_field: Field::Width,
            error_message: String::new(),
            confirmed: false,
            result_width: DEFAULT_WIDTH,
            result_height: DEFAULT_HEIGHT,
            result_palette_name: String::new(),
            result_palette_swatches: Vec::new(),
            theme: Theme::default(),
        }
    }

    pub fn enter_new_image(&mut self) {
        self.active = true;
        self.width_buffer = DEFAULT_WIDTH.to_string();
        self.height_buffer = DEFAULT_HEIGHT.to_string();
        self.selected_palette = 0;
        self.selected_field = Field::Width;
        self.error_message.clear();
        self.confirmed = false;
    }

    pub fn close(&mut self) {
        self.active = false;
        self.confirmed = false;
        self.error_message.clear();
    }

    fn palette_label(&self) -> &str {
        if self.palette_names.is_empty() {
            "(none)"
        } else {
            self.palette_names[self.selected_palette % self.palette_names.len()]
        }
    }

    fn parse_width(&self) -> Option<u16> {
        if self.width_buffer.is_empty() {
            return None;
        }
        self.width_buffer.parse::<u16>().ok().filter(|&v| v >= 1)
    }

    fn parse_height(&self) -> Option<u16> {
        if self.height_buffer.is_empty() {
            return None;
        }
        self.height_buffer.parse::<u16>().ok().filter(|&v| v >= 1)
    }

    fn next_field(&mut self) {
        self.selected_field = match self.selected_field {
            Field::Width => Field::Height,
            Field::Height => Field::Palette,
            Field::Palette => Field::Width,
        };
    }

    fn prev_field(&mut self) {
        self.selected_field = match self.selected_field {
            Field::Width => Field::Palette,
            Field::Height => Field::Width,
            Field::Palette => Field::Height,
        };
    }

    pub fn confirm(&mut self) {
        let w = match self.parse_width() {
            Some(v) => v,
            None => {
                self.error_message = "Width must be 1-65535".to_string();
                self.selected_field = Field::Width;
                return;
            }
        };
        let h = match self.parse_height() {
            Some(v) => v,
            None => {
                self.error_message = "Height must be 1-65535".to_string();
                self.selected_field = Field::Height;
                return;
            }
        };
        // F-12: a 65535×65535 canvas is 4.3 billion cells — reject anything
        // beyond the shared cell budget before any allocation.
        if !super::super::canvas::canvas_cells_within_budget(w as usize, h as usize) {
            self.error_message = format!(
                "{}×{} exceeds the {} cell budget",
                w,
                h,
                super::super::canvas::MAX_CANVAS_CELLS
            );
            return;
        }
        self.result_width = w;
        self.result_height = h;
        let palettes = builtin_palettes();
        let idx = self.selected_palette % self.palette_names.len().max(1);
        self.result_palette_name = self.palette_names[idx].to_string();
        self.result_palette_swatches = if !palettes.is_empty() {
            palettes[idx].1.clone()
        } else {
            Vec::new()
        };
        self.confirmed = true;
        self.active = false;
    }

    pub fn handle_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Tab | KeyCode::Down => {
                self.next_field();
                true
            }
            KeyCode::BackTab | KeyCode::Up => {
                self.prev_field();
                true
            }
            KeyCode::Char(c) if c.is_ascii_digit() => {
                match self.selected_field {
                    Field::Width => {
                        if self.width_buffer.len() < 5 {
                            self.width_buffer.push(c);
                        }
                    }
                    Field::Height => {
                        if self.height_buffer.len() < 5 {
                            self.height_buffer.push(c);
                        }
                    }
                    Field::Palette => {}
                }
                self.error_message.clear();
                true
            }
            KeyCode::Backspace => {
                match self.selected_field {
                    Field::Width => {
                        self.width_buffer.pop();
                    }
                    Field::Height => {
                        self.height_buffer.pop();
                    }
                    Field::Palette => {}
                }
                self.error_message.clear();
                true
            }
            KeyCode::Left => {
                if self.selected_field == Field::Palette {
                    let n = self.palette_names.len().max(1);
                    self.selected_palette = self
                        .selected_palette
                        .saturating_sub(1)
                        .min(n.saturating_sub(1));
                } else {
                    self.prev_field();
                }
                true
            }
            KeyCode::Right => {
                if self.selected_field == Field::Palette {
                    let n = self.palette_names.len().max(1);
                    self.selected_palette = (self.selected_palette + 1).min(n.saturating_sub(1));
                } else {
                    self.next_field();
                }
                true
            }
            KeyCode::Enter => {
                self.confirm();
                true
            }
            KeyCode::Esc => {
                self.close();
                true
            }
            _ => false,
        }
    }

    fn field_style(&self, field: Field) -> Style {
        if self.selected_field == field {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default()
        }
    }
}

impl Default for NewImageDialog {
    fn default() -> Self {
        Self::new()
    }
}

pub fn render_new_image_dialog(dialog: &NewImageDialog, frame: &mut Frame, area: Rect) {
    frame.render_widget(Clear, area);
    let block = Block::default()
        .title(" New Image ")
        .borders(Borders::ALL)
        .style(Style::default().fg(dialog.theme.dialog.highlight));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width < 30 || inner.height < 10 {
        return;
    }

    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::from(""));

    lines.push(Line::from(vec![
        Span::styled(" Width:  ", Style::default().add_modifier(Modifier::BOLD)),
        Span::styled(
            format!(
                "{}  ",
                if dialog.width_buffer.is_empty() {
                    "(enter width)"
                } else {
                    &dialog.width_buffer
                }
            ),
            dialog.field_style(Field::Width),
        ),
        Span::styled(
            "(digits, Tab/Up/Down)",
            Style::default().fg(dialog.theme.dialog.meta),
        ),
    ]));

    lines.push(Line::from(vec![
        Span::styled(" Height: ", Style::default().add_modifier(Modifier::BOLD)),
        Span::styled(
            format!(
                "{}  ",
                if dialog.height_buffer.is_empty() {
                    "(enter height)"
                } else {
                    &dialog.height_buffer
                }
            ),
            dialog.field_style(Field::Height),
        ),
        Span::styled(
            "(digits, Tab/Up/Down)",
            Style::default().fg(dialog.theme.dialog.meta),
        ),
    ]));

    lines.push(Line::from(vec![
        Span::styled(" Palette:", Style::default().add_modifier(Modifier::BOLD)),
        Span::styled(
            format!(" [{}]  ", dialog.palette_label()),
            dialog.field_style(Field::Palette),
        ),
        Span::styled(
            "(\u{2190}\u{2192} cycle)",
            Style::default().fg(dialog.theme.dialog.meta),
        ),
    ]));

    lines.push(Line::from(""));

    if !dialog.error_message.is_empty() {
        lines.push(Line::from(Span::styled(
            format!(" Error: {}", dialog.error_message),
            Style::default().fg(dialog.theme.dialog.error),
        )));
        lines.push(Line::from(""));
    }

    lines.push(Line::from(Span::styled(
        " Enter: create  Esc: cancel  \u{2191}\u{2193}: navigate fields",
        Style::default().fg(dialog.theme.dialog.meta),
    )));

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// F-12 (GPT review): a 65535×65535 canvas is ~4.3 billion cells and must
    /// be rejected by the cell budget instead of allocated.
    #[test]
    fn test_confirm_rejects_huge_canvas() {
        let mut dialog = NewImageDialog::new();
        dialog.width_buffer = "65535".to_string();
        dialog.height_buffer = "65535".to_string();
        dialog.confirm();
        assert!(!dialog.confirmed);
        assert!(
            !dialog.error_message.is_empty(),
            "error must explain the rejection"
        );
    }

    #[test]
    fn test_confirm_accepts_normal_canvas() {
        let mut dialog = NewImageDialog::new();
        dialog.width_buffer = "80".to_string();
        dialog.height_buffer = "24".to_string();
        dialog.confirm();
        assert!(dialog.confirmed);
        assert_eq!(dialog.result_width, 80);
        assert_eq!(dialog.result_height, 24);
    }

    #[test]
    fn test_confirm_rejects_zero_dimension() {
        let mut dialog = NewImageDialog::new();
        dialog.width_buffer = "0".to_string();
        dialog.height_buffer = "10".to_string();
        dialog.confirm();
        assert!(!dialog.confirmed);
        assert!(dialog.error_message.contains("1-65535"));
    }
}
