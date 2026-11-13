//! Menu bar actions and the settings dialog.

use super::*;

impl TuiApp {
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
            menu::MenuAction::FileSaveAsFigmap => {
                self.start_save_as_figmap();
                self.ui.menu_bar_state.reset();
            }
            menu::MenuAction::FileNewTabFont => {
                self.new_document(crate::tui::documents::DocumentKind::Font);
                self.ui.menu_bar_state.reset();
            }
            menu::MenuAction::FileNewTabImage => {
                self.new_document(crate::tui::documents::DocumentKind::Image);
                self.ui.menu_bar_state.reset();
            }
            menu::MenuAction::FileNewAnimation => {
                // 8.6.3 leftover, unblocked by 8.5: a fresh image tab with
                // the timeline panel already open, ready to add frames.
                self.new_document(crate::tui::documents::DocumentKind::Image);
                self.animation.timeline_visible = true;
                self.ui.menu_bar_state.reset();
            }
            menu::MenuAction::FileCloseTab => {
                self.request_close_document(self.active_doc);
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
                    self.palette_editor
                        .open_with_swatches(name.to_string(), swatches);
                    self.palette_editor.available_palettes(None);
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
