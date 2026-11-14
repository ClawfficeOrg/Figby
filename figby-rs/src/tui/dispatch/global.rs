//! Global keymap actions (file ops, view toggles, mode cycling, quit).

use super::*;

impl TuiApp {
    pub(super) fn dispatch_global(&mut self, action: keymap::GlobalAction) -> Option<AppEvent> {
        use keymap::GlobalAction as GA;
        match action {
            GA::FileNew => {
                self.dialogs.new_image.enter_new_image();
                None
            }
            GA::FileOpen => {
                self.start_open();
                None
            }
            GA::FileSave => {
                self.start_save();
                None
            }
            GA::FileSaveAs => {
                self.start_save_as();
                None
            }
            GA::Export => {
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
                self.frame.dirty = true;
                None
            }
            GA::Undo => {
                self.perform_undo();
                Some(AppEvent::Undo)
            }
            GA::Redo => {
                self.perform_redo();
                Some(AppEvent::Redo)
            }
            GA::ToggleUndoPanel => {
                self.dialogs.undo_panel.toggle();
                Some(AppEvent::UndoPanelToggled)
            }
            GA::ToggleRenderMode => {
                self.ctx.render_mode = self.ctx.render_mode.toggle();
                self.frame.dirty = true;
                Some(AppEvent::RenderModeChanged)
            }
            GA::ToggleZenMode => {
                self.ui.zen_mode = !self.ui.zen_mode;
                self.frame.dirty = true;
                None
            }
            GA::CycleDrawer => {
                self.side_panel.toggle_open();
                self.frame.dirty = true;
                None
            }
            GA::ToggleKeybindings => {
                self.ui.show_keybindings = !self.ui.show_keybindings;
                self.frame.dirty = true;
                None
            }
            GA::ToggleFxGraph => {
                self.dialogs.fx_graph.toggle();
                self.frame.dirty = true;
                None
            }
            GA::ToggleTimeline => {
                self.animation.timeline_visible = !self.animation.timeline_visible;
                self.frame.dirty = true;
                None
            }
            GA::OpenTweenPanel => {
                self.animation.timeline_state.open_tween();
                self.frame.dirty = true;
                None
            }
            GA::PlayAnimation => {
                // Start playback from the current frame. Pause/resume is
                // AnimationPlayer::handle_key's job (F8/Space reach it via
                // the inline-player intercept before this arm runs).
                self.start_inline_playback_from_timeline();
                self.frame.dirty = true;
                None
            }
            GA::CycleTabPrev => {
                self.side_panel.cycle_tab(false);
                self.frame.dirty = true;
                None
            }
            GA::CycleTabNext => {
                self.side_panel.cycle_tab(true);
                self.frame.dirty = true;
                None
            }
            GA::DocNext => {
                let n = self.document_count();
                if n > 1 {
                    self.switch_document((self.active_doc + 1) % n);
                }
                None
            }
            GA::DocPrev => {
                let n = self.document_count();
                if n > 1 {
                    self.switch_document((self.active_doc + n - 1) % n);
                }
                None
            }
            GA::DocClose => {
                self.request_close_document(self.active_doc);
                None
            }
            GA::NextMode => {
                self.ui.mode = self.ui.mode.next_for(self.ui.session_type);
                self.editor.undo.clear();
                Some(AppEvent::ModeChanged)
            }
            GA::PrevMode => {
                self.ui.mode = self.ui.mode.prev_for(self.ui.session_type);
                self.editor.undo.clear();
                Some(AppEvent::ModeChanged)
            }
            GA::Quit => {
                self.trigger_quit();
                None
            }
        }
    }
}

#[cfg(test)]
mod keybind_collision_tests {
    use super::*;
    use crate::font::{FIGcharacter, FIGfont};
    use crate::tui::font_editor::FontEditorView;
    use ratatui::crossterm::event::KeyEvent;
    use std::collections::HashMap;

    fn test_font() -> FIGfont {
        FIGfont {
            charheight: 3,
            maxlength: 5,
            chars: HashMap::from([
                (
                    0,
                    FIGcharacter::from(vec![
                        "     ".to_string(),
                        "     ".to_string(),
                        "     ".to_string(),
                    ]),
                ),
                (
                    65,
                    FIGcharacter::from(vec![
                        " AA  ".to_string(),
                        "A  A ".to_string(),
                        "AAAA ".to_string(),
                    ]),
                ),
                (
                    66,
                    FIGcharacter::from(vec![
                        "BBB  ".to_string(),
                        "B  B ".to_string(),
                        "BBB  ".to_string(),
                    ]),
                ),
            ]),
            ..Default::default()
        }
    }

