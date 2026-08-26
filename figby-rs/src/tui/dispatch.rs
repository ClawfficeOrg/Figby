use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use rand::rngs::StdRng;
use rand::Rng;
use rand::SeedableRng;
use ratatui::layout::Rect;
use std::sync::mpsc;

use super::dialogs;
use super::*;
use super::{capture_thumbnail, rotate_drag_steps};

impl TuiApp {
    fn dispatch_welcome_action(&mut self, action: welcome::WelcomeAction) {
        use welcome::WelcomeAction;
        match action {
            WelcomeAction::Dismiss => {
                self.welcome.screen.show = false;
                self.welcome.fx = None;
                self.frame.dirty = true;
            }
            WelcomeAction::OpenRecent(idx) => {
                if let Some(path) = self.dialogs.recent_files.get(idx) {
                    self.dialogs.file_ops.path_buffer = path.to_string_lossy().to_string();
                    self.perform_open();
                    self.welcome.screen.show = false;
                    self.welcome.fx = None;
                    self.frame.dirty = true;
                }
            }
            WelcomeAction::Open => {
                self.start_open();
                self.welcome.screen.show = false;
                self.welcome.fx = None;
                self.frame.dirty = true;
            }
            WelcomeAction::NewFile => {
                self.request_transition(PendingDocAction::NewFileSession);
                self.welcome.screen.show = false;
                self.welcome.fx = None;
                self.frame.dirty = true;
            }
            WelcomeAction::ToggleHelp => {
                self.ui.show_keybindings = !self.ui.show_keybindings;
                self.frame.dirty = true;
            }
            WelcomeAction::OpenSettings => {
                self.dialogs.settings.canvas_width = self.editor.canvas.buffer.width() as u16;
                self.dialogs.settings.canvas_height = self.editor.canvas.buffer.height() as u16;
                self.dialogs.settings.show_grid = self.editor.canvas.show_grid();
                self.dialogs.settings.settings_open = true;
                self.welcome.screen.show = false;
                self.welcome.fx = None;
                self.frame.dirty = true;
            }
            WelcomeAction::ScrollUp => {
                self.welcome.screen.scroll_up();
                self.frame.dirty = true;
            }
            WelcomeAction::ScrollDown => {
                let count = self.dialogs.recent_files.len();
                self.welcome.screen.scroll_down(count);
                self.frame.dirty = true;
            }
            WelcomeAction::FontOpen => {
                self.ui.session_type = SessionType::Font;
                self.start_open();
                self.welcome.screen.show = false;
                self.welcome.fx = None;
                self.frame.dirty = true;
            }
            WelcomeAction::ImageOpenFigmap => {
                self.ui.session_type = SessionType::Image;
                self.editor.image_editor = image_editor::ImageEditor::new();
                self.ui.mode = AppMode::ImageEditor;
                self.start_open();
                self.welcome.screen.show = false;
                self.welcome.fx = None;
                self.frame.dirty = true;
            }
            WelcomeAction::FontNewFromFile => {
                self.start_font_import_from_file();
                self.welcome.screen.show = false;
                self.welcome.fx = None;
                self.frame.dirty = true;
            }
            WelcomeAction::FontNewBlank => {
                self.ui.session_type = SessionType::Font;
                self.request_transition(PendingDocAction::FontNewBlankSession);
                self.welcome.screen.show = false;
                self.welcome.fx = None;
                self.frame.dirty = true;
            }
            WelcomeAction::FontNewFromSystem => {
                self.ui.session_type = SessionType::Font;
                self.dialogs.system_font.enter();
                self.welcome.screen.show = false;
                self.welcome.fx = None;
                self.frame.dirty = true;
            }
            WelcomeAction::FontDuplicate => {
                self.ui.session_type = SessionType::Font;
                // TODO: duplicate-from-existing-font flow
                self.welcome.screen.show = false;
                self.welcome.fx = None;
                self.frame.dirty = true;
            }
            WelcomeAction::ImageNewBlank => {
                self.dialogs.new_image.enter_new_image();
                self.welcome.screen.show = false;
                self.welcome.fx = None;
                self.frame.dirty = true;
            }
            WelcomeAction::ImageNewFromTemplate => {
                self.ui.session_type = SessionType::Image;
                // TODO: template picker (5.0.4)
                self.ui.mode = AppMode::ImageEditor;
                self.welcome.screen.show = false;
                self.welcome.fx = None;
                self.frame.dirty = true;
            }
            WelcomeAction::ImageConvert => {
                self.ui.session_type = SessionType::Image;
                self.dialogs.rascii_import.enter_import();
                self.welcome.screen.show = false;
                self.welcome.fx = None;
                self.frame.dirty = true;
            }
            WelcomeAction::ImageImportGif => {
                self.ui.session_type = SessionType::Image;
                self.dialogs.gif_import.enter();
                self.welcome.screen.show = false;
                self.welcome.fx = None;
                self.frame.dirty = true;
            }
            WelcomeAction::ImageOpen => {
                self.ui.session_type = SessionType::Image;
                self.editor.image_editor = image_editor::ImageEditor::new();
                self.ui.mode = AppMode::ImageEditor;
                self.dialogs.file_ops.enter_open_image();
                self.welcome.screen.show = false;
                self.welcome.fx = None;
                self.frame.dirty = true;
            }
        }
    }

    fn dispatch_props_action(&mut self, action: PropAction) {
        match action {
            PropAction::SizeUp => self.editor.brush.size_up(),
            PropAction::SizeDown => self.editor.brush.size_down(),
            PropAction::DensityUp => self.editor.brush.density_up(),
            PropAction::DensityDown => self.editor.brush.density_down(),
            PropAction::CycleShape => self.editor.brush.cycle_shape(),
            PropAction::CycleSubMode => {
                self.editor
                    .brush
                    .cycle_sub_mode(self.editor.palette.has_multi_select());
                if self.editor.brush.sub_mode == brush::BrushSubMode::Normal {
                    self.animation.marker_accum.clear();
                }
            }
            PropAction::CycleJust => {
                use crate::render::Justification;
                self.editor.text_tool.justification = match self.editor.text_tool.justification {
                    Justification::Left => Justification::Center,
                    Justification::Center => Justification::Right,
                    Justification::Right => Justification::Left,
                };
            }
            PropAction::ScaleUp => {
                if self.editor.text_tool.scale < 10 {
                    self.editor.text_tool.scale += 1;
                }
            }
            PropAction::ScaleDown => {
                if self.editor.text_tool.scale > 1 {
                    self.editor.text_tool.scale -= 1;
                }
            }
            PropAction::FontNext => {
                let count = self.editor.text_tool.available_fonts.len();
                if count > 0 {
                    self.editor.text_tool.font_index =
                        (self.editor.text_tool.font_index + 1) % count;
                    self.editor.text_tool.load_selected_font();
                }
            }
            PropAction::FontPrev => {
                let count = self.editor.text_tool.available_fonts.len();
                if count > 0 {
                    self.editor.text_tool.font_index =
                        (self.editor.text_tool.font_index + count - 1) % count;
                    self.editor.text_tool.load_selected_font();
                }
            }
            PropAction::BeginEditChar => {
                self.props_panel.start_char_edit();
            }
            PropAction::BeginEditField => {
                self.props_panel.start_char_edit();
            }
            PropAction::CommitChar(ch) => {
                self.editor.brush.ch = ch;
            }
            PropAction::CancelEdit => {}
            PropAction::FillThresholdUp => {
                self.editor.fill_threshold = self.editor.fill_threshold.saturating_add(5);
            }
            PropAction::FillThresholdDown => {
                self.editor.fill_threshold = self.editor.fill_threshold.saturating_sub(5);
            }
            PropAction::MoveStrideUp => {
                self.editor.move_state.stride = self.editor.move_state.stride.saturating_add(1);
            }
            PropAction::MoveStrideDown => {
                self.editor.move_state.stride = self.editor.move_state.stride.saturating_sub(1);
            }
            PropAction::MoveSnapToggle => {
                self.editor.move_state.snap = !self.editor.move_state.snap;
            }
            PropAction::MoveWrapToggle => {
                self.editor.move_state.wrap = !self.editor.move_state.wrap;
            }
            PropAction::RotateStepUp => {
                self.editor.rotate_state.step_angle =
                    self.editor.rotate_state.step_angle.saturating_add(15);
            }
            PropAction::RotateStepDown => {
                self.editor.rotate_state.step_angle =
                    self.editor.rotate_state.step_angle.saturating_sub(15);
            }
            PropAction::RotateDirToggle => {
                self.editor.rotate_state.direction.toggle();
            }
            PropAction::RotatePivotCycle => {
                self.editor.rotate_state.pivot.cycle();
            }
            PropAction::SelectFeatherUp => {
                self.editor.selection_state.feather =
                    self.editor.selection_state.feather.saturating_add(1);
            }
            PropAction::SelectFeatherDown => {
                self.editor.selection_state.feather =
                    self.editor.selection_state.feather.saturating_sub(1);
            }
            PropAction::SelectAdditiveToggle => {
                self.editor.selection_state.additive = !self.editor.selection_state.additive;
                if self.editor.selection_state.additive {
                    self.editor.selection_state.subtractive = false;
                }
            }
            PropAction::SelectSubtractiveToggle => {
                self.editor.selection_state.subtractive = !self.editor.selection_state.subtractive;
                if self.editor.selection_state.subtractive {
                    self.editor.selection_state.additive = false;
                }
            }
            PropAction::SelectMoveToggle => {
                self.editor.selection_state.move_with_arrows =
                    !self.editor.selection_state.move_with_arrows;
            }
            PropAction::LineWidthUp => {
                self.editor.line_state.width = self.editor.line_state.width.saturating_add(1);
            }
            PropAction::LineWidthDown => {
                self.editor.line_state.width = self.editor.line_state.width.saturating_sub(1);
            }
            PropAction::LineArrowCycle => {
                self.editor.line_state.arrowhead.cycle();
            }
            PropAction::LineCurveToggle => {
                self.editor.line_state.curve.toggle();
            }
            PropAction::RasterizeBlock => {
                let block_idx = match self.editor.text_tool.selected_block {
                    Some(i) if i < self.editor.text_tool.blocks.len() => i,
                    _ => return,
                };
                let (bx, by, bw, bh) = self.editor.text_tool.compute_bounding_box(block_idx);
                let aidx = self.editor.layer_stack.active;
                let need_w = bx as usize + bw;
                let need_h = by as usize + bh;
                let cur_w = self.editor.layer_stack.layers[aidx].buffer.width();
                let cur_h = self.editor.layer_stack.layers[aidx].buffer.height();
                if need_w > cur_w || need_h > cur_h {
                    let new_w = need_w.max(cur_w);
                    let new_h = need_h.max(cur_h);
                    let mut new_buf = crate::tui::canvas::CanvasBuffer::new(new_w, new_h);
                    for y in 0..cur_h {
                        for x in 0..cur_w {
                            if let Some(cell) =
                                self.editor.layer_stack.layers[aidx].buffer.get(x, y)
                            {
                                new_buf.set(x, y, *cell);
                            }
                        }
                    }
                    self.editor.layer_stack.layers[aidx].buffer = new_buf;
                    self.editor.layer_stack.resize_all(new_w, new_h);
                }
                self.editor
                    .text_tool
                    .rasterize_selected_block(&mut self.editor.layer_stack.layers[aidx].buffer);
                self.editor.recomposite_canvas();
                self.editor.mark_dirty();
            }
        }
        self.frame.dirty = true;
    }

    /// Undo the most recent action, restoring into the RECORDED layer.
    /// The redo entry captures that layer's current buffer so redo is
    /// exact even across layer switches (GPT review F-09).
    pub(crate) fn perform_undo(&mut self) {
        if !self.editor.undo.can_undo() {
            return;
        }
        let target = match self.editor.undo.undo_top_layer() {
            Some(i) => i,
            None => return,
        };
        if target >= self.editor.layer_stack.layers.len() {
            // Layer was deleted after the snapshot; drop the stale entry.
            let _ = self.editor.undo.undo(canvas::CanvasBuffer::new(0, 0), 0);
            return;
        }
        let cur = self.editor.layer_stack.layers[target].buffer.clone();
        if let Some((buf, idx, _)) = self.editor.undo.undo(cur, target) {
            if self.editor.restore_undo_snapshot(idx, buf) {
                self.editor.mark_dirty();
            }
        }
    }

    /// Redo counterpart of [`Self::perform_undo`].
    pub(crate) fn perform_redo(&mut self) {
        if !self.editor.undo.can_redo() {
            return;
        }
        let target = match self.editor.undo.redo_top_layer() {
            Some(i) => i,
            None => return,
        };
        if target >= self.editor.layer_stack.layers.len() {
            let _ = self.editor.undo.redo(canvas::CanvasBuffer::new(0, 0), 0);
            return;
        }
        let cur = self.editor.layer_stack.layers[target].buffer.clone();
        if let Some((buf, idx, _)) = self.editor.undo.redo(cur, target) {
            if self.editor.restore_undo_snapshot(idx, buf) {
                self.editor.mark_dirty();
            }
        }
    }

    /// Route a destructive transition through the unsaved-changes dialog
    /// when needed (GPT review F-08). Clean documents proceed straight
    /// through.
    pub fn request_transition(&mut self, action: PendingDocAction) {
        if !self.editor.unsaved {
            self.execute_action(action);
            return;
        }
        self.dialogs.pending_transition = Some(action);
        self.dialogs.quit_confirm_dialog = true;
        self.frame.dirty = true;
    }

    /// Immediately perform a transition (no confirmation). For OpenFont
    /// this means the file picker, or the stashed path if one was already
    /// resolved.
    pub(crate) fn execute_action(&mut self, action: PendingDocAction) {
        match action {
            PendingDocAction::Quit => self.ui.should_quit = true,
            PendingDocAction::OpenFont => {
                let path = self.dialogs.pending_open_path.take();
                match path {
                    Some(path) => self.perform_open_at(path),
                    None => self.start_open(),
                }
            }
            PendingDocAction::NewFileSession => self.do_new_file_session(self.ui.session_type),
            PendingDocAction::FontNewBlankSession => self.do_new_file_session(SessionType::Font),
        }
    }

    /// Fresh blank document for the given session type (welcome "New").
    pub(crate) fn do_new_file_session(&mut self, session_type: SessionType) {
        self.ui.session_type = session_type;
        self.reset_document(32, 16);
        self.welcome.screen.show = false;
        self.welcome.fx = None;
        self.frame.dirty = true;
    }

