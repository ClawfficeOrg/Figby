use serde::de;
use serde::{Deserialize, Serialize};

use super::canvas::CanvasBuffer;
use super::layers::BlendMode;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EmissionShape {
    Point,
    CircleRadius(f64),
    RectWH(f64, f64),
}

impl EmissionShape {
    pub fn cycle(&self) -> Self {
        match self {
            EmissionShape::Point => EmissionShape::CircleRadius(5.0),
            EmissionShape::CircleRadius(_) => EmissionShape::RectWH(8.0, 4.0),
            EmissionShape::RectWH(_, _) => EmissionShape::Point,
        }
    }

    pub fn display_name(&self) -> &str {
        match self {
            EmissionShape::Point => "Point",
            EmissionShape::CircleRadius(_) => "Circle",
            EmissionShape::RectWH(_, _) => "Rect",
        }
    }

    pub fn display_value(&self) -> String {
        match self {
            EmissionShape::Point => "Point".to_string(),
            EmissionShape::CircleRadius(r) => format!("Circle({r})"),
            EmissionShape::RectWH(w, h) => format!("Rect({w}×{h})"),
        }
    }
}

impl Serialize for EmissionShape {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let s = match self {
            EmissionShape::Point => "point".to_string(),
            EmissionShape::CircleRadius(r) => format!("circle({r})"),
            EmissionShape::RectWH(w, h) => format!("rect({w},{h})"),
        };
        serializer.serialize_str(&s)
    }
}

impl<'de> Deserialize<'de> for EmissionShape {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        if s == "point" {
            return Ok(EmissionShape::Point);
        }
        if let Some(rest) = s.strip_prefix("circle(").and_then(|s| s.strip_suffix(')')) {
            if let Ok(r) = rest.parse::<f64>() {
                return Ok(EmissionShape::CircleRadius(r));
            }
        }
        if let Some(rest) = s.strip_prefix("rect(").and_then(|s| s.strip_suffix(')')) {
            if let Some((w_str, h_str)) = rest.split_once(',') {
                if let (Ok(w), Ok(h)) = (w_str.trim().parse::<f64>(), h_str.trim().parse::<f64>()) {
                    return Ok(EmissionShape::RectWH(w, h));
                }
            }
        }
        Err(de::Error::custom(format!("invalid emission shape: {s}")))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeMode {
    Bounce,
    Wrap,
    Despawn,
}

impl EdgeMode {
    pub fn cycle(&self) -> Self {
        match self {
            EdgeMode::Bounce => EdgeMode::Wrap,
            EdgeMode::Wrap => EdgeMode::Despawn,
            EdgeMode::Despawn => EdgeMode::Bounce,
        }
    }

    pub fn display_name(&self) -> &str {
        match self {
            EdgeMode::Bounce => "Bounce",
            EdgeMode::Wrap => "Wrap",
            EdgeMode::Despawn => "Despawn",
        }
    }
}

impl Serialize for EdgeMode {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(match self {
            EdgeMode::Bounce => "bounce",
            EdgeMode::Wrap => "wrap",
            EdgeMode::Despawn => "despawn",
        })
    }
}

impl<'de> Deserialize<'de> for EdgeMode {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "bounce" => Ok(EdgeMode::Bounce),
            "wrap" => Ok(EdgeMode::Wrap),
            "despawn" => Ok(EdgeMode::Despawn),
            _ => Err(de::Error::custom(format!("invalid edge mode: {s}"))),
        }
    }
}

fn default_emission_shape() -> EmissionShape {
    EmissionShape::Point
}

fn default_spread_angle() -> f64 {
    0.0
}

fn default_edge_mode() -> EdgeMode {
    EdgeMode::Bounce
}

#[derive(Debug, Clone, Deserialize)]
pub struct ParticleConfig {
    #[serde(default)]
    pub emitter_x: f64,
    #[serde(default)]
    pub emitter_y: f64,
    #[serde(default = "default_spawn_rate")]
    pub spawn_rate: f64,
    #[serde(default = "default_lifetime")]
    pub lifetime_min: f64,
    #[serde(default = "default_lifetime")]
    pub lifetime_max: f64,
    #[serde(default)]
    pub velocity_x_min: f64,
    #[serde(default)]
    pub velocity_x_max: f64,
    #[serde(default)]
    pub velocity_y_min: f64,
    #[serde(default)]
    pub velocity_y_max: f64,
    #[serde(default)]
    pub acceleration_x: f64,
    #[serde(default)]
    pub acceleration_y: f64,
    #[serde(default)]
    pub size: u8,
    #[serde(default)]
    pub color_r: Option<u8>,
    #[serde(default)]
    pub color_g: Option<u8>,
    #[serde(default)]
    pub color_b: Option<u8>,
    #[serde(default = "default_char")]
    pub character: char,
    #[serde(default = "default_opacity")]
    pub opacity: u8,
    #[serde(default)]
    pub blend_mode: Option<String>,
    #[serde(default = "default_spread_angle")]
    pub spread_angle: f64,
    #[serde(default = "default_emission_shape")]
    pub emission_shape: EmissionShape,
    #[serde(default = "default_edge_mode")]
    pub edge_mode: EdgeMode,
    #[serde(default)]
    pub collide_with_layer: bool,
    /// Optional per-particle keyframe track copied to every spawned particle.
    /// When non-empty, render-time appearance interpolates between adjacent
    /// keyframes based on the particle's lifetime progress.
    #[serde(default = "default_keyframes")]
    pub keyframes: Vec<ParticleKeyframe>,
    /// Number of secondary particles to emit when a primary particle dies.
    /// Sub-config `on_death_config` is used; the spawned particles are marked
    /// `is_secondary` so the burst does not trigger a recursive burst.
    #[serde(default = "default_on_death_count")]
    pub on_death_count: usize,
    /// Sub-configuration for the death-event burst. Carried in a `Box` to keep
    /// `ParticleConfig` a fixed size despite the recursive reference.
    #[serde(default)]
    pub on_death_config: Option<Box<ParticleConfig>>,
}

fn default_spawn_rate() -> f64 {
    10.0
}

fn default_lifetime() -> f64 {
    1.0
}

fn default_char() -> char {
    '*'
}

fn default_opacity() -> u8 {
    255
}

