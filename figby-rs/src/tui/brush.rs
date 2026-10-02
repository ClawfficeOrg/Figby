use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrushShape {
    Square,
    Circle,
    SprayPaint,
    Dither,
    Custom,
}

impl BrushShape {
    pub fn cycle(&self) -> Self {
        match self {
            BrushShape::Square => BrushShape::Circle,
            BrushShape::Circle => BrushShape::SprayPaint,
            BrushShape::SprayPaint => BrushShape::Dither,
            BrushShape::Dither => BrushShape::Custom,
            BrushShape::Custom => BrushShape::Square,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            BrushShape::Square => "Square",
            BrushShape::Circle => "Circle",
            BrushShape::SprayPaint => "Spray",
            BrushShape::Dither => "Dither",
            BrushShape::Custom => "Custom",
        }
    }

    pub fn all() -> &'static [BrushShape] {
        &[
            BrushShape::Square,
            BrushShape::Circle,
            BrushShape::SprayPaint,
            BrushShape::Dither,
            BrushShape::Custom,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrushSubMode {
    Normal,
    Marker,
}

impl BrushSubMode {
    pub fn cycle(&self) -> Self {
        match self {
            BrushSubMode::Normal => BrushSubMode::Marker,
            BrushSubMode::Marker => BrushSubMode::Normal,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            BrushSubMode::Normal => "Normal",
            BrushSubMode::Marker => "Marker",
        }
    }
}

const MIN_SIZE: u8 = 1;
const MAX_SIZE: u8 = 20;
const MIN_DENSITY: u8 = 1;
const MAX_DENSITY: u8 = 100;

#[derive(Debug, Clone)]
pub struct BrushState {
    pub shape: BrushShape,
    pub size: u8,
    pub ch: char,
    pub density: u8,
    pub borders: Borders,
    pub sub_mode: BrushSubMode,
    /// Sub-cell dot cursor for the Braille tool (2x4 dots per canvas cell):
    /// `sub_x` in 0..=1, `sub_y` in 0..=3. Unused by every other tool.
    pub sub_x: u8,
    pub sub_y: u8,
}

impl BrushState {
    pub fn required_outer_width(&self) -> u16 {
        17
    }

    pub fn new() -> Self {
        Self {
            shape: BrushShape::Square,
            size: 1,
            ch: '\u{2588}',
            density: 35,
            borders: Borders::ALL,
            sub_mode: BrushSubMode::Normal,
            sub_x: 0,
            sub_y: 0,
        }
    }

    /// Move the braille sub-cell dot cursor, clamped within one cell's 2x4
    /// dot grid (does not cross into neighboring cells).
    pub fn move_braille_cursor(&mut self, dx: i8, dy: i8) {
        self.sub_x = (self.sub_x as i8 + dx).clamp(0, 1) as u8;
        self.sub_y = (self.sub_y as i8 + dy).clamp(0, 3) as u8;
    }

    pub fn set_borders(&mut self, borders: Borders) {
        self.borders = borders;
    }

    pub fn set_size(&mut self, n: u8) {
        self.size = n.clamp(MIN_SIZE, MAX_SIZE);
    }

    pub fn size_up(&mut self) {
        if self.size < MAX_SIZE {
            self.size += 1;
        }
    }

    pub fn size_down(&mut self) {
        if self.size > MIN_SIZE {
            self.size -= 1;
        }
    }

    pub fn cycle_shape(&mut self) {
        self.shape = self.shape.cycle();
    }

    pub fn set_density(&mut self, n: u8) {
        self.density = n.clamp(MIN_DENSITY, MAX_DENSITY);
    }

    pub fn density_up(&mut self) {
        if self.density < MAX_DENSITY {
            self.density += 1;
        }
    }

    pub fn density_down(&mut self) {
        if self.density > MIN_DENSITY {
            self.density -= 1;
        }
    }

    pub fn cycle_sub_mode(&mut self, has_colors: bool) {
        if has_colors || self.sub_mode == BrushSubMode::Marker {
            self.sub_mode = self.sub_mode.cycle();
        }
    }

    pub fn render_preview(&self, max_size: u8) -> Vec<String> {
        let s = self.size.min(max_size) as usize;
        match self.shape {
            BrushShape::Square => render_square_preview(s),
            BrushShape::Circle => render_circle_preview(s),
            BrushShape::SprayPaint => render_spray_preview(s, self.density),
            BrushShape::Dither => render_dither_preview(s),
            BrushShape::Custom => render_custom_preview(s),
        }
    }

    pub fn render_mini_preview(&self) -> Vec<String> {
        const GRID: usize = 5;
        let size = self.size as usize;

        let full = match self.shape {
            BrushShape::Square => render_square_preview(size),
            BrushShape::Circle => render_circle_preview(size),
            BrushShape::SprayPaint => render_spray_preview(size, self.density),
            BrushShape::Dither => render_dither_preview(size),
            BrushShape::Custom => render_custom_preview(size),
        };

        let map_cell = |c: char| -> char {
            if self.shape == BrushShape::Dither {
                // Keep ░▒▓█ so the falloff ring stays visible.
                c
            } else if c != ' ' {
                self.ch
            } else {
                ' '
            }
        };

        if size >= GRID {
            let offset = (size - GRID) / 2;
            full.iter()
                .skip(offset)
                .take(GRID)
                .map(|row| row.chars().skip(offset).take(GRID).map(map_cell).collect())
                .collect()
        } else {
            let offset = (GRID - size) / 2;
            let mut result: Vec<String> = Vec::with_capacity(GRID);
            for _ in 0..offset {
                result.push(" ".repeat(GRID));
            }
            for row in &full {
                let row_chars = row.chars().count();
                let mut line = String::with_capacity(GRID);
                line.push_str(&" ".repeat(offset));
                line.push_str(&row.chars().map(map_cell).collect::<String>());
                line.push_str(&" ".repeat(GRID.saturating_sub(offset + row_chars)));
                result.push(line);
            }
            while result.len() < GRID {
                result.push(" ".repeat(GRID));
            }
            result
        }
    }

    pub fn render(&self, frame: &mut Frame<'_>, area: Rect) {
        let block = Block::default()
            .title(" Brush ")
            .borders(self.borders)
            .title_style(Style::default().add_modifier(Modifier::BOLD));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        if inner.width < 2 || inner.height < 2 {
            return;
        }

        let mut lines: Vec<Line<'_>> = Vec::new();

        lines.push(Line::from(vec![
            Span::styled("Char:", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format!(" {}", self.ch)),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Size:", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format!(" {}", self.size)),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Shape:", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format!(" {}", self.shape.name())),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Mode:", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format!(" {}", self.sub_mode.name())),
        ]));

        lines.push(Line::from(Span::raw("")));

        let preview = self.render_mini_preview();
        let visible_height = inner.height.saturating_sub(lines.len() as u16 + 1);
        for row in preview.iter().take(visible_height as usize) {
            lines.push(Line::from(Span::raw(format!(" {}", row))));
        }

        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, inner);
    }
}

