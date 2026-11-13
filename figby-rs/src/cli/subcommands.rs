//! `figby play|bake|image|font|tui` subcommands (review A2).
//!
//! The FIGlet CLI stays the default: `figby [figlet flags] [message]` is
//! parsed exactly as before, with C getopt semantics. A subcommand is
//! recognised only when it is the very first argument, so every FIGlet
//! invocation that starts with a flag, and every message that is not one of
//! these five words, is untouched. `figby -- play` or `figby -A play`
//! still renders the word "play", and a binary installed or linked as
//! `figlet` never treats its first argument as a subcommand.
//!
//! Each subcommand fills the same [`CliArgs`] fields the legacy long flags
//! (`--play`, `--bake`, `--image`, `--create-font`, `--tui`, ...) set, so
//! both spellings run the same code with the same exit codes. The legacy
//! flags remain accepted.

use crate::{parse_cli, CliArgs};
use clap::{Args, Parser, Subcommand};
use std::ffi::OsString;

/// Subcommand names, in the order `--help` lists them.
pub(crate) const NAMES: &[&str] = &["play", "bake", "image", "font", "tui"];

#[derive(Parser, Debug)]
#[command(
    name = "figby",
    disable_help_subcommand = true,
    about = "Figby extension programs (the FIGlet CLI is `figby [options] [message]`)"
)]
struct SubcommandCli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Play an animated GIF, .figmap or .figseq in the terminal
    /// (same as `--play`; static figmaps display inline)
    Play(PlayArgs),
    /// Bake a .figmap into a .figseq frame sequence (same as `--bake`)
    Bake(BakeArgs),
    /// Convert images to ASCII or braille art (same as `--image`)
    Image(ImageArgs),
    /// Generate a FIGfont from a system font or a .ttf/.otf file
    /// (same as `--create-font` / `--create-font-path`)
    Font(FontArgs),
    /// Launch the interactive TUI editor (same as `--tui`)
    Tui(TuiArgs),
}

#[derive(Args, Debug)]
struct PlayArgs {
    /// Animation to play (.gif, .figmap or .figseq)
    file: String,
    /// Repeat until any key is pressed, instead of playing once
    #[arg(long = "loop")]
    loop_playback: bool,
    /// Keep the final frame on screen this many seconds after a
    /// non-looping animation ends (any key dismisses)
    #[arg(long, value_name = "SECONDS")]
    hold_last: Option<f64>,
    /// Scale playback to N terminal columns, preserving aspect ratio
    /// [default: fit to terminal]
    #[arg(long, visible_alias = "play-width", value_name = "COLS")]
    width: Option<usize>,
    /// Render at the cursor position without clearing the screen
    #[arg(long, visible_alias = "play-inline")]
    inline: bool,
    /// With --inline: show a one-row playback timeline under the animation
    #[arg(long, visible_alias = "play-timeline")]
    timeline: bool,
    /// Fork playback to the background and return to the shell
    /// (same as `--play-daemon`)
    #[arg(long, group = "spawn")]
    daemon: bool,
    /// Open playback in a new tmux pane, falling back to --daemon outside
    /// tmux (same as `--play-tmux`)
    #[arg(long, group = "spawn")]
    tmux: bool,
    /// With --tmux: pane width as a percentage of the terminal [default: 50]
    #[arg(
        long,
        visible_alias = "play-tmux-size",
        value_name = "PERCENT",
        requires = "tmux"
    )]
    tmux_size: Option<u8>,
    /// Play in a child process and return when it finishes
    /// (same as `--play-detach`)
    #[arg(long, group = "spawn")]
    detach: bool,
}

#[derive(Args, Debug)]
struct BakeArgs {
    /// Project to bake (.figmap)
    file: String,
    /// Output .figseq path
    #[arg(short, long, value_name = "PATH")]
    output: String,
    /// Mark the sequence as looping
    #[arg(long = "loop")]
    loop_playback: bool,
    /// Record a final-frame hold of this many seconds in the sequence
    #[arg(long, value_name = "SECONDS")]
    hold_last: Option<f64>,
}

#[derive(Args, Debug)]
struct ImageArgs {
    /// Image file(s) to convert (local files only)
    #[arg(required = true, value_name = "FILE")]
    files: Vec<String>,
    /// Custom character map (darkest to brightest)
    #[arg(long)]
    map: Option<String>,
    /// Use braille characters instead of ASCII
    #[arg(short, long)]
    braille: bool,
    /// Output with 24-bit ANSI color codes
    #[arg(long)]
    color: bool,
    /// Convert to grayscale before output
    #[arg(long)]
    grayscale: bool,
    /// Invert image colors
    #[arg(long)]
    negative: bool,
    /// Apply Floyd-Steinberg dithering for braille mode
    #[arg(long)]
    dither: bool,
    /// Output width in characters
    #[arg(long)]
    width: Option<u32>,
    /// Output height in characters
    #[arg(long)]
    height: Option<u32>,
    /// Output dimensions (format: WxH, e.g. 80x40)
    #[arg(long)]
    dimensions: Option<String>,
    /// Flip image horizontally
    #[arg(long = "flipX")]
    flip_x: bool,
    /// Flip image vertically
    #[arg(long = "flipY")]
    flip_y: bool,
}

