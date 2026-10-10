//! `.figseq` — a baked frame sequence: an ordered series of rasterized screens
//! with per-frame hold times.
//!
//! A `.figmap` is a *project* — layers, layer groups, keyframes, lights, editable
//! Text-tool blocks — and the only single-screen document format Figby has. It is
//! the wrong shape for "just play these N screens back": every frame drags a layer
//! stack and a keyframe table along with it, pretty-printed JSON costs ~220 KB for
//! one 76x26 frame, and nothing in it can represent state that has no editable
//! representation at all (a live particle bake, for instance).
//!
//! A figseq is what is left once you stop editing: one flat cell grid per frame,
//! a hold time, and a loop flag. That makes it a *terminal* format rather than an
//! editor format — cheap to write, cheap to read, and directly consumable by
//! [`crate::tui::player::play_raw_timed`], which is the same engine the GIF
//! playback path already drives.
//!
//! # On-disk shape
//!
//! ```json
//! {"version":1,"width":76,"height":26,"loop_enabled":false,
//!  "hold_last_frame_cs":250,
//!  "frames":[{"delay":10,"chars":"…","attrs":[…]}]}
//! ```
//!
//! `loop_enabled` repeats until a key is pressed instead of playing once, and
//! `hold_last_frame_cs` keeps the final frame up for that long after a
//! non-looping sequence ends, rather than clearing the screen the instant it
//! finishes. Neither is derivable from a figmap, so `--bake --loop` /
//! `--bake --hold-last <SECONDS>` choose them, and both are playback-time
//! overrides: a flag beats the baked value.
//!
//! Per frame the cell grid is stored column-flattened rather than as nested rows:
//! `chars` is one string of `width * height` characters in row-major order, and
//! `attrs` is three integers per cell — `fg`, `bg`, `height`. That last part
//! matters: a `CanvasCell` serialized the figmap way emits
//! `{"ch":" ","fg":null,"bg":null,"height":null}` per cell, ~112 bytes of JSON for
//! a blank cell. Encoding colours as integers (see [`encode_color`]) and packing
//! the whole grid into two arrays brings a 76x26 frame from ~90 KB of compact
//! figmap JSON down to a few KB.
//!
//! # What this format deliberately is not
//!
//! It is not an editor format and not a palette format. Cells carry resolved
//! colours, so there is no swatch table to keep in sync, and there is nothing here
//! to convert back into layers. Write a figmap if you want to keep editing.

use std::path::Path;

use ratatui::style::Color;
use serde::{Deserialize, Serialize};

use crate::CanvasCell;

pub const FIGSEQ_VERSION: u32 = 1;
pub const FIGSEQ_EXTENSION: &str = "figseq";
/// Hold time used for a frame that does not carry its own, in centiseconds
/// (1/100 s) — the same unit as `TimelineFrame::delay` and GIF import delays.
pub const DEFAULT_DELAY_CS: u16 = 10;

// ── colour codec ───────────────────────────────────────────────────────────
//
// One integer per colour. Kept exhaustive over `ratatui::style::Color` so a
// round-trip never silently collapses a named colour onto an indexed one (or an
// RGB one onto a named one), which would repaint the art on the next load.
const C_NONE: i32 = 0;
const C_RESET: i32 = 1;
/// `NAMED_COLORS[i]` is encoded as `C_NAMED_BASE + i`.
const C_NAMED_BASE: i32 = 2;
/// `Color::Indexed(n)` is encoded as `C_INDEXED_BASE + n`.
const C_INDEXED_BASE: i32 = C_NAMED_BASE + 16;
/// `Color::Rgb(r,g,b)` is encoded as `C_RGB_BASE + (r<<16 | g<<8 | b)`.
const C_RGB_BASE: i32 = C_INDEXED_BASE + 256;

/// The 16 named `Color` variants, in `Color`'s own declaration order.
const NAMED_COLORS: [Color; 16] = [
    Color::Black,
    Color::Red,
    Color::Green,
    Color::Yellow,
    Color::Blue,
    Color::Magenta,
    Color::Cyan,
    Color::Gray,
    Color::DarkGray,
    Color::LightRed,
    Color::LightGreen,
    Color::LightYellow,
    Color::LightBlue,
    Color::LightMagenta,
    Color::LightCyan,
    Color::White,
];

