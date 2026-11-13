//! Font, GIF, image and rascii import, and the export dialog.

use super::*;

impl TuiApp {
    pub(super) fn perform_import_font(
        &mut self,
        path: std::path::PathBuf,
        size: f32,
        charset_name: String,
    ) {
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
    pub(super) fn start_font_import_from_file(&mut self) {
        self.ui.session_type = SessionType::Font;
        self.dialogs.file_ops.enter_import_font();
        self.frame.dirty = true;
    }

    pub(super) fn start_system_font_conversion(&mut self) {
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

    pub(super) fn perform_import_gif(&mut self, config: dialogs::GifImportConfig) {
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
                            // Plain snapshot: no transform markers.
                            // (None,None) interpolates to the default
                            // properties, same as the old Some(default).
                            layer_keyframes: vec![None],
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

    pub(super) fn perform_open_image(&mut self, path: std::path::PathBuf) {
        // Images open in a new tab, never over the current document.
        self.new_document(crate::tui::documents::DocumentKind::Image);
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

    pub(super) fn perform_rascii_import(&mut self) {
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
        //
        // Bake text blocks non-destructively into a cloned stack: output sees
        // the pixels, the editable objects stay live in the document.
        let baked_stack;
        let layer_stack_ref: &crate::tui::layers::LayerStack =
            if self.editor.text_tool.blocks.is_empty() {
                &self.editor.layer_stack
            } else {
                baked_stack = self.layer_stack_with_text_baked();
                &baked_stack
            };
        let frames: Vec<Vec<Vec<canvas::CanvasCell>>> = if (format
            == crate::tui::export::ExportMode::Gif
            || format == crate::tui::export::ExportMode::Apng
            || format == crate::tui::export::ExportMode::Ansi)
            && timeline_available
            && !self.animation.timeline_state.frames.is_empty()
        {
            let ts = &self.animation.timeline_state;
            let swatch_data = self.palette_editor.lighting_swatches();
            let lighting_ctx = if let Some(scene) = &self.lighting.scene {
                if !self.lighting.light_keyframes.is_empty() {
                    Some(export::LightingContext {
                        base_scene: scene,
                        light_keyframes: &self.lighting.light_keyframes,
                        lut: &self.lighting.lut,
                        layer_stack: layer_stack_ref,
                        max_shadow_distance: self.lighting.max_shadow_distance,
                        height_scale: self.lighting.height_scale,
                        palette_rgb_to_swatch: &self.ctx.palette_rgb_to_swatch,
                        swatch_data: &swatch_data,
                    })
                } else {
                    None
                }
            } else {
                None
            };
            export::capture_timeline_frames(ts, layer_stack_ref, w, h, lighting_ctx.as_ref())
        } else if self.editor.text_tool.blocks.is_empty() {
            vec![cells.clone()]
        } else {
            // Static export with text: bake onto a canvas-sized buffer.
            let mut buf = self.editor.canvas.buffer.clone();
            self.editor.text_tool.bake_blocks_into(&mut buf);
            let baked_cells: Vec<Vec<canvas::CanvasCell>> = (0..h)
                .map(|y| {
                    (0..w)
                        .map(|x| buf.get(x, y).copied().unwrap_or_default())
                        .collect()
                })
                .collect();
            vec![baked_cells]
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
}
