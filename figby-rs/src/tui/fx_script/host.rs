//! The loaded effect script and its trigger bookkeeping: which effects are
//! queued to start and which `loop` effects keep restarting.
//!
//! Triggers only queue effects. `TuiApp` compiles and starts the queue once
//! per frame, after the layout is known (`dispatch/fx_run.rs`), so targets
//! such as `canvas` are bound to where things are *now*, not where they
//! were when, say, an open finished. Pure state: no filesystem, no terminal.

use std::collections::BTreeSet;

use ratatui::crossterm::event::KeyEvent;

use super::runtime::FxSlot;
use super::script::{FxScript, FxTrigger};
use super::script_file::FxScriptFile;

/// A script loaded from a file (or from text, in tests).
#[derive(Debug, Clone)]
pub struct FxScriptHost {
    pub script: FxScript,
    /// The file it came from; polled for hot reload.
    pub file: Option<FxScriptFile>,
    /// Effects to start at the next frame, in trigger order.
    pending: Vec<usize>,
    /// `loop` effects currently enabled (restarted whenever they end).
    looping: BTreeSet<usize>,
}

impl FxScriptHost {
    /// Every `loop` effect starts enabled.
    pub fn new(script: FxScript, file: Option<FxScriptFile>) -> Self {
        let looping = script.with_trigger(FxTrigger::Loop).into_iter().collect();
        Self {
            script,
            file,
            pending: Vec::new(),
            looping,
        }
    }

    fn queue(&mut self, idx: usize) {
        if !self.pending.contains(&idx) {
            self.pending.push(idx);
        }
    }

    /// Queue every effect with this trigger; true if any matched.
    pub fn fire(&mut self, trigger: FxTrigger) -> bool {
        let hits = self.script.with_trigger(trigger);
        for &i in &hits {
            self.queue(i);
        }
        !hits.is_empty()
    }

    /// Queue the `on_key` effects bound to `key`; true if any matched (the
    /// caller then consumes the key).
    pub fn fire_key(&mut self, key: &KeyEvent) -> bool {
        let hits = self.script.for_key(key);
        for &i in &hits {
            self.queue(i);
        }
        !hits.is_empty()
    }

    /// Play effect `idx` now, whatever its trigger. A `loop` effect is
    /// (re-)enabled as well.
    pub fn play(&mut self, idx: usize) {
        if idx >= self.script.effects.len() {
            return;
        }
        if self.script.effects[idx].trigger == FxTrigger::Loop {
            self.looping.insert(idx);
        }
        self.queue(idx);
    }

    /// Stop effect `idx`: drop it from the queue and, for a `loop`, disable
    /// it. Returns the slot to cancel.
    pub fn stop(&mut self, idx: usize) -> Option<FxSlot> {
        if idx >= self.script.effects.len() {
            return None;
        }
        self.pending.retain(|&i| i != idx);
        self.looping.remove(&idx);
        Some(self.script.slot_of(idx))
    }

    /// Whether a `loop` effect is enabled.
    pub fn is_looping(&self, idx: usize) -> bool {
        self.looping.contains(&idx)
    }

    /// Something must start at the next frame (queued, or a loop to keep
    /// going), so the event loop has to keep drawing.
    pub fn has_work(&self) -> bool {
        !self.pending.is_empty() || !self.looping.is_empty()
    }

    /// The effects to start now: everything queued, then every enabled
    /// loop whose slot has gone idle. `running` says whether a slot still
    /// runs an effect.
    pub fn take_due(&mut self, running: impl Fn(FxSlot) -> bool) -> Vec<usize> {
        let mut due = std::mem::take(&mut self.pending);
        for &i in &self.looping {
            if !due.contains(&i) && !running(self.script.slot_of(i)) {
                due.push(i);
            }
        }
        due
    }

    /// Disable a loop that failed to compile, so it is not retried (and
    /// its error re-reported) every frame.
    pub fn disable_loop(&mut self, idx: usize) {
        self.looping.remove(&idx);
    }

    /// Swap in a reloaded script. Queued effects are dropped. A `loop`
    /// effect keeps its enabled or stopped state across the reload when an
    /// effect of the same name was a loop before; new loops start enabled.
    /// The caller cancels the slots that went away ([`slots_to_cancel`]).
    pub fn replace(&mut self, script: FxScript) {
        let was_loop = |name: &str| {
            self.script
                .index_of(name)
                .filter(|&i| self.script.effects[i].trigger == FxTrigger::Loop)
        };
        let looping = script
            .with_trigger(FxTrigger::Loop)
            .into_iter()
            .filter(|&i| match was_loop(&script.effects[i].name) {
                Some(old) => self.looping.contains(&old),
                None => true,
            })
            .collect();
        self.looping = looping;
        self.pending.clear();
        self.script = script;
    }
}

