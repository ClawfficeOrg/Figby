//! `figby image` / `--image`: convert image files to ASCII or braille text
//! on stdout. Moved out of `main.rs` verbatim.

use crate::{get_columns, CliArgs};
use figby_lib::image_input;
use std::io::{self, Write};

#[derive(Debug, Clone)]
pub(crate) struct ImageOptions {
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

pub(crate) fn is_image_mode(args: &CliArgs) -> bool {
    !args.image_paths.is_empty()
}

impl ImageOptions {
    pub(crate) fn from_args(args: &CliArgs) -> Self {
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

/// Convert each image to ASCII on stdout. Returns `false` if any input
/// failed, so the process can exit non-zero (review F13): a missing file,
/// a URL or a decode error used to print a message and still exit 0.
pub(crate) fn run_image(config: ImageOptions) -> bool {
    if config.img_width == Some(0) || config.img_height == Some(0) {
        eprintln!("Error: image --width/--height must be at least 1");
        return false;
    }
    let term_width = get_columns().unwrap_or(80) as usize;
    let mut ok = true;
    let mut stdout = io::stdout().lock();
    for path in &config.paths {
        let is_url = path.starts_with("http://") || path.starts_with("https://");
        if is_url {
            eprintln!("Error: URLs are not supported, download the image first: {path}");
            ok = false;
            continue;
        }

        let result = if config.braille {
            let matrix = match image_input::load_luminance_matrix(path) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("Error loading image '{}': {e}", path);
                    ok = false;
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
                    ok = false;
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
                    ok = false;
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

        if writeln!(stdout, "{result}").is_err() {
            // Reader went away (EPIPE): nothing left to write to.
            return ok;
        }
    }
    ok
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;
    use clap::Parser;

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
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_image(opts)));
        assert_eq!(result.ok(), Some(true), "run_image should succeed");
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
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_image(opts)));
        assert_eq!(
            result.ok(),
            Some(true),
            "run_image (braille) should succeed"
        );
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
}
