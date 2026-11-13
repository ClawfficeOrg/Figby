//! Welcome-screen and Props-panel actions, plus palette-driven lighting rebuilds.

use super::*;

impl TuiApp {
    pub(super) fn dispatch_welcome_action(&mut self, action: welcome::WelcomeAction) {
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

    pub(super) fn dispatch_props_action(&mut self, action: PropAction) {
        match action {
            PropAction::SizeUp => self.editor.brush.size_up(),
            PropAction::SizeDown => self.editor.brush.size_down(),
            PropAction::DensityUp => self.editor.brush.density_up(),
            PropAction::DensityDown => self.editor.brush.density_down(),
            PropAction::CycleShape => self.editor.brush.cycle_shape(),
            PropAction::CycleSubMode => {
                if self.editor.toolbox.selected == Tool::Text {
                    self.editor.text_tool.cycle_mode();
                } else {
                    self.editor
                        .brush
                        .cycle_sub_mode(self.editor.palette.has_multi_select());
                    if self.editor.brush.sub_mode == brush::BrushSubMode::Normal {
                        self.animation.marker_accum.clear();
                    }
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
            PropAction::TextBgToggle => {
                self.editor.text_tool.bg_color = None;
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

    /// Rebuild lighting LUT and rgb→swatch mapping from palette editor data.
    /// Make sure every RGB colour used by lit layers has a lighting swatch, so
    /// lit art keeps its hues instead of collapsing onto whichever palette
    /// swatch happens to be nearest (e.g. a Marker-shaded title lit grey).
    pub(super) fn sync_art_swatches(&mut self) {
        crate::tui::palette_editor::sync_art_swatches(
            &mut self.palette_editor.swatches,
            &self.editor.layer_stack,
        );
    }

    pub(super) fn rebuild_lighting_from_palette(&mut self) {
        self.sync_art_swatches();
        let swatch_data = self.palette_editor.lighting_swatches();
        // Lit output keeps the art's glyph style: block-shade art stays block art.
        let ramp = lighting::pick_char_ramp(self.editor.layer_stack.layers.iter().flat_map(|l| {
            let b = l.buffer();
            (0..b.height())
                .flat_map(move |y| (0..b.width()).filter_map(move |x| b.get(x, y).map(|c| c.ch)))
        }));
        self.lighting.lut = lighting::LightingLut::from_swatches(&swatch_data, ramp);
        // Build rgb→swatch map from palette editor swatches
        let swatch_pairs: Vec<(String, String)> = self
            .palette_editor
            .swatches
            .iter()
            .map(|s| (s.name.clone(), s.hex.clone()))
            .collect();
        self.ctx.palette_rgb_to_swatch = palette::build_rgb_to_swatch(&swatch_pairs);
    }
}
