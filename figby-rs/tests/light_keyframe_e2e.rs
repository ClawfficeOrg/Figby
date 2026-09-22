//! End-to-end test for light keyframed animation.
//!
//!     cargo test --manifest-path figby-rs/Cargo.toml --test light_keyframe_e2e -- --nocapture

use figby::font_gen::font_file_to_figfont;
use figby::output::export_cells_to_gif;
use figby::render::render_string;
use figby::tui::canvas::{CanvasBuffer, CanvasCell};
use figby::tui::lighting::{
    self, interpolate_scene, Attenuation, Light, LightKeyframe, LightProperties, Rgb, Scene,
};
use figby::tui::timeline::EasingFunction;

const SD: u16 = 5;

fn out() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("lighting_demos")
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

/// Test 1: Light sweep animation — point light moves left to right,
/// color shifts from warm (orange) to cool (blue) over 20 frames.
#[test]
fn light_sweep_animation() {
    let d = out();
    std::fs::create_dir_all(&d).unwrap();

    let ttf = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/FiraMono-Regular.ttf");
    let charset: Vec<&str> = (32u8..127u8)
        .map(|b| Box::leak(Box::new([b as char].iter().collect::<String>())) as &str)
        .collect();
    let font =
        font_file_to_figfont(&ttf, 48.0, &charset).expect("failed to generate font from TTF");

    let figby_lines = render_string(&font, "HELLO");
    let hf = figlet_hf(&figby_lines);
    let nm = lighting::compute_normal_map_figfont(&hf, 2.0);
    let mk = mk(&hf);
    let (w, h) = (hf[0].len(), hf.len());
    println!("Text size: {w}x{h}");

    // Base scene: ambient + point light
    let base_scene = Scene {
        lights: vec![
            Light::Ambient {
                intensity: 0.3,
                color: Rgb(255, 255, 255),
            },
            Light::Point {
                position: (0.0, h as f32 * 0.5, 20.0),
                intensity: 1.5,
                color: Rgb(255, 200, 80),
                attenuation: Attenuation {
                    constant: 0.5,
                    linear: 0.008,
                    quadratic: 0.0005,
                },
            },
        ],
    };

    // Keyframes: move point light from left to right, shift color
    let keyframes = vec![
        LightKeyframe {
            time: 0.0,
            light_index: 1,
            properties: LightProperties {
                position: Some((2.0, h as f32 * 0.5, 20.0)),
                color: Some(Rgb(255, 180, 60)),
                ..Default::default()
            },
            easing: EasingFunction::Linear,
        },
        LightKeyframe {
            time: 1.0,
            light_index: 1,
            properties: LightProperties {
                position: Some((w as f32 - 2.0, h as f32 * 0.5, 20.0)),
                color: Some(Rgb(80, 160, 255)),
                ..Default::default()
            },
            easing: EasingFunction::Linear,
        },
    ];

    let num_frames = 20;
    let mut sorted_kfs = keyframes.clone();
    lighting::sort_light_keyframes(&mut sorted_kfs);

    let frame_cells: Vec<Vec<Vec<figby::CanvasCell>>> = (0..num_frames)
        .map(|i| {
            let t = i as f32 / (num_frames - 1) as f32;
            let scene = interpolate_scene(&base_scene, &sorted_kfs, t);
            let lum = lighting::shade_canvas(
                &scene,
                &nm,
                |x: u16, y: u16| mk[y as usize][x as usize],
                SD,
            );
            shade_to_cells(&lum, &mk, (200, 200, 255))
        })
        .collect();

    // Verify frames are different (light is moving)
    let frame0_sum: u32 = frame_cells[0]
        .iter()
        .flat_map(|row| row.iter())
        .map(|c| {
            c.fg.map_or(0, |rgb| {
                if let ratatui::style::Color::Rgb(r, g, b) = rgb {
                    r as u32 + g as u32 + b as u32
                } else {
                    0
                }
            })
        })
        .sum();
    let frame10_sum: u32 = frame_cells[10]
        .iter()
        .flat_map(|row| row.iter())
        .map(|c| {
            c.fg.map_or(0, |rgb| {
                if let ratatui::style::Color::Rgb(r, g, b) = rgb {
                    r as u32 + g as u32 + b as u32
                } else {
                    0
                }
            })
        })
        .sum();
    let frame19_sum: u32 = frame_cells[19]
        .iter()
        .flat_map(|row| row.iter())
        .map(|c| {
            c.fg.map_or(0, |rgb| {
                if let ratatui::style::Color::Rgb(r, g, b) = rgb {
                    r as u32 + g as u32 + b as u32
                } else {
                    0
                }
            })
        })
        .sum();

    println!("Frame 0 color sum: {frame0_sum}");
    println!("Frame 10 color sum: {frame10_sum}");
    println!("Frame 19 color sum: {frame19_sum}");

    // Frames should have different color distributions as light moves
    assert_ne!(frame0_sum, frame10_sum, "light should move between frames");
    assert_ne!(frame10_sum, frame19_sum, "light should continue moving");

    // Export as GIF
    let delays: Vec<u16> = vec![5; num_frames];
    let gif_bytes = export_cells_to_gif(&frame_cells, &delays, 1, 0).expect("failed to export GIF");
    let gif_path = d.join("light_sweep_keyframed.gif");
    std::fs::write(&gif_path, &gif_bytes).unwrap();
    let meta = std::fs::metadata(&gif_path).unwrap();
    println!(
        "[light_sweep] -> {} ({} KB)",
        gif_path.display(),
        meta.len() / 1024
    );
    assert!(meta.len() > 100, "GIF should be non-trivial size");
}

