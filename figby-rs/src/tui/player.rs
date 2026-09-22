use std::cell::Cell;
use std::io::{self, Write};
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode};
use crossterm::terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::Widget;
use ratatui::Terminal;

use super::canvas::CanvasCell;

/// A single animation frame: a 2D grid of styled cells.
pub type AnimationFrame = Vec<Vec<CanvasCell>>;

const MIN_SPEED: f64 = 0.25;
const MAX_SPEED: f64 = 4.0;

/// Animation player widget for alternate-screen playback.
///
/// Uses interior mutability (`Cell`) so it can implement `Widget for &AnimationPlayer`.
/// Call `advance(delta)` in the event loop to progress frames based on elapsed time.
pub struct AnimationPlayer {
    frames: Vec<AnimationFrame>,
    fps: u8,
    /// Per-frame hold times in centiseconds. `None` → uniform `fps` timing.
    /// When present, `advance()` uses the current frame's own delay, so
    /// GIF-imported variable timing survives playback (GPT review F-26).
    frame_delays: Option<Vec<u16>>,
    current_frame: Cell<usize>,
    playing: Cell<bool>,
    loop_: Cell<bool>,
    speed: Cell<f64>,
    accumulator: Cell<f64>,
}

impl std::fmt::Debug for AnimationPlayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnimationPlayer")
            .field("frames", &self.frames.len())
            .field("fps", &self.fps)
            .field(
                "frame_delays",
                &self.frame_delays.as_ref().map(|d| d.len()).unwrap_or(0),
            )
            .field("current_frame", &self.current_frame)
            .field("playing", &self.playing)
            .field("loop_", &self.loop_)
            .field("speed", &self.speed)
            .finish()
    }
}

impl AnimationPlayer {
    pub fn new(frames: Vec<AnimationFrame>, fps: u8) -> Self {
        Self {
            frames,
            fps,
            frame_delays: None,
            current_frame: Cell::new(0),
            playing: Cell::new(false),
            loop_: Cell::new(false),
            speed: Cell::new(1.0),
            accumulator: Cell::new(0.0),
        }
    }

    /// Consuming builder to attach per-frame hold times (centiseconds).
    /// Shorter than the frame count → missing frames fall back to 10cs.
    pub fn with_frame_delays(mut self, delays: Vec<u16>) -> Self {
        self.frame_delays = Some(delays);
        self.accumulator.set(0.0);
        self
    }

    pub fn play(&self) {
        self.playing.set(true);
        self.accumulator.set(0.0);
    }

    pub fn pause(&self) {
        self.playing.set(false);
    }

    pub fn toggle_play(&self) {
        if self.playing.get() {
            self.pause();
        } else {
            self.play();
        }
    }

    /// Seek to a specific frame index. Clamps to valid range.
    pub fn seek(&self, idx: usize) {
        let max = self.frames.len().saturating_sub(1);
        self.current_frame.set(idx.min(max));
        self.accumulator.set(0.0);
    }

    /// Set playback speed multiplier. Clamped to [0.25, 4.0].
    pub fn set_speed(&self, mult: f64) {
        self.speed.set(mult.clamp(MIN_SPEED, MAX_SPEED));
        self.accumulator.set(0.0);
    }

    pub fn toggle_loop(&self) {
        self.loop_.set(!self.loop_.get());
    }

    /// Consuming builder to set the initial loop state without touching
    /// `new()`'s signature (used by ~25 call sites, most of them tests).
    pub fn with_loop(self, enabled: bool) -> Self {
        self.loop_.set(enabled);
        self
    }

    /// Advance animation by `delta` duration.
    /// Returns number of frames advanced (for testing).
    pub fn advance(&self, delta: Duration) -> usize {
        if !self.playing.get() || self.frames.is_empty() {
            return 0;
        }

        let mut acc = self.accumulator.get() + delta.as_secs_f64();
        let mut advanced = 0usize;
        let total = self.frames.len();

        loop {
            let cur = self.current_frame.get();
            let interval = self.frame_interval_for(cur);
            if acc < interval {
                break;
            }
            acc -= interval;
            advanced += 1;
            if self.loop_.get() {
                self.current_frame.set((cur + 1) % total);
            } else {
                if cur + 1 >= total {
                    // Final frame has had its natural on-screen interval.
                    self.current_frame.set(cur);
                    self.playing.set(false);
                    acc = 0.0;
                    break;
                }
                self.current_frame.set(cur + 1);
            }
        }

        self.accumulator.set(acc);
        advanced
    }

    /// Hold time for the frame at `idx` in seconds, scaled by playback
    /// speed. Uses the per-frame delay when present, otherwise the
    /// uniform fps-derived interval.
    fn frame_interval_for(&self, idx: usize) -> f64 {
        if let Some(delays) = &self.frame_delays {
            let d = delays.get(idx).copied().unwrap_or(10).max(1) as f64;
            d / 100.0 / self.speed.get().max(0.001)
        } else {
            1.0 / (self.fps as f64 * self.speed.get())
        }
    }

    /// Duration until the current frame should advance — used by callers
    /// that throttle redraws/sleeps to the animation's own cadence, which
    /// is per-frame when variable timing is attached (GPT review F-26).
    pub fn frame_interval(&self) -> Duration {
        let s = self.frame_interval_for(self.current_frame.get());
        Duration::from_secs_f64(s.max(0.0001))
    }

    pub fn progress(&self) -> (usize, usize) {
        let total = self.frames.len();
        let current = self.current_frame.get().min(total.saturating_sub(1));
        (current, total)
    }

    pub fn is_playing(&self) -> bool {
        self.playing.get()
    }

    pub fn is_looping(&self) -> bool {
        self.loop_.get()
    }

    pub fn total_frames(&self) -> usize {
        self.frames.len()
    }

    /// (width, height) of a single frame in cells, plus one row reserved
    /// for the progress bar. Used to size/center the render area the same
    /// way the normal canvas centers its buffer within its panel.
    pub fn content_dimensions(&self) -> (u16, u16) {
        let h = self.frames.first().map(|f| f.len()).unwrap_or(0);
        let w = self
            .frames
            .first()
            .and_then(|f| f.first())
            .map(|row| row.len())
            .unwrap_or(0);
        (w as u16, h.saturating_add(1) as u16)
    }

    pub fn current_frame(&self) -> usize {
        self.current_frame.get()
    }

    pub fn speed_mult(&self) -> f64 {
        self.speed.get()
    }

    pub fn fps(&self) -> u8 {
        self.fps
    }

    /// Prepend a frame at index 0, shifting all existing frames right.
    pub fn prepend_frame(&mut self, frame: AnimationFrame) {
        self.frames.insert(0, frame);
    }

    /// Return a reference to all frames.
    pub fn all_frames(&self) -> &[AnimationFrame] {
        &self.frames
    }

