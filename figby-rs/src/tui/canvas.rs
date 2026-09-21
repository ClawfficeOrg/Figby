use crossterm::event::KeyCode;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::Widget;

use std::time::{Duration, Instant};

use super::theme::Theme;

/// Maximum number of canvas cells (`width × height`) a document may allocate.
/// Mirrors the budget used by template rendering (`template.rs`) and GIF
/// import (`gif_import.rs`) — GPT review F-12. A u16 max × u16 max canvas
/// (65535² ≈ 4.3 billion cells) would otherwise allocate absurd buffers from
/// the new-image / GIF-import / settings dialogs.
pub const MAX_CANVAS_CELLS: usize = 1_000_000;

/// Whether a `w × h` canvas is within the cell budget and has nonzero
/// dimensions. Used by every canvas-creating dialog so a hostile or typo'd
/// dimension cannot allocate billions of rows (GPT review F-12).
pub fn canvas_cells_within_budget(w: usize, h: usize) -> bool {
    w != 0 && h != 0 && w.saturating_mul(h) <= MAX_CANVAS_CELLS
}

#[derive(Debug, Clone)]
pub struct GlyphCursor {
    pub x: u16,
    pub y: u16,
    pub visible: bool,
    last_blink: Instant,
}

impl GlyphCursor {
    pub fn new(x: u16, y: u16) -> Self {
        Self {
            x,
            y,
            visible: true,
            last_blink: Instant::now(),
        }
    }

    pub fn blink(&mut self) {
        let now = Instant::now();
        if now.duration_since(self.last_blink) >= Duration::from_millis(500) {
            self.visible = !self.visible;
            self.last_blink = now;
        }
    }
}

pub use crate::CanvasCell;

#[derive(Debug, Clone)]
pub struct CanvasBuffer {
    cells: Vec<Vec<CanvasCell>>,
    width: usize,
    height: usize,
}

impl serde::Serialize for CanvasBuffer {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("CanvasBuffer", 3)?;
        state.serialize_field("cells", &self.cells)?;
        state.serialize_field("width", &self.width)?;
        state.serialize_field("height", &self.height)?;
        state.end()
    }
}

impl<'de> serde::Deserialize<'de> for CanvasBuffer {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(serde::Deserialize)]
        struct RawCanvasBuffer {
            cells: Vec<Vec<CanvasCell>>,
            width: usize,
            height: usize,
        }
        let raw = RawCanvasBuffer::deserialize(deserializer)?;
        if raw.width == 0 || raw.height == 0 {
            return Err(serde::de::Error::custom(
                "CanvasBuffer dimensions must be nonzero",
            ));
        }
        if raw.cells.len() != raw.height {
            return Err(serde::de::Error::custom(format!(
                "CanvasBuffer cells height {} does not match declared height {}",
                raw.cells.len(),
                raw.height
            )));
        }
        for (i, row) in raw.cells.iter().enumerate() {
            if row.len() != raw.width {
                return Err(serde::de::Error::custom(format!(
                    "CanvasBuffer row {} width {} does not match declared width {}",
                    i,
                    row.len(),
                    raw.width
                )));
            }
        }
        Ok(CanvasBuffer {
            cells: raw.cells,
            width: raw.width,
            height: raw.height,
        })
    }
}

impl CanvasBuffer {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            cells: vec![vec![CanvasCell::default(); width]; height],
            width,
            height,
        }
    }

    pub fn get(&self, x: usize, y: usize) -> Option<&CanvasCell> {
        if x < self.width && y < self.height {
            Some(&self.cells[y][x])
        } else {
            None
        }
    }

    pub fn get_mut(&mut self, x: usize, y: usize) -> Option<&mut CanvasCell> {
        if x < self.width && y < self.height {
            Some(&mut self.cells[y][x])
        } else {
            None
        }
    }

    pub fn set(&mut self, x: usize, y: usize, cell: CanvasCell) {
        if let Some(c) = self.get_mut(x, y) {
            *c = cell;
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }
}

