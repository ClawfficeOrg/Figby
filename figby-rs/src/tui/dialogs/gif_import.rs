use crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use super::super::theme::Theme;
use crate::gif_import::{probe_gif_dimensions, GifScaleTarget};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GifField {
    ImageWidth,
    ImageHeight,
    CanvasWidth,
    CanvasHeight,
    KeepProportions,
}

impl GifField {
    fn next(self) -> Self {
        match self {
            GifField::ImageWidth => GifField::ImageHeight,
            GifField::ImageHeight => GifField::CanvasWidth,
            GifField::CanvasWidth => GifField::CanvasHeight,
            GifField::CanvasHeight => GifField::KeepProportions,
            GifField::KeepProportions => GifField::ImageWidth,
        }
    }

    fn prev(self) -> Self {
        match self {
            GifField::ImageWidth => GifField::KeepProportions,
            GifField::ImageHeight => GifField::ImageWidth,
            GifField::CanvasWidth => GifField::ImageHeight,
            GifField::CanvasHeight => GifField::CanvasWidth,
            GifField::KeepProportions => GifField::CanvasHeight,
        }
    }
}

const DEFAULT_IMAGE_W: u16 = 80;
const DEFAULT_IMAGE_H: u16 = 24;

pub struct GifImportConfig {
    pub path: std::path::PathBuf,
    pub image_scale: GifScaleTarget,
    pub canvas_width: u16,
    pub canvas_height: u16,
}

pub struct GifImportDialog {
    pub active: bool,
    pub confirmed: bool,
    pub config: Option<GifImportConfig>,
    pub error_message: String,
    pub theme: Theme,

    // Phase 1: file browsing
    pub path_buffer: String,
    pub directory_entries: Vec<String>,
    pub selected_entry: usize,
    pub hide_dotfiles: bool,

    // GIF native dimensions (probed when file selected)
    pub native_width: u16,
    pub native_height: u16,

    // Phase 2: sizing options (string buffers for digit entry)
    pub image_w_buf: String,
    pub image_h_buf: String,
    pub canvas_w_buf: String,
    pub canvas_h_buf: String,
    pub keep_proportions: bool,
    pub selected_field: GifField,
}

impl GifImportDialog {
    pub fn new() -> Self {
        Self {
            active: false,
            confirmed: false,
            config: None,
            error_message: String::new(),
            theme: Theme::default(),
            path_buffer: String::new(),
            directory_entries: Vec::new(),
            selected_entry: 0,
            hide_dotfiles: true,
            native_width: 0,
            native_height: 0,
            image_w_buf: DEFAULT_IMAGE_W.to_string(),
            image_h_buf: DEFAULT_IMAGE_H.to_string(),
            canvas_w_buf: DEFAULT_IMAGE_W.to_string(),
            canvas_h_buf: DEFAULT_IMAGE_H.to_string(),
            keep_proportions: true,
            selected_field: GifField::ImageWidth,
        }
    }

    pub fn enter(&mut self) {
        self.active = true;
        self.confirmed = false;
        self.config = None;
        self.error_message.clear();
        self.path_buffer.clear();
        self.selected_entry = 0;
        self.directory_entries.clear();
        self.native_width = 0;
        self.native_height = 0;
        self.selected_field = GifField::ImageWidth;
        self.keep_proportions = true;
        self.refresh_directory();
    }