    fn app_font_editor_overview() -> TuiApp {
        let mut app = TuiApp::new();
        app.welcome.screen.show = false;
        app.editor.font_editor.load_font(test_font());
        assert_eq!(app.ui.mode, AppMode::FontEditor);
        assert_eq!(
            app.editor.font_editor.view,
            FontEditorView::Overview,
            "TuiApp::new() should default to FontEditor Overview"
        );
        app
    }

    fn app_not_font_editor() -> TuiApp {
        let mut app = TuiApp::new();
        app.welcome.screen.show = false;
        app.ui.mode = AppMode::ImageEditor;
        app
    }

    // --- T key: Transform editor vs ToggleTimeline ---

    #[test]
    fn test_t_opens_transform_editor_in_font_overview() {
        let mut app = app_font_editor_overview();
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Char('T'), KeyModifiers::NONE));
        assert_eq!(
            app.editor.font_editor.view,
            FontEditorView::TransformEditor,
            "T in FontEditor Overview must open Transform editor"
        );
    }

    #[test]
    fn test_t_does_not_toggle_timeline_in_font_overview() {
        let mut app = app_font_editor_overview();
        let was_visible = app.animation.timeline_visible;
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Char('T'), KeyModifiers::NONE));
        assert_eq!(
            app.animation.timeline_visible, was_visible,
            "T in FontEditor Overview must NOT toggle timeline"
        );
    }

    #[test]
    fn test_t_toggles_timeline_outside_font_editor() {
        let mut app = app_not_font_editor();
        let was_visible = app.animation.timeline_visible;
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Char('T'), KeyModifiers::NONE));
        assert_ne!(
            app.animation.timeline_visible, was_visible,
            "T outside FontEditor must toggle timeline"
        );
    }

    // --- S key: Smushing editor vs Settings ---

    #[test]
    fn test_s_opens_smushing_editor_in_font_overview() {
        let mut app = app_font_editor_overview();
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Char('S'), KeyModifiers::NONE));
        assert_eq!(
            app.editor.font_editor.view,
            FontEditorView::SmushRuleEditor,
            "S in FontEditor Overview must open Smushing rule editor"
        );
    }

    #[test]
    fn test_s_does_not_open_settings_in_font_overview() {
        let mut app = app_font_editor_overview();
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Char('S'), KeyModifiers::NONE));
        assert!(
            !app.dialogs.settings.settings_open,
            "S in FontEditor Overview must NOT open settings"
        );
    }

    // --- W key: edit the font editor preview text ---

    fn press(app: &mut TuiApp, code: KeyCode) {
        let _ = app.handle_key_event(KeyEvent::new(code, KeyModifiers::NONE));
    }

    #[test]
    fn test_w_opens_preview_prompt_in_font_overview() {
        let mut app = app_font_editor_overview();
        press(&mut app, KeyCode::Char('W'));
        assert!(app.editor.font_editor.preview_input_active);
        assert!(!app.editor.font_editor.search_active);
        assert!(app.is_typing(), "open preview prompt counts as typing");
    }

    // --- Overview glyph search: `/` opens it, letters stay commands ---

    #[test]
    fn test_letter_outside_search_reaches_tool_shortcut() {
        let mut app = app_font_editor_overview();
        app.editor.toolbox.selected = crate::tui::toolbox::Tool::Brush;
        press(&mut app, KeyCode::Char('k'));
        assert!(!app.editor.font_editor.search_active, "k started a search");
        assert_eq!(
            app.editor.toolbox.selected,
            crate::tui::toolbox::Tool::Braille
        );
        press(&mut app, KeyCode::Char('x'));
        assert!(!app.editor.font_editor.search_active, "x started a search");
    }

    #[test]
    fn test_slash_search_captures_action_letters_in_app() {
        let mut app = app_font_editor_overview();
        let tool = app.editor.toolbox.selected;
        let timeline = app.animation.timeline_visible;
        press(&mut app, KeyCode::Char('/'));
        assert!(app.editor.font_editor.search_active);
        assert!(app.is_typing(), "active search counts as typing");
        for c in "WHSTkq".chars() {
            press(&mut app, KeyCode::Char(c));
        }
        let fe = &app.editor.font_editor;
        assert_eq!(fe.search_query, "WHSTkq");
        assert!(!fe.preview_input_active);
        assert_eq!(fe.view, FontEditorView::Overview);
        assert!(!app.dialogs.settings.settings_open);
        assert!(!app.ui.should_quit && !app.dialogs.quit_confirm_dialog);
        assert_eq!(app.editor.toolbox.selected, tool);
        assert_eq!(app.animation.timeline_visible, timeline);
        press(&mut app, KeyCode::Esc);
        assert!(!app.editor.font_editor.search_active);
        assert!(app.editor.font_editor.search_query.is_empty());
        assert_eq!(app.ui.mode, AppMode::FontEditor, "Esc only ended search");
    }

    #[test]
    fn test_shift_w_opens_preview_after_search_ends() {
        let mut app = app_font_editor_overview();
        press(&mut app, KeyCode::Char('/'));
        press(&mut app, KeyCode::Char('A'));
        press(&mut app, KeyCode::Esc);
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Char('W'), KeyModifiers::SHIFT));
        assert!(app.editor.font_editor.preview_input_active);
        assert!(!app.editor.font_editor.search_active);
    }

    #[test]
    fn test_overview_action_keys_run_in_app() {
        for (key, view) in [
            ('H', FontEditorView::HeaderEditor),
            ('S', FontEditorView::SmushRuleEditor),
            ('T', FontEditorView::TransformEditor),
        ] {
            let mut app = app_font_editor_overview();
            press(&mut app, KeyCode::Char(key));
            assert_eq!(app.editor.font_editor.view, view, "{key}");
        }
        for key in ['A', 'D', 'C'] {
            let mut app = app_font_editor_overview();
            press(&mut app, KeyCode::Char(key));
            assert!(app.editor.font_editor.code_input_active, "{key}");
            assert!(!app.editor.font_editor.search_active, "{key}");
        }
    }

    #[test]
    fn test_preview_prompt_edit_applies_without_dirty_or_side_effects() {
        let mut app = app_font_editor_overview();
        app.editor.unsaved = false;
        let revision = app.editor.revision;
        let tool = app.editor.toolbox.selected;
        let timeline = app.animation.timeline_visible;
        press(&mut app, KeyCode::Char('W'));
        for _ in 0..crate::tui::font_editor::PREVIEW_STRING.len() {
            press(&mut app, KeyCode::Backspace);
        }
        // q quits, p/b select tools, T toggles the timeline, ? cycles the
        // drawer elsewhere: inside the prompt they are just text.
        for c in "qpbT?".chars() {
            press(&mut app, KeyCode::Char(c));
        }
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.editor.font_editor.preview_text, "qpbT?");
        assert!(!app.editor.font_editor.preview_input_active);
        assert!(!app.ui.should_quit);
        assert!(!app.dialogs.quit_confirm_dialog);
        assert_eq!(app.editor.toolbox.selected, tool);
        assert_eq!(app.animation.timeline_visible, timeline);
        assert!(!app.editor.unsaved, "preview edit must not mark font dirty");
        assert_eq!(app.editor.revision, revision);
    }

    /// Send a char the way a real terminal does: uppercase and shifted
    /// symbols carry SHIFT.
    fn type_char(app: &mut TuiApp, c: char) {
        let mods = if c.is_ascii_uppercase() || "?{}|:\"<>!@#$%^&*()_+~".contains(c) {
            KeyModifiers::SHIFT
        } else {
            KeyModifiers::NONE
        };
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Char(c), mods));
    }

    /// Overview with the drawer open on Layers, as the event loop opens it
    /// on a wide terminal (`default_side_panel_open`).
    fn app_overview_layers_drawer() -> TuiApp {
        let mut app = app_font_editor_overview();
        app.side_panel.open = true;
        app.side_panel.active_tab = crate::tui::side_panel::TabId::Layers;
        app
    }

    fn open_preview_and_clear(app: &mut TuiApp) {
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Char('W'), KeyModifiers::SHIFT));
        assert!(app.editor.font_editor.preview_input_active);
        for _ in 0..crate::tui::font_editor::PREVIEW_STRING.len() {
            press(app, KeyCode::Backspace);
        }
        assert!(app.editor.font_editor.preview_input_buffer.is_empty());
    }

    fn assert_no_side_effects(app: &TuiApp, layers: usize, tool: crate::tui::toolbox::Tool) {
        assert_eq!(app.editor.layer_stack.layers.len(), layers, "layer op ran");
        assert!(
            app.editor.layer_stack.layers.iter().all(|l| l.visible),
            "Enter toggled layer visibility"
        );
        assert!(app
            .editor
            .layer_stack
            .layers
            .iter()
            .all(|l| l.mask.is_none()));
        assert_eq!(app.editor.toolbox.selected, tool);
        assert_eq!(app.ui.mode, AppMode::FontEditor);
        assert_eq!(app.editor.font_editor.view, FontEditorView::Overview);
        assert!(!app.ui.should_quit && !app.dialogs.quit_confirm_dialog);
        assert!(!app.dialogs.settings.settings_open);
        assert!(!app.animation.timeline_state.keyframe_editor.open);
        assert!(app.animation.timeline_state.tween.is_none());
        assert!(app.side_panel.open, "drawer toggled");
    }

    #[test]
    fn test_preview_prompt_types_compewtertutor_with_layers_drawer_open() {
        // User report: typing "CompewterTutor" was cut off at the m (the
        // Layers drawer merged layers) and Enter then toggled layer
        // visibility instead of applying the text.
        let mut app = app_overview_layers_drawer();
        let layers = app.editor.layer_stack.layers.len();
        let tool = app.editor.toolbox.selected;
        open_preview_and_clear(&mut app);
        for c in "CompewterTutorM".chars() {
            type_char(&mut app, c);
        }
        assert_eq!(
            app.editor.font_editor.preview_input_buffer,
            "CompewterTutorM"
        );
        press(&mut app, KeyCode::Backspace);
        press(&mut app, KeyCode::Enter);
        assert!(!app.editor.font_editor.preview_input_active);
        assert_eq!(app.editor.font_editor.preview_text, "CompewterTutor");
        assert_no_side_effects(&app, layers, tool);
    }

    #[test]
    fn test_preview_prompt_types_every_shortcut_char_with_layers_drawer_open() {
        let text = "MTCSHADWQPGIOBVLENRKYU?[]0123456789 mtcshadwqpgiobvlenrkyux,.;'\\+-=/";
        for drawer in [false, true] {
            let mut app = if drawer {
                app_overview_layers_drawer()
            } else {
                app_font_editor_overview()
            };
            let layers = app.editor.layer_stack.layers.len();
            let tool = app.editor.toolbox.selected;
            let brush = app.editor.brush.size;
            open_preview_and_clear(&mut app);
            let mut typed = String::new();
            for c in text.chars() {
                type_char(&mut app, c);
                typed.push(c);
                assert_eq!(
                    app.editor.font_editor.preview_input_buffer, typed,
                    "char {c:?} (drawer {drawer}) was not typed"
                );
            }
            assert_eq!(app.editor.brush.size, brush, "[ or ] resized brush");
            press(&mut app, KeyCode::Enter);
            assert_eq!(app.editor.font_editor.preview_text, text);
            if drawer {
                assert_no_side_effects(&app, layers, tool);
            }
        }
    }

    #[test]
    fn test_preview_prompt_esc_and_ctrl_combos_with_layers_drawer_open() {
        let mut app = app_overview_layers_drawer();
        open_preview_and_clear(&mut app);
        type_char(&mut app, 'm');
        // Ctrl combos stay global while the prompt is open.
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL));
        assert!(app.dialogs.export_dialog.active, "Ctrl+E still exports");
        press(&mut app, KeyCode::Esc); // closes the export dialog only
        assert!(!app.dialogs.export_dialog.active);
        assert!(app.editor.font_editor.preview_input_active);
        assert_eq!(app.editor.font_editor.preview_input_buffer, "m");
        press(&mut app, KeyCode::Esc);
        assert!(!app.editor.font_editor.preview_input_active);
        assert_eq!(
            app.editor.font_editor.preview_text,
            crate::tui::font_editor::PREVIEW_STRING
        );
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL));
        assert!(app.ui.should_quit || app.dialogs.quit_confirm_dialog);
    }

    #[test]
    fn test_preview_prompt_esc_cancels_in_app() {
        let mut app = app_font_editor_overview();
        press(&mut app, KeyCode::Char('W'));
        press(&mut app, KeyCode::Char('x'));
        press(&mut app, KeyCode::Esc);
        assert!(!app.editor.font_editor.preview_input_active);
        assert_eq!(
            app.editor.font_editor.preview_text,
            crate::tui::font_editor::PREVIEW_STRING
        );
        assert!(!app.editor.unsaved);
    }

    #[test]
    fn test_preview_prompt_empty_resets_default_in_app() {
        let mut app = app_font_editor_overview();
        app.editor.font_editor.preview_text = "AB".to_string();
        press(&mut app, KeyCode::Char('W'));
        press(&mut app, KeyCode::Backspace);
        press(&mut app, KeyCode::Backspace);
        press(&mut app, KeyCode::Enter);
        assert_eq!(
            app.editor.font_editor.preview_text,
            crate::tui::font_editor::PREVIEW_STRING
        );
    }

    #[test]
    fn test_s_opens_settings_outside_font_editor() {
        let mut app = app_not_font_editor();
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Char('S'), KeyModifiers::NONE));
        assert!(
            app.dialogs.settings.settings_open,
            "S outside FontEditor must open settings"
        );
    }

    // --- Toolbox catch-all exclusion ---

    #[test]
    fn test_t_not_consumed_by_toolbox_in_image_editor() {
        let mut app = app_not_font_editor();
        app.editor.toolbox.selected = crate::tui::toolbox::Tool::Brush;
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Char('T'), KeyModifiers::NONE));
        // T should toggle timeline, not select a tool
        assert_ne!(
            app.editor.toolbox.selected,
            crate::tui::toolbox::Tool::Text,
            "T must not be consumed by toolbox catch-all"
        );
    }

    #[test]
    fn test_s_not_consumed_by_toolbox_in_image_editor() {
        let mut app = app_not_font_editor();
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Char('S'), KeyModifiers::NONE));
        // S should open settings, not be consumed by toolbox catch-all
        assert!(
            app.dialogs.settings.settings_open,
            "S must not be consumed by toolbox catch-all"
        );
    }

    // --- 2026-11-04 review F9: unsaved-changes dialog must never lose work ---

    #[test]
    fn test_esc_in_save_as_does_not_save() {
        // Regression: Esc closed the Save As dialog, complete_file_ops_dialog
        // then ran perform_save on the cleared buffers — i.e. a silent save
        // to ./untitled.flf with no overwrite guard.
        let mut app = app_font_editor_overview();
        app.editor.unsaved = true;
        app.start_save_as();
        assert_eq!(
            app.dialogs.file_ops.mode,
            crate::tui::file_ops::FileOpsMode::SaveAs
        );
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(
            app.dialogs.file_ops.mode,
            crate::tui::file_ops::FileOpsMode::Idle
        );
        assert!(
            !app.ctx.throbber.is_active() && app.dialogs.pending_save_revision.is_none(),
            "cancelling Save As must not start a save"
        );
        assert!(app.editor.unsaved);
    }

    #[test]
    fn test_save_as_new_font_starts_in_figlet_font_dir() {
        let mut app = app_font_editor_overview();
        app.editor.font_editor.font_storage_name = "Bear Days".to_string();
        app.start_save_as();
        let expected = crate::font::resolve_figlet_font_dir()
            .map(|d| d.to_string_lossy().to_string())
            .unwrap_or_default();
        assert_eq!(app.dialogs.file_ops.path_buffer, expected);
        assert_eq!(app.dialogs.file_ops.filename_buffer, "Bear Days.flf");
    }

    #[test]
    fn test_save_as_confirm_writes_shown_path_exactly_once() {
        let key = |app: &mut TuiApp, code| {
            let _ = app.handle_key_event(KeyEvent::new(code, KeyModifiers::NONE));
        };
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("child")).unwrap();
        let mut app = app_font_editor_overview();
        app.editor.unsaved = true;
        app.editor.font_editor.font_storage_name = "Bear Days".to_string();
        app.start_save_as();
        // Point the dialog at the temp dir as if it were the font dir.
        app.dialogs
            .file_ops
            .enter_save_font_as(None, "Bear Days", Some(dir.path()));

        key(&mut app, KeyCode::Down); // highlight child/
        key(&mut app, KeyCode::Enter); // open it
        assert_eq!(
            app.dialogs.file_ops.mode,
            crate::tui::file_ops::FileOpsMode::SaveAs
        );
        assert!(app.dialogs.pending_save_revision.is_none(), "no save yet");
        let shown = app.dialogs.file_ops.save_target_display();
        assert_eq!(shown, dir.path().join("child").join("Bear Days.flf"));

        key(&mut app, KeyCode::Tab); // filename field
        key(&mut app, KeyCode::Enter); // confirm
        assert_eq!(
            app.dialogs.file_ops.mode,
            crate::tui::file_ops::FileOpsMode::Idle
        );
        let mut completions = 0;
        for _ in 0..300 {
            if app.ctx.async_rx.is_none() {
                break;
            }
            app.check_async_completion();
            if app.ctx.async_rx.is_none() {
                completions += 1;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(completions, 1, "exactly one save ran");
        assert!(app.ctx.async_rx.is_none(), "no second save queued");
        assert!(shown.is_file(), "font written to the shown path");
        assert_eq!(
            app.editor.font_editor.current_path.as_deref(),
            Some(shown.as_path())
        );
        assert!(!app.editor.unsaved);
        let stray: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .filter(|e| e.path().is_file())
            .collect();
        assert!(stray.is_empty(), "nothing saved into the parent folder");
    }

    #[test]
    fn test_cancelled_save_as_from_quit_prompt_disarms_quit() {
        use crate::tui::app_state::PendingDocAction;
        let mut app = app_not_font_editor();
        app.editor.unsaved = true;
        app.trigger_quit();
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));
        // No figmap path yet: Save goes through Save As.
        assert_eq!(
            app.dialogs.file_ops.mode,
            crate::tui::file_ops::FileOpsMode::SaveAs
        );
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(!app.ui.should_quit);
        assert!(!app.dialogs.quit_after_save);
        assert_eq!(app.dialogs.pending_transition, None::<PendingDocAction>);
        assert!(app.editor.unsaved);
    }

    #[test]
    fn test_enter_in_quit_prompt_never_discards_image_work() {
        // Regression: in image mode Save was "unavailable", so Enter/Y fell
        // through to "treat as discard" and quit, dropping the drawing.
        let mut app = app_not_font_editor();
        app.editor.unsaved = true;
        app.trigger_quit();
        assert!(app.can_save_now(), "image documents save as .figmap");
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(!app.ui.should_quit, "Enter must not discard and quit");
        assert!(app.editor.unsaved);
    }

    #[test]
    fn test_save_and_quit_image_document_writes_figmap() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("art.figmap");
        let mut app = app_not_font_editor();
        app.editor.unsaved = true;
        app.editor.figmap_path = Some(path.clone());
        app.trigger_quit();
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));
        assert!(
            path.exists(),
            "save must land at the document's figmap path"
        );
        assert!(crate::figmap::load_figmap(&path).is_ok());
        assert!(!app.editor.unsaved);
        assert!(app.ui.should_quit, "successful save then quits");
    }

    #[test]
    fn test_figmap_save_as_remembers_path_for_next_save() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("remember.figmap");
        let mut app = app_not_font_editor();
        app.perform_save(Some(path.clone()));
        assert_eq!(app.editor.figmap_path.as_deref(), Some(path.as_path()));
        // Ctrl+S now saves in place instead of reopening Save As.
        std::fs::remove_file(&path).unwrap();
        app.editor.unsaved = true;
        app.start_save();
        assert!(path.exists());
        assert_eq!(
            app.dialogs.file_ops.mode,
            crate::tui::file_ops::FileOpsMode::Idle
        );
        assert_eq!(app.doc_title(app.active_doc), "remember");
    }

    #[test]
    fn test_quit_prompt_buttons_track_label_text() {
        // Hit boxes used to be hard-coded at columns 2/22/45, so with Save
        // hidden (or "continue" instead of "quit") they sat on the wrong
        // labels. Check every rect covers the label it acts for.
        fn check(app: &mut TuiApp) {
            let verb = if app.dialogs.pending_transition
                == Some(crate::tui::app_state::PendingDocAction::Quit)
            {
                "quit"
            } else {
                "continue"
            };
            let mut terminal =
                ratatui::Terminal::new(ratatui::backend::TestBackend::new(120, 40)).unwrap();
            terminal.draw(|f| app.render(f)).unwrap();
            let buf = terminal.backend().buffer().clone();
            let labels = [
                format!("[Y] Save and {verb}"),
                format!("[N] Discard and {verb}"),
                "[C] Cancel".to_string(),
            ];
            for (rect, label) in app.dialogs.quit_confirm_buttons.iter().zip(labels) {
                if rect.width == 0 {
                    continue;
                }
                let text: String = (rect.x..rect.x + rect.width)
                    .map(|x| buf[(x, rect.y)].symbol().to_string())
                    .collect();
                assert_eq!(text, label);
            }
        }
        let mut app = app_not_font_editor();
        app.editor.unsaved = true;
        app.trigger_quit();
        check(&mut app);
        assert!(
            app.dialogs.quit_confirm_buttons[0].width > 0,
            "image docs offer Save"
        );

        let mut font_app = TuiApp::new();
        font_app.welcome.screen.show = false;
        font_app.editor.unsaved = true;
        font_app.request_transition(crate::tui::app_state::PendingDocAction::NewFileSession);
        check(&mut font_app); // no font loaded: Save hidden, "continue" labels
        assert_eq!(font_app.dialogs.quit_confirm_buttons[0].width, 0);
    }

    /// Open the quit prompt on an unsaved image doc and render it once so
    /// the button rects are stored.
    fn quit_prompt_app() -> (TuiApp, ratatui::Terminal<ratatui::backend::TestBackend>) {
        let mut app = app_not_font_editor();
        app.editor.unsaved = true;
        app.trigger_quit();
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(120, 40)).unwrap();
        terminal.draw(|f| app.render(f)).unwrap();
        (app, terminal)
    }

    fn quit_mouse(
        kind: ratatui::crossterm::event::MouseEventKind,
        column: u16,
        row: u16,
    ) -> ratatui::crossterm::event::MouseEvent {
        ratatui::crossterm::event::MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    /// A cell in the middle of the label text, well past the `[X]` letter.
    fn label_middle(r: ratatui::layout::Rect) -> (u16, u16) {
        (r.x + r.width / 2 + 2, r.y)
    }

    #[test]
    fn test_quit_prompt_click_full_label_activates_each_button() {
        use ratatui::crossterm::event::{MouseButton, MouseEventKind};
        let down = MouseEventKind::Down(MouseButton::Left);

        // Save: no figmap path yet, so it opens Save As.
        let (mut app, _t) = quit_prompt_app();
        let (x, y) = label_middle(app.dialogs.quit_confirm_buttons[0]);
        assert!(
            x >= app.dialogs.quit_confirm_buttons[0].x + 3,
            "past the letter"
        );
        app.handle_mouse_event(quit_mouse(down, x, y));
        assert!(!app.dialogs.quit_confirm_dialog);
        assert_eq!(
            app.dialogs.file_ops.mode,
            crate::tui::file_ops::FileOpsMode::SaveAs
        );
        assert!(app.dialogs.quit_after_save);

        // Discard: proceeds with the quit.
        let (mut app, _t) = quit_prompt_app();
        let (x, y) = label_middle(app.dialogs.quit_confirm_buttons[1]);
        app.handle_mouse_event(quit_mouse(down, x, y));
        assert!(!app.dialogs.quit_confirm_dialog);
        assert!(app.ui.should_quit);

        // Cancel: closes the prompt and disarms the transition.
        let (mut app, _t) = quit_prompt_app();
        let r = app.dialogs.quit_confirm_buttons[2];
        app.handle_mouse_event(quit_mouse(down, r.x + r.width - 1, r.y));
        assert!(!app.dialogs.quit_confirm_dialog);
        assert!(!app.ui.should_quit);
        assert_eq!(app.dialogs.pending_transition, None);
        assert!(app.editor.unsaved);
    }

    #[test]
    fn test_quit_prompt_hover_highlights_full_label() {
        use ratatui::crossterm::event::MouseEventKind;
        let (mut app, mut terminal) = quit_prompt_app();
        let r = app.dialogs.quit_confirm_buttons[1];
        let (x, y) = label_middle(r);
        app.handle_mouse_event(quit_mouse(MouseEventKind::Moved, x, y));
        assert_eq!(app.dialogs.quit_confirm_hovered, Some(1));
        assert!(app.dialogs.quit_confirm_dialog, "hover never acts");
        assert!(!app.ui.should_quit);
        terminal.draw(|f| app.render(f)).unwrap();
        let buf = terminal.backend().buffer().clone();
        let sel = app.ctx.theme.dialog.selected_bg;
        for cx in r.x..r.x + r.width {
            assert_eq!(buf[(cx, r.y)].bg, sel, "whole label highlighted");
        }
        let other = app.dialogs.quit_confirm_buttons[2];
        assert_ne!(buf[(other.x, other.y)].bg, sel);

        // Moving off every button clears the highlight.
        app.handle_mouse_event(quit_mouse(MouseEventKind::Moved, 0, 0));
        assert_eq!(app.dialogs.quit_confirm_hovered, None);
    }

    #[test]
    fn test_quit_prompt_click_outside_buttons_does_nothing() {
        use ratatui::crossterm::event::{MouseButton, MouseEventKind};
        let down = MouseEventKind::Down(MouseButton::Left);
        let (mut app, _t) = quit_prompt_app();
        let [save, discard, _] = app.dialogs.quit_confirm_buttons;
        // The gap between Save and Discard, the row above, and far away.
        for (x, y) in [
            (save.x + save.width + 1, save.y),
            (discard.x + 2, discard.y - 1),
            (0, 0),
        ] {
            app.handle_mouse_event(quit_mouse(down, x, y));
            assert!(app.dialogs.quit_confirm_dialog, "click at ({x},{y}) acted");
        }
        assert!(!app.ui.should_quit);
        assert_eq!(
            app.dialogs.file_ops.mode,
            crate::tui::file_ops::FileOpsMode::Idle
        );
        assert_eq!(
            app.dialogs.pending_transition,
            Some(crate::tui::app_state::PendingDocAction::Quit)
        );
    }

    #[test]
    fn test_quit_prompt_keys_unchanged_by_hover() {
        use ratatui::crossterm::event::MouseEventKind;
        // Hovering Cancel must not change what the keys do.
        let (mut app, _t) = quit_prompt_app();
        let (x, y) = label_middle(app.dialogs.quit_confirm_buttons[2]);
        app.handle_mouse_event(quit_mouse(MouseEventKind::Moved, x, y));
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE));
        assert!(app.ui.should_quit, "N still discards and quits");

        let (mut app, _t) = quit_prompt_app();
        let (x, y) = label_middle(app.dialogs.quit_confirm_buttons[1]);
        app.handle_mouse_event(quit_mouse(MouseEventKind::Moved, x, y));
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(!app.dialogs.quit_confirm_dialog && !app.ui.should_quit);
        assert_eq!(app.dialogs.pending_transition, None);

        let (mut app, _t) = quit_prompt_app();
        let (x, y) = label_middle(app.dialogs.quit_confirm_buttons[1]);
        app.handle_mouse_event(quit_mouse(MouseEventKind::Moved, x, y));
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(!app.ui.should_quit, "Enter still saves, never discards");
        assert_eq!(
            app.dialogs.file_ops.mode,
            crate::tui::file_ops::FileOpsMode::SaveAs
        );
    }

    #[test]
    fn test_save_as_figmap_reachable_outside_font_mode() {
        // Regression: image/animation sessions had no path to the .figmap
        // branch of perform_save (start_save_as is font-gated).
        let mut app = app_not_font_editor();
        app.start_save_as_figmap();
        assert_eq!(
            app.dialogs.file_ops.mode,
            crate::tui::file_ops::FileOpsMode::SaveAs
        );
        assert_eq!(app.dialogs.file_ops.filename_buffer, "untitled.figmap");
    }

    #[test]
    fn test_doc_tab_click_switches_document() {
        use ratatui::crossterm::event::KeyModifiers;
        use ratatui::crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
        use ratatui::layout::Rect;

        let mut app = app_not_font_editor();
        app.new_document(crate::tui::documents::DocumentKind::Font);
        assert_eq!(app.active_doc, 1);
        // Seed the hit rects the render pass would have stored.
        app.ui.tab_rects = vec![Rect::new(0, 1, 12, 1), Rect::new(13, 1, 12, 1)];
        let click = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 3,
            row: 1,
            modifiers: KeyModifiers::NONE,
        };
        app.handle_mouse_event(click);
        assert_eq!(app.active_doc, 0, "click on first tab must switch");
        // Clicking empty chrome (no rect) changes nothing.
        let miss = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 100,
            row: 1,
            modifiers: KeyModifiers::NONE,
        };
        app.handle_mouse_event(miss);
        assert_eq!(app.active_doc, 0);
    }

    #[test]
    fn test_doc_tab_keys_cycle_and_close() {
        let mut app = app_not_font_editor();
        app.new_document(crate::tui::documents::DocumentKind::Font);
        assert_eq!(app.document_count(), 2);
        // Ctrl+PageDown wraps around the tab list.
        let pgdn = KeyEvent::new(KeyCode::PageDown, KeyModifiers::CONTROL);
        let _ = app.handle_key_event(pgdn);
        assert_eq!(app.active_doc, 0);
        let pgup = KeyEvent::new(KeyCode::PageUp, KeyModifiers::CONTROL);
        let _ = app.handle_key_event(pgup);
        assert_eq!(app.active_doc, 1);
        // Ctrl+W closes the clean tab at once.
        let ctrl_w = KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL);
        let _ = app.handle_key_event(ctrl_w);
        assert_eq!(app.document_count(), 1);
    }

    #[test]
    fn test_doc_close_unsaved_prompts_first() {
        use crate::tui::app_state::PendingDocAction;

        let mut app = app_not_font_editor();
        app.new_document(crate::tui::documents::DocumentKind::Font);
        app.editor.unsaved = true;
        app.request_close_document(app.active_doc);
        assert!(app.dialogs.quit_confirm_dialog);
        assert_eq!(
            app.dialogs.pending_transition,
            Some(PendingDocAction::CloseDocument(1))
        );
        assert_eq!(app.document_count(), 2, "prompt must precede close");
        // Discard proceeds.
        app.execute_pending_transition();
        assert_eq!(app.document_count(), 1);
    }

    #[test]
    fn test_menu_new_tab_and_close_tab() {
        use crate::tui::menu::MenuAction;

        let mut app = app_not_font_editor();
        app.handle_menu_action(MenuAction::FileNewTabImage);
        assert_eq!(app.document_count(), 2);
        assert_eq!(app.ui.mode, AppMode::ImageEditor);
        app.handle_menu_action(MenuAction::FileNewTabFont);
        assert_eq!(app.document_count(), 3);
        app.handle_menu_action(MenuAction::FileCloseTab);
        assert_eq!(app.document_count(), 2);
    }

    #[test]
    fn test_menu_new_animation_opens_image_tab_with_timeline() {
        use crate::tui::menu::MenuAction;

        let mut app = app_not_font_editor();
        assert!(!app.animation.timeline_visible);
        app.handle_menu_action(MenuAction::FileNewAnimation);
        assert_eq!(app.document_count(), 2);
        assert_eq!(app.ui.mode, AppMode::ImageEditor);
        assert!(
            app.animation.timeline_visible,
            "new animation must show the timeline"
        );
    }
}
