//! The playback programs behind `figby play` / `figby bake` (and the
//! legacy `--play`, `--bake`, `--play-daemon`, `--play-tmux` and
//! `--play-detach` flags). Moved out of `main()` verbatim (review A2).

use crate::CliArgs;
use std::process;

/// Bake a project into a flat frame sequence.
///
/// Runs before any text rendering: --bake is a whole-pipeline mode, not a
/// rendering option. Resolves layers, Text-tool blocks and lighting to the
/// pixels the editor's exporter produces, then writes a .figseq that
/// `figby --play` can stream without any of that machinery.
pub(crate) fn run_bake(args: &CliArgs, path: &str, hold_last_cs: u16) {
    let out = match args.output_path.as_deref() {
        Some(o) => o,
        None => {
            eprintln!("--bake requires --output <path.figseq>");
            process::exit(2);
        }
    };
    let out_path = std::path::Path::new(out);
    let intent = figby::figseq::PlaybackIntent {
        loop_enabled: args.play_loop,
        hold_last_frame_cs: hold_last_cs,
    };
    match figby::figseq::bake_figmap(std::path::Path::new(path), Some(out_path), intent) {
        Ok(seq) => {
            let mut notes = String::new();
            if seq.loop_enabled {
                notes.push_str(", looping");
            }
            if seq.hold_last_frame_cs > 0 {
                notes.push_str(&format!(
                    ", holds last frame {:.1}s",
                    seq.hold_last_frame_cs as f64 / 100.0
                ));
            }
            eprintln!(
                "Baked {} frame{} ({}x{}) -> {} ({:.1}s{})",
                seq.frames.len(),
                if seq.frames.len() == 1 { "" } else { "s" },
                seq.width,
                seq.height,
                out,
                seq.total_duration_cs() as f64 / 100.0,
                notes
            );
        }
        Err(e) => {
            eprintln!("Bake error: {e}");
            process::exit(1);
        }
    }
}

