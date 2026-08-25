use crate::CanvasCell;
use gif::DisposalMethod;
use ratatui::style::Color;
use std::fs::File;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct GifImportResult {
    pub frames: Vec<Vec<Vec<CanvasCell>>>,
    pub frame_delays: Vec<u16>,
    pub loop_count: u16,
    pub palette_colors: Vec<Color>,
}

#[derive(Debug)]
pub enum GifImportError {
    Io(std::io::Error),
    Decode(String),
    NoFrames,
    TooLarge {
        width: usize,
        height: usize,
        frames: usize,
    },
}

impl std::fmt::Display for GifImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GifImportError::Io(e) => write!(f, "IO error: {e}"),
            GifImportError::Decode(e) => write!(f, "GIF decode error: {e}"),
            GifImportError::NoFrames => write!(f, "GIF has no frames"),
            GifImportError::TooLarge {
                width,
                height,
                frames,
            } => write!(f, "GIF too large: {width}x{height} x {frames} frames"),
        }
    }
}

impl std::error::Error for GifImportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            GifImportError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for GifImportError {
    fn from(e: std::io::Error) -> Self {
        GifImportError::Io(e)
    }
}

impl From<gif::DecodingError> for GifImportError {
    fn from(e: gif::DecodingError) -> Self {
        GifImportError::Decode(e.to_string())
    }
}

const MAX_TOTAL_CELLS: usize = 1_000_000;
/// Hard cap on decoded frame count: with a tiny scaled output the
/// per-cell budget alone would allow absurd frame counts (CPU/memory
/// amplification — GPT review F-13).
const MAX_FRAME_COUNT: usize = 10_000;

/// How an imported GIF's frames should be sized for output.
///
/// Compositing (disposal methods, partial-frame positioning) always happens
/// at the GIF's native resolution regardless of this setting — only the
/// final stored frames are scaled — so `Native` and a `FitWidth`/`FitBox`
/// that happens to equal native dimensions produce identical output.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GifScaleTarget {
    /// Keep the GIF's native pixel dimensions (1 pixel = 1 cell).
    Native,
    /// Scale to exactly `width` columns; height is derived from the source
    /// aspect ratio, compensating for terminal cells being roughly twice as
    /// tall as they are wide (same convention as `image_input::luminance_to_ascii`).
    FitWidth(usize),
    /// Scale to fit within a `(max_cols, max_rows)` box, preserving aspect
    /// ratio (same terminal-cell compensation as `FitWidth`). Scales up or
    /// down as needed to fill the box on the more constrained axis.
    FitBox(usize, usize),
    /// Scale to exact cell dimensions `(width, height)`. No aspect-ratio
    /// preservation or terminal-cell compensation — the output will be
    /// exactly `width` × `height` cells regardless of source proportions.
    /// Useful when the caller wants to specify precise canvas cell counts.
    Exact(usize, usize),
}

impl GifScaleTarget {
    pub(crate) fn resolve(self, native_w: usize, native_h: usize) -> (usize, usize) {
        match self {
            GifScaleTarget::Native => (native_w, native_h),
            GifScaleTarget::FitWidth(w) => {
                let w = w.max(1);
                let h = ((w as f64 * native_h as f64 / native_w as f64) * 0.5)
                    .ceil()
                    .max(1.0) as usize;
                (w, h)
            }
            GifScaleTarget::FitBox(max_w, max_h) => {
                let scale_w = max_w.max(1) as f64 / native_w as f64;
                let scale_h = max_h.max(1) as f64 / (native_h as f64 * 0.5);
                let scale = scale_w.min(scale_h);
                let w = (native_w as f64 * scale).round().max(1.0) as usize;
                let h = (native_h as f64 * scale * 0.5).round().max(1.0) as usize;
                (w, h)
            }
            GifScaleTarget::Exact(w, h) => (w.max(1), h.max(1)),
        }
    }
}

