use clap::Parser;
use figby::config::{self, FigbyConfig};
use figby::control::{self, CharReader};
use figby::font::{self, FIGfont};
use figby::image_input;
use figby::input;
use figby::render::{add_char, lookup_char, render_line, split_line, Justification};
use figby::smush::SmushMode;
use figby::template;
use std::io::{self, IsTerminal, Read, Write};
use std::process;

const VERSION_INT: i32 = 20205;
const VERSION: &str = "2.2.5";
const DATE: &str = "31 May 2012";
const FONTFILE_MAGIC: &str = "flf2";
const TOILETFILE_MAGIC: &str = "tlf2";

#[derive(Debug, Clone)]
struct ImageOptions {
    paths: Vec<String>,
    char_map: String,
    braille: bool,
    colored: bool,
    grayscale: bool,
    negative: bool,
    dither: bool,
    img_width: Option<u32>,
    img_height: Option<u32>,
    flip_x: bool,
    flip_y: bool,
}

impl Default for ImageOptions {
    fn default() -> Self {
        Self {
            paths: Vec::new(),
            char_map: image_input::DEFAULT_CHAR_MAP.to_string(),
            braille: false,
            colored: false,
            grayscale: false,
            negative: false,
            dither: false,
            img_width: None,
            img_height: None,
            flip_x: false,
            flip_y: false,
        }
    }
}

fn is_image_mode(args: &CliArgs) -> bool {
    !args.image_paths.is_empty()
}

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
    controlfile: Option<String>,
    to_file: Option<String>,
    color_mode: Option<String>,
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
            controlfile: None,
            to_file: None,
            color_mode: None,
        }
    }
}