#[derive(Debug, Clone)]
pub struct TextOverlay {
    pub x: i16,
    pub y: i16,
    pub rows: Vec<String>,
    pub color: Option<Color>,
    pub scale: u8,
    pub rotation: u16,
}

pub struct CanvasWidget {
    pub buffer: CanvasBuffer,
    cursor: (u16, u16),
    scroll: (u16, u16),
    zoom: u8,
    show_grid: bool,
    pub selection_perimeter: Option<Vec<(usize, usize)>>,
    pub polygon_vertices: Vec<(i16, i16)>,
    pub text_overlays: Vec<TextOverlay>,
    pub text_block_perimeter: Option<Vec<(usize, usize)>>,
    pub glyph_cursor: Option<GlyphCursor>,
    pub theme: Theme,
}

impl CanvasWidget {
    pub fn new(width: u16, height: u16) -> Self {
        let w = if width > 0 { width as usize } else { 1 };
        let h = if height > 0 { height as usize } else { 1 };
        Self {
            buffer: CanvasBuffer::new(w, h),
            cursor: (0, 0),
            scroll: (0, 0),
            zoom: 1,
            show_grid: false,
            selection_perimeter: None,
            polygon_vertices: Vec::new(),
            text_overlays: Vec::new(),
            text_block_perimeter: None,
            glyph_cursor: None,
            theme: Theme::default(),
        }
    }

    pub fn move_up(&mut self) {
        if self.cursor.1 > 0 {
            self.cursor.1 -= 1;
        }
    }

    pub fn move_down(&mut self) {
        let max_y = self.buffer.height().saturating_sub(1) as u16;
        if self.cursor.1 < max_y {
            self.cursor.1 += 1;
        }
    }

    pub fn move_left(&mut self) {
        if self.cursor.0 > 0 {
            self.cursor.0 -= 1;
        }
    }

    pub fn move_right(&mut self) {
        let max_x = self.buffer.width().saturating_sub(1) as u16;
        if self.cursor.0 < max_x {
            self.cursor.0 += 1;
        }
    }

    pub fn zoom_in(&mut self) {
        self.zoom = (self.zoom * 2).clamp(1, 8);
    }

    pub fn zoom_out(&mut self) {
        self.zoom = (self.zoom / 2).clamp(1, 8);
    }

    pub fn toggle_grid(&mut self) {
        self.show_grid = !self.show_grid;
    }

    pub fn cursor(&self) -> (u16, u16) {
        self.cursor
    }

    pub fn zoom_level(&self) -> u8 {
        self.zoom
    }

    pub fn show_grid(&self) -> bool {
        self.show_grid
    }

    pub fn set_cursor(&mut self, x: u16, y: u16) {
        let max_x = self.buffer.width().saturating_sub(1) as u16;
        let max_y = self.buffer.height().saturating_sub(1) as u16;
        self.cursor = (x.min(max_x), y.min(max_y));
    }

    pub fn scroll_offset(&self) -> (u16, u16) {
        self.scroll
    }

    pub fn ensure_cursor_visible(&mut self, inner_width: u16, inner_height: u16) {
        let zoom = self.zoom.max(1) as u16;
        if inner_width == 0 || inner_height == 0 {
            return;
        }
        let vis_w = inner_width / zoom;
        let vis_h = inner_height / zoom;

        if self.cursor.0 < self.scroll.0 {
            self.scroll.0 = self.cursor.0;
        }
        if vis_w > 0 && self.cursor.0 >= self.scroll.0 + vis_w {
            self.scroll.0 = self.cursor.0 - vis_w + 1;
        }
        if self.cursor.1 < self.scroll.1 {
            self.scroll.1 = self.cursor.1;
        }
        if vis_h > 0 && self.cursor.1 >= self.scroll.1 + vis_h {
            self.scroll.1 = self.cursor.1 - vis_h + 1;
        }
    }

