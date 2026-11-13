//! Key routing: each focused component gets the event in priority order.

use super::*;
use std::ops::ControlFlow;

/// A key handler either passes the key on (`Continue`) or consumes it
/// (`Break`), optionally producing an event for the caller.
type KeyRoute = ControlFlow<Option<AppEvent>>;

impl TuiApp {
    /// Route one key event. Handlers run in priority order (modal
    /// overlays first, canvas tools and global shortcuts last); the first
    /// that returns `Break` ends routing with its result.
    pub fn handle_key_event(&mut self, key: impl Into<KeyEvent>) -> Option<AppEvent> {
        let key = key.into();
        let handlers: &[fn(&mut Self, KeyEvent) -> KeyRoute] = &[
            Self::key_keybindings_overlay,
            Self::key_quit_prompt,
            Self::key_inline_player,
            Self::key_modal_dialogs,
            Self::key_timeline_popups,
            Self::key_palette_editor,
            Self::key_menu_bar,
            Self::key_early_global,
            Self::key_settings,
            // Space paints. It is never a playback shortcut: timeline playback
            // starts from the transport bar or Animation > Play. A dual binding
            // made Space ambiguous in live sessions (paint vs play depending on
            // whether frames existed).
            Self::key_lighting,
            Self::key_layer_panel,
            Self::key_welcome,
            Self::key_editor_modes,
            Self::key_text_tool,
            Self::key_editor_state,
            Self::key_side_panel_tabs,
            Self::key_animation,
            Self::key_enter_lighting,
            Self::key_emitter_panel,
            Self::key_canvas_view,
            Self::key_panel_toggles,
            Self::key_props_panel,
            Self::key_toolbox,
            Self::key_palette,
            Self::key_keyboard_paint,
        ];
        for handler in handlers {
            if let ControlFlow::Break(event) = handler(self, key) {
                return event;
            }
        }
        // Global dispatch: file ops, view toggles, mode cycling, quit
        keymap::lookup_global(key.code, key.modifiers)
            .and_then(|action| self.dispatch_global(action))
    }

    /// Keybindings overlay: Esc closes it, arrows scroll, swallow all other keys
    fn key_keybindings_overlay(&mut self, key: KeyEvent) -> KeyRoute {
        let code = key.code;
        if self.ui.show_keybindings {
            match code {
                KeyCode::Esc | KeyCode::Char('q') => {
                    self.ui.show_keybindings = false;
                    self.ui.keybindings_scroll = 0;
                    self.frame.dirty = true;
                }
                KeyCode::Up => {
                    self.ui.keybindings_scroll = self.ui.keybindings_scroll.saturating_sub(1);
                    self.frame.dirty = true;
                }
                KeyCode::Down => {
                    self.ui.keybindings_scroll = self.ui.keybindings_scroll.saturating_add(1);
                    self.frame.dirty = true;
                }
                KeyCode::PageUp => {
                    self.ui.keybindings_scroll = self.ui.keybindings_scroll.saturating_sub(10);
                    self.frame.dirty = true;
                }
                KeyCode::PageDown => {
                    self.ui.keybindings_scroll = self.ui.keybindings_scroll.saturating_add(10);
                    self.frame.dirty = true;
                }
                _ => {}
            }
            return ControlFlow::Break(None);
        }
        ControlFlow::Continue(())
    }

