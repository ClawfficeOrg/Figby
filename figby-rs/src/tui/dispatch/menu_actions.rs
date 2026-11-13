//! Menu bar actions and the settings dialog.
//!
//! `handle_menu_action` is a router: it hands each action to the handler for
//! its menu (File, Edit, View, Tools, Layer, Animation, Image/Help) and then
//! closes the menu bar.

use super::*;

const MISROUTED: &str = "menu action routed to the wrong handler";

impl TuiApp {
    pub(crate) fn handle_menu_action(&mut self, action: menu::MenuAction) {
        use menu::MenuAction as M;
        self.frame.dirty = true;
        match action {
            M::FileNew
            | M::FontNewFromFile
            | M::FontNewFromSystem
            | M::FileOpen
            | M::FileSave
            | M::FileSaveAs
            | M::FileSaveAsFigmap
            | M::FileNewTabFont
            | M::FileNewTabImage
            | M::FileNewAnimation
            | M::FileCloseTab
            | M::FileExport
            | M::FileImportGif
            | M::FileQuit => self.handle_file_menu_action(action),
            M::EditUndo | M::EditRedo | M::EditCut | M::EditCopy | M::EditPaste => {
                self.handle_edit_menu_action(action)
            }
            M::ViewZoomIn
            | M::ViewZoomOut
            | M::ViewToggleGrid
            | M::ViewToggleUndoPanel
            | M::ViewToggleTimeline
            | M::ViewToggleSidePanel
            | M::ViewLoadBuiltinPalette(_)
            | M::ViewPaletteEditor => self.handle_view_menu_action(action),
            M::ToolsSelect(tool) => self.handle_tools_menu_action(tool),
            M::LayerNew
            | M::LayerDuplicate
            | M::LayerDelete
            | M::LayerMergeDown
            | M::LayerMoveUp
            | M::LayerMoveDown
            | M::LayerToggleVisibility
            | M::LayerToggleLock => self.handle_layer_menu_action(action),
            M::AnimFrameAdd | M::AnimFrameDelete | M::AnimPlay | M::AnimToggleTimeline => {
                self.handle_anim_menu_action(action)
            }
            M::ImageResizeCanvas | M::HelpAbout | M::HelpKeybindings => {
                self.handle_dialog_menu_action(action)
            }
        }
        // Every menu action closes the menu bar once it has run.
        self.ui.menu_bar_state.reset();
    }

    /// File menu (and the Font > New entries that live under it): new, open,
    /// save, tabs, export, import, quit.
    pub(super) fn handle_file_menu_action(&mut self, action: menu::MenuAction) {
        use menu::MenuAction as M;
        match action {
            M::FileNew => self.dialogs.new_image.enter_new_image(),
            M::FontNewFromFile => self.start_font_import_from_file(),
            M::FontNewFromSystem => self.dialogs.system_font.enter(),
            M::FileOpen => self.start_open(),
            M::FileSave => self.start_save(),
            M::FileSaveAs => self.start_save_as(),
            M::FileSaveAsFigmap => self.start_save_as_figmap(),
            M::FileNewTabFont => {
                self.new_document(crate::tui::documents::DocumentKind::Font);
            }
            M::FileNewTabImage => {
                self.new_document(crate::tui::documents::DocumentKind::Image);
            }
            M::FileNewAnimation => {
                // 8.6.3 leftover, unblocked by 8.5: a fresh image tab with
                // the timeline panel already open, ready to add frames.
                self.new_document(crate::tui::documents::DocumentKind::Image);
                self.animation.timeline_visible = true;
            }
            M::FileCloseTab => self.request_close_document(self.active_doc),
            M::FileExport => {
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
            }
            M::FileImportGif => self.dialogs.gif_import.enter(),
            M::FileQuit => self.trigger_quit(),
            _ => unreachable!("{MISROUTED}: {action:?}"),
        }
    }

    /// Edit menu: undo, redo and the selection clipboard.
    pub(super) fn handle_edit_menu_action(&mut self, action: menu::MenuAction) {
        use menu::MenuAction as M;
        match action {
            M::EditUndo => self.perform_undo(),
            M::EditRedo => self.perform_redo(),
            M::EditCut => {
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
            }
            M::EditCopy => {
                if let Some(ref sel) = self.editor.selection {
                    if sel.is_active() {
                        self.editor.clipboard = Some(sel.copy_from(&self.editor.canvas.buffer));
                    }
                }
            }
            M::EditPaste => {
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
            }
            _ => unreachable!("{MISROUTED}: {action:?}"),
        }
    }

    /// View menu: zoom, grid, panel toggles and the palette editor.
    pub(super) fn handle_view_menu_action(&mut self, action: menu::MenuAction) {
        use menu::MenuAction as M;
        match action {
            M::ViewZoomIn => {
                if self.editor.canvas.zoom_level() < 8 {
                    self.editor.canvas.zoom_in();
                }
            }
            M::ViewZoomOut => {
                if self.editor.canvas.zoom_level() > 1 {
                    self.editor.canvas.zoom_out();
                }
            }
            M::ViewToggleGrid => self.editor.canvas.toggle_grid(),
            M::ViewToggleUndoPanel => self.dialogs.undo_panel.toggle(),
            M::ViewToggleTimeline => {
                self.animation.timeline_visible = !self.animation.timeline_visible;
                self.frame.dirty = true;
            }
            M::ViewToggleSidePanel => {
                self.side_panel.toggle_open();
                self.frame.dirty = true;
            }
            M::ViewLoadBuiltinPalette(name) => {
                let palettes = crate::palette_import::builtin_palettes();
                if let Some((_, swatches)) = palettes.into_iter().find(|(n, _)| *n == name) {
                    self.palette_editor
                        .open_with_swatches(name.to_string(), swatches);
                    self.palette_editor.available_palettes(None);
                }
                self.frame.dirty = true;
            }
            M::ViewPaletteEditor => {
                self.palette_editor.open = !self.palette_editor.open;
                if self.palette_editor.open {
                    self.palette_editor
                        .load_current_from_palette(&self.editor.palette);
                    self.palette_editor.available_palettes(None);
                    self.palette_editor.lighting_pickers_visible =
                        self.ui.mode == AppMode::Lighting || self.lighting.scene.is_some();
                }
                self.frame.dirty = true;
            }
            _ => unreachable!("{MISROUTED}: {action:?}"),
        }
    }

