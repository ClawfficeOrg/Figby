//! Save, Save As and Open, including the file-ops dialog completion.

use super::*;

impl TuiApp {
    pub(super) fn start_save(&mut self) {
        if !self.save_target_is_font() {
            // Image/animation documents save as a .figmap project: in place
            // when the document already has a path, else via Save As.
            match self.editor.figmap_path.clone() {
                Some(path) => self.perform_save(Some(path)),
                None => self.start_save_as_figmap(),
            }
            return;
        }
        if let Some(ref path) = self.editor.font_editor.current_path {
            if let Some(ref font) = self.editor.font_editor.font {
                if self.ctx.throbber.is_active() {
                    return;
                }
                let font = font.clone();
                let path = path.clone();
                self.dialogs.pending_save_revision = Some(self.editor.revision);
                let (tx, rx) = mpsc::channel();
                self.ctx.async_rx = Some(rx);
                self.ctx.throbber.start("Saving...");
                self.frame.dirty = true;
                std::thread::spawn(move || {
                    let result = file_ops::save_font(&font, &path)
                        .map(|_| path)
                        .map_err(|e| e.to_string());
                    let _ = tx.send(AsyncResult::SaveComplete(result));
                });
                return;
            }
        }
        self.start_save_as();
    }

    pub(super) fn start_save_as(&mut self) {
        if !self.save_target_is_font() {
            self.start_save_as_figmap();
            return;
        }
        let default_name = if self.editor.font_editor.font_storage_name.is_empty() {
            "untitled".to_string()
        } else {
            self.editor.font_editor.font_storage_name.clone()
        };
        // A never-saved font (e.g. one built from a system font) starts in
        // the FIGlet font directory the font lookup searches, not the cwd.
        let font_dir = crate::font::resolve_figlet_font_dir();
        self.dialogs.file_ops.enter_save_font_as(
            self.editor.font_editor.current_path.as_deref(),
            &default_name,
            font_dir.as_deref(),
        );
        self.frame.dirty = true;
    }

    /// Open the Save As dialog pre-seeded for a `.figmap` project file.
    /// Unlike `start_save_as` (font mode only), this is reachable from any
    /// mode — image/animation sessions otherwise had no path to the
    /// `.figmap` branch of `perform_save`, which keys off the extension.
    pub(super) fn start_save_as_figmap(&mut self) {
        let current = self.editor.figmap_path.clone();
        let name = current
            .as_deref()
            .and_then(|p| p.file_stem())
            .and_then(|s| s.to_str())
            .unwrap_or("untitled")
            .to_string();
        self.dialogs
            .file_ops
            .enter_save_as_with_extension(current.as_deref(), &name, "figmap");
        self.frame.dirty = true;
    }

