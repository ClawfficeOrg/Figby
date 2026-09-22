//! Particle effect demo — exports GIFs and plays them in the terminal.
//!
//! Export GIF files:
//!     cargo test --manifest-path figby-rs/Cargo.toml --test particle_demo -- --nocapture
//!
//! The GIFs are saved to `figby-rs/target/particle_demos/`.
//! Open them with any image viewer, or play back in terminal:
//!     figby --play figby-rs/target/particle_demos/rain.gif

use figby::output::export_cells_to_gif;
use figby::tui::canvas::CanvasBuffer;
use figby::tui::particles::{
    EdgeMode, EmissionShape, ParticleConfig, ParticleKeyframe, ParticleSystem,
};

const W: usize = 44;
const H: usize = 20;

// ── CanvasBuffer → Vec<Vec<Vec<CanvasCell>>> conversion ───────────────

fn buffer_to_cells(buf: &CanvasBuffer) -> Vec<Vec<figby::CanvasCell>> {
    let mut rows = Vec::with_capacity(buf.height());
    for y in 0..buf.height() {
        let mut row = Vec::with_capacity(buf.width());
        for x in 0..buf.width() {
            row.push(*buf.get(x, y).unwrap_or(&figby::CanvasCell::default()));
        }
        rows.push(row);
    }
    rows
}

fn buffers_to_frame_cells(buffers: &[CanvasBuffer]) -> Vec<Vec<Vec<figby::CanvasCell>>> {
    buffers.iter().map(buffer_to_cells).collect()
}

// ── Baking helper ────────────────────────────────────────────────────

fn bake(config: ParticleConfig, num_frames: usize, dt: f64) -> Vec<CanvasBuffer> {
    let mut system = ParticleSystem::new(config);
    system.bake_frames(num_frames, W, H, dt)
}

fn export_gif(
    name: &str,
    config: ParticleConfig,
    num_frames: usize,
    dt: f64,
    out_dir: &std::path::Path,
) -> Result<String, Box<dyn std::error::Error>> {
    let frames = bake(config, num_frames, dt);
    let cell_frames = buffers_to_frame_cells(&frames);

    // dt seconds per frame → centiseconds for GIF delay
    let delay_cs = ((dt * 100.0).round() as u16).max(1);
    let delays: Vec<u16> = vec![delay_cs; cell_frames.len()];

    let gif_bytes = export_cells_to_gif(&cell_frames, &delays, 1, 0)?;

    let path = out_dir.join(format!("{name}.gif"));
    std::fs::write(&path, &gif_bytes)?;

    // Also save first and last frames as PNG for quick preview
    if let Some(first) = frames.first() {
        let first_path = out_dir.join(format!("{name}_frame0.png"));
        save_buffer_as_png(first, &first_path)?;
    }
    if let Some(last) = frames.last() {
        let last_path = out_dir.join(format!("{name}_frame_last.png"));
        save_buffer_as_png(last, &last_path)?;
    }

    Ok(path.display().to_string())
}

fn save_buffer_as_png(
    buf: &CanvasBuffer,
    path: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let w = buf.width() as u32;
    let h = buf.height() as u32;
    let mut imgbuf = image::RgbImage::new(w, h);

    for y in 0..buf.height() {
        for x in 0..buf.width() {
            if let Some(cell) = buf.get(x, y) {
                let pixel = match cell.fg {
                    Some(ratatui::style::Color::Rgb(r, g, b)) => image::Rgb([r, g, b]),
                    Some(ratatui::style::Color::White) => image::Rgb([255, 255, 255]),
                    Some(ratatui::style::Color::Red) => image::Rgb([255, 0, 0]),
                    Some(ratatui::style::Color::Green) => image::Rgb([0, 255, 0]),
                    Some(ratatui::style::Color::Blue) => image::Rgb([0, 0, 255]),
                    Some(ratatui::style::Color::Yellow) => image::Rgb([255, 255, 0]),
                    Some(ratatui::style::Color::Cyan) => image::Rgb([0, 255, 255]),
                    Some(ratatui::style::Color::Magenta) => image::Rgb([255, 0, 255]),
                    _ => {
                        if cell.ch != ' ' {
                            image::Rgb([200, 200, 200])
                        } else {
                            image::Rgb([0, 0, 0])
                        }
                    }
                };
                imgbuf.put_pixel(x as u32, y as u32, pixel);
            }
        }
    }
    imgbuf.save(path)?;
    Ok(())
}