    pub(crate) fn handle_mouse_event(&mut self, mouse: MouseEvent) {
        // Unsaved-changes dialog: intercept all mouse events
        if self.dialogs.quit_confirm_dialog {
            if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
                let btns = self.dialogs.quit_confirm_buttons;
                let can_save = self.can_save_now();
                if btns[0].contains((mouse.column, mouse.row).into()) && can_save {
                    self.dialogs.quit_confirm_dialog = false;
                    let is_quit = self.dialogs.pending_transition == Some(PendingDocAction::Quit);
                    if is_quit {
                        self.dialogs.quit_after_save = true;
                    }
                    self.frame.dirty = true;
                    self.start_save();
                } else if btns[1].contains((mouse.column, mouse.row).into())
                    || (btns[0].contains((mouse.column, mouse.row).into()) && !can_save)
                {
                    // Discard & proceed (also when Save is unavailable and
                    // the user clicks where the save button used to be).
                    self.dialogs.quit_confirm_dialog = false;
                    self.frame.dirty = true;
                    self.execute_pending_transition();
                } else if btns[2].contains((mouse.column, mouse.row).into()) {
                    self.dialogs.quit_confirm_dialog = false;
                    self.dialogs.pending_transition = None;
                    self.dialogs.pending_open_path = None;
                    self.frame.dirty = true;
                }
            }
            return;
        }

        // Menu bar mouse event
        if self.ui.menu_bar.handle_mouse_event(
            mouse.column,
            mouse.row,
            mouse.kind,
            &mut self.ui.menu_bar_state,
        ) {
            if let Some(action) = self.ui.menu_bar_state.drain_actions() {
                self.process_event(&AppEvent::Menu(action));
            }
            return;
        }

        // Welcome screen captures all mouse events while visible
        if self.welcome.screen.show {
            let recent_count = self.dialogs.recent_files.len();
            let (action, hover_dirty) =
                self.welcome
                    .screen
                    .handle_mouse(mouse.column, mouse.row, mouse.kind, recent_count);
            if hover_dirty {
                self.frame.dirty = true;
            }
            if let Some(action) = action {
                self.dispatch_welcome_action(action);
            }
            return;
        }

        if self.dialogs.settings.settings_open {
            return;
        }

        if self.dialogs.file_ops.mode != file_ops::FileOpsMode::Idle {
            let prev_mode = self.dialogs.file_ops.mode;
            if self
                .dialogs
                .file_ops
                .handle_mouse(mouse.column, mouse.row, mouse.kind)
            {
                self.frame.dirty = true;
                if self.dialogs.file_ops.mode == file_ops::FileOpsMode::Idle {
                    if let Some(event) = self.complete_file_ops_dialog(prev_mode) {
                        self.process_event(&event);
                    }
                }
            }
            return;
        }

        if self.dialogs.export_dialog.active {
            return;
        }

        if self.dialogs.rascii_import.active {
            return;
        }

        // Font editor overview: glyph grid mouse click + scroll
        if self.ui.mode == AppMode::FontEditor
            && self.editor.font_editor.view == font_editor::FontEditorView::Overview
        {
            match mouse.kind {
                MouseEventKind::Down(MouseButton::Left)
                    if self
                        .editor
                        .font_editor
                        .handle_mouse_click_overview(mouse.column, mouse.row) =>
                {
                    self.frame.dirty = true;
                    return;
                }
                MouseEventKind::ScrollDown => {
                    self.editor.font_editor.handle_mouse_scroll_overview(1);
                    self.frame.dirty = true;
                    return;
                }
                MouseEventKind::ScrollUp => {
                    self.editor.font_editor.handle_mouse_scroll_overview(-1);
                    self.frame.dirty = true;
                    return;
                }
                _ => {}
            }
        }