    /// Handle a key event. Returns `true` if the key was consumed.
    pub fn handle_key(&self, code: KeyCode) -> bool {
        match code {
            KeyCode::Char(' ') => {
                self.toggle_play();
                true
            }
            KeyCode::Left => {
                let cur = self.current_frame.get();
                self.seek(cur.saturating_sub(1));
                true
            }
            KeyCode::Right => {
                let cur = self.current_frame.get();
                self.seek(cur.saturating_add(1));
                true
            }
            KeyCode::Up | KeyCode::Char('+') | KeyCode::Char('=') => {
                let s = self.speed.get() + 0.25;
                self.set_speed(s);
                true
            }
            KeyCode::Down | KeyCode::Char('-') | KeyCode::Char('_') => {
                let s = self.speed.get() - 0.25;
                self.set_speed(s);
                true
            }
            KeyCode::Char('l') | KeyCode::Char('L') => {
                self.toggle_loop();
                true
            }
            KeyCode::Esc => {
                self.pause();
                true
            }
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                // `play_fullscreen`'s (and `play_raw`'s) loops both check the
                // raw keycode for Esc/'q' to decide whether to exit, but only
                // act on it when `handle_key` reports the key as consumed —
                // without this arm, 'q' fell through to `_ => false` and the
                // exit check could never fire, so 'q' silently did nothing.
                self.pause();
                true
            }
            _ => false,
        }
    }
}

impl Widget for &AnimationPlayer {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 || self.frames.is_empty() {
            return;
        }

        let current = self
            .current_frame
            .get()
            .min(self.frames.len().saturating_sub(1));
        let frame = &self.frames[current];

        let frame_h = frame.len();
        let frame_w = if frame_h > 0 { frame[0].len() } else { 0 };

        if frame_h == 0 || frame_w == 0 {
            return;
        }

        // Reserve bottom row for progress bar
        let frame_render_h = (area.height.saturating_sub(1)).min(frame_h as u16);
        let render_w = frame_w.min(area.width as usize);

        for (row_idx, row) in frame.iter().enumerate().take(frame_render_h as usize) {
            let y = area.y + row_idx as u16;
            for (col_idx, cell) in row.iter().enumerate().take(render_w) {
                let x = area.x + col_idx as u16;
                if let Some(buf_cell) = buf.cell_mut((x, y)) {
                    buf_cell.set_char(cell.ch);
                    let mut style = Style::default();
                    if let Some(fg) = cell.fg {
                        style = style.fg(fg);
                    }
                    if let Some(bg) = cell.bg {
                        style = style.bg(bg);
                    }
                    buf_cell.set_style(style);
                }
            }
        }

        // Progress bar on bottom row
        let progress_y = area.y + area.height.saturating_sub(1);
        self.render_progress_bar(area.x, area.width, progress_y, buf);
    }
}

/// Private helpers
impl AnimationPlayer {
    fn render_progress_bar(&self, origin_x: u16, area_width: u16, y: u16, buf: &mut Buffer) {
        let total = self.frames.len();
        let cur = self.current_frame.get().min(total.saturating_sub(1));

        let play_ch = if self.playing.get() {
            '\u{23F8}'
        } else {
            '\u{25B6}'
        };
        let total_digits = total.to_string().len();
        let counter_str = format!("{:0width$}/{}", cur + 1, total, width = total_digits);
        let loop_str = if self.loop_.get() { " \u{1F501}" } else { "" };
        let speed_str = format!(" {:.2}x", self.speed.get());

        let prefix = format!("{} {}", play_ch, counter_str);
        let prefix_len = prefix.chars().count();
        let suffix = format!("{speed_str}{loop_str}");
        let suffix_len = suffix.chars().count();

        let bar_available = (area_width as usize).saturating_sub(prefix_len + suffix_len + 3);
        let bar_width = bar_available.clamp(2, 60);

        let mut x = origin_x;

        // Prefix
        for ch in prefix.chars() {
            if x >= origin_x + area_width {
                break;
            }
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_char(ch);
            }
            x += 1;
        }

        // Space before bar
        if x < origin_x + area_width {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_char(' ');
            }
            x += 1;
        }

        // Opening bracket
        if x < origin_x + area_width {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_char('[');
            }
            x += 1;
        }

        // Bar fill
        let filled = if total > 1 {
            ((cur as f64) / ((total - 1) as f64) * bar_width as f64).round() as usize
        } else {
            bar_width
        };
        let filled = filled.min(bar_width);

        for i in 0..bar_width {
            if x >= origin_x + area_width {
                break;
            }
            let ch = if i < filled { '\u{2588}' } else { '\u{2591}' };
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_char(ch);
            }
            x += 1;
        }

        // Closing bracket
        if x < origin_x + area_width {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_char(']');
            }
            x += 1;
        }

        // Suffix (speed)
        for ch in suffix.chars() {
            if x >= origin_x + area_width {
                break;
            }
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_char(ch);
            }
            x += 1;
        }
    }
}

/// Capture current terminal content as an AnimationFrame.
/// Falls back to a blank frame sized to current terminal dimensions.
pub fn capture_terminal_content() -> io::Result<AnimationFrame> {
    let (cols, rows) = terminal::size()?;
    let w = cols as usize;
    let h = rows as usize;
    match try_query_terminal_cells(w, h) {
        Ok(frame) => Ok(frame),
        Err(_) => Ok(blank_frame(w, h)),
    }
}

/// Always returns `Unsupported` — there is no portable way to read back
/// arbitrary previously-rendered terminal cell content.
///
/// An earlier version of this doc comment suggested DECRQCRA (Request
/// Checksum of Rectangular Area) as a future implementation path. That was
/// mistaken: DECRQCRA's response (DECCKSR) is a terminal-defined *checksum*
/// of a region, used by conformance test suites (e.g. vttest) to verify a
/// terminal renders *already-known* content correctly — it cannot be
/// inverted to recover unknown character/color data, so it can't implement
/// "capture the screen as an animation frame." No standard escape sequence
/// does that; a few terminals (kitty, iTerm2) expose proprietary,
/// non-portable extensions for it, which crossterm does not wrap. Returning
/// `Unsupported` (and falling back to a blank frame) is the correct
/// behavior here, not a stub awaiting completion.
fn try_query_terminal_cells(_w: usize, _h: usize) -> io::Result<AnimationFrame> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "reading back terminal cell content is not portably supported",
    ))
}

/// Create a blank frame of the given dimensions with default (space) cells.
fn blank_frame(w: usize, h: usize) -> AnimationFrame {
    vec![vec![CanvasCell::default(); w]; h]
}

/// Terminal session managing capture → playback lifecycle.
///
/// Call `capture()` to get a session whose `captured_frame` can be
/// prepended to an animation as frame 0. Note `capture()` cannot actually
/// read existing on-screen content (see `try_query_terminal_cells`), so
/// `captured_frame` is always blank today, just sized to the real terminal
/// dimensions. Does not manage the alternate screen — callers own that.
pub struct TerminalSession {
    /// Captured terminal content as first frame — always blank; see the
    /// struct-level doc comment.
    pub captured_frame: AnimationFrame,
    /// Terminal dimensions at capture time (cols, rows).
    pub terminal_size: (u16, u16),
    /// Whether raw mode was enabled before entering player mode.
    pub was_raw_mode: bool,
}

