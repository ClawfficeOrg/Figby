use rand::rngs::StdRng;
use rand::Rng;
use rand::SeedableRng;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::widgets::{Block, Borders};
use ratatui::Frame;
use std::collections::{BTreeMap, HashMap};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use super::{
    brush, canvas, capture_thumbnail, dialogs, documents, export, file_ops, font_editor, fx,
    fx_script, image_editor, layers, layout, light_panel, lighting, palette, palette_editor,
    particles, player, status, theme, timeline, toolbox, tools, undo, undo_panel, welcome,
    LightPanel, MenuBar, MenuBarState, PropsPanel, PropsPanelMode, RenderMode, SidePanel,
    ThrobberState, Tool,
};
use crate::config;

const ICONS_YAML: &str = include_str!("../../assets/icons.yaml");
const ICONS_ASCII_YAML: &str = include_str!("../../assets/icons-ascii.yaml");

/// Which icon set the TUI draws with (review U3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconSet {
    /// Nerd Font glyphs (`assets/icons.yaml`). The default; needs a Nerd
    /// Font in the terminal or the icons render as tofu.
    Nerd,
    /// Plain ASCII (`assets/icons-ascii.yaml` over the built-in fallbacks).
    Ascii,
}

impl IconSet {
    /// `FIGBY_ICONS` wins over `[tui] icons` in config.toml; anything
    /// other than `ascii` (case-insensitive) means the Nerd Font set.
    pub fn resolve(env: Option<&str>, config: Option<&str>) -> Self {
        match env.filter(|v| !v.trim().is_empty()).or(config) {
            Some(v) if v.trim().eq_ignore_ascii_case("ascii") => IconSet::Ascii,
            _ => IconSet::Nerd,
        }
    }

    /// The icon map for this set. Keys absent from the map fall back to
    /// each widget's own plain-text label.
    pub fn icons(self) -> BTreeMap<String, String> {
        let yaml = match self {
            IconSet::Nerd => ICONS_YAML,
            IconSet::Ascii => ICONS_ASCII_YAML,
        };
        serde_yaml_ng::from_str(yaml).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    FontEditor,
    ImageEditor,
    AsciiPreview,
    Lighting,
}

/// Tracks which kind of project is open, so Tab cycling skips irrelevant modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SessionType {
    /// No project opened yet — all modes available.
    #[default]
    Any,
    /// Font project: cycle FontEditor ↔ AsciiPreview only.
    Font,
    /// Image project: cycle ImageEditor ↔ AsciiPreview only.
    Image,
}

impl AppMode {
    pub fn title(&self) -> &str {
        match self {
            AppMode::FontEditor => " Font Editor ",
            AppMode::ImageEditor => " Image Editor ",
            AppMode::AsciiPreview => " ASCII Preview ",
            AppMode::Lighting => " Lighting Editor ",
        }
    }

    pub fn next_for(&self, session: SessionType) -> Self {
        match session {
            SessionType::Font => match self {
                AppMode::FontEditor => AppMode::AsciiPreview,
                AppMode::AsciiPreview => AppMode::FontEditor,
                _ => AppMode::FontEditor,
            },
            SessionType::Image => match self {
                AppMode::ImageEditor => AppMode::AsciiPreview,
                AppMode::AsciiPreview => AppMode::ImageEditor,
                _ => AppMode::ImageEditor,
            },
            SessionType::Any => self.next(),
        }
    }

    pub fn prev_for(&self, session: SessionType) -> Self {
        match session {
            SessionType::Font => match self {
                AppMode::FontEditor => AppMode::AsciiPreview,
                AppMode::AsciiPreview => AppMode::FontEditor,
                _ => AppMode::FontEditor,
            },
            SessionType::Image => match self {
                AppMode::ImageEditor => AppMode::AsciiPreview,
                AppMode::AsciiPreview => AppMode::ImageEditor,
                _ => AppMode::ImageEditor,
            },
            SessionType::Any => self.prev(),
        }
    }

    fn next(&self) -> Self {
        match self {
            AppMode::FontEditor => AppMode::ImageEditor,
            AppMode::ImageEditor => AppMode::AsciiPreview,
            AppMode::AsciiPreview => AppMode::FontEditor,
            AppMode::Lighting => AppMode::Lighting,
        }
    }

    fn prev(&self) -> Self {
        match self {
            AppMode::FontEditor => AppMode::AsciiPreview,
            AppMode::AsciiPreview => AppMode::ImageEditor,
            AppMode::ImageEditor => AppMode::FontEditor,
            AppMode::Lighting => AppMode::Lighting,
        }
    }
}

/// Canvas/tool/undo state — everything needed to edit a document.
pub struct EditorState {
    pub canvas: canvas::CanvasWidget,
    pub toolbox: toolbox::Toolbox,
    pub brush: brush::BrushState,
    pub palette: palette::Palette,
    pub font_editor: font_editor::FontEditor,
    pub image_editor: image_editor::ImageEditor,
    pub text_tool: tools::text::TextToolState,
    pub undo: undo::UndoSystem,
    pub unsaved: bool,
    /// Monotonically increasing counter of document mutations. Bumped by
    /// `mark_dirty()`; used to detect edits that happen while an async
    /// save is in flight (GPT review F-07).
    pub revision: u64,
    pub selection: Option<tools::selection::Selection>,
    pub clipboard: Option<tools::selection::Clipboard>,
    pub layer_stack: layers::LayerStack,
    pub layer_panel: layers::LayerPanel,
    pub fill_threshold: u8,
    pub eyedropper_sample: Option<canvas::CanvasCell>,
    pub move_state: tools::move_tool::MoveState,
    pub rotate_state: tools::rotate_tool::RotateState,
    pub selection_state: tools::selection::SelectionState,
    pub line_state: tools::line::LineState,
    pub selection_polygon_points: Vec<(i16, i16)>,
    /// Where this image/animation document was last saved or opened as a
    /// `.figmap`. Lets Save (and the unsaved-changes dialog) write in place
    /// instead of always going through Save As.
    pub figmap_path: Option<std::path::PathBuf>,
    /// The glyph whose rows the canvas currently holds, set when the
    /// character editor loads it. The canvas is written back into the
    /// glyph only while this matches the open glyph.
    pub font_char_loaded: Option<u32>,
}

impl EditorState {
    /// Record a document mutation: flags the document unsaved and bumps
    /// the revision counter so in-flight async saves can detect that the
    /// document changed since they snapshotted.
    pub fn mark_dirty(&mut self) {
        self.unsaved = true;
        self.revision = self.revision.wrapping_add(1);
    }

    pub fn recomposite_canvas(&mut self) {
        self.canvas.buffer = self.layer_stack.composite();
    }

    /// Restore a timeline frame's committed per-layer buffers and
    /// recomposite. Restores EVERY layer (not just the active one), so
    /// navigating the timeline never flattens or duplicates multilayer
    /// content (GPT review F-05). Snapshot layers beyond the current
    /// stack are ignored; stack layers beyond the snapshot are cleared.
    pub fn load_timeline_frame(&mut self, states: &[canvas::CanvasBuffer]) {
        for (i, layer) in self.layer_stack.layers.iter_mut().enumerate() {
            match states.get(i) {
                Some(buf) => *layer.buffer_mut() = buf.clone(),
                None => {
                    let w = layer.buffer().width();
                    let h = layer.buffer().height();
                    *layer.buffer_mut() = canvas::CanvasBuffer::new(w, h);
                }
            }
        }
        self.recomposite_canvas();
    }

    /// Whether the active layer rejects writes. All mutating paths
    /// (brush, eraser, fill, line, spray, marker, move, rotate, text
    /// placement, keyboard paint, cut, paste) must check this before
    /// touching the layer — the lock flag is otherwise cosmetic (GPT
    /// review F-09).
    pub(crate) fn active_layer_locked(&self) -> bool {
        self.layer_stack.active_layer().locked
    }

    pub(crate) fn push_undo_snapshot(&mut self, label: &str) {
        let layer_index = self.layer_stack.active;
        let buffer = self.layer_stack.active_layer().buffer.clone();
        self.undo
            .push_snapshot(buffer, layer_index, label.to_string());
    }

    /// Restore an undo/redo snapshot into ITS OWN layer (GPT review F-09),
    /// make that layer active so subsequent edits target what the user
    /// sees, and recomposite. Returns false if the recorded layer no
    /// longer exists (e.g. it was deleted after the snapshot) or is
    /// locked — locked layers reject writes, including undo restorations.
    pub fn restore_undo_snapshot(
        &mut self,
        layer_index: usize,
        buffer: canvas::CanvasBuffer,
    ) -> bool {
        if layer_index >= self.layer_stack.layers.len() {
            return false;
        }
        if self.layer_stack.layers[layer_index].locked {
            return false;
        }
        *self.layer_stack.layers[layer_index].buffer_mut() = buffer;
        self.layer_stack.active = layer_index;
        self.recomposite_canvas();
        true
    }

    pub(crate) fn compute_canvas_rect(&self, inner: Rect) -> Rect {
        let zoom = self.canvas.zoom_level().max(1) as u16;
        let buf_w = self.canvas.buffer.width() as u16;
        let buf_h = self.canvas.buffer.height() as u16;
        let grid_w = (buf_w * zoom).min(inner.width);
        let grid_h = (buf_h * zoom).min(inner.height);
        Rect {
            x: inner.x + (inner.width.saturating_sub(grid_w) / 2),
            y: inner.y + (inner.height.saturating_sub(grid_h) / 2),
            width: grid_w,
            height: grid_h,
        }
    }

    pub(crate) fn screen_to_buffer(
        &self,
        col: u16,
        row: u16,
        canvas_inner_rect: Rect,
    ) -> Option<(i16, i16)> {
        let zoom = self.canvas.zoom_level().max(1) as i16;
        if col < canvas_inner_rect.x || col >= canvas_inner_rect.x + canvas_inner_rect.width {
            return None;
        }
        if row < canvas_inner_rect.y || row >= canvas_inner_rect.y + canvas_inner_rect.height {
            return None;
        }
        let (sx, sy) = self.canvas.scroll_offset();
        let bx = sx as i16 + (col as i16 - canvas_inner_rect.x as i16) / zoom;
        let by = sy as i16 + (row as i16 - canvas_inner_rect.y as i16) / zoom;
        Some((bx, by))
    }

    /// Write the canvas back into the open glyph. Only valid once that
    /// glyph has been loaded into the canvas: until then the canvas holds
    /// whatever was there before (another glyph, the default blank page)
    /// and writing it back would replace the glyph's art. In that case the
    /// glyph is loaded instead, whichever input path opened the editor.
    pub(crate) fn sync_canvas_to_font_char(&mut self) {
        if let font_editor::FontEditorView::CharEditor(code) = self.font_editor.view {
            if self.font_char_loaded != Some(code) {
                self.sync_font_char_to_canvas();
                return;
            }
            self.font_editor.sync_from_canvas(code, &self.canvas.buffer);
        }
    }

    /// Forget which glyph the canvas holds once the character editor is
    /// left, so the next entry (even into the same glyph, which may have
    /// been transformed or resized meanwhile) reloads it.
    pub(crate) fn note_font_view_change(&mut self) {
        if !matches!(
            self.font_editor.view,
            font_editor::FontEditorView::CharEditor(_)
        ) {
            self.font_char_loaded = None;
        }
    }

    /// Load the open glyph into the canvas: the font's Char Height rows
    /// tall (glyph rows beyond the glyph's own count are blank, rows past
    /// Char Height are not shown), every cell rewritten so nothing of a
    /// previous glyph survives.
    pub(crate) fn sync_font_char_to_canvas(&mut self) {
        let Some((code, ch)) = self.font_editor.selected_char() else {
            self.font_char_loaded = None;
            return;
        };
        let charheight = self
            .font_editor
            .font
            .as_ref()
            .map_or(0, |f| f.charheight as usize);
        let w = ch.width().max(1);
        let h = charheight.max(1);
        let mut buf = canvas::CanvasBuffer::new(w, h);
        for (y, row) in ch.rows().iter().take(h).enumerate() {
            for (x, c) in row.chars().take(w).enumerate() {
                buf.set(
                    x,
                    y,
                    canvas::CanvasCell {
                        ch: c,
                        fg: None,
                        bg: None,
                        height: None,
                    },
                );
            }
        }
        if self.canvas.buffer.width() != w || self.canvas.buffer.height() != h {
            self.canvas = canvas::CanvasWidget::new(w as u16, h as u16);
            self.layer_stack.resize_all(w, h);
        }
        *self.layer_stack.active_layer_mut().buffer_mut() = buf;
        self.recomposite_canvas();
        self.font_char_loaded = Some(code);
    }

