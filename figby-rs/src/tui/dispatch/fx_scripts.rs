//! `.figfx.toml` effect scripts in the app (docs/fx-scripting-plan.md,
//! phase 2): loading (Open dialog, or the per-user default at startup),
//! hot reload, the `on_open`/`on_save`/`on_key`/`loop` triggers, and
//! starting queued effects once per frame with fresh targets.

use super::*;
use crate::tui::fx_script::host::slots_to_cancel;
use crate::tui::fx_script::script::{is_script_path, parse_script, FxScriptError};
use crate::tui::fx_script::script_file::{user_script_path, FxScriptFile};
use crate::tui::fx_script::{compile::compile, FxScriptHost, FxStatus, FxTrigger};
use std::ops::ControlFlow;
use std::path::Path;
use std::time::Instant;

impl TuiApp {
    fn fx_script_error(&mut self, file: &str, e: &FxScriptError) {
        self.frame.effects.status = Some(FxStatus {
            text: format!("fx {file}:{e}"),
            is_error: true,
        });
        self.frame.dirty = true;
    }

    /// Load the script at `path`, replacing any loaded one. A script that
    /// fails to parse is still watched (as an empty script), so fixing the
    /// file loads it.
    pub(crate) fn load_fx_script(&mut self, path: &Path) {
        self.frame.dirty = true;
        let (file, text) = match FxScriptFile::open(path) {
            Ok(v) => v,
            Err(e) => {
                self.frame.effects.status = Some(FxStatus {
                    text: format!("fx: cannot read {}: {e}", path.display()),
                    is_error: true,
                });
                return;
            }
        };
        let name = file.display_name();
        if let Some(old) = self.frame.effects.script.take() {
            for slot in slots_to_cancel(&old.script, &Default::default()) {
                self.frame.effects.cancel(slot);
            }
        }
        match parse_script(&text) {
            Ok(script) => {
                let n = script.effects.len();
                self.frame.effects.script = Some(FxScriptHost::new(script, Some(file)));
                self.frame.effects.status = Some(FxStatus {
                    text: format!("fx: loaded {name} ({n} effects)"),
                    is_error: false,
                });
            }
            Err(e) => {
                self.frame.effects.script = Some(FxScriptHost::new(Default::default(), Some(file)));
                self.fx_script_error(&name, &e);
            }
        }
    }

    /// Open dialog: a `.figfx.toml` loads as the effect script instead of
    /// opening a document. Returns whether `path` was a script.
    pub(crate) fn open_fx_script_from_dialog(&mut self, path: &Path) -> bool {
        if !is_script_path(path) {
            return false;
        }
        self.load_fx_script(path);
        true
    }

    /// Load `<config dir>/effects.figfx.toml` if it exists (startup).
    pub(crate) fn load_user_fx_script(&mut self) {
        if let Some(path) = user_script_path().filter(|p| p.is_file()) {
            self.load_fx_script(&path);
        }
    }

    /// Hot reload: re-read the script if its file changed. A broken edit
    /// keeps the previous version running and reports the error.
    pub(crate) fn poll_fx_script(&mut self, now: Instant) {
        let Some(host) = self.frame.effects.script.as_mut() else {
            return;
        };
        let Some(file) = host.file.as_mut() else {
            return;
        };
        let Some(text) = file.poll(now) else {
            return;
        };
        let name = file.display_name();
        self.apply_fx_script_text(&name, &text);
    }

    /// Swap reloaded `text` into the loaded script.
    pub(crate) fn apply_fx_script_text(&mut self, name: &str, text: &str) {
        self.frame.dirty = true;
        match parse_script(text) {
            Ok(script) => {
                let Some(mut host) = self.frame.effects.script.take() else {
                    return;
                };
                for slot in slots_to_cancel(&host.script, &script) {
                    self.frame.effects.cancel(slot);
                }
                let n = script.effects.len();
                host.replace(script);
                self.frame.effects.script = Some(host);
                self.frame.effects.status = Some(FxStatus {
                    text: format!("fx: reloaded {name} ({n} effects)"),
                    is_error: false,
                });
            }
            Err(e) => self.fx_script_error(name, &e),
        }
    }

    /// Queue the loaded script's effects for `trigger` (`on_open`,
    /// `on_save`). They start at the next frame.
    pub(crate) fn fire_fx_trigger(&mut self, trigger: FxTrigger) {
        if let Some(host) = self.frame.effects.script.as_mut() {
            if host.fire(trigger) {
                self.frame.dirty = true;
            }
        }
    }

    /// Play one script effect by index (Effects tab).
    pub(crate) fn play_fx_script_effect(&mut self, idx: usize) {
        if let Some(host) = self.frame.effects.script.as_mut() {
            host.play(idx);
            self.frame.dirty = true;
        }
    }

    /// Stop one script effect by index (Effects tab).
    pub(crate) fn stop_fx_script_effect(&mut self, idx: usize) {
        if let Some(slot) = self.frame.effects.script.as_mut().and_then(|h| h.stop(idx)) {
            self.frame.effects.cancel(slot);
            self.frame.dirty = true;
        }
    }