/// `--play`: a .figseq, .figmap or animated GIF, chosen by extension.
pub(crate) fn run_play(args: &CliArgs, path: &str, hold_last_cs: u16) {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    if ext.eq_ignore_ascii_case(figby::figseq::FIGSEQ_EXTENSION) {
        // ─── .figseq playback ───────────────────────────────────
        let seq = match figby::figseq::load_figseq(std::path::Path::new(path)) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Error loading figseq '{path}': {e}");
                process::exit(1);
            }
        };
        // A single-frame sequence is a still, not an animation: print it inline the way
        // a static figmap does, so it works through a pipe. Anything longer
        // goes to the timed player (which needs a tty).
        let cells = seq.cells_for_playback();
        if cells.len() == 1 {
            print!("{}", figby::output::export_cells_to_ansi(&cells[0]));
            return;
        }
        // Otherwise: hand the flat, per-frame-timed screens straight to the
        // same engine the GIF path drives. Per-frame delays ride along; fps
        // is only the fallback for frames without one.
        //
        // `--loop` and `--hold-last` are overrides: a flag beats the value
        // the sequence carries, so a baked hold applies by default but
        // can be changed (or dropped, with `--hold-last 0`) per playback.
        let hold = if hold_last_cs > 0 {
            hold_last_cs
        } else {
            seq.hold_last_frame_cs
        };
        if let Err(e) = figby::tui::player::play_raw_timed(
            cells,
            seq.fps_fallback(),
            Some(seq.frame_delays()),
            figby::tui::player::PlayOptions {
                loop_playback: args.play_loop || seq.loop_enabled,
                inline: args.play_inline,
                show_timeline: args.play_timeline,
                hold_last_frame_cs: hold,
            },
        ) {
            eprintln!("Playback error: {e}");
            process::exit(1);
        }
        return;
    }

    if ext.eq_ignore_ascii_case("figmap") {
        // ─── .figmap playback ───────────────────────────────────
        let figmap = match figby::figmap::load_figmap(std::path::Path::new(path)) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Error loading figmap '{path}': {e}");
                process::exit(1);
            }
        };
        let (layers, timeline, lights, _palette, text_blocks) = figby::figmap::into_runtime(figmap);
        // Static figmap (kind=Image, no timeline frames): not an error —
        // display the still. Plain stdout print via the ANSI exporter:
        // no screen clear, no raw mode, cursor stays put, so this is
        // inline by default.
        let timeline = match timeline {
            Some(t) if !t.frames.is_empty() => t,
            _ => {
                let composite = layers.composite();
                let (w, h) = (composite.width(), composite.height());
                let cells: Vec<Vec<figby::CanvasCell>> = (0..h)
                    .map(|y| {
                        (0..w)
                            .map(|x| composite.get(x, y).copied().unwrap_or_default())
                            .collect()
                    })
                    .collect();
                print!("{}", figby::output::export_cells_to_ansi(&cells));
                return;
            }
        };
        let w = layers
            .layers
            .first()
            .map(|l| l.buffer.width())
            .unwrap_or(80);
        let h = layers
            .layers
            .first()
            .map(|l| l.buffer.height())
            .unwrap_or(24);

        // Build frames with light keyframing
        let mut sorted_kfs = timeline.light_keyframes.clone();
        figby::tui::lighting::sort_light_keyframes(&mut sorted_kfs);

        let scene = if lights.is_empty() {
            figby::tui::lighting::Scene::new()
        } else {
            figby::tui::lighting::Scene {
                lights: lights.clone(),
            }
        };

        let frame_cells: Vec<Vec<Vec<figby::CanvasCell>>> = timeline
            .frames
            .iter()
            .enumerate()
            .map(|(frame_idx, frame)| {
                // Use committed document_state if available
                let composite = if !frame.document_state.is_empty() {
                    // Composite layers from document_state with keyframe transforms
                    let mut buf = figby::tui::canvas::CanvasBuffer::new(w, h);
                    for (layer_idx, state_buf) in frame.document_state.iter().enumerate() {
                        let props = timeline.get_interpolated_properties(frame_idx, layer_idx);
                        if props.opacity == 0 {
                            continue;
                        }
                        let (ox, oy) = props.position_offset;
                        for y in 0..h.min(state_buf.height()) {
                            for x in 0..w.min(state_buf.width()) {
                                let bx = (x as i16 + ox) as usize;
                                let by = (y as i16 + oy) as usize;
                                if bx >= w || by >= h {
                                    continue;
                                }
                                if let Some(top) = state_buf.get(x, y) {
                                    if top.ch == ' ' && top.fg.is_none() && top.bg.is_none() {
                                        continue;
                                    }
                                    buf.set(bx, by, *top);
                                }
                            }
                        }
                    }
                    buf
                } else {
                    // Use live layers
                    layers.composite()
                };
                // Bake persisted text blocks (non-destructive: CLI only
                // reads them). Without this, --play on a figmap with
                // editable text shows only the layer pixels.
                let composite = {
                    let mut buf = composite;
                    for block in &text_blocks {
                        block.bake_into(&mut buf);
                    }
                    buf
                };

                // Apply light shading if keyframes exist
                let shaded = if !sorted_kfs.is_empty() && !scene.lights.is_empty() {
                    let total = timeline.frames.len().max(1) as f32;
                    let t = frame_idx as f32 / total;
                    let interpolated =
                        figby::tui::lighting::interpolate_scene(&scene, &sorted_kfs, t);
                    let nmap = figby::tui::lighting::compute_normal_map_figfont(
                        &heightfield_from_composite(&composite),
                        0.5,
                    );
                    let (fg_lum, bg_lum) = figby::tui::lighting::shade_canvas_heightfield(
                        &interpolated,
                        &nmap,
                        |x: u16, y: u16| {
                            composite
                                .get(x as usize, y as usize)
                                .filter(|c| c.ch != ' ' || c.fg.is_some() || c.bg.is_some())
                                .map(|c| {
                                    c.height.unwrap_or(0) as f32 / 255.0
                                        * figby::tui::lighting::SHADOW_HEIGHT_UNITS
                                })
                        },
                        50,
                    );
                    apply_luminance_to_composite(&composite, &fg_lum, &bg_lum)
                } else {
                    composite
                };

                // Convert to cell grid
                (0..h)
                    .map(|y| {
                        (0..w)
                            .map(|x| shaded.get(x, y).copied().unwrap_or_default())
                            .collect()
                    })
                    .collect()
            })
            .collect();

        let delays: Vec<u16> = timeline.frames.iter().map(|f| f.delay).collect();
        let fps = 100u16
            .checked_div(delays.first().copied().unwrap_or(10).max(1))
            .map(|f| f.clamp(1, 60) as u8)
            .unwrap_or(10);
        if let Err(e) = figby::tui::player::play_raw_timed(
            frame_cells,
            fps,
            Some(delays),
            figby::tui::player::PlayOptions {
                loop_playback: args.play_loop,
                inline: args.play_inline,
                show_timeline: args.play_timeline,
                hold_last_frame_cs: hold_last_cs,
            },
        ) {
            eprintln!("Playback error: {e}");
            process::exit(1);
        }
        return;
    }

    // ─── GIF playback (existing) ──────────────────────────────
    let scale = match args.play_width {
        Some(w) => figby::gif_import::GifScaleTarget::FitWidth(w),
        None => {
            // Fit within the current terminal, reserving 1 row for the
            // playback progress bar. Falls back to a common default if
            // the terminal size can't be queried (e.g. not a real tty).
            let (cols, rows) = ratatui::crossterm::terminal::size().unwrap_or((80, 24));
            figby::gif_import::GifScaleTarget::FitBox(
                cols as usize,
                (rows as usize).saturating_sub(1).max(1),
            )
        }
    };
    let gif_result = match figby::gif_import::import_gif_scaled(std::path::Path::new(path), scale) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error importing GIF '{path}': {e}");
            process::exit(1);
        }
    };
    // Per-frame delays from the source GIF are passed through so
    // playback honors its real (possibly variable) timing instead of a
    // uniform FPS approximation (GPT review F-26). `fps` below remains
    // the fallback cadence for frames without an explicit delay.
    let first_delay_cs = gif_result.frame_delays.first().copied().unwrap_or(10);
    let fps = 100u16
        .checked_div(first_delay_cs.max(1))
        .map(|f| f.clamp(1, 60) as u8)
        .unwrap_or(10);
    if let Err(e) = figby::tui::player::play_raw_timed(
        gif_result.frames,
        fps,
        Some(gif_result.frame_delays),
        figby::tui::player::PlayOptions {
            loop_playback: args.play_loop,
            inline: args.play_inline,
            show_timeline: args.play_timeline,
            hold_last_frame_cs: hold_last_cs,
        },
    ) {
        eprintln!("Playback error: {e}");
        process::exit(1);
    }
}

