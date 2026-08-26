//! Top-level TUI application glue.
//!
//! The bulk of the application lives in topical submodules — see
//! `app_state`, `event_loop`, `dispatch`, and the feature-specific
//! submodules below. This file keeps only the module declarations, public
//! re-exports, the high-level `render` pipeline, and a couple of shared free
//! helpers.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Tabs};
use ratatui::Frame;
use std::time::Instant;

pub mod brush;
pub mod canvas;
pub mod components;
pub mod dialogs;
pub mod events;
pub mod export;
pub mod file_ops;
pub mod font_editor;
pub mod fx;
pub mod image_editor;
pub mod keymap;
pub mod layers;
pub mod layout;
pub mod light_panel;
pub mod lighting;
pub mod menu;
mod overlays;
pub mod palette;
pub mod palette_editor;
pub mod particles;
pub mod player;
pub mod props_panel;
pub mod render_mode;
pub mod side_panel;
pub mod status;
pub mod theme;
pub mod throbber;
pub mod timeline;
pub mod toolbox;
pub mod tools;
pub mod undo;
pub mod undo_panel;
pub mod welcome;

mod app_state;
mod dispatch;
mod event_loop;

pub use app_state::*;
pub use brush::BrushState;
pub use dialogs::RasciiImportDialog;
pub use events::AppEvent;
pub use export::ExportMode;
pub use light_panel::LightPanel;
pub use menu::{MenuBar, MenuBarState};
pub use palette::Palette;
pub use player::AnimationPlayer;
pub use props_panel::{PropAction, PropsPanel, PropsPanelMode};
pub use render_mode::RenderMode;
pub use side_panel::{SidePanel, TabId};
pub use status::CanvasSettings;
pub use throbber::ThrobberState;
pub use toolbox::Tool;

