use std::collections::HashMap;

use ratatui::style::Color;

use crate::tui::brush::{dither_char, BrushShape};
use crate::tui::canvas::{CanvasBuffer, CanvasCell};
use crate::tui::palette::ColorTarget;

pub fn stamp_offsets(shape: BrushShape, size: u8) -> Vec<(i16, i16)> {
    if size == 0 {
        return vec![(0, 0)];
    }
    let half = size as i16 / 2;
    let start = -half;
    let end = start + size as i16 - 1;
    match shape {
        BrushShape::Square => square_offsets(start, end),
        BrushShape::Circle => circle_offsets(size, start, end),
        BrushShape::SprayPaint => spray_offsets(size, start, end),
        // Dither stamps a soft disk; erase/marker treat it like a circle.
        BrushShape::Dither => circle_offsets(size, start, end),
        BrushShape::Custom => vec![(0, 0)],
    }
}

fn square_offsets(start: i16, end: i16) -> Vec<(i16, i16)> {
    let mut offsets = Vec::new();
    for dy in start..=end {
        for dx in start..=end {
            offsets.push((dx, dy));
        }
    }
    offsets
}

fn circle_offsets(size: u8, start: i16, end: i16) -> Vec<(i16, i16)> {
    let r = size as f64 / 2.0;
    let mut offsets = Vec::new();
    for dy in start..=end {
        for dx in start..=end {
            let cx = dx as f64 + 0.5;
            let cy = dy as f64 + 0.5;
            if cx * cx + cy * cy <= r * r {
                offsets.push((dx, dy));
            }
        }
    }
    offsets
}

fn spray_offsets(size: u8, start: i16, end: i16) -> Vec<(i16, i16)> {
    let half = size as i16 / 2;
    let seed: u64 = 42;
    let mut offsets = Vec::new();
    for dy in start..=end {
        for dx in start..=end {
            let col = (dx + half) as u64;
            let row = (dy + half) as u64;
            let hash = col.wrapping_mul(7) + row.wrapping_mul(31) + seed;
            if hash % 100 < 35 {
                offsets.push((dx, dy));
            }
        }
    }
    offsets
}

pub fn paint_stamp(
    buffer: &mut CanvasBuffer,
    cx: i16,
    cy: i16,
    shape: BrushShape,
    size: u8,
    cell: CanvasCell,
) {
    if shape == BrushShape::Dither {
        paint_dither_stamp(buffer, cx, cy, size, cell);
        return;
    }
    for (dx, dy) in stamp_offsets(shape, size) {
        let x = cx.wrapping_add(dx);
        let y = cy.wrapping_add(dy);
        if x >= 0 && y >= 0 {
            if let Some(c) = buffer.get_mut(x as usize, y as usize) {
                *c = cell;
            }
        }
    }
}

