// On wasm32 `main` only runs `figby::web`; the FIGlet CLI below is native-only
// and would otherwise trip dead-code lints in the wasm clippy job.
#![cfg_attr(target_arch = "wasm32", allow(dead_code, unused_imports))]

use clap::Parser;
use figby::config::{self, FigbyConfig};
use figby::control::{self, CharReader};
use figby::font::{self, FIGfont};
use figby::input;
use figby::render::{lookup_char, render_line, Justification, OutputLine};
use figby::smush::SmushMode;
use std::cell::Cell;
use std::io::{self, IsTerminal, Read, Write};
use std::process;

mod cli;

const VERSION_INT: i32 = 20205;
const VERSION: &str = "2.2.5";
const DATE: &str = "31 May 2012";
const FONTFILE_MAGIC: &str = "flf2";
/// FIGlet's `standard` font, used only when no font directory has one.
const EMBEDDED_STANDARD_FONT: &str = include_str!("../assets/fonts/standard.flf");
const TOILETFILE_MAGIC: &str = "tlf2";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SmushOverride {
    No = 0,
    Yes = 1,
    Force = 2,
}

enum InputIter {
    Stdin {
        bytes: io::Bytes<io::BufReader<io::Stdin>>,
        buf: Option<u32>,
    },
    Args {
        args: Vec<String>,
        arg_idx: usize,
        char_idx: usize,
        exhausted: bool,
        buf: Option<u32>,
    },
}

impl InputIter {
    fn new(message: Vec<String>, cmdinput: bool) -> Self {
        if cmdinput {
            let exhausted = message.is_empty();
            Self::Args {
                args: message,
                arg_idx: 0,
                char_idx: 0,
                exhausted,
                buf: None,
            }
        } else {
            Self::Stdin {
                bytes: io::BufReader::new(io::stdin()).bytes(),
                buf: None,
            }
        }
    }

    fn next(&mut self) -> Option<u32> {
        match self {
            Self::Stdin { bytes, buf } => {
                if let Some(c) = buf.take() {
                    return Some(c);
                }
                match bytes.next() {
                    Some(Ok(b)) => Some(b as u32),
                    _ => None,
                }
            }
            Self::Args {
                args,
                arg_idx,
                char_idx,
                exhausted,
                buf,
            } => {
                if let Some(c) = buf.take() {
                    return Some(c);
                }
                if *exhausted {
                    return None;
                }
                if *arg_idx >= args.len() {
                    *exhausted = true;
                    return None;
                }
                let arg_bytes = args[*arg_idx].as_bytes();
                if *char_idx < arg_bytes.len() {
                    let c = arg_bytes[*char_idx] as u32;
                    *char_idx += 1;
                    return Some(c);
                }
                let is_empty = *char_idx == 0;
                let is_last = *arg_idx + 1 >= args.len();
                *arg_idx += 1;
                *char_idx = 0;
                if is_last {
                    *exhausted = true;
                    return None;
                }
                Some(if is_empty { b'\n' as u32 } else { b' ' as u32 })
            }
        }
    }

    fn unget(&mut self, c: u32) {
        match self {
            Self::Stdin { buf, .. } | Self::Args { buf, .. } => {
                *buf = Some(c);
            }
        }
    }
}

impl CharReader for InputIter {
    fn next(&mut self) -> Option<u32> {
        InputIter::next(self)
    }
    fn unget(&mut self, c: u32) {
        InputIter::unget(self, c);
    }
}

#[derive(Debug, Clone)]
struct CliConfig {
    smushmode: u32,
    smushoverride: SmushOverride,
    justification: i32,
    right2left: i32,
    paragraphflag: bool,
    deutschflag: bool,
    cmdinput: bool,
    outputwidth: u32,
    fontdirname: String,
    fontname: String,
    multibyte: u32,
    controlfiles: Vec<String>,
    to_file: Option<String>,
}

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            smushmode: 0,
            smushoverride: SmushOverride::No,
            justification: -1,
            right2left: -1,
            paragraphflag: false,
            deutschflag: false,
            cmdinput: false,
            outputwidth: 80,
            fontdirname: "/usr/share/figlet".to_string(),
            fontname: "standard".to_string(),
            multibyte: 0,
            controlfiles: Vec::new(),
            to_file: None,
        }
    }
}

/// Appended to `figby --help`. Subcommands are recognised only as the first
/// argument (see `cli::subcommands`).
const SUBCOMMAND_HELP: &str = "\
Subcommands (first argument only; the long options above still work):
  figby play <FILE>              Play a .gif, .figmap or .figseq (--play)
  figby bake <FILE> -o <OUT>     Bake a .figmap into a .figseq (--bake)
  figby image <FILE>...          Convert images to ASCII/braille (--image)
  figby font <NAME> | --file <F> Generate a FIGfont (--create-font)
  figby tui                      Launch the TUI editor (--tui)
Run `figby <subcommand> --help` for its options. To render one of these
words as a banner, use `figby -- play` or `echo play | figby`.";