    /// Shared completion logic for when the file-ops dialog transitions
    /// back to Idle, regardless of whether that happened via keyboard
    /// (Enter) or mouse (click). `prev_mode` is the mode the dialog was in
    /// just before it closed.
    pub(super) fn complete_file_ops_dialog(
        &mut self,
        prev_mode: file_ops::FileOpsMode,
    ) -> Option<AppEvent> {
        // Esc / cancel closes the dialog too; that must not act on the
        // (just cleared) buffers. A cancelled Save As used to save to
        // ./untitled.flf, and the legacy GIF picker opened the importer.
        if std::mem::take(&mut self.dialogs.file_ops.dismissed) {
            if prev_mode == file_ops::FileOpsMode::SaveAs {
                // The quit/close/new waiting on this save is abandoned too.
                self.dialogs.quit_after_save = false;
                self.dialogs.pending_transition = None;
            }
            self.frame.dirty = true;
            return None;
        }
        // Capture the dialog's target path BEFORE perform_save runs: the
        // dialog is already Idle here, and selected_path() keys off the
        // live mode — reading it late resolves the SaveAs split
        // dir/filename fields as a bare directory + ".flf", silently
        // dropping .figmap saves (font branch no-ops without a font).
        let save_path = (prev_mode == file_ops::FileOpsMode::SaveAs)
            .then(|| self.dialogs.file_ops.save_target_path());
        match prev_mode {
            file_ops::FileOpsMode::SaveAs => {
                self.perform_save(save_path);
                Some(AppEvent::SaveAsRequested)
            }
            file_ops::FileOpsMode::Open => {
                if self.dialogs.file_ops.is_browsing_zip() {
                    self.perform_open();
                    return Some(AppEvent::OpenRequested);
                }
                if self.dialogs.file_ops.path_buffer.trim().is_empty() {
                    return None;
                }
                let path = self.dialogs.file_ops.selected_path();
                if !path.exists() {
                    self.dialogs.file_ops.error_message =
                        format!("File not found: {}", path.display());
                    self.dialogs.file_ops.mode = file_ops::FileOpsMode::Open;
                    return None;
                }
                if path.is_dir() {
                    self.dialogs.file_ops.error_message =
                        "Select a .flf, .tlf, or .figmap file, not a directory".to_string();
                    self.dialogs.file_ops.mode = file_ops::FileOpsMode::Open;
                    return None;
                }
                // Handle .figmap files: load and apply document state
                if path.extension().and_then(|e| e.to_str()) == Some("figmap") {
                    return self.perform_open_figmap(&path);
                }
                self.perform_open();
                Some(AppEvent::OpenRequested)
            }
            file_ops::FileOpsMode::ImportFont => {
                if self.dialogs.file_ops.is_browsing_zip() {
                    // Zip bundles reachable from the font-import flow contain
                    // .flf/.tlf fonts (FIGlet's own zip convention, not
                    // .ttf/.otf) — route through the same Open/ZipEntry
                    // completion the Open dialog uses, not font_gen's TTF
                    // conversion path.
                    self.perform_open();
                    return Some(AppEvent::OpenRequested);
                }
                if self.dialogs.file_ops.path_buffer.trim().is_empty() {
                    return None;
                }
                let path = self.dialogs.file_ops.selected_path();
                if !path.exists() {
                    self.dialogs.file_ops.error_message =
                        format!("File not found: {}", path.display());
                    self.dialogs.file_ops.mode = file_ops::FileOpsMode::ImportFont;
                    return None;
                }
                // Open the size/charset options dialog instead of importing
                // immediately — this flow previously had no options step at
                // all (size/charset were hardcoded).
                self.dialogs.font_import_options.enter(path);
                self.frame.dirty = true;
                None
            }
            file_ops::FileOpsMode::ImportGif => {
                // Legacy path: open the new gif_import dialog instead
                self.dialogs.gif_import.enter();
                None
            }
            file_ops::FileOpsMode::OpenImage => {
                if self.dialogs.file_ops.path_buffer.trim().is_empty() {
                    return None;
                }
                let path = self.dialogs.file_ops.selected_path();
                if !path.exists() {
                    self.dialogs.file_ops.error_message =
                        format!("File not found: {}", path.display());
                    self.dialogs.file_ops.mode = file_ops::FileOpsMode::OpenImage;
                    return None;
                }
                self.perform_open_image(path);
                Some(AppEvent::ImageEditor)
            }
            file_ops::FileOpsMode::Idle => None,
        }
    }

    /// Clone the layer stack with every text block baked into the active layer,
    /// WITHOUT mutating the document. Export and player capture use this so
    /// output sees text pixels while the editable blocks stay live.
    pub(super) fn layer_stack_with_text_baked(&self) -> crate::tui::layers::LayerStack {
        let mut stack = self.editor.layer_stack.clone();
        if self.editor.text_tool.blocks.is_empty() {
            return stack;
        }
        let aidx = stack.active;
        self.editor
            .text_tool
            .bake_blocks_into(&mut stack.layers[aidx].buffer);
        stack
    }