    /// Tools menu: select a tool.
    pub(super) fn handle_tools_menu_action(&mut self, tool: toolbox::Tool) {
        self.editor.toolbox.selected = tool;
        if tool != toolbox::Tool::PolygonSelect {
            self.editor.selection_polygon_points.clear();
        }
    }

    /// Layer menu: add, duplicate, delete, merge, reorder, visibility, lock.
    pub(super) fn handle_layer_menu_action(&mut self, action: menu::MenuAction) {
        use menu::MenuAction as M;
        let idx = self.editor.layer_stack.active;
        match action {
            M::LayerNew => {
                let w = self.editor.layer_stack.layers[0].buffer.width();
                let h = self.editor.layer_stack.layers[0].buffer.height();
                self.editor.layer_stack.add(w, h);
                self.editor.recomposite_canvas();
            }
            M::LayerDuplicate => {
                self.editor.layer_stack.duplicate(idx);
                self.editor.recomposite_canvas();
            }
            M::LayerDelete => {
                self.editor.layer_stack.delete(idx);
                self.editor.recomposite_canvas();
            }
            M::LayerMergeDown => {
                self.editor.layer_stack.merge_down(idx);
                self.editor.recomposite_canvas();
            }
            M::LayerMoveUp => {
                if self.editor.layer_stack.move_up(idx) {
                    self.editor.layer_stack.active = idx.saturating_sub(1);
                    self.editor.recomposite_canvas();
                }
            }
            M::LayerMoveDown => {
                if self.editor.layer_stack.move_down(idx) {
                    let new_idx = (idx + 1).min(self.editor.layer_stack.layers.len() - 1);
                    self.editor.layer_stack.active = new_idx;
                    self.editor.recomposite_canvas();
                }
            }
            M::LayerToggleVisibility => {
                self.editor.layer_stack.toggle_visibility(idx);
                self.editor.recomposite_canvas();
            }
            M::LayerToggleLock => self.editor.layer_stack.toggle_lock(idx),
            _ => unreachable!("{MISROUTED}: {action:?}"),
        }
        self.frame.dirty = true;
    }

    /// Animation menu: add or delete a frame, play, toggle the timeline.
    pub(super) fn handle_anim_menu_action(&mut self, action: menu::MenuAction) {
        use menu::MenuAction as M;
        match action {
            M::AnimFrameAdd => self.add_timeline_frame_from_layers(),
            M::AnimFrameDelete => {
                let cur = self.animation.timeline_state.current_frame;
                let _ = self.animation.timeline_state.remove_frame(cur);
            }
            M::AnimPlay => {
                // Was previously a no-op: it toggled TimelineState::playing,
                // a field nothing else ever reads. Now actually starts
                // in-canvas playback, same as pressing Enter on the timeline.
                self.start_inline_playback_from_timeline();
            }
            M::AnimToggleTimeline => {
                self.animation.timeline_visible = !self.animation.timeline_visible;
            }
            _ => unreachable!("{MISROUTED}: {action:?}"),
        }
        self.frame.dirty = true;
    }

    /// Animation > Add frame: append a frame holding every layer and open
    /// the timeline.
    fn add_timeline_frame_from_layers(&mut self) {
        // Same rule as A-key capture: snapshot every layer so
        // navigation never clears layers beyond a flattened buffer.
        let document_state: Vec<canvas::CanvasBuffer> = self
            .editor
            .layer_stack
            .layers
            .iter()
            .map(|l| l.buffer.clone())
            .collect();
        let composite = self.editor.layer_stack.composite();
        let thumbnail = capture_thumbnail(&composite, 12, 6);
        let new_frame = timeline::TimelineFrame {
            thumbnail,
            has_keyframe: false,
            label: format!("F{}", self.animation.timeline_state.frames.len()),
            delay: self.animation.timeline_state.default_delay(),
            document_state,
            layer_keyframes: vec![None; self.editor.layer_stack.layers.len()],
        };
        self.animation
            .timeline_state
            .sync_layer_names(&self.editor.layer_stack);
        self.animation.timeline_state.add_frame(new_frame);
        self.animation.timeline_visible = true;
    }

    /// Image and Help menus: entries that open a dialog or overlay.
    pub(super) fn handle_dialog_menu_action(&mut self, action: menu::MenuAction) {
        use menu::MenuAction as M;
        match action {
            M::ImageResizeCanvas => {
                self.dialogs.settings.canvas_width = self.editor.canvas.buffer.width() as u16;
                self.dialogs.settings.canvas_height = self.editor.canvas.buffer.height() as u16;
                self.dialogs.settings.settings_open = true;
                self.frame.dirty = true;
            }
            M::HelpAbout => {}
            M::HelpKeybindings => {
                self.ui.show_keybindings = true;
                self.frame.dirty = true;
            }
            _ => unreachable!("{MISROUTED}: {action:?}"),
        }
    }

    pub(super) fn apply_settings(&mut self) {
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