impl Default for BrushState {
    fn default() -> Self {
        Self::new()
    }
}

use ratatui::buffer::Buffer;
use ratatui::widgets::Widget;

impl Widget for &BrushState {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .title(" Brush ")
            .borders(Borders::ALL)
            .title_style(Style::default().add_modifier(Modifier::BOLD));
        let inner = block.inner(area);
        Widget::render(block, area, buf);

        if inner.width < 2 || inner.height < 2 {
            return;
        }

        let mut lines: Vec<Line<'_>> = Vec::new();

        lines.push(Line::from(vec![
            Span::styled("Char:", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format!(" {}", self.ch)),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Size:", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format!(" {}", self.size)),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Shape:", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format!(" {}", self.shape.name())),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Mode:", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format!(" {}", self.sub_mode.name())),
        ]));

        lines.push(Line::from(Span::raw("")));

        let preview = self.render_mini_preview();
        let visible_height = inner.height.saturating_sub(lines.len() as u16 + 1);
        for row in preview.iter().take(visible_height as usize) {
            lines.push(Line::from(Span::raw(format!(" {}", row))));
        }

        let paragraph = Paragraph::new(lines);
        Widget::render(paragraph, inner, buf);
    }
}

fn render_square_preview(size: usize) -> Vec<String> {
    if size == 0 {
        return vec![String::new()];
    }
    (0..size).map(|_| "@".repeat(size)).collect()
}

