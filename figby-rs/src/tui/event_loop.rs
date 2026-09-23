use crossterm::event::{
    self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    Event, KeyEventKind,
};
use crossterm::execute;
use std::io::{self, Write};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use super::*;

/// RAII guard: restores the terminal (raw mode, alternate screen,
/// mouse/bracketed-paste modes) on Drop — including panics and early
/// returns. Previously cleanup ran only on the normal tail of the
/// event loop, so a read/render error left the user's terminal in raw
/// mode (GPT review F-25).
struct TerminalRestoreGuard;

impl Drop for TerminalRestoreGuard {
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), DisableBracketedPaste, DisableMouseCapture);
        ratatui::restore();
        let _ = io::stdout().flush();
    }
}

impl TuiApp {
    /// Install a panic hook that restores terminal state before
    /// delegating to the previous hook, so a panic during render/event
    /// handling cannot leave raw mode enabled (GPT review F-25).
    fn install_panic_hook() {
        let prev_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let _ = execute!(io::stdout(), DisableBracketedPaste, DisableMouseCapture);
            ratatui::restore();
            prev_hook(info);
        }));
    }

    pub fn run(&mut self) -> io::Result<()> {
        Self::install_panic_hook();
        // The guard owns ALL teardown: nothing below needs a manual
        // restore, and every exit path (including `?` and panic unwinds)
        // goes through Drop.
        let _terminal_guard = TerminalRestoreGuard;
        let mut terminal = ratatui::init();
        execute!(io::stdout(), EnableBracketedPaste, EnableMouseCapture)?;

        let (term_width, _) = crossterm::terminal::size().unwrap_or((80, 24));
        self.side_panel.open = self.default_side_panel_open(term_width);

        while !self.ui.should_quit {
            self.handle_event()?;

            let now = Instant::now();
            // Scheduler-driven export preview (GPT review F-26): the export
            // dialog's frame preview advances on elapsed time here, from the
            // event loop, never from inside the render pass — previously it
            // ticked during render_overlays, so it stalled whenever redraws
            // were suppressed (RenderMode::Dirty with nothing dirty).
            let preview_advanced = if self.dialogs.export_dialog.active {
                let elapsed = now.saturating_duration_since(self.frame.last_draw_time);
                self.dialogs.export_dialog.preview_tick(elapsed)
            } else {
                false
            };
            if preview_advanced {
                self.frame.dirty = true;
            }
            // Same throttle pattern as the throbber below: redraw at most
            // once per the animation's own frame interval, rather than
            // busy-looping — `advance()` only actually changes
            // current_frame once real elapsed time crosses that interval.
            let inline_playing_due = self.animation.inline_player.as_ref().is_some_and(|p| {
                p.is_playing()
                    && now.saturating_duration_since(self.frame.last_draw_time)
                        >= p.frame_interval()
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

        // Teardown happens in TerminalRestoreGuard::drop.
        Ok(())
    }

    pub fn trigger_quit(&mut self) {
        if self.editor.unsaved {
            self.dialogs.quit_confirm_dialog = true;
            self.dialogs.pending_transition = Some(PendingDocAction::Quit);
            self.frame.dirty = true;
        } else {
            self.ui.should_quit = true;
        }
    }

    /// Whether a "save and proceed" option can actually save right now.
    /// Image sessions have no direct save path (they export), so the
    /// confirm dialog must not offer a no-op Save (GPT review F-08).
    pub fn can_save_now(&self) -> bool {
        self.ui.mode == AppMode::FontEditor && self.editor.font_editor.font.is_some()
    }

    /// Run the transition that was waiting behind the unsaved-changes
    /// dialog. Called after the user picks Discard, or after a successful
    /// save completes.
    pub(crate) fn execute_pending_transition(&mut self) {
        let action = self.dialogs.pending_transition.take();
        match action {
            Some(PendingDocAction::Quit) => {
                self.dialogs.quit_after_save = false;
                self.ui.should_quit = true;
            }
            Some(PendingDocAction::NewFileSession) => {
                self.do_new_file_session(self.ui.session_type);
            }
            Some(PendingDocAction::FontNewBlankSession) => {
                self.do_new_file_session(SessionType::Font);
            }
            Some(PendingDocAction::CloseDocument(idx)) => {
                // Discard path, or save path once the save lands.
                let _ = self.finish_close_document(idx);
            }
            None => {}
        }
    }

    /// Fresh blank document of the given size, replacing ALL document and
    /// transient state atomically — undo history, selection/clipboard,
    /// tool transient state, image-editor cache, timeline, inline
    /// playback and export-dialog timeline data (GPT review F-08: Open/
    /// New used to reset only part of this).
    pub(crate) fn reset_document(&mut self, w: u16, h: u16) {
        use canvas::CanvasWidget;
        self.editor.canvas = CanvasWidget::new(w, h);
        self.editor.layer_stack = layers::LayerStack::new(w as usize, h as usize);
        self.editor.layer_panel = layers::LayerPanel::new();
        self.editor.layer_panel.theme = self.ctx.theme.clone();
        self.editor.layer_panel.icons = self.ctx.icons.clone();
        self.editor.undo.clear();
        self.editor.selection = None;
        self.editor.clipboard = None;
        self.editor.selection_polygon_points.clear();
        self.editor.selection_state = tools::selection::SelectionState::default();
        self.editor.move_state = tools::move_tool::MoveState::default();
        self.editor.rotate_state = tools::rotate_tool::RotateState::default();
        self.editor.line_state = tools::line::LineState::default();
        self.editor.fill_threshold = 0;
        self.editor.image_editor = image_editor::ImageEditor::new();
        self.editor.font_editor.font = None;
        self.editor.font_editor.current_path = None;
        self.animation.timeline_state = timeline::TimelineState::default();
        self.animation.inline_player = None;
        self.dialogs.export_dialog.timeline_available = false;
        self.dialogs.export_dialog.frame_delays.clear();
        // A fresh document starts clean.
        self.editor.unsaved = false;
        self.editor.revision = self.editor.revision.wrapping_add(1);
        self.dialogs.pending_save_revision = None;
        self.editor.recomposite_canvas();
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
                            if saved_unchanged && self.dialogs.pending_transition.is_some() {
                                // Save & continue: the open/new that was
                                // waiting behind the dialog now runs.
                                self.execute_pending_transition();
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
                            // The tab was created up front when the open
                            // started; land there, or a fresh one if the
                            // user closed it mid-load. Either way nothing
                            // else is touched.
                            match self.dialogs.pending_open_tab.take() {
                                Some(tab) if tab < self.document_count() => {
                                    self.switch_document(tab);
                                }
                                _ => {
                                    self.new_document(crate::tui::documents::DocumentKind::Font);
                                }
                            }
                            // Opening replaces the whole document: drop
                            // stale selection, timeline, playback and
                            // image-editor state along with undo history
                            // (GPT review F-08 — partial resets leaked
                            // previous-document state into the new one).
                            self.editor.undo.clear();
                            self.editor.selection = None;
                            self.editor.clipboard = None;
                            self.editor.selection_polygon_points.clear();
                            self.editor.image_editor = image_editor::ImageEditor::new();
                            self.animation.timeline_state = timeline::TimelineState::default();
                            self.animation.inline_player = None;
                            self.dialogs.export_dialog.timeline_available = false;
                            self.dialogs.export_dialog.frame_delays.clear();
                            self.editor.unsaved = false;
                            self.editor.font_editor.load_font(font);
                            self.editor.font_editor.current_path = Some(path.clone());
                            self.editor.recomposite_canvas();
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
