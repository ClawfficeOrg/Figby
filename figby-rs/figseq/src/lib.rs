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
//! `figby`'s `tui::player::play_raw_timed`, which is the same engine the GIF
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

/// One terminal cell: a character plus optional colours and a lighting height.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CanvasCell {
    pub ch: char,
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub height: Option<u8>,
}

impl Default for CanvasCell {
    fn default() -> Self {
        Self {
            ch: ' ',
            fg: None,
            bg: None,
            height: None,
        }
    }
}

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

impl FigSeqFile {
    /// Parse and validate a sequence from JSON text, e.g. one embedded with
    /// `include_str!("banner.figseq")`.
    pub fn from_json_str(json: &str) -> Result<Self, FigSeqError> {
        let seq: FigSeqFile = serde_json::from_str(json)?;
        seq.validate()?;
        Ok(seq)
    }
}

// ── player ─────────────────────────────────────────────────────────────────

/// Time-driven playback state for a sequence, independent of any event loop.
///
/// Call [`Player::tick`] with the time since the last call (e.g. once per
/// render), then draw [`Player::frame`]. Holds each frame for its own delay,
/// loops when `loop_enabled`, otherwise stops on the last frame.
#[derive(Debug, Clone)]
pub struct Player {
    seq: FigSeqFile,
    frames: Vec<Vec<Vec<CanvasCell>>>,
    idx: usize,
    elapsed_cs: f64,
    looping: bool,
    finished: bool,
}

impl Player {
    pub fn new(seq: FigSeqFile) -> Self {
        let frames = seq.cells_for_playback();
        let looping = seq.loop_enabled;
        Self {
            seq,
            frames,
            idx: 0,
            elapsed_cs: 0.0,
            looping,
            finished: false,
        }
    }

    /// Override the baked loop flag.
    pub fn with_loop(mut self, looping: bool) -> Self {
        self.looping = looping;
        self
    }

    pub fn width(&self) -> u16 {
        self.seq.width as u16
    }

    pub fn height(&self) -> u16 {
        self.seq.height as u16
    }

    pub fn is_finished(&self) -> bool {
        self.finished
    }

    pub fn frame_index(&self) -> usize {
        self.idx
    }

    /// Show a specific frame (clamped); resets the hold timer. Use for a
    /// static banner: `player.seek(n)` and never tick.
    pub fn seek(&mut self, idx: usize) {
        self.idx = idx.min(self.frames.len() - 1);
        self.elapsed_cs = 0.0;
        self.finished = false;
    }

    /// Advance by `dt`; returns `true` if the visible frame changed.
    pub fn tick(&mut self, dt: std::time::Duration) -> bool {
        if self.finished {
            return false;
        }
        let start = self.idx;
        self.elapsed_cs += dt.as_secs_f64() * 100.0;
        loop {
            let hold = f64::from(self.seq.frames[self.idx].delay.max(1));
            if self.elapsed_cs < hold {
                break;
            }
            if self.idx + 1 < self.frames.len() {
                self.elapsed_cs -= hold;
                self.idx += 1;
            } else if self.looping {
                self.elapsed_cs -= hold;
                self.idx = 0;
            } else {
                self.finished = true;
                break;
            }
        }
        self.idx != start
    }

    /// The current frame as `rows -> cells`.
    pub fn frame(&self) -> &[Vec<CanvasCell>] {
        &self.frames[self.idx]
    }
}

/// A ratatui widget that draws a [`Player`]'s current frame, clipped to the
/// area. Cells without a colour keep the buffer's existing style.
pub struct Banner<'a>(pub &'a Player);

impl ratatui::widgets::Widget for Banner<'_> {
    fn render(self, area: ratatui::layout::Rect, buf: &mut ratatui::buffer::Buffer) {
        use ratatui::style::Style;
        for (y, row) in self.0.frame().iter().enumerate().take(area.height as usize) {
            for (x, cell) in row.iter().enumerate().take(area.width as usize) {
                let mut style = Style::default();
                if let Some(fg) = cell.fg {
                    style = style.fg(fg);
                }
                if let Some(bg) = cell.bg {
                    style = style.bg(bg);
                }
                let mut tmp = [0u8; 4];
                buf[(area.x + x as u16, area.y + y as u16)]
                    .set_symbol(cell.ch.encode_utf8(&mut tmp))
                    .set_style(style);
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn player_steps_loops_and_stops() {
        use std::time::Duration;
        let cells = grid(2, 1);
        let mk = |looping| {
            FigSeqFile::new(
                2,
                1,
                vec![cells.clone(), cells.clone()],
                Some(&[10, 10]),
                looping,
                0,
            )
        };
        let mut p = Player::new(mk(false));
        assert!(p.tick(Duration::from_millis(100)));
        assert_eq!(p.frame_index(), 1);
        p.tick(Duration::from_millis(100));
        assert!(p.is_finished());
        assert_eq!(p.frame_index(), 1);
        let mut p = Player::new(mk(true));
        p.tick(Duration::from_millis(200));
        assert_eq!(p.frame_index(), 0);
        assert!(!p.is_finished());
    }

    #[test]
    fn from_json_str_validates() {
        let seq = FigSeqFile::new(2, 1, vec![grid(2, 1)], None, false, 0);
        let json = serde_json::to_string(&seq).unwrap();
        assert!(FigSeqFile::from_json_str(&json).is_ok());
        assert!(FigSeqFile::from_json_str("{}").is_err());
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
    fn grid(w: usize, h: usize) -> Vec<Vec<CanvasCell>> {
        vec![vec![CanvasCell::default(); w]; h]
    }
}