    /// Start every queued effect, and restart idle loops. Called from
    /// `render` after the frame's layout is known, so targets are current.
    /// A compile error (a target can only fail here if the DSL depends on
    /// it) is shown in the status bar; a failing loop is stopped.
    pub(crate) fn pump_fx_script(&mut self) {
        let Some(mut host) = self.frame.effects.script.take() else {
            return;
        };
        let due = host.take_due(|slot| self.frame.effects.slot_running(slot));
        if !due.is_empty() {
            let targets = self.fx_targets();
            for idx in due {
                let effect = &host.script.effects[idx];
                match compile(&effect.source, &targets) {
                    Ok(fx) => self.frame.effects.run(host.script.slot_of(idx), fx),
                    Err(e) => {
                        let first = e.message.lines().next().unwrap_or_default();
                        self.frame.effects.status = Some(FxStatus {
                            text: format!("fx `{}` {}:{}: {first}", effect.name, e.line, e.column),
                            is_error: true,
                        });
                        host.disable_loop(idx);
                    }
                }
            }
        }
        self.frame.effects.script = Some(host);
    }

    /// `on_key` effects: a bound key starts its effects and is consumed.
    /// Runs after modal dialogs and text entry, so typing into a field
    /// never fires a script.
    pub(super) fn key_fx_script(&mut self, key: KeyEvent) -> ControlFlow<Option<AppEvent>> {
        let fired = self
            .frame
            .effects
            .script
            .as_mut()
            .is_some_and(|host| host.fire_key(&key));
        if fired {
            self.frame.dirty = true;
            ControlFlow::Break(None)
        } else {
            ControlFlow::Continue(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::fx_script::FxSlot;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use std::time::{Duration, SystemTime};

    fn app() -> TuiApp {
        let mut app = TuiApp::new();
        app.welcome.screen.show = false;
        app.welcome.fx = None;
        app.welcome.fade_in = None;
        app
    }

    fn draw(app: &mut TuiApp) {
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
        terminal.draw(|f| app.render(f)).unwrap();
    }

    fn script_file(tag: &str, text: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("figby-fx-app-{}-{tag}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.figfx.toml");
        std::fs::write(&path, text).unwrap();
        path
    }

    fn bump_mtime(path: &Path, secs: u64) {
        let f = std::fs::OpenOptions::new().write(true).open(path).unwrap();
        f.set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000 + secs))
            .unwrap();
    }

    const SCRIPT: &str = r#"
[[effect]]
name = "opened"
trigger = "on_open"
source = "fx::sleep(10000)"

[[effect]]
name = "saved"
trigger = "on_save"
source = "fx::sleep(10000)"

[[effect]]
name = "key"
trigger = "on_key:F8"
source = "fx::sequence(&[fx::delay(100, fx::dissolve(100)), fx::parallel(&[fx::sleep(10000), fx::coalesce(50)])]).with_area(canvas)"

[[effect]]
name = "spin"
trigger = "loop"
source = "fx::sleep(50)"
"#;

    fn slot(app: &TuiApp, name: &str) -> FxSlot {
        let host = app.frame.effects.script.as_ref().unwrap();
        host.script.slot_of(host.script.index_of(name).unwrap())
    }

    fn running(app: &TuiApp, name: &str) -> bool {
        app.frame.effects.slot_running(slot(app, name))
    }

    #[test]
    fn loading_reports_and_starts_loops_on_the_next_frame() {
        let mut app = app();
        app.load_fx_script(&script_file("load", SCRIPT));
        let status = app.frame.effects.status.clone().unwrap();
        assert!(!status.is_error, "{status:?}");
        assert!(status.text.contains("4 effects"), "{}", status.text);
        assert!(
            app.frame.effects.needs_redraw(),
            "a loop is waiting to start"
        );
        draw(&mut app);
        assert!(running(&app, "spin"));
        assert!(!running(&app, "opened"));
        // The 50 ms loop is restarted after it ends.
        for _ in 0..10 {
            std::thread::sleep(Duration::from_millis(15));
            draw(&mut app);
        }
        assert!(running(&app, "spin"));
        app.stop_fx_script_effect(
            app.frame
                .effects
                .script
                .as_ref()
                .unwrap()
                .script
                .index_of("spin")
                .unwrap(),
        );
        assert!(!running(&app, "spin"));
    }

    #[test]
    fn open_save_and_key_triggers_start_their_effects() {
        let mut app = app();
        app.load_fx_script(&script_file("triggers", SCRIPT));
        draw(&mut app);
        app.fire_fx_trigger(FxTrigger::OnOpen);
        assert!(!running(&app, "opened"), "starts at the next frame");
        draw(&mut app);
        assert!(running(&app, "opened"));
        assert!(!running(&app, "saved"));
        app.fire_fx_trigger(FxTrigger::OnSave);
        draw(&mut app);
        assert!(running(&app, "saved"));
        // F8 is consumed by the script and starts the sequence.
        let ev = app.handle_key_event(KeyEvent::new(KeyCode::F(8), KeyModifiers::NONE));
        assert!(ev.is_none());
        draw(&mut app);
        assert!(running(&app, "key"));
    }

    #[test]
    fn key_trigger_does_not_fire_inside_the_fx_prompt() {
        let mut app = app();
        app.load_fx_script(&script_file("prompt", SCRIPT));
        app.open_fx_prompt();
        let _ = app.handle_key_event(KeyEvent::new(KeyCode::F(8), KeyModifiers::NONE));
        draw(&mut app);
        assert!(!running(&app, "key"));
    }

    #[test]
    fn figmap_save_fires_on_save() {
        let mut app = app();
        let path = script_file("figmap", SCRIPT);
        app.load_fx_script(&path);
        draw(&mut app);
        let doc = path.with_file_name("doc.figmap");
        app.perform_save(Some(doc.clone()));
        assert!(doc.exists());
        draw(&mut app);
        assert!(running(&app, "saved"));
        // And reopening it fires on_open.
        let _ = app.perform_open_figmap(&doc);
        draw(&mut app);
        assert!(running(&app, "opened"));
    }

    #[test]
    fn hot_reload_swaps_the_script_and_bad_edits_keep_the_old_one() {
        let mut app = app();
        let path = script_file("reload", SCRIPT);
        app.load_fx_script(&path);
        draw(&mut app);
        assert!(running(&app, "spin"));
        let t0 = Instant::now();
        app.poll_fx_script(t0);
        // A broken edit: reported, old script kept.
        std::fs::write(&path, "[[effect]]\nname = \"x\"\nsource = 'fx::nope(1)'\n").unwrap();
        bump_mtime(&path, 10);
        app.poll_fx_script(t0 + Duration::from_secs(1));
        let status = app.frame.effects.status.clone().unwrap();
        assert!(status.is_error, "{status:?}");
        assert!(
            status.text.starts_with("fx test.figfx.toml:3:"),
            "{}",
            status.text
        );
        assert_eq!(
            app.frame
                .effects
                .script
                .as_ref()
                .unwrap()
                .script
                .effects
                .len(),
            4
        );
        // A good edit: swapped in; the removed loop's slot is cancelled.
        std::fs::write(
            &path,
            "[[effect]]\nname = \"only\"\ntrigger = \"on_open\"\nsource = 'fx::sleep(100)'\n",
        )
        .unwrap();
        bump_mtime(&path, 11);
        app.poll_fx_script(t0 + Duration::from_secs(2));
        let status = app.frame.effects.status.clone().unwrap();
        assert!(!status.is_error, "{status:?}");
        assert!(status.text.contains("reloaded"), "{}", status.text);
        let host = app.frame.effects.script.as_ref().unwrap();
        assert_eq!(host.script.effects.len(), 1);
        assert!(!host.has_work(), "no loops left");
        assert!(!app.frame.effects.slot_running(FxSlot::Script(3)));
    }

    #[test]
    fn broken_script_is_watched_until_fixed() {
        let mut app = app();
        let path = script_file("broken", "[[effect]]\nname = 1\n");
        app.load_fx_script(&path);
        assert!(app.frame.effects.status.as_ref().unwrap().is_error);
        std::fs::write(&path, SCRIPT).unwrap();
        bump_mtime(&path, 20);
        app.poll_fx_script(Instant::now());
        assert_eq!(
            app.frame
                .effects
                .script
                .as_ref()
                .unwrap()
                .script
                .effects
                .len(),
            4
        );
    }

    #[test]
    fn effects_tab_play_and_stop_buttons() {
        let mut app = app();
        app.load_fx_script(&script_file("panel", SCRIPT));
        app.side_panel.open = true;
        app.side_panel.set_active_tab(TabId::Effects);
        draw(&mut app);
        let fl = app.frame_layout(Rect::new(0, 0, 100, 30));
        let area = app
            .side_panel
            .content_area(fl.right_panel.expect("panel open"));
        let click = |app: &mut TuiApp, x: u16, y: u16| {
            let ev = MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: x,
                row: y,
                modifiers: KeyModifiers::NONE,
            };
            app.mouse_effects_panel(ev, &fl)
        };
        // Row 0 of the list is "opened" (on_open): play starts it.
        assert!(click(&mut app, area.x + 2, area.y + 3));
        draw(&mut app);
        assert!(running(&app, "opened"));
        assert!(click(&mut app, area.x + 6, area.y + 3));
        assert!(!running(&app, "opened"));
        // Stop on the loop (row 3) disables it.
        assert!(click(&mut app, area.x + 6, area.y + 6));
        draw(&mut app);
        assert!(!running(&app, "spin"));
        assert!(!app.frame.effects.script.as_ref().unwrap().is_looping(3));
        // Not a button.
        assert!(!click(&mut app, area.x + 12, area.y + 3));
    }

    #[test]
    fn open_dialog_target_loads_a_script() {
        let mut app = app();
        let path = script_file("dialog", SCRIPT);
        assert!(app.open_fx_script_from_dialog(&path));
        assert!(app.frame.effects.script.is_some());
        assert!(!app.open_fx_script_from_dialog(&path.with_file_name("x.figmap")));
    }
}