impl TuiApp {
    pub fn render(&mut self, frame: &mut Frame<'_>) {
        self.check_async_completion();
        self.ctx.throbber.tick();

        let now = Instant::now();
        self.frame.delta_time = now.duration_since(self.frame.fx_last_tick);
        self.frame.fx_last_tick = now;

        // Welcome screen: full-screen overlay, dismisses on any constructive action
        if self.welcome.screen.show {
            let area = frame.area();
            self.welcome.screen.render(
                frame,
                area,
                self.dialogs.recent_files.list(),
                env!("CARGO_PKG_VERSION"),
                &self.ctx.theme,
                &self.ctx.icons,
            );

            if let Some(ref mut welcome_fx) = self.welcome.fx {
                let welcome_area = welcome::centered_welcome(area);
                welcome_fx.process(self.frame.delta_time, frame.buffer_mut(), welcome_area);
                if welcome_fx.done() {
                    self.welcome.fx = None;
                }
            }

            self.render_overlays(frame);
            let area = frame.area();
            if let Some(ref mut fade) = self.welcome.fade_in {
                fade.process(self.frame.delta_time, frame.buffer_mut(), area);
                if fade.done() {
                    self.welcome.fade_in = None;
                }
            }
            return;
        }

        // App fade-in (runs outside welcome screen — covers zen + normal modes)
        let area = frame.area();
        if let Some(ref mut fade) = self.welcome.fade_in {
            fade.process(self.frame.delta_time, frame.buffer_mut(), area);
            if fade.done() {
                self.welcome.fade_in = None;
            }
        }

        // Single-pass layout computation — stored for mouse handlers next cycle.
        let tw = self
            .editor
            .toolbox
            .required_width(self.editor.brush.required_outer_width());
        let toolbox_h = Tool::all().len() as u16 + 2;
        let fl = layout::FrameLayout::compute(
            frame.area(),
            self.ui.zen_mode,
            self.side_panel.open,
            tw,
            toolbox_h,
            self.animation.timeline_visible,
        );

        // --- Zen mode: canvas only, hint overlay ---
        if self.ui.zen_mode {
            self.render_canvas_area(frame, fl.canvas, &fl);
            // Hint bar at bottom-right corner
            let area = frame.area();
            if area.height > 0 && area.width > 30 {
                let hint = " F11=exit zen  ?=keys  ^K=keybinds ";
                let hint_w = hint.len() as u16;
                let hint_rect = Rect {
                    x: area.width.saturating_sub(hint_w),
                    y: area.height - 1,
                    width: hint_w.min(area.width),
                    height: 1,
                };
                let hint_para = Paragraph::new(hint).style(
                    Style::default()
                        .fg(self.ctx.theme.general.secondary)
                        .add_modifier(Modifier::DIM),
                );
                frame.render_widget(hint_para, hint_rect);
            }
            // Still render overlays in zen mode
            self.render_overlays(frame);
            return;
        }

        // --- Lighting mode ---
        if self.ui.mode == AppMode::Lighting {
            if let Some(tb_list) = fl.toolbox_list {
                self.lighting
                    .panel
                    .render(frame, tb_list, &self.lighting.scene, &self.ctx.theme);
            }
            self.render_canvas_area(frame, fl.canvas, &fl);
            // Status bar
            let lighting_active = true;
            let light_type =
                self.lighting.scene.as_ref().and_then(|s| {
                    LightPanel::light_type_str(s, self.lighting.panel.selected_index())
                });
            let light_intensity =
                self.lighting.scene.as_ref().and_then(|s| {
                    LightPanel::light_intensity(s, self.lighting.panel.selected_index())
                });
            frame.render_widget(
                components::status_bar::StatusBarWidget::new(
                    self.ui.mode,
                    &self.mode_name_string(),
                    self.editor.canvas.cursor(),
                    self.editor.canvas.zoom_level(),
                    self.editor.toolbox.selected.full_name(),
                    self.editor.unsaved,
                    None,
                    None,
                    self.ctx.git_branch.as_deref(),
                    self.frame.fps,
                    self.ctx.render_mode.label(),
                    &format_clock(),
                    0,
                    0,
                    &self.ctx.throbber.render_string(),
                    &self.ctx.icons,
                    &self.ctx.theme,
                )
                .with_canvas_size(
                    self.editor.canvas.buffer.width() as u16,
                    self.editor.canvas.buffer.height() as u16,
                )
                .with_brush(self.editor.brush.size, self.editor.brush.shape.name())
                .with_lighting(lighting_active, light_type, light_intensity),
                fl.status,
            );

            frame.render_stateful_widget(&self.ui.menu_bar, fl.menu, &mut self.ui.menu_bar_state);
            self.render_overlays(frame);
            return;
        }

        // --- Normal mode ---

        // Mode tabs
        let mode_labels = [
            ("mode_font_editor", "Font Editor"),
            ("mode_image_editor", "Image Editor"),
            ("mode_ascii_preview", "ASCII Preview"),
        ];
        let titles: Vec<String> = mode_labels
            .iter()
            .map(|(key, name)| {
                let icon = self.ctx.icons.get(*key).map(|s| s.as_str()).unwrap_or("");
                format!("{icon}  {name}")
            })
            .collect();
        let selected = match self.ui.mode {
            AppMode::FontEditor => 0,
            AppMode::ImageEditor => 1,
            AppMode::AsciiPreview => 2,
            AppMode::Lighting => 0,
        };
        let titles_refs: Vec<&str> = titles.iter().map(|s| s.as_str()).collect();
        let tabs = Tabs::new(titles_refs)
            .style(Style::default().fg(self.ctx.theme.general.secondary))
            .highlight_style(
                Style::default()
                    .fg(self.ctx.theme.general.primary)
                    .add_modifier(Modifier::BOLD),
            )
            .select(selected);
        frame.render_widget(tabs, fl.tabs);

        // Toolbox + brush/text options (left panel)
        if let Some(tb_list) = fl.toolbox_list {
            self.editor
                .toolbox
                .set_borders(layout::toolbox_list_borders());
            frame.render_widget(&self.editor.toolbox, tb_list);
        }

        // Palette / settings panel below toolbox (left column)
        if let Some(palette_rect) = fl.palette {
            if self.dialogs.settings.settings_open {
                frame.render_widget(&self.dialogs.settings, palette_rect);
            } else {
                frame.render_widget(&self.editor.palette, palette_rect);
            }
        }

        // Canvas / font editor area
        self.render_canvas_area(frame, fl.canvas, &fl);

        // Right drawer: side panel
        if let Some(rp) = fl.right_panel {
            let font_name = self.editor.font_editor.font.as_ref().and_then(|_f| {
                let name = if self.editor.font_editor.font_storage_name.is_empty() {
                    self.editor
                        .font_editor
                        .current_path
                        .as_ref()
                        .and_then(|p| p.file_stem())
                        .and_then(|s| s.to_str())
                        .unwrap_or("Untitled")
                        .to_string()
                } else {
                    self.editor.font_editor.font_storage_name.clone()
                };
                (!name.is_empty()).then_some(name)
            });
            self.props_panel.clear_rects();
            self.side_panel.render(
                frame,
                rp,
                Some(&mut self.editor.layer_panel),
                Some(&self.editor.layer_stack),
                self.editor.toolbox.selected,
                &self.editor.brush,
                Some(&self.editor.text_tool),
                self.editor.eyedropper_sample,
                self.editor.fill_threshold,
                Some(&self.animation.particle_system.config),
                self.editor.canvas.buffer.width() as u16,
                self.editor.canvas.buffer.height() as u16,
                font_name.as_deref(),
                self.editor.canvas.zoom_level(),
                self.lighting.scene.as_ref(),
                Some(&self.lighting.panel),
                &mut self.props_panel.rects,
                &self.editor.move_state,
                &self.editor.rotate_state,
                &self.editor.selection_state,
                &self.editor.line_state,
            );
        }

        // Timeline panel at bottom of canvas
        if let Some(timeline_rect) = fl.timeline {
            let block = Block::default()
                .title(" Timeline ")
                .borders(fl.timeline_borders())
                .style(Style::default().fg(self.ctx.theme.general.secondary));
            let inner = block.inner(timeline_rect);
            if inner.height >= 5 {
                let (grid_area, toolbar_area) = timeline::AnimationTimeline::split_area(inner);
                let anim_timeline = timeline::AnimationTimeline::panel_instance();
                frame.render_widget(block, timeline_rect);
                frame.render_stateful_widget(
                    &anim_timeline,
                    grid_area,
                    &mut self.animation.timeline_state,
                );
                let playing = self
                    .animation
                    .inline_player
                    .as_ref()
                    .is_some_and(|p| p.is_playing());
                let loop_enabled = self
                    .animation
                    .inline_player
                    .as_ref()
                    .map(|p| p.is_looping())
                    .unwrap_or(self.animation.loop_enabled);
                self.animation.transport_rects = timeline::render_transport_bar(
                    frame,
                    toolbar_area,
                    playing,
                    loop_enabled,
                    &self.ctx.theme,
                );
            } else {
                frame.render_widget(block, timeline_rect);
            }
        }

        // FPS tracking
        let now = Instant::now();
        let elapsed = now - self.frame.last_frame_time;
        self.frame.last_frame_time = now;
        let instant_fps = if elapsed.as_secs_f64() > 0.0 {
            1.0 / elapsed.as_secs_f64()
        } else {
            0.0
        };
        self.frame.fps = self.frame.fps * 0.9 + instant_fps * 0.1;

        // Status bar
        let status_font_name = self.editor.font_editor.font.as_ref().and_then(|_f| {
            let name = if self.editor.font_editor.font_storage_name.is_empty() {
                self.editor
                    .font_editor
                    .current_path
                    .as_ref()
                    .and_then(|p| p.file_stem())
                    .and_then(|s| s.to_str())
                    .unwrap_or("Untitled")
                    .to_string()
            } else {
                self.editor.font_editor.font_storage_name.clone()
            };
            (!name.is_empty()).then_some(name)
        });
        let status_glyph_count = self.editor.font_editor.font.as_ref().map(|f| f.chars.len());
        let lighting_active = self.ui.mode == AppMode::Lighting;
        let light_type = if lighting_active {
            self.lighting
                .scene
                .as_ref()
                .and_then(|s| LightPanel::light_type_str(s, self.lighting.panel.selected_index()))
        } else {
            None
        };
        let light_intensity = if lighting_active {
            self.lighting
                .scene
                .as_ref()
                .and_then(|s| LightPanel::light_intensity(s, self.lighting.panel.selected_index()))
        } else {
            None
        };
        frame.render_widget(
            components::status_bar::StatusBarWidget::new(
                self.ui.mode,
                &self.mode_name_string(),
                self.editor.canvas.cursor(),
                self.editor.canvas.zoom_level(),
                self.editor.toolbox.selected.full_name(),
                self.editor.unsaved,
                status_font_name.as_deref(),
                status_glyph_count,
                self.ctx.git_branch.as_deref(),
                self.frame.fps,
                self.ctx.render_mode.label(),
                &format_clock(),
                self.editor.layer_stack.len() as u8,
                self.editor.undo.history_len(),
                &self.ctx.throbber.render_string(),
                &self.ctx.icons,
                &self.ctx.theme,
            )
            .with_canvas_size(
                self.editor.canvas.buffer.width() as u16,
                self.editor.canvas.buffer.height() as u16,
            )
            .with_brush(self.editor.brush.size, self.editor.brush.shape.name())
            .with_lighting(lighting_active, light_type, light_intensity),
            fl.status,
        );

        // Menu bar (rendered last so dropdown overlays main content)
        frame.render_stateful_widget(&self.ui.menu_bar, fl.menu, &mut self.ui.menu_bar_state);

        self.render_overlays(frame);
    }