#[allow(non_snake_case)]
#[derive(Parser, Debug)]
#[command(
    name = "figby",
    about = "Rust port of FIGlet — ASCII art banner generator",
    long_about = "FIGby is a Rust port of FIGlet 2.2.5 (Frank, Ian & Glenn's Letters).\nRenders text in large characters using FIGfont (.flf) and TOIlet (.tlf)\nfont files with kerning, smushing, and multi-byte character support."
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
    infocode: Option<i32>,
    #[arg(
        short = 'm',
        allow_hyphen_values = true,
        help = "Set smush mode (-1=kerning, 0=default, >0=smush with mode)"
    )]
    smushmode_arg: Option<i32>,
    #[arg(short = 'w', help = "Set output width in columns [default: 80]")]
    outputwidth_arg: Option<u32>,
    #[arg(short = 'd', help = "Font directory path")]
    fontdir: Option<String>,
    #[arg(short = 'f', help = "Font name to use [default: standard]")]
    fontname_arg: Option<String>,
    #[arg(short = 'C', help = "Path to control file (.flc)")]
    controlfile: Option<String>,
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
    #[arg(long = "output", help = "Write output to file instead of stdout")]
    create_font_output: Option<String>,
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
    #[arg(long = "to-file", help = "Write output to file instead of stdout")]
    to_file: Option<String>,
    #[arg(
        short = 'i',
        long = "image",
        help = "Image file path(s) or URL(s) to convert to ASCII"
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
        help = "Play an animated GIF fullscreen in the terminal, then exit"
    )]
    play_path: Option<String>,
    #[arg(
        long = "play-width",
        help = "Scale playback to N terminal columns, preserving aspect ratio [default: fit to terminal]"
    )]
    play_width: Option<usize>,
    #[arg(
        long = "loop",
        help = "With --play: repeat the animation until any key is pressed, instead of playing once"
    )]
    play_loop: bool,
    #[arg(
        long = "play-inline",
        help = "With --play: render animation at cursor position without clearing screen"
    )]
    play_inline: bool,
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
        config.color_mode = config_file.cli.color_mode.clone();

        if !args.message.is_empty() || args.flag_A {
            config.cmdinput = true;
        }

        if args.flag_D {
            config.deutschflag = true;
        }
        if args.flag_E {
            config.deutschflag = false;
        }

        if args.flag_X {
            config.right2left = -1;
        }
        if args.flag_L {
            config.right2left = 0;
        }
        if args.flag_R {
            config.right2left = 1;
        }

        if args.flag_x {
            config.justification = -1;
        }
        if args.flag_l {
            config.justification = 0;
        }
        if args.flag_c {
            config.justification = 1;
        }
        if args.flag_r {
            config.justification = 2;
        }

        if args.flag_p {
            config.paragraphflag = true;
        }
        if args.flag_n {
            config.paragraphflag = false;
        }

        if args.flag_N {
            config.multibyte = 0;
        }

        if args.flag_W {
            config.smushmode = 0;
            config.smushoverride = SmushOverride::Yes;
        }
        if args.flag_k {
            config.smushmode = 64;
            config.smushoverride = SmushOverride::Yes;
        }
        if args.flag_o {
            config.smushmode = 128;
            config.smushoverride = SmushOverride::Yes;
        }
        if args.flag_S {
            config.smushmode = 128;
            config.smushoverride = SmushOverride::Force;
        }
        if args.flag_s {
            config.smushoverride = SmushOverride::No;
        }

        if let Some(val) = args.smushmode_arg {
            if val < -1 {
                config.smushoverride = SmushOverride::No;
            } else if val == -1 {
                config.smushmode = 0;
                config.smushoverride = SmushOverride::Yes;
            } else if val == 0 {
                config.smushmode = 64;
                config.smushoverride = SmushOverride::Yes;
            } else {
                config.smushmode = (val as u32 & 63) | 128;
                config.smushoverride = SmushOverride::Yes;
            }
        }

        if args.flag_t {
            if let Some(cols) = get_columns() {
                if cols > 0 {
                    config.outputwidth = cols as u32;
                }
            }
        }

        if let Some(val) = args.outputwidth_arg {
            config.outputwidth = val;
        }

        if let Ok(val) = std::env::var("FIGLET_FONTDIR") {
            if !val.is_empty() {
                config.fontdirname = val;
            }
        }

        if let Some(val) = args.fontdir {
            config.fontdirname = val;
        }

        if let Some(val) = args.fontname_arg {
            config.fontname = val;
        }

        config.controlfile = args.controlfile;
        config.to_file = args.to_file;

        config
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
    crossterm::terminal::size().ok().map(|(cols, _)| cols)
}

#[cfg(target_arch = "wasm32")]
fn get_columns() -> Option<u16> {
    None
}

impl ImageOptions {
    fn from_args(args: &CliArgs) -> Self {
        let char_map = args
            .map
            .clone()
            .unwrap_or_else(|| image_input::DEFAULT_CHAR_MAP.to_string());

        let (img_width, img_height) = if let Some(ref dim) = args.dimensions {
            let parts: Vec<&str> = dim.split('x').collect();
            let w = parts.first().and_then(|s| s.parse::<u32>().ok());
            let h = parts.get(1).and_then(|s| s.parse::<u32>().ok());
            (w, h)
        } else {
            (None, None)
        };

        let img_width = args.img_width_arg.or(img_width);
        let img_height = args.img_height_arg.or(img_height);

        Self {
            paths: args.image_paths.clone(),
            char_map,
            braille: args.braille,
            colored: args.color,
            grayscale: args.grayscale,
            negative: args.negative,
            dither: args.dither,
            img_width,
            img_height,
            flip_x: args.flip_x,
            flip_y: args.flip_y,
        }
    }
}

fn flip_horizontal(matrix: &[Vec<u8>]) -> Vec<Vec<u8>> {
    matrix
        .iter()
        .map(|row| row.iter().copied().rev().collect())
        .collect()
}

fn flip_vertical(matrix: &[Vec<u8>]) -> Vec<Vec<u8>> {
    matrix.iter().rev().cloned().collect()
}

