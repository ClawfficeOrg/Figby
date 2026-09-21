use crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;
use std::time::Duration;

use crate::output::{
    export_cells_to_ansi, export_cells_to_ansi_multi, export_cells_to_apng, export_cells_to_gif,
    export_cells_to_png, export_cells_to_png_with_alpha, export_cells_to_txt, ExportError,
    ExportFormat,
};

use super::canvas::{CanvasBuffer, CanvasCell};
use super::components;
use super::layers::{blend_colors, blend_mode_color, LayerStack};
use super::lighting::{
    interpolate_scene, sort_light_keyframes, LightKeyframe, LightingLut, Scene, SwatchLightingData,
};
use super::theme::Theme;
use super::timeline::TimelineState;

use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportMode {
    Png,
    Apng,
    Gif,
    Txt,
    Ansi,
}

impl ExportMode {
    pub fn label(&self) -> &str {
        match self {
            ExportMode::Png => "PNG",
            ExportMode::Apng => "APNG",
            ExportMode::Gif => "GIF",
            ExportMode::Txt => "TXT",
            ExportMode::Ansi => "ANSI",
        }
    }

    pub fn cycle(&self) -> Self {
        match self {
            ExportMode::Png => ExportMode::Apng,
            ExportMode::Apng => ExportMode::Gif,
            ExportMode::Gif => ExportMode::Txt,
            ExportMode::Txt => ExportMode::Ansi,
            ExportMode::Ansi => ExportMode::Png,
        }
    }

    pub fn to_export_format(&self) -> ExportFormat {
        match self {
            ExportMode::Png => ExportFormat::Png,
            ExportMode::Apng => ExportFormat::Apng,
            ExportMode::Gif => ExportFormat::Gif,
            ExportMode::Txt => ExportFormat::Txt,
            ExportMode::Ansi => ExportFormat::Ansi,
        }
    }

    pub fn extension(&self) -> &str {
        match self {
            ExportMode::Png => ".png",
            ExportMode::Apng => ".apng",
            ExportMode::Gif => ".gif",
            ExportMode::Txt => ".txt",
            ExportMode::Ansi => ".ans",
        }
    }
}

pub struct ExportDialog {
    pub active: bool,
    pub format: ExportMode,
    pub path_buffer: String,
    pub font_size: u8,
    pub export_layers: bool,
    pub use_transparency: bool,
    pub error_message: String,
    pub selected_entry: usize,
    pub directory_entries: Vec<String>,
    pub theme: Theme,
    // GIF timeline export fields
    pub fps: u8,
    pub loop_count: u16,
    pub frame_delays: Vec<u16>,
    pub preview_frame: usize,
    pub preview_playing: bool,
    /// Elapsed time since the preview last advanced, accumulated by the
    /// event loop. Preview ticking is scheduler-driven (GPT review F-26):
    /// it advances on elapsed time, never from inside the render pass, so
    /// it cannot stall when redraws are suppressed.
    pub preview_accum: Duration,
    pub play_requested: bool,
    pub timeline_available: bool,
    pub timeline_frames: Vec<Vec<Vec<CanvasCell>>>,
}

impl ExportDialog {
    pub fn new() -> Self {
        Self {
            active: false,
            format: ExportMode::Png,
            path_buffer: String::new(),
            font_size: 2,
            export_layers: false,
            use_transparency: false,
            error_message: String::new(),
            selected_entry: 0,
            directory_entries: Vec::new(),
            theme: Theme::default(),
            fps: 12,
            loop_count: 0,
            frame_delays: Vec::new(),
            preview_frame: 0,
            preview_playing: false,
            preview_accum: Duration::ZERO,
            play_requested: false,
            timeline_available: false,
            timeline_frames: Vec::new(),
        }
    }

    pub fn enter_export(&mut self, mode: ExportMode) {
        self.active = true;
        self.format = mode;
        self.path_buffer = format!("export{}", mode.extension());
        self.error_message.clear();
        self.selected_entry = 0;
        // Deliberately does NOT clear_timeline() here: a GIF import may have
        // already populated real per-frame delays (see gif_import.rs) before
        // the dialog was ever opened. Clearing unconditionally on every open
        // silently threw that timing away in favor of a uniform FPS-derived
        // one. Stale state from a *previous* export session is reset by
        // close(), so there is nothing left over to clear here anyway.
        self.refresh_directory();
    }

    pub fn close(&mut self) {
        self.active = false;
        self.path_buffer.clear();
        self.directory_entries.clear();
        self.selected_entry = 0;
        self.error_message.clear();
        self.clear_timeline();
    }

    pub fn set_timeline(&mut self, fps: u8, frame_count: usize) {
        self.timeline_available = true;
        self.fps = fps;
        let delay = (100u16 / fps.max(1) as u16).max(1);
        self.frame_delays = vec![delay; frame_count];
        self.preview_frame = 0;
        self.preview_playing = false;
        self.preview_accum = Duration::ZERO;
    }

    /// Populate timeline export data from a `TimelineState`, using each
    /// frame's own `delay` (which travels with the frame through insert/
    /// delete/reorder — GPT review F-26) instead of a uniform FPS-derived
    /// value that would flatten imported GIF timing.
    pub fn set_timeline_from_timeline(&mut self, timeline: &TimelineState) {
        if timeline.frames.is_empty() {
            self.clear_timeline();
            return;
        }
        self.timeline_available = true;
        self.fps = timeline.fps;
        self.frame_delays = timeline.frame_delays();
        self.preview_frame = 0;
        self.preview_playing = false;
        self.preview_accum = Duration::ZERO;
    }

    pub fn clear_timeline(&mut self) {
        self.timeline_available = false;
        self.frame_delays.clear();
        self.timeline_frames.clear();
        self.preview_frame = 0;
        self.preview_playing = false;
        self.preview_accum = Duration::ZERO;
        self.play_requested = false;
    }

    pub fn set_per_frame_delays(&mut self, delays: Vec<u16>) {
        self.frame_delays = delays;
    }

    pub fn populate_from_timeline(
        &mut self,
        timeline: &TimelineState,
        layer_stack: &LayerStack,
        width: usize,
        height: usize,
    ) {
        let frames = capture_timeline_frames(timeline, layer_stack, width, height, None);
        if frames.is_empty() {
            self.clear_timeline();
            return;
        }
        self.timeline_frames = frames;
        self.timeline_available = true;
        self.fps = timeline.fps;
        self.frame_delays = timeline.frame_delays();
        self.preview_frame = 0;
        self.preview_playing = false;
        self.preview_accum = Duration::ZERO;
    }

    /// Advance the preview by elapsed `delta`, scheduler-driven (GPT review
    /// F-26): time accumulates and is consumed one frame's delay at a time,
    /// honoring per-frame delays. Returns whether the preview frame moved
    /// (so the caller can request a redraw). Never called from render code.
    pub fn preview_tick(&mut self, delta: Duration) -> bool {
        if !self.active
            || !self.preview_playing
            || !self.timeline_available
            || (self.format != ExportMode::Gif && self.format != ExportMode::Apng)
        {
            return false;
        }
        let count = self.frame_delays.len();
        if count == 0 {
            return false;
        }
        self.preview_accum += delta;
        let mut advanced = false;
        let mut guard = 0;
        while guard < count {
            let delay = self.frame_delays[self.preview_frame].max(1) as u64;
            let interval = Duration::from_millis(delay * 10);
            if self.preview_accum < interval {
                break;
            }
            self.preview_accum -= interval;
            self.preview_frame = (self.preview_frame + 1) % count;
            advanced = true;
            guard += 1;
        }
        advanced
    }

    fn refresh_directory(&mut self) {
        self.directory_entries.clear();
        self.selected_entry = 0;

        let parent = if self.path_buffer.is_empty() {
            std::path::PathBuf::from(".")
        } else {
            let p = std::path::PathBuf::from(&self.path_buffer);
            if p.is_dir() {
                p
            } else {
                p.parent()
                    .map(|pp| pp.to_path_buf())
                    .unwrap_or_else(|| std::path::PathBuf::from("."))
            }
        };

        let read_dir = match std::fs::read_dir(&parent) {
            Ok(rd) => rd,
            Err(_) => {
                self.error_message = format!("Cannot read directory: {}", parent.display());
                return;
            }
        };

        let mut entries: Vec<String> = Vec::new();
        for entry in read_dir.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if is_dir || name.ends_with(".flf") || name.ends_with(".tlf") {
                entries.push(name);
            }
        }

