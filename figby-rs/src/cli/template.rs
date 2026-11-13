//! `figby --render-template <file.ftmp>`: parse a template and render it to
//! stdout or `--to-file` / `--output`. Moved out of `main()` verbatim.

use crate::{c_atoi, get_columns, write_output_file, CliArgs};
use figby_lib::font;
use figby_lib::template;
use std::io::{self, Write};
use std::process;

/// Render the template at `path`. Exits with status 1 on a read, parse or
/// render error.
pub(crate) fn run_template(args: &CliArgs, path: &str) {
    let content = match figby_lib::bounded_io::read_bounded_string(
        std::path::Path::new(path),
        figby_lib::bounded_io::MAX_TEXT_BYTES,
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
    let override_width = args
        .outputwidth_arg
        .as_deref()
        .map(c_atoi)
        .filter(|w| *w > 0)
        .map(|w| w.min(i64::from(u32::MAX)) as u32);

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

    match args.to_file.as_deref().or(args.output_path.as_deref()) {
        Some(dest) => {
            let mut text = String::new();
            for row in &output {
                text.push_str(row);
                text.push('\n');
            }
            write_output_file(dest, text.as_bytes());
        }
        None => {
            let stdout = io::stdout();
            let mut out = stdout.lock();
            for row in &output {
                let _ = writeln!(out, "{}", row);
            }
        }
    }
}
