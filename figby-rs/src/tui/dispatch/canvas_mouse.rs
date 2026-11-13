//! Canvas mouse input: the Text tool and the painting, selection and
//! transform tools. Reached only after every panel and overlay declined the
//! event (see `mouse.rs`).

use super::*;

impl TuiApp {
    /// Text tool: hover shows preview, click places text or selects block.
    /// Consumes every event while the Text tool is selected.
    pub(super) fn mouse_text_tool(&mut self, mouse: MouseEvent, canvas_inner_rect: Rect) -> bool {
        if self.editor.toolbox.selected != Tool::Text {
            return false;
        }
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
        true
    }

    /// Painting, selection, move and rotate tools on the canvas.
    pub(super) fn mouse_canvas_tools(&mut self, mouse: MouseEvent, canvas_inner_rect: Rect) {
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
                    let mut rng = StdRng::seed_from_u64(rand::rng().random());
                    let size = self.editor.brush.size;
                    let density = self.editor.brush.density;
                    let mut buf = self.editor.layer_stack.active_layer().buffer.clone();
                    tools::spray::spray_stamp(&mut buf, bx, by, size, density, cell, &mut rng);
                    *self.editor.layer_stack.active_layer_mut().buffer_mut() = buf;
                    self.editor.recomposite_canvas();
                } else if self.editor.toolbox.selected == Tool::Emitter {
                    self.animation.emitter_active = true;
                    self.animation.emitter_follow_layer = Some(self.editor.layer_stack.active);
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
                        let mut rng = StdRng::seed_from_u64(rand::rng().random());
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
}