    /// Render the canvas (or font editor overview) inside `canvas_area`.
    fn render_canvas_area(
        &mut self,
        frame: &mut Frame<'_>,
        canvas_area: Rect,
        fl: &layout::FrameLayout,
    ) {
        let borders = if self.ui.zen_mode {
            Borders::NONE
        } else {
            fl.canvas_borders()
        };
        if self.animation.render(frame, canvas_area, borders) {
            return;
        }

        let mode_title = match self.ui.mode {
            AppMode::ImageEditor => {
                if self.editor.image_editor.entering_path() {
                    format!(
                        " Image Editor [Path: {}] ",
                        self.editor.image_editor.path_buffer()
                    )
                } else if let Some(err) = self.editor.image_editor.error_message() {
                    format!(" Image Editor [Error: {err}] ")
                } else if self.editor.image_editor.has_cells() {
                    format!(
                        " Image Editor {} ",
                        self.editor.image_editor.adjustment_status()
                    )
                } else {
                    self.ui.mode.title().to_string()
                }
            }
            _ => self.ui.mode.title().to_string(),
        };

        let canvas_borders = if self.ui.zen_mode {
            Borders::NONE
        } else {
            fl.canvas_borders()
        };
        let block = Block::default().title(mode_title).borders(canvas_borders);
        let inner = block.inner(canvas_area);

        let is_font_ui_mode = self.ui.mode == AppMode::FontEditor
            && !matches!(
                self.editor.font_editor.view,
                font_editor::FontEditorView::CharEditor(_)
            );

        if is_font_ui_mode {
            self.editor.font_editor.before_render(inner);
            frame.render_widget(block, canvas_area);
            frame.render_widget(&self.editor.font_editor, inner);
        } else {
            if self.ui.mode == AppMode::FontEditor {
                // Read-only w.r.t. the document: copies canvas content
                // into the font editor's display cache.
                self.editor.sync_canvas_to_font_char();
            }
            // NOTE (GPT review F-06): rendering must not mutate document
            // state. Image-editor → canvas synchronization used to run on
            // every render here, letting the image editor's stale cell
            // cache overwrite paint operations. That sync now happens
            // only at explicit model-mutation points (image load,
            // conversion keys, GIF import), never during rendering.

            // Selection perimeter
            if let Some(ref sel) = self.editor.selection {
                if sel.is_active() {
                    self.editor.canvas.selection_perimeter = Some(sel.perimeter());
                } else {
                    self.editor.canvas.selection_perimeter = None;
                }
            } else {
                self.editor.canvas.selection_perimeter = None;
            }
            self.editor
                .canvas
                .polygon_vertices
                .clone_from(&self.editor.selection_polygon_points);

            // Text overlays
            if self.editor.toolbox.selected == Tool::Text {
                let mut overlays: Vec<_> = self
                    .editor
                    .text_tool
                    .blocks
                    .iter()
                    .enumerate()
                    .filter_map(|(i, _)| self.editor.text_tool.render_block_to_overlay(i))
                    .collect();
                // Live preview overlay when hovering with text buffer
                if self.editor.text_tool.show_preview
                    && !self.editor.text_tool.text_buffer.is_empty()
                {
                    let text = self.editor.text_tool.text_buffer.clone();
                    let font_idx = self.editor.text_tool.font_index;
                    let just = self.editor.text_tool.justification;
                    let color = self.editor.text_tool.text_color;
                    let scale = self.editor.text_tool.scale;
                    let px = self.editor.text_tool.preview_pos.0;
                    let py = self.editor.text_tool.preview_pos.1;
                    // Build a temporary TextToolState for rendering preview rows
                    let mut preview_state = self.editor.text_tool.clone();
                    preview_state.text_buffer = text;
                    preview_state.font_index = font_idx;
                    preview_state.justification = just;
                    preview_state.text_color = color;
                    preview_state.scale = scale;
                    preview_state.preview_pos = (px, py);
                    if preview_state.font.is_none() {
                        preview_state.load_selected_font();
                    }
                    let (rows, width) = preview_state.render_rows_from_buffer().unwrap_or_default();
                    if !rows.is_empty() && width > 0 {
                        let bb_w = width * scale.max(1) as usize;
                        let left_x = match just {
                            crate::render::Justification::Left => px,
                            crate::render::Justification::Center => px - (bb_w as i16 / 2),
                            crate::render::Justification::Right => px - bb_w as i16,
                        };
                        overlays.push(canvas::TextOverlay {
                            x: left_x,
                            y: py,
                            rows,
                            color,
                            scale,
                            rotation: 0,
                        });
                    }
                }
                self.editor.canvas.text_overlays = overlays;
                self.editor.canvas.text_block_perimeter =
                    self.editor.text_tool.selected_block.and_then(|idx| {
                        if idx < self.editor.text_tool.blocks.len() {
                            let (bx, by, bw, bh) = self.editor.text_tool.compute_bounding_box(idx);
                            if bw == 0 || bh == 0 {
                                return None;
                            }
                            let mut perim = Vec::new();
                            for x in bx..bx + bw as i16 {
                                perim.push((x.max(0) as usize, by.max(0) as usize));
                                perim.push((
                                    x.max(0) as usize,
                                    (by + bh as i16 - 1).max(0) as usize,
                                ));
                            }
                            for y in (by + 1)..(by + bh as i16 - 1) {
                                if y < 0 {
                                    continue;
                                }
                                perim.push((bx.max(0) as usize, y as usize));
                                perim.push(((bx + bw as i16 - 1).max(0) as usize, y as usize));
                            }
                            Some(perim)
                        } else {
                            None
                        }
                    });
            } else {
                self.editor.canvas.text_overlays.clear();
                self.editor.canvas.text_block_perimeter = None;
            }

            frame.render_widget(block, canvas_area);

            let canvas_inner_rect = self.editor.compute_canvas_rect(inner);
            if canvas_inner_rect.width > 1 && canvas_inner_rect.height > 1 {
                let w = self.editor.canvas.buffer.width();
                let h = self.editor.canvas.buffer.height();
                // Draw border 1 cell outside canvas rect so canvas cells don't overwrite it.
                let border_rect = Rect {
                    x: canvas_inner_rect.x.saturating_sub(1),
                    y: canvas_inner_rect.y.saturating_sub(1),
                    width: (canvas_inner_rect.width + 2).min(inner.width),
                    height: (canvas_inner_rect.height + 2).min(inner.height),
                }
                .intersection(inner);
                let edge = Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Double)
                    .title(format!(" {}x{} ", w, h))
                    .style(Style::default().fg(self.ctx.theme.canvas.border));
                frame.render_widget(edge, border_rect);
            }
            // Sync glyph cursor for CharEditor mode
            if self.ui.mode == AppMode::FontEditor
                && matches!(
                    self.editor.font_editor.view,
                    font_editor::FontEditorView::CharEditor(_)
                )
            {
                let gc = &mut self.editor.canvas;
                if gc.glyph_cursor.is_none() {
                    gc.glyph_cursor = Some(canvas::GlyphCursor::new(
                        self.editor.font_editor.glyph_cursor_x,
                        self.editor.font_editor.glyph_cursor_y,
                    ));
                } else if let Some(ref mut g) = gc.glyph_cursor {
                    g.x = self.editor.font_editor.glyph_cursor_x;
                    g.y = self.editor.font_editor.glyph_cursor_y;
                }
                if let Some(ref mut g) = gc.glyph_cursor {
                    g.blink();
                }
            } else {
                self.editor.canvas.glyph_cursor = None;
            }

            let composited = self.editor.canvas.buffer.clone();

            if let Some(ref scene) = self.lighting.scene {
                let swatch_data = self.palette_editor.lighting_swatches();
                let shaded = components::canvas::shade_composited(
                    &composited,
                    &self.editor.layer_stack,
                    scene,
                    &self.lighting.lut,
                    self.lighting.max_shadow_distance,
                    self.lighting.height_scale,
                    &self.ctx.palette_rgb_to_swatch,
                    &swatch_data,
                );
                self.editor.canvas.buffer = shaded;
            }

            if self.animation.emitter_active && self.animation.show_live_particles {
                let saved = self.editor.canvas.buffer.clone();
                self.animation
                    .particle_system
                    .render_to_canvas(&mut self.editor.canvas.buffer);
                frame.render_widget(&self.editor.canvas, canvas_inner_rect);
                self.editor.canvas.buffer = saved;
            } else {
                frame.render_widget(&self.editor.canvas, canvas_inner_rect);
            }

            // Point light overlays (lighting mode)
            if self.ui.mode == AppMode::Lighting {
                if let Some(ref scene) = self.lighting.scene {
                    let zoom = self.editor.canvas.zoom_level().max(1) as i16;
                    let (sx, sy) = self.editor.canvas.scroll_offset();
                    let buf = frame.buffer_mut();
                    for (i, light) in scene.lights.iter().enumerate() {
                        if let lighting::Light::Point { position, .. } = light {
                            let bx = position.0 as i16;
                            let by = position.1 as i16;
                            let screen_x = canvas_inner_rect.x as i16 + (bx - sx as i16) * zoom;
                            let screen_y = canvas_inner_rect.y as i16 + (by - sy as i16) * zoom;
                            if screen_x >= canvas_inner_rect.x as i16
                                && screen_x < (canvas_inner_rect.x + canvas_inner_rect.width) as i16
                                && screen_y >= canvas_inner_rect.y as i16
                                && screen_y
                                    < (canvas_inner_rect.y + canvas_inner_rect.height) as i16
                            {
                                if let Some(cell) = buf.cell_mut((screen_x as u16, screen_y as u16))
                                {
                                    let marker = "\u{2726}";
                                    let fg = if i == self.lighting.panel.selected_index {
                                        self.ctx.theme.general.primary
                                    } else {
                                        self.ctx.theme.general.secondary
                                    };
                                    cell.set_symbol(marker);
                                    cell.set_fg(fg);
                                    cell.set_bg(ratatui::style::Color::Reset);
                                }
                            }
                        }
                    }
                }
            }

            self.editor.canvas.buffer = composited;
        }
    }

    /// Build the mode name string for the status bar.
    fn mode_name_string(&self) -> String {
        match self.ui.mode {
            AppMode::ImageEditor => {
                if self.editor.image_editor.has_cells() {
                    format!(
                        "Image Editor {}",
                        self.editor.image_editor.adjustment_status()
                    )
                } else {
                    "Image Editor".to_string()
                }
            }
            AppMode::AsciiPreview => "ASCII Preview".to_string(),
            AppMode::Lighting => "Lighting Editor".to_string(),
            AppMode::FontEditor => {
                if let font_editor::FontEditorView::CharEditor(code) = self.editor.font_editor.view
                {
                    format!("Font Editor [U+{code:04X}]")
                } else if self.editor.font_editor.view == font_editor::FontEditorView::HeaderEditor
                {
                    "Font Editor - Header".to_string()
                } else if self.editor.font_editor.view
                    == font_editor::FontEditorView::SmushRuleEditor
                {
                    "Font Editor - Smushing Rules".to_string()
                } else if self.editor.font_editor.view
                    == font_editor::FontEditorView::TransformEditor
                {
                    "Font Editor - Transforms".to_string()
                } else {
                    "Font Editor".to_string()
                }
            }
        }
    }
}

