use crossterm::event::{KeyCode, KeyModifiers};
use rand::rngs::StdRng;
use rand::Rng;
use rand::SeedableRng;
use ratatui::layout::Rect;
use ratatui::widgets::{Block, Borders};
use ratatui::Frame;
use std::collections::{BTreeMap, HashMap};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use super::{
    brush, canvas, capture_thumbnail, dialogs, export, file_ops, font_editor, fx, image_editor,
    layers, layout, light_panel, lighting, palette, palette_editor, particles, player, status,
    theme, timeline, toolbox, tools, undo, undo_panel, welcome, LightPanel, MenuBar, MenuBarState,
    PropsPanel, RenderMode, SidePanel, ThrobberState, Tool,
};
use crate::config;

const ICONS_YAML: &str = include_str!("../../assets/icons.yaml");

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

    pub(crate) fn sync_canvas_to_font_char(&mut self) {
        if let font_editor::FontEditorView::CharEditor(code) = self.font_editor.view {
            self.font_editor.sync_from_canvas(code, &self.canvas.buffer);
        }
    }

    pub(crate) fn sync_font_char_to_canvas(&mut self) {
        if let Some((_, ch)) = self.font_editor.selected_char() {
            let w = ch.width().max(1);
            let h = ch.rows().len().max(1);
            if self.canvas.buffer.width() != w || self.canvas.buffer.height() != h {
                self.canvas = canvas::CanvasWidget::new(w as u16, h as u16);
                self.layer_stack.resize_all(w, h);
            }
            let mut buf = self.layer_stack.active_layer().buffer.clone();
            for y in 0..h {
                let row = &ch.rows()[y];
                for (x, c) in row.chars().enumerate() {
                    if x < w {
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
            }
            *self.layer_stack.active_layer_mut().buffer_mut() = buf;
            self.recomposite_canvas();
        }
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
                let mut rng = StdRng::seed_from_u64(rand::thread_rng().gen());
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
                    lighting::Light::Ambient { intensity, color } => {
                        props.intensity = Some(*intensity);
                        props.color = Some(*color);
                    }
                    lighting::Light::Directional {
                        direction,
                        intensity,
                        color,
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
                    lighting::Light::Ambient { intensity, color } => {
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
                    });
                    self.panel.selected_index = scene.lights.len() - 1;
                    *dirty = true;
                }
            }
            KeyCode::Char('P') => {
                if let Some(ref mut scene) = self.scene {
                    scene.add_light(lighting::Light::Point {
                        position: (w as f32 / 2.0, h as f32 / 2.0, 5.0),
                        intensity: 0.8,
                        color: lighting::Rgb(255, 255, 255),
                        attenuation: lighting::Attenuation::default(),
                    });
                    self.panel.selected_index = scene.lights.len() - 1;
                    *dirty = true;
                }
            }
            KeyCode::Delete => {
                if let Some(ref mut scene) = self.scene {
                    let idx = self.panel.selected_index;
                    if idx < scene.lights.len() {
                        scene.remove_light(idx);
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
            frame.has_keyframe = true;
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
                    editor.sync_canvas_to_font_char();
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
                    editor.sync_canvas_to_font_char();
                    *dirty = true;
                    return true;
                }
                KeyCode::Char('A') => {
                    let buffer = editor.layer_stack.composite();
                    let thumbnail = capture_thumbnail(&buffer, 8, 3);
                    let layer_keyframes = editor
                        .layer_stack
                        .layers
                        .iter()
                        .map(|_| Some(timeline::LayerKeyframe::default()))
                        .collect();
                    let frame = timeline::TimelineFrame {
                        thumbnail,
                        has_keyframe: true,
                        label: format!("F{}", self.timeline_state.frames.len()),
                        delay: self.timeline_state.default_delay(),
                        document_state: vec![buffer],
                        layer_keyframes,
                    };
                    self.timeline_state.sync_layer_names(&editor.layer_stack);
                    self.timeline_state.add_frame(frame);
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
    OpenFont,
    NewFileSession,
    FontNewBlankSession,
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
    pub quit_confirm_dialog: bool,
    pub quit_confirm_buttons: [Rect; 3],
    pub quit_after_save: bool,
    /// Document revision captured when the current async save started.
    /// `None` when no save is in flight. On `SaveComplete`, unsaved state
    /// is only cleared (and a deferred quit only honored) if the document
    /// hasn't been mutated since — edits made during the save keep the
    /// document dirty (GPT review F-07).
    pub pending_save_revision: Option<u64>,
    /// The transition awaiting confirmation in the unsaved-changes dialog.
    pub pending_transition: Option<PendingDocAction>,
    /// Resolved path for an open that is waiting behind a confirmation.
    pub pending_open_path: Option<std::path::PathBuf>,
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
        let icons: BTreeMap<String, String> = serde_yaml::from_str(ICONS_YAML).unwrap_or_default();
        let config = config::load_config();
        let theme = theme::load_theme(&config.tui.theme);

        let mut brush = brush::BrushState::new();
        if let Some(ref shape) = config.tui.brush.shape {
            brush.shape = match shape.as_str() {
                "square" => brush::BrushShape::Square,
                "circle" => brush::BrushShape::Circle,
                "spray" => brush::BrushShape::SprayPaint,
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
                    text_tool: tools::text::TextToolState::new("fonts"),
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
                quit_confirm_dialog: false,
                quit_confirm_buttons: [Rect::default(); 3],
                quit_after_save: false,
                pending_save_revision: None,
                pending_transition: None,
                pending_open_path: None,
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
                emitter_panel: particles::EmitterConfigPanel::new(),
                show_live_particles: true,
                baked_layer_indices: Vec::new(),
                timeline_visible: false,
                marker_accum: HashMap::new(),
                loop_enabled: false,
                inline_player: None,
                transport_rects: Vec::new(),
            },
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
    pub(crate) fn default_side_panel_open(&self, term_width: u16) -> bool {
        const MIN_CANVAS_WIDTH: u16 = 60;
        let toolbox_width = self
            .editor
            .toolbox
            .required_width(self.editor.brush.required_outer_width());
        term_width >= toolbox_width + layout::DRAWER_WIDTH + MIN_CANVAS_WIDTH
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
        }
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