/// `--play-daemon`, `--play-tmux` and `--play-detach`.
///
/// Each re-runs this binary with `--play <path>` plus every playback
/// option the user gave (they used to forward only `--loop`).
///
/// Returns `true` when one of these modes was requested (and has run).
pub(crate) fn run_spawned_playback(args: &CliArgs) -> bool {
    let spawn_player = |path: &str| {
        let exe = std::env::current_exe().unwrap_or_else(|_| "figby".into());
        let mut cmd = std::process::Command::new(exe);
        cmd.args(playback_child_args(args, path));
        cmd
    };
    let start_daemon = |path: &str| match spawn_player(path).spawn() {
        Ok(child) => println!("figby playback started (PID {})", child.id()),
        Err(e) => {
            eprintln!("Failed to start daemon: {e}");
            process::exit(1);
        }
    };

    if let Some(ref path) = args.play_daemon {
        start_daemon(path);
        return true;
    }

    if let Some(ref path) = args.play_tmux {
        if std::env::var("TMUX").is_ok() {
            let exe = std::env::current_exe().unwrap_or_else(|_| "figby".into());
            // tmux runs the pane command through the user's shell, so every
            // word is single-quoted. The path used to be interpolated inside
            // double quotes, where `$(...)`, backticks and a stray `"` all
            // still execute — a file named `$(cmd).gif` ran `cmd`.
            let mut words = vec![shell_quote(&exe.to_string_lossy())];
            words.extend(
                playback_child_args(args, path)
                    .iter()
                    .map(|a| shell_quote(a)),
            );
            let play_cmd = words.join(" ");
            let size_pct = args.play_tmux_size.unwrap_or(50).clamp(10, 90);
            // `-l N%` is the tmux >= 3.1 spelling; 3.4 rejects the old
            // `-pN` outright ("size missing"), which silently forced the
            // daemon fallback. Older tmux only knows `-p N`.
            let split = |size: [&str; 2]| {
                std::process::Command::new("tmux")
                    .args(["split-window", "-h", size[0], size[1], &play_cmd])
                    .stderr(std::process::Stdio::null())
                    .status()
            };
            let pct = size_pct.to_string();
            let status = match split(["-l", &format!("{pct}%")]) {
                Ok(s) if s.success() => Ok(s),
                _ => split(["-p", &pct]),
            };
            match status {
                Ok(s) if s.success() => return true,
                Ok(_) => {
                    eprintln!("tmux split-window failed, falling back to daemon mode");
                }
                Err(e) => {
                    eprintln!("tmux not available ({e}), falling back to daemon mode");
                }
            }
        }
        start_daemon(path);
        return true;
    }

    if let Some(ref path) = args.play_detach {
        match spawn_player(path).status() {
            Ok(s) => {
                if !s.success() {
                    process::exit(s.code().unwrap_or(1));
                }
                return true;
            }
            Err(e) => {
                eprintln!("Failed to start playback: {e}");
                process::exit(1);
            }
        }
    }
    false
}

/// Build a heightfield from a composite CanvasBuffer (for lighting normal map).
pub(crate) fn heightfield_from_composite(buf: &figby::tui::canvas::CanvasBuffer) -> Vec<Vec<f32>> {
    (0..buf.height())
        .map(|y| {
            (0..buf.width())
                .map(|x| {
                    buf.get(x, y)
                        .map_or(0.0, |c| c.height.unwrap_or(0) as f32 / 255.0)
                })
                .collect()
        })
        .collect()
}

