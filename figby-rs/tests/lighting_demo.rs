//! Lighting demo — FIGlet text at native resolution via font_gen.
//!
//!     cargo test --manifest-path figby-rs/Cargo.toml --test lighting_demo -- --nocapture

use figby::font_gen::font_file_to_figfont;
use figby::output::export_cells_to_gif;
use figby::render::render_string;
use figby::tui::lighting::{
    compute_normal_map_figfont, shade_canvas, Attenuation, Light, LightTarget, Rgb, Scene,
};

const SC: u32 = 2;
const SD: u16 = 5;
const AMB: f32 = 0.30;

fn out() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("lighting_demos")
}

fn save(lum: &[Vec<f32>], mask: &[Vec<bool>], tint: (u8, u8, u8), path: &std::path::Path) {
    let h = lum.len() as u32;
    let w = lum[0].len() as u32;
    let mut img = image::RgbImage::new(w * SC, h * SC);
    for (y, row) in lum.iter().enumerate() {
        for (x, &l) in row.iter().enumerate() {
            let t = if mask[y][x] { l.clamp(0.0, 1.0) } else { 0.0 };
            for dy in 0..SC {
                for dx in 0..SC {
                    img.put_pixel(
                        x as u32 * SC + dx,
                        y as u32 * SC + dy,
                        image::Rgb([
                            (tint.0 as f32 * t) as u8,
                            (tint.1 as f32 * t) as u8,
                            (tint.2 as f32 * t) as u8,
                        ]),
                    );
                }
            }
        }
    }
    img.save(path).unwrap();
}

fn save_split(
    lum: &[Vec<f32>],
    mask: &[Vec<bool>],
    tints: [(u8, u8, u8); 2],
    path: &std::path::Path,
) {
    let h = lum.len() as u32;
    let w = lum[0].len() as u32;
    let mid = w as usize / 2;
    let mut img = image::RgbImage::new(w * SC, h * SC);
    for (y, row) in lum.iter().enumerate() {
        for (x, &l) in row.iter().enumerate() {
            let t = if mask[y][x] { l.clamp(0.0, 1.0) } else { 0.0 };
            let tint = if x < mid { tints[0] } else { tints[1] };
            for dy in 0..SC {
                for dx in 0..SC {
                    img.put_pixel(
                        x as u32 * SC + dx,
                        y as u32 * SC + dy,
                        image::Rgb([
                            (tint.0 as f32 * t) as u8,
                            (tint.1 as f32 * t) as u8,
                            (tint.2 as f32 * t) as u8,
                        ]),
                    );
                }
            }
        }
    }
    img.save(path).unwrap();
}

fn figlet_hf(lines: &[String]) -> Vec<Vec<f32>> {
    let h = lines.len();
    let w = lines.iter().map(|l| l.len()).max().unwrap_or(0);
    if w == 0 || h == 0 {
        return vec![vec![0.0]; 1];
    }
    lines
        .iter()
        .map(|line| {
            (0..w)
                .map(|x| {
                    line.chars()
                        .nth(x)
                        .map_or(0.0, |ch| if ch == ' ' { 0.0 } else { 1.0 })
                })
                .collect()
        })
        .collect()
}

fn mk(hf: &[Vec<f32>]) -> Vec<Vec<bool>> {
    hf.iter()
        .map(|r| r.iter().map(|&v| v > 0.05).collect())
        .collect()
}

fn shade_to_cells(
    lum: &[Vec<f32>],
    mask: &[Vec<bool>],
    tint: (u8, u8, u8),
) -> Vec<Vec<figby::CanvasCell>> {
    lum.iter()
        .enumerate()
        .map(|(y, row)| {
            row.iter()
                .enumerate()
                .map(|(x, &l)| {
                    let t = if mask[y][x] { l.clamp(0.0, 1.0) } else { 0.0 };
                    let r = (tint.0 as f32 * t) as u8;
                    let g = (tint.1 as f32 * t) as u8;
                    let b = (tint.2 as f32 * t) as u8;
                    figby::CanvasCell {
                        ch: if t > 0.1 { '#' } else { ' ' },
                        fg: Some(ratatui::style::Color::Rgb(r, g, b)),
                        bg: Some(ratatui::style::Color::Rgb(10, 10, 18)),
                        height: None,
                    }
                })
                .collect()
        })
        .collect()
}

