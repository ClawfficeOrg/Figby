use std::collections::HashMap;

use crate::tui::canvas::{CanvasBuffer, CanvasCell};
use crate::tui::layers::LayerStack;
use crate::tui::lighting::{self, LightingLut, Scene, SwatchLightingData};

#[allow(clippy::too_many_arguments)]
pub fn shade_composited(
    composited: &CanvasBuffer,
    layer_stack: &LayerStack,
    scene: &Scene,
    lut: &LightingLut,
    max_shadow_distance: u16,
    height_scale: f32,
    palette_rgb_to_swatch: &HashMap<(u8, u8, u8), usize>,
    swatch_data: &[SwatchLightingData],
) -> CanvasBuffer {
    let w = composited.width();
    let h = composited.height();
    if w == 0 || h == 0 {
        return composited.clone();
    }

    let mut shadow_mask = vec![vec![false; w]; h];
    let mut lighting_mask = vec![vec![false; w]; h];

    for layer in &layer_stack.layers {
        if !layer.visible {
            continue;
        }
        for y in 0..h.min(layer.buffer.height()) {
            for x in 0..w.min(layer.buffer.width()) {
                if let Some(cell) = layer.buffer.get(x, y) {
                    let has_content = cell.ch != ' ' || cell.fg.is_some() || cell.bg.is_some();
                    if has_content {
                        if layer.casts_shadow {
                            shadow_mask[y][x] = true;
                        }
                        if layer.accepts_lighting {
                            lighting_mask[y][x] = true;
                        }
                    }
                }
            }
        }
    }

    let heightfield: Vec<Vec<f32>> = (0..h)
        .map(|y| {
            (0..w)
                .map(|x| {
                    composited
                        .get(x, y)
                        .map_or(0.0, |c| c.height.unwrap_or(0) as f32 / 255.0)
                })
                .collect()
        })
        .collect();

    let normal_map = lighting::compute_normal_map_figfont(&heightfield, height_scale);

    let shadow_check = |x: u16, y: u16| -> bool {
        let (ux, uy) = (x as usize, y as usize);
        ux < w && uy < h && shadow_mask[uy][ux]
    };
    let (fg_luminance, bg_luminance) =
        lighting::shade_canvas(scene, &normal_map, shadow_check, max_shadow_distance);

    // Pre-compute specular contribution per cell
    let specular_luminance: Vec<Vec<f32>> = (0..h)
        .map(|y| {
            (0..w)
                .map(|x| {
                    let normal = normal_map.cells[y][x];
                    compute_specular(normal, scene)
                })
                .collect()
        })
        .collect();

    let mut result = CanvasBuffer::new(w, h);
    for y in 0..h {
        for x in 0..w {
            if lighting_mask[y][x] {
                let cell = composited.get(x, y).copied().unwrap_or_default();
                let swatch_idx = find_swatch_for_cell(&cell, palette_rgb_to_swatch, swatch_data);
                let has_specular = swatch_data
                    .get(swatch_idx)
                    .map(|s| s.specular)
                    .unwrap_or(false);
                let shininess = swatch_data
                    .get(swatch_idx)
                    .map(|s| s.shininess)
                    .unwrap_or(32.0);
                let mut fg_lum = fg_luminance[y][x];
                let mut bg_lum = bg_luminance[y][x];
                if has_specular {
                    let spec_term = specular_luminance[y][x] * shininess.recip();
                    fg_lum = (fg_lum + spec_term * 0.5).min(1.0);
                    bg_lum = (bg_lum + spec_term * 0.5).min(1.0);
                }
                let fg_entry = lut.get_swatched(fg_lum, swatch_idx);
                let bg_entry = lut.get_swatched(bg_lum, swatch_idx);
                result.set(
                    x,
                    y,
                    CanvasCell {
                        ch: fg_entry.ch,
                        fg: Some(ratatui::style::Color::Rgb(
                            fg_entry.fg_color.0,
                            fg_entry.fg_color.1,
                            fg_entry.fg_color.2,
                        )),
                        bg: bg_entry
                            .bg_color
                            .map(|(r, gg, b)| ratatui::style::Color::Rgb(r, gg, b)),
                        height: None,
                    },
                );
            } else if let Some(cell) = composited.get(x, y) {
                result.set(x, y, *cell);
            }
        }
    }

    result
}

fn compute_specular(normal: lighting::Normal3, scene: &Scene) -> f32 {
    let (nx, ny, nz) = normal.to_f32();
    let view_dir = (0.0f32, 0.0f32, -1.0f32);
    let mut total_spec = 0.0f32;

    for light in &scene.lights {
        let light_dir = match light {
            lighting::Light::Directional { direction, .. } => *direction,
            lighting::Light::Point { position, .. } => (position.0, position.1, position.2),
            _ => continue,
        };

        let l_len =
            (light_dir.0 * light_dir.0 + light_dir.1 * light_dir.1 + light_dir.2 * light_dir.2)
                .sqrt();
        if l_len < 1e-6 {
            continue;
        }
        let (lx, ly, lz) = (
            light_dir.0 / l_len,
            light_dir.1 / l_len,
            light_dir.2 / l_len,
        );

        // Blinn-Phong half-vector
        let hx = lx + view_dir.0;
        let hy = ly + view_dir.1;
        let hz = lz + view_dir.2;
        let h_len = (hx * hx + hy * hy + hz * hz).sqrt();
        if h_len < 1e-6 {
            continue;
        }
        let ndoth = (nx * hx / h_len + ny * hy / h_len + nz * hz / h_len).max(0.0);
        total_spec += ndoth;
    }

    total_spec
}

fn find_swatch_for_cell(
    cell: &CanvasCell,
    rgb_to_swatch: &HashMap<(u8, u8, u8), usize>,
    swatch_data: &[SwatchLightingData],
) -> usize {
    if swatch_data.is_empty() {
        return 0;
    }
    if let Some(ratatui::style::Color::Rgb(r, g, b)) = cell.fg {
        if let Some(&idx) = rgb_to_swatch.get(&(r, g, b)) {
            return idx;
        }
        // Fall back to nearest by Euclidean distance
        return nearest_rgb(r, g, b, rgb_to_swatch);
    }
    0
}

fn nearest_rgb(r: u8, g: u8, b: u8, rgb_to_swatch: &HashMap<(u8, u8, u8), usize>) -> usize {
    let mut best = 0usize;
    let mut best_dist = f32::MAX;
    for (&(sr, sg, sb), &idx) in rgb_to_swatch {
        let dr = r as f32 - sr as f32;
        let dg = g as f32 - sg as f32;
        let db = b as f32 - sb as f32;
        let dist = dr * dr + dg * dg + db * db;
        if dist < best_dist {
            best_dist = dist;
            best = idx;
        }
    }
    best
}