    pub(crate) fn perform_save(&mut self, save_path: Option<std::path::PathBuf>) {
        if self.ctx.throbber.is_active() {
            return;
        }
        let path = save_path.unwrap_or_else(|| self.dialogs.file_ops.selected_path());

        // Handle .figmap save
        if path.extension().and_then(|e| e.to_str()) == Some("figmap") {
            // Text blocks are saved as editable objects — NOT rasterized.
            // Baking happens only at export/player capture, non-destructively.
            let swatch_data = self.palette_editor.lighting_swatches();
            let figmap_timeline = if self.animation.timeline_state.frames.is_empty() {
                None
            } else {
                Some(crate::figmap::FigmapTimeline {
                    fps: self.animation.timeline_state.fps,
                    loop_enabled: false,
                    frames: self.animation.timeline_state.frames.clone(),
                    light_keyframes: self.lighting.light_keyframes.clone(),
                })
            };
            let lights = self
                .lighting
                .scene
                .as_ref()
                .map(|s| s.lights.clone())
                .unwrap_or_default();
            let palette: Vec<crate::palette_import::Swatch> = swatch_data
                .iter()
                .map(|s| crate::palette_import::Swatch {
                    name: String::new(),
                    hex: format!("#{:02X}{:02X}{:02X}", s.lit.0, s.lit.1, s.lit.2),
                    lit_hex: None,
                    shadow_hex: Some(format!(
                        "#{:02X}{:02X}{:02X}",
                        s.shadow.0, s.shadow.1, s.shadow.2
                    )),
                    specular: Some(s.specular),
                    shininess: Some(s.shininess),
                })
                .collect();
            let result = crate::figmap::save_figmap(
                &self.editor.layer_stack,
                figmap_timeline.as_ref(),
                &lights,
                &palette,
                &self.editor.text_tool.blocks,
                &path,
            );
            match result {
                Ok(()) => {
                    self.editor.unsaved = false;
                    self.editor.figmap_path = Some(path);
                    self.ctx.last_save_time = std::time::Instant::now();
                    self.dialogs.file_ops.error_message.clear();
                    self.frame.dirty = true;
                    // Synchronous save: run the quit/close/new that was
                    // waiting behind the unsaved-changes dialog now.
                    if self.dialogs.quit_after_save || self.dialogs.pending_transition.is_some() {
                        self.execute_pending_transition();
                    }
                }
                Err(e) => {
                    // Leave any pending transition armed: the dialog reopens
                    // for a retry, and Esc there disarms it.
                    self.dialogs.file_ops.error_message = format!("Failed to save figmap: {e}");
                    self.dialogs.file_ops.mode = file_ops::FileOpsMode::SaveAs;
                }
            }
            return;
        }

        // Font save (existing)
        let font = match &self.editor.font_editor.font {
            Some(f) => f.clone(),
            None => return,
        };
        let result_path = path.clone();
        self.dialogs.pending_save_revision = Some(self.editor.revision);
        let (tx, rx) = mpsc::channel();
        self.ctx.async_rx = Some(rx);
        self.ctx.throbber.start("Saving...");
        self.frame.dirty = true;
        std::thread::spawn(move || {
            let result = file_ops::save_font(&font, &result_path)
                .map(|_| result_path)
                .map_err(|e| e.to_string());
            let _ = tx.send(AsyncResult::SaveComplete(result));
        });
    }

    pub(crate) fn handle_paste_event(&mut self, text: String) {
        if self.dialogs.file_ops.mode != file_ops::FileOpsMode::Idle {
            self.dialogs.file_ops.handle_paste(&text);
        } else if self.dialogs.export_dialog.active {
            // tmux send-keys -l lands here as bracketed paste: replace the
            // suggested path wholesale (select-all semantics), so pasted
            // absolute paths work instead of appending to "export.png".
            self.dialogs.export_dialog.path_buffer = text;
            self.dialogs.export_dialog.path_selected = false;
        }
    }

    pub(crate) fn start_open(&mut self) {
        self.dialogs
            .file_ops
            .enter_open(self.dialogs.recent_files.list());
        self.frame.dirty = true;
    }