    pub(crate) fn sync_image_to_canvas(&mut self) {
        if let Some(cells) = self.image_editor.cells() {
            let h = cells.len();
            let w = cells[0].len();
            if self.canvas.buffer.width() != w || self.canvas.buffer.height() != h {
                self.canvas = canvas::CanvasWidget::new(w as u16, h as u16);
                self.layer_stack.resize_all(w, h);
            }
            let mut buf = self.layer_stack.active_layer().buffer.clone();
            for (y, row) in cells.iter().enumerate() {
                for (x, cell) in row.iter().enumerate() {
                    buf.set(x, y, *cell);
                }
            }
            *self.layer_stack.active_layer_mut().buffer_mut() = buf;
            self.recomposite_canvas();
        }
    }

    fn move_selection(&mut self, dx: i16, dy: i16) {
        if let Some(ref mut sel) = self.selection {
            if sel.is_active() {
                let mut buf = self.layer_stack.active_layer().buffer.clone();
                sel.move_selection(&mut buf, dx, dy);
                *self.layer_stack.active_layer_mut().buffer_mut() = buf;
                self.recomposite_canvas();
            }
        }
    }

    /// Nudge the whole active layer by one cell. Used by the Move tool's
    /// arrow-key shortcut when there's no selection to move instead.
    fn move_layer(&mut self, dx: i16, dy: i16) {
        let moved =
            tools::move_tool::translate_buffer(&self.layer_stack.active_layer().buffer, dx, dy);
        *self.layer_stack.active_layer_mut().buffer_mut() = moved;
        self.recomposite_canvas();
    }

    /// Rotate the active selection 90°, or the whole active layer if no
    /// selection is active. Used by the Rotate tool's arrow-key shortcut,
    /// one discrete step per keypress.
    fn rotate_selection_or_layer(&mut self, clockwise: bool) {
        if let Some(ref sel) = self.selection {
            if sel.is_active() {
                let mut buf = self.layer_stack.active_layer().buffer.clone();
                buf = tools::rotate_tool::rotate_region(&buf, sel.bounds(), clockwise);
                *self.layer_stack.active_layer_mut().buffer_mut() = buf;
                self.selection = Some(sel.rotate_90(clockwise));
                self.recomposite_canvas();
                return;
            }
        }
        let rotated = tools::rotate_tool::rotate_whole_buffer(
            &self.layer_stack.active_layer().buffer,
            clockwise,
        );
        *self.layer_stack.active_layer_mut().buffer_mut() = rotated;
        self.recomposite_canvas();
    }

    pub(crate) fn handle_selection_down(
        &mut self,
        bx: i16,
        by: i16,
        selection_drag_origin: &mut Option<(i16, i16)>,
        selection_lasso_points: &mut Vec<(i16, i16)>,
    ) {
        match self.toolbox.selected {
            Tool::Marquee => {
                self.selection = None;
                *selection_drag_origin = Some((bx, by));
            }
            Tool::CircleSelect => {
                self.selection = None;
                *selection_drag_origin = Some((bx, by));
            }
            Tool::Lasso => {
                self.selection = None;
                *selection_lasso_points = vec![(bx, by)];
            }
            Tool::PolygonSelect => {
                if self.selection_polygon_points.len() >= 3 {
                    let (fx, fy) = self.selection_polygon_points[0];
                    let dist = ((bx - fx).abs() + (by - fy).abs()) as f64;
                    if dist < 3.0 {
                        let vertices = std::mem::take(&mut self.selection_polygon_points);
                        self.selection = Some(tools::selection::Selection::polygon(
                            &self.canvas.buffer,
                            &vertices,
                        ));
                        return;
                    }
                }
                self.selection_polygon_points.push((bx, by));
            }
            _ => {}
        }
    }

    pub(crate) fn handle_selection_drag(
        &mut self,
        bx: i16,
        by: i16,
        selection_drag_origin: &mut Option<(i16, i16)>,
        selection_lasso_points: &mut Vec<(i16, i16)>,
    ) {
        match self.toolbox.selected {
            Tool::Marquee => {
                if let Some((ox, oy)) = *selection_drag_origin {
                    self.selection = Some(tools::selection::Selection::marquee(
                        &self.canvas.buffer,
                        ox,
                        oy,
                        bx,
                        by,
                    ));
                }
            }
            Tool::CircleSelect => {
                if let Some((ox, oy)) = *selection_drag_origin {
                    let dx = bx - ox;
                    let dy = by - oy;
                    let r = ((dx * dx + dy * dy) as f64).sqrt().round() as i16;
                    self.selection = Some(tools::selection::Selection::circle(
                        &self.canvas.buffer,
                        ox,
                        oy,
                        r,
                    ));
                }
            }
            Tool::Lasso => {
                selection_lasso_points.push((bx, by));
            }
            _ => {}
        }
    }

    pub(crate) fn handle_selection_up(
        &mut self,
        selection_drag_origin: &mut Option<(i16, i16)>,
        selection_lasso_points: &mut Vec<(i16, i16)>,
    ) {
        match self.toolbox.selected {
            Tool::Marquee | Tool::CircleSelect => {
                *selection_drag_origin = None;
            }
            Tool::Lasso => {
                let points = std::mem::take(selection_lasso_points);
                if points.len() >= 3 {
                    self.selection = Some(tools::selection::Selection::lasso(
                        &self.canvas.buffer,
                        &points,
                    ));
                }
            }
            Tool::PolygonSelect => {}
            _ => {}
        }
    }

    pub(crate) fn handle_key(
        &mut self,
        code: KeyCode,
        modifiers: KeyModifiers,
        dirty: &mut bool,
    ) -> bool {
        // When the palette is in hex-entry mode, don't intercept any keys.
        // Enter/Escape/Backspace must reach the palette handler in dispatch
        // to confirm/cancel/edit the hex value. Without this, keyboard
        // painting and other editor actions steal Enter before the palette
        // sees it, leaving custom_mode stuck true.
        if self.palette.is_typing() {
            return false;
        }
        // Rotate tool: Left/Right step 90°
        if self.toolbox.selected == Tool::Rotate {
            match code {
                KeyCode::Left => {
                    self.push_undo_snapshot("Rotate");
                    self.rotate_selection_or_layer(false);
                    self.mark_dirty();
                    *dirty = true;
                    return true;
                }
                KeyCode::Right => {
                    self.push_undo_snapshot("Rotate");
                    self.rotate_selection_or_layer(true);
                    self.mark_dirty();
                    *dirty = true;
                    return true;
                }
                _ => {}
            }
        }

        // Selection operations
        let selection_active = self.selection.as_ref().is_some_and(|s| s.is_active());
        if selection_active {
            match code {
                KeyCode::Up => {
                    self.push_undo_snapshot("Move selection");
                    self.move_selection(0, -1);
                    self.mark_dirty();
                    return true;
                }
                KeyCode::Down => {
                    self.push_undo_snapshot("Move selection");
                    self.move_selection(0, 1);
                    self.mark_dirty();
                    return true;
                }
                KeyCode::Left => {
                    self.push_undo_snapshot("Move selection");
                    self.move_selection(-1, 0);
                    self.mark_dirty();
                    return true;
                }
                KeyCode::Right => {
                    self.push_undo_snapshot("Move selection");
                    self.move_selection(1, 0);
                    self.mark_dirty();
                    return true;
                }
                KeyCode::Delete | KeyCode::Backspace => {
                    self.push_undo_snapshot("Delete selection");
                    if let Some(sel) = self.selection.take() {
                        let mut buf = self.layer_stack.active_layer().buffer.clone();
                        sel.delete_from(&mut buf);
                        *self.layer_stack.active_layer_mut().buffer_mut() = buf;
                        self.recomposite_canvas();
                        self.mark_dirty();
                    }
                    return true;
                }
                _ => {}
            }

            if modifiers.contains(KeyModifiers::CONTROL) {
                match code {
                    KeyCode::Char('c') => {
                        if let Some(ref sel) = self.selection {
                            self.clipboard = Some(sel.copy_from(&self.canvas.buffer));
                        }
                        return true;
                    }
                    KeyCode::Char('x') => {
                        self.push_undo_snapshot("Cut selection");
                        if let Some(sel) = self.selection.take() {
                            let mut buf = self.layer_stack.active_layer().buffer.clone();
                            self.clipboard = Some(sel.cut_from(&mut buf));
                            *self.layer_stack.active_layer_mut().buffer_mut() = buf;
                            self.recomposite_canvas();
                            self.mark_dirty();
                        }
                        return true;
                    }
                    KeyCode::Char('v') => {
                        self.push_undo_snapshot("Paste");
                        if let Some(ref clip) = self.clipboard {
                            let (cx, cy) = self.canvas.cursor();
                            let mut buf = self.layer_stack.active_layer().buffer.clone();
                            tools::selection::Selection::paste_into(
                                &mut buf, clip, cx as i16, cy as i16,
                            );
                            *self.layer_stack.active_layer_mut().buffer_mut() = buf;
                            self.recomposite_canvas();
                            self.mark_dirty();
                        }
                        return true;
                    }
                    _ => {}
                }
            }
        } else if self.toolbox.selected == Tool::Move {
            // Move tool arrow keys nudge the whole layer
            match code {
                KeyCode::Up => {
                    self.push_undo_snapshot("Move layer");
                    self.move_layer(0, -1);
                    self.mark_dirty();
                    return true;
                }
                KeyCode::Down => {
                    self.push_undo_snapshot("Move layer");
                    self.move_layer(0, 1);
                    self.mark_dirty();
                    return true;
                }
                KeyCode::Left => {
                    self.push_undo_snapshot("Move layer");
                    self.move_layer(-1, 0);
                    self.mark_dirty();
                    return true;
                }
                KeyCode::Right => {
                    self.push_undo_snapshot("Move layer");
                    self.move_layer(1, 0);
                    self.mark_dirty();
                    return true;
                }
                _ => {}
            }
        } else if self.toolbox.selected == Tool::Braille {
            // Braille tool: arrow keys move a 2x4 sub-cell dot cursor
            // within the current canvas cell (mouse can only address whole
            // cells, not individual dots), Space/Enter toggles the dot
            // under it. Reuses the same bit convention as the whole-image
            // braille converter in image_input.rs.
            match code {
                KeyCode::Up => {
                    self.brush.move_braille_cursor(0, -1);
                    *dirty = true;
                    return true;
                }
                KeyCode::Down => {
                    self.brush.move_braille_cursor(0, 1);
                    *dirty = true;
                    return true;
                }
                KeyCode::Left => {
                    self.brush.move_braille_cursor(-1, 0);
                    *dirty = true;
                    return true;
                }
                KeyCode::Right => {
                    self.brush.move_braille_cursor(1, 0);
                    *dirty = true;
                    return true;
                }
                KeyCode::Char(' ') | KeyCode::Enter if !self.active_layer_locked() => {
                    self.push_undo_snapshot("Braille dot");
                    let (cx, cy) = self.canvas.cursor();
                    let (cx, cy) = (cx as usize, cy as usize);
                    let mut buf = self.layer_stack.active_layer().buffer.clone();
                    if let Some(existing) = buf.get(cx, cy) {
                        let mut cell = *existing;
                        cell.ch = crate::image_input::toggle_braille_dot(
                            cell.ch,
                            self.brush.sub_x,
                            self.brush.sub_y,
                        );
                        self.palette.apply_to_cell(&mut cell);
                        buf.set(cx, cy, cell);
                    }
                    *self.layer_stack.active_layer_mut().buffer_mut() = buf;
                    self.recomposite_canvas();
                    self.mark_dirty();
                    *dirty = true;
                    return true;
                }
                _ => {}
            }
        }

        // Polygon select: Enter closes polygon, Esc cancels
        if self.toolbox.selected == Tool::PolygonSelect && !self.selection_polygon_points.is_empty()
        {
            match code {
                KeyCode::Enter => {
                    let points = std::mem::take(&mut self.selection_polygon_points);
                    if points.len() >= 3 {
                        self.selection = Some(tools::selection::Selection::polygon(
                            &self.canvas.buffer,
                            &points,
                        ));
                    }
                    return true;
                }
                KeyCode::Esc => {
                    self.selection_polygon_points.clear();
                    return true;
                }
                _ => {}
            }
        }

        // Deselect on Esc
        if self.selection.is_some() && code == KeyCode::Esc {
            self.selection = None;
            return true;
        }

        // Keyboard painting: Space/Enter paints or erases at cursor.
        // Locked layers reject writes (GPT review F-09).
        if Tool::is_paint_tool(self.toolbox.selected)
            && matches!(code, KeyCode::Char(' ') | KeyCode::Enter)
            && !self.active_layer_locked()
        {
            let (cx, cy) = self.canvas.cursor();
            self.push_undo_snapshot("Keyboard paint");
            if self.toolbox.selected == Tool::Fill {
                let mut cell = canvas::CanvasCell {
                    ch: self.brush.ch,
                    fg: None,
                    bg: None,
                    height: None,
                };
                self.palette.apply_to_cell(&mut cell);
                let mut buf = self.layer_stack.active_layer().buffer.clone();
                tools::fill::flood_fill(&mut buf, cx as i16, cy as i16, cell);
                *self.layer_stack.active_layer_mut().buffer_mut() = buf;
                self.recomposite_canvas();
            } else if self.toolbox.selected == Tool::Eraser {
                let shape = self.brush.shape;
                let size = self.brush.size;
                let mut buf = self.layer_stack.active_layer().buffer.clone();
                tools::eraser::erase_stamp(&mut buf, cx as i16, cy as i16, shape, size);
                *self.layer_stack.active_layer_mut().buffer_mut() = buf;
                self.recomposite_canvas();
            } else if self.toolbox.selected == Tool::Spray {
                let mut cell = canvas::CanvasCell {
                    ch: self.brush.ch,
                    fg: None,
                    bg: None,
                    height: None,
                };
                self.palette.apply_to_cell(&mut cell);
                let mut rng = StdRng::seed_from_u64(rand::rng().random());
                let size = self.brush.size;
                let density = self.brush.density;
                let mut buf = self.layer_stack.active_layer().buffer.clone();
                tools::spray::spray_stamp(
                    &mut buf, cx as i16, cy as i16, size, density, cell, &mut rng,
                );
                *self.layer_stack.active_layer_mut().buffer_mut() = buf;
                self.recomposite_canvas();
            } else {
                let mut cell = canvas::CanvasCell {
                    ch: self.brush.ch,
                    fg: None,
                    bg: None,
                    height: None,
                };
                self.palette.apply_to_cell(&mut cell);
                let shape = self.brush.shape;
                let size = self.brush.size;
                let mut buf = self.layer_stack.active_layer().buffer.clone();
                tools::brush::paint_stamp(&mut buf, cx as i16, cy as i16, shape, size, cell);
                *self.layer_stack.active_layer_mut().buffer_mut() = buf;
                self.recomposite_canvas();
            }
            self.mark_dirty();
            return true;
        }

        false
    }
}