    /// Unsaved-changes dialog: intercept all keys before anything else.
    /// Y = save & proceed (only offered when saving is possible),
    /// N = discard & proceed, C/Esc = cancel the transition entirely
    /// (GPT review F-08).
    fn key_quit_prompt(&mut self, key: KeyEvent) -> KeyRoute {
        let code = key.code;
        if self.dialogs.quit_confirm_dialog {
            match code {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                    // Save & proceed. When there is nothing that can be
                    // saved the key does nothing: Enter is the default key
                    // and must never silently discard work (it used to).
                    if self.can_save_now() {
                        self.dialogs.quit_confirm_dialog = false;
                        self.frame.dirty = true;
                        if self.dialogs.pending_transition == Some(PendingDocAction::Quit) {
                            self.dialogs.quit_after_save = true;
                        }
                        // The pending transition stays armed; it runs once
                        // the save succeeds (async SaveComplete for fonts,
                        // immediately for .figmap).
                        self.start_save();
                    }
                }
                KeyCode::Char('n') | KeyCode::Char('N') => {
                    self.dialogs.quit_confirm_dialog = false;
                    self.frame.dirty = true;
                    self.execute_pending_transition();
                }
                KeyCode::Char('c') | KeyCode::Char('C') | KeyCode::Esc => {
                    self.dialogs.quit_confirm_dialog = false;
                    self.dialogs.pending_transition = None;
                    self.frame.dirty = true;
                }
                _ => {}
            }
            return ControlFlow::Break(None);
        }
        ControlFlow::Continue(())
    }

    /// In-canvas animation playback: intercept all keys, mirroring the
    /// controls already implemented on AnimationPlayer::handle_key
    /// (space/F8=pause, arrows=seek, +/-=speed, l/L=loop toggle). Esc/q
    /// dismiss playback and return to normal editing. F8 is handled by
    /// the player itself — no special case needed here.
    fn key_inline_player(&mut self, key: KeyEvent) -> KeyRoute {
        let code = key.code;
        if let Some(player) = self.animation.inline_player.as_ref() {
            let consumed = player.handle_key(code);
            let should_dismiss =
                consumed && matches!(code, KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q'));
            self.frame.dirty = true;
            if should_dismiss {
                self.stop_inline_playback();
            }
            return ControlFlow::Break(None);
        }
        ControlFlow::Continue(())
    }

    /// New image dialog active
    fn key_modal_dialogs(&mut self, key: KeyEvent) -> KeyRoute {
        let code = key.code;
        if self.dialogs.new_image.active {
            self.dialogs.new_image.handle_key(code);
            if !self.dialogs.new_image.active && self.dialogs.new_image.confirmed {
                self.new_document_or_reuse_pristine(crate::tui::documents::DocumentKind::Image);
                let w = self.dialogs.new_image.result_width;
                let h = self.dialogs.new_image.result_height;
                let pal_name = self.dialogs.new_image.result_palette_name.clone();
                let pal_swatches = self.dialogs.new_image.result_palette_swatches.clone();
                self.ui.session_type = SessionType::Image;
                self.editor.image_editor = image_editor::ImageEditor::new();
                self.ui.mode = AppMode::ImageEditor;
                self.editor.canvas = canvas::CanvasWidget::new(w, h);
                self.editor.layer_stack = layers::LayerStack::new(w as usize, h as usize);
                self.editor.layer_panel = layers::LayerPanel::new();
                self.editor.layer_panel.theme = self.ctx.theme.clone();
                self.editor.layer_panel.icons = self.ctx.icons.clone();
                if !pal_swatches.is_empty() {
                    // Apply the chosen palette silently to the live
                    // palette — no editor overlay. The swatches were
                    // previewed in the dialog itself; Ctrl+Shift+P
                    // reopens the editor on demand.
                    self.palette_editor.swatches = pal_swatches;
                    self.palette_editor.name_buffer = pal_name;
                    self.palette_editor.selected = 0;
                    self.palette_editor.init_lighting_from_swatches();
                    self.palette_editor
                        .apply_to_palette(&mut self.editor.palette);
                    self.palette_editor.open = false;
                }
                self.editor.recomposite_canvas();
                self.welcome.screen.show = false;
                self.welcome.fx = None;
                self.frame.dirty = true;
            }
            return ControlFlow::Break(None);
        }

        // System font picker dialog active
        if self.dialogs.system_font.active {
            self.dialogs.system_font.handle_key(code);
            if !self.dialogs.system_font.active && self.dialogs.system_font.confirmed {
                self.start_system_font_conversion();
            }
            return ControlFlow::Break(None);
        }

        // Font import (TTF/OTF) options dialog active
        if self.dialogs.font_import_options.active {
            self.dialogs.font_import_options.handle_key(code);
            if !self.dialogs.font_import_options.active
                && self.dialogs.font_import_options.confirmed
            {
                let path = self.dialogs.font_import_options.path.clone();
                let size = self.dialogs.font_import_options.result_size;
                let charset = self.dialogs.font_import_options.result_charset.clone();
                self.perform_import_font(path, size, charset);
            }
            return ControlFlow::Break(None);
        }

        // GIF import dialog active
        if self.dialogs.gif_import.active {
            self.dialogs.gif_import.handle_key(code);
            if !self.dialogs.gif_import.active && self.dialogs.gif_import.confirmed {
                if let Some(cfg) = self.dialogs.gif_import.config.take() {
                    self.perform_import_gif(cfg);
                }
            }
            return ControlFlow::Break(None);
        }

        // File ops dialog active: dispatch all keys to it. This sits above
        // the FontEditor search (which would otherwise swallow typed paths —
        // `/`, `.`, letters all start glyph searches in Overview).
        if self.dialogs.file_ops.mode != file_ops::FileOpsMode::Idle {
            let prev_mode = self.dialogs.file_ops.mode;
            self.dialogs.file_ops.handle_key(code);
            if self.dialogs.file_ops.mode == file_ops::FileOpsMode::Idle {
                return ControlFlow::Break(self.complete_file_ops_dialog(prev_mode));
            }
            return ControlFlow::Break(None);
        }

        // Export dialog active: dispatch all keys to it
        if self.dialogs.export_dialog.active {
            let prev_format = self.dialogs.export_dialog.format;
            let prev_len = self.dialogs.export_dialog.path_buffer.len();
            self.dialogs.export_dialog.handle_key(code);
            let closed_by_esc =
                code == KeyCode::Esc && self.dialogs.export_dialog.path_buffer.len() < prev_len;
            // If format changed to an animated format and timeline has frames,
            // populate timeline data. Skip this if frame_delays already
            // matches the frame count — that means timing is already sitting
            // there (e.g. an FPS-preset override during this session), and
            // re-deriving would clobber it. Per-frame delays come from the
            // frames themselves (F-26).
            let count = self.animation.timeline_state.frames.len();
            if (self.dialogs.export_dialog.format == export::ExportMode::Gif
                || self.dialogs.export_dialog.format == export::ExportMode::Apng
                || self.dialogs.export_dialog.format == export::ExportMode::Ansi)
                && prev_format != self.dialogs.export_dialog.format
                && count > 0
                && self.dialogs.export_dialog.frame_delays.len() != count
            {
                let ts = &self.animation.timeline_state;
                self.dialogs.export_dialog.set_timeline_from_timeline(ts);
            }
            if self.dialogs.export_dialog.play_requested {
                self.dialogs.export_dialog.play_requested = false;
                self.launch_player_from_export();
                return ControlFlow::Break(None);
            }
            // Esc closes via `close()` (clears the path) while Enter confirms
            // with the typed path intact — only Enter may kick off the async
            // export. An Esc-close must never fire a doomed "no path
            // specified" job that reopens the dialog with an error.
            if !self.dialogs.export_dialog.active && !closed_by_esc {
                self.perform_export();
            }
            return ControlFlow::Break(None);
        }

        // Rascii import dialog active: dispatch all keys to it
        if self.dialogs.rascii_import.active {
            let prev_confirmed = self.dialogs.rascii_import.confirmed;
            self.dialogs.rascii_import.handle_key(code);
            if !self.dialogs.rascii_import.active {
                if self.dialogs.rascii_import.confirmed && !prev_confirmed {
                    self.perform_rascii_import();
                }
                self.frame.dirty = true;
            }
            return ControlFlow::Break(None);
        }

        // Undo history panel open: dispatch to it first
        if self.dialogs.undo_panel.open {
            self.dialogs.undo_panel.handle_key(code);
            return ControlFlow::Break(None);
        }
        ControlFlow::Continue(())
    }

    /// Keyframe editor: intercept all keys when open
    fn key_timeline_popups(&mut self, key: KeyEvent) -> KeyRoute {
        let code = key.code;
        if self.animation.timeline_state.keyframe_editor.open
            && self
                .animation
                .timeline_state
                .handle_keyframe_editor_key(code)
        {
            self.frame.dirty = true;
            return ControlFlow::Break(None);
        }

        // Tween panel: intercept keys when open
        if self.animation.timeline_state.tween.is_some()
            && self.animation.timeline_state.handle_tween_key(code)
        {
            self.frame.dirty = true;
            return ControlFlow::Break(None);
        }
        ControlFlow::Continue(())
    }

    /// Palette editor: dispatch all keys when open. Unhandled keys
    /// fall through so globals (Ctrl+N/O/S, Ctrl+Shift+P toggle,
    /// menus) keep working while open.
    fn key_palette_editor(&mut self, key: KeyEvent) -> KeyRoute {
        let code = key.code;
        if self.palette_editor.open && self.palette_editor.handle_key(code) {
            if self.palette_editor.modified {
                self.palette_editor
                    .apply_to_palette(&mut self.editor.palette);
                self.palette_editor.modified = false;
                if self.lighting.scene.is_some() {
                    self.rebuild_lighting_from_palette();
                }
            }
            self.frame.dirty = true;
            return ControlFlow::Break(None);
        }
        ControlFlow::Continue(())
    }

    /// Menu bar active: dispatch all keys to it
    fn key_menu_bar(&mut self, key: KeyEvent) -> KeyRoute {
        let modifiers = key.modifiers;
        if self.ui.menu_bar_state.is_active() {
            self.ui
                .menu_bar
                .handle_key_event(key, &mut self.ui.menu_bar_state);
            if let Some(action) = self.ui.menu_bar_state.drain_actions() {
                return ControlFlow::Break(Some(AppEvent::Menu(action)));
            }
            return ControlFlow::Break(None);
        }

        // Alt+key: open menu bar
        if modifiers == KeyModifiers::ALT
            && self
                .ui
                .menu_bar
                .handle_key_event(key, &mut self.ui.menu_bar_state)
        {
            return ControlFlow::Break(None);
        }
        ControlFlow::Continue(())
    }

    /// Global key dispatch (early global actions before modal/mode checks)
    fn key_early_global(&mut self, key: KeyEvent) -> KeyRoute {
        let code = key.code;
        let modifiers = key.modifiers;
        if let Some(action) = keymap::lookup_global(code, modifiers) {
            match action {
                keymap::GlobalAction::Undo
                | keymap::GlobalAction::Redo
                | keymap::GlobalAction::ToggleUndoPanel => {
                    return ControlFlow::Break(self.dispatch_global(action));
                }
                _ => {}
            }
        }
        ControlFlow::Continue(())
    }

    fn key_settings(&mut self, key: KeyEvent) -> KeyRoute {
        let code = key.code;
        if self.dialogs.settings.settings_open {
            if self.dialogs.settings.handle_key(code) {
                self.apply_settings();
                return ControlFlow::Break(None);
            }
            if let KeyCode::Char('S') = code {
                self.dialogs.settings.settings_open = false;
            }
            return ControlFlow::Break(None);
        }
        ControlFlow::Continue(())
    }

    /// Lighting mode: key handling. Runs before the Layers drawer and the
    /// timeline so `+`/`-`/`D`/Shift+arrows/`A` reach the light editor
    /// instead of being captured as layer ops or frame capture.
    fn key_lighting(&mut self, key: KeyEvent) -> KeyRoute {
        let code = key.code;
        let modifiers = key.modifiers;
        if self.ui.mode == AppMode::Lighting {
            let w = self.editor.canvas.buffer.width() as i16;
            let h = self.editor.canvas.buffer.height() as i16;
            let (tf, tt) = if self.animation.timeline_state.frames.is_empty() {
                (0, 0)
            } else {
                (
                    self.animation.timeline_state.current_frame,
                    self.animation.timeline_state.frames.len(),
                )
            };
            match self
                .lighting
                .handle_key(code, modifiers, w, h, &mut self.frame.dirty, tf, tt)
            {
                Some(false) => {
                    self.ui.mode = self.ui.prev_mode;
                    self.frame.dirty = true;
                    return ControlFlow::Break(None);
                }
                Some(true) => {
                    self.animation
                        .timeline_state
                        .update_light_keyframe_frames(&self.lighting.light_keyframes);
                    return ControlFlow::Break(None);
                }
                None => {}
            }
        }
        ControlFlow::Continue(())
    }

    /// Layer panel: dispatch keys when drawer shows layers
    fn key_layer_panel(&mut self, key: KeyEvent) -> KeyRoute {
        if self.side_panel.open
            && self.side_panel.active_tab == TabId::Layers
            && self
                .editor
                .layer_panel
                .handle_key(key, &mut self.editor.layer_stack)
        {
            self.editor.recomposite_canvas();
            self.editor.mark_dirty();
            self.frame.dirty = true;
            return ControlFlow::Break(None);
        }
        ControlFlow::Continue(())
    }

    /// Welcome screen: intercept before mode-specific dispatch
    fn key_welcome(&mut self, key: KeyEvent) -> KeyRoute {
        let code = key.code;
        let modifiers = key.modifiers;
        if self.welcome.screen.show {
            let recent_count = self.dialogs.recent_files.len();
            if let Some(action) = self
                .welcome
                .screen
                .handle_key(code, modifiers, recent_count)
            {
                self.dispatch_welcome_action(action);
                return ControlFlow::Break(None);
            }
        }
        ControlFlow::Continue(())
    }

    /// Font Editor mode: dispatch to font_editor before canvas/tools.
    /// The Open dialog sits above the Overview search: typed paths
    /// (`/`, `.`, letters) must reach the dialog, not start a search.
    fn key_editor_modes(&mut self, key: KeyEvent) -> KeyRoute {
        let code = key.code;
        let modifiers = key.modifiers;
        if self.dialogs.file_ops.mode == file_ops::FileOpsMode::Open
            && self.ui.mode == AppMode::FontEditor
            && !modifiers.contains(KeyModifiers::CONTROL)
            && !modifiers.contains(KeyModifiers::ALT)
        {
            let prev_mode = self.dialogs.file_ops.mode;
            self.dialogs.file_ops.handle_key(code);
            if self.dialogs.file_ops.mode == file_ops::FileOpsMode::Idle {
                return ControlFlow::Break(self.complete_file_ops_dialog(prev_mode));
            }
            return ControlFlow::Break(None);
        }
        if self.ui.mode == AppMode::FontEditor {
            if let Some(ev) = self.handle_font_editor_key(key) {
                return ControlFlow::Break(Some(ev));
            }
        }

        // Image Editor mode: dispatch to image_editor before canvas/tools.
        // Skip while any text-entry field is live (is_typing guard):
        // single-char typing (F/I/G/B/Y...) collides with image
        // adjustment keys (invert/brightness/braille/threshold), which
        // would toggle filters and resync the canvas mid-word.
        if self.ui.mode == AppMode::ImageEditor && !self.is_typing() {
            if let Some(ev) = self.handle_image_editor_key(code) {
                return ControlFlow::Break(Some(ev));
            }
        }
        ControlFlow::Continue(())
    }

    /// Text tool dispatch
    fn key_text_tool(&mut self, key: KeyEvent) -> KeyRoute {
        let code = key.code;
        let modifiers = key.modifiers;
        if self.editor.toolbox.selected == Tool::Text {
            // Auto-activate editing when side panel Text tab is open,
            // OR when the user types a printable character that isn't
            // a tool shortcut — otherwise selecting the Text tool and
            // typing does nothing unless the side panel happens to be
            // on the Text tab. Tool shortcuts (b/e/g/…) still switch
            // tools so the user can leave the Text tool by keyboard.
            // Esc must reach the tool's own handler (it clears the buffer
            // there) — pre-clearing editing here makes the tool see
            // not-editing and skip its Esc arm, leaking the buffer.
            // Alt+arrows (drawer tab cycle) still clear editing so the
            // capture can't re-arm mid-cycle and trap the user.
            if modifiers.contains(KeyModifiers::ALT)
                && matches!(code, KeyCode::Left | KeyCode::Right)
            {
                self.editor.text_tool.editing = false;
            } else if code != KeyCode::Esc
                && !self.editor.text_tool.editing
                && (self.side_panel.open && self.side_panel.active_tab == TabId::Text
                    || matches!(code, KeyCode::Char(c) if !Tool::all().iter().any(|t| t.key_shortcut() == KeyCode::Char(c.to_ascii_lowercase()))))
            {
                self.editor.text_tool.editing = true;
            }
            let cursor = self.editor.canvas.cursor();
            // Ctrl+R rasterizes the selected block — keyboard path that
            // doesn't depend on hitting a 1-cell mouse rect under tmux.
            if code == KeyCode::Char('r')
                && modifiers.contains(KeyModifiers::CONTROL)
                && self.editor.text_tool.selected_block.is_some()
            {
                self.editor.push_undo_snapshot("Rasterize text");
                self.dispatch_props_action(crate::tui::props_panel::PropAction::RasterizeBlock);
                self.frame.dirty = true;
                return ControlFlow::Break(None);
            }
            if let Some(undo_label) = self.editor.text_tool.handle_key(code, modifiers, cursor) {
                if !undo_label.is_empty() {
                    self.editor.push_undo_snapshot(undo_label);
                    self.editor.mark_dirty();
                }
                // Esc while editing cancels (buffer cleared by the tool
                // itself) AND exits the Text tool to Brush: otherwise every
                // later keystroke is swallowed by the editing capture with
                // no visible state (the trap). No buffer check — Esc means
                // cancel regardless of what the tool did with the text.
                if code == KeyCode::Esc && self.editor.text_tool.selected_block.is_none() {
                    self.editor.toolbox.selected = Tool::Brush;
                }
                return ControlFlow::Break(None);
            }
            // Esc fell through (empty buffer + no block + not editing):
            // handle_key returns None for it, so exit the tool here —
            // same trap, no visible state, no other way out.
            if code == KeyCode::Esc {
                self.editor.toolbox.selected = Tool::Brush;
                return ControlFlow::Break(None);
            }
        }
        ControlFlow::Continue(())
    }

    /// Editor state dispatch (rotate, selection, move tool, polygon select,
    /// deselect, keyboard painting)
    fn key_editor_state(&mut self, key: KeyEvent) -> KeyRoute {
        let code = key.code;
        let modifiers = key.modifiers;
        if self
            .editor
            .handle_key(code, modifiers, &mut self.frame.dirty)
        {
            return ControlFlow::Break(None);
        }
        ControlFlow::Continue(())
    }

    /// Side panel: Alt+left/right arrows switch tabs when open
    fn key_side_panel_tabs(&mut self, key: KeyEvent) -> KeyRoute {
        let code = key.code;
        let modifiers = key.modifiers;
        if self.side_panel.open && modifiers == KeyModifiers::ALT {
            match code {
                KeyCode::Left => {
                    self.side_panel.cycle_tab(false);
                    if self.side_panel.active_tab == TabId::Text
                        && self.editor.toolbox.selected == Tool::Text
                    {
                        self.editor.text_tool.editing = true;
                    }
                    self.frame.dirty = true;
                    return ControlFlow::Break(None);
                }
                KeyCode::Right => {
                    self.side_panel.cycle_tab(true);
                    if self.side_panel.active_tab == TabId::Text
                        && self.editor.toolbox.selected == Tool::Text
                    {
                        self.editor.text_tool.editing = true;
                    }
                    self.frame.dirty = true;
                    return ControlFlow::Break(None);
                }
                _ => {}
            }
        }
        ControlFlow::Continue(())
    }

    /// Animation state dispatch (timeline nav, emitter bake/toggle)
    fn key_animation(&mut self, key: KeyEvent) -> KeyRoute {
        let code = key.code;
        let modifiers = key.modifiers;
        if self
            .animation
            .handle_key(code, modifiers, &mut self.editor, &mut self.frame.dirty)
        {
            return ControlFlow::Break(None);
        }
        ControlFlow::Continue(())
    }

    /// Enter lighting mode with uppercase G
    fn key_enter_lighting(&mut self, key: KeyEvent) -> KeyRoute {
        let code = key.code;
        if code == KeyCode::Char('G') && self.ui.mode != AppMode::Lighting {
            self.ui.prev_mode = self.ui.mode;
            self.ui.mode = AppMode::Lighting;
            if self.lighting.scene.is_none() {
                let mut scene = lighting::Scene::new();
                scene.add_light(lighting::Light::Ambient {
                    intensity: 0.5,
                    color: lighting::Rgb(255, 255, 255),
                    target: lighting::LightTarget::default(),
                });
                self.lighting.scene = Some(scene);
                // Regenerate LUT from palette when scene activates
                self.rebuild_lighting_from_palette();
            }
            self.lighting.panel.selected_index = 0;
            self.lighting.panel.show_help = true;
            self.frame.dirty = true;
            return ControlFlow::Break(None);
        }
        ControlFlow::Continue(())
    }

    /// Emitter config panel: dispatch when panel is open. Must run before the canvas
    /// cursor/zoom handler or the arrow keys never reach its field list.
    fn key_emitter_panel(&mut self, key: KeyEvent) -> KeyRoute {
        let code = key.code;
        if self.animation.emitter_panel.open {
            let handled = self
                .animation
                .emitter_panel
                .handle_config_key(code, &mut self.animation.particle_system.config);
            if handled {
                self.frame.dirty = true;
                return ControlFlow::Break(None);
            }
        }
        ControlFlow::Continue(())
    }

    /// Canvas cursor movement, zoom, grid
    fn key_canvas_view(&mut self, key: KeyEvent) -> KeyRoute {
        let ck = key.code;
        // 0x0: the canvas scrolls against the viewport size recorded at the
        // last render (`CanvasWidget::set_viewport`).
        if matches!(
            ck,
            KeyCode::Up
                | KeyCode::Down
                | KeyCode::Left
                | KeyCode::Right
                | KeyCode::Char('+')
                | KeyCode::Char('=')
                | KeyCode::Char('-')
                | KeyCode::Char('_')
        ) && self.editor.canvas.handle_key(ck, 0, 0)
        {
            return ControlFlow::Break(Some(AppEvent::Canvas(
                crate::tui::events::CanvasEvent::Modified,
            )));
        }
        ControlFlow::Continue(())
    }

    /// Settings toggle — skip when FontEditor Overview is active so S
    /// reaches the smushing-rule editor (GPT review keybind collision).
    fn key_panel_toggles(&mut self, key: KeyEvent) -> KeyRoute {
        let code = key.code;
        let modifiers = key.modifiers;
        if code == KeyCode::Char('S')
            && !modifiers.contains(KeyModifiers::CONTROL)
            && !(self.ui.mode == AppMode::FontEditor
                && self.editor.font_editor.view == font_editor::FontEditorView::Overview)
        {
            self.dialogs.settings.canvas_width = self.editor.canvas.buffer.width() as u16;
            self.dialogs.settings.canvas_height = self.editor.canvas.buffer.height() as u16;
            self.dialogs.settings.show_grid = self.editor.canvas.show_grid();
            self.dialogs.settings.settings_open = true;
            self.frame.dirty = true;
            return ControlFlow::Break(None);
        }

        // Toggle keyframe editor (uppercase only to avoid conflict)
        if code == KeyCode::Char('K') {
            self.animation.timeline_state.keyframe_editor.open =
                !self.animation.timeline_state.keyframe_editor.open;
            self.frame.dirty = true;
            return ControlFlow::Break(None);
        }

        // Ctrl+Shift+P: toggle palette editor
        if code == KeyCode::Char('P') && modifiers == KeyModifiers::CONTROL | KeyModifiers::SHIFT {
            self.palette_editor.open = !self.palette_editor.open;
            if self.palette_editor.open {
                self.palette_editor
                    .load_current_from_palette(&self.editor.palette);
                self.palette_editor.available_palettes(None);
                self.palette_editor.lighting_pickers_visible =
                    self.ui.mode == AppMode::Lighting || self.lighting.scene.is_some();
                if self.lighting.scene.is_some() {
                    self.rebuild_lighting_from_palette();
                }
            }
            self.frame.dirty = true;
            return ControlFlow::Break(None);
        }
        ControlFlow::Continue(())
    }

    /// Props panel typed-entry mode intercept
    fn key_props_panel(&mut self, key: KeyEvent) -> KeyRoute {
        let code = key.code;
        if self.props_panel.mode != PropsPanelMode::Idle {
            if let Some(action) = self.props_panel.handle_key(code) {
                self.dispatch_props_action(action);
                self.frame.dirty = true;
            }
            return ControlFlow::Break(None);
        }

        // Props/Text tab keyboard focus: Tab/Shift+Tab move the button
        // cursor, Enter activates (e.g. Rasterize). Only when the drawer
        // is open on those tabs so canvas Tab navigation is untouched.
        if self.side_panel.open
            && matches!(self.side_panel.active_tab, TabId::Props | TabId::Text)
            && matches!(code, KeyCode::Tab | KeyCode::BackTab | KeyCode::Enter)
        {
            if let Some(action) = self.props_panel.handle_key(code) {
                self.dispatch_props_action(action);
            }
            // Consume Tab/Enter here even when no button matched: with the
            // drawer focused on Props/Text, Enter must not fall through to
            // canvas paint or layer toggles.
            self.frame.dirty = true;
            return ControlFlow::Break(None);
        }
        ControlFlow::Continue(())
    }

    /// Toolbox tool selection + brush adjustments (inline from old ToolboxComponent)
    /// NOTE: 'T' is excluded from the tool-selector catch-all so it falls
    /// through to dispatch_global for ToggleTimeline / OpenTweenPanel.
    fn key_toolbox(&mut self, key: KeyEvent) -> KeyRoute {
        let code = key.code;
        let modifiers = key.modifiers;
        {
            use crate::tui::events::ToolboxEvent;
            let handled = match code {
                KeyCode::Char('[') => {
                    self.editor.brush.size_down();
                    Some(AppEvent::Toolbox(ToolboxEvent::BrushChanged))
                }
                KeyCode::Char(']') => {
                    self.editor.brush.size_up();
                    Some(AppEvent::Toolbox(ToolboxEvent::BrushChanged))
                }
                KeyCode::Char(';') => {
                    self.editor.brush.density_down();
                    Some(AppEvent::Toolbox(ToolboxEvent::BrushChanged))
                }
                KeyCode::Char('\'') => {
                    self.editor.brush.density_up();
                    Some(AppEvent::Toolbox(ToolboxEvent::BrushChanged))
                }
                KeyCode::Char('\\') => {
                    self.editor.brush.cycle_shape();
                    Some(AppEvent::Toolbox(ToolboxEvent::BrushChanged))
                }
                KeyCode::Char('M') if self.editor.toolbox.selected == Tool::Brush => {
                    self.editor
                        .brush
                        .cycle_sub_mode(self.editor.palette.has_multi_select());
                    if self.editor.brush.sub_mode == brush::BrushSubMode::Normal {
                        self.animation.marker_accum.clear();
                    }
                    Some(AppEvent::Toolbox(ToolboxEvent::BrushChanged))
                }
                KeyCode::Char(c)
                    if !modifiers.contains(KeyModifiers::CONTROL)
                        && c != 'T'
                        && c != 'S'
                        && !self.is_typing() =>
                {
                    let lower = c.to_ascii_lowercase();
                    let mut found = None;
                    for tool in Tool::all() {
                        if let KeyCode::Char(tc) = tool.key_shortcut() {
                            if tc == lower {
                                let was_brush = self.editor.toolbox.selected == Tool::Brush;
                                self.editor.toolbox.selected = *tool;
                                if was_brush && *tool != Tool::Brush {
                                    self.animation.marker_accum.clear();
                                }
                                found = Some(AppEvent::Toolbox(ToolboxEvent::ToolSelected));
                                break;
                            }
                        }
                    }
                    found
                }
                _ => None,
            };
            if let Some(action) = handled {
                if self.editor.toolbox.selected != Tool::PolygonSelect {
                    self.editor.selection_polygon_points.clear();
                }
                return ControlFlow::Break(Some(action));
            }
        }
        ControlFlow::Continue(())
    }

    /// Palette color selection (inline from old PaletteComponent).
    /// Skipped while any text-entry field is live (is_typing guard):
    /// x/f/h/z/arrows/Enter/Esc all collide with typed characters.
    /// Exception: when the palette itself is in hex-entry mode
    /// (custom_mode), still route keys to it so Enter/Escape can
    /// confirm/cancel and reset the mode. Without this, the palette
    /// deadlocks — is_typing() blocks the very keys that exit it.
    fn key_palette(&mut self, key: KeyEvent) -> KeyRoute {
        let code = key.code;
        if !self.is_typing() || self.editor.palette.is_typing() {
            use crate::tui::events::PaletteEvent;
            let handled = match code {
                KeyCode::Char('x')
                | KeyCode::Char('X')
                | KeyCode::Char('f')
                | KeyCode::Char('F')
                | KeyCode::Char('h')
                | KeyCode::Char('H')
                | KeyCode::Char('j')
                | KeyCode::Char('J')
                | KeyCode::Char('z')
                | KeyCode::Char('Z')
                | KeyCode::Left
                | KeyCode::Right
                | KeyCode::Up
                | KeyCode::Down
                | KeyCode::Enter
                | KeyCode::Backspace
                | KeyCode::Esc => {
                    if self.editor.palette.handle_key(code) {
                        let color = self.editor.palette.selected_color;
                        let target = self.editor.palette.target;
                        if let Some(c) = color {
                            Some(AppEvent::Palette(PaletteEvent::ColorChanged(c, target)))
                        } else {
                            Some(AppEvent::Palette(PaletteEvent::BrushChanged))
                        }
                    } else {
                        None
                    }
                }
                _ => None,
            };
            if let Some(action) = handled {
                return ControlFlow::Break(Some(action));
            }
        }
        ControlFlow::Continue(())
    }

    /// Keyboard painting: Space/Enter paints or erases at cursor
    fn key_keyboard_paint(&mut self, key: KeyEvent) -> KeyRoute {
        let code = key.code;
        if Tool::is_paint_tool(self.editor.toolbox.selected)
            && matches!(code, KeyCode::Char(' ') | KeyCode::Enter)
            // Layer lock enforcement (GPT review F-09)
            && !self.editor.active_layer_locked()
        {
            let (cx, cy) = self.editor.canvas.cursor();
            self.editor.push_undo_snapshot("Keyboard paint");
            if self.editor.toolbox.selected == Tool::Fill {
                let mut cell = canvas::CanvasCell {
                    ch: self.editor.brush.ch,
                    fg: None,
                    bg: None,
                    height: None,
                };
                self.editor.palette.apply_to_cell(&mut cell);
                let mut buf = self.editor.layer_stack.active_layer().buffer.clone();
                tools::fill::flood_fill(&mut buf, cx as i16, cy as i16, cell);
                *self.editor.layer_stack.active_layer_mut().buffer_mut() = buf;
                self.editor.recomposite_canvas();
            } else if self.editor.toolbox.selected == Tool::Eraser {
                let shape = self.editor.brush.shape;
                let size = self.editor.brush.size;
                let mut buf = self.editor.layer_stack.active_layer().buffer.clone();
                tools::eraser::erase_stamp(&mut buf, cx as i16, cy as i16, shape, size);
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
                tools::spray::spray_stamp(
                    &mut buf, cx as i16, cy as i16, size, density, cell, &mut rng,
                );
                *self.editor.layer_stack.active_layer_mut().buffer_mut() = buf;
                self.editor.recomposite_canvas();
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
                tools::brush::paint_stamp(&mut buf, cx as i16, cy as i16, shape, size, cell);
                *self.editor.layer_stack.active_layer_mut().buffer_mut() = buf;
                self.editor.recomposite_canvas();
            }
            self.editor.mark_dirty();
            return ControlFlow::Break(None);
        }
        ControlFlow::Continue(())
    }

    pub(super) fn handle_image_editor_key(&mut self, code: KeyCode) -> Option<AppEvent> {
        if matches!(code, KeyCode::Char('o') | KeyCode::Char('O'))
            && !self.editor.image_editor.entering_path()
        {
            self.dialogs.file_ops.enter_open_image();
            self.frame.dirty = true;
            return Some(AppEvent::ImageEditor);
        }
        let was_entering = self.editor.image_editor.entering_path();
        if self.editor.image_editor.handle_key(code) {
            self.editor.sync_image_to_canvas();
            if was_entering && !self.editor.image_editor.entering_path() {
                self.editor.undo.clear();
            }
            return Some(AppEvent::ImageEditor);
        }
        None
    }

    pub(super) fn handle_font_editor_key(&mut self, key: KeyEvent) -> Option<AppEvent> {
        if matches!(
            self.editor.font_editor.view,
            font_editor::FontEditorView::CharEditor(_)
        ) {
            self.editor.font_editor.brush_char = self.editor.brush.ch;
        }
        let area_width = ratatui::crossterm::terminal::size().unwrap_or((80, 24)).0;
        let epoch_before = self.editor.font_editor.mutation_epoch;
        if self
            .editor
            .font_editor
            .handle_key(key.code, key.modifiers, area_width)
        {
            if self.editor.font_editor.mutation_epoch != epoch_before {
                // Real content change (glyph paint, header field, smush
                // rule, transform) — navigation keys don't trip this, so
                // dirty tracking is neither missed nor spurious (F-08).
                self.editor.mark_dirty();
            }
            if self.editor.font_editor.view != font_editor::FontEditorView::Overview {
                self.editor.sync_font_char_to_canvas();
            }
            return Some(AppEvent::FontEditor(
                crate::tui::events::FontEditorEvent::Changed(self.editor.font_editor.view),
            ));
        }
        None
    }
}

