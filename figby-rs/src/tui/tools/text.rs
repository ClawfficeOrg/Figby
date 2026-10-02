use crate::font::{load_font, FIGfont};
use crate::render::{add_char, Justification};
use crate::smush::SmushMode;
use crate::tui::canvas::{CanvasBuffer, CanvasCell, TextOverlay};
use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TextBlock {
    pub id: usize,
    pub text: String,
    pub font_index: usize,
    pub x: i16,
    pub y: i16,
    pub scale: u8,
    pub justification: Justification,
    pub text_color: Option<Color>,
    pub rotation: u16,
    pub cached_rows: Vec<String>,
    pub width: usize,
    pub height: usize,
}

#[derive(Debug, Clone)]
pub struct TextToolState {
    pub editing: bool,
    pub text_buffer: String,
    pub font_index: usize,
    pub available_fonts: Vec<String>,
    pub font: Option<FIGfont>,
    pub justification: Justification,
    pub text_color: Option<Color>,
    pub scale: u8,
    pub preview_pos: (i16, i16),
    pub show_preview: bool,
    pub blocks: Vec<TextBlock>,
    pub selected_block: Option<usize>,
    font_dir: String,
    next_block_id: usize,
}

impl TextToolState {
    pub fn new(font_dir: &str) -> Self {
        let available = list_available_fonts(font_dir);
        Self {
            editing: false,
            text_buffer: String::new(),
            font_index: 0,
            available_fonts: available,
            font: None,
            justification: Justification::Left,
            text_color: None,
            scale: 1,
            preview_pos: (0, 0),
            show_preview: false,
            blocks: Vec::new(),
            selected_block: None,
            font_dir: font_dir.to_string(),
            next_block_id: 0,
        }
    }

    pub fn load_selected_font(&mut self) {
        if self.available_fonts.is_empty() {
            self.font = None;
            return;
        }
        let idx = self.font_index.min(self.available_fonts.len() - 1);
        let name = &self.available_fonts[idx];
        let mut dirs: Vec<&str> = vec![&self.font_dir];
        dirs.extend(crate::font::DEFAULT_FONT_DIRS);
        if let Ok(f) = load_font(name, &dirs) {
            self.font = Some(f);
        } else {
            self.font = None;
        }
    }

    /// Restore persisted text blocks (figmap load). Clamps `font_index` into
    /// this session's font list and rebuilds `next_block_id` so new commits
    /// never collide with restored ids.
    pub fn restore_blocks(&mut self, blocks: Vec<TextBlock>) {
        let nfonts = self.available_fonts.len();
        self.blocks = blocks
            .into_iter()
            .map(|mut b| {
                if b.font_index >= nfonts {
                    b.font_index = 0;
                }
                b
            })
            .collect();
        self.selected_block = None;
        self.editing = false;
        self.text_buffer.clear();
        self.next_block_id = self.blocks.iter().map(|b| b.id + 1).max().unwrap_or(0);
    }

    pub(crate) fn render_rows_from_buffer(&mut self) -> Option<(Vec<String>, usize)> {
        if self.text_buffer.is_empty() {
            return None;
        }
        if self.font.is_none() {
            self.load_selected_font();
        }
        let font = self.font.as_ref()?;
        if font.chars.is_empty() {
            return None;
        }
        let height = font.charheight as usize;
        if height == 0 {
            return None;
        }
        let mut output_rows = vec![String::new(); height];
        let mut outlinelen = 0;
        let mut prev_width = 0;
        let mode = if font.full_layout >= 0 {
            SmushMode::new(font.full_layout as u32)
        } else {
            SmushMode::new(SmushMode::KERN)
        };
        let limit = 9999;

        for c in self.text_buffer.chars() {
            let code = c as u32;
            add_char(
                font,
                code,
                &mut output_rows,
                &mut outlinelen,
                &mut prev_width,
                mode,
                false,
                limit,
            );
        }

        let width = output_rows[0].chars().count();
        if width == 0 {
            return None;
        }
        Some((output_rows, width))
    }

    pub fn commit_block(&mut self) {
        if self.text_buffer.is_empty() {
            return;
        }
        let (rows, width) = match self.render_rows_from_buffer() {
            Some(v) => v,
            None => return,
        };
        let height = rows.len();
        let id = self.next_block_id;
        self.next_block_id += 1;
        let block = TextBlock {
            id,
            text: self.text_buffer.clone(),
            font_index: self.font_index,
            x: self.preview_pos.0,
            y: self.preview_pos.1,
            scale: self.scale,
            justification: self.justification,
            text_color: self.text_color,
            rotation: 0,
            cached_rows: rows,
            width,
            height,
        };
        self.blocks.push(block);
        self.selected_block = Some(self.blocks.len() - 1);
        self.show_preview = false;
    }