/// Encode an optional colour as a single integer. Total over every `Color`
/// variant, so decode is the exact inverse.
pub fn encode_color(c: Option<Color>) -> i32 {
    match c {
        None => C_NONE,
        Some(Color::Reset) => C_RESET,
        Some(Color::Indexed(n)) => C_INDEXED_BASE + n as i32,
        Some(Color::Rgb(r, g, b)) => C_RGB_BASE + ((r as i32) << 16 | (g as i32) << 8 | b as i32),
        Some(named) => {
            let idx = NAMED_COLORS
                .iter()
                .position(|n| *n == named)
                .expect("every non-named, non-Reset Color variant is listed in NAMED_COLORS");
            C_NAMED_BASE + idx as i32
        }
    }
}

/// Inverse of [`encode_color`]. Out-of-range codes decode to `None` rather than
/// panicking: a corrupt or hand-edited file should render as an uncoloured cell,
/// not take the player down mid-animation.
pub fn decode_color(code: i32) -> Option<Color> {
    match code {
        C_NONE => None,
        C_RESET => Some(Color::Reset),
        c if (C_NAMED_BASE..C_INDEXED_BASE).contains(&c) => {
            NAMED_COLORS[(c - C_NAMED_BASE) as usize].into()
        }
        c if (C_INDEXED_BASE..C_RGB_BASE).contains(&c) => {
            Some(Color::Indexed((c - C_INDEXED_BASE) as u8))
        }
        c if c >= C_RGB_BASE => {
            let packed = (c - C_RGB_BASE) as u32;
            Some(Color::Rgb(
                ((packed >> 16) & 0xff) as u8,
                ((packed >> 8) & 0xff) as u8,
                (packed & 0xff) as u8,
            ))
        }
        _ => None,
    }
}

// ── container ──────────────────────────────────────────────────────────────

/// One rasterized screen plus its hold time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FigSeqFrame {
    /// Hold time in centiseconds (1/100 s).
    pub delay: u16,
    /// `width * height` characters, row-major.
    pub chars: String,
    /// Three integers per cell, row-major: `fg`, `bg`, `height` (`-1` = absent).
    pub attrs: Vec<i32>,
}

/// A baked frame sequence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FigSeqFile {
    pub version: u32,
    pub width: u32,
    pub height: u32,
    /// Repeat on playback until a key is pressed, rather than playing once.
    #[serde(default)]
    pub loop_enabled: bool,
    /// Keep the final frame on screen this many centiseconds after playback
    /// ends. `0` means "exit immediately" — the terminal's pre-hold behaviour,
    /// where the teardown clears the screen and the last frame vanishes.
    /// Any key dismisses the hold early.
    #[serde(default)]
    pub hold_last_frame_cs: u16,
    pub frames: Vec<FigSeqFrame>,
}

#[derive(Debug)]
pub enum FigSeqError {
    Io(std::io::Error),
    Json(serde_json::Error),
    InvalidVersion(u32),
    InvalidDimensions {
        width: u32,
        height: u32,
    },
    /// Frame cell arrays disagree with the declared grid size.
    FrameSize {
        frame: usize,
        expected_cells: usize,
        actual_chars: usize,
        actual_attrs: usize,
    },
    NoFrames,
}

impl std::fmt::Display for FigSeqError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FigSeqError::Io(e) => write!(f, "I/O error: {e}"),
            FigSeqError::Json(e) => write!(f, "JSON error: {e}"),
            FigSeqError::InvalidVersion(v) => {
                write!(
                    f,
                    "Unsupported figseq version {v} (expected {FIGSEQ_VERSION})"
                )
            }
            FigSeqError::InvalidDimensions { width, height } => {
                write!(f, "Invalid dimensions: {width}x{height}")
            }
            FigSeqError::FrameSize {
                frame,
                expected_cells,
                actual_chars,
                actual_attrs,
            } => write!(
                f,
                "Frame {frame} holds {actual_chars} chars / {actual_attrs} attrs, \
                 expected {expected_cells} cells (3 attrs each)"
            ),
            FigSeqError::NoFrames => write!(f, "Sequence contains no frames"),
        }
    }
}