// ── Presets ──────────────────────────────────────────────────────────

fn rain_config() -> ParticleConfig {
    ParticleConfig {
        emitter_x: 22.0,
        emitter_y: 0.0,
        spawn_rate: 18.0,
        lifetime_min: 0.6,
        lifetime_max: 1.0,
        velocity_x_min: -0.5,
        velocity_x_max: 0.5,
        velocity_y_min: 8.0,
        velocity_y_max: 14.0,
        acceleration_y: 6.0,
        character: '|',
        opacity: 200,
        emission_shape: EmissionShape::RectWH(40.0, 0.0),
        edge_mode: EdgeMode::Despawn,
        keyframes: vec![
            ParticleKeyframe {
                time: 0.0,
                color: Some((80, 140, 255)),
                character: '|',
                opacity: 255,
                size: 1,
            },
            ParticleKeyframe {
                time: 0.6,
                color: Some((50, 100, 220)),
                character: ':',
                opacity: 180,
                size: 1,
            },
            ParticleKeyframe {
                time: 1.0,
                color: Some((20, 40, 100)),
                character: '.',
                opacity: 60,
                size: 1,
            },
        ],
        ..Default::default()
    }
}

fn fire_config() -> ParticleConfig {
    ParticleConfig {
        emitter_x: 22.0,
        emitter_y: 19.0,
        spawn_rate: 30.0,
        lifetime_min: 0.3,
        lifetime_max: 0.8,
        velocity_x_min: -1.5,
        velocity_x_max: 1.5,
        velocity_y_min: -14.0,
        velocity_y_max: -6.0,
        acceleration_y: 2.0,
        character: '*',
        opacity: 255,
        emission_shape: EmissionShape::RectWH(10.0, 0.0),
        edge_mode: EdgeMode::Despawn,
        keyframes: vec![
            ParticleKeyframe {
                time: 0.0,
                color: Some((255, 255, 180)),
                character: '@',
                opacity: 255,
                size: 1,
            },
            ParticleKeyframe {
                time: 0.25,
                color: Some((255, 180, 30)),
                character: '*',
                opacity: 255,
                size: 1,
            },
            ParticleKeyframe {
                time: 0.5,
                color: Some((255, 80, 0)),
                character: '#',
                opacity: 220,
                size: 1,
            },
            ParticleKeyframe {
                time: 1.0,
                color: Some((80, 10, 0)),
                character: '.',
                opacity: 30,
                size: 1,
            },
        ],
        ..Default::default()
    }
}

fn snow_config() -> ParticleConfig {
    ParticleConfig {
        emitter_x: 22.0,
        emitter_y: 0.0,
        spawn_rate: 8.0,
        lifetime_min: 2.5,
        lifetime_max: 4.0,
        velocity_x_min: -2.0,
        velocity_x_max: 2.0,
        velocity_y_min: 1.5,
        velocity_y_max: 4.0,
        character: '*',
        opacity: 220,
        emission_shape: EmissionShape::RectWH(40.0, 0.0),
        edge_mode: EdgeMode::Bounce,
        keyframes: vec![
            ParticleKeyframe {
                time: 0.0,
                color: Some((255, 255, 255)),
                character: '*',
                opacity: 255,
                size: 1,
            },
            ParticleKeyframe {
                time: 0.5,
                color: Some((200, 210, 255)),
                character: '.',
                opacity: 200,
                size: 1,
            },
            ParticleKeyframe {
                time: 1.0,
                color: Some((150, 160, 200)),
                character: ',',
                opacity: 100,
                size: 1,
            },
        ],
        ..Default::default()
    }
}