impl Default for ParticleConfig {
    fn default() -> Self {
        Self {
            emitter_x: 0.0,
            emitter_y: 0.0,
            spawn_rate: default_spawn_rate(),
            lifetime_min: default_lifetime(),
            lifetime_max: default_lifetime(),
            velocity_x_min: 0.0,
            velocity_x_max: 0.0,
            velocity_y_min: 0.0,
            velocity_y_max: 0.0,
            acceleration_x: 0.0,
            acceleration_y: 0.0,
            size: 1,
            color_r: None,
            color_g: None,
            color_b: None,
            character: default_char(),
            opacity: default_opacity(),
            blend_mode: None,
            spread_angle: default_spread_angle(),
            emission_shape: default_emission_shape(),
            edge_mode: default_edge_mode(),
            collide_with_layer: false,
            keyframes: Vec::new(),
            on_death_count: 0,
            on_death_config: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParticleKeyframe {
    /// Fraction of total lifetime in range [0.0, 1.0] at which this keyframe applies.
    pub time: f64,
    pub color: Option<(u8, u8, u8)>,
    pub size: u8,
    pub character: char,
    pub opacity: u8,
}

impl ParticleKeyframe {
    pub fn new(time: f64) -> Self {
        Self {
            time,
            color: None,
            size: 1,
            character: '*',
            opacity: 255,
        }
    }
}

impl Default for ParticleKeyframe {
    fn default() -> Self {
        Self::new(0.0)
    }
}

/// Before a particle glyph overwrites `cell`, fold a solid-block fill into the
/// background so the particle sits on the object's colour, not terminal black.
fn keep_backdrop(cell: &mut crate::CanvasCell) {
    if cell.bg.is_none() && cell.ch == '\u{2588}' {
        cell.bg = cell.fg;
    }
}

fn default_keyframes() -> Vec<ParticleKeyframe> {
    Vec::new()
}

fn default_on_death_count() -> usize {
    0
}

#[derive(Debug, Clone)]
pub struct Particle {
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub remaining_lifetime: f64,
    pub size: u8,
    pub color: Option<(u8, u8, u8)>,
    pub character: char,
    pub opacity: u8,
    pub blend_mode: BlendMode,
    /// Original lifetime used as the denominator when computing per-particle
    /// progress for keyframe interpolation. Set at spawn and never mutated.
    pub total_lifetime: f64,
    /// Optional per-particle keyframe track. Sorted ascending by `time` for
    /// interpolation. When empty, the particle's own fields drive rendering.
    pub keyframes: Vec<ParticleKeyframe>,
    /// `true` when the particle was spawned by another particle's death event
    /// so it does not itself trigger a recursively-spawned burst on death.
    pub is_secondary: bool,
}

impl Default for Particle {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            vx: 0.0,
            vy: 0.0,
            remaining_lifetime: 0.0,
            size: 1,
            color: None,
            character: '*',
            opacity: 255,
            blend_mode: BlendMode::Normal,
            total_lifetime: 0.0,
            keyframes: Vec::new(),
            is_secondary: false,
        }
    }
}

fn lerp_u8(a: u8, b: u8, t: f64) -> u8 {
    let av = a as f64;
    let bv = b as f64;
    let v = av + (bv - av) * t;
    v.round().clamp(0.0, 255.0) as u8
}

fn lerp_color(a: (u8, u8, u8), b: (u8, u8, u8), t: f64) -> (u8, u8, u8) {
    (
        lerp_u8(a.0, b.0, t),
        lerp_u8(a.1, b.1, t),
        lerp_u8(a.2, b.2, t),
    )
}

impl Particle {
    /// Render-time values for (color, size, character, opacity) after applying
    /// the optional keyframe track. When the particle has no keyframes, returns
    /// its own static fields, preserving backwards-compatible behavior.
    pub fn render_values(&self) -> (Option<(u8, u8, u8)>, u8, char, u8) {
        if self.keyframes.is_empty() || self.total_lifetime <= f64::EPSILON {
            return (self.color, self.size, self.character, self.opacity);
        }
        let progress = (1.0 - self.remaining_lifetime / self.total_lifetime).clamp(0.0, 1.0);
        let mut kfs = self.keyframes.clone();
        kfs.sort_by(|a, b| {
            a.time
                .partial_cmp(&b.time)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        if kfs.is_empty() {
            return (self.color, self.size, self.character, self.opacity);
        }
        if progress <= kfs[0].time {
            let kf = &kfs[0];
            return (kf.color, kf.size, kf.character, kf.opacity);
        }
        if progress >= kfs[kfs.len() - 1].time {
            let kf = &kfs[kfs.len() - 1];
            return (kf.color, kf.size, kf.character, kf.opacity);
        }
        let mut i = 0;
        while i + 1 < kfs.len() && kfs[i + 1].time < progress {
            i += 1;
        }
        let a = &kfs[i];
        let b = &kfs[i + 1];
        let span = (b.time - a.time).max(f64::EPSILON);
        let t = ((progress - a.time) / span).clamp(0.0, 1.0);
        let color = match (a.color, b.color) {
            (Some(ac), Some(bc)) => Some(lerp_color(ac, bc, t)),
            (Some(c), None) | (None, Some(c)) => Some(c),
            (None, None) => None,
        };
        let size = lerp_u8(a.size, b.size, t);
        let opacity = lerp_u8(a.opacity, b.opacity, t);
        let character = if t < 0.5 { a.character } else { b.character };
        (color, size, character, opacity)
    }

    /// Lifetime progress in `[0.0, 1.0]` (0 = just spawned, 1 = about to die).
    pub fn progress(&self) -> f64 {
        if self.total_lifetime <= f64::EPSILON {
            1.0
        } else {
            (1.0 - self.remaining_lifetime / self.total_lifetime).clamp(0.0, 1.0)
        }
    }

    fn new(config: &ParticleConfig) -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        let lifetime = if config.lifetime_min < config.lifetime_max {
            rng.gen_range(config.lifetime_min..config.lifetime_max)
        } else {
            config.lifetime_min
        };

        let mut vx = if config.velocity_x_min < config.velocity_x_max {
            rng.gen_range(config.velocity_x_min..config.velocity_x_max)
        } else {
            config.velocity_x_min
        };

        let mut vy = if config.velocity_y_min < config.velocity_y_max {
            rng.gen_range(config.velocity_y_min..config.velocity_y_max)
        } else {
            config.velocity_y_min
        };

        // Apply spread angle: randomize velocity direction within cone
        if config.spread_angle > 0.0 {
            let speed = (vx * vx + vy * vy).sqrt();
            if speed > 0.0 {
                let base_angle = vy.atan2(vx);
                let half_spread = config.spread_angle.to_radians() / 2.0;
                let angle = base_angle + rng.gen_range(-half_spread..half_spread);
                vx = speed * angle.cos();
                vy = speed * angle.sin();
            }
        }

        // Apply emission shape offset
        let (ox, oy) = match config.emission_shape {
            EmissionShape::Point => (0.0, 0.0),
            EmissionShape::CircleRadius(r) if r > 0.0 => {
                let a = rng.gen_range(0.0..std::f64::consts::TAU);
                let d = rng.gen_range(0.0..=r);
                (d * a.cos(), d * a.sin())
            }
            EmissionShape::CircleRadius(_) => (0.0, 0.0),
            EmissionShape::RectWH(w, h) if w > 0.0 && h > 0.0 => {
                let hw = w / 2.0;
                let hh = h / 2.0;
                (rng.gen_range(-hw..hw), rng.gen_range(-hh..hh))
            }
            EmissionShape::RectWH(_, _) => (0.0, 0.0),
        };

        let color = match (config.color_r, config.color_g, config.color_b) {
            (Some(r), Some(g), Some(b)) => Some((r, g, b)),
            _ => None,
        };

        let blend_mode = config
            .blend_mode
            .as_deref()
            .and_then(|s| s.parse::<BlendMode>().ok())
            .unwrap_or(BlendMode::Normal);

        Self {
            x: config.emitter_x + ox,
            y: config.emitter_y + oy,
            vx,
            vy,
            remaining_lifetime: lifetime,
            total_lifetime: lifetime,
            size: config.size,
            color,
            character: config.character,
            opacity: config.opacity,
            blend_mode,
            keyframes: config.keyframes.clone(),
            is_secondary: false,
        }
    }

    /// Spawn a secondary particle at `(x, y)` using the given sub-config.
    /// The new particle carries no keyframe track of its own and is marked
    /// `is_secondary = true` so its own death cannot trigger a recursive burst.
    fn spawn_secondary_at(config: &ParticleConfig, x: f64, y: f64) -> Self {
        let mut p = Particle::new(config);
        p.x = x;
        p.y = y;
        p.is_secondary = true;
        p
    }
}

#[derive(Debug, Clone)]
pub struct ParticleSystem {
    pub config: ParticleConfig,
    pub active_particles: Vec<Particle>,
    pub age: f64,
    spawn_rate_accumulator: f64,
    paused: bool,
}

impl ParticleSystem {
    pub fn new(config: ParticleConfig) -> Self {
        Self {
            config,
            active_particles: Vec::new(),
            age: 0.0,
            spawn_rate_accumulator: 0.0,
            paused: false,
        }
    }