fn centered_overlay(area: Rect) -> Rect {
    Rect {
        x: area.width / 6,
        y: area.height / 6,
        width: area.width * 2 / 3,
        height: area.height * 2 / 3,
    }
}

/// Map a Rotate-tool drag's horizontal distance to a signed step count: every
/// `ROTATE_DRAG_STEP` cells is one 90° turn, sign gives direction, and the
/// result is reduced mod 4 since four steps is a no-op (keeps replay loops
/// short for long drags).
fn rotate_drag_steps(dx: i16) -> i16 {
    const ROTATE_DRAG_STEP: i16 = 4;
    (dx / ROTATE_DRAG_STEP) % 4
}

/// Downsample a `CanvasBuffer` to `thumb_w × thumb_h` char grid for timeline thumbnails.
fn capture_thumbnail(buffer: &canvas::CanvasBuffer, thumb_w: u16, thumb_h: u16) -> Vec<Vec<char>> {
    let bw = buffer.width();
    let bh = buffer.height();
    let mut thumb = Vec::with_capacity(thumb_h as usize);
    for ty in 0..thumb_h {
        let mut row = Vec::with_capacity(thumb_w as usize);
        for tx in 0..thumb_w {
            let bx = (tx as usize * bw / thumb_w as usize).min(bw.saturating_sub(1));
            let by = (ty as usize * bh / thumb_h as usize).min(bh.saturating_sub(1));
            let ch = buffer.get(bx, by).map(|c| c.ch).unwrap_or(' ');
            row.push(ch);
        }
        thumb.push(row);
    }
    thumb
}