/// Test 2: Easing functions — verify EaseIn produces different interpolation
/// than Linear for the same keyframes.
#[test]
fn light_keyframe_easing_comparison() {
    let base = Scene {
        lights: vec![Light::Ambient {
            intensity: 0.0,
            color: Rgb(0, 0, 0),
        }],
    };

    let linear_kfs = vec![
        LightKeyframe {
            time: 0.0,
            light_index: 0,
            properties: LightProperties {
                intensity: Some(0.0),
                ..Default::default()
            },
            easing: EasingFunction::Linear,
        },
        LightKeyframe {
            time: 1.0,
            light_index: 0,
            properties: LightProperties {
                intensity: Some(1.0),
                ..Default::default()
            },
            easing: EasingFunction::Linear,
        },
    ];

    let easein_kfs = vec![
        LightKeyframe {
            time: 0.0,
            light_index: 0,
            properties: LightProperties {
                intensity: Some(0.0),
                ..Default::default()
            },
            easing: EasingFunction::Linear,
        },
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

    let t = 0.5;
    let linear_scene = interpolate_scene(&base, &linear_kfs, t);
    let easein_scene = interpolate_scene(&base, &easein_kfs, t);

    let linear_i = match &linear_scene.lights[0] {
        Light::Ambient { intensity, .. } => *intensity,
        _ => panic!("expected Ambient"),
    };
    let easein_i = match &easein_scene.lights[0] {
        Light::Ambient { intensity, .. } => *intensity,
        _ => panic!("expected Ambient"),
    };

    println!("Linear at t=0.5: {linear_i:.4}");
    println!("EaseIn at t=0.5: {easein_i:.4}");

    // Linear: 0.5, EaseIn: 0.5^3 = 0.125
    assert!((linear_i - 0.5).abs() < 0.01, "linear should be ~0.5");
    assert!((easein_i - 0.125).abs() < 0.01, "ease-in should be ~0.125");
    assert!(easein_i < linear_i, "ease-in should be slower at midpoint");
}

/// Test 3: Figmap roundtrip with light keyframes.
#[test]
fn figmap_roundtrip_with_light_keyframes() {
    use figby::figmap::{self, FigmapKind, FigmapTimeline};
    use figby::tui::layers::{Layer, LayerStack};
    use figby::tui::timeline::TimelineFrame;

    let mut layer = Layer::new(4, 2, "Test".to_string());
    layer.buffer.set(
        0,
        0,
        CanvasCell {
            ch: 'X',
            fg: Some(ratatui::style::Color::Red),
            bg: None,
            height: None,
        },
    );

    let frame = TimelineFrame {
        thumbnail: vec![vec![' '; 4]; 2],
        has_keyframe: true,
        label: "Frame 0".to_string(),
        delay: 10,
        document_state: vec![CanvasBuffer::new(4, 2)],
        layer_keyframes: vec![None],
    };

    let light_kfs = vec![
        LightKeyframe {
            time: 0.0,
            light_index: 0,
            properties: LightProperties {
                intensity: Some(0.3),
                ..Default::default()
            },
            easing: EasingFunction::Linear,
        },
        LightKeyframe {
            time: 0.5,
            light_index: 0,
            properties: LightProperties {
                intensity: Some(1.0),
                ..Default::default()
            },
            easing: EasingFunction::EaseOut,
        },
        LightKeyframe {
            time: 1.0,
            light_index: 0,
            properties: LightProperties {
                intensity: Some(0.3),
                ..Default::default()
            },
            easing: EasingFunction::Linear,
        },
    ];

    let lights = vec![Light::Ambient {
        intensity: 0.5,
        color: Rgb(255, 255, 255),
    }];

    let layers = LayerStack {
        layers: vec![layer],
        active: 0,
        groups: Vec::new(),
        links: Vec::new(),
    };

    let timeline = FigmapTimeline {
        fps: 12,
        loop_enabled: true,
        frames: vec![frame],
        light_keyframes: light_kfs.clone(),
    };

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test_light_kf.figmap");

    figmap::save_figmap(&layers, Some(&timeline), &lights, &[], &path).unwrap();
    let loaded = figmap::load_figmap(&path).unwrap();

    assert_eq!(loaded.kind, FigmapKind::Animation);
    let tl = loaded.timeline.unwrap();
    assert_eq!(tl.light_keyframes.len(), 3);
    assert!((tl.light_keyframes[0].time - 0.0).abs() < 1e-6);
    assert!((tl.light_keyframes[1].time - 0.5).abs() < 1e-6);
    assert!((tl.light_keyframes[2].time - 1.0).abs() < 1e-6);
    assert_eq!(tl.light_keyframes[1].easing, EasingFunction::EaseOut);

    // Verify interpolation works on loaded data
    let base = Scene {
        lights: vec![Light::Ambient {
            intensity: 0.0,
            color: Rgb(0, 0, 0),
        }],
    };
    let scene = interpolate_scene(&base, &tl.light_keyframes, 0.5);
    match &scene.lights[0] {
        Light::Ambient { intensity, .. } => {
            // EaseOut at t=0.5: 1 - (1-0.5)^3 = 0.875
            println!("Loaded keyframe interpolation at t=0.5: intensity={intensity:.4}");
            assert!(*intensity > 0.8, "should be near peak intensity");
        }
        _ => panic!("expected Ambient"),
    }

    println!("[figmap_roundtrip] -> {}", path.display());
}

/// Test 4: Multi-light keyframing — ambient and point lights keyframe
/// independently.
#[test]
fn multi_light_independent_keyframes() {
    let base = Scene {
        lights: vec![
            Light::Ambient {
                intensity: 0.0,
                color: Rgb(0, 0, 0),
            },
            Light::Point {
                position: (10.0, 10.0, 5.0),
                intensity: 0.0,
                color: Rgb(0, 0, 0),
                attenuation: Attenuation::default(),
            },
        ],
    };

    let keyframes = vec![
        // Ambient fades in
        LightKeyframe {
            time: 0.0,
            light_index: 0,
            properties: LightProperties {
                intensity: Some(0.0),
                ..Default::default()
            },
            easing: EasingFunction::Linear,
        },
        LightKeyframe {
            time: 1.0,
            light_index: 0,
            properties: LightProperties {
                intensity: Some(1.0),
                ..Default::default()
            },
            easing: EasingFunction::Linear,
        },
        // Point light moves and brightens
        LightKeyframe {
            time: 0.0,
            light_index: 1,
            properties: LightProperties {
                position: Some((0.0, 0.0, 5.0)),
                intensity: Some(0.2),
                color: Some(Rgb(255, 0, 0)),
                ..Default::default()
            },
            easing: EasingFunction::Linear,
        },
        LightKeyframe {
            time: 1.0,
            light_index: 1,
            properties: LightProperties {
                position: Some((20.0, 20.0, 5.0)),
                intensity: Some(1.0),
                color: Some(Rgb(0, 0, 255)),
                ..Default::default()
            },
            easing: EasingFunction::Linear,
        },
    ];

    let mut sorted = keyframes.clone();
    lighting::sort_light_keyframes(&mut sorted);

    // Sample at t=0.25, 0.5, 0.75
    for t in &[0.25, 0.5, 0.75] {
        let scene = interpolate_scene(&base, &sorted, *t);

        let amb_i = match &scene.lights[0] {
            Light::Ambient { intensity, .. } => *intensity,
            _ => panic!("expected Ambient"),
        };
        let (pt_pos, pt_i, pt_color) = match &scene.lights[1] {
            Light::Point {
                position,
                intensity,
                color,
                ..
            } => (*position, *intensity, *color),
            _ => panic!("expected Point"),
        };

        println!(
            "t={t:.2}: amb={amb_i:.3} pt_pos={pt_pos:?} pt_i={pt_i:.3} pt_color=({},{},{})",
            pt_color.0, pt_color.1, pt_color.2
        );

        // Ambient should increase linearly
        assert!((amb_i - t).abs() < 0.01, "ambient should be ~t");
        // Point intensity should increase linearly
        assert!(
            (pt_i - (0.2 + t * 0.8)).abs() < 0.01,
            "point intensity should be ~0.2 + t*0.8"
        );
        // Position should interpolate
        assert!((pt_pos.0 - t * 20.0).abs() < 0.5, "point x should move");
        // Color should shift from red to blue
        assert!(pt_color.0 < 200, "red should decrease");
        assert!(pt_color.2 > 50, "blue should increase");
    }
}
