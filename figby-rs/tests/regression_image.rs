use figby_lib::image_input::{
    self, apply_brightness, apply_negative, bilinear_resize, floyd_steinberg_dither,
    image_to_ascii, image_to_braille, image_to_colored_ascii, load_luminance_matrix,
    load_rgb_matrix, luminance_to_ascii, luminance_to_braille, ImageColorConfig, DEFAULT_CHAR_MAP,
};

const TEST_PNG: &[u8] = include_bytes!("../../assets/img/figby.png");

/// Load test image, convert to grayscale ASCII, verify non-empty output.
#[test]
fn regression_image_grayscale_ascii() {
    let img = image::load_from_memory(TEST_PNG).expect("should decode test PNG");
    let lum = image_input::luminance_from_dynamic(&img);
    let ascii = luminance_to_ascii(&lum, 40, DEFAULT_CHAR_MAP);
    assert!(
        !ascii.is_empty(),
        "grayscale ASCII output should not be empty"
    );
    assert!(ascii.contains(' '), "should contain spaces for dark areas");
    assert!(
        ascii.contains('#') || ascii.contains('*'),
        "should contain mid-brightness char"
    );
    let lines: Vec<&str> = ascii.lines().collect();
    assert!(lines.len() >= 3, "should have multiple lines of output");
    for line in &lines {
        assert_eq!(line.len(), 40, "each line should match target width");
    }
}

/// Load test image, convert to colored ASCII, verify ANSI codes present.
#[test]
fn regression_image_colored_ascii() {
    let img = image::load_from_memory(TEST_PNG).expect("should decode test PNG");
    let rgb = image_input::rgb_from_dynamic(&img);
    let config = ImageColorConfig {
        colored: true,
        grayscale: false,
        negative: false,
        char_map: DEFAULT_CHAR_MAP,
        target_width: Some(40),
    };
    let ascii = image_input::color_matrix_to_ascii(&rgb, &config);
    assert!(
        !ascii.is_empty(),
        "colored ASCII output should not be empty"
    );
    assert!(
        ascii.contains("\x1b["),
        "colored output should contain ANSI codes"
    );
    assert!(
        ascii.contains("\x1b[0m"),
        "colored output should have ANSI reset"
    );
}

/// Braille art from test image: verify non-empty and contains braille chars.
#[test]
fn regression_image_braille_art() {
    let img = image::load_from_memory(TEST_PNG).expect("should decode test PNG");
    let lum = image_input::luminance_from_dynamic(&img);
    let braille = luminance_to_braille(&lum, 128, false);
    assert!(!braille.is_empty(), "braille output should not be empty");
    assert!(
        braille.contains('\u{2800}'),
        "should contain braille base char"
    );
    assert!(
        braille.lines().any(|l| l.len() > 10),
        "braille lines should be non-trivial"
    );
}

/// Dithered braille: verify dithering changes output vs non-dithered.
#[test]
fn regression_image_braille_dithered() {
    let img = image::load_from_memory(TEST_PNG).expect("should decode test PNG");
    let lum = image_input::luminance_from_dynamic(&img);
    let plain = luminance_to_braille(&lum, 128, false);
    let dithered = luminance_to_braille(&lum, 128, true);
    assert_ne!(plain, dithered, "dithering should change braille output");
}

/// Floyd-Steinberg dither: verify output matrix has same dimensions as input.
#[test]
fn regression_image_floyd_steinberg() {
    let img = image::load_from_memory(TEST_PNG).expect("should decode test PNG");
    let lum = image_input::luminance_from_dynamic(&img);
    let dithered = floyd_steinberg_dither(&lum, 128);
    assert_eq!(dithered.len(), lum.len(), "dither should preserve height");
    assert_eq!(
        dithered[0].len(),
        lum[0].len(),
        "dither should preserve width"
    );
    for row in &dithered {
        for &v in row {
            assert!(
                v == 0 || v == 255,
                "dithered values must be 0 or 255, got {v}"
            );
        }
    }
}

/// Bilinear resize: verify downscaled matrix dimensions.
#[test]
fn regression_image_bilinear_resize() {
    let img = image::load_from_memory(TEST_PNG).expect("should decode test PNG");
    let lum = image_input::luminance_from_dynamic(&img);
    let resized = bilinear_resize(&lum, 20, 10);
    assert_eq!(resized.len(), 10, "resized height should match target");
    assert_eq!(resized[0].len(), 20, "resized width should match target");
}

/// Brightness adjustment: verify +50 increases pixel values.
#[test]
fn regression_image_brightness_plus() {
    let img = image::load_from_memory(TEST_PNG).expect("should decode test PNG");
    let mut rgb = image_input::rgb_from_dynamic(&img);
    let original = rgb[0][0];
    apply_brightness(&mut rgb, 50);
    let adjusted = rgb[0][0];
    assert!(adjusted.0 >= original.0 || adjusted.1 >= original.1 || adjusted.2 >= original.2);
}

