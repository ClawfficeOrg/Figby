//! Mouse routing: each focused component gets the event in priority order.
//!
//! `handle_mouse_event` is only a router. Every `mouse_*` handler returns
//! `true` when it consumed the event; the first one that does ends routing.
//! Canvas tools come last and live in `canvas_mouse.rs`.

use super::*;

impl TuiApp {
    pub(crate) fn handle_mouse_event(&mut self, mouse: MouseEvent) {
        // Modal layers and full-screen views, highest priority first.
        if self.mouse_quit_prompt(mouse)
            || self.mouse_menu_bar(mouse)
            || self.mouse_doc_tabs(mouse)
            || self.mouse_welcome(mouse)
            || self.mouse_modal_dialogs(mouse)
            || self.mouse_font_overview(mouse)
            || self.mouse_image_editor(mouse)
        {
            return;
        }

        let fl = self.mouse_frame_layout();
        let canvas_inner_rect = self.editor.compute_canvas_rect(
            ratatui::widgets::Block::default()
                .borders(fl.canvas_borders())
                .inner(fl.canvas),
        );

        // Docked panels around the canvas.
        if self.mouse_side_panel_tabs(mouse, &fl)
            || self.mouse_layer_panel(mouse, &fl)
            || self.mouse_props_panel(mouse)
            || self.mouse_timeline(mouse, &fl)
        {
            return;
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

        if self.mouse_toolbox(mouse, &fl) || self.mouse_palette(mouse, &fl) {
            return;
        }

        if self.mouse_text_tool(mouse, canvas_inner_rect) {
            return;
        }
        self.mouse_canvas_tools(mouse, canvas_inner_rect);
    }

    /// Unsaved-changes dialog: intercepts all mouse events while open.
    fn mouse_quit_prompt(&mut self, mouse: MouseEvent) -> bool {
        if !self.dialogs.quit_confirm_dialog {
            return false;
        }
        if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            let btns = self.dialogs.quit_confirm_buttons;
            let can_save = self.can_save_now();
            if can_save && btns[0].contains((mouse.column, mouse.row).into()) {
                self.dialogs.quit_confirm_dialog = false;
                let is_quit = self.dialogs.pending_transition == Some(PendingDocAction::Quit);
                if is_quit {
                    self.dialogs.quit_after_save = true;
                }
                self.frame.dirty = true;
                self.start_save();
            } else if btns[1].contains((mouse.column, mouse.row).into()) {
                // Discard & proceed — only ever from the [N] button.
                self.dialogs.quit_confirm_dialog = false;
                self.frame.dirty = true;
                self.execute_pending_transition();
            } else if btns[2].contains((mouse.column, mouse.row).into()) {
                self.dialogs.quit_confirm_dialog = false;
                self.dialogs.pending_transition = None;
                self.frame.dirty = true;
            }
        }
        true
    }

    /// Menu bar: open/close menus and run the chosen action.
    fn mouse_menu_bar(&mut self, mouse: MouseEvent) -> bool {
        if !self.ui.menu_bar.handle_mouse_event(
            mouse.column,
            mouse.row,
            mouse.kind,
            &mut self.ui.menu_bar_state,
        ) {
            return false;
        }
        if let Some(action) = self.ui.menu_bar_state.drain_actions() {
            self.process_event(&AppEvent::Menu(action));
        }
        true
    }

    /// Document tab strip: left-click switches tabs (8.5.2). Rects are
    /// stored every render next to what's drawn, same pattern as the
    /// transport bar and quit-confirm buttons.
    fn mouse_doc_tabs(&mut self, mouse: MouseEvent) -> bool {
        if mouse.kind != MouseEventKind::Down(MouseButton::Left) {
            return false;
        }
        let pos = (mouse.column, mouse.row).into();
        let hit = self.ui.tab_rects.iter().position(|r| r.contains(pos));
        match hit {
            Some(i) => {
                self.switch_document(i);
                true
            }
            None => false,
        }
    }

    /// Welcome screen captures all mouse events while visible.
    fn mouse_welcome(&mut self, mouse: MouseEvent) -> bool {
        if !self.welcome.screen.show {
            return false;
        }
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
        true
    }

    /// Settings, file-ops, export and rascii dialogs. Only the file-ops
    /// dialog takes mouse input; the others just block the canvas.
    fn mouse_modal_dialogs(&mut self, mouse: MouseEvent) -> bool {
        if self.dialogs.settings.settings_open {
            return true;
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
            return true;
        }

        self.dialogs.export_dialog.active || self.dialogs.rascii_import.active
    }