fn render_circle_preview(size: usize) -> Vec<String> {
    if size == 0 {
        return vec![String::new()];
    }
    let r = size as f64 / 2.0;
    let cx = r;
    let cy = r;
    (0..size)
        .map(|y| {
            (0..size)
                .map(|x| {
                    let dx = x as f64 - cx + 0.5;
                    let dy = y as f64 - cy + 0.5;
                    if dx * dx + dy * dy <= r * r {
                        '@'
                    } else {
                        ' '
                    }
                })
                .collect()
        })
        .collect()
}

fn render_spray_preview(size: usize, density: u8) -> Vec<String> {
    if size == 0 {
        return vec![String::new()];
    }
    let seed: u64 = 42;
    let d = density as u64;
    (0..size)
        .map(|y| {
            (0..size)
                .map(|x| {
                    let hash = (x as u64).wrapping_mul(7) + (y as u64).wrapping_mul(31) + seed;
                    let val = hash % 100;
                    if val < d {
                        '@'
                    } else {
                        ' '
                    }
                })
                .collect()
        })
        .collect()
}

fn render_custom_preview(size: usize) -> Vec<String> {
    if size == 0 {
        return vec![String::new()];
    }
    (0..size)
        .map(|y| {
            (0..size)
                .map(|x| {
                    let cx = size / 2;
                    let cy = size / 2;
                    if x == cx && y == cy {
                        '+'
                    } else {
                        ' '
                    }
                })
                .collect()
        })
        .collect()
}

fn render_dither_preview(size: usize) -> Vec<String> {
    if size == 0 {
        return vec![String::new()];
    }
    let r = size as f64 / 2.0;
    (0..size)
        .map(|y| {
            (0..size)
                .map(|x| {
                    // Match paint_dither_stamp: offsets centered on (0,0).
                    let dx = x as f64 - (size as f64 - 1.0) / 2.0;
                    let dy = y as f64 - (size as f64 - 1.0) / 2.0;
                    let dist = (dx * dx + dy * dy).sqrt();
                    if dist > r {
                        ' '
                    } else {
                        let falloff = (1.0 - dist / r).clamp(0.0, 1.0);
                        dither_char(falloff)
                    }
                })
                .collect()
        })
        .collect()
}