        // Image editor: handle state-dependent mouse events
        if self.ui.mode == AppMode::ImageEditor {
            if self.editor.image_editor.entering_path() {
                // Swallow all mouse events while user is typing a file path
                self.frame.dirty = true;
                return;
            }
            if self.editor.image_editor.error_message().is_some() {
                if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
                    self.editor.image_editor.clear_error();
                    self.frame.dirty = true;
                    return;
                }
            }
            // adjustment_mode and other states: fall through to general handlers
        }

        // Toolbox click: select tool by row
        let mouse_fl = {
            let (cols, rows) = crossterm::terminal::size().unwrap_or((80, 24));
            let tw = self
                .editor
                .toolbox
                .required_width(self.editor.brush.required_outer_width());
            let toolbox_h = Tool::all().len() as u16 + 2;
            layout::FrameLayout::compute(
                Rect::new(0, 0, cols, rows),
                self.ui.zen_mode,
                self.side_panel.open,
                tw,
                toolbox_h,
                self.animation.timeline_visible,
            )
        };
        let canvas_inner_rect = self.editor.compute_canvas_rect(
            ratatui::widgets::Block::default()
                .borders(mouse_fl.canvas_borders())
                .inner(mouse_fl.canvas),
        );

        // Side panel tab label click
        if let Some(rp) = mouse_fl.right_panel {
            if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
                if let Some(tab) = self.side_panel.tab_at_pos(mouse.column, mouse.row, rp) {
                    self.side_panel.set_active_tab(tab);
                    // Activate text editing when opening Text tab
                    if tab == TabId::Text && self.editor.toolbox.selected == Tool::Text {
                        self.editor.text_tool.editing = true;
                    }
                    self.frame.dirty = true;
                    return;
                }
            }
        }

        // Layer panel: click/drag on layer rows
        if let Some(rp) = mouse_fl.right_panel {
            if self.side_panel.open
                && self.side_panel.active_tab == TabId::Layers
                && self.editor.layer_panel.handle_mouse(
                    mouse.column,
                    mouse.row,
                    mouse.kind,
                    self.side_panel.content_area(rp),
                    &mut self.editor.layer_stack,
                )
            {
                self.editor.recomposite_canvas();
                self.editor.mark_dirty();
                self.frame.dirty = true;
                return;
            }
        }

        // Props tab: click on widget rects
        if self.side_panel.open && mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            if self.side_panel.active_tab == TabId::Props {
                if let Some(action) = self.props_panel.handle_click(mouse.column, mouse.row) {
                    self.dispatch_props_action(action);
                    self.frame.dirty = true;
                    return;
                }
            }
            // Text tab: click on widget rects
            if self.side_panel.active_tab == TabId::Text {
                if let Some(action) = self.props_panel.handle_click(mouse.column, mouse.row) {
                    self.dispatch_props_action(action);
                    self.frame.dirty = true;
                    return;
                }
            }
        }

        // Timeline: click-to-seek and mouse-wheel scroll
        if let Some(timeline_rect) = mouse_fl.timeline {
            if self.animation.timeline_visible
                && mouse.column >= timeline_rect.x
                && mouse.column < timeline_rect.x + timeline_rect.width
                && mouse.row >= timeline_rect.y
                && mouse.row < timeline_rect.y + timeline_rect.height
            {
                let block_inner = ratatui::widgets::Block::default()
                    .borders(mouse_fl.timeline_borders())
                    .inner(timeline_rect);
                if block_inner.height >= 5 {
                    let (grid_area, _toolbar_area) =
                        timeline::AnimationTimeline::split_area(block_inner);
                    let anim_timeline = timeline::AnimationTimeline::panel_instance();
                    match mouse.kind {
                        MouseEventKind::Down(MouseButton::Left) => {
                            if let Some(&(action, _)) =
                                self.animation.transport_rects.iter().find(|(_, r)| {
                                    mouse.column >= r.x
                                        && mouse.column < r.x + r.width
                                        && mouse.row == r.y
                                })
                            {
                                self.handle_transport_button(action);
                                return;
                            }
                            if let Some(idx) = anim_timeline.frame_at_col(
                                mouse.column,
                                grid_area,
                                &self.animation.timeline_state,
                            ) {
                                self.animation.commit_current_timeline_frame(&self.editor);
                                self.animation.timeline_state.current_frame = idx;
                                self.animation.load_current_timeline_frame(&mut self.editor);
                                self.editor.sync_canvas_to_font_char();
                                self.frame.dirty = true;
                                return;
                            }
                        }
                        MouseEventKind::ScrollUp => {
                            if mouse.modifiers.contains(KeyModifiers::SHIFT) {
                                self.animation.timeline_state.layer_row_offset = self
                                    .animation
                                    .timeline_state
                                    .layer_row_offset
                                    .saturating_sub(1);
                            } else {
                                self.animation.timeline_state.scroll_offset = self
                                    .animation
                                    .timeline_state
                                    .scroll_offset
                                    .saturating_sub(1);
                            }
                            self.frame.dirty = true;
                            return;
                        }
                        MouseEventKind::ScrollDown => {
                            if mouse.modifiers.contains(KeyModifiers::SHIFT) {
                                let max_row = self
                                    .animation
                                    .timeline_state
                                    .layer_names
                                    .len()
                                    .saturating_sub(1);
                                self.animation.timeline_state.layer_row_offset =
                                    (self.animation.timeline_state.layer_row_offset + 1)
                                        .min(max_row);
                            } else {
                                let max_scroll =
                                    self.animation.timeline_state.frames.len().saturating_sub(
                                        self.animation.timeline_state.cached_max_vis_frames.max(1),
                                    );
                                self.animation.timeline_state.scroll_offset =
                                    (self.animation.timeline_state.scroll_offset + 1)
                                        .min(max_scroll);
                            }
                            self.frame.dirty = true;
                            return;
                        }
                        _ => {}
                    }
                }
            }
        }

        // While in-canvas animation playback is active, only the
        // timeline/transport bar (handled above) responds to clicks — swallow
        // everything else so it doesn't fall through to canvas draw-tool
        // logic underneath the player (previously a latent bug: clicks
        // during playback painted on the canvas buffer instead of being
        // ignored).
        if self.animation.inline_player.is_some() {
            return;
        }

        if let Some(tb) = mouse_fl.toolbox_list {
            let tool_count = Tool::all().len() as u16;
            let toolbox_inner_y = tb.y + 1;
            if mouse.kind == MouseEventKind::Down(MouseButton::Left)
                && mouse.column >= tb.x
                && mouse.column < tb.x + tb.width
                && mouse.row >= toolbox_inner_y
                && mouse.row < toolbox_inner_y + tool_count
            {
                let idx = (mouse.row - toolbox_inner_y) as usize;
                let tools = Tool::all();
                if idx < tools.len() {
                    self.editor.toolbox.selected = tools[idx];
                    self.editor.selection_polygon_points.clear();
                }
                return;
            }
        }

        // Palette panel click (left column, below toolbox)
        if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            if let Some(palette_rect) = mouse_fl.palette {
                if !self.dialogs.settings.settings_open
                    && self
                        .editor
                        .palette
                        .handle_click(mouse.column, mouse.row, palette_rect)
                {
                    use crate::tui::events::PaletteEvent;
                    let color = self.editor.palette.selected_color;
                    let target = self.editor.palette.target;
                    if let Some(c) = color {
                        self.process_event(&AppEvent::Palette(PaletteEvent::ColorChanged(
                            c, target,
                        )));
                    }
                    self.frame.dirty = true;
                    return;
                }
            }
        }

        // Palette hover (mouse move over swatches)
        if mouse.kind == MouseEventKind::Moved {
            if let Some(palette_rect) = mouse_fl.palette {
                if !self.dialogs.settings.settings_open
                    && self
                        .editor
                        .palette
                        .handle_hover(mouse.column, mouse.row, palette_rect)
                {
                    self.frame.dirty = true;
                }
            }
        }

        // Text tool: hover shows preview, click places text or selects block
        if self.editor.toolbox.selected == Tool::Text {
            if let Some((bx, by)) =
                self.editor
                    .screen_to_buffer(mouse.column, mouse.row, canvas_inner_rect)
            {
                // Always update preview position on mouse move/hover
                self.editor.text_tool.preview_pos = (bx, by);
                self.editor.text_tool.show_preview = true;

                if let MouseEventKind::Down(_) = mouse.kind {
                    // Try to select existing block
                    if let Some(idx) = self.editor.text_tool.hit_test(bx, by) {
                        self.editor.text_tool.selected_block = Some(idx);
                    } else if !self.editor.text_tool.text_buffer.is_empty()
                        && !self.editor.active_layer_locked()
                    {
                        // Place text at click position
                        self.editor.text_tool.place_at(bx, by);
                        self.editor.push_undo_snapshot("Place text");
                        self.editor.mark_dirty();
                    }
                }
            } else {
                self.editor.text_tool.show_preview = false;
            }
            self.interaction.prev_mouse_buf = None;
            self.interaction.line_start = None;
            self.interaction.saved_buffer = None;
            return;
        }

        let is_selection_tool = matches!(
            self.editor.toolbox.selected,
            Tool::Marquee | Tool::Lasso | Tool::CircleSelect | Tool::PolygonSelect
        );

        if !is_selection_tool
            && !matches!(
                self.editor.toolbox.selected,
                Tool::Brush
                    | Tool::Eraser
                    | Tool::Line
                    | Tool::Fill
                    | Tool::Eyedropper
                    | Tool::Spray
                    | Tool::Emitter
                    | Tool::Move
                    | Tool::Rotate
            )
        {
            self.interaction.prev_mouse_buf = None;
            self.interaction.line_start = None;
            self.interaction.saved_buffer = None;
            self.interaction.move_origin = None;
            self.interaction.rotate_origin = None;
            self.interaction.saved_selection = None;
            return;
        }

        match mouse.kind {
            MouseEventKind::Down(_) => {
                let Some((bx, by)) =
                    self.editor
                        .screen_to_buffer(mouse.column, mouse.row, canvas_inner_rect)
                else {
                    self.interaction.prev_mouse_buf = None;
                    self.interaction.line_start = None;
                    return;
                };
                self.editor
                    .canvas
                    .set_cursor(bx.max(0) as u16, by.max(0) as u16);
                self.editor.mark_dirty();

                if is_selection_tool {
                    self.editor.handle_selection_down(
                        bx.max(0),
                        by.max(0),
                        &mut self.interaction.selection_drag_origin,
                        &mut self.interaction.selection_lasso_points,
                    );
                    return;
                }

                // Layer lock enforcement (GPT review F-09): mutating tools
                // are no-ops on a locked layer. Eyedropper still samples;
                // Emitter manages particles, not pixels.
                {
                    let read_only = matches!(
                        self.editor.toolbox.selected,
                        Tool::Eyedropper | Tool::Emitter
                    );
                    if self.editor.active_layer_locked() && !read_only {
                        return;
                    }
                }

                // Start batch for drag operations, push initial snapshot
                self.editor.undo.begin_batch();
                self.interaction.mouse_batch_active = true;
                if self.editor.toolbox.selected == Tool::Fill {
                    self.editor.push_undo_snapshot("Flood fill");
                    let mut cell = canvas::CanvasCell {
                        ch: self.editor.brush.ch,
                        fg: None,
                        bg: None,
                        height: None,
                    };
                    self.editor.palette.apply_to_cell(&mut cell);
                    let mut buf = self.editor.layer_stack.active_layer().buffer.clone();
                    tools::fill::flood_fill(&mut buf, bx, by, cell);
                    *self.editor.layer_stack.active_layer_mut().buffer_mut() = buf;
                    self.editor.recomposite_canvas();
                    return;
                }
                if self.editor.toolbox.selected == Tool::Line {
                    self.editor.push_undo_snapshot("Line tool");
                    self.interaction.line_start = Some((bx, by));
                    self.interaction.saved_buffer =
                        Some(self.editor.layer_stack.active_layer().buffer.clone());
                    return;
                }
                if self.editor.toolbox.selected == Tool::Move {
                    self.editor.push_undo_snapshot("Move");
                    self.interaction.move_origin = Some((bx, by));
                    self.interaction.saved_buffer =
                        Some(self.editor.layer_stack.active_layer().buffer.clone());
                    self.interaction.saved_selection = self.editor.selection.clone();
                    return;
                }
                if self.editor.toolbox.selected == Tool::Rotate {
                    self.editor.push_undo_snapshot("Rotate");
                    self.interaction.rotate_origin = Some((bx, by));
                    self.interaction.saved_buffer =
                        Some(self.editor.layer_stack.active_layer().buffer.clone());
                    self.interaction.saved_selection = self.editor.selection.clone();
                    return;
                }
                if self.editor.toolbox.selected == Tool::Eraser {
                    self.editor.push_undo_snapshot("Eraser");
                    let shape = self.editor.brush.shape;
                    let size = self.editor.brush.size;
                    let mut buf = self.editor.layer_stack.active_layer().buffer.clone();
                    tools::eraser::erase_stamp(&mut buf, bx, by, shape, size);
                    *self.editor.layer_stack.active_layer_mut().buffer_mut() = buf;
                    self.editor.recomposite_canvas();
                } else if self.editor.toolbox.selected == Tool::Eyedropper {
                    if let Some(cell) =
                        tools::eyedropper::sample(&self.editor.canvas.buffer, bx, by)
                    {
                        self.editor.brush.ch = cell.ch;
                        if let Some(fg) = cell.fg {
                            self.editor.palette.selected_color = Some(fg);
                            self.editor.palette.push_recent(fg);
                            self.editor.palette.target = palette::ColorTarget::Foreground;
                        }
                    }
                } else if self.editor.toolbox.selected == Tool::Spray {
                    self.editor.push_undo_snapshot("Spray");
                    let mut cell = canvas::CanvasCell {
                        ch: self.editor.brush.ch,
                        fg: None,
                        bg: None,
                        height: None,
                    };
                    self.editor.palette.apply_to_cell(&mut cell);
                    let mut rng = StdRng::seed_from_u64(rand::thread_rng().gen());
                    let size = self.editor.brush.size;
                    let density = self.editor.brush.density;
                    let mut buf = self.editor.layer_stack.active_layer().buffer.clone();
                    tools::spray::spray_stamp(&mut buf, bx, by, size, density, cell, &mut rng);
                    *self.editor.layer_stack.active_layer_mut().buffer_mut() = buf;
                    self.editor.recomposite_canvas();
                } else if self.editor.toolbox.selected == Tool::Emitter {
                    self.animation.emitter_active = true;
                    self.animation.particle_system.config.emitter_x = bx as f64;
                    self.animation.particle_system.config.emitter_y = by as f64;
                    self.animation.particle_system = particles::ParticleSystem::new(
                        self.animation.particle_system.config.clone(),
                    );
                    self.animation.emitter_panel = particles::EmitterConfigPanel::new();
                    self.animation.emitter_panel.open = true;
                    self.frame.dirty = true;
                } else if self.editor.brush.sub_mode == brush::BrushSubMode::Marker {
                    self.editor.push_undo_snapshot("Marker stroke");
                    let shape = self.editor.brush.shape;
                    let size = self.editor.brush.size;
                    let buf = self.editor.layer_stack.active_layer().buffer.clone();
                    tools::brush::accumulate_marker_stamp(
                        &buf,
                        bx,
                        by,
                        shape,
                        size,
                        &mut self.animation.marker_accum,
                    );
                } else {
                    self.editor.push_undo_snapshot("Brush");
                    let mut cell = canvas::CanvasCell {
                        ch: self.editor.brush.ch,
                        fg: None,
                        bg: None,
                        height: None,
                    };
                    self.editor.palette.apply_to_cell(&mut cell);
                    let shape = self.editor.brush.shape;
                    let size = self.editor.brush.size;
                    let mut buf = self.editor.layer_stack.active_layer().buffer.clone();
                    tools::brush::paint_stamp(&mut buf, bx, by, shape, size, cell);
                    *self.editor.layer_stack.active_layer_mut().buffer_mut() = buf;
                    self.editor.recomposite_canvas();
                }
                self.interaction.prev_mouse_buf = Some((bx, by));
            }
            MouseEventKind::Drag(_) => {
                let Some((bx, by)) =
                    self.editor
                        .screen_to_buffer(mouse.column, mouse.row, canvas_inner_rect)
                else {
                    return;
                };
                self.editor
                    .canvas
                    .set_cursor(bx.max(0) as u16, by.max(0) as u16);
                self.editor.mark_dirty();

                if is_selection_tool {
                    self.editor.handle_selection_drag(
                        bx,
                        by,
                        &mut self.interaction.selection_drag_origin,
                        &mut self.interaction.selection_lasso_points,
                    );
                    return;
                }

                // Layer lock enforcement (GPT review F-09): no drag
                // painting on a locked layer.
                if self.editor.active_layer_locked()
                    && !matches!(
                        self.editor.toolbox.selected,
                        Tool::Eyedropper | Tool::Emitter
                    )
                {
                    self.interaction.prev_mouse_buf = None;
                    self.interaction.line_start = None;
                    return;
                }

                if self.editor.toolbox.selected == Tool::Line {
                    if let (Some((sx, sy)), Some(ref saved)) = (
                        self.interaction.line_start,
                        self.interaction.saved_buffer.clone(),
                    ) {
                        let saved_clone = saved.clone();
                        let mut cell = canvas::CanvasCell {
                            ch: self.editor.brush.ch,
                            fg: None,
                            bg: None,
                            height: None,
                        };
                        self.editor.palette.apply_to_cell(&mut cell);
                        let shape = self.editor.brush.shape;
                        let size = self.editor.brush.size;
                        let mut buf = saved_clone;
                        tools::line::draw_line_segment(&mut buf, sx, sy, bx, by, shape, size, cell);
                        *self.editor.layer_stack.active_layer_mut().buffer_mut() = buf;
                        self.editor.recomposite_canvas();
                    }
                    return;
                }
                if self.editor.toolbox.selected == Tool::Move {
                    if let Some((ox, oy)) = self.interaction.move_origin {
                        let dx = bx - ox;
                        let dy = by - oy;
                        let has_selection = self
                            .interaction
                            .saved_selection
                            .as_ref()
                            .is_some_and(|s| s.is_active());
                        if has_selection {
                            if let (Some(mut moved_sel), Some(mut buf)) = (
                                self.interaction.saved_selection.clone(),
                                self.interaction.saved_buffer.clone(),
                            ) {
                                moved_sel.move_selection(&mut buf, dx, dy);
                                *self.editor.layer_stack.active_layer_mut().buffer_mut() = buf;
                                self.editor.selection = Some(moved_sel);
                                self.editor.recomposite_canvas();
                            }
                        } else if let Some(ref saved_buf) = self.interaction.saved_buffer {
                            let moved = tools::move_tool::translate_buffer(saved_buf, dx, dy);
                            *self.editor.layer_stack.active_layer_mut().buffer_mut() = moved;
                            self.editor.recomposite_canvas();
                        }
                    }
                    return;
                }
                if self.editor.toolbox.selected == Tool::Rotate {
                    if let Some((ox, _)) = self.interaction.rotate_origin {
                        let steps = rotate_drag_steps(bx - ox);
                        let clockwise = steps > 0;
                        let has_selection = self
                            .interaction
                            .saved_selection
                            .as_ref()
                            .is_some_and(|s| s.is_active());
                        if has_selection {
                            if let (Some(mut sel), Some(mut buf)) = (
                                self.interaction.saved_selection.clone(),
                                self.interaction.saved_buffer.clone(),
                            ) {
                                for _ in 0..steps.unsigned_abs() {
                                    buf = tools::rotate_tool::rotate_region(
                                        &buf,
                                        sel.bounds(),
                                        clockwise,
                                    );
                                    sel = sel.rotate_90(clockwise);
                                }
                                *self.editor.layer_stack.active_layer_mut().buffer_mut() = buf;
                                self.editor.selection = Some(sel);
                                self.editor.recomposite_canvas();
                            }
                        } else if let Some(ref saved_buf) = self.interaction.saved_buffer {
                            let mut buf = saved_buf.clone();
                            for _ in 0..steps.unsigned_abs() {
                                buf = tools::rotate_tool::rotate_whole_buffer(&buf, clockwise);
                            }
                            *self.editor.layer_stack.active_layer_mut().buffer_mut() = buf;
                            self.editor.recomposite_canvas();
                        }
                    }
                    return;
                }
                if let Some((px, py)) = self.interaction.prev_mouse_buf {
                    if self.editor.toolbox.selected == Tool::Eraser {
                        let shape = self.editor.brush.shape;
                        let size = self.editor.brush.size;
                        let mut buf = self.editor.layer_stack.active_layer().buffer.clone();
                        tools::eraser::erase_line(&mut buf, px, py, bx, by, shape, size);
                        *self.editor.layer_stack.active_layer_mut().buffer_mut() = buf;
                        self.editor.recomposite_canvas();
                    } else if self.editor.toolbox.selected == Tool::Spray {
                        let mut cell = canvas::CanvasCell {
                            ch: self.editor.brush.ch,
                            fg: None,
                            bg: None,
                            height: None,
                        };
                        self.editor.palette.apply_to_cell(&mut cell);
                        let mut rng = StdRng::seed_from_u64(rand::thread_rng().gen());
                        let size = self.editor.brush.size;
                        let density = self.editor.brush.density;
                        let mut buf = self.editor.layer_stack.active_layer().buffer.clone();
                        tools::spray::spray_line(
                            &mut buf, px, py, bx, by, size, density, cell, &mut rng,
                        );
                        *self.editor.layer_stack.active_layer_mut().buffer_mut() = buf;
                        self.editor.recomposite_canvas();
                    } else if self.editor.brush.sub_mode == brush::BrushSubMode::Marker {
                        let shape = self.editor.brush.shape;
                        let size = self.editor.brush.size;
                        let buf = self.editor.layer_stack.active_layer().buffer.clone();
                        tools::brush::accumulate_marker_line(
                            &buf,
                            px,
                            py,
                            bx,
                            by,
                            shape,
                            size,
                            &mut self.animation.marker_accum,
                        );
                    } else {
                        let mut cell = canvas::CanvasCell {
                            ch: self.editor.brush.ch,
                            fg: None,
                            bg: None,
                            height: None,
                        };
                        self.editor.palette.apply_to_cell(&mut cell);
                        let shape = self.editor.brush.shape;
                        let size = self.editor.brush.size;
                        let mut buf = self.editor.layer_stack.active_layer().buffer.clone();
                        tools::brush::paint_line(&mut buf, px, py, bx, by, shape, size, cell);
                        *self.editor.layer_stack.active_layer_mut().buffer_mut() = buf;
                        self.editor.recomposite_canvas();
                    }
                }
                self.interaction.prev_mouse_buf = Some((bx, by));
            }
            MouseEventKind::Up(_) => {
                if self.interaction.mouse_batch_active {
                    if self.editor.brush.sub_mode == brush::BrushSubMode::Marker
                        && !self.animation.marker_accum.is_empty()
                    {
                        let colors = self.editor.palette.selected_color_array();
                        if !colors.is_empty() {
                            let target = self.editor.palette.target;
                            let mut buf = self.editor.layer_stack.active_layer().buffer.clone();
                            tools::brush::commit_marker_accum(
                                &mut buf,
                                &mut self.animation.marker_accum,
                                &colors,
                                target,
                                mouse.modifiers.contains(KeyModifiers::ALT),
                            );
                            *self.editor.layer_stack.active_layer_mut().buffer_mut() = buf;
                            self.editor.recomposite_canvas();
                        }
                    }
                    self.editor.undo.end_batch();
                    self.interaction.mouse_batch_active = false;
                }
                if is_selection_tool {
                    self.editor.handle_selection_up(
                        &mut self.interaction.selection_drag_origin,
                        &mut self.interaction.selection_lasso_points,
                    );
                }
                self.interaction.prev_mouse_buf = None;
                self.interaction.line_start = None;
                self.interaction.saved_buffer = None;
                self.interaction.move_origin = None;
                self.interaction.rotate_origin = None;
                self.interaction.saved_selection = None;
            }
            MouseEventKind::Moved => {
                if let Some((bx, by)) =
                    self.editor
                        .screen_to_buffer(mouse.column, mouse.row, canvas_inner_rect)
                {
                    self.editor
                        .canvas
                        .set_cursor(bx.max(0) as u16, by.max(0) as u16);
                }
            }
            _ => {}
        }
    }

    /// Rebuild lighting LUT and rgb→swatch mapping from palette editor data.
    fn rebuild_lighting_from_palette(&mut self) {
        let swatch_data = self.palette_editor.lighting_swatches();
        self.lighting.lut = lighting::LightingLut::from_swatches(
            &swatch_data,
            crate::image_input::DEFAULT_CHAR_MAP,
        );
        // Build rgb→swatch map from palette editor swatches
        let swatch_pairs: Vec<(String, String)> = self
            .palette_editor
            .swatches
            .iter()
            .map(|s| (s.name.clone(), s.hex.clone()))
            .collect();
        self.ctx.palette_rgb_to_swatch = palette::build_rgb_to_swatch(&swatch_pairs);
    }

    pub fn handle_key_event(&mut self, key: impl Into<KeyEvent>) -> Option<AppEvent> {
        let key = key.into();
        let code = key.code;
        let modifiers = key.modifiers;

        // Keybindings overlay: Esc closes it, arrows scroll, swallow all other keys
        if self.ui.show_keybindings {
            match code {
                KeyCode::Esc | KeyCode::Char('q') => {
                    self.ui.show_keybindings = false;
                    self.ui.keybindings_scroll = 0;
                    self.frame.dirty = true;
                }
                KeyCode::Up => {
                    self.ui.keybindings_scroll = self.ui.keybindings_scroll.saturating_sub(1);
                    self.frame.dirty = true;
                }
                KeyCode::Down => {
                    self.ui.keybindings_scroll = self.ui.keybindings_scroll.saturating_add(1);
                    self.frame.dirty = true;
                }
                KeyCode::PageUp => {
                    self.ui.keybindings_scroll = self.ui.keybindings_scroll.saturating_sub(10);
                    self.frame.dirty = true;
                }
                KeyCode::PageDown => {
                    self.ui.keybindings_scroll = self.ui.keybindings_scroll.saturating_add(10);
                    self.frame.dirty = true;
                }
                _ => {}
            }
            return None;
        }

        // Unsaved-changes dialog: intercept all keys before anything else.
        // Y = save & proceed (only offered when saving is possible),
        // N = discard & proceed, C/Esc = cancel the transition entirely
        // (GPT review F-08).
        if self.dialogs.quit_confirm_dialog {
            match code {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                    self.dialogs.quit_confirm_dialog = false;
                    self.frame.dirty = true;
                    let is_quit = self.dialogs.pending_transition == Some(PendingDocAction::Quit);
                    if self.can_save_now() {
                        if is_quit {
                            self.dialogs.quit_after_save = true;
                        }
                        // The pending transition stays armed; it runs when
                        // SaveComplete reports an unchanged successful save.
                        self.start_save();
                    } else {
                        // Nothing to save with — treat as discard.
                        self.execute_pending_transition();
                    }
                }
                KeyCode::Char('n') | KeyCode::Char('N') => {
                    self.dialogs.quit_confirm_dialog = false;
                    self.frame.dirty = true;
                    self.execute_pending_transition();
                }
                KeyCode::Char('c') | KeyCode::Char('C') | KeyCode::Esc => {
                    self.dialogs.quit_confirm_dialog = false;
                    self.dialogs.pending_transition = None;
                    self.dialogs.pending_open_path = None;
                    self.frame.dirty = true;
                }
                _ => {}
            }
            return None;
        }

        // In-canvas animation playback: intercept all keys, mirroring the
        // controls already implemented on AnimationPlayer::handle_key
        // (space=pause, arrows=seek, +/-=speed, l/L=loop toggle). Esc/q
        // dismiss playback and return to normal editing.
        if let Some(player) = self.animation.inline_player.as_ref() {
            let consumed = player.handle_key(code);
            let should_dismiss =
                consumed && matches!(code, KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q'));
            self.frame.dirty = true;
            if should_dismiss {
                self.stop_inline_playback();
            }
            return None;
        }

        // New image dialog active
        if self.dialogs.new_image.active {
            self.dialogs.new_image.handle_key(code);
            if !self.dialogs.new_image.active && self.dialogs.new_image.confirmed {
                let w = self.dialogs.new_image.result_width;
                let h = self.dialogs.new_image.result_height;
                let pal_name = self.dialogs.new_image.result_palette_name.clone();
                let pal_swatches = self.dialogs.new_image.result_palette_swatches.clone();
                self.ui.session_type = SessionType::Image;
                self.editor.image_editor = image_editor::ImageEditor::new();
                self.ui.mode = AppMode::ImageEditor;
                self.editor.canvas = canvas::CanvasWidget::new(w, h);
                self.editor.layer_stack = layers::LayerStack::new(w as usize, h as usize);
                self.editor.layer_panel = layers::LayerPanel::new();
                self.editor.layer_panel.theme = self.ctx.theme.clone();
                self.editor.layer_panel.icons = self.ctx.icons.clone();
                if !pal_swatches.is_empty() {
                    self.palette_editor.open = true;
                    self.palette_editor.name_buffer = pal_name;
                    self.palette_editor.swatches = pal_swatches;
                }
                self.editor.recomposite_canvas();
                self.welcome.screen.show = false;
                self.welcome.fx = None;
                self.frame.dirty = true;
            }
            return None;
        }

        // System font picker dialog active
        if self.dialogs.system_font.active {
            self.dialogs.system_font.handle_key(code);
            if !self.dialogs.system_font.active && self.dialogs.system_font.confirmed {
                self.start_system_font_conversion();
            }
            return None;
        }

        // Font import (TTF/OTF) options dialog active
        if self.dialogs.font_import_options.active {
            self.dialogs.font_import_options.handle_key(code);
            if !self.dialogs.font_import_options.active
                && self.dialogs.font_import_options.confirmed
            {
                let path = self.dialogs.font_import_options.path.clone();
                let size = self.dialogs.font_import_options.result_size;
                let charset = self.dialogs.font_import_options.result_charset.clone();
                self.perform_import_font(path, size, charset);
            }
            return None;
        }

        // GIF import dialog active
        if self.dialogs.gif_import.active {
            self.dialogs.gif_import.handle_key(code);
            if !self.dialogs.gif_import.active && self.dialogs.gif_import.confirmed {
                if let Some(cfg) = self.dialogs.gif_import.config.take() {
                    self.perform_import_gif(cfg);
                }
            }
            return None;
        }

        // File ops dialog active: dispatch all keys to it
        if self.dialogs.file_ops.mode != file_ops::FileOpsMode::Idle {
            let prev_mode = self.dialogs.file_ops.mode;
            self.dialogs.file_ops.handle_key(code);
            if self.dialogs.file_ops.mode == file_ops::FileOpsMode::Idle {
                return self.complete_file_ops_dialog(prev_mode);
            }
            return None;
        }

        // Export dialog active: dispatch all keys to it
        if self.dialogs.export_dialog.active {
            let prev_format = self.dialogs.export_dialog.format;
            self.dialogs.export_dialog.handle_key(code);
            // If format changed to an animated format and timeline has frames,
            // populate timeline data. Skip this if frame_delays already
            // matches the frame count — that means timing is already sitting
            // there (e.g. an FPS-preset override during this session), and
            // re-deriving would clobber it. Per-frame delays come from the
            // frames themselves (F-26).
            let count = self.animation.timeline_state.frames.len();
            if (self.dialogs.export_dialog.format == export::ExportMode::Gif
                || self.dialogs.export_dialog.format == export::ExportMode::Apng
                || self.dialogs.export_dialog.format == export::ExportMode::Ansi)
                && prev_format != self.dialogs.export_dialog.format
                && count > 0
                && self.dialogs.export_dialog.frame_delays.len() != count
            {
                let ts = &self.animation.timeline_state;
                self.dialogs.export_dialog.set_timeline_from_timeline(ts);
            }
            if self.dialogs.export_dialog.play_requested {
                self.dialogs.export_dialog.play_requested = false;
                self.launch_player_from_export();
                return None;
            }
            if !self.dialogs.export_dialog.active {
                self.perform_export();
            }
            return None;
        }

        // Rascii import dialog active: dispatch all keys to it
        if self.dialogs.rascii_import.active {
            let prev_confirmed = self.dialogs.rascii_import.confirmed;
            self.dialogs.rascii_import.handle_key(code);
            if !self.dialogs.rascii_import.active {
                if self.dialogs.rascii_import.confirmed && !prev_confirmed {
                    self.perform_rascii_import();
                }
                self.frame.dirty = true;
            }
            return None;
        }

        // Undo history panel open: dispatch to it first
        if self.dialogs.undo_panel.open {
            self.dialogs.undo_panel.handle_key(code);
            return None;
        }

        // Keyframe editor: intercept all keys when open
        if self.animation.timeline_state.keyframe_editor.open
            && self
                .animation
                .timeline_state
                .handle_keyframe_editor_key(code)
        {
            self.frame.dirty = true;
            return None;
        }

        // Tween panel: intercept keys when open
        if self.animation.timeline_state.tween.is_some()
            && self.animation.timeline_state.handle_tween_key(code)
        {
            self.frame.dirty = true;
            return None;
        }

        // Palette editor: dispatch all keys when open
        if self.palette_editor.open {
            if self.palette_editor.handle_key(code) {
                if self.palette_editor.modified {
                    self.palette_editor
                        .apply_to_palette(&mut self.editor.palette);
                    self.palette_editor.modified = false;
                    if self.lighting.scene.is_some() {
                        self.rebuild_lighting_from_palette();
                    }
                }
                self.frame.dirty = true;
            }
            return None;
        }

        // Menu bar active: dispatch all keys to it
        if self.ui.menu_bar_state.is_active() {
            self.ui
                .menu_bar
                .handle_key_event(key, &mut self.ui.menu_bar_state);
            if let Some(action) = self.ui.menu_bar_state.drain_actions() {
                return Some(AppEvent::Menu(action));
            }
            return None;
        }

        // Alt+key: open menu bar
        if modifiers == KeyModifiers::ALT
            && self
                .ui
                .menu_bar
                .handle_key_event(key, &mut self.ui.menu_bar_state)
        {
            return None;
        }

        // Global key dispatch (early global actions before modal/mode checks)
        if let Some(action) = keymap::lookup_global(code, modifiers) {
            match action {
                keymap::GlobalAction::Undo
                | keymap::GlobalAction::Redo
                | keymap::GlobalAction::ToggleUndoPanel => {
                    return self.dispatch_global(action);
                }
                _ => {}
            }
        }

        if self.dialogs.settings.settings_open {
            if self.dialogs.settings.handle_key(code) {
                self.apply_settings();
                return None;
            }
            if let KeyCode::Char('S') = code {
                self.dialogs.settings.settings_open = false;
            }
            return None;
        }

        // Timeline: Space to play animation from current frame, in place in
        // the canvas (the rest of the editor UI stays visible around it).
        // Checked here — after every modal dialog/overlay above but before
        // the Layers panel dispatch and the keyboard-paint block below — so
        // starting playback always wins over both the Layers panel's own
        // Space/Enter binding (toggle visibility) and keyboard-painting
        // tools, exactly mirroring Enter's old priority. Keyboard paint
        // still works via Enter (see the paint block below), which is no
        // longer claimed by playback.
        if code == KeyCode::Char(' ') && !self.animation.timeline_state.frames.is_empty() {
            self.start_inline_playback_from_timeline();
            return None;
        }

        // Layer panel: dispatch keys when drawer shows layers
        if self.side_panel.open
            && self.side_panel.active_tab == TabId::Layers
            && self
                .editor
                .layer_panel
                .handle_key(key, &mut self.editor.layer_stack)
        {
            self.editor.recomposite_canvas();
            self.editor.mark_dirty();
            self.frame.dirty = true;
            return None;
        }

        // Welcome screen: intercept before mode-specific dispatch
        if self.welcome.screen.show {
            let recent_count = self.dialogs.recent_files.len();
            if let Some(action) = self
                .welcome
                .screen
                .handle_key(code, modifiers, recent_count)
            {
                self.dispatch_welcome_action(action);
                return None;
            }
        }

        // Font Editor mode: dispatch to font_editor before canvas/tools
        if self.ui.mode == AppMode::FontEditor {
            if let Some(ev) = self.handle_font_editor_key(key) {
                return Some(ev);
            }
        }

        // Image Editor mode: dispatch to image_editor before canvas/tools
        if self.ui.mode == AppMode::ImageEditor {
            if let Some(ev) = self.handle_image_editor_key(code) {
                return Some(ev);
            }
        }

        // Text tool dispatch
        if self.editor.toolbox.selected == Tool::Text {
            // Auto-activate editing when side panel Text tab is open
            if !self.editor.text_tool.editing
                && self.side_panel.open
                && self.side_panel.active_tab == TabId::Text
            {
                self.editor.text_tool.editing = true;
            }
            let cursor = self.editor.canvas.cursor();
            if let Some(undo_label) = self.editor.text_tool.handle_key(code, modifiers, cursor) {
                if !undo_label.is_empty() {
                    self.editor.push_undo_snapshot(undo_label);
                    self.editor.mark_dirty();
                }
                return None;
            }
        }

        // Editor state dispatch (rotate, selection, move tool, polygon select,
        // deselect, keyboard painting)
        if self
            .editor
            .handle_key(code, modifiers, &mut self.frame.dirty)
        {
            return None;
        }

        // Side panel: Alt+left/right arrows switch tabs when open
        if self.side_panel.open && modifiers == KeyModifiers::ALT {
            match code {
                KeyCode::Left => {
                    self.side_panel.cycle_tab(false);
                    if self.side_panel.active_tab == TabId::Text
                        && self.editor.toolbox.selected == Tool::Text
                    {
                        self.editor.text_tool.editing = true;
                    }
                    self.frame.dirty = true;
                    return None;
                }
                KeyCode::Right => {
                    self.side_panel.cycle_tab(true);
                    if self.side_panel.active_tab == TabId::Text
                        && self.editor.toolbox.selected == Tool::Text
                    {
                        self.editor.text_tool.editing = true;
                    }
                    self.frame.dirty = true;
                    return None;
                }
                _ => {}
            }
        }

        // Animation state dispatch (timeline nav, emitter bake/toggle)
        if self
            .animation
            .handle_key(code, modifiers, &mut self.editor, &mut self.frame.dirty)
        {
            return None;
        }

        // Lighting mode: key handling
        if self.ui.mode == AppMode::Lighting {
            let w = self.editor.canvas.buffer.width() as i16;
            let h = self.editor.canvas.buffer.height() as i16;
            match self
                .lighting
                .handle_key(code, modifiers, w, h, &mut self.frame.dirty)
            {
                Some(false) => {
                    self.ui.mode = self.ui.prev_mode;
                    self.frame.dirty = true;
                    return None;
                }
                Some(true) => return None,
                None => {}
            }
        }

        // Enter lighting mode with uppercase G
        if code == KeyCode::Char('G') && self.ui.mode != AppMode::Lighting {
            self.ui.prev_mode = self.ui.mode;
            self.ui.mode = AppMode::Lighting;
            if self.lighting.scene.is_none() {
                let mut scene = lighting::Scene::new();
                scene.add_light(lighting::Light::Ambient {
                    intensity: 0.5,
                    color: lighting::Rgb(255, 255, 255),
                });
                self.lighting.scene = Some(scene);
                // Regenerate LUT from palette when scene activates
                self.rebuild_lighting_from_palette();
            }
            self.lighting.panel.selected_index = 0;
            self.lighting.panel.show_help = true;
            self.frame.dirty = true;
            return None;
        }

        // Canvas cursor movement, zoom, grid
        {
            let ck = key.code;
            if matches!(
                ck,
                KeyCode::Up
                    | KeyCode::Down
                    | KeyCode::Left
                    | KeyCode::Right
                    | KeyCode::Char('+')
                    | KeyCode::Char('=')
                    | KeyCode::Char('-')
                    | KeyCode::Char('_')
            ) && self.editor.canvas.handle_key(ck, 0, 0)
            {
                return Some(AppEvent::Canvas(crate::tui::events::CanvasEvent::Modified));
            }
        }

        // Emitter config panel: dispatch when panel is open
        if self.animation.emitter_panel.open {
            let handled = self
                .animation
                .emitter_panel
                .handle_config_key(code, &mut self.animation.particle_system.config);
            if handled {
                self.frame.dirty = true;
                return None;
            }
        }
        // Settings toggle
        if code == KeyCode::Char('S') && !modifiers.contains(KeyModifiers::CONTROL) {
            self.dialogs.settings.canvas_width = self.editor.canvas.buffer.width() as u16;
            self.dialogs.settings.canvas_height = self.editor.canvas.buffer.height() as u16;
            self.dialogs.settings.show_grid = self.editor.canvas.show_grid();
            self.dialogs.settings.settings_open = true;
            self.frame.dirty = true;
            return None;
        }

        // Toggle keyframe editor (uppercase only to avoid conflict)
        if code == KeyCode::Char('K') {
            self.animation.timeline_state.keyframe_editor.open =
                !self.animation.timeline_state.keyframe_editor.open;
            self.frame.dirty = true;
            return None;
        }

        // Ctrl+Shift+P: toggle palette editor
        if code == KeyCode::Char('P') && modifiers == KeyModifiers::CONTROL | KeyModifiers::SHIFT {
            self.palette_editor.open = !self.palette_editor.open;
            if self.palette_editor.open {
                self.palette_editor
                    .load_current_from_palette(&self.editor.palette);
                self.palette_editor.available_palettes(None);
                self.palette_editor.lighting_pickers_visible =
                    self.ui.mode == AppMode::Lighting || self.lighting.scene.is_some();
                if self.lighting.scene.is_some() {
                    self.rebuild_lighting_from_palette();
                }
            }
            self.frame.dirty = true;
            return None;
        }

        // Props panel typed-entry mode intercept
        if self.props_panel.mode != PropsPanelMode::Idle {
            if let Some(action) = self.props_panel.handle_key(code) {
                self.dispatch_props_action(action);
                self.frame.dirty = true;
            }
            return None;
        }

        // Toolbox tool selection + brush adjustments (inline from old ToolboxComponent)
        // NOTE: 'T' is excluded from the tool-selector catch-all so it falls
        // through to dispatch_global for ToggleTimeline / OpenTweenPanel.
        {
            use crate::tui::events::ToolboxEvent;
            let handled = match code {
                KeyCode::Char('[') => {
                    self.editor.brush.size_down();
                    Some(AppEvent::Toolbox(ToolboxEvent::BrushChanged))
                }
                KeyCode::Char(']') => {
                    self.editor.brush.size_up();
                    Some(AppEvent::Toolbox(ToolboxEvent::BrushChanged))
                }
                KeyCode::Char(';') => {
                    self.editor.brush.density_down();
                    Some(AppEvent::Toolbox(ToolboxEvent::BrushChanged))
                }
                KeyCode::Char('\'') => {
                    self.editor.brush.density_up();
                    Some(AppEvent::Toolbox(ToolboxEvent::BrushChanged))
                }
                KeyCode::Char('\\') => {
                    self.editor.brush.cycle_shape();
                    Some(AppEvent::Toolbox(ToolboxEvent::BrushChanged))
                }
                KeyCode::Char('M') if self.editor.toolbox.selected == Tool::Brush => {
                    self.editor
                        .brush
                        .cycle_sub_mode(self.editor.palette.has_multi_select());
                    if self.editor.brush.sub_mode == brush::BrushSubMode::Normal {
                        self.animation.marker_accum.clear();
                    }
                    Some(AppEvent::Toolbox(ToolboxEvent::BrushChanged))
                }
                KeyCode::Char(c) if !modifiers.contains(KeyModifiers::CONTROL) && c != 'T' => {
                    let lower = c.to_ascii_lowercase();
                    let mut found = None;
                    for tool in Tool::all() {
                        if let KeyCode::Char(tc) = tool.key_shortcut() {
                            if tc == lower {
                                let was_brush = self.editor.toolbox.selected == Tool::Brush;
                                self.editor.toolbox.selected = *tool;
                                if was_brush && *tool != Tool::Brush {
                                    self.animation.marker_accum.clear();
                                }
                                found = Some(AppEvent::Toolbox(ToolboxEvent::ToolSelected));
                                break;
                            }
                        }
                    }
                    found
                }
                _ => None,
            };
            if let Some(action) = handled {
                if self.editor.toolbox.selected != Tool::PolygonSelect {
                    self.editor.selection_polygon_points.clear();
                }
                return Some(action);
            }
        }

        // Palette color selection (inline from old PaletteComponent)
        {
            use crate::tui::events::PaletteEvent;
            let handled = match code {
                KeyCode::Char('x')
                | KeyCode::Char('X')
                | KeyCode::Char('f')
                | KeyCode::Char('F')
                | KeyCode::Char('h')
                | KeyCode::Char('H')
                | KeyCode::Char('z')
                | KeyCode::Char('Z')
                | KeyCode::Left
                | KeyCode::Right
                | KeyCode::Up
                | KeyCode::Down
                | KeyCode::Enter
                | KeyCode::Backspace
                | KeyCode::Esc => {
                    if self.editor.palette.handle_key(code) {
                        let color = self.editor.palette.selected_color;
                        let target = self.editor.palette.target;
                        if let Some(c) = color {
                            Some(AppEvent::Palette(PaletteEvent::ColorChanged(c, target)))
                        } else {
                            Some(AppEvent::Palette(PaletteEvent::BrushChanged))
                        }
                    } else {
                        None
                    }
                }
                _ => None,
            };
            if let Some(action) = handled {
                return Some(action);
            }
        }

        // Keyboard painting: Space/Enter paints or erases at cursor
        if Tool::is_paint_tool(self.editor.toolbox.selected)
            && matches!(code, KeyCode::Char(' ') | KeyCode::Enter)
            // Layer lock enforcement (GPT review F-09)
            && !self.editor.active_layer_locked()
        {
            let (cx, cy) = self.editor.canvas.cursor();
            self.editor.push_undo_snapshot("Keyboard paint");
            if self.editor.toolbox.selected == Tool::Fill {
                let mut cell = canvas::CanvasCell {
                    ch: self.editor.brush.ch,
                    fg: None,
                    bg: None,
                    height: None,
                };
                self.editor.palette.apply_to_cell(&mut cell);
                let mut buf = self.editor.layer_stack.active_layer().buffer.clone();
                tools::fill::flood_fill(&mut buf, cx as i16, cy as i16, cell);
                *self.editor.layer_stack.active_layer_mut().buffer_mut() = buf;
                self.editor.recomposite_canvas();
            } else if self.editor.toolbox.selected == Tool::Eraser {
                let shape = self.editor.brush.shape;
                let size = self.editor.brush.size;
                let mut buf = self.editor.layer_stack.active_layer().buffer.clone();
                tools::eraser::erase_stamp(&mut buf, cx as i16, cy as i16, shape, size);
                *self.editor.layer_stack.active_layer_mut().buffer_mut() = buf;
                self.editor.recomposite_canvas();
            } else if self.editor.toolbox.selected == Tool::Spray {
                let mut cell = canvas::CanvasCell {
                    ch: self.editor.brush.ch,
                    fg: None,
                    bg: None,
                    height: None,
                };
                self.editor.palette.apply_to_cell(&mut cell);
                let mut rng = StdRng::seed_from_u64(rand::thread_rng().gen());
                let size = self.editor.brush.size;
                let density = self.editor.brush.density;
                let mut buf = self.editor.layer_stack.active_layer().buffer.clone();
                tools::spray::spray_stamp(
                    &mut buf, cx as i16, cy as i16, size, density, cell, &mut rng,
                );
                *self.editor.layer_stack.active_layer_mut().buffer_mut() = buf;
                self.editor.recomposite_canvas();
            } else {
                let mut cell = canvas::CanvasCell {
                    ch: self.editor.brush.ch,
                    fg: None,
                    bg: None,
                    height: None,
                };
                self.editor.palette.apply_to_cell(&mut cell);
                let shape = self.editor.brush.shape;
                let size = self.editor.brush.size;
                let mut buf = self.editor.layer_stack.active_layer().buffer.clone();
                tools::brush::paint_stamp(&mut buf, cx as i16, cy as i16, shape, size, cell);
                *self.editor.layer_stack.active_layer_mut().buffer_mut() = buf;
                self.editor.recomposite_canvas();
            }
            self.editor.mark_dirty();
            return None;
        }

        // Global dispatch: file ops, view toggles, mode cycling, quit
        if let Some(action) = keymap::lookup_global(code, modifiers) {
            return self.dispatch_global(action);
        }

        None
    }

    fn dispatch_global(&mut self, action: keymap::GlobalAction) -> Option<AppEvent> {
        use keymap::GlobalAction as GA;
        match action {
            GA::FileNew => {
                self.dialogs.new_image.enter_new_image();
                None
            }
            GA::FileOpen => {
                self.start_open();
                None
            }
            GA::FileSave => {
                self.start_save();
                None
            }
            GA::FileSaveAs => {
                self.start_save_as();
                None
            }
            GA::Export => {
                let mode = match self.ui.mode {
                    AppMode::FontEditor => export::ExportMode::Txt,
                    _ => export::ExportMode::Png,
                };
                self.dialogs.export_dialog.enter_export(mode);
                if (mode == export::ExportMode::Gif
                    || mode == export::ExportMode::Apng
                    || mode == export::ExportMode::Ansi)
                    && !self.animation.timeline_state.frames.is_empty()
                {
                    self.dialogs
                        .export_dialog
                        .set_timeline_from_timeline(&self.animation.timeline_state);
                }
                self.frame.dirty = true;
                None
            }
            GA::Undo => {
                self.perform_undo();
                Some(AppEvent::Undo)
            }
            GA::Redo => {
                self.perform_redo();
                Some(AppEvent::Redo)
            }
            GA::ToggleUndoPanel => {
                self.dialogs.undo_panel.toggle();
                Some(AppEvent::UndoPanelToggled)
            }
            GA::ToggleRenderMode => {
                self.ctx.render_mode = self.ctx.render_mode.toggle();
                self.frame.dirty = true;
                Some(AppEvent::RenderModeChanged)
            }
            GA::ToggleZenMode => {
                self.ui.zen_mode = !self.ui.zen_mode;
                self.frame.dirty = true;
                None
            }
            GA::CycleDrawer => {
                self.side_panel.toggle_open();
                self.frame.dirty = true;
                None
            }
            GA::ToggleKeybindings => {
                self.ui.show_keybindings = !self.ui.show_keybindings;
                self.frame.dirty = true;
                None
            }
            GA::ToggleTimeline => {
                self.animation.timeline_visible = !self.animation.timeline_visible;
                self.frame.dirty = true;
                None
            }
            GA::OpenTweenPanel => {
                self.animation.timeline_state.open_tween();
                self.frame.dirty = true;
                None
            }
            GA::CycleTabPrev => {
                self.side_panel.cycle_tab(false);
                self.frame.dirty = true;
                None
            }
            GA::CycleTabNext => {
                self.side_panel.cycle_tab(true);
                self.frame.dirty = true;
                None
            }
            GA::NextMode => {
                self.ui.mode = self.ui.mode.next_for(self.ui.session_type);
                self.editor.undo.clear();
                Some(AppEvent::ModeChanged)
            }
            GA::PrevMode => {
                self.ui.mode = self.ui.mode.prev_for(self.ui.session_type);
                self.editor.undo.clear();
                Some(AppEvent::ModeChanged)
            }
            GA::Quit => {
                self.trigger_quit();
                None
            }
        }
    }

    fn start_save(&mut self) {
        if self.ui.mode != AppMode::FontEditor {
            return;
        }
        if let Some(ref path) = self.editor.font_editor.current_path {
            if let Some(ref font) = self.editor.font_editor.font {
                if self.ctx.throbber.is_active() {
                    return;
                }
                let font = font.clone();
                let path = path.clone();
                self.dialogs.pending_save_revision = Some(self.editor.revision);
                let (tx, rx) = mpsc::channel();
                self.ctx.async_rx = Some(rx);
                self.ctx.throbber.start("Saving...");
                self.frame.dirty = true;
                std::thread::spawn(move || {
                    let result = file_ops::save_font(&font, &path)
                        .map(|_| path)
                        .map_err(|e| e.to_string());
                    let _ = tx.send(AsyncResult::SaveComplete(result));
                });
                return;
            }
        }
        self.start_save_as();
    }

    fn handle_image_editor_key(&mut self, code: KeyCode) -> Option<AppEvent> {
        if matches!(code, KeyCode::Char('o') | KeyCode::Char('O'))
            && !self.editor.image_editor.entering_path()
        {
            self.dialogs.file_ops.enter_open_image();
            self.frame.dirty = true;
            return Some(AppEvent::ImageEditor);
        }
        let was_entering = self.editor.image_editor.entering_path();
        if self.editor.image_editor.handle_key(code) {
            self.editor.sync_image_to_canvas();
            if was_entering && !self.editor.image_editor.entering_path() {
                self.editor.undo.clear();
            }
            return Some(AppEvent::ImageEditor);
        }
        None
    }

    fn handle_font_editor_key(&mut self, key: KeyEvent) -> Option<AppEvent> {
        if matches!(
            self.editor.font_editor.view,
            font_editor::FontEditorView::CharEditor(_)
        ) {
            self.editor.font_editor.brush_char = self.editor.brush.ch;
        }
        let area_width = crossterm::terminal::size().unwrap_or((80, 24)).0;
        let epoch_before = self.editor.font_editor.mutation_epoch;
        if self
            .editor
            .font_editor
            .handle_key(key.code, key.modifiers, area_width)
        {
            if self.editor.font_editor.mutation_epoch != epoch_before {
                // Real content change (glyph paint, header field, smush
                // rule, transform) — navigation keys don't trip this, so
                // dirty tracking is neither missed nor spurious (F-08).
                self.editor.mark_dirty();
            }
            if self.editor.font_editor.view != font_editor::FontEditorView::Overview {
                self.editor.sync_font_char_to_canvas();
            }
            return Some(AppEvent::FontEditor(
                crate::tui::events::FontEditorEvent::Changed(self.editor.font_editor.view),
            ));
        }
        None
    }

    fn start_save_as(&mut self) {
        if self.ui.mode != AppMode::FontEditor {
            return;
        }
        let default_name = if self.editor.font_editor.font_storage_name.is_empty() {
            "untitled".to_string()
        } else {
            self.editor.font_editor.font_storage_name.clone()
        };
        self.dialogs.file_ops.enter_save_as(
            self.editor.font_editor.current_path.as_deref(),
            &default_name,
        );
        self.frame.dirty = true;
    }

    /// Shared completion logic for when the file-ops dialog transitions
    /// back to Idle, regardless of whether that happened via keyboard
    /// (Enter) or mouse (click). `prev_mode` is the mode the dialog was in
    /// just before it closed.
    fn complete_file_ops_dialog(&mut self, prev_mode: file_ops::FileOpsMode) -> Option<AppEvent> {
        match prev_mode {
            file_ops::FileOpsMode::SaveAs => {
                self.perform_save();
                Some(AppEvent::SaveAsRequested)
            }
            file_ops::FileOpsMode::Open => {
                if self.dialogs.file_ops.is_browsing_zip() {
                    self.perform_open();
                    return Some(AppEvent::OpenRequested);
                }
                if self.dialogs.file_ops.path_buffer.trim().is_empty() {
                    return None;
                }
                let path = self.dialogs.file_ops.selected_path();
                if !path.exists() {
                    self.dialogs.file_ops.error_message =
                        format!("File not found: {}", path.display());
                    self.dialogs.file_ops.mode = file_ops::FileOpsMode::Open;
                    return None;
                }
                if path.is_dir() {
                    self.dialogs.file_ops.error_message =
                        "Select a .flf or .tlf file, not a directory".to_string();
                    self.dialogs.file_ops.mode = file_ops::FileOpsMode::Open;
                    return None;
                }
                self.perform_open();
                Some(AppEvent::OpenRequested)
            }
            file_ops::FileOpsMode::ImportFont => {
                if self.dialogs.file_ops.is_browsing_zip() {
                    // Zip bundles reachable from the font-import flow contain
                    // .flf/.tlf fonts (FIGlet's own zip convention, not
                    // .ttf/.otf) — route through the same Open/ZipEntry
                    // completion the Open dialog uses, not font_gen's TTF
                    // conversion path.
                    self.perform_open();
                    return Some(AppEvent::OpenRequested);
                }
                if self.dialogs.file_ops.path_buffer.trim().is_empty() {
                    return None;
                }
                let path = self.dialogs.file_ops.selected_path();
                if !path.exists() {
                    self.dialogs.file_ops.error_message =
                        format!("File not found: {}", path.display());
                    self.dialogs.file_ops.mode = file_ops::FileOpsMode::ImportFont;
                    return None;
                }
                // Open the size/charset options dialog instead of importing
                // immediately — this flow previously had no options step at
                // all (size/charset were hardcoded).
                self.dialogs.font_import_options.enter(path);
                self.frame.dirty = true;
                None
            }
            file_ops::FileOpsMode::ImportGif => {
                // Legacy path: open the new gif_import dialog instead
                self.dialogs.gif_import.enter();
                None
            }
            file_ops::FileOpsMode::OpenImage => {
                if self.dialogs.file_ops.path_buffer.trim().is_empty() {
                    return None;
                }
                let path = self.dialogs.file_ops.selected_path();
                if !path.exists() {
                    self.dialogs.file_ops.error_message =
                        format!("File not found: {}", path.display());
                    self.dialogs.file_ops.mode = file_ops::FileOpsMode::OpenImage;
                    return None;
                }
                self.perform_open_image(path);
                Some(AppEvent::ImageEditor)
            }
            file_ops::FileOpsMode::Idle => None,
        }
    }

    pub(crate) fn perform_save(&mut self) {
        if self.ctx.throbber.is_active() {
            return;
        }
        let path = self.dialogs.file_ops.selected_path();
        let font = match &self.editor.font_editor.font {
            Some(f) => f.clone(),
            None => return,
        };
        let result_path = path.clone();
        self.dialogs.pending_save_revision = Some(self.editor.revision);
        let (tx, rx) = mpsc::channel();
        self.ctx.async_rx = Some(rx);
        self.ctx.throbber.start("Saving...");
        self.frame.dirty = true;
        std::thread::spawn(move || {
            let result = file_ops::save_font(&font, &result_path)
                .map(|_| result_path)
                .map_err(|e| e.to_string());
            let _ = tx.send(AsyncResult::SaveComplete(result));
        });
    }

    pub(crate) fn handle_paste_event(&mut self, text: String) {
        if self.dialogs.file_ops.mode != file_ops::FileOpsMode::Idle {
            self.dialogs.file_ops.handle_paste(&text);
        }
    }

    pub(crate) fn start_open(&mut self) {
        if self.ui.mode != AppMode::FontEditor {
            return;
        }
        self.dialogs
            .file_ops
            .enter_open(self.dialogs.recent_files.list());
        self.frame.dirty = true;
    }

    /// Request opening a font. If the document has unsaved changes, the
    /// open waits behind the unsaved-changes dialog instead of silently
    /// replacing work (GPT review F-08).
    pub(crate) fn request_open(&mut self, target: Option<file_ops::OpenTarget>) {
        if self.editor.unsaved && self.dialogs.pending_transition.is_none() {
            self.dialogs.pending_transition = Some(PendingDocAction::OpenFont);
            // Keep the resolved target for when the transition runs.
            if let Some(t) = target {
                self.dialogs.pending_open_path = Some(match t {
                    file_ops::OpenTarget::File(p) => p,
                    file_ops::OpenTarget::ZipEntry { zip_path, .. } => zip_path,
                });
            }
            self.dialogs.quit_confirm_dialog = true;
            self.frame.dirty = true;
            return;
        }
        if let Some(path) = self.dialogs.pending_open_path.take() {
            self.perform_open_at(path);
            return;
        }
        self.perform_open();
    }

    pub(crate) fn perform_open_at(&mut self, path: std::path::PathBuf) {
        if self.ctx.throbber.is_active() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.ctx.async_rx = Some(rx);
        self.ctx.throbber.start("Loading...");
        self.frame.dirty = true;
        let path_clone = path.clone();
        std::thread::spawn(move || {
            let result = (|| -> Result<(crate::font::FIGfont, std::path::PathBuf), String> {
                let content = std::fs::read_to_string(&path_clone)
                    .map_err(|e| format!("Cannot read file: {e}"))?;
                let font = crate::font::parse_tlf_font(&content)
                    .map_err(|e| format!("Parse error: {e}"))?;
                Ok((font, path_clone))
            })();
            let _ = tx.send(AsyncResult::OpenComplete(result));
        });
    }

    pub(crate) fn perform_open(&mut self) {
        if self.ctx.throbber.is_active() {
            return;
        }
        let target = self.dialogs.file_ops.resolve_open_target();
        match target {
            file_ops::OpenTarget::File(_) => self.request_open(Some(target)),
            file_ops::OpenTarget::ZipEntry {
                zip_path,
                entry_name,
            } => {
                // ZIP entries are read straight from the archive; the
                // unsaved-changes guard applies at OpenComplete below.
                let (tx, rx) = mpsc::channel();
                self.ctx.async_rx = Some(rx);
                self.ctx.throbber.start("Loading...");
                self.frame.dirty = true;
                std::thread::spawn(move || {
                    let result =
                        (|| -> Result<(crate::font::FIGfont, std::path::PathBuf), String> {
                            let bytes = crate::font::read_zip_entry(&zip_path, &entry_name)
                                .map_err(|e| format!("ZIP read error: {e}"))?;
                            let content = String::from_utf8_lossy(&bytes).into_owned();
                            let font = crate::font::parse_tlf_font(&content)
                                .map_err(|e| format!("Parse error: {e}"))?;
                            Ok((font, zip_path))
                        })();
                    let _ = tx.send(AsyncResult::OpenComplete(result));
                });
            }
        }
    }

    fn perform_import_font(&mut self, path: std::path::PathBuf, size: f32, charset_name: String) {
        if self.ctx.throbber.is_active() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.ctx.async_rx = Some(rx);
        self.ctx.throbber.start("Converting font...");
        self.frame.dirty = true;
        std::thread::spawn(move || {
            let result = (|| -> Result<(crate::font::FIGfont, std::path::PathBuf), String> {
                let charset = crate::font_gen::resolve_charset(&charset_name)
                    .unwrap_or(rascii_art::charsets::DEFAULT);
                let font = crate::font_gen::font_file_to_figfont(&path, size, charset)
                    .map_err(|e| format!("Import failed: {e}"))?;
                Ok((font, path))
            })();
            let _ = tx.send(AsyncResult::OpenComplete(result));
        });
    }

    /// Extract the working "New Font from File" flow so it can be reached
    /// from both the welcome screen and the in-editor File menu.
    fn start_font_import_from_file(&mut self) {
        self.ui.session_type = SessionType::Font;
        self.dialogs.file_ops.enter_import_font();
        self.frame.dirty = true;
    }

    fn start_system_font_conversion(&mut self) {
        if self.ctx.throbber.is_active() {
            return;
        }
        let family = self.dialogs.system_font.result_family.clone();
        let size = self.dialogs.system_font.result_size;
        let charset_name = self.dialogs.system_font.result_charset.clone();
        let (tx, rx) = mpsc::channel();
        self.ctx.async_rx = Some(rx);
        self.ctx.throbber.start("Generating font...");
        self.frame.dirty = true;
        std::thread::spawn(move || {
            let charset = crate::font_gen::resolve_charset(&charset_name)
                .unwrap_or(rascii_art::charsets::DEFAULT);
            let result = crate::font_gen::system_font_to_figfont(&family, size, charset)
                .map(|font| (font, family))
                .map_err(|e| format!("Font generation failed: {e}"));
            let _ = tx.send(AsyncResult::SystemFontComplete(result));
        });
    }

    fn perform_import_gif(&mut self, config: dialogs::GifImportConfig) {
        let path = &config.path;
        let cw = config.canvas_width as usize;
        let ch = config.canvas_height as usize;
        match crate::gif_import::import_gif_scaled(path, config.image_scale) {
            Ok(gif_data) => {
                if gif_data.frames.is_empty() {
                    return;
                }
                let fh = gif_data.frames[0].len();
                let fw = if fh > 0 {
                    gif_data.frames[0][0].len()
                } else {
                    0
                };
                if fw == 0 || fh == 0 {
                    return;
                }

                self.editor.canvas = canvas::CanvasWidget::new(cw as u16, ch as u16);
                self.editor.layer_stack.resize_all(cw, ch);

                // Center the scaled image on the canvas
                let x_off = (cw.saturating_sub(fw)) / 2;
                let y_off = (ch.saturating_sub(fh)) / 2;

                // Copy first frame into active layer (centered)
                {
                    let mut buf = self.editor.layer_stack.active_layer().buffer.clone();
                    for (y, row) in gif_data.frames[0]
                        .iter()
                        .enumerate()
                        .take(fh.min(gif_data.frames[0].len()))
                    {
                        for (x, cell) in row.iter().enumerate().take(fw.min(row.len())) {
                            let dx = x_off + x;
                            let dy = y_off + y;
                            if dx < cw && dy < ch {
                                buf.set(dx, dy, *cell);
                            }
                        }
                    }
                    *self.editor.layer_stack.active_layer_mut().buffer_mut() = buf;
                }

                // Populate timeline
                let thumb_w = 8;
                let thumb_h = 3;
                self.animation.timeline_state.frames.clear();
                self.animation.timeline_state.current_frame = 0;
                self.animation
                    .timeline_state
                    .sync_layer_names(&self.editor.layer_stack);

                for (i, frame_cells) in gif_data.frames.iter().enumerate() {
                    let mut frame_buf = canvas::CanvasBuffer::new(cw, ch);
                    for (y, row) in frame_cells
                        .iter()
                        .enumerate()
                        .take(fh.min(frame_cells.len()))
                    {
                        for (x, cell) in row.iter().enumerate().take(fw.min(row.len())) {
                            let dx = x_off + x;
                            let dy = y_off + y;
                            if dx < cw && dy < ch {
                                frame_buf.set(dx, dy, *cell);
                            }
                        }
                    }

                    let thumbnail = capture_thumbnail(&frame_buf, thumb_w, thumb_h);

                    self.animation
                        .timeline_state
                        .add_frame(timeline::TimelineFrame {
                            thumbnail,
                            has_keyframe: false,
                            label: format!("F{}", i),
                            // Timing is part of frame identity (F-26): the
                            // GIF's real per-frame delay rides with the
                            // frame, so reorder/insert/delete keep it
                            // attached to its content.
                            delay: gif_data.frame_delays.get(i).copied().unwrap_or(10),
                            document_state: vec![frame_buf],
                            layer_keyframes: vec![Some(timeline::LayerKeyframe::default())],
                        });
                }

                // Store frame delays and loop count in export dialog
                self.dialogs.export_dialog.frame_delays = gif_data.frame_delays;
                self.dialogs.export_dialog.loop_count = gif_data.loop_count;
                self.dialogs.export_dialog.timeline_available = true;
                self.animation.loop_enabled = gif_data.loop_count == 0;

                let first_delay_cs = self
                    .dialogs
                    .export_dialog
                    .frame_delays
                    .first()
                    .copied()
                    .unwrap_or(10);
                self.animation.timeline_state.fps = 100u16
                    .checked_div(first_delay_cs)
                    .map(|fps| fps.clamp(1, 60) as u8)
                    .unwrap_or(10);

                self.ui.mode = AppMode::ImageEditor;
                self.animation.timeline_visible = true;
                // The GIF now owns the timeline/canvas; drop any stale
                // image-editor conversion so a later sync can't clobber
                // the imported content (GPT review F-06).
                self.editor.image_editor.clear_cells();
                self.editor.recomposite_canvas();
                self.editor.mark_dirty();
                self.frame.dirty = true;
            }
            Err(e) => {
                // Re-open the dialog with the error so user can adjust settings
                self.dialogs.gif_import.active = true;
                self.dialogs.gif_import.error_message = format!("Import failed: {e}");
            }
        }
    }

    fn perform_open_image(&mut self, path: std::path::PathBuf) {
        let path_str = path.to_string_lossy().into_owned();
        match self.editor.image_editor.load_from_path(&path_str) {
            Ok(()) => {
                self.editor.sync_image_to_canvas();
                self.editor.undo.clear();
                self.ui.mode = AppMode::ImageEditor;
                self.editor.mark_dirty();
                self.frame.dirty = true;
            }
            Err(e) => {
                self.dialogs.file_ops.error_message = format!("Image open failed: {e}");
                self.dialogs.file_ops.mode = file_ops::FileOpsMode::OpenImage;
            }
        }
    }

    fn perform_rascii_import(&mut self) {
        let cells = match self.dialogs.rascii_import.preview_cells.take() {
            Some(cells) => cells,
            None => return,
        };
        if cells.is_empty() || cells[0].is_empty() {
            return;
        }
        let h = cells.len();
        let w = cells[0].len();
        if self.editor.canvas.buffer.width() != w || self.editor.canvas.buffer.height() != h {
            self.editor.canvas = canvas::CanvasWidget::new(w as u16, h as u16);
            self.editor.layer_stack.resize_all(w, h);
        }
        let mut buf = self.editor.layer_stack.active_layer().buffer.clone();
        for (y, row) in cells.iter().enumerate() {
            for (x, cell) in row.iter().enumerate() {
                buf.set(x, y, *cell);
            }
        }
        *self.editor.layer_stack.active_layer_mut().buffer_mut() = buf;
        self.editor.image_editor.clear_cells();
        self.editor.recomposite_canvas();
        self.editor.mark_dirty();
        self.ui.mode = AppMode::ImageEditor;
        self.frame.dirty = true;
    }

    pub(crate) fn perform_export(&mut self) {
        if self.ctx.throbber.is_active() {
            return;
        }
        let w = self.editor.canvas.buffer.width();
        let h = self.editor.canvas.buffer.height();
        let cells: Vec<Vec<canvas::CanvasCell>> = (0..h)
            .map(|y| {
                (0..w)
                    .map(|x| {
                        self.editor
                            .canvas
                            .buffer
                            .get(x, y)
                            .copied()
                            .unwrap_or_default()
                    })
                    .collect()
            })
            .collect();
        let format = self.dialogs.export_dialog.format;
        let font_size = self.dialogs.export_dialog.font_size;
        let path_buf = std::path::PathBuf::from(&self.dialogs.export_dialog.path_buffer);
        let export_layers = self.dialogs.export_dialog.export_layers;
        let use_transparency = self.dialogs.export_dialog.use_transparency;
        let timeline_available = self.dialogs.export_dialog.timeline_available;
        let frame_delays = self.dialogs.export_dialog.frame_delays.clone();
        let loop_count = self.dialogs.export_dialog.loop_count;
        let layer_stack = if export_layers {
            Some(self.editor.layer_stack.clone())
        } else {
            None
        };

        // Compose timeline frames if animation format + timeline has frames.
        // Use the shared compositor (same one as preview/tests) so captured
        // frame snapshots (document_state) are respected; an earlier inline
        // copy of this logic skipped them and exported repeated copies of
        // the live canvas (GPT review F-04). ANSI is included here too —
        // previously only GIF/APNG composed, so an ANSI animation export
        // received just the *current* frame (GPT review F-26).
        let frames: Vec<Vec<Vec<canvas::CanvasCell>>> = if (format
            == crate::tui::export::ExportMode::Gif
            || format == crate::tui::export::ExportMode::Apng
            || format == crate::tui::export::ExportMode::Ansi)
            && timeline_available
            && !self.animation.timeline_state.frames.is_empty()
        {
            let ts = &self.animation.timeline_state;
            export::capture_timeline_frames(ts, &self.editor.layer_stack, w, h)
        } else {
            vec![cells.clone()]
        };

        let (tx, rx) = mpsc::channel();
        self.ctx.async_rx = Some(rx);
        self.ctx.throbber.start("Exporting...");
        self.frame.dirty = true;
        std::thread::spawn(move || {
            let result = (|| -> Result<(), String> {
                if path_buf.as_os_str().is_empty() {
                    return Err("no path specified".to_string());
                }
                let bytes: Vec<u8> = match format {
                    crate::tui::export::ExportMode::Png => {
                        if use_transparency {
                            crate::output::export_cells_to_png_with_alpha(&cells, font_size, true)
                                .map_err(|e| e.to_string())?
                        } else {
                            crate::output::export_cells_to_png(&cells, font_size)
                                .map_err(|e| e.to_string())?
                        }
                    }
                    crate::tui::export::ExportMode::Txt => {
                        crate::output::export_cells_to_txt(&cells).into_bytes()
                    }
                    crate::tui::export::ExportMode::Gif => {
                        let delay_slice: &[u16] = if timeline_available && !frame_delays.is_empty()
                        {
                            frame_delays.as_slice()
                        } else {
                            &[10]
                        };
                        crate::output::export_cells_to_gif(
                            &frames,
                            delay_slice,
                            font_size,
                            loop_count,
                        )
                        .map_err(|e| e.to_string())?
                    }
                    crate::tui::export::ExportMode::Apng => {
                        let delay_slice: &[u16] = if timeline_available && !frame_delays.is_empty()
                        {
                            frame_delays.as_slice()
                        } else {
                            &[10]
                        };
                        crate::output::export_cells_to_apng(
                            &frames,
                            delay_slice,
                            font_size,
                            loop_count,
                        )
                        .map_err(|e| e.to_string())?
                    }
                    crate::tui::export::ExportMode::Ansi => {
                        if timeline_available && !frames.is_empty() {
                            crate::output::export_cells_to_ansi_multi(&frames, &frame_delays)
                                .into_bytes()
                        } else {
                            crate::output::export_cells_to_ansi(&cells).into_bytes()
                        }
                    }
                };
                crate::atomic_io::atomic_write(&path_buf, &bytes)
                    .map_err(|e| format!("IoError({e})"))?;
                if let Some(stack) = layer_stack {
                    let mode = format;
                    if mode == crate::tui::export::ExportMode::Png {
                        crate::tui::export::ExportDialog::perform_layer_export(
                            &stack,
                            &path_buf,
                            font_size,
                            use_transparency,
                        )
                        .map_err(|e| e.to_string())?;
                    }
                }
                Ok(())
            })();
            let _ = tx.send(AsyncResult::ExportComplete(result));
        });
    }

    fn launch_player_from_export(&mut self) {
        let w = self.editor.canvas.buffer.width();
        let h = self.editor.canvas.buffer.height();
        if self.animation.timeline_state.frames.is_empty() {
            return;
        }
        let frames = export::capture_timeline_frames(
            &self.animation.timeline_state,
            &self.editor.layer_stack,
            w,
            h,
        );
        if frames.is_empty() {
            return;
        }
        // Per-frame delays ride with the timeline frames (F-26).
        let delays = self.animation.timeline_state.frame_delays();
        let fps = self.dialogs.export_dialog.fps;
        let start_frame = self.dialogs.export_dialog.preview_frame;
        self.play_standalone_preview(frames, delays, fps, start_frame);
    }

    /// Fullscreen "preview standalone" playback — takes over the whole
    /// terminal via `player::play_fullscreen`. Used only by the Export
    /// dialog's Play button, e.g. to preview how a GIF/APNG export will
    /// actually look played back outside the editor. Normal in-editor
    /// playback (Timeline Enter / Animation > Play) uses `play_inline`
    /// instead, which stays inside the canvas and leaves the rest of the
    /// editor UI visible and interactive.
    fn play_standalone_preview(
        &mut self,
        frames: Vec<Vec<Vec<canvas::CanvasCell>>>,
        delays: Vec<u16>,
        fps: u8,
        start_frame: usize,
    ) {
        let frames = if start_frame < frames.len() {
            frames[start_frame..].to_vec()
        } else {
            frames
        };
        let delays = if start_frame < delays.len() {
            delays[start_frame..].to_vec()
        } else {
            delays
        };

        if frames.is_empty() {
            return;
        }

        // Deliberately synchronous, not backgrounded onto a thread: the
        // player and the TUI both write directly to the same exclusive
        // terminal fd via crossterm/ratatui, so nothing useful could run
        // concurrently while play_fullscreen owns the terminal and its own
        // keyboard input (pause/seek/Esc/q) anyway — a background thread
        // here would add a channel + lifecycle surface purely to reproduce
        // the same blocking behavior, with a real risk of the two writers
        // racing on stdout if not synchronized carefully.
        if let Err(e) = player::play_fullscreen_timed(frames, fps, Some(delays)) {
            let _ = e;
        }

        // play_fullscreen renders through its own throwaway ratatui Terminal
        // (see its doc comment), which leaves our real Terminal's diff cache
        // stale relative to the actual screen — force a full repaint rather
        // than a diffed one on the next draw.
        self.frame.force_full_redraw = true;
        self.frame.dirty = true;
    }

    /// Capture the current timeline (at canvas resolution, so frames are
    /// already sized correctly — no scaling needed) and start playing it in
    /// place in the canvas, from the current frame.
    fn start_inline_playback_from_timeline(&mut self) {
        if self.animation.timeline_state.frames.is_empty() {
            return;
        }
        let w = self.editor.canvas.buffer.width();
        let h = self.editor.canvas.buffer.height();
        let frames = export::capture_timeline_frames(
            &self.animation.timeline_state,
            &self.editor.layer_stack,
            w,
            h,
        );
        if frames.is_empty() {
            return;
        }
        let fps = self.animation.timeline_state.fps;
        let start_frame = self.animation.timeline_state.current_frame;
        let delays = self.animation.timeline_state.frame_delays();
        self.play_inline(frames, delays, fps, start_frame);
    }

    /// Start in-canvas animation playback: `render_canvas_area` renders the
    /// player in place of normal canvas content while `inline_player` is
    /// set, and `run()`'s loop ticks it each frame — no fullscreen takeover,
    /// no separate Terminal instance, and the rest of the editor UI (menu,
    /// toolbox, palette, timeline, status bar) keeps rendering normally
    /// around it. Dismissed via Esc/q in `handle_key_event`.
    fn play_inline(
        &mut self,
        frames: Vec<Vec<Vec<canvas::CanvasCell>>>,
        delays: Vec<u16>,
        fps: u8,
        start_frame: usize,
    ) {
        if frames.is_empty() {
            return;
        }
        let player = player::AnimationPlayer::new(frames, fps.max(1))
            .with_frame_delays(delays)
            .with_loop(self.animation.loop_enabled);
        player.seek(start_frame);
        player.play();
        self.animation.inline_player = Some(player);
        self.frame.dirty = true;
    }

    /// Dismiss in-canvas animation playback. Shared by the Esc/q keyboard
    /// path and the transport bar's Stop button.
    fn stop_inline_playback(&mut self) {
        if let Some(player) = self.animation.inline_player.as_ref() {
            self.animation.timeline_state.current_frame = player.current_frame();
            self.animation.load_current_timeline_frame(&mut self.editor);
        }
        self.animation.inline_player = None;
        self.frame.dirty = true;
    }

    /// Dispatch a transport-bar button click to the same playback methods
    /// already reachable via keyboard (`AnimationPlayer::toggle_play` /
    /// `toggle_loop`, `start_inline_playback_from_timeline`,
    /// `stop_inline_playback`) — no new playback logic, just a mouse entry
    /// point into what already exists.
    fn handle_transport_button(&mut self, action: timeline::TransportButton) {
        match action {
            timeline::TransportButton::PlayPause => {
                if let Some(player) = self.animation.inline_player.as_ref() {
                    player.toggle_play();
                } else {
                    self.start_inline_playback_from_timeline();
                }
            }
            timeline::TransportButton::Stop => {
                self.stop_inline_playback();
            }
            timeline::TransportButton::Loop => {
                if let Some(player) = self.animation.inline_player.as_ref() {
                    player.toggle_loop();
                } else {
                    self.animation.loop_enabled = !self.animation.loop_enabled;
                }
            }
        }
        self.frame.dirty = true;
    }

    pub(crate) fn handle_menu_action(&mut self, action: menu::MenuAction) {
        self.frame.dirty = true;
        match action {
            menu::MenuAction::FileNew => {
                self.dialogs.new_image.enter_new_image();
                self.ui.menu_bar_state.reset();
            }
            menu::MenuAction::FontNewFromFile => {
                self.start_font_import_from_file();
                self.ui.menu_bar_state.reset();
            }
            menu::MenuAction::FontNewFromSystem => {
                self.dialogs.system_font.enter();
                self.ui.menu_bar_state.reset();
            }
            menu::MenuAction::FileOpen => {
                self.start_open();
                self.ui.menu_bar_state.reset();
            }
            menu::MenuAction::FileSave => {
                self.start_save();
                self.ui.menu_bar_state.reset();
            }
            menu::MenuAction::FileSaveAs => {
                self.start_save_as();
                self.ui.menu_bar_state.reset();
            }
            menu::MenuAction::FileExport => {
                let mode = match self.ui.mode {
                    AppMode::FontEditor => export::ExportMode::Txt,
                    _ => export::ExportMode::Png,
                };
                self.dialogs.export_dialog.enter_export(mode);
                if (mode == export::ExportMode::Gif
                    || mode == export::ExportMode::Apng
                    || mode == export::ExportMode::Ansi)
                    && !self.animation.timeline_state.frames.is_empty()
                {
                    self.dialogs
                        .export_dialog
                        .set_timeline_from_timeline(&self.animation.timeline_state);
                }
                self.ui.menu_bar_state.reset();
            }
            menu::MenuAction::FileImportGif => {
                self.dialogs.gif_import.enter();
                self.ui.menu_bar_state.reset();
            }
            menu::MenuAction::FileQuit => {
                self.trigger_quit();
                self.ui.menu_bar_state.reset();
            }
            menu::MenuAction::EditUndo => {
                self.perform_undo();
                self.ui.menu_bar_state.reset();
            }
            menu::MenuAction::EditRedo => {
                self.perform_redo();
                self.ui.menu_bar_state.reset();
            }
            menu::MenuAction::EditCut => {
                if let Some(ref sel) = self.editor.selection {
                    if sel.is_active() && !self.editor.active_layer_locked() {
                        self.editor.push_undo_snapshot("Cut selection");
                        if let Some(sel_owned) = self.editor.selection.take() {
                            let mut buf = self.editor.layer_stack.active_layer().buffer.clone();
                            self.editor.clipboard = Some(sel_owned.cut_from(&mut buf));
                            *self.editor.layer_stack.active_layer_mut().buffer_mut() = buf;
                            self.editor.recomposite_canvas();
                            self.editor.mark_dirty();
                        }
                    }
                }
                self.ui.menu_bar_state.reset();
            }
            menu::MenuAction::EditCopy => {
                if let Some(ref sel) = self.editor.selection {
                    if sel.is_active() {
                        self.editor.clipboard = Some(sel.copy_from(&self.editor.canvas.buffer));
                    }
                }
                self.ui.menu_bar_state.reset();
            }
            menu::MenuAction::EditPaste => {
                if self.editor.clipboard.is_some() && !self.editor.active_layer_locked() {
                    self.editor.push_undo_snapshot("Paste");
                    let clip = self.editor.clipboard.clone();
                    if let Some(ref clip_data) = clip {
                        let (cx, cy) = self.editor.canvas.cursor();
                        let mut buf = self.editor.layer_stack.active_layer().buffer.clone();
                        tools::selection::Selection::paste_into(
                            &mut buf, clip_data, cx as i16, cy as i16,
                        );
                        *self.editor.layer_stack.active_layer_mut().buffer_mut() = buf;
                        self.editor.recomposite_canvas();
                        self.editor.mark_dirty();
                    }
                }
                self.ui.menu_bar_state.reset();
            }
            menu::MenuAction::ViewZoomIn => {
                if self.editor.canvas.zoom_level() < 8 {
                    self.editor.canvas.zoom_in();
                }
                self.ui.menu_bar_state.reset();
            }
            menu::MenuAction::ViewZoomOut => {
                if self.editor.canvas.zoom_level() > 1 {
                    self.editor.canvas.zoom_out();
                }
                self.ui.menu_bar_state.reset();
            }
            menu::MenuAction::ViewToggleGrid => {
                self.editor.canvas.toggle_grid();
                self.ui.menu_bar_state.reset();
            }
            menu::MenuAction::ViewToggleUndoPanel => {
                self.dialogs.undo_panel.toggle();
                self.ui.menu_bar_state.reset();
            }
            menu::MenuAction::ViewToggleTimeline => {
                self.animation.timeline_visible = !self.animation.timeline_visible;
                self.frame.dirty = true;
                self.ui.menu_bar_state.reset();
            }
            menu::MenuAction::ViewToggleSidePanel => {
                self.side_panel.toggle_open();
                self.frame.dirty = true;
                self.ui.menu_bar_state.reset();
            }
            menu::MenuAction::ToolsSelect(tool) => {
                self.editor.toolbox.selected = tool;
                if tool != toolbox::Tool::PolygonSelect {
                    self.editor.selection_polygon_points.clear();
                }
                self.ui.menu_bar_state.reset();
            }
            menu::MenuAction::ViewLoadBuiltinPalette(name) => {
                let palettes = crate::palette_import::builtin_palettes();
                if let Some((_, swatches)) = palettes.into_iter().find(|(n, _)| *n == name) {
                    self.palette_editor.swatches = swatches;
                    self.palette_editor.name_buffer = name.to_string();
                    self.palette_editor.open = true;
                }
                self.ui.menu_bar_state.reset();
                self.frame.dirty = true;
            }
            menu::MenuAction::ViewPaletteEditor => {
                self.palette_editor.open = !self.palette_editor.open;
                if self.palette_editor.open {
                    self.palette_editor
                        .load_current_from_palette(&self.editor.palette);
                    self.palette_editor.available_palettes(None);
                    self.palette_editor.lighting_pickers_visible =
                        self.ui.mode == AppMode::Lighting || self.lighting.scene.is_some();
                }
                self.ui.menu_bar_state.reset();
                self.frame.dirty = true;
            }
            menu::MenuAction::LayerNew => {
                let w = self.editor.layer_stack.layers[0].buffer.width();
                let h = self.editor.layer_stack.layers[0].buffer.height();
                self.editor.layer_stack.add(w, h);
                self.editor.recomposite_canvas();
                self.ui.menu_bar_state.reset();
                self.frame.dirty = true;
            }
            menu::MenuAction::LayerDuplicate => {
                let idx = self.editor.layer_stack.active;
                self.editor.layer_stack.duplicate(idx);
                self.editor.recomposite_canvas();
                self.ui.menu_bar_state.reset();
                self.frame.dirty = true;
            }
            menu::MenuAction::LayerDelete => {
                let idx = self.editor.layer_stack.active;
                self.editor.layer_stack.delete(idx);
                self.editor.recomposite_canvas();
                self.ui.menu_bar_state.reset();
                self.frame.dirty = true;
            }
            menu::MenuAction::LayerMergeDown => {
                let idx = self.editor.layer_stack.active;
                self.editor.layer_stack.merge_down(idx);
                self.editor.recomposite_canvas();
                self.ui.menu_bar_state.reset();
                self.frame.dirty = true;
            }
            menu::MenuAction::LayerMoveUp => {
                let idx = self.editor.layer_stack.active;
                if self.editor.layer_stack.move_up(idx) {
                    self.editor.layer_stack.active = idx.saturating_sub(1);
                    self.editor.recomposite_canvas();
                }
                self.ui.menu_bar_state.reset();
                self.frame.dirty = true;
            }
            menu::MenuAction::LayerMoveDown => {
                let idx = self.editor.layer_stack.active;
                if self.editor.layer_stack.move_down(idx) {
                    let new_idx = (idx + 1).min(self.editor.layer_stack.layers.len() - 1);
                    self.editor.layer_stack.active = new_idx;
                    self.editor.recomposite_canvas();
                }
                self.ui.menu_bar_state.reset();
                self.frame.dirty = true;
            }
            menu::MenuAction::LayerToggleVisibility => {
                let idx = self.editor.layer_stack.active;
                self.editor.layer_stack.toggle_visibility(idx);
                self.editor.recomposite_canvas();
                self.ui.menu_bar_state.reset();
                self.frame.dirty = true;
            }
            menu::MenuAction::LayerToggleLock => {
                let idx = self.editor.layer_stack.active;
                self.editor.layer_stack.toggle_lock(idx);
                self.ui.menu_bar_state.reset();
                self.frame.dirty = true;
            }
            menu::MenuAction::AnimFrameAdd => {
                let buffer = self.editor.canvas.buffer.clone();
                let thumbnail = capture_thumbnail(&buffer, 12, 6);
                let layer_keyframes = self
                    .editor
                    .layer_stack
                    .layers
                    .iter()
                    .map(|_| Some(timeline::LayerKeyframe::default()))
                    .collect();
                let new_frame = timeline::TimelineFrame {
                    thumbnail,
                    has_keyframe: true,
                    label: format!("F{}", self.animation.timeline_state.frames.len()),
                    delay: self.animation.timeline_state.default_delay(),
                    document_state: vec![buffer],
                    layer_keyframes,
                };
                self.animation
                    .timeline_state
                    .sync_layer_names(&self.editor.layer_stack);
                self.animation.timeline_state.add_frame(new_frame);
                self.ui.menu_bar_state.reset();
                self.frame.dirty = true;
            }
            menu::MenuAction::AnimFrameDelete => {
                let cur = self.animation.timeline_state.current_frame;
                let _ = self.animation.timeline_state.remove_frame(cur);
                self.ui.menu_bar_state.reset();
                self.frame.dirty = true;
            }
            menu::MenuAction::AnimPlay => {
                // Was previously a no-op: it toggled TimelineState::playing,
                // a field nothing else ever reads. Now actually starts
                // in-canvas playback, same as pressing Enter on the timeline.
                self.start_inline_playback_from_timeline();
                self.ui.menu_bar_state.reset();
                self.frame.dirty = true;
            }
            menu::MenuAction::AnimToggleTimeline => {
                self.animation.timeline_visible = !self.animation.timeline_visible;
                self.ui.menu_bar_state.reset();
                self.frame.dirty = true;
            }
            menu::MenuAction::ImageResizeCanvas => {
                self.dialogs.settings.canvas_width = self.editor.canvas.buffer.width() as u16;
                self.dialogs.settings.canvas_height = self.editor.canvas.buffer.height() as u16;
                self.dialogs.settings.settings_open = true;
                self.ui.menu_bar_state.reset();
                self.frame.dirty = true;
            }
            menu::MenuAction::HelpAbout => {
                self.ui.menu_bar_state.reset();
            }
            menu::MenuAction::HelpKeybindings => {
                self.ui.menu_bar_state.reset();
                self.ui.show_keybindings = true;
                self.frame.dirty = true;
            }
        }
    }

    fn apply_settings(&mut self) {
        let w = self.dialogs.settings.canvas_width as usize;
        let h = self.dialogs.settings.canvas_height as usize;
        // F-12: the settings dialog clamps to 200×200, but a canvas already at
        // an oversized size could round-trip here — refuse beyond budget.
        if !canvas::canvas_cells_within_budget(w, h) {
            self.frame.dirty = true;
            return;
        }
        if self.editor.canvas.buffer.width() != w || self.editor.canvas.buffer.height() != h {
            self.editor.canvas = canvas::CanvasWidget::new(w as u16, h as u16);
            self.editor.layer_stack.resize_all(w, h);
            self.editor.recomposite_canvas();
            self.editor.undo.clear();
        }
        if self.dialogs.settings.show_grid != self.editor.canvas.show_grid() {
            self.editor.canvas.toggle_grid();
        }
        self.frame.dirty = true;
    }
}

