use crossterm::event::KeyCode;
use ratatui::style::Color;

use crate::image_input::{
    apply_brightness, apply_contrast, apply_negative, bilinear_resize_rgb, floyd_steinberg_dither,
    load_rgb_matrix, luminance_to_char, pixels_to_braille_char, rgb_to_luminance_matrix, RgbPixel,
    DEFAULT_CHAR_MAP,
};
use crate::tui::canvas::CanvasCell;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsciiMode {
    Color,
    Grayscale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdjustmentMode {
    None,
    Brightness,
    Contrast,
    Threshold,
    TargetWidth,
}

pub struct ImageEditor {
    cells: Vec<Vec<CanvasCell>>,
    mode: AsciiMode,
    source_path: Option<PathBuf>,
    original_rgb: Option<Vec<Vec<RgbPixel>>>,
    target_width: usize,
    entering_path: bool,
    path_buffer: String,
    error_message: Option<String>,
    adjustment_mode: AdjustmentMode,
    brightness: i16,
    contrast: f64,
    threshold: u8,
    dither: bool,
    invert: bool,
    braille: bool,
}

impl ImageEditor {
    pub fn new() -> Self {
        Self {
            cells: Vec::new(),
            mode: AsciiMode::Grayscale,
            source_path: None,
            original_rgb: None,
            target_width: 80,
            entering_path: false,
            path_buffer: String::new(),
            error_message: None,
            adjustment_mode: AdjustmentMode::None,
            brightness: 0,
            contrast: 1.0,
            threshold: 128,
            dither: false,
            invert: false,
            braille: false,
        }
    }

    pub fn cells(&self) -> Option<&Vec<Vec<CanvasCell>>> {
        if self.cells.is_empty() {
            None
        } else {
            Some(&self.cells)
        }
    }

    /// Drop the cached ASCII conversion. Called when the document
    /// changes underneath the editor (e.g. GIF import, new document) so
    /// a stale cache can never be re-applied to the canvas by a later
    /// sync (GPT review F-06).
    pub fn clear_cells(&mut self) {
        self.cells.clear();
    }

    pub fn mode(&self) -> AsciiMode {
        self.mode
    }

    pub fn entering_path(&self) -> bool {
        self.entering_path
    }

    pub fn path_buffer(&self) -> &str {
        &self.path_buffer
    }

    pub fn error_message(&self) -> Option<&str> {
        self.error_message.as_deref()
    }

    pub fn clear_error(&mut self) {
        self.error_message = None;
    }

    pub fn has_cells(&self) -> bool {
        !self.cells.is_empty()
    }

    pub fn brightness(&self) -> i16 {
        self.brightness
    }

    pub fn contrast(&self) -> f64 {
        self.contrast
    }

    pub fn threshold(&self) -> u8 {
        self.threshold
    }

    pub fn dither(&self) -> bool {
        self.dither
    }

    pub fn invert(&self) -> bool {
        self.invert
    }

    pub fn braille(&self) -> bool {
        self.braille
    }

    pub fn adjustment_mode(&self) -> AdjustmentMode {
        self.adjustment_mode
    }

    pub fn load_from_path(&mut self, path: &str) -> Result<(), String> {
        let path_buf = PathBuf::from(path);

        let rgb = load_rgb_matrix(&path_buf).map_err(|e| format!("Failed to load image: {e}"))?;

        if rgb.is_empty() || rgb[0].is_empty() {
            return Err("Loaded image is empty".to_string());
        }

        self.original_rgb = Some(rgb);
        self.source_path = Some(path_buf);
        self.error_message = None;
        self.reset_adjustments();
        Ok(())
    }

    fn rgb_to_cells(rgb: &[Vec<RgbPixel>], mode: AsciiMode) -> Vec<Vec<CanvasCell>> {
        let mut cells = Vec::with_capacity(rgb.len());
        for row in rgb {
            let mut cell_row = Vec::with_capacity(row.len());
            for &(r, g, b) in row {
                let luma =
                    (0.2126 * r as f64 + 0.7152 * g as f64 + 0.0722 * b as f64).round() as u8;
                let ch = luminance_to_char(luma, DEFAULT_CHAR_MAP);
                let cell = match mode {
                    AsciiMode::Color => CanvasCell {
                        ch,
                        fg: Some(Color::Rgb(r, g, b)),
                        bg: None,
                        height: None,
                    },
                    AsciiMode::Grayscale => CanvasCell {
                        ch,
                        fg: None,
                        bg: None,
                        height: None,
                    },
                };
                cell_row.push(cell);
            }
            cells.push(cell_row);
        }
        cells
    }

    fn luminance_to_braille_cells(
        matrix: &[Vec<u8>],
        threshold: u8,
        dither: bool,
    ) -> Vec<Vec<CanvasCell>> {
        let working = if dither {
            floyd_steinberg_dither(matrix, threshold)
        } else {
            matrix.to_vec()
        };
        if working.is_empty() || working[0].is_empty() {
            return Vec::new();
        }
        let h = working.len();
        let w = working[0].len();
        let mut rows = Vec::new();
        let mut y = 0;
        while y < h {
            let mut row = Vec::new();
            let mut x = 0;
            while x < w {
                let ch = pixels_to_braille_char(&working, x, y, threshold, w, h);
                row.push(CanvasCell {
                    ch,
                    fg: None,
                    bg: None,
                    height: None,
                });
                x += 2;
            }
            rows.push(row);
            y += 4;
        }
        rows
    }

    pub fn reapply_adjustments(&mut self) {
        let Some(ref original) = self.original_rgb else {
            return;
        };

        let src_h = original.len();
        let src_w = original[0].len();
        let target_height = ((self.target_width as f64 * src_h as f64 / src_w as f64) * 0.5)
            .ceil()
            .max(1.0) as usize;

        let mut working = bilinear_resize_rgb(original, self.target_width, target_height);

        if self.brightness != 0 {
            apply_brightness(&mut working, self.brightness);
        }

        if (self.contrast - 1.0).abs() > f64::EPSILON {
            apply_contrast(&mut working, self.contrast);
        }

        if self.invert {
            apply_negative(&mut working);
        }

        if self.braille {
            let luminance = rgb_to_luminance_matrix(&working);
            self.cells = Self::luminance_to_braille_cells(&luminance, self.threshold, self.dither);
        } else {
            self.cells = Self::rgb_to_cells(&working, self.mode);
        }
    }

    pub fn reset_adjustments(&mut self) {
        self.adjustment_mode = AdjustmentMode::None;
        self.brightness = 0;
        self.contrast = 1.0;
        self.threshold = 128;
        self.dither = false;
        self.invert = false;
        self.braille = false;
        self.reapply_adjustments();
    }

    pub fn toggle_mode(&mut self) {
        self.mode = match self.mode {
            AsciiMode::Color => AsciiMode::Grayscale,
            AsciiMode::Grayscale => AsciiMode::Color,
        };
        self.reapply_adjustments();
    }

    pub fn adjustment_status(&self) -> String {
        let mut parts = Vec::new();
        match self.adjustment_mode {
            AdjustmentMode::Brightness => parts.push(format!("Brightness[{}]", self.brightness)),
            AdjustmentMode::Contrast => parts.push(format!("Contrast[{:.1}]", self.contrast)),
            AdjustmentMode::Threshold => parts.push(format!("Threshold[{}]", self.threshold)),
            AdjustmentMode::TargetWidth => parts.push(format!("Width[{}]", self.target_width)),
            AdjustmentMode::None => {}
        }
        if self.brightness != 0 {
            parts.push(format!(
                "B:{}{}",
                if self.brightness > 0 { "+" } else { "" },
                self.brightness
            ));
        }
        if (self.contrast - 1.0).abs() > f64::EPSILON {
            parts.push(format!("C:{:.1}", self.contrast));
        }
        if self.invert {
            parts.push("Inv".to_string());
        }
        if self.braille {
            parts.push("Braille".to_string());
        }
        if self.dither {
            parts.push("Dither".to_string());
        }
        let mode_str = match self.mode {
            AsciiMode::Color => "Color",
            AsciiMode::Grayscale => "Gray",
        };
        parts.push(mode_str.to_string());
        format!("[{}]", parts.join(" "))
    }

    pub fn handle_key(&mut self, code: KeyCode) -> bool {
        if self.entering_path {
            match code {
                KeyCode::Char(c) => {
                    self.path_buffer.push(c);
                }
                KeyCode::Enter => {
                    let path = std::mem::take(&mut self.path_buffer);
                    if !path.is_empty() {
                        if let Err(e) = self.load_from_path(&path) {
                            self.error_message = Some(e);
                        }
                    }
                    self.entering_path = false;
                }
                KeyCode::Esc => {
                    self.entering_path = false;
                    self.path_buffer.clear();
                }
                KeyCode::Backspace => {
                    self.path_buffer.pop();
                }
                _ => {}
            }
            return true;
        }

        match code {
            KeyCode::Char('o') | KeyCode::Char('O') => {
                self.entering_path = true;
                self.path_buffer.clear();
                self.error_message = None;
                true
            }
            KeyCode::Char('c') | KeyCode::Char('C') => {
                self.toggle_mode();
                true
            }
            KeyCode::Char('b') => {
                self.adjustment_mode = AdjustmentMode::Brightness;
                true
            }
            KeyCode::Char('k') => {
                self.adjustment_mode = AdjustmentMode::Contrast;
                true
            }
            KeyCode::Char('t') => {
                self.adjustment_mode = AdjustmentMode::Threshold;
                true
            }
            KeyCode::Char('w') => {
                self.adjustment_mode = AdjustmentMode::TargetWidth;
                true
            }
            KeyCode::Char('i') | KeyCode::Char('I') => {
                self.invert = !self.invert;
                self.reapply_adjustments();
                true
            }
            KeyCode::Char('d') | KeyCode::Char('D') => {
                self.dither = !self.dither;
                self.reapply_adjustments();
                true
            }
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                self.braille = !self.braille;
                self.reapply_adjustments();
                true
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                self.reset_adjustments();
                true
            }
            KeyCode::Char('+') | KeyCode::Char('=')
                if self.adjustment_mode != AdjustmentMode::None =>
            {
                match self.adjustment_mode {
                    AdjustmentMode::Brightness => {
                        self.brightness = (self.brightness + 5).min(255);
                    }
                    AdjustmentMode::Contrast => {
                        self.contrast = (self.contrast + 0.1).min(5.0);
                    }
                    AdjustmentMode::Threshold => {
                        self.threshold = self.threshold.saturating_add(8);
                    }
                    AdjustmentMode::TargetWidth => {
                        self.target_width = (self.target_width + 4).min(1000);
                    }
                    AdjustmentMode::None => {}
                }
                self.reapply_adjustments();
                true
            }
            KeyCode::Char('-') | KeyCode::Char('_')
                if self.adjustment_mode != AdjustmentMode::None =>
            {
                match self.adjustment_mode {
                    AdjustmentMode::Brightness => {
                        self.brightness = (self.brightness - 5).max(-255);
                    }
                    AdjustmentMode::Contrast => {
                        self.contrast = (self.contrast - 0.1).max(0.0);
                    }
                    AdjustmentMode::Threshold => {
                        self.threshold = self.threshold.saturating_sub(8);
                    }
                    AdjustmentMode::TargetWidth => {
                        self.target_width = self.target_width.saturating_sub(4).max(1);
                    }
                    AdjustmentMode::None => {}
                }
                self.reapply_adjustments();
                true
            }
            KeyCode::Esc if self.adjustment_mode != AdjustmentMode::None => {
                self.adjustment_mode = AdjustmentMode::None;
                true
            }
            KeyCode::Esc => false,
            _ => false,
        }
    }
}