fn flip_horizontal_rgb(matrix: &[Vec<image_input::RgbPixel>]) -> Vec<Vec<image_input::RgbPixel>> {
    matrix
        .iter()
        .map(|row| row.iter().copied().rev().collect())
        .collect()
}

fn flip_vertical_rgb(matrix: &[Vec<image_input::RgbPixel>]) -> Vec<Vec<image_input::RgbPixel>> {
    matrix.iter().rev().cloned().collect()
}

fn run_image(config: ImageOptions) {
    let term_width = get_columns().unwrap_or(80) as usize;
    for path in &config.paths {
        let is_url = path.starts_with("http://") || path.starts_with("https://");
        if is_url {
            eprintln!("Error: URL support not yet implemented: {path}");
            continue;
        }

        let result = if config.braille {
            let matrix = match image_input::load_luminance_matrix(path) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("Error loading image '{}': {e}", path);
                    continue;
                }
            };
            let matrix = if config.flip_x {
                flip_horizontal(&matrix)
            } else {
                matrix
            };
            let matrix = if config.flip_y {
                flip_vertical(&matrix)
            } else {
                matrix
            };
            let threshold = 128;
            let braille = image_input::luminance_to_braille(&matrix, threshold, config.dither);
            // Apply width/height restrictions via braille-to-terminal mapping
            if let Some(max_w) = config.img_width {
                let lines: Vec<&str> = braille.lines().collect();
                let truncated: Vec<String> = lines
                    .into_iter()
                    .map(|line| {
                        let end = max_w as usize;
                        if line.chars().count() > end {
                            line.chars().take(end).collect()
                        } else {
                            line.to_string()
                        }
                    })
                    .collect();
                truncated.join("\n")
            } else {
                braille
            }
        } else if config.colored || config.grayscale || config.negative {
            let matrix = match image_input::load_rgb_matrix(path) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("Error loading image '{}': {e}", path);
                    continue;
                }
            };
            let matrix = if config.flip_x {
                flip_horizontal_rgb(&matrix)
            } else {
                matrix
            };
            let matrix = if config.flip_y {
                flip_vertical_rgb(&matrix)
            } else {
                matrix
            };
            let color_config = image_input::ImageColorConfig {
                colored: config.colored,
                grayscale: config.grayscale,
                negative: config.negative,
                char_map: &config.char_map,
                target_width: config.img_width.map(|w| w as usize).or(Some(term_width)),
            };
            image_input::color_matrix_to_ascii(&matrix, &color_config)
        } else {
            let matrix = match image_input::load_luminance_matrix(path) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("Error loading image '{}': {e}", path);
                    continue;
                }
            };
            let matrix = if config.flip_x {
                flip_horizontal(&matrix)
            } else {
                matrix
            };
            let matrix = if config.flip_y {
                flip_vertical(&matrix)
            } else {
                matrix
            };
            let width = config.img_width.map(|w| w as usize).unwrap_or(term_width);
            image_input::luminance_to_ascii(&matrix, width, &config.char_map)
        };

        let result = if let Some(max_h) = config.img_height {
            let lines: Vec<&str> = result.lines().collect();
            let truncated: Vec<&str> = lines.iter().take(max_h as usize).copied().collect();
            truncated.join("\n")
        } else {
            result
        };

        println!("{}", result);
    }
}

#[allow(clippy::ptr_arg, clippy::too_many_arguments)]
fn flush_output_line(
    output_rows: &mut Vec<String>,
    font: &FIGfont,
    justification: Justification,
    output_width: usize,
    char_buffer: &mut Vec<u32>,
    outlinelen: &mut usize,
    prev_width: &mut usize,
    out: &mut impl Write,
) {
    let rendered = render_line(output_rows, font.hardblank, justification, output_width);
    for row in &rendered {
        let _ = writeln!(out, "{}", row);
    }
    for row in output_rows.iter_mut() {
        row.clear();
    }
    char_buffer.clear();
    *outlinelen = 0;
    *prev_width = 0;
}