impl TerminalSession {
    /// Capture current terminal output as the first frame.
    pub fn capture() -> io::Result<Self> {
        let (cols, rows) = terminal::size()?;
        let w = cols as usize;
        let h = rows as usize;
        let frame = match try_query_terminal_cells(w, h) {
            Ok(f) => f,
            Err(_) => blank_frame(w, h),
        };
        Ok(Self {
            captured_frame: frame,
            terminal_size: (cols, rows),
            was_raw_mode: false,
        })
    }
}

/// Play animation fullscreen: capture terminal, render at given FPS.
///
/// Prepends a captured frame 0 — always blank, since there is no portable
/// way to read back existing terminal content (see
/// `try_query_terminal_cells`) — then renders all frames at the given FPS
/// and handles keyboard input. Does NOT manage alternate screen — caller is
/// responsible for that.
pub fn play_fullscreen(frames: Vec<AnimationFrame>, fps: u8) -> io::Result<()> {
    play_fullscreen_timed(frames, fps, None)
}

/// `play_fullscreen` with per-frame hold times (centiseconds) so imported
/// GIF timing isn't flattened to a uniform FPS (GPT review F-26).
pub fn play_fullscreen_timed(
    frames: Vec<AnimationFrame>,
    fps: u8,
    delays: Option<Vec<u16>>,
) -> io::Result<()> {
    let session = TerminalSession::capture()?;

    let mut all_frames = vec![session.captured_frame.clone()];
    all_frames.extend(frames);

    // The prepended capture is always blank; give it the first real
    // frame's hold time so playback doesn't stall on it.
    let mut delays = delays;
    if let Some(d) = delays.as_mut() {
        let first = d.first().copied().unwrap_or(10);
        d.insert(0, first);
    }

    // Cap frame dimensions to terminal size to avoid rendering issues
    let (term_w, term_h) = terminal::size()?;
    for frame in &mut all_frames {
        let h = frame.len().min(term_h as usize);
        let w = if h > 0 {
            frame[0].len().min(term_w as usize)
        } else {
            0
        };
        frame.truncate(h);
        for row in frame.iter_mut() {
            row.truncate(w);
        }
    }

    let mut player = AnimationPlayer::new(all_frames, fps);
    if let Some(d) = delays {
        player = player.with_frame_delays(d);
    }
    player.play();

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;
    terminal.hide_cursor()?;

    let mut finished = false;

    while !finished {
        terminal.draw(|f| {
            let area = f.area();
            f.render_widget(&player, area);
        })?;

        let tick = player.frame_interval();
        if event::poll(tick)? {
            if let Event::Key(key) = event::read()? {
                let consumed = player.handle_key(key.code);
                if consumed && (key.code == KeyCode::Esc || key.code == KeyCode::Char('q')) {
                    finished = true;
                }
            }
        }

        if !finished {
            player.advance(tick);
        }

        let (cur, total) = player.progress();
        if total > 0
            && cur >= total.saturating_sub(1)
            && !player.is_looping()
            && !player.is_playing()
        {
            finished = true;
        }
    }

    drop(terminal);
    Ok(())
}

/// Convert a ratatui `Color` to ANSI foreground escape code string.
fn color_fg_ansi(c: &Color) -> String {
    match c {
        Color::Reset => "\x1b[39m".into(),
        Color::Black => "\x1b[30m".into(),
        Color::Red => "\x1b[31m".into(),
        Color::Green => "\x1b[32m".into(),
        Color::Yellow => "\x1b[33m".into(),
        Color::Blue => "\x1b[34m".into(),
        Color::Magenta => "\x1b[35m".into(),
        Color::Cyan => "\x1b[36m".into(),
        Color::White => "\x1b[37m".into(),
        Color::Gray | Color::DarkGray => "\x1b[90m".into(),
        Color::LightRed => "\x1b[91m".into(),
        Color::LightGreen => "\x1b[92m".into(),
        Color::LightYellow => "\x1b[93m".into(),
        Color::LightBlue => "\x1b[94m".into(),
        Color::LightMagenta => "\x1b[95m".into(),
        Color::LightCyan => "\x1b[96m".into(),
        Color::Rgb(r, g, b) => format!("\x1b[38;2;{};{};{}m", r, g, b),
        Color::Indexed(idx) => format!("\x1b[38;5;{}m", idx),
    }
}

/// Convert a ratatui `Color` to ANSI background escape code string.
fn color_bg_ansi(c: &Color) -> String {
    match c {
        Color::Reset => "\x1b[49m".into(),
        Color::Black => "\x1b[40m".into(),
        Color::Red => "\x1b[41m".into(),
        Color::Green => "\x1b[42m".into(),
        Color::Yellow => "\x1b[43m".into(),
        Color::Blue => "\x1b[44m".into(),
        Color::Magenta => "\x1b[45m".into(),
        Color::Cyan => "\x1b[46m".into(),
        Color::White => "\x1b[47m".into(),
        Color::Gray | Color::DarkGray => "\x1b[100m".into(),
        Color::LightRed => "\x1b[101m".into(),
        Color::LightGreen => "\x1b[102m".into(),
        Color::LightYellow => "\x1b[103m".into(),
        Color::LightBlue => "\x1b[104m".into(),
        Color::LightMagenta => "\x1b[105m".into(),
        Color::LightCyan => "\x1b[106m".into(),
        Color::Rgb(r, g, b) => format!("\x1b[48;2;{};{};{}m", r, g, b),
        Color::Indexed(idx) => format!("\x1b[48;5;{}m", idx),
    }
}

/// Render a single frame as ANSI escape sequences into a String.
/// Uses absolute CUP (cursor position), bypassing ratatui diffing.
///
/// Fullscreen-only: coordinates are absolute from the top-left (1,1).
/// Do NOT use for inline playback — see `render_frame_inline`, which is
/// cursor-relative and leaves existing screen content alone.
pub fn render_frame_raw(frame: &AnimationFrame) -> String {
    let mut out = String::new();
    for (y, row) in frame.iter().enumerate() {
        for (x, cell) in row.iter().enumerate() {
            let has_content = cell.ch != ' ' || cell.fg.is_some() || cell.bg.is_some();
            if !has_content {
                continue;
            }
            out.push_str(&format!("\x1b[{};{}H\x1b[0m{}", y + 1, x + 1, {
                let mut s = String::new();
                if let Some(ref fg) = cell.fg {
                    s.push_str(&color_fg_ansi(fg));
                }
                if let Some(ref bg) = cell.bg {
                    s.push_str(&color_bg_ansi(bg));
                }
                s
            }));
            out.push(cell.ch);
        }
    }
    out
}

