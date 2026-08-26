use crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::font_gen::resolve_charset;
use crate::image_input::{
    bilinear_resize_rgb, load_rgb_matrix, luminance_to_char, rgb_to_luminance_matrix, RgbPixel,
};
use crate::tui::canvas::CanvasCell;
use crate::tui::theme::Theme;

const CHARSET_NAMES: &[&str] = &["block", "smooth", "full", "braille", "deluxe"];
const IMAGE_EXTENSIONS: &[&str] = &[".png", ".jpg", ".jpeg", ".bmp", ".webp", ".gif"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    Mono,
    Ansi256,
    Truecolor,
}

impl ColorMode {
    pub fn label(&self) -> &str {
        match self {
            ColorMode::Mono => "Mono",
            ColorMode::Ansi256 => "256",
            ColorMode::Truecolor => "Truecolor",
        }
    }

    pub fn cycle(&self) -> Self {
        match self {
            ColorMode::Mono => ColorMode::Ansi256,
            ColorMode::Ansi256 => ColorMode::Truecolor,
            ColorMode::Truecolor => ColorMode::Mono,
        }
    }
}

fn quantize_256(r: u8, g: u8, b: u8) -> ratatui::style::Color {
    let ri = (r as u16 * 5 / 255).min(5) as u8;
    let gi = (g as u16 * 5 / 255).min(5) as u8;
    let bi = (b as u16 * 5 / 255).min(5) as u8;
    let idx = 16 + ri * 36 + gi * 6 + bi;
    ratatui::style::Color::Indexed(idx)
}

fn image_ext_to_cells(
    rgb: &[Vec<RgbPixel>],
    charset: &[&str],
    color_mode: ColorMode,
) -> Vec<Vec<CanvasCell>> {
    if rgb.is_empty() || rgb[0].is_empty() {
        return Vec::new();
    }
    let char_str: String = charset.join("");
    let char_map = if char_str.is_empty() {
        " .-:=+*#%@"
    } else {
        &char_str
    };

    if color_mode == ColorMode::Mono {
        let lum = rgb_to_luminance_matrix(rgb);
        let mut cells = Vec::with_capacity(lum.len());
        for row in &lum {
            let mut cell_row = Vec::with_capacity(row.len());
            for &l in row {
                let ch = luminance_to_char(l, char_map);
                cell_row.push(CanvasCell {
                    ch,
                    fg: None,
                    bg: None,
                    height: None,
                });
            }
            cells.push(cell_row);
        }
        return cells;
    }

    let mut cells = Vec::with_capacity(rgb.len());
    for row in rgb {
        let mut cell_row = Vec::with_capacity(row.len());
        for &(r, g, b) in row {
            let luma = (0.2126 * r as f64 + 0.7152 * g as f64 + 0.0722 * b as f64).round() as u8;
            let ch = luminance_to_char(luma, char_map);
            let fg = match color_mode {
                ColorMode::Ansi256 => quantize_256(r, g, b),
                ColorMode::Truecolor => ratatui::style::Color::Rgb(r, g, b),
                ColorMode::Mono => unreachable!(),
            };
            cell_row.push(CanvasCell {
                ch,
                fg: Some(fg),
                bg: None,
                height: None,
            });
        }
        cells.push(cell_row);
    }
    cells
}

fn is_image_file(name: &str) -> bool {
    let lower = name.to_lowercase();
    IMAGE_EXTENSIONS.iter().any(|ext| lower.ends_with(ext))
}

pub struct RasciiImportDialog {
    pub active: bool,
    pub path_buffer: String,
    pub directory_entries: Vec<String>,
    pub selected_entry: usize,
    pub error_message: String,
    pub theme: Theme,
    pub charset_index: usize,
    pub output_width: u32,
    pub color_mode: ColorMode,
    pub preview_cells: Option<Vec<Vec<CanvasCell>>>,
    pub preview_dirty: bool,
    pub confirmed: bool,
    pub hide_dotfiles: bool,
}