#[cfg(test)]
mod sidebar_keybindings_tests {
    use super::*;
    use crate::tui::side_panel::TabId;
    use ratatui::crossterm::event::KeyEvent;

    fn app_with_frames(frame_count: usize) -> TuiApp {
        let mut app = TuiApp::new();
        // TuiApp::new() defaults to FontEditor mode, which intercepts arrow
        // keys in the font overview grid. Switch to AsciiPreview so that key
        // events reach the sidebar/timeline/layer-panel handlers we're testing.
        app.ui.mode = AppMode::AsciiPreview;
        let w = app.editor.canvas.buffer.width();
        let h = app.editor.canvas.buffer.height();
        for i in 0..frame_count {
            let mut buf = canvas::CanvasBuffer::new(w, h);
            let ch = char::from_u32(b'A' as u32 + (i % 26) as u32).unwrap();
            buf.set(
                0,
                0,
                canvas::CanvasCell {
                    ch,
                    fg: None,
                    bg: None,
                    height: None,
                },
            );
            let frame = timeline::TimelineFrame {
                thumbnail: capture_thumbnail(&buf, 8, 3),
                has_keyframe: true,
                label: format!("F{}", i),
                delay: 10,
                document_state: vec![buf],
                layer_keyframes: Vec::new(),
            };
            app.animation.timeline_state.add_frame(frame);
        }
        app.side_panel.open = true;
        app.side_panel.active_tab = TabId::Layers;
        app.animation.timeline_visible = true;
        app
    }