        entries.sort();
        self.directory_entries = entries;
    }

    const FPS_PRESETS: &'static [u8] = &[6, 8, 12, 24, 30, 60];
    const LOOP_PRESETS: &'static [u16] = &[0, 1, 2, 5, 10];

    pub fn handle_key(&mut self, code: KeyCode) -> bool {
        // Animation export keys (only when GIF/APNG mode + timeline available)
        if (self.format == ExportMode::Gif || self.format == ExportMode::Apng)
            && self.timeline_available
        {
            match code {
                KeyCode::Char('f') | KeyCode::Char('F') => {
                    let idx = Self::FPS_PRESETS
                        .iter()
                        .position(|&f| f == self.fps)
                        .unwrap_or(0);
                    self.fps = Self::FPS_PRESETS[(idx + 1) % Self::FPS_PRESETS.len()];
                    let delay = (100u16 / self.fps.max(1) as u16).max(1);
                    let count = self.frame_delays.len();
                    self.frame_delays = vec![delay; count];
                    self.preview_accum = Duration::ZERO;
                    return true;
                }
                KeyCode::Char('L') | KeyCode::Char('l') => {
                    let idx = Self::LOOP_PRESETS
                        .iter()
                        .position(|&l| l == self.loop_count)
                        .unwrap_or(0);
                    self.loop_count = Self::LOOP_PRESETS[(idx + 1) % Self::LOOP_PRESETS.len()];
                    return true;
                }
                KeyCode::Char('P') | KeyCode::Char('p') => {
                    self.play_requested = true;
                    return true;
                }
                KeyCode::Char('V') | KeyCode::Char('v') => {
                    self.preview_playing = !self.preview_playing;
                    self.preview_accum = Duration::ZERO;
                    return true;
                }
                KeyCode::Char(' ') => {
                    if !self.preview_playing {
                        let count = self.frame_delays.len();
                        if count > 0 {
                            self.preview_frame = (self.preview_frame + 1) % count;
                        }
                    }
                    return true;
                }
                _ => {}
            }
        }

        match code {
            KeyCode::Char('T') | KeyCode::Char('t') => {
                self.format = self.format.cycle();
                true
            }
            KeyCode::Char('L') | KeyCode::Char('l') if self.format != ExportMode::Ansi => {
                self.export_layers = !self.export_layers;
                true
            }
            KeyCode::Char('P') | KeyCode::Char('p') if self.format != ExportMode::Ansi => {
                self.use_transparency = !self.use_transparency;
                true
            }
            KeyCode::Char(c) if !c.is_control() => {
                self.path_buffer.push(c);
                self.error_message.clear();
                self.selected_entry = 0;
                self.refresh_directory();
                true
            }
            KeyCode::Backspace => {
                self.path_buffer.pop();
                self.error_message.clear();
                self.selected_entry = 0;
                self.refresh_directory();
                true
            }
            KeyCode::Up => {
                if !self.directory_entries.is_empty() && self.selected_entry > 0 {
                    self.selected_entry -= 1;
                }
                true
            }
            KeyCode::Down => {
                if !self.directory_entries.is_empty()
                    && self.selected_entry < self.directory_entries.len() - 1
                {
                    self.selected_entry += 1;
                }
                true
            }
            KeyCode::Tab => {
                if !self.directory_entries.is_empty() {
                    self.select_entry();
                }
                true
            }
            KeyCode::Enter => {
                self.active = false;
                true
            }
            KeyCode::Esc => {
                self.close();
                true
            }
            _ => false,
        }
    }

    fn select_entry(&mut self) {
        if self.selected_entry >= self.directory_entries.len() {
            return;
        }
        let entry = &self.directory_entries[self.selected_entry];
        let parent = if self.path_buffer.is_empty() {
            std::path::PathBuf::from(".")
        } else {
            let p = std::path::PathBuf::from(&self.path_buffer);
            if p.is_dir() {
                p
            } else {
                p.parent()
                    .map(|pp| pp.to_path_buf())
                    .unwrap_or_else(|| std::path::PathBuf::from("."))
            }
        };
        let abs = parent.join(entry);
        self.path_buffer = abs.to_string_lossy().to_string();
        self.selected_entry = 0;
        self.error_message.clear();
        self.refresh_directory();
    }

    pub fn perform_export(&mut self, cells: &[Vec<CanvasCell>]) -> Result<(), ExportError> {
        if self.path_buffer.is_empty() {
            return Err(ExportError::InvalidCells("no path specified".to_string()));
        }
        let path = std::path::PathBuf::from(&self.path_buffer);
        let bytes: Vec<u8> = match self.format {
            ExportMode::Png => export_cells_to_png(cells, self.font_size)?,
            ExportMode::Txt => export_cells_to_txt(cells).into_bytes(),
            ExportMode::Ansi => {
                if self.timeline_available && !self.timeline_frames.is_empty() {
                    export_cells_to_ansi_multi(&self.timeline_frames, &self.frame_delays)
                        .into_bytes()
                } else {
                    export_cells_to_ansi(cells).into_bytes()
                }
            }
            ExportMode::Apng | ExportMode::Gif => {
                let frame_slice: &[Vec<Vec<CanvasCell>>] =
                    if self.timeline_available && !self.timeline_frames.is_empty() {
                        self.timeline_frames.as_slice()
                    } else {
                        &[cells.to_vec()]
                    };
                let delay_slice: &[u16] =
                    if self.timeline_available && !self.frame_delays.is_empty() {
                        self.frame_delays.as_slice()
                    } else {
                        &[10]
                    };
                if self.format == ExportMode::Gif {
                    export_cells_to_gif(frame_slice, delay_slice, self.font_size, self.loop_count)?
                } else {
                    export_cells_to_apng(frame_slice, delay_slice, self.font_size, self.loop_count)?
                }
            }
        };
        crate::atomic_io::atomic_write(&path, &bytes)
            .map_err(|e| ExportError::IoError(e.to_string()))?;
        self.error_message.clear();
        Ok(())
    }

    fn sanitize_layer_name(name: &str) -> String {
        let sanitized: String = name
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
            .collect();
        if sanitized.is_empty() {
            "layer".to_string()
        } else {
            sanitized
        }
    }

    pub fn perform_layer_export(
        stack: &super::layers::LayerStack,
        base_path: &std::path::Path,
        font_size: u8,
        use_transparency: bool,
    ) -> Result<(), ExportError> {
        let ext = ".png";
        let mut used_names: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();

        for layer in stack.layers.iter() {
            if !layer.visible {
                continue;
            }

            let cells: Vec<Vec<super::canvas::CanvasCell>> = (0..layer.buffer.height())
                .map(|y| {
                    (0..layer.buffer.width())
                        .map(|x| layer.buffer.get(x, y).copied().unwrap_or_default())
                        .collect()
                })
                .collect();

            let stem = Self::sanitize_layer_name(&layer.name);
            let count = used_names.entry(stem.clone()).or_insert(0);
            let filename = if *count > 0 {
                format!("{}_{}{}", stem, count, ext)
            } else {
                format!("{}{}", stem, ext)
            };
            *count += 1;

            let path = base_path.with_file_name(&filename);
            let bytes = if use_transparency {
                export_cells_to_png_with_alpha(&cells, font_size, true)?
            } else {
                export_cells_to_png(&cells, font_size)?
            };
            crate::atomic_io::atomic_write(&path, &bytes)
                .map_err(|e| ExportError::IoError(e.to_string()))?;
        }

        Ok(())
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if !self.active {
            return;
        }

        frame.render_widget(Clear, area);
        let block = Block::default()
            .title(" Export ")
            .borders(Borders::ALL)
            .style(Style::default().fg(self.theme.dialog.border_success));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        if inner.width < 24 || inner.height < 8 {
            return;
        }

        let is_animation = self.format == ExportMode::Gif || self.format == ExportMode::Apng;
        let is_ansi = self.format == ExportMode::Ansi;

        let mut lines: Vec<Line> = Vec::new();

        lines.push(Line::from(Span::styled(
            format!(" Format: [{}]  (T to cycle)", self.format.label()),
            Style::default().add_modifier(Modifier::BOLD),
        )));

        if is_animation && self.timeline_available {
            lines.push(Line::from(Span::styled(
                format!(" FPS: [{}]  (F to cycle preset)", self.fps),
                Style::default().fg(self.theme.dialog.meta),
            )));
            let loop_label = if self.loop_count == 0 {
                "Infinite".to_string()
            } else {
                format!("{}x", self.loop_count)
            };
            lines.push(Line::from(Span::styled(
                format!(" Loop: [{}]  (L to cycle)", loop_label),
                Style::default().fg(self.theme.dialog.meta),
            )));
            lines.push(Line::from(Span::styled(
                format!(" Frames: [{}]", self.frame_delays.len()),
                Style::default().fg(self.theme.dialog.meta),
            )));
            let play_ch = if self.preview_playing {
                "\u{23F8}"
            } else {
                "\u{25B6}"
            };
            lines.push(Line::from(Span::styled(
                format!(
                    " Preview: {} (frame {}/{})  (V to toggle, Space to step)",
                    play_ch,
                    self.preview_frame + 1,
                    self.frame_delays.len().max(1)
                ),
                Style::default().fg(self.theme.dialog.meta),
            )));
            lines.push(Line::from(Span::styled(
                " P: Play Animation",
                Style::default().fg(self.theme.dialog.meta),
            )));
        } else if is_animation {
            lines.push(Line::from(Span::styled(
                " Timeline: No frames available",
                Style::default().fg(self.theme.dialog.error),
            )));
        }

        lines.push(Line::from(Span::styled(
            format!(
                " Size: {}x (z = char at {})",
                self.font_size,
                8 * self.font_size as u16 * 16 * self.font_size as u16
            ),
            Style::default().fg(self.theme.dialog.meta),
        )));

        if (!is_animation || !self.timeline_available) && !is_ansi {
            lines.push(Line::from(Span::styled(
                format!(
                    " Layers: [{}]  (L to toggle)",
                    if self.export_layers {
                        "Per-Layer"
                    } else {
                        "Single"
                    }
                ),
                Style::default().fg(self.theme.dialog.meta),
            )));

            lines.push(Line::from(Span::styled(
                format!(
                    " Alpha: [{}]  (P to toggle)",
                    if self.use_transparency {
                        "Transparent"
                    } else {
                        "Opaque"
                    }
                ),
                Style::default().fg(self.theme.dialog.meta),
            )));
        }

        lines.push(Line::from(Span::styled(
            " Path:",
            Style::default().add_modifier(Modifier::BOLD),
        )));

        let path_display = if self.path_buffer.is_empty() {
            " (type path or use arrows to browse)".to_string()
        } else {
            self.path_buffer.clone()
        };
        lines.push(Line::from(Span::styled(
            format!(" {}", path_display),
            Style::default().fg(self.theme.dialog.border_path),
        )));

        if !self.error_message.is_empty() {
            lines.push(Line::from(Span::styled(
                format!(" Error: {}", self.error_message),
                Style::default().fg(self.theme.dialog.error),
            )));
        }

        lines.push(Line::from(""));

        if self.directory_entries.is_empty() {
            lines.push(Line::from(Span::styled(
                " (empty directory)",
                Style::default().fg(self.theme.dialog.meta),
            )));
        } else {
            lines.push(Line::from(Span::styled(
                " Directory:",
                Style::default().add_modifier(Modifier::BOLD),
            )));

            let max_visible = (inner.height as usize).saturating_sub(9).min(10);
            let start = self.selected_entry.saturating_sub(max_visible / 2);
            let end = (start + max_visible).min(self.directory_entries.len());
            for i in start..end {
                let entry = &self.directory_entries[i];
                let is_selected = i == self.selected_entry;
                let parent = if self.path_buffer.is_empty() {
                    std::path::PathBuf::from(".")
                } else {
                    let p = std::path::PathBuf::from(&self.path_buffer);
                    if p.is_dir() {
                        p
                    } else {
                        p.parent()
                            .map(|pp| pp.to_path_buf())
                            .unwrap_or_else(|| std::path::PathBuf::from("."))
                    }
                };
                let is_dir = parent.join(entry).is_dir();
                let prefix = if is_selected { " >" } else { "  " };
                let suffix = if is_dir { "/" } else { "" };
                let text = format!("{prefix}{entry}{suffix}");
                let style = if is_selected {
                    Style::default().add_modifier(Modifier::REVERSED)
                } else {
                    Style::default()
                };
                lines.push(Line::from(Span::styled(text, style)));
            }
        }

        lines.push(Line::from(""));
        let gif_hint = if is_animation && self.timeline_available {
            " F:FPS  L:Loop  V:Preview  P:Play  Space:Step  "
        } else {
            ""
        };
        lines.push(Line::from(Span::styled(
            format!(
                " T:format  Enter:export  Esc:cancel  \u{2191}\u{2193}:navigate  Tab:select{}",
                gif_hint
            ),
            Style::default().fg(self.theme.dialog.meta),
        )));

        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, inner);
    }
}