/// Mouse/drag/interaction transient state.
pub struct InteractionState {
    pub selection_drag_origin: Option<(i16, i16)>,
    pub selection_lasso_points: Vec<(i16, i16)>,
    pub prev_mouse_buf: Option<(i16, i16)>,
    pub mouse_batch_active: bool,
    pub line_start: Option<(i16, i16)>,
    /// Layer buffer snapshot taken at Line/Move/Rotate drag start. Every
    /// drag event recomputes the result from this pristine snapshot (never
    /// incrementally from the previous event), so repeated recomputation
    /// stays exact instead of drifting.
    pub saved_buffer: Option<canvas::CanvasBuffer>,
    /// Buffer position where a Move-tool drag started; deltas for the whole
    /// drag are computed from this anchor. See `saved_buffer`.
    pub move_origin: Option<(i16, i16)>,
    /// Buffer position where a Rotate-tool drag started; drag distance maps
    /// to a number of 90° steps, computed from this anchor each event (see
    /// `saved_buffer`) rather than accumulated, so it never drifts.
    pub rotate_origin: Option<(i16, i16)>,
    /// Selection mask snapshot taken at Move/Rotate drag start, if any was
    /// active. `None` means the whole active layer is being transformed
    /// instead of just the selected region.
    pub saved_selection: Option<tools::selection::Selection>,
}

/// Animation/particle/timeline subsystem state.
pub struct AnimationState {
    pub timeline_state: timeline::TimelineState,
    pub particle_system: particles::ParticleSystem,
    pub emitter_active: bool,
    /// Layer the emitter rides on during playback (the layer that was active when
    /// the emitter was placed); its keyframed position offset moves the emitter.
    pub emitter_follow_layer: Option<usize>,
    pub emitter_panel: particles::EmitterConfigPanel,
    pub show_live_particles: bool,
    pub baked_layer_indices: Vec<usize>,
    pub timeline_visible: bool,
    pub marker_accum: HashMap<(i16, i16), f64>,
    /// Default loop state for inline playback, seeded from the imported
    /// GIF's `loop_count` when one exists (`0` = infinite → `true`); can
    /// also be toggled directly via the transport bar / `l` key. A binary
    /// flag can't represent a finite repeat count, so any GIF with a
    /// specific finite repeat count is conservatively treated as `false`.
    pub loop_enabled: bool,
    /// Active in-canvas animation playback (Timeline Enter / Animation >
    /// Play), rendered in place of normal canvas content while set. Distinct
    /// from the standalone fullscreen preview player (`player::play_fullscreen`,
    /// used by the Export dialog's Play button), which still takes over the
    /// whole terminal on request.
    pub inline_player: Option<player::AnimationPlayer>,
    /// Transport-bar button hit-rects, recomputed each render and consulted
    /// by `handle_mouse_event`.
    pub transport_rects: Vec<(timeline::TransportButton, Rect)>,
}

/// Lighting subsystem state — scene, LUT, shadow params, light-panel UI.
pub struct LightingState {
    pub scene: Option<lighting::Scene>,
    pub lut: lighting::LightingLut,
    pub max_shadow_distance: u16,
    pub height_scale: f32,
    pub panel: light_panel::LightPanel,
    pub light_keyframes: Vec<lighting::LightKeyframe>,
}

impl LightingState {
    /// Extract current properties from the selected light for keyframe insertion.
    fn current_light_properties(&self, light_idx: usize) -> lighting::LightProperties {
        let mut props = lighting::LightProperties::default();
        if let Some(ref scene) = self.scene {
            if let Some(light) = scene.lights.get(light_idx) {
                match light {
                    lighting::Light::Ambient {
                        intensity, color, ..
                    } => {
                        props.intensity = Some(*intensity);
                        props.color = Some(*color);
                    }
                    lighting::Light::Directional {
                        direction,
                        intensity,
                        color,
                        ..
                    } => {
                        props.direction = Some(*direction);
                        props.intensity = Some(*intensity);
                        props.color = Some(*color);
                    }
                    lighting::Light::Point {
                        position,
                        intensity,
                        color,
                        attenuation,
                        ..
                    } => {
                        props.position = Some(*position);
                        props.intensity = Some(*intensity);
                        props.color = Some(*color);
                        props.attenuation = Some(attenuation.clone());
                    }
                }
            }
        }
        props
    }

    /// Apply keyframe property overrides to a live light in the scene.
    fn apply_keyframe_to_light(&mut self, light_idx: usize, props: &lighting::LightProperties) {
        if let Some(ref mut scene) = self.scene {
            if let Some(light) = scene.lights.get_mut(light_idx) {
                match light {
                    lighting::Light::Ambient {
                        intensity, color, ..
                    } => {
                        if let Some(i) = props.intensity {
                            *intensity = i;
                        }
                        if let Some(c) = props.color {
                            *color = c;
                        }
                    }
                    lighting::Light::Directional {
                        direction,
                        intensity,
                        color,
                        ..
                    } => {
                        if let Some(d) = props.direction {
                            *direction = d;
                        }
                        if let Some(i) = props.intensity {
                            *intensity = i;
                        }
                        if let Some(c) = props.color {
                            *color = c;
                        }
                    }
                    lighting::Light::Point {
                        position,
                        intensity,
                        color,
                        attenuation,
                        ..
                    } => {
                        if let Some(p) = props.position {
                            *position = p;
                        }
                        if let Some(i) = props.intensity {
                            *intensity = i;
                        }
                        if let Some(c) = props.color {
                            *color = c;
                        }
                        if let Some(a) = &props.attenuation {
                            *attenuation = a.clone();
                        }
                    }
                }
            }
        }
    }