    #[test]
    fn test_bare_arrow_right_advances_timeline_with_sidebar_open() {
        let mut app = app_with_frames(3);
        let current_frame = app.animation.timeline_state.current_frame;

        eprintln!(
            "DEBUG: frames={}, current_frame={}, timeline_visible={}, side_panel.open={}, active_tab={:?}",
            app.animation.timeline_state.frames.len(),
            current_frame,
            app.animation.timeline_visible,
            app.side_panel.open,
            app.side_panel.active_tab,
        );

        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));

        eprintln!(
            "DEBUG after: current_frame={}",
            app.animation.timeline_state.current_frame,
        );

        assert_eq!(
            app.animation.timeline_state.current_frame,
            current_frame + 1,
            "bare Right should advance timeline frame even with sidebar open"
        );
        assert_eq!(
            app.side_panel.active_tab,
            TabId::Layers,
            "bare Right must not change side-panel tab"
        );
    }

    #[test]
    fn test_alt_left_right_cycles_sidebar_tabs() {
        let mut app = app_with_frames(3);
        let orig_frame = app.animation.timeline_state.current_frame;

        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Right, KeyModifiers::ALT));

        assert_eq!(
            app.side_panel.active_tab,
            TabId::Props,
            "Alt+Right should cycle side-panel tab forward"
        );
        assert_eq!(
            app.animation.timeline_state.current_frame, orig_frame,
            "Alt+Right must not advance timeline"
        );

        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Left, KeyModifiers::ALT));

        assert_eq!(
            app.side_panel.active_tab,
            TabId::Layers,
            "Alt+Left should cycle side-panel tab backward"
        );
    }

    #[test]
    fn test_bare_up_down_does_nothing_in_layer_panel() {
        let mut app = app_with_frames(1);
        let orig_active = app.editor.layer_stack.active;

        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
        assert_eq!(app.editor.layer_stack.active, orig_active);

        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(app.editor.layer_stack.active, orig_active);
    }

    #[test]
    fn test_tab_cycles_mode_with_layer_groups() {
        let mut app = app_with_frames(1);
        let w = app.editor.canvas.buffer.width();
        let h = app.editor.canvas.buffer.height();
        app.editor.layer_stack.add(w, h);
        app.editor.layer_stack.add(w, h);
        let indices = [0, 1];
        app.editor
            .layer_stack
            .create_group(&indices, "TestGroup".to_string());
        let orig_mode = app.ui.mode;

        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));

        assert!(
            app.ui.mode != orig_mode,
            "bare Tab must cycle app mode even when layer groups exist"
        );
    }

    #[test]
    fn test_alt_s_toggles_cast_shadow() {
        let mut app = app_with_frames(1);
        let layer_idx = app.editor.layer_stack.active;
        let has_shadow_before = app.editor.layer_stack.layers[layer_idx].casts_shadow;

        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Char('S'), KeyModifiers::NONE));
        assert!(
            app.dialogs.settings.settings_open,
            "bare S must open settings dialog"
        );
        assert_eq!(
            app.editor.layer_stack.layers[layer_idx].casts_shadow, has_shadow_before,
            "bare S must not toggle cast shadow"
        );

        app.dialogs.settings.settings_open = false;

        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Char('S'), KeyModifiers::ALT));
        assert_eq!(
            app.editor.layer_stack.layers[layer_idx].casts_shadow, !has_shadow_before,
            "Alt+S must toggle layer cast shadow"
        );
    }
}