#[cfg(test)]
mod playback_reconciliation_tests {
    use super::*;
    use std::time::Duration;

    fn make_app_with_frames(frame_count: usize) -> TuiApp {
        let mut app = TuiApp::new();
        let w = app.editor.canvas.buffer.width();
        let h = app.editor.canvas.buffer.height();
        for i in 0..frame_count {
            let mut buf = canvas::CanvasBuffer::new(w, h);
            let ch = char::from_u32(b'A' as u32 + (i % 26) as u32).unwrap();
            buf.set(
                0,
                0,
                canvas::CanvasCell {
                    ch,
                    fg: None,
                    bg: None,
                    height: None,
                },
            );
            let frame = timeline::TimelineFrame {
                thumbnail: capture_thumbnail(&buf, 8, 3),
                has_keyframe: true,
                label: format!("F{}", i),
                delay: 10,
                document_state: vec![buf],
                layer_keyframes: Vec::new(),
            };
            app.animation.timeline_state.add_frame(frame);
        }
        app
    }

    #[test]
    fn test_tick_syncs_timeline_frame_to_player_frame() {
        let mut app = make_app_with_frames(5);
        app.start_inline_playback_from_timeline();
        assert!(app.animation.inline_player.is_some());

        let player_frame = {
            let player = app.animation.inline_player.as_ref().unwrap();
            player.advance(Duration::from_millis(200));
            player.current_frame()
        };

        app.animation.timeline_state.current_frame = player_frame;

        assert_eq!(app.animation.timeline_state.current_frame, player_frame);
        assert!(player_frame > 0, "player should have advanced past frame 0");
    }