impl Default for ImageEditor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image_input::{color_matrix_to_ascii, load_rgb_matrix, ImageColorConfig};

    const TEST_PNG: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../assets/img/figby.png");

    #[test]
    fn test_image_editor_load_grayscale() {
        let mut editor = ImageEditor::new();
        editor.target_width = 40;
        editor.load_from_path(TEST_PNG).expect("failed to load PNG");
        assert!(editor.has_cells(), "should have cells after load");
        let cells = editor.cells().unwrap();
        assert!(!cells.is_empty(), "cells should not be empty");
        assert!(!cells[0].is_empty(), "first row should not be empty");
        for row in cells {
            for cell in row {
                assert!(
                    DEFAULT_CHAR_MAP.contains(cell.ch),
                    "char '{}' not in default map",
                    cell.ch
                );
                assert_eq!(cell.fg, None, "grayscale mode should have no fg color");
            }
        }
    }

    #[test]
    fn test_image_editor_load_color() {
        let mut editor = ImageEditor::new();
        editor.mode = AsciiMode::Color;
        editor.target_width = 40;
        editor.load_from_path(TEST_PNG).expect("failed to load PNG");
        assert!(editor.has_cells(), "should have cells after load");
        let cells = editor.cells().unwrap();
        assert!(!cells.is_empty(), "cells should not be empty");
        for row in cells {
            for cell in row {
                assert!(
                    DEFAULT_CHAR_MAP.contains(cell.ch),
                    "char '{}' not in default map",
                    cell.ch
                );
                assert!(
                    cell.fg.is_some(),
                    "color mode should have fg color for each cell"
                );
            }
        }
    }

    #[test]
    fn test_image_editor_nonexistent_path() {
        let mut editor = ImageEditor::new();
        let result = editor.load_from_path("/nonexistent/path/image.png");
        assert!(result.is_err(), "expected error for nonexistent path");
    }

    #[test]
    fn test_image_editor_mode_toggle() {
        let mut editor = ImageEditor::new();
        editor.target_width = 40;
        editor.load_from_path(TEST_PNG).expect("failed to load PNG");
        assert_eq!(editor.mode, AsciiMode::Grayscale);
        assert!(
            editor
                .cells()
                .unwrap()
                .iter()
                .all(|row| row.iter().all(|c| c.fg.is_none())),
            "all cells should have no fg in grayscale"
        );

        editor.toggle_mode();
        assert_eq!(editor.mode, AsciiMode::Color);
        assert!(
            editor
                .cells()
                .unwrap()
                .iter()
                .all(|row| row.iter().all(|c| c.fg.is_some())),
            "all cells should have fg in color mode"
        );

        editor.toggle_mode();
        assert_eq!(editor.mode, AsciiMode::Grayscale);
        assert!(
            editor
                .cells()
                .unwrap()
                .iter()
                .all(|row| row.iter().all(|c| c.fg.is_none())),
            "all cells should have no fg after toggling back to grayscale"
        );
    }

    #[test]
    fn test_image_editor_matches_cli_output() {
        let mut editor = ImageEditor::new();
        editor.target_width = 40;
        editor.load_from_path(TEST_PNG).expect("failed to load PNG");

        let rgb = load_rgb_matrix(TEST_PNG).expect("failed to load RGB matrix");
        let expected = color_matrix_to_ascii(
            &rgb,
            &ImageColorConfig {
                target_width: Some(40),
                ..Default::default()
            },
        );

        let cells = editor.cells().unwrap();
        let canvas_output: String = cells
            .iter()
            .map(|row| row.iter().map(|c| c.ch).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");

        assert_eq!(
            canvas_output, expected,
            "canvas output should match color_matrix_to_ascii output"
        );
    }

    #[test]
    fn test_image_editor_render_to_canvas() {
        let mut editor = ImageEditor::new();
        editor.target_width = 40;
        editor.load_from_path(TEST_PNG).expect("failed to load PNG");
        let cells = editor.cells().unwrap().clone();
        let h = cells.len();
        let w = cells[0].len();

        let mut canvas = crate::tui::canvas::CanvasBuffer::new(w, h);
        for (y, row) in cells.iter().enumerate() {
            for (x, cell) in row.iter().enumerate() {
                canvas.set(x, y, *cell);
            }
        }

        assert_eq!(canvas.width(), w);
        assert_eq!(canvas.height(), h);
        for (y, row) in cells.iter().enumerate() {
            for (x, src) in row.iter().enumerate() {
                let dst = canvas.get(x, y).expect("cell should exist");
                assert_eq!(src.ch, dst.ch, "char mismatch at ({x},{y})");
                assert_eq!(src.fg, dst.fg, "fg mismatch at ({x},{y})");
            }
        }
    }

    #[test]
    fn test_image_editor_key_path_entry() {
        let mut editor = ImageEditor::new();
        assert!(!editor.entering_path());

        assert!(editor.handle_key(KeyCode::Char('o')));
        assert!(editor.entering_path());
        assert!(editor.path_buffer().is_empty());

        editor.handle_key(KeyCode::Char('t'));
        editor.handle_key(KeyCode::Char('e'));
        editor.handle_key(KeyCode::Char('s'));
        editor.handle_key(KeyCode::Char('t'));
        assert_eq!(editor.path_buffer(), "test");

        editor.handle_key(KeyCode::Backspace);
        assert_eq!(editor.path_buffer(), "tes");

        editor.handle_key(KeyCode::Esc);
        assert!(!editor.entering_path());
        assert!(editor.path_buffer().is_empty());
    }

    #[test]
    fn test_image_editor_key_mode_toggle() {
        let mut editor = ImageEditor::new();
        editor.target_width = 40;
        editor.load_from_path(TEST_PNG).expect("failed to load PNG");
        assert_eq!(editor.mode(), AsciiMode::Grayscale);

        assert!(editor.handle_key(KeyCode::Char('c')));
        assert_eq!(editor.mode(), AsciiMode::Color);

        assert!(editor.handle_key(KeyCode::Char('C')));
        assert_eq!(editor.mode(), AsciiMode::Grayscale);
    }

    fn cells_to_string(cells: &[Vec<CanvasCell>]) -> String {
        cells
            .iter()
            .map(|row| row.iter().map(|c| c.ch).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn test_image_editor_brightness_increase() {
        let mut editor = ImageEditor::new();
        editor.target_width = 40;
        editor.load_from_path(TEST_PNG).expect("failed to load PNG");
        let before = cells_to_string(editor.cells().unwrap());

        editor.brightness = 50;
        editor.reapply_adjustments();
        let after = cells_to_string(editor.cells().unwrap());

        assert_ne!(before, after, "brightness increase should change output");
    }

    #[test]
    fn test_image_editor_brightness_decrease() {
        let mut editor = ImageEditor::new();
        editor.target_width = 40;
        editor.load_from_path(TEST_PNG).expect("failed to load PNG");
        let before = cells_to_string(editor.cells().unwrap());

        editor.brightness = -50;
        editor.reapply_adjustments();
        let after = cells_to_string(editor.cells().unwrap());

        assert_ne!(before, after, "brightness decrease should change output");
    }

    #[test]
    fn test_image_editor_contrast_increase() {
        let mut editor = ImageEditor::new();
        editor.target_width = 40;
        editor.load_from_path(TEST_PNG).expect("failed to load PNG");
        let before = cells_to_string(editor.cells().unwrap());

        editor.contrast = 2.0;
        editor.reapply_adjustments();
        let after = cells_to_string(editor.cells().unwrap());

        assert_ne!(before, after, "contrast increase should change output");
    }

    #[test]
    fn test_image_editor_invert_toggle() {
        let mut editor = ImageEditor::new();
        editor.target_width = 40;
        editor.load_from_path(TEST_PNG).expect("failed to load PNG");
        let original = editor.cells().unwrap().clone();

        editor.invert = true;
        editor.reapply_adjustments();
        let inverted = editor.cells().unwrap().clone();
        assert_ne!(
            cells_to_string(&original),
            cells_to_string(&inverted),
            "invert should change output"
        );

        editor.invert = false;
        editor.reapply_adjustments();
        let restored = editor.cells().unwrap().clone();
        assert_eq!(
            cells_to_string(&original),
            cells_to_string(&restored),
            "un-invert should restore original"
        );
    }

    #[test]
    fn test_image_editor_threshold_adjustment() {
        let mut editor = ImageEditor::new();
        editor.target_width = 40;
        editor.load_from_path(TEST_PNG).expect("failed to load PNG");
        editor.braille = true;
        editor.reapply_adjustments();
        let before = cells_to_string(editor.cells().unwrap());

        editor.threshold = 64;
        editor.reapply_adjustments();
        let after = cells_to_string(editor.cells().unwrap());

        assert_ne!(
            before, after,
            "threshold change should affect braille output"
        );
    }

    #[test]
    fn test_image_editor_dither_toggle() {
        let mut editor = ImageEditor::new();
        editor.target_width = 40;
        editor.load_from_path(TEST_PNG).expect("failed to load PNG");
        editor.braille = true;
        editor.reapply_adjustments();
        let no_dither = cells_to_string(editor.cells().unwrap());

        editor.dither = true;
        editor.reapply_adjustments();
        let with_dither = cells_to_string(editor.cells().unwrap());

        assert_ne!(
            no_dither, with_dither,
            "dither toggle should change braille output"
        );
    }

    #[test]
    fn test_image_editor_target_width_change() {
        let mut editor = ImageEditor::new();
        editor.target_width = 20;
        editor.load_from_path(TEST_PNG).expect("failed to load PNG");
        let narrow_cols = editor.cells().unwrap()[0].len();

        editor.target_width = 60;
        editor.reapply_adjustments();
        let wide_cols = editor.cells().unwrap()[0].len();

        assert!(
            wide_cols > narrow_cols,
            "wider target should produce more columns"
        );
    }

    #[test]
    fn test_image_editor_reset_adjustments() {
        let mut editor = ImageEditor::new();
        editor.target_width = 40;
        editor.load_from_path(TEST_PNG).expect("failed to load PNG");
        let original = editor.cells().unwrap().clone();

        editor.brightness = 50;
        editor.contrast = 2.0;
        editor.invert = true;
        editor.reapply_adjustments();
        assert_ne!(
            cells_to_string(&original),
            cells_to_string(editor.cells().unwrap()),
            "adjustments should change output"
        );

        editor.reset_adjustments();
        assert_eq!(
            cells_to_string(&original),
            cells_to_string(editor.cells().unwrap()),
            "reset should restore original"
        );
    }

    #[test]
    fn test_image_editor_braille_mode_toggle() {
        let mut editor = ImageEditor::new();
        editor.target_width = 40;
        editor.load_from_path(TEST_PNG).expect("failed to load PNG");

        editor.braille = true;
        editor.reapply_adjustments();
        let cells = editor.cells().unwrap();
        assert!(!cells.is_empty(), "braille cells should not be empty");

        for row in cells {
            for cell in row {
                let code = cell.ch as u32;
                assert!(
                    (0x2800..=0x28FF).contains(&code),
                    "braille char U+{code:04X} out of range"
                );
            }
        }
    }

    #[test]
    fn test_image_editor_adjustment_preserves_after_toggle() {
        let mut editor = ImageEditor::new();
        editor.target_width = 40;
        editor.load_from_path(TEST_PNG).expect("failed to load PNG");

        editor.brightness = 50;
        editor.reapply_adjustments();

        assert!(
            editor.cells().unwrap()[0].iter().all(|c| c.fg.is_none()),
            "grayscale mode should not have fg colors"
        );

        editor.toggle_mode();

        assert_eq!(
            editor.brightness, 50,
            "brightness should persist after mode toggle"
        );
        assert!(
            editor.cells().unwrap()[0].iter().any(|c| c.fg.is_some()),
            "color mode should have fg colors"
        );
    }

    #[test]
    fn test_image_editor_key_adjustment_mode_selectors() {
        let mut editor = ImageEditor::new();

        assert_eq!(editor.adjustment_mode(), AdjustmentMode::None);

        assert!(editor.handle_key(KeyCode::Char('b')));
        assert_eq!(editor.adjustment_mode(), AdjustmentMode::Brightness);

        assert!(editor.handle_key(KeyCode::Char('k')));
        assert_eq!(editor.adjustment_mode(), AdjustmentMode::Contrast);

        assert!(editor.handle_key(KeyCode::Char('t')));
        assert_eq!(editor.adjustment_mode(), AdjustmentMode::Threshold);

        assert!(editor.handle_key(KeyCode::Char('w')));
        assert_eq!(editor.adjustment_mode(), AdjustmentMode::TargetWidth);

        assert!(editor.handle_key(KeyCode::Esc));
        assert_eq!(editor.adjustment_mode(), AdjustmentMode::None);
    }

    #[test]
    fn test_image_editor_key_adjustment_increase_decrease() {
        let mut editor = ImageEditor::new();
        editor.target_width = 40;
        editor.load_from_path(TEST_PNG).expect("failed to load PNG");

        editor.handle_key(KeyCode::Char('b'));
        let before = editor.brightness();
        editor.handle_key(KeyCode::Char('+'));
        assert_eq!(
            editor.brightness(),
            before + 5,
            "brightness should increase by 5"
        );

        editor.handle_key(KeyCode::Char('k'));
        let before_contrast = editor.contrast();
        editor.handle_key(KeyCode::Char('-'));
        assert!(
            (editor.contrast() - (before_contrast - 0.1)).abs() < 0.001,
            "contrast should decrease by 0.1"
        );

        editor.handle_key(KeyCode::Char('w'));
        let before_width = editor.target_width;
        editor.handle_key(KeyCode::Char('+'));
        assert_eq!(
            editor.target_width,
            before_width + 4,
            "target_width should increase by 4"
        );
    }

    #[test]
    fn test_image_editor_key_direct_toggles() {
        let mut editor = ImageEditor::new();
        editor.target_width = 40;
        editor.load_from_path(TEST_PNG).expect("failed to load PNG");

        assert!(!editor.invert());
        editor.handle_key(KeyCode::Char('i'));
        assert!(editor.invert(), "i should toggle invert on");
        editor.handle_key(KeyCode::Char('I'));
        assert!(!editor.invert(), "I should toggle invert off");

        assert!(!editor.dither());
        editor.handle_key(KeyCode::Char('d'));
        assert!(editor.dither(), "d should toggle dither on");
        editor.handle_key(KeyCode::Char('D'));
        assert!(!editor.dither(), "D should toggle dither off");

        assert!(!editor.braille());
        editor.handle_key(KeyCode::Char('y'));
        assert!(editor.braille(), "y should toggle braille on");
        editor.handle_key(KeyCode::Char('Y'));
        assert!(!editor.braille(), "Y should toggle braille off");
    }

    #[test]
    fn test_image_editor_key_reset() {
        let mut editor = ImageEditor::new();
        editor.target_width = 40;
        editor.load_from_path(TEST_PNG).expect("failed to load PNG");
        let original = cells_to_string(editor.cells().unwrap());

        editor.brightness = 50;
        editor.contrast = 2.0;
        editor.invert = true;
        editor.reapply_adjustments();
        assert_ne!(
            original,
            cells_to_string(editor.cells().unwrap()),
            "adjustments should change output"
        );

        editor.handle_key(KeyCode::Char('r'));
        assert_eq!(
            original,
            cells_to_string(editor.cells().unwrap()),
            "reset should restore original"
        );
    }
}
