use figby_lib::output::{export_cells_to_gif, export_cells_to_png, export_cells_to_txt};
use figby_lib::tui::canvas::CanvasCell;
use ratatui::style::Color;

fn make_test_cells(rows: usize, cols: usize, ch: char) -> Vec<Vec<CanvasCell>> {
    (0..rows)
        .map(|y| {
            (0..cols)
                .map(|x| CanvasCell {
                    ch,
                    fg: if x % 2 == 0 { Some(Color::Red) } else { None },
                    bg: if y % 2 == 0 { Some(Color::Blue) } else { None },
                    height: None,
                })
                .collect()
        })
        .collect()
}

/// Export cells to TXT, read back, verify char content matches.
#[test]
fn regression_export_txt_roundtrip() {
    let cells = make_test_cells(4, 6, 'X');
    let txt = export_cells_to_txt(&cells);
    assert_eq!(
        txt, "XXXXXX\nXXXXXX\nXXXXXX\nXXXXXX",
        "TXT export should match char grid"
    );
    assert!(
        !txt.contains('\x1b'),
        "TXT export should not contain ANSI codes"
    );
}

/// Export cells to PNG bytes, decode and verify dimensions.
#[test]
fn regression_export_png_dimensions() {
    let cells = make_test_cells(3, 5, 'A');
    let png_bytes = export_cells_to_png(&cells, 1).expect("PNG export should succeed");
    let img = image::load_from_memory(&png_bytes).expect("should decode PNG");
    // Each cell is 8x16 pixels at font_size=1
    assert_eq!(img.width(), 40);
    assert_eq!(img.height(), 48);
}

/// Export cells to PNG at font_size=2, verify larger dimensions.
#[test]
fn regression_export_png_font_size_2() {
    let cells = make_test_cells(2, 4, 'B');
    let png_bytes = export_cells_to_png(&cells, 2).expect("PNG export should succeed");
    let img = image::load_from_memory(&png_bytes).expect("should decode PNG");
    assert_eq!(img.width(), 4 * 8 * 2);
    assert_eq!(img.height(), 2 * 16 * 2);
}

/// Export cells to PNG, verify pixel content is not blank (non-zero alpha).
#[test]
fn regression_export_png_non_blank() {
    let cells = make_test_cells(2, 2, '#');
    let png_bytes = export_cells_to_png(&cells, 1).expect("PNG export should succeed");
    let img = image::load_from_memory(&png_bytes).expect("should decode PNG");
    let rgba = img.to_rgba8();
    let has_nonzero_alpha = rgba.pixels().any(|p| p[3] > 0);
    assert!(has_nonzero_alpha, "PNG should have non-zero alpha pixels");
}

/// Export single frame GIF, verify dimensions and frame count.
#[test]
fn regression_export_gif_single_frame() {
    use gif::DecodeOptions;
    let cells = make_test_cells(2, 2, 'X');
    let gif_bytes = export_cells_to_gif(&[cells], &[10], 1, 0).expect("GIF export should succeed");

    let mut decoder = DecodeOptions::new();
    decoder.set_color_output(gif::ColorOutput::RGBA);
    let mut reader = decoder
        .read_info(&gif_bytes[..])
        .expect("should decode GIF");
    let info = reader.next_frame_info().unwrap().unwrap();
    assert_eq!(info.width, 16);
    assert_eq!(info.height, 32);
    assert_eq!(info.delay, 10);
    // No more frames
    assert!(reader.next_frame_info().unwrap().is_none());
}

/// Export multi-frame GIF, verify each frame has correct delay.
#[test]
fn regression_export_gif_multi_frame() {
    use gif::DecodeOptions;
    let cells_a = make_test_cells(1, 1, 'A');
    let cells_b = make_test_cells(1, 1, 'B');
    let frames = vec![cells_a, cells_b];
    let delays = vec![15, 30];
    let gif_bytes = export_cells_to_gif(&frames, &delays, 1, 0).expect("multi-frame GIF export");

    let mut decoder = DecodeOptions::new();
    decoder.set_color_output(gif::ColorOutput::RGBA);
    let mut reader = decoder
        .read_info(&gif_bytes[..])
        .expect("should decode GIF");
    let f1 = reader.next_frame_info().unwrap().unwrap();
    assert_eq!(f1.delay, 15);
    let f2 = reader.next_frame_info().unwrap().unwrap();
    assert_eq!(f2.delay, 30);
}