impl RasciiImportDialog {
    pub fn new() -> Self {
        Self {
            active: false,
            path_buffer: String::new(),
            directory_entries: Vec::new(),
            selected_entry: 0,
            error_message: String::new(),
            theme: Theme::default(),
            charset_index: 0,
            output_width: 80,
            color_mode: ColorMode::Mono,
            preview_cells: None,
            preview_dirty: false,
            confirmed: false,
            hide_dotfiles: true,
        }
    }

    pub fn enter_import(&mut self) {
        self.active = true;
        self.path_buffer.clear();
        self.selected_entry = 0;
        self.error_message.clear();
        self.charset_index = 0;
        self.output_width = 80;
        self.color_mode = ColorMode::Mono;
        self.preview_cells = None;
        self.preview_dirty = false;
        self.confirmed = false;
        self.refresh_directory();
    }

    pub fn close(&mut self) {
        self.active = false;
        self.path_buffer.clear();
        self.directory_entries.clear();
        self.selected_entry = 0;
        self.error_message.clear();
        self.preview_cells = None;
        self.preview_dirty = false;
        self.confirmed = false;
    }

    fn refresh_directory(&mut self) {
        self.directory_entries.clear();
        self.selected_entry = 0;

        let parent = if self.path_buffer.is_empty() {
            std::path::PathBuf::from(".")
        } else {
            let p = std::path::PathBuf::from(&self.path_buffer);
            if p.is_dir() {
                p
            } else {
                p.parent()
                    .map(|pp| pp.to_path_buf())
                    .unwrap_or_else(|| std::path::PathBuf::from("."))
            }
        };

        let read_dir = match std::fs::read_dir(&parent) {
            Ok(rd) => rd,
            Err(_) => {
                return;
            }
        };

        let mut entries: Vec<String> = Vec::new();
        for entry in read_dir.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if self.hide_dotfiles && name.starts_with('.') {
                continue;
            }
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if is_dir || is_image_file(&name) {
                entries.push(name);
            }
        }
        entries.sort();
        self.directory_entries = entries;
    }

    fn select_entry(&mut self) {
        if self.selected_entry >= self.directory_entries.len() {
            return;
        }
        let entry = &self.directory_entries[self.selected_entry].clone();
        let parent = if self.path_buffer.is_empty() {
            std::path::PathBuf::from(".")
        } else {
            let p = std::path::PathBuf::from(&self.path_buffer);
            if p.is_dir() {
                p
            } else {
                p.parent()
                    .map(|pp| pp.to_path_buf())
                    .unwrap_or_else(|| std::path::PathBuf::from("."))
            }
        };
        let abs = parent.join(entry);
        self.path_buffer = abs.to_string_lossy().to_string();
        self.selected_entry = 0;
        self.error_message.clear();
        self.refresh_directory();
    }

    fn load_image(&mut self) {
        let path = self.path_buffer.trim().to_string();
        if path.is_empty() {
            self.error_message = "No file selected".to_string();
            return;
        }
        if !is_image_file(&path) {
            self.error_message = "Select an image file (.png, .jpg, .bmp, .webp, .gif)".to_string();
            return;
        }

        let rgb = match load_rgb_matrix(&path) {
            Ok(rgb) => rgb,
            Err(e) => {
                self.error_message = format!("Failed to load image: {e}");
                return;
            }
        };

        if rgb.is_empty() || rgb[0].is_empty() {
            self.error_message = "Loaded image is empty".to_string();
            return;
        }

        self.error_message.clear();
        self.generate_preview(&rgb);
    }

    fn generate_preview(&mut self, rgb: &[Vec<RgbPixel>]) {
        let src_h = rgb.len();
        let src_w = rgb[0].len();
        let target_h = ((self.output_width as f64 * src_h as f64 / src_w as f64) * 0.5)
            .ceil()
            .max(1.0) as usize;

        let resized = bilinear_resize_rgb(rgb, self.output_width as usize, target_h);

        let charset = resolve_charset(CHARSET_NAMES[self.charset_index])
            .unwrap_or(&["." as &str, "#" as &str]);

        self.preview_cells = Some(image_ext_to_cells(&resized, charset, self.color_mode));
        self.preview_dirty = false;
    }

    pub fn cycle_charset(&mut self) {
        self.charset_index = (self.charset_index + 1) % CHARSET_NAMES.len();
        self.preview_dirty = true;
    }

    pub fn cycle_color_mode(&mut self) {
        self.color_mode = self.color_mode.cycle();
        self.preview_dirty = true;
    }

    pub fn width_up(&mut self) {
        self.output_width = (self.output_width + 4).min(500);
        self.preview_dirty = true;
    }

    pub fn width_down(&mut self) {
        self.output_width = self.output_width.saturating_sub(4).max(8);
        self.preview_dirty = true;
    }

    pub fn handle_key(&mut self, code: KeyCode) -> bool {
        match self.preview_cells {
            None => self.handle_key_browsing(code),
            Some(_) => self.handle_key_options(code),
        }
    }

    fn handle_key_browsing(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Char(c) if !c.is_control() => {
                self.path_buffer.push(c);
                self.error_message.clear();
                self.selected_entry = 0;
                self.refresh_directory();
                true
            }
            KeyCode::Backspace => {
                self.path_buffer.pop();
                self.error_message.clear();
                self.selected_entry = 0;
                self.refresh_directory();
                true
            }
            KeyCode::Up => {
                if !self.directory_entries.is_empty() && self.selected_entry > 0 {
                    self.selected_entry -= 1;
                }
                true
            }
            KeyCode::Down => {
                if !self.directory_entries.is_empty()
                    && self.selected_entry < self.directory_entries.len() - 1
                {
                    self.selected_entry += 1;
                }
                true
            }
            KeyCode::Tab => {
                if !self.directory_entries.is_empty() {
                    self.select_entry();
                }
                true
            }
            KeyCode::Enter => {
                let trimmed = self.path_buffer.trim().to_string();
                if !trimmed.is_empty() {
                    let p = std::path::PathBuf::from(&trimmed);
                    if p.is_dir() {
                        self.selected_entry = 0;
                        self.error_message.clear();
                        self.refresh_directory();
                        return true;
                    }
                    if p.is_file() {
                        self.load_image();
                        return true;
                    }
                }
                if !self.directory_entries.is_empty() {
                    let entry = &self.directory_entries[self.selected_entry].clone();
                    let parent = if self.path_buffer.is_empty() {
                        std::path::PathBuf::from(".")
                    } else {
                        let p = std::path::PathBuf::from(&self.path_buffer);
                        if p.is_dir() {
                            p
                        } else {
                            p.parent()
                                .map(|pp| pp.to_path_buf())
                                .unwrap_or_else(|| std::path::PathBuf::from("."))
                        }
                    };
                    let abs = parent.join(entry);
                    if abs.is_dir() {
                        self.path_buffer = abs.to_string_lossy().to_string();
                        self.selected_entry = 0;
                        self.error_message.clear();
                        self.refresh_directory();
                    } else if is_image_file(entry) {
                        self.path_buffer = abs.to_string_lossy().to_string();
                        self.load_image();
                    }
                }
                true
            }
            KeyCode::Esc => {
                self.close();
                true
            }
            _ => false,
        }
    }

    fn handle_key_options(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Enter => {
                self.confirmed = true;
                self.active = false;
                true
            }
            KeyCode::Esc => {
                self.preview_cells = None;
                self.preview_dirty = false;
                self.confirmed = false;
                self.error_message.clear();
                true
            }
            KeyCode::Char('C') | KeyCode::Char('c') => {
                self.cycle_charset();
                if self.can_regenerate() {
                    self.regenerate_preview();
                }
                true
            }
            KeyCode::Char('T') | KeyCode::Char('t') => {
                self.cycle_color_mode();
                if self.can_regenerate() {
                    self.regenerate_preview();
                }
                true
            }
            KeyCode::Up | KeyCode::Char('+') | KeyCode::Char('=') => {
                self.width_up();
                if self.can_regenerate() {
                    self.regenerate_preview();
                }
                true
            }
            KeyCode::Down | KeyCode::Char('-') | KeyCode::Char('_') => {
                self.width_down();
                if self.can_regenerate() {
                    self.regenerate_preview();
                }
                true
            }
            _ => false,
        }
    }

    fn can_regenerate(&self) -> bool {
        self.preview_cells.is_some() && !self.path_buffer.trim().is_empty()
    }

    fn regenerate_preview(&mut self) {
        let path = self.path_buffer.trim();
        if let Ok(rgb) = load_rgb_matrix(path) {
            self.generate_preview(&rgb);
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        frame.render_widget(Clear, area);
        let block = Block::default()
            .title(" Convert Image to ASCII ")
            .borders(Borders::ALL)
            .style(Style::default().fg(self.theme.dialog.border_path));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        if inner.width < 30 || inner.height < 12 {
            return;
        }

        match self.preview_cells {
            None => self.render_browser(frame, inner),
            Some(ref cells) => self.render_options(frame, inner, cells),
        }
    }

    fn render_browser(&self, frame: &mut Frame, inner: Rect) {
        let mut lines: Vec<Line> = Vec::new();

        lines.push(Line::from(Span::styled(
            " Path:",
            Style::default().add_modifier(Modifier::BOLD),
        )));

        let path_display = if self.path_buffer.is_empty() {
            " (type path, browse with arrows)".to_string()
        } else {
            self.path_buffer.clone()
        };
        lines.push(Line::from(Span::styled(
            format!(" {}", path_display),
            Style::default().fg(self.theme.dialog.border_path),
        )));

        if !self.error_message.is_empty() {
            lines.push(Line::from(Span::styled(
                format!(" Error: {}", self.error_message),
                Style::default().fg(self.theme.dialog.error),
            )));
        }

        lines.push(Line::from(""));

        if self.directory_entries.is_empty() {
            lines.push(Line::from(Span::styled(
                " (no image files in directory)",
                Style::default().fg(self.theme.dialog.meta),
            )));
        } else {
            lines.push(Line::from(Span::styled(
                " Directory:",
                Style::default().add_modifier(Modifier::BOLD),
            )));

            let max_visible = (inner.height as usize).saturating_sub(6).min(10);
            let start = self.selected_entry.saturating_sub(max_visible / 2);
            let end = (start + max_visible).min(self.directory_entries.len());
            for i in start..end {
                let entry = &self.directory_entries[i];
                let is_selected = i == self.selected_entry;
                let parent = if self.path_buffer.is_empty() {
                    std::path::PathBuf::from(".")
                } else {
                    let p = std::path::PathBuf::from(&self.path_buffer);
                    if p.is_dir() {
                        p
                    } else {
                        p.parent()
                            .map(|pp| pp.to_path_buf())
                            .unwrap_or_else(|| std::path::PathBuf::from("."))
                    }
                };
                let is_dir = parent.join(entry).is_dir();
                let prefix = if is_selected { " >" } else { "  " };
                let suffix = if is_dir { "/" } else { "" };
                let text = format!("{prefix}{entry}{suffix}");
                let style = if is_selected {
                    Style::default().add_modifier(Modifier::REVERSED)
                } else {
                    Style::default()
                };
                lines.push(Line::from(Span::styled(text, style)));
            }
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            " Tab: select entry  Enter: load  Esc: cancel  \u{2191}\u{2193}: navigate",
            Style::default().fg(self.theme.dialog.meta),
        )));

        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, inner);
    }

    fn render_options(&self, frame: &mut Frame, inner: Rect, cells: &[Vec<CanvasCell>]) {
        let mut lines: Vec<Line> = Vec::new();

        let charset_name = CHARSET_NAMES[self.charset_index];
        lines.push(Line::from(Span::styled(
            format!(" Charset: [{charset_name}]  (C to cycle)"),
            Style::default().add_modifier(Modifier::BOLD),
        )));

        lines.push(Line::from(Span::styled(
            format!(" Width: [{}]  (+/- to adjust)", self.output_width),
            Style::default().fg(self.theme.dialog.meta),
        )));

        lines.push(Line::from(Span::styled(
            format!(" Color: [{}]  (T to cycle)", self.color_mode.label()),
            Style::default().fg(self.theme.dialog.meta),
        )));

        if !self.error_message.is_empty() {
            lines.push(Line::from(Span::styled(
                format!(" Error: {}", self.error_message),
                Style::default().fg(self.theme.dialog.error),
            )));
        }

        let preview_h = inner.height as usize;
        let max_preview_lines = preview_h.saturating_sub(lines.len() + 3);
        let mut preview_lines: Vec<Line> = Vec::new();

        if !cells.is_empty() {
            for row in cells.iter().take(max_preview_lines) {
                let s: String = row.iter().map(|c| c.ch).collect();
                let truncated = if s.chars().count() > inner.width as usize {
                    // Truncate at a char boundary — `s` can contain multi-byte
                    // chars, so `&s[..n]` could panic mid-char (GPT review F-17).
                    let take = inner.width.saturating_sub(3) as usize;
                    format!("{}...", s.chars().take(take).collect::<String>())
                } else {
                    s
                };
                preview_lines.push(Line::from(Span::styled(
                    truncated,
                    Style::default().fg(self.theme.dialog.label),
                )));
            }
            if cells.len() > max_preview_lines {
                preview_lines.push(Line::from(Span::styled(
                    format!(" ... ({} rows total)", cells.len()),
                    Style::default().fg(self.theme.dialog.meta),
                )));
            }
        }

        lines.extend(preview_lines);

        lines.push(Line::from(""));
        let hint = if self.confirmed {
            " Importing...".to_string()
        } else {
            format!(
                " {}x{}  Enter: import  Esc: back  C:charset  T:color  +/-:width",
                cells.first().map(|r| r.len()).unwrap_or(0),
                cells.len()
            )
        };
        lines.push(Line::from(Span::styled(
            hint,
            Style::default().fg(self.theme.dialog.meta),
        )));

        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, inner);
    }
}