    pub fn close(&mut self) {
        self.active = false;
        self.confirmed = false;
        self.config = None;
        self.error_message.clear();
        self.path_buffer.clear();
        self.directory_entries.clear();
        self.selected_entry = 0;
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
            let lower = name.to_lowercase();
            if is_dir || lower.ends_with(".gif") {
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

    fn probe_file(&mut self) {
        let trimmed = self.path_buffer.trim().to_string();
        if trimmed.is_empty() {
            self.error_message = "No file selected".to_string();
            return;
        }
        let lower = trimmed.to_lowercase();
        if !lower.ends_with(".gif") {
            self.error_message = "Select a .gif file".to_string();
            return;
        }
        let path = std::path::PathBuf::from(&trimmed);
        if !path.exists() {
            self.error_message = "File not found".to_string();
            return;
        }

        match probe_gif_dimensions(&path) {
            Ok((w, h)) => {
                self.native_width = w.min(u16::MAX as usize) as u16;
                self.native_height = h.min(u16::MAX as usize) as u16;
                self.error_message.clear();

                // Default image size: FitBox(80, 24), or native if smaller
                let default =
                    GifScaleTarget::FitBox(DEFAULT_IMAGE_W as usize, DEFAULT_IMAGE_H as usize)
                        .resolve(w, h);
                let iw = default.0.min(u16::MAX as usize) as u16;
                let ih = default.1.min(u16::MAX as usize) as u16;
                self.image_w_buf = iw.to_string();
                self.image_h_buf = ih.to_string();
                self.canvas_w_buf = iw.to_string();
                self.canvas_h_buf = ih.to_string();
                self.selected_field = GifField::ImageWidth;
                self.keep_proportions = true;
            }
            Err(e) => {
                self.error_message = format!("Failed to read GIF: {e}");
            }
        }
    }

    fn update_height_from_width(&mut self) {
        if !self.keep_proportions || self.native_width == 0 || self.native_height == 0 {
            return;
        }
        let w: usize = self.image_w_buf.parse().unwrap_or(0);
        if w == 0 {
            return;
        }
        let resolved = GifScaleTarget::FitWidth(w)
            .resolve(self.native_width as usize, self.native_height as usize);
        let h = resolved.1.min(u16::MAX as usize) as u16;
        self.image_h_buf = h.to_string();
    }

    fn update_width_from_height(&mut self) {
        if !self.keep_proportions || self.native_width == 0 || self.native_height == 0 {
            return;
        }
        let h: usize = self.image_h_buf.parse().unwrap_or(0);
        if h == 0 {
            return;
        }
        // Invert FitWidth logic: given a target cell-height, find the width
        // that would produce it. h = (w * native_h / native_w * 0.5).ceil()
        // Reversing: w ≈ (h * native_w) / (native_h * 0.5)
        let native_w = self.native_width as f64;
        let native_h = self.native_height as f64;
        let w = ((h as f64 * native_w) / (native_h * 0.5)).round().max(1.0) as u16;
        self.image_w_buf = w.to_string();
    }

    fn parse_u16(buf: &str) -> Option<u16> {
        if buf.is_empty() {
            return None;
        }
        buf.parse::<u16>().ok().filter(|&v| v >= 1)
    }

    pub fn confirm(&mut self) {
        let path = std::path::PathBuf::from(self.path_buffer.trim());
        if !path.exists() {
            self.error_message = "File not found".to_string();
            self.selected_field = GifField::ImageWidth;
            return;
        }

        let iw = match Self::parse_u16(&self.image_w_buf) {
            Some(v) => v,
            None => {
                self.error_message = "Image width must be 1-65535".to_string();
                self.selected_field = GifField::ImageWidth;
                return;
            }
        };
        let ih = match Self::parse_u16(&self.image_h_buf) {
            Some(v) => v,
            None => {
                self.error_message = "Image height must be 1-65535".to_string();
                self.selected_field = GifField::ImageHeight;
                return;
            }
        };
        let cw = match Self::parse_u16(&self.canvas_w_buf) {
            Some(v) => v,
            None => {
                self.error_message = "Canvas width must be 1-65535".to_string();
                self.selected_field = GifField::CanvasWidth;
                return;
            }
        };
        let ch = match Self::parse_u16(&self.canvas_h_buf) {
            Some(v) => v,
            None => {
                self.error_message = "Canvas height must be 1-65535".to_string();
                self.selected_field = GifField::CanvasHeight;
                return;
            }
        };

        // F-12: a 65535×65535 canvas is 4.3 billion cells — reject anything
        // beyond the shared cell budget before any allocation.
        if !super::super::canvas::canvas_cells_within_budget(cw as usize, ch as usize) {
            self.error_message = format!(
                "Canvas {}×{} exceeds the {} cell budget",
                cw,
                ch,
                super::super::canvas::MAX_CANVAS_CELLS
            );
            return;
        }

        let image_scale = if self.keep_proportions {
            GifScaleTarget::FitBox(iw as usize, ih as usize)
        } else {
            GifScaleTarget::Exact(iw as usize, ih as usize)
        };

        self.config = Some(GifImportConfig {
            path,
            image_scale,
            canvas_width: cw,
            canvas_height: ch,
        });
        self.confirmed = true;
        self.active = false;
    }

    fn is_browsing(&self) -> bool {
        self.native_width == 0 || self.native_height == 0
    }

    pub fn handle_key(&mut self, code: KeyCode) -> bool {
        if self.is_browsing() {
            self.handle_key_browsing(code)
        } else {
            self.handle_key_options(code)
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
                        self.probe_file();
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
                    } else {
                        self.path_buffer = abs.to_string_lossy().to_string();
                        self.probe_file();
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
                self.confirm();
                true
            }
            KeyCode::Esc => {
                // Go back to browsing
                self.native_width = 0;
                self.native_height = 0;
                self.error_message.clear();
                true
            }
            KeyCode::Tab | KeyCode::Down => {
                self.selected_field = self.selected_field.next();
                true
            }
            KeyCode::BackTab | KeyCode::Up => {
                self.selected_field = self.selected_field.prev();
                true
            }
            KeyCode::Char(c) if c.is_ascii_digit() => {
                match self.selected_field {
                    GifField::ImageWidth => {
                        if self.image_w_buf.len() < 5 {
                            self.image_w_buf.push(c);
                            self.update_height_from_width();
                        }
                    }
                    GifField::ImageHeight => {
                        if self.image_h_buf.len() < 5 {
                            self.image_h_buf.push(c);
                            self.update_width_from_height();
                        }
                    }
                    GifField::CanvasWidth => {
                        if self.canvas_w_buf.len() < 5 {
                            self.canvas_w_buf.push(c);
                        }
                    }
                    GifField::CanvasHeight => {
                        if self.canvas_h_buf.len() < 5 {
                            self.canvas_h_buf.push(c);
                        }
                    }
                    GifField::KeepProportions => {}
                }
                self.error_message.clear();
                true
            }
            KeyCode::Backspace => {
                match self.selected_field {
                    GifField::ImageWidth => {
                        self.image_w_buf.pop();
                        self.update_height_from_width();
                    }
                    GifField::ImageHeight => {
                        self.image_h_buf.pop();
                        self.update_width_from_height();
                    }
                    GifField::CanvasWidth => {
                        self.canvas_w_buf.pop();
                    }
                    GifField::CanvasHeight => {
                        self.canvas_h_buf.pop();
                    }
                    GifField::KeepProportions => {}
                }
                self.error_message.clear();
                true
            }
            KeyCode::Char(' ') => {
                if self.selected_field == GifField::KeepProportions {
                    self.keep_proportions = !self.keep_proportions;
                    if self.keep_proportions {
                        self.update_height_from_width();
                    }
                }
                true
            }
            _ => false,
        }
    }

    fn field_style(&self, field: GifField) -> Style {
        if self.selected_field == field {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default()
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        frame.render_widget(Clear, area);
        let block = Block::default()
            .title(" Import GIF ")
            .borders(Borders::ALL)
            .style(Style::default().fg(self.theme.dialog.border_path));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        if inner.width < 36 || inner.height < 14 {
            return;
        }

        if self.is_browsing() {
            self.render_browser(frame, inner);
        } else {
            self.render_options(frame, inner);
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
                " (no .gif files in directory)",
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
            " Tab: select  Enter: load  Esc: cancel  \u{2191}\u{2193}: navigate",
            Style::default().fg(self.theme.dialog.meta),
        )));

        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, inner);
    }

    fn render_options(&self, frame: &mut Frame, inner: Rect) {
        let mut lines: Vec<Line> = Vec::new();

        // Original resolution (read-only)
        lines.push(Line::from(Span::styled(
            format!(" Original: {}x{} px", self.native_width, self.native_height),
            Style::default().fg(self.theme.dialog.meta),
        )));
        lines.push(Line::from(""));

        // Image size fields
        lines.push(Line::from(Span::styled(
            " Image Size:",
            Style::default().add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(vec![
            Span::styled("  Width:  ", Style::default()),
            Span::styled(
                format!(
                    "{}  ",
                    if self.image_w_buf.is_empty() {
                        "(enter)"
                    } else {
                        &self.image_w_buf
                    }
                ),
                self.field_style(GifField::ImageWidth),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  Height: ", Style::default()),
            Span::styled(
                format!(
                    "{}  ",
                    if self.image_h_buf.is_empty() {
                        "(enter)"
                    } else {
                        &self.image_h_buf
                    }
                ),
                self.field_style(GifField::ImageHeight),
            ),
        ]));

        lines.push(Line::from(""));

        // Canvas size fields
        lines.push(Line::from(Span::styled(
            " Canvas Size:",
            Style::default().add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(vec![
            Span::styled("  Width:  ", Style::default()),
            Span::styled(
                format!(
                    "{}  ",
                    if self.canvas_w_buf.is_empty() {
                        "(enter)"
                    } else {
                        &self.canvas_w_buf
                    }
                ),
                self.field_style(GifField::CanvasWidth),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  Height: ", Style::default()),
            Span::styled(
                format!(
                    "{}  ",
                    if self.canvas_h_buf.is_empty() {
                        "(enter)"
                    } else {
                        &self.canvas_h_buf
                    }
                ),
                self.field_style(GifField::CanvasHeight),
            ),
        ]));

        lines.push(Line::from(""));

        // Keep proportions toggle
        let prop_label = if self.keep_proportions {
            "Keep Proportions: [Yes]"
        } else {
            "Keep Proportions: [No] "
        };
        let prop_style = if self.selected_field == GifField::KeepProportions {
            Style::default()
                .fg(self.theme.dialog.highlight)
                .add_modifier(Modifier::REVERSED)
        } else {
            Style::default().fg(self.theme.dialog.highlight)
        };
        lines.push(Line::from(Span::styled(prop_label, prop_style)));

        if !self.error_message.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                format!(" Error: {}", self.error_message),
                Style::default().fg(self.theme.dialog.error),
            )));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            " Enter: import  Esc: back  Tab/\u{2191}\u{2193}: field  Space: toggle",
            Style::default().fg(self.theme.dialog.meta),
        )));

        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, inner);
    }
}