/// Bilinearly scale a composited frame's cells to `(out_w, out_h)`.
///
/// Each cell's background color is treated as its pixel value (untouched/
/// transparent cells — `bg: None` — count as black); the result is a solid
/// grid of `Color::Rgb` cells, matching how gif_import already flattens
/// transparency during native-resolution compositing.
fn scale_canvas(canvas: &[Vec<CanvasCell>], out_w: usize, out_h: usize) -> Vec<Vec<CanvasCell>> {
    let rgb: Vec<Vec<(u8, u8, u8)>> = canvas
        .iter()
        .map(|row| {
            row.iter()
                .map(|cell| match cell.bg {
                    Some(Color::Rgb(r, g, b)) => (r, g, b),
                    _ => (0, 0, 0),
                })
                .collect()
        })
        .collect();
    crate::image_input::bilinear_resize_rgb(&rgb, out_w, out_h)
        .into_iter()
        .map(|row| {
            row.into_iter()
                .map(|(r, g, b)| CanvasCell {
                    ch: ' ',
                    fg: None,
                    bg: Some(Color::Rgb(r, g, b)),
                    height: None,
                })
                .collect()
        })
        .collect()
}

/// Quick-probe a GIF file to read its native pixel dimensions without
/// decoding frames or compositing. Returns `(width, height)` in pixels.
pub fn probe_gif_dimensions(path: &Path) -> Result<(usize, usize), GifImportError> {
    let file = File::open(path)?;
    let decoder = gif::Decoder::new(file)?;
    let w = decoder.width() as usize;
    let h = decoder.height() as usize;
    if w == 0 || h == 0 {
        Err(GifImportError::Decode("GIF has zero dimensions".into()))
    } else {
        Ok((w, h))
    }
}

pub fn import_gif(path: &Path) -> Result<GifImportResult, GifImportError> {
    import_gif_scaled(path, GifScaleTarget::Native)
}