impl std::error::Error for FigSeqError {}

impl From<std::io::Error> for FigSeqError {
    fn from(e: std::io::Error) -> Self {
        FigSeqError::Io(e)
    }
}

impl From<serde_json::Error> for FigSeqError {
    fn from(e: serde_json::Error) -> Self {
        FigSeqError::Json(e)
    }
}

impl FigSeqFrame {
    /// Flatten a `height` x `width` cell grid into the on-disk form.
    pub fn encode(cells: &[Vec<CanvasCell>], width: usize, height: usize, delay: u16) -> Self {
        let mut chars = String::with_capacity(width * height);
        let mut attrs = Vec::with_capacity(width * height * 3);
        for y in 0..height {
            let row = cells.get(y);
            for x in 0..width {
                let cell = row.and_then(|r| r.get(x)).copied().unwrap_or_default();
                chars.push(cell.ch);
                attrs.push(encode_color(cell.fg));
                attrs.push(encode_color(cell.bg));
                attrs.push(cell.height.map_or(-1, i32::from));
            }
        }
        Self {
            delay,
            chars,
            attrs,
        }
    }

    /// Rebuild the `height` x `width` cell grid. Missing or short input yields
    /// default cells rather than panicking — see [`decode_color`].
    pub fn decode(&self, width: usize, height: usize) -> Vec<Vec<CanvasCell>> {
        let src = self.chars.chars();
        let mut out = Vec::with_capacity(height);
        for y in 0..height {
            let mut row = Vec::with_capacity(width);
            for x in 0..width {
                let i = y * width + x;
                let ch = src.clone().nth(i).unwrap_or(' ');
                let fg = decode_color(self.attrs.get(i * 3).copied().unwrap_or(-1));
                let bg = decode_color(self.attrs.get(i * 3 + 1).copied().unwrap_or(-1));
                let height = self
                    .attrs
                    .get(i * 3 + 2)
                    .copied()
                    .filter(|h| (0..=255).contains(h))
                    .map(|h| h as u8);
                row.push(CanvasCell { ch, fg, bg, height });
            }
            out.push(row);
        }
        out
    }
}

impl FigSeqFile {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        width: usize,
        height: usize,
        frames: Vec<Vec<Vec<CanvasCell>>>,
        delays: Option<&[u16]>,
        loop_enabled: bool,
        hold_last_frame_cs: u16,
    ) -> Self {
        let frames = frames
            .into_iter()
            .enumerate()
            .map(|(i, cells)| {
                let delay = delays
                    .and_then(|d| d.get(i).copied())
                    .filter(|d| *d > 0)
                    .unwrap_or(DEFAULT_DELAY_CS);
                FigSeqFrame::encode(&cells, width, height, delay)
            })
            .collect();
        Self {
            version: FIGSEQ_VERSION,
            width: width as u32,
            height: height as u32,
            loop_enabled,
            hold_last_frame_cs,
            frames,
        }
    }

    /// Total run time in centiseconds, including the end-of-playback hold.
    /// This is the sequence's real on-screen lifetime, which is what a caller
    /// reporting "baked N frames (X.Xs)" should mean.
    pub fn total_duration_cs(&self) -> u32 {
        let frames: u32 = self.frames.iter().map(|f| f.delay as u32).sum();
        frames + self.hold_last_frame_cs as u32
    }

    /// Cadence for `play_raw_timed`, derived from the first frame's hold time.
    /// Per-frame delays are passed alongside it, so this is only the fallback for
    /// frames without one — the same derivation the GIF playback path uses.
    pub fn fps_fallback(&self) -> u8 {
        let first = self
            .frames
            .first()
            .map(|f| f.delay)
            .unwrap_or(DEFAULT_DELAY_CS);
        100u16
            .checked_div(first.max(1))
            .map(|f| f.clamp(1, 60) as u8)
            .unwrap_or(10)
    }

    /// Per-frame hold times, for `play_raw_timed`.
    pub fn frame_delays(&self) -> Vec<u16> {
        self.frames.iter().map(|f| f.delay.max(1)).collect()
    }

    /// Decode every frame into the `frames -> rows -> cells` shape
    /// `play_raw_timed` consumes.
    pub fn cells_for_playback(&self) -> Vec<Vec<Vec<CanvasCell>>> {
        let (w, h) = (self.width as usize, self.height as usize);
        self.frames.iter().map(|f| f.decode(w, h)).collect()
    }

    /// Reject anything the player would choke on, so callers get one typed error
    /// instead of a panic deep inside playback.
    pub fn validate(&self) -> Result<(), FigSeqError> {
        if self.version != FIGSEQ_VERSION {
            return Err(FigSeqError::InvalidVersion(self.version));
        }
        if self.width == 0 || self.height == 0 {
            return Err(FigSeqError::InvalidDimensions {
                width: self.width,
                height: self.height,
            });
        }
        let expected = (self.width as usize) * (self.height as usize);
        for (frame, f) in self.frames.iter().enumerate() {
            let actual_chars = f.chars.chars().count();
            if actual_chars != expected || f.attrs.len() != expected * 3 {
                return Err(FigSeqError::FrameSize {
                    frame,
                    expected_cells: expected,
                    actual_chars,
                    actual_attrs: f.attrs.len(),
                });
            }
        }
        if self.frames.is_empty() {
            return Err(FigSeqError::NoFrames);
        }
        Ok(())
    }
}