impl Default for ExportDialog {
    fn default() -> Self {
        Self::new()
    }
}

/// Source index range for compositing a layer at a signed keyframe offset
/// into a canvas. A negative offset crops the leading edge of the source
/// (the layer is shifted left/up and its left/top columns fall off-canvas);
/// a positive offset crops the trailing edge. The old code clamped the
/// offset to zero with `.max(0)`, which flattened negative-position
/// keyframes onto the canvas edge instead of cropping them (GPT review
/// F-26). Returns `[start, end)` source indices that land in `[0, canvas)`.
fn signed_src_range(offset: i16, src_len: usize, canvas_len: usize) -> std::ops::Range<usize> {
    let start = (-offset).max(0) as usize;
    let end = if offset >= 0 {
        canvas_len.saturating_sub(offset as usize)
    } else {
        canvas_len.saturating_add((-offset) as usize)
    };
    let end = src_len.min(end).max(start);
    start..end
}

/// Optional lighting context for applying per-frame light keyframing during
/// timeline capture. Pass `None` to skip lighting (current behavior).
pub struct LightingContext<'a> {
    pub base_scene: &'a Scene,
    pub light_keyframes: &'a [LightKeyframe],
    pub lut: &'a LightingLut,
    pub layer_stack: &'a LayerStack,
    pub max_shadow_distance: u16,
    pub height_scale: f32,
    pub palette_rgb_to_swatch: &'a HashMap<(u8, u8, u8), usize>,
    pub swatch_data: &'a [SwatchLightingData],
}