#[derive(Args, Debug)]
#[command(group(clap::ArgGroup::new("source").required(true).args(["name", "file"])))]
struct FontArgs {
    /// System font name to convert
    name: Option<String>,
    /// Convert this font file (.ttf/.otf) instead of a system font
    #[arg(long, value_name = "PATH")]
    file: Option<String>,
    /// Font size in points
    #[arg(long, visible_alias = "font-size", default_value = "12.0")]
    size: f32,
    /// Charset: block, default, slight, smooth, full, deluxe, or comma-separated
    #[arg(long, default_value = "smooth")]
    charset: String,
    /// Write the .flf here instead of stdout
    #[arg(short, long, value_name = "PATH")]
    output: Option<String>,
}

#[derive(Args, Debug)]
struct TuiArgs {
    /// Render mode: fast (always redraw) or dirty (only on change)
    /// [default: dirty]
    #[arg(long, visible_alias = "tui-render-mode", value_name = "MODE")]
    render_mode: Option<String>,
}

/// Whether `argv` is a subcommand invocation: its first argument is one of
/// [`NAMES`] and the program is not running under the name `figlet`.
pub(crate) fn is_subcommand_invocation(argv: &[OsString]) -> bool {
    let Some(first) = argv.get(1).and_then(|a| a.to_str()) else {
        return false;
    };
    if !NAMES.contains(&first) {
        return false;
    }
    let invoked_as = argv
        .first()
        .map(std::path::Path::new)
        .and_then(|p| p.file_stem())
        .and_then(|s| s.to_str())
        .unwrap_or("");
    !invoked_as.eq_ignore_ascii_case("figlet")
}

/// Parse a subcommand invocation into the equivalent legacy [`CliArgs`].
pub(crate) fn parse_subcommand<I, T>(argv: I) -> Result<CliArgs, clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cli = SubcommandCli::try_parse_from(argv)?;
    // Start from "no options given" so every default matches the legacy CLI.
    let mut args = parse_cli(["figby"])?;
    match cli.command {
        Command::Play(p) => {
            args.play_loop = p.loop_playback;
            args.hold_last = p.hold_last;
            args.play_width = p.width;
            args.play_inline = p.inline;
            args.play_timeline = p.timeline;
            args.play_tmux_size = p.tmux_size;
            let slot = if p.daemon {
                &mut args.play_daemon
            } else if p.tmux {
                &mut args.play_tmux
            } else if p.detach {
                &mut args.play_detach
            } else {
                &mut args.play_path
            };
            *slot = Some(p.file);
        }
        Command::Bake(b) => {
            args.bake_path = Some(b.file);
            args.output_path = Some(b.output);
            args.play_loop = b.loop_playback;
            args.hold_last = b.hold_last;
        }
        Command::Image(i) => {
            args.image_paths = i.files;
            args.map = i.map;
            args.braille = i.braille;
            args.color = i.color;
            args.grayscale = i.grayscale;
            args.negative = i.negative;
            args.dither = i.dither;
            args.img_width_arg = i.width;
            args.img_height_arg = i.height;
            args.dimensions = i.dimensions;
            args.flip_x = i.flip_x;
            args.flip_y = i.flip_y;
        }
        Command::Font(f) => {
            args.create_font_name = f.name;
            args.create_font_path = f.file;
            args.create_font_size = f.size;
            args.create_font_charset = f.charset;
            args.output_path = f.output;
        }
        Command::Tui(t) => {
            args.flag_tui = true;
            args.tui_render_mode = t.render_mode;
        }
    }
    Ok(args)
}