/// Slots an old script used that a new one does not; their effects should
/// stop on reload. Slot indices are positional, so a slot that kept its
/// name but moved is cancelled too (its effect restarts from the new
/// script's triggers).
pub fn slots_to_cancel(old: &FxScript, new: &FxScript) -> Vec<FxSlot> {
    old.slots
        .iter()
        .enumerate()
        .filter(|(i, name)| new.slots.get(*i) != Some(*name))
        .map(|(i, _)| FxSlot::Script(i as u16))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::fx_script::script::parse_script;
    use ratatui::crossterm::event::{KeyCode, KeyModifiers};

    fn effect(name: &str, trigger: &str) -> String {
        format!(
            "[[effect]]\nname = \"{name}\"\ntrigger = \"{trigger}\"\nsource = \"fx::sleep(100)\"\n"
        )
    }

    fn host(src: &str) -> FxScriptHost {
        FxScriptHost::new(parse_script(src).unwrap(), None)
    }

    #[test]
    fn triggers_queue_only_their_effects() {
        let src = [
            effect("a", "on_open"),
            effect("b", "on_save"),
            effect("c", "on_open"),
            effect("d", "manual"),
            effect("k", "on_key:F8"),
        ]
        .concat();
        let mut h = host(&src);
        assert!(!h.has_work());
        assert!(h.fire(FxTrigger::OnOpen));
        assert!(h.has_work());
        assert_eq!(h.take_due(|_| false), [0, 2]);
        assert!(!h.has_work(), "the queue drains");
        assert!(h.fire(FxTrigger::OnSave));
        // Firing twice before a frame queues once.
        assert!(h.fire(FxTrigger::OnSave));
        assert_eq!(h.take_due(|_| false), [1]);
        assert!(!h.fire_key(&KeyEvent::new(KeyCode::F(7), KeyModifiers::NONE)));
        assert!(h.fire_key(&KeyEvent::new(KeyCode::F(8), KeyModifiers::NONE)));
        assert_eq!(h.take_due(|_| false), [4]);
    }

    #[test]
    fn loops_restart_when_their_slot_is_idle_until_stopped() {
        let mut h = host(&[effect("spin", "loop"), effect("x", "manual")].concat());
        assert!(h.has_work(), "loops start on load");
        assert!(h.is_looping(0));
        assert_eq!(h.take_due(|_| false), [0]);
        // Still running: not restarted.
        assert!(h.take_due(|_| true).is_empty());
        // Ended: restarted.
        assert_eq!(h.take_due(|_| false), [0]);
        assert_eq!(h.stop(0), Some(h.script.slot_of(0)));
        assert!(!h.has_work());
        assert!(h.take_due(|_| false).is_empty());
        h.play(0);
        assert!(h.is_looping(0));
        assert_eq!(h.take_due(|_| false), [0]);
    }

    #[test]
    fn play_and_stop_manual_effects() {
        let mut h = host(&effect("x", "manual"));
        h.play(0);
        h.play(5); // out of range: ignored
        assert_eq!(h.stop(0), Some(FxSlot::Script(0)));
        assert!(
            h.take_due(|_| false).is_empty(),
            "stop drops the queued play"
        );
        h.play(0);
        assert_eq!(h.take_due(|_| false), [0]);
        assert!(!h.is_looping(0));
    }

    #[test]
    fn reload_keeps_stopped_loops_stopped_and_starts_new_ones() {
        let mut h = host(&[effect("spin", "loop"), effect("glow", "loop")].concat());
        h.stop(1);
        h.fire(FxTrigger::OnOpen);
        let new = parse_script(
            &[
                effect("glow", "loop"),
                effect("spin", "loop"),
                effect("new", "loop"),
            ]
            .concat(),
        )
        .unwrap();
        h.replace(new);
        // glow (now 0) stays stopped, spin (now 1) stays on, new starts.
        assert!(!h.is_looping(0));
        assert!(h.is_looping(1));
        assert!(h.is_looping(2));
    }

    #[test]
    fn removed_or_moved_slots_are_cancelled_on_reload() {
        let old = parse_script(&[effect("a", "manual"), effect("b", "manual")].concat()).unwrap();
        let same = old.clone();
        assert!(slots_to_cancel(&old, &same).is_empty());
        let fewer = parse_script(&effect("a", "manual")).unwrap();
        assert_eq!(slots_to_cancel(&old, &fewer), [FxSlot::Script(1)]);
        let swapped =
            parse_script(&[effect("b", "manual"), effect("a", "manual")].concat()).unwrap();
        assert_eq!(slots_to_cancel(&old, &swapped).len(), 2);
    }
}