    /// Handle a key press in lighting mode.
    /// `timeline_frame` and `timeline_total` are the current/total frame counts
    /// for computing normalized keyframe time. Pass (0, 0) when no timeline exists.
    /// Returns `Some(true)` = consumed, `Some(false)` = exit mode (Esc), `None` = not matched.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn handle_key(
        &mut self,
        code: KeyCode,
        modifiers: KeyModifiers,
        w: i16,
        h: i16,
        dirty: &mut bool,
        timeline_frame: usize,
        timeline_total: usize,
    ) -> Option<bool> {
        match code {
            KeyCode::Esc => return Some(false),
            KeyCode::Up => {
                if modifiers == KeyModifiers::SHIFT {
                    if let Some(ref mut scene) = self.scene {
                        let idx = self.panel.selected_index;
                        if idx < scene.lights.len() {
                            if let lighting::Light::Point {
                                ref mut position, ..
                            } = scene.lights[idx]
                            {
                                position.1 = (position.1 - 1.0).max(0.0);
                                *dirty = true;
                            }
                        }
                    }
                } else if self.panel.selected_index > 0 {
                    self.panel.selected_index -= 1;
                    *dirty = true;
                }
            }
            KeyCode::Down => {
                if modifiers == KeyModifiers::SHIFT {
                    if let Some(ref mut scene) = self.scene {
                        let idx = self.panel.selected_index;
                        if idx < scene.lights.len() {
                            if let lighting::Light::Point {
                                ref mut position, ..
                            } = scene.lights[idx]
                            {
                                position.1 = (position.1 + 1.0).min(h as f32 - 1.0);
                                *dirty = true;
                            }
                        }
                    }
                } else if let Some(ref scene) = self.scene {
                    if self.panel.selected_index + 1 < scene.lights.len() {
                        self.panel.selected_index += 1;
                        *dirty = true;
                    }
                }
            }
            KeyCode::Left => {
                if let Some(ref mut scene) = self.scene {
                    let idx = self.panel.selected_index;
                    if idx < scene.lights.len() {
                        if let lighting::Light::Point {
                            ref mut position, ..
                        } = scene.lights[idx]
                        {
                            position.0 = (position.0 - 1.0).max(0.0);
                            *dirty = true;
                        }
                    }
                }
            }
            KeyCode::Right => {
                if let Some(ref mut scene) = self.scene {
                    let idx = self.panel.selected_index;
                    if idx < scene.lights.len() {
                        if let lighting::Light::Point {
                            ref mut position, ..
                        } = scene.lights[idx]
                        {
                            position.0 = (position.0 + 1.0).min(w as f32 - 1.0);
                            *dirty = true;
                        }
                    }
                }
            }
            KeyCode::Char('+') | KeyCode::Char('=') => {
                if let Some(ref mut scene) = self.scene {
                    LightPanel::adjust_intensity(scene, self.panel.selected_index, 0.1);
                    *dirty = true;
                }
            }
            KeyCode::Char('-') | KeyCode::Char('_') => {
                if let Some(ref mut scene) = self.scene {
                    LightPanel::adjust_intensity(scene, self.panel.selected_index, -0.1);
                    *dirty = true;
                }
            }
            KeyCode::Char('A') => {
                if let Some(ref mut scene) = self.scene {
                    scene.add_light(lighting::Light::Ambient {
                        intensity: 0.5,
                        color: lighting::Rgb(255, 255, 255),
                        target: lighting::LightTarget::default(),
                    });
                    self.panel.selected_index = scene.lights.len() - 1;
                    *dirty = true;
                }
            }
            KeyCode::Char('D') => {
                if let Some(ref mut scene) = self.scene {
                    scene.add_light(lighting::Light::Directional {
                        direction: (0.0, 0.0, 1.0),
                        intensity: 0.8,
                        color: lighting::Rgb(255, 255, 255),
                        target: lighting::LightTarget::default(),
                    });
                    self.panel.selected_index = scene.lights.len() - 1;
                    *dirty = true;
                }
            }
            KeyCode::Char('C') => {
                // Cycle the selected light's colour through the presets
                if let Some(ref mut scene) = self.scene {
                    if let Some(light) = scene.lights.get_mut(self.panel.selected_index) {
                        light.cycle_color();
                        *dirty = true;
                    }
                }
            }
            KeyCode::Char('P') => {
                if let Some(ref mut scene) = self.scene {
                    scene.add_light(lighting::Light::Point {
                        position: (w as f32 / 2.0, h as f32 / 2.0, 5.0),
                        intensity: 0.8,
                        color: lighting::Rgb(255, 255, 255),
                        attenuation: lighting::Attenuation::default(),
                        target: lighting::LightTarget::default(),
                    });
                    self.panel.selected_index = scene.lights.len() - 1;
                    *dirty = true;
                }
            }
            KeyCode::Delete => {
                if let Some(ref mut scene) = self.scene {
                    let idx = self.panel.selected_index;
                    if scene.remove_light(idx) {
                        if self.panel.selected_index >= scene.lights.len()
                            && !scene.lights.is_empty()
                        {
                            self.panel.selected_index = scene.lights.len() - 1;
                        }
                        *dirty = true;
                    }
                }
            }
            KeyCode::Char('K') => {
                // Insert/update a light keyframe at the current timeline time
                if let Some(ref scene) = self.scene {
                    let idx = self.panel.selected_index;
                    if idx < scene.lights.len() && timeline_total > 0 {
                        let t = timeline_frame as f32 / timeline_total as f32;
                        let props = self.current_light_properties(idx);
                        let kf = lighting::LightKeyframe::new(t, idx, props);
                        // Remove existing keyframe for this light at this time
                        self.light_keyframes
                            .retain(|k| !(k.light_index == idx && (k.time - t).abs() < 0.001));
                        self.light_keyframes.push(kf);
                        lighting::sort_light_keyframes(&mut self.light_keyframes);
                        *dirty = true;
                    }
                }
            }
            KeyCode::Char('N') => {
                // Jump to the next keyframe for the selected light
                if let Some(ref scene) = self.scene {
                    let idx = self.panel.selected_index;
                    if idx < scene.lights.len() {
                        let t = if timeline_total > 0 {
                            timeline_frame as f32 / timeline_total as f32
                        } else {
                            0.0
                        };
                        let next = self
                            .light_keyframes
                            .iter()
                            .filter(|k| k.light_index == idx && k.time > t + 0.001)
                            .min_by(|a, b| a.time.partial_cmp(&b.time).unwrap())
                            .map(|k| k.properties.clone());
                        if let Some(props) = next {
                            self.apply_keyframe_to_light(idx, &props);
                            *dirty = true;
                        }
                    }
                }
            }
            KeyCode::Char('X') => {
                // Delete keyframe(s) for the selected light at the current time
                if timeline_total > 0 {
                    let t = timeline_frame as f32 / timeline_total as f32;
                    let idx = self.panel.selected_index;
                    let before = self.light_keyframes.len();
                    self.light_keyframes
                        .retain(|k| !(k.light_index == idx && (k.time - t).abs() < 0.001));
                    if self.light_keyframes.len() < before {
                        *dirty = true;
                    }
                }
            }
            KeyCode::Char('G') => {}
            _ => return None,
        }
        Some(true)
    }
}

impl AnimationState {
    pub(crate) fn commit_current_timeline_frame(&mut self, editor: &EditorState) {
        let cf = self.timeline_state.current_frame;
        if cf < self.timeline_state.frames.len() {
            // Commit EVERY layer's buffer (GPT review F-05): an earlier
            // version stored only the flattened composite, which the load
            // path then wrote back into the active layer alone —
            // duplicating/corrupting multilayer documents on navigation.
            let document_state: Vec<canvas::CanvasBuffer> = editor
                .layer_stack
                .layers
                .iter()
                .map(|l| l.buffer.clone())
                .collect();
            let composite = editor.layer_stack.composite();
            let thumbnail = capture_thumbnail(&composite, 8, 3);
            let frame = &mut self.timeline_state.frames[cf];
            frame.document_state = document_state;
            frame.thumbnail = thumbnail;
            // NOTE: deliberately not touching has_keyframe — committing
            // pixels must not mint transform markers. Keyframes are
            // opt-in via the tween editor (set_keyframe derives the flag).
        }
    }

    pub(crate) fn load_current_timeline_frame(&mut self, editor: &mut EditorState) {
        let cf = self.timeline_state.current_frame;
        if let Some(states) = self
            .timeline_state
            .frames
            .get(cf)
            .map(|f| f.document_state.clone())
            .filter(|s| !s.is_empty())
        {
            editor.load_timeline_frame(&states);
        }
    }

    pub(crate) fn handle_key(
        &mut self,
        code: KeyCode,
        _modifiers: KeyModifiers,
        editor: &mut EditorState,
        dirty: &mut bool,
    ) -> bool {
        // Frame capture: the first captured frame opens the timeline
        // itself, so the animation system is discoverable from a blank
        // canvas. Frames are plain snapshots — keyframes (transform
        // markers) are opt-in via the tween editor, not stamped at
        // capture. Navigation below stays visibility-gated so arrows
        // keep driving the canvas cursor when the panel is hidden.
        if code == KeyCode::Char('A') {
            // Snapshot EVERY layer, not just the composite: a single
            // flattened buffer in document_state[0] makes load_timeline_frame
            // clear every other layer on navigation (canvas "reset").
            let document_state: Vec<canvas::CanvasBuffer> = editor
                .layer_stack
                .layers
                .iter()
                .map(|l| l.buffer.clone())
                .collect();
            let composite = editor.layer_stack.composite();
            let thumbnail = capture_thumbnail(&composite, 8, 3);
            let frame = timeline::TimelineFrame {
                thumbnail,
                has_keyframe: false,
                label: format!("F{}", self.timeline_state.frames.len()),
                delay: self.timeline_state.default_delay(),
                document_state,
                layer_keyframes: vec![None; editor.layer_stack.layers.len()],
            };
            self.timeline_state.sync_layer_names(&editor.layer_stack);
            self.timeline_state.add_frame(frame);
            self.timeline_visible = true;
            *dirty = true;
            return true;
        }
        // Timeline frame navigation (visible only)
        if self.timeline_visible {
            match code {
                KeyCode::Left if self.timeline_state.current_frame > 0 => {
                    self.commit_current_timeline_frame(editor);
                    self.timeline_state.current_frame -= 1;
                    let cf = self.timeline_state.current_frame;
                    if cf < self.timeline_state.scroll_offset {
                        self.timeline_state.scroll_offset = cf;
                    }
                    self.load_current_timeline_frame(editor);
                    *dirty = true;
                    return true;
                }
                KeyCode::Right
                    if self.timeline_state.current_frame + 1 < self.timeline_state.frames.len() =>
                {
                    self.commit_current_timeline_frame(editor);
                    self.timeline_state.current_frame += 1;
                    let cf = self.timeline_state.current_frame;
                    let max_vis = self.timeline_state.cached_max_vis_frames;
                    if cf >= self.timeline_state.scroll_offset + max_vis {
                        self.timeline_state.scroll_offset =
                            cf.saturating_sub(max_vis.saturating_sub(1));
                    }
                    self.load_current_timeline_frame(editor);
                    *dirty = true;
                    return true;
                }
                KeyCode::Delete if self.timeline_state.frames.len() > 1 => {
                    let _ = self
                        .timeline_state
                        .remove_frame(self.timeline_state.current_frame);
                    self.load_current_timeline_frame(editor);
                    *dirty = true;
                    return true;
                }
                _ => {}
            }
        }

        // Emitter bake / toggle keybindings
        if self.emitter_active {
            match code {
                KeyCode::Char('b') => {
                    let w = editor.canvas.buffer.width();
                    let h = editor.canvas.buffer.height();
                    let buf = self.particle_system.bake_to_buffer(w, h);
                    let indices = editor.layer_stack.add_frozen_frames(vec![buf], "bake");
                    self.baked_layer_indices.extend(indices);
                    editor.recomposite_canvas();
                    *dirty = true;
                    return true;
                }
                KeyCode::Char('B') => {
                    let w = editor.canvas.buffer.width();
                    let h = editor.canvas.buffer.height();
                    let frames = self.particle_system.bake_frames(10, w, h, 0.1);
                    let indices = editor.layer_stack.add_frozen_frames(frames, "bake");
                    self.baked_layer_indices.extend(indices);
                    editor.recomposite_canvas();
                    self.show_live_particles = false;
                    *dirty = true;
                    return true;
                }
                KeyCode::Char('v') => {
                    self.show_live_particles = !self.show_live_particles;
                    *dirty = true;
                    return true;
                }
                _ => {}
            }
        }

        false
    }

    /// Render the inline player widget if active.
    /// Returns `true` if the player rendered (caller should skip normal canvas rendering).
    pub fn render(&self, frame: &mut Frame<'_>, area: Rect, canvas_borders: Borders) -> bool {
        if let Some(ref player) = self.inline_player {
            let block = Block::default()
                .title(" Playing  [Space] pause  [\u{2190}/\u{2192}] seek  [+/-] speed  [l] loop  [Esc/q] stop ")
                .borders(canvas_borders);
            let inner = block.inner(area);
            frame.render_widget(block, area);

            let (content_w, content_h) = player.content_dimensions();
            let render_w = content_w.min(inner.width);
            let render_h = content_h.min(inner.height);
            let player_rect = Rect {
                x: inner.x + (inner.width.saturating_sub(render_w) / 2),
                y: inner.y + (inner.height.saturating_sub(render_h) / 2),
                width: render_w,
                height: render_h,
            };
            frame.render_widget(player, player_rect);
            true
        } else {
            false
        }
    }
}

/// A destructive document transition (quit / open / new) that is waiting
/// for the user to confirm what to do about unsaved changes. Shared by
/// the quit-confirm dialog so Open and New get the same save/discard/
/// cancel treatment as quitting (GPT review F-08).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingDocAction {
    Quit,
    NewFileSession,
    FontNewBlankSession,
    /// Close a document tab (8.5.3): armed when the tab has unsaved
    /// changes, executed after Save/Discard from the confirm dialog.
    CloseDocument(usize),
}

/// Dialog/overlay state — file ops, export, undo panel, settings panel, rascii import.
pub struct DialogState {
    pub file_ops: file_ops::FileOpsDialog,
    pub recent_files: file_ops::RecentFiles,
    pub export_dialog: export::ExportDialog,
    pub undo_panel: undo_panel::UndoPanel,
    pub settings: status::CanvasSettings,
    pub rascii_import: dialogs::RasciiImportDialog,
    pub gif_import: dialogs::GifImportDialog,
    pub new_image: dialogs::NewImageDialog,
    pub system_font: dialogs::SystemFontPickerDialog,
    pub font_import_options: dialogs::FontImportOptionsDialog,
    /// Read-only FX Graph overlay (F9).
    pub fx_graph: fx_script::FxGraphView,
    pub quit_confirm_dialog: bool,
    /// Full-label hit rects of the Save / Discard / Cancel buttons, stored
    /// each render.
    pub quit_confirm_buttons: [Rect; 3],
    /// Button under the mouse, drawn highlighted (welcome-screen style).
    pub quit_confirm_hovered: Option<usize>,
    pub quit_after_save: bool,
    /// Document revision captured when the current async save started.
    /// `None` when no save is in flight. On `SaveComplete`, unsaved state
    /// is only cleared (and a deferred quit only honored) if the document
    /// hasn't been mutated since — edits made during the save keep the
    /// document dirty (GPT review F-07).
    pub pending_save_revision: Option<u64>,
    /// The transition awaiting confirmation in the unsaved-changes dialog.
    pub pending_transition: Option<PendingDocAction>,
    /// Document tab a pending async font open should land in. Set when
    /// the open starts (which creates the tab up front); taken by the
    /// OpenComplete handler, which falls back to a fresh tab if the user
    /// closed it mid-load.
    pub pending_open_tab: Option<usize>,
}