#[cfg(test)]
mod timeline_key_wiring_tests {
    use super::super::AppMode;
    use super::*;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn app_with_visible_timeline() -> TuiApp {
        let mut app = TuiApp::new();
        app.welcome.screen.show = false;
        app.ui.mode = AppMode::ImageEditor;
        app.animation.timeline_visible = true;
        app
    }

    fn lighting_app_with_layers_tab() -> TuiApp {
        let mut app = app_with_visible_timeline();
        let none = KeyModifiers::NONE;
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Char('G'), none));
        assert_eq!(app.ui.mode, AppMode::Lighting);
        app.side_panel.open = true;
        app.side_panel.active_tab = crate::tui::side_panel::TabId::Layers;
        app
    }

    #[test]
    fn test_lighting_a_adds_ambient_not_frame_capture() {
        let mut app = lighting_app_with_layers_tab();
        let before = app.lighting.scene.as_ref().unwrap().lights.len();
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Char('A'), KeyModifiers::NONE));
        assert_eq!(
            app.lighting.scene.as_ref().unwrap().lights.len(),
            before + 1
        );
        assert!(app.animation.timeline_state.frames.is_empty());
    }

    #[test]
    fn test_lighting_plus_reaches_light_with_layers_tab_open() {
        let mut app = lighting_app_with_layers_tab();
        let idx = app.lighting.panel.selected_index();
        let get = |a: &TuiApp| {
            crate::tui::light_panel::LightPanel::light_intensity(
                a.lighting.scene.as_ref().unwrap(),
                idx,
            )
        };
        let before = get(&app);
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Char('-'), KeyModifiers::NONE));
        assert_ne!(get(&app), before);
    }

    #[test]
    fn test_a_captures_plain_frame_and_nav_works() {
        let mut app = app_with_visible_timeline();
        let none = KeyModifiers::NONE;
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Char('A'), none));
        assert_eq!(app.animation.timeline_state.frames.len(), 1);
        let f = &app.animation.timeline_state.frames[0];
        assert!(!f.has_keyframe, "captured frames must be plain snapshots");
        assert!(f.layer_keyframes.iter().all(|k| k.is_none()));
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Char('A'), none));
        assert_eq!(app.animation.timeline_state.frames.len(), 2);
        // Right navigates (commit-on-leave must not mint keyframes).
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Right, none));
        assert_eq!(app.animation.timeline_state.current_frame, 1);
        assert!(!app.animation.timeline_state.frames[0].has_keyframe);
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Left, none));
        assert_eq!(app.animation.timeline_state.current_frame, 0);
    }

    #[test]
    fn test_a_captures_every_layer_not_just_composite() {
        use crate::tui::layers::LayerStack;
        let mut app = app_with_visible_timeline();
        app.editor.layer_stack = LayerStack::with_capacity(4, 4, 2);
        // Distinct content on each layer.
        app.editor.layer_stack.layers[0].buffer.set(
            0,
            0,
            canvas::CanvasCell {
                ch: 'A',
                fg: None,
                bg: None,
                height: None,
            },
        );
        app.editor.layer_stack.layers[1].buffer.set(
            1,
            1,
            canvas::CanvasCell {
                ch: 'B',
                fg: None,
                bg: None,
                height: None,
            },
        );

        let none = KeyModifiers::NONE;
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Char('A'), none));
        let f = &app.animation.timeline_state.frames[0];
        assert_eq!(
            f.document_state.len(),
            2,
            "A-key capture must snapshot every layer"
        );
        assert_eq!(f.document_state[0].get(0, 0).map(|c| c.ch), Some('A'));
        assert_eq!(f.document_state[1].get(1, 1).map(|c| c.ch), Some('B'));

        // Capture a second frame, then navigate Left. Navigation commits the
        // *current* frame (F1) before loading F0 — F0's snapshot must still
        // have both layers, not a cleared layer 1.
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Char('A'), none));
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Left, none));
        assert_eq!(
            app.animation.timeline_state.current_frame, 0,
            "Left should land on frame 0"
        );
        assert_eq!(
            app.editor.layer_stack.layers[0]
                .buffer
                .get(0, 0)
                .map(|c| c.ch),
            Some('A'),
            "layer 0 must restore from F0 snapshot"
        );
        assert_eq!(
            app.editor.layer_stack.layers[1]
                .buffer
                .get(1, 1)
                .map(|c| c.ch),
            Some('B'),
            "layer 1 must restore from F0 snapshot, not be cleared"
        );
    }

    #[test]
    fn test_a_captures_when_hidden_but_nav_needs_visible() {
        let mut app = app_with_visible_timeline();
        app.animation.timeline_visible = false;
        let none = KeyModifiers::NONE;
        // Capture works hidden (and opens the timeline itself)...
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Char('A'), none));
        assert_eq!(app.animation.timeline_state.frames.len(), 1);
        assert!(app.animation.timeline_visible);
        // ...but arrows never navigate a hidden panel (two frames so
        // Right *could* advance if the gate were missing).
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Char('A'), none));
        assert_eq!(app.animation.timeline_state.frames.len(), 2);
        app.animation.timeline_state.current_frame = 0;
        app.animation.timeline_visible = false;
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Right, none));
        assert_eq!(app.animation.timeline_state.current_frame, 0);
    }
}