    pub fn update(
        &mut self,
        dt: f64,
        bounds: Option<(usize, usize)>,
        layer_mask: Option<&CanvasBuffer>,
    ) {
        if self.paused || dt <= 0.0 {
            return;
        }

        self.age += dt;

        // Spawn new particles
        self.spawn_rate_accumulator += self.config.spawn_rate * dt;
        let to_spawn = self.spawn_rate_accumulator.floor() as usize;
        self.spawn_rate_accumulator -= to_spawn as f64;

        for _ in 0..to_spawn {
            self.active_particles.push(Particle::new(&self.config));
        }

        // Update existing particles
        let ax = self.config.acceleration_x;
        let ay = self.config.acceleration_y;

        for p in &mut self.active_particles {
            p.vx += ax * dt;
            p.vy += ay * dt;
            p.x += p.vx * dt;
            p.y += p.vy * dt;
            p.remaining_lifetime -= dt;

            // Edge collision
            if let Some((bw, bh)) = bounds {
                let ew = bw as f64;
                let eh = bh as f64;
                match self.config.edge_mode {
                    EdgeMode::Bounce => {
                        if p.x < 0.0 {
                            p.x = -p.x;
                            p.vx = -p.vx;
                        } else if p.x >= ew {
                            p.x = 2.0 * ew - p.x;
                            p.vx = -p.vx;
                        }
                        if p.y < 0.0 {
                            p.y = -p.y;
                            p.vy = -p.vy;
                        } else if p.y >= eh {
                            p.y = 2.0 * eh - p.y;
                            p.vy = -p.vy;
                        }
                    }
                    EdgeMode::Wrap => {
                        if p.x < 0.0 {
                            p.x += ew;
                        } else if p.x >= ew {
                            p.x -= ew;
                        }
                        if p.y < 0.0 {
                            p.y += eh;
                        } else if p.y >= eh {
                            p.y -= eh;
                        }
                    }
                    EdgeMode::Despawn => {
                        if p.x < 0.0 || p.x >= ew || p.y < 0.0 || p.y >= eh {
                            p.remaining_lifetime = 0.0;
                        }
                    }
                }
            }

            // Layer-cell collision
            if self.config.collide_with_layer {
                if let Some(mask) = layer_mask {
                    let cx = p.x.round() as usize;
                    let cy = p.y.round() as usize;
                    let occupied = |x: usize, y: usize| -> bool {
                        mask.get(x, y).is_some_and(|c| c.ch != ' ')
                    };
                    if cx < mask.width() && cy < mask.height() && occupied(cx, cy) {
                        let left = cx > 0 && occupied(cx - 1, cy);
                        let right = cx + 1 < mask.width() && occupied(cx + 1, cy);
                        let up = cy > 0 && occupied(cx, cy - 1);
                        let down = cy + 1 < mask.height() && occupied(cx, cy + 1);

                        let mut nx = 0.0f64;
                        let mut ny = 0.0f64;
                        if left {
                            nx += 1.0;
                        }
                        if right {
                            nx -= 1.0;
                        }
                        if up {
                            ny += 1.0;
                        }
                        if down {
                            ny -= 1.0;
                        }

                        let len = (nx * nx + ny * ny).sqrt();
                        if len > 0.0 {
                            nx /= len;
                            ny /= len;
                            let dot = p.vx * nx + p.vy * ny;
                            p.vx -= 2.0 * dot * nx;
                            p.vy -= 2.0 * dot * ny;
                        } else {
                            p.vx = -p.vx;
                            p.vy = -p.vy;
                        }
                        // Push particle out of occupied cell
                        p.x = cx as f64 + nx * 0.5;
                        p.y = cy as f64 + ny * 0.5;
                    }
                }
            }
        }

        // Collect secondary emissions for dying primary particles before removal.
        // `is_secondary` ensures the burst is not recursive.
        let mut secondaries: Vec<Particle> = Vec::new();
        if self.config.on_death_count > 0 {
            if let Some(sub_cfg) = self.config.on_death_config.as_ref() {
                for p in &self.active_particles {
                    if !p.is_secondary && p.remaining_lifetime <= 0.0 {
                        for _ in 0..self.config.on_death_count {
                            secondaries.push(Particle::spawn_secondary_at(sub_cfg, p.x, p.y));
                        }
                    }
                }
            }
        }

        // Remove expired particles
        self.active_particles.retain(|p| p.remaining_lifetime > 0.0);

        // Append any on-death bursts
        self.active_particles.extend(secondaries);
    }

    pub fn active_count(&self) -> usize {
        self.active_particles.len()
    }

    pub fn clear(&mut self) {
        self.active_particles.clear();
        self.spawn_rate_accumulator = 0.0;
        self.age = 0.0;
    }

    pub fn pause(&mut self) {
        self.paused = true;
    }

    pub fn resume(&mut self) {
        self.paused = false;
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// Render visible particles onto a canvas buffer.
    /// Clips to buffer bounds — no panic on out-of-range positions.
    pub fn render_to_canvas(&self, buffer: &mut CanvasBuffer) {
        let w = buffer.width() as i16;
        let h = buffer.height() as i16;
        for p in &self.active_particles {
            let px = p.x.round() as i16;
            let py = p.y.round() as i16;
            if px < 0 || py < 0 || px >= w || py >= h {
                continue;
            }
            if let Some(cell) = buffer.get_mut(px as usize, py as usize) {
                let (color, _size, character, _opacity) = p.render_values();
                keep_backdrop(cell);
                cell.ch = character;
                if let Some((r, g, b)) = color {
                    cell.fg = Some(ratatui::style::Color::Rgb(r, g, b));
                }
            }
        }
    }

    pub fn bake_to_buffer(&self, width: usize, height: usize) -> CanvasBuffer {
        let mut buffer = CanvasBuffer::new(width, height);
        let w = width as i16;
        let h = height as i16;
        for p in &self.active_particles {
            let px = p.x.round() as i16;
            let py = p.y.round() as i16;
            if px < 0 || py < 0 || px >= w || py >= h {
                continue;
            }
            if let Some(cell) = buffer.get_mut(px as usize, py as usize) {
                let (color, _size, character, _opacity) = p.render_values();
                keep_backdrop(cell);
                cell.ch = character;
                if let Some((r, g, b)) = color {
                    cell.fg = Some(ratatui::style::Color::Rgb(r, g, b));
                }
            }
        }
        buffer
    }

    pub fn bake_frames(
        &mut self,
        num_frames: usize,
        width: usize,
        height: usize,
        dt: f64,
    ) -> Vec<CanvasBuffer> {
        self.clear();
        let mut frames = Vec::with_capacity(num_frames);
        for _ in 0..num_frames {
            self.update(dt, Some((width, height)), None);
            frames.push(self.bake_to_buffer(width, height));
        }
        frames
    }
}

/// Config panel field definitions for the emitter panel UI.
pub struct EmitterConfigPanel {
    pub open: bool,
    pub selected_field: usize,
    pub editing: bool,
    pub edit_buffer: String,
    pub error_message: String,
}

impl EmitterConfigPanel {
    pub fn new() -> Self {
        Self {
            open: false,
            selected_field: 0,
            editing: false,
            edit_buffer: String::new(),
            error_message: String::new(),
        }
    }

