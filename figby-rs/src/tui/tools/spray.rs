use crate::tui::canvas::{CanvasBuffer, CanvasCell};
use rand::Rng;

pub fn spray_stamp(
    buffer: &mut CanvasBuffer,
    cx: i16,
    cy: i16,
    radius: u8,
    density: u8,
    cell: CanvasCell,
    rng: &mut impl Rng,
) {
    if radius == 0 {
        return;
    }
    let r = radius as f64;
    let rr = r * r;
    let r_signed = radius as i16;
    for dy in -r_signed..=r_signed {
        for dx in -r_signed..=r_signed {
            let d2 = (dx as f64).mul_add(dx as f64, (dy as f64) * (dy as f64));
            if d2 > rr {
                continue;
            }
            if !rng.random_bool(density as f64 / 100.0) {
                continue;
            }
            let x = cx.wrapping_add(dx);
            let y = cy.wrapping_add(dy);
            if x >= 0 && y >= 0 {
                if let Some(c) = buffer.get_mut(x as usize, y as usize) {
                    *c = cell;
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn spray_line(
    buffer: &mut CanvasBuffer,
    x0: i16,
    y0: i16,
    x1: i16,
    y1: i16,
    radius: u8,
    density: u8,
    cell: CanvasCell,
    rng: &mut impl Rng,
) {
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut x = x0;
    let mut y = y0;
    loop {
        spray_stamp(buffer, x, y, radius, density, cell, rng);
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

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    fn canvas_20x20() -> CanvasBuffer {
        CanvasBuffer::new(20, 20)
    }

    fn filled_cell() -> CanvasCell {
        CanvasCell {
            ch: '@',
            fg: None,
            bg: None,
            height: None,
        }
    }

    fn count_painted(buf: &CanvasBuffer) -> usize {
        let mut count = 0;
        for y in 0..buf.height() {
            for x in 0..buf.width() {
                if buf.get(x, y).unwrap().ch != ' ' {
                    count += 1;
                }
            }
        }
        count
    }

    #[test]
    fn test_spray_within_circle() {
        let mut buf = canvas_20x20();
        let cell = filled_cell();
        let mut rng = StdRng::seed_from_u64(12345);
        let radius = 5;
        let center: i16 = 10;
        let r = radius as f64;
        let rr = r * r;
        spray_stamp(&mut buf, center, center, radius, 100, cell, &mut rng);
        for y in 0..20 {
            for x in 0..20 {
                let dx = (x as i16 - center) as f64;
                let dy = (y as i16 - center) as f64;
                let d2 = dx * dx + dy * dy;
                let c = buf.get(x, y).unwrap();
                if c.ch != ' ' {
                    assert!(
                        d2 <= rr + 0.001,
                        "painted cell ({},{}) at distance {} outside radius {}",
                        x,
                        y,
                        d2.sqrt(),
                        r
                    );
                }
            }
        }
    }

    #[test]
    fn test_spray_density_distribution() {
        let mut total_painted: usize = 0;
        let radius = 5;
        let r = radius as f64;
        let total_cells_in_circle = {
            let mut count = 0u64;
            let r_signed = radius as i16;
            for dy in -r_signed..=r_signed {
                for dx in -r_signed..=r_signed {
                    let d2 = (dx as f64).mul_add(dx as f64, (dy as f64) * (dy as f64));
                    if d2 <= r * r + 0.001 {
                        count += 1;
                    }
                }
            }
            count
        };
        let stamps = 200;
        for seed in 0..stamps {
            let mut buf = canvas_20x20();
            let cell = filled_cell();
            let mut rng = StdRng::seed_from_u64(seed);
            spray_stamp(&mut buf, 10, 10, radius, 50, cell, &mut rng);
            total_painted += count_painted(&buf);
        }
        let expected_per_stamp = total_cells_in_circle as f64 * 0.5;
        let total_expected = expected_per_stamp * stamps as f64;
        let actual = total_painted as f64;
        let tolerance = total_expected * 0.1;
        assert!(
            (actual - total_expected).abs() < tolerance,
            "density distribution: expected {:.0} ± {:.0}, got {:.0}",
            total_expected,
            tolerance,
            actual
        );
    }

    #[test]
    fn test_spray_stochastic_different() {
        let mut buf_a = canvas_20x20();
        let mut buf_b = canvas_20x20();
        let cell = filled_cell();
        let mut rng_a = StdRng::seed_from_u64(1);
        let mut rng_b = StdRng::seed_from_u64(2);
        spray_stamp(&mut buf_a, 10, 10, 5, 50, cell, &mut rng_a);
        spray_stamp(&mut buf_b, 10, 10, 5, 50, cell, &mut rng_b);
        let a_count = count_painted(&buf_a);
        let b_count = count_painted(&buf_b);
        assert_ne!(
            a_count, b_count,
            "two different seeds should produce different patterns"
        );
    }

    #[test]
    fn test_spray_deterministic_seed() {
        let cell = filled_cell();
        let mut result_a = canvas_20x20();
        let mut rng_a = StdRng::seed_from_u64(42);
        spray_stamp(&mut result_a, 10, 10, 5, 50, cell, &mut rng_a);
        let mut result_b = canvas_20x20();
        let mut rng_b = StdRng::seed_from_u64(42);
        spray_stamp(&mut result_b, 10, 10, 5, 50, cell, &mut rng_b);
        for y in 0..20 {
            for x in 0..20 {
                assert_eq!(
                    result_a.get(x, y).unwrap().ch,
                    result_b.get(x, y).unwrap().ch,
                    "deterministic spray differs at ({},{})",
                    x,
                    y
                );
            }
        }
    }

    #[test]
    fn test_spray_clips_to_bounds() {
        let mut buf = CanvasBuffer::new(5, 5);
        let cell = filled_cell();
        let mut rng = StdRng::seed_from_u64(42);
        // Stamp at (0,0) with radius 5 — lots of out-of-bounds offsets, should not panic
        spray_stamp(&mut buf, 0, 0, 5, 50, cell, &mut rng);
        // No crash means success
    }

    #[test]
    fn test_spray_density_extremes() {
        let mut buf_zero = canvas_20x20();
        let cell = filled_cell();
        let mut rng_zero = StdRng::seed_from_u64(42);
        spray_stamp(&mut buf_zero, 10, 10, 5, 0, cell, &mut rng_zero);
        assert_eq!(
            count_painted(&buf_zero),
            0,
            "density 0 should paint nothing"
        );

        let mut buf_full = canvas_20x20();
        let mut rng_full = StdRng::seed_from_u64(42);
        spray_stamp(&mut buf_full, 10, 10, 5, 100, cell, &mut rng_full);
        let r = 5.0;
        let rr = r * r;
        let mut expected_cells = 0;
        for dy in -5i16..=5 {
            for dx in -5i16..=5 {
                let d2 = (dx as f64).mul_add(dx as f64, (dy as f64) * (dy as f64));
                if d2 <= rr + 0.001 && 10i16 + dx >= 0 && 10i16 + dy >= 0 {
                    let bx = (10i16 + dx) as usize;
                    let by = (10i16 + dy) as usize;
                    if bx < 20 && by < 20 {
                        expected_cells += 1;
                    }
                }
            }
        }
        let painted = count_painted(&buf_full);
        assert_eq!(
            painted, expected_cells,
            "density 100 should paint all {} circle cells, got {}",
            expected_cells, painted
        );
    }
}