    #[test]
    fn test_stop_preserves_last_rendered_frame() {
        let mut app = make_app_with_frames(5);
        app.animation.timeline_state.current_frame = 2;
        app.start_inline_playback_from_timeline();
        assert!(app.animation.inline_player.is_some());

        let last_frame = {
            let player = app.animation.inline_player.as_ref().unwrap();
            player.advance(Duration::from_millis(100));
            player.current_frame()
        };

        app.stop_inline_playback();

        assert_eq!(app.animation.timeline_state.current_frame, last_frame);
        assert!(app.animation.inline_player.is_none());

        let expected_ch = char::from_u32(b'A' as u32 + (last_frame % 26) as u32).unwrap();
        assert_eq!(
            app.editor.canvas.buffer.get(0, 0).unwrap().ch,
            expected_ch,
            "canvas should hold the last rendered frame content"
        );
    }
}

#[cfg(test)]
mod sidebar_keybindings_tests {
    use super::*;
    use crate::tui::side_panel::TabId;
    use crossterm::event::KeyEvent;

    fn app_with_frames(frame_count: usize) -> TuiApp {
        let mut app = TuiApp::new();
        // TuiApp::new() defaults to FontEditor mode, which intercepts arrow
        // keys in the font overview grid. Switch to AsciiPreview so that key
        // events reach the sidebar/timeline/layer-panel handlers we're testing.
        app.ui.mode = AppMode::AsciiPreview;
        let w = app.editor.canvas.buffer.width();
        let h = app.editor.canvas.buffer.height();
        for i in 0..frame_count {
            let mut buf = canvas::CanvasBuffer::new(w, h);
            let ch = char::from_u32(b'A' as u32 + (i % 26) as u32).unwrap();
            buf.set(
                0,
                0,
                canvas::CanvasCell {
                    ch,
                    fg: None,
                    bg: None,
                    height: None,
                },
            );
            let frame = timeline::TimelineFrame {
                thumbnail: capture_thumbnail(&buf, 8, 3),
                has_keyframe: true,
                label: format!("F{}", i),
                delay: 10,
                document_state: vec![buf],
                layer_keyframes: Vec::new(),
            };
            app.animation.timeline_state.add_frame(frame);
        }
        app.side_panel.open = true;
        app.side_panel.active_tab = TabId::Layers;
        app.animation.timeline_visible = true;
        app
    }

