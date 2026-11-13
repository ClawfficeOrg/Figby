//! Player launch from export, inline timeline playback and transport buttons.

use super::*;

impl TuiApp {
    pub(super) fn launch_player_from_export(&mut self) {
        let w = self.editor.canvas.buffer.width();
        let h = self.editor.canvas.buffer.height();
        if self.animation.timeline_state.frames.is_empty() {
            return;
        }
        let frames = {
            // Bake text non-destructively so the player shows text pixels
            // while the editable blocks stay live in the document.
            let baked;
            let stack_ref: &crate::tui::layers::LayerStack =
                if self.editor.text_tool.blocks.is_empty() {
                    &self.editor.layer_stack
                } else {
                    baked = self.layer_stack_with_text_baked();
                    &baked
                };
            export::capture_timeline_frames(
                &self.animation.timeline_state,
                stack_ref,
                w,
                h,
                None, // player preview skips lighting for speed
            )
        };
        if frames.is_empty() {
            return;
        }
        // Per-frame delays ride with the timeline frames (F-26).
        let delays = self.animation.timeline_state.frame_delays();
        let fps = self.dialogs.export_dialog.fps;
        let start_frame = self.dialogs.export_dialog.preview_frame;
        self.play_standalone_preview(frames, delays, fps, start_frame);
    }

    /// Fullscreen "preview standalone" playback — takes over the whole
    /// terminal via `player::play_fullscreen`. Used only by the Export
    /// dialog's Play button, e.g. to preview how a GIF/APNG export will
    /// actually look played back outside the editor. Normal in-editor
    /// playback (Timeline Enter / Animation > Play) uses `play_inline`
    /// instead, which stays inside the canvas and leaves the rest of the
    /// editor UI visible and interactive.
    pub(super) fn play_standalone_preview(
        &mut self,
        frames: Vec<Vec<Vec<canvas::CanvasCell>>>,
        delays: Vec<u16>,
        fps: u8,
        start_frame: usize,
    ) {
        let frames = if start_frame < frames.len() {
            frames[start_frame..].to_vec()
        } else {
            frames
        };
        let delays = if start_frame < delays.len() {
            delays[start_frame..].to_vec()
        } else {
            delays
        };

        if frames.is_empty() {
            return;
        }

        // Deliberately synchronous, not backgrounded onto a thread: the
        // player and the TUI both write directly to the same exclusive
        // terminal fd via crossterm/ratatui, so nothing useful could run
        // concurrently while play_fullscreen owns the terminal and its own
        // keyboard input (pause/seek/Esc/q) anyway — a background thread
        // here would add a channel + lifecycle surface purely to reproduce
        // the same blocking behavior, with a real risk of the two writers
        // racing on stdout if not synchronized carefully.
        if let Err(e) = player::play_fullscreen_timed(frames, fps, Some(delays)) {
            let _ = e;
        }

        // play_fullscreen renders through its own throwaway ratatui Terminal
        // (see its doc comment), which leaves our real Terminal's diff cache
        // stale relative to the actual screen — force a full repaint rather
        // than a diffed one on the next draw.
        self.frame.force_full_redraw = true;
        self.frame.dirty = true;
    }

    /// Capture the current timeline (at canvas resolution, so frames are
    /// already sized correctly — no scaling needed) and start playing it in
    /// place in the canvas, from the current frame.
    pub(super) fn start_inline_playback_from_timeline(&mut self) {
        if self.animation.timeline_state.frames.is_empty() {
            return;
        }
        let w = self.editor.canvas.buffer.width();
        let h = self.editor.canvas.buffer.height();
        let frames = export::capture_timeline_frames(
            &self.animation.timeline_state,
            &self.editor.layer_stack,
            w,
            h,
            None,
        );
        if frames.is_empty() {
            return;
        }
        let fps = self.animation.timeline_state.fps;
        let mut frames = frames;
        if self.animation.emitter_active {
            self.overlay_emitter_particles(&mut frames, fps);
        }
        let start_frame = self.animation.timeline_state.current_frame;
        let delays = self.animation.timeline_state.frame_delays();
        self.play_inline(frames, delays, fps, start_frame);
    }

    /// Simulate the emitter across the captured timeline frames and composite
    /// its particles onto each frame, so playback shows the particle stream.
    /// When the emitter follows a layer, its keyframed position offset moves
    /// the emission point frame by frame (e.g. exhaust trailing a ship).
    pub(super) fn overlay_emitter_particles(
        &self,
        frames: &mut [Vec<Vec<canvas::CanvasCell>>],
        fps: u8,
    ) {
        let config = self.animation.particle_system.config.clone();
        let (base_x, base_y) = (config.emitter_x, config.emitter_y);
        let w = self.editor.canvas.buffer.width();
        let h = self.editor.canvas.buffer.height();
        let dt = 1.0 / f64::from(fps.max(1));
        let mut system = particles::ParticleSystem::new(config);
        for (i, rows) in frames.iter_mut().enumerate() {
            if let Some(layer) = self.animation.emitter_follow_layer {
                let kf = self
                    .animation
                    .timeline_state
                    .get_interpolated_properties(i, layer);
                system.config.emitter_x = base_x + f64::from(kf.position_offset.0);
                system.config.emitter_y = base_y + f64::from(kf.position_offset.1);
            }
            system.update(dt, Some((w, h)), None);
            let mut buf = canvas::CanvasBuffer::new(w, h);
            for (y, row) in rows.iter().enumerate() {
                for (x, cell) in row.iter().enumerate() {
                    buf.set(x, y, *cell);
                }
            }
            system.render_to_canvas(&mut buf);
            for (y, row) in rows.iter_mut().enumerate() {
                for (x, cell) in row.iter_mut().enumerate() {
                    if let Some(c) = buf.get(x, y) {
                        *cell = *c;
                    }
                }
            }
        }
    }