#[allow(non_snake_case)]
#[derive(Parser, Debug)]
#[command(
    name = "figby",
    // FIGlet options may repeat; as with C getopt, the last one wins.
    args_override_self = true,
    about = "Rust port of FIGlet — ASCII art banner generator",
    long_about = "FIGby is a Rust port of FIGlet 2.2.5 (Frank, Ian & Glenn's Letters).\nRenders text in large characters using FIGfont (.flf) and TOIlet (.tlf)\nfont files with kerning, smushing, and multi-byte character support.",
    after_help = SUBCOMMAND_HELP
)]
struct CliArgs {
    #[arg(
        short = 'A',
        help = "Read input from arguments (implies command-line input)"
    )]
    flag_A: bool,
    #[arg(short = 'D', help = "Enable German character handling")]
    flag_D: bool,
    #[arg(short = 'E', help = "Disable German character handling")]
    flag_E: bool,
    #[arg(short = 'X', help = "Use font's default writing direction")]
    flag_X: bool,
    #[arg(short = 'L', help = "Force left-to-right writing direction")]
    flag_L: bool,
    #[arg(short = 'R', help = "Force right-to-left writing direction")]
    flag_R: bool,
    #[arg(short = 'x', help = "Use font's default justification")]
    flag_x: bool,
    #[arg(short = 'l', help = "Left justify output")]
    flag_l: bool,
    #[arg(short = 'c', help = "Center justify output")]
    flag_c: bool,
    #[arg(short = 'r', help = "Right justify output")]
    flag_r: bool,
    #[arg(short = 'p', help = "Enable paragraph mode")]
    flag_p: bool,
    #[arg(short = 'n', help = "Disable paragraph mode")]
    flag_n: bool,
    #[arg(short = 's', help = "Use font's default layout/smushing")]
    flag_s: bool,
    #[arg(short = 'k', help = "Use kerning (no smushing)")]
    flag_k: bool,
    #[arg(
        short = 'S',
        help = "Force smushing (font layout combined with smush mode)"
    )]
    flag_S: bool,
    #[arg(short = 'o', help = "Use smushing (replaces font's layout)")]
    flag_o: bool,
    #[arg(short = 'W', help = "Width-only (no kerning or smushing)")]
    flag_W: bool,
    #[arg(short = 't', help = "Use terminal width for output")]
    flag_t: bool,
    #[arg(short = 'v', help = "Display version information and exit")]
    flag_v: bool,
    #[arg(short = 'N', help = "Disable multi-byte input processing")]
    flag_N: bool,
    #[arg(
        short = 'F',
        help = "Removed (matches C FIGlet 2.2.5) — list fonts with figlist"
    )]
    flag_F: bool,
    #[arg(
        short = 'I',
        help = "Print info code (0=copyright, 1=version, 2=fontdir, 3=font, 4=width, 5=formats)"
    )]
    infocode: Option<String>,
    #[arg(
        short = 'm',
        allow_hyphen_values = true,
        help = "Set layout mode (-2=font default, -1=full width, 0=kerning, >0=smush rules)"
    )]
    smushmode_arg: Option<String>,
    #[arg(
        short = 'w',
        allow_hyphen_values = true,
        help = "Set output width in columns [default: 80]; values below 1 are ignored, as in C"
    )]
    outputwidth_arg: Option<String>,
    #[arg(short = 'd', help = "Font directory path")]
    fontdir: Option<String>,
    #[arg(short = 'f', help = "Font name to use [default: standard]")]
    fontname_arg: Option<String>,
    #[arg(
        short = 'C',
        action = clap::ArgAction::Append,
        help = "Control file (.flc), looked up in the font directory; repeatable, applied in order (-N clears earlier ones)"
    )]
    controlfile: Vec<String>,
    #[arg(
        long = "create-font",
        help = "Generate a FIGfont from a system font by name"
    )]
    create_font_name: Option<String>,
    #[arg(
        long = "create-font-path",
        help = "Generate a FIGfont from a font file (.ttf/.otf)"
    )]
    create_font_path: Option<String>,
    #[arg(
        long = "font-size",
        default_value = "12.0",
        help = "Font size in points for --create-font"
    )]
    create_font_size: f32,
    #[arg(
        long = "output",
        help = "Write output to file instead of stdout (text, templates, --create-font; required by --bake)"
    )]
    output_path: Option<String>,
    #[arg(
        long = "create-font-charset",
        default_value = "smooth",
        help = "Charset for --create-font: block, default, slight, smooth, full, deluxe, or comma-separated"
    )]
    create_font_charset: String,
    #[arg(
        short = 'T',
        long = "render-template",
        help = "Render a .ftmp template file"
    )]
    render_template: Option<String>,
    #[arg(
        long = "to-file",
        help = "Write rendered text or template output to a file instead of stdout (same as --output)"
    )]
    to_file: Option<String>,
    #[arg(
        short = 'i',
        long = "image",
        help = "Image file path(s) to convert to ASCII (local files only)"
    )]
    image_paths: Vec<String>,
    #[arg(long = "map", help = "Custom character map (darkest to brightest)")]
    map: Option<String>,
    #[arg(
        short = 'b',
        long = "braille",
        help = "Use braille characters instead of ASCII"
    )]
    braille: bool,
    #[arg(long = "color", help = "Output with 24-bit ANSI color codes")]
    color: bool,
    #[arg(long = "grayscale", help = "Convert to grayscale before output")]
    grayscale: bool,
    #[arg(long = "negative", help = "Invert image colors")]
    negative: bool,
    #[arg(
        long = "dither",
        help = "Apply Floyd-Steinberg dithering for braille mode"
    )]
    dither: bool,
    #[arg(long = "width", help = "Output width in characters")]
    img_width_arg: Option<u32>,
    #[arg(long = "height", help = "Output height in characters")]
    img_height_arg: Option<u32>,
    #[arg(
        long = "dimensions",
        help = "Output dimensions (format: WxH, e.g. 80x40)"
    )]
    dimensions: Option<String>,
    #[arg(long = "flipX", help = "Flip image horizontally")]
    flip_x: bool,
    #[arg(long = "flipY", help = "Flip image vertically")]
    flip_y: bool,
    #[arg(long = "tui", help = "Launch interactive TUI editor")]
    flag_tui: bool,
    #[arg(
        long = "tui-render-mode",
        help = "TUI render mode: fast (always redraw) or dirty (only on change) [default: dirty]"
    )]
    tui_render_mode: Option<String>,
    #[arg(
        long = "play",
        help = "Play an animated GIF, figmap or figseq in the terminal (static figmaps display inline); then exit"
    )]
    play_path: Option<String>,
    #[arg(
        long = "bake",
        help = "Bake a .figmap into a .figseq frame sequence (flat rasterized screens, per-frame hold times); needs --output"
    )]
    bake_path: Option<String>,
    #[arg(
        long = "play-width",
        help = "Scale playback to N terminal columns, preserving aspect ratio [default: fit to terminal]"
    )]
    play_width: Option<usize>,
    #[arg(
        long = "loop",
        help = "With --play: repeat the animation until any key is pressed, instead of playing once. With --bake: mark the sequence as looping"
    )]
    play_loop: bool,
    #[arg(
        long = "hold-last",
        value_name = "SECONDS",
        help = "Keep the final frame on screen this many seconds after a non-looping animation ends, instead of exiting immediately (any key dismisses). With --bake: record it in the sequence"
    )]
    hold_last: Option<f64>,
    #[arg(
        long = "play-inline",
        help = "With --play: render animation at cursor position without clearing screen"
    )]
    play_inline: bool,
    #[arg(
        long = "play-timeline",
        help = "With --play --play-inline: show a one-row playback timeline under the animation [default: off]"
    )]
    play_timeline: bool,
    #[arg(
        long = "play-daemon",
        help = "Fork animation to background, return to shell immediately (kill PID to stop)"
    )]
    play_daemon: Option<String>,
    #[arg(
        long = "play-tmux",
        help = "Open animation in new tmux pane (falls back to --play-daemon if not in tmux)"
    )]
    play_tmux: Option<String>,
    #[arg(
        long = "play-tmux-size",
        help = "With --play-tmux: pane width as percentage of terminal (default: 50)"
    )]
    play_tmux_size: Option<u8>,
    #[arg(
        long = "play-detach",
        help = "Run animation synchronously in child process, return to shell when done"
    )]
    play_detach: Option<String>,
    #[arg(help = "Text to render (reads from stdin if omitted)")]
    message: Vec<String>,
    /// FIGlet options in command-line order, filled by [`parse_cli`]. C
    /// FIGlet applies options as getopt returns them, so `-c -l` is left
    /// and `-s -k` is kerning; empty means "use the fixed fallback order".
    #[arg(skip)]
    order: Vec<CliEvent>,
}

/// One order-sensitive FIGlet option occurrence (see `CliArgs::order`).
#[derive(Debug, Clone, PartialEq, Eq)]
enum CliEvent {
    /// A flag or single-valued option, by clap id (its field name).
    Opt(&'static str),
    /// One `-C` occurrence.
    Control(String),
}

/// FIGlet options whose relative order matters, by clap id. Everything
/// else (-A, -v, -I, -F, the long Figby options) is order-independent.
const ORDERED_OPTS: &[&str] = &[
    "flag_D",
    "flag_E",
    "flag_X",
    "flag_L",
    "flag_R",
    "flag_x",
    "flag_l",
    "flag_c",
    "flag_r",
    "flag_p",
    "flag_n",
    "flag_N",
    "flag_W",
    "flag_k",
    "flag_o",
    "flag_S",
    "flag_s",
    "smushmode_arg",
    "flag_t",
    "outputwidth_arg",
    "fontdir",
    "fontname_arg",
];

/// Parse the command line and record the order of FIGlet options.
fn parse_cli<I, T>(argv: I) -> Result<CliArgs, clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    use clap::parser::ValueSource;
    use clap::{CommandFactory, FromArgMatches};
    let matches = CliArgs::command().try_get_matches_from(argv)?;
    let mut args = CliArgs::from_arg_matches(&matches)?;
    let mut events: Vec<(usize, CliEvent)> = Vec::new();
    for &id in ORDERED_OPTS {
        if matches.value_source(id) != Some(ValueSource::CommandLine) {
            continue;
        }
        // args_override_self keeps only the last occurrence, which is the
        // one that decides the outcome.
        if let Some(idx) = matches.indices_of(id).and_then(|mut i| i.next_back()) {
            events.push((idx, CliEvent::Opt(id)));
        }
    }
    if let (Some(idx), Some(vals)) = (
        matches.indices_of("controlfile"),
        matches.get_many::<String>("controlfile"),
    ) {
        for (i, v) in idx.zip(vals) {
            events.push((i, CliEvent::Control(v.clone())));
        }
    }
    events.sort_by_key(|(i, _)| *i);
    args.order = events.into_iter().map(|(_, e)| e).collect();
    Ok(args)
}