/// Build a heightfield from a composite CanvasBuffer (for lighting normal map).
fn heightfield_from_composite(buf: &figby::tui::canvas::CanvasBuffer) -> Vec<Vec<f32>> {
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
fn apply_luminance_to_composite(
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

fn run(config: CliConfig, message: Vec<String>) {
    let mut dirs: Vec<&str> = vec![&config.fontdirname];
    dirs.extend(font::DEFAULT_FONT_DIRS);
    let font = match font::load_font(&config.fontname, &dirs) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Error: {e}");
            process::exit(1);
        }
    };

    let mut control_state = match config.controlfile {
        Some(ref path) => {
            let mut state = control::ControlState::default();
            if let Err(e) = control::read_control(path, &mut state) {
                eprintln!("Error reading control file: {e}");
                process::exit(1);
            }
            state
        }
        None => control::ControlState::default(),
    };

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

    let mut output_rows = vec![String::new(); height];
    let mut char_buffer: Vec<u32> = Vec::new();
    let mut outlinelen: usize = 0;
    let mut prev_width: usize = 0;
    let mut wordbreakmode: i32 = 0;
    let mut last_was_eol_flag: bool = false;

    let mut input = InputIter::new(message, config.cmdinput);
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let mut hz_state = input::HZState::default();

    let next_char = |input: &mut InputIter,
                     state: &mut control::ControlState,
                     hz: &mut input::HZState|
     -> Option<u32> {
        match config.multibyte {
            0 => state.iso2022(input),
            1 | 4 => input::read_dbcs_char(input),
            2 => input::read_utf8_char(input),
            3 => input::read_hz_char(input, hz),
            _ => input.next(),
        }
    };

    loop {
        let c = match next_char(&mut input, &mut control_state, &mut hz_state) {
            Some(c) => c,
            None => {
                if outlinelen != 0 {
                    flush_output_line(
                        &mut output_rows,
                        &font,
                        justification,
                        output_width,
                        &mut char_buffer,
                        &mut outlinelen,
                        &mut prev_width,
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
                    input.unget(c2);
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
                    &mut output_rows,
                    &font,
                    justification,
                    output_width,
                    &mut char_buffer,
                    &mut outlinelen,
                    &mut prev_width,
                    &mut out,
                );
                wordbreakmode = 0;
                break;
            }

            if add_char(
                &font,
                c,
                &mut output_rows,
                &mut outlinelen,
                &mut prev_width,
                smush_mode,
                rtl,
                outlinelen_limit,
            ) {
                char_buffer.push(c);
                if c != b' ' as u32 {
                    wordbreakmode = if wordbreakmode >= 2 { 3 } else { 1 };
                } else {
                    wordbreakmode = if wordbreakmode > 0 { 2 } else { 0 };
                }
                break;
            }

            if outlinelen == 0 {
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
                    if let Some((part1_rows, part2_start)) = split_line(
                        &font,
                        &char_buffer,
                        &mut output_rows,
                        &mut outlinelen,
                        &mut prev_width,
                        smush_mode,
                        rtl,
                        outlinelen_limit,
                    ) {
                        let rendered =
                            render_line(&part1_rows, font.hardblank, justification, output_width);
                        for row in &rendered {
                            let _ = writeln!(out, "{}", row);
                        }
                        char_buffer.drain(..part2_start);
                    } else {
                        flush_output_line(
                            &mut output_rows,
                            &font,
                            justification,
                            output_width,
                            &mut char_buffer,
                            &mut outlinelen,
                            &mut prev_width,
                            &mut out,
                        );
                    }
                } else {
                    flush_output_line(
                        &mut output_rows,
                        &font,
                        justification,
                        output_width,
                        &mut char_buffer,
                        &mut outlinelen,
                        &mut prev_width,
                        &mut out,
                    );
                }
                wordbreakmode = -1;
                break;
            }

            // Non-space char that didn't fit — retry after flush/split
            if wordbreakmode >= 2 {
                if let Some((part1_rows, part2_start)) = split_line(
                    &font,
                    &char_buffer,
                    &mut output_rows,
                    &mut outlinelen,
                    &mut prev_width,
                    smush_mode,
                    rtl,
                    outlinelen_limit,
                ) {
                    let rendered =
                        render_line(&part1_rows, font.hardblank, justification, output_width);
                    for row in &rendered {
                        let _ = writeln!(out, "{}", row);
                    }
                    char_buffer.drain(..part2_start);
                } else {
                    flush_output_line(
                        &mut output_rows,
                        &font,
                        justification,
                        output_width,
                        &mut char_buffer,
                        &mut outlinelen,
                        &mut prev_width,
                        &mut out,
                    );
                }
            } else {
                flush_output_line(
                    &mut output_rows,
                    &font,
                    justification,
                    output_width,
                    &mut char_buffer,
                    &mut outlinelen,
                    &mut prev_width,
                    &mut out,
                );
            }
            wordbreakmode = if wordbreakmode == 3 { 1 } else { 0 };
            // loop continues (retry addchar with fresh output line)
        }
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
        || args.controlfile.is_some()
}

#[cfg(target_arch = "wasm32")]
fn main() {
    figby::web::run_web().expect("Figby web error");
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    let args = CliArgs::parse();
    let infocode = args.infocode;

    if let Some(ref path) = args.play_path {
        let ext = std::path::Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        if ext.eq_ignore_ascii_case("figmap") {
            // ─── .figmap playback ───────────────────────────────────
            let figmap = match figby::figmap::load_figmap(std::path::Path::new(path)) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("Error loading figmap '{path}': {e}");
                    process::exit(1);
                }
            };
            let (layers, timeline, lights, _palette) = figby::figmap::into_runtime(figmap);
            let timeline = match timeline {
                Some(t) if !t.frames.is_empty() => t,
                _ => {
                    eprintln!("Figmap '{path}' has no animation frames to play");
                    process::exit(1);
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
                        let (fg_lum, bg_lum) = figby::tui::lighting::shade_canvas(
                            &interpolated,
                            &nmap,
                            |x: u16, y: u16| {
                                composite.get(x as usize, y as usize).is_some_and(|c| {
                                    c.ch != ' ' || c.fg.is_some() || c.bg.is_some()
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
            if let Err(e) =
                figby::tui::player::play_raw_timed(frame_cells, fps, Some(delays), args.play_loop, args.play_inline)
            {
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
                let (cols, rows) = crossterm::terminal::size().unwrap_or((80, 24));
                figby::gif_import::GifScaleTarget::FitBox(
                    cols as usize,
                    (rows as usize).saturating_sub(1).max(1),
                )
            }
        };
        let gif_result =
            match figby::gif_import::import_gif_scaled(std::path::Path::new(path), scale) {
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
            args.play_loop,
            args.play_inline,
        ) {
            eprintln!("Playback error: {e}");
            process::exit(1);
        }
        return;
    }

    // ─── Daemon/tmux/detach modes ───────────────────────────────
    if let Some(ref path) = args.play_daemon {
        let exe = std::env::current_exe().unwrap_or_else(|_| "figby".into());
        let mut cmd = std::process::Command::new(exe);
        cmd.arg("--play").arg(path);
        if args.play_loop {
            cmd.arg("--loop");
        }
        match cmd.spawn() {
            Ok(child) => {
                println!("figby playback started (PID {})", child.id());
                return;
            }
            Err(e) => {
                eprintln!("Failed to start daemon: {e}");
                process::exit(1);
            }
        }
    }

    if let Some(ref path) = args.play_tmux {
        let in_tmux = std::env::var("TMUX").is_ok();
        if in_tmux {
            let exe = std::env::current_exe().unwrap_or_else(|_| "figby".into());
            let mut play_cmd = format!("{} --play \"{}\"", exe.display(), path);
            if args.play_loop {
                play_cmd.push_str(" --loop");
            }
            let size_pct = args.play_tmux_size.unwrap_or(50).clamp(10, 90);
            let size_arg = format!("-p{size_pct}");
            let status = std::process::Command::new("tmux")
                .args(["split-window", "-h", &size_arg, &play_cmd])
                .status();
            match status {
                Ok(s) if s.success() => return,
                Ok(_) => {
                    eprintln!("tmux split-window failed, falling back to daemon mode");
                }
                Err(e) => {
                    eprintln!("tmux not available ({e}), falling back to daemon mode");
                }
            }
        }
        // Fallback: daemon mode
        let exe = std::env::current_exe().unwrap_or_else(|_| "figby".into());
        let mut cmd = std::process::Command::new(exe);
        cmd.arg("--play").arg(path);
        if args.play_loop {
            cmd.arg("--loop");
        }
        match cmd.spawn() {
            Ok(child) => {
                println!("figby playback started (PID {})", child.id());
                return;
            }
            Err(e) => {
                eprintln!("Failed to start daemon: {e}");
                process::exit(1);
            }
        }
    }

    if let Some(ref path) = args.play_detach {
        let exe = std::env::current_exe().unwrap_or_else(|_| "figby".into());
        let mut cmd = std::process::Command::new(exe);
        cmd.arg("--play").arg(path);
        if args.play_loop {
            cmd.arg("--loop");
        }
        match cmd.status() {
            Ok(s) => {
                if !s.success() {
                    process::exit(s.code().unwrap_or(1));
                }
                return;
            }
            Err(e) => {
                eprintln!("Failed to start playback: {e}");
                process::exit(1);
            }
        }
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

    // Shared helpers for --create-font and --create-font-path
    let resolve_create_font_charset = || -> &'static [&'static str] {
        figby::font_gen::resolve_charset(&args.create_font_charset).unwrap_or_else(|| {
            let leaked: &'static [&'static str] = Box::leak(
                args.create_font_charset
                    .split(',')
                    .map(|s| Box::leak(s.trim().to_string().into_boxed_str()) as &'static str)
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            );
            leaked
        })
    };
    let write_font_output = |content: &str| match args.create_font_output {
        Some(ref path) => {
            if let Err(e) = std::fs::write(path, content) {
                eprintln!("Error writing to '{}': {}", path, e);
                process::exit(1);
            }
        }
        None => {
            print!("{}", content);
        }
    };

    if let Some(ref name) = args.create_font_name {
        let charset = resolve_create_font_charset();
        let result = figby::font_gen::system_font_to_figfont(name, args.create_font_size, charset);
        let font = match result {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Error creating font: {e}");
                process::exit(1);
            }
        };
        let content = figby::font_gen::generate_figfont(&font);
        write_font_output(&content);
        return;
    }

    if let Some(ref path_str) = args.create_font_path {
        let charset = resolve_create_font_charset();
        let result = figby::font_gen::font_file_to_figfont(
            std::path::Path::new(path_str),
            args.create_font_size,
            charset,
        );
        let font = match result {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Error creating font from file: {e}");
                process::exit(1);
            }
        };
        let content = figby::font_gen::generate_figfont(&font);
        write_font_output(&content);
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
        let content = match figby::bounded_io::read_bounded_string(
            std::path::Path::new(path),
            figby::bounded_io::MAX_TEXT_BYTES,
        ) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Error reading template file '{}': {}", path, e);
                process::exit(1);
            }
        };

        let tmpl = match template::parse_ftmp(&content) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("Template parse error: {}", e);
                process::exit(1);
            }
        };

        let font_dir = if let Some(ref d) = args.fontdir {
            d.clone()
        } else if let Ok(val) = std::env::var("FIGLET_FONTDIR") {
            if !val.is_empty() {
                val
            } else {
                font::DEFAULT_FONT_DIRS[0].to_string()
            }
        } else {
            font::DEFAULT_FONT_DIRS[0].to_string()
        };

        let term_width = get_columns().unwrap_or(80) as u32;
        let override_width = args.outputwidth_arg;

        let config = template::RenderConfig {
            font_dir,
            term_width,
            override_width,
            // CLI templates: containment base is the template file's
            // directory; env expansion stays opt-in (off).
            base_dir: std::path::Path::new(path).parent().map(|d| d.to_path_buf()),
            expand_env: false,
        };

        let output = match template::render_template(&tmpl, &config) {
            Ok(o) => o,
            Err(e) => {
                eprintln!("Template render error: {}", e);
                process::exit(1);
            }
        };

        let stdout = io::stdout();
        let mut out = stdout.lock();
        for row in &output {
            let _ = writeln!(out, "{}", row);
        }

        return;
    }

    let message = args.message.clone();

    if is_image_mode(&args) {
        let img_config = ImageOptions::from_args(&args);
        run_image(img_config);
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
        assert_eq!(args.infocode, Some(3));
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
        assert_eq!(args.controlfile, Some("my.flc".to_string()));
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

    // --- Image CLI flag tests ---

    const TEST_IMG: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../assets/img/figby.png");

    fn test_img_path() -> &'static str {
        TEST_IMG
    }

    #[test]
    fn test_image_flag_image() {
        let args = CliArgs::try_parse_from(["figby", "--image", "path.png"]).unwrap();
        assert_eq!(args.image_paths, vec!["path.png"]);
    }

    #[test]
    fn test_image_flag_braille() {
        let args = CliArgs::try_parse_from(["figby", "--image", "x.png", "--braille"]).unwrap();
        assert!(args.braille);
    }

    #[test]
    fn test_image_flag_color() {
        let args = CliArgs::try_parse_from(["figby", "--image", "x.png", "--color"]).unwrap();
        assert!(args.color);
    }

    #[test]
    fn test_image_flag_grayscale() {
        let args = CliArgs::try_parse_from(["figby", "--image", "x.png", "--grayscale"]).unwrap();
        assert!(args.grayscale);
    }

    #[test]
    fn test_image_flag_negative() {
        let args = CliArgs::try_parse_from(["figby", "--image", "x.png", "--negative"]).unwrap();
        assert!(args.negative);
    }

    #[test]
    fn test_image_flag_dither() {
        let args = CliArgs::try_parse_from(["figby", "--image", "x.png", "--dither"]).unwrap();
        assert!(args.dither);
    }

    #[test]
    fn test_image_flag_map() {
        let args = CliArgs::try_parse_from(["figby", "--image", "x.png", "--map", "@#$"]).unwrap();
        let opts = ImageOptions::from_args(&args);
        assert_eq!(opts.char_map, "@#$");
    }

    #[test]
    fn test_image_flag_width() {
        let args = CliArgs::try_parse_from(["figby", "--image", "x.png", "--width", "60"]).unwrap();
        let opts = ImageOptions::from_args(&args);
        assert_eq!(opts.img_width, Some(60));
    }

    #[test]
    fn test_image_flag_height() {
        let args =
            CliArgs::try_parse_from(["figby", "--image", "x.png", "--height", "30"]).unwrap();
        let opts = ImageOptions::from_args(&args);
        assert_eq!(opts.img_height, Some(30));
    }

    #[test]
    fn test_image_flag_dimensions() {
        let args = CliArgs::try_parse_from(["figby", "--image", "x.png", "--dimensions", "80x40"])
            .unwrap();
        let opts = ImageOptions::from_args(&args);
        assert_eq!(opts.img_width, Some(80));
        assert_eq!(opts.img_height, Some(40));
    }

    #[test]
    fn test_image_flag_dimensions_override() {
        let args = CliArgs::try_parse_from([
            "figby",
            "--image",
            "x.png",
            "--dimensions",
            "80x40",
            "--width",
            "100",
        ])
        .unwrap();
        let opts = ImageOptions::from_args(&args);
        assert_eq!(opts.img_width, Some(100));
        assert_eq!(opts.img_height, Some(40));
    }

    #[test]
    fn test_image_flag_flipX_flipY() {
        let args =
            CliArgs::try_parse_from(["figby", "--image", "x.png", "--flipX", "--flipY"]).unwrap();
        assert!(args.flip_x);
        assert!(args.flip_y);
    }

    #[test]
    fn test_image_flag_multiple_paths() {
        let args =
            CliArgs::try_parse_from(["figby", "--image", "a.png", "--image", "b.png"]).unwrap();
        assert_eq!(args.image_paths, vec!["a.png", "b.png"]);
    }

    #[test]
    fn test_image_mode_detection() {
        let args = CliArgs::try_parse_from(["figby"]).unwrap();
        assert!(!is_image_mode(&args));
        let args = CliArgs::try_parse_from(["figby", "--image", "x.png"]).unwrap();
        assert!(is_image_mode(&args));
    }

    #[test]
    fn test_image_integration() {
        let path = test_img_path();
        if !std::path::Path::new(path).exists() {
            return;
        }
        let args = CliArgs::try_parse_from(["figby", "--image", path, "--width", "40"]).unwrap();
        let opts = ImageOptions::from_args(&args);
        // Capture stdout — just verify no panic and produces output
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run_image(opts);
        }));
        assert!(result.is_ok(), "run_image should not panic");
    }

    #[test]
    fn test_image_integration_braille() {
        let path = test_img_path();
        if !std::path::Path::new(path).exists() {
            return;
        }
        let args =
            CliArgs::try_parse_from(["figby", "--image", path, "--braille", "--width", "40"])
                .unwrap();
        let opts = ImageOptions::from_args(&args);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run_image(opts);
        }));
        assert!(result.is_ok(), "run_image (braille) should not panic");
    }

    #[test]
    fn test_image_flag_short_i() {
        let args = CliArgs::try_parse_from(["figby", "-i", "path.png"]).unwrap();
        assert_eq!(args.image_paths, vec!["path.png"]);
    }

    #[test]
    fn test_image_flag_short_b() {
        let args = CliArgs::try_parse_from(["figby", "-i", "x.png", "-b"]).unwrap();
        assert!(args.braille);
    }

    // --- Config override hierarchy tests ---

    #[test]
    fn test_config_cli_override_hierarchy() {
        let config_file = FigbyConfig {
            cli: figby::config::CliSection {
                font: Some("big".to_string()),
                output_width: Some(100),
                color_mode: None,
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
                color_mode: None,
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
                color_mode: None,
            },
            tui: figby::config::TuiSection::default(),
        };

        let args = CliArgs::try_parse_from(["figby", "-f", "small"]).unwrap();
        let cli_config = CliConfig::from_args_with_config(args, &config_file);

        assert_eq!(cli_config.fontname, "small");
        assert_eq!(cli_config.outputwidth, 100);
    }

    #[test]
    fn test_config_color_mode_field() {
        let config_file = FigbyConfig {
            cli: figby::config::CliSection {
                font: None,
                output_width: None,
                color_mode: Some("always".to_string()),
            },
            tui: figby::config::TuiSection::default(),
        };

        let args = CliArgs::try_parse_from(["figby"]).unwrap();
        let cli_config = CliConfig::from_args_with_config(args, &config_file);
        assert_eq!(cli_config.color_mode, Some("always".to_string()));
    }

    #[test]
    fn test_config_color_mode_default() {
        let args = CliArgs::try_parse_from(["figby"]).unwrap();
        let cli_config = CliConfig::from_args_with_config(args, &FigbyConfig::default());
        assert_eq!(cli_config.color_mode, None);
    }
}