    pub fn handle_key(&mut self, code: KeyCode, inner_width: u16, inner_height: u16) -> bool {
        match code {
            KeyCode::Up => {
                self.move_up();
                self.ensure_cursor_visible(inner_width, inner_height);
                true
            }
            KeyCode::Down => {
                self.move_down();
                self.ensure_cursor_visible(inner_width, inner_height);
                true
            }
            KeyCode::Left => {
                self.move_left();
                self.ensure_cursor_visible(inner_width, inner_height);
                true
            }
            KeyCode::Right => {
                self.move_right();
                self.ensure_cursor_visible(inner_width, inner_height);
                true
            }
            KeyCode::Char('+') | KeyCode::Char('=') => {
                self.zoom_in();
                self.ensure_cursor_visible(inner_width, inner_height);
                true
            }
            KeyCode::Char('-') | KeyCode::Char('_') => {
                self.zoom_out();
                self.ensure_cursor_visible(inner_width, inner_height);
                true
            }
            KeyCode::Char('G') => {
                self.toggle_grid();
                true
            }
            _ => false,
        }
    }
}

impl Widget for &CanvasWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let zoom = self.zoom.max(1) as u16;
        let sx = self.scroll.0;
        let sy = self.scroll.1;

        // Fill area with buffer cells
        for row in 0..area.height {
            for col in 0..area.width {
                let bx = sx + col / zoom;
                let by = sy + row / zoom;

                let (ch, style) = if let Some(cell) = self.buffer.get(bx as usize, by as usize) {
                    let mut s = Style::default();
                    if let Some(fg) = cell.fg {
                        s = s.fg(fg);
                    }
                    if let Some(bg) = cell.bg {
                        s = s.bg(bg);
                    }
                    (cell.ch, s)
                } else {
                    (' ', Style::default())
                };

                if let Some(cell) = buf.cell_mut((area.x + col, area.y + row)) {
                    cell.set_char(ch);
                    cell.set_style(style);
                }
            }
        }

        // Grid overlay (before cursor so cursor highlight wins)
        if self.show_grid && zoom > 1 {
            let grid_style = Style::default().add_modifier(Modifier::DIM);
            let max_vis_buf_w =
                (area.width / zoom).min((self.buffer.width() as u16).saturating_sub(sx));
            let max_vis_buf_h =
                (area.height / zoom).min((self.buffer.height() as u16).saturating_sub(sy));

            // Horizontal lines at row boundaries
            for by in 1..max_vis_buf_h {
                let cy = area.y + by * zoom;
                for c in area.x..area.x + area.width {
                    if let Some(cell) = buf.cell_mut((c, cy)) {
                        if (c - area.x).is_multiple_of(zoom) && (c - area.x) > 0 {
                            cell.set_char('┼');
                        } else {
                            cell.set_char('─');
                        }
                        cell.set_style(grid_style);
                    }
                }
            }

            // Vertical lines at column boundaries
            for bx in 1..max_vis_buf_w {
                let cx = area.x + bx * zoom;
                for r in area.y..area.y + area.height {
                    if let Some(cell) = buf.cell_mut((cx, r)) {
                        if (r - area.y).is_multiple_of(zoom) && (r - area.y) > 0 {
                            cell.set_char('┼');
                        } else {
                            cell.set_char('│');
                        }
                        cell.set_style(grid_style);
                    }
                }
            }
        }

        let vis_w = area.width / zoom;
        let vis_h = area.height / zoom;

        // Polygon vertex preview (rendered before selection overlay)
        for &(vx, vy) in &self.polygon_vertices {
            if vx >= sx as i16
                && vx < (sx + vis_w) as i16
                && vy >= sy as i16
                && vy < (sy + vis_h) as i16
            {
                let cx = area.x + (vx as u16 - sx) * zoom;
                let cy = area.y + (vy as u16 - sy) * zoom;
                for r in cy..(cy + zoom).min(area.y + area.height) {
                    for c in cx..(cx + zoom).min(area.x + area.width) {
                        if let Some(cell) = buf.cell_mut((c, r)) {
                            cell.set_style(
                                cell.style()
                                    .fg(self.theme.canvas.selection)
                                    .add_modifier(Modifier::BOLD),
                            );
                            let existing = cell.symbol().chars().next().unwrap_or(' ');
                            if existing == ' ' {
                                cell.set_char('+');
                            }
                        }
                    }
                }
            }
        }

        // Selection perimeter dashed overlay
        if let Some(ref perim) = self.selection_perimeter {
            let dash_style = Style::default()
                .fg(self.theme.canvas.selection)
                .add_modifier(Modifier::BOLD);
            let mut sorted: Vec<&(usize, usize)> = perim.iter().collect();
            sorted.sort_by_key(|(x, y)| x + y * (self.buffer.width() + 1));
            for (i, &&(bx, by)) in sorted.iter().enumerate() {
                if bx >= sx as usize
                    && bx < (sx + vis_w) as usize
                    && by >= sy as usize
                    && by < (sy + vis_h) as usize
                {
                    // Alternate dash pattern: every other cell gets the dash char
                    let dash_char = if i % 2 == 0 { '▒' } else { ' ' };
                    let cx = area.x + (bx as u16 - sx) * zoom;
                    let cy = area.y + (by as u16 - sy) * zoom;
                    for r in cy..(cy + zoom).min(area.y + area.height) {
                        for c in cx..(cx + zoom).min(area.x + area.width) {
                            if let Some(cell) = buf.cell_mut((c, r)) {
                                if dash_char != ' ' {
                                    cell.set_char(dash_char);
                                }
                                cell.set_style(dash_style);
                            }
                        }
                    }
                }
            }
        }

        // Text block perimeter dashed overlay (marquee around selected block)
        if let Some(ref perim) = self.text_block_perimeter {
            let marquee_style = Style::default()
                .fg(self.theme.canvas.text_block)
                .add_modifier(Modifier::BOLD);
            let mut sorted: Vec<&(usize, usize)> = perim.iter().collect();
            sorted.sort_by_key(|(x, y)| x + y * 20000);
            for (i, &&(bx, by)) in sorted.iter().enumerate() {
                if bx >= sx as usize
                    && bx < (sx + vis_w) as usize
                    && by >= sy as usize
                    && by < (sy + vis_h) as usize
                {
                    let dash_char = if i % 2 == 0 { '▒' } else { ' ' };
                    let cx = area.x + (bx as u16 - sx) * zoom;
                    let cy = area.y + (by as u16 - sy) * zoom;
                    for r in cy..(cy + zoom).min(area.y + area.height) {
                        for c in cx..(cx + zoom).min(area.x + area.width) {
                            if let Some(cell) = buf.cell_mut((c, r)) {
                                if dash_char != ' ' {
                                    cell.set_char(dash_char);
                                }
                                cell.set_style(marquee_style);
                            }
                        }
                    }
                }
            }
        }

        // Text overlays (rendered as FIGlet text on canvas)
        for overlay in &self.text_overlays {
            let s = overlay.scale.max(1) as i16;
            if overlay.rows.is_empty() {
                continue;
            }
            let h = overlay.rows.len() as i16;
            let w = overlay.rows[0].chars().count() as i16;
            if w == 0 || h == 0 {
                continue;
            }
            for (r, row) in overlay.rows.iter().enumerate() {
                for (c, ch) in row.chars().enumerate() {
                    if ch == ' ' {
                        continue;
                    }
                    let cr = r as i16;
                    let cc = c as i16;
                    let (base_x, base_y) = match overlay.rotation {
                        90 => (overlay.x + cr * s, overlay.y + (h - 1 - cc) * s),
                        180 => (overlay.x + (w - 1 - cc) * s, overlay.y + (h - 1 - cr) * s),
                        270 => (overlay.x + (h - 1 - cr) * s, overlay.y + cc * s),
                        _ => (overlay.x + cc * s, overlay.y + cr * s),
                    };
                    let fg_style = overlay.color.map(|c| Style::default().fg(c));
                    for dy in 0..s {
                        for dx in 0..s {
                            let bx = base_x + dx;
                            let by = base_y + dy;
                            if bx < sx as i16
                                || bx >= (sx + vis_w) as i16
                                || by < sy as i16
                                || by >= (sy + vis_h) as i16
                            {
                                continue;
                            }
                            let term_x = area.x + (bx as u16 - sx) * zoom;
                            let term_y = area.y + (by as u16 - sy) * zoom;
                            for tdy in 0..zoom {
                                for tdx in 0..zoom {
                                    let tx = term_x + tdx;
                                    let ty = term_y + tdy;
                                    if tx >= area.x + area.width || ty >= area.y + area.height {
                                        continue;
                                    }
                                    if let Some(cell) = buf.cell_mut((tx, ty)) {
                                        cell.set_char(ch);
                                        if let Some(ref s) = fg_style {
                                            cell.set_style(*s);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Glyph cursor overlay (blinking █) — replaces normal cursor when set
        if let Some(ref gc) = self.glyph_cursor {
            if gc.visible && gc.x >= sx && gc.x < sx + vis_w && gc.y >= sy && gc.y < sy + vis_h {
                let cx = area.x + (gc.x - sx) * zoom;
                let cy = area.y + (gc.y - sy) * zoom;
                for r in cy..(cy + zoom).min(area.y + area.height) {
                    for c in cx..(cx + zoom).min(area.x + area.width) {
                        if let Some(cell) = buf.cell_mut((c, r)) {
                            cell.set_char('\u{2588}');
                            cell.set_style(cell.style().reversed());
                        }
                    }
                }
            }
        } else {
            // Cursor highlight (rendered after grid so it's visible on top)
            if self.cursor.0 >= sx
                && self.cursor.0 < sx + vis_w
                && self.cursor.1 >= sy
                && self.cursor.1 < sy + vis_h
            {
                let cx = area.x + (self.cursor.0 - sx) * zoom;
                let cy = area.y + (self.cursor.1 - sy) * zoom;
                for r in cy..(cy + zoom).min(area.y + area.height) {
                    for c in cx..(cx + zoom).min(area.x + area.width) {
                        if let Some(cell) = buf.cell_mut((c, r)) {
                            cell.set_style(cell.style().reversed());
                        }
                    }
                }
            }
        }
    }
}

impl Default for CanvasWidget {
    fn default() -> Self {
        Self::new(40, 20)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canvas_buffer_serde_roundtrip() {
        let mut buf = CanvasBuffer::new(3, 2);
        buf.set(
            0,
            0,
            CanvasCell {
                ch: 'A',
                fg: Some(Color::Red),
                bg: None,
                height: None,
            },
        );
        buf.set(
            1,
            1,
            CanvasCell {
                ch: '#',
                fg: None,
                bg: Some(Color::Blue),
                height: Some(128),
            },
        );

        let json = serde_json::to_string(&buf).unwrap();
        let restored: CanvasBuffer = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.width(), 3);
        assert_eq!(restored.height(), 2);
        assert_eq!(restored.get(0, 0).unwrap().ch, 'A');
        assert_eq!(restored.get(1, 1).unwrap().ch, '#');
        assert_eq!(restored.get(1, 1).unwrap().height, Some(128));
    }

    #[test]
    fn canvas_buffer_serde_rejects_zero_width() {
        let json = r#"{"cells":[[]],"width":0,"height":1}"#;
        let result = serde_json::from_str::<CanvasBuffer>(json);
        assert!(result.is_err());
    }

    #[test]
    fn canvas_buffer_serde_rejects_mismatched_dimensions() {
        let json = r#"{"cells":[[" "," "]],"width":1,"height":1}"#;
        let result = serde_json::from_str::<CanvasBuffer>(json);
        assert!(result.is_err());
    }
}