/// C `atoi()`: optional leading whitespace and sign, then digits; anything
/// unparsable is 0. FIGlet reads `-w`, `-m` and `-I` this way, so `-w abc`
/// and `-m x` are accepted (as 0) rather than rejected.
fn c_atoi(s: &str) -> i64 {
    let t = s.trim_start();
    let (neg, digits) = match t.as_bytes().first() {
        Some(b'-') => (true, &t[1..]),
        Some(b'+') => (false, &t[1..]),
        _ => (false, t),
    };
    let mut n: i64 = 0;
    for b in digits.bytes().take_while(u8::is_ascii_digit) {
        n = n.saturating_mul(10).saturating_add(i64::from(b - b'0'));
    }
    if neg {
        -n
    } else {
        n
    }
}

/// `-f name.flf` / `-f name.tlf` → `name`, as C does while parsing `-f`.
fn strip_font_suffix(name: &str) -> String {
    name.strip_suffix(".flf")
        .or_else(|| name.strip_suffix(".tlf"))
        .unwrap_or(name)
        .to_string()
}

impl CliConfig {
    #[cfg(test)]
    fn from_args(args: CliArgs) -> Self {
        Self::from_args_with_config(args, &FigbyConfig::default())
    }

    fn from_args_with_config(args: CliArgs, config_file: &FigbyConfig) -> Self {
        let mut config = CliConfig::default();

        if let Some(ref font) = config_file.cli.font {
            config.fontname = font.clone();
        }
        if let Some(width) = config_file.cli.output_width {
            config.outputwidth = width;
        }

        if !args.message.is_empty() || args.flag_A {
            config.cmdinput = true;
        }

        // C reads FIGLET_FONTDIR before getopt, so a later -d still wins.
        if let Ok(val) = std::env::var("FIGLET_FONTDIR") {
            if !val.is_empty() {
                config.fontdirname = val;
            }
        }

        // Replay the FIGlet options in command-line order (C getopt
        // semantics). Structs built without parse_cli (unit tests) fall back
        // to a fixed order: every present option in ORDERED_OPTS order, then
        // the -C files.
        let events: Vec<CliEvent> = if args.order.is_empty() {
            let mut ev: Vec<CliEvent> = ORDERED_OPTS
                .iter()
                .filter(|id| args.is_present(id))
                .map(|id| CliEvent::Opt(id))
                .collect();
            ev.extend(args.controlfile.iter().cloned().map(CliEvent::Control));
            ev
        } else {
            args.order.clone()
        };
        for event in events {
            match event {
                CliEvent::Control(name) => {
                    let name = name.strip_suffix(".flc").unwrap_or(&name).to_string();
                    config.controlfiles.push(name);
                }
                CliEvent::Opt(id) => config.apply_opt(id, &args),
            }
        }

        // `--to-file` and `--output` are synonyms for text rendering.
        config.to_file = args.to_file.or(args.output_path);

        config
    }

    /// Apply one FIGlet option, exactly as C's getopt switch does.
    fn apply_opt(&mut self, id: &str, args: &CliArgs) {
        match id {
            "flag_D" => self.deutschflag = true,
            "flag_E" => self.deutschflag = false,
            "flag_X" => self.right2left = -1,
            "flag_L" => self.right2left = 0,
            "flag_R" => self.right2left = 1,
            "flag_x" => self.justification = -1,
            "flag_l" => self.justification = 0,
            "flag_c" => self.justification = 1,
            "flag_r" => self.justification = 2,
            "flag_p" => self.paragraphflag = true,
            "flag_n" => self.paragraphflag = false,
            "flag_N" => {
                // C clearcfilelist(): drops the control files named so far.
                self.controlfiles.clear();
                self.multibyte = 0;
            }
            "flag_W" => {
                self.smushmode = 0;
                self.smushoverride = SmushOverride::Yes;
            }
            "flag_k" => {
                self.smushmode = 64;
                self.smushoverride = SmushOverride::Yes;
            }
            "flag_o" => {
                self.smushmode = 128;
                self.smushoverride = SmushOverride::Yes;
            }
            "flag_S" => {
                self.smushmode = 128;
                self.smushoverride = SmushOverride::Force;
            }
            "flag_s" => self.smushoverride = SmushOverride::No,
            "smushmode_arg" => {
                let val = args.smushmode_arg.as_deref().map_or(0, c_atoi);
                if val < -1 {
                    self.smushoverride = SmushOverride::No;
                } else {
                    self.smushmode = match val {
                        -1 => 0,
                        0 => 64,
                        v => (v as u32 & 63) | 128,
                    };
                    self.smushoverride = SmushOverride::Yes;
                }
            }
            "flag_t" => {
                if let Some(cols) = get_columns() {
                    if cols > 0 {
                        self.outputwidth = cols as u32;
                    }
                }
            }
            "outputwidth_arg" => {
                let cols = args.outputwidth_arg.as_deref().map_or(0, c_atoi);
                if cols > 0 {
                    self.outputwidth = cols.min(i64::from(u32::MAX)) as u32;
                }
            }
            "fontdir" => {
                if let Some(ref d) = args.fontdir {
                    self.fontdirname = d.clone();
                }
            }
            "fontname_arg" => {
                if let Some(ref f) = args.fontname_arg {
                    self.fontname = strip_font_suffix(f);
                }
            }
            _ => {}
        }
    }
}

impl CliArgs {
    /// Whether an order-sensitive option (by clap id) was given.
    fn is_present(&self, id: &str) -> bool {
        match id {
            "flag_D" => self.flag_D,
            "flag_E" => self.flag_E,
            "flag_X" => self.flag_X,
            "flag_L" => self.flag_L,
            "flag_R" => self.flag_R,
            "flag_x" => self.flag_x,
            "flag_l" => self.flag_l,
            "flag_c" => self.flag_c,
            "flag_r" => self.flag_r,
            "flag_p" => self.flag_p,
            "flag_n" => self.flag_n,
            "flag_N" => self.flag_N,
            "flag_W" => self.flag_W,
            "flag_k" => self.flag_k,
            "flag_o" => self.flag_o,
            "flag_S" => self.flag_S,
            "flag_s" => self.flag_s,
            "smushmode_arg" => self.smushmode_arg.is_some(),
            "flag_t" => self.flag_t,
            "outputwidth_arg" => self.outputwidth_arg.is_some(),
            "fontdir" => self.fontdir.is_some(),
            "fontname_arg" => self.fontname_arg.is_some(),
            _ => false,
        }
    }
}

fn printusage(out: &mut impl Write, myname: &str) -> io::Result<()> {
    writeln!(
        out,
        "Usage: {myname} [ -cklnoprstvxDELNRSWX ] [ -d fontdirectory ]"
    )?;
    writeln!(
        out,
        "              [ -f fontfile ] [ -m smushmode ] [ -w outputwidth ]"
    )?;
    writeln!(
        out,
        "              [ -C controlfile ] [ -I infocode ] [ message ]"
    )?;
    Ok(())
}

