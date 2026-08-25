use crossterm::event::{
    self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    Event, KeyEventKind,
};
use crossterm::execute;
use std::io;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use super::*;

impl TuiApp {
    pub fn run(&mut self) -> io::Result<()> {
        let mut terminal = ratatui::init();
        execute!(io::stdout(), EnableBracketedPaste, EnableMouseCapture)?;

        let (term_width, _) = crossterm::terminal::size().unwrap_or((80, 24));
        self.side_panel.open = self.default_side_panel_open(term_width);

        while !self.ui.should_quit {
            self.handle_event()?;

            let now = Instant::now();
            // Same throttle pattern as the throbber below: redraw at most
            // once per the animation's own frame interval, rather than
            // busy-looping — `advance()` only actually changes
            // current_frame once real elapsed time crosses that interval.
            let inline_playing_due = self.animation.inline_player.as_ref().is_some_and(|p| {
                p.is_playing()
                    && now.saturating_duration_since(self.frame.last_draw_time)
                        >= Duration::from_millis(1000 / p.fps().max(1) as u64)
            });
            let needs_redraw = match self.ctx.render_mode {
                RenderMode::Fast => true,
                RenderMode::Dirty => {
                    self.frame.dirty
                        || self.welcome.fade_in.is_some()
                        || self.welcome.fx.is_some()
                        || inline_playing_due
                        || (self.ctx.throbber.is_active()
                            && now.saturating_duration_since(self.frame.last_draw_time)
                                >= Duration::from_millis(100))
                }
            };

            if needs_redraw {
                if let Some(player) = self.animation.inline_player.as_ref() {
                    if player.is_playing() {
                        let elapsed = now.saturating_duration_since(self.frame.last_draw_time);
                        let advanced = player.advance(elapsed);
                        self.animation.timeline_state.current_frame = player.current_frame();
                        if advanced > 0 {
                            self.frame.force_full_redraw = true;
                        }
                        let (cur, total) = player.progress();
                        if total > 0 && cur >= total.saturating_sub(1) && !player.is_looping() {
                            // Natural end of a non-looping playthrough: stop
                            // ticking (stays visible on the last frame until
                            // the user dismisses with Esc/q) instead of
                            // redrawing forever at the animation's fps.
                            player.pause();
                        }
                    }
                }
                if self.frame.force_full_redraw {
                    // Something outside our Terminal (the animation player)
                    // wrote to the screen directly; ratatui's diff cache no
                    // longer matches reality. clear() resets that cache so
                    // the draw below is a full repaint, not a stale diff.
                    terminal.clear()?;
                    self.frame.force_full_redraw = false;
                }
                terminal.draw(|f| self.render(f))?;
                self.frame.dirty = false;
                self.frame.last_draw_time = now;
            } else {
                std::thread::sleep(Duration::from_millis(5));
            }
        }

        execute!(
            terminal.backend_mut(),
            DisableBracketedPaste,
            DisableMouseCapture
        )?;
        ratatui::restore();
        Ok(())
    }

    pub(crate) fn trigger_quit(&mut self) {
        if self.editor.unsaved {
            self.dialogs.quit_confirm_dialog = true;
            self.frame.dirty = true;
        } else {
            self.ui.should_quit = true;
        }
    }

    pub(crate) fn process_event(&mut self, event: &AppEvent) {
        match event {
            AppEvent::Quit => self.trigger_quit(),
            AppEvent::Toolbox(crate::tui::events::ToolboxEvent::ToolSelected)
                if self.editor.toolbox.selected != Tool::PolygonSelect =>
            {
                self.editor.selection_polygon_points.clear();
            }
            AppEvent::Palette(crate::tui::events::PaletteEvent::ColorChanged(color, target)) => {
                self.editor.palette.selected_color = Some(*color);
                match target {
                    palette::ColorTarget::Foreground => {
                        self.editor.palette.target = palette::ColorTarget::Foreground;
                    }
                    palette::ColorTarget::Background => {
                        self.editor.palette.target = palette::ColorTarget::Background;
                    }
                }
            }
            AppEvent::ModeChanged => self.frame.dirty = true,
            AppEvent::RenderModeChanged => self.frame.dirty = true,
            AppEvent::SaveAsRequested => self.perform_save(),
            AppEvent::OpenRequested => self.perform_open(),
            AppEvent::ExportRequested(_) => self.perform_export(),
            AppEvent::Menu(action) => self.handle_menu_action(action.clone()),
            _ => {}
        }
    }

