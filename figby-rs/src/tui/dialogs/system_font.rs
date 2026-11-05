use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use super::super::theme::Theme;
use crate::font_gen::{self, FontFamilyInfo};

const DEFAULT_SIZE: &str = "12";

/// Charset presets offered by the TUI creation dialogs, matching the names
/// `font_gen::resolve_charset` understands (the CLI's `--create-font-charset`
/// accepts the same names, plus an arbitrary custom comma-separated list —
/// that free-text fallback isn't exposed here, only the named presets).
pub const CHARSET_NAMES: &[&str] = &[
    "default",
    "slight",
    "smooth",
    "block",
    "blocks",
    "box",
    "braille",
    "ogham",
    "dithered",
    "geometric",
    "deluxe",
    "full",
];

const DEFAULT_CHARSET_INDEX: usize = 2; // "smooth" — matches the prior hardcoded default

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    List,
    Size,
    Charset,
}

pub struct SystemFontPickerDialog {
    pub active: bool,
    pub families: Vec<FontFamilyInfo>,
    pub filter: String,
    pub selected: usize,
    pub size_buffer: String,
    pub charset_index: usize,
    pub selected_field: Field,
    pub error_message: String,
    pub confirmed: bool,
    pub result_family: String,
    pub result_size: f32,
    pub result_charset: String,
    pub theme: Theme,
}

impl SystemFontPickerDialog {
    pub fn new() -> Self {
        Self {
            active: false,
            families: Vec::new(),
            filter: String::new(),
            selected: 0,
            size_buffer: DEFAULT_SIZE.to_string(),
            charset_index: DEFAULT_CHARSET_INDEX,
            selected_field: Field::List,
            error_message: String::new(),
            confirmed: false,
            result_family: String::new(),
            result_size: 12.0,
            result_charset: CHARSET_NAMES[DEFAULT_CHARSET_INDEX].to_string(),
            theme: Theme::default(),
        }
    }

    pub fn enter(&mut self) {
        self.active = true;
        self.filter.clear();
        self.selected = 0;
        self.size_buffer = DEFAULT_SIZE.to_string();
        self.charset_index = DEFAULT_CHARSET_INDEX;
        self.selected_field = Field::List;
        self.error_message.clear();
        self.confirmed = false;
        match font_gen::list_system_fonts() {
            Ok(mut families) => {
                families.sort_by_key(|a| a.family.to_lowercase());
                self.families = families;
            }
            Err(e) => {
                self.families.clear();
                self.error_message = format!("Could not list system fonts: {e}");
            }
        }
    }

    pub fn cycle_charset(&mut self, forward: bool) {
        let len = CHARSET_NAMES.len();
        self.charset_index = if forward {
            (self.charset_index + 1) % len
        } else {
            (self.charset_index + len - 1) % len
        };
    }

    pub fn close(&mut self) {
        self.active = false;
        self.confirmed = false;
    }

    pub fn filtered_indices(&self) -> Vec<usize> {
        if self.filter.is_empty() {
            return (0..self.families.len()).collect();
        }
        let needle = self.filter.to_lowercase();
        self.families
            .iter()
            .enumerate()
            .filter(|(_, f)| f.family.to_lowercase().contains(&needle))
            .map(|(i, _)| i)
            .collect()
    }

    fn parse_size(&self) -> Option<f32> {
        self.size_buffer
            .parse::<f32>()
            .ok()
            .filter(|v| (4.0..=200.0).contains(v))
    }

    pub fn confirm(&mut self) {
        let indices = self.filtered_indices();
        let Some(&family_idx) = indices.get(self.selected) else {
            self.error_message = "Select a font family".to_string();
            return;
        };
        let Some(size) = self.parse_size() else {
            self.error_message = "Size must be 4-200".to_string();
            self.selected_field = Field::Size;
            return;
        };
        self.result_family = self.families[family_idx].family.clone();
        self.result_size = size;
        self.result_charset = CHARSET_NAMES[self.charset_index].to_string();
        self.confirmed = true;
        self.active = false;
    }