    /// Render the config panel as a bordered overlay with all emitter parameters.
    pub fn render_config_panel(&self, frame: &mut Frame, area: Rect, config: &ParticleConfig) {
        let field_names: [&str; 19] = [
            "Spawn Rate",
            "Lifetime Min",
            "Lifetime Max",
            "Vel X Min",
            "Vel X Max",
            "Vel Y Min",
            "Vel Y Max",
            "Accel X",
            "Accel Y",
            "Spread Angle",
            "Emission Shape",
            "Size",
            "Character",
            "Color R",
            "Color G",
            "Color B",
            "Opacity",
            "Edge Mode",
            "Collide w/ Layer",
        ];

        let mut lines: Vec<Line> = Vec::new();

        lines.push(Line::from(Span::styled(
            " Emitter Config",
            Style::default().add_modifier(Modifier::BOLD),
        )));

        for (i, &name) in field_names.iter().enumerate() {
            let is_selected = i == self.selected_field;
            let value = if self.editing && i == self.selected_field {
                self.edit_buffer.clone()
            } else {
                field_display_value(config, i)
            };

            let prefix = if is_selected && !self.editing {
                " >"
            } else if is_selected && self.editing {
                ">>"
            } else {
                "  "
            };

            let text = format!("{} {}: {}", prefix, name, value);

            let style = if is_selected {
                if self.editing {
                    Style::default()
                        .fg(ratatui::style::Color::Green)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().add_modifier(Modifier::REVERSED)
                }
            } else {
                Style::default()
            };

            lines.push(Line::from(Span::styled(text, style)));
        }

        if !self.error_message.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                format!(" Error: {}", self.error_message),
                Style::default().fg(ratatui::style::Color::Red),
            )));
        }

        lines.push(Line::from(""));
        if self.editing {
            lines.push(Line::from(Span::styled(
                " Enter: Save  Esc: Cancel  Backspace: Delete",
                Style::default().fg(ratatui::style::Color::DarkGray),
            )));
        } else {
            lines.push(Line::from(Span::styled(
                " \u{2191}\u{2193}: Nav  Enter: Edit  Esc: Close",
                Style::default().fg(ratatui::style::Color::DarkGray),
            )));
        }

        let paragraph = Paragraph::new(lines).block(Block::default().borders(Borders::ALL));
        frame.render_widget(paragraph, area);
    }

    /// Handle key events for the config panel.
    /// Returns true if the key was consumed, false to pass through.
    /// Modifies `config` when a field value is saved.
    pub fn handle_config_key(
        &mut self,
        code: crossterm::event::KeyCode,
        config: &mut ParticleConfig,
    ) -> bool {
        if self.editing {
            match code {
                crossterm::event::KeyCode::Enter => {
                    self.error_message.clear();
                    if self.save_field(config) {
                        self.editing = false;
                    }
                    true
                }
                crossterm::event::KeyCode::Esc => {
                    self.editing = false;
                    self.edit_buffer.clear();
                    self.error_message.clear();
                    true
                }
                crossterm::event::KeyCode::Backspace => {
                    self.edit_buffer.pop();
                    self.error_message.clear();
                    true
                }
                crossterm::event::KeyCode::Char(c) if !c.is_control() => {
                    // Shape/edge/toggle fields: cycling on Enter only, chars ignored in edit mode
                    if self.selected_field == 10
                        || self.selected_field == 17
                        || self.selected_field == 18
                    {
                        return true;
                    }
                    self.edit_buffer.push(c);
                    self.error_message.clear();
                    true
                }
                _ => false,
            }
        } else {
            match code {
                crossterm::event::KeyCode::Up => {
                    if self.selected_field > 0 {
                        self.selected_field -= 1;
                    }
                    self.error_message.clear();
                    true
                }
                crossterm::event::KeyCode::Down => {
                    if self.selected_field < 18 {
                        self.selected_field += 1;
                    }
                    self.error_message.clear();
                    true
                }
                crossterm::event::KeyCode::Enter => {
                    self.edit_buffer.clear();
                    self.error_message.clear();
                    self.editing = true;
                    self.start_editing();
                    true
                }
                crossterm::event::KeyCode::Esc => {
                    self.open = false;
                    self.error_message.clear();
                    true
                }
                _ => false,
            }
        }
    }

    fn start_editing(&mut self) {
        if self.selected_field == 10 || self.selected_field == 17 || self.selected_field == 18 {
            return;
        }
        self.edit_buffer.clear();
    }

    fn save_field(&mut self, config: &mut ParticleConfig) -> bool {
        let idx = self.selected_field;
        let val = &self.edit_buffer;
        match idx {
            0 => parse_f64(val, &mut config.spawn_rate, "spawn_rate"),
            1 => parse_f64(val, &mut config.lifetime_min, "lifetime_min"),
            2 => parse_f64(val, &mut config.lifetime_max, "lifetime_max"),
            3 => parse_f64(val, &mut config.velocity_x_min, "velocity_x_min"),
            4 => parse_f64(val, &mut config.velocity_x_max, "velocity_x_max"),
            5 => parse_f64(val, &mut config.velocity_y_min, "velocity_y_min"),
            6 => parse_f64(val, &mut config.velocity_y_max, "velocity_y_max"),
            7 => parse_f64(val, &mut config.acceleration_x, "acceleration_x"),
            8 => parse_f64(val, &mut config.acceleration_y, "acceleration_y"),
            9 => parse_f64(val, &mut config.spread_angle, "spread_angle"),
            10 => {
                config.emission_shape = config.emission_shape.cycle();
                true
            }
            11 => parse_u8(val, &mut config.size, "size"),
            12 => {
                if let Some(c) = val.chars().next() {
                    config.character = c;
                    true
                } else {
                    self.error_message = "Enter a single character".to_string();
                    false
                }
            }
            13 => parse_optional_u8(val, &mut config.color_r, "color_r"),
            14 => parse_optional_u8(val, &mut config.color_g, "color_g"),
            15 => parse_optional_u8(val, &mut config.color_b, "color_b"),
            16 => parse_u8(val, &mut config.opacity, "opacity"),
            17 => {
                config.edge_mode = config.edge_mode.cycle();
                true
            }
            18 => {
                config.collide_with_layer = !config.collide_with_layer;
                true
            }
            _ => false,
        }
    }
}

impl Default for EmitterConfigPanel {
    fn default() -> Self {
        Self::new()
    }
}

fn field_display_value(config: &ParticleConfig, idx: usize) -> String {
    match idx {
        0 => format_f64(config.spawn_rate),
        1 => format_f64(config.lifetime_min),
        2 => format_f64(config.lifetime_max),
        3 => format_f64(config.velocity_x_min),
        4 => format_f64(config.velocity_x_max),
        5 => format_f64(config.velocity_y_min),
        6 => format_f64(config.velocity_y_max),
        7 => format_f64(config.acceleration_x),
        8 => format_f64(config.acceleration_y),
        9 => format_f64(config.spread_angle),
        10 => config.emission_shape.display_value(),
        11 => config.size.to_string(),
        12 => config.character.to_string(),
        13 => config.color_r.map(|v| v.to_string()).unwrap_or_default(),
        14 => config.color_g.map(|v| v.to_string()).unwrap_or_default(),
        15 => config.color_b.map(|v| v.to_string()).unwrap_or_default(),
        16 => config.opacity.to_string(),
        17 => config.edge_mode.display_name().to_string(),
        18 => if config.collide_with_layer {
            "On"
        } else {
            "Off"
        }
        .to_string(),
        _ => String::new(),
    }
}

fn parse_f64(val: &str, target: &mut f64, _name: &str) -> bool {
    val.parse::<f64>().map(|v| *target = v).is_ok()
}

fn parse_u8(val: &str, target: &mut u8, _name: &str) -> bool {
    val.parse::<u8>().map(|v| *target = v).is_ok()
}

fn parse_optional_u8(val: &str, target: &mut Option<u8>, _name: &str) -> bool {
    if val.is_empty() {
        *target = None;
        true
    } else {
        val.parse::<u8>().map(|v| *target = Some(v)).is_ok()
    }
}