    /// Font editor overview: glyph grid mouse click + scroll.
    fn mouse_font_overview(&mut self, mouse: MouseEvent) -> bool {
        if self.ui.mode != AppMode::FontEditor
            || self.editor.font_editor.view != font_editor::FontEditorView::Overview
        {
            return false;
        }
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left)
                if self
                    .editor
                    .font_editor
                    .handle_mouse_click_overview(mouse.column, mouse.row) =>
            {
                self.frame.dirty = true;
                true
            }
            MouseEventKind::ScrollDown => {
                self.editor.font_editor.handle_mouse_scroll_overview(1);
                self.frame.dirty = true;
                true
            }
            MouseEventKind::ScrollUp => {
                self.editor.font_editor.handle_mouse_scroll_overview(-1);
                self.frame.dirty = true;
                true
            }
            _ => false,
        }
    }

    /// Image editor: state-dependent mouse events (path entry, error box).
    /// Adjustment mode and other states fall through to the general handlers.
    fn mouse_image_editor(&mut self, mouse: MouseEvent) -> bool {
        if self.ui.mode != AppMode::ImageEditor {
            return false;
        }
        if self.editor.image_editor.entering_path() {
            // Swallow all mouse events while user is typing a file path
            self.frame.dirty = true;
            return true;
        }
        if self.editor.image_editor.error_message().is_some() {
            if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
                self.editor.image_editor.clear_error();
                self.frame.dirty = true;
                return true;
            }
        }
        false
    }

    /// Screen layout the mouse handlers hit-test against (same computation
    /// the renderer uses).
    fn mouse_frame_layout(&self) -> layout::FrameLayout {
        let (cols, rows) = ratatui::crossterm::terminal::size().unwrap_or((80, 24));
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
    }

    /// Side panel tab label click.
    fn mouse_side_panel_tabs(&mut self, mouse: MouseEvent, fl: &layout::FrameLayout) -> bool {
        let Some(rp) = fl.right_panel else {
            return false;
        };
        if mouse.kind != MouseEventKind::Down(MouseButton::Left) {
            return false;
        }
        let Some(tab) = self.side_panel.tab_at_pos(mouse.column, mouse.row, rp) else {
            return false;
        };
        self.side_panel.set_active_tab(tab);
        // Activate text editing when opening Text tab
        if tab == TabId::Text && self.editor.toolbox.selected == Tool::Text {
            self.editor.text_tool.editing = true;
        }
        self.frame.dirty = true;
        true
    }

    /// Layer panel: click/drag/scroll on layer rows.
    fn mouse_layer_panel(&mut self, mouse: MouseEvent, fl: &layout::FrameLayout) -> bool {
        let Some(rp) = fl.right_panel else {
            return false;
        };
        if !(self.side_panel.open
            && self.side_panel.active_tab == TabId::Layers
            && self.editor.layer_panel.handle_mouse(
                mouse.column,
                mouse.row,
                mouse.kind,
                self.side_panel.content_area(rp),
                &mut self.editor.layer_stack,
            ))
        {
            return false;
        }
        // Wheel scroll only changes the view, not the document.
        if !matches!(
            mouse.kind,
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
        ) {
            self.editor.recomposite_canvas();
            self.editor.mark_dirty();
        }
        self.frame.dirty = true;
        true
    }

    /// Props and Text tabs: click on widget rects.
    fn mouse_props_panel(&mut self, mouse: MouseEvent) -> bool {
        if !self.side_panel.open
            || mouse.kind != MouseEventKind::Down(MouseButton::Left)
            || !matches!(self.side_panel.active_tab, TabId::Props | TabId::Text)
        {
            return false;
        }
        let Some(action) = self.props_panel.handle_click(mouse.column, mouse.row) else {
            return false;
        };
        self.dispatch_props_action(action);
        self.frame.dirty = true;
        true
    }

    /// Timeline: transport buttons, click-to-seek, ghost-frame fill and
    /// mouse-wheel scroll.
    fn mouse_timeline(&mut self, mouse: MouseEvent, fl: &layout::FrameLayout) -> bool {
        let Some(timeline_rect) = fl.timeline else {
            return false;
        };
        if !(self.animation.timeline_visible
            && mouse.column >= timeline_rect.x
            && mouse.column < timeline_rect.x + timeline_rect.width
            && mouse.row >= timeline_rect.y
            && mouse.row < timeline_rect.y + timeline_rect.height)
        {
            return false;
        }
        let block_inner = ratatui::widgets::Block::default()
            .borders(fl.timeline_borders())
            .inner(timeline_rect);
        if block_inner.height < 5 {
            return false;
        }
        let (grid_area, _toolbar_area) = timeline::AnimationTimeline::split_area(block_inner);
        let anim_timeline = timeline::AnimationTimeline::panel_instance();
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(&(action, _)) = self.animation.transport_rects.iter().find(|(_, r)| {
                    mouse.column >= r.x && mouse.column < r.x + r.width && mouse.row == r.y
                }) {
                    self.handle_transport_button(action);
                    return true;
                }
                if let Some(idx) = anim_timeline.frame_at_col(
                    mouse.column,
                    grid_area,
                    &self.animation.timeline_state,
                ) {
                    if anim_timeline.is_ghost_idx(idx, &self.animation.timeline_state) {
                        // Left-click a ghost: fill plain clones
                        // up to it, then land on it.
                        self.timeline_fill_ghosts_to(idx, false);
                        return true;
                    }
                    self.animation.commit_current_timeline_frame(&self.editor);
                    self.animation.timeline_state.current_frame = idx;
                    self.animation.load_current_timeline_frame(&mut self.editor);
                    self.editor.sync_canvas_to_font_char();
                    self.frame.dirty = true;
                    return true;
                }
                false
            }
            MouseEventKind::Down(MouseButton::Right) => {
                match anim_timeline.frame_at_col(
                    mouse.column,
                    grid_area,
                    &self.animation.timeline_state,
                ) {
                    Some(idx)
                        if anim_timeline.is_ghost_idx(idx, &self.animation.timeline_state) =>
                    {
                        // Right-click a ghost: same fill, but
                        // stamped as keyframes.
                        self.timeline_fill_ghosts_to(idx, true);
                        true
                    }
                    _ => false,
                }
            }
            MouseEventKind::ScrollUp => {
                let ts = &mut self.animation.timeline_state;
                if mouse.modifiers.contains(KeyModifiers::SHIFT) {
                    ts.layer_row_offset = ts.layer_row_offset.saturating_sub(1);
                } else {
                    ts.scroll_offset = ts.scroll_offset.saturating_sub(1);
                }
                self.frame.dirty = true;
                true
            }
            MouseEventKind::ScrollDown => {
                let ts = &mut self.animation.timeline_state;
                if mouse.modifiers.contains(KeyModifiers::SHIFT) {
                    let max_row = ts.layer_names.len().saturating_sub(1);
                    ts.layer_row_offset = (ts.layer_row_offset + 1).min(max_row);
                } else {
                    let max_scroll = ts
                        .frames
                        .len()
                        .saturating_sub(ts.cached_max_vis_frames.max(1));
                    ts.scroll_offset = (ts.scroll_offset + 1).min(max_scroll);
                }
                self.frame.dirty = true;
                true
            }
            _ => false,
        }
    }

    /// Fill ghost frames up to `idx` (as keyframes when `keyframes`), then
    /// land on it.
    fn timeline_fill_ghosts_to(&mut self, idx: usize, keyframes: bool) {
        self.animation.commit_current_timeline_frame(&self.editor);
        self.animation.timeline_state.fill_ghosts_to(idx, keyframes);
        self.animation.timeline_state.current_frame = idx;
        self.animation
            .timeline_state
            .sync_layer_names(&self.editor.layer_stack);
        self.animation.load_current_timeline_frame(&mut self.editor);
        self.editor.sync_canvas_to_font_char();
        self.frame.dirty = true;
    }

    /// Toolbox click: select tool by row.
    fn mouse_toolbox(&mut self, mouse: MouseEvent, fl: &layout::FrameLayout) -> bool {
        let Some(tb) = fl.toolbox_list else {
            return false;
        };
        let tool_count = Tool::all().len() as u16;
        let toolbox_inner_y = tb.y + 1;
        if !(mouse.kind == MouseEventKind::Down(MouseButton::Left)
            && mouse.column >= tb.x
            && mouse.column < tb.x + tb.width
            && mouse.row >= toolbox_inner_y
            && mouse.row < toolbox_inner_y + tool_count)
        {
            return false;
        }
        let idx = (mouse.row - toolbox_inner_y) as usize;
        let tools = Tool::all();
        if idx < tools.len() {
            self.editor.toolbox.selected = tools[idx];
            self.editor.selection_polygon_points.clear();
        }
        true
    }

    /// Palette editor overlay, palette swatch clicks and swatch hover.
    /// Hover never consumes the event.
    fn mouse_palette(&mut self, mouse: MouseEvent, fl: &layout::FrameLayout) -> bool {
        // Palette editor overlay open: swallow ALL mouse button clicks
        // so the canvas underneath never paints (no pixels, no undo
        // entry). Outside clicks dismiss the overlay; inside clicks
        // are consumed for future swatch handling.
        if self.palette_editor.open
            && matches!(
                mouse.kind,
                MouseEventKind::Down(_) | MouseEventKind::Drag(_) | MouseEventKind::Up(_)
            )
        {
            let (cols, rows) = ratatui::crossterm::terminal::size().unwrap_or((80, 24));
            let full = Rect::new(0, 0, cols, rows);
            self.palette_editor
                .handle_overlay_click(mouse.column, mouse.row, full);
            self.frame.dirty = true;
            return true;
        }
        if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            if let Some(palette_rect) = fl.palette {
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
                    return true;
                }
            }
        }

        // Palette hover (mouse move over swatches)
        if mouse.kind == MouseEventKind::Moved {
            if let Some(palette_rect) = fl.palette {
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
        false
    }
}