#[cfg(test)]
mod canvas_scroll_tests {
    use super::*;
    use ratatui::crossterm::event::KeyEvent;

    /// Draw one frame, as the event loop does between key events.
    fn draw(app: &mut TuiApp) {
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 24)).unwrap();
        terminal.draw(|f| app.render(f)).unwrap();
    }

    fn big_canvas_app() -> TuiApp {
        let mut app = TuiApp::new();
        app.welcome.screen.show = false;
        app.ui.mode = AppMode::AsciiPreview;
        app.animation.timeline_visible = false;
        app.editor.canvas = canvas::CanvasWidget::new(300, 200);
        app
    }

    fn cursor_on_screen(app: &TuiApp) -> bool {
        let c = &app.editor.canvas;
        let (sx, sy) = c.scroll_offset();
        let (cx, cy) = c.cursor();
        let (vw, vh) = (80, 24);
        cx >= sx && cx < sx + vw && cy >= sy && cy < sy + vh
    }

    #[test]
    fn arrow_keys_scroll_the_rendered_canvas_to_follow_the_cursor() {
        let mut app = big_canvas_app();
        draw(&mut app);
        for _ in 0..120 {
            let _ = app.handle_key_event(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
            assert!(cursor_on_screen(&app));
            draw(&mut app);
        }
        assert_eq!(app.editor.canvas.cursor().0, 120);
        assert!(app.editor.canvas.scroll_offset().0 > 0, "view must scroll");
        for _ in 0..80 {
            let _ = app.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
            draw(&mut app);
        }
        assert_eq!(app.editor.canvas.cursor().1, 80);
        assert!(app.editor.canvas.scroll_offset().1 > 0, "view must scroll");
        for _ in 0..200 {
            let _ = app.handle_key_event(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
            let _ = app.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
            draw(&mut app);
        }
        assert_eq!(app.editor.canvas.cursor(), (0, 0));
        assert_eq!(app.editor.canvas.scroll_offset(), (0, 0));
    }
}