/// Apply luminance shading to a composite, producing lit CanvasCells.
pub(crate) fn apply_luminance_to_composite(
    buf: &figby::tui::canvas::CanvasBuffer,
    fg_lum: &[Vec<f32>],
    bg_lum: &[Vec<f32>],
) -> figby::tui::canvas::CanvasBuffer {
    let w = buf.width();
    let h = buf.height();
    let mut result = figby::tui::canvas::CanvasBuffer::new(w, h);
    for y in 0..h {
        for x in 0..w {
            if let Some(cell) = buf.get(x, y) {
                let fl = fg_lum
                    .get(y)
                    .and_then(|row| row.get(x))
                    .copied()
                    .unwrap_or(1.0);
                let bl = bg_lum
                    .get(y)
                    .and_then(|row| row.get(x))
                    .copied()
                    .unwrap_or(1.0);
                let modulate_fg = |c: u8| (c as f32 * fl).round() as u8;
                let modulate_bg = |c: u8| (c as f32 * bl).round() as u8;
                let fg = cell.fg.map(|color| {
                    if let ratatui::style::Color::Rgb(r, g, b) = color {
                        ratatui::style::Color::Rgb(modulate_fg(r), modulate_fg(g), modulate_fg(b))
                    } else {
                        color
                    }
                });
                let bg = cell.bg.map(|color| {
                    if let ratatui::style::Color::Rgb(r, g, b) = color {
                        ratatui::style::Color::Rgb(modulate_bg(r), modulate_bg(g), modulate_bg(b))
                    } else {
                        color
                    }
                });
                result.set(
                    x,
                    y,
                    figby::CanvasCell {
                        ch: cell.ch,
                        fg,
                        bg,
                        height: cell.height,
                    },
                );
            }
        }
    }
    result
}

/// Arguments for a child `figby --play` process started by `--play-daemon`,
/// `--play-tmux` or `--play-detach`: the file plus every playback option.
pub(crate) fn playback_child_args(args: &CliArgs, path: &str) -> Vec<String> {
    let mut out = vec!["--play".to_string(), path.to_string()];
    if args.play_loop {
        out.push("--loop".to_string());
    }
    if let Some(w) = args.play_width {
        out.push("--play-width".to_string());
        out.push(w.to_string());
    }
    if let Some(s) = args.hold_last {
        out.push("--hold-last".to_string());
        out.push(s.to_string());
    }
    if args.play_inline {
        out.push("--play-inline".to_string());
    }
    if args.play_timeline {
        out.push("--play-timeline".to_string());
    }
    out
}

/// Quote one word for a POSIX shell: wrap in single quotes, and close,
/// escape and reopen around each embedded `'`. Nothing is special inside
/// single quotes, so `$(...)`, backticks, `"` and globs stay literal. The
/// same form is also valid in fish, which tmux may use as default-shell.
pub(crate) fn shell_quote(word: &str) -> String {
    format!("'{}'", word.replace('\'', r"'\''"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    // ── 2026-11-04 review S1: --play-tmux shell injection ──────────────────

    #[cfg(unix)]
    #[test]
    fn test_shell_quote_survives_sh_and_never_executes() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("pwned");
        let m = marker.display();
        let payloads = [
            format!("$(touch {m}).gif"),
            format!("`touch {m}`.gif"),
            format!("x\"; touch {m}; \".gif"),
            format!("it's; touch {m} #.gif"),
            "spaces and * globs ?.gif".to_string(),
            "back\\slash\nnewline.gif".to_string(),
            "'".to_string(),
            String::new(),
        ];
        for p in &payloads {
            let script = format!("printf '%s' {}", shell_quote(p));
            let out = std::process::Command::new("sh")
                .args(["-c", &script])
                .output()
                .unwrap();
            assert!(out.status.success(), "{p:?}");
            assert_eq!(
                String::from_utf8_lossy(&out.stdout),
                *p,
                "argv must round-trip"
            );
        }
        assert!(
            !marker.exists(),
            "a quoted filename must never run a command"
        );
    }

    #[test]
    fn test_playback_child_args_forward_every_option() {
        let args = CliArgs::try_parse_from([
            "figby",
            "--play-tmux",
            "a.gif",
            "--loop",
            "--play-width",
            "40",
            "--hold-last",
            "1.5",
            "--play-inline",
            "--play-timeline",
        ])
        .unwrap();
        assert_eq!(
            playback_child_args(&args, "a.gif"),
            [
                "--play",
                "a.gif",
                "--loop",
                "--play-width",
                "40",
                "--hold-last",
                "1.5",
                "--play-inline",
                "--play-timeline"
            ]
        );
        let bare = CliArgs::try_parse_from(["figby", "--play-daemon", "b.gif"]).unwrap();
        assert_eq!(playback_child_args(&bare, "b.gif"), ["--play", "b.gif"]);
    }
}