    pub fn re_edit_block(&mut self, idx: usize) {
        if idx >= self.blocks.len() {
            return;
        }
        let block = self.blocks.remove(idx);
        self.text_buffer = block.text;
        self.font_index = block.font_index;
        self.justification = block.justification;
        self.text_color = block.text_color;
        self.scale = block.scale;
        self.preview_pos = (block.x, block.y);
        self.selected_block = None;
        self.editing = true;
        self.load_selected_font();
    }

    /// Render every committed block into `buffer` WITHOUT removing them.
    /// Used before export / player capture so output sees text pixels while
    /// the editable text objects stay live in the document.
    pub fn bake_blocks_into(&self, buffer: &mut CanvasBuffer) {
        for idx in 0..self.blocks.len() {
            let block = &self.blocks[idx];
            let scale = block.scale.max(1) as usize;
            let (bx, by, _bw, _bh) = self.compute_bounding_box(idx);
            for (oy, row) in block.cached_rows.iter().enumerate() {
                for (ox, ch) in row.chars().enumerate() {
                    if ch == ' ' {
                        continue;
                    }
                    for dy in 0..scale {
                        for dx in 0..scale {
                            let cell_x = bx as usize + ox * scale + dx;
                            let cell_y = by as usize + oy * scale + dy;
                            if cell_x < buffer.width() && cell_y < buffer.height() {
                                let cell = CanvasCell {
                                    ch,
                                    fg: block.text_color,
                                    bg: None,
                                    height: Some(255),
                                };
                                buffer.set(cell_x, cell_y, cell);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Render selected block into canvas buffer and remove the block.
    pub fn rasterize_selected_block(&mut self, buffer: &mut CanvasBuffer) {
        let idx = match self.selected_block {
            Some(i) if i < self.blocks.len() => i,
            _ => return,
        };
        let block = &self.blocks[idx];
        let scale = block.scale.max(1) as usize;
        let (bx, by, _bw, _bh) = self.compute_bounding_box(idx);
        for (oy, row) in block.cached_rows.iter().enumerate() {
            for (ox, ch) in row.chars().enumerate() {
                if ch == ' ' {
                    continue;
                }
                for dy in 0..scale {
                    for dx in 0..scale {
                        let cell_x = bx as usize + ox * scale + dx;
                        let cell_y = by as usize + oy * scale + dy;
                        if cell_x < buffer.width() && cell_y < buffer.height() {
                            let cell = CanvasCell {
                                ch,
                                fg: block.text_color,
                                bg: None,
                                height: Some(255),
                            };
                            buffer.set(cell_x, cell_y, cell);
                        }
                    }
                }
            }
        }
        self.blocks.remove(idx);
        self.selected_block = None;
    }

    /// Place text at a specific canvas position (used by click-to-place).
    pub fn place_at(&mut self, x: i16, y: i16) {
        if self.text_buffer.is_empty() {
            return;
        }
        self.preview_pos = (x, y);
        self.commit_block();
    }

    /// Cycle to next available font.
    pub fn next_font(&mut self) {
        if self.available_fonts.is_empty() {
            return;
        }
        self.font_index = (self.font_index + 1).min(self.available_fonts.len() - 1);
        self.load_selected_font();
    }

    /// Cycle to previous available font.
    pub fn prev_font(&mut self) {
        if self.available_fonts.is_empty() {
            return;
        }
        self.font_index = self.font_index.saturating_sub(1);
        self.load_selected_font();
    }

    pub fn hit_test(&self, x: i16, y: i16) -> Option<usize> {
        for i in 0..self.blocks.len() {
            let (bx, by, bw, bh) = self.compute_bounding_box(i);
            if x >= bx && x < bx + bw as i16 && y >= by && y < by + bh as i16 {
                return Some(i);
            }
        }
        None
    }

    pub fn move_selected_block(&mut self, dx: i16, dy: i16) {
        if let Some(idx) = self.selected_block {
            if idx < self.blocks.len() {
                self.blocks[idx].x = self.blocks[idx].x.wrapping_add(dx);
                self.blocks[idx].y = self.blocks[idx].y.wrapping_add(dy);
            }
        }
    }

    pub fn scale_selected_block(&mut self, delta: i8) {
        if let Some(idx) = self.selected_block {
            if idx >= self.blocks.len() {
                return;
            }
            let new_scale = self.blocks[idx].scale as i8 + delta;
            if !(1..=4).contains(&new_scale) {
                return;
            }
            self.blocks[idx].scale = new_scale as u8;
        }
    }

    pub fn rotate_selected_block(&mut self) {
        if let Some(idx) = self.selected_block {
            if idx < self.blocks.len() {
                self.blocks[idx].rotation = (self.blocks[idx].rotation + 90) % 360;
            }
        }
    }

    pub fn delete_selected_block(&mut self) {
        if let Some(idx) = self.selected_block {
            if idx < self.blocks.len() {
                self.blocks.remove(idx);
                self.selected_block = None;
            }
        }
    }

    pub fn compute_bounding_box(&self, idx: usize) -> (i16, i16, usize, usize) {
        if idx >= self.blocks.len() {
            return (0, 0, 0, 0);
        }
        let block = &self.blocks[idx];
        let scale = block.scale.max(1) as usize;
        let (bb_w, bb_h) = match block.rotation {
            90 | 270 => (block.height * scale, block.width * scale),
            _ => (block.width * scale, block.height * scale),
        };
        let left_x = match block.justification {
            Justification::Left => block.x,
            Justification::Center => block.x - (bb_w as i16 / 2),
            Justification::Right => block.x - bb_w as i16,
        };
        (left_x, block.y, bb_w, bb_h)
    }

    pub fn render_block_to_overlay(&self, idx: usize) -> Option<TextOverlay> {
        if idx >= self.blocks.len() {
            return None;
        }
        let block = &self.blocks[idx];
        let scale = block.scale.max(1) as usize;
        let bb_w = match block.rotation {
            90 | 270 => block.height * scale,
            _ => block.width * scale,
        };
        let left_x = match block.justification {
            Justification::Left => block.x,
            Justification::Center => block.x - (bb_w as i16 / 2),
            Justification::Right => block.x - bb_w as i16,
        };
        Some(TextOverlay {
            x: left_x,
            y: block.y,
            rows: block.cached_rows.clone(),
            color: block.text_color,
            scale: block.scale,
            rotation: block.rotation,
        })
    }

    pub fn render_text_to_buffer(&mut self, buffer: &mut CanvasBuffer) {
        let (output_rows, width) = match self.render_rows_from_buffer() {
            Some(v) => v,
            None => return,
        };

        let font = match self.font.as_ref() {
            Some(f) => f,
            None => return,
        };

        let (cx, cy) = self.preview_pos;
        let left_x = match self.justification {
            Justification::Left => cx,
            Justification::Center => cx - (width as i16 / 2),
            Justification::Right => cx - width as i16,
        };

        let scale = self.scale.max(1) as i16;

        for (oy, row) in output_rows.iter().enumerate() {
            for (ox, ch) in row.chars().enumerate() {
                let cell_char = if ch == font.hardblank { ' ' } else { ch };
                if cell_char == ' ' {
                    continue;
                }
                let base_x = left_x + scale * ox as i16;
                let base_y = cy + scale * oy as i16;
                for dy in 0..scale {
                    for dx in 0..scale {
                        let bx = base_x + dx;
                        let by = base_y + dy;
                        if bx >= 0 && by >= 0 {
                            let cell = CanvasCell {
                                ch: cell_char,
                                fg: self.text_color,
                                bg: None,
                                height: Some(255),
                            };
                            buffer.set(bx as usize, by as usize, cell);
                        }
                    }
                }
            }
        }
    }

    pub fn render_options(&self, frame: &mut Frame<'_>, area: Rect, borders: Borders) {
        let block = Block::default()
            .title(" Text ")
            .borders(borders)
            .title_style(Style::default().add_modifier(Modifier::BOLD));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        if inner.width < 2 || inner.height < 2 {
            return;
        }

        let mut lines: Vec<Line<'_>> = Vec::new();

        let font_name = if self.font_index < self.available_fonts.len() {
            &self.available_fonts[self.font_index]
        } else {
            "?"
        };
        lines.push(Line::from(vec![
            Span::styled("Font:", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format!(" {}", font_name)),
        ]));

        let just_str = match self.justification {
            Justification::Left => "Left",
            Justification::Center => "Center",
            Justification::Right => "Right",
        };
        lines.push(Line::from(vec![
            Span::styled("Just:", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format!(" {}", just_str)),
        ]));

        lines.push(Line::from(vec![
            Span::styled("Scale:", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format!(" {}", self.scale)),
        ]));

        lines.push(Line::from(Span::raw("")));

        if self.editing {
            let display = if self.text_buffer.is_empty() {
                "(type in sidebar)".to_string()
            } else {
                self.text_buffer.clone()
            };
            lines.push(Line::from(vec![Span::styled(
                "Editing:",
                Style::default().add_modifier(Modifier::BOLD),
            )]));
            lines.push(Line::from(Span::raw(format!(" {}", display))));
        } else {
            let hint = if self.text_buffer.is_empty() {
                "Type text in sidebar, hover to preview"
            } else {
                "Hover canvas to preview, click to place"
            };
            lines.push(Line::from(Span::raw(format!(" {}", hint))));
        }

        let max_lines = inner.height as usize;
        let truncated: Vec<Line<'_>> = lines.into_iter().take(max_lines).collect();
        let paragraph = Paragraph::new(truncated);
        frame.render_widget(paragraph, inner);
    }

    /// Returns `Some(undo_label)` if handled with action that needs undo (label != "").
    /// Returns `Some("")` if handled but no undo needed.
    /// Returns `None` if not handled.
    pub(crate) fn handle_key(
        &mut self,
        code: KeyCode,
        modifiers: KeyModifiers,
        _canvas_cursor: (u16, u16),
    ) -> Option<&'static str> {
        // Text editing mode (sidebar text field active)
        if self.editing {
            match code {
                KeyCode::Enter => {
                    self.commit_block();
                    self.editing = false;
                    return Some("Commit text");
                }
                KeyCode::Esc => {
                    self.text_buffer.clear();
                    self.editing = false;
                    return Some("");
                }
                KeyCode::Backspace => {
                    self.text_buffer.pop();
                    return Some("");
                }
                KeyCode::Char(c) if !modifiers.contains(KeyModifiers::CONTROL) => {
                    self.text_buffer.push(c);
                    return Some("");
                }
                _ => {}
            }
        }

        // Block operations (selected block, no editing)
        if !self.editing && self.selected_block.is_some() {
            match code {
                KeyCode::Up => {
                    self.move_selected_block(0, -1);
                    return Some("Move text block");
                }
                KeyCode::Down => {
                    self.move_selected_block(0, 1);
                    return Some("Move text block");
                }
                KeyCode::Left => {
                    self.move_selected_block(-1, 0);
                    return Some("Move text block");
                }
                KeyCode::Right => {
                    self.move_selected_block(1, 0);
                    return Some("Move text block");
                }
                KeyCode::Char('+') | KeyCode::Char('=') => {
                    self.scale_selected_block(1);
                    return Some("Scale text block");
                }
                KeyCode::Char('-') | KeyCode::Char('_') => {
                    self.scale_selected_block(-1);
                    return Some("Scale text block");
                }
                KeyCode::Char('r') | KeyCode::Char('R') => {
                    self.rotate_selected_block();
                    return Some("Rotate text block");
                }
                KeyCode::Delete | KeyCode::Backspace => {
                    self.delete_selected_block();
                    return Some("Delete text block");
                }
                KeyCode::Char(' ') | KeyCode::Enter => {
                    if let Some(idx) = self.selected_block {
                        self.re_edit_block(idx);
                    }
                    return Some("");
                }
                KeyCode::Esc => {
                    self.selected_block = None;
                    return Some("");
                }
                _ => {}
            }
        }

        None
    }
}

impl Default for TextToolState {
    fn default() -> Self {
        Self::new("fonts")
    }
}

pub fn list_available_fonts(font_dir: &str) -> Vec<String> {
    let mut fonts = Vec::new();
    if let Ok(entries) = std::fs::read_dir(font_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension() {
                if ext == "flf" || ext == "tlf" {
                    if let Some(stem) = path.file_stem() {
                        let name = stem.to_string_lossy().to_string();
                        fonts.push(name);
                    }
                }
            }
        }
    }
    fonts.sort();
    fonts.dedup();
    fonts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::canvas::CanvasBuffer;
    use ratatui::style::Color;

    fn test_font_dir() -> &'static str {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../fonts")
    }

    #[test]
    fn test_list_fonts_nonempty() {
        let fonts = list_available_fonts(test_font_dir());
        assert!(!fonts.is_empty(), "expected at least one font");
        assert!(
            fonts.contains(&"standard".to_string()),
            "expected 'standard' font"
        );
    }

    #[test]
    fn test_list_fonts_nonexistent_dir() {
        let fonts = list_available_fonts("/nonexistent/path");
        assert!(fonts.is_empty());
    }

    #[test]
    fn test_text_tool_initial_state() {
        let state = TextToolState::new(test_font_dir());
        assert!(!state.editing);
        assert!(state.text_buffer.is_empty());
        assert_eq!(state.scale, 1);
        assert_eq!(state.justification, Justification::Left);
        assert!(state.text_color.is_none());
    }

    #[test]
    fn test_text_tool_render_single_char() {
        let mut state = TextToolState::new(test_font_dir());
        if !state.available_fonts.contains(&"standard".to_string()) {
            return;
        }
        state.font_index = state
            .available_fonts
            .iter()
            .position(|n| n == "standard")
            .unwrap_or(0);
        state.load_selected_font();
        assert!(state.font.is_some(), "standard font should load");

        state.text_buffer = "A".to_string();
        state.preview_pos = (0, 0);

        let mut buf = CanvasBuffer::new(80, 40);
        state.render_text_to_buffer(&mut buf);

        let has_content = (0..buf.height())
            .any(|y| (0..buf.width()).any(|x| buf.get(x, y).is_some_and(|c| c.ch != ' ')));
        assert!(has_content, "FIGlet 'A' should produce non-space output");
    }

    #[test]
    fn test_text_tool_render_multi_char() {
        let mut state = TextToolState::new(test_font_dir());
        if !state.available_fonts.contains(&"standard".to_string()) {
            return;
        }
        state.font_index = state
            .available_fonts
            .iter()
            .position(|n| n == "standard")
            .unwrap_or(0);
        state.load_selected_font();
        assert!(state.font.is_some());

        state.text_buffer = "Hi".to_string();
        state.preview_pos = (0, 0);

        let mut buf = CanvasBuffer::new(80, 40);
        state.render_text_to_buffer(&mut buf);

        let has_content = (0..buf.height())
            .any(|y| (0..buf.width()).any(|x| buf.get(x, y).is_some_and(|c| c.ch != ' ')));
        assert!(has_content, "FIGlet 'Hi' should produce non-space output");
    }

    #[test]
    fn test_text_tool_justification_left() {
        let mut state = TextToolState::new(test_font_dir());
        if !state.available_fonts.contains(&"standard".to_string()) {
            return;
        }
        state.font_index = state
            .available_fonts
            .iter()
            .position(|n| n == "standard")
            .unwrap_or(0);
        state.load_selected_font();
        state.text_buffer = "A".to_string();
        state.preview_pos = (10, 5);
        state.justification = Justification::Left;

        let mut buf = CanvasBuffer::new(80, 40);
        state.render_text_to_buffer(&mut buf);

        let mut min_x = 999;
        for y in 0..buf.height() {
            for x in 0..buf.width() {
                if buf.get(x, y).is_some_and(|c| c.ch != ' ') {
                    min_x = min_x.min(x);
                }
            }
        }
        assert_eq!(min_x, 10, "left-justified text should start at cursor x");
    }

    #[test]
    fn test_text_tool_justification_right() {
        let mut state = TextToolState::new(test_font_dir());
        if !state.available_fonts.contains(&"standard".to_string()) {
            return;
        }
        state.font_index = state
            .available_fonts
            .iter()
            .position(|n| n == "standard")
            .unwrap_or(0);
        state.load_selected_font();
        state.text_buffer = "A".to_string();
        state.preview_pos = (20, 5);
        state.justification = Justification::Right;

        let mut buf = CanvasBuffer::new(80, 40);
        state.render_text_to_buffer(&mut buf);

        let mut max_x = 0usize;
        for y in 0..buf.height() {
            for x in 0..buf.width() {
                if buf.get(x, y).is_some_and(|c| c.ch != ' ') {
                    max_x = max_x.max(x);
                }
            }
        }
        assert!(
            max_x <= 20,
            "right-justified text should end at or before cursor x (max_x={max_x}, cursor_x=20)"
        );
    }

    #[test]
    fn test_text_tool_justification_center() {
        let mut state = TextToolState::new(test_font_dir());
        if !state.available_fonts.contains(&"standard".to_string()) {
            return;
        }
        state.font_index = state
            .available_fonts
            .iter()
            .position(|n| n == "standard")
            .unwrap_or(0);
        state.load_selected_font();
        state.text_buffer = "A".to_string();
        state.preview_pos = (30, 5);
        state.justification = Justification::Center;

        let mut buf = CanvasBuffer::new(80, 40);
        state.render_text_to_buffer(&mut buf);

        let mut min_x = 999;
        let mut max_x = 0usize;
        for y in 0..buf.height() {
            for x in 0..buf.width() {
                if buf.get(x, y).is_some_and(|c| c.ch != ' ') {
                    min_x = min_x.min(x);
                    max_x = max_x.max(x);
                }
            }
        }
        if max_x >= min_x {
            let center = (min_x + max_x) / 2;
            let diff = (center as i16 - 30).abs();
            assert!(
                diff <= 5,
                "centered text center ({center}) should be near cursor x (30), diff={diff}"
            );
        }
    }

    #[test]
    fn test_text_tool_color_applied() {
        let mut state = TextToolState::new(test_font_dir());
        if !state.available_fonts.contains(&"standard".to_string()) {
            return;
        }
        state.font_index = state
            .available_fonts
            .iter()
            .position(|n| n == "standard")
            .unwrap_or(0);
        state.load_selected_font();
        state.text_buffer = "A".to_string();
        state.preview_pos = (0, 0);
        state.text_color = Some(Color::Red);

        let mut buf = CanvasBuffer::new(80, 40);
        state.render_text_to_buffer(&mut buf);

        let has_red = (0..buf.height()).any(|y| {
            (0..buf.width()).any(|x| buf.get(x, y).is_some_and(|c| c.fg == Some(Color::Red)))
        });
        assert!(has_red, "Some cells should have red foreground");
    }

    #[test]
    fn test_text_tool_font_switching() {
        let mut state = TextToolState::new(test_font_dir());
        let mut buf = CanvasBuffer::new(80, 40);

        if state.available_fonts.is_empty() {
            return;
        }
        state.font_index = 0;
        state.load_selected_font();

        state.font_index = state.available_fonts.len() - 1;
        state.load_selected_font();
        assert!(state.font.is_some(), "last font should load successfully");

        state.text_buffer = "A".to_string();
        state.preview_pos = (0, 0);
        state.render_text_to_buffer(&mut buf);

        let has_content = (0..buf.height())
            .any(|y| (0..buf.width()).any(|x| buf.get(x, y).is_some_and(|c| c.ch != ' ')));
        assert!(has_content, "text should render after font switch");
    }

    #[test]
    fn test_text_tool_scale_factor() {
        let mut state = TextToolState::new(test_font_dir());
        if !state.available_fonts.contains(&"standard".to_string()) {
            return;
        }
        state.font_index = state
            .available_fonts
            .iter()
            .position(|n| n == "standard")
            .unwrap_or(0);
        state.load_selected_font();
        state.text_buffer = "A".to_string();
        state.preview_pos = (0, 0);
        state.scale = 2;

        let mut buf = CanvasBuffer::new(80, 40);
        state.render_text_to_buffer(&mut buf);

        let count = (0..buf.height())
            .flat_map(|y| (0..buf.width()).map(move |x| (x, y)))
            .filter(|&(x, y)| buf.get(x, y).is_some_and(|c| c.ch != ' '))
            .count();
        assert!(count >= 2, "scale=2 should produce at least 2 cells");
    }

    #[test]
    fn test_text_tool_clips_at_buffer_edge() {
        let mut state = TextToolState::new(test_font_dir());
        if !state.available_fonts.contains(&"standard".to_string()) {
            return;
        }
        state.font_index = state
            .available_fonts
            .iter()
            .position(|n| n == "standard")
            .unwrap_or(0);
        state.load_selected_font();
        state.text_buffer = "Hello World!".to_string();
        state.preview_pos = (75, 35);
        let mut buf = CanvasBuffer::new(80, 40);
        state.render_text_to_buffer(&mut buf);
    }

    #[test]
    fn test_text_tool_empty_text_noop() {
        let mut state = TextToolState::new(test_font_dir());
        let mut buf = CanvasBuffer::new(10, 10);
        state.render_text_to_buffer(&mut buf);

        for y in 0..buf.height() {
            for x in 0..buf.width() {
                assert_eq!(buf.get(x, y).unwrap().ch, ' ');
            }
        }
    }

    #[test]
    fn test_text_tool_no_font_no_panic() {
        let mut state = TextToolState::new("/nonexistent/dir");
        state.text_buffer = "Hello".to_string();
        state.preview_pos = (0, 0);
        let mut buf = CanvasBuffer::new(10, 10);
        state.render_text_to_buffer(&mut buf);
    }

    #[test]
    fn test_text_tool_editing_state() {
        let mut state = TextToolState::new(test_font_dir());
        assert!(!state.editing);
        state.editing = true;
        assert!(state.editing);
        state.text_buffer.push('H');
        state.text_buffer.push('i');
        assert_eq!(state.text_buffer, "Hi");
        state.editing = false;
        assert!(!state.editing);
    }

    fn setup_state_with_standard_font() -> TextToolState {
        let mut state = TextToolState::new(test_font_dir());
        if !state.available_fonts.contains(&"standard".to_string()) {
            return state;
        }
        state.font_index = state
            .available_fonts
            .iter()
            .position(|n| n == "standard")
            .unwrap_or(0);
        state.load_selected_font();
        state
    }

    #[test]
    fn test_text_block_create() {
        let mut state = setup_state_with_standard_font();
        if state.font.is_none() {
            return;
        }
        state.text_buffer = "A".to_string();
        state.preview_pos = (10, 5);
        state.justification = Justification::Center;
        state.text_color = Some(Color::Red);
        state.scale = 2;
        state.commit_block();
        assert_eq!(state.blocks.len(), 1);
        let block = &state.blocks[0];
        assert_eq!(block.text, "A");
        assert_eq!(block.x, 10);
        assert_eq!(block.y, 5);
        assert_eq!(block.scale, 2);
        assert_eq!(block.justification, Justification::Center);
        assert_eq!(block.text_color, Some(Color::Red));
        assert_eq!(block.rotation, 0);
        assert!(block.width > 0);
        assert!(block.height > 0);
        assert!(!block.cached_rows.is_empty());
    }

    #[test]
    fn test_text_block_multiple() {
        let mut state = setup_state_with_standard_font();
        if state.font.is_none() {
            return;
        }
        state.text_buffer = "A".to_string();
        state.preview_pos = (0, 0);
        state.commit_block();
        state.text_buffer = "B".to_string();
        state.preview_pos = (20, 10);
        state.commit_block();
        assert_eq!(state.blocks.len(), 2);
        assert_ne!(state.blocks[0].id, state.blocks[1].id);
        assert_eq!(state.blocks[0].x, 0);
        assert_eq!(state.blocks[1].x, 20);
    }

    #[test]
    fn test_text_block_hit_test() {
        let mut state = setup_state_with_standard_font();
        if state.font.is_none() {
            return;
        }
        state.text_buffer = "A".to_string();
        state.preview_pos = (50, 50);
        state.commit_block();
        let (bx, by, bw, bh) = state.compute_bounding_box(0);
        assert!(bw > 0);
        assert!(bh > 0);
        assert!(state
            .hit_test(bx + bw as i16 / 2, by + bh as i16 / 2)
            .is_some());
        assert!(state.hit_test(bx + bw as i16, by).is_none());
        assert!(state.hit_test(bx - 1, by - 1).is_none());
    }

    #[test]
    fn test_text_block_move() {
        let mut state = setup_state_with_standard_font();
        if state.font.is_none() {
            return;
        }
        state.text_buffer = "A".to_string();
        state.preview_pos = (10, 10);
        state.selected_block = Some(0);
        state.commit_block();
        assert!(
            state.selected_block == Some(0)
                || (!state.blocks.is_empty() && state.blocks[0].x == 10)
        );
        if state.blocks.is_empty() {
            return;
        }
        let idx = state.selected_block.unwrap_or(0);
        if idx < state.blocks.len() {
            state.move_selected_block(5, -3);
            assert_eq!(state.blocks[idx].x, 15);
            assert_eq!(state.blocks[idx].y, 7);
        }
    }

    #[test]
    fn test_text_block_scale() {
        let mut state = setup_state_with_standard_font();
        if state.font.is_none() {
            return;
        }
        state.text_buffer = "A".to_string();
        state.preview_pos = (0, 0);
        state.scale = 1;
        state.commit_block();
        let idx = state.selected_block.unwrap_or(0);
        if idx >= state.blocks.len() {
            return;
        }
        assert_eq!(state.blocks[idx].scale, 1);
        state.scale_selected_block(1);
        assert_eq!(state.blocks[idx].scale, 2);
        state.scale_selected_block(1);
        assert_eq!(state.blocks[idx].scale, 3);
        state.scale_selected_block(-1);
        assert_eq!(state.blocks[idx].scale, 2);
    }

    #[test]
    fn test_text_block_rotation() {
        let mut state = setup_state_with_standard_font();
        if state.font.is_none() {
            return;
        }
        state.text_buffer = "A".to_string();
        state.preview_pos = (0, 0);
        state.commit_block();
        let idx = state.selected_block.unwrap_or(0);
        if idx >= state.blocks.len() {
            return;
        }
        assert_eq!(state.blocks[idx].rotation, 0);
        let orig_w = state.blocks[idx].width;
        let orig_h = state.blocks[idx].height;
        state.rotate_selected_block();
        assert_eq!(state.blocks[idx].rotation, 90);
        state.rotate_selected_block();
        assert_eq!(state.blocks[idx].rotation, 180);
        state.rotate_selected_block();
        assert_eq!(state.blocks[idx].rotation, 270);
        state.rotate_selected_block();
        assert_eq!(state.blocks[idx].rotation, 0);
        assert_eq!(state.blocks[idx].width, orig_w);
        assert_eq!(state.blocks[idx].height, orig_h);
    }

    #[test]
    fn test_text_block_delete() {
        let mut state = setup_state_with_standard_font();
        if state.font.is_none() {
            return;
        }
        state.text_buffer = "A".to_string();
        state.preview_pos = (0, 0);
        state.commit_block();
        assert_eq!(state.blocks.len(), 1);
        let idx = state.selected_block.unwrap_or(0);
        if idx >= state.blocks.len() {
            return;
        }
        state.delete_selected_block();
        assert!(state.blocks.is_empty());
        assert!(state.selected_block.is_none());
    }

    #[test]
    fn test_text_block_re_edit() {
        let mut state = setup_state_with_standard_font();
        if state.font.is_none() {
            return;
        }
        state.text_buffer = "Hello".to_string();
        state.preview_pos = (30, 15);
        state.justification = Justification::Right;
        state.scale = 3;
        state.text_color = Some(Color::Blue);
        state.commit_block();
        let font_idx = state.font_index;
        assert_eq!(state.blocks.len(), 1);
        state.re_edit_block(0);
        assert!(state.blocks.is_empty());
        assert_eq!(state.text_buffer, "Hello");
        assert_eq!(state.font_index, font_idx);
        assert_eq!(state.justification, Justification::Right);
        assert_eq!(state.scale, 3);
        assert_eq!(state.text_color, Some(Color::Blue));
        assert!(state.editing);
    }

    #[test]
    fn test_text_block_bounding_box() {
        let mut state = setup_state_with_standard_font();
        if state.font.is_none() {
            return;
        }
        state.text_buffer = "Hi".to_string();
        state.preview_pos = (100, 200);
        state.justification = Justification::Left;
        state.scale = 1;
        state.commit_block();
        assert!(!state.blocks.is_empty());
        let (bx, by, bw, bh) = state.compute_bounding_box(0);
        assert_eq!(bx, 100);
        assert_eq!(by, 200);
        assert!(bw > 0);
        assert!(bh > 0);

        state.blocks[0].rotation = 90;
        let (bx90, by90, bw90, bh90) = state.compute_bounding_box(0);
        assert_eq!(bx90, 100);
        assert_eq!(by90, 200);
        assert_eq!(bw90, bh);
        assert_eq!(bh90, bw);

        state.blocks[0].justification = Justification::Center;
        let (bxc, byc, bwc, _bhc) = state.compute_bounding_box(0);
        let expected_x = 100 - (bwc as i16 / 2);
        assert_eq!(bxc, expected_x);
        assert_eq!(byc, 200);

        state.blocks[0].scale = 3;
        state.blocks[0].rotation = 0;
        state.blocks[0].justification = Justification::Right;
        let (bxr, byr, bwr, bhr) = state.compute_bounding_box(0);
        assert_eq!(bxr, 100 - (bwr as i16));
        assert_eq!(byr, 200);
        assert_eq!(bwr, state.blocks[0].width * 3);
        assert_eq!(bhr, state.blocks[0].height * 3);
    }

    #[test]
    fn test_bake_blocks_into_is_non_destructive() {
        // Export/player capture must bake text pixels into a buffer while
        // leaving the editable blocks intact — save keeps them as objects.
        let mut state = TextToolState::new(test_font_dir());
        if !state.available_fonts.contains(&"standard".to_string()) {
            return;
        }
        state.font_index = state
            .available_fonts
            .iter()
            .position(|n| n == "standard")
            .unwrap();
        state.load_selected_font();
        state.text_buffer = "X".to_string();
        state.preview_pos = (1, 1);
        state.commit_block();
        assert_eq!(state.blocks.len(), 1);

        let mut buf = CanvasBuffer::new(20, 10);
        state.bake_blocks_into(&mut buf);

        assert_eq!(
            state.blocks.len(),
            1,
            "bake_blocks_into must not remove blocks"
        );
        assert!(
            (0..buf.height())
                .flat_map(|y| (0..buf.width()).map(move |x| (x, y)))
                .any(|(x, y)| buf.get(x, y).is_some_and(|c| c.ch != ' ')),
            "baked buffer should contain the text pixels"
        );
    }

    #[test]
    fn test_text_tool_unicode_no_panic() {
        // 6.7.3: typing non-ASCII chars (Ä Ö Ü ä ö ü ß) must not panic.
        // lookup_char falls back to char-0 / blank glyph (fixed in 6.5.1).
        let mut state = TextToolState::new(test_font_dir());
        if !state.available_fonts.contains(&"standard".to_string()) {
            return;
        }
        state.font_index = state
            .available_fonts
            .iter()
            .position(|n| n == "standard")
            .unwrap_or(0);
        state.load_selected_font();
        state.text_buffer = "ÄÖÜäöüß".to_string();
        state.preview_pos = (0, 0);
        let mut buf = CanvasBuffer::new(80, 40);
        // Must not panic.
        state.render_text_to_buffer(&mut buf);
    }
}