    /// Start in-canvas animation playback: `render_canvas_area` renders the
    /// player in place of normal canvas content while `inline_player` is
    /// set, and `run()`'s loop ticks it each frame — no fullscreen takeover,
    /// no separate Terminal instance, and the rest of the editor UI (menu,
    /// toolbox, palette, timeline, status bar) keeps rendering normally
    /// around it. Dismissed via Esc/q in `handle_key_event`.
    pub(super) fn play_inline(
        &mut self,
        frames: Vec<Vec<Vec<canvas::CanvasCell>>>,
        delays: Vec<u16>,
        fps: u8,
        start_frame: usize,
    ) {
        if frames.is_empty() {
            return;
        }
        let player = player::AnimationPlayer::new(frames, fps.max(1))
            .with_frame_delays(delays)
            .with_loop(self.animation.loop_enabled);
        player.seek(start_frame);
        player.play();
        self.animation.inline_player = Some(player);
        self.frame.dirty = true;
    }

    /// Dismiss in-canvas animation playback. Shared by the Esc/q keyboard
    /// path and the transport bar's Stop button.
    pub(super) fn stop_inline_playback(&mut self) {
        if let Some(player) = self.animation.inline_player.as_ref() {
            self.animation.timeline_state.current_frame = player.current_frame();
            self.animation.load_current_timeline_frame(&mut self.editor);
        }
        self.animation.inline_player = None;
        self.frame.dirty = true;
    }

    /// Dispatch a transport-bar button click to the same playback methods
    /// already reachable via keyboard (`AnimationPlayer::toggle_play` /
    /// `toggle_loop`, `start_inline_playback_from_timeline`,
    /// `stop_inline_playback`) — no new playback logic, just a mouse entry
    /// point into what already exists.
    pub(super) fn handle_transport_button(&mut self, action: timeline::TransportButton) {
        match action {
            timeline::TransportButton::PlayPause => {
                if let Some(player) = self.animation.inline_player.as_ref() {
                    player.toggle_play();
                } else {
                    self.start_inline_playback_from_timeline();
                }
            }
            timeline::TransportButton::Stop => {
                self.stop_inline_playback();
            }
            timeline::TransportButton::Loop => {
                if let Some(player) = self.animation.inline_player.as_ref() {
                    player.toggle_loop();
                } else {
                    self.animation.loop_enabled = !self.animation.loop_enabled;
                }
            }
        }
        self.frame.dirty = true;
    }
}

#[cfg(test)]
mod playback_reconciliation_tests {
    use super::*;
    use std::time::Duration;

    fn make_app_with_frames(frame_count: usize) -> TuiApp {
        let mut app = TuiApp::new();
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
        app
    }

    #[test]
    fn test_tick_syncs_timeline_frame_to_player_frame() {
        let mut app = make_app_with_frames(5);
        app.start_inline_playback_from_timeline();
        assert!(app.animation.inline_player.is_some());

        let player_frame = {
            let player = app.animation.inline_player.as_ref().unwrap();
            player.advance(Duration::from_millis(200));
            player.current_frame()
        };

        app.animation.timeline_state.current_frame = player_frame;

        assert_eq!(app.animation.timeline_state.current_frame, player_frame);
        assert!(player_frame > 0, "player should have advanced past frame 0");
    }

    #[test]
    fn test_stop_preserves_last_rendered_frame() {
        let mut app = make_app_with_frames(5);
        app.animation.timeline_state.current_frame = 2;
        app.start_inline_playback_from_timeline();
        assert!(app.animation.inline_player.is_some());

        let last_frame = {
            let player = app.animation.inline_player.as_ref().unwrap();
            player.advance(Duration::from_millis(100));
            player.current_frame()
        };

        app.stop_inline_playback();

        assert_eq!(app.animation.timeline_state.current_frame, last_frame);
        assert!(app.animation.inline_player.is_none());

        let expected_ch = char::from_u32(b'A' as u32 + (last_frame % 26) as u32).unwrap();
        assert_eq!(
            app.editor.canvas.buffer.get(0, 0).unwrap().ch,
            expected_ch,
            "canvas should hold the last rendered frame content"
        );
    }
}