/// Parse the process command line: a subcommand when the first argument
/// names one, otherwise the FIGlet CLI.
pub(crate) fn parse_args(argv: Vec<OsString>) -> Result<CliArgs, clap::Error> {
    if is_subcommand_invocation(&argv) {
        parse_subcommand(argv)
    } else {
        parse_cli(argv)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os(argv: &[&str]) -> Vec<OsString> {
        argv.iter().map(OsString::from).collect()
    }

    fn sub(argv: &[&str]) -> CliArgs {
        parse_args(os(argv)).expect("parses")
    }

    #[test]
    fn subcommand_only_as_first_argument() {
        assert!(is_subcommand_invocation(&os(&["figby", "play", "a.gif"])));
        assert!(is_subcommand_invocation(&os(&["figby", "tui"])));
        // FIGlet messages are untouched unless the first word is a subcommand.
        assert!(!is_subcommand_invocation(&os(&["figby", "hello", "play"])));
        assert!(!is_subcommand_invocation(&os(&["figby", "-A", "play"])));
        assert!(!is_subcommand_invocation(&os(&["figby", "--", "play"])));
        assert!(!is_subcommand_invocation(&os(&["figby", "Play"])));
        assert!(!is_subcommand_invocation(&os(&["figby"])));
        // Installed as `figlet`: never a subcommand (drop-in parity).
        assert!(!is_subcommand_invocation(&os(&["figlet", "play"])));
        assert!(!is_subcommand_invocation(&os(&["/usr/bin/figlet", "tui"])));
        assert!(!is_subcommand_invocation(&os(&[
            r"C:\bin\FIGLET.exe",
            "font"
        ])));
    }

    #[test]
    fn non_subcommand_words_still_render_as_messages() {
        let args = sub(&["figby", "--", "play"]);
        assert_eq!(args.message, vec!["play".to_string()]);
        assert!(args.play_path.is_none());
        let args = sub(&["figby", "-A", "tui"]);
        assert_eq!(args.message, vec!["tui".to_string()]);
        assert!(!args.flag_tui);
        let args = sub(&["figlet", "bake"]);
        assert_eq!(args.message, vec!["bake".to_string()]);
    }

    #[test]
    fn play_maps_to_legacy_flags() {
        let a = sub(&[
            "figby",
            "play",
            "x.gif",
            "--loop",
            "--hold-last",
            "1.5",
            "--width",
            "40",
            "--inline",
            "--timeline",
        ]);
        let b = sub(&[
            "figby",
            "--play",
            "x.gif",
            "--loop",
            "--hold-last",
            "1.5",
            "--play-width",
            "40",
            "--play-inline",
            "--play-timeline",
        ]);
        assert_eq!(format!("{a:?}"), format!("{b:?}"));
        assert_eq!(a.play_path.as_deref(), Some("x.gif"));
    }

    #[test]
    fn play_spawn_modes_map_to_legacy_flags() {
        assert_eq!(
            sub(&["figby", "play", "--daemon", "x.gif"])
                .play_daemon
                .as_deref(),
            Some("x.gif")
        );
        let t = sub(&["figby", "play", "--tmux", "--tmux-size", "30", "x.gif"]);
        assert_eq!(t.play_tmux.as_deref(), Some("x.gif"));
        assert_eq!(t.play_tmux_size, Some(30));
        assert!(t.play_path.is_none());
        assert_eq!(
            sub(&["figby", "play", "--detach", "x.gif"])
                .play_detach
                .as_deref(),
            Some("x.gif")
        );
        // Spawn modes are mutually exclusive; --tmux-size needs --tmux.
        assert!(parse_args(os(&["figby", "play", "--daemon", "--tmux", "x.gif"])).is_err());
        assert!(parse_args(os(&["figby", "play", "--tmux-size", "30", "x.gif"])).is_err());
    }

    #[test]
    fn bake_requires_output_and_maps_to_legacy_flags() {
        let a = sub(&["figby", "bake", "p.figmap", "-o", "p.figseq", "--loop"]);
        let b = sub(&[
            "figby", "--bake", "p.figmap", "--output", "p.figseq", "--loop",
        ]);
        assert_eq!(format!("{a:?}"), format!("{b:?}"));
        let err = parse_args(os(&["figby", "bake", "p.figmap"])).unwrap_err();
        // Same exit status as the legacy `--bake` without `--output`.
        assert_eq!(err.exit_code(), 2);
    }

    #[test]
    fn image_maps_to_legacy_flags() {
        let a = sub(&[
            "figby",
            "image",
            "a.png",
            "b.png",
            "-b",
            "--color",
            "--dimensions",
            "80x40",
            "--flipX",
        ]);
        let b = sub(&[
            "figby",
            "--image",
            "a.png",
            "--image",
            "b.png",
            "-b",
            "--color",
            "--dimensions",
            "80x40",
            "--flipX",
        ]);
        assert_eq!(format!("{a:?}"), format!("{b:?}"));
        assert!(parse_args(os(&["figby", "image"])).is_err());
    }

    #[test]
    fn font_maps_to_legacy_flags() {
        let a = sub(&["figby", "font", "Arial", "--size", "16", "-o", "a.flf"]);
        let b = sub(&[
            "figby",
            "--create-font",
            "Arial",
            "--font-size",
            "16",
            "--output",
            "a.flf",
        ]);
        assert_eq!(format!("{a:?}"), format!("{b:?}"));
        let f = sub(&["figby", "font", "--file", "x.ttf", "--charset", "block"]);
        assert_eq!(f.create_font_path.as_deref(), Some("x.ttf"));
        assert_eq!(f.create_font_charset, "block");
        assert!(f.create_font_name.is_none());
        // Exactly one source.
        assert!(parse_args(os(&["figby", "font"])).is_err());
        assert!(parse_args(os(&["figby", "font", "Arial", "--file", "x.ttf"])).is_err());
    }

    #[test]
    fn tui_maps_to_legacy_flags() {
        let a = sub(&["figby", "tui", "--render-mode", "fast"]);
        let b = sub(&["figby", "--tui", "--tui-render-mode", "fast"]);
        assert_eq!(format!("{a:?}"), format!("{b:?}"));
        assert!(sub(&["figby", "tui"]).flag_tui);
    }

    #[test]
    fn subcommand_help_exits_zero() {
        for name in NAMES {
            let err = parse_args(os(&["figby", name, "--help"])).unwrap_err();
            assert_eq!(err.kind(), clap::error::ErrorKind::DisplayHelp, "{name}");
            assert_eq!(err.exit_code(), 0, "{name}");
        }
    }
}