/// Soft block: center uses the brush char; edges dissolve through ▓▒░.
fn paint_dither_stamp(buffer: &mut CanvasBuffer, cx: i16, cy: i16, size: u8, cell: CanvasCell) {
    if size <= 1 {
        if cx >= 0 && cy >= 0 {
            if let Some(c) = buffer.get_mut(cx as usize, cy as usize) {
                *c = cell;
            }
        }
        return;
    }
    // Offsets are already centered on (0,0); radius = half extent.
    let r = size as f64 / 2.0;
    let half = size as i16 / 2;
    let start = -half;
    let end = start + size as i16 - 1;
    for dy in start..=end {
        for dx in start..=end {
            let fx = dx as f64;
            let fy = dy as f64;
            let dist = (fx * fx + fy * fy).sqrt();
            if dist > r {
                continue;
            }
            let falloff = (1.0 - dist / r).clamp(0.0, 1.0);
            let ch = if falloff > 0.75 {
                cell.ch
            } else {
                dither_char(falloff)
            };
            let x = cx.wrapping_add(dx);
            let y = cy.wrapping_add(dy);
            if x >= 0 && y >= 0 {
                if let Some(c) = buffer.get_mut(x as usize, y as usize) {
                    *c = CanvasCell {
                        ch,
                        fg: cell.fg,
                        bg: cell.bg,
                        height: cell.height,
                    };
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn paint_line(
    buffer: &mut CanvasBuffer,
    x0: i16,
    y0: i16,
    x1: i16,
    y1: i16,
    shape: BrushShape,
    size: u8,
    cell: CanvasCell,
) {
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut x = x0;
    let mut y = y0;
    loop {
        paint_stamp(buffer, x, y, shape, size, cell);
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

pub fn stamp_offsets_with_falloff(shape: BrushShape, size: u8) -> Vec<(i16, i16, f64)> {
    if size == 0 {
        return vec![(0, 0, 1.0)];
    }
    let half = size as i16 / 2;
    let start = -half;
    let end = start + size as i16 - 1;
    match shape {
        BrushShape::Square => square_offsets_falloff(start, end, half),
        BrushShape::Circle => circle_offsets_falloff(size, start, end),
        BrushShape::SprayPaint => spray_offsets_falloff(size, start, end),
        BrushShape::Dither => circle_offsets_falloff(size, start, end),
        BrushShape::Custom => vec![(0, 0, 1.0)],
    }
}

fn square_offsets_falloff(start: i16, end: i16, half: i16) -> Vec<(i16, i16, f64)> {
    let mut offsets = Vec::new();
    let half_f = half.max(1) as f64;
    for dy in start..=end {
        for dx in start..=end {
            let max_norm = dx.abs() as f64 / half_f;
            let max_norm = max_norm.max(dy.abs() as f64 / half_f);
            let falloff = 1.0 - 0.5 * max_norm.clamp(0.0, 1.0);
            offsets.push((dx, dy, falloff));
        }
    }
    offsets
}

fn circle_offsets_falloff(size: u8, start: i16, end: i16) -> Vec<(i16, i16, f64)> {
    let r = size as f64 / 2.0;
    let mut offsets = Vec::new();
    for dy in start..=end {
        for dx in start..=end {
            let cx = dx as f64 + 0.5;
            let cy = dy as f64 + 0.5;
            let dist = (cx * cx + cy * cy).sqrt();
            if dist <= r {
                let falloff = (1.0 - dist / r).clamp(0.0, 1.0);
                offsets.push((dx, dy, falloff));
            }
        }
    }
    offsets
}

fn spray_offsets_falloff(size: u8, start: i16, end: i16) -> Vec<(i16, i16, f64)> {
    let half = size as i16 / 2;
    let seed: u64 = 42;
    let mut offsets = Vec::new();
    for dy in start..=end {
        for dx in start..=end {
            let col = (dx + half) as u64;
            let row = (dy + half) as u64;
            let hash = col.wrapping_mul(7) + row.wrapping_mul(31) + seed;
            if hash % 100 < 35 {
                offsets.push((dx, dy, 1.0));
            }
        }
    }
    offsets
}

fn is_cell_non_empty(cell: &CanvasCell) -> bool {
    cell.ch != ' ' || cell.fg.is_some() || cell.bg.is_some()
}

pub fn accumulate_marker_stamp(
    buffer: &CanvasBuffer,
    cx: i16,
    cy: i16,
    shape: BrushShape,
    size: u8,
    accum: &mut HashMap<(i16, i16), f64>,
) {
    for (dx, dy, falloff) in stamp_offsets_with_falloff(shape, size) {
        let x = cx.wrapping_add(dx);
        let y = cy.wrapping_add(dy);
        if x >= 0 && y >= 0 {
            if let Some(cell) = buffer.get(x as usize, y as usize) {
                if is_cell_non_empty(cell) {
                    *accum.entry((x, y)).or_insert(0.0) += falloff;
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn accumulate_marker_line(
    buffer: &CanvasBuffer,
    x0: i16,
    y0: i16,
    x1: i16,
    y1: i16,
    shape: BrushShape,
    size: u8,
    accum: &mut HashMap<(i16, i16), f64>,
) {
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut x = x0;
    let mut y = y0;
    loop {
        accumulate_marker_stamp(buffer, x, y, shape, size, accum);
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

pub fn commit_marker_accum(
    buffer: &mut CanvasBuffer,
    accum: &mut HashMap<(i16, i16), f64>,
    colors: &[Color],
    target: ColorTarget,
    reverse: bool,
) {
    let mut to_remove: Vec<(i16, i16)> = Vec::new();
    for (&(x, y), amount) in accum.iter_mut() {
        if x < 0 || y < 0 {
            to_remove.push((x, y));
            continue;
        }
        let steps = amount.floor() as usize;
        if steps == 0 {
            continue;
        }
        if let Some(cell) = buffer.get_mut(x as usize, y as usize) {
            let current_color = match target {
                ColorTarget::Foreground => cell.fg,
                ColorTarget::Background => cell.bg,
            };
            let start_idx = current_color.and_then(|c| colors.iter().position(|pc| *pc == c));
            let consumed = if start_idx.is_none() && steps > 0 {
                1
            } else {
                0
            };
            let idx = if reverse {
                start_idx.unwrap_or(colors.len().saturating_sub(1))
            } else {
                start_idx.unwrap_or(0)
            };
            let remaining_steps = steps.saturating_sub(consumed);
            let new_idx = if reverse {
                idx.saturating_sub(remaining_steps)
            } else {
                (idx + remaining_steps).min(colors.len().saturating_sub(1))
            };
            let new_color = Some(colors[new_idx]);
            match target {
                ColorTarget::Foreground => cell.fg = new_color,
                ColorTarget::Background => cell.bg = new_color,
            }
            let actual_steps = consumed + remaining_steps;
            *amount -= actual_steps as f64;
        } else {
            to_remove.push((x, y));
        }
    }
    for key in to_remove {
        accum.remove(&key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::brush::BrushShape;

    fn filled_cell() -> CanvasCell {
        CanvasCell {
            ch: '@',
            fg: None,
            bg: None,
            height: None,
        }
    }

    fn canvas_5x5() -> CanvasBuffer {
        CanvasBuffer::new(5, 5)
    }

    fn canvas_10x10() -> CanvasBuffer {
        CanvasBuffer::new(10, 10)
    }

    fn canvas_30x10() -> CanvasBuffer {
        CanvasBuffer::new(30, 10)
    }

    #[test]
    fn test_stamp_square_covers_correct_cells() {
        let mut buf = canvas_5x5();
        let cell = filled_cell();
        // Square brush size 3 at center (2,2): covers 3x3 block (1..=3, 1..=3)
        paint_stamp(&mut buf, 2, 2, BrushShape::Square, 3, cell);
        for y in 0..5 {
            for x in 0..5 {
                let in_brush = (1..=3).contains(&x) && (1..=3).contains(&y);
                let c = buf.get(x, y).unwrap();
                if in_brush {
                    assert_eq!(c.ch, '@', "cell ({},{}) should be painted", x, y);
                } else {
                    assert_eq!(c.ch, ' ', "cell ({},{}) should be empty", x, y);
                }
            }
        }
    }

    #[test]
    fn test_stamp_circle_round_shape() {
        let mut buf = canvas_10x10();
        let cell = filled_cell();
        // Size-5 circle at (5,5): radius 2.5
        paint_stamp(&mut buf, 5, 5, BrushShape::Circle, 5, cell);
        // Center cell always painted
        assert_eq!(buf.get(5, 5).unwrap().ch, '@');
        // Corner cells of bounding square (3,3), (7,3), (3,7), (7,7) may or may not be painted
        // depending on radius check. Let's just verify that at least the center N cells are filled
        // and some corner cells are empty (circle is not a full square)
        let painted = (0..10)
            .flat_map(|y| (0..10).map(move |x| (x, y)))
            .filter(|&(x, y)| buf.get(x, y).unwrap().ch == '@')
            .count();
        let uncovered = (0..10)
            .flat_map(|y| (0..10).map(move |x| (x, y)))
            .filter(|&(x, y)| buf.get(x, y).unwrap().ch == ' ')
            .count();
        assert!(painted > 0, "circle should paint some cells");
        assert!(uncovered > 0, "circle should leave some cells empty");
    }

    #[test]
    fn test_stamp_spray_deterministic() {
        let mut buf_a = canvas_10x10();
        let mut buf_b = canvas_10x10();
        let cell = filled_cell();
        paint_stamp(&mut buf_a, 5, 5, BrushShape::SprayPaint, 7, cell);
        paint_stamp(&mut buf_b, 5, 5, BrushShape::SprayPaint, 7, cell);
        for y in 0..10 {
            for x in 0..10 {
                assert_eq!(
                    buf_a.get(x, y).unwrap().ch,
                    buf_b.get(x, y).unwrap().ch,
                    "spray at ({},{}) differs between runs",
                    x,
                    y
                );
            }
        }
    }

    #[test]
    fn test_stamp_clips_to_bounds() {
        let mut buf = canvas_5x5();
        let cell = filled_cell();
        // Stamp at (0,0) with size-5 square: offsets -2..=2 in both axes
        // Some cells will be out of bounds (negative x or y) — should not panic
        paint_stamp(&mut buf, 0, 0, BrushShape::Square, 5, cell);
        // Only cells with x>=0 and y>=0 should be painted
        for y in 0..5 {
            for x in 0..5 {
                let in_brush = x <= 2 && y <= 2; // from start..=end where start=-2, end=2
                let c = buf.get(x, y).unwrap();
                if in_brush {
                    assert_eq!(c.ch, '@', "cell ({},{}) should be in-bounds painted", x, y);
                }
            }
        }
    }

    #[test]
    fn test_stamp_applies_cell_attributes() {
        let mut buf = canvas_5x5();
        let cell = CanvasCell {
            ch: '#',
            fg: None,
            bg: None,
            height: None,
        };
        paint_stamp(&mut buf, 2, 2, BrushShape::Square, 3, cell);
        let painted = buf.get(2, 2).unwrap();
        assert_eq!(painted.ch, '#');
    }

    #[test]
    fn test_line_horizontal_no_gaps() {
        let mut buf = canvas_30x10();
        let cell = filled_cell();
        // Horizontal line at y=5 from x=0 to x=20
        paint_line(&mut buf, 0, 5, 20, 5, BrushShape::Square, 1, cell);
        for x in 0..=20 {
            assert_eq!(
                buf.get(x, 5).unwrap().ch,
                '@',
                "horizontal cell ({},{}) should be painted",
                x,
                5
            );
        }
    }

    #[test]
    fn test_line_vertical_no_gaps() {
        let mut buf = canvas_30x10();
        let cell = filled_cell();
        // Vertical line at x=5 from y=0 to y=9
        paint_line(&mut buf, 5, 0, 5, 9, BrushShape::Square, 1, cell);
        for y in 0..=9 {
            assert_eq!(
                buf.get(5, y).unwrap().ch,
                '@',
                "vertical cell ({},{}) should be painted",
                5,
                y
            );
        }
    }

    #[test]
    fn test_line_diagonal_no_gaps() {
        let mut buf = canvas_10x10();
        let cell = filled_cell();
        // Diagonal from (0,0) to (9,9)
        paint_line(&mut buf, 0, 0, 9, 9, BrushShape::Square, 1, cell);
        for i in 0..=9 {
            assert_eq!(
                buf.get(i, i).unwrap().ch,
                '@',
                "diagonal cell ({},{}) should be painted",
                i,
                i
            );
        }
    }

    #[test]
    fn test_line_clips_endpoints() {
        let mut buf = canvas_10x10();
        let cell = filled_cell();
        // Line from (-5,5) to (25,5) — most endpoints are out of bounds
        paint_line(&mut buf, -5, 5, 25, 5, BrushShape::Square, 1, cell);
        // Visible portion x=0..9 should all be painted
        for x in 0..10 {
            assert_eq!(
                buf.get(x, 5).unwrap().ch,
                '@',
                "visible cell ({},{}) should be painted",
                x,
                5
            );
        }
    }

    #[test]
    fn test_stamp_square_size_one() {
        let mut buf = canvas_5x5();
        let cell = filled_cell();
        paint_stamp(&mut buf, 2, 2, BrushShape::Square, 1, cell);
        assert_eq!(buf.get(2, 2).unwrap().ch, '@');
        assert_eq!(buf.get(1, 2).unwrap().ch, ' ');
        assert_eq!(buf.get(2, 1).unwrap().ch, ' ');
        assert_eq!(buf.get(3, 2).unwrap().ch, ' ');
        assert_eq!(buf.get(2, 3).unwrap().ch, ' ');
    }

    #[test]
    fn test_stamp_custom_only_center() {
        let mut buf = canvas_5x5();
        let cell = filled_cell();
        paint_stamp(&mut buf, 2, 2, BrushShape::Custom, 10, cell);
        assert_eq!(buf.get(2, 2).unwrap().ch, '@');
        assert_eq!(buf.get(1, 2).unwrap().ch, ' ');
        assert_eq!(buf.get(2, 1).unwrap().ch, ' ');
    }

    #[test]
    fn test_dither_stamp_soft_edges() {
        let mut buf = canvas_10x10();
        let cell = filled_cell();
        paint_stamp(&mut buf, 5, 5, BrushShape::Dither, 5, cell);
        // Center keeps the brush char.
        assert_eq!(buf.get(5, 5).unwrap().ch, '@');
        // Mid ring is a block-density char, not the brush char.
        let mid = buf.get(5, 3).unwrap().ch;
        assert!(
            matches!(mid, '\u{2593}' | '\u{2592}' | '\u{2591}' | '@'),
            "mid ring should be dither density, got {mid:?}"
        );
        // Far corner outside the disk stays empty.
        assert_eq!(buf.get(0, 0).unwrap().ch, ' ');
    }

    #[test]
    fn test_dither_size_one_is_solid() {
        let mut buf = canvas_5x5();
        let cell = filled_cell();
        paint_stamp(&mut buf, 2, 2, BrushShape::Dither, 1, cell);
        assert_eq!(buf.get(2, 2).unwrap().ch, '@');
        assert_eq!(buf.get(1, 2).unwrap().ch, ' ');
    }

    #[test]
    fn test_dither_char_density_order() {
        assert_eq!(crate::tui::brush::dither_char(1.0), '\u{2588}');
        assert_eq!(crate::tui::brush::dither_char(0.6), '\u{2593}');
        assert_eq!(crate::tui::brush::dither_char(0.4), '\u{2592}');
        assert_eq!(crate::tui::brush::dither_char(0.1), '\u{2591}');
    }

    #[test]
    fn test_line_reverse_direction() {
        let mut buf = canvas_10x10();
        let cell = filled_cell();
        // Line from high to low
        paint_line(&mut buf, 8, 8, 2, 2, BrushShape::Square, 1, cell);
        for i in 2..=8 {
            assert_eq!(
                buf.get(i, i).unwrap().ch,
                '@',
                "reverse-diagonal cell ({},{}) should be painted",
                i,
                i
            );
        }
    }

    // --- Marker brush tests ---

    fn cell_with_fg(fg: Color) -> CanvasCell {
        CanvasCell {
            ch: '@',
            fg: Some(fg),
            bg: None,
            height: None,
        }
    }

    #[test]
    fn test_stamp_falloff_circle_center_1() {
        let offsets = stamp_offsets_with_falloff(BrushShape::Circle, 5);
        let center = offsets.iter().find(|(dx, dy, _)| *dx == 0 && *dy == 0);
        assert!(center.is_some(), "circle size 5 must include center (0,0)");
        let (_, _, falloff) = center.unwrap();
        let diff = (1.0 - falloff).abs();
        assert!(
            diff < 0.3,
            "center falloff should be close to 1.0, got {falloff}"
        );
    }

    #[test]
    fn test_stamp_falloff_circle_edge_0() {
        let offsets = stamp_offsets_with_falloff(BrushShape::Circle, 5);
        // corner of bounding box (2,2) should be outside radius or near-0
        let corner = offsets.iter().find(|(dx, dy, _)| *dx == 2 && *dy == 2);
        assert!(
            corner.is_none(),
            "corner (2,2) should not be in circle size 5"
        );
    }

    #[test]
    fn test_stamp_falloff_square_center_1() {
        let offsets = stamp_offsets_with_falloff(BrushShape::Square, 5);
        let center = offsets.iter().find(|(dx, dy, _)| *dx == 0 && *dy == 0);
        assert!(center.is_some());
        let (_, _, falloff) = center.unwrap();
        assert!(
            (*falloff - 1.0).abs() < f64::EPSILON,
            "square center falloff should be 1.0, got {falloff}"
        );
    }

    #[test]
    fn test_stamp_falloff_square_edge_05() {
        let offsets = stamp_offsets_with_falloff(BrushShape::Square, 5);
        let edge = offsets.iter().find(|(dx, dy, _)| *dx == 2 && *dy == 0);
        assert!(edge.is_some());
        let (_, _, falloff) = edge.unwrap();
        assert!(
            (*falloff - 0.5).abs() < f64::EPSILON,
            "square edge falloff should be 0.5, got {falloff}"
        );
    }

    #[test]
    fn test_accumulate_no_effect_on_empty() {
        let buf = canvas_5x5(); // all cells default (space, no fg/bg)
        let mut accum = HashMap::new();
        accumulate_marker_stamp(&buf, 2, 2, BrushShape::Square, 3, &mut accum);
        assert!(accum.is_empty(), "empty cells should not be accumulated");
    }

    #[test]
    fn test_accumulate_adds_to_existing_cell() {
        let mut buf = canvas_5x5();
        buf.set(
            2,
            2,
            CanvasCell {
                ch: '@',
                fg: None,
                bg: None,
                height: None,
            },
        );
        let mut accum = HashMap::new();
        accumulate_marker_stamp(&buf, 2, 2, BrushShape::Square, 1, &mut accum);
        assert_eq!(accum.len(), 1, "should accumulate the filled cell");
        let amount = accum.get(&(2, 2)).copied().unwrap_or(0.0);
        assert!(
            (amount - 1.0).abs() < f64::EPSILON,
            "size-1 square should have falloff 1.0, got {amount}"
        );
    }

    #[test]
    fn test_commit_steps_color_forward() {
        let mut buf = canvas_5x5();
        let red = Color::Indexed(1);
        let green = Color::Indexed(2);
        let blue = Color::Indexed(3);
        buf.set(2, 2, cell_with_fg(red));
        let colors = vec![red, green, blue];
        let mut accum = HashMap::new();
        accum.insert((2, 2), 1.0);
        commit_marker_accum(
            &mut buf,
            &mut accum,
            &colors,
            ColorTarget::Foreground,
            false,
        );
        let cell = buf.get(2, 2).unwrap();
        assert_eq!(cell.fg, Some(green), "should step from red to green");
        assert!(
            accum.contains_key(&(2, 2)),
            "remaining fraction should stay in accum"
        );
        let remaining = accum.get(&(2, 2)).copied().unwrap_or(0.0);
        assert!(
            (remaining - 0.0).abs() < 1e-10,
            "remaining should be 0.0 after 1 step from 1.0"
        );
    }

    #[test]
    fn test_commit_steps_multiple_positions() {
        let mut buf = canvas_5x5();
        let red = Color::Indexed(1);
        let green = Color::Indexed(2);
        let blue = Color::Indexed(3);
        let yellow = Color::Indexed(4);
        let colors = vec![red, green, blue, yellow];
        buf.set(2, 2, cell_with_fg(red));
        let mut accum = HashMap::new();
        accum.insert((2, 2), 3.0);
        commit_marker_accum(
            &mut buf,
            &mut accum,
            &colors,
            ColorTarget::Foreground,
            false,
        );
        let cell = buf.get(2, 2).unwrap();
        assert_eq!(
            cell.fg,
            Some(yellow),
            "3 steps from red should reach yellow"
        );
    }

    #[test]
    fn test_commit_clamps_at_last_color() {
        let mut buf = canvas_5x5();
        let red = Color::Indexed(1);
        let green = Color::Indexed(2);
        let blue = Color::Indexed(3);
        let colors = vec![red, green, blue];
        buf.set(2, 2, cell_with_fg(red));
        let mut accum = HashMap::new();
        accum.insert((2, 2), 10.0);
        commit_marker_accum(
            &mut buf,
            &mut accum,
            &colors,
            ColorTarget::Foreground,
            false,
        );
        let cell = buf.get(2, 2).unwrap();
        assert_eq!(cell.fg, Some(blue), "should clamp at last color (blue)");
    }

    #[test]
    fn test_commit_preserves_fractional_remainder() {
        let mut buf = canvas_5x5();
        let red = Color::Indexed(1);
        let green = Color::Indexed(2);
        let colors = vec![red, green];
        buf.set(2, 2, cell_with_fg(red));
        let mut accum = HashMap::new();
        accum.insert((2, 2), 2.7);
        commit_marker_accum(
            &mut buf,
            &mut accum,
            &colors,
            ColorTarget::Foreground,
            false,
        );
        let cell = buf.get(2, 2).unwrap();
        assert_eq!(
            cell.fg,
            Some(green),
            "2 steps from red should land on green"
        );
        let remaining = accum.get(&(2, 2)).copied().unwrap_or(0.0);
        assert!(
            (remaining - 0.7).abs() < 1e-10,
            "0.7 should remain after 2 steps from 2.7, got {remaining}"
        );
    }

    #[test]
    fn test_commit_no_color_match_starts_from_0() {
        let mut buf = canvas_5x5();
        let cyan = Color::Indexed(6);
        let red = Color::Indexed(1);
        let green = Color::Indexed(2);
        let colors = vec![red, green];
        // cell has cyan, not in the color array
        buf.set(2, 2, cell_with_fg(cyan));
        let mut accum = HashMap::new();
        accum.insert((2, 2), 1.0);
        commit_marker_accum(
            &mut buf,
            &mut accum,
            &colors,
            ColorTarget::Foreground,
            false,
        );
        let cell = buf.get(2, 2).unwrap();
        assert_eq!(cell.fg, Some(red), "no match should start from color[0]");
    }

    #[test]
    fn test_commit_bg_target() {
        let mut buf = canvas_5x5();
        let red = Color::Indexed(1);
        let green = Color::Indexed(2);
        let colors = vec![red, green];
        buf.set(
            2,
            2,
            CanvasCell {
                ch: '@',
                fg: None,
                bg: Some(red),
                height: None,
            },
        );
        let mut accum = HashMap::new();
        accum.insert((2, 2), 1.0);
        commit_marker_accum(
            &mut buf,
            &mut accum,
            &colors,
            ColorTarget::Background,
            false,
        );
        let cell = buf.get(2, 2).unwrap();
        assert_eq!(cell.bg, Some(green), "should step background forward");
    }

    #[test]
    fn test_accumulate_line_multiple_cells() {
        let mut buf = canvas_10x10();
        // fill a row of cells
        for x in 0..10 {
            buf.set(
                x,
                5,
                CanvasCell {
                    ch: '@',
                    fg: None,
                    bg: None,
                    height: None,
                },
            );
        }
        let mut accum = HashMap::new();
        accumulate_marker_line(&buf, 0, 5, 9, 5, BrushShape::Square, 1, &mut accum);
        assert_eq!(
            accum.len(),
            10,
            "should accumulate all 10 cells on the line"
        );
    }

    #[test]
    fn test_accumulate_skips_empty_cells_along_line() {
        let mut buf = canvas_10x10();
        // only fill cell at (5,5)
        buf.set(
            5,
            5,
            CanvasCell {
                ch: '@',
                fg: None,
                bg: None,
                height: None,
            },
        );
        let mut accum = HashMap::new();
        accumulate_marker_line(&buf, 0, 5, 9, 5, BrushShape::Square, 1, &mut accum);
        assert_eq!(
            accum.len(),
            1,
            "only the one filled cell should be accumulated"
        );
        assert!(
            accum.contains_key(&(5, 5)),
            "the filled cell (5,5) must be in accum"
        );
    }
}