fn printinfo(
    out: &mut impl Write,
    infocode: i32,
    config: &CliConfig,
    myname: &str,
) -> io::Result<()> {
    match infocode {
        0 => {
            writeln!(
                out,
                "FIGlet Copyright (C) 1991-2012 Glenn Chappell, Ian Chai, John Cowan,"
            )?;
            writeln!(out, "Christiaan Keet and Claudio Matsuoka")?;
            writeln!(
                out,
                "Internet: <info@figlet.org> Version: {}, date: {}",
                VERSION, DATE
            )?;
            writeln!(out)?;
            writeln!(
                out,
                "FIGlet, along with the various FIGlet fonts and documentation, may be"
            )?;
            writeln!(out, "freely copied and distributed.")?;
            writeln!(out)?;
            writeln!(
                out,
                "If you use FIGlet, please send an e-mail message to <info@figlet.org>."
            )?;
            writeln!(out)?;
            writeln!(
                out,
                "The latest version of FIGlet is available from the web site,"
            )?;
            writeln!(out, "\thttp://www.figlet.org/")?;
            writeln!(out)?;
            printusage(out, myname)?;
        }
        1 => {
            writeln!(out, "{}", VERSION_INT)?;
        }
        2 => {
            writeln!(out, "{}", config.fontdirname)?;
        }
        3 => {
            writeln!(out, "{}", config.fontname)?;
        }
        4 => {
            writeln!(out, "{}", config.outputwidth)?;
        }
        5 => {
            write!(out, "{}", FONTFILE_MAGIC)?;
            write!(out, " {}", TOILETFILE_MAGIC)?;
            writeln!(out)?;
        }
        _ => {}
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn get_columns() -> Option<u16> {
    ratatui::crossterm::terminal::size()
        .ok()
        .map(|(cols, _)| cols)
}

#[cfg(target_arch = "wasm32")]
fn get_columns() -> Option<u16> {
    None
}

/// Writer that records the first failed write in `broken` and fails every
/// write after it, so the render loop can stop instead of rendering the rest
/// of the input into a closed pipe (`figby … | head -1`).
struct BrokenWriter<'a, W: Write> {
    inner: W,
    broken: &'a Cell<bool>,
}

impl<W: Write> Write for BrokenWriter<'_, W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.broken.get() {
            return Err(io::Error::from(io::ErrorKind::BrokenPipe));
        }
        let r = self.inner.write(buf);
        if r.is_err() {
            self.broken.set(true);
        }
        r
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.broken.get() {
            return Err(io::Error::from(io::ErrorKind::BrokenPipe));
        }
        let r = self.inner.flush();
        if r.is_err() {
            self.broken.set(true);
        }
        r
    }
}

#[allow(clippy::ptr_arg, clippy::too_many_arguments)]
fn flush_output_line(
    line: &mut OutputLine,
    font: &FIGfont,
    justification: Justification,
    output_width: usize,
    char_buffer: &mut Vec<u32>,
    out: &mut impl Write,
) {
    let rendered = render_line(&line.to_rows(), font.hardblank, justification, output_width);
    for row in &rendered {
        let _ = writeln!(out, "{}", row);
    }
    line.clear();
    char_buffer.clear();
}

fn run(config: CliConfig, message: Vec<String>) {
    let mut dirs: Vec<&str> = vec![&config.fontdirname];
    dirs.extend(font::DEFAULT_FONT_DIRS);
    let font = match font::load_font(&config.fontname, &dirs) {
        Ok(f) => f,
        // A fresh `cargo install` has no font directory at all; the default
        // font ships inside the binary so plain `figby Hi` still works.
        Err(_) if config.fontname == "standard" => {
            match font::parse_tlf_font(EMBEDDED_STANDARD_FONT) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("Error: embedded standard font: {e}");
                    process::exit(1);
                }
            }
        }
        Err(e) => {
            eprintln!("Error: {e}");
            process::exit(1);
        }
    };

    // Control files apply in command-line order into one state, like C's
    // readcontrolfiles(): commands accumulate, the last input mode wins.
    let mut control_state = control::ControlState::default();
    for name in &config.controlfiles {
        // Resolve like C FIGopen(): `<fontdir>/<name>.flc`, then `<name>.flc`.
        let path = match control::resolve_control_file(name, &dirs) {
            Some(p) => p,
            None => {
                eprintln!("Error: {name}: Unable to open control file");
                process::exit(1);
            }
        };
        if let Err(e) = control::read_control(&path, &mut control_state) {
            eprintln!("Error reading control file: {e}");
            process::exit(1);
        }
    }

    let smush_mode = match config.smushoverride {
        SmushOverride::No => SmushMode::new(font.full_layout as u32),
        SmushOverride::Yes => SmushMode::new(config.smushmode),
        SmushOverride::Force => SmushMode::new(font.full_layout as u32 | config.smushmode),
    };

    let rtl = if config.right2left == -1 {
        font.print_direction == 1
    } else {
        config.right2left == 1
    };
    let justification = if config.justification == -1 {
        if rtl {
            Justification::Right
        } else {
            Justification::Left
        }
    } else {
        Justification::from_i32(config.justification)
    };

    let outlinelen_limit = config.outputwidth.saturating_sub(1) as usize;
    let output_width = config.outputwidth as usize;
    let height = font.charheight as usize;

    // The line under construction; see render::OutputLine for why it is not
    // a Vec<String> (that made long lines quadratic).
    let mut line = OutputLine::new(height);
    let mut char_buffer: Vec<u32> = Vec::new();
    let mut wordbreakmode: i32 = 0;
    let mut last_was_eol_flag: bool = false;

    let mut input = InputIter::new(message, config.cmdinput);
    // `--to-file` / `--output`: render into memory, then replace the file
    // atomically at the end so a failed run never leaves a partial banner.
    let mut file_buf: Vec<u8> = Vec::new();
    let stdout = io::stdout();
    let mut stdout_lock = stdout.lock();
    // Set by BrokenWriter on the first failed write (EPIPE: the reader went
    // away). The loop below checks it and stops rendering.
    let broken = Cell::new(false);
    let inner: &mut dyn Write = if config.to_file.is_some() {
        &mut file_buf
    } else {
        &mut stdout_lock
    };
    let mut out = BrokenWriter {
        inner,
        broken: &broken,
    };
    let mut hz_state = input::HZState::default();

    let next_char = |input: &mut InputIter,
                     state: &mut control::ControlState,
                     hz: &mut input::HZState|
     -> Option<u32> {
        // The input mode comes from the control files (`b`/`u`/`h`/`j`/`g`),
        // exactly like C's `multibyte` global; `-N` only resets it before
        // they are read.
        match state.multibyte {
            0 => state.iso2022(input),
            1 | 4 => input::read_dbcs_char(input),
            2 => input::read_utf8_char(input),
            3 => input::read_hz_char(input, hz),
            _ => input.next(),
        }
    };

    let mut pending_char: Option<u32> = None;
    loop {
        if broken.get() {
            break;
        }
        let c = match pending_char
            .take()
            .or_else(|| next_char(&mut input, &mut control_state, &mut hz_state))
        {
            Some(c) => c,
            None => {
                if !line.is_empty() {
                    flush_output_line(
                        &mut line,
                        &font,
                        justification,
                        output_width,
                        &mut char_buffer,
                        &mut out,
                    );
                }
                break;
            }
        };

        let mut c = c;

        // Paragraph mode
        if c == b'\n' as u32 && config.paragraphflag && !last_was_eol_flag {
            match next_char(&mut input, &mut control_state, &mut hz_state) {
                None => {
                    // Trailing newline at EOF becomes a space (matches C figlet behavior)
                    c = b' ' as u32;
                }
                Some(c2) => {
                    let is_ws = c2 <= 127 && (c2 as u8 as char).is_ascii_whitespace();
                    if !is_ws {
                        c = b' ' as u32;
                    }
                    // Push back the *decoded* character (C getinchr_buffer).
                    // Ungetting it into the byte reader re-decoded it as a
                    // raw byte, so a multibyte character starting a line
                    // vanished in `-p -C utf8` mode.
                    pending_char = Some(c2);
                }
            }
        }

        // Update last_was_eol_flag
        last_was_eol_flag = c <= 127
            && (c as u8 as char).is_ascii_whitespace()
            && c != b'\t' as u32
            && c != b' ' as u32;

        c = input::deutsch_reroute(c, config.deutschflag);

        c = control::remap_char(&control_state, c);

        // Space normalization
        if c <= 127 && (c as u8 as char).is_ascii_whitespace() {
            c = if c == b'\t' as u32 || c == b' ' as u32 {
                b' ' as u32
            } else {
                b'\n' as u32
            };
        }

        // Skip control chars 1-31 (except \n) and 127 (DEL)
        if (c > 0 && c < b' ' as u32 && c != b'\n' as u32) || c == 127 {
            continue;
        }

        // Inner loop (handles addchar retry after split/print, like C do-while)
        // Every branch either breaks (char handled) or falls through to retry
        loop {
            if wordbreakmode == -1 {
                if c == b' ' as u32 {
                    break;
                } else if c == b'\n' as u32 {
                    wordbreakmode = 0;
                    break;
                }
                wordbreakmode = 0;
            }

            if c == b'\n' as u32 {
                flush_output_line(
                    &mut line,
                    &font,
                    justification,
                    output_width,
                    &mut char_buffer,
                    &mut out,
                );
                wordbreakmode = 0;
                break;
            }

            if line.add_char(&font, c, smush_mode, rtl, outlinelen_limit) {
                char_buffer.push(c);
                if c != b' ' as u32 {
                    wordbreakmode = if wordbreakmode >= 2 { 3 } else { 1 };
                } else {
                    wordbreakmode = if wordbreakmode > 0 { 2 } else { 0 };
                }
                break;
            }

            if line.is_empty() {
                // Raw-char path: char wider than empty line
                let mut dummy = 0;
                let ch = lookup_char(&font, c, &mut dummy);
                let rows = ch.rows();
                let raw_rows: Vec<String> = if rtl && output_width > 1 {
                    rows.iter()
                        .map(|row| {
                            let len = row.chars().count();
                            let start = len.saturating_sub(outlinelen_limit);
                            row.chars().skip(start).collect()
                        })
                        .collect()
                } else {
                    rows.to_vec()
                };
                let rendered = render_line(&raw_rows, font.hardblank, justification, output_width);
                for row in &rendered {
                    let _ = writeln!(out, "{}", row);
                }
                wordbreakmode = -1;
                break;
            }

            // addchar failed — need to flush current line and retry
            if c == b' ' as u32 {
                if wordbreakmode == 2 {
                    if let Some((part1_rows, part2_start)) =
                        line.split(&font, &char_buffer, smush_mode, rtl, outlinelen_limit)
                    {
                        let rendered =
                            render_line(&part1_rows, font.hardblank, justification, output_width);
                        for row in &rendered {
                            let _ = writeln!(out, "{}", row);
                        }
                        char_buffer.drain(..part2_start);
                    } else {
                        flush_output_line(
                            &mut line,
                            &font,
                            justification,
                            output_width,
                            &mut char_buffer,
                            &mut out,
                        );
                    }
                } else {
                    flush_output_line(
                        &mut line,
                        &font,
                        justification,
                        output_width,
                        &mut char_buffer,
                        &mut out,
                    );
                }
                wordbreakmode = -1;
                break;
            }

            // Non-space char that didn't fit — retry after flush/split
            if wordbreakmode >= 2 {
                if let Some((part1_rows, part2_start)) =
                    line.split(&font, &char_buffer, smush_mode, rtl, outlinelen_limit)
                {
                    let rendered =
                        render_line(&part1_rows, font.hardblank, justification, output_width);
                    for row in &rendered {
                        let _ = writeln!(out, "{}", row);
                    }
                    char_buffer.drain(..part2_start);
                } else {
                    flush_output_line(
                        &mut line,
                        &font,
                        justification,
                        output_width,
                        &mut char_buffer,
                        &mut out,
                    );
                }
            } else {
                flush_output_line(
                    &mut line,
                    &font,
                    justification,
                    output_width,
                    &mut char_buffer,
                    &mut out,
                );
            }
            wordbreakmode = if wordbreakmode == 3 { 1 } else { 0 };
            // loop continues (retry addchar with fresh output line)
        }
    }

    if let Some(ref path) = config.to_file {
        write_output_file(path, &file_buf);
    }
}