pub fn capture_timeline_frames(
    timeline: &TimelineState,
    layer_stack: &LayerStack,
    width: usize,
    height: usize,
    lighting: Option<&LightingContext<'_>>,
) -> Vec<Vec<Vec<CanvasCell>>> {
    if timeline.frames.is_empty() {
        return Vec::new();
    }
    (0..timeline.frames.len())
        .map(|frame_idx| {
            // Single-authority rule (GPT review F-05): a frame's pixels
            // come from its committed `document_state` snapshot when one
            // exists; keyframes contribute transforms (offset/opacity/
            // blend) applied on top, per layer, in stack order. Frames
            // without a snapshot fall through to the per-layer
            // keyframe-interpolation path against the live layer stack.
            //
            // (An even earlier version of the production exporter
            // re-derived every frame from the *live* stack — unchanged
            // across frame_idx — so captured frames all came out
            // identical.)
            if !timeline.frames[frame_idx].document_state.is_empty() {
                let states = &timeline.frames[frame_idx].document_state;
                let mut result_buf = CanvasBuffer::new(width, height);
                for (layer_idx, buf) in states.iter().enumerate() {
                    let props = timeline.get_interpolated_properties(frame_idx, layer_idx);
                    if props.opacity == 0 {
                        continue;
                    }
                    let (ox, oy) = props.position_offset;
                    let x_range = signed_src_range(ox, buf.width(), width);
                    let y_range = signed_src_range(oy, buf.height(), height);
                    for y in y_range.clone() {
                        for x in x_range.clone() {
                            let bx = (x as i16 + ox) as usize;
                            let by = (y as i16 + oy) as usize;
                            if bx >= width || by >= height {
                                continue;
                            }
                            if let Some(top) = buf.get(x, y) {
                                if top.ch == ' ' && top.fg.is_none() && top.bg.is_none() {
                                    continue;
                                }
                                let bottom = result_buf.get(bx, by).copied().unwrap_or_default();
                                let blended_fg =
                                    blend_mode_color(top.fg, bottom.fg, props.blend_mode);
                                let blended_bg =
                                    blend_mode_color(top.bg, bottom.bg, props.blend_mode);
                                let final_fg = blend_colors(blended_fg, bottom.fg, props.opacity);
                                let final_bg = blend_colors(blended_bg, bottom.bg, props.opacity);
                                result_buf.set(
                                    bx,
                                    by,
                                    CanvasCell {
                                        ch: top.ch,
                                        fg: final_fg,
                                        bg: final_bg,
                                        height: None,
                                    },
                                );
                            }
                        }
                    }
                }
                // Apply lighting to the composited result if a context was provided
                if let Some(lc) = lighting {
                    let mut sorted_kfs = lc.light_keyframes.to_vec();
                    sort_light_keyframes(&mut sorted_kfs);
                    let total = timeline.frames.len().max(1) as f32;
                    let t = frame_idx as f32 / total;
                    let interpolated = interpolate_scene(lc.base_scene, &sorted_kfs, t);
                    result_buf = components::canvas::shade_composited(
                        &result_buf,
                        lc.layer_stack,
                        &interpolated,
                        lc.lut,
                        lc.max_shadow_distance,
                        lc.height_scale,
                        lc.palette_rgb_to_swatch,
                        lc.swatch_data,
                    );
                }
                return (0..height)
                    .map(|y| {
                        (0..width)
                            .map(|x| result_buf.get(x, y).copied().unwrap_or_default())
                            .collect()
                    })
                    .collect();
            }

            let mut result_buf = CanvasBuffer::new(width, height);
            for (layer_idx, layer) in layer_stack.layers.iter().enumerate() {
                if !layer.visible {
                    continue;
                }
                let props = timeline.get_interpolated_properties(frame_idx, layer_idx);
                if props.opacity == 0 {
                    continue;
                }
                let (ox, oy) = props.position_offset;
                let x_range = signed_src_range(ox, layer.buffer.width(), width);
                let y_range = signed_src_range(oy, layer.buffer.height(), height);
                for y in y_range.clone() {
                    for x in x_range.clone() {
                        let bx = (x as i16 + ox) as usize;
                        let by = (y as i16 + oy) as usize;
                        if bx >= width || by >= height {
                            continue;
                        }
                        if let Some(top) = layer.buffer.get(x, y) {
                            if top.ch == ' ' && top.fg.is_none() && top.bg.is_none() {
                                continue;
                            }
                            let bottom = result_buf.get(bx, by).copied().unwrap_or_default();
                            let blended_fg = blend_mode_color(top.fg, bottom.fg, props.blend_mode);
                            let blended_bg = blend_mode_color(top.bg, bottom.bg, props.blend_mode);
                            let final_fg = blend_colors(blended_fg, bottom.fg, props.opacity);
                            let final_bg = blend_colors(blended_bg, bottom.bg, props.opacity);
                            result_buf.set(
                                bx,
                                by,
                                CanvasCell {
                                    ch: top.ch,
                                    fg: final_fg,
                                    bg: final_bg,
                                    height: None,
                                },
                            );
                        }
                    }
                }
            }
            // Apply lighting to the composited result if a context was provided
            if let Some(lc) = lighting {
                let mut sorted_kfs = lc.light_keyframes.to_vec();
                sort_light_keyframes(&mut sorted_kfs);
                let total = timeline.frames.len().max(1) as f32;
                let t = frame_idx as f32 / total;
                let interpolated = interpolate_scene(lc.base_scene, &sorted_kfs, t);
                result_buf = components::canvas::shade_composited(
                    &result_buf,
                    lc.layer_stack,
                    &interpolated,
                    lc.lut,
                    lc.max_shadow_distance,
                    lc.height_scale,
                    lc.palette_rgb_to_swatch,
                    lc.swatch_data,
                );
            }
            (0..result_buf.height())
                .map(|y| {
                    (0..result_buf.width())
                        .map(|x| result_buf.get(x, y).copied().unwrap_or_default())
                        .collect()
                })
                .collect()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::layers::{BlendMode, Layer, LayerStack};
    use super::super::timeline::{LayerKeyframe, TimelineFrame, TimelineState};
    use super::*;
    use ratatui::style::Color;

    #[test]
    fn test_export_dialog_new() {
        let dialog = ExportDialog::new();
        assert!(!dialog.active);
        assert_eq!(dialog.format, ExportMode::Png);
        assert!(dialog.path_buffer.is_empty());
        assert_eq!(dialog.font_size, 2);
    }

    #[test]
    fn test_export_dialog_open_close() {
        let mut dialog = ExportDialog::new();
        dialog.enter_export(ExportMode::Png);
        assert!(dialog.active);
        dialog.close();
        assert!(!dialog.active);
    }

    #[test]
    fn test_export_dialog_format_toggle() {
        let mut dialog = ExportDialog::new();
        dialog.enter_export(ExportMode::Png);
        dialog.handle_key(KeyCode::Char('T'));
        assert_eq!(dialog.format, ExportMode::Apng);
        dialog.handle_key(KeyCode::Char('t'));
        assert_eq!(dialog.format, ExportMode::Gif);
        dialog.handle_key(KeyCode::Char('t'));
        assert_eq!(dialog.format, ExportMode::Txt);
        dialog.handle_key(KeyCode::Char('t'));
        assert_eq!(dialog.format, ExportMode::Ansi);
        dialog.handle_key(KeyCode::Char('t'));
        assert_eq!(dialog.format, ExportMode::Png);
    }

    #[test]
    fn test_export_dialog_path_entry() {
        let mut dialog = ExportDialog::new();
        dialog.enter_export(ExportMode::Png);
        dialog.handle_key(KeyCode::Char('m'));
        dialog.handle_key(KeyCode::Char('y'));
        dialog.handle_key(KeyCode::Char('f'));
        assert_eq!(dialog.path_buffer, "export.pngmyf");
    }

    #[test]
    fn test_export_dialog_enter_closes() {
        let mut dialog = ExportDialog::new();
        dialog.enter_export(ExportMode::Png);
        assert!(dialog.active);
        dialog.handle_key(KeyCode::Enter);
        assert!(!dialog.active);
    }

    #[test]
    fn test_export_dialog_esc_closes() {
        let mut dialog = ExportDialog::new();
        dialog.enter_export(ExportMode::Png);
        assert!(dialog.active);
        dialog.handle_key(KeyCode::Esc);
        assert!(!dialog.active);
    }

    #[test]
    fn test_export_dialog_backspace() {
        let mut dialog = ExportDialog::new();
        dialog.enter_export(ExportMode::Png);
        dialog.handle_key(KeyCode::Char('a'));
        dialog.handle_key(KeyCode::Char('b'));
        dialog.handle_key(KeyCode::Backspace);
        assert_eq!(dialog.path_buffer, "export.pnga");
    }

    #[test]
    fn test_export_mode_labels() {
        assert_eq!(ExportMode::Png.label(), "PNG");
        assert_eq!(ExportMode::Apng.label(), "APNG");
        assert_eq!(ExportMode::Gif.label(), "GIF");
        assert_eq!(ExportMode::Txt.label(), "TXT");
        assert_eq!(ExportMode::Ansi.label(), "ANSI");
    }

    #[test]
    fn test_export_mode_extensions() {
        assert_eq!(ExportMode::Png.extension(), ".png");
        assert_eq!(ExportMode::Apng.extension(), ".apng");
        assert_eq!(ExportMode::Gif.extension(), ".gif");
        assert_eq!(ExportMode::Txt.extension(), ".txt");
        assert_eq!(ExportMode::Ansi.extension(), ".ans");
    }

    #[test]
    fn test_export_dialog_toggle_layers() {
        let mut dialog = ExportDialog::new();
        dialog.enter_export(ExportMode::Png);
        assert!(!dialog.export_layers);
        dialog.handle_key(KeyCode::Char('L'));
        assert!(dialog.export_layers);
        dialog.handle_key(KeyCode::Char('l'));
        assert!(!dialog.export_layers);
    }

    #[test]
    fn test_export_dialog_toggle_transparency() {
        let mut dialog = ExportDialog::new();
        dialog.enter_export(ExportMode::Png);
        assert!(!dialog.use_transparency);
        dialog.handle_key(KeyCode::Char('P'));
        assert!(dialog.use_transparency);
        dialog.handle_key(KeyCode::Char('p'));
        assert!(!dialog.use_transparency);
    }

    #[test]
    fn test_sanitize_layer_name_alphanumeric() {
        assert_eq!(ExportDialog::sanitize_layer_name("Layer1"), "Layer1");
    }

    #[test]
    fn test_sanitize_layer_name_strips_special_chars() {
        assert_eq!(
            ExportDialog::sanitize_layer_name("hello/world:test"),
            "helloworldtest"
        );
    }

    #[test]
    fn test_sanitize_layer_name_underscore_and_hyphen() {
        assert_eq!(
            ExportDialog::sanitize_layer_name("my_layer-v2"),
            "my_layer-v2"
        );
    }

    #[test]
    fn test_sanitize_layer_name_empty_fallback() {
        assert_eq!(ExportDialog::sanitize_layer_name("!@#$%"), "layer");
    }

    #[test]
    fn test_export_gif_fps_cycle() {
        let mut dialog = ExportDialog::new();
        dialog.enter_export(ExportMode::Gif);
        dialog.set_timeline(12, 10);
        dialog.handle_key(KeyCode::Char('F'));
        assert_eq!(dialog.fps, 24);
        dialog.handle_key(KeyCode::Char('f'));
        assert_eq!(dialog.fps, 30);
    }

    #[test]
    fn test_export_gif_loop_cycle() {
        let mut dialog = ExportDialog::new();
        dialog.enter_export(ExportMode::Gif);
        dialog.set_timeline(12, 10);
        dialog.handle_key(KeyCode::Char('L'));
        assert_eq!(dialog.loop_count, 1);
        dialog.handle_key(KeyCode::Char('l'));
        assert_eq!(dialog.loop_count, 2);
    }

    #[test]
    fn test_export_gif_preview_toggle() {
        let mut dialog = ExportDialog::new();
        dialog.enter_export(ExportMode::Gif);
        dialog.set_timeline(12, 5);
        assert!(!dialog.preview_playing);
        dialog.handle_key(KeyCode::Char('V'));
        assert!(dialog.preview_playing);
        dialog.handle_key(KeyCode::Char('v'));
        assert!(!dialog.preview_playing);
    }

    #[test]
    fn test_export_gif_frame_delays_from_fps() {
        let mut dialog = ExportDialog::new();
        dialog.enter_export(ExportMode::Gif);
        dialog.set_timeline(12, 10);
        assert_eq!(dialog.frame_delays.len(), 10);
        // FPS 12 = 100/12 = 8 cs per frame
        assert!(dialog.frame_delays.iter().all(|&d| d == 8));
        dialog.handle_key(KeyCode::Char('F'));
        // Now FPS 24 = 100/24 = 4 cs per frame
        assert!(dialog.frame_delays.iter().all(|&d| d == 4));
    }

    #[test]
    fn test_export_gif_set_timeline() {
        let mut dialog = ExportDialog::new();
        dialog.enter_export(ExportMode::Png);
        assert!(!dialog.timeline_available);
        dialog.format = ExportMode::Gif;
        dialog.set_timeline(24, 15);
        assert!(dialog.timeline_available);
        assert_eq!(dialog.fps, 24);
        assert_eq!(dialog.frame_delays.len(), 15);
        assert_eq!(dialog.frame_delays[0], 100 / 24);
    }

    #[test]
    fn test_enter_export_preserves_imported_frame_delays() {
        // Simulates a GIF import: real per-frame delays are set directly on
        // the dialog (mimicking tui/mod.rs's `export_dialog.frame_delays =
        // gif_data.frame_delays`) *before* the export dialog is ever opened.
        let mut dialog = ExportDialog::new();
        dialog.set_per_frame_delays(vec![5, 20, 5, 50]);
        dialog.timeline_available = true;

        // Opening the export dialog must not discard that real timing.
        dialog.enter_export(ExportMode::Gif);
        assert!(dialog.timeline_available);
        assert_eq!(dialog.frame_delays, vec![5, 20, 5, 50]);
    }

    #[test]
    fn test_export_gif_keys_only_in_gif_mode() {
        let mut dialog = ExportDialog::new();
        dialog.enter_export(ExportMode::Png);
        dialog.set_timeline(12, 5);
        // In PNG mode, GIF keys should not work
        dialog.handle_key(KeyCode::Char('F'));
        assert_eq!(dialog.fps, 12); // unchanged
                                    // In GIF mode with timeline, they should work
        dialog.format = ExportMode::Gif;
        dialog.handle_key(KeyCode::Char('F'));
        assert_eq!(dialog.fps, 24);
    }

    #[test]
    fn test_export_gif_preview_tick() {
        let mut dialog = ExportDialog::new();
        dialog.enter_export(ExportMode::Gif);
        dialog.set_timeline(12, 5); // uniform 8cs → 80ms per frame
        dialog.preview_playing = true;
        assert!(dialog.preview_tick(Duration::from_millis(80)));
        assert_eq!(dialog.preview_frame, 1);
        assert!(dialog.preview_tick(Duration::from_millis(80)));
        assert_eq!(dialog.preview_frame, 2);
        // Cycle around
        dialog.preview_frame = 4;
        assert!(dialog.preview_tick(Duration::from_millis(80)));
        assert_eq!(dialog.preview_frame, 0);
    }

    #[test]
    fn test_export_gif_preview_tick_respects_per_frame_delays() {
        // Variable timing: frame 0 holds 50ms, frame 1 holds 200ms.
        let mut dialog = ExportDialog::new();
        dialog.enter_export(ExportMode::Gif);
        dialog.set_per_frame_delays(vec![5, 20, 5]);
        dialog.timeline_available = true;
        dialog.preview_playing = true;
        // Exactly frame 0's 50ms → advance to frame 1.
        assert!(dialog.preview_tick(Duration::from_millis(50)));
        assert_eq!(dialog.preview_frame, 1);
        // 199ms is just short of frame 1's 200ms → no advance.
        assert!(!dialog.preview_tick(Duration::from_millis(199)));
        assert_eq!(dialog.preview_frame, 1);
        // 1ms crosses 200ms → advance to frame 2.
        assert!(dialog.preview_tick(Duration::from_millis(1)));
        assert_eq!(dialog.preview_frame, 2);
    }

    #[test]
    fn test_export_gif_preview_tick_only_when_playing() {
        let mut dialog = ExportDialog::new();
        dialog.enter_export(ExportMode::Gif);
        dialog.set_timeline(12, 5);
        dialog.preview_playing = false;
        assert!(!dialog.preview_tick(Duration::from_millis(500)));
        assert_eq!(dialog.preview_frame, 0);
    }

    #[test]
    fn test_export_gif_space_step_when_paused() {
        let mut dialog = ExportDialog::new();
        dialog.enter_export(ExportMode::Gif);
        dialog.set_timeline(12, 5);
        dialog.preview_playing = false;
        dialog.handle_key(KeyCode::Char(' '));
        assert_eq!(dialog.preview_frame, 1);
        dialog.handle_key(KeyCode::Char(' '));
        assert_eq!(dialog.preview_frame, 2);
    }

    // ── play_requested tests ──────────────────────────────────────────

    #[test]
    fn test_export_play_button_in_gif_mode() {
        let mut dialog = ExportDialog::new();
        dialog.enter_export(ExportMode::Gif);
        dialog.set_timeline(12, 5);
        assert!(!dialog.play_requested);
        dialog.handle_key(KeyCode::Char('P'));
        assert!(dialog.play_requested);
        // Press again, should still be true (set each time)
        dialog.play_requested = false;
        dialog.handle_key(KeyCode::Char('p'));
        assert!(dialog.play_requested);
    }

    #[test]
    fn test_export_play_button_not_in_png_mode() {
        let mut dialog = ExportDialog::new();
        dialog.enter_export(ExportMode::Png);
        dialog.set_timeline(12, 5);
        assert!(!dialog.play_requested);
        dialog.handle_key(KeyCode::Char('P'));
        assert!(!dialog.play_requested);
    }

    #[test]
    fn test_export_play_button_resets_on_close() {
        let mut dialog = ExportDialog::new();
        dialog.enter_export(ExportMode::Gif);
        dialog.set_timeline(12, 5);
        dialog.handle_key(KeyCode::Char('P'));
        assert!(dialog.play_requested);
        dialog.close();
        assert!(!dialog.play_requested);
    }

    // ── capture_timeline_frames tests ──────────────────────────────────

    #[test]
    fn test_capture_empty_timeline() {
        let timeline = TimelineState::default();
        let stack = LayerStack::new(5, 5);
        let frames = capture_timeline_frames(&timeline, &stack, 5, 5, None);
        assert!(frames.is_empty());
    }

    #[test]
    fn test_capture_uses_per_frame_layer_state_when_present() {
        // GIF import / 'A'-key manual capture stamps each TimelineFrame
        // with its own fully-composited `layer_state` snapshot. Without
        // reading it, capture_timeline_frames would instead re-derive
        // every frame from the *live*, unchanging layer stack — making
        // every captured frame identical regardless of frame_idx, which
        // is exactly the "counter advances, picture never changes" bug
        // this test guards against.
        let stack = LayerStack::new(2, 2); // live layer stays blank throughout

        let mut buf0 = CanvasBuffer::new(2, 2);
        buf0.set(
            0,
            0,
            CanvasCell {
                ch: '0',
                fg: Some(Color::Red),
                bg: None,
                height: None,
            },
        );
        let mut buf1 = CanvasBuffer::new(2, 2);
        buf1.set(
            0,
            0,
            CanvasCell {
                ch: '1',
                fg: Some(Color::Green),
                bg: None,
                height: None,
            },
        );

        let mut timeline = TimelineState::default();
        timeline.add_frame(TimelineFrame {
            thumbnail: vec![],
            has_keyframe: true,
            label: "F0".into(),
            delay: 10,
            document_state: vec![buf0],
            layer_keyframes: vec![Some(LayerKeyframe::default())],
        });
        timeline.add_frame(TimelineFrame {
            thumbnail: vec![],
            has_keyframe: true,
            label: "F1".into(),
            delay: 10,
            document_state: vec![buf1],
            layer_keyframes: vec![Some(LayerKeyframe::default())],
        });

        let frames = capture_timeline_frames(&timeline, &stack, 2, 2, None);
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0][0][0].ch, '0');
        assert_eq!(frames[0][0][0].fg, Some(Color::Red));
        assert_eq!(frames[1][0][0].ch, '1');
        assert_eq!(frames[1][0][0].fg, Some(Color::Green));
        assert_ne!(
            frames[0], frames[1],
            "each frame's own layer_state snapshot must be used, not the live (unchanging) layer stack"
        );
    }

    #[test]
    fn test_capture_single_layer_single_frame() {
        let mut stack = LayerStack::new(3, 3);
        stack.layers[0].buffer.set(
            0,
            0,
            CanvasCell {
                ch: 'A',
                fg: Some(Color::Red),
                bg: None,
                height: None,
            },
        );
        let mut timeline = TimelineState::default();
        timeline.add_frame(TimelineFrame {
            thumbnail: vec![],
            has_keyframe: true,
            label: "F0".into(),
            delay: 10,
            document_state: Vec::new(),
            layer_keyframes: vec![Some(LayerKeyframe::default())],
        });
        let frames = capture_timeline_frames(&timeline, &stack, 3, 3, None);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0][0][0].ch, 'A');
        assert_eq!(frames[0][0][0].fg, Some(Color::Red));
    }

    #[test]
    fn test_capture_two_layers_composite() {
        let mut stack = LayerStack::new(3, 3);
        stack.layers[0].buffer.set(
            0,
            0,
            CanvasCell {
                ch: 'A',
                fg: Some(Color::Red),
                bg: None,
                height: None,
            },
        );
        stack.layers.push(Layer::new(3, 3, "Layer 1".into()));
        stack.layers[1].buffer.set(
            1,
            0,
            CanvasCell {
                ch: 'B',
                fg: Some(Color::Green),
                bg: None,
                height: None,
            },
        );
        let mut timeline = TimelineState::default();
        timeline.add_frame(TimelineFrame {
            thumbnail: vec![],
            has_keyframe: true,
            label: "F0".into(),
            delay: 10,
            document_state: Vec::new(),
            layer_keyframes: vec![
                Some(LayerKeyframe::default()),
                Some(LayerKeyframe::default()),
            ],
        });
        let frames = capture_timeline_frames(&timeline, &stack, 3, 3, None);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0][0][0].ch, 'A');
        assert_eq!(frames[0][0][1].ch, 'B');
    }

    #[test]
    fn test_capture_keyframe_position_offset() {
        let mut stack = LayerStack::new(5, 3);
        stack.layers[0].buffer.set(
            0,
            0,
            CanvasCell {
                ch: 'X',
                fg: Some(Color::Blue),
                bg: None,
                height: None,
            },
        );
        let mut timeline = TimelineState::default();
        timeline.add_frame(TimelineFrame {
            thumbnail: vec![],
            has_keyframe: true,
            label: "F0".into(),
            delay: 10,
            document_state: Vec::new(),
            layer_keyframes: vec![Some(LayerKeyframe::default())],
        });
        timeline.add_frame(TimelineFrame {
            thumbnail: vec![],
            has_keyframe: true,
            label: "F1".into(),
            delay: 10,
            document_state: Vec::new(),
            layer_keyframes: vec![Some(LayerKeyframe {
                position_offset: (1, 0),
                ..Default::default()
            })],
        });
        let frames = capture_timeline_frames(&timeline, &stack, 5, 3, None);
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0][0][0].ch, 'X');
        assert_eq!(frames[1][0][0].ch, ' ');
        assert_eq!(frames[1][0][1].ch, 'X');
    }

    #[test]
    fn test_capture_negative_keyframe_offset_crops_leading_edge() {
        // A negative offset shifts the layer left/up; source columns/rows
        // that fall off-canvas must be cropped, not clamped to zero (F-26).
        let mut stack = LayerStack::new(4, 3);
        for (x, ch) in [(0usize, 'a'), (1, 'b'), (2, 'c'), (3, 'd')] {
            stack.layers[0].buffer.set(
                x,
                0,
                CanvasCell {
                    ch,
                    fg: Some(Color::Red),
                    bg: None,
                    height: None,
                },
            );
        }
        let mut timeline = TimelineState::default();
        timeline.add_frame(TimelineFrame {
            thumbnail: vec![],
            has_keyframe: true,
            label: "F0".into(),
            delay: 10,
            document_state: Vec::new(),
            layer_keyframes: vec![Some(LayerKeyframe {
                position_offset: (-2, 0),
                ..Default::default()
            })],
        });
        let frames = capture_timeline_frames(&timeline, &stack, 4, 3, None);
        assert_eq!(frames.len(), 1);
        // 'a' and 'b' crop off-canvas; 'c' and 'd' land at x=0,1.
        let row = &frames[0][0];
        let chars: Vec<char> = row.iter().map(|c| c.ch).collect();
        assert_eq!(chars, vec!['c', 'd', ' ', ' ']);
    }

    #[test]
    fn test_capture_negative_vertical_offset_crops_top() {
        let mut stack = LayerStack::new(2, 3);
        stack.layers[0].buffer.set(
            0,
            0,
            CanvasCell {
                ch: 'T',
                fg: Some(Color::Red),
                bg: None,
                height: None,
            },
        );
        stack.layers[0].buffer.set(
            0,
            2,
            CanvasCell {
                ch: 'B',
                fg: Some(Color::Red),
                bg: None,
                height: None,
            },
        );
        let mut timeline = TimelineState::default();
        timeline.add_frame(TimelineFrame {
            thumbnail: vec![],
            has_keyframe: true,
            label: "F0".into(),
            delay: 10,
            document_state: Vec::new(),
            layer_keyframes: vec![Some(LayerKeyframe {
                position_offset: (0, -2),
                ..Default::default()
            })],
        });
        let frames = capture_timeline_frames(&timeline, &stack, 2, 3, None);
        // y=0 (top row) crops off-canvas; y=2 content lands at row 0.
        assert_eq!(frames[0][0][0].ch, 'B');
        assert_eq!(frames[0][1][0].ch, ' ');
        assert_eq!(frames[0][2][0].ch, ' ');
    }

    #[test]
    fn test_capture_keyframe_opacity() {
        let mut stack = LayerStack::new(3, 3);
        stack.layers[0].buffer.set(
            0,
            0,
            CanvasCell {
                ch: 'Z',
                fg: Some(Color::Cyan),
                bg: None,
                height: None,
            },
        );
        let mut timeline = TimelineState::default();
        timeline.add_frame(TimelineFrame {
            thumbnail: vec![],
            has_keyframe: true,
            label: "F0".into(),
            delay: 10,
            document_state: Vec::new(),
            layer_keyframes: vec![Some(LayerKeyframe::default())],
        });
        timeline.add_frame(TimelineFrame {
            thumbnail: vec![],
            has_keyframe: true,
            label: "F1".into(),
            delay: 10,
            document_state: Vec::new(),
            layer_keyframes: vec![Some(LayerKeyframe {
                opacity: 0,
                ..Default::default()
            })],
        });
        let frames = capture_timeline_frames(&timeline, &stack, 3, 3, None);
        assert_eq!(frames.len(), 2);
        let cell = &frames[1][0][0];
        assert_eq!(cell.ch, ' ');
        assert_eq!(cell.fg, None);
    }

    #[test]
    fn test_capture_keyframe_blend_mode() {
        let mut stack = LayerStack::new(1, 1);
        stack.layers[0].buffer.set(
            0,
            0,
            CanvasCell {
                ch: ' ',
                fg: Some(Color::Rgb(200, 100, 50)),
                bg: None,
                height: None,
            },
        );
        stack.layers.push(Layer::new(1, 1, "Layer 1".into()));
        stack.layers[1].buffer.set(
            0,
            0,
            CanvasCell {
                ch: 'X',
                fg: Some(Color::Rgb(100, 200, 50)),
                bg: None,
                height: None,
            },
        );
        let mut timeline = TimelineState::default();
        timeline.add_frame(TimelineFrame {
            thumbnail: vec![],
            has_keyframe: true,
            label: "F0".into(),
            delay: 10,
            document_state: Vec::new(),
            layer_keyframes: vec![
                Some(LayerKeyframe::default()),
                Some(LayerKeyframe {
                    blend_mode: BlendMode::Multiply,
                    ..Default::default()
                }),
            ],
        });
        let frames = capture_timeline_frames(&timeline, &stack, 1, 1, None);
        // Multiply(100,200,50) × (200,100,50) = (78,78,9)
        let blended = frames[0][0][0].fg;
        assert_eq!(blended, Some(Color::Rgb(78, 78, 9)));
    }

    #[test]
    fn test_capture_populate_dialog() {
        let mut dialog = ExportDialog::new();
        dialog.fps = 12;
        let mut stack = LayerStack::new(3, 3);
        stack.layers[0].buffer.set(
            0,
            0,
            CanvasCell {
                ch: 'A',
                fg: Some(Color::Red),
                bg: None,
                height: None,
            },
        );
        let mut timeline = TimelineState::default();
        timeline.add_frame(TimelineFrame {
            thumbnail: vec![],
            has_keyframe: true,
            label: "F0".into(),
            delay: 10,
            document_state: Vec::new(),
            layer_keyframes: vec![Some(LayerKeyframe::default())],
        });
        assert!(!dialog.timeline_available);
        dialog.populate_from_timeline(&timeline, &stack, 3, 3);
        assert!(dialog.timeline_available);
        assert_eq!(dialog.timeline_frames.len(), 1);
        assert_eq!(dialog.timeline_frames[0][0][0].ch, 'A');
        assert_eq!(
            dialog.frame_delays,
            vec![10],
            "delay taken from the frame itself"
        );
    }

    #[test]
    fn test_capture_populate_dialog_empty_timeline() {
        let mut dialog = ExportDialog::new();
        let timeline = TimelineState::default();
        let stack = LayerStack::new(3, 3);
        dialog.timeline_available = true;
        dialog.populate_from_timeline(&timeline, &stack, 3, 3);
        assert!(!dialog.timeline_available);
        assert!(dialog.timeline_frames.is_empty());
    }

    #[test]
    fn test_export_ansi_no_layers_toggle() {
        let mut dialog = ExportDialog::new();
        dialog.enter_export(ExportMode::Ansi);
        assert!(!dialog.export_layers);
        // L key should not toggle layers in ANSI mode
        dialog.handle_key(KeyCode::Char('L'));
        assert!(!dialog.export_layers);
        dialog.handle_key(KeyCode::Char('l'));
        assert!(!dialog.export_layers);
    }

    #[test]
    fn test_export_ansi_no_transparency_toggle() {
        let mut dialog = ExportDialog::new();
        dialog.enter_export(ExportMode::Ansi);
        assert!(!dialog.use_transparency);
        // P key should not toggle transparency in ANSI mode
        dialog.handle_key(KeyCode::Char('P'));
        assert!(!dialog.use_transparency);
        dialog.handle_key(KeyCode::Char('p'));
        assert!(!dialog.use_transparency);
    }

    #[test]
    fn test_export_ansi_enter_export_path() {
        let mut dialog = ExportDialog::new();
        dialog.enter_export(ExportMode::Ansi);
        assert_eq!(dialog.path_buffer, "export.ans");
    }

    #[test]
    fn test_export_ansi_perform_single() {
        let tmpdir = std::env::temp_dir();
        let path = tmpdir.join("test_export_ansi_perform_single.ans");
        let mut dialog = ExportDialog::new();
        dialog.enter_export(ExportMode::Ansi);
        dialog.path_buffer = path.to_string_lossy().to_string();
        let cells = vec![vec![CanvasCell {
            ch: 'A',
            fg: Some(Color::Red),
            bg: None,
            height: None,
        }]];
        let result = dialog.perform_export(&cells);
        assert!(result.is_ok());
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(content.contains('A'));
        assert!(content.contains("\x1b[38;2;255;0;0m"));
        assert!(content.contains("\x1b[0m"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_export_ansi_mode_gated_layers_png() {
        // Verify that L and P still work for PNG mode
        let mut dialog = ExportDialog::new();
        dialog.enter_export(ExportMode::Png);
        assert!(!dialog.export_layers);
        dialog.handle_key(KeyCode::Char('L'));
        assert!(dialog.export_layers);
    }

    fn make_single_cell(
        rows: usize,
        cols: usize,
        ch: char,
        fg: Option<Color>,
        bg: Option<Color>,
    ) -> Vec<Vec<CanvasCell>> {
        (0..rows)
            .map(|_| {
                (0..cols)
                    .map(|_| CanvasCell {
                        ch,
                        fg,
                        bg,
                        height: None,
                    })
                    .collect()
            })
            .collect()
    }

    #[test]
    fn test_perform_export_gif_5_frames() {
        let tmpdir = std::env::temp_dir();
        let path = tmpdir.join("test_perform_export_gif_5_frames.gif");
        let mut dialog = ExportDialog::new();
        dialog.enter_export(ExportMode::Gif);
        dialog.path_buffer = path.to_string_lossy().to_string();
        let chars = ['A', 'B', 'C', 'D', 'E'];
        let frames: Vec<Vec<Vec<CanvasCell>>> = chars
            .iter()
            .map(|&ch| make_single_cell(1, 1, ch, Some(Color::Red), None))
            .collect();
        dialog.timeline_frames = frames;
        dialog.timeline_available = true;
        dialog.set_per_frame_delays(vec![10, 20, 30, 40, 50]);
        let single_cell = vec![vec![CanvasCell {
            ch: ' ',
            fg: None,
            bg: None,
            height: None,
        }]];
        let result = dialog.perform_export(&single_cell);
        assert!(result.is_ok());
        let bytes = std::fs::read(&path).unwrap_or_default();
        let mut decoder = gif::DecodeOptions::new();
        decoder.set_color_output(gif::ColorOutput::RGBA);
        let mut reader = decoder.read_info(&bytes[..]).expect("should decode GIF");
        assert_eq!(reader.next_frame_info().unwrap().unwrap().delay, 10);
        assert_eq!(reader.next_frame_info().unwrap().unwrap().delay, 20);
        assert_eq!(reader.next_frame_info().unwrap().unwrap().delay, 30);
        assert_eq!(reader.next_frame_info().unwrap().unwrap().delay, 40);
        assert_eq!(reader.next_frame_info().unwrap().unwrap().delay, 50);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_perform_export_apng_5_frames() {
        use std::io::{BufReader, Cursor};
        let tmpdir = std::env::temp_dir();
        let path = tmpdir.join("test_perform_export_apng_5_frames.apng");
        let mut dialog = ExportDialog::new();
        dialog.enter_export(ExportMode::Apng);
        dialog.path_buffer = path.to_string_lossy().to_string();
        let chars = ['A', 'B', 'C', 'D', 'E'];
        let frames: Vec<Vec<Vec<CanvasCell>>> = chars
            .iter()
            .map(|&ch| make_single_cell(1, 1, ch, Some(Color::Red), None))
            .collect();
        dialog.timeline_frames = frames;
        dialog.timeline_available = true;
        dialog.set_per_frame_delays(vec![10, 20, 30, 40, 50]);
        let single_cell = vec![vec![CanvasCell {
            ch: ' ',
            fg: None,
            bg: None,
            height: None,
        }]];
        let result = dialog.perform_export(&single_cell);
        assert!(result.is_ok());
        let bytes = std::fs::read(&path).unwrap_or_default();
        let cursor = Cursor::new(&bytes[..]);
        let decoder = png::Decoder::new(BufReader::new(cursor));
        let mut reader = decoder.read_info().expect("should decode APNG header");
        let buf_size = reader.output_buffer_size().unwrap_or(1024);
        let mut buf = vec![0u8; buf_size];
        reader.next_frame(&mut buf).expect("frame 1");
        let fc2 = reader.next_frame_info().expect("frame 2 control");
        assert_eq!(fc2.delay_num, 20);
        reader.next_frame(&mut buf).expect("frame 2");
        let fc3 = reader.next_frame_info().expect("frame 3 control");
        assert_eq!(fc3.delay_num, 30);
        reader.next_frame(&mut buf).expect("frame 3");
        let fc4 = reader.next_frame_info().expect("frame 4 control");
        assert_eq!(fc4.delay_num, 40);
        reader.next_frame(&mut buf).expect("frame 4");
        let fc5 = reader.next_frame_info().expect("frame 5 control");
        assert_eq!(fc5.delay_num, 50);
        reader.next_frame(&mut buf).expect("frame 5");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_perform_export_ansi_5_frames() {
        let tmpdir = std::env::temp_dir();
        let path = tmpdir.join("test_perform_export_ansi_5_frames.ans");
        let mut dialog = ExportDialog::new();
        dialog.enter_export(ExportMode::Ansi);
        dialog.path_buffer = path.to_string_lossy().to_string();
        let chars = ['A', 'B', 'C', 'D', 'E'];
        let frames: Vec<Vec<Vec<CanvasCell>>> = chars
            .iter()
            .map(|&ch| make_single_cell(1, 1, ch, Some(Color::Red), None))
            .collect();
        dialog.timeline_frames = frames;
        dialog.timeline_available = true;
        dialog.set_per_frame_delays(vec![10, 20, 30, 40, 50]);
        let single_cell = vec![vec![CanvasCell {
            ch: ' ',
            fg: None,
            bg: None,
            height: None,
        }]];
        let result = dialog.perform_export(&single_cell);
        assert!(result.is_ok());
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        assert_eq!(content.matches("\x1b[2J").count(), 5);
        assert!(content.contains('A'));
        assert!(content.contains('B'));
        assert!(content.contains('C'));
        assert!(content.contains('D'));
        assert!(content.contains('E'));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_export_cycle_reaches_animation_formats() {
        let mut dialog = ExportDialog::new();
        dialog.enter_export(ExportMode::Png);
        let mut saw_apng = false;
        let mut saw_gif = false;
        let mut saw_ansi = false;
        for _ in 0..10 {
            dialog.handle_key(KeyCode::Char('T'));
            match dialog.format {
                ExportMode::Apng => saw_apng = true,
                ExportMode::Gif => saw_gif = true,
                ExportMode::Ansi => saw_ansi = true,
                _ => {}
            }
        }
        assert!(saw_apng, "Apng should be reachable");
        assert!(saw_gif, "Gif should be reachable");
        assert!(saw_ansi, "Ansi should be reachable");
        assert_eq!(ExportMode::Apng.label(), "APNG");
        assert_eq!(ExportMode::Gif.label(), "GIF");
        assert_eq!(ExportMode::Ansi.label(), "ANSI");
    }

    /// F-05 (GPT review): keyframes own *transforms*; a committed
    /// document_state owns *pixels*. The compositor must apply the two on
    /// top of each other instead of treating them as rival authorities.
    #[test]
    fn test_capture_applies_keyframe_transform_to_committed_snapshot() {
        let stack = LayerStack::new(3, 3);

        let mut buf = CanvasBuffer::new(3, 3);
        buf.set(
            0,
            0,
            CanvasCell {
                ch: 'A',
                fg: Some(Color::Red),
                bg: None,
                height: None,
            },
        );

        let mut timeline = TimelineState::default();
        timeline.add_frame(TimelineFrame {
            thumbnail: vec![],
            has_keyframe: true,
            label: "F0".into(),
            delay: 10,
            document_state: vec![buf],
            layer_keyframes: vec![Some(LayerKeyframe {
                position_offset: (1, 1),
                opacity: 128,
                blend_mode: BlendMode::Normal,
            })],
        });

        let frames = capture_timeline_frames(&timeline, &stack, 3, 3, None);
        assert_eq!(frames.len(), 1);
        assert_eq!(
            frames[0][1][1].ch, 'A',
            "snapshot pixel must be shifted by the keyframe offset"
        );
        // Opacity over an empty background cell keeps the top color
        // (nothing to blend against), so assert color passthrough.
        assert_eq!(frames[0][1][1].fg, Some(Color::Red));
    }

    #[test]
    fn test_export_gif_two_saved_frames_decode_distinct() {
        // End-to-end guard for F-04: two frames with visually different
        // saved snapshots must survive compositor → GIF encode → decode
        // as distinct pixel data. This is what production export broke
        // when it re-derived frames from the live layer stack.
        let stack = LayerStack::new(2, 2);

        let mut buf0 = CanvasBuffer::new(2, 2);
        buf0.set(
            0,
            0,
            CanvasCell {
                ch: '#',
                fg: Some(Color::Red),
                bg: None,
                height: None,
            },
        );
        let mut buf1 = CanvasBuffer::new(2, 2);
        buf1.set(
            1,
            1,
            CanvasCell {
                ch: '@',
                fg: Some(Color::Green),
                bg: None,
                height: None,
            },
        );

        let mut timeline = TimelineState::default();
        timeline.add_frame(TimelineFrame {
            thumbnail: vec![],
            has_keyframe: true,
            label: "F0".into(),
            delay: 10,
            document_state: vec![buf0],
            layer_keyframes: vec![Some(LayerKeyframe::default())],
        });
        timeline.add_frame(TimelineFrame {
            thumbnail: vec![],
            has_keyframe: true,
            label: "F1".into(),
            delay: 10,
            document_state: vec![buf1],
            layer_keyframes: vec![Some(LayerKeyframe::default())],
        });

        let frames = capture_timeline_frames(&timeline, &stack, 2, 2, None);
        assert_eq!(frames.len(), 2);
        assert_ne!(
            frames[0], frames[1],
            "compositor must produce distinct cells for distinct saved frames"
        );

        let gif_bytes = crate::output::export_cells_to_gif(&frames, &[10, 10], 4, 0)
            .expect("GIF encode should succeed");

        use gif::DecodeOptions;
        let mut decoder = DecodeOptions::new();
        decoder.set_color_output(gif::ColorOutput::RGBA);
        let mut reader = decoder
            .read_info(&gif_bytes[..])
            .expect("should decode GIF");

        let mut decoded: Vec<Vec<u8>> = Vec::new();
        while let Ok(Some(frame)) = reader.read_next_frame() {
            decoded.push(frame.buffer.to_vec());
        }
        assert_eq!(decoded.len(), 2, "GIF should contain both frames");
        assert_ne!(
            decoded[0], decoded[1],
            "decoded GIF frames must differ in pixels, not just timing"
        );
    }
}