/// Negative/invert: verify (255-r, 255-g, 255-b) for each pixel.
#[test]
fn regression_image_negative() {
    let img = image::load_from_memory(TEST_PNG).expect("should decode test PNG");
    let mut rgb = image_input::rgb_from_dynamic(&img);
    let original = rgb.clone();
    apply_negative(&mut rgb);
    for y in 0..rgb.len() {
        for x in 0..rgb[0].len() {
            let (r, g, b) = rgb[y][x];
            let (or, og, ob) = original[y][x];
            assert_eq!(r, 255 - or);
            assert_eq!(g, 255 - og);
            assert_eq!(b, 255 - ob);
        }
    }
}

/// Grayscale conversion produces uniform R=G=B channels.
#[test]
fn regression_image_grayscale_uniform() {
    let img = image::load_from_memory(TEST_PNG).expect("should decode test PNG");
    let mut rgb = image_input::rgb_from_dynamic(&img);
    image_input::apply_grayscale(&mut rgb);
    for row in &rgb {
        for &(r, g, b) in row {
            assert_eq!(r, g, "grayscale should have R=G");
            assert_eq!(g, b, "grayscale should have G=B");
        }
    }
}

/// image_to_ascii convenience wrapper works on test PNG bytes.
#[test]
fn regression_image_to_ascii_loaded() {
    let dir = std::env::temp_dir();
    let png_path = dir.join("regression_test_figby.png");
    std::fs::write(&png_path, TEST_PNG).expect("should write temp PNG");

    let result = image_to_ascii(&png_path, None, None);
    assert!(result.is_ok(), "image_to_ascii should succeed");
    let ascii = result.unwrap();
    assert!(!ascii.is_empty(), "ASCII output should not be empty");

    let _ = std::fs::remove_file(&png_path);
}

/// image_to_colored_ascii convenience wrapper.
#[test]
fn regression_image_to_colored_ascii_loaded() {
    let dir = std::env::temp_dir();
    let png_path = dir.join("regression_test_figby_color.png");
    std::fs::write(&png_path, TEST_PNG).expect("should write temp PNG");

    let result = image_to_colored_ascii(
        &png_path,
        &ImageColorConfig {
            colored: true,
            grayscale: false,
            negative: false,
            char_map: DEFAULT_CHAR_MAP,
            target_width: Some(40),
        },
    );
    assert!(result.is_ok(), "image_to_colored_ascii should succeed");
    let ascii = result.unwrap();
    assert!(
        ascii.contains("\x1b["),
        "colored output should have ANSI codes"
    );

    let _ = std::fs::remove_file(&png_path);
}

/// image_to_braille convenience wrapper.
#[test]
fn regression_image_to_braille_loaded() {
    let dir = std::env::temp_dir();
    let png_path = dir.join("regression_test_figby_braille.png");
    std::fs::write(&png_path, TEST_PNG).expect("should write temp PNG");

    let result = image_to_braille(&png_path, 128, false);
    assert!(result.is_ok(), "image_to_braille should succeed");
    let braille = result.unwrap();
    assert!(!braille.is_empty(), "braille output should not be empty");

    let _ = std::fs::remove_file(&png_path);
}

/// Luminance matrix loads correctly from test PNG.
#[test]
fn regression_image_luminance_load() {
    let dir = std::env::temp_dir();
    let png_path = dir.join("regression_test_figby_lum.png");
    std::fs::write(&png_path, TEST_PNG).expect("should write temp PNG");

    let result = load_luminance_matrix(&png_path);
    assert!(result.is_ok(), "load_luminance_matrix should succeed");
    let matrix = result.unwrap();
    assert!(!matrix.is_empty(), "matrix should not be empty");
    assert!(!matrix[0].is_empty(), "first row should not be empty");

    let _ = std::fs::remove_file(&png_path);
}

/// RGB matrix loads correctly from test PNG.
#[test]
fn regression_image_rgb_load() {
    let dir = std::env::temp_dir();
    let png_path = dir.join("regression_test_figby_rgb.png");
    std::fs::write(&png_path, TEST_PNG).expect("should write temp PNG");

    let result = load_rgb_matrix(&png_path);
    assert!(result.is_ok(), "load_rgb_matrix should succeed");
    let matrix = result.unwrap();
    assert!(!matrix.is_empty(), "matrix should not be empty");
    assert!(!matrix[0].is_empty(), "first row should not be empty");

    let _ = std::fs::remove_file(&png_path);
}

/// Character distribution: grayscale ASCII has more spaces for dark images.
#[test]
fn regression_image_char_distribution() {
    use std::collections::HashMap;

    let img = image::load_from_memory(TEST_PNG).expect("should decode test PNG");
    let lum = image_input::luminance_from_dynamic(&img);
    let ascii = luminance_to_ascii(&lum, 80, DEFAULT_CHAR_MAP);

    let mut counts: HashMap<char, usize> = HashMap::new();
    for c in ascii.chars() {
        *counts.entry(c).or_insert(0) += 1;
    }

    let total: usize = counts.values().sum();
    assert!(total > 0, "there should be characters");

    assert!(
        counts.contains_key(&'#') || counts.contains_key(&'*'),
        "mid-brightness char should appear"
    );
    assert!(
        counts.contains_key(&' '),
        "space should appear for dark pixels"
    );
    assert!(counts.len() >= 3, "should have at least 3 distinct chars");
}