fn fountain_config() -> ParticleConfig {
    ParticleConfig {
        emitter_x: 22.0,
        emitter_y: 19.0,
        spawn_rate: 22.0,
        lifetime_min: 0.7,
        lifetime_max: 1.3,
        velocity_x_min: -4.0,
        velocity_x_max: 4.0,
        velocity_y_min: -18.0,
        velocity_y_max: -10.0,
        acceleration_y: 12.0,
        character: '*',
        opacity: 255,
        emission_shape: EmissionShape::Point,
        edge_mode: EdgeMode::Despawn,
        keyframes: vec![
            ParticleKeyframe {
                time: 0.0,
                color: Some((100, 255, 255)),
                character: 'O',
                opacity: 255,
                size: 1,
            },
            ParticleKeyframe {
                time: 0.25,
                color: Some((60, 200, 255)),
                character: '*',
                opacity: 255,
                size: 1,
            },
            ParticleKeyframe {
                time: 0.6,
                color: Some((30, 120, 220)),
                character: '+',
                opacity: 200,
                size: 1,
            },
            ParticleKeyframe {
                time: 1.0,
                color: Some((15, 50, 120)),
                character: '.',
                opacity: 60,
                size: 1,
            },
        ],
        ..Default::default()
    }
}

fn sparkle_trail_config() -> ParticleConfig {
    ParticleConfig {
        emitter_x: 1.0,
        emitter_y: 10.0,
        spawn_rate: 14.0,
        lifetime_min: 0.3,
        lifetime_max: 0.7,
        velocity_x_min: 6.0,
        velocity_x_max: 14.0,
        velocity_y_min: -1.5,
        velocity_y_max: 1.5,
        character: '.',
        opacity: 255,
        emission_shape: EmissionShape::Point,
        edge_mode: EdgeMode::Despawn,
        keyframes: vec![
            ParticleKeyframe {
                time: 0.0,
                color: Some((255, 255, 200)),
                character: '#',
                opacity: 255,
                size: 1,
            },
            ParticleKeyframe {
                time: 0.2,
                color: Some((255, 200, 50)),
                character: '*',
                opacity: 255,
                size: 1,
            },
            ParticleKeyframe {
                time: 0.5,
                color: Some((255, 120, 20)),
                character: '+',
                opacity: 180,
                size: 1,
            },
            ParticleKeyframe {
                time: 1.0,
                color: Some((200, 60, 10)),
                character: '.',
                opacity: 30,
                size: 1,
            },
        ],
        ..Default::default()
    }
}

fn fireworks_burst_config() -> ParticleConfig {
    ParticleConfig {
        spawn_rate: 0.0,
        lifetime_min: 0.8,
        lifetime_max: 1.2,
        acceleration_y: 2.5,
        edge_mode: EdgeMode::Despawn,
        keyframes: vec![
            ParticleKeyframe {
                time: 0.0,
                color: Some((255, 255, 100)),
                character: '@',
                opacity: 255,
                size: 1,
            },
            ParticleKeyframe {
                time: 0.15,
                color: Some((255, 100, 255)),
                character: '*',
                opacity: 255,
                size: 1,
            },
            ParticleKeyframe {
                time: 0.4,
                color: Some((100, 200, 255)),
                character: '+',
                opacity: 200,
                size: 1,
            },
            ParticleKeyframe {
                time: 0.7,
                color: Some((255, 180, 40)),
                character: '.',
                opacity: 120,
                size: 1,
            },
            ParticleKeyframe {
                time: 1.0,
                color: Some((60, 60, 80)),
                character: ' ',
                opacity: 0,
                size: 1,
            },
        ],
        ..Default::default()
    }
}

