use crossterm::event::KeyCode;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, StatefulWidget, Widget};
use ratatui::Frame;
use serde::{Deserialize, Serialize};

use super::canvas::CanvasBuffer;
use super::layers::{BlendMode, LayerStack};
use super::theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LayerKeyframe {
    pub position_offset: (i16, i16),
    pub opacity: u8,
    pub blend_mode: BlendMode,
}

impl Default for LayerKeyframe {
    fn default() -> Self {
        Self {
            position_offset: (0, 0),
            opacity: 255,
            blend_mode: BlendMode::Normal,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineFrame {
    pub thumbnail: Vec<Vec<char>>,
    pub has_keyframe: bool,
    pub label: String,
    /// Hold time for this frame in centiseconds (1/100 s), the GIF
    /// convention used everywhere else in this codebase. Lives *inside* the
    /// frame so insert/delete/reorder/duplicate carry timing with the
    /// content (GPT review F-26) — a separate delays vector can detach from
    /// the frames it describes.
    pub delay: u16,
    /// Committed pixel state: one buffer per layer, in render order
    /// (bottom-to-top). Empty when the frame has no committed snapshot —
    /// such frames derive pixels from the live layer stack.
    ///
    /// Single-authority rule (GPT review F-05): this owns *pixels*;
    /// `layer_keyframes` own *transforms* (offset/opacity/blend), which
    /// are applied on top of these buffers at composite time.
    pub document_state: Vec<CanvasBuffer>,
    pub layer_keyframes: Vec<Option<LayerKeyframe>>,
}

#[derive(Debug, Clone)]
pub struct TimelineTheme {
    pub playhead: Color,
    pub keyframe: Color,
    pub ruler: Color,
    pub thumbnail_border: Color,
    pub thumbnail_bg: Color,
    pub active_frame_border: Color,
}

impl Default for TimelineTheme {
    fn default() -> Self {
        Self {
            playhead: Color::Red,
            keyframe: Color::Yellow,
            ruler: Color::DarkGray,
            thumbnail_border: Color::DarkGray,
            thumbnail_bg: Color::Reset,
            active_frame_border: Color::Cyan,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct KeyframeEditState {
    pub open: bool,
    pub selected_layer: usize,
    pub selected_property: usize,
    pub edit_mode: bool,
    pub edit_buffer: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum EasingFunction {
    Linear,
    EaseIn,
    EaseOut,
    Bounce,
}

impl EasingFunction {
    pub fn apply(&self, t: f64) -> f64 {
        match self {
            EasingFunction::Linear => t,
            EasingFunction::EaseIn => t * t * t,
            EasingFunction::EaseOut => 1.0 - (1.0 - t).powi(3),
            EasingFunction::Bounce => ease_bounce(t),
        }
    }

    pub fn display_name(&self) -> &str {
        match self {
            EasingFunction::Linear => "Linear",
            EasingFunction::EaseIn => "Ease In",
            EasingFunction::EaseOut => "Ease Out",
            EasingFunction::Bounce => "Bounce",
        }
    }

    pub fn cycle(&self) -> Self {
        match self {
            EasingFunction::Linear => EasingFunction::EaseIn,
            EasingFunction::EaseIn => EasingFunction::EaseOut,
            EasingFunction::EaseOut => EasingFunction::Bounce,
            EasingFunction::Bounce => EasingFunction::Linear,
        }
    }

    pub fn cycle_back(&self) -> Self {
        match self {
            EasingFunction::Linear => EasingFunction::Bounce,
            EasingFunction::EaseIn => EasingFunction::Linear,
            EasingFunction::EaseOut => EasingFunction::EaseIn,
            EasingFunction::Bounce => EasingFunction::EaseOut,
        }
    }

    pub fn all() -> &'static [EasingFunction] {
        &[
            EasingFunction::Linear,
            EasingFunction::EaseIn,
            EasingFunction::EaseOut,
            EasingFunction::Bounce,
        ]
    }
}

#[derive(Debug, Clone)]
pub struct TweenConfig {
    pub start_frame: usize,
    pub end_frame: usize,
    pub num_frames: usize,
    pub easing: EasingFunction,
}

impl Default for TweenConfig {
    fn default() -> Self {
        Self {
            start_frame: 0,
            end_frame: 0,
            num_frames: 5,
            easing: EasingFunction::Linear,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct TweenPreview {
    pub generated_frames: Vec<TimelineFrame>,
    pub config: TweenConfig,
    pub valid: bool,
    pub field_index: usize,
}

#[derive(Debug, Clone)]
pub struct AnimationTimeline {
    /// Width of the layer-name column (chars).
    pub label_col_width: u16,
    /// Content width of each frame cell (chars); separator `│` adds 1 more.
    pub cell_width: u16,
    pub theme: TimelineTheme,
}

#[derive(Debug, Clone)]
pub struct TimelineState {
    pub frames: Vec<TimelineFrame>,
    pub current_frame: usize,
    pub scroll_offset: usize,
    pub fps: u8,
    pub keyframe_editor: KeyframeEditState,
    pub tween: Option<TweenPreview>,
    /// Layer names in render order (bottom-to-top of stack).
    pub layer_names: Vec<String>,
    /// Vertical scroll offset for layers (row index of first visible layer).
    pub layer_row_offset: usize,
    /// Updated each render pass — used by key handler to keep current frame in view.
    pub cached_max_vis_frames: usize,
    /// Frame indices that have light keyframes (for ruler markers).
    pub light_keyframe_frames: Vec<usize>,
}

impl Default for TimelineState {
    fn default() -> Self {
        Self {
            frames: Vec::new(),
            current_frame: 0,
            scroll_offset: 0,
            fps: 12,
            keyframe_editor: KeyframeEditState::default(),
            tween: None,
            layer_names: Vec::new(),
            layer_row_offset: 0,
            cached_max_vis_frames: 5,
            light_keyframe_frames: Vec::new(),
        }
    }
}

impl TimelineState {
    /// Default hold time (centiseconds) for a freshly captured frame at the
    /// timeline's current fps — e.g. 12fps → 8cs (≈83ms). `.max(1)` keeps a
    /// very high fps from producing a zero (meaningless) delay.
    pub fn default_delay(&self) -> u16 {
        (100u16 / self.fps.max(1) as u16).max(1)
    }

    /// Recompute which frames have light keyframes based on the given
    /// light keyframe list. Call after modifying light keyframes.
    pub fn update_light_keyframe_frames(
        &mut self,
        light_keyframes: &[super::lighting::LightKeyframe],
    ) {
        let total = self.frames.len().max(1) as f32;
        let mut frames: Vec<usize> = light_keyframes
            .iter()
            .map(|kf| (kf.time * total).round() as usize)
            .collect();
        frames.sort();
        frames.dedup();
        self.light_keyframe_frames = frames;
    }

    /// Per-frame hold times in timeline order. Because `delay` lives inside
    /// `TimelineFrame`, this always matches the frames it accompanies — even
    /// after insert/delete/reorder/duplicate (GPT review F-26).
    pub fn frame_delays(&self) -> Vec<u16> {
        self.frames.iter().map(|f| f.delay.max(1)).collect()
    }

    /// Refresh the display-only layer-name column from the current layer
    /// stack. Must be called any time a frame is added (manual capture,
    /// menu "Add Frame", GIF import) — the timeline widget sizes its
    /// layer rows from `layer_names.len()`, so a stale/empty value here is
    /// why the timeline can appear to only have one layer.
    pub fn sync_layer_names(&mut self, stack: &LayerStack) {
        self.layer_names = stack.layers.iter().map(|l| l.name.clone()).collect();
    }

    /// Add a frame to the end of the timeline.
    pub fn add_frame(&mut self, frame: TimelineFrame) {
        self.frames.push(frame);
    }
    /// Fill ghost slots `len..=target` with frames cloned from the last
    /// real frame (pixels + layer keyframes copied). `keyframe=true`
    /// (right-click) marks every new frame `has_keyframe` with default
    /// identity transforms for layers missing them; `false`
    /// (left-click) leaves plain snapshots. Returns the number created.
    pub fn fill_ghosts_to(&mut self, target: usize, keyframe: bool) -> usize {
        if target < self.frames.len() || self.frames.is_empty() {
            return 0;
        }
        let delay = self.default_delay();
        let n_layers = self
            .frames
            .iter()
            .map(|f| f.layer_keyframes.len())
            .max()
            .unwrap_or(0);
        let mut made = 0;
        while self.frames.len() <= target {
            let src = self.frames.last().expect("checked non-empty");
            let mut kf = src.layer_keyframes.clone();
            kf.resize(n_layers, None);
            let has_kf = if keyframe {
                for slot in kf.iter_mut() {
                    if slot.is_none() {
                        *slot = Some(LayerKeyframe::default());
                    }
                }
                true
            } else {
                src.has_keyframe
            };
            let idx = self.frames.len();
            self.frames.push(TimelineFrame {
                thumbnail: src.thumbnail.clone(),
                has_keyframe: has_kf,
                label: format!("F{idx}"),
                delay,
                document_state: src.document_state.clone(),
                layer_keyframes: kf,
            });
            made += 1;
        }
        made
    }

    /// Insert a frame at the given index.
    pub fn insert_frame(&mut self, index: usize, frame: TimelineFrame) {
        let idx = index.min(self.frames.len());
        self.frames.insert(idx, frame);
        if idx <= self.current_frame {
            self.current_frame += 1;
        }
    }

    /// Remove the frame at `index`. Fails if it would leave the timeline empty.
    pub fn remove_frame(&mut self, index: usize) -> Result<TimelineFrame, String> {
        if index >= self.frames.len() {
            return Err(format!(
                "Frame index {} out of bounds (len={})",
                index,
                self.frames.len()
            ));
        }
        if self.frames.len() <= 1 {
            return Err("Cannot remove the last remaining frame".into());
        }
        let removed = self.frames.remove(index);
        if self.current_frame >= self.frames.len() {
            self.current_frame = self.frames.len().saturating_sub(1);
        } else if index < self.current_frame {
            self.current_frame -= 1;
        }
        Ok(removed)
    }

    /// Duplicate the frame at `index`, inserting the copy after it.
    pub fn duplicate_frame(&mut self, index: usize) -> Result<(), String> {
        if index >= self.frames.len() {
            return Err(format!(
                "Frame index {} out of bounds (len={})",
                index,
                self.frames.len()
            ));
        }
        let mut dup = self.frames[index].clone();
        dup.label = format!("{} (copy)", dup.label);
        let insert_at = index + 1;
        self.frames.insert(insert_at, dup);
        if insert_at <= self.current_frame {
            self.current_frame += 1;
        }
        Ok(())
    }

    /// Move a frame from `from` to `to`.
    pub fn reorder_frame(&mut self, from: usize, to: usize) -> Result<(), String> {
        if from >= self.frames.len() {
            return Err(format!(
                "Source index {} out of bounds (len={})",
                from,
                self.frames.len()
            ));
        }
        if to >= self.frames.len() {
            return Err(format!(
                "Target index {} out of bounds (len={})",
                to,
                self.frames.len()
            ));
        }
        if from == to {
            return Ok(());
        }
        let frame = self.frames.remove(from);
        let insert_at = if to > from { to.saturating_sub(1) } else { to };
        self.frames.insert(insert_at, frame);

        let old = self.current_frame;
        if from == old {
            // The moved frame lands at `insert_at` above: `to` when moving
            // left, `to - 1` when moving right (removal shifts everything
            // after `from` left by one). Pointing at `to` unconditionally
            // tracked the wrong logical frame for a rightward move (F-26).
            self.current_frame = insert_at;
        } else if from < old && to >= old {
            self.current_frame = old.saturating_sub(1);
        } else if from > old && to <= old {
            self.current_frame = old.saturating_add(1);
        }
        Ok(())
    }

    // ── Keyframing ────────────────────────────────────────────────────

    pub fn set_keyframe(
        &mut self,
        frame_idx: usize,
        layer_idx: usize,
        props: LayerKeyframe,
    ) -> bool {
        let frame = match self.frames.get_mut(frame_idx) {
            Some(f) => f,
            None => return false,
        };
        while frame.layer_keyframes.len() <= layer_idx {
            frame.layer_keyframes.push(None);
        }
        frame.layer_keyframes[layer_idx] = Some(props);
        frame.has_keyframe = frame.layer_keyframes.iter().any(|k| k.is_some());
        true
    }

    pub fn remove_keyframe(&mut self, frame_idx: usize, layer_idx: usize) -> bool {
        let frame = match self.frames.get_mut(frame_idx) {
            Some(f) => f,
            None => return false,
        };
        if layer_idx < frame.layer_keyframes.len() {
            frame.layer_keyframes[layer_idx] = None;
            frame.has_keyframe = frame.layer_keyframes.iter().any(|k| k.is_some());
            true
        } else {
            false
        }
    }

    pub fn get_keyframe(&self, frame_idx: usize, layer_idx: usize) -> Option<LayerKeyframe> {
        self.frames
            .get(frame_idx)?
            .layer_keyframes
            .get(layer_idx)
            .copied()
            .flatten()
    }

    pub fn get_interpolated_properties(&self, frame_idx: usize, layer_idx: usize) -> LayerKeyframe {
        let prev = (0..=frame_idx).rev().find_map(|i| {
            let f = self.frames.get(i)?;
            let kf = (*f.layer_keyframes.get(layer_idx)?)?;
            Some((i, kf))
        });
        let next = (frame_idx..self.frames.len()).find_map(|i| {
            let f = self.frames.get(i)?;
            let kf = (*f.layer_keyframes.get(layer_idx)?)?;
            Some((i, kf))
        });
        match (prev, next) {
            (Some((pi, pk)), Some((_ni, _))) if pi == _ni => pk,
            (Some((pi, pk)), Some((ni, nk))) => {
                if frame_idx <= pi {
                    pk
                } else if frame_idx >= ni {
                    nk
                } else {
                    let range = ni - pi;
                    let offset = frame_idx.saturating_sub(pi);
                    let t = if range == 0 {
                        0.0
                    } else {
                        offset as f64 / range as f64
                    };
                    LayerKeyframe {
                        position_offset: (
                            lerp_i16(pk.position_offset.0, nk.position_offset.0, t),
                            lerp_i16(pk.position_offset.1, nk.position_offset.1, t),
                        ),
                        opacity: lerp_u8(pk.opacity, nk.opacity, t),
                        blend_mode: step_blend_mode(pk.blend_mode, nk.blend_mode, t),
                    }
                }
            }
            (Some((_, pk)), None) => pk,
            (None, Some((_, nk))) => nk,
            (None, None) => LayerKeyframe::default(),
        }
    }

    // ── Tweening ──────────────────────────────────────────────────────

    pub fn open_tween(&mut self) {
        let start = self.current_frame;
        let end = (start + 1).min(self.frames.len().saturating_sub(1));
        self.tween = Some(TweenPreview {
            config: TweenConfig {
                start_frame: start,
                end_frame: end,
                ..TweenConfig::default()
            },
            ..TweenPreview::default()
        });
    }

    pub fn compute_tween(&mut self) {
        let tween = match self.tween.as_mut() {
            Some(t) => t,
            None => return,
        };

        let config = &tween.config;
        let start = config.start_frame;
        let end = config.end_frame;
        let num_frames = config.num_frames.clamp(1, 120);
        let easing = config.easing;

        if start >= end || end >= self.frames.len() || start >= self.frames.len() {
            tween.generated_frames.clear();
            tween.valid = false;
            return;
        }

        let start_frame = &self.frames[start];
        let end_frame = &self.frames[end];

        let max_layers = start_frame
            .layer_keyframes
            .len()
            .max(end_frame.layer_keyframes.len());

        let mut generated = Vec::with_capacity(num_frames);
        let mut has_any_keyframe = false;

        for i in 0..num_frames {
            let t = (i + 1) as f64 / (num_frames + 1) as f64;
            let et = easing.apply(t);

            let mut frame_layers: Vec<Option<LayerKeyframe>> = Vec::new();
            for layer in 0..max_layers {
                let start_kf = start_frame.layer_keyframes.get(layer).copied().flatten();
                let end_kf = end_frame.layer_keyframes.get(layer).copied().flatten();

                match (start_kf, end_kf) {
                    (Some(skf), Some(ekf)) => {
                        has_any_keyframe = true;
                        frame_layers.push(Some(LayerKeyframe {
                            position_offset: (
                                lerp_i16(skf.position_offset.0, ekf.position_offset.0, et),
                                lerp_i16(skf.position_offset.1, ekf.position_offset.1, et),
                            ),
                            opacity: lerp_u8(skf.opacity, ekf.opacity, et),
                            blend_mode: step_blend_mode(skf.blend_mode, ekf.blend_mode, et),
                        }));
                    }
                    _ => {
                        frame_layers.push(None);
                    }
                }
            }

            let has_kf = frame_layers.iter().any(|k| k.is_some());
            generated.push(TimelineFrame {
                thumbnail: start_frame.thumbnail.clone(),
                has_keyframe: has_kf,
                label: format!("tween {}/{}", i + 1, num_frames),
                delay: start_frame.delay.max(1),
                document_state: Vec::new(),
                layer_keyframes: frame_layers,
            });
        }

        tween.generated_frames = generated;
        tween.valid = has_any_keyframe;
    }

    pub fn commit_tween(&mut self) {
        let tween = match self.tween.take() {
            Some(t) if t.valid => t,
            _ => return,
        };

        let insert_at = tween.config.start_frame + 1;
        let num = tween.generated_frames.len();

        for (i, frame) in tween.generated_frames.into_iter().enumerate() {
            let idx = insert_at + i;
            if idx <= self.frames.len() {
                self.frames.insert(idx, frame);
            } else {
                self.frames.push(frame);
            }
        }

        if insert_at <= self.current_frame {
            self.current_frame += num;
        }
    }

    pub fn discard_tween(&mut self) {
        self.tween = None;
    }

    pub fn render_tween_panel(&self, frame: &mut Frame, area: Rect, theme: &TimelineTheme) {
        let tween = match self.tween.as_ref() {
            Some(t) => t,
            None => return,
        };

        if area.width < 20 || area.height < 6 {
            return;
        }

        let block = Block::default()
            .title(" Tween ")
            .borders(Borders::ALL)
            .style(Style::default().fg(theme.keyframe));
        let inner = block.inner(area);
        frame.render_widget(Clear, area);
        frame.render_widget(block, area);

        let mut lines: Vec<String> = Vec::new();
        let has_frames = !self.frames.is_empty();
        if !has_frames {
            lines.push(" No frames in timeline".to_string());
            let para = Paragraph::new(lines.join("\n")).style(Style::default());
            frame.render_widget(para, inner);
            return;
        }

        let cfg = &tween.config;
        let fields = [
            ("Start Frame", cfg.start_frame, 0),
            ("End Frame", cfg.end_frame, 1),
            ("Frames", cfg.num_frames, 2),
        ];

        for (label, value, idx) in fields {
            let prefix = if tween.field_index == idx { '>' } else { ' ' };
            lines.push(format!(" {} {}: {}", prefix, label, value));
        }

        let easing_prefix = if tween.field_index == 3 { '>' } else { ' ' };
        lines.push(format!(
            " {} Easing: {}",
            easing_prefix,
            cfg.easing.display_name()
        ));

        lines.push(String::new());

        if tween.valid && !tween.generated_frames.is_empty() {
            lines.push(format!(
                " Status: Generated ({} frames)",
                tween.generated_frames.len()
            ));
        } else {
            lines.push(" Status: Needs generate".to_string());
        }

        lines.push(String::new());
        lines.push(
            " \u{2191}\u{2193} field  \u{2190}\u{2192} value  Enter=Generate/Commit  C=Commit  Esc=Discard"
                .to_string(),
        );

        let para = Paragraph::new(lines.join("\n")).style(Style::default());
        frame.render_widget(para, inner);
    }

    pub fn handle_tween_key(&mut self, code: KeyCode) -> bool {
        let tween = match self.tween.as_mut() {
            Some(t) => t,
            None => return false,
        };

        match code {
            KeyCode::Up if tween.field_index > 0 => {
                tween.field_index -= 1;
            }
            KeyCode::Down if tween.field_index < 3 => {
                tween.field_index += 1;
            }
            KeyCode::Left => match tween.field_index {
                0 => {
                    if tween.config.start_frame > 0 {
                        tween.config.start_frame -= 1;
                    }
                    tween.valid = false;
                }
                1 => {
                    if tween.config.end_frame > tween.config.start_frame + 1 {
                        tween.config.end_frame -= 1;
                    }
                    tween.valid = false;
                }
                2 => {
                    if tween.config.num_frames > 1 {
                        tween.config.num_frames -= 1;
                    }
                    tween.valid = false;
                }
                3 => {
                    tween.config.easing = tween.config.easing.cycle_back();
                    tween.valid = false;
                }
                _ => {}
            },
            KeyCode::Right => match tween.field_index {
                0 => {
                    if tween.config.start_frame + 1 < tween.config.end_frame {
                        tween.config.start_frame += 1;
                    }
                    tween.valid = false;
                }
                1 => {
                    if tween.config.end_frame + 1 < self.frames.len() {
                        tween.config.end_frame += 1;
                    }
                    tween.valid = false;
                }
                2 => {
                    if tween.config.num_frames < 120 {
                        tween.config.num_frames += 1;
                    }
                    tween.valid = false;
                }
                3 => {
                    tween.config.easing = tween.config.easing.cycle();
                    tween.valid = false;
                }
                _ => {}
            },
            KeyCode::Enter => {
                if tween.valid && !tween.generated_frames.is_empty() {
                    self.commit_tween();
                } else {
                    self.compute_tween();
                }
            }
            KeyCode::Char('c') | KeyCode::Char('C') => {
                if tween.valid && !tween.generated_frames.is_empty() {
                    self.commit_tween();
                }
            }
            KeyCode::Esc => {
                self.discard_tween();
            }
            _ => return false,
        }
        true
    }

    pub fn handle_keyframe_editor_key(&mut self, code: KeyCode) -> bool {
        if !self.keyframe_editor.open {
            return false;
        }
        if self.keyframe_editor.edit_mode {
            match code {
                KeyCode::Esc => {
                    self.keyframe_editor.edit_mode = false;
                    self.keyframe_editor.edit_buffer.clear();
                }
                KeyCode::Enter => {
                    let value_str = self.keyframe_editor.edit_buffer.trim().to_string();
                    self.keyframe_editor.edit_mode = false;
                    if !value_str.is_empty() {
                        if let Ok(value) = value_str.parse::<i16>() {
                            let current = self.get_interpolated_properties(
                                self.current_frame,
                                self.keyframe_editor.selected_layer,
                            );
                            let mut new_kf = current;
                            match self.keyframe_editor.selected_property {
                                0 => new_kf.position_offset.0 = value,
                                1 => new_kf.position_offset.1 = value,
                                2 => new_kf.opacity = value.clamp(0, 255) as u8,
                                _ => {}
                            }
                            self.set_keyframe(
                                self.current_frame,
                                self.keyframe_editor.selected_layer,
                                new_kf,
                            );
                        }
                    }
                    self.keyframe_editor.edit_buffer.clear();
                }
                KeyCode::Backspace => {
                    self.keyframe_editor.edit_buffer.pop();
                }
                KeyCode::Char(c) if c.is_ascii_digit() || c == '-' => {
                    self.keyframe_editor.edit_buffer.push(c);
                }
                _ => {}
            }
            return true;
        }
        match code {
            KeyCode::Up if self.keyframe_editor.selected_layer > 0 => {
                self.keyframe_editor.selected_layer -= 1;
            }
            KeyCode::Down => {
                self.keyframe_editor.selected_layer += 1;
            }
            KeyCode::Left if self.keyframe_editor.selected_property > 0 => {
                self.keyframe_editor.selected_property -= 1;
            }
            KeyCode::Right if self.keyframe_editor.selected_property < 3 => {
                self.keyframe_editor.selected_property += 1;
            }
            KeyCode::Enter => {
                if self.keyframe_editor.selected_property == 3 {
                    let current = self.get_interpolated_properties(
                        self.current_frame,
                        self.keyframe_editor.selected_layer,
                    );
                    let next_blend = match current.blend_mode {
                        BlendMode::Normal => BlendMode::Multiply,
                        BlendMode::Multiply => BlendMode::Overlay,
                        BlendMode::Overlay => BlendMode::Screen,
                        BlendMode::Screen => BlendMode::Add,
                        BlendMode::Add => BlendMode::Subtract,
                        BlendMode::Subtract => BlendMode::Normal,
                    };
                    self.set_keyframe(
                        self.current_frame,
                        self.keyframe_editor.selected_layer,
                        LayerKeyframe {
                            blend_mode: next_blend,
                            ..current
                        },
                    );
                } else {
                    self.keyframe_editor.edit_mode = true;
                    self.keyframe_editor.edit_buffer.clear();
                    let current = self.get_interpolated_properties(
                        self.current_frame,
                        self.keyframe_editor.selected_layer,
                    );
                    let val = match self.keyframe_editor.selected_property {
                        0 => current.position_offset.0,
                        1 => current.position_offset.1,
                        _ => current.opacity as i16,
                    };
                    self.keyframe_editor.edit_buffer = val.to_string();
                }
            }
            KeyCode::Esc => {
                self.keyframe_editor.open = false;
            }
            _ => {}
        }
        true
    }

    pub fn render_keyframe_editor(&self, frame: &mut Frame, area: Rect, theme: &TimelineTheme) {
        if !self.keyframe_editor.open || area.width < 20 || area.height < 6 {
            return;
        }
        let block = Block::default()
            .title(" Keyframe Editor ")
            .borders(Borders::ALL)
            .style(Style::default().fg(theme.keyframe));
        let inner = block.inner(area);
        frame.render_widget(Clear, area);
        frame.render_widget(block, area);

        let mut lines: Vec<String> = Vec::new();
        let has_frames = !self.frames.is_empty();
        if !has_frames {
            lines.push(" No frames in timeline".to_string());
            let para = Paragraph::new(lines.join("\n")).style(Style::default());
            frame.render_widget(para, inner);
            return;
        }

        let frame_idx = self.current_frame.min(self.frames.len().saturating_sub(1));
        lines.push(format!(
            " Frame: {}  Layer: {}",
            frame_idx, self.keyframe_editor.selected_layer
        ));
        lines.push(String::new());

        let max_layer = self
            .frames
            .first()
            .map(|f| {
                f.layer_keyframes
                    .len()
                    .max(self.keyframe_editor.selected_layer + 1)
            })
            .unwrap_or(4);

        for layer_idx in 0..max_layer.min(6) {
            let props = self.get_interpolated_properties(frame_idx, layer_idx);
            let is_selected = layer_idx == self.keyframe_editor.selected_layer;
            let prefix = if is_selected { '>' } else { ' ' };
            lines.push(format!(" {} Layer {}:", prefix, layer_idx));

            for prop_idx in 0..4 {
                let is_prop_selected =
                    is_selected && prop_idx == self.keyframe_editor.selected_property;
                let prop_prefix = if is_prop_selected { '>' } else { ' ' };
                let value_str = match prop_idx {
                    0 => {
                        if is_prop_selected && self.keyframe_editor.edit_mode {
                            format!("Pos X: {}", self.keyframe_editor.edit_buffer)
                        } else {
                            format!("Pos X: {:4}", props.position_offset.0)
                        }
                    }
                    1 => {
                        if is_prop_selected && self.keyframe_editor.edit_mode {
                            format!("Pos Y: {}", self.keyframe_editor.edit_buffer)
                        } else {
                            format!("Pos Y: {:4}", props.position_offset.1)
                        }
                    }
                    2 => {
                        if is_prop_selected && self.keyframe_editor.edit_mode {
                            format!("Opacity: {}", self.keyframe_editor.edit_buffer)
                        } else {
                            format!("Opacity: {:3}", props.opacity)
                        }
                    }
                    _ => format!("Blend: {}", props.blend_mode.display_name()),
                };
                lines.push(format!(" {}  {}", prop_prefix, value_str));
            }
        }

        lines.push(String::new());
        lines.push(
            " \u{2191}\u{2193} layer  \u{2190}\u{2192} prop  Enter edit  Esc close".to_string(),
        );

        let para = Paragraph::new(lines.join("\n")).style(Style::default());
        frame.render_widget(para, inner);
    }
}

fn lerp_i16(a: i16, b: i16, t: f64) -> i16 {
    (a as f64 + (b as f64 - a as f64) * t).round() as i16
}

fn lerp_u8(a: u8, b: u8, t: f64) -> u8 {
    (a as f64 + (b as f64 - a as f64) * t).round() as u8
}

fn step_blend_mode(a: BlendMode, b: BlendMode, t: f64) -> BlendMode {
    if t < 0.5 {
        a
    } else {
        b
    }
}

fn ease_bounce(t: f64) -> f64 {
    if t < 1.0 / 2.75 {
        7.5625 * t * t
    } else if t < 2.0 / 2.75 {
        let t2 = t - 1.5 / 2.75;
        7.5625 * t2 * t2 + 0.75
    } else if t < 2.5 / 2.75 {
        let t2 = t - 2.25 / 2.75;
        7.5625 * t2 * t2 + 0.9375
    } else {
        let t2 = t - 2.625 / 2.75;
        7.5625 * t2 * t2 + 0.984375
    }
}

impl AnimationTimeline {
    /// Create an `AnimationTimeline` configured for the bottom timeline panel.
    pub fn panel_instance() -> Self {
        Self {
            label_col_width: 11,
            cell_width: 3,
            theme: TimelineTheme::default(),
        }
    }

    fn frame_stride(&self) -> u16 {
        self.cell_width + 1 // content + separator
    }

    /// Split the timeline panel's inner area into the frame-grid area and
    /// the single-row toolbar area below it. Shared by the render call
    /// site and mouse hit-testing so they can't drift apart.
    pub fn split_area(area: Rect) -> (Rect, Rect) {
        let split = Layout::vertical([Constraint::Fill(1), Constraint::Length(1)])
            .spacing(0)
            .split(area);
        (split[0], split[1])
    }

    /// Map a screen column within the frame-grid area back to a frame
    /// index, accounting for horizontal scroll. Real frames return
    /// `Some`; the next `GHOST_SLOTS` columns past the last frame return
    /// `Some` too via `ghost_at_col` — plain `None` only for the label
    /// gutter or truly out-of-bounds columns.
    pub fn frame_at_col(&self, col: u16, area: Rect, state: &TimelineState) -> Option<usize> {
        let stride = self.frame_stride();
        if stride == 0 || col < area.x + self.label_col_width {
            return None;
        }
        let vis_i = (col - area.x - self.label_col_width) / stride;
        let idx = state.scroll_offset + vis_i as usize;
        if idx < state.frames.len() {
            return Some(idx);
        }
        // Ghost zone: clickable blank slots past the last frame.
        let ghosts = self.ghost_count(area, state);
        if idx < state.frames.len() + ghosts {
            return Some(idx);
        }
        None
    }

    /// Number of clickable ghost slots visible past the last frame:
    /// fixed affordance count clamped to the visible window.
    pub fn ghost_count(&self, area: Rect, state: &TimelineState) -> usize {
        let stride = self.frame_stride();
        if stride == 0 || area.width <= self.label_col_width {
            return 0;
        }
        let max_vis = (area.width - self.label_col_width) as usize / stride as usize;
        let real_vis = state
            .frames
            .len()
            .saturating_sub(state.scroll_offset)
            .min(max_vis);
        const GHOST_SLOTS: usize = 3;
        (GHOST_SLOTS).min(max_vis.saturating_sub(real_vis))
    }

    /// True when `idx` is a ghost slot (at/after the end of `frames`).
    pub fn is_ghost_idx(&self, idx: usize, state: &TimelineState) -> bool {
        idx >= state.frames.len()
    }
}

/// Clickable transport-bar buttons rendered in the timeline's toolbar row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportButton {
    /// Toggles play/pause — starts inline playback if none is active yet.
    PlayPause,
    Stop,
    Loop,
}

/// Render a persistent, mouse-clickable transport bar (play/pause, stop,
/// loop) plus the existing keyboard-shortcut hint text, into the timeline
/// panel's toolbar row. Visible whenever the timeline is shown, independent
/// of whether playback is currently active — replaces the old static,
/// non-interactive hint-only `Paragraph`. Returns each button's hit-rect
/// for `handle_mouse_event` to consult (same store/hit-test convention as
/// `menu.rs`'s `MenuBarState.item_rects`).
pub fn render_transport_bar(
    frame: &mut Frame,
    area: Rect,
    playing: bool,
    loop_enabled: bool,
    theme: &Theme,
) -> Vec<(TransportButton, Rect)> {
    if area.width == 0 || area.height == 0 {
        return Vec::new();
    }

    let play_glyph = if playing { '\u{23F8}' } else { '\u{25B6}' };
    let labels: [(TransportButton, String); 3] = [
        (TransportButton::PlayPause, format!(" {play_glyph} ")),
        (TransportButton::Stop, " \u{23F9} ".to_string()),
        (
            TransportButton::Loop,
            format!(" \u{1F501}{} ", if loop_enabled { "On" } else { "Off" }),
        ),
    ];

    let mut rects = Vec::new();
    let mut spans = Vec::new();
    let mut x = area.x;
    for (action, label) in &labels {
        let w = (label.chars().count() as u16).min(area.x + area.width - x);
        rects.push((
            *action,
            Rect {
                x,
                y: area.y,
                width: w,
                height: 1,
            },
        ));
        let style = if *action == TransportButton::Loop && loop_enabled {
            Style::default()
                .fg(theme.general.primary)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.general.secondary)
        };
        spans.push(Span::styled(label.clone(), style));
        x += w;
    }

    spans.push(Span::styled(
        " [Space] pause  [\u{2190}/\u{2192}] seek  [+/-] speed  [Esc/q] stop  |  [A] Add Frame  [Del] Delete",
        Style::default().fg(theme.general.secondary),
    ));

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
    rects
}

impl Widget for &AnimationTimeline {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        for x in area.x..area.x + area.width {
            if let Some(cell) = buf.cell_mut((x, area.y)) {
                cell.set_char('─');
                cell.set_style(Style::default().fg(self.theme.ruler));
            }
        }
    }
}

impl StatefulWidget for &AnimationTimeline {
    type State = TimelineState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        // No frames yet — show a hint.
        if state.frames.is_empty() {
            let hint = "[A] Capture frame to start animation";
            let x = area.x + area.width.saturating_sub(hint.len() as u16) / 2;
            for (i, ch) in hint.chars().enumerate() {
                let cx = x + i as u16;
                if cx >= area.x + area.width {
                    break;
                }
                if let Some(cell) = buf.cell_mut((cx, area.y)) {
                    cell.set_char(ch);
                    cell.set_style(Style::default().fg(self.theme.ruler));
                }
            }
            return;
        }

        let stride = self.frame_stride(); // separator + cell_width
        if stride == 0 || area.width <= self.label_col_width {
            return;
        }

        let frame_area_w = area.width - self.label_col_width;
        let max_vis_frames = (frame_area_w / stride) as usize;
        state.cached_max_vis_frames = max_vis_frames.max(1);
        let f_start = state.scroll_offset.min(state.frames.len());
        let f_end = (f_start + max_vis_frames).min(state.frames.len());

        let n_layers = state.layer_names.len().max(
            state
                .frames
                .iter()
                .map(|f| f.layer_keyframes.len())
                .max()
                .unwrap_or(0),
        );

        // ── Ruler row (row 0) ────────────────────────────────────────────────
        let ruler_y = area.y;
        // Header cell over the label column
        let header = "Layers";
        for (i, ch) in header.chars().enumerate() {
            let cx = area.x + i as u16;
            if cx >= area.x + self.label_col_width {
                break;
            }
            if let Some(cell) = buf.cell_mut((cx, ruler_y)) {
                cell.set_char(ch);
                cell.set_style(
                    Style::default()
                        .fg(self.theme.ruler)
                        .add_modifier(ratatui::style::Modifier::BOLD),
                );
            }
        }
        // Frame number columns
        for (vis_i, frame_idx) in (f_start..f_end).enumerate() {
            let col_x = area.x + self.label_col_width + vis_i as u16 * stride;
            let is_active = frame_idx == state.current_frame;
            let has_light_kf = state.light_keyframe_frames.contains(&frame_idx);

            // Separator
            if let Some(cell) = buf.cell_mut((col_x, ruler_y)) {
                cell.set_char('│');
                cell.set_style(Style::default().fg(self.theme.ruler));
            }
            // Light keyframe indicator (diamond in first char position)
            if has_light_kf {
                let lx = col_x + 1;
                if lx < area.x + area.width {
                    if let Some(cell) = buf.cell_mut((lx, ruler_y)) {
                        cell.set_char('\u{25C6}');
                        cell.set_style(
                            Style::default()
                                .fg(self.theme.keyframe)
                                .add_modifier(ratatui::style::Modifier::BOLD),
                        );
                    }
                }
            }
            // Frame label (right-justified in cell_width)
            let label = format!("{:>width$}", frame_idx, width = self.cell_width as usize);
            let label_style = if is_active {
                Style::default()
                    .fg(self.theme.playhead)
                    .add_modifier(ratatui::style::Modifier::BOLD)
            } else {
                Style::default().fg(self.theme.ruler)
            };
            for (ci, ch) in label.chars().enumerate() {
                let cx = col_x + 1 + ci as u16;
                if cx >= area.x + area.width {
                    break;
                }
                if let Some(cell) = buf.cell_mut((cx, ruler_y)) {
                    cell.set_char(ch);
                    cell.set_style(label_style);
                }
            }
        }
        // Ghost slots: dashed `+` affordance past the last frame.
        let ghosts = self.ghost_count(area, state);
        for g in 0..ghosts {
            let vis_i = (f_end - f_start) + g;
            let col_x = area.x + self.label_col_width + vis_i as u16 * stride;
            if col_x >= area.x + area.width {
                break;
            }
            if let Some(cell) = buf.cell_mut((col_x, ruler_y)) {
                cell.set_char('┆');
                cell.set_style(Style::default().fg(self.theme.ruler));
            }
            let ghost_style = Style::default().fg(self.theme.ruler);
            for ci in 0..self.cell_width {
                let cx = col_x + 1 + ci;
                if cx >= area.x + area.width {
                    break;
                }
                if let Some(cell) = buf.cell_mut((cx, ruler_y)) {
                    cell.set_char(if ci == self.cell_width / 2 {
                        '+'
                    } else {
                        '╌'
                    });
                    cell.set_style(ghost_style);
                }
            }
        }

        // ── Layer rows ───────────────────────────────────────────────────────
        let max_layer_rows = area.height.saturating_sub(1) as usize;
        let l_start = state
            .layer_row_offset
            .min(if n_layers > 0 { n_layers - 1 } else { 0 });
        let l_end = (l_start + max_layer_rows).min(n_layers.max(1));

        for (vis_row, layer_idx) in (l_start..l_end).enumerate() {
            let row_y = area.y + 1 + vis_row as u16;
            if row_y >= area.y + area.height {
                break;
            }

            // Layer name label
            let name = state
                .layer_names
                .get(layer_idx)
                .map(|s| s.as_str())
                .unwrap_or("");
            let truncated: String = name
                .chars()
                .take(self.label_col_width.saturating_sub(1) as usize)
                .collect();
            let padded = format!(
                "{:<width$}",
                truncated,
                width = self.label_col_width as usize
            );
            for (i, ch) in padded.chars().enumerate() {
                let cx = area.x + i as u16;
                if let Some(cell) = buf.cell_mut((cx, row_y)) {
                    cell.set_char(ch);
                }
            }

            // Frame cells for this layer
            for (vis_i, frame_idx) in (f_start..f_end).enumerate() {
                let col_x = area.x + self.label_col_width + vis_i as u16 * stride;
                let is_active_frame = frame_idx == state.current_frame;
                let frame = &state.frames[frame_idx];
                let has_kf = frame
                    .layer_keyframes
                    .get(layer_idx)
                    .and_then(|k| *k)
                    .is_some();

                // Separator
                if let Some(cell) = buf.cell_mut((col_x, row_y)) {
                    cell.set_char('│');
                    cell.set_style(Style::default().fg(self.theme.ruler));
                }

                // Cell content: centre char + padding
                let (marker, style) = if has_kf {
                    (
                        '◆',
                        if is_active_frame {
                            Style::default().fg(self.theme.active_frame_border)
                        } else {
                            Style::default().fg(self.theme.keyframe)
                        },
                    )
                } else {
                    (
                        '·',
                        if is_active_frame {
                            Style::default().fg(self.theme.active_frame_border)
                        } else {
                            Style::default().fg(self.theme.ruler)
                        },
                    )
                };

                // Write centred marker inside cell_width
                let marker_offset = self.cell_width / 2;
                for ci in 0..self.cell_width {
                    let cx = col_x + 1 + ci;
                    if cx >= area.x + area.width {
                        break;
                    }
                    if let Some(cell) = buf.cell_mut((cx, row_y)) {
                        let ch = if ci == marker_offset { marker } else { ' ' };
                        cell.set_char(ch);
                        cell.set_style(style);
                    }
                }
            }
            // Ghost cells: dashed outline with centered `+`.
            let ghosts = self.ghost_count(area, state);
            for g in 0..ghosts {
                let vis_i = (f_end - f_start) + g;
                let col_x = area.x + self.label_col_width + vis_i as u16 * stride;
                if col_x >= area.x + area.width {
                    break;
                }
                if let Some(cell) = buf.cell_mut((col_x, row_y)) {
                    cell.set_char('┆');
                    cell.set_style(Style::default().fg(self.theme.ruler));
                }
                let marker_offset = self.cell_width / 2;
                for ci in 0..self.cell_width {
                    let cx = col_x + 1 + ci;
                    if cx >= area.x + area.width {
                        break;
                    }
                    if let Some(cell) = buf.cell_mut((cx, row_y)) {
                        let ch = if ci == marker_offset { '+' } else { '╌' };
                        cell.set_char(ch);
                        cell.set_style(Style::default().fg(self.theme.ruler));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Modifier;

    // label_col_width=11, cell_width=3, stride=4
    fn make_test_timeline() -> AnimationTimeline {
        AnimationTimeline {
            label_col_width: 11,
            cell_width: 3,
            theme: TimelineTheme::default(),
        }
    }

    fn make_frame(thumb: Vec<Vec<char>>, has_kf: bool, label: &str) -> TimelineFrame {
        TimelineFrame {
            thumbnail: thumb,
            has_keyframe: has_kf,
            label: label.to_string(),
            delay: 10,
            document_state: Vec::new(),
            layer_keyframes: Vec::new(),
        }
    }

    #[test]
    fn test_sync_layer_names_populates_from_stack() {
        let mut stack = LayerStack::new(5, 5);
        stack.add(5, 5);
        stack.add(5, 5);
        stack.layers[1].name = "Midground".to_string();
        stack.layers[2].name = "Foreground".to_string();

        let mut state = TimelineState::default();
        assert!(state.layer_names.is_empty());
        state.sync_layer_names(&stack);
        assert_eq!(
            state.layer_names,
            vec!["Background", "Midground", "Foreground"]
        );
    }

    #[test]
    fn test_sync_layer_names_overwrites_stale_value() {
        let stack = LayerStack::new(5, 5);
        let mut state = TimelineState {
            layer_names: vec!["Stale".to_string(), "Stale2".to_string()],
            ..TimelineState::default()
        };
        state.sync_layer_names(&stack);
        assert_eq!(state.layer_names, vec!["Background"]);
    }

    #[test]
    fn test_split_area() {
        let area = Rect::new(2, 3, 20, 10);
        let (grid, toolbar) = AnimationTimeline::split_area(area);
        assert_eq!(toolbar.height, 1);
        assert_eq!(toolbar.y, area.y + area.height - 1);
        assert_eq!(grid.height, 9);
        assert_eq!(grid.y, area.y);
        assert_eq!(grid.width, area.width);
        assert_eq!(toolbar.width, area.width);
    }

    #[test]
    fn test_fill_ghosts_left_click_clones_plain() {
        let timeline = make_test_timeline();
        let mut state = TimelineState {
            frames: vec![
                make_frame(vec![vec!['a']], false, "0"),
                make_frame(vec![vec!['b']], false, "1"),
            ],
            ..TimelineState::default()
        };
        assert!(timeline.is_ghost_idx(2, &state));
        let made = state.fill_ghosts_to(4, false);
        assert_eq!(made, 3);
        assert_eq!(state.frames.len(), 5);
        for f in &state.frames[2..] {
            assert!(!f.has_keyframe, "left-click fills plain snapshots");
            assert_eq!(f.thumbnail, state.frames[1].thumbnail);
        }
    }

    #[test]
    fn test_fill_ghosts_right_click_stamps_keyframes() {
        let mut state = TimelineState {
            frames: vec![make_frame(vec![vec!['a']], false, "0")],
            ..TimelineState::default()
        };
        let made = state.fill_ghosts_to(2, true);
        assert_eq!(made, 2);
        for f in &state.frames[1..] {
            assert!(f.has_keyframe, "right-click stamps keyframes");
        }
    }

    #[test]
    fn test_fill_ghosts_noop_on_real_or_empty() {
        let mut state = TimelineState {
            frames: vec![make_frame(vec![], false, "0")],
            ..TimelineState::default()
        };
        assert_eq!(state.fill_ghosts_to(0, false), 0);
        let mut empty = TimelineState::default();
        assert_eq!(empty.fill_ghosts_to(3, true), 0);
    }

    #[test]
    fn test_frame_at_col_maps_grid_columns_to_frame_index() {
        // label_col_width=11, cell_width=3, stride=4
        let timeline = make_test_timeline();
        let state = TimelineState {
            frames: (0..5)
                .map(|i| make_frame(vec![], false, &format!("{i}")))
                .collect(),
            ..TimelineState::default()
        };
        let area = Rect::new(0, 0, 30, 2);

        assert_eq!(
            timeline.frame_at_col(10, area, &state),
            None,
            "columns before the label gutter aren't a frame"
        );
        assert_eq!(timeline.frame_at_col(11, area, &state), Some(0));
        assert_eq!(timeline.frame_at_col(14, area, &state), Some(0));
        assert_eq!(timeline.frame_at_col(15, area, &state), Some(1));
        assert_eq!(timeline.frame_at_col(19, area, &state), Some(2));
    }

    #[test]
    fn test_frame_at_col_out_of_bounds_is_none() {
        let timeline = make_test_timeline();
        let state = TimelineState {
            frames: (0..2)
                .map(|i| make_frame(vec![], false, &format!("{i}")))
                .collect(),
            ..TimelineState::default()
        };
        let area = Rect::new(0, 0, 30, 2);
        // col 19 -> vis_i=2 -> idx=2: the first ghost slot past the 2
        // real frames.
        assert_eq!(timeline.frame_at_col(19, area, &state), Some(2));
        assert!(timeline.is_ghost_idx(2, &state));
        // Far past the ghost zone is still None.
        assert_eq!(timeline.frame_at_col(60, area, &state), None);
    }

    #[test]
    fn test_frame_at_col_respects_scroll_offset() {
        let timeline = make_test_timeline();
        let state = TimelineState {
            frames: (0..20)
                .map(|i| make_frame(vec![], false, &format!("{i}")))
                .collect(),
            scroll_offset: 10,
            ..TimelineState::default()
        };
        let area = Rect::new(0, 0, 30, 2);
        assert_eq!(timeline.frame_at_col(11, area, &state), Some(10));
        assert_eq!(timeline.frame_at_col(15, area, &state), Some(11));
    }

    fn render_transport(
        playing: bool,
        loop_enabled: bool,
        w: u16,
    ) -> (Vec<(TransportButton, Rect)>, Buffer) {
        let backend = ratatui::backend::TestBackend::new(w, 1);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let theme = Theme::default();
        let mut rects = Vec::new();
        terminal
            .draw(|frame| {
                rects = render_transport_bar(frame, frame.area(), playing, loop_enabled, &theme);
            })
            .unwrap();
        (rects, terminal.backend().buffer().clone())
    }

    #[test]
    fn test_render_transport_bar_returns_three_button_rects() {
        let (rects, _buf) = render_transport(false, false, 60);
        assert_eq!(rects.len(), 3);
        assert_eq!(rects[0].0, TransportButton::PlayPause);
        assert_eq!(rects[1].0, TransportButton::Stop);
        assert_eq!(rects[2].0, TransportButton::Loop);
    }

    #[test]
    fn test_render_transport_bar_play_glyph_reflects_playing_state() {
        let (_rects, buf) = render_transport(false, false, 60);
        assert_eq!(
            buf.cell((1, 0)).unwrap().symbol(),
            "\u{25B6}",
            "paused shows play glyph"
        );

        let (_rects, buf) = render_transport(true, false, 60);
        assert_eq!(
            buf.cell((1, 0)).unwrap().symbol(),
            "\u{23F8}",
            "playing shows pause glyph"
        );
    }

    #[test]
    fn test_render_transport_bar_button_rects_dont_overlap() {
        let (rects, _buf) = render_transport(false, false, 60);
        for pair in rects.windows(2) {
            assert!(
                pair[0].1.x + pair[0].1.width <= pair[1].1.x,
                "button rects must not overlap"
            );
        }
    }

    #[test]
    fn test_render_transport_bar_too_narrow_still_returns_rects_without_panic() {
        let (rects, _buf) = render_transport(false, false, 2);
        assert!(!rects.is_empty());
    }

    // ─── Render tests ────────────────────────────────────────────────

    #[test]
    fn test_timeline_basic_render() {
        // label_col=11, stride=4
        // Frame 0: col_x=11, sep at (11,0), label "  0" at (12,13,14)
        let timeline = make_test_timeline();
        let mut state = TimelineState {
            frames: (0..3)
                .map(|i| make_frame(vec![], false, &format!("{}", i)))
                .collect(),
            layer_names: vec!["Layer 1".into()],
            ..TimelineState::default()
        };

        let area = Rect::new(0, 0, 30, 4);
        let mut buf = Buffer::empty(area);
        StatefulWidget::render(&timeline, area, &mut buf, &mut state);

        assert_eq!(
            buf.cell((0, 0)).unwrap().symbol(),
            "L",
            "ruler header 'Layers'"
        );
        assert_eq!(
            buf.cell((11, 0)).unwrap().symbol(),
            "│",
            "frame 0 separator"
        );
        assert_eq!(buf.cell((14, 0)).unwrap().symbol(), "0", "frame 0 number");
        assert_eq!(buf.cell((18, 0)).unwrap().symbol(), "1", "frame 1 number");
    }

    #[test]
    fn test_timeline_playhead_update() {
        // current_frame=2: col_x=11+2*4=19, label "  2" at (20,21,22) — BOLD
        // frame 0: col_x=11, label "  0" at (12,13,14) — not BOLD
        let timeline = make_test_timeline();
        let mut state = TimelineState {
            frames: (0..5)
                .map(|i| make_frame(vec![], false, &format!("{}", i)))
                .collect(),
            current_frame: 2,
            layer_names: vec!["L1".into()],
            ..TimelineState::default()
        };

        let area = Rect::new(0, 0, 40, 3);
        let mut buf = Buffer::empty(area);
        StatefulWidget::render(&timeline, area, &mut buf, &mut state);

        let active = buf.cell((22, 0)).unwrap();
        assert_eq!(active.symbol(), "2");
        assert!(
            active.style().add_modifier.contains(Modifier::BOLD),
            "active frame number should be BOLD"
        );

        let inactive = buf.cell((14, 0)).unwrap();
        assert_eq!(inactive.symbol(), "0");
        assert!(
            !inactive.style().add_modifier.contains(Modifier::BOLD),
            "non-active frame number should not be BOLD"
        );
    }

    #[test]
    fn test_timeline_constraints() {
        // label_col=11, stride=4, 3 frames → width=23, 1 ruler + 2 layer rows
        let timeline = make_test_timeline();
        let mut state = TimelineState {
            frames: (0..3)
                .map(|i| make_frame(vec![], false, &format!("{}", i)))
                .collect(),
            layer_names: vec!["Alpha".into(), "Beta".into()],
            ..TimelineState::default()
        };

        let area = Rect::new(0, 0, 23, 3);
        let mut buf = Buffer::empty(area);
        StatefulWidget::render(&timeline, area, &mut buf, &mut state);

        assert_eq!(
            buf.cell((0, 1)).unwrap().symbol(),
            "A",
            "layer 0 name 'Alpha'"
        );
        assert_eq!(
            buf.cell((0, 2)).unwrap().symbol(),
            "B",
            "layer 1 name 'Beta'"
        );
        assert_eq!(
            buf.cell((11, 1)).unwrap().symbol(),
            "│",
            "frame 0 sep row 1"
        );
        assert_eq!(
            buf.cell((15, 1)).unwrap().symbol(),
            "│",
            "frame 1 sep row 1"
        );
        assert_eq!(
            buf.cell((19, 1)).unwrap().symbol(),
            "│",
            "frame 2 sep row 1"
        );
    }

    #[test]
    fn test_timeline_scroll() {
        // scroll_offset=10 → frame 10 first visible
        // col_x=11, label " 10": (12,' '),(13,'1'),(14,'0')
        let timeline = make_test_timeline();
        let mut state = TimelineState {
            frames: (0..20)
                .map(|i| make_frame(vec![], false, &format!("{}", i)))
                .collect(),
            scroll_offset: 10,
            layer_names: vec!["L".into()],
            ..TimelineState::default()
        };

        let area = Rect::new(0, 0, 23, 2);
        let mut buf = Buffer::empty(area);
        StatefulWidget::render(&timeline, area, &mut buf, &mut state);

        assert_eq!(
            buf.cell((11, 0)).unwrap().symbol(),
            "│",
            "first sep is frame 10"
        );
        assert_eq!(
            buf.cell((13, 0)).unwrap().symbol(),
            "1",
            "frame 10 label '1'"
        );
        assert_eq!(
            buf.cell((14, 0)).unwrap().symbol(),
            "0",
            "frame 10 label '0'"
        );
    }

    #[test]
    fn test_timeline_keyframe_markers() {
        // Frames 0 and 2 have a keyframe for layer 0; frame 1 does not.
        // Layer 0 row (y=1). marker_offset = cell_width/2 = 1.
        // Frame 0: col_x=11, marker at (11+1+1)=13
        // Frame 1: col_x=15, marker at 17
        // Frame 2: col_x=19, marker at 21
        let timeline = make_test_timeline();
        let mut frame0 = make_frame(vec![], false, "0");
        frame0.layer_keyframes = vec![Some(LayerKeyframe::default())];
        let frame1 = make_frame(vec![], false, "1");
        let mut frame2 = make_frame(vec![], false, "2");
        frame2.layer_keyframes = vec![Some(LayerKeyframe::default())];

        let mut state = TimelineState {
            frames: vec![frame0, frame1, frame2],
            layer_names: vec!["Base".into()],
            ..TimelineState::default()
        };

        let area = Rect::new(0, 0, 23, 2);
        let mut buf = Buffer::empty(area);
        StatefulWidget::render(&timeline, area, &mut buf, &mut state);

        assert_eq!(
            buf.cell((13, 1)).unwrap().symbol(),
            "◆",
            "frame 0 has keyframe"
        );
        assert_eq!(
            buf.cell((17, 1)).unwrap().symbol(),
            "·",
            "frame 1 has no keyframe"
        );
        assert_eq!(
            buf.cell((21, 1)).unwrap().symbol(),
            "◆",
            "frame 2 has keyframe"
        );
    }

    #[test]
    fn test_timeline_empty() {
        // "[A] Capture frame to start animation" len=36, centered in width=40 → x=2
        let timeline = make_test_timeline();
        let mut state = TimelineState::default();

        let area = Rect::new(0, 0, 40, 3);
        let mut buf = Buffer::empty(area);
        StatefulWidget::render(&timeline, area, &mut buf, &mut state);

        assert_eq!(
            buf.cell((2, 0)).unwrap().symbol(),
            "[",
            "hint starts with '['"
        );
        assert_eq!(buf.cell((3, 0)).unwrap().symbol(), "A");
    }

    #[test]
    fn test_timeline_layer_names_display() {
        // Layer names truncated to label_col_width-1=10 chars, padded to 11.
        let timeline = make_test_timeline();
        let mut state = TimelineState {
            frames: vec![make_frame(vec![], false, "0")],
            layer_names: vec!["Background".into(), "Foreground".into()],
            ..TimelineState::default()
        };

        let area = Rect::new(0, 0, 15, 3);
        let mut buf = Buffer::empty(area);
        StatefulWidget::render(&timeline, area, &mut buf, &mut state);

        assert_eq!(
            buf.cell((0, 1)).unwrap().symbol(),
            "B",
            "layer 0 first char"
        );
        assert_eq!(buf.cell((9, 1)).unwrap().symbol(), "d", "layer 0 last char");
        assert_eq!(
            buf.cell((0, 2)).unwrap().symbol(),
            "F",
            "layer 1 first char"
        );
    }

    #[test]
    fn test_active_frame_column_style() {
        // current_frame=1: col_x=11+4=15, label "  1" at (16,17,18) — BOLD
        // frame 0: col_x=11, label "  0" at (12,13,14) — not BOLD
        let timeline = make_test_timeline();
        let mut state = TimelineState {
            frames: (0..2)
                .map(|i| make_frame(vec![], false, &format!("{}", i)))
                .collect(),
            current_frame: 1,
            layer_names: vec!["L".into()],
            ..TimelineState::default()
        };

        let area = Rect::new(0, 0, 19, 2);
        let mut buf = Buffer::empty(area);
        StatefulWidget::render(&timeline, area, &mut buf, &mut state);

        let active = buf.cell((18, 0)).unwrap();
        assert_eq!(active.symbol(), "1");
        assert!(active.style().add_modifier.contains(Modifier::BOLD));

        let inactive = buf.cell((14, 0)).unwrap();
        assert_eq!(inactive.symbol(), "0");
        assert!(!inactive.style().add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn test_add_frame() {
        let mut state = TimelineState::default();
        assert_eq!(state.frames.len(), 0);
        state.add_frame(make_frame(vec![vec!['A'; 3]; 2], false, "new"));
        assert_eq!(state.frames.len(), 1);
        assert_eq!(state.frames[0].label, "new");
    }

    #[test]
    fn test_insert_frame_updates_current() {
        let mut state = TimelineState {
            frames: (0..3)
                .map(|i| make_frame(vec![vec!['X'; 3]; 2], false, &format!("{}", i)))
                .collect(),
            current_frame: 1,
            scroll_offset: 0,
            fps: 12,
            keyframe_editor: KeyframeEditState::default(),
            tween: None,
            ..TimelineState::default()
        };
        state.insert_frame(0, make_frame(vec![vec!['Y'; 3]; 2], false, "inserted"));
        assert_eq!(state.frames.len(), 4);
        assert_eq!(state.frames[0].label, "inserted");
        assert_eq!(state.current_frame, 2, "current_frame should shift right");
    }

    #[test]
    fn test_remove_frame_middle() {
        let mut state = TimelineState {
            frames: (0..3)
                .map(|i| make_frame(vec![vec!['X'; 3]; 2], false, &format!("{}", i)))
                .collect(),
            current_frame: 2,
            scroll_offset: 0,
            fps: 12,
            keyframe_editor: KeyframeEditState::default(),
            tween: None,
            ..TimelineState::default()
        };
        let removed = state.remove_frame(1).unwrap();
        assert_eq!(removed.label, "1");
        assert_eq!(state.frames.len(), 2);
        assert_eq!(state.current_frame, 1, "current_frame should clamp");
        assert_eq!(state.frames[1].label, "2");
    }

    #[test]
    fn test_remove_frame_last_remaining_fails() {
        let mut state = TimelineState {
            frames: vec![make_frame(vec![vec!['X'; 3]; 2], false, "only")],
            current_frame: 0,
            scroll_offset: 0,
            fps: 12,
            keyframe_editor: KeyframeEditState::default(),
            tween: None,
            ..TimelineState::default()
        };
        assert!(state.remove_frame(0).is_err());
    }

    #[test]
    fn test_remove_frame_out_of_bounds_fails() {
        let mut state = TimelineState {
            frames: (0..2)
                .map(|i| make_frame(vec![vec!['X'; 3]; 2], false, &format!("{}", i)))
                .collect(),
            current_frame: 0,
            scroll_offset: 0,
            fps: 12,
            keyframe_editor: KeyframeEditState::default(),
            tween: None,
            ..TimelineState::default()
        };
        assert!(state.remove_frame(5).is_err());
    }

    #[test]
    fn test_duplicate_frame() {
        let mut state = TimelineState {
            frames: (0..3)
                .map(|i| make_frame(vec![vec!['X'; 3]; 2], i == 1, &format!("{}", i)))
                .collect(),
            current_frame: 1,
            scroll_offset: 0,
            fps: 12,
            keyframe_editor: KeyframeEditState::default(),
            tween: None,
            ..TimelineState::default()
        };
        state.duplicate_frame(1).unwrap();
        assert_eq!(state.frames.len(), 4);
        assert_eq!(state.frames[2].label, "1 (copy)");
        assert!(state.frames[2].has_keyframe);
        assert_eq!(
            state.current_frame, 1,
            "current_frame stays on original frame"
        );
    }

    #[test]
    fn test_duplicate_frame_out_of_bounds_fails() {
        let mut state = TimelineState::default();
        assert!(state.duplicate_frame(0).is_err());
    }

    #[test]
    fn test_reorder_forward() {
        let mut state = TimelineState {
            frames: (0..4)
                .map(|i| make_frame(vec![vec!['X'; 3]; 2], false, &format!("{}", i)))
                .collect(),
            current_frame: 0,
            scroll_offset: 0,
            fps: 12,
            keyframe_editor: KeyframeEditState::default(),
            tween: None,
            ..TimelineState::default()
        };
        state.reorder_frame(0, 2).unwrap();
        let labels: Vec<&str> = state.frames.iter().map(|f| f.label.as_str()).collect();
        assert_eq!(labels, vec!["1", "0", "2", "3"]);
    }

    #[test]
    fn test_reorder_backward() {
        let mut state = TimelineState {
            frames: (0..4)
                .map(|i| make_frame(vec![vec!['X'; 3]; 2], false, &format!("{}", i)))
                .collect(),
            current_frame: 3,
            scroll_offset: 0,
            fps: 12,
            keyframe_editor: KeyframeEditState::default(),
            tween: None,
            ..TimelineState::default()
        };
        state.reorder_frame(3, 0).unwrap();
        let labels: Vec<&str> = state.frames.iter().map(|f| f.label.as_str()).collect();
        assert_eq!(labels, vec!["3", "0", "1", "2"]);
    }

    #[test]
    fn test_reorder_forward_keeps_current_frame_on_moved_frame() {
        // Moving the current frame right: the frame is inserted at `to - 1`
        // (index 1), so current_frame must follow it there, not jump to `to`
        // (index 2), which would point at a different logical frame (F-26).
        let mut state = TimelineState {
            frames: (0..4)
                .map(|i| make_frame(vec![vec!['X'; 3]; 2], false, &format!("{}", i)))
                .collect(),
            current_frame: 0,
            ..TimelineState::default()
        };
        state.reorder_frame(0, 2).unwrap();
        assert_eq!(state.current_frame, 1, "moved frame's new slot");
        assert_eq!(state.frames[state.current_frame].label, "0");
    }

    #[test]
    fn test_reorder_backward_keeps_current_frame_on_moved_frame() {
        // Moving the current frame left: inserted at index `to` (0).
        let mut state = TimelineState {
            frames: (0..4)
                .map(|i| make_frame(vec![vec!['X'; 3]; 2], false, &format!("{}", i)))
                .collect(),
            current_frame: 3,
            ..TimelineState::default()
        };
        state.reorder_frame(3, 0).unwrap();
        assert_eq!(state.current_frame, 0, "moved frame's new slot");
        assert_eq!(state.frames[state.current_frame].label, "3");
    }

    #[test]
    fn test_reorder_forward_keeps_current_frame_on_tracked_content() {
        // Moving frame 0 right past the current frame (2): frame 2 slides
        // left by one, so current_frame must decrement to keep pointing at it.
        let mut state = TimelineState {
            frames: (0..4)
                .map(|i| make_frame(vec![vec!['X'; 3]; 2], false, &format!("{}", i)))
                .collect(),
            current_frame: 2,
            ..TimelineState::default()
        };
        state.reorder_frame(0, 3).unwrap();
        assert_eq!(state.current_frame, 1);
        assert_eq!(state.frames[state.current_frame].label, "2");
    }

    #[test]
    fn test_frame_delays_match_frames_after_reorder() {
        // Per-frame delays travel with their frames through reorder: the
        // delay for the frame labeled "3" must stay with it (F-26).
        let mut state = TimelineState::default();
        state.add_frame(make_frame(vec![], false, "a"));
        state.frames[0].delay = 7;
        state.add_frame(make_frame(vec![], false, "b"));
        state.frames[1].delay = 23;
        state.add_frame(make_frame(vec![], false, "c"));
        state.frames[2].delay = 5;
        state.add_frame(make_frame(vec![], false, "d"));
        state.frames[3].delay = 11;

        state.reorder_frame(1, 3).unwrap();
        let labels: Vec<&str> = state.frames.iter().map(|f| f.label.as_str()).collect();
        assert_eq!(labels, vec!["a", "c", "b", "d"]);
        assert_eq!(state.frame_delays(), vec![7, 5, 23, 11]);
    }

    #[test]
    fn test_frame_delays_default_from_fps() {
        let mut state = TimelineState {
            fps: 12,
            ..TimelineState::default()
        };
        assert_eq!(state.default_delay(), 8);
        state.fps = 24;
        assert_eq!(state.default_delay(), 4);
        state.fps = 200;
        assert_eq!(state.default_delay(), 1, "clamped to a positive delay");
    }

    #[test]
    fn test_reorder_out_of_bounds_fails() {
        let mut state = TimelineState::default();
        state.add_frame(make_frame(vec![vec!['A'; 3]; 2], false, "a"));
        assert!(state.reorder_frame(0, 1).is_err());
        assert!(state.reorder_frame(2, 0).is_err());
    }

    // ─── Keyframing tests ────────────────────────────────────────────

    fn make_keyframe(dx: i16, dy: i16, opacity: u8, blend: BlendMode) -> LayerKeyframe {
        LayerKeyframe {
            position_offset: (dx, dy),
            opacity,
            blend_mode: blend,
        }
    }

    #[test]
    fn test_set_keyframe_properties() {
        let mut state = TimelineState {
            frames: (0..5)
                .map(|i| make_frame(vec![vec!['X'; 3]; 2], false, &format!("{}", i)))
                .collect(),
            current_frame: 0,
            scroll_offset: 0,
            fps: 12,
            keyframe_editor: KeyframeEditState::default(),
            tween: None,
            ..TimelineState::default()
        };
        let kf = make_keyframe(5, -3, 128, BlendMode::Screen);
        assert!(state.set_keyframe(2, 0, kf));
        let got = state.get_keyframe(2, 0).unwrap();
        assert_eq!(got.position_offset, (5, -3));
        assert_eq!(got.opacity, 128);
        assert_eq!(got.blend_mode, BlendMode::Screen);
        assert!(state.frames[2].has_keyframe);
    }

    #[test]
    fn test_set_keyframe_out_of_bounds() {
        let mut state = TimelineState::default();
        let kf = make_keyframe(0, 0, 255, BlendMode::Normal);
        assert!(!state.set_keyframe(0, 0, kf));
    }

    #[test]
    fn test_interpolate_position_linear() {
        let mut state = TimelineState {
            frames: (0..11)
                .map(|i| make_frame(vec![vec![' '; 3]; 2], false, &format!("{}", i)))
                .collect(),
            current_frame: 0,
            scroll_offset: 0,
            fps: 12,
            keyframe_editor: KeyframeEditState::default(),
            tween: None,
            ..TimelineState::default()
        };
        state.set_keyframe(0, 0, make_keyframe(0, 0, 255, BlendMode::Normal));
        state.set_keyframe(10, 0, make_keyframe(100, 50, 255, BlendMode::Normal));
        let interp = state.get_interpolated_properties(5, 0);
        assert_eq!(interp.position_offset, (50, 25));
    }

    #[test]
    fn test_interpolate_opacity_linear() {
        let mut state = TimelineState {
            frames: (0..11)
                .map(|i| make_frame(vec![vec![' '; 3]; 2], false, &format!("{}", i)))
                .collect(),
            current_frame: 0,
            scroll_offset: 0,
            fps: 12,
            keyframe_editor: KeyframeEditState::default(),
            tween: None,
            ..TimelineState::default()
        };
        state.set_keyframe(0, 0, make_keyframe(0, 0, 255, BlendMode::Normal));
        state.set_keyframe(10, 0, make_keyframe(0, 0, 0, BlendMode::Normal));
        let at3 = state.get_interpolated_properties(3, 0);
        let at7 = state.get_interpolated_properties(7, 0);
        assert_eq!(at3.opacity, 179);
        assert_eq!(at7.opacity, 77);
    }

    #[test]
    fn test_interpolate_blend_mode_step() {
        let mut state = TimelineState {
            frames: (0..11)
                .map(|i| make_frame(vec![vec![' '; 3]; 2], false, &format!("{}", i)))
                .collect(),
            current_frame: 0,
            scroll_offset: 0,
            fps: 12,
            keyframe_editor: KeyframeEditState::default(),
            tween: None,
            ..TimelineState::default()
        };
        state.set_keyframe(0, 0, make_keyframe(0, 0, 255, BlendMode::Normal));
        state.set_keyframe(10, 0, make_keyframe(0, 0, 255, BlendMode::Multiply));
        for f in 0..5 {
            let props = state.get_interpolated_properties(f, 0);
            assert_eq!(
                props.blend_mode,
                BlendMode::Normal,
                "frame {f} should be Normal"
            );
        }
        for f in 5..10 {
            let props = state.get_interpolated_properties(f, 0);
            assert_eq!(
                props.blend_mode,
                BlendMode::Multiply,
                "frame {f} should be Multiply"
            );
        }
    }

    #[test]
    fn test_interpolate_before_first_keyframe() {
        let mut state = TimelineState {
            frames: (0..10)
                .map(|i| make_frame(vec![vec![' '; 3]; 2], false, &format!("{}", i)))
                .collect(),
            current_frame: 0,
            scroll_offset: 0,
            fps: 12,
            keyframe_editor: KeyframeEditState::default(),
            tween: None,
            ..TimelineState::default()
        };
        state.set_keyframe(5, 0, make_keyframe(42, 99, 128, BlendMode::Screen));
        let props = state.get_interpolated_properties(2, 0);
        assert_eq!(props.position_offset, (42, 99));
        assert_eq!(props.opacity, 128);
        assert_eq!(props.blend_mode, BlendMode::Screen);
    }

    #[test]
    fn test_interpolate_after_last_keyframe() {
        let mut state = TimelineState {
            frames: (0..10)
                .map(|i| make_frame(vec![vec![' '; 3]; 2], false, &format!("{}", i)))
                .collect(),
            current_frame: 0,
            scroll_offset: 0,
            fps: 12,
            keyframe_editor: KeyframeEditState::default(),
            tween: None,
            ..TimelineState::default()
        };
        state.set_keyframe(3, 0, make_keyframe(10, 20, 200, BlendMode::Add));
        let props = state.get_interpolated_properties(8, 0);
        assert_eq!(props.position_offset, (10, 20));
        assert_eq!(props.opacity, 200);
        assert_eq!(props.blend_mode, BlendMode::Add);
    }

    #[test]
    fn test_interpolate_single_keyframe() {
        let mut state = TimelineState {
            frames: (0..10)
                .map(|i| make_frame(vec![vec![' '; 3]; 2], false, &format!("{}", i)))
                .collect(),
            current_frame: 0,
            scroll_offset: 0,
            fps: 12,
            keyframe_editor: KeyframeEditState::default(),
            tween: None,
            ..TimelineState::default()
        };
        state.set_keyframe(5, 0, make_keyframe(7, 8, 100, BlendMode::Overlay));
        for f in 0..10 {
            let props = state.get_interpolated_properties(f, 0);
            assert_eq!(props.position_offset, (7, 8), "frame {f}");
            assert_eq!(props.opacity, 100, "frame {f}");
        }
    }

    #[test]
    fn test_interpolate_no_keyframes() {
        let state = TimelineState {
            frames: (0..5)
                .map(|i| make_frame(vec![vec![' '; 3]; 2], false, &format!("{}", i)))
                .collect(),
            current_frame: 0,
            scroll_offset: 0,
            fps: 12,
            keyframe_editor: KeyframeEditState::default(),
            tween: None,
            ..TimelineState::default()
        };
        let props = state.get_interpolated_properties(2, 0);
        assert_eq!(props.position_offset, (0, 0));
        assert_eq!(props.opacity, 255);
        assert_eq!(props.blend_mode, BlendMode::Normal);
    }

    #[test]
    fn test_remove_keyframe() {
        let mut state = TimelineState {
            frames: (0..10)
                .map(|i| make_frame(vec![vec![' '; 3]; 2], false, &format!("{}", i)))
                .collect(),
            current_frame: 0,
            scroll_offset: 0,
            fps: 12,
            keyframe_editor: KeyframeEditState::default(),
            tween: None,
            ..TimelineState::default()
        };
        state.set_keyframe(0, 0, make_keyframe(0, 0, 255, BlendMode::Normal));
        state.set_keyframe(5, 0, make_keyframe(50, 0, 128, BlendMode::Multiply));
        state.set_keyframe(9, 0, make_keyframe(100, 0, 0, BlendMode::Screen));
        assert!(state.remove_keyframe(5, 0));
        let props = state.get_interpolated_properties(5, 0);
        // After removal, interpolates between frame 0 (0,0) and frame 9 (100,0): lerp at t=5/9
        assert_eq!(props.position_offset, (56, 0));
    }

    #[test]
    fn test_keyframe_multi_layer() {
        let mut state = TimelineState {
            frames: (0..11)
                .map(|i| make_frame(vec![vec![' '; 3]; 2], false, &format!("{}", i)))
                .collect(),
            current_frame: 0,
            scroll_offset: 0,
            fps: 12,
            keyframe_editor: KeyframeEditState::default(),
            tween: None,
            ..TimelineState::default()
        };
        state.set_keyframe(0, 0, make_keyframe(0, 0, 100, BlendMode::Normal));
        state.set_keyframe(10, 0, make_keyframe(100, 0, 200, BlendMode::Normal));
        state.set_keyframe(0, 1, make_keyframe(0, 50, 50, BlendMode::Screen));
        state.set_keyframe(10, 1, make_keyframe(0, 100, 150, BlendMode::Add));
        let l0 = state.get_interpolated_properties(5, 0);
        let l1 = state.get_interpolated_properties(5, 1);
        assert_eq!(l0.position_offset, (50, 0));
        assert_eq!(l0.opacity, 150);
        assert_eq!(l1.position_offset, (0, 75));
        assert_eq!(l1.opacity, 100);
        assert_eq!(l1.blend_mode, BlendMode::Add);
    }

    #[test]
    fn test_keyframe_editor_open_close() {
        let mut state = TimelineState::default();
        assert!(!state.keyframe_editor.open);
        state.keyframe_editor.open = true;
        assert!(state.keyframe_editor.open);
        state.handle_keyframe_editor_key(KeyCode::Esc);
        assert!(!state.keyframe_editor.open);
    }

    #[test]
    fn test_keyframe_editor_navigation() {
        let mut state = TimelineState::default();
        state.keyframe_editor.open = true;
        state.keyframe_editor.selected_layer = 2;
        state.handle_keyframe_editor_key(KeyCode::Up);
        assert_eq!(state.keyframe_editor.selected_layer, 1);
        state.handle_keyframe_editor_key(KeyCode::Down);
        assert_eq!(state.keyframe_editor.selected_layer, 2);
        state.handle_keyframe_editor_key(KeyCode::Left);
        assert_eq!(state.keyframe_editor.selected_property, 0);
        state.keyframe_editor.selected_property = 2;
        state.handle_keyframe_editor_key(KeyCode::Right);
        assert_eq!(state.keyframe_editor.selected_property, 3);
        state.handle_keyframe_editor_key(KeyCode::Right);
        assert_eq!(state.keyframe_editor.selected_property, 3);
    }

    #[test]
    fn test_keyframe_editor_numeric_edit() {
        let mut state = TimelineState {
            frames: (0..5)
                .map(|i| make_frame(vec![vec![' '; 3]; 2], false, &format!("{}", i)))
                .collect(),
            current_frame: 0,
            scroll_offset: 0,
            fps: 12,
            keyframe_editor: KeyframeEditState::default(),
            tween: None,
            ..TimelineState::default()
        };
        state.keyframe_editor.open = true;
        state.set_keyframe(0, 0, make_keyframe(10, 20, 128, BlendMode::Normal));
        state.handle_keyframe_editor_key(KeyCode::Enter);
        assert!(state.keyframe_editor.edit_mode);
        state.handle_keyframe_editor_key(KeyCode::Backspace);
        state.handle_keyframe_editor_key(KeyCode::Backspace);
        state.handle_keyframe_editor_key(KeyCode::Char('3'));
        state.handle_keyframe_editor_key(KeyCode::Char('0'));
        state.handle_keyframe_editor_key(KeyCode::Enter);
        assert!(!state.keyframe_editor.edit_mode);
        let props = state.get_interpolated_properties(0, 0);
        assert_eq!(props.position_offset.0, 30);
    }

    #[test]
    fn test_keyframe_editor_blend_cycle() {
        let mut state = TimelineState {
            frames: (0..5)
                .map(|i| make_frame(vec![vec![' '; 3]; 2], false, &format!("{}", i)))
                .collect(),
            current_frame: 0,
            scroll_offset: 0,
            fps: 12,
            keyframe_editor: KeyframeEditState::default(),
            tween: None,
            ..TimelineState::default()
        };
        state.set_keyframe(0, 0, make_keyframe(0, 0, 255, BlendMode::Normal));
        state.keyframe_editor.open = true;
        state.keyframe_editor.selected_property = 3;
        // Blend mode: Normal -> Multiply
        state.handle_keyframe_editor_key(KeyCode::Enter);
        let props = state.get_interpolated_properties(0, 0);
        assert_eq!(props.blend_mode, BlendMode::Multiply);
    }

    #[test]
    fn test_keyframe_has_keyframe_derived() {
        let mut state = TimelineState {
            frames: (0..3)
                .map(|i| make_frame(vec![vec![' '; 3]; 2], false, &format!("{}", i)))
                .collect(),
            current_frame: 0,
            scroll_offset: 0,
            fps: 12,
            keyframe_editor: KeyframeEditState::default(),
            tween: None,
            ..TimelineState::default()
        };
        assert!(!state.frames[0].has_keyframe);
        state.set_keyframe(0, 0, make_keyframe(0, 0, 255, BlendMode::Normal));
        assert!(state.frames[0].has_keyframe);
        state.remove_keyframe(0, 0);
        assert!(!state.frames[0].has_keyframe);
    }

    #[test]
    fn test_get_keyframe_nonexistent() {
        let mut state = TimelineState::default();
        state.add_frame(make_frame(vec![vec![' '; 3]; 2], false, "0"));
        assert!(state.get_keyframe(0, 0).is_none());
        assert!(state.get_keyframe(5, 0).is_none());
    }

    #[test]
    fn test_interpolate_same_frame_keyframe() {
        let mut state = TimelineState {
            frames: (0..10)
                .map(|i| make_frame(vec![vec![' '; 3]; 2], false, &format!("{}", i)))
                .collect(),
            current_frame: 0,
            scroll_offset: 0,
            fps: 12,
            keyframe_editor: KeyframeEditState::default(),
            tween: None,
            ..TimelineState::default()
        };
        state.set_keyframe(5, 0, make_keyframe(10, 20, 100, BlendMode::Overlay));
        let props = state.get_interpolated_properties(5, 0);
        assert_eq!(props.position_offset, (10, 20));
        assert_eq!(props.opacity, 100);
        assert_eq!(props.blend_mode, BlendMode::Overlay);
    }

    // ─── Tweening tests ──────────────────────────────────────────────

    fn make_tween_state() -> TimelineState {
        let mut state = TimelineState {
            frames: (0..11)
                .map(|i| make_frame(vec![vec![' '; 3]; 2], false, &format!("{}", i)))
                .collect(),
            current_frame: 0,
            scroll_offset: 0,
            fps: 12,
            keyframe_editor: KeyframeEditState::default(),
            tween: None,
            ..TimelineState::default()
        };
        state.set_keyframe(0, 0, make_keyframe(0, 0, 255, BlendMode::Normal));
        state.set_keyframe(10, 0, make_keyframe(100, 50, 0, BlendMode::Multiply));
        state
    }

    #[test]
    fn test_easing_linear() {
        let f = EasingFunction::Linear;
        assert!((f.apply(0.0) - 0.0).abs() < 1e-10);
        assert!((f.apply(0.5) - 0.5).abs() < 1e-10);
        assert!((f.apply(1.0) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_easing_ease_in() {
        let f = EasingFunction::EaseIn;
        assert!((f.apply(0.0) - 0.0).abs() < 1e-10);
        assert!((f.apply(1.0) - 1.0).abs() < 1e-10);
        // EaseIn at t=0.5 is slower (cubic: 0.125) vs linear (0.5)
        assert!(f.apply(0.5) < 0.5);
        assert!((f.apply(0.5) - 0.125).abs() < 1e-10);
    }

    #[test]
    fn test_easing_ease_out() {
        let f = EasingFunction::EaseOut;
        assert!((f.apply(0.0) - 0.0).abs() < 1e-10);
        assert!((f.apply(1.0) - 1.0).abs() < 1e-10);
        // EaseOut at t=0.5 is faster (1 - 0.5^3 = 0.875) vs linear (0.5)
        assert!(f.apply(0.5) > 0.5);
    }

    #[test]
    fn test_easing_bounce() {
        let f = EasingFunction::Bounce;
        assert!((f.apply(0.0) - 0.0).abs() < 1e-10);
        assert!((f.apply(1.0) - 1.0).abs() < 1e-10);
        // Bounce overshoots 1.0 at some t < 1.0
        assert!(f.apply(0.5) > 0.5);
    }

    #[test]
    fn test_easing_display_names() {
        assert_eq!(EasingFunction::Linear.display_name(), "Linear");
        assert_eq!(EasingFunction::EaseIn.display_name(), "Ease In");
        assert_eq!(EasingFunction::EaseOut.display_name(), "Ease Out");
        assert_eq!(EasingFunction::Bounce.display_name(), "Bounce");
    }

    #[test]
    fn test_easing_cycle() {
        assert_eq!(EasingFunction::Linear.cycle(), EasingFunction::EaseIn);
        assert_eq!(EasingFunction::EaseIn.cycle(), EasingFunction::EaseOut);
        assert_eq!(EasingFunction::EaseOut.cycle(), EasingFunction::Bounce);
        assert_eq!(EasingFunction::Bounce.cycle(), EasingFunction::Linear);
    }

    #[test]
    fn test_tween_generates_correct_count() {
        let mut state = make_tween_state();
        state.open_tween();
        state.tween.as_mut().unwrap().config.start_frame = 0;
        state.tween.as_mut().unwrap().config.end_frame = 10;
        state.tween.as_mut().unwrap().config.num_frames = 5;
        state.compute_tween();
        let tp = state.tween.as_ref().unwrap();
        assert!(tp.valid);
        assert_eq!(tp.generated_frames.len(), 5);
    }

    #[test]
    fn test_tween_linear_midpoint() {
        let mut state = make_tween_state();
        state.open_tween();
        {
            let t = state.tween.as_mut().unwrap();
            t.config.start_frame = 0;
            t.config.end_frame = 10;
            t.config.num_frames = 1;
            t.config.easing = EasingFunction::Linear;
        }
        state.compute_tween();
        let tp = state.tween.as_ref().unwrap();
        assert!(tp.valid);
        assert_eq!(tp.generated_frames.len(), 1);
        let kf = tp.generated_frames[0].layer_keyframes[0].unwrap();
        // With 1 intermediate frame, t = 1/(1+1) = 0.5
        // Linear midpoint between (0,0) and (100,50) = (50, 25)
        assert_eq!(kf.position_offset, (50, 25));
    }

    #[test]
    fn test_tween_ease_in_midpoint() {
        let mut state = make_tween_state();
        state.open_tween();
        {
            let t = state.tween.as_mut().unwrap();
            t.config.start_frame = 0;
            t.config.end_frame = 10;
            t.config.num_frames = 1;
            t.config.easing = EasingFunction::EaseIn;
        }
        state.compute_tween();
        let tp = state.tween.as_ref().unwrap();
        assert!(tp.valid);
        let kf = tp.generated_frames[0].layer_keyframes[0].unwrap();
        // EaseIn at t=0.5: 0.125. Interpolate X: 0 + (100-0) * 0.125 = 12.5 ≈ 13
        assert!(kf.position_offset.0 < 50);
        assert_eq!(kf.position_offset.0, 13);
    }

    #[test]
    fn test_tween_ease_out_midpoint() {
        let mut state = make_tween_state();
        state.open_tween();
        {
            let t = state.tween.as_mut().unwrap();
            t.config.start_frame = 0;
            t.config.end_frame = 10;
            t.config.num_frames = 1;
            t.config.easing = EasingFunction::EaseOut;
        }
        state.compute_tween();
        let tp = state.tween.as_ref().unwrap();
        assert!(tp.valid);
        let kf = tp.generated_frames[0].layer_keyframes[0].unwrap();
        // EaseOut at t=0.5: 1 - (0.5)^3 = 0.875. Interpolate X: 0 + (100-0) * 0.875 = 87.5 ≈ 88
        assert!(kf.position_offset.0 > 50);
        assert_eq!(kf.position_offset.0, 88);
    }

    #[test]
    fn test_tween_bounce_midpoint() {
        let mut state = make_tween_state();
        state.open_tween();
        {
            let t = state.tween.as_mut().unwrap();
            t.config.start_frame = 0;
            t.config.end_frame = 10;
            t.config.num_frames = 1;
            t.config.easing = EasingFunction::Bounce;
        }
        state.compute_tween();
        let tp = state.tween.as_ref().unwrap();
        assert!(tp.valid);
        let kf = tp.generated_frames[0].layer_keyframes[0].unwrap();
        // Bounce at t=0.5 produces > 0.5, so position X > 50
        assert!(kf.position_offset.0 > 50);
    }

    #[test]
    fn test_tween_opacity_blend() {
        let mut state = make_tween_state();
        state.open_tween();
        {
            let t = state.tween.as_mut().unwrap();
            t.config.start_frame = 0;
            t.config.end_frame = 10;
            t.config.num_frames = 3;
        }
        state.compute_tween();
        let tp = state.tween.as_ref().unwrap();
        assert!(tp.valid);

        // Frame 0: t = 1/4 = 0.25, opacity: 255 + (0 - 255) * 0.25 = 191
        let kf0 = tp.generated_frames[0].layer_keyframes[0].unwrap();
        assert_eq!(kf0.opacity, 191);
        assert_eq!(kf0.blend_mode, BlendMode::Normal); // t < 0.5

        // Frame 1: t = 2/4 = 0.5, opacity: 255 + (0 - 255) * 0.5 = 128
        let kf1 = tp.generated_frames[1].layer_keyframes[0].unwrap();
        assert_eq!(kf1.opacity, 128);
        // blend_mode at t=0.5: step => end (Multiply) because t >= 0.5
        assert_eq!(kf1.blend_mode, BlendMode::Multiply);

        // Frame 2: t = 3/4 = 0.75, opacity: 255 + (0 - 255) * 0.75 = 64
        let kf2 = tp.generated_frames[2].layer_keyframes[0].unwrap();
        assert_eq!(kf2.opacity, 64);
        assert_eq!(kf2.blend_mode, BlendMode::Multiply); // t > 0.5
    }

    #[test]
    fn test_tween_commit_inserts_frames() {
        let mut state = make_tween_state();
        assert_eq!(state.frames.len(), 11);
        state.open_tween();
        {
            let t = state.tween.as_mut().unwrap();
            t.config.start_frame = 0;
            t.config.end_frame = 10;
            t.config.num_frames = 3;
        }
        state.compute_tween();
        assert_eq!(state.tween.as_ref().unwrap().generated_frames.len(), 3);
        state.commit_tween();
        assert_eq!(state.frames.len(), 14);
        assert!(state.tween.is_none());
        // Check frames 1, 2, 3 are tween frames
        assert!(state.frames[1].label.starts_with("tween"));
        assert!(state.frames[2].label.starts_with("tween"));
        assert!(state.frames[3].label.starts_with("tween"));
    }

    #[test]
    fn test_tween_commit_advances_current() {
        let mut state = make_tween_state();
        state.current_frame = 0;
        state.open_tween();
        {
            let t = state.tween.as_mut().unwrap();
            t.config.start_frame = 0;
            t.config.end_frame = 10;
            t.config.num_frames = 3;
        }
        state.compute_tween();
        state.commit_tween();
        // Insert at 1, current_frame is 0 (before insert point), no shift
        assert_eq!(state.current_frame, 0);
        assert_eq!(state.frames.len(), 14);
        assert!(state.frames[1].label.starts_with("tween"));
    }

    #[test]
    fn test_tween_discard_clears() {
        let mut state = make_tween_state();
        assert_eq!(state.frames.len(), 11);
        state.open_tween();
        state.discard_tween();
        assert!(state.tween.is_none());
        assert_eq!(state.frames.len(), 11);
    }

    #[test]
    fn test_tween_skip_unkeyframed_layer() {
        let mut state = make_tween_state();
        // Layer 1 has no keyframes
        state.open_tween();
        {
            let t = state.tween.as_mut().unwrap();
            t.config.start_frame = 0;
            t.config.end_frame = 10;
            t.config.num_frames = 2;
        }
        state.compute_tween();
        let tp = state.tween.as_ref().unwrap();
        assert!(tp.valid);
        // Layer 0 should have keyframes
        assert!(tp.generated_frames[0].layer_keyframes[0].is_some());
        // Layer 1 should not have keyframes (not keyframed in either boundary)
        assert!(
            tp.generated_frames[0].layer_keyframes.get(1).is_none()
                || tp.generated_frames[0].layer_keyframes[1].is_none()
        );
    }

    #[test]
    fn test_tween_start_equals_end() {
        let mut state = make_tween_state();
        state.open_tween();
        {
            let t = state.tween.as_mut().unwrap();
            t.config.start_frame = 0;
            t.config.end_frame = 0;
        }
        state.compute_tween();
        let tp = state.tween.as_ref().unwrap();
        assert!(!tp.valid);
        assert!(tp.generated_frames.is_empty());
    }

    #[test]
    fn test_tween_handle_key_navigation() {
        let mut state = make_tween_state();
        state.open_tween();
        assert!(state.tween.is_some());
        assert_eq!(state.tween.as_ref().unwrap().field_index, 0);

        state.handle_tween_key(KeyCode::Down);
        assert_eq!(state.tween.as_ref().unwrap().field_index, 1);

        state.handle_tween_key(KeyCode::Down);
        assert_eq!(state.tween.as_ref().unwrap().field_index, 2);

        state.handle_tween_key(KeyCode::Down);
        assert_eq!(state.tween.as_ref().unwrap().field_index, 3);

        // Cant go below 3
        state.handle_tween_key(KeyCode::Down);
        assert_eq!(state.tween.as_ref().unwrap().field_index, 3);

        state.handle_tween_key(KeyCode::Up);
        assert_eq!(state.tween.as_ref().unwrap().field_index, 2);
    }

    #[test]
    fn test_tween_handle_key_easing_cycle() {
        let mut state = make_tween_state();
        state.open_tween();
        {
            let t = state.tween.as_mut().unwrap();
            t.field_index = 3;
        }
        assert_eq!(
            state.tween.as_ref().unwrap().config.easing,
            EasingFunction::Linear
        );

        state.handle_tween_key(KeyCode::Right);
        assert_eq!(
            state.tween.as_ref().unwrap().config.easing,
            EasingFunction::EaseIn
        );

        state.handle_tween_key(KeyCode::Left);
        assert_eq!(
            state.tween.as_ref().unwrap().config.easing,
            EasingFunction::Linear
        );
    }

    #[test]
    fn test_tween_handle_key_esc_discards() {
        let mut state = make_tween_state();
        state.open_tween();
        assert!(state.tween.is_some());
        state.handle_tween_key(KeyCode::Esc);
        assert!(state.tween.is_none());
    }
}