    /// Request opening a font. If the document has unsaved changes, the
    /// open waits behind the unsaved-changes dialog instead of silently
    /// replacing work (GPT review F-08).
    pub(crate) fn perform_open_at(&mut self, path: std::path::PathBuf) {
        if self.ctx.throbber.is_active() {
            return;
        }
        // The file lands in a fresh tab, never over the current document.
        let tab = self.new_document(crate::tui::documents::DocumentKind::Font);
        self.dialogs.pending_open_tab = Some(tab);
        let (tx, rx) = mpsc::channel();
        self.ctx.async_rx = Some(rx);
        self.ctx.throbber.start("Loading...");
        self.frame.dirty = true;
        let path_clone = path.clone();
        std::thread::spawn(move || {
            let result = (|| -> Result<(crate::font::FIGfont, std::path::PathBuf), String> {
                // Same reader as the CLI: bounded, ZIP-aware, and Latin-1
                // tolerant (read_to_string rejected 8-bit fonts outright).
                let font = crate::font::load_font_file(&path_clone).map_err(|e| match e {
                    crate::font::FontError::IoError(io) => format!("Cannot read file: {io}"),
                    other => format!("Parse error: {other}"),
                })?;
                Ok((font, path_clone))
            })();
            let _ = tx.send(AsyncResult::OpenComplete(result));
        });
    }

    pub(super) fn perform_open_figmap(&mut self, path: &std::path::Path) -> Option<AppEvent> {
        // Figmaps open in a new image tab: no unsaved-changes prompt,
        // nothing is replaced.
        self.new_document(crate::tui::documents::DocumentKind::Image);
        match crate::figmap::load_figmap(path) {
            Ok(figmap) => {
                let (layers, timeline, lights, palette, text_blocks) =
                    crate::figmap::into_runtime(figmap);
                self.editor.layer_stack = layers;
                // Text blocks come back as editable objects, not pixels.
                self.editor.text_tool.restore_blocks(text_blocks);
                if let Some(tl) = timeline {
                    self.animation.timeline_state.frames = tl.frames;
                    self.animation.timeline_state.fps = tl.fps;
                    self.lighting.light_keyframes = tl.light_keyframes;
                }
                if !lights.is_empty() {
                    let mut scene = crate::tui::lighting::Scene { lights };
                    scene.ensure_ambient();
                    self.lighting.scene = Some(scene);
                }
                // Restore the saved palette swatches into the palette
                // editor: the lighting LUT/shading path is built from this
                // list, and an empty one shades every cell through swatch 0
                // of a 1-entry default LUT (wrong colors at best, OOB crash
                // at worst). Saved entries may lack lit_hex (older saves);
                // backfill lighting defaults so the LUT ramp is complete.
                if !palette.is_empty() {
                    let mut swatches = palette;
                    for s in &mut swatches {
                        if s.lit_hex.is_none() {
                            s.lit_hex = Some(s.hex.clone());
                        }
                    }
                    self.palette_editor.swatches = swatches;
                    self.palette_editor.name_buffer = "figmap".to_string();
                    self.palette_editor.selected = 0;
                    self.palette_editor.init_lighting_from_swatches();
                    self.palette_editor
                        .apply_to_palette(&mut self.editor.palette);
                    self.rebuild_lighting_from_palette();
                }
                // composite the loaded layers into it (E2E: figmap opened to
                // an empty canvas with only the layer names visible).
                self.editor.recomposite_canvas();
                self.editor.unsaved = false;
                self.editor.figmap_path = Some(path.to_path_buf());
                self.frame.dirty = true;
                self.frame.force_full_redraw = true;
                None
            }
            Err(e) => {
                self.dialogs.file_ops.error_message = format!("Failed to load figmap: {e}");
                self.dialogs.file_ops.mode = file_ops::FileOpsMode::Open;
                None
            }
        }
    }