/// Atomically replace `path` with `bytes` for `--to-file` / `--output`;
/// exits with status 1 if the write fails.
fn write_output_file(path: &str, bytes: &[u8]) {
    if let Err(e) = figby::atomic_io::atomic_write(std::path::Path::new(path), bytes) {
        eprintln!("Error writing to '{path}': {e}");
        process::exit(1);
    }
}

fn has_figlet_flags(args: &CliArgs) -> bool {
    args.flag_A
        || args.flag_D
        || args.flag_E
        || args.flag_X
        || args.flag_L
        || args.flag_R
        || args.flag_x
        || args.flag_l
        || args.flag_c
        || args.flag_r
        || args.flag_p
        || args.flag_n
        || args.flag_s
        || args.flag_k
        || args.flag_S
        || args.flag_o
        || args.flag_W
        || args.flag_t
        || args.flag_N
        || args.flag_F
        || args.smushmode_arg.is_some()
        || args.outputwidth_arg.is_some()
        || args.fontdir.is_some()
        || args.fontname_arg.is_some()
        || !args.controlfile.is_empty()
}

#[cfg(target_arch = "wasm32")]
fn main() {
    figby::web::run_web().expect("Figby web error");
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    // `figby play|bake|image|font|tui ...` when the first argument names a
    // subcommand; otherwise the FIGlet CLI, parsed exactly as before.
    let args =
        cli::subcommands::parse_args(std::env::args_os().collect()).unwrap_or_else(|e| e.exit());
    // `-I` is read with atoi in C, so `-I x` means 0.
    let infocode = args.infocode.as_deref().map(|s| c_atoi(s) as i32);

    // `--hold-last` is in seconds on the command line and centiseconds in every
    // on-disk timing field (the GIF convention this codebase already uses).
    // Non-finite, zero and negative values all mean "no hold".
    let hold_last_cs = args
        .hold_last
        .filter(|s| s.is_finite() && *s > 0.0)
        .map(|s| (s * 100.0).round().clamp(0.0, f64::from(u16::MAX)) as u16)
        .unwrap_or(0);

    // Bake and playback are whole-pipeline programs, not rendering options.
    if let Some(ref path) = args.bake_path {
        cli::playback::run_bake(&args, path, hold_last_cs);
        return;
    }

    if let Some(ref path) = args.play_path {
        cli::playback::run_play(&args, path, hold_last_cs);
        return;
    }

    if cli::playback::run_spawned_playback(&args) {
        return;
    }

    if args.flag_tui {
        let mut app = figby::tui::TuiApp::new();
        if let Some(mode) = args.tui_render_mode.as_deref() {
            match mode {
                "fast" | "Fast" => app.ctx.render_mode = figby::tui::RenderMode::Fast,
                "dirty" | "Dirty" => app.ctx.render_mode = figby::tui::RenderMode::Dirty,
                other => eprintln!("Unknown render mode '{other}', using default"),
            }
        }
        if let Err(e) = app.run() {
            eprintln!("TUI error: {e}");
            process::exit(1);
        }
        return;
    }

    if cli::create_font::run_create_font(&args) {
        return;
    }

    if args.flag_F {
        eprintln!(
            "Error: the -F option was removed (as in C FIGlet 2.2.5); \
             use 'figlist' to list fonts."
        );
        process::exit(1);
    }

    if let Some(ref path) = args.render_template {
        cli::template::run_template(&args, path);
        return;
    }

    let message = args.message.clone();

    if cli::image::is_image_mode(&args) {
        let img_config = cli::image::ImageOptions::from_args(&args);
        if !cli::image::run_image(img_config) {
            process::exit(1);
        }
        return;
    }

    // No message, no FIGlet flags, and stdin is a terminal → launch TUI
    if message.is_empty()
        && !has_figlet_flags(&args)
        && args.create_font_name.is_none()
        && args.create_font_path.is_none()
        && args.render_template.is_none()
        && infocode.is_none()
        && !args.flag_v
        && std::io::stdin().is_terminal()
    {
        let mut app = figby::tui::TuiApp::new();
        if let Err(e) = app.run() {
            eprintln!("TUI error: {e}");
            process::exit(1);
        }
        return;
    }

    if args.flag_v {
        let mut stdout = io::stdout().lock();
        let _ = writeln!(stdout, "figby {}", env!("CARGO_PKG_VERSION"));
        process::exit(0);
    }

    let config_file = config::load_config();
    let config = CliConfig::from_args_with_config(args, &config_file);

    if let Some(infocode) = infocode {
        let myname = match std::env::args().next() {
            Some(s) => {
                let s = s.rsplit('/').next().unwrap_or(&s);
                s.to_string()
            }
            None => "figby".to_string(),
        };
        let mut stdout = io::stdout().lock();
        if let Err(e) = printinfo(&mut stdout, infocode, &config, &myname) {
            eprintln!("Error writing info: {e}");
            process::exit(1);
        }
        process::exit(0);
    }

    run(config, message);
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    /// A sink that fails like a closed pipe after `ok_bytes` bytes.
    struct ClosedAfter(usize);
    impl Write for ClosedAfter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            if self.0 == 0 {
                return Err(io::Error::from(io::ErrorKind::BrokenPipe));
            }
            let n = buf.len().min(self.0);
            self.0 -= n;
            Ok(n)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn broken_writer_latches_first_failure() {
        let broken = Cell::new(false);
        let mut w = BrokenWriter {
            inner: ClosedAfter(4),
            broken: &broken,
        };
        assert!(writeln!(w, "ab").is_ok());
        assert!(!broken.get());
        assert!(writeln!(w, "cdef").is_err());
        assert!(broken.get(), "first failed write must set the flag");
        // Later writes fail without touching the inner writer again.
        assert!(writeln!(w, "x").is_err());
    }

    // ── 2026-11-04 review F4/F5/F7c: C getopt semantics ────────────────────

    fn cfg(argv: &[&str]) -> CliConfig {
        CliConfig::from_args(parse_cli(argv).unwrap())
    }

    #[test]
    fn test_parse_cli_last_option_wins_in_argv_order() {
        assert_eq!(cfg(&["figby", "-c", "-l"]).justification, 0);
        assert_eq!(cfg(&["figby", "-l", "-c"]).justification, 1);
        assert_eq!(
            cfg(&["figby", "-s", "-k"]).smushoverride,
            SmushOverride::Yes
        );
        assert_eq!(cfg(&["figby", "-k", "-s"]).smushoverride, SmushOverride::No);
        assert!(cfg(&["figby", "-E", "-D"]).deutschflag);
        assert!(!cfg(&["figby", "-D", "-E"]).deutschflag);
        assert_eq!(cfg(&["figby", "-L", "-R"]).right2left, 1);
        assert_eq!(cfg(&["figby", "-w", "30", "-w", "50"]).outputwidth, 50);
        assert_eq!(
            cfg(&["figby", "-f", "big", "-f", "slant.flf"]).fontname,
            "slant"
        );
        // Combined short flags keep their order too.
        assert_eq!(cfg(&["figby", "-cl"]).justification, 0);
        assert_eq!(cfg(&["figby", "-lc"]).justification, 1);
    }

    #[test]
    fn test_parse_cli_control_files_and_n() {
        assert_eq!(
            cfg(&["figby", "-C", "upper", "-C", "utf8.flc"]).controlfiles,
            ["upper", "utf8"]
        );
        assert_eq!(
            cfg(&["figby", "-C", "upper", "-N", "-C", "utf8"]).controlfiles,
            ["utf8"]
        );
        assert!(cfg(&["figby", "-C", "upper", "-N"]).controlfiles.is_empty());
    }

    #[test]
    fn test_width_and_mode_use_c_atoi() {
        assert_eq!(cfg(&["figby", "-w", "0"]).outputwidth, 80);
        assert_eq!(cfg(&["figby", "-w", "-5"]).outputwidth, 80);
        assert_eq!(cfg(&["figby", "-w", "abc"]).outputwidth, 80);
        assert_eq!(cfg(&["figby", "-w", "40x"]).outputwidth, 40);
        let m = cfg(&["figby", "-m", "abc"]);
        assert_eq!((m.smushmode, m.smushoverride), (64, SmushOverride::Yes));
        assert_eq!(c_atoi("  -12z"), -12);
        assert_eq!(c_atoi("+7"), 7);
        assert_eq!(c_atoi(""), 0);
        assert_eq!(c_atoi("99999999999999999999999"), i64::MAX);
    }

    #[test]
    fn test_strip_font_suffix() {
        assert_eq!(strip_font_suffix("slant.flf"), "slant");
        assert_eq!(strip_font_suffix("tests/emboss.tlf"), "tests/emboss");
        assert_eq!(strip_font_suffix("big"), "big");
    }

    use figby::font::DEUTSCH_CHARS;

    #[test]
    fn test_default_values() {
        let args = CliArgs::try_parse_from(["figby"]).unwrap();
        let config = CliConfig::from_args_with_config(args, &FigbyConfig::default());
        assert_eq!(config.smushmode, 0);
        assert_eq!(config.smushoverride, SmushOverride::No);
        assert_eq!(config.justification, -1);
        assert_eq!(config.right2left, -1);
        assert!(!config.paragraphflag);
        assert!(!config.deutschflag);
        assert!(!config.cmdinput);
        assert_eq!(config.outputwidth, 80);
        assert_eq!(config.fontdirname, "/usr/share/figlet");
        assert_eq!(config.fontname, "standard");
        assert_eq!(config.multibyte, 0);
    }

    #[test]
    fn test_flag_A_cmdinput() {
        let args = CliArgs::try_parse_from(["figby", "-A"]).unwrap();
        let config = CliConfig::from_args_with_config(args, &FigbyConfig::default());
        assert!(config.cmdinput);
    }

    #[test]
    fn test_flag_D_deutsch() {
        let args = CliArgs::try_parse_from(["figby", "-D"]).unwrap();
        let config = CliConfig::from_args_with_config(args, &FigbyConfig::default());
        assert!(config.deutschflag);
    }

    #[test]
    fn test_flag_E_deutsch() {
        let args = CliArgs::try_parse_from(["figby", "-E"]).unwrap();
        let config = CliConfig::from_args_with_config(args, &FigbyConfig::default());
        assert!(!config.deutschflag);
    }

    #[test]
    fn test_flags_X_L_R_right2left() {
        let args_x = CliArgs::try_parse_from(["figby", "-X"]).unwrap();
        assert_eq!(CliConfig::from_args(args_x).right2left, -1);

        let args_l = CliArgs::try_parse_from(["figby", "-L"]).unwrap();
        assert_eq!(CliConfig::from_args(args_l).right2left, 0);

        let args_r = CliArgs::try_parse_from(["figby", "-R"]).unwrap();
        assert_eq!(CliConfig::from_args(args_r).right2left, 1);
    }

    #[test]
    fn test_flags_x_l_c_r_justification() {
        let args_x = CliArgs::try_parse_from(["figby", "-x"]).unwrap();
        assert_eq!(CliConfig::from_args(args_x).justification, -1);

        let args_l = CliArgs::try_parse_from(["figby", "-l"]).unwrap();
        assert_eq!(CliConfig::from_args(args_l).justification, 0);

        let args_c = CliArgs::try_parse_from(["figby", "-c"]).unwrap();
        assert_eq!(CliConfig::from_args(args_c).justification, 1);

        let args_r = CliArgs::try_parse_from(["figby", "-r"]).unwrap();
        assert_eq!(CliConfig::from_args(args_r).justification, 2);
    }

    #[test]
    fn test_flags_p_n_paragraph() {
        let args_p = CliArgs::try_parse_from(["figby", "-p"]).unwrap();
        assert!(CliConfig::from_args(args_p).paragraphflag);

        let args_n = CliArgs::try_parse_from(["figby", "-n"]).unwrap();
        assert!(!CliConfig::from_args(args_n).paragraphflag);
    }

    #[test]
    fn test_flags_s_k_S_o_W_smush() {
        let args_s = CliArgs::try_parse_from(["figby", "-s"]).unwrap();
        let config_s = CliConfig::from_args(args_s);
        assert_eq!(config_s.smushoverride, SmushOverride::No);

        let args_k = CliArgs::try_parse_from(["figby", "-k"]).unwrap();
        let config_k = CliConfig::from_args(args_k);
        assert_eq!(config_k.smushmode, 64);
        assert_eq!(config_k.smushoverride, SmushOverride::Yes);

        let args_S = CliArgs::try_parse_from(["figby", "-S"]).unwrap();
        let config_S = CliConfig::from_args(args_S);
        assert_eq!(config_S.smushmode, 128);
        assert_eq!(config_S.smushoverride, SmushOverride::Force);

        let args_o = CliArgs::try_parse_from(["figby", "-o"]).unwrap();
        let config_o = CliConfig::from_args(args_o);
        assert_eq!(config_o.smushmode, 128);
        assert_eq!(config_o.smushoverride, SmushOverride::Yes);

        let args_W = CliArgs::try_parse_from(["figby", "-W"]).unwrap();
        let config_W = CliConfig::from_args(args_W);
        assert_eq!(config_W.smushmode, 0);
        assert_eq!(config_W.smushoverride, SmushOverride::Yes);
    }

    #[test]
    fn test_flag_N_multibyte() {
        let args = CliArgs::try_parse_from(["figby", "-N"]).unwrap();
        let config = CliConfig::from_args_with_config(args, &FigbyConfig::default());
        assert_eq!(config.multibyte, 0);
    }

    #[test]
    fn test_flag_t_terminal() {
        let args = CliArgs::try_parse_from(["figby", "-t"]).unwrap();
        assert!(args.flag_t);
    }

    #[test]
    fn test_flag_t_updates_width() {
        let args = CliArgs::try_parse_from(["figby", "-t"]).unwrap();
        let config = CliConfig::from_args_with_config(args, &FigbyConfig::default());
        assert!(config.outputwidth >= 80);
    }

    #[test]
    fn test_get_columns_never_panics() {
        let cols = get_columns();
        let _ = cols; // never panics, always returns Some or None
        assert!(cols.is_none() || cols.unwrap() >= 20);
    }

    #[test]
    fn test_flag_t_w_override() {
        let args = CliArgs::try_parse_from(["figby", "-t", "-w", "120"]).unwrap();
        let config = CliConfig::from_args_with_config(args, &FigbyConfig::default());
        assert_eq!(config.outputwidth, 120);
    }

    #[test]
    fn test_flag_v_version() {
        let args = CliArgs::try_parse_from(["figby", "-v"]).unwrap();
        assert!(args.flag_v);
    }

    #[test]
    fn test_flag_I_infocode() {
        let args = CliArgs::try_parse_from(["figby", "-I", "3"]).unwrap();
        assert_eq!(args.infocode.as_deref(), Some("3"));
    }

    #[test]
    fn test_flag_m_smushmode() {
        let args_0 = CliArgs::try_parse_from(["figby", "-m", "0"]).unwrap();
        let config_0 = CliConfig::from_args(args_0);
        assert_eq!(config_0.smushmode, 64);
        assert_eq!(config_0.smushoverride, SmushOverride::Yes);

        let args_neg1 = CliArgs::try_parse_from(["figby", "-m", "-1"]).unwrap();
        let config_neg1 = CliConfig::from_args(args_neg1);
        assert_eq!(config_neg1.smushmode, 0);
        assert_eq!(config_neg1.smushoverride, SmushOverride::Yes);

        let args_neg2 = CliArgs::try_parse_from(["figby", "-m", "-2"]).unwrap();
        let config_neg2 = CliConfig::from_args(args_neg2);
        assert_eq!(config_neg2.smushoverride, SmushOverride::No);

        let args_5 = CliArgs::try_parse_from(["figby", "-m", "5"]).unwrap();
        let config_5 = CliConfig::from_args(args_5);
        assert_eq!(config_5.smushmode, (5 & 63) | 128);
        assert_eq!(config_5.smushoverride, SmushOverride::Yes);
    }

    #[test]
    fn test_flag_w_width() {
        let args = CliArgs::try_parse_from(["figby", "-w", "120"]).unwrap();
        let config = CliConfig::from_args_with_config(args, &FigbyConfig::default());
        assert_eq!(config.outputwidth, 120);
    }

    #[test]
    fn test_flag_d_fontdir() {
        let args = CliArgs::try_parse_from(["figby", "-d", "/my/fonts"]).unwrap();
        let config = CliConfig::from_args_with_config(args, &FigbyConfig::default());
        assert_eq!(config.fontdirname, "/my/fonts");
    }

    #[test]
    fn test_flag_f_fontname() {
        let args = CliArgs::try_parse_from(["figby", "-f", "big"]).unwrap();
        let config = CliConfig::from_args_with_config(args, &FigbyConfig::default());
        assert_eq!(config.fontname, "big");
    }

    #[test]
    fn test_flag_C_controlfile() {
        let args = CliArgs::try_parse_from(["figby", "-C", "my.flc"]).unwrap();
        assert_eq!(args.controlfile, vec!["my.flc".to_string()]);
    }

    #[test]
    fn test_flag_to_file() {
        let args = CliArgs::try_parse_from(["figby", "--to-file", "output.txt"]).unwrap();
        let config = CliConfig::from_args_with_config(args, &FigbyConfig::default());
        assert_eq!(config.to_file, Some("output.txt".to_string()));
    }

    #[test]
    fn test_flag_F_error() {
        let args = CliArgs::try_parse_from(["figby", "-F"]).unwrap();
        assert!(args.flag_F);
    }

    #[test]
    fn test_positional_args_cmdinput() {
        let args = CliArgs::try_parse_from(["figby", "hello"]).unwrap();
        let config = CliConfig::from_args_with_config(args, &FigbyConfig::default());
        assert!(config.cmdinput);
    }

    #[test]
    fn test_flag_last_wins() {
        let args = CliArgs::try_parse_from(["figby", "-k", "-s"]).unwrap();
        let config = CliConfig::from_args_with_config(args, &FigbyConfig::default());
        assert_eq!(config.smushoverride, SmushOverride::No);
    }

    #[test]
    fn test_infocode_0_copyright() {
        let config = CliConfig::default();
        let mut buf = Vec::new();
        printinfo(&mut buf, 0, &config, "figby").unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("FIGlet Copyright (C)"));
        assert!(output.contains("figby"));
    }

    #[test]
    fn test_infocode_1_version() {
        let config = CliConfig::default();
        let mut buf = Vec::new();
        printinfo(&mut buf, 1, &config, "figby").unwrap();
        assert_eq!(buf, b"20205\n");
    }

    #[test]
    fn test_infocode_2_fontdir() {
        let config = CliConfig::default();
        let mut buf = Vec::new();
        printinfo(&mut buf, 2, &config, "figby").unwrap();
        assert_eq!(buf, b"/usr/share/figlet\n");
    }

    #[test]
    fn test_infocode_3_font() {
        let config = CliConfig::default();
        let mut buf = Vec::new();
        printinfo(&mut buf, 3, &config, "figby").unwrap();
        assert_eq!(buf, b"standard\n");
    }

    #[test]
    fn test_infocode_4_outputwidth() {
        let config = CliConfig::default();
        let mut buf = Vec::new();
        printinfo(&mut buf, 4, &config, "figby").unwrap();
        assert_eq!(buf, b"80\n");
    }

    #[test]
    fn test_infocode_5_formats() {
        let config = CliConfig::default();
        let mut buf = Vec::new();
        printinfo(&mut buf, 5, &config, "figby").unwrap();
        assert_eq!(buf, b"flf2 tlf2\n");
    }

    // --- InputIter tests ---

    #[test]
    fn test_input_iter_stdin_empty() {
        // Stdin variant reads from real stdin — cannot test without hanging
        // in harness. The Args variant covers the empty-input path.
        // Use cmdinput=true to bypass stdin.
        let mut iter = InputIter::new(vec![], true);
        assert!(iter.next().is_none());
    }

    #[test]
    fn test_input_iter_args_empty() {
        let mut iter = InputIter::new(vec![], true);
        assert!(iter.next().is_none());
    }

    #[test]
    fn test_input_iter_single_word() {
        let mut iter = InputIter::new(vec!["hello".to_string()], true);
        assert_eq!(iter.next(), Some(b'h' as u32));
        assert_eq!(iter.next(), Some(b'e' as u32));
        assert_eq!(iter.next(), Some(b'l' as u32));
        assert_eq!(iter.next(), Some(b'l' as u32));
        assert_eq!(iter.next(), Some(b'o' as u32));
        assert!(iter.next().is_none());
    }

    #[test]
    fn test_input_iter_two_words() {
        let mut iter = InputIter::new(vec!["hello".to_string(), "world".to_string()], true);
        let chars: Vec<u32> = std::iter::from_fn(|| iter.next()).collect();
        assert_eq!(
            chars,
            vec![
                b'h' as u32,
                b'e' as u32,
                b'l' as u32,
                b'l' as u32,
                b'o' as u32,
                b' ' as u32,
                b'w' as u32,
                b'o' as u32,
                b'r' as u32,
                b'l' as u32,
                b'd' as u32,
            ]
        );
    }

    #[test]
    fn test_input_iter_empty_word_middle() {
        let mut iter = InputIter::new(
            vec!["hello".to_string(), "".to_string(), "world".to_string()],
            true,
        );
        let chars: Vec<u32> = std::iter::from_fn(|| iter.next()).collect();
        assert_eq!(
            chars,
            vec![
                b'h' as u32,
                b'e' as u32,
                b'l' as u32,
                b'l' as u32,
                b'o' as u32,
                b' ' as u32,
                b'\n' as u32,
                b'w' as u32,
                b'o' as u32,
                b'r' as u32,
                b'l' as u32,
                b'd' as u32,
            ]
        );
    }

    #[test]
    fn test_input_iter_empty_word_at_end() {
        let mut iter = InputIter::new(vec!["hello".to_string(), "".to_string()], true);
        let chars: Vec<u32> = std::iter::from_fn(|| iter.next()).collect();
        assert_eq!(
            chars,
            vec![
                b'h' as u32,
                b'e' as u32,
                b'l' as u32,
                b'l' as u32,
                b'o' as u32,
                b' ' as u32,
            ]
        );
    }

    #[test]
    fn test_input_iter_unget() {
        let mut iter = InputIter::new(vec!["ab".to_string()], true);
        assert_eq!(iter.next(), Some(b'a' as u32));
        iter.unget(b'x' as u32);
        assert_eq!(iter.next(), Some(b'x' as u32));
        assert_eq!(iter.next(), Some(b'b' as u32));
        assert!(iter.next().is_none());
    }

    #[test]
    fn test_end_to_end_hello() {
        let tmpdir = std::env::temp_dir().join("figby-test-1.3.4");
        let _ = std::fs::create_dir_all(&tmpdir);
        let fontpath = tmpdir.join("testfont.flf");
        // Minimal FIGfont (height=1, baseline=0, max=1, old=0, comment=0)
        let mut font = String::from("flf2a$ 1 0 1 0 0\n");
        for code in 32..=126u32 {
            let c = char::from_u32(code).unwrap();
            font.push(c);
            font.push_str("  @\n");
        }
        for &code in &DEUTSCH_CHARS {
            let c = char::from_u32(code).unwrap();
            font.push(c);
            font.push_str("  @\n");
        }
        std::fs::write(&fontpath, &font).unwrap();
        let config = CliConfig {
            cmdinput: true,
            outputwidth: 80,
            fontdirname: tmpdir.to_str().unwrap().to_string(),
            fontname: "testfont".to_string(),
            ..Default::default()
        };
        // Exercise full pipeline: input → font → render → output
        run(config, vec!["Hello".to_string()]);
        // Should not panic. stdout captured by test framework.
        let _ = std::fs::remove_file(&fontpath);
        let _ = std::fs::remove_dir(&tmpdir);
    }

    #[test]
    fn test_input_iter_all_empty() {
        let mut iter = InputIter::new(vec!["".to_string()], true);
        assert!(iter.next().is_none());
    }

    #[test]
    fn test_input_iter_three_empty() {
        let mut iter = InputIter::new(vec!["".to_string(), "".to_string(), "".to_string()], true);
        let chars: Vec<u32> = std::iter::from_fn(|| iter.next()).collect();
        assert_eq!(chars, vec![b'\n' as u32, b'\n' as u32]);
    }

    #[test]
    fn test_input_iter_multiple_empty_then_word() {
        let mut iter = InputIter::new(vec!["".to_string(), "".to_string(), "a".to_string()], true);
        let chars: Vec<u32> = std::iter::from_fn(|| iter.next()).collect();
        assert_eq!(chars, vec![b'\n' as u32, b'\n' as u32, b'a' as u32]);
    }

    #[test]
    fn test_help_exits_with_display_help() {
        let result = CliArgs::try_parse_from(["figby", "--help"]);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().kind(),
            clap::error::ErrorKind::DisplayHelp
        );
    }

    #[test]
    fn test_long_help_contains_long_descriptions() {
        use clap::CommandFactory;
        let help = CliArgs::command().render_long_help().to_string();
        assert!(help.contains("--help"));
        assert!(help.contains("FIGlet 2.2.5"));
    }

    #[test]
    fn test_help_contains_expected_flags() {
        use clap::CommandFactory;
        let help = CliArgs::command().render_help().to_string();
        assert!(help.contains("-A"));
        assert!(help.contains("-f"));
        assert!(help.contains("MESSAGE"));
        assert!(help.contains("FIGlet"));
    }

    // --- Config override hierarchy tests ---

    #[test]
    fn test_config_cli_override_hierarchy() {
        let config_file = FigbyConfig {
            cli: figby::config::CliSection {
                font: Some("big".to_string()),
                output_width: Some(100),
            },
            tui: figby::config::TuiSection::default(),
        };

        let args = CliArgs::try_parse_from(["figby", "-f", "small", "-w", "120"]).unwrap();
        let cli_config = CliConfig::from_args_with_config(args, &config_file);

        assert_eq!(cli_config.fontname, "small");
        assert_eq!(cli_config.outputwidth, 120);
    }

    #[test]
    fn test_config_cli_fallback_to_config() {
        let config_file = FigbyConfig {
            cli: figby::config::CliSection {
                font: Some("big".to_string()),
                output_width: Some(100),
            },
            tui: figby::config::TuiSection::default(),
        };

        let args = CliArgs::try_parse_from(["figby"]).unwrap();
        let cli_config = CliConfig::from_args_with_config(args, &config_file);

        assert_eq!(cli_config.fontname, "big");
        assert_eq!(cli_config.outputwidth, 100);
    }

    #[test]
    fn test_config_partial_cli_mix() {
        let config_file = FigbyConfig {
            cli: figby::config::CliSection {
                font: None,
                output_width: Some(100),
            },
            tui: figby::config::TuiSection::default(),
        };

        let args = CliArgs::try_parse_from(["figby", "-f", "small"]).unwrap();
        let cli_config = CliConfig::from_args_with_config(args, &config_file);

        assert_eq!(cli_config.fontname, "small");
        assert_eq!(cli_config.outputwidth, 100);
    }
}
