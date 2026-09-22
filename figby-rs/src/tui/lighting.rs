use serde::{Deserialize, Serialize};

use super::timeline::EasingFunction;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Normal3(pub i8, pub i8, pub i8);

impl Normal3 {
    pub fn from_f32(x: f32, y: f32, z: f32) -> Self {
        Normal3(
            (x.clamp(-1.0, 1.0) * 127.0) as i8,
            (y.clamp(-1.0, 1.0) * 127.0) as i8,
            (z.clamp(-1.0, 1.0) * 127.0) as i8,
        )
    }

    pub fn to_f32(self) -> (f32, f32, f32) {
        (
            self.0 as f32 / 127.0,
            self.1 as f32 / 127.0,
            self.2 as f32 / 127.0,
        )
    }

    pub fn dot(self, other: Normal3) -> f32 {
        let (x1, y1, z1) = self.to_f32();
        let (x2, y2, z2) = other.to_f32();
        x1 * x2 + y1 * y2 + z1 * z2
    }
}

pub struct NormalMap {
    pub cells: Vec<Vec<Normal3>>,
    pub width: u16,
    pub height: u16,
}

impl NormalMap {
    pub fn new(width: u16, height: u16) -> Self {
        let flat = Normal3(0, 0, 127);
        let cells = vec![vec![flat; width as usize]; height as usize];
        NormalMap {
            cells,
            width,
            height,
        }
    }

    pub fn get(&self, x: u16, y: u16) -> Option<&Normal3> {
        if x < self.width && y < self.height {
            Some(&self.cells[y as usize][x as usize])
        } else {
            None
        }
    }

    pub fn get_mut(&mut self, x: u16, y: u16) -> Option<&mut Normal3> {
        if x < self.width && y < self.height {
            Some(&mut self.cells[y as usize][x as usize])
        } else {
            None
        }
    }

    pub fn set(&mut self, x: u16, y: u16, normal: Normal3) {
        if x < self.width && y < self.height {
            self.cells[y as usize][x as usize] = normal;
        }
    }

    pub fn width(&self) -> u16 {
        self.width
    }

    pub fn height(&self) -> u16 {
        self.height
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Serialize for Rgb {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&format!("#{:02X}{:02X}{:02X}", self.0, self.1, self.2))
    }
}