/// Write a sequence to disk as compact JSON. Compact rather than pretty on
/// purpose: the payload is a grid of integers, and indentation would roughly
/// double it for no gain — the shape is already documented and validated on read.
pub fn save_figseq(seq: &FigSeqFile, path: &Path) -> Result<(), FigSeqError> {
    seq.validate()?;
    let json = serde_json::to_vec(seq)?;
    std::fs::write(path, json)?;
    Ok(())
}

/// Read and validate a `.figseq` file.
pub fn load_figseq(path: &Path) -> Result<FigSeqFile, FigSeqError> {
    let data = std::fs::read(path)?;
    let seq: FigSeqFile = serde_json::from_slice(&data)?;
    seq.validate()?;
    Ok(seq)
}

// ── bake: figmap → figseq ──────────────────────────────────────────────────

#[derive(Debug)]
pub enum BakeError {
    Figmap(crate::figmap::FigmapError),
    FigSeq(FigSeqError),
    /// The source document had no layers to flatten.
    NoLayers,
}

impl std::fmt::Display for BakeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BakeError::Figmap(e) => write!(f, "source figmap: {e}"),
            BakeError::FigSeq(e) => write!(f, "{e}"),
            BakeError::NoLayers => write!(f, "source figmap has no layers to bake"),
        }
    }
}

impl std::error::Error for BakeError {}

impl From<crate::figmap::FigmapError> for BakeError {
    fn from(e: crate::figmap::FigmapError) -> Self {
        BakeError::Figmap(e)
    }
}

impl From<FigSeqError> for BakeError {
    fn from(e: FigSeqError) -> Self {
        BakeError::FigSeq(e)
    }
}

/// Lighting parameters used when a figmap carries lights. These match
/// `LightingState::default()`; a figmap does not store them, so a bake uses the
/// defaults rather than inventing per-document values.
const BAKE_MAX_SHADOW_DISTANCE: u16 = 50;
const BAKE_HEIGHT_SCALE: f32 = 0.5;

/// Flatten a `.figmap` into a frame sequence.
///
/// This is the write half of the format and the reason it exists: a figmap
/// stores a *project*, and several things worth playing back have no editable
/// representation in one — most notably a lighting scene with no keyframes,
/// which `--play` on a figmap currently ignores. Baking resolves every frame to
/// the pixels the editor's exporter would produce (layers composited, Text-tool
/// blocks rasterized, lighting applied with the document's palette), so what
/// plays back is what you saw.
///
/// A figmap with no timeline frames bakes to a one-frame sequence.
///
/// `playback` carries the two end-of-playback behaviours a sequence can ask for.
/// Neither is inferable from a figmap, so the caller chooses them (`--loop` and
/// `--hold-last` on the CLI).
#[derive(Debug, Clone, Copy, Default)]
pub struct PlaybackIntent {
    pub loop_enabled: bool,
    pub hold_last_frame_cs: u16,
}