    pub(crate) fn perform_open(&mut self) {
        if self.ctx.throbber.is_active() {
            return;
        }
        let target = self.dialogs.file_ops.resolve_open_target();
        match target {
            file_ops::OpenTarget::File(path) => {
                // Opens land in a new tab: no unsaved-changes prompt,
                // nothing is replaced.
                self.perform_open_at(path);
            }
            file_ops::OpenTarget::ZipEntry {
                zip_path,
                entry_name,
            } => {
                // ZIP entries are read straight from the archive into a
                // fresh tab recorded for OpenComplete below.
                let tab = self.new_document(crate::tui::documents::DocumentKind::Font);
                self.dialogs.pending_open_tab = Some(tab);
                let (tx, rx) = mpsc::channel();
                self.ctx.async_rx = Some(rx);
                self.ctx.throbber.start("Loading...");
                self.frame.dirty = true;
                std::thread::spawn(move || {
                    let result =
                        (|| -> Result<(crate::font::FIGfont, std::path::PathBuf), String> {
                            let bytes = crate::font::read_zip_entry(&zip_path, &entry_name)
                                .map_err(|e| format!("ZIP read error: {e}"))?;
                            let content = crate::font::decode_font_text(&bytes);
                            let font = crate::font::parse_tlf_font(&content)
                                .map_err(|e| format!("Parse error: {e}"))?;
                            Ok((font, zip_path))
                        })();
                    let _ = tx.send(AsyncResult::OpenComplete(result));
                });
            }
        }
    }
}

#[cfg(test)]
mod open_in_new_tab_tests {
    use super::super::{AppMode, Tool, TuiApp};
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn dirty_app() -> TuiApp {
        let mut app = TuiApp::new();
        app.welcome.screen.show = false;
        app.editor.mark_dirty();
        app
    }