impl<'de> Deserialize<'de> for Rgb {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        match &value {
            serde_json::Value::String(s) => {
                if let Some(hex) = s.strip_prefix('#') {
                    if hex.len() == 6 {
                        let r =
                            u8::from_str_radix(&hex[0..2], 16).map_err(serde::de::Error::custom)?;
                        let g =
                            u8::from_str_radix(&hex[2..4], 16).map_err(serde::de::Error::custom)?;
                        let b =
                            u8::from_str_radix(&hex[4..6], 16).map_err(serde::de::Error::custom)?;
                        return Ok(Rgb(r, g, b));
                    }
                }
                Err(serde::de::Error::custom(format!(
                    "expected '#RRGGBB' hex string, got: {s}"
                )))
            }
            serde_json::Value::Array(arr) if arr.len() == 3 => {
                let r = arr[0]
                    .as_u64()
                    .ok_or_else(|| serde::de::Error::custom("array element 0 is not a u8"))?
                    as u8;
                let g = arr[1]
                    .as_u64()
                    .ok_or_else(|| serde::de::Error::custom("array element 1 is not a u8"))?
                    as u8;
                let b = arr[2]
                    .as_u64()
                    .ok_or_else(|| serde::de::Error::custom("array element 2 is not a u8"))?
                    as u8;
                Ok(Rgb(r, g, b))
            }
            _ => Err(serde::de::Error::custom(format!(
                "expected '#RRGGBB' hex string or [r,g,b] array, got: {value}"
            ))),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Attenuation {
    pub constant: f32,
    pub linear: f32,
    pub quadratic: f32,
}

impl Default for Attenuation {
    fn default() -> Self {
        Attenuation {
            constant: 1.0,
            linear: 0.09,
            quadratic: 0.032,
        }
    }
}

/// Which channel(s) a light affects.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LightTarget {
    Fg,
    Bg,
    #[default]
    Both,
}

impl LightTarget {
    pub fn applies_fg(&self) -> bool {
        matches!(self, LightTarget::Fg | LightTarget::Both)
    }
    pub fn applies_bg(&self) -> bool {
        matches!(self, LightTarget::Bg | LightTarget::Both)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Light {
    Ambient {
        intensity: f32,
        color: Rgb,
        #[serde(default)]
        target: LightTarget,
    },
    Directional {
        direction: (f32, f32, f32),
        intensity: f32,
        color: Rgb,
        #[serde(default)]
        target: LightTarget,
    },
    Point {
        position: (f32, f32, f32),
        intensity: f32,
        color: Rgb,
        attenuation: Attenuation,
        #[serde(default)]
        target: LightTarget,
    },
}

impl Light {
    pub fn target(&self) -> LightTarget {
        match self {
            Light::Ambient { target, .. }
            | Light::Directional { target, .. }
            | Light::Point { target, .. } => *target,
        }
    }
    pub fn set_target(&mut self, t: LightTarget) {
        match self {
            Light::Ambient { target, .. }
            | Light::Directional { target, .. }
            | Light::Point { target, .. } => *target = t,
        }
    }
    pub fn ambient(intensity: f32, color: Rgb) -> Self {
        Light::Ambient {
            intensity,
            color,
            target: LightTarget::default(),
        }
    }
    pub fn directional(direction: (f32, f32, f32), intensity: f32, color: Rgb) -> Self {
        Light::Directional {
            direction,
            intensity,
            color,
            target: LightTarget::default(),
        }
    }
    pub fn point(
        position: (f32, f32, f32),
        intensity: f32,
        color: Rgb,
        attenuation: Attenuation,
    ) -> Self {
        Light::Point {
            position,
            intensity,
            color,
            attenuation,
            target: LightTarget::default(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Scene {
    pub lights: Vec<Light>,
}

impl Scene {
    pub fn new() -> Self {
        Scene { lights: Vec::new() }
    }

    pub fn add_light(&mut self, light: Light) {
        self.lights.push(light);
    }

    pub fn remove_light(&mut self, index: usize) {
        if index < self.lights.len() {
            self.lights.remove(index);
        }
    }

    pub fn clear_lights(&mut self) {
        self.lights.clear();
    }
}

impl Default for Scene {
    fn default() -> Self {
        Scene::new()
    }
}

// ─── Light Keyframing ────────────────────────────────────────────────

/// Optional overrides for a light's properties at a keyframe.
/// `None` fields use the base light's value.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LightProperties {
    pub position: Option<(f32, f32, f32)>,
    pub direction: Option<(f32, f32, f32)>,
    pub intensity: Option<f32>,
    pub color: Option<Rgb>,
    pub attenuation: Option<Attenuation>,
}

/// A single light keyframe: at `time` (0.0–1.0), override properties of `light_index`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightKeyframe {
    /// Normalized time position within the animation (0.0 = start, 1.0 = end).
    pub time: f32,
    /// Index into `Scene.lights`.
    pub light_index: usize,
    /// Property overrides (None = inherit from base light).
    pub properties: LightProperties,
    /// Easing applied when interpolating from the previous keyframe to this one.
    pub easing: EasingFunction,
}

impl LightKeyframe {
    pub fn new(time: f32, light_index: usize, properties: LightProperties) -> Self {
        Self {
            time: time.clamp(0.0, 1.0),
            light_index,
            properties,
            easing: EasingFunction::Linear,
        }
    }
}

fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn lerp_rgb(a: Rgb, b: Rgb, t: f32) -> Rgb {
    Rgb(
        (a.0 as f32 + (b.0 as f32 - a.0 as f32) * t).round() as u8,
        (a.1 as f32 + (b.1 as f32 - a.1 as f32) * t).round() as u8,
        (a.2 as f32 + (b.2 as f32 - a.2 as f32) * t).round() as u8,
    )
}

fn lerp_tuple3(a: (f32, f32, f32), b: (f32, f32, f32), t: f32) -> (f32, f32, f32) {
    (
        lerp_f32(a.0, b.0, t),
        lerp_f32(a.1, b.1, t),
        lerp_f32(a.2, b.2, t),
    )
}

fn lerp_attenuation(a: &Attenuation, b: &Attenuation, t: f32) -> Attenuation {
    Attenuation {
        constant: lerp_f32(a.constant, b.constant, t),
        linear: lerp_f32(a.linear, b.linear, t),
        quadratic: lerp_f32(a.quadratic, b.quadratic, t),
    }
}

/// Apply property overrides from a keyframe to a base light, returning a new Light.
fn apply_overrides(base: &Light, props: &LightProperties) -> Light {
    match base {
        Light::Ambient {
            intensity,
            color,
            target: _,
        } => Light::Ambient {
            intensity: props.intensity.unwrap_or(*intensity),
            color: props.color.unwrap_or(*color),
            target: LightTarget::default(),
        },
        Light::Directional {
            direction,
            intensity,
            color,
            target: _,
        } => Light::Directional {
            direction: props.direction.unwrap_or(*direction),
            intensity: props.intensity.unwrap_or(*intensity),
            color: props.color.unwrap_or(*color),
            target: LightTarget::default(),
        },
        Light::Point {
            position,
            intensity,
            color,
            attenuation,
            target: _,
        } => Light::Point {
            position: props.position.unwrap_or(*position),
            intensity: props.intensity.unwrap_or(*intensity),
            color: props.color.unwrap_or(*color),
            attenuation: props.attenuation.as_ref().unwrap_or(attenuation).clone(),
            target: LightTarget::default(),
        },
    }
}

/// Interpolate between two lights using properties from `from_props` and `to_props`.
fn interpolate_lights(base: &Light, from: &Light, to: &Light, t: f32) -> Light {
    match (from, to) {
        (
            Light::Ambient {
                intensity: i1,
                color: c1,
                target: _,
            },
            Light::Ambient {
                intensity: i2,
                color: c2,
                target: _,
            },
        ) => Light::Ambient {
            intensity: lerp_f32(*i1, *i2, t),
            color: lerp_rgb(*c1, *c2, t),
            target: LightTarget::default(),
        },
        (
            Light::Directional {
                direction: d1,
                intensity: i1,
                color: c1,
                target: _,
            },
            Light::Directional {
                direction: d2,
                intensity: i2,
                color: c2,
                target: _,
            },
        ) => Light::Directional {
            direction: lerp_tuple3(*d1, *d2, t),
            intensity: lerp_f32(*i1, *i2, t),
            color: lerp_rgb(*c1, *c2, t),
            target: LightTarget::default(),
        },
        (
            Light::Point {
                position: p1,
                intensity: i1,
                color: c1,
                attenuation: a1,
                target: _,
            },
            Light::Point {
                position: p2,
                intensity: i2,
                color: c2,
                attenuation: a2,
                target: _,
            },
        ) => Light::Point {
            position: lerp_tuple3(*p1, *p2, t),
            intensity: lerp_f32(*i1, *i2, t),
            color: lerp_rgb(*c1, *c2, t),
            attenuation: lerp_attenuation(a1, a2, t),
            target: LightTarget::default(),
        },
        _ => base.clone(),
    }
}

/// Given sorted keyframes for a single light and a time `t`, compute the interpolated Light.
/// Falls back to the base light when no keyframes exist for this light.
fn interpolate_single_light(base: &Light, keyframes: &[LightKeyframe], t: f32) -> Light {
    if keyframes.is_empty() {
        return base.clone();
    }

    // If before first keyframe, apply first keyframe's overrides to base
    if t <= keyframes[0].time {
        return apply_overrides(base, &keyframes[0].properties);
    }

    // If after last keyframe, apply last keyframe's overrides to base
    if t >= keyframes.last().unwrap().time {
        return apply_overrides(base, &keyframes.last().unwrap().properties);
    }

    // Find the two surrounding keyframes
    let mut prev = &keyframes[0];
    let mut next = &keyframes[0];
    for kf in keyframes {
        if kf.time <= t {
            prev = kf;
        }
        if kf.time >= t && next.time < kf.time {
            next = kf;
        }
    }

    if prev.time == next.time {
        return apply_overrides(base, &prev.properties);
    }

    // Compute raw [0,1] lerp factor between the two keyframes
    let raw_t = (t - prev.time) / (next.time - prev.time);
    // Apply the destination keyframe's easing
    let eased_t = next.easing.apply(raw_t as f64) as f32;

    // Build intermediate lights by applying each keyframe's overrides to the base
    let from_light = apply_overrides(base, &prev.properties);
    let to_light = apply_overrides(base, &next.properties);

    interpolate_lights(base, &from_light, &to_light, eased_t)
}

/// Build an interpolated Scene at time `t` from base lights and light keyframes.
///
/// Keyframes are grouped by `light_index`. Within each group they must be
/// sorted by `time` (the caller should ensure this — `sort_light_keyframes`
/// does it).
pub fn interpolate_scene(base: &Scene, keyframes: &[LightKeyframe], t: f32) -> Scene {
    let mut result = Scene::new();

    for (idx, light) in base.lights.iter().enumerate() {
        let light_kfs: Vec<&LightKeyframe> = keyframes
            .iter()
            .filter(|kf| kf.light_index == idx)
            .collect();

        if light_kfs.is_empty() {
            result.add_light(light.clone());
        } else {
            // Convert references to owned for the helper
            let owned_kfs: Vec<LightKeyframe> = light_kfs.into_iter().cloned().collect();
            result.add_light(interpolate_single_light(light, &owned_kfs, t));
        }
    }

    result
}

/// Sort light keyframes by time for deterministic interpolation.
pub fn sort_light_keyframes(keyframes: &mut [LightKeyframe]) {
    keyframes.sort_by(|a, b| {
        a.time
            .partial_cmp(&b.time)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

#[derive(Clone, Debug)]
pub struct LutEntry {
    pub fg_color: (u8, u8, u8),
    pub bg_color: Option<(u8, u8, u8)>,
    pub ch: char,
}

/// Per-swatch lighting data for LUT generation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SwatchLightingData {
    pub lit: (u8, u8, u8),
    pub shadow: (u8, u8, u8),
    pub specular: bool,
    pub shininess: f32,
}

const ENTRIES_PER_SWATCH: usize = 256;

pub struct LightingLut {
    pub entries: Vec<LutEntry>,
    pub swatch_count: usize,
}

impl LightingLut {
    pub fn from_palette(
        shadow_color: (u8, u8, u8),
        lit_color: (u8, u8, u8),
        char_map: &str,
    ) -> Self {
        let data = SwatchLightingData {
            lit: lit_color,
            shadow: shadow_color,
            specular: false,
            shininess: 32.0,
        };
        Self::from_swatches(&[data], char_map)
    }

    /// Build a multi-swatch LUT. Each swatch gets 256 entries (shadow→lit ramp).
    pub fn from_swatches(swatches: &[SwatchLightingData], char_map: &str) -> Self {
        let chars: Vec<char> = char_map.chars().collect();
        let mut entries = Vec::with_capacity(swatches.len() * ENTRIES_PER_SWATCH);

        for swatch in swatches {
            for i in 0..ENTRIES_PER_SWATCH {
                let t = i as f32 / (ENTRIES_PER_SWATCH - 1) as f32;
                let r = (swatch.shadow.0 as f32 * (1.0 - t) + swatch.lit.0 as f32 * t) as u8;
                let g = (swatch.shadow.1 as f32 * (1.0 - t) + swatch.lit.1 as f32 * t) as u8;
                let b = (swatch.shadow.2 as f32 * (1.0 - t) + swatch.lit.2 as f32 * t) as u8;
                let ch = if chars.is_empty() {
                    ' '
                } else {
                    let idx = (t * (chars.len() - 1) as f32).round() as usize;
                    chars[idx.min(chars.len() - 1)]
                };
                entries.push(LutEntry {
                    fg_color: (r, g, b),
                    bg_color: None,
                    ch,
                });
            }
        }

        LightingLut {
            entries,
            swatch_count: swatches.len(),
        }
    }

    /// Get entry for a given swatch at a given luminance.
    pub fn get_swatched(&self, luminance: f32, swatch_idx: usize) -> &LutEntry {
        let swatch_idx = swatch_idx.min(self.swatch_count.saturating_sub(1));
        let base = swatch_idx * ENTRIES_PER_SWATCH;
        let idx = ((luminance.clamp(0.0, 1.0)) * (ENTRIES_PER_SWATCH - 1) as f32).round() as usize;
        &self.entries[base + idx.min(ENTRIES_PER_SWATCH - 1)]
    }

    /// Convenience: get entry for swatch 0.
    pub fn get(&self, luminance: f32) -> &LutEntry {
        self.get_swatched(luminance, 0)
    }

    pub fn swatch_count(&self) -> usize {
        self.swatch_count
    }
}

fn mirror_idx(i: i32, limit: usize) -> usize {
    let limit = limit as i32;
    if i < 0 {
        (-i - 1) as usize
    } else if i >= limit {
        (2 * limit - i - 1) as usize
    } else {
        i as usize
    }
}

pub fn compute_normal_map_figfont(heightfield: &[Vec<f32>], height_scale: f32) -> NormalMap {
    let h = heightfield.len();
    if h == 0 {
        return NormalMap::new(0, 0);
    }
    let w = heightfield[0].len();
    let mut map = NormalMap::new(w as u16, h as u16);
    if w < 2 || h < 2 {
        return map;
    }

    let sx_kernel: [[i32; 3]; 3] = [[-1, 0, 1], [-2, 0, 2], [-1, 0, 1]];
    let sy_kernel: [[i32; 3]; 3] = [[-1, -2, -1], [0, 0, 0], [1, 2, 1]];

    for y in 0..h {
        for x in 0..w {
            let mut gx = 0.0f32;
            let mut gy = 0.0f32;
            for ky in 0..3 {
                for kx in 0..3 {
                    let sx = mirror_idx(x as i32 + kx - 1, w);
                    let sy = mirror_idx(y as i32 + ky - 1, h);
                    let hval = heightfield[sy][sx];
                    gx += sx_kernel[ky as usize][kx as usize] as f32 * hval;
                    gy += sy_kernel[ky as usize][kx as usize] as f32 * hval;
                }
            }

            let nx = -gx * height_scale;
            let ny = -gy * height_scale;
            let nz = 1.0f32;
            let len = (nx * nx + ny * ny + nz * nz).sqrt();
            let (nx, ny, nz) = if len > 1e-10 {
                (nx / len, ny / len, nz / len)
            } else {
                (0.0, 0.0, 1.0)
            };
            map.cells[y][x] = Normal3::from_f32(nx, ny, nz);
        }
    }

    map
}

pub fn shade_canvas(
    scene: &Scene,
    normal_map: &NormalMap,
    cell_has_content: impl Fn(u16, u16) -> bool,
    max_shadow_distance: u16,
) -> Vec<Vec<f32>> {
    let height = normal_map.height() as usize;
    let width = normal_map.width() as usize;
    let mut luminance = vec![vec![0.0f32; width]; height];

    for (y, row) in luminance.iter_mut().enumerate().take(height) {
        for (x, cell) in row.iter_mut().enumerate().take(width) {
            let mut total = 0.0f32;
            let normal = normal_map.cells[y][x];

            for light in &scene.lights {
                match light {
                    Light::Ambient {
                        intensity,
                        color: _,
                        target: _,
                    } => {
                        total += intensity;
                    }
                    Light::Directional {
                        direction,
                        intensity,
                        color: _,
                        target: _,
                    } => {
                        let (nx, ny, nz) = normal.to_f32();
                        let ndotl =
                            (nx * direction.0 + ny * direction.1 + nz * direction.2).max(0.0);
                        if ndotl > 0.0 {
                            let shadowed = cast_shadow(
                                x as u16,
                                y as u16,
                                (direction.0, direction.1),
                                &cell_has_content,
                                width as u16,
                                height as u16,
                                max_shadow_distance,
                            );
                            if !shadowed {
                                total += ndotl * intensity;
                            }
                        }
                    }
                    Light::Point {
                        position,
                        intensity,
                        color: _,
                        attenuation,
                        target: _,
                    } => {
                        let lx = position.0 - x as f32;
                        let ly = position.1 - y as f32;
                        let lz = position.2;
                        let dist2 = lx * lx + ly * ly + lz * lz;
                        let dist = dist2.sqrt();
                        let ndotl = if dist > 1e-10 {
                            let (nx, ny, nz) = normal.to_f32();
                            (nx * lx / dist + ny * ly / dist + nz * lz / dist).max(0.0)
                        } else {
                            1.0
                        };
                        if ndotl > 0.0 {
                            let atten = 1.0
                                / (attenuation.constant
                                    + attenuation.linear * dist
                                    + attenuation.quadratic * dist2);
                            let shadow_dir = if dist > 1e-10 {
                                (lx / dist, ly / dist)
                            } else {
                                (0.0, 0.0)
                            };
                            let shadowed = cast_shadow(
                                x as u16,
                                y as u16,
                                shadow_dir,
                                &cell_has_content,
                                width as u16,
                                height as u16,
                                max_shadow_distance,
                            );
                            if !shadowed && dist > 1e-10 {
                                total += ndotl * intensity * atten;
                            }
                        }
                    }
                }
            }

            *cell = total.clamp(0.0, 1.0);
        }
    }

    luminance
}

pub fn cast_shadow(
    cell_x: u16,
    cell_y: u16,
    light_dir: (f32, f32),
    cell_has_content: impl Fn(u16, u16) -> bool,
    canvas_w: u16,
    canvas_h: u16,
    max_distance: u16,
) -> bool {
    if max_distance == 0 {
        return false;
    }

    let (dx, dy) = light_dir;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-6 {
        return false;
    }
    let (dx, dy) = (dx / len, dy / len);

    let rx = cell_x as f32 + 0.5;
    let ry = cell_y as f32 + 0.5;

    let step_x = if dx > 0.0 { 1i32 } else { -1i32 };
    let step_y = if dy > 0.0 { 1i32 } else { -1i32 };

    let t_delta_x = if dx.abs() > 1e-10 {
        1.0 / dx.abs()
    } else {
        f32::MAX
    };
    let t_delta_y = if dy.abs() > 1e-10 {
        1.0 / dy.abs()
    } else {
        f32::MAX
    };

    let mut t_max_x = if dx > 0.0 {
        (cell_x as f32 + 1.0 - rx) / dx.abs()
    } else if dx < 0.0 {
        (rx - cell_x as f32) / dx.abs()
    } else {
        f32::MAX
    };

    let mut t_max_y = if dy > 0.0 {
        (cell_y as f32 + 1.0 - ry) / dy.abs()
    } else if dy < 0.0 {
        (ry - cell_y as f32) / dy.abs()
    } else {
        f32::MAX
    };

    let max_t = max_distance as f32;
    let mut cx = cell_x as i32;
    let mut cy = cell_y as i32;

    loop {
        if t_max_x < t_max_y {
            if t_max_x > max_t {
                break;
            }
            cx += step_x;
            t_max_x += t_delta_x;
        } else if t_max_y < t_max_x {
            if t_max_y > max_t {
                break;
            }
            cy += step_y;
            t_max_y += t_delta_y;
        } else {
            if t_max_x > max_t {
                break;
            }
            cx += step_x;
            cy += step_y;
            t_max_x += t_delta_x;
            t_max_y += t_delta_y;
        }

        if cx < 0 || cx >= canvas_w as i32 || cy < 0 || cy >= canvas_h as i32 {
            return false;
        }
        if cell_has_content(cx as u16, cy as u16) {
            return true;
        }
    }

    false
}

pub fn intensity_to_char(t: f32, char_map: &str) -> char {
    let chars: Vec<char> = char_map.chars().collect();
    if chars.is_empty() {
        return ' ';
    }
    let idx = (t.clamp(0.0, 1.0) * (chars.len() - 1) as f32).round() as usize;
    chars[idx.min(chars.len() - 1)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image_input::DEFAULT_CHAR_MAP;

    #[test]
    fn normal3_roundtrip() {
        let n = Normal3::from_f32(0.5, -0.5, 0.707);
        let (x, y, z) = n.to_f32();
        assert!((x - 0.5).abs() < 0.01, "x={}", x);
        assert!((y + 0.5).abs() < 0.01, "y={}", y);
        assert!((z - 0.707).abs() < 0.01, "z={}", z);
    }

    #[test]
    fn normal3_clamp() {
        let n = Normal3::from_f32(2.0, -2.0, 0.0);
        assert_eq!(n, Normal3(127, -127, 0));
    }

    #[test]
    fn normal3_dot() {
        let a = Normal3::from_f32(1.0, 0.0, 0.0);
        let b = Normal3::from_f32(1.0, 0.0, 0.0);
        assert!((a.dot(b) - 1.0).abs() < 0.01);

        let c = Normal3::from_f32(0.0, 1.0, 0.0);
        assert!((a.dot(c).abs()) < 0.01);
    }

    #[test]
    fn normal_map_new() {
        let nm = NormalMap::new(5, 3);
        assert_eq!(nm.width(), 5);
        assert_eq!(nm.height(), 3);
        assert_eq!(nm.cells.len(), 3);
        assert_eq!(nm.cells[0].len(), 5);
        for row in &nm.cells {
            for cell in row {
                assert_eq!(*cell, Normal3(0, 0, 127));
            }
        }
    }

    #[test]
    fn normal_map_bounds() {
        let nm = NormalMap::new(1, 1);
        assert!(nm.get(0, 0).is_some());
        assert!(nm.get(1, 0).is_none());
        assert!(nm.get(0, 1).is_none());
        assert!(nm.get(2, 0).is_none());
    }

    #[test]
    fn normal_map_set_get() {
        let mut nm = NormalMap::new(2, 2);
        nm.set(1, 1, Normal3(100, 50, 30));
        assert_eq!(nm.get(1, 1), Some(&Normal3(100, 50, 30)));
        assert_eq!(nm.get(0, 0), Some(&Normal3(0, 0, 127)));
    }

    #[test]
    fn compute_normal_map_flat() {
        let heightfield = vec![vec![0.0f32; 3]; 3];
        let nmap = compute_normal_map_figfont(&heightfield, 0.5);
        for row in &nmap.cells {
            for cell in row {
                assert_eq!(*cell, Normal3(0, 0, 127));
            }
        }
    }

    #[test]
    fn compute_normal_map_slope() {
        let mut heightfield = vec![vec![0.0f32; 3]; 3];
        for row in heightfield.iter_mut() {
            for (x, cell) in row.iter_mut().enumerate() {
                *cell = x as f32 / 2.0;
            }
        }
        let nmap = compute_normal_map_figfont(&heightfield, 0.5);
        let n = nmap.get(1, 1).unwrap();
        let (nx, _ny, nz) = n.to_f32();
        assert!(nx < 0.0, "normal should tilt against positive gradient");
        assert!(nz > 0.0, "normal should point upward");
        assert!(_ny.abs() < 0.01, "no cross-gradient expected");
    }

    #[test]
    fn compute_normal_map_step() {
        let heightfield = vec![
            vec![0.0, 0.0, 1.0],
            vec![0.0, 0.0, 1.0],
            vec![0.0, 0.0, 1.0],
        ];
        let nmap = compute_normal_map_figfont(&heightfield, 0.5);
        let n = nmap.get(1, 1).unwrap();
        let (nx, _ny, nz) = n.to_f32();
        assert!(nx < 0.0, "edge normal should point away from step");
        assert!(nz > 0.0, "normal should point upward");
    }

    #[test]
    fn compute_normal_map_non_empty_glyph() {
        let heightfield = vec![
            vec![0.0, 0.0, 0.0, 0.0, 0.0],
            vec![0.0, 1.0, 1.0, 1.0, 0.0],
            vec![0.0, 1.0, 1.0, 1.0, 0.0],
            vec![0.0, 1.0, 1.0, 1.0, 0.0],
            vec![0.0, 0.0, 0.0, 0.0, 0.0],
        ];
        let nmap = compute_normal_map_figfont(&heightfield, 0.5);
        let center = nmap.get(2, 2).unwrap();
        let (_nx, _ny, nz) = center.to_f32();
        assert!(nz > 0.0, "center of raised area should point upward");
        // Edge cells should have normals tilted away from the raised area
        let left_edge = nmap.get(1, 2).unwrap();
        let (lx, _, _) = left_edge.to_f32();
        assert!(
            lx < 0.0,
            "left edge should tilt left (away from step) lx={}",
            lx
        );
        let right_edge = nmap.get(3, 2).unwrap();
        let (rx, _, _) = right_edge.to_f32();
        assert!(
            rx > 0.0,
            "right edge should tilt right (away from step) rx={}",
            rx
        );
    }

    #[test]
    fn light_ambient_only() {
        let scene = Scene {
            lights: vec![Light::Ambient {
                intensity: 0.7,
                color: Rgb(255, 255, 255),
                target: LightTarget::default(),
            }],
        };
        let nmap = NormalMap::new(2, 2);
        let (lum, _) = shade_canvas(&scene, &nmap, |_, _| false, 50);
        for row in &lum {
            for val in row {
                assert!((val - 0.7).abs() < 0.001);
            }
        }
    }

    #[test]
    fn light_directional_lambertian() {
        let scene = Scene {
            lights: vec![Light::Directional {
                direction: (0.0, 0.0, 1.0),
                intensity: 1.0,
                color: Rgb(255, 255, 255),
                target: LightTarget::default(),
            }],
        };
        let nmap = NormalMap::new(1, 1);
        let (lum, _) = shade_canvas(&scene, &nmap, |_, _| false, 50);
        assert!((lum[0][0] - 1.0).abs() < 0.001);
    }

    #[test]
    fn light_point_attenuation() {
        let scene = Scene {
            lights: vec![Light::Point {
                position: (0.0, 0.0, 5.0),
                intensity: 1.0,
                color: Rgb(255, 255, 255),
                attenuation: Attenuation::default(),
                target: LightTarget::default(),
            }],
        };
        let nmap = NormalMap::new(1, 1);
        let (lum, _) = shade_canvas(&scene, &nmap, |_, _| false, 50);
        let expected = 1.0 / (1.0 + 0.09 * 5.0 + 0.032 * 25.0);
        assert!(
            (lum[0][0] - expected).abs() < 0.001,
            "expected {}, got {}",
            expected,
            lum[0][0]
        );
    }

    #[test]
    fn multiple_lights_sum() {
        let scene = Scene {
            lights: vec![
                Light::Ambient {
                    intensity: 0.6,
                    color: Rgb(255, 255, 255),
                    target: LightTarget::default(),
                },
                Light::Ambient {
                    intensity: 0.7,
                    color: Rgb(255, 255, 255),
                    target: LightTarget::default(),
                },
            ],
        };
        let nmap = NormalMap::new(1, 1);
        let (lum, _) = shade_canvas(&scene, &nmap, |_, _| false, 50);
        assert!((lum[0][0] - 1.0).abs() < 0.001);
    }

    #[test]
    fn cast_shadow_no_block() {
        let blocked = cast_shadow(0, 0, (1.0, 0.0), |_, _| false, 10, 10, 20);
        assert!(!blocked);
    }

    #[test]
    fn cast_shadow_blocked() {
        let blocked = cast_shadow(0, 0, (1.0, 0.0), |x, y| x == 2 && y == 0, 10, 10, 20);
        assert!(blocked);
    }

    #[test]
    fn cast_shadow_out_of_bounds() {
        let blocked = cast_shadow(0, 0, (-1.0, 0.0), |_, _| true, 10, 10, 20);
        assert!(!blocked);
    }

    #[test]
    fn cast_shadow_max_distance() {
        let blocked = cast_shadow(0, 0, (1.0, 0.0), |x, _| x == 3, 10, 10, 2);
        assert!(!blocked);
    }

    #[test]
    fn lut_entry_creation() {
        let lut = LightingLut::from_palette((0, 0, 0), (255, 255, 255), DEFAULT_CHAR_MAP);
        assert_eq!(lut.entries.len(), 256);
        assert_eq!(lut.entries[0].fg_color, (0, 0, 0));
        assert_eq!(lut.entries[255].fg_color, (255, 255, 255));
    }

    #[test]
    fn lut_get_luminance() {
        let lut = LightingLut::from_palette((0, 0, 0), (255, 255, 255), DEFAULT_CHAR_MAP);
        assert_eq!(lut.get(0.0).fg_color, (0, 0, 0));
        assert_eq!(lut.get(1.0).fg_color, (255, 255, 255));
    }

    #[test]
    fn intensity_to_char_endpoints() {
        assert_eq!(intensity_to_char(0.0, DEFAULT_CHAR_MAP), ' ');
        assert_eq!(intensity_to_char(1.0, DEFAULT_CHAR_MAP), '@');
    }

    #[test]
    fn intensity_to_char_mid() {
        let c = intensity_to_char(0.5, DEFAULT_CHAR_MAP);
        let cmap: Vec<char> = DEFAULT_CHAR_MAP.chars().collect();
        let mid_idx = ((cmap.len() - 1) as f32 * 0.5).round() as usize;
        assert_eq!(c, cmap[mid_idx]);
    }

    #[test]
    fn intensity_to_char_clamp() {
        assert_eq!(intensity_to_char(1.5, DEFAULT_CHAR_MAP), '@');
    }

    #[test]
    fn test_lut_from_swatches_multiple() {
        let swatches = vec![
            SwatchLightingData {
                lit: (255, 0, 0),
                shadow: (0, 0, 0),
                specular: false,
                shininess: 32.0,
            },
            SwatchLightingData {
                lit: (0, 255, 0),
                shadow: (0, 0, 0),
                specular: false,
                shininess: 32.0,
            },
            SwatchLightingData {
                lit: (0, 0, 255),
                shadow: (0, 0, 0),
                specular: false,
                shininess: 32.0,
            },
        ];
        let lut = LightingLut::from_swatches(&swatches, DEFAULT_CHAR_MAP);
        assert_eq!(lut.swatch_count(), 3);
        assert_eq!(lut.entries.len(), 3 * 256);
        // Same luminance, different swatches should give different colors
        let e0 = lut.get_swatched(1.0, 0);
        let e1 = lut.get_swatched(1.0, 1);
        let e2 = lut.get_swatched(1.0, 2);
        assert_eq!(e0.fg_color, (255, 0, 0));
        assert_eq!(e1.fg_color, (0, 255, 0));
        assert_eq!(e2.fg_color, (0, 0, 255));
    }

    #[test]
    fn test_lut_default_fallbacks() {
        // Swatch with no explicit lit/shadow defaults to shadow=fg*0.3, lit=fg
        // In our model, we pass explicit values, so test that from_palette works as before
        let lut = LightingLut::from_palette((0, 0, 0), (255, 255, 255), DEFAULT_CHAR_MAP);
        assert_eq!(lut.get(0.0).fg_color, (0, 0, 0));
        assert_eq!(lut.get(1.0).fg_color, (255, 255, 255));
        // Midpoint should be 50% gray
        let mid = lut.get(0.5);
        assert!((mid.fg_color.0 as i16 - 127).abs() <= 1);
    }

    #[test]
    fn test_get_swatched_bounds() {
        let swatches = vec![
            SwatchLightingData {
                lit: (255, 255, 255),
                shadow: (0, 0, 0),
                specular: false,
                shininess: 32.0,
            },
            SwatchLightingData {
                lit: (255, 0, 0),
                shadow: (0, 0, 0),
                specular: false,
                shininess: 32.0,
            },
        ];
        let lut = LightingLut::from_swatches(&swatches, DEFAULT_CHAR_MAP);
        // Out-of-range swatch index should clamp to last
        let e = lut.get_swatched(1.0, 5);
        assert_eq!(e.fg_color, (255, 0, 0));
        // Out-of-range luminance should clamp
        let e2 = lut.get_swatched(1.5, 0);
        assert_eq!(e2.fg_color, (255, 255, 255));
        let e3 = lut.get_swatched(-0.5, 0);
        assert_eq!(e3.fg_color, (0, 0, 0));
    }

    // ─── Light keyframing tests ──────────────────────────────────

    #[test]
    fn lerp_f32_basic() {
        assert!((lerp_f32(0.0, 10.0, 0.5) - 5.0).abs() < 1e-6);
        assert!((lerp_f32(0.0, 10.0, 0.0) - 0.0).abs() < 1e-6);
        assert!((lerp_f32(0.0, 10.0, 1.0) - 10.0).abs() < 1e-6);
    }

    #[test]
    fn lerp_rgb_basic() {
        let a = Rgb(0, 0, 0);
        let b = Rgb(255, 128, 64);
        let mid = lerp_rgb(a, b, 0.5);
        assert_eq!(mid.0, 128);
        assert_eq!(mid.1, 64);
        assert_eq!(mid.2, 32);
    }

    #[test]
    fn lerp_attenuation_basic() {
        let a = Attenuation {
            constant: 1.0,
            linear: 0.1,
            quadratic: 0.01,
        };
        let b = Attenuation {
            constant: 2.0,
            linear: 0.2,
            quadratic: 0.02,
        };
        let mid = lerp_attenuation(&a, &b, 0.5);
        assert!((mid.constant - 1.5).abs() < 1e-6);
        assert!((mid.linear - 0.15).abs() < 1e-6);
        assert!((mid.quadratic - 0.015).abs() < 1e-6);
    }

    #[test]
    fn apply_overrides_ambient() {
        let base = Light::Ambient {
            intensity: 0.5,
            color: Rgb(255, 255, 255),
        };
        let props = LightProperties {
            intensity: Some(0.8),
            ..Default::default()
        };
        let result = apply_overrides(&base, &props);
        match result {
            Light::Ambient { intensity, color } => {
                assert!((intensity - 0.8).abs() < 1e-6);
                assert_eq!(color, Rgb(255, 255, 255));
            }
            _ => panic!("expected Ambient"),
        }
    }

    #[test]
    fn apply_overrides_point_color() {
        let base = Light::Point {
            position: (1.0, 2.0, 3.0),
            intensity: 1.0,
            color: Rgb(100, 100, 100),
            attenuation: Attenuation::default(),
        };
        let props = LightProperties {
            color: Some(Rgb(255, 0, 0)),
            ..Default::default()
        };
        let result = apply_overrides(&base, &props);
        match result {
            Light::Point {
                color, position, ..
            } => {
                assert_eq!(color, Rgb(255, 0, 0));
                assert_eq!(position, (1.0, 2.0, 3.0));
            }
            _ => panic!("expected Point"),
        }
    }

    #[test]
    fn interpolate_scene_no_keyframes() {
        let base = Scene {
            lights: vec![Light::Ambient {
                intensity: 0.5,
                color: Rgb(255, 255, 255),
            }],
        };
        let result = interpolate_scene(&base, &[], 0.5);
        assert_eq!(result.lights.len(), 1);
        match &result.lights[0] {
            Light::Ambient { intensity, .. } => assert!((intensity - 0.5).abs() < 1e-6),
            _ => panic!("expected Ambient"),
        }
    }

    #[test]
    fn interpolate_scene_two_keyframes() {
        let base = Scene {
            lights: vec![Light::Ambient {
                intensity: 0.0,
                color: Rgb(0, 0, 0),
            }],
        };
        let kfs = vec![
            LightKeyframe::new(
                0.0,
                0,
                LightProperties {
                    intensity: Some(0.0),
                    ..Default::default()
                },
            ),
            LightKeyframe::new(
                1.0,
                0,
                LightProperties {
                    intensity: Some(1.0),
                    ..Default::default()
                },
            ),
        ];
        let result = interpolate_scene(&base, &kfs, 0.5);
        match &result.lights[0] {
            Light::Ambient { intensity, .. } => assert!((intensity - 0.5).abs() < 1e-6),
            _ => panic!("expected Ambient"),
        }
    }

    #[test]
    fn interpolate_scene_easing() {
        let base = Scene {
            lights: vec![Light::Ambient {
                intensity: 0.0,
                color: Rgb(0, 0, 0),
            }],
        };
        let kfs = vec![
            LightKeyframe::new(
                0.0,
                0,
                LightProperties {
                    intensity: Some(0.0),
                    ..Default::default()
                },
            ),
            LightKeyframe {
                time: 1.0,
                light_index: 0,
                properties: LightProperties {
                    intensity: Some(1.0),
                    ..Default::default()
                },
                easing: EasingFunction::EaseIn,
            },
        ];
        let result = interpolate_scene(&base, &kfs, 0.5);
        match &result.lights[0] {
            Light::Ambient { intensity, .. } => {
                assert!((intensity - 0.125).abs() < 0.01);
            }
            _ => panic!("expected Ambient"),
        }
    }

    #[test]
    fn sort_light_keyframes_orders_by_time() {
        let mut kfs = vec![
            LightKeyframe::new(0.8, 0, LightProperties::default()),
            LightKeyframe::new(0.2, 0, LightProperties::default()),
            LightKeyframe::new(0.5, 0, LightProperties::default()),
        ];
        sort_light_keyframes(&mut kfs);
        assert!((kfs[0].time - 0.2).abs() < 1e-6);
        assert!((kfs[1].time - 0.5).abs() < 1e-6);
        assert!((kfs[2].time - 0.8).abs() < 1e-6);
    }

    #[test]
    fn interpolate_scene_multi_light() {
        let base = Scene {
            lights: vec![
                Light::Ambient {
                    intensity: 0.0,
                    color: Rgb(0, 0, 0),
                },
                Light::Point {
                    position: (0.0, 0.0, 0.0),
                    intensity: 0.0,
                    color: Rgb(0, 0, 0),
                    attenuation: Attenuation::default(),
                },
            ],
        };
        let kfs = vec![
            LightKeyframe::new(
                0.0,
                0,
                LightProperties {
                    intensity: Some(0.0),
                    ..Default::default()
                },
            ),
            LightKeyframe::new(
                1.0,
                0,
                LightProperties {
                    intensity: Some(1.0),
                    ..Default::default()
                },
            ),
            LightKeyframe::new(
                0.0,
                1,
                LightProperties {
                    intensity: Some(0.0),
                    ..Default::default()
                },
            ),
            LightKeyframe::new(
                1.0,
                1,
                LightProperties {
                    intensity: Some(2.0),
                    ..Default::default()
                },
            ),
        ];
        let result = interpolate_scene(&base, &kfs, 0.5);
        assert_eq!(result.lights.len(), 2);
        match &result.lights[0] {
            Light::Ambient { intensity, .. } => assert!((intensity - 0.5).abs() < 1e-6),
            _ => panic!("expected Ambient"),
        }
        match &result.lights[1] {
            Light::Point { intensity, .. } => assert!((intensity - 1.0).abs() < 1e-6),
            _ => panic!("expected Point"),
        }
    }
}