/// Map circular falloff to a block-density character.
/// Center is solid `█`; edges dissolve through `▓`/`▒`/`░`.
pub fn dither_char(falloff: f64) -> char {
    if falloff > 0.75 {
        '\u{2588}' // █
    } else if falloff > 0.5 {
        '\u{2593}' // ▓
    } else if falloff > 0.25 {
        '\u{2592}' // ▒
    } else {
        '\u{2591}' // ░
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brush_default_shape() {
        let brush = BrushState::new();
        assert_eq!(brush.shape, BrushShape::Square);
    }

    #[test]
    fn test_brush_default_size() {
        let brush = BrushState::new();
        assert_eq!(brush.size, 1);
    }

    #[test]
    fn test_brush_cycle_shape() {
        let mut brush = BrushState::new();
        assert_eq!(brush.shape, BrushShape::Square);
        brush.cycle_shape();
        assert_eq!(brush.shape, BrushShape::Circle);
        brush.cycle_shape();
        assert_eq!(brush.shape, BrushShape::SprayPaint);
        brush.cycle_shape();
        assert_eq!(brush.shape, BrushShape::Dither);
        brush.cycle_shape();
        assert_eq!(brush.shape, BrushShape::Custom);
        brush.cycle_shape();
        assert_eq!(brush.shape, BrushShape::Square);
    }

    #[test]
    fn test_brush_size_clamp() {
        let mut brush = BrushState::new();
        brush.set_size(0);
        assert_eq!(brush.size, 1);
        brush.set_size(21);
        assert_eq!(brush.size, 20);
    }

    #[test]
    fn test_brush_size_up_down() {
        let mut brush = BrushState::new();
        brush.size_down();
        assert_eq!(brush.size, 1, "size_down should clamp at MIN_SIZE");
        brush.size_up();
        assert_eq!(brush.size, 2);
        brush.size_up();
        assert_eq!(brush.size, 3);
    }

    #[test]
    fn test_brush_size_up_max_boundary() {
        let mut brush = BrushState::new();
        brush.set_size(20);
        brush.size_up();
        assert_eq!(brush.size, 20);
    }

    #[test]
    fn test_brush_size_down_min_boundary() {
        let mut brush = BrushState::new();
        brush.set_size(1);
        brush.size_down();
        assert_eq!(brush.size, 1);
    }

    #[test]
    fn test_brush_preview_square() {
        let mut brush = BrushState::new();
        brush.set_size(3);
        let preview = brush.render_preview(5);
        assert_eq!(preview.len(), 3);
        for row in &preview {
            assert_eq!(row.len(), 3);
            assert!(row.chars().all(|c| c == '@'));
        }
    }

    #[test]
    fn test_brush_preview_circle() {
        let brush = BrushState {
            shape: BrushShape::Circle,
            size: 5,
            ch: '\u{2588}',
            density: 35,
            borders: Borders::ALL,
            sub_mode: BrushSubMode::Normal,
            sub_x: 0,
            sub_y: 0,
        };
        let preview = brush.render_preview(10);
        assert_eq!(preview.len(), 5);
        for row in &preview {
            assert_eq!(row.len(), 5);
        }
        let center = 2;
        assert_eq!(preview[center].as_bytes()[center] as char, '@');
    }

    #[test]
    fn test_brush_preview_spray() {
        let brush = BrushState {
            shape: BrushShape::SprayPaint,
            size: 5,
            ch: '\u{2588}',
            density: 35,
            borders: Borders::ALL,
            sub_mode: BrushSubMode::Normal,
            sub_x: 0,
            sub_y: 0,
        };
        let preview = brush.render_preview(10);
        assert_eq!(preview.len(), 5);
        for row in &preview {
            assert_eq!(row.len(), 5);
        }
        let has_dot = preview.iter().any(|r| r.contains('@'));
        let has_space = preview.iter().any(|r| r.contains(' '));
        assert!(has_dot, "spray preview should have some dots");
        assert!(has_space, "spray preview should have some spaces");
    }

    #[test]
    fn test_brush_preview_spray_deterministic() {
        let a = BrushState {
            shape: BrushShape::SprayPaint,
            size: 7,
            ch: '\u{2588}',
            density: 35,
            borders: Borders::ALL,
            sub_mode: BrushSubMode::Normal,
            sub_x: 0,
            sub_y: 0,
        };
        let b = BrushState {
            shape: BrushShape::SprayPaint,
            size: 7,
            ch: '\u{2588}',
            density: 35,
            borders: Borders::ALL,
            sub_mode: BrushSubMode::Normal,
            sub_x: 0,
            sub_y: 0,
        };
        assert_eq!(a.render_preview(10), b.render_preview(10));
    }

    #[test]
    fn test_brush_preview_custom() {
        let brush = BrushState {
            shape: BrushShape::Custom,
            size: 5,
            ch: '\u{2588}',
            density: 35,
            borders: Borders::ALL,
            sub_mode: BrushSubMode::Normal,
            sub_x: 0,
            sub_y: 0,
        };
        let preview = brush.render_preview(10);
        assert_eq!(preview.len(), 5);
        assert_eq!(preview[2].as_bytes()[2] as char, '+');
    }

    #[test]
    fn test_brush_preview_respects_max_size() {
        let mut brush = BrushState::new();
        brush.set_size(10);
        let preview = brush.render_preview(5);
        assert_eq!(preview.len(), 5);
        for row in &preview {
            assert_eq!(row.len(), 5);
        }
    }

    #[test]
    fn test_brush_preview_size_zero() {
        let brush = BrushState {
            shape: BrushShape::Square,
            size: 0,
            ch: '\u{2588}',
            density: 35,
            borders: Borders::ALL,
            sub_mode: BrushSubMode::Normal,
            sub_x: 0,
            sub_y: 0,
        };
        let preview = brush.render_preview(5);
        assert!(!preview.is_empty());
    }

    #[test]
    fn test_brush_preview_all_shapes_size_one() {
        for shape in BrushShape::all() {
            let brush = BrushState {
                shape: *shape,
                size: 1,
                ch: '\u{2588}',
                density: 35,
                borders: Borders::ALL,
                sub_mode: BrushSubMode::Normal,
                sub_x: 0,
                sub_y: 0,
            };
            let preview = brush.render_preview(5);
            assert_eq!(preview.len(), 1);
            assert_eq!(preview[0].chars().count(), 1);
        }
    }

    #[test]
    fn test_mini_preview_size_5() {
        let brush = BrushState {
            shape: BrushShape::Square,
            size: 5,
            ch: '\u{2588}',
            density: 35,
            borders: Borders::ALL,
            sub_mode: BrushSubMode::Normal,
            sub_x: 0,
            sub_y: 0,
        };
        let preview = brush.render_mini_preview();
        assert_eq!(preview.len(), 5);
        for row in &preview {
            assert_eq!(row.chars().count(), 5);
        }
        assert!(preview.iter().all(|r| r.chars().all(|c| c == '\u{2588}')));
    }

    #[test]
    fn test_mini_preview_resize_1_to_20() {
        for size in 1..=20 {
            for shape in BrushShape::all() {
                let brush = BrushState {
                    shape: *shape,
                    size,
                    ch: '#',
                    density: 35,
                    borders: Borders::ALL,
                    sub_mode: BrushSubMode::Normal,
                    sub_x: 0,
                    sub_y: 0,
                };
                let preview = brush.render_mini_preview();
                assert_eq!(preview.len(), 5, "size={size} shape={:?}", shape);
                for row in &preview {
                    assert_eq!(row.chars().count(), 5, "size={size} shape={:?}", shape);
                }
            }
        }
    }

    #[test]
    fn test_mini_preview_shape_cycle() {
        use std::collections::HashSet;
        let mut brush = BrushState {
            shape: BrushShape::Square,
            size: 5,
            ch: '#',
            density: 35,
            borders: Borders::ALL,
            sub_mode: BrushSubMode::Normal,
            sub_x: 0,
            sub_y: 0,
        };
        let mut outputs = HashSet::new();
        // One pass over every shape — previews must all differ.
        for _ in 0..BrushShape::all().len() {
            let preview = brush.render_mini_preview();
            outputs.insert(preview.join("\n"));
            brush.cycle_shape();
        }
        assert_eq!(outputs.len(), BrushShape::all().len());
    }

    #[test]
    fn test_mini_preview_uses_brush_char() {
        let brush = BrushState {
            shape: BrushShape::Square,
            size: 3,
            ch: '#',
            density: 35,
            borders: Borders::ALL,
            sub_mode: BrushSubMode::Normal,
            sub_x: 0,
            sub_y: 0,
        };
        let preview = brush.render_mini_preview();
        for row in &preview {
            for c in row.chars() {
                if c != ' ' {
                    assert_eq!(c, '#');
                }
            }
        }
    }

    #[test]
    fn test_brush_required_outer_width() {
        let brush = BrushState::new();
        assert_eq!(brush.required_outer_width(), 17);
    }

    #[test]
    fn test_mini_preview_center_small_size() {
        let brush = BrushState {
            shape: BrushShape::Square,
            size: 1,
            ch: '#',
            density: 35,
            borders: Borders::ALL,
            sub_mode: BrushSubMode::Normal,
            sub_x: 0,
            sub_y: 0,
        };
        let preview = brush.render_mini_preview();
        assert_eq!(preview.len(), 5);
        assert_eq!(preview[2].chars().nth(2), Some('#'));
        for (y, row) in preview.iter().enumerate() {
            for (x, c) in row.chars().enumerate() {
                if y != 2 || x != 2 {
                    assert_eq!(c, ' ');
                }
            }
        }
    }
}