fn format_clock() -> String {
    let since_epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let total_secs = since_epoch.as_secs();
    let h = (total_secs / 3600) % 24;
    let m = (total_secs / 60) % 60;
    let s = total_secs % 60;
    format!("{h:02}:{m:02}:{s:02}")
}

#[cfg(test)]
mod rotate_drag_steps_tests {
    use super::rotate_drag_steps;

    #[test]
    fn test_rotate_drag_steps_below_threshold_is_zero() {
        assert_eq!(rotate_drag_steps(0), 0);
        assert_eq!(rotate_drag_steps(3), 0);
        assert_eq!(rotate_drag_steps(-3), 0);
    }

    #[test]
    fn test_rotate_drag_steps_one_step_each_direction() {
        assert_eq!(rotate_drag_steps(4), 1);
        assert_eq!(rotate_drag_steps(-4), -1);
    }

    #[test]
    fn test_rotate_drag_steps_multiple_steps() {
        assert_eq!(rotate_drag_steps(20), 1, "20/4=5 steps, reduced mod 4");
        assert_eq!(rotate_drag_steps(-20), -1);
    }

    #[test]
    fn test_rotate_drag_steps_full_turn_is_a_noop() {
        assert_eq!(
            rotate_drag_steps(16),
            0,
            "16/4=4 steps == full turn == identity"
        );
    }
}