impl Default for GifImportDialog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gif_import_dialog_new() {
        let dlg = GifImportDialog::new();
        assert!(!dlg.active);
        assert_eq!(dlg.image_w_buf, "80");
        assert_eq!(dlg.image_h_buf, "24");
        assert_eq!(dlg.canvas_w_buf, "80");
        assert_eq!(dlg.canvas_h_buf, "24");
        assert!(dlg.keep_proportions);
    }

    #[test]
    fn test_enter_close() {
        let mut dlg = GifImportDialog::new();
        dlg.enter();
        assert!(dlg.active);
        assert!(dlg.is_browsing());
        dlg.close();
        assert!(!dlg.active);
    }

    #[test]
    fn test_field_cycle() {
        assert_eq!(GifField::ImageWidth.next(), GifField::ImageHeight);
        assert_eq!(GifField::ImageHeight.next(), GifField::CanvasWidth);
        assert_eq!(GifField::CanvasWidth.next(), GifField::CanvasHeight);
        assert_eq!(GifField::CanvasHeight.next(), GifField::KeepProportions);
        assert_eq!(GifField::KeepProportions.next(), GifField::ImageWidth);

        assert_eq!(GifField::ImageWidth.prev(), GifField::KeepProportions);
        assert_eq!(GifField::KeepProportions.prev(), GifField::CanvasHeight);
    }

    #[test]
    fn test_parse_u16() {
        assert_eq!(GifImportDialog::parse_u16("80"), Some(80));
        assert_eq!(GifImportDialog::parse_u16("0"), None);
        assert_eq!(GifImportDialog::parse_u16("abc"), None);
        assert_eq!(GifImportDialog::parse_u16(""), None);
    }

    #[test]
    fn test_digit_entry() {
        let mut dlg = GifImportDialog::new();
        dlg.native_width = 100;
        dlg.native_height = 50;
        dlg.image_w_buf.clear();

        dlg.handle_key(KeyCode::Char('4'));
        dlg.handle_key(KeyCode::Char('0'));
        assert_eq!(dlg.image_w_buf, "40");
    }

    #[test]
    fn test_backspace() {
        let mut dlg = GifImportDialog::new();
        dlg.native_width = 100;
        dlg.native_height = 50;
        dlg.image_w_buf = "80".to_string();
        dlg.handle_key(KeyCode::Backspace);
        assert_eq!(dlg.image_w_buf, "8");
    }

    #[test]
    fn test_esc_goes_back_to_browsing() {
        let mut dlg = GifImportDialog::new();
        dlg.native_width = 100;
        dlg.native_height = 50;
        assert!(!dlg.is_browsing());
        dlg.handle_key(KeyCode::Esc);
        assert!(dlg.is_browsing());
    }

    #[test]
    fn test_toggle_proportions() {
        let mut dlg = GifImportDialog::new();
        dlg.native_width = 100;
        dlg.native_height = 50;
        dlg.selected_field = GifField::KeepProportions;
        assert!(dlg.keep_proportions);
        dlg.handle_key(KeyCode::Char(' '));
        assert!(!dlg.keep_proportions);
        dlg.handle_key(KeyCode::Char(' '));
        assert!(dlg.keep_proportions);
    }

    #[test]
    fn test_confirm_without_path_fails() {
        let mut dlg = GifImportDialog::new();
        dlg.native_width = 100;
        dlg.native_height = 50;
        dlg.confirm();
        assert!(!dlg.confirmed);
        assert!(!dlg.error_message.is_empty());
    }

    /// F-12 (GPT review): a 65535×65535 canvas is ~4.3 billion cells and must
    /// be rejected by the cell budget instead of allocated.
    #[test]
    fn test_confirm_rejects_huge_canvas() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("x.gif");
        std::fs::write(&path, b"GIF89a").expect("write");

        let mut dlg = GifImportDialog::new();
        dlg.native_width = 8;
        dlg.native_height = 8;
        dlg.path_buffer = path.to_string_lossy().into_owned();
        dlg.image_w_buf = "8".to_string();
        dlg.image_h_buf = "8".to_string();
        dlg.canvas_w_buf = "65535".to_string();
        dlg.canvas_h_buf = "65535".to_string();
        dlg.confirm();
        assert!(!dlg.confirmed);
        assert!(
            dlg.error_message.contains("cell budget"),
            "got: {}",
            dlg.error_message
        );
    }

    #[test]
    fn test_confirm_accepts_small_canvas() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("x.gif");
        std::fs::write(&path, b"GIF89a").expect("write");

        let mut dlg = GifImportDialog::new();
        dlg.native_width = 8;
        dlg.native_height = 8;
        dlg.path_buffer = path.to_string_lossy().into_owned();
        dlg.image_w_buf = "8".to_string();
        dlg.image_h_buf = "8".to_string();
        dlg.canvas_w_buf = "40".to_string();
        dlg.canvas_h_buf = "24".to_string();
        dlg.confirm();
        assert!(dlg.confirmed);
    }
}