/// Render a single frame for inline (at-cursor) playback.
///
/// Cursor-relative only: starts with `\x1b[u` (restore the origin saved
/// with `\x1b[s` at playback start) and then uses sequential writes plus
/// relative down/left moves — no absolute CUP, so the animation draws
/// wherever the cursor was instead of at the top-left.
///
/// Every cell of the `max_w × max_h` bounding box is written (spaces
/// included), so a smaller frame fully erases a larger previous one
/// instead of leaving ghost cells behind. `frame` rows shorter than
/// `max_w`, or fewer than `max_h`, are space-padded.
pub fn render_frame_inline(frame: &AnimationFrame, max_w: usize, max_h: usize) -> String {
    let mut out = String::new();
    out.push_str("\x1b[u");
    for y in 0..max_h {
        for x in 0..max_w {
            let cell = frame.get(y).and_then(|row| row.get(x));
            match cell {
                Some(cell) => {
                    out.push_str("\x1b[0m");
                    if let Some(ref fg) = cell.fg {
                        out.push_str(&color_fg_ansi(fg));
                    }
                    if let Some(ref bg) = cell.bg {
                        out.push_str(&color_bg_ansi(bg));
                    }
                    out.push(cell.ch);
                }
                None => {
                    out.push_str("\x1b[0m ");
                }
            }
        }
        if y + 1 < max_h {
            out.push_str("\x1b[1B");
            out.push_str(&format!("\x1b[{max_w}D"));
        }
    }
    out.push_str("\x1b[0m");
    out
}

/// Raw mode playback engine.
///
/// Enters raw mode (no echo, no line buffering), renders frames by writing
/// pre-computed ANSI escape codes directly to stdout (bypassing ratatui's
/// Terminal::draw diffing). Frame timing via `sleep`.
///
/// When `loop_playback` is `false` (the normal case): keyboard controls are
/// Space=pause, Esc=exit, Left/Right=seek, +/-=speed, l/L=toggle loop; and
/// playback auto-exits once a non-looping animation naturally reaches its
/// last frame.
///
/// When `loop_playback` is `true`: the animation repeats indefinitely (no
/// natural end to wait for), and the normal interactive controls are
/// bypassed — any keypress exits immediately. This is the "banner" mode:
/// loop until dismissed, rather than play-once-and-return.
pub fn play_raw(frames: Vec<AnimationFrame>, fps: u8, loop_playback: bool) -> io::Result<()> {
    play_raw_timed(frames, fps, None, loop_playback, false, false)
}

/// `play_raw` with per-frame hold times (centiseconds) so GIF-imported
/// variable timing isn't discarded (GPT review F-26). `fps` is still used
/// as the fallback for any frame without an explicit delay.
///
/// When `inline` is true, renders at the current cursor position without
/// clearing the screen: the cursor is saved (`\x1b[s`) once, every frame
/// restores it (`\x1b[u`) and draws with relative moves only, and on exit
/// the cursor is parked below the animation. No absolute CUP is emitted,
/// so surrounding shell output is left alone.
///
/// `show_timeline` adds a one-row playback timeline under the animation
/// when `inline` is true (off by default). Fullscreen playback always
/// shows its progress bar regardless of this flag.
pub fn play_raw_timed(
    frames: Vec<AnimationFrame>,
    fps: u8,
    delays: Option<Vec<u16>>,
    loop_playback: bool,
    inline: bool,
    show_timeline: bool,
) -> io::Result<()> {
    if frames.is_empty() {
        return Ok(());
    }

    let total = frames.len();
    // Inline frames are rendered relative to the saved cursor, so they
    // need the shared bounding box to fully erase the previous frame.
    // Fullscreen frames use absolute CUP and skip blank cells instead.
    let (max_w, max_h) = if inline {
        let w = frames
            .iter()
            .filter_map(|f| f.iter().map(|row| row.len()).max())
            .max()
            .unwrap_or(0);
        let h = frames.iter().map(|f| f.len()).max().unwrap_or(0);
        (w.max(1), h.max(1))
    } else {
        (0, 0)
    };
    // The inline timeline (opt-in via --play-timeline) is one extra
    // cursor-relative row under the animation; the exit parking and the
    // guard fallback must step over it too.
    let inline_timeline = inline && show_timeline;
    let box_h = max_h + usize::from(inline_timeline);
    let precomputed: Vec<String> = if inline {
        frames
            .iter()
            .map(|f| render_frame_inline(f, max_w, max_h))
            .collect()
    } else {
        frames.iter().map(render_frame_raw).collect()
    };
    let mut player = AnimationPlayer::new(frames, fps);
    if let Some(d) = delays {
        player = player.with_frame_delays(d);
    }
    player.play();
    if loop_playback {
        player.toggle_loop();
    }

    terminal::enable_raw_mode()?;
    // RAII guard (GPT review F-25): restores cursor visibility and raw
    // mode on ANY exit path, including errors mid-playback — previously
    // cleanup ran only on the normal tail. `parked` is shared with the
    // main loop: the loop sets it once it parks the cursor below the
    // animation, so the guard's fallback move runs only on error paths
    // that skip the normal exit positioning.
    use std::rc::Rc;
    let parked = Rc::new(Cell::new(false));
    struct RawModeGuard {
        inline: bool,
        max_h: usize,
        parked: Rc<Cell<bool>>,
    }
    impl Drop for RawModeGuard {
        fn drop(&mut self) {
            if self.inline {
                // Inline: never clear — just restore visibility. Park
                // below the box first if the main loop never got there
                // (error path); clean exits already parked and set the
                // flag, so this is skipped and the cursor stays put.
                if !self.parked.get() {
                    let _ = write!(io::stdout(), "\x1b[u\x1b[0m\x1b[{}B\r\n", self.max_h.max(1));
                }
                let _ = write!(io::stdout(), "\x1b[0m\x1b[?25h");
            } else {
                // Fullscreen: restore terminal
                let _ = write!(io::stdout(), "\x1b[?25h\x1b[0m\x1b[2J\x1b[H");
            }
            let _ = io::stdout().flush();
            let _ = terminal::disable_raw_mode();
        }
    }
    let _raw_guard = RawModeGuard {
        inline,
        max_h: box_h,
        parked: Rc::clone(&parked),
    };
    if inline {
        // Inline: save cursor as the animation origin, hide cursor,
        // don't clear screen.
        write!(io::stdout(), "\x1b[s\x1b[?25l")?;
    } else {
        // Fullscreen: hide cursor, clear screen
        write!(io::stdout(), "\x1b[?25l\x1b[2J")?;
    }
    io::stdout().flush()?;

    let mut finished = false;

    while !finished {
        let cur = player.current_frame();

        write!(io::stdout(), "{}", precomputed[cur])?;
        if inline_timeline {
            // Cursor-relative timeline row under the animation box: down
            // one from the box's last row, back to the origin column, then
            // the bar. Next frame's `\x1b[u` restores anyway.
            write!(
                io::stdout(),
                "\x1b[1B\x1b[{max_w}D{}",
                inline_progress_bar(&player, cur, total, max_w)
            )?;
        }
        if !inline {
            write_playback_progress_bar(&player, cur, total)?;
        }
        io::stdout().flush()?;

        let frame_interval = player.frame_interval();
        std::thread::sleep(frame_interval);

        if event::poll(Duration::ZERO)? {
            if let Event::Key(key) = event::read()? {
                if loop_playback {
                    // No natural end while looping — any keypress dismisses.
                    finished = true;
                } else {
                    let consumed = player.handle_key(key.code);
                    if consumed && (key.code == KeyCode::Esc || key.code == KeyCode::Char('q')) {
                        finished = true;
                    }
                }
            }
        }

        // `cur` (just rendered + slept on above) was already the final frame
        // of a non-looping, still-playing animation — it's had its full
        // interval on screen, so this is a natural end-of-playback, not a
        // user pausing partway through. Exit automatically instead of
        // waiting for player.is_playing() to become false, which nothing
        // ever sets on its own — that made an unattended `figby --play`
        // hang forever on the last frame.
        let (_, total_frames) = player.progress();
        let finished_naturally = total_frames > 0
            && cur >= total_frames.saturating_sub(1)
            && !player.is_looping()
            && player.is_playing();

        if player.is_playing() {
            player.advance(frame_interval);
        }

        if finished_naturally && !finished {
            finished = true;
        }
    }

    if inline {
        // Park the cursor below the animation box (plus the timeline row
        // when shown): back to the saved origin, down past the last row,
        // onto a fresh line. Sets the shared flag so the guard skips its
        // fallback move.
        write!(io::stdout(), "\x1b[u\x1b[{box_h}B\r\n\x1b[0m")?;
        io::stdout().flush()?;
        parked.set(true);
    }

    // Teardown happens in RawModeGuard::drop.
    Ok(())
}