/// Welcome/startup effects state.
pub struct WelcomeState {
    pub screen: welcome::WelcomeScreen,
    pub fx: Option<fx::WelcomeFx>,
    pub fade_in: Option<fx::AppFadeIn>,
}

/// Frame-timing bookkeeping for the render loop.
pub struct FrameState {
    pub dirty: bool,
    pub force_full_redraw: bool,
    pub last_draw_time: Instant,
    pub fps: f64,
    pub last_frame_time: Instant,
    pub delta_time: Duration,
    pub fx_last_tick: Instant,
}

/// UI-navigation/mode/chrome flags.
pub struct UiState {
    pub mode: AppMode,
    pub prev_mode: AppMode,
    pub session_type: SessionType,
    pub zen_mode: bool,
    pub show_keybindings: bool,
    pub keybindings_scroll: usize,
    pub menu_bar: MenuBar,
    pub menu_bar_state: MenuBarState,
    /// Per-tab hit rects for the document strip, recomputed every render
    /// (titles vary in width, so unlike `SidePanel::tab_rects` these can't
    /// be derived from the area alone) and consumed by the mouse handler.
    pub tab_rects: Vec<Rect>,
    pub should_quit: bool,
}

/// Ambient config, environment, and background infrastructure.
pub struct AppContext {
    pub icons: BTreeMap<String, String>,
    pub theme: theme::Theme,
    pub render_mode: RenderMode,
    pub git_branch: Option<String>,
    pub auto_save_interval: u64,
    pub last_save_time: Instant,
    pub throbber: ThrobberState,
    pub async_rx: Option<mpsc::Receiver<AsyncResult>>,
    pub palette_rgb_to_swatch: HashMap<(u8, u8, u8), usize>,
}

pub struct TuiApp {
    pub editor: EditorState,
    pub dialogs: DialogState,
    pub interaction: InteractionState,
    pub animation: AnimationState,
    pub lighting: LightingState,
    /// Open documents (8.5.1). The live per-document state is in the hot
    /// `editor`/`animation`/`lighting` fields above; `documents[active_doc]`
    /// always holds a placeholder — see `tui::documents`.
    pub documents: Vec<documents::Document>,
    pub active_doc: usize,
    pub side_panel: SidePanel,
    pub palette_editor: palette_editor::PaletteEditor,
    pub props_panel: PropsPanel,
    pub welcome: WelcomeState,
    pub frame: FrameState,
    pub ui: UiState,
    pub ctx: AppContext,
}

impl TuiApp {
    pub fn new() -> Self {
        let config = config::load_config();
        let icons = IconSet::resolve(
            std::env::var("FIGBY_ICONS").ok().as_deref(),
            config.tui.icons.as_deref(),
        )
        .icons();
        let theme = theme::load_theme(&config.tui.theme);

        let mut brush = brush::BrushState::new();
        if let Some(ref shape) = config.tui.brush.shape {
            brush.shape = match shape.as_str() {
                "square" => brush::BrushShape::Square,
                "circle" => brush::BrushShape::Circle,
                "spray" => brush::BrushShape::SprayPaint,
                "dither" => brush::BrushShape::Dither,
                "custom" => brush::BrushShape::Custom,
                _ => brush.shape,
            };
        }
        if let Some(size) = config.tui.brush.size {
            brush.set_size(size);
        }
        if let Some(density) = config.tui.brush.density {
            brush.set_density(density);
        }
        if let Some(ref ch_str) = config.tui.brush.ch {
            if let Some(ch) = ch_str.chars().next() {
                brush.ch = ch;
            }
        }

        let mut font_editor = font_editor::FontEditor::new();
        font_editor.theme = theme.clone();

        let mut file_ops = file_ops::FileOpsDialog::new();
        file_ops.theme = theme.clone();
        let mut recent_files = file_ops::RecentFiles::load_from_disk();
        if let Some(max) = config.tui.recent_files_max {
            recent_files.set_max(max);
        }

        let git_branch = std::process::Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    String::from_utf8(o.stdout)
                        .ok()
                        .map(|s| s.trim().to_string())
                } else {
                    None
                }
            });

        let render_mode = match config.tui.render_mode.as_deref() {
            Some("fast") | Some("Fast") => RenderMode::Fast,
            _ => RenderMode::Dirty,
        };

        let mut canvas = canvas::CanvasWidget::default();
        canvas.theme = theme.clone();
        let mut palette = palette::Palette::new();
        palette.theme = theme.clone();
        let mut export_dialog = export::ExportDialog::new();
        export_dialog.theme = theme.clone();
        let mut undo_panel = undo_panel::UndoPanel::new();
        undo_panel.theme = theme.clone();
        let mut settings = status::CanvasSettings::new();
        settings.theme = theme.clone();
        let mut toolbox = toolbox::Toolbox::new();
        toolbox.theme = theme.clone();
        toolbox.icons = icons.clone();

        let canvas_w = canvas.buffer.width();
        let canvas_h = canvas.buffer.height();
        let layer_stack = layers::LayerStack::new(canvas_w, canvas_h);
        let mut layer_panel = layers::LayerPanel::new();
        layer_panel.theme = theme.clone();
        layer_panel.icons = icons.clone();

        let mut rascii_import = dialogs::RasciiImportDialog::new();
        rascii_import.theme = theme.clone();
        let mut gif_import = dialogs::GifImportDialog::new();
        gif_import.theme = theme.clone();
        let mut new_image = dialogs::NewImageDialog::new();
        new_image.theme = theme.clone();
        let mut system_font = dialogs::SystemFontPickerDialog::new();
        system_font.theme = theme.clone();
        let mut font_import_options = dialogs::FontImportOptionsDialog::new();
        font_import_options.theme = theme.clone();

        Self {
            editor: {
                let mut editor = EditorState {
                    canvas,
                    toolbox,
                    brush,
                    palette,
                    font_editor,
                    image_editor: image_editor::ImageEditor::new(),
                    text_tool: tools::text::TextToolState::new(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../fonts"
                    )),
                    // Config-supplied limits are clamped: a hostile config with a huge
                    // undo_limit would pre-allocate unbounded history capacity
                    // (GPT review F-23).
                    undo: undo::UndoSystem::new(config.tui.undo_limit.unwrap_or(50).min(10_000)),
                    unsaved: false,
                    revision: 0,
                    selection: None,
                    clipboard: None,
                    layer_stack,
                    layer_panel,
                    fill_threshold: 0,
                    eyedropper_sample: None,
                    move_state: tools::move_tool::MoveState::default(),
                    rotate_state: tools::rotate_tool::RotateState::default(),
                    selection_state: tools::selection::SelectionState::default(),
                    line_state: tools::line::LineState::default(),
                    selection_polygon_points: Vec::new(),
                    figmap_path: None,
                    font_char_loaded: None,
                };
                editor.recomposite_canvas();
                editor
            },
            dialogs: DialogState {
                file_ops,
                recent_files,
                export_dialog,
                undo_panel,
                settings,
                rascii_import,
                gif_import,
                new_image,
                system_font,
                font_import_options,
                fx_graph: fx_script::FxGraphView::new(),
                quit_confirm_dialog: false,
                quit_confirm_buttons: [Rect::default(); 3],
                quit_confirm_hovered: None,
                quit_after_save: false,
                pending_save_revision: None,
                pending_transition: None,
                pending_open_tab: None,
            },
            interaction: InteractionState {
                selection_drag_origin: None,
                selection_lasso_points: Vec::new(),
                prev_mouse_buf: None,
                mouse_batch_active: false,
                line_start: None,
                saved_buffer: None,
                move_origin: None,
                rotate_origin: None,
                saved_selection: None,
            },
            animation: AnimationState {
                timeline_state: timeline::TimelineState::default(),
                particle_system: particles::ParticleSystem::new(
                    particles::ParticleConfig::default(),
                ),
                emitter_active: false,
                emitter_follow_layer: None,
                emitter_panel: particles::EmitterConfigPanel::new(),
                show_live_particles: true,
                baked_layer_indices: Vec::new(),
                timeline_visible: false,
                marker_accum: HashMap::new(),
                loop_enabled: false,
                inline_player: None,
                transport_rects: Vec::new(),
            },
            // Single initial document; the slot holds a placeholder while
            // the hot fields above are live (see tui::documents).
            documents: vec![documents::Document::placeholder(
                documents::DocumentKind::Font,
            )],
            active_doc: 0,
            lighting: LightingState {
                scene: None,
                max_shadow_distance: 50,
                height_scale: 0.5,
                lut: lighting::LightingLut::from_palette(
                    (0, 0, 0),
                    (255, 255, 255),
                    crate::image_input::DEFAULT_CHAR_MAP,
                ),
                panel: LightPanel::new(),
                light_keyframes: Vec::new(),
            },
            side_panel: SidePanel::new(icons.clone(), theme.clone()),
            palette_editor: palette_editor::PaletteEditor::new(),
            props_panel: PropsPanel::new(),
            welcome: WelcomeState {
                screen: welcome::WelcomeScreen::new(),
                fx: Some(fx::WelcomeFx::new()),
                fade_in: Some(fx::AppFadeIn::new()),
            },
            frame: FrameState {
                dirty: true,
                force_full_redraw: false,
                last_draw_time: Instant::now(),
                fps: 0.0,
                last_frame_time: Instant::now(),
                delta_time: Duration::ZERO,
                fx_last_tick: Instant::now(),
            },
            ui: UiState {
                mode: AppMode::FontEditor,
                prev_mode: AppMode::FontEditor,
                session_type: SessionType::Any,
                zen_mode: false,
                show_keybindings: false,
                keybindings_scroll: 0,
                menu_bar: MenuBar::new(),
                menu_bar_state: MenuBarState::new(),
                tab_rects: Vec::new(),
                should_quit: false,
            },
            ctx: AppContext {
                icons: icons.clone(),
                theme: theme.clone(),
                render_mode,
                git_branch,
                auto_save_interval: 0,
                last_save_time: Instant::now(),
                throbber: ThrobberState::new(),
                async_rx: None,
                palette_rgb_to_swatch: HashMap::new(),
            },
        }
    }

    /// Whether the side panel (drawer) should start open by default: only
    /// when the terminal is wide enough that opening it still leaves a
    /// reasonably usable canvas, rather than cramming toolbox + canvas +
    /// drawer into too little space.
    ///
    /// This is the default for the image/canvas modes only. The font
    /// editor always starts with the drawer closed, see
    /// [`TuiApp::apply_default_side_panel`].
    pub(crate) fn default_side_panel_open(&self, term_width: u16) -> bool {
        const MIN_CANVAS_WIDTH: u16 = 60;
        let toolbox_width = self
            .editor
            .toolbox
            .required_width(self.editor.brush.required_outer_width());
        term_width >= toolbox_width + layout::DRAWER_WIDTH + MIN_CANVAS_WIDTH
    }

    /// Apply the startup drawer default for both mode families: closed in
    /// the font editor, open on a wide terminal everywhere else.
    pub(crate) fn apply_default_side_panel(&mut self, term_width: u16) {
        let image_open = self.default_side_panel_open(term_width);
        let font_mode = self.ui.mode == AppMode::FontEditor;
        self.side_panel.set_default_open(font_mode, image_open);
    }

    /// Point the drawer at the current mode (own open state per mode
    /// family, no Layers tab in the font editor). Called at the top of
    /// render and of key/mouse dispatch.
    pub(crate) fn sync_side_panel_mode(&mut self) {
        self.side_panel
            .sync_mode(self.ui.mode == AppMode::FontEditor);
    }

    /// Whether the canvas belongs to the glyph character editor: font mode
    /// with a glyph open. In every other mode or view the shared canvas
    /// holds something else and must never be written into a glyph.
    pub(crate) fn char_editor_active(&self) -> bool {
        self.ui.mode == AppMode::FontEditor
            && matches!(
                self.editor.font_editor.view,
                font_editor::FontEditorView::CharEditor(_)
            )
    }

    /// Keep the glyph and the canvas marker consistent at every event and
    /// frame boundary. Called at the top of render and of key/mouse
    /// dispatch. While the character editor is active the canvas is saved
    /// into its glyph (so edits are kept whatever the next key does, a
    /// mode switch included). Anywhere else the marker is cleared, so a
    /// canvas changed by another mode is never written into the glyph and
    /// the glyph is reloaded when its editor is shown again.
    pub(crate) fn sync_font_char_owner(&mut self) {
        if self.char_editor_active() {
            self.editor.sync_canvas_to_font_char();
        } else {
            self.editor.font_char_loaded = None;
        }
    }

    /// Write the canvas back into the open glyph after a canvas change
    /// (timeline frame navigation), only while the character editor owns
    /// the canvas.
    pub(crate) fn sync_canvas_to_open_glyph(&mut self) {
        if self.char_editor_active() {
            self.editor.sync_canvas_to_font_char();
        }
    }

    /// The toolbox/palette column is shown everywhere except the font
    /// editor's non-drawing views (overview, header, smush, transform):
    /// only the glyph character editor paints.
    pub(crate) fn toolbox_visible(&self) -> bool {
        self.ui.mode != AppMode::FontEditor
            || matches!(
                self.editor.font_editor.view,
                font_editor::FontEditorView::CharEditor(_)
            )
    }

    /// Screen layout for `area`, shared by render and the mouse handlers
    /// so hit-testing always matches what is drawn.
    pub(crate) fn frame_layout(&self, area: Rect) -> layout::FrameLayout {
        let tw = if self.toolbox_visible() {
            self.editor
                .toolbox
                .required_width(self.editor.brush.required_outer_width())
        } else {
            0
        };
        let toolbox_h = Tool::all().len() as u16 + 2;
        layout::FrameLayout::compute(
            area,
            self.ui.zen_mode,
            self.side_panel.open,
            tw,
            toolbox_h,
            self.animation.timeline_visible,
        )
    }

    /// Returns `true` when any text-entry field is live across the
    /// application. Used to suppress bare-letter tool-select, palette
    /// color, and image-editor shortcuts that would collide with typed
    /// characters.
    pub(crate) fn is_typing(&self) -> bool {
        self.editor.text_tool.editing
            || self.editor.image_editor.entering_path()
            || self.editor.palette.is_typing()
            || self.editor.layer_panel.renaming.is_some()
            || self.editor.font_editor.search_active
            || self.editor.font_editor.code_input_active
            || self.editor.font_editor.preview_input_active
            || self.props_panel.mode != PropsPanelMode::Idle
            || self.animation.emitter_panel.editing
            || self.palette_editor.is_typing()
            || self.animation.timeline_state.keyframe_editor.edit_mode
    }
}