    #[test]
    fn test_bare_arrow_right_advances_timeline_with_sidebar_open() {
        let mut app = app_with_frames(3);
        let current_frame = app.animation.timeline_state.current_frame;

        eprintln!(
            "DEBUG: frames={}, current_frame={}, timeline_visible={}, side_panel.open={}, active_tab={:?}",
            app.animation.timeline_state.frames.len(),
            current_frame,
            app.animation.timeline_visible,
            app.side_panel.open,
            app.side_panel.active_tab,
        );

        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));

        eprintln!(
            "DEBUG after: current_frame={}",
            app.animation.timeline_state.current_frame,
        );

        assert_eq!(
            app.animation.timeline_state.current_frame,
            current_frame + 1,
            "bare Right should advance timeline frame even with sidebar open"
        );
        assert_eq!(
            app.side_panel.active_tab,
            TabId::Layers,
            "bare Right must not change side-panel tab"
        );
    }

    #[test]
    fn test_alt_left_right_cycles_sidebar_tabs() {
        let mut app = app_with_frames(3);
        let orig_frame = app.animation.timeline_state.current_frame;

        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Right, KeyModifiers::ALT));

        assert_eq!(
            app.side_panel.active_tab,
            TabId::Props,
            "Alt+Right should cycle side-panel tab forward"
        );
        assert_eq!(
            app.animation.timeline_state.current_frame, orig_frame,
            "Alt+Right must not advance timeline"
        );

        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Left, KeyModifiers::ALT));

        assert_eq!(
            app.side_panel.active_tab,
            TabId::Layers,
            "Alt+Left should cycle side-panel tab backward"
        );
    }

    #[test]
    fn test_bare_up_down_does_nothing_in_layer_panel() {
        let mut app = app_with_frames(1);
        let orig_active = app.editor.layer_stack.active;

        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
        assert_eq!(app.editor.layer_stack.active, orig_active);

        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(app.editor.layer_stack.active, orig_active);
    }

    #[test]
    fn test_tab_cycles_mode_with_layer_groups() {
        let mut app = app_with_frames(1);
        let w = app.editor.canvas.buffer.width();
        let h = app.editor.canvas.buffer.height();
        app.editor.layer_stack.add(w, h);
        app.editor.layer_stack.add(w, h);
        let indices = [0, 1];
        app.editor
            .layer_stack
            .create_group(&indices, "TestGroup".to_string());
        let orig_mode = app.ui.mode;

        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));

        assert!(
            app.ui.mode != orig_mode,
            "bare Tab must cycle app mode even when layer groups exist"
        );
    }

    #[test]
    fn test_alt_s_toggles_cast_shadow() {
        let mut app = app_with_frames(1);
        let layer_idx = app.editor.layer_stack.active;
        let has_shadow_before = app.editor.layer_stack.layers[layer_idx].casts_shadow;

        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Char('S'), KeyModifiers::NONE));
        assert!(
            app.dialogs.settings.settings_open,
            "bare S must open settings dialog"
        );
        assert_eq!(
            app.editor.layer_stack.layers[layer_idx].casts_shadow, has_shadow_before,
            "bare S must not toggle cast shadow"
        );

        app.dialogs.settings.settings_open = false;

        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Char('S'), KeyModifiers::ALT));
        assert_eq!(
            app.editor.layer_stack.layers[layer_idx].casts_shadow, !has_shadow_before,
            "Alt+S must toggle layer cast shadow"
        );
    }
}