fn format_f64(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{v:.0}")
    } else {
        format!("{v:.2}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> ParticleConfig {
        ParticleConfig {
            spawn_rate: 10.0,
            lifetime_min: 2.0,
            lifetime_max: 2.0,
            ..Default::default()
        }
    }

    #[test]
    fn test_particle_spawn() {
        let mut system = ParticleSystem::new(test_config());
        system.update(1.0, None, None);
        assert!(system.active_count() >= 1);
        for p in &system.active_particles {
            assert_eq!(p.x, 0.0);
            assert_eq!(p.y, 0.0);
        }
    }

    #[test]
    fn test_particle_update_motion() {
        let config = ParticleConfig {
            spawn_rate: 10.0,
            lifetime_min: 2.0,
            lifetime_max: 2.0,
            velocity_x_min: 5.0,
            velocity_x_max: 5.0,
            ..Default::default()
        };
        let mut system = ParticleSystem::new(config);
        system.update(0.1, None, None);
        assert!(system.active_count() >= 1);
        let p = &system.active_particles[0];
        assert!((p.x - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_particle_expire() {
        let config = ParticleConfig {
            spawn_rate: 10.0,
            lifetime_min: 0.5,
            lifetime_max: 0.5,
            ..Default::default()
        };
        let mut system = ParticleSystem::new(config);
        system.update(0.3, None, None);
        assert!(system.active_count() > 0);
        system.update(0.6, None, None);
        assert_eq!(system.active_count(), 0);
    }

    #[test]
    fn test_particle_acceleration() {
        let config = ParticleConfig {
            spawn_rate: 1.0,
            lifetime_min: 2.0,
            lifetime_max: 2.0,
            acceleration_y: 5.0,
            ..Default::default()
        };
        let mut system = ParticleSystem::new(config);
        system.update(1.0, None, None);
        assert_eq!(system.active_count(), 1);
        let p = &system.active_particles[0];
        assert!((p.vy - 5.0).abs() < 0.01);
        assert!((p.y - 5.0).abs() < 0.01);
    }

    #[test]
    fn test_particle_spawn_rate_accumulator() {
        let config = ParticleConfig {
            spawn_rate: 0.5,
            lifetime_min: 10.0,
            lifetime_max: 10.0,
            ..Default::default()
        };
        let mut system = ParticleSystem::new(config);
        system.update(1.0, None, None);
        let after_1s = system.active_count();
        system.update(3.0, None, None);
        assert_eq!(after_1s + system.active_count(), 2);
    }

    #[test]
    fn test_particle_full_lifecycle() {
        let config = ParticleConfig {
            spawn_rate: 1.0,
            lifetime_min: 3.0,
            lifetime_max: 3.0,
            velocity_x_min: 2.0,
            velocity_x_max: 2.0,
            ..Default::default()
        };
        let mut system = ParticleSystem::new(config);
        system.update(1.0, None, None);
        assert_eq!(system.active_count(), 1);
        let p = &system.active_particles[0];
        assert!((p.x - 2.0).abs() < 0.01);

        system.update(1.0, None, None);
        assert_eq!(system.active_count(), 2);
        let p1 = &system.active_particles[0];
        let p2 = &system.active_particles[1];
        assert!((p1.x - 4.0).abs() < 0.01);
        assert!((p2.x - 2.0).abs() < 0.01);

        // Both still alive at t=2.99
        system.update(0.99, None, None);
        assert_eq!(system.active_count(), 2);

        // Expire after lifetime exceeded for oldest (P2 still alive, P3 spawned)
        system.update(1.01, None, None);
        assert_eq!(system.active_count(), 2);
    }

    #[test]
    fn test_particle_color_from_config() {
        let config = ParticleConfig {
            spawn_rate: 10.0,
            lifetime_min: 2.0,
            lifetime_max: 2.0,
            color_r: Some(255),
            color_g: Some(128),
            color_b: Some(64),
            ..Default::default()
        };
        let mut system = ParticleSystem::new(config);
        system.update(0.1, None, None);
        let p = &system.active_particles[0];
        assert_eq!(p.color, Some((255, 128, 64)));
    }

    #[test]
    fn test_particle_no_color_default() {
        let config = ParticleConfig {
            spawn_rate: 10.0,
            lifetime_min: 2.0,
            lifetime_max: 2.0,
            ..Default::default()
        };
        let mut system = ParticleSystem::new(config);
        system.update(0.1, None, None);
        let p = &system.active_particles[0];
        assert_eq!(p.color, None);
    }

    #[test]
    fn test_particle_system_pause_resume() {
        let config = ParticleConfig {
            spawn_rate: 100.0,
            lifetime_min: 10.0,
            lifetime_max: 10.0,
            ..Default::default()
        };
        let mut system = ParticleSystem::new(config);
        system.update(1.0, None, None);
        let count_before_pause = system.active_count();
        assert!(count_before_pause > 0);

        system.pause();
        assert!(system.is_paused());
        system.update(1.0, None, None);
        assert_eq!(system.active_count(), count_before_pause);

        system.resume();
        assert!(!system.is_paused());
        system.update(1.0, None, None);
        assert!(system.active_count() > count_before_pause);
    }

    #[test]
    fn test_particle_system_clear() {
        let config = ParticleConfig {
            spawn_rate: 100.0,
            lifetime_min: 10.0,
            lifetime_max: 10.0,
            ..Default::default()
        };
        let mut system = ParticleSystem::new(config);
        system.update(1.0, None, None);
        assert!(system.active_count() > 0);
        system.clear();
        assert_eq!(system.active_count(), 0);
        assert_eq!(system.age, 0.0);
    }

    #[test]
    fn test_particle_negative_dt_noop() {
        let mut system = ParticleSystem::new(test_config());
        system.update(1.0, None, None);
        let count = system.active_count();
        system.update(-1.0, None, None);
        assert_eq!(system.active_count(), count);
    }

    #[test]
    fn test_particle_zero_dt_noop() {
        let mut system = ParticleSystem::new(test_config());
        system.update(1.0, None, None);
        let count = system.active_count();
        system.update(0.0, None, None);
        assert_eq!(system.active_count(), count);
    }

    // ── Emission shape tests ──────────────────────────────────

    #[test]
    fn test_emitter_shape_point() {
        let config = ParticleConfig {
            spawn_rate: 100.0,
            lifetime_min: 10.0,
            lifetime_max: 10.0,
            emission_shape: EmissionShape::Point,
            ..Default::default()
        };
        let mut system = ParticleSystem::new(config);
        system.update(1.0, None, None);
        assert!(!system.active_particles.is_empty());
        for p in &system.active_particles {
            assert!((p.x - 0.0).abs() < 0.01, "particle x {} not at 0", p.x);
            assert!((p.y - 0.0).abs() < 0.01, "particle y {} not at 0", p.y);
        }
    }

    #[test]
    fn test_emitter_shape_circle() {
        let config = ParticleConfig {
            spawn_rate: 100.0,
            lifetime_min: 10.0,
            lifetime_max: 10.0,
            emission_shape: EmissionShape::CircleRadius(5.0),
            ..Default::default()
        };
        let mut system = ParticleSystem::new(config);
        system.update(1.0, None, None);
        assert!(!system.active_particles.is_empty());
        for p in &system.active_particles {
            let dist = ((p.x * p.x) + (p.y * p.y)).sqrt();
            assert!(dist <= 5.01, "particle at dist {} > 5.0", dist);
        }
    }

    #[test]
    fn test_emitter_shape_rect() {
        let config = ParticleConfig {
            spawn_rate: 100.0,
            lifetime_min: 10.0,
            lifetime_max: 10.0,
            emission_shape: EmissionShape::RectWH(8.0, 4.0),
            ..Default::default()
        };
        let mut system = ParticleSystem::new(config);
        system.update(1.0, None, None);
        assert!(!system.active_particles.is_empty());
        for p in &system.active_particles {
            assert!(
                p.x >= -4.01 && p.x <= 4.01,
                "particle x {} out of rect",
                p.x
            );
            assert!(
                p.y >= -2.01 && p.y <= 2.01,
                "particle y {} out of rect",
                p.y
            );
        }
    }

    // ── Spread angle tests ────────────────────────────────────

    #[test]
    fn test_emitter_spread_angle() {
        let config = ParticleConfig {
            spawn_rate: 100.0,
            lifetime_min: 10.0,
            lifetime_max: 10.0,
            velocity_x_min: 10.0,
            velocity_x_max: 10.0,
            spread_angle: 90.0,
            ..Default::default()
        };
        let mut system = ParticleSystem::new(config);
        system.update(1.0, None, None);
        assert!(!system.active_particles.is_empty());
        for p in &system.active_particles {
            let angle = p.vy.atan2(p.vx).abs().to_degrees();
            assert!(angle <= 45.01, "angle {} > 45 from base", angle);
        }
    }

    // ── Render to canvas tests ────────────────────────────────

    #[test]
    fn test_emitter_render_to_canvas() {
        let config = ParticleConfig {
            spawn_rate: 5.0,
            lifetime_min: 10.0,
            lifetime_max: 10.0,
            emitter_x: 5.0,
            emitter_y: 5.0,
            character: '@',
            ..Default::default()
        };
        let mut system = ParticleSystem::new(config);
        system.update(1.0, None, None);
        assert_eq!(system.active_count(), 5);

        let mut buffer = super::super::canvas::CanvasBuffer::new(20, 20);
        system.render_to_canvas(&mut buffer);

        // All 5 particles should be at (5,5) with '@' char
        let cell = buffer.get(5, 5).expect("cell at (5,5)");
        assert_eq!(cell.ch, '@');
    }

    #[test]
    fn test_emitter_render_bounds_clip() {
        let config = ParticleConfig {
            spawn_rate: 100.0,
            lifetime_min: 10.0,
            lifetime_max: 10.0,
            emitter_x: 999.0,
            emitter_y: 999.0,
            ..Default::default()
        };
        let mut system = ParticleSystem::new(config);
        system.update(1.0, None, None);

        let mut buffer = super::super::canvas::CanvasBuffer::new(10, 10);
        // Should not panic
        system.render_to_canvas(&mut buffer);
        // No particles visible in bounds
        for y in 0..10 {
            for x in 0..10 {
                let cell = buffer.get(x, y).unwrap();
                assert_eq!(cell.ch, ' ', "unexpected char at ({x},{y})");
            }
        }
    }

    // ── Config panel tests ────────────────────────────────────

    #[test]
    fn test_emitter_config_field_navigate() {
        use crossterm::event::KeyCode;
        let mut panel = EmitterConfigPanel::new();
        let mut config = ParticleConfig::default();

        // Navigate down through all fields
        for expected in 1..=18 {
            assert!(panel.handle_config_key(KeyCode::Down, &mut config));
            assert_eq!(panel.selected_field, expected);
        }
        // Stop at last field
        assert!(panel.handle_config_key(KeyCode::Down, &mut config));
        assert_eq!(panel.selected_field, 18);

        // Navigate up
        for expected in (0..=17).rev() {
            assert!(panel.handle_config_key(KeyCode::Up, &mut config));
            assert_eq!(panel.selected_field, expected);
        }
    }

    #[test]
    fn test_emitter_config_edit_float() {
        use crossterm::event::KeyCode;
        let mut panel = EmitterConfigPanel::new();
        let mut config = ParticleConfig::default();

        // Select spawn_rate field (index 0)
        panel.selected_field = 0;
        // Start editing
        panel.handle_config_key(KeyCode::Enter, &mut config);
        assert!(panel.editing);

        // Type new value
        for c in "25.5".chars() {
            panel.handle_config_key(KeyCode::Char(c), &mut config);
        }
        assert_eq!(panel.edit_buffer, "25.5");

        // Save
        panel.handle_config_key(KeyCode::Enter, &mut config);
        assert!(!panel.editing);
        assert!((config.spawn_rate - 25.5).abs() < 0.001);
    }

    #[test]
    fn test_emitter_config_edit_shape() {
        use crossterm::event::KeyCode;
        let mut panel = EmitterConfigPanel::new();
        let mut config = ParticleConfig::default();

        assert_eq!(config.emission_shape, EmissionShape::Point);

        // Select emission shape field (index 10)
        panel.selected_field = 10;
        // Enter cycles to Circle
        panel.handle_config_key(KeyCode::Enter, &mut config);
        panel.handle_config_key(KeyCode::Enter, &mut config);
        assert_eq!(config.emission_shape, EmissionShape::CircleRadius(5.0));

        // Enter again cycles to Rect
        panel.handle_config_key(KeyCode::Enter, &mut config);
        panel.handle_config_key(KeyCode::Enter, &mut config);
        assert_eq!(config.emission_shape, EmissionShape::RectWH(8.0, 4.0));

        // Enter again cycles back to Point
        panel.handle_config_key(KeyCode::Enter, &mut config);
        panel.handle_config_key(KeyCode::Enter, &mut config);
        assert_eq!(config.emission_shape, EmissionShape::Point);
    }

    #[test]
    fn test_emission_shape_cycle() {
        let mut shape = EmissionShape::Point;
        shape = shape.cycle();
        assert_eq!(shape, EmissionShape::CircleRadius(5.0));
        shape = shape.cycle();
        assert_eq!(shape, EmissionShape::RectWH(8.0, 4.0));
        shape = shape.cycle();
        assert_eq!(shape, EmissionShape::Point);
    }

    #[test]
    fn test_emission_shape_serde_roundtrip() {
        let shapes = [
            EmissionShape::Point,
            EmissionShape::CircleRadius(3.5),
            EmissionShape::RectWH(10.0, 20.0),
        ];
        for shape in &shapes {
            let json = serde_json::to_string(shape).unwrap();
            let parsed: EmissionShape = serde_json::from_str(&json).unwrap();
            assert_eq!(*shape, parsed, "roundtrip failed for {shape:?}");
        }
    }

    // ── Bake to buffer tests ──────────────────────────────────

    #[test]
    fn test_bake_to_buffer_independence() {
        let config = ParticleConfig {
            spawn_rate: 10.0,
            lifetime_min: 5.0,
            lifetime_max: 5.0,
            velocity_x_min: 10.0,
            velocity_x_max: 10.0,
            ..Default::default()
        };
        let mut system = ParticleSystem::new(config);
        system.update(0.1, None, None);
        let frame1 = system.bake_to_buffer(20, 20);
        system.update(1.0, None, None);
        let frame2 = system.bake_to_buffer(20, 20);
        // Particles should have moved, so buffers differ
        let mut any_diff = false;
        for y in 0..20 {
            for x in 0..20 {
                if frame1.get(x, y) != frame2.get(x, y) {
                    any_diff = true;
                }
            }
        }
        assert!(any_diff, "baked buffers should differ after update");
    }

    #[test]
    fn test_bake_frames_count_and_independence() {
        let config = ParticleConfig {
            spawn_rate: 5.0,
            lifetime_min: 5.0,
            lifetime_max: 5.0,
            velocity_x_min: 5.0,
            velocity_x_max: 5.0,
            ..Default::default()
        };
        let mut system = ParticleSystem::new(config);
        let frames = system.bake_frames(10, 20, 20, 0.1);
        assert_eq!(frames.len(), 10, "should produce exactly 10 frames");
        let all_same = frames.windows(2).all(|w| {
            for y in 0..20 {
                for x in 0..20 {
                    if w[0].get(x, y) != w[1].get(x, y) {
                        return false;
                    }
                }
            }
            true
        });
        assert!(!all_same, "frames should not all be identical");
    }

    #[test]
    fn test_bake_empty_system() {
        let system = ParticleSystem::new(ParticleConfig::default());
        let buffer = system.bake_to_buffer(10, 10);
        for y in 0..10 {
            for x in 0..10 {
                let cell = buffer.get(x, y).unwrap();
                assert_eq!(cell.ch, ' ', "cell at ({x},{y}) should be empty");
                assert!(cell.fg.is_none());
            }
        }
    }

    #[test]
    fn test_baked_frame_content() {
        let config = ParticleConfig {
            spawn_rate: 1.0,
            lifetime_min: 10.0,
            lifetime_max: 10.0,
            emitter_x: 5.0,
            emitter_y: 5.0,
            character: '@',
            ..Default::default()
        };
        let mut system = ParticleSystem::new(config);
        system.update(1.0, None, None);
        let buffer = system.bake_to_buffer(20, 20);
        let cell = buffer.get(5, 5).expect("cell at emitter position");
        assert_eq!(cell.ch, '@', "baked cell should have particle char");
    }

    // ── Edge collision tests ───────────────────────────────────

    #[test]
    fn test_edge_bounce_right() {
        let mut system = ParticleSystem::new(ParticleConfig {
            edge_mode: EdgeMode::Bounce,
            ..Default::default()
        });
        // Manually place a particle moving right near right edge
        system.active_particles.push(Particle {
            x: 9.8,
            y: 5.0,
            vx: 5.0,
            vy: 0.0,
            remaining_lifetime: 10.0,
            size: 1,
            color: None,
            character: '*',
            opacity: 255,
            blend_mode: BlendMode::Normal,
            total_lifetime: 10.0,
            keyframes: Vec::new(),
            is_secondary: false,
        });
        system.update(1.0, Some((10, 10)), None);
        let p = &system.active_particles[0];
        assert!(p.vx < 0.0, "vx should reverse on bounce, got {}", p.vx);
        assert!(p.x < 10.0, "x should be clamped inside bounds");
    }

    #[test]
    fn test_edge_bounce_left() {
        let mut system = ParticleSystem::new(ParticleConfig {
            edge_mode: EdgeMode::Bounce,
            ..Default::default()
        });
        system.active_particles.push(Particle {
            x: 0.2,
            y: 5.0,
            vx: -5.0,
            vy: 0.0,
            remaining_lifetime: 10.0,
            size: 1,
            color: None,
            character: '*',
            opacity: 255,
            blend_mode: BlendMode::Normal,
            total_lifetime: 10.0,
            keyframes: Vec::new(),
            is_secondary: false,
        });
        system.update(1.0, Some((10, 10)), None);
        let p = &system.active_particles[0];
        assert!(p.vx > 0.0, "vx should reverse on left bounce, got {}", p.vx);
        assert!(p.x >= 0.0, "x should be non-negative");
    }

    #[test]
    fn test_edge_wrap() {
        let mut system = ParticleSystem::new(ParticleConfig {
            edge_mode: EdgeMode::Wrap,
            ..Default::default()
        });
        system.active_particles.push(Particle {
            x: 9.8,
            y: 5.0,
            vx: 5.0,
            vy: 0.0,
            remaining_lifetime: 10.0,
            size: 1,
            color: None,
            character: '*',
            opacity: 255,
            blend_mode: BlendMode::Normal,
            total_lifetime: 10.0,
            keyframes: Vec::new(),
            is_secondary: false,
        });
        system.update(1.0, Some((10, 10)), None);
        let p = &system.active_particles[0];
        assert!(p.x < 5.0, "x should wrap to left side, got {}", p.x);
    }

    #[test]
    fn test_edge_despawn() {
        let mut system = ParticleSystem::new(ParticleConfig {
            edge_mode: EdgeMode::Despawn,
            ..Default::default()
        });
        system.active_particles.push(Particle {
            x: 9.8,
            y: 5.0,
            vx: 5.0,
            vy: 0.0,
            remaining_lifetime: 10.0,
            size: 1,
            color: None,
            character: '*',
            opacity: 255,
            blend_mode: BlendMode::Normal,
            total_lifetime: 10.0,
            keyframes: Vec::new(),
            is_secondary: false,
        });
        system.update(1.0, Some((10, 10)), None);
        assert!(
            system.active_particles.is_empty(),
            "particle should despawn at edge"
        );
    }

    #[test]
    fn test_edge_no_bounce_without_bounds() {
        let mut system = ParticleSystem::new(ParticleConfig {
            edge_mode: EdgeMode::Bounce,
            ..Default::default()
        });
        system.active_particles.push(Particle {
            x: 9.8,
            y: 5.0,
            vx: 5.0,
            vy: 0.0,
            remaining_lifetime: 10.0,
            size: 1,
            color: None,
            character: '*',
            opacity: 255,
            blend_mode: BlendMode::Normal,
            total_lifetime: 10.0,
            keyframes: Vec::new(),
            is_secondary: false,
        });
        // No bounds passed — particle should keep moving
        system.update(1.0, None, None);
        let p = &system.active_particles[0];
        assert!(p.vx > 0.0, "vx should be unchanged without bounds");
    }

    // ── Layer-cell collision tests ─────────────────────────────

    #[test]
    fn test_layer_collision_reflect() {
        use crate::tui::canvas::CanvasBuffer;
        let mut system = ParticleSystem::new(ParticleConfig {
            collide_with_layer: true,
            ..Default::default()
        });
        // Create a buffer with a filled cell at (5, 5)
        let mut mask = CanvasBuffer::new(10, 10);
        mask.set(
            5,
            5,
            crate::CanvasCell {
                ch: '#',
                ..Default::default()
            },
        );
        // Place particle just left of (5,5) moving right into it
        system.active_particles.push(Particle {
            x: 4.5,
            y: 5.0,
            vx: 0.5,
            vy: 0.0,
            remaining_lifetime: 10.0,
            size: 1,
            color: None,
            character: '*',
            opacity: 255,
            blend_mode: BlendMode::Normal,
            total_lifetime: 10.0,
            keyframes: Vec::new(),
            is_secondary: false,
        });
        system.update(1.0, Some((10, 10)), Some(&mask));
        let p = &system.active_particles[0];
        assert!(
            p.vx < 0.0,
            "vx should reverse on layer collision, got {}",
            p.vx
        );
    }

    #[test]
    fn test_layer_collision_noop_when_disabled() {
        use crate::tui::canvas::CanvasBuffer;
        let mut system = ParticleSystem::new(ParticleConfig {
            collide_with_layer: false, // disabled
            ..Default::default()
        });
        let mut mask = CanvasBuffer::new(10, 10);
        mask.set(
            5,
            5,
            crate::CanvasCell {
                ch: '#',
                ..Default::default()
            },
        );
        system.active_particles.push(Particle {
            x: 4.5,
            y: 5.0,
            vx: 5.0,
            vy: 0.0,
            remaining_lifetime: 10.0,
            size: 1,
            color: None,
            character: '*',
            opacity: 255,
            blend_mode: BlendMode::Normal,
            total_lifetime: 10.0,
            keyframes: Vec::new(),
            is_secondary: false,
        });
        system.update(1.0, Some((10, 10)), Some(&mask));
        let p = &system.active_particles[0];
        assert!(p.vx > 0.0, "vx should be unchanged when collide disabled");
    }

    // ── Per-particle keyframe interpolation tests ──────────────

    fn kf_test_particle(lifetime: f64, progress: f64) -> Particle {
        let keyframes = vec![
            ParticleKeyframe {
                time: 0.0,
                color: Some((255, 0, 0)),
                size: 1,
                character: '*',
                opacity: 255,
            },
            ParticleKeyframe {
                time: 0.5,
                color: Some((0, 0, 255)),
                size: 1,
                character: '+',
                opacity: 255,
            },
            ParticleKeyframe {
                time: 1.0,
                color: Some((255, 255, 255)),
                size: 1,
                character: '#',
                opacity: 255,
            },
        ];
        let remaining = lifetime * (1.0 - progress);
        Particle {
            remaining_lifetime: remaining,
            total_lifetime: lifetime,
            keyframes,
            ..Default::default()
        }
    }

    #[test]
    fn test_keyframe_color_at_25_percent() {
        let p = kf_test_particle(4.0, 0.25);
        let (color, _size, _ch, _op) = p.render_values();
        let c = color.expect("keyframe provides color");
        assert_eq!(c, (128, 0, 128), "red→blue midpoint at 25%");
    }

    #[test]
    fn test_keyframe_color_at_50_percent() {
        let p = kf_test_particle(4.0, 0.5);
        let (color, _size, _ch, _op) = p.render_values();
        let c = color.expect("keyframe provides color");
        assert_eq!(c, (0, 0, 255), "exact blue at 50%");
    }

    #[test]
    fn test_keyframe_color_at_75_percent() {
        let p = kf_test_particle(4.0, 0.75);
        let (color, _size, _ch, _op) = p.render_values();
        let c = color.expect("keyframe provides color");
        assert_eq!(c, (128, 128, 255), "blue→white midpoint at 75%");
    }

    #[test]
    fn test_keyframe_character_picks_nearest() {
        // At 10% (t=0.2 in 0..0.5 segment) → lower character '*'
        let p = kf_test_particle(4.0, 0.1);
        let (_, _, ch, _) = p.render_values();
        assert_eq!(ch, '*', "low-t picks lower character");
        // At 90% (t=0.8 in 0.5..1.0 segment) → upper character '#'
        let p = kf_test_particle(4.0, 0.9);
        let (_, _, ch, _) = p.render_values();
        assert_eq!(ch, '#', "high-t picks upper character");
    }

    #[test]
    fn test_keyframe_empty_falls_back_to_static_fields() {
        let p = Particle {
            color: Some((10, 20, 30)),
            size: 7,
            character: '@',
            opacity: 100,
            ..Default::default()
        };
        let (color, size, ch, opacity) = p.render_values();
        assert_eq!(color, Some((10, 20, 30)));
        assert_eq!(size, 7);
        assert_eq!(ch, '@');
        assert_eq!(opacity, 100);
    }

    #[test]
    fn test_keyframe_progress_clamps_to_endpoints() {
        let p1 = kf_test_particle(4.0, -0.5);
        let (c, _, _, _) = p1.render_values();
        assert_eq!(c, Some((255, 0, 0)), "negative progress clamps to start");
        let p2 = kf_test_particle(4.0, 2.0);
        let (c, _, _, _) = p2.render_values();
        assert_eq!(
            c,
            Some((255, 255, 255)),
            "progress >1 clamps to final keyframe"
        );
    }

    #[test]
    fn test_keyframe_render_to_canvas_uses_interpolated_color() {
        let mut system = ParticleSystem::new(ParticleConfig {
            spawn_rate: 1.0,
            lifetime_min: 4.0,
            lifetime_max: 4.0,
            emitter_x: 5.0,
            emitter_y: 5.0,
            velocity_x_min: 0.0,
            velocity_x_max: 0.0,
            keyframes: vec![
                ParticleKeyframe {
                    time: 0.0,
                    color: Some((255, 0, 0)),
                    size: 1,
                    character: '*',
                    opacity: 255,
                },
                ParticleKeyframe {
                    time: 1.0,
                    color: Some((0, 0, 255)),
                    size: 1,
                    character: '*',
                    opacity: 255,
                },
            ],
            ..Default::default()
        });
        system.update(1.0, None, None);
        assert_eq!(system.active_count(), 1);
        // At t=1.0 with lifetime=4.0 → progress=0.25 → red→blue linear t=0.25
        // r=255+(0-255)*0.25=191.25→191, b=0+(255-0)*0.25=63.75→64
        let p = &system.active_particles[0];
        let (color, _, _, _) = p.render_values();
        let (r, g, b) = color.unwrap();
        assert_eq!(r, 191);
        assert_eq!(g, 0);
        assert_eq!(b, 64);
    }

    // ── On-death secondary burst tests ─────────────────────────

    #[test]
    fn test_on_death_burst_spawns_secondaries() {
        let sub_cfg = Box::new(ParticleConfig {
            spawn_rate: 0.0,
            lifetime_min: 1.0,
            lifetime_max: 1.0,
            character: '!',
            ..Default::default()
        });
        let mut system = ParticleSystem::new(ParticleConfig {
            spawn_rate: 0.0,
            lifetime_min: 1.0,
            lifetime_max: 1.0,
            on_death_count: 3,
            on_death_config: Some(sub_cfg),
            ..Default::default()
        });
        // Place one primary particle about to expire
        system.active_particles.push(Particle {
            x: 5.0,
            y: 5.0,
            remaining_lifetime: 0.5,
            total_lifetime: 1.0,
            is_secondary: false,
            ..Default::default()
        });
        system.update(1.0, None, None);
        // Primary dies → 3 secondaries spawned
        assert_eq!(system.active_count(), 3, "3 secondaries on primary death");
        for p in &system.active_particles {
            assert!(p.is_secondary, "secondary must be flagged");
            assert_eq!(p.x, 5.0, "secondary inherits parent position x");
            assert_eq!(p.y, 5.0, "secondary inherits parent position y");
            assert_eq!(p.character, '!', "secondary uses sub-config char");
        }
    }

    #[test]
    fn test_on_death_burst_not_recursive() {
        // Sub-config carries its own on_death_count but secondaries never trigger it.
        let sub_cfg = Box::new(ParticleConfig {
            spawn_rate: 0.0,
            lifetime_min: 0.5,
            lifetime_max: 0.5,
            on_death_count: 99, // should be ignored for secondaries
            on_death_config: None,
            ..Default::default()
        });
        let mut system = ParticleSystem::new(ParticleConfig {
            spawn_rate: 0.0,
            on_death_count: 2,
            on_death_config: Some(sub_cfg),
            ..Default::default()
        });
        system.active_particles.push(Particle {
            x: 1.0,
            y: 1.0,
            remaining_lifetime: 0.4,
            total_lifetime: 1.0,
            ..Default::default()
        });
        // Step 1: primary dies → 2 secondaries (life=0.5)
        system.update(1.0, None, None);
        assert_eq!(system.active_count(), 2);
        // Step 2: step again — secondaries have 0.5 remaining and die at next step.
        // No further bursts allowed because secondaries' is_secondary flag guards it.
        system.update(0.55, None, None);
        // At this point secondaries have remaining_lifetime <= 0 → they're removed.
        // Without recursion they should NOT spawn new ones.
        assert_eq!(system.active_count(), 0, "secondaries must not recurse");
    }

    #[test]
    fn test_on_death_disabled_when_count_zero() {
        let mut system = ParticleSystem::new(ParticleConfig {
            spawn_rate: 0.0,
            on_death_count: 0,
            on_death_config: Some(Box::new(ParticleConfig::default())),
            ..Default::default()
        });
        system.active_particles.push(Particle {
            remaining_lifetime: 0.5,
            total_lifetime: 1.0,
            ..Default::default()
        });
        system.update(1.0, None, None);
        assert_eq!(system.active_count(), 0, "no burst when on_death_count = 0");
    }

    #[test]
    fn test_secondary_carries_keyframes() {
        let sub_cfg = Box::new(ParticleConfig {
            spawn_rate: 0.0,
            lifetime_min: 2.0,
            lifetime_max: 2.0,
            keyframes: vec![ParticleKeyframe {
                time: 0.5,
                color: Some((0, 255, 0)),
                size: 2,
                character: 'X',
                opacity: 200,
            }],
            ..Default::default()
        });
        let mut system = ParticleSystem::new(ParticleConfig {
            spawn_rate: 0.0,
            on_death_count: 1,
            on_death_config: Some(sub_cfg),
            ..Default::default()
        });
        system.active_particles.push(Particle {
            x: 2.0,
            y: 3.0,
            remaining_lifetime: 0.4,
            total_lifetime: 1.0,
            ..Default::default()
        });
        system.update(1.0, None, None);
        assert_eq!(system.active_count(), 1);
        let s = &system.active_particles[0];
        assert_eq!(s.total_lifetime, 2.0);
        assert_eq!(s.keyframes.len(), 1);
        assert!(s.is_secondary);
    }
}