    pub fn check_async_completion(&mut self) {
        let rx = match self.ctx.async_rx.take() {
            Some(rx) => rx,
            None => return,
        };
        match rx.try_recv() {
            Ok(result) => {
                self.ctx.throbber.stop();
                self.frame.dirty = true;
                match result {
                    AsyncResult::SaveComplete(r) => match r {
                        Ok(path) => {
                            // Only treat the document as clean (and honor a
                            // deferred quit) if it wasn't edited while the
                            // async save was in flight — otherwise clearing
                            // the flag would silently discard those edits
                            // on quit (GPT review F-07).
                            let saved_unchanged =
                                self.dialogs.pending_save_revision == Some(self.editor.revision);
                            self.dialogs.pending_save_revision = None;
                            if saved_unchanged {
                                self.editor.unsaved = false;
                            }
                            self.editor.font_editor.current_path = Some(path);
                            self.ctx.last_save_time = Instant::now();
                            self.dialogs.file_ops.error_message.clear();
                            if saved_unchanged && self.dialogs.quit_after_save {
                                self.dialogs.quit_after_save = false;
                                self.ui.should_quit = true;
                            }
                        }
                        Err(e) => {
                            self.dialogs.pending_save_revision = None;
                            self.dialogs.quit_after_save = false;
                            self.dialogs.file_ops.error_message = format!("Save failed: {e}");
                        }
                    },
                    AsyncResult::OpenComplete(r) => match r {
                        Ok((font, path)) => {
                            self.editor.unsaved = false;
                            self.editor.undo.clear();
                            self.editor.font_editor.load_font(font);
                            self.editor.font_editor.current_path = Some(path.clone());
                            self.dialogs.recent_files.push(path);
                            self.dialogs.recent_files.save_to_disk();
                            self.dialogs.file_ops.error_message.clear();
                        }
                        Err(e) => {
                            self.dialogs.file_ops.error_message = e;
                            self.dialogs.file_ops.mode = file_ops::FileOpsMode::Open;
                        }
                    },
                    AsyncResult::SystemFontComplete(r) => match r {
                        Ok((font, family_name)) => {
                            self.ui.session_type = SessionType::Font;
                            self.ui.mode = AppMode::FontEditor;
                            self.editor.mark_dirty();
                            self.editor.undo.clear();
                            self.editor.font_editor.load_font(font);
                            self.editor.font_editor.current_path = None;
                            self.editor.font_editor.font_storage_name = family_name;
                            self.dialogs.system_font.error_message.clear();
                        }
                        Err(e) => {
                            self.dialogs.system_font.error_message = e;
                            self.dialogs.system_font.active = true;
                        }
                    },
                    AsyncResult::ExportComplete(r) => match r {
                        Ok(()) => {
                            self.dialogs.export_dialog.active = false;
                        }
                        Err(e) => {
                            self.dialogs.export_dialog.error_message = e;
                            self.dialogs.export_dialog.active = true;
                        }
                    },
                    AsyncResult::AutoSaveComplete => {}
                }
            }
            Err(mpsc::TryRecvError::Empty) => {
                self.ctx.async_rx = Some(rx);
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.ctx.throbber.stop();
                self.frame.dirty = true;
            }
        }
    }

    pub fn handle_event(&mut self) -> io::Result<()> {
        if event::poll(Duration::from_millis(self.ctx.render_mode.poll_ms())).unwrap_or(false) {
            self.frame.dirty = true;
            loop {
                match event::read()? {
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        let event = self.handle_key_event(key);
                        if let Some(ref e) = event {
                            self.process_event(e);
                        }
                    }
                    Event::Mouse(mouse) => {
                        self.handle_mouse_event(mouse);
                    }
                    Event::Paste(text) => {
                        self.handle_paste_event(text);
                    }
                    _ => {}
                }
                if !event::poll(Duration::ZERO).unwrap_or(false) {
                    break;
                }
            }
        }

        self.check_async_completion();

        // Update particle system if emitter is active
        if self.animation.emitter_active {
            let now = Instant::now();
            let dt = now
                .saturating_duration_since(self.frame.last_frame_time)
                .as_secs_f64();
            if dt > 0.0 {
                let bw = self.editor.canvas.buffer.width();
                let bh = self.editor.canvas.buffer.height();
                let layer = if self.animation.particle_system.config.collide_with_layer {
                    Some(&self.editor.canvas.buffer)
                } else {
                    None
                };
                self.animation
                    .particle_system
                    .update(dt, Some((bw, bh)), layer);
                self.frame.dirty = true;
            }
        }

        // Auto-save check
        if self.ctx.auto_save_interval > 0
            && self.editor.unsaved
            && self.ui.mode == AppMode::FontEditor
            && !self.ctx.throbber.is_active()
        {
            if let Some(ref path) = self.editor.font_editor.current_path {
                if self.ctx.last_save_time.elapsed()
                    >= Duration::from_secs(self.ctx.auto_save_interval)
                {
                    if let Some(ref font) = self.editor.font_editor.font {
                        self.ctx.last_save_time = Instant::now();
                        let font = font.clone();
                        let path = path.clone();
                        let (tx, rx) = mpsc::channel();
                        self.ctx.async_rx = Some(rx);
                        self.ctx.throbber.start("Auto-saving...");
                        self.frame.dirty = true;
                        std::thread::spawn(move || {
                            let _ = file_ops::save_font(&font, &path);
                            let _ = tx.send(AsyncResult::AutoSaveComplete);
                        });
                    }
                }
            }
        }

        Ok(())
    }
}