    #[test]
    fn test_open_figmap_never_prompts_and_keeps_current_doc() {
        let mut app = dirty_app();
        let dir = std::env::temp_dir().join("figby-open-tab-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("doc.figmap");
        crate::figmap::save_figmap(&app.editor.layer_stack, None, &[], &[], &[], &path).unwrap();

        app.perform_open_figmap(&path);
        assert_eq!(app.document_count(), 2);
        assert!(!app.dialogs.quit_confirm_dialog, "open must not prompt");
        assert!(!app.editor.unsaved, "fresh tab loads clean");
        assert!(app.documents[0].editor.unsaved, "original tab untouched");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_open_image_failure_still_uses_new_tab() {
        let mut app = dirty_app();
        app.perform_open_image("/nonexistent-figby-test.png".into());
        assert_eq!(app.document_count(), 2);
        assert!(
            !app.dialogs.file_ops.error_message.is_empty(),
            "load error must surface in the dialog"
        );
        // Original dirty document parked untouched in tab 0.
        assert!(app.documents[0].editor.unsaved);
    }

    #[test]
    fn test_open_image_success_loads_into_new_tab() {
        let mut app = dirty_app();
        let dir = std::env::temp_dir().join("figby-open-tab-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("tiny.png");
        let img = image::RgbImage::from_pixel(4, 4, image::Rgb([255, 0, 0]));
        img.save(&path).unwrap();

        app.perform_open_image(path.clone());
        assert_eq!(app.document_count(), 2);
        assert_eq!(app.ui.mode, AppMode::ImageEditor);
        assert!(app.dialogs.file_ops.error_message.is_empty());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_font_open_lands_in_recorded_tab() {
        let mut app = dirty_app();
        let path = std::path::PathBuf::from("assets/fonts/standard.flf");
        assert!(path.exists(), "font fixture must exist under crate dir");
        app.perform_open_at(path);
        // Tab + recording happen synchronously, before the loader thread.
        assert_eq!(app.document_count(), 2);
        assert_eq!(app.dialogs.pending_open_tab, Some(1));
        // Drain the loader thread (bounded wait).
        for _ in 0..200 {
            app.check_async_completion();
            if app.editor.font_editor.font.is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(
            app.editor.font_editor.font.is_some(),
            "async font load must land in the new tab"
        );
        assert!(app.documents[0].editor.unsaved, "original tab untouched");
        assert_eq!(app.dialogs.pending_open_tab, None, "recording consumed");
    }

    #[test]
    fn test_start_open_reachable_outside_font_mode() {
        let mut app = TuiApp::new();
        app.welcome.screen.show = false;
        app.ui.mode = AppMode::ImageEditor;
        app.handle_menu_action(crate::tui::menu::MenuAction::FileOpen);
        assert_eq!(
            app.dialogs.file_ops.mode,
            crate::tui::file_ops::FileOpsMode::Open
        );
    }

    #[test]
    fn test_new_session_reuses_pristine_doc_but_tabs_otherwise() {
        let mut fresh = TuiApp::new();
        fresh.welcome.screen.show = false;
        fresh.do_new_file_session(crate::tui::app_state::SessionType::Image);
        assert_eq!(fresh.document_count(), 1, "pristine doc reused");
        assert_eq!(
            fresh.documents[0].kind,
            crate::tui::documents::DocumentKind::Image,
            "reused tab takes the new kind (icon, Tab cycle)"
        );
        assert_eq!(fresh.ui.mode, AppMode::ImageEditor);

        let mut used = dirty_app();
        used.do_new_file_session(crate::tui::app_state::SessionType::Image);
        assert_eq!(used.document_count(), 2, "dirty doc gets a new tab");
        assert_eq!(used.ui.mode, AppMode::ImageEditor);
    }

    /// U2: welcome → Create Image → Enter used to leave the blank startup
    /// font document behind as a second "Untitled" tab.
    #[test]
    fn test_welcome_create_image_reuses_startup_doc() {
        let mut app = TuiApp::new();
        assert!(app.welcome.screen.show);
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE));
        assert!(app.dialogs.new_image.active, "welcome [C] opens New Image");
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(!app.dialogs.new_image.active);
        assert_eq!(app.document_count(), 1, "no leftover empty tab");
        assert_eq!(
            app.documents[0].kind,
            crate::tui::documents::DocumentKind::Image
        );
        assert_eq!(app.ui.mode, AppMode::ImageEditor);
        assert!(!app.doc_unsaved(0), "a fresh image is not dirty");

        // With real work in the first tab, a new image still gets its own.
        app.editor.mark_dirty();
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL));
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(app.document_count(), 2);
    }

    #[test]
    fn test_ctrl_n_opens_tab_not_replace() {
        let mut app = dirty_app();
        // Ctrl+N opens the New Image dialog instead of replacing.
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL));
        assert!(app.dialogs.new_image.active);
        assert_eq!(app.document_count(), 1, "dialog first, tab on confirm");
    }

    #[test]
    fn test_palette_hex_mode_exits_on_escape() {
        // Regression: 'h' enters custom hex mode, but is_typing() then
        // blocked Enter/Escape from reaching palette.handle_key(), so
        // custom_mode could never reset — user was stuck until restart.
        let mut app = dirty_app();
        app.ui.mode = AppMode::ImageEditor;
        // Enter hex mode.
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE));
        assert!(app.editor.palette.is_typing(), "h should enter hex mode");
        // Escape must exit hex mode.
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(
            !app.editor.palette.is_typing(),
            "Escape should exit hex mode"
        );
    }

    #[test]
    fn test_palette_hex_mode_exits_on_enter() {
        let mut app = dirty_app();
        app.ui.mode = AppMode::ImageEditor;
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE));
        assert!(app.editor.palette.is_typing(), "h should enter hex mode");
        // Enter confirms and exits hex mode.
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(
            !app.editor.palette.is_typing(),
            "Enter should confirm and exit hex mode"
        );
    }

    #[test]
    fn test_palette_hex_mode_accepts_hex_digits() {
        let mut app = dirty_app();
        app.ui.mode = AppMode::ImageEditor;
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE));
        assert!(app.editor.palette.is_typing(), "h should enter hex mode");
        // Typing hex digits should be accepted (not blocked by is_typing guard).
        for ch in ['0', '7', '0', 'B', '1', '8'] {
            let _ = app.handle_key_event(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
        }
        assert!(
            app.editor.palette.is_typing(),
            "still in hex mode after digits"
        );
        // Escape cancels.
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(!app.editor.palette.is_typing(), "Escape cancels hex input");
    }

    #[test]
    fn test_space_paints_with_brush() {
        let mut app = dirty_app();
        app.ui.mode = AppMode::ImageEditor;
        app.editor.toolbox.selected = Tool::Brush;
        let (cx, cy) = app.editor.canvas.cursor();
        let before = app
            .editor
            .layer_stack
            .active_layer()
            .buffer
            .get(cx as usize, cy as usize)
            .copied();
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
        let after = app
            .editor
            .layer_stack
            .active_layer()
            .buffer
            .get(cx as usize, cy as usize)
            .copied();
        assert_ne!(before, after, "Space should paint a dot with Brush");
    }

    #[test]
    fn test_space_paints_with_fill() {
        let mut app = dirty_app();
        app.ui.mode = AppMode::ImageEditor;
        app.editor.toolbox.selected = Tool::Fill;
        let (cx, cy) = app.editor.canvas.cursor();
        let before = app
            .editor
            .layer_stack
            .active_layer()
            .buffer
            .get(cx as usize, cy as usize)
            .copied();
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
        let after = app
            .editor
            .layer_stack
            .active_layer()
            .buffer
            .get(cx as usize, cy as usize)
            .copied();
        assert_ne!(before, after, "Space should paint with Fill");
    }

    #[test]
    fn test_b_selects_brush() {
        let mut app = dirty_app();
        app.ui.mode = AppMode::ImageEditor;
        app.editor.toolbox.selected = Tool::Fill;
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE));
        assert_eq!(
            app.editor.toolbox.selected,
            Tool::Brush,
            "b should select Brush"
        );
    }

    #[test]
    fn test_g_selects_fill() {
        let mut app = dirty_app();
        app.ui.mode = AppMode::ImageEditor;
        app.editor.toolbox.selected = Tool::Brush;
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE));
        assert_eq!(
            app.editor.toolbox.selected,
            Tool::Fill,
            "g should select Fill"
        );
    }

    #[test]
    fn test_e_selects_eraser() {
        let mut app = dirty_app();
        app.ui.mode = AppMode::ImageEditor;
        app.editor.toolbox.selected = Tool::Brush;
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE));
        assert_eq!(
            app.editor.toolbox.selected,
            Tool::Eraser,
            "e should select Eraser"
        );
    }

    #[test]
    fn test_text_tool_type_and_commit() {
        let mut app = dirty_app();
        app.ui.mode = AppMode::ImageEditor;
        app.editor.toolbox.selected = Tool::Text;
        app.editor.text_tool.editing = true;

        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Char('H'), KeyModifiers::NONE));
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE));
        assert_eq!(app.editor.text_tool.text_buffer, "Hi");

        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(app.editor.text_tool.blocks.len(), 1, "should have 1 block");
        assert!(
            !app.editor.text_tool.editing,
            "editing should be false after commit"
        );
    }

    #[test]
    fn test_text_tool_commit_rasterizes_to_canvas() {
        let mut app = dirty_app();
        app.ui.mode = AppMode::ImageEditor;
        app.editor.toolbox.selected = Tool::Text;
        app.editor.text_tool.editing = true;

        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Char('H'), KeyModifiers::NONE));
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE));
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(app.editor.text_tool.blocks.len(), 1);

        // Rasterize the committed block onto the canvas
        use crate::tui::props_panel::PropAction;
        app.dispatch_props_action(PropAction::RasterizeBlock);

        let buf = &app.editor.layer_stack.active_layer().buffer;
        let non_space: usize = (0..buf.height())
            .flat_map(|y| (0..buf.width()).map(move |x| (x, y)))
            .filter(|&(x, y)| buf.get(x, y).map(|c| c.ch != ' ').unwrap_or(false))
            .count();
        assert!(
            non_space > 0,
            "rasterized text should appear on canvas, got {non_space} non-space cells"
        );
    }

    #[test]
    fn test_text_tool_auto_activates_on_typing() {
        // Regression: selecting Text tool and typing did nothing unless
        // the side panel Text tab was open. Now a printable character
        // auto-activates editing mode.
        let mut app = dirty_app();
        app.ui.mode = AppMode::ImageEditor;
        app.editor.toolbox.selected = Tool::Text;
        // editing is false — do NOT set it manually
        assert!(!app.editor.text_tool.editing, "editing starts false");

        // Type a printable character — should auto-activate editing
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::Char('H'), KeyModifiers::NONE));
        assert!(
            app.editor.text_tool.editing,
            "editing should auto-activate on printable char"
        );
        assert_eq!(
            app.editor.text_tool.text_buffer, "H",
            "character should be captured"
        );
    }
}