impl Default for RasciiImportDialog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rascii_import_dialog_new() {
        let dialog = RasciiImportDialog::new();
        assert!(!dialog.active);
        assert_eq!(dialog.charset_index, 0);
        assert_eq!(dialog.output_width, 80);
        assert_eq!(dialog.color_mode, ColorMode::Mono);
        assert!(dialog.preview_cells.is_none());
        assert!(dialog.path_buffer.is_empty());
    }

    #[test]
    fn test_rascii_import_enter_close() {
        let mut dialog = RasciiImportDialog::new();
        dialog.enter_import();
        assert!(dialog.active);
        dialog.close();
        assert!(!dialog.active);
    }

    #[test]
    fn test_rascii_import_charset_cycle() {
        let mut dialog = RasciiImportDialog::new();
        assert_eq!(dialog.charset_index, 0);
        dialog.cycle_charset();
        assert_eq!(dialog.charset_index, 1);
        for _ in 0..4 {
            dialog.cycle_charset();
        }
        assert_eq!(dialog.charset_index, 0);
    }

    #[test]
    fn test_rascii_import_width_adjust() {
        let mut dialog = RasciiImportDialog::new();
        assert_eq!(dialog.output_width, 80);
        dialog.width_up();
        assert_eq!(dialog.output_width, 84);
        dialog.width_down();
        assert_eq!(dialog.output_width, 80);

        // clamped to [8, 500]
        dialog.output_width = 500;
        dialog.width_up();
        assert_eq!(dialog.output_width, 500);
        dialog.output_width = 8;
        dialog.width_down();
        assert_eq!(dialog.output_width, 8);
    }

    #[test]
    fn test_rascii_import_color_mode_cycle() {
        let mut dialog = RasciiImportDialog::new();
        assert_eq!(dialog.color_mode, ColorMode::Mono);
        dialog.cycle_color_mode();
        assert_eq!(dialog.color_mode, ColorMode::Ansi256);
        dialog.cycle_color_mode();
        assert_eq!(dialog.color_mode, ColorMode::Truecolor);
        dialog.cycle_color_mode();
        assert_eq!(dialog.color_mode, ColorMode::Mono);
    }

    #[test]
    fn test_rascii_import_path_entry() {
        let mut dialog = RasciiImportDialog::new();
        dialog.enter_import();
        dialog.handle_key(KeyCode::Char('t'));
        dialog.handle_key(KeyCode::Char('e'));
        dialog.handle_key(KeyCode::Char('s'));
        dialog.handle_key(KeyCode::Char('t'));
        assert_eq!(dialog.path_buffer, "test");
        dialog.handle_key(KeyCode::Backspace);
        assert_eq!(dialog.path_buffer, "tes");
    }

    #[test]
    fn test_rascii_import_esc_cancels() {
        let mut dialog = RasciiImportDialog::new();
        dialog.enter_import();
        assert!(dialog.active);
        dialog.handle_key(KeyCode::Esc);
        assert!(!dialog.active);
    }

    #[test]
    fn test_rascii_import_preview_dirty_on_options_change() {
        let mut dialog = RasciiImportDialog::new();
        assert!(!dialog.preview_dirty);
        dialog.cycle_charset();
        assert!(dialog.preview_dirty);
        dialog.preview_dirty = false;
        dialog.cycle_color_mode();
        assert!(dialog.preview_dirty);
        dialog.preview_dirty = false;
        dialog.width_up();
        assert!(dialog.preview_dirty);
    }

    #[test]
    fn test_rascii_import_confirm_without_path_noop() {
        let mut dialog = RasciiImportDialog::new();
        dialog.enter_import();
        dialog.handle_key(KeyCode::Enter);
        // Still in browsing mode, not confirmed
        assert!(dialog.active);
        assert!(!dialog.confirmed);
    }

    #[test]
    fn test_color_mode_labels() {
        assert_eq!(ColorMode::Mono.label(), "Mono");
        assert_eq!(ColorMode::Ansi256.label(), "256");
        assert_eq!(ColorMode::Truecolor.label(), "Truecolor");
    }

    #[test]
    fn test_is_image_file() {
        assert!(is_image_file("photo.png"));
        assert!(is_image_file("photo.jpg"));
        assert!(is_image_file("photo.jpeg"));
        assert!(is_image_file("photo.bmp"));
        assert!(is_image_file("photo.webp"));
        assert!(is_image_file("photo.gif"));
        assert!(!is_image_file("font.flf"));
        assert!(!is_image_file("text.txt"));
        assert!(!is_image_file("dir/"));
    }

    #[test]
    fn test_quantize_256_black() {
        let c = quantize_256(0, 0, 0);
        assert_eq!(c, ratatui::style::Color::Indexed(16));
    }

    #[test]
    fn test_quantize_256_white() {
        let c = quantize_256(255, 255, 255);
        assert_eq!(c, ratatui::style::Color::Indexed(231));
    }

    #[test]
    fn test_quantize_256_red() {
        let c = quantize_256(255, 0, 0);
        assert_eq!(c, ratatui::style::Color::Indexed(196));
    }

    #[test]
    fn test_esc_from_options_returns_to_browsing() {
        let mut dialog = RasciiImportDialog::new();
        dialog.enter_import();
        dialog.preview_cells = Some(vec![vec![CanvasCell::default()]]);
        dialog.handle_key(KeyCode::Esc);
        // Should go back to browsing, not close
        assert!(dialog.active);
        assert!(dialog.preview_cells.is_none());
    }
}
