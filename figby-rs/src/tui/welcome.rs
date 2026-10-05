use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget};
use ratatui::Frame;
use std::collections::BTreeMap;
use std::path::PathBuf;

use super::theme::Theme;

const MASCOT_RAW: &str = include_str!("../../assets/img/figby.block.ascii.image.txt");

/// (icon_key, key_char, label_suffix)  →  displays as `icon [K]label_suffix`
const FONT_ACTIONS: &[(&str, char, &str)] = &[
    ("file_new", 'N', "ew Font from System"),
    ("file_import", 'I', "mport Font from File"),
    ("font_header", 'B', "lank Font"),
    ("file_open", 'O', "pen Font"),
    ("edit_duplicate", 'D', "uplicate Font"),
];

const IMAGE_ACTIONS: &[(&str, char, &str)] = &[
    ("image_import", 'C', "reate Image"),
    ("nav_forward", 'T', "emplate"),
    ("image_import", 'V', "iew as ASCII"),
    ("file_open", 'F', "igmap  (Open Image)"),
    ("image_import", 'A', "nimated GIF Import"),
    ("file_open", 'L', "oad Image"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WelcomeAction {
    Dismiss,
    OpenRecent(usize),
    Open,
    NewFile,
    ToggleHelp,
    OpenSettings,
    ScrollUp,
    ScrollDown,
    FontNewFromSystem,
    FontNewFromFile,
    FontNewBlank,
    FontOpen,
    FontDuplicate,
    ImageNewBlank,
    ImageNewFromTemplate,
    ImageConvert,
    ImageOpenFigmap,
    ImageImportGif,
    ImageOpen,
}

pub struct WelcomeScreen {
    pub show: bool,
    pub scroll_offset: usize,
    mascot_lines: Vec<Line<'static>>,
    mascot_width: u16,
    title_lines_large: Vec<String>,
    title_lines_small: Vec<String>,
    /// Start of the title animation loop (colorize, light sweep, fade).
    anim_start: std::time::Instant,
    // Hit-test rects (updated each render)
    recent_rects: Vec<Rect>,
    font_rects: Vec<Rect>,
    image_rects: Vec<Rect>,
    // Hover state
    hovered_recent: Option<usize>,
    hovered_font: Option<usize>,
    hovered_image: Option<usize>,
}

impl WelcomeScreen {
    pub fn new() -> Self {
        let mascot_lines = parse_ansi_lines(MASCOT_RAW);
        let mascot_width = mascot_lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.chars().count())
                    .sum::<usize>()
            })
            .max()
            .unwrap_or(30) as u16;
        let title_lines_large = render_title_with_bob();
        let title_lines_small = ascii_fallback_title();
        Self {
            show: true,
            scroll_offset: 0,
            mascot_lines,
            mascot_width,
            title_lines_large,
            title_lines_small,
            anim_start: std::time::Instant::now(),
            recent_rects: Vec::new(),
            font_rects: Vec::new(),
            image_rects: Vec::new(),
            hovered_recent: None,
            hovered_font: None,
            hovered_image: None,
        }
    }

    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    pub fn scroll_down(&mut self, recent_count: usize) {
        if self.scroll_offset + 1 < recent_count {
            self.scroll_offset += 1;
        }
    }

    pub fn handle_mouse(
        &mut self,
        col: u16,
        row: u16,
        kind: MouseEventKind,
        recent_count: usize,
    ) -> (Option<WelcomeAction>, bool) {
        let hit = |rects: &[Rect]| {
            rects.iter().position(|r| {
                col >= r.x && col < r.x + r.width && row >= r.y && row < r.y + r.height
            })
        };

        match kind {
            MouseEventKind::Moved => {
                let prev = (self.hovered_recent, self.hovered_font, self.hovered_image);
                self.hovered_recent = hit(&self.recent_rects);
                self.hovered_font = hit(&self.font_rects);
                self.hovered_image = hit(&self.image_rects);
                let dirty = (self.hovered_recent, self.hovered_font, self.hovered_image) != prev;
                (None, dirty)
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(idx) = hit(&self.recent_rects) {
                    let abs = self.scroll_offset + idx;
                    if abs < recent_count {
                        return (Some(WelcomeAction::OpenRecent(abs)), false);
                    }
                }
                if let Some(idx) = hit(&self.font_rects) {
                    return (Some(font_action_for(idx)), false);
                }
                if let Some(idx) = hit(&self.image_rects) {
                    return (Some(image_action_for(idx)), false);
                }
                (None, false)
            }
            MouseEventKind::ScrollUp => (Some(WelcomeAction::ScrollUp), false),
            MouseEventKind::ScrollDown => (Some(WelcomeAction::ScrollDown), false),
            _ => (None, false),
        }
    }

    pub fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        recent_files: &[PathBuf],
        version: &str,
        theme: &Theme,
        icons: &BTreeMap<String, String>,
    ) {
        frame.render_widget(Clear, area);

        let welcome_area = centered_welcome(area);
        frame.render_widget(Clear, welcome_area);

        let block = Block::default()
            .title(format!(" Welcome to Figby  v{version} "))
            .borders(Borders::ALL)
            .style(Style::default().fg(theme.general.primary));
        let inner = block.inner(welcome_area);
        frame.render_widget(block, welcome_area);

        // --- Banner row ---
        let title_area_width = inner.width.saturating_sub(self.mascot_width + 1);
        let title_lines: &[String] = {
            let large_w = self
                .title_lines_large
                .iter()
                .map(|l| l.chars().count())
                .max()
                .unwrap_or(0) as u16;
            if !self.title_lines_large.is_empty() && large_w <= title_area_width {
                &self.title_lines_large
            } else {
                &self.title_lines_small
            }
        };

        let mascot_h = self.mascot_lines.len() as u16;
        let title_h = title_lines.len() as u16;
        let banner_height = mascot_h.max(title_h);

        let vert = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(banner_height),
                Constraint::Length(1),
                Constraint::Min(0),
            ])
            .split(inner);

        let horiz = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(self.mascot_width + 1),
                Constraint::Min(0),
            ])
            .split(vert[0]);

        // Mascot — vertically centered
        let mascot_top = (banner_height.saturating_sub(mascot_h)) / 2;
        let mut mascot_para_lines: Vec<Line> = (0..mascot_top).map(|_| Line::from("")).collect();
        mascot_para_lines.extend(self.mascot_lines.clone());
        frame.render_widget(Paragraph::new(mascot_para_lines), horiz[0]);

        // Title — vertically and horizontally centered
        if !title_lines.is_empty() {
            let title_top = (banner_height.saturating_sub(title_h)) / 2;
            let mut lines: Vec<Line> = (0..title_top).map(|_| Line::from("")).collect();
            if std::ptr::eq(title_lines, self.title_lines_large.as_slice()) {
                let t = self.anim_start.elapsed().as_secs_f32() % TITLE_LOOP_SECS;
                lines.extend(animated_title_lines(title_lines, t));
            } else {
                let title_color = theme.general.primary;
                lines.extend(title_lines.iter().map(|l| {
                    Line::from(Span::styled(l.clone(), Style::default().fg(title_color)))
                }));
            }
            frame.render_widget(Paragraph::new(lines).alignment(Alignment::Center), horiz[1]);
        }

        // --- Two-column content area ---
        let content_area = vert[2];
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Min(0)])
            .split(content_area);

        let right_rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Fill(1), Constraint::Fill(1)])
            .split(cols[1]);

        // Compute and store hit-test rects before rendering
        self.recent_rects = panel_row_rects(
            cols[0],
            recent_files.len().saturating_sub(self.scroll_offset),
        );
        self.font_rects = panel_row_rects(right_rows[0], FONT_ACTIONS.len());
        self.image_rects = panel_row_rects(right_rows[1], IMAGE_ACTIONS.len());

        let hovered_recent = self.hovered_recent;
        let hovered_font = self.hovered_font;
        let hovered_image = self.hovered_image;

        self.render_recent_files(frame, cols[0], recent_files, theme, hovered_recent);
        self.render_font_panel(frame, right_rows[0], theme, icons, hovered_font);
        self.render_image_panel(frame, right_rows[1], theme, icons, hovered_image);
    }

    fn render_recent_files(
        &self,
        frame: &mut Frame,
        area: Rect,
        recent_files: &[PathBuf],
        theme: &Theme,
        hovered: Option<usize>,
    ) {
        let block = Block::default()
            .title(" Recent Files ")
            .borders(Borders::ALL)
            .style(Style::default().fg(theme.general.secondary));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let visible_rows = inner.height as usize;
        let mut lines: Vec<Line> = Vec::new();

        if recent_files.is_empty() {
            lines.push(Line::from(Span::styled(
                " No recent files",
                Style::default().fg(theme.dialog.meta),
            )));
        } else {
            for (i, path) in recent_files
                .iter()
                .enumerate()
                .skip(self.scroll_offset)
                .take(visible_rows)
            {
                let local_idx = i - self.scroll_offset;
                let selected = hovered == Some(local_idx);
                let num = i + 1;
                let display = path.to_string_lossy();
                let max_w = inner.width.saturating_sub(5) as usize;
                let label = if display.len() > max_w {
                    format!("…{}", &display[display.len().saturating_sub(max_w)..])
                } else {
                    display.to_string()
                };

                let (num_style, path_style) = if selected {
                    let hl = Style::default()
                        .fg(theme.dialog.highlight)
                        .bg(theme.dialog.selected_bg)
                        .add_modifier(Modifier::BOLD);
                    (hl, hl)
                } else {
                    (
                        Style::default()
                            .fg(theme.general.primary)
                            .add_modifier(Modifier::BOLD),
                        Style::default().fg(theme.dialog.meta),
                    )
                };

                lines.push(Line::from(vec![
                    Span::styled(format!(" {num}. "), num_style),
                    Span::styled(label, path_style),
                ]));
            }

            if recent_files.len() > visible_rows {
                let total = recent_files.len();
                let shown = (self.scroll_offset + visible_rows).min(total);
                lines.push(Line::from(Span::styled(
                    format!(" ↑↓ {}-{}/{}", self.scroll_offset + 1, shown, total),
                    Style::default().fg(theme.general.secondary),
                )));
            }
        }

        frame.render_widget(Paragraph::new(lines), inner);
    }

    fn render_font_panel(
        &self,
        frame: &mut Frame,
        area: Rect,
        theme: &Theme,
        icons: &BTreeMap<String, String>,
        hovered: Option<usize>,
    ) {
        let block = Block::default()
            .title(" Font ")
            .borders(Borders::ALL)
            .style(Style::default().fg(theme.general.secondary));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let lines: Vec<Line> = FONT_ACTIONS
            .iter()
            .enumerate()
            .map(|(i, (icon_key, key_char, suffix))| {
                let icon = icons.get(*icon_key).map(|s| s.as_str()).unwrap_or(" ");
                build_action_row(icon, *key_char, suffix, hovered == Some(i), theme)
            })
            .collect();

        frame.render_widget(Paragraph::new(lines), inner);
    }

    fn render_image_panel(
        &self,
        frame: &mut Frame,
        area: Rect,
        theme: &Theme,
        icons: &BTreeMap<String, String>,
        hovered: Option<usize>,
    ) {
        let block = Block::default()
            .title(" Image ")
            .borders(Borders::ALL)
            .style(Style::default().fg(theme.general.secondary));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let lines: Vec<Line> = IMAGE_ACTIONS
            .iter()
            .enumerate()
            .map(|(i, (icon_key, key_char, suffix))| {
                let icon = icons.get(*icon_key).map(|s| s.as_str()).unwrap_or(" ");
                build_action_row(icon, *key_char, suffix, hovered == Some(i), theme)
            })
            .collect();

        frame.render_widget(Paragraph::new(lines), inner);
    }

    pub fn handle_key(
        &self,
        code: KeyCode,
        modifiers: KeyModifiers,
        recent_count: usize,
    ) -> Option<WelcomeAction> {
        match code {
            KeyCode::Esc if modifiers == KeyModifiers::NONE => Some(WelcomeAction::Dismiss),
            KeyCode::Up => Some(WelcomeAction::ScrollUp),
            KeyCode::Down => Some(WelcomeAction::ScrollDown),
            KeyCode::Char('?') => Some(WelcomeAction::ToggleHelp),
            KeyCode::Char('n') | KeyCode::Char('N') if modifiers == KeyModifiers::NONE => {
                Some(WelcomeAction::FontNewFromSystem)
            }
            KeyCode::Char('i') | KeyCode::Char('I') if modifiers == KeyModifiers::NONE => {
                Some(WelcomeAction::FontNewFromFile)
            }
            KeyCode::Char('b') | KeyCode::Char('B') if modifiers == KeyModifiers::NONE => {
                Some(WelcomeAction::FontNewBlank)
            }
            KeyCode::Char('o') | KeyCode::Char('O') if modifiers == KeyModifiers::NONE => {
                Some(WelcomeAction::FontOpen)
            }
            KeyCode::Char('d') | KeyCode::Char('D') if modifiers == KeyModifiers::NONE => {
                Some(WelcomeAction::FontDuplicate)
            }
            KeyCode::Char('c') | KeyCode::Char('C') if modifiers == KeyModifiers::NONE => {
                Some(WelcomeAction::ImageNewBlank)
            }
            KeyCode::Char('t') | KeyCode::Char('T') if modifiers == KeyModifiers::NONE => {
                Some(WelcomeAction::ImageNewFromTemplate)
            }
            KeyCode::Char('v') | KeyCode::Char('V') if modifiers == KeyModifiers::NONE => {
                Some(WelcomeAction::ImageConvert)
            }
            KeyCode::Char('f') | KeyCode::Char('F') if modifiers == KeyModifiers::NONE => {
                Some(WelcomeAction::ImageOpenFigmap)
            }
            KeyCode::Char('a') | KeyCode::Char('A') if modifiers == KeyModifiers::NONE => {
                Some(WelcomeAction::ImageImportGif)
            }
            KeyCode::Char('l') | KeyCode::Char('L') if modifiers == KeyModifiers::NONE => {
                Some(WelcomeAction::ImageOpen)
            }
            KeyCode::Char('s') | KeyCode::Char('S') if modifiers == KeyModifiers::NONE => {
                Some(WelcomeAction::OpenSettings)
            }
            KeyCode::Char(c) if modifiers == KeyModifiers::CONTROL => match c {
                'o' | 'O' => Some(WelcomeAction::Open),
                'n' | 'N' => Some(WelcomeAction::NewFile),
                _ => None,
            },
            KeyCode::Char(c)
                if modifiers == KeyModifiers::NONE && c.is_ascii_digit() && c != '0' =>
            {
                let idx = (c as u8 - b'1') as usize;
                if idx < recent_count {
                    Some(WelcomeAction::OpenRecent(idx))
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

impl Widget for &WelcomeScreen {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Widget::render(Clear, area, buf);

        let welcome_area = centered_welcome(area);
        Widget::render(Clear, welcome_area, buf);

        let block = Block::default().title(" Welcome ").borders(Borders::ALL);
        let inner = block.inner(welcome_area);
        Widget::render(block, welcome_area, buf);

        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                " Welcome to Figby",
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ];
        Widget::render(Paragraph::new(lines), inner, buf);
    }
}

impl Default for WelcomeScreen {
    fn default() -> Self {
        Self::new()
    }
}

pub fn centered_welcome(area: Rect) -> Rect {
    let w = (area.width / 5 * 3)
        .max(70)
        .min(area.width.saturating_sub(2));
    let h = (area.height / 5 * 3)
        .max(35)
        .min(area.height.saturating_sub(2));
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    Rect {
        x,
        y,
        width: w,
        height: h,
    }
}

/// Compute row rects inside a panel area (borders ALL = 1px each side).
/// Returns up to `count` single-row rects that fit inside the panel.
fn panel_row_rects(panel_area: Rect, count: usize) -> Vec<Rect> {
    let inner_x = panel_area.x + 1;
    let inner_y = panel_area.y + 1;
    let inner_w = panel_area.width.saturating_sub(2);
    let inner_h = panel_area.height.saturating_sub(2);
    (0..count as u16)
        .filter(|&i| i < inner_h)
        .map(|i| Rect {
            x: inner_x,
            y: inner_y + i,
            width: inner_w,
            height: 1,
        })
        .collect()
}

fn font_action_for(idx: usize) -> WelcomeAction {
    match idx {
        0 => WelcomeAction::FontNewFromSystem,
        1 => WelcomeAction::FontNewFromFile,
        2 => WelcomeAction::FontNewBlank,
        3 => WelcomeAction::FontOpen,
        _ => WelcomeAction::FontDuplicate,
    }
}

fn image_action_for(idx: usize) -> WelcomeAction {
    match idx {
        0 => WelcomeAction::ImageNewBlank,
        1 => WelcomeAction::ImageNewFromTemplate,
        2 => WelcomeAction::ImageConvert,
        3 => WelcomeAction::ImageOpenFigmap,
        4 => WelcomeAction::ImageImportGif,
        _ => WelcomeAction::ImageOpen,
    }
}

/// Build a single action row: ` icon [K]suffix`, highlighted if selected.
fn build_action_row<'a>(
    icon: &'a str,
    key_char: char,
    suffix: &'a str,
    selected: bool,
    theme: &Theme,
) -> Line<'a> {
    let base = if selected {
        Style::default()
            .fg(theme.dialog.highlight)
            .bg(theme.dialog.selected_bg)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    let key_style = if selected {
        base
    } else {
        Style::default()
            .fg(theme.general.primary)
            .add_modifier(Modifier::BOLD)
    };
    let suffix_style = if selected {
        base
    } else {
        Style::default().fg(theme.dialog.meta)
    };

    Line::from(vec![
        Span::styled(format!(" {icon} "), base),
        Span::styled(format!("[{key_char}]"), key_style),
        Span::styled(suffix.to_string(), suffix_style),
    ])
}

/// Length of the title animation loop in seconds.
const TITLE_LOOP_SECS: f32 = 10.0;
const TITLE_COLORIZE_END: f32 = 3.0;
const TITLE_FADE_START: f32 = 9.0;
/// Unlit title colour before it is colorized.
const TITLE_BASE: (f32, f32, f32) = (70.0, 74.0, 104.0);
/// Top-to-bottom colour ramp (the Marker-brush sunset): yellow, red, pink, purple.
const TITLE_RAMP: [(f32, f32, f32); 4] = [
    (250.0, 240.0, 70.0),
    (240.0, 80.0, 70.0),
    (255.0, 105.0, 180.0),
    (150.0, 60.0, 170.0),
];

fn lerp3(a: (f32, f32, f32), b: (f32, f32, f32), t: f32) -> (f32, f32, f32) {
    let t = t.clamp(0.0, 1.0);
    (
        a.0 + (b.0 - a.0) * t,
        a.1 + (b.1 - a.1) * t,
        a.2 + (b.2 - a.2) * t,
    )
}

fn ramp_at(y: f32) -> (f32, f32, f32) {
    let pos = y.clamp(0.0, 1.0) * (TITLE_RAMP.len() - 1) as f32;
    let i = (pos.floor() as usize).min(TITLE_RAMP.len() - 2);
    lerp3(TITLE_RAMP[i], TITLE_RAMP[i + 1], pos - i as f32)
}

/// Colour and glyph for one title cell at loop time `t`: the block title is
/// colorized top to bottom, then a light spot sweeps across it (brightening
/// and densifying the shade glyphs), then it fades back to the base colour.
fn title_cell(ch: char, x: usize, y: usize, w: usize, h: usize, t: f32) -> (char, Color) {
    let yf = y as f32 / h.max(2).saturating_sub(1) as f32;
    // colorize: a front moving down the rows
    let front = (t / TITLE_COLORIZE_END) * 1.5;
    let amount = (front - yf * 0.5).clamp(0.0, 1.0);
    let mut rgb = lerp3(TITLE_BASE, ramp_at(yf), amount);
    let mut glyph = ch;
    // light sweep after colorizing
    if t >= TITLE_COLORIZE_END {
        let p =
            ((t - TITLE_COLORIZE_END) / (TITLE_FADE_START - TITLE_COLORIZE_END)).clamp(0.0, 1.0);
        let spot_x = -8.0 + p * (w as f32 + 16.0);
        let spot_y = h as f32 * 0.5;
        // cells are ~2x taller than wide
        let d = (((x as f32 - spot_x).powi(2)) + ((y as f32 - spot_y) * 2.0).powi(2)).sqrt();
        let f = (1.0 - d / 12.0).clamp(0.0, 1.0);
        let f = f * f;
        rgb = lerp3(rgb, (255.0, 255.0, 255.0), f * 0.75);
        if f > 0.35 {
            glyph = match ch {
                '░' => '▒',
                '▒' => '▓',
                '▓' => '█',
                c => c,
            };
        }
    }
    // fade the colour back out so the loop restarts cleanly
    if t >= TITLE_FADE_START {
        let k = (t - TITLE_FADE_START) / (TITLE_LOOP_SECS - TITLE_FADE_START);
        rgb = lerp3(rgb, TITLE_BASE, k);
    }
    (glyph, Color::Rgb(rgb.0 as u8, rgb.1 as u8, rgb.2 as u8))
}

fn animated_title_lines(rows: &[String], t: f32) -> Vec<Line<'static>> {
    let h = rows.len();
    let w = rows.iter().map(|r| r.chars().count()).max().unwrap_or(0);
    rows.iter()
        .enumerate()
        .map(|(y, row)| {
            let mut spans: Vec<Span<'static>> = Vec::new();
            let mut run = String::new();
            let mut run_color: Option<Color> = None;
            for (x, ch) in row.chars().enumerate() {
                let (glyph, color) = if ch == ' ' {
                    (' ', Color::Reset)
                } else {
                    title_cell(ch, x, y, w, h, t)
                };
                if run_color != Some(color) && !run.is_empty() {
                    spans.push(Span::styled(
                        std::mem::take(&mut run),
                        Style::default().fg(run_color.unwrap_or(Color::Reset)),
                    ));
                }
                run_color = Some(color);
                run.push(glyph);
            }
            if !run.is_empty() {
                spans.push(Span::styled(
                    run,
                    Style::default().fg(run_color.unwrap_or(Color::Reset)),
                ));
            }
            Line::from(spans)
        })
        .collect()
}