pub fn bake_figmap(
    figmap_path: &Path,
    out_path: Option<&Path>,
    playback: PlaybackIntent,
) -> Result<FigSeqFile, BakeError> {
    use crate::tui::canvas::CanvasBuffer;
    use crate::tui::export::{self, LightingContext};
    use crate::tui::lighting::{self, LightingLut, Scene};
    use crate::tui::palette::build_rgb_to_swatch;
    use crate::tui::palette_editor;
    use crate::tui::timeline::TimelineState;

    let figmap = crate::figmap::load_figmap(figmap_path)?;
    let (mut layers, timeline, lights, mut palette, text_blocks) =
        crate::figmap::into_runtime(figmap);
    if layers.layers.is_empty() {
        return Err(BakeError::NoLayers);
    }

    let w = layers.layers[0].buffer.width();
    let h = layers.layers[0].buffer.height();

    // Bake persisted Text-tool blocks into the active layer, non-destructively
    // (the figmap on disk keeps them editable). Same rule as the exporter.
    if !text_blocks.is_empty() {
        let active = layers.active;
        for block in &text_blocks {
            block.bake_into(&mut layers.layers[active].buffer);
        }
    }

    // Rebuild the lighting LUT from the document's palette, with the artwork's
    // own colours added, so lit frames keep their hues exactly as the editor
    // shows them rather than snapping to the nearest swatch.
    palette_editor::sync_art_swatches(&mut palette, &layers);
    let swatch_data = palette_editor::lighting_swatch_data(&palette);
    let char_ramp = lighting::pick_char_ramp(layers.layers.iter().flat_map(|l| {
        let b = l.buffer();
        (0..b.height())
            .flat_map(move |y| (0..b.width()).filter_map(move |x| b.get(x, y).map(|c| c.ch)))
    }));
    let lut = LightingLut::from_swatches(&swatch_data, char_ramp);
    let rgb_to_swatch = build_rgb_to_swatch(
        &palette
            .iter()
            .map(|s| (s.name.clone(), s.hex.clone()))
            .collect::<Vec<_>>(),
    );

    let has_timeline = timeline.as_ref().is_some_and(|tl| !tl.frames.is_empty());

    if has_timeline {
        let tl = timeline.expect("checked above");
        let mut scene = Scene {
            lights: lights.clone(),
        };
        scene.ensure_ambient();

        let ts = TimelineState {
            fps: tl.fps,
            frames: tl.frames,
            layer_names: layers.layers.iter().map(|l| l.name.clone()).collect(),
            ..TimelineState::default()
        };

        let lighting_ctx = LightingContext {
            base_scene: &scene,
            light_keyframes: &tl.light_keyframes,
            lut: &lut,
            layer_stack: &layers,
            max_shadow_distance: BAKE_MAX_SHADOW_DISTANCE,
            height_scale: BAKE_HEIGHT_SCALE,
            palette_rgb_to_swatch: &rgb_to_swatch,
            swatch_data: &swatch_data,
        };

        let frames = export::capture_timeline_frames(&ts, &layers, w, h, Some(&lighting_ctx));
        let delays: Vec<u16> = ts.frames.iter().map(|f| f.delay).collect();
        let seq = FigSeqFile::new(
            w,
            h,
            frames,
            Some(&delays),
            playback.loop_enabled,
            playback.hold_last_frame_cs,
        );
        if let Some(p) = out_path {
            save_figseq(&seq, p)?;
        }
        Ok(seq)
    } else {
        // Static document: one frame, lighting applied if lights were saved.
        let mut buf: CanvasBuffer = layers.composite();
        if !lights.is_empty() {
            let mut scene = Scene {
                lights: lights.clone(),
            };
            scene.ensure_ambient();
            buf = crate::tui::components::canvas::shade_composited(
                &buf,
                &layers,
                &scene,
                &lut,
                BAKE_MAX_SHADOW_DISTANCE,
                BAKE_HEIGHT_SCALE,
                &rgb_to_swatch,
                &swatch_data,
            );
        }
        let cells: Vec<Vec<CanvasCell>> = (0..h)
            .map(|y| {
                (0..w)
                    .map(|x| buf.get(x, y).copied().unwrap_or_default())
                    .collect()
            })
            .collect();
        let seq = FigSeqFile::new(
            w,
            h,
            vec![cells],
            None,
            playback.loop_enabled,
            playback.hold_last_frame_cs,
        );
        if let Some(p) = out_path {
            save_figseq(&seq, p)?;
        }
        Ok(seq)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bake_flattens_a_static_figmap_to_one_frame() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src.figmap");
        let out = dir.path().join("out.figseq");
        crate::figmap::tests_support::write_static_figmap(&src, 6, 3);
        let seq = bake_figmap(&src, Some(&out), PlaybackIntent::default()).unwrap();
        assert_eq!(seq.frames.len(), 1);
        assert_eq!((seq.width, seq.height), (6, 3));
        let cells = seq.cells_for_playback();
        assert_eq!(cells[0].len(), 3);
        assert_eq!(cells[0][0].len(), 6);
        // Re-reading the written file must give back the same sequence.
        assert_eq!(load_figseq(&out).unwrap(), seq);
    }

    #[test]
    fn bake_preserves_the_artwork() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src.figmap");
        crate::figmap::tests_support::write_static_figmap(&src, 6, 3);
        let expect_cell = crate::figmap::load_figmap(&src).unwrap().layers[0]
            .buffer
            .get(1, 1)
            .copied()
            .unwrap();

        let seq = bake_figmap(&src, None, PlaybackIntent::default()).unwrap();
        let got = seq.cells_for_playback()[0][1][1];
        assert_eq!(got.ch, expect_cell.ch);
        assert_eq!(got.fg, expect_cell.fg);
    }

    #[test]
    fn bake_applies_a_static_light_scene() {
        // The reason --bake exists: `--play` on a figmap only shades when the
        // document has light *keyframes*, so a saved scene with none still plays
        // back unlit. Baked pixels must be lit.
        let dir = tempfile::tempdir().unwrap();
        let unlit = dir.path().join("unlit.figmap");
        let lit = dir.path().join("lit.figmap");
        crate::figmap::tests_support::write_static_figmap(&unlit, 6, 3);
        crate::figmap::tests_support::write_static_figmap_with_light(&lit, 6, 3);

        let plain = bake_figmap(&unlit, None, PlaybackIntent::default()).unwrap();
        let shaded = bake_figmap(&lit, None, PlaybackIntent::default()).unwrap();
        assert_eq!(
            plain.frames[0].chars, shaded.frames[0].chars,
            "lighting must not repaint the glyphs"
        );
        assert_ne!(
            plain.frames[0].attrs, shaded.frames[0].attrs,
            "a static light scene did not alter any cell colour"
        );
    }

    #[test]
    fn bake_reports_a_source_without_layers() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("empty.figmap");
        let layers = crate::tui::layers::LayerStack {
            layers: Vec::new(),
            active: 0,
            groups: Vec::new(),
            links: Vec::new(),
        };
        crate::figmap::save_figmap(&layers, None, &[], &[], &[], &src).unwrap();
        // save_figmap writes width/height 0 for an empty stack, which
        // load_figmap rejects before we ever get to the layer check.
        assert!(matches!(
            bake_figmap(&src, None, PlaybackIntent::default()),
            Err(BakeError::Figmap(_))
        ));
    }

    fn grid(w: usize, h: usize) -> Vec<Vec<CanvasCell>> {
        vec![vec![CanvasCell::default(); w]; h]
    }

    #[test]
    fn color_codec_roundtrips_every_variant() {
        let mut colors: Vec<Option<Color>> = vec![None, Some(Color::Reset)];
        colors.extend(NAMED_COLORS.iter().map(|c| Some(*c)));
        colors.extend((0..=255u8).map(|n| Some(Color::Indexed(n))));
        colors.extend([
            Some(Color::Rgb(0, 0, 0)),
            Some(Color::Rgb(255, 255, 255)),
            Some(Color::Rgb(18, 52, 86)),
            Some(Color::Rgb(1, 2, 3)),
        ]);
        for c in colors {
            let code = encode_color(c);
            assert_eq!(
                decode_color(code),
                c,
                "round-trip failed for {c:?} (code {code})"
            );
        }
    }

    #[test]
    fn color_codes_are_injective() {
        // Named/Indexed must not collide: Color::Red and Color::Indexed(1) are
        // different values that must survive a round-trip distinctly.
        assert_ne!(
            encode_color(Some(Color::Red)),
            encode_color(Some(Color::Indexed(1)))
        );
        assert_ne!(
            encode_color(Some(Color::Indexed(1))),
            encode_color(Some(Color::Indexed(2)))
        );
        assert_eq!(encode_color(None), C_NONE);
    }

    #[test]
    fn decode_color_tolerates_corrupt_codes() {
        // Negative codes are the only unreachable region (0..=C_RESET covers
        // none/unset); they must read back as "no colour", not panic.
        assert_eq!(decode_color(-5), None);
        assert_eq!(decode_color(i32::MIN), None);
        // An oversized RGB payload wraps into a valid colour rather than
        // panicking mid-animation.
        assert_eq!(
            decode_color(C_RGB_BASE + 0x0100_0000),
            Some(Color::Rgb(0, 0, 0))
        );
    }

    #[test]
    fn frame_roundtrip_preserves_cells() {
        let mut cells = grid(4, 3);
        cells[0][0] = CanvasCell {
            ch: '█',
            fg: Some(Color::Indexed(11)),
            bg: None,
            height: Some(255),
        };
        cells[2][3] = CanvasCell {
            ch: '░',
            fg: Some(Color::Rgb(10, 20, 30)),
            bg: Some(Color::Cyan),
            height: Some(7),
        };
        let f = FigSeqFrame::encode(&cells, 4, 3, 12);
        assert_eq!(f.chars.chars().count(), 12);
        assert_eq!(f.attrs.len(), 36);
        assert_eq!(f.decode(4, 3), cells);
    }

    #[test]
    fn encode_pads_short_rows_with_default_cells() {
        let cells = vec![vec![CanvasCell {
            ch: 'x',
            ..Default::default()
        }]];
        let f = FigSeqFrame::encode(&cells, 3, 2, 10);
        assert_eq!(f.chars.chars().count(), 6);
        let back = f.decode(3, 2);
        assert_eq!(back[0][0].ch, 'x');
        assert_eq!(back[1][2], CanvasCell::default());
    }

    #[test]
    fn file_roundtrip_through_disk() {
        let mut cells = grid(5, 2);
        cells[1][4].fg = Some(Color::Rgb(255, 170, 60));
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("seq.figseq");

        let seq = FigSeqFile::new(5, 2, vec![cells.clone(), cells], Some(&[5, 7]), true, 250);
        save_figseq(&seq, &path).unwrap();
        let loaded = load_figseq(&path).unwrap();

        assert_eq!(loaded, seq);
        assert_eq!(loaded.version, FIGSEQ_VERSION);
        assert_eq!(loaded.frame_delays(), vec![5, 7]);
        // total_duration_cs is the sequence's real on-screen lifetime, so it
        // counts the end-of-playback hold as well as the frames.
        assert_eq!(loaded.total_duration_cs(), 5 + 7 + 250);
        assert_eq!(loaded.hold_last_frame_cs, 250);
        assert!(loaded.loop_enabled);
        assert_eq!(loaded.cells_for_playback().len(), 2);
    }

    #[test]
    fn a_missing_hold_field_defaults_to_no_hold() {
        // Hand-written and older files must not need the field: absent means
        // "exit immediately", the pre-hold behaviour.
        let json = br#"{"version":1,"width":2,"height":1,"frames":[
            {"delay":10,"chars":"ab","attrs":[0,0,-1,0,0,-1]}]}"#;
        let seq: FigSeqFile = serde_json::from_slice(json).unwrap();
        assert_eq!(seq.hold_last_frame_cs, 0);
        assert!(!seq.loop_enabled);
        assert_eq!(seq.total_duration_cs(), 10);
        seq.validate().unwrap();
    }

    #[test]
    fn bake_records_the_requested_playback_intent() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src.figmap");
        crate::figmap::tests_support::write_static_figmap(&src, 4, 2);
        let seq = bake_figmap(
            &src,
            None,
            PlaybackIntent {
                loop_enabled: true,
                hold_last_frame_cs: 250,
            },
        )
        .unwrap();
        assert!(seq.loop_enabled);
        assert_eq!(seq.hold_last_frame_cs, 250);
        // The hold is not a frame delay: 250cs extra on top of the frames.
        assert_eq!(seq.total_duration_cs(), 250 + seq.frames[0].delay as u32);
    }

    #[test]
    fn fps_fallback_follows_first_frame_hold() {
        let cells = grid(2, 2);
        assert_eq!(
            FigSeqFile::new(2, 2, vec![cells.clone()], Some(&[10]), false, 0).fps_fallback(),
            10
        );
        assert_eq!(
            FigSeqFile::new(2, 2, vec![cells.clone()], Some(&[4]), false, 0).fps_fallback(),
            25
        );
        assert_eq!(
            FigSeqFile::new(2, 2, vec![cells.clone()], Some(&[1]), false, 0).fps_fallback(),
            60
        );
        // Clamped, not wrapped: a 0cs hold cannot mean "infinite fps".
        assert_eq!(
            FigSeqFile::new(2, 2, vec![cells], Some(&[0]), false, 0).fps_fallback(),
            10
        );
    }

    #[test]
    fn zero_delay_is_replaced_not_kept() {
        // A 0cs frame means "never shown"; the container substitutes the default.
        let cells = grid(2, 2);
        let seq = FigSeqFile::new(2, 2, vec![cells], Some(&[0]), false, 0);
        assert_eq!(seq.frames[0].delay, DEFAULT_DELAY_CS);
    }

    #[test]
    fn validate_rejects_wrong_version() {
        let cells = grid(2, 2);
        let mut seq = FigSeqFile::new(2, 2, vec![cells], None, false, 0);
        seq.version = 99;
        assert!(matches!(
            seq.validate(),
            Err(FigSeqError::InvalidVersion(99))
        ));
    }

    #[test]
    fn validate_rejects_zero_dimensions() {
        let cells = grid(2, 2);
        let mut seq = FigSeqFile::new(2, 2, vec![cells], None, false, 0);
        seq.width = 0;
        assert!(matches!(
            seq.validate(),
            Err(FigSeqError::InvalidDimensions { width: 0, .. })
        ));
    }

    #[test]
    fn validate_rejects_truncated_frame() {
        let cells = grid(3, 3);
        let mut seq = FigSeqFile::new(3, 3, vec![cells], None, false, 0);
        seq.frames[0].chars.pop();
        seq.frames[0].attrs.truncate(10);
        assert!(matches!(
            seq.validate(),
            Err(FigSeqError::FrameSize { frame: 0, .. })
        ));
    }

    #[test]
    fn validate_rejects_empty_sequence() {
        let seq = FigSeqFile::new(2, 2, Vec::new(), None, false, 0);
        assert!(matches!(seq.validate(), Err(FigSeqError::NoFrames)));
    }

    #[test]
    fn decode_pads_a_corrupt_frame_instead_of_panicking() {
        let f = FigSeqFrame {
            delay: 10,
            chars: "ab".into(),
            attrs: vec![encode_color(Some(Color::Red))],
        };
        let out = f.decode(3, 2);
        assert_eq!(out.len(), 2);
        assert_eq!(out[1][2], CanvasCell::default());
        assert_eq!(out[0][0].fg, Some(Color::Red));
    }

    #[test]
    fn written_file_is_much_smaller_than_a_figmap_style_grid() {
        // The whole point of the flat encoding: a realistic 76x26 title frame
        // must not cost 90 KB. Guard against a regression to per-cell objects.
        let mut cells = grid(76, 26);
        for (x, y) in [(3usize, 3usize), (10, 8), (40, 20)] {
            cells[y][x] = CanvasCell {
                ch: '█',
                fg: Some(Color::Indexed(11)),
                bg: None,
                height: Some(255),
            };
        }
        let f = FigSeqFrame::encode(&cells, 76, 26, 10);
        let encoded = serde_json::to_string(&f).unwrap();
        let figmap_style = serde_json::to_string(&cells).unwrap();
        assert!(
            encoded.len() * 4 < figmap_style.len(),
            "flat encoding only {}x smaller than per-cell objects ({} vs {})",
            figmap_style.len() / encoded.len(),
            encoded.len(),
            figmap_style.len()
        );
    }
}