/// Write a one-line progress bar for raw playback (bottom of terminal).
fn write_playback_progress_bar(
    player: &AnimationPlayer,
    cur: usize,
    total: usize,
) -> io::Result<()> {
    let play_ch = if player.is_playing() {
        '\u{23F8}'
    } else {
        '\u{25B6}'
    };
    let total_digits = total.to_string().len();
    let counter = format!("{:0width$}/{}", cur + 1, total, width = total_digits);
    let speed = format!(" {:.2}x", player.speed_mult());
    let prefix = format!("{} {} [", play_ch, counter);
    let suffix = format!("]{}", speed);

    let (cols, rows) = terminal::size().unwrap_or((80, 24));
    let bar_width = (cols as usize)
        .saturating_sub(prefix.len() + suffix.len() + 1)
        .clamp(2, 60);

    let filled = if total > 1 {
        ((cur * bar_width) as f64 / (total - 1) as f64).round() as usize
    } else {
        bar_width
    };
    let filled = filled.min(bar_width);

    write!(io::stdout(), "\x1b[{};1H\x1b[0m{}", rows, prefix)?;
    for i in 0..bar_width {
        let ch = if i < filled { '\u{2588}' } else { '\u{2591}' };
        write!(io::stdout(), "{}", ch)?;
    }
    write!(io::stdout(), "{}", suffix)?;
    Ok(())
}