/// Same as [`import_gif`], but scales every composited frame to
/// `scale`. Useful for playing back a GIF larger than the terminal (or
/// larger than [`MAX_TOTAL_CELLS`] would otherwise allow at native
/// resolution) at a terminal-friendly size.
pub fn import_gif_scaled(
    path: &Path,
    scale: GifScaleTarget,
) -> Result<GifImportResult, GifImportError> {
    let file = File::open(path)?;
    let mut decoder = gif::Decoder::new(file)?;

    let width = decoder.width() as usize;
    let height = decoder.height() as usize;

    if width == 0 || height == 0 {
        return Err(GifImportError::Decode("GIF has zero dimensions".into()));
    }

    // Guard: check native per-frame cell count before reading any frames.
    // This is independent of the requested output scale — disposal-method
    // compositing needs a full native-resolution canvas regardless of how
    // small the final scaled output will be.
    if width.saturating_mul(height) > MAX_TOTAL_CELLS {
        return Err(GifImportError::TooLarge {
            width,
            height,
            frames: 1,
        });
    }

    let (out_w, out_h) = scale.resolve(width, height);
    if out_w == 0 || out_h == 0 {
        return Err(GifImportError::Decode(
            "scaled output dimensions must be non-zero".into(),
        ));
    }

    let global_palette: Vec<[u8; 3]> = decoder
        .global_palette()
        .map(|pal| pal.chunks(3).map(|c| [c[0], c[1], c[2]]).collect())
        .unwrap_or_default();

    let bg_color_index = decoder.bg_color().unwrap_or(0);

    // Single-pass decode/composite: each frame is consumed as it is read
    // so native-resolution data is never retained beyond the current
    // frame (an earlier version cloned every native frame into
    // raw_frames, letting a huge GIF scaled down to e.g. 1×1 pass the
    // output-cell cap while holding all full-resolution buffers — GPT
    // review F-13).
    let mut frame_count: usize = 0;
    let mut composited_frames: Vec<Vec<Vec<CanvasCell>>> = Vec::new();
    let mut delays: Vec<u16> = Vec::new();

    let loop_count = match decoder.repeat() {
        gif::Repeat::Infinite => 0,
        gif::Repeat::Finite(n) => n,
    };

    let palette_colors: Vec<Color> = global_palette
        .iter()
        .map(|[r, g, b]| Color::Rgb(*r, *g, *b))
        .collect();

    // Background color for Dispose::Background
    let bg_color = global_palette
        .get(bg_color_index)
        .map(|[r, g, b]| Color::Rgb(*r, *g, *b));

    // Composite frames with proper disposal
    let empty_cell = CanvasCell {
        ch: ' ',
        fg: None,
        bg: bg_color,
        height: None,
    };
    let transparent = CanvasCell {
        ch: ' ',
        fg: None,
        bg: None,
        height: None,
    };
    let mut canvas = vec![vec![transparent; width]; height];
    let mut saved_canvas: Vec<Vec<CanvasCell>> = canvas.clone();

    // Track properties of the previous frame for disposal
    let mut prev_dispose = DisposalMethod::Any;
    let mut prev_width: usize = 0;
    let mut prev_height: usize = 0;
    let mut prev_left: usize = 0;
    let mut prev_top: usize = 0;

    while let Some(frame) = decoder.read_next_frame()? {
        frame_count += 1;
        if frame_count > MAX_FRAME_COUNT {
            return Err(GifImportError::TooLarge {
                width: out_w,
                height: out_h,
                frames: frame_count,
            });
        }
        delays.push(frame.delay);

        // Apply previous frame's dispose method before rendering this frame
        match prev_dispose {
            DisposalMethod::Background => {
                for row in canvas.iter_mut().skip(prev_top).take(prev_height) {
                    for cell in row.iter_mut().skip(prev_left).take(prev_width) {
                        *cell = empty_cell;
                    }
                }
            }
            DisposalMethod::Previous => {
                canvas.clone_from(&saved_canvas);
            }
            _ => {}
        }

        // Save state for future Dispose::Previous
        if frame.dispose == DisposalMethod::Previous {
            saved_canvas = canvas.clone();
        }

        // Track current frame's properties for next iteration's disposal
        prev_dispose = frame.dispose;
        prev_width = frame.width as usize;
        prev_height = frame.height as usize;
        prev_left = frame.left as usize;
        prev_top = frame.top as usize;

        // Determine palette for this frame
        let frame_palette: Vec<[u8; 3]> = frame
            .palette
            .as_ref()
            .map(|pal| pal.chunks(3).map(|c| [c[0], c[1], c[2]]).collect())
            .unwrap_or_else(|| global_palette.clone());

        let fw = frame.width as usize;
        let fh = frame.height as usize;
        let fl = frame.left as usize;
        let ft = frame.top as usize;

        // Render frame onto canvas
        if frame_palette.is_empty() {
            for y in 0..fh {
                let cy = ft + y;
                if cy >= height {
                    break;
                }
                for x in 0..fw {
                    let cx = fl + x;
                    if cx >= width {
                        break;
                    }
                    let idx = y * fw + x;
                    let Some(&pixel_value) = frame.buffer.get(idx) else {
                        continue;
                    };
                    let is_transparent =
                        frame.transparent.map(|t| pixel_value == t).unwrap_or(false);
                    if !is_transparent {
                        canvas[cy][cx] = CanvasCell {
                            ch: ' ',
                            fg: None,
                            bg: Some(Color::Rgb(pixel_value, pixel_value, pixel_value)),
                            height: None,
                        };
                    }
                }
            }
        } else {
            for y in 0..fh {
                let cy = ft + y;
                if cy >= height {
                    break;
                }
                for x in 0..fw {
                    let cx = fl + x;
                    if cx >= width {
                        break;
                    }
                    let idx = y * fw + x;
                    let Some(&raw_pixel) = frame.buffer.get(idx) else {
                        continue;
                    };
                    let pixel_idx = raw_pixel as usize;
                    let is_transparent = frame
                        .transparent
                        .map(|t| pixel_idx == t as usize)
                        .unwrap_or(false);
                    if !is_transparent && pixel_idx < frame_palette.len() {
                        let [r, g, b] = frame_palette[pixel_idx];
                        canvas[cy][cx] = CanvasCell {
                            ch: ' ',
                            fg: None,
                            bg: Some(Color::Rgb(r, g, b)),
                            height: None,
                        };
                    }
                }
            }
        }

        if (out_w, out_h) == (width, height) {
            composited_frames.push(canvas.clone());
        } else {
            composited_frames.push(scale_canvas(&canvas, out_w, out_h));
        }

        // Cumulative *output* cell budget — what actually accumulates in
        // composited_frames.
        if out_w.saturating_mul(out_h).saturating_mul(frame_count) > MAX_TOTAL_CELLS {
            return Err(GifImportError::TooLarge {
                width: out_w,
                height: out_h,
                frames: frame_count,
            });
        }
    }

    if composited_frames.is_empty() {
        return Err(GifImportError::NoFrames);
    }

    Ok(GifImportResult {
        frames: composited_frames,
        frame_delays: delays,
        loop_count,
        palette_colors,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use gif::{Encoder, Repeat};
    use std::io::Write;

    /// (width, height, left, top, rgb, delay, dispose) for one test-GIF frame.
    type TestFrameSpec = (u16, u16, u16, u16, [u8; 3], u16, DisposalMethod);

    /// Encodes a small in-memory GIF from solid-color frames and writes it
    /// to a temp file, returning the file (kept alive for its Drop) and path.
    fn write_test_gif(
        width: u16,
        height: u16,
        frames: &[TestFrameSpec],
        repeat: Repeat,
    ) -> tempfile::NamedTempFile {
        let mut buf = Vec::new();
        {
            let mut encoder = Encoder::new(&mut buf, width, height, &[]).expect("encoder");
            encoder.set_repeat(repeat).expect("set_repeat");
            for &(fw, fh, left, top, [r, g, b], delay, dispose) in frames {
                let pixels: Vec<u8> = std::iter::repeat_n([r, g, b], fw as usize * fh as usize)
                    .flatten()
                    .collect();
                let mut frame = gif::Frame::from_rgb(fw, fh, &pixels);
                frame.left = left;
                frame.top = top;
                frame.delay = delay;
                frame.dispose = dispose;
                encoder.write_frame(&frame).expect("write_frame");
            }
        }
        let mut tmp = tempfile::NamedTempFile::new().expect("tempfile");
        tmp.write_all(&buf).expect("write gif bytes");
        tmp
    }

    #[test]
    fn test_scale_target_native_is_identity() {
        assert_eq!(GifScaleTarget::Native.resolve(64, 48), (64, 48));
    }

    #[test]
    fn test_scale_target_fit_width_preserves_aspect() {
        // 100x50 native (2:1) at width 20 -> height derives from aspect with
        // the 0.5 terminal-cell-height compensation: 20 * 50/100 * 0.5 = 5.
        let (w, h) = GifScaleTarget::FitWidth(20).resolve(100, 50);
        assert_eq!((w, h), (20, 5));
    }

    #[test]
    fn test_scale_target_fit_box_constrained_by_width() {
        // 400x100 native is wide; fitting into a 40x40 box should be
        // constrained by width, not height.
        let (w, h) = GifScaleTarget::FitBox(40, 40).resolve(400, 100);
        assert_eq!(w, 40);
        assert!((1..=40).contains(&h));
    }

    #[test]
    fn test_scale_target_fit_box_constrained_by_height() {
        // 100x400 native is tall; fitting into a 40x40 box should be
        // constrained by height, not width.
        let (w, h) = GifScaleTarget::FitBox(40, 40).resolve(100, 400);
        assert_eq!(h, 40);
        assert!((1..=40).contains(&w));
    }

    #[test]
    fn test_import_gif_scaled_downscales_frame_dimensions() {
        let tmp = write_test_gif(
            8,
            8,
            &[(8, 8, 0, 0, [0, 200, 0], 10, DisposalMethod::Any)],
            Repeat::Infinite,
        );
        let result = import_gif_scaled(tmp.path(), GifScaleTarget::FitWidth(4))
            .expect("scaled import should succeed");
        assert_eq!(result.frames.len(), 1);
        assert_eq!(result.frames[0][0].len(), 4); // width
        assert_eq!(result.frames[0].len(), 2); // height: 4 * 8/8 * 0.5 = 2
                                               // A solid-color source should stay solid after bilinear resize.
        assert_eq!(result.frames[0][0][0].bg, Some(Color::Rgb(0, 200, 0)));
    }

    #[test]
    fn test_import_gif_scaled_allows_oversized_native_gif() {
        // 700x700 = 490,000 cells/frame (under the 1,000,000 per-frame cap),
        // but 3 frames = 1,470,000 cells cumulative (over cap) at native
        // resolution -- unscaled import_gif must reject this exact GIF...
        let tmp = write_test_gif(
            700,
            700,
            &[
                (700, 700, 0, 0, [255, 0, 0], 10, DisposalMethod::Any),
                (700, 700, 0, 0, [0, 255, 0], 10, DisposalMethod::Any),
                (700, 700, 0, 0, [0, 0, 255], 10, DisposalMethod::Any),
            ],
            Repeat::Infinite,
        );
        assert!(matches!(
            import_gif(tmp.path()),
            Err(GifImportError::TooLarge { .. })
        ));

        // ...but scaling down to a terminal-friendly size brings the
        // cumulative *output* well under the cap, so it must succeed.
        let result = import_gif_scaled(tmp.path(), GifScaleTarget::FitWidth(80))
            .expect("scaled import of an oversized-native GIF should succeed");
        assert_eq!(result.frames.len(), 3);
        assert_eq!(result.frames[0][0].len(), 80);
    }

    #[test]
    fn test_scale_target_exact() {
        assert_eq!(GifScaleTarget::Exact(40, 20).resolve(100, 50), (40, 20));
        assert_eq!(GifScaleTarget::Exact(10, 5).resolve(999, 1), (10, 5));
    }

    #[test]
    fn test_import_single_frame_roundtrip() {
        let tmp = write_test_gif(
            2,
            2,
            &[(2, 2, 0, 0, [255, 0, 0], 12, DisposalMethod::Any)],
            Repeat::Infinite,
        );
        let result = import_gif(tmp.path()).expect("import should succeed");
        assert_eq!(result.frames.len(), 1);
        assert_eq!(result.frame_delays, vec![12]);
        assert_eq!(result.loop_count, 0); // Infinite -> 0
        assert_eq!(result.frames[0].len(), 2);
        assert_eq!(result.frames[0][0].len(), 2);
        // Solid red frame — every cell's bg should carry the red channel.
        for row in &result.frames[0] {
            for cell in row {
                assert_eq!(cell.bg, Some(Color::Rgb(255, 0, 0)));
            }
        }
    }

    #[test]
    fn test_import_multi_frame_preserves_delays() {
        let tmp = write_test_gif(
            1,
            1,
            &[
                (1, 1, 0, 0, [255, 0, 0], 5, DisposalMethod::Any),
                (1, 1, 0, 0, [0, 255, 0], 20, DisposalMethod::Any),
                (1, 1, 0, 0, [0, 0, 255], 99, DisposalMethod::Any),
            ],
            Repeat::Finite(3),
        );
        let result = import_gif(tmp.path()).expect("import should succeed");
        assert_eq!(result.frames.len(), 3);
        // Real per-frame delays must be preserved in order, not flattened.
        assert_eq!(result.frame_delays, vec![5, 20, 99]);
        assert_eq!(result.loop_count, 3);
    }

    #[test]
    fn test_import_disposal_background_clears_region() {
        // Frame 1: opaque red covering the whole 4x4 canvas, disposal=Background.
        // Frame 2: opaque blue covering only the top-left 2x2 corner.
        // After frame 1's Background disposal runs (before frame 2 renders),
        // the whole canvas should reset to the (paletteless) bg cell, so the
        // untouched bottom-right corner of frame 2 must NOT still be red.
        let tmp = write_test_gif(
            4,
            4,
            &[
                (4, 4, 0, 0, [255, 0, 0], 10, DisposalMethod::Background),
                (2, 2, 0, 0, [0, 0, 255], 10, DisposalMethod::Any),
            ],
            Repeat::Infinite,
        );
        let result = import_gif(tmp.path()).expect("import should succeed");
        assert_eq!(result.frames.len(), 2);

        let frame2 = &result.frames[1];
        // Top-left 2x2 was drawn blue by frame 2.
        assert_eq!(frame2[0][0].bg, Some(Color::Rgb(0, 0, 255)));
        assert_eq!(frame2[1][1].bg, Some(Color::Rgb(0, 0, 255)));
        // Bottom-right corner was outside frame 2 and must have been reset
        // by frame 1's Background disposal, not left over as red.
        assert_ne!(frame2[3][3].bg, Some(Color::Rgb(255, 0, 0)));
    }

    #[test]
    fn test_import_malformed_gif_returns_error() {
        let mut tmp = tempfile::NamedTempFile::new().expect("tempfile");
        tmp.write_all(b"not a gif at all, just garbage bytes")
            .expect("write garbage");
        let result = import_gif(tmp.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_import_nonexistent_path_returns_io_error() {
        let result = import_gif(Path::new("/nonexistent/path/to/nowhere.gif"));
        assert!(matches!(result, Err(GifImportError::Io(_))));
    }

    #[test]
    fn test_import_oversized_dimensions_rejected() {
        // Header-only GIF (no frames) at dimensions whose product exceeds
        // MAX_TOTAL_CELLS — must be rejected before any frame is read.
        let tmp = write_test_gif(2000, 2000, &[], Repeat::Infinite);
        let result = import_gif(tmp.path());
        assert!(matches!(result, Err(GifImportError::TooLarge { .. })));
    }

    #[test]
    fn test_import_error_display() {
        assert_eq!(GifImportError::NoFrames.to_string(), "GIF has no frames");
        assert_eq!(
            GifImportError::TooLarge {
                width: 10,
                height: 10,
                frames: 5
            }
            .to_string(),
            "GIF too large: 10x10 x 5 frames"
        );
    }
}