pub enum AsyncResult {
    SaveComplete(Result<std::path::PathBuf, String>),
    OpenComplete(Result<(crate::font::FIGfont, std::path::PathBuf), String>),
    SystemFontComplete(Result<(crate::font::FIGfont, String), String>),
    ExportComplete(Result<(), String>),
    AutoSaveComplete,
}

impl Default for TuiApp {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod editor_state_tests {
    use super::*;

    fn make_test_editor(w: usize, h: usize) -> EditorState {
        EditorState {
            canvas: canvas::CanvasWidget::new(w as u16, h as u16),
            toolbox: toolbox::Toolbox::new(),
            brush: brush::BrushState::new(),
            palette: palette::Palette::new(),
            font_editor: font_editor::FontEditor::new(),
            image_editor: image_editor::ImageEditor::new(),
            text_tool: tools::text::TextToolState::new(""),
            undo: undo::UndoSystem::new(50),
            unsaved: false,
            revision: 0,
            selection: None,
            clipboard: None,
            layer_stack: layers::LayerStack::new(w, h),
            layer_panel: layers::LayerPanel::new(),
            fill_threshold: 10,
            eyedropper_sample: None,
            move_state: tools::move_tool::MoveState::default(),
            rotate_state: tools::rotate_tool::RotateState::default(),
            selection_state: tools::selection::SelectionState::default(),
            line_state: tools::line::LineState::default(),
            selection_polygon_points: Vec::new(),
            figmap_path: None,
            font_char_loaded: None,
        }
    }

    #[test]
    fn test_icon_set_resolution() {
        assert_eq!(IconSet::resolve(None, None), IconSet::Nerd);
        assert_eq!(IconSet::resolve(None, Some("ascii")), IconSet::Ascii);
        assert_eq!(IconSet::resolve(None, Some(" ASCII ")), IconSet::Ascii);
        assert_eq!(IconSet::resolve(None, Some("nerd")), IconSet::Nerd);
        // The environment wins over the config file, both ways...
        assert_eq!(IconSet::resolve(Some("nerd"), Some("ascii")), IconSet::Nerd);
        assert_eq!(
            IconSet::resolve(Some("ascii"), Some("nerd")),
            IconSet::Ascii
        );
        // ...unless it is set but empty.
        assert_eq!(IconSet::resolve(Some(""), Some("ascii")), IconSet::Ascii);
        // Unknown values keep the default rather than failing.
        assert_eq!(IconSet::resolve(Some("emoji"), None), IconSet::Nerd);
    }

    #[test]
    fn test_load_timeline_frame_updates_canvas_and_active_layer() {
        let mut editor = make_test_editor(3, 3);
        let mut buf = canvas::CanvasBuffer::new(3, 3);
        buf.set(
            0,
            0,
            canvas::CanvasCell {
                ch: 'X',
                fg: None,
                bg: None,
                height: None,
            },
        );

        editor.load_timeline_frame(&[buf]);

        assert_eq!(editor.canvas.buffer.get(0, 0).unwrap().ch, 'X');
        assert_eq!(
            editor.layer_stack.active_layer().buffer.get(0, 0).unwrap().ch,
            'X',
            "the active layer should carry the frame's raster so future edits and recomposites stay consistent"
        );
    }

    #[test]
    fn test_load_timeline_frame_overwrites_prior_canvas_content() {
        let mut editor = make_test_editor(2, 2);
        editor.layer_stack.active_layer_mut().buffer_mut().set(
            1,
            1,
            canvas::CanvasCell {
                ch: 'A',
                fg: None,
                bg: None,
                height: None,
            },
        );
        editor.recomposite_canvas();
        assert_eq!(editor.canvas.buffer.get(1, 1).unwrap().ch, 'A');

        let frame_buf = canvas::CanvasBuffer::new(2, 2); // blank frame
        editor.load_timeline_frame(&[frame_buf]);

        assert_eq!(
            editor.canvas.buffer.get(1, 1).unwrap().ch,
            ' ',
            "selecting a different timeline frame should replace the previously shown content"
        );
    }

    #[test]
    fn test_move_layer_shifts_content_and_recomposites_canvas() {
        let mut editor = make_test_editor(4, 4);
        editor.layer_stack.active_layer_mut().buffer_mut().set(
            1,
            1,
            canvas::CanvasCell {
                ch: 'X',
                fg: None,
                bg: None,
                height: None,
            },
        );
        editor.recomposite_canvas();

        editor.move_layer(1, 0);

        assert_eq!(
            editor
                .layer_stack
                .active_layer()
                .buffer
                .get(2, 1)
                .unwrap()
                .ch,
            'X',
            "move_layer should shift the active layer's content"
        );
        assert_eq!(
            editor.canvas.buffer.get(2, 1).unwrap().ch,
            'X',
            "move_layer must recomposite so the canvas reflects the shift immediately"
        );
    }

    #[test]
    fn test_move_selection_recomposites_canvas() {
        let mut editor = make_test_editor(4, 4);
        editor.layer_stack.active_layer_mut().buffer_mut().set(
            0,
            0,
            canvas::CanvasCell {
                ch: 'Y',
                fg: None,
                bg: None,
                height: None,
            },
        );
        editor.recomposite_canvas();
        editor.selection = Some(tools::selection::Selection::marquee(
            &editor.canvas.buffer,
            0,
            0,
            0,
            0,
        ));

        editor.move_selection(1, 1);

        assert_eq!(
            editor.canvas.buffer.get(1, 1).unwrap().ch,
            'Y',
            "move_selection must recomposite so the canvas reflects the move immediately, \
             not just the underlying layer buffer"
        );
    }

    #[test]
    fn test_rotate_layer_no_selection_rotates_whole_buffer_and_recomposites() {
        let mut editor = make_test_editor(3, 3);
        editor.layer_stack.active_layer_mut().buffer_mut().set(
            1,
            0,
            canvas::CanvasCell {
                ch: 'X',
                fg: None,
                bg: None,
                height: None,
            },
        );
        editor.recomposite_canvas();

        editor.rotate_selection_or_layer(true);

        assert_eq!(
            editor
                .layer_stack
                .active_layer()
                .buffer
                .get(2, 1)
                .unwrap()
                .ch,
            'X',
            "rotate_selection_or_layer should rotate the whole layer clockwise when no selection is active"
        );
        assert_eq!(
            editor.canvas.buffer.get(2, 1).unwrap().ch,
            'X',
            "rotate_selection_or_layer must recomposite so the canvas reflects the rotation immediately"
        );
    }

    #[test]
    fn test_rotate_selection_rotates_mask_and_content_together() {
        let mut editor = make_test_editor(3, 3);
        editor.layer_stack.active_layer_mut().buffer_mut().set(
            2,
            0,
            canvas::CanvasCell {
                ch: 'X',
                fg: None,
                bg: None,
                height: None,
            },
        );
        editor.recomposite_canvas();
        editor.selection = Some(tools::selection::Selection::marquee(
            &editor.canvas.buffer,
            0,
            0,
            2,
            0,
        ));

        editor.rotate_selection_or_layer(true);

        assert_eq!(
            editor.canvas.buffer.get(1, 1).unwrap().ch,
            'X',
            "rotating an active selection must rotate its content, not just the mask"
        );
        assert!(
            editor.selection.as_ref().unwrap().is_selected(1, 1),
            "rotating an active selection must move its mask along with the content"
        );
        assert!(
            !editor.selection.as_ref().unwrap().is_selected(2, 0),
            "the mask's old position should no longer be selected after rotating"
        );
    }

    /// F-05 (GPT review): committing a frame must snapshot EVERY layer,
    /// and loading it back must restore every layer — navigation used to
    /// flatten the composite into the active layer only, corrupting
    /// multilayer documents.
    /// F-09 (GPT review): editing layer 0, switching to layer 1, then
    /// undoing must restore layer 0's own state and leave layer 1 alone
    /// — not paste layer 0's pixels into layer 1.
    #[test]
    fn test_undo_targets_recorded_layer_not_active() {
        let mut editor = make_test_editor(2, 2);
        editor
            .layer_stack
            .layers
            .push(layers::Layer::new(2, 2, "Second".to_string()));
        let cell = |ch| canvas::CanvasCell {
            ch,
            fg: None,
            bg: None,
            height: None,
        };

        // Snapshot layer 0 BEFORE editing it (the app pushes snapshots
        // prior to applying a stroke), then paint 'A'.
        editor.layer_stack.active = 0;
        editor.push_undo_snapshot("draw A");
        editor.layer_stack.layers[0]
            .buffer_mut()
            .set(0, 0, cell('A'));

        // Switch to layer 1 and paint 'B' directly (no snapshot).
        editor.layer_stack.active = 1;
        editor.layer_stack.layers[1]
            .buffer_mut()
            .set(1, 1, cell('B'));

        // Undo while layer 1 is active.
        let target = editor.undo.undo_top_layer().expect("undo entry exists");
        assert_eq!(target, 0, "entry belongs to layer 0");
        let cur = editor.layer_stack.layers[target].buffer.clone();
        if let Some((buf, idx, _)) = editor.undo.undo(cur, target) {
            assert!(editor.restore_undo_snapshot(idx, buf));
        } else {
            panic!("undo should pop an entry");
        }

        assert_eq!(
            editor.layer_stack.layers[0].buffer().get(0, 0).unwrap().ch,
            ' ',
            "layer 0 must be restored to its pre-edit state"
        );
        assert_eq!(
            editor.layer_stack.active, 0,
            "undo should make the restored layer active"
        );
        assert_eq!(
            editor.layer_stack.layers[1].buffer().get(1, 1).unwrap().ch,
            'B',
            "layer 1 content must be untouched by layer 0's undo"
        );
    }