/// Build the inline timeline row: same content as the fullscreen progress
/// bar, but as a fixed-`width` String with no absolute CUP — the caller
/// positions the cursor relatively first. Padded with spaces so a longer
/// previous row (e.g. after a speed change widens the suffix) is erased.
fn inline_progress_bar(player: &AnimationPlayer, cur: usize, total: usize, width: usize) -> String {
    let play_ch = if player.is_playing() { '⏸' } else { '▶' };
    let total_digits = total.to_string().len();
    let counter = format!("{:0width$}/{}", cur + 1, total, width = total_digits);
    let speed = format!(" {:.2}x", player.speed_mult());
    let loop_str = if player.is_looping() { " 🔁" } else { "" };
    let prefix = format!("{play_ch} {counter} [");
    let suffix = format!("]{speed}{loop_str}");

    let bar_width = width
        .saturating_sub(prefix.chars().count() + suffix.chars().count() + 1)
        .clamp(1, 60);
    let filled = if total > 1 {
        ((cur * bar_width) as f64 / (total - 1) as f64).round() as usize
    } else {
        bar_width
    };
    let filled = filled.min(bar_width);

    let mut out = String::new();
    out.push_str("\x1b[0m");
    out.push_str(&prefix);
    for i in 0..bar_width {
        out.push(if i < filled { '█' } else { '░' });
    }
    out.push_str(&suffix);
    // Pad to the full box width so stale cells from a wider previous
    // row can't linger. (Visible width tracked separately — the SGR
    // escape above is zero-width.)
    let visible = prefix.chars().count() + bar_width + suffix.chars().count();
    for _ in visible..width {
        out.push(' ');
    }
    out.push_str("\x1b[0m");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_frames(count: usize, w: usize, h: usize) -> Vec<AnimationFrame> {
        (0..count)
            .map(|i| {
                let ch = char::from_u32(b'A' as u32 + (i % 26) as u32).unwrap();
                vec![
                    vec![
                        CanvasCell {
                            ch,
                            fg: None,
                            bg: None,
                            height: None,
                        };
                        w
                    ];
                    h
                ]
            })
            .collect()
    }

    #[test]
    fn test_player_advance_single_frame() {
        let frames = make_test_frames(10, 3, 2);
        let player = AnimationPlayer::new(frames, 10);
        player.play();
        assert_eq!(player.current_frame(), 0);

        player.advance(Duration::from_millis(100));
        assert_eq!(player.current_frame(), 1);
    }

    #[test]
    fn test_player_advance_multiple_frames() {
        let frames = make_test_frames(10, 3, 2);
        let player = AnimationPlayer::new(frames, 10);
        player.play();

        let n = player.advance(Duration::from_millis(350));
        assert_eq!(n, 3);
        assert_eq!(player.current_frame(), 3);
    }

    #[test]
    fn test_player_does_not_advance_when_paused() {
        let frames = make_test_frames(10, 3, 2);
        let player = AnimationPlayer::new(frames, 10);
        // Not playing, never called play()
        assert!(!player.is_playing());

        player.advance(Duration::from_millis(200));
        assert_eq!(player.current_frame(), 0);
    }

    #[test]
    fn test_player_loops() {
        let frames = make_test_frames(10, 3, 2);
        let player = AnimationPlayer::new(frames, 10);
        player.play();
        player.toggle_loop();
        assert!(player.is_looping());

        // 10fps = 100ms per frame. 1.1s = 11 frames.
        player.advance(Duration::from_millis(1100));
        assert_eq!(player.current_frame(), 1);
    }

    #[test]
    fn test_player_does_not_loop_when_disabled() {
        let frames = make_test_frames(10, 3, 2);
        let player = AnimationPlayer::new(frames, 10);
        player.play();
        assert!(!player.is_looping());

        // 10fps. 2s = 20 frames. Without loop, should clamp at last frame (9).
        player.advance(Duration::from_secs(2));
        assert_eq!(player.current_frame(), 9);
    }

    #[test]
    fn test_player_with_loop_sets_initial_state() {
        let frames = make_test_frames(10, 3, 2);
        let player = AnimationPlayer::new(frames, 10).with_loop(true);
        assert!(player.is_looping());

        let frames = make_test_frames(10, 3, 2);
        let player = AnimationPlayer::new(frames, 10).with_loop(false);
        assert!(!player.is_looping());
    }

    #[test]
    fn test_content_dimensions() {
        // make_test_frames(count, w, h) — width 3, height 2, + 1 reserved
        // progress-bar row.
        let frames = make_test_frames(10, 3, 2);
        let player = AnimationPlayer::new(frames, 10);
        assert_eq!(player.content_dimensions(), (3, 3));
    }

    #[test]
    fn test_content_dimensions_empty_frames() {
        let player = AnimationPlayer::new(vec![], 10);
        assert_eq!(player.content_dimensions(), (0, 1));
    }

    #[test]
    fn test_player_seek() {
        let frames = make_test_frames(10, 3, 2);
        let player = AnimationPlayer::new(frames, 10);

        player.seek(5);
        assert_eq!(player.current_frame(), 5);

        // Seek past end clamps to last
        player.seek(100);
        assert_eq!(player.current_frame(), 9);

        // Seek before start clamps to first
        player.seek(0);
        assert_eq!(player.current_frame(), 0);
    }

    #[test]
    fn test_player_speed_control() {
        let frames = make_test_frames(10, 3, 2);
        let player = AnimationPlayer::new(frames, 10);
        player.play();
        player.set_speed(2.0);

        // 10fps * 2x = 20fps effective, frame_interval = 50ms.
        // 100ms = 2 frames.
        let n = player.advance(Duration::from_millis(100));
        assert_eq!(n, 2);
        assert_eq!(player.current_frame(), 2);
    }

    #[test]
    fn test_player_variable_frame_delays() {
        // Delays in centiseconds: 50ms, 200ms, 50ms. FPS is ignored when
        // per-frame delays are attached (GPT review F-26).
        let frames = make_test_frames(3, 3, 2);
        let player = AnimationPlayer::new(frames, 30).with_frame_delays(vec![5, 20, 5]);
        player.play();

        assert_eq!(player.advance(Duration::from_millis(50)), 1);
        assert_eq!(player.current_frame(), 1);
        // 199ms is short of frame 1's 200ms → stays.
        assert_eq!(player.advance(Duration::from_millis(199)), 0);
        assert_eq!(player.current_frame(), 1);
        // 1ms crosses 200ms → frame 2.
        assert_eq!(player.advance(Duration::from_millis(1)), 1);
        assert_eq!(player.current_frame(), 2);
    }

    #[test]
    fn test_player_variable_frame_delays_fallback_when_shorter() {
        // Only 2 delays for 3 frames: the third falls back to 10cs (100ms).
        let frames = make_test_frames(3, 3, 2);
        let player = AnimationPlayer::new(frames, 10).with_frame_delays(vec![20, 5]);
        player.play();
        // 200ms → frame 1.
        assert_eq!(player.advance(Duration::from_millis(200)), 1);
        assert_eq!(player.current_frame(), 1);
        // 50ms → frame 2; 100ms (fallback) → non-looping end.
        player.advance(Duration::from_millis(50));
        assert_eq!(player.current_frame(), 2);
        player.advance(Duration::from_millis(100));
        assert!(!player.is_playing(), "stops on the last frame");
    }

    #[test]
    fn test_player_variable_frame_delays_loop() {
        let frames = make_test_frames(2, 3, 2);
        let player = AnimationPlayer::new(frames, 10)
            .with_frame_delays(vec![5, 5])
            .with_loop(true);
        player.play();
        // 50 + 50ms → wraps back to frame 0.
        player.advance(Duration::from_millis(100));
        assert_eq!(player.current_frame(), 0);
        assert!(player.is_playing());
    }

    #[test]
    fn test_player_frame_interval_reflects_per_frame_delay() {
        let frames = make_test_frames(2, 3, 2);
        let player = AnimationPlayer::new(frames, 10).with_frame_delays(vec![5, 20]);
        assert_eq!(player.frame_interval(), Duration::from_millis(50));
        player.seek(1);
        assert_eq!(player.frame_interval(), Duration::from_millis(200));
        // Speed scales the interval.
        player.set_speed(2.0);
        assert_eq!(player.frame_interval(), Duration::from_millis(100));
    }

    #[test]
    fn test_player_speed_clamping() {
        let frames = make_test_frames(10, 3, 2);
        let player = AnimationPlayer::new(frames, 10);

        player.set_speed(0.1);
        assert!((player.speed_mult() - MIN_SPEED).abs() < 1e-6);

        player.set_speed(5.0);
        assert!((player.speed_mult() - MAX_SPEED).abs() < 1e-6);
    }

    #[test]
    fn test_player_render_progress_bar() {
        let frames = make_test_frames(10, 3, 2);
        let player = AnimationPlayer::new(frames, 10);

        let area = Rect::new(0, 0, 40, 5);
        let mut buf = Buffer::empty(area);
        Widget::render(&player, area, &mut buf);

        // Bottom row should have progress bar content
        let progress_y = area.y + area.height - 1;
        let cell = buf.cell((0, progress_y)).unwrap();
        // First char should be play/pause indicator
        let ch = cell.symbol().chars().next().unwrap();
        assert!(ch == '\u{25B6}' || ch == '\u{23F8}');

        // Should have '/' in the counter portion
        let mut has_slash = false;
        for x in 0..area.width {
            if let Some(c) = buf.cell((x, progress_y)) {
                if c.symbol() == "/" {
                    has_slash = true;
                    break;
                }
            }
        }
        assert!(has_slash);
    }

    #[test]
    fn test_player_render_frame_content() {
        let frame: AnimationFrame = vec![
            vec![
                CanvasCell {
                    ch: 'A',
                    fg: None,
                    bg: None,
                    height: None,
                },
                CanvasCell {
                    ch: 'B',
                    fg: None,
                    bg: None,
                    height: None,
                },
            ],
            vec![
                CanvasCell {
                    ch: 'C',
                    fg: None,
                    bg: None,
                    height: None,
                },
                CanvasCell {
                    ch: 'D',
                    fg: None,
                    bg: None,
                    height: None,
                },
            ],
        ];
        let player = AnimationPlayer::new(vec![frame], 10);
        let area = Rect::new(0, 0, 5, 5);
        let mut buf = Buffer::empty(area);
        Widget::render(&player, area, &mut buf);

        assert_eq!(buf.cell((0, 0)).unwrap().symbol(), "A");
        assert_eq!(buf.cell((1, 0)).unwrap().symbol(), "B");
        assert_eq!(buf.cell((0, 1)).unwrap().symbol(), "C");
        assert_eq!(buf.cell((1, 1)).unwrap().symbol(), "D");
    }

    #[test]
    fn test_player_handle_key_play_pause() {
        let frames = make_test_frames(10, 3, 2);
        let player = AnimationPlayer::new(frames, 10);
        assert!(!player.is_playing());

        player.handle_key(KeyCode::Char(' '));
        assert!(player.is_playing());

        player.handle_key(KeyCode::Char(' '));
        assert!(!player.is_playing());
    }

    #[test]
    fn test_player_handle_key_seek() {
        let frames = make_test_frames(10, 3, 2);
        let player = AnimationPlayer::new(frames, 10);
        player.seek(5);
        assert_eq!(player.current_frame(), 5);

        player.handle_key(KeyCode::Left);
        assert_eq!(player.current_frame(), 4);

        player.handle_key(KeyCode::Right);
        assert_eq!(player.current_frame(), 5);
    }

    #[test]
    fn test_player_handle_key_speed() {
        let frames = make_test_frames(10, 3, 2);
        let player = AnimationPlayer::new(frames, 10);

        player.handle_key(KeyCode::Up);
        assert!((player.speed_mult() - 1.25).abs() < 1e-6);

        player.handle_key(KeyCode::Down);
        assert!((player.speed_mult() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_player_handle_key_loop() {
        let frames = make_test_frames(10, 3, 2);
        let player = AnimationPlayer::new(frames, 10);
        assert!(!player.is_looping());

        player.handle_key(KeyCode::Char('L'));
        assert!(player.is_looping());

        player.handle_key(KeyCode::Char('l'));
        assert!(!player.is_looping());
    }

    #[test]
    fn test_player_handle_key_esc_pauses_and_preserves_frame() {
        let frames = make_test_frames(10, 3, 2);
        let player = AnimationPlayer::new(frames, 10);
        player.play();
        player.seek(5);
        assert!(player.is_playing());
        assert_eq!(player.current_frame(), 5);

        player.handle_key(KeyCode::Esc);
        assert!(!player.is_playing());
        assert_eq!(
            player.current_frame(),
            5,
            "Esc should preserve current frame, not seek(0)"
        );
    }

    #[test]
    fn test_player_handle_key_q_is_consumed_and_pauses() {
        // Regression test: 'q' previously fell through to `_ => false`, so
        // callers' `consumed && key.code == Char('q')` exit checks (in both
        // play_fullscreen and play_raw) could never fire — pressing 'q'
        // silently did nothing instead of quitting the player.
        let frames = make_test_frames(10, 3, 2);
        let player = AnimationPlayer::new(frames, 10);
        player.play();
        assert!(player.is_playing());

        let consumed = player.handle_key(KeyCode::Char('q'));
        assert!(
            consumed,
            "'q' must be reported as consumed to exit playback"
        );
        assert!(!player.is_playing());

        player.play();
        let consumed = player.handle_key(KeyCode::Char('Q'));
        assert!(consumed, "'Q' must also be reported as consumed");
        assert!(!player.is_playing());
    }

    #[test]
    fn test_player_handle_key_enter_not_consumed() {
        let frames = make_test_frames(10, 3, 2);
        let player = AnimationPlayer::new(frames, 10);
        assert!(!player.is_playing());

        let consumed = player.handle_key(KeyCode::Enter);
        assert!(!consumed, "Enter should no longer start playback");
        assert!(!player.is_playing());
    }

    #[test]
    fn test_player_progress() {
        let frames = make_test_frames(10, 3, 2);
        let player = AnimationPlayer::new(frames, 10);

        let (cur, total) = player.progress();
        assert_eq!(cur, 0);
        assert_eq!(total, 10);

        player.seek(5);
        let (cur, total) = player.progress();
        assert_eq!(cur, 5);
        assert_eq!(total, 10);
    }

    #[test]
    fn test_player_empty_frames_advance() {
        let player = AnimationPlayer::new(vec![], 10);
        player.play();

        assert_eq!(player.advance(Duration::from_secs(1)), 0);
        assert_eq!(player.total_frames(), 0);
    }

    #[test]
    fn test_player_fps() {
        let player = AnimationPlayer::new(vec![], 30);
        assert_eq!(player.fps(), 30);
    }

    #[test]
    fn test_prepend_frame() {
        let f0 = make_test_frames(1, 2, 2);
        let f1 = make_test_frames(1, 2, 2);
        let f2 = make_test_frames(1, 2, 2);
        let mut player = AnimationPlayer::new(vec![f0[0].clone(), f1[0].clone()], 10);
        assert_eq!(player.total_frames(), 2);

        player.prepend_frame(f2[0].clone());
        assert_eq!(player.total_frames(), 3);

        let all = player.all_frames();
        assert_eq!(all[0].len(), 2);
        assert_eq!(all[1].len(), 2);
        assert_eq!(all[2].len(), 2);
    }

    #[test]
    fn test_capture_terminal_content_fallback_blank() {
        let frame = match capture_terminal_content() {
            Ok(frame) => frame,
            // No tty in this environment (e.g. CI sandbox); nothing to assert.
            Err(e) => {
                eprintln!("skipping: terminal size unavailable: {e}");
                return;
            }
        };
        // Terminal size should be available even in test (80x24 default)
        assert!(!frame.is_empty());
        for row in &frame {
            for cell in row {
                assert_eq!(cell.ch, ' ');
                assert!(cell.fg.is_none());
                assert!(cell.bg.is_none());
            }
        }
    }

    #[test]
    fn test_terminal_session_capture() {
        let session = match TerminalSession::capture() {
            Ok(session) => session,
            // No tty in this environment (e.g. CI sandbox); nothing to assert.
            Err(e) => {
                eprintln!("skipping: terminal size unavailable: {e}");
                return;
            }
        };
        let (cols, rows) = session.terminal_size;
        assert!(cols > 0);
        assert!(rows > 0);
        assert_eq!(session.captured_frame.len(), rows as usize);
        if rows > 0 {
            assert_eq!(session.captured_frame[0].len(), cols as usize);
        }
        assert!(!session.was_raw_mode);
    }

    #[test]
    fn test_blank_frame_dimensions() {
        let frame = blank_frame(5, 3);
        assert_eq!(frame.len(), 3);
        assert_eq!(frame[0].len(), 5);
        assert_eq!(frame[1].len(), 5);
        assert_eq!(frame[2].len(), 5);
    }

    #[test]
    fn test_play_fullscreen_empty_frames() {
        // Must not panic or hang (advance auto-stops at end).
        let _ = play_fullscreen(vec![], 10);
    }

    #[test]
    fn test_color_fg_ansi_named() {
        assert_eq!(color_fg_ansi(&Color::Red), "\x1b[31m");
        assert_eq!(color_fg_ansi(&Color::Green), "\x1b[32m");
        assert_eq!(color_fg_ansi(&Color::White), "\x1b[37m");
        assert_eq!(color_fg_ansi(&Color::LightBlue), "\x1b[94m");
    }

    #[test]
    fn test_color_fg_ansi_rgb() {
        assert_eq!(color_fg_ansi(&Color::Rgb(255, 0, 0)), "\x1b[38;2;255;0;0m");
        assert_eq!(
            color_fg_ansi(&Color::Rgb(0, 128, 255)),
            "\x1b[38;2;0;128;255m"
        );
    }

    #[test]
    fn test_color_fg_ansi_indexed() {
        assert_eq!(color_fg_ansi(&Color::Indexed(42)), "\x1b[38;5;42m");
    }

    #[test]
    fn test_color_bg_ansi_named() {
        assert_eq!(color_bg_ansi(&Color::Black), "\x1b[40m");
        assert_eq!(color_bg_ansi(&Color::Cyan), "\x1b[46m");
    }

    #[test]
    fn test_color_bg_ansi_rgb() {
        assert_eq!(
            color_bg_ansi(&Color::Rgb(10, 20, 30)),
            "\x1b[48;2;10;20;30m"
        );
    }

    #[test]
    fn test_render_frame_raw_basic() {
        let frame = vec![
            vec![
                CanvasCell {
                    ch: 'X',
                    fg: None,
                    bg: None,
                    height: None,
                },
                CanvasCell {
                    ch: ' ',
                    fg: None,
                    bg: None,
                    height: None,
                },
            ],
            vec![
                CanvasCell {
                    ch: 'Y',
                    fg: None,
                    bg: None,
                    height: None,
                },
                CanvasCell {
                    ch: 'Z',
                    fg: None,
                    bg: None,
                    height: None,
                },
            ],
        ];
        let out = render_frame_raw(&frame);
        // Should include cursor positions for non-space cells
        assert!(out.contains("\x1b[1;1H\x1b[0mX"));
        assert!(out.contains("\x1b[2;1H\x1b[0mY"));
        assert!(out.contains("\x1b[2;2H\x1b[0mZ"));
        // Space cell at (1,2) should be skipped
        assert!(!out.contains("\x1b[1;2H"));
    }

    #[test]
    fn test_render_frame_raw_with_colors() {
        let frame = vec![vec![CanvasCell {
            ch: 'A',
            fg: Some(Color::Red),
            bg: None,
            height: None,
        }]];
        let out = render_frame_raw(&frame);
        assert!(out.contains("\x1b[31m"));
        assert!(out.contains("A"));
    }

    #[test]
    fn test_render_frame_raw_empty() {
        let out = render_frame_raw(&vec![]);
        assert_eq!(out, "");
    }

    #[test]
    fn test_render_frame_inline_no_absolute_cup() {
        // Regression test: inline rendering must be cursor-relative.
        // The old code reused render_frame_raw's absolute `\x1b[y;xH`,
        // which yanked playback to the top-left instead of the cursor.
        let frames = make_test_frames(2, 3, 2);
        for f in &frames {
            let out = render_frame_inline(f, 3, 2);
            assert!(
                out.starts_with("\x1b[u"),
                "must restore the saved cursor origin, got {out:?}"
            );
            // No absolute CUP (`ESC[{row};{col}H`) anywhere.
            let mut idx = 0;
            while let Some(pos) = out[idx..].find("\x1b[") {
                let seq = &out[idx + pos..];
                let end = seq.find(|c: char| c.is_ascii_alphabetic()).unwrap();
                let code = &seq[..=end];
                assert!(
                    !(code.contains(';') && code.ends_with('H')),
                    "absolute CUP leaked into inline output: {code:?} in {out:?}"
                );
                idx += pos + end + 1;
            }
        }
    }

    #[test]
    fn test_render_frame_inline_draws_content_and_erases() {
        // Content cells are drawn; a smaller frame padded to the shared
        // box emits spaces so the previous (larger) frame is erased.
        let frame = vec![vec![CanvasCell {
            ch: 'Q',
            fg: None,
            bg: None,
            height: None,
        }]];
        let out = render_frame_inline(&frame, 3, 2);
        assert!(out.contains('Q'));
        // 1 content cell + 5 padding cells = full 3x2 box written.
        assert_eq!(out.matches("\x1b[0m ").count(), 5);
        // Row advance uses relative moves only.
        assert!(out.contains("\x1b[1B\x1b[3D"));
    }

    #[test]
    fn test_render_frame_inline_colors() {
        let frame = vec![vec![CanvasCell {
            ch: 'A',
            fg: Some(Color::Red),
            bg: None,
            height: None,
        }]];
        let out = render_frame_inline(&frame, 1, 1);
        assert!(out.contains("\x1b[31m"));
        assert!(out.contains("A"));
    }

    #[test]
    fn test_inline_progress_bar_fixed_width_no_absolute_cup() {
        let frames = make_test_frames(10, 3, 2);
        let player = AnimationPlayer::new(frames, 10);
        player.play();
        let out = inline_progress_bar(&player, 0, 10, 20);
        // Exactly the box width in visible cells (SGR escapes excluded).
        let stripped: String = out
            .replace("\x1b[0m", "")
            .chars()
            .filter(|&c| c != '\x1b')
            .collect();
        assert_eq!(stripped.chars().count(), 20, "bar must pad to {out:?}");
        assert!(out.contains("01/10"), "counter visible, got {out:?}");
        // No absolute CUP: no `ESC[` sequence with `;` in it.
        assert!(
            !out.split("\x1b").skip(1).any(|seq| seq.contains(';')),
            "absolute CUP leaked: {out:?}"
        );
    }

    #[test]
    fn test_inline_progress_bar_narrow_width_no_panic() {
        let frames = make_test_frames(3, 3, 2);
        let player = AnimationPlayer::new(frames, 10);
        // Width smaller than prefix+suffix: must not panic, still a String.
        let out = inline_progress_bar(&player, 2, 3, 2);
        assert!(out.contains("3/3"));
    }

    #[test]
    fn test_play_raw_empty_frames() {
        let result = play_raw(vec![], 30, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_play_raw_empty_frames_looping() {
        let result = play_raw(vec![], 30, true);
        assert!(result.is_ok());
    }

    #[test]
    fn test_handle_key_plus_speed() {
        let frames = make_test_frames(10, 3, 2);
        let player = AnimationPlayer::new(frames, 10);

        player.handle_key(KeyCode::Char('='));
        assert!((player.speed_mult() - 1.25).abs() < 1e-6);

        player.handle_key(KeyCode::Char('+'));
        assert!((player.speed_mult() - 1.5).abs() < 1e-6);
    }

    #[test]
    fn test_handle_key_minus_speed() {
        let frames = make_test_frames(10, 3, 2);
        let player = AnimationPlayer::new(frames, 10);

        // Start at 2.0, then decrement
        player.set_speed(2.0);
        player.handle_key(KeyCode::Char('-'));
        assert!((player.speed_mult() - 1.75).abs() < 1e-6);

        player.handle_key(KeyCode::Char('_'));
        assert!((player.speed_mult() - 1.5).abs() < 1e-6);
    }
}