/// The bundled `bob` FIGfont (same one used in the recordings), embedded so
/// the welcome title never depends on system FIGlet fonts being installed.
const BOB_FONT: &str = include_str!("../../assets/fonts/bob.flf");

fn render_title_with_bob() -> Vec<String> {
    let Ok(font) = crate::font::parse_font_bytes(BOB_FONT) else {
        return Vec::new();
    };
    let rows = crate::render::render_string(&font, "FIGBY");
    let trimmed: Vec<String> = rows
        .into_iter()
        .map(|l| l.replace('\u{00A0}', " "))
        .collect();
    let last_content = trimmed
        .iter()
        .rposition(|l| !l.trim_end().is_empty())
        .unwrap_or(0);
    trimmed[..=last_content].to_vec()
}

/// Narrow-terminal fallback: plain text, not the old hand-drawn art.
fn ascii_fallback_title() -> Vec<String> {
    vec!["F I G B Y".to_string()]
}

fn parse_ansi_lines(text: &str) -> Vec<Line<'static>> {
    text.lines()
        .map(|raw| {
            let mut spans: Vec<Span<'static>> = Vec::new();
            let mut current_color: Option<Color> = None;
            let mut current_text = String::new();
            let bytes = raw.as_bytes();
            let mut i = 0;

            while i < bytes.len() {
                if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
                    if !current_text.is_empty() {
                        let style =
                            current_color.map_or(Style::default(), |c| Style::default().fg(c));
                        spans.push(Span::styled(current_text.clone(), style));
                        current_text.clear();
                    }
                    i += 2;
                    let start = i;
                    while i < bytes.len() && bytes[i] != b'm' {
                        i += 1;
                    }
                    let seq = std::str::from_utf8(&bytes[start..i]).unwrap_or("");
                    i += 1;

                    if seq == "0" || seq.is_empty() {
                        current_color = None;
                    } else {
                        let parts: Vec<u8> =
                            seq.split(';').filter_map(|s| s.parse().ok()).collect();
                        if parts.len() == 5 && parts[0] == 38 && parts[1] == 2 {
                            current_color = Some(Color::Rgb(parts[2], parts[3], parts[4]));
                        }
                    }
                } else {
                    let ch_start = i;
                    i += 1;
                    while i < bytes.len() && (bytes[i] & 0xC0) == 0x80 {
                        i += 1;
                    }
                    if let Ok(s) = std::str::from_utf8(&bytes[ch_start..i]) {
                        current_text.push_str(s);
                    }
                }
            }

            if !current_text.is_empty() {
                let style = current_color.map_or(Style::default(), |c| Style::default().fg(c));
                spans.push(Span::styled(current_text, style));
            }

            Line::from(spans)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_shortcuts_have_no_duplicates() {
        let mut seen = std::collections::HashSet::new();
        for (_, key_char, _) in FONT_ACTIONS.iter().chain(IMAGE_ACTIONS.iter()) {
            assert!(
                seen.insert(key_char.to_ascii_lowercase()),
                "duplicate welcome screen shortcut '{key_char}'"
            );
        }
    }

    #[test]
    fn test_every_action_char_dispatches_its_own_action() {
        let screen = WelcomeScreen::new();
        for (i, (_, key_char, _)) in FONT_ACTIONS.iter().enumerate() {
            assert_eq!(
                screen.handle_key(KeyCode::Char(*key_char), KeyModifiers::NONE, 0),
                Some(font_action_for(i)),
                "font action {i} shortcut '{key_char}' should dispatch its own action"
            );
        }
        for (i, (_, key_char, _)) in IMAGE_ACTIONS.iter().enumerate() {
            assert_eq!(
                screen.handle_key(KeyCode::Char(*key_char), KeyModifiers::NONE, 0),
                Some(image_action_for(i)),
                "image action {i} shortcut '{key_char}' should dispatch its own action"
            );
        }
    }

    #[test]
    fn test_action_shortcuts_work_without_shift() {
        let screen = WelcomeScreen::new();
        for (_, key_char, _) in FONT_ACTIONS.iter().chain(IMAGE_ACTIONS.iter()) {
            let lower = key_char.to_ascii_lowercase();
            assert!(
                screen
                    .handle_key(KeyCode::Char(lower), KeyModifiers::NONE, 0)
                    .is_some(),
                "shortcut '{key_char}' should also work as unshifted '{lower}'"
            );
        }
    }
}

#[cfg(test)]
mod title_anim_tests {
    use super::*;

    #[test]
    fn title_animation_loops_through_phases() {
        let rows = vec!["█▒░ █".to_string(), "▓▓ ░█".to_string()];
        let start = animated_title_lines(&rows, 0.0);
        let mid = animated_title_lines(&rows, 6.0);
        let end = animated_title_lines(&rows, TITLE_LOOP_SECS - 0.001);
        assert_eq!(start.len(), 2);
        // colorized by mid-loop, back near the base colour at loop end
        assert_ne!(start, mid);
        let colour_of = |l: &Vec<Line>| l[0].spans[0].style.fg;
        assert_ne!(colour_of(&mid), colour_of(&start));
        let near = |a: Option<Color>, b: Option<Color>| match (a, b) {
            (Some(Color::Rgb(r1, g1, b1)), Some(Color::Rgb(r2, g2, b2))) => {
                (r1 as i32 - r2 as i32).abs() < 8
                    && (g1 as i32 - g2 as i32).abs() < 8
                    && (b1 as i32 - b2 as i32).abs() < 8
            }
            _ => false,
        };
        assert!(
            near(colour_of(&end), colour_of(&start)),
            "loop restarts seamlessly"
        );
    }
}