    /// F-09 (GPT review): locked layers reject undo restorations too —
    /// all writes pass one lock-aware check.
    #[test]
    fn test_restore_rejects_locked_layer() {
        let mut editor = make_test_editor(2, 2);
        editor.layer_stack.active_layer_mut().locked = true;
        let buf = canvas::CanvasBuffer::new(2, 2);
        assert!(!editor.restore_undo_snapshot(0, buf));
    }

    #[test]
    fn test_timeline_commit_and_load_preserve_all_layers() {
        let mut editor = make_test_editor(2, 2);
        // Give the stack a second layer with distinct content:
        // L0='0' at (0,0), L1='1' at (1,1).
        editor
            .layer_stack
            .layers
            .push(layers::Layer::new(2, 2, "Second".to_string()));
        editor.layer_stack.layers[0].buffer_mut().set(
            0,
            0,
            canvas::CanvasCell {
                ch: '0',
                fg: None,
                bg: None,
                height: None,
            },
        );
        editor.layer_stack.layers[1].buffer_mut().set(
            1,
            1,
            canvas::CanvasCell {
                ch: '1',
                fg: None,
                bg: None,
                height: None,
            },
        );

        let mut anim = AnimationState {
            timeline_state: timeline::TimelineState::default(),
            particle_system: particles::ParticleSystem::new(particles::ParticleConfig::default()),
            emitter_active: false,
            emitter_follow_layer: None,
            emitter_panel: particles::EmitterConfigPanel::new(),
            show_live_particles: true,
            baked_layer_indices: Vec::new(),
            timeline_visible: false,
            marker_accum: HashMap::new(),
            loop_enabled: false,
            inline_player: None,
            transport_rects: Vec::new(),
        };
        anim.timeline_state.add_frame(timeline::TimelineFrame {
            thumbnail: vec![],
            has_keyframe: true,
            label: "F0".into(),
            delay: 10,
            document_state: Vec::new(),
            layer_keyframes: vec![None, None],
        });
        anim.commit_current_timeline_frame(&editor);
        assert_eq!(
            anim.timeline_state.frames[0].document_state.len(),
            2,
            "commit must snapshot every layer"
        );

        // Mutate both layers after commit (simulating edits on frame 1).
        editor.layer_stack.layers[0].buffer_mut().set(
            0,
            0,
            canvas::CanvasCell {
                ch: 'x',
                fg: None,
                bg: None,
                height: None,
            },
        );
        editor.layer_stack.layers[1].buffer_mut().set(
            1,
            1,
            canvas::CanvasCell {
                ch: 'y',
                fg: None,
                bg: None,
                height: None,
            },
        );

        // Navigate back to the committed frame.
        anim.load_current_timeline_frame(&mut editor);
        assert_eq!(
            editor.layer_stack.layers[0].buffer().get(0, 0).unwrap().ch,
            '0',
            "layer 0 content must be restored exactly"
        );
        assert_eq!(
            editor.layer_stack.layers[1].buffer().get(1, 1).unwrap().ch,
            '1',
            "layer 1 must be restored too, not just the active layer"
        );
    }
}

#[cfg(test)]
mod default_side_panel_open_tests {
    use super::TuiApp;

    #[test]
    fn test_default_side_panel_open_respects_width_threshold() {
        let app = TuiApp::new();
        let toolbox_width = app
            .editor
            .toolbox
            .required_width(app.editor.brush.required_outer_width());
        let threshold = toolbox_width + super::layout::DRAWER_WIDTH + 60;

        assert!(!app.default_side_panel_open(threshold - 1));
        assert!(app.default_side_panel_open(threshold));
        assert!(app.default_side_panel_open(threshold + 20));
    }

    #[test]
    fn test_default_side_panel_open_closed_for_narrow_classic_80col_terminal() {
        let app = TuiApp::new();
        assert!(!app.default_side_panel_open(80));
    }
}

/// Font mode hides the toolbox column outside the glyph character
/// editor, starts with the drawer closed and never offers Layers.
#[cfg(test)]
mod font_mode_chrome_tests {
    use super::{AppMode, TuiApp};
    use crate::font::{FIGcharacter, FIGfont};
    use crate::tui::font_editor::FontEditorView;
    use crate::tui::side_panel::TabId;
    use crate::tui::toolbox::Tool;
    use ratatui::backend::TestBackend;
    use ratatui::crossterm::event::{
        KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    };
    use ratatui::layout::Rect;
    use ratatui::Terminal;
    use std::collections::HashMap;

    const W: u16 = 120;
    const H: u16 = 40;

    /// 40 glyphs, 3 rows high, so the grid fills several rows at any width.
    fn font() -> FIGfont {
        let chars = (0..40u32)
            .map(|i| {
                let c = char::from_u32(65 + (i % 26)).unwrap();
                let row = format!("{c}{c}{c}  ");
                (
                    65 + i,
                    FIGcharacter::from(vec![row.clone(), row.clone(), row]),
                )
            })
            .collect::<HashMap<_, _>>();
        FIGfont {
            charheight: 3,
            maxlength: 5,
            chars,
            ..FIGfont::default()
        }
    }

    /// Font overview as the event loop starts it on a 120x40 terminal.
    fn font_app() -> TuiApp {
        let mut app = TuiApp::new();
        app.welcome.screen.show = false;
        app.animation.timeline_visible = false;
        app.editor.font_editor.load_font(font());
        app.apply_default_side_panel(W);
        assert_eq!(app.ui.mode, AppMode::FontEditor);
        assert_eq!(app.editor.font_editor.view, FontEditorView::Overview);
        app
    }