/// Export TXT to file, read back, verify content.
#[test]
fn regression_export_txt_to_file() {
    let cells = make_test_cells(2, 3, 'T');
    let txt = export_cells_to_txt(&cells);
    let tmp = std::env::temp_dir().join("regression_test_export.txt");
    std::fs::write(&tmp, &txt).expect("should write TXT file");
    let read_back = std::fs::read_to_string(&tmp).expect("should read TXT file");
    assert_eq!(read_back, txt, "read back TXT should match export");
    let _ = std::fs::remove_file(&tmp);
}

/// Export PNG to file, read back, verify it decodes.
#[test]
fn regression_export_png_to_file() {
    let cells = make_test_cells(3, 4, 'P');
    let png_bytes = export_cells_to_png(&cells, 1).expect("PNG export");
    let tmp = std::env::temp_dir().join("regression_test_export.png");
    std::fs::write(&tmp, &png_bytes).expect("should write PNG file");
    let img = image::open(&tmp).expect("should open PNG file");
    assert_eq!(img.width(), 32);
    let _ = std::fs::remove_file(&tmp);
}

/// Export GIF to file, read back, verify it decodes.
#[test]
fn regression_export_gif_to_file() {
    let cells = make_test_cells(1, 1, 'G');
    let gif_bytes = export_cells_to_gif(&[cells], &[10], 1, 0).expect("GIF export");
    let tmp = std::env::temp_dir().join("regression_test_export.gif");
    std::fs::write(&tmp, &gif_bytes).expect("should write GIF file");
    use std::fs;
    let meta = fs::metadata(&tmp).expect("should read GIF metadata");
    assert!(meta.len() > 0, "GIF file should have content");
    let _ = std::fs::remove_file(&tmp);
}

/// TXT export preserves multi-byte chars.
#[test]
fn regression_export_txt_unicode() {
    use figby_lib::tui::canvas::CanvasCell;
    let cells = vec![
        vec![CanvasCell {
            ch: '\u{2603}',
            fg: None,
            bg: None,
            height: None,
        }],
        vec![CanvasCell {
            ch: '\u{2764}',
            fg: None,
            bg: None,
            height: None,
        }],
    ];
    let txt = export_cells_to_txt(&cells);
    assert_eq!(txt, "\u{2603}\n\u{2764}");
}

/// TXT export with colored cells strips color codes.
#[test]
fn regression_export_txt_strips_color() {
    let cells = vec![
        vec![CanvasCell {
            ch: 'A',
            fg: Some(Color::Red),
            bg: Some(Color::Blue),
            height: None,
        }],
        vec![CanvasCell {
            ch: 'B',
            fg: Some(Color::Green),
            bg: None,
            height: None,
        }],
    ];
    let txt = export_cells_to_txt(&cells);
    assert_eq!(txt, "A\nB");
    assert!(!txt.contains('\x1b'));
}

/// Export error on empty cells returns InvalidCells error.
#[test]
fn regression_export_empty_cells_error() {
    let err = export_cells_to_png(&[], 1).unwrap_err();
    assert!(
        format!("{err}").contains("empty"),
        "empty cells should error"
    );
}

/// Export PNG at font_size=4 verifies very large dimensions.
#[test]
fn regression_export_png_large_font() {
    let cells = make_test_cells(2, 3, 'L');
    let png_bytes = export_cells_to_png(&cells, 4).expect("PNG export at font_size=4");
    let img = image::load_from_memory(&png_bytes).expect("should decode PNG");
    assert_eq!(img.width(), 3 * 8 * 4);
    assert_eq!(img.height(), 2 * 16 * 4);
}