    pub fn handle_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Tab => {
                self.selected_field = match self.selected_field {
                    Field::List => Field::Size,
                    Field::Size => Field::Charset,
                    Field::Charset => Field::List,
                };
                true
            }
            KeyCode::Up if self.selected_field == Field::List => {
                self.selected = self.selected.saturating_sub(1);
                true
            }
            KeyCode::Down if self.selected_field == Field::List => {
                let count = self.filtered_indices().len();
                if self.selected + 1 < count {
                    self.selected += 1;
                }
                true
            }
            KeyCode::Left if self.selected_field == Field::Charset => {
                self.cycle_charset(false);
                true
            }
            KeyCode::Right if self.selected_field == Field::Charset => {
                self.cycle_charset(true);
                true
            }
            KeyCode::Char(c) if self.selected_field == Field::List && !c.is_ascii_digit() => {
                self.filter.push(c);
                self.selected = 0;
                self.error_message.clear();
                true
            }
            KeyCode::Char(c) if self.selected_field == Field::Size && c.is_ascii_digit() => {
                if self.size_buffer.len() < 3 {
                    self.size_buffer.push(c);
                }
                self.error_message.clear();
                true
            }
            KeyCode::Char('.') if self.selected_field == Field::Size => {
                if !self.size_buffer.contains('.') {
                    self.size_buffer.push('.');
                }
                true
            }
            KeyCode::Backspace => {
                match self.selected_field {
                    Field::List => {
                        self.filter.pop();
                        self.selected = 0;
                    }
                    Field::Size => {
                        self.size_buffer.pop();
                    }
                    Field::Charset => {}
                }
                self.error_message.clear();
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
}

impl Default for SystemFontPickerDialog {
    fn default() -> Self {
        Self::new()
    }
}

pub fn render_system_font_dialog(dialog: &SystemFontPickerDialog, frame: &mut Frame, area: Rect) {
    frame.render_widget(Clear, area);
    let block = Block::default()
        .title(" New Font from System ")
        .borders(Borders::ALL)
        .style(Style::default().fg(dialog.theme.dialog.highlight));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width < 30 || inner.height < 10 {
        return;
    }

    let list_h = (inner.height as usize).saturating_sub(6);
    let indices = dialog.filtered_indices();

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(vec![
        Span::styled(" Filter: ", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(if dialog.filter.is_empty() {
            "(type to filter)".to_string()
        } else {
            dialog.filter.clone()
        }),
    ]));
    lines.push(Line::from(""));

    if indices.is_empty() {
        lines.push(Line::from(" (no matching fonts)"));
    } else {
        let start = if list_h > 0 && dialog.selected >= list_h {
            dialog.selected + 1 - list_h
        } else {
            0
        };
        for (row, &idx) in indices.iter().enumerate().skip(start).take(list_h) {
            let is_selected = row == dialog.selected;
            let style = if is_selected && dialog.selected_field == Field::List {
                Style::default().add_modifier(Modifier::REVERSED)
            } else if is_selected {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            lines.push(Line::from(Span::styled(
                format!(" {}", dialog.families[idx].family),
                style,
            )));
        }
    }

    while lines.len() < list_h + 2 {
        lines.push(Line::from(""));
    }

    let size_style = if dialog.selected_field == Field::Size {
        Style::default().add_modifier(Modifier::REVERSED)
    } else {
        Style::default()
    };
    lines.push(Line::from(vec![
        Span::styled(" Size: ", Style::default().add_modifier(Modifier::BOLD)),
        Span::styled(format!("{}  ", dialog.size_buffer), size_style),
        Span::styled(
            "(points, 4-200)",
            Style::default().fg(dialog.theme.dialog.meta),
        ),
    ]));

    let charset_style = if dialog.selected_field == Field::Charset {
        Style::default().add_modifier(Modifier::REVERSED)
    } else {
        Style::default()
    };
    lines.push(Line::from(vec![
        Span::styled(" Charset: ", Style::default().add_modifier(Modifier::BOLD)),
        Span::styled(
            format!("{}  ", CHARSET_NAMES[dialog.charset_index]),
            charset_style,
        ),
        Span::styled(
            "(\u{2190}/\u{2192} to change)",
            Style::default().fg(dialog.theme.dialog.meta),
        ),
    ]));

    if !dialog.error_message.is_empty() {
        lines.push(Line::from(Span::styled(
            format!(" Error: {}", dialog.error_message),
            Style::default().fg(dialog.theme.dialog.error),
        )));
    }

    lines.push(Line::from(Span::styled(
        " Enter: create  Esc: cancel  Tab: switch field  \u{2191}\u{2193}/\u{2190}\u{2192}: select",
        Style::default().fg(dialog.theme.dialog.meta),
    )));

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_defaults_to_smooth_charset() {
        let dialog = SystemFontPickerDialog::new();
        assert_eq!(CHARSET_NAMES[dialog.charset_index], "smooth");
        assert_eq!(dialog.result_charset, "smooth");
    }

    #[test]
    fn test_tab_cycles_through_all_three_fields() {
        let mut dialog = SystemFontPickerDialog::new();
        assert_eq!(dialog.selected_field, Field::List);
        dialog.handle_key(KeyCode::Tab);
        assert_eq!(dialog.selected_field, Field::Size);
        dialog.handle_key(KeyCode::Tab);
        assert_eq!(dialog.selected_field, Field::Charset);
        dialog.handle_key(KeyCode::Tab);
        assert_eq!(dialog.selected_field, Field::List);
    }

    #[test]
    fn test_left_right_cycle_charset_when_field_focused() {
        let mut dialog = SystemFontPickerDialog::new();
        dialog.selected_field = Field::Charset;
        let start = dialog.charset_index;
        dialog.handle_key(KeyCode::Right);
        assert_eq!(dialog.charset_index, (start + 1) % CHARSET_NAMES.len());
        dialog.handle_key(KeyCode::Left);
        assert_eq!(dialog.charset_index, start);
    }

    #[test]
    fn test_left_right_ignored_when_charset_not_focused() {
        let mut dialog = SystemFontPickerDialog::new();
        assert_eq!(dialog.selected_field, Field::List);
        let start = dialog.charset_index;
        let consumed = dialog.handle_key(KeyCode::Right);
        assert!(!consumed, "Right should not be consumed by the List field");
        assert_eq!(dialog.charset_index, start);
    }

    #[test]
    fn test_confirm_carries_selected_charset_into_result() {
        let mut dialog = SystemFontPickerDialog::new();
        dialog.families = vec![FontFamilyInfo {
            family: "TestFont".to_string(),
            styles: vec![],
        }];
        dialog.cycle_charset(true);
        let expected = CHARSET_NAMES[dialog.charset_index].to_string();
        dialog.confirm();
        assert!(dialog.confirmed);
        assert_eq!(dialog.result_charset, expected);
    }

    #[test]
    fn test_cycle_charset_wraps_both_directions() {
        let mut dialog = SystemFontPickerDialog::new();
        let start = dialog.charset_index;
        dialog.cycle_charset(false);
        assert_eq!(
            dialog.charset_index,
            (start + CHARSET_NAMES.len() - 1) % CHARSET_NAMES.len()
        );
        dialog.cycle_charset(true);
        assert_eq!(dialog.charset_index, start);
    }
}