    /// Render and return the main area only (below the menu bar and tab
    /// strip, above the status bar): the menu bar has its own "Tools"
    /// and "Layers" menus.
    fn draw(app: &mut TuiApp) -> String {
        let mut terminal = Terminal::new(TestBackend::new(W, H)).unwrap();
        terminal.draw(|f| app.render(f)).unwrap();
        let buf = terminal.backend().buffer();
        let mut out = String::new();
        for y in 4..H - 1 {
            for x in 0..W {
                out.push_str(buf[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    fn key(app: &mut TuiApp, code: KeyCode) {
        let _ = app.handle_key_event(KeyEvent::new(code, KeyModifiers::NONE));
    }

    fn area() -> Rect {
        Rect::new(0, 0, W, H)
    }

    #[test]
    fn test_font_overview_has_no_toolbox_and_no_drawer_by_default() {
        let mut app = font_app();
        let out = draw(&mut app);
        assert!(
            !app.side_panel.open,
            "drawer must start closed in font mode"
        );
        let fl = app.frame_layout(area());
        assert!(fl.toolbox_full.is_none() && fl.toolbox_list.is_none());
        assert!(fl.palette.is_none());
        assert!(fl.right_panel.is_none());
        assert_eq!(fl.canvas.x, 0);
        assert_eq!(fl.canvas.width, W);
        assert!(!out.contains(" Tools "), "toolbox drawn in overview");
        assert!(!out.contains("Palette"), "palette drawn in overview");
        assert!(!out.contains("tabs "), "drawer drawn in overview");
    }

    #[test]
    fn test_font_header_smush_transform_views_hide_toolbox() {
        let mut app = font_app();
        for view in [
            FontEditorView::HeaderEditor,
            FontEditorView::SmushRuleEditor,
            FontEditorView::TransformEditor,
        ] {
            app.editor.font_editor.view = view;
            let out = draw(&mut app);
            assert!(app.frame_layout(area()).toolbox_full.is_none(), "{view:?}");
            assert!(!out.contains(" Tools "), "toolbox drawn in {view:?}");
        }
    }

    #[test]
    fn test_char_editor_shows_toolbox() {
        let mut app = font_app();
        draw(&mut app);
        key(&mut app, KeyCode::Enter);
        assert!(matches!(
            app.editor.font_editor.view,
            FontEditorView::CharEditor(_)
        ));
        let out = draw(&mut app);
        let fl = app.frame_layout(area());
        assert!(fl.toolbox_list.is_some() && fl.palette.is_some());
        assert!(out.contains(" Tools "), "toolbox missing in char editor");
    }

    #[test]
    fn test_overview_to_glyph_and_back_toggles_toolbox() {
        let mut app = font_app();
        assert!(!draw(&mut app).contains(" Tools "));
        key(&mut app, KeyCode::Enter);
        assert!(draw(&mut app).contains(" Tools "), "glyph open: toolbox");
        key(&mut app, KeyCode::Esc);
        assert_eq!(app.editor.font_editor.view, FontEditorView::Overview);
        assert!(!draw(&mut app).contains(" Tools "), "back: no toolbox");
    }

    /// Set Char Height the way the header editor's H field does: only the
    /// header value changes, glyph rows are kept as they are.
    fn set_header_height(app: &mut TuiApp, h: u32) {
        app.editor.font_editor.font.as_mut().unwrap().charheight = h;
    }

    fn double_click_glyph(app: &mut TuiApp, code: u32) {
        draw(app);
        let rect = app
            .editor
            .font_editor
            .grid_state
            .cell_rects
            .iter()
            .find(|(c, _)| *c == code)
            .map(|(_, r)| *r)
            .expect("glyph cell drawn");
        for _ in 0..2 {
            app.handle_mouse_event(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: rect.x + 1,
                row: rect.y + 1,
                modifiers: KeyModifiers::NONE,
            });
        }
        assert_eq!(
            app.editor.font_editor.view,
            FontEditorView::CharEditor(code)
        );
    }

    fn canvas_rows(app: &TuiApp) -> Vec<String> {
        let b = &app.editor.canvas.buffer;
        (0..b.height())
            .map(|y| {
                (0..b.width())
                    .map(|x| b.get(x, y).map_or(' ', |c| c.ch))
                    .collect()
            })
            .collect()
    }

    fn glyph_rows(app: &TuiApp, code: u32) -> Vec<String> {
        app.editor.font_editor.font.as_ref().unwrap().chars[&code]
            .rows()
            .to_vec()
    }

    #[test]
    fn test_double_click_after_height_increase_shows_glyph() {
        let mut app = font_app();
        set_header_height(&mut app, 7);
        double_click_glyph(&mut app, 66);
        draw(&mut app);
        let rows = canvas_rows(&app);
        assert_eq!(rows.len(), 7, "canvas sized to the new height");
        for row in &rows[..3] {
            assert_eq!(row, "BBB  ", "glyph art visible in the canvas");
        }
        for row in &rows[3..] {
            assert_eq!(row.trim(), "", "padding rows are blank");
        }
        // Write-back keeps the art and fills the full height.
        let glyph = glyph_rows(&app, 66);
        assert_eq!(glyph.len(), 7);
        assert_eq!(glyph[0], "BBB  ");
        assert_eq!(glyph[2], "BBB  ");
        // Paint into a new row: it is saved, and the art stays.
        let mut buf = app.editor.layer_stack.active_layer().buffer.clone();
        buf.set(
            1,
            5,
            crate::tui::canvas::CanvasCell {
                ch: '#',
                fg: None,
                bg: None,
                height: None,
            },
        );
        *app.editor.layer_stack.active_layer_mut().buffer_mut() = buf;
        app.editor.recomposite_canvas();
        draw(&mut app);
        let glyph = glyph_rows(&app, 66);
        assert_eq!(glyph[0], "BBB  ");
        assert_eq!(glyph[5], " #   ");
    }

    #[test]
    fn test_double_click_after_height_decrease_shows_glyph() {
        let mut app = font_app();
        set_header_height(&mut app, 2);
        double_click_glyph(&mut app, 67);
        draw(&mut app);
        assert_eq!(canvas_rows(&app), vec!["CCC  ", "CCC  "]);
        assert_eq!(glyph_rows(&app, 67), vec!["CCC  ", "CCC  "]);
    }

    #[test]
    fn test_glyph_with_fewer_rows_than_height_is_padded() {
        let mut app = font_app();
        app.editor
            .font_editor
            .font
            .as_mut()
            .unwrap()
            .chars
            .insert(65, FIGcharacter::from(vec!["X X  ".to_string()]));
        // Keyboard entry (Enter on the first glyph) and mouse entry agree.
        draw(&mut app);
        key(&mut app, KeyCode::Enter);
        assert_eq!(app.editor.font_editor.view, FontEditorView::CharEditor(65));
        draw(&mut app);
        assert_eq!(canvas_rows(&app), vec!["X X  ", "     ", "     "]);
        assert_eq!(glyph_rows(&app, 65).len(), 3);
        key(&mut app, KeyCode::Esc);
        double_click_glyph(&mut app, 65);
        draw(&mut app);
        assert_eq!(canvas_rows(&app), vec!["X X  ", "     ", "     "]);
    }

    #[test]
    fn test_reopening_after_other_glyph_shows_current_glyph() {
        let mut app = font_app();
        double_click_glyph(&mut app, 66);
        draw(&mut app);
        key(&mut app, KeyCode::Esc);
        set_header_height(&mut app, 5);
        double_click_glyph(&mut app, 68);
        draw(&mut app);
        let rows = canvas_rows(&app);
        assert_eq!(rows.len(), 5);
        assert_eq!(rows[0], "DDD  ", "no stale B from the previous glyph");
        assert_eq!(glyph_rows(&app, 66)[0], "BBB  ", "other glyph untouched");
    }

    #[test]
    fn test_overview_renders_taller_glyph_after_height_increase() {
        let mut app = font_app();
        set_header_height(&mut app, 5);
        double_click_glyph(&mut app, 66);
        draw(&mut app);
        key(&mut app, KeyCode::Esc);
        let out = draw(&mut app);
        assert!(out.contains(" BBB"), "glyph art still drawn in the grid");
        assert_eq!(glyph_rows(&app, 66).len(), 5);
    }

    #[test]
    fn test_image_mode_shows_toolbox_and_drawer() {
        let mut app = TuiApp::new();
        app.welcome.screen.show = false;
        app.ui.mode = AppMode::ImageEditor;
        app.apply_default_side_panel(W);
        let out = draw(&mut app);
        assert!(app.side_panel.open, "wide image mode opens the drawer");
        assert_eq!(app.side_panel.active_tab, TabId::Layers);
        let fl = app.frame_layout(area());
        assert!(fl.toolbox_full.is_some() && fl.right_panel.is_some());
        assert!(out.contains(" Tools "));
        assert!(out.contains("Layers"));
    }

    #[test]
    fn test_image_drawer_default_survives_starting_in_font_mode() {
        // The app starts in the font editor (welcome screen); switching
        // to image mode must still bring up the wide-terminal drawer.
        let mut app = font_app();
        draw(&mut app);
        assert!(!app.side_panel.open);
        app.ui.mode = AppMode::ImageEditor;
        draw(&mut app);
        assert!(app.side_panel.open, "image mode keeps its drawer default");
        assert_eq!(app.side_panel.active_tab, TabId::Layers);
        app.ui.mode = AppMode::FontEditor;
        draw(&mut app);
        assert!(!app.side_panel.open, "font mode keeps its own state");
    }

    #[test]
    fn test_layers_tab_not_offered_in_font_mode() {
        let mut app = font_app();
        draw(&mut app);
        // The user can still open the drawer by hand.
        app.side_panel.toggle_open();
        let out = draw(&mut app);
        assert!(app.side_panel.open);
        assert_eq!(app.side_panel.tabs(), TabId::font_mode());
        assert!(!app.side_panel.tabs().contains(&TabId::Layers));
        assert_ne!(app.side_panel.active_tab, TabId::Layers);
        assert!(!out.contains("Layers"), "Layers tab label drawn");
        for _ in 0..10 {
            let _ = app.handle_key_event(KeyEvent::new(KeyCode::Right, KeyModifiers::ALT));
            assert_ne!(app.side_panel.active_tab, TabId::Layers);
        }
        app.side_panel.set_active_tab(TabId::Layers);
        assert_ne!(app.side_panel.active_tab, TabId::Layers);
        // A stale Layers tab (set directly) is corrected on the next sync.
        app.side_panel.active_tab = TabId::Layers;
        draw(&mut app);
        assert_eq!(app.side_panel.active_tab, TabId::Props);
        // Every tab label click lands on a font-mode tab.
        let rp = app.frame_layout(area()).right_panel.unwrap();
        for r in app.side_panel.tab_rects(rp) {
            let tab = app.side_panel.tab_at_pos(r.x, r.y, rp).unwrap();
            assert_ne!(tab, TabId::Layers);
        }
    }

    #[test]
    fn test_grid_uses_full_width_without_toolbox() {
        let mut app = font_app();
        draw(&mut app);
        let cell_w = 8; // (maxlength 5 + 2).max(8)
        let inner_w = W as usize - 2; // canvas borders, no toolbox, no drawer
        assert_eq!(app.editor.font_editor.grid_state.cols(), inner_w / cell_w);
        // The rightmost recorded cell sits past where the toolbox column
        // used to end the grid.
        let max_x = app
            .editor
            .font_editor
            .grid_state
            .cell_rects
            .iter()
            .map(|(_, r)| r.x + r.width)
            .max()
            .unwrap();
        assert!(max_x > W - 10, "grid stops at {max_x}");
        assert!(app.editor.font_editor.grid_state.cell_rects[0].1.x <= 1);
    }

    #[test]
    fn test_clicks_on_hidden_toolbox_area_do_not_select_tools() {
        let mut app = font_app();
        app.editor.toolbox.selected = Tool::Brush;
        draw(&mut app);
        let revision = app.editor.revision;
        // Where the toolbox list would be: rows below the tab strip, the
        // first columns of the main area.
        for row in 5..30 {
            for col in 0..20 {
                app.handle_mouse_event(MouseEvent {
                    kind: MouseEventKind::Down(MouseButton::Left),
                    column: col,
                    row,
                    modifiers: KeyModifiers::NONE,
                });
                if app.editor.font_editor.view != FontEditorView::Overview {
                    app.editor.font_editor.view = FontEditorView::Overview;
                }
            }
        }
        assert_eq!(app.editor.toolbox.selected, Tool::Brush);
        assert_eq!(app.editor.revision, revision, "canvas painted");
    }

    /// Paint `ch` at (x, y) of the active layer, as a canvas tool does.
    fn paint_cell(app: &mut TuiApp, x: usize, y: usize, ch: char) {
        let mut buf = app.editor.layer_stack.active_layer().buffer.clone();
        buf.set(
            x,
            y,
            crate::tui::canvas::CanvasCell {
                ch,
                fg: None,
                bg: None,
                height: None,
            },
        );
        *app.editor.layer_stack.active_layer_mut().buffer_mut() = buf;
        app.editor.recomposite_canvas();
    }

    fn shift_tab(app: &mut TuiApp) {
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::SHIFT));
    }

    /// Open glyph 65 with Enter and draw, so its rows are in the canvas.
    fn open_glyph_a(app: &mut TuiApp) {
        draw(app);
        key(app, KeyCode::Enter);
        assert_eq!(app.editor.font_editor.view, FontEditorView::CharEditor(65));
        draw(app);
        assert_eq!(canvas_rows(app), vec!["AAA  "; 3]);
    }

    #[test]
    fn test_mode_switch_does_not_write_other_canvas_into_glyph() {
        let mut app = font_app();
        open_glyph_a(&mut app);
        // Tab to the image editor and paint there.
        key(&mut app, KeyCode::Tab);
        assert_eq!(app.ui.mode, AppMode::ImageEditor);
        draw(&mut app);
        paint_cell(&mut app, 0, 0, '@');
        draw(&mut app);
        assert_eq!(glyph_rows(&app, 65), vec!["AAA  "; 3], "image paint leaked");
        // Back to the font editor: the glyph is reloaded, not overwritten.
        app.ui.mode = AppMode::FontEditor;
        draw(&mut app);
        assert_eq!(glyph_rows(&app, 65), vec!["AAA  "; 3], "image paint saved");
        assert_eq!(canvas_rows(&app), vec!["AAA  "; 3], "glyph reloaded");
    }

    #[test]
    fn test_mode_cycle_and_back_does_not_write_other_canvas_into_glyph() {
        let mut app = font_app();
        open_glyph_a(&mut app);
        // Tab, Tab, Tab: image, ASCII preview, back to font, painting the
        // shared canvas in each other mode without a render in between.
        key(&mut app, KeyCode::Tab);
        paint_cell(&mut app, 1, 0, '@');
        key(&mut app, KeyCode::Tab);
        assert_eq!(app.ui.mode, AppMode::AsciiPreview);
        paint_cell(&mut app, 2, 1, '%');
        key(&mut app, KeyCode::Tab);
        assert_eq!(app.ui.mode, AppMode::FontEditor);
        draw(&mut app);
        assert_eq!(glyph_rows(&app, 65), vec!["AAA  "; 3]);
        assert_eq!(canvas_rows(&app), vec!["AAA  "; 3]);
        // Shift+Tab out and back the other way.
        shift_tab(&mut app);
        paint_cell(&mut app, 0, 2, '!');
        draw(&mut app);
        app.ui.mode = AppMode::FontEditor;
        draw(&mut app);
        assert_eq!(glyph_rows(&app, 65), vec!["AAA  "; 3]);
    }

    #[test]
    fn test_lighting_mode_does_not_write_canvas_into_glyph() {
        let mut app = font_app();
        open_glyph_a(&mut app);
        app.ui.mode = AppMode::Lighting;
        draw(&mut app);
        paint_cell(&mut app, 0, 1, '*');
        draw(&mut app);
        app.ui.mode = AppMode::FontEditor;
        draw(&mut app);
        assert_eq!(glyph_rows(&app, 65), vec!["AAA  "; 3]);
    }

    #[test]
    fn test_char_editor_edits_reach_glyph_before_mode_switch() {
        let mut app = font_app();
        open_glyph_a(&mut app);
        // Paint and switch mode in the same frame (no render between).
        paint_cell(&mut app, 4, 0, '#');
        key(&mut app, KeyCode::Tab);
        assert_eq!(glyph_rows(&app, 65)[0], "AAA #", "char-editor edit lost");
        draw(&mut app);
        app.ui.mode = AppMode::FontEditor;
        draw(&mut app);
        assert_eq!(glyph_rows(&app, 65)[0], "AAA #");
        assert_eq!(canvas_rows(&app)[0], "AAA #");
    }

    #[test]
    fn test_timeline_nav_in_other_mode_leaves_glyph_and_canvas_alone() {
        let mut app = font_app();
        open_glyph_a(&mut app);
        key(&mut app, KeyCode::Tab);
        assert_eq!(app.ui.mode, AppMode::ImageEditor);
        draw(&mut app);
        // Two timeline frames with image art, then step between them.
        paint_cell(&mut app, 0, 0, '1');
        key(&mut app, KeyCode::Char('A'));
        paint_cell(&mut app, 0, 0, '2');
        key(&mut app, KeyCode::Char('A'));
        assert_eq!(app.animation.timeline_state.frames.len(), 2);
        app.animation.timeline_visible = true;
        for code in [KeyCode::Left, KeyCode::Right, KeyCode::Left] {
            key(&mut app, code);
            let row0 = canvas_rows(&app)[0].clone();
            assert!(row0.starts_with(['1', '2']), "glyph loaded: {row0}");
            assert_eq!(glyph_rows(&app, 65), vec!["AAA  "; 3], "frame leaked");
        }
        draw(&mut app);
        assert_eq!(glyph_rows(&app, 65), vec!["AAA  "; 3]);
    }

    #[test]
    fn test_esc_leave_saves_char_editor_edits() {
        let mut app = font_app();
        open_glyph_a(&mut app);
        paint_cell(&mut app, 3, 1, '#');
        draw(&mut app);
        paint_cell(&mut app, 4, 2, '+');
        key(&mut app, KeyCode::Esc);
        assert_eq!(app.editor.font_editor.view, FontEditorView::Overview);
        assert_eq!(glyph_rows(&app, 65), vec!["AAA  ", "AAA# ", "AAA +"]);
        // Enter reopens the same glyph with the saved edits.
        draw(&mut app);
        key(&mut app, KeyCode::Enter);
        draw(&mut app);
        assert_eq!(canvas_rows(&app), vec!["AAA  ", "AAA# ", "AAA +"]);
    }
}