fn make_fireworks_system() -> ParticleSystem {
    let mut system = ParticleSystem::new(fireworks_burst_config());
    let (cx, cy) = (W as f64 / 2.0, H as f64 / 3.0);
    let count = 50;
    let colors: Vec<(u8, u8, u8)> = vec![
        (255, 100, 100),
        (100, 255, 100),
        (100, 100, 255),
        (255, 255, 100),
        (255, 100, 255),
        (100, 255, 255),
    ];
    for i in 0..count {
        let angle = (i as f64 / count as f64) * std::f64::consts::TAU;
        let speed = 3.0 + (i as f64 % 4.0) * 2.0;
        let color = colors[i % colors.len()];
        system
            .active_particles
            .push(figby::tui::particles::Particle {
                x: cx,
                y: cy,
                vx: angle.cos() * speed,
                vy: angle.sin() * speed - 2.0,
                remaining_lifetime: 0.6 + (i as f64 % 5.0) * 0.15,
                total_lifetime: 1.0,
                color: Some(color),
                character: '*',
                opacity: 255,
                keyframes: system.config.keyframes.clone(),
                ..figby::tui::particles::Particle::default()
            });
    }
    system
}

// ── Tests ────────────────────────────────────────────────────────────

#[test]
fn export_particle_gifs() {
    let out_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("particle_demos");
    std::fs::create_dir_all(&out_dir).unwrap();

    let dt = 1.0 / 20.0; // 20 fps
    let num_frames = 40; // 2 seconds

    let presets: Vec<(&str, ParticleConfig)> = vec![
        ("rain", rain_config()),
        ("fire", fire_config()),
        ("snow", snow_config()),
        ("fountain", fountain_config()),
        ("sparkle_trail", sparkle_trail_config()),
    ];

    for (name, config) in &presets {
        let path = export_gif(name, config.clone(), num_frames, dt, &out_dir)
            .unwrap_or_else(|e| panic!("failed to export {name}: {e}"));
        let meta = std::fs::metadata(&path).unwrap();
        println!("[{name}] → {path} ({} KB)", meta.len() / 1024);
    }

    // Fireworks: manual burst
    let mut fw_system = make_fireworks_system();
    let fw_frames: Vec<CanvasBuffer> = {
        let mut frames = Vec::with_capacity(40);
        for _ in 0..40 {
            fw_system.update(dt, Some((W, H)), None);
            frames.push(fw_system.bake_to_buffer(W, H));
        }
        frames
    };
    let fw_cells = buffers_to_frame_cells(&fw_frames);
    let delay_cs = ((dt * 100.0).round() as u16).max(1);
    let fw_delays: Vec<u16> = vec![delay_cs; fw_cells.len()];
    let fw_bytes = export_cells_to_gif(&fw_cells, &fw_delays, 1, 0).unwrap();
    let fw_path = out_dir.join("fireworks.gif");
    std::fs::write(&fw_path, &fw_bytes).unwrap();
    let meta = std::fs::metadata(&fw_path).unwrap();
    println!(
        "[fireworks] → {} ({} KB)",
        fw_path.display(),
        meta.len() / 1024
    );

    // Summary
    println!("\n--- Done ---");
    println!("GIF files saved to: {}", out_dir.display());
    println!("Play any GIF in terminal:");
    println!("  figby --play {}/rain.gif", out_dir.display());
    println!("  figby --play {}/fire.gif", out_dir.display());
    println!("  figby --play {}/snow.gif", out_dir.display());
    println!("  figby --play {}/fountain.gif", out_dir.display());
    println!("  figby --play {}/sparkle_trail.gif", out_dir.display());
    println!("  figby --play {}/fireworks.gif", out_dir.display());

    // Verify files exist and are non-empty
    for name in &[
        "rain",
        "fire",
        "snow",
        "fountain",
        "sparkle_trail",
        "fireworks",
    ] {
        let p = out_dir.join(format!("{name}.gif"));
        assert!(p.exists(), "missing {name}.gif");
        assert!(
            std::fs::metadata(&p).unwrap().len() > 100,
            "{name}.gif too small"
        );
    }
}