#[test]
fn export_lighting_pngs() {
    let d = out();
    std::fs::create_dir_all(&d).unwrap();

    // Generate tall FIGlet font from TTF at 48pt
    let ttf = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/FiraMono-Regular.ttf");
    let charset: Vec<&str> = (32u8..127u8)
        .map(|b| {
            // leak each string to get &'static str
            Box::leak(Box::new([b as char].iter().collect::<String>())) as &str
        })
        .collect();
    let font =
        font_file_to_figfont(&ttf, 48.0, &charset).expect("failed to generate font from TTF");
    println!(
        "Generated font: charheight={}, hardblank='{}'",
        font.charheight, font.hardblank
    );

    // Render "FIGby" at native resolution
    let figby_lines = render_string(&font, "FIGby");
    let hf = figlet_hf(&figby_lines);
    let nm = compute_normal_map_figfont(&hf, 2.0);
    let mk = mk(&hf);
    let (w, h) = (hf[0].len(), hf.len());
    println!("FIGby native: {w}x{h}");

    // 1. Warm point
    let mut s = Scene::new();
    s.add_light(Light::Ambient {
        intensity: AMB,
        color: Rgb(255, 255, 255),
        target: LightTarget::default(),
    });
    s.add_light(Light::Point {
        position: (w as f32 * 0.5, h as f32 * 0.5, 25.0),
        intensity: 1.5,
        color: Rgb(255, 200, 80),
        attenuation: Attenuation {
            constant: 0.5,
            linear: 0.008,
            quadratic: 0.0005,
        },
        target: LightTarget::default(),
    });
    let (lum, _) = shade_canvas(&s, &nm, |x: u16, y: u16| mk[y as usize][x as usize], SD);
    save(&lum, &mk, (255, 200, 80), &d.join("01_figby_warm.png"));

    // 2. Cool directional
    let mut s = Scene::new();
    s.add_light(Light::Ambient {
        intensity: AMB,
        color: Rgb(255, 255, 255),
        target: LightTarget::default(),
    });
    s.add_light(Light::Directional {
        direction: (-0.7, -0.7, 0.3),
        intensity: 0.8,
        color: Rgb(255, 255, 255),
        target: LightTarget::default(),
    });
    let (lum, _) = shade_canvas(&s, &nm, |x: u16, y: u16| mk[y as usize][x as usize], SD);
    save(&lum, &mk, (180, 220, 255), &d.join("02_figby_cool.png"));

    // 3. Fire + ice
    let mut s = Scene::new();
    s.add_light(Light::Ambient {
        intensity: AMB,
        color: Rgb(255, 255, 255),
        target: LightTarget::default(),
    });
    s.add_light(Light::Point {
        position: (w as f32 * 0.25, h as f32 * 0.3, 15.0),
        intensity: 1.2,
        color: Rgb(255, 100, 50),
        attenuation: Attenuation {
            constant: 0.5,
            linear: 0.012,
            quadratic: 0.001,
        },
        target: LightTarget::default(),
    });
    s.add_light(Light::Point {
        position: (w as f32 * 0.75, h as f32 * 0.7, 15.0),
        intensity: 1.2,
        color: Rgb(50, 150, 255),
        attenuation: Attenuation {
            constant: 0.5,
            linear: 0.012,
            quadratic: 0.001,
        },
        target: LightTarget::default(),
    });
    let (lum, _) = shade_canvas(&s, &nm, |x: u16, y: u16| mk[y as usize][x as usize], SD);
    save_split(
        &lum,
        &mk,
        [(255, 100, 50), (50, 150, 255)],
        &d.join("03_figby_fire_ice.png"),
    );

    // ── Animated GIF: light sweep ──
    println!("Rendering animated light sweep...");
    let num_frames = 30;
    let frame_cells: Vec<Vec<Vec<figby::CanvasCell>>> = (0..num_frames)
        .map(|i| {
            let t = i as f32 / (num_frames - 1) as f32;
            let light_x = t * w as f32;
            let light_y = h as f32 * 0.5;
            let r = ((1.0 - t) * 255.0 + t * 80.0) as u8;
            let g = ((1.0 - t) * 80.0 + t * 200.0) as u8;
            let b = ((1.0 - t) * 30.0 + t * 255.0) as u8;

            let mut s = Scene::new();
            s.add_light(Light::Ambient {
                intensity: AMB,
                color: Rgb(255, 255, 255),
                target: LightTarget::default(),
            });
            s.add_light(Light::Point {
                position: (light_x, light_y, 20.0),
                intensity: 2.0,
                color: Rgb(r, g, b),
                attenuation: Attenuation {
                    constant: 0.5,
                    linear: 0.006,
                    quadratic: 0.0003,
                },
                target: LightTarget::default(),
            });
            let (lum, _) = shade_canvas(&s, &nm, |x: u16, y: u16| mk[y as usize][x as usize], SD);
            shade_to_cells(&lum, &mk, (r, g, b))
        })
        .collect();

    let delays: Vec<u16> = vec![3; num_frames];
    let gif_bytes = export_cells_to_gif(&frame_cells, &delays, 1, 0).expect("failed to export GIF");
    let gif_path = d.join("04_figby_light_sweep.gif");
    std::fs::write(&gif_path, &gif_bytes).unwrap();

    println!("\nAll saved to {d:?}");
    for e in std::fs::read_dir(&d).unwrap() {
        let e = e.unwrap();
        let meta = e.metadata().unwrap();
        let name = e.path().file_name().unwrap().to_str().unwrap().to_string();
        if name.ends_with(".png") || name.ends_with(".gif") {
            println!("  {name} — {} KB", meta.len() / 1024);
        }
    }
}
