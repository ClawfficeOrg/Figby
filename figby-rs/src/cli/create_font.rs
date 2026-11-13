//! `figby --create-font <name>` / `--create-font-path <file>`: generate a
//! FIGfont from a system or file font and write it to stdout or `--output`.
//! Moved out of `main()` verbatim.

use crate::CliArgs;
use std::process;

/// Run `--create-font` or `--create-font-path` if either was given.
/// Returns `true` when one of them ran (the caller then returns); exits with
/// status 1 on a font or write error.
pub(crate) fn run_create_font(args: &CliArgs) -> bool {
    // Shared helpers for --create-font and --create-font-path
    let resolve_create_font_charset = || -> &'static [&'static str] {
        figby_lib::font_gen::resolve_charset(&args.create_font_charset).unwrap_or_else(|| {
            let leaked: &'static [&'static str] = Box::leak(
                args.create_font_charset
                    .split(',')
                    .map(|s| Box::leak(s.to_string().into_boxed_str()) as &'static str)
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            );
            leaked
        })
    };
    let write_font_output = |content: &str| match args.output_path {
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
        let result =
            figby_lib::font_gen::system_font_to_figfont(name, args.create_font_size, charset);
        let font = match result {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Error creating font: {e}");
                process::exit(1);
            }
        };
        let content = figby_lib::font_gen::generate_figfont(&font);
        write_font_output(&content);
        return true;
    }

    if let Some(ref path_str) = args.create_font_path {
        let charset = resolve_create_font_charset();
        let result = figby_lib::font_gen::font_file_to_figfont(
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
        let content = figby_lib::font_gen::generate_figfont(&font);
        write_font_output(&content);
        return true;
    }

    false
}
