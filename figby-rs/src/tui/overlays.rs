use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use super::{app_state::PendingDocAction, dialogs, file_ops, keymap, timeline, AppMode, TuiApp};

impl TuiApp {
    /// Render all floating overlays (dialogs, keybindings, undo panel).
    pub(super) fn render_overlays(&mut self, frame: &mut Frame<'_>) {
        // Export dialog overlay
        if self.dialogs.export_dialog.active {
            let overlay = super::centered_overlay(frame.area());
            frame.render_widget(Clear, overlay);
            self.dialogs.export_dialog.render(frame, overlay);
        }

        // File ops overlay
        if self.dialogs.file_ops.mode != file_ops::FileOpsMode::Idle {
            let overlay = super::centered_overlay(frame.area());
            frame.render_widget(Clear, overlay);
            self.dialogs.file_ops.render(frame, overlay);
        }

        // Keybindings overlay (scrollable)
        if self.ui.show_keybindings {
            let area = frame.area();
            let overlay = Rect {
                x: area.width / 8,
                y: area.height / 8,
                width: area.width * 3 / 4,
                height: area.height * 3 / 4,
            };
            frame.render_widget(Clear, overlay);
            let block = Block::default()
                .title(" Keybindings (Esc/q: close  ↑↓/PgUp/PgDn: scroll) ")
                .borders(Borders::ALL)
                .style(
                    Style::default()
                        .bg(self.ctx.theme.menu.dropdown_bg)
                        .fg(self.ctx.theme.menu.fg),
                );
            let inner = block.inner(overlay);
            frame.render_widget(block, overlay);

            let mut lines: Vec<Line> = Vec::new();
            let mut last_scope: Option<keymap::Scope> = None;
            for binding in keymap::KEYMAP {
                if last_scope != Some(binding.scope) {
                    if last_scope.is_some() {
                        lines.push(Line::from(""));
                    }
                    lines.push(Line::from(Span::styled(
                        format!(" {}", binding.scope.label()),
                        Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                    )));
                    last_scope = Some(binding.scope);
                }
                lines.push(Line::from(format!(
                    "  {:<22} {}",
                    binding.keys, binding.description
                )));
            }
            let total = lines.len();
            let visible = inner.height as usize;
            let max_scroll = total.saturating_sub(visible);
            let scroll = self.ui.keybindings_scroll.min(max_scroll) as u16;
            frame.render_widget(Paragraph::new(lines).scroll((scroll, 0)), inner);
        }

        // Undo history panel overlay
        if self.dialogs.undo_panel.open {
            frame.render_widget(Clear, frame.area());
            self.dialogs
                .undo_panel
                .render(frame, frame.area(), self.editor.undo.history_entries());
        }

        // Keyframe editor panel
        if self.animation.timeline_state.keyframe_editor.open {
            let area = frame.area();
            let panel_w = area.width.clamp(30, 42);
            let panel_x = area.x + area.width.saturating_sub(panel_w);
            let panel_h = (area.height / 2).max(10).min(area.height - 3);
            let panel_y = area.y + area.height.saturating_sub(panel_h + 3);
            let panel_rect = Rect {
                x: panel_x,
                y: panel_y,
                width: panel_w,
                height: panel_h,
            };
            frame.render_widget(Clear, panel_rect);
            self.animation.timeline_state.render_keyframe_editor(
                frame,
                panel_rect,
                &timeline::TimelineTheme::default(),
            );
        }

        // Tween panel
        if self.animation.timeline_state.tween.is_some() {
            let area = frame.area();
            let panel_w = area.width.clamp(30, 42);
            let panel_x = area.x + area.width.saturating_sub(panel_w);
            let panel_h = (area.height / 2).max(10).min(area.height - 3);
            let panel_y = area.y + 1;
            let panel_rect = Rect {
                x: panel_x,
                y: panel_y,
                width: panel_w,
                height: panel_h,
            };
            frame.render_widget(Clear, panel_rect);
            self.animation.timeline_state.render_tween_panel(
                frame,
                panel_rect,
                &timeline::TimelineTheme::default(),
            );
        }

        // New Image dialog
        if self.dialogs.new_image.active {
            let overlay = super::centered_overlay(frame.area());
            frame.render_widget(Clear, overlay);
            dialogs::new_image::render_new_image_dialog(&self.dialogs.new_image, frame, overlay);
        }

        // System font picker dialog
        if self.dialogs.system_font.active {
            let overlay = super::centered_overlay(frame.area());
            frame.render_widget(Clear, overlay);
            dialogs::system_font::render_system_font_dialog(
                &self.dialogs.system_font,
                frame,
                overlay,
            );
        }

        // Font import (TTF/OTF) options dialog
        if self.dialogs.font_import_options.active {
            let overlay = super::centered_overlay(frame.area());
            frame.render_widget(Clear, overlay);
            dialogs::font_import_options::render_font_import_options_dialog(
                &self.dialogs.font_import_options,
                frame,
                overlay,
            );
        }

        // Rascii import dialog
        if self.dialogs.rascii_import.active {
            let overlay = super::centered_overlay(frame.area());
            frame.render_widget(Clear, overlay);
            self.dialogs.rascii_import.render(frame, overlay);
        }

        // GIF import dialog
        if self.dialogs.gif_import.active {
            let overlay = super::centered_overlay(frame.area());
            frame.render_widget(Clear, overlay);
            self.dialogs.gif_import.render(frame, overlay);
        }

        // Emitter config panel overlay
        if self.animation.emitter_panel.open {
            let area = frame.area();
            let panel_w = area.width.clamp(30, 36);
            let panel_x = area.x + area.width.saturating_sub(panel_w);
            let panel_h = (area.height / 2).max(14).min(area.height - 3);
            let panel_y = area.y + 1;
            let panel_rect = Rect {
                x: panel_x,
                y: panel_y,
                width: panel_w,
                height: panel_h,
            };
            frame.render_widget(Clear, panel_rect);
            self.animation.emitter_panel.render_config_panel(
                frame,
                panel_rect,
                &self.animation.particle_system.config,
            );
        }

        // Palette editor overlay
        if self.palette_editor.open {
            let was_lighting = self.palette_editor.lighting_pickers_visible;
            self.palette_editor.lighting_pickers_visible =
                self.ui.mode == AppMode::Lighting || self.lighting.scene.is_some();
            if was_lighting != self.palette_editor.lighting_pickers_visible {
                self.frame.dirty = true;
            }
            self.palette_editor
                .render(frame, frame.area(), &self.ctx.theme);
        }

        // Unsaved-changes dialog
        if self.dialogs.quit_confirm_dialog {
            let area = frame.area();
            let proceed_label = if self.dialogs.pending_transition == Some(PendingDocAction::Quit) {
                "quit"
            } else {
                "continue"
            };
            let can_save =
                self.ui.mode == AppMode::FontEditor && self.editor.font_editor.font.is_some();
            let hint = if can_save {
                format!(
                    "  [Y] Save and {proceed_label}   [N] Discard and {proceed_label}   [C] Cancel"
                )
            } else {
                format!("  [N] Discard and {proceed_label}   [C] Cancel")
            };
            let hint_len = hint.len() as u16;
            let w = (hint_len + 4).min(area.width);
            let h: u16 = 7.min(area.height);
            let dialog = Rect {
                x: area.x + area.width.saturating_sub(w) / 2,
                y: area.y + area.height.saturating_sub(h) / 2,
                width: w,
                height: h,
            };
            frame.render_widget(Clear, dialog);
            let block = Block::default()
                .title(" Unsaved Changes ")
                .borders(Borders::ALL)
                .style(
                    Style::default()
                        .bg(self.ctx.theme.menu.dropdown_bg)
                        .fg(self.ctx.theme.menu.fg),
                );
            let inner = block.inner(dialog);
            frame.render_widget(block, dialog);
            let lines = vec![
                Line::from(""),
                Line::from(Span::styled(
                    "  You have unsaved changes.",
                    Style::default().add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(hint),
            ];
            frame.render_widget(Paragraph::new(lines), inner);
            // Store button rects for mouse hit-testing
            self.dialogs.quit_confirm_buttons = [
                Rect {
                    x: inner.x + 2,
                    y: inner.y + 3,
                    width: 3,
                    height: 1,
                },
                Rect {
                    x: inner.x + 22,
                    y: inner.y + 3,
                    width: 3,
                    height: 1,
                },
                Rect {
                    x: inner.x + 45,
                    y: inner.y + 3,
                    width: 3,
                    height: 1,
                },
            ];
        }
    }
}
