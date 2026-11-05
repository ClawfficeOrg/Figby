use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;
use std::path::PathBuf;

use super::super::theme::Theme;
use super::system_font::CHARSET_NAMES;

const DEFAULT_SIZE: &str = "12";
const DEFAULT_CHARSET_INDEX: usize = 0; // "default" — matches font_file_to_figfont's prior hardcoded charset

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Size,
    Charset,
}

/// Size/charset options for the "New Font from File" (TTF/OTF import) flow,
/// shown after a file is picked and before conversion runs. Previously this
/// flow had no options step at all — size and charset were hardcoded.
pub struct FontImportOptionsDialog {
    pub active: bool,
    pub path: PathBuf,
    pub size_buffer: String,
    pub charset_index: usize,
    pub selected_field: Field,
    pub error_message: String,
    pub confirmed: bool,
    pub result_size: f32,
    pub result_charset: String,
    pub theme: Theme,
}

impl FontImportOptionsDialog {
    pub fn new() -> Self {
        Self {
            active: false,
            path: PathBuf::new(),
            size_buffer: DEFAULT_SIZE.to_string(),
            charset_index: DEFAULT_CHARSET_INDEX,
            selected_field: Field::Size,
            error_message: String::new(),
            confirmed: false,
            result_size: 12.0,
            result_charset: CHARSET_NAMES[DEFAULT_CHARSET_INDEX].to_string(),
            theme: Theme::default(),
        }
    }

    pub fn enter(&mut self, path: PathBuf) {
        self.active = true;
        self.path = path;
        self.size_buffer = DEFAULT_SIZE.to_string();
        self.charset_index = DEFAULT_CHARSET_INDEX;
        self.selected_field = Field::Size;
        self.error_message.clear();
        self.confirmed = false;
    }

    pub fn close(&mut self) {
        self.active = false;
        self.confirmed = false;
    }

    pub fn cycle_charset(&mut self, forward: bool) {
        let len = CHARSET_NAMES.len();
        self.charset_index = if forward {
            (self.charset_index + 1) % len
        } else {
            (self.charset_index + len - 1) % len
        };
    }

    fn parse_size(&self) -> Option<f32> {
        self.size_buffer
            .parse::<f32>()
            .ok()
            .filter(|v| (4.0..=200.0).contains(v))
    }

    pub fn confirm(&mut self) {
        let Some(size) = self.parse_size() else {
            self.error_message = "Size must be 4-200".to_string();
            self.selected_field = Field::Size;
            return;
        };
        self.result_size = size;
        self.result_charset = CHARSET_NAMES[self.charset_index].to_string();
        self.confirmed = true;
        self.active = false;
    }

    pub fn handle_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Tab => {
                self.selected_field = match self.selected_field {
                    Field::Size => Field::Charset,
                    Field::Charset => Field::Size,
                };
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
            KeyCode::Backspace if self.selected_field == Field::Size => {
                self.size_buffer.pop();
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

impl Default for FontImportOptionsDialog {
    fn default() -> Self {
        Self::new()
    }
}

pub fn render_font_import_options_dialog(
    dialog: &FontImportOptionsDialog,
    frame: &mut Frame,
    area: Rect,
) {
    frame.render_widget(Clear, area);
    let block = Block::default()
        .title(" Import Font Options ")
        .borders(Borders::ALL)
        .style(Style::default().fg(dialog.theme.dialog.highlight));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width < 30 || inner.height < 8 {
        return;
    }

    let mut lines: Vec<Line> = Vec::new();

    let file_name = dialog
        .path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    lines.push(Line::from(vec![
        Span::styled(" File: ", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(file_name),
    ]));
    lines.push(Line::from(""));

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

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " Enter: import  Esc: cancel  Tab: switch field  \u{2190}\u{2192}: change",
        Style::default().fg(dialog.theme.dialog.meta),
    )));

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_defaults() {
        let dialog = FontImportOptionsDialog::new();
        assert!(!dialog.active);
        assert_eq!(dialog.size_buffer, "12");
        assert_eq!(CHARSET_NAMES[dialog.charset_index], "default");
    }

    #[test]
    fn test_enter_sets_path_and_activates() {
        let mut dialog = FontImportOptionsDialog::new();
        dialog.enter(PathBuf::from("/tmp/MyFont.ttf"));
        assert!(dialog.active);
        assert_eq!(dialog.path, PathBuf::from("/tmp/MyFont.ttf"));
        assert!(!dialog.confirmed);
    }

    #[test]
    fn test_confirm_rejects_out_of_range_size() {
        let mut dialog = FontImportOptionsDialog::new();
        dialog.enter(PathBuf::from("/tmp/f.ttf"));
        dialog.size_buffer = "1".to_string();
        dialog.confirm();
        assert!(!dialog.confirmed);
        assert!(dialog.active, "dialog stays open on invalid size");
        assert!(!dialog.error_message.is_empty());
    }

    #[test]
    fn test_confirm_succeeds_with_valid_size_and_charset() {
        let mut dialog = FontImportOptionsDialog::new();
        dialog.enter(PathBuf::from("/tmp/f.ttf"));
        dialog.size_buffer = "24".to_string();
        dialog.cycle_charset(true);
        let expected_charset = CHARSET_NAMES[dialog.charset_index].to_string();
        dialog.confirm();
        assert!(dialog.confirmed);
        assert!(!dialog.active);
        assert_eq!(dialog.result_size, 24.0);
        assert_eq!(dialog.result_charset, expected_charset);
    }

    #[test]
    fn test_cycle_charset_wraps_both_directions() {
        let mut dialog = FontImportOptionsDialog::new();
        let start = dialog.charset_index;
        dialog.cycle_charset(false);
        assert_eq!(dialog.charset_index, CHARSET_NAMES.len() - 1);
        dialog.cycle_charset(true);
        assert_eq!(dialog.charset_index, start);
    }

    #[test]
    fn test_tab_switches_field() {
        let mut dialog = FontImportOptionsDialog::new();
        assert_eq!(dialog.selected_field, Field::Size);
        dialog.handle_key(KeyCode::Tab);
        assert_eq!(dialog.selected_field, Field::Charset);
        dialog.handle_key(KeyCode::Tab);
        assert_eq!(dialog.selected_field, Field::Size);
    }

    #[test]
    fn test_esc_closes_without_confirming() {
        let mut dialog = FontImportOptionsDialog::new();
        dialog.enter(PathBuf::from("/tmp/f.ttf"));
        dialog.handle_key(KeyCode::Esc);
        assert!(!dialog.active);
        assert!(!dialog.confirmed);
    }
}
