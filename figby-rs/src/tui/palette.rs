use super::canvas::CanvasCell;
use super::theme::Theme;
use crossterm::event::KeyCode;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorTarget {
    Foreground,
    Background,
}

impl ColorTarget {
    fn toggle(&mut self) {
        *self = match self {
            ColorTarget::Foreground => ColorTarget::Background,
            ColorTarget::Background => ColorTarget::Foreground,
        };
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HueGroup {
    Neutrals,
    Reds,
    Oranges,
    Yellows,
    Greens,
    Cyans,
    Blues,
    Purples,
}

impl HueGroup {
    pub fn display_name(&self) -> &'static str {
        match self {
            HueGroup::Neutrals => "Neutrals",
            HueGroup::Reds => "Reds",
            HueGroup::Oranges => "Oranges",
            HueGroup::Yellows => "Yellows",
            HueGroup::Greens => "Greens",
            HueGroup::Cyans => "Cyans",
            HueGroup::Blues => "Blues",
            HueGroup::Purples => "Purples",
        }
    }

    pub fn ordered() -> &'static [HueGroup] {
        &[
            HueGroup::Neutrals,
            HueGroup::Reds,
            HueGroup::Oranges,
            HueGroup::Yellows,
            HueGroup::Greens,
            HueGroup::Cyans,
            HueGroup::Blues,
            HueGroup::Purples,
        ]
    }
}

/// Named character groups for the palette char picker.
pub struct CharGroup {
    pub name: &'static str,
    pub chars: &'static str,
}

/// All palette char groups, ordered for display.
/// "deluxe" is listed first as the richest set, combining ASCII printable,
/// blocks, box drawing, dithered, geometric shapes, braille, and ogham.
pub const CHAR_GROUPS: &[CharGroup] = &[
    CharGroup {
        name: "deluxe",
        chars: concat!(
            " !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~",
            "▀▁▂▃▄▅▆▇█▉▊▋▌▍▎▏▐░▒▓▔▕▖▗▘▙▚▛▜▝▞▟",
            "─━│┃┄┅┆┇┈┉┊┋┌┍┎┏┐┑┒┓└┕┖┗┘┙┚┛├┝┞┟┠┡┢┣┤┥┦┧┨┩┪┫┬┭┮┯┰┱┲┳┴┵┶┷┸┹┺┻┼┽┾┿╀╁╂╃╄╅╆╇╈╉╊╋╌╍╎╏═║╒╓╔╕╖╗╘╙╚╛╜╝╞╟╠╡╢╣╤╥╦╧╨╩╪╫╬╭╮╯╰╱╲╳╴╵╶╷╸╹╺╻╼╽╾╿",
            "░▒▓",
            "■□▪▫▲△▶▷▼▽◀◁◆◇◈◊○◎●◐◑◦◯",
            "⠀⠁⠂⠃⠄⠅⠆⠇⠈⠉⠊⠋⠌⠍⠎⠏⠐⠑⠒⠓⠔⠕⠖⠗⠘⠙⠚⠛⠜⠝⠞⠟⠠⠡⠢⠣⠤⠥⠦⠧⠨⠩⠪⠫⠬⠭⠮⠯⠰⠱⠲⠳⠴⠵⠶⠷⠸⠹⠺⠻⠼⠽⠾⠿⡀⡁⡂⡃⡄⡅⡆⡇⡈⡉⡊⡋⡌⡍⡎⡏⡐⡑⡒⡓⡔⡕⡖⡗⡘⡙⡚⡛⡜⡝⡞⡟⡠⡡⡢⡣⡤⡥⡦⡧⡨⡩⡪⡫⡬⡭⡮⡯⡰⡱⡲⡳⡴⡵⡶⡷⡸⡹⡺⡻⡼⡽⡾⡿⢀⢁⢂⢃⢄⢅⢆⢇⢈⢉⢊⢋⢌⢍⢎⢏⢐⢑⢒⢓⢔⢕⢖⢗⢘⢙⢚⢛⢜⢝⢞⢟⢠⢡⢢⢣⢤⢥⢦⢧⢨⢩⢪⢫⢬⢭⢮⢯⢰⢱⢲⢳⢴⢵⢶⢷⢸⢹⢺⢻⢼⢽⢾⢿⣀⣁⣂⣃⣄⣅⣆⣇⣈⣉⣊⣋⣌⣍⣎⣏⣐⣑⣒⣓⣔⣕⣖⣗⣘⣙⣚⣛⣜⣝⣞⣟⣠⣡⣢⣣⣤⣥⣦⣧⣨⣩⣪⣫⣬⣭⣮⣯⣰⣱⣲⣳⣴⣵⣶⣷⣸⣹⣺⣻⣼⣽⣾⣿",
            " ᚁᚂᚃᚄᚅᚆᚇᚈᚉᚊᚋᚌᚍᚎᚏᚐᚑᚒᚓᚔᚕᚖᚗᚘᚙᚚ᚛᚜",
        ),
    },
    CharGroup { name: "ascii",   chars: " !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~" },
    CharGroup { name: "braille", chars: "⠀⠁⠂⠃⠄⠅⠆⠇⠈⠉⠊⠋⠌⠍⠎⠏⠐⠑⠒⠓⠔⠕⠖⠗⠘⠙⠚⠛⠜⠝⠞⠟⠠⠡⠢⠣⠤⠥⠦⠧⠨⠩⠪⠫⠬⠭⠮⠯⠰⠱⠲⠳⠴⠵⠶⠷⠸⠹⠺⠻⠼⠽⠾⠿⡀⡁⡂⡃⡄⡅⡆⡇⡈⡉⡊⡋⡌⡍⡎⡏⡐⡑⡒⡓⡔⡕⡖⡗⡘⡙⡚⡛⡜⡝⡞⡟⡠⡡⡢⡣⡤⡥⡦⡧⡨⡩⡪⡫⡬⡭⡮⡯⡰⡱⡲⡳⡴⡵⡶⡷⡸⡹⡺⡻⡼⡽⡾⡿⢀⢁⢂⢃⢄⢅⢆⢇⢈⢉⢊⢋⢌⢍⢎⢏⢐⢑⢒⢓⢔⢕⢖⢗⢘⢙⢚⢛⢜⢝⢞⢟⢠⢡⢢⢣⢤⢥⢦⢧⢨⢩⢪⢫⢬⢭⢮⢯⢰⢱⢲⢳⢴⢵⢶⢷⢸⢹⢺⢻⢼⢽⢾⢿⣀⣁⣂⣃⣄⣅⣆⣇⣈⣉⣊⣋⣌⣍⣎⣏⣐⣑⣒⣓⣔⣕⣖⣗⣘⣙⣚⣛⣜⣝⣞⣟⣠⣡⣢⣣⣤⣥⣦⣧⣨⣩⣪⣫⣬⣭⣮⣯⣰⣱⣲⣳⣴⣵⣶⣷⣸⣹⣺⣻⣼⣽⣾⣿" },
    CharGroup { name: "blocks",  chars: "▀▁▂▃▄▅▆▇█▉▊▋▌▍▎▏▐░▒▓▔▕▖▗▘▙▚▛▜▝▞▟" },
    CharGroup { name: "box",     chars: "─━│┃┄┅┆┇┈┉┊┋┌┍┎┏┐┑┒┓└┕┖┗┘┙┚┛├┝┞┟┠┡┢┣┤┥┦┧┨┩┪┫┬┭┮┯┰┱┲┳┴┵┶┷┸┹┺┻┼┽┾┿╀╁╂╃╄╅╆╇╈╉╊╋╌╍╎╏═║╒╓╔╕╖╗╘╙╚╛╜╝╞╟╠╡╢╣╤╥╦╧╨╩╪╫╬╭╮╯╰╱╲╳╴╵╶╷╸╹╺╻╼╽╾╿" },
    CharGroup { name: "dithered", chars: "░▒▓" },
    CharGroup { name: "geometric", chars: "■□▪▫▲△▶▷▼▽◀◁◆◇◈◊○◎●◐◑◦◯" },
    CharGroup { name: "ogham",   chars: " ᚁᚂᚃᚄᚅᚆᚇᚈᚉᚊᚋᚌᚍᚎᚏᚐᚑᚒᚓᚔᚕᚖᚗᚘᚙᚚ᚛᚜" },
];

pub const ANSI_COLOR_NAMES: [&str; 16] = [
    "Black",
    "Red",
    "Green",
    "Yellow",
    "Blue",
    "Magenta",
    "Cyan",
    "White",
    "Bright Black",
    "Bright Red",
    "Bright Green",
    "Bright Yellow",
    "Bright Blue",
    "Bright Magenta",
    "Bright Cyan",
    "Bright White",
];

pub const ANSI_16_COLORS: [Color; 16] = [
    Color::Indexed(0),
    Color::Indexed(1),
    Color::Indexed(2),
    Color::Indexed(3),
    Color::Indexed(4),
    Color::Indexed(5),
    Color::Indexed(6),
    Color::Indexed(7),
    Color::Indexed(8),
    Color::Indexed(9),
    Color::Indexed(10),
    Color::Indexed(11),
    Color::Indexed(12),
    Color::Indexed(13),
    Color::Indexed(14),
    Color::Indexed(15),
];

pub fn hue_group_for_ansi(index: usize) -> HueGroup {
    match index {
        0 | 7 | 8 | 15 => HueGroup::Neutrals,
        1 | 9 => HueGroup::Reds,
        3 | 11 => HueGroup::Yellows,
        2 | 10 => HueGroup::Greens,
        6 | 14 => HueGroup::Cyans,
        4 | 12 => HueGroup::Blues,
        5 | 13 => HueGroup::Purples,
        _ => HueGroup::Neutrals,
    }
}

/// Build a mapping from RGB color to swatch index for exact-match lookup.
/// Used by the shading pipeline to find which swatch a cell's fg color belongs to.
pub fn build_rgb_to_swatch(
    swatches: &[(String, String)],
) -> std::collections::HashMap<(u8, u8, u8), usize> {
    let mut map = std::collections::HashMap::new();
    for (i, (_name, hex)) in swatches.iter().enumerate() {
        if let Some([r, g, b]) = super::theme::parse_hex_rgb(hex) {
            map.insert((r, g, b), i);
        }
    }
    map
}

pub fn build_flat_palette() -> Vec<(usize, Color, &'static str)> {
    let mut result = Vec::with_capacity(16);
    for group in HueGroup::ordered() {
        for (i, color) in ANSI_16_COLORS.iter().enumerate() {
            if hue_group_for_ansi(i) == *group {
                result.push((i, *color, ANSI_COLOR_NAMES[i]));
            }
        }
    }
    result
}

fn extended_color(page: u8, offset: u8) -> Color {
    let idx = 16u16 + page as u16 * 16 + offset as u16;
    Color::Indexed(idx.min(255) as u8)
}

const MAX_RECENT: usize = 8;

pub struct Palette {
    pub target: ColorTarget,
    pub selected_color: Option<Color>,
    pub recent: Vec<Color>,
    selected_index: usize,
    custom_hex: String,
    custom_mode: bool,
    show_extended: bool,
    extended_page: u8,
    pub hover_index: Option<usize>,
    pub multi_select_indices: Vec<usize>,
    multi_select_active: bool,
    pub theme: Theme,
}

impl Palette {
    pub fn new() -> Self {
        Self {
            target: ColorTarget::Foreground,
            selected_color: None,
            selected_index: 0,
            recent: Vec::with_capacity(MAX_RECENT),
            custom_hex: String::new(),
            custom_mode: false,
            show_extended: false,
            extended_page: 0,
            hover_index: None,
            multi_select_indices: Vec::new(),
            multi_select_active: false,
            theme: Theme::default(),
        }
    }

    pub fn toggle_target(&mut self) {
        self.target.toggle();
    }

    pub fn select_color(&mut self, index: usize) {
        let color = self.current_color(index);
        self.selected_index = index;
        self.selected_color = Some(color);
        self.push_recent(color);
        self.multi_select_indices.clear();
    }

    pub fn toggle_multi_select_color(&mut self, index: usize) {
        if let Some(pos) = self.multi_select_indices.iter().position(|i| *i == index) {
            self.multi_select_indices.remove(pos);
        } else {
            self.multi_select_indices.push(index);
        }
    }

    pub fn has_multi_select(&self) -> bool {
        self.multi_select_indices.len() >= 2
    }

    pub fn selected_color_array(&self) -> Vec<Color> {
        let flat = build_flat_palette();
        self.multi_select_indices
            .iter()
            .filter_map(|i| flat.get(*i))
            .map(|e| e.1)
            .collect()
    }

    fn current_color(&self, index: usize) -> Color {
        if self.show_extended {
            if index < 16 {
                extended_color(self.extended_page, index as u8)
            } else {
                ANSI_16_COLORS[0]
            }
        } else {
            let flat = build_flat_palette();
            if index < flat.len() {
                flat[index].1
            } else {
                ANSI_16_COLORS[0]
            }
        }
    }

    fn visible_count(&self) -> usize {
        16
    }

    pub fn push_recent(&mut self, color: Color) {
        self.recent.retain(|c| *c != color);
        self.recent.push(color);
        if self.recent.len() > MAX_RECENT {
            self.recent.remove(0);
        }
    }

    pub fn set_custom_hex(&mut self, hex: &str) -> bool {
        self.custom_hex.clear();
        self.custom_hex.push_str(hex);
        match super::theme::parse_hex_rgb(hex) {
            Some([r, g, b]) => {
                let color = Color::Rgb(r, g, b);
                self.selected_color = Some(color);
                self.push_recent(color);
                true
            }
            None => false,
        }
    }

    pub fn apply_to_cell(&self, cell: &mut CanvasCell) {
        if let Some(color) = self.selected_color {
            match self.target {
                ColorTarget::Foreground => cell.fg = Some(color),
                ColorTarget::Background => cell.bg = Some(color),
            }
        }
    }

    pub fn handle_key(&mut self, code: KeyCode) -> bool {
        if self.custom_mode {
            return match code {
                KeyCode::Char(c) if c.is_ascii_hexdigit() || c == '#' => {
                    if self.custom_hex.len() < 7 {
                        self.custom_hex.push(c);
                    }
                    true
                }
                KeyCode::Backspace => {
                    self.custom_hex.pop();
                    true
                }
                KeyCode::Enter => {
                    let plain = self.custom_hex.trim_start_matches('#');
                    let hex = format!("#{:0>6}", plain);
                    self.custom_hex = hex.clone();
                    self.set_custom_hex(&hex);
                    self.custom_mode = false;
                    true
                }
                KeyCode::Esc => {
                    self.custom_mode = false;
                    true
                }
                _ => false,
            };
        }
        match code {
            KeyCode::Char('x') | KeyCode::Char('X') => {
                self.toggle_target();
                true
            }
            KeyCode::Char('f') | KeyCode::Char('F') => {
                self.target = ColorTarget::Foreground;
                true
            }
            KeyCode::Char('h') | KeyCode::Char('H') => {
                self.custom_mode = true;
                self.custom_hex.clear();
                true
            }
            KeyCode::Char('z') | KeyCode::Char('Z') => {
                self.show_extended = !self.show_extended;
                self.selected_index = 0;
                true
            }
            KeyCode::Left => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                }
                true
            }
            KeyCode::Right => {
                let max_idx = self.visible_count().saturating_sub(1);
                if self.selected_index < max_idx {
                    self.selected_index += 1;
                }
                true
            }
            KeyCode::Up => {
                if self.selected_index >= 5 {
                    self.selected_index -= 5;
                }
                true
            }
            KeyCode::Down => {
                let max_idx = self.visible_count().saturating_sub(1);
                if self.selected_index + 5 <= max_idx {
                    self.selected_index += 5;
                }
                true
            }
            KeyCode::Tab => {
                self.multi_select_active = !self.multi_select_active;
                if self.multi_select_active {
                    self.multi_select_indices.clear();
                }
                true
            }
            KeyCode::Enter => {
                if self.multi_select_active {
                    self.toggle_multi_select_color(self.selected_index);
                } else {
                    self.select_color(self.selected_index);
                }
                true
            }
            _ => false,
        }
    }

    fn color_name(&self, index: usize) -> String {
        if self.show_extended {
            let abs = 16u16 + self.extended_page as u16 * 16 + index as u16;
            format!("Color {}", abs)
        } else {
            let flat = build_flat_palette();
            if index < flat.len() {
                flat[index].2.to_string()
            } else {
                String::new()
            }
        }
    }

    /// Walk groups in standard mode to find visual index at (rel_col, rel_row).
    fn standard_index_at(&self, rel_col: u16, rel_row: u16) -> Option<usize> {
        if rel_row == 0 {
            return None;
        }
        let flat = build_flat_palette();
        let mut current_inner_row = 1u16;
        let mut flat_offset = 0usize;

        for group in HueGroup::ordered() {
            let group_count = flat
                .iter()
                .filter(|(i, _, _)| hue_group_for_ansi(*i) == *group)
                .count();
            if group_count == 0 {
                continue;
            }

            if rel_row == current_inner_row {
                return None;
            }
            current_inner_row += 1;

            let data_rows = group_count.div_ceil(5);
            let data_end = current_inner_row + data_rows as u16;

            if (current_inner_row..data_end).contains(&rel_row) {
                let data_row = (rel_row - current_inner_row) as usize;
                let swatch_col = (rel_col / 2) as usize;
                let index = flat_offset + data_row * 5 + swatch_col;
                if index < flat_offset + group_count {
                    return Some(index);
                }
                return None;
            }

            current_inner_row = data_end;
            flat_offset += group_count;
        }

        None
    }

    /// Hit-test a mouse move at terminal coordinates (`col`, `row`) against `area`
    /// and set `hover_index` accordingly. Returns `true` if hover state changed.
    pub fn handle_hover(&mut self, col: u16, row: u16, area: Rect) -> bool {
        let ix = area.x + 1;
        let iy = area.y + 1;
        let iw = area.width.saturating_sub(2);
        let ih = area.height.saturating_sub(2);
        if col < ix || col >= ix + iw || row < iy || row >= iy + ih {
            let changed = self.hover_index.is_some();
            self.hover_index = None;
            return changed;
        }
        let rel_col = col - ix;
        let rel_row = row - iy;

        let (swatch_start, swatch_end) = if self.show_extended {
            (2u16, 6u16)
        } else {
            (1u16, 20u16)
        };

        if (swatch_start..swatch_end).contains(&rel_row) {
            if self.show_extended {
                let swatch_col = (rel_col / 2) as usize;
                if swatch_col < 5 {
                    let idx = (rel_row as usize - 2) * 5 + swatch_col;
                    if idx < 16 {
                        if self.hover_index != Some(idx) {
                            self.hover_index = Some(idx);
                            return true;
                        }
                        return false;
                    }
                }
            } else if let Some(idx) = self.standard_index_at(rel_col, rel_row) {
                if self.hover_index != Some(idx) {
                    self.hover_index = Some(idx);
                    return true;
                }
                return false;
            }
        }

        let changed = self.hover_index.is_some();
        self.hover_index = None;
        changed
    }

    /// Hit-test a left click at terminal coordinates (`col`, `row`) against `area`
    /// (the full panel rect including border). Returns true if the click landed on
    /// a colour swatch or FG/BG toggle and state changed.
    pub fn handle_click(&mut self, col: u16, row: u16, area: Rect) -> bool {
        // inner area: strip 1-cell border on each side
        let ix = area.x + 1;
        let iy = area.y + 1;
        let iw = area.width.saturating_sub(2);
        let ih = area.height.saturating_sub(2);
        if col < ix || col >= ix + iw || row < iy || row >= iy + ih {
            return false;
        }
        let rel_col = col - ix;
        let rel_row = row - iy;

        // Row 0: " [FG]" (0..4)  " " (5)  " [BG]" (6..10)
        if rel_row == 0 {
            if rel_col < 5 {
                self.target = ColorTarget::Foreground;
                return true;
            } else if (6..11).contains(&rel_col) {
                self.target = ColorTarget::Background;
                return true;
            }
            return false;
        }

        if self.show_extended {
            // Row 1: "Ext pg:N" — not clickable
            // Rows 2-5: 5 swatches each, 2 cols wide (5+5+5+1)
            if (2..=5).contains(&rel_row) {
                let swatch_col = (rel_col / 2) as usize;
                if swatch_col < 5 {
                    let idx = (rel_row as usize - 2) * 5 + swatch_col;
                    if idx < 16 {
                        self.selected_index = idx;
                        self.selected_color = Some(extended_color(self.extended_page, idx as u8));
                        return true;
                    }
                }
            }
        } else if let Some(idx) = self.standard_index_at(rel_col, rel_row) {
            self.select_color(idx);
            return true;
        }

        false
    }
}

impl Widget for &Palette {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default().title(" Palette ").borders(Borders::ALL);
        let inner = block.inner(area);
        Widget::render(block, area, buf);

        if inner.width < 4 || inner.height < 2 {
            return;
        }

        let mut lines: Vec<Line<'_>> = Vec::new();

        let active_style = Style::default()
            .fg(self.theme.palette.active_target)
            .bg(self.theme.palette.cell_bg);
        let fg_label = if self.target == ColorTarget::Foreground {
            Span::styled(" [FG]", active_style)
        } else {
            Span::styled(" [FG]", Style::default())
        };
        let bg_label = if self.target == ColorTarget::Background {
            Span::styled(" [BG]", active_style)
        } else {
            Span::styled(" [BG]", Style::default())
        };
        lines.push(Line::from(vec![fg_label, Span::raw(" "), bg_label]));

        if self.multi_select_active {
            lines.push(Line::from(Span::styled(
                " Sel:",
                Style::default().fg(self.theme.general.secondary),
            )));
        }

        if self.show_extended {
            lines.push(Line::from(Span::raw(format!(
                " Ext pg:{}",
                self.extended_page + 1
            ))));

            for row in 0..4 {
                let mut spans = Vec::new();
                for col in 0..5 {
                    let idx = row * 5 + col;
                    if idx < 16 {
                        let color = extended_color(self.extended_page, idx as u8);
                        let is_multi = self.multi_select_indices.contains(&idx);
                        let swatch = if idx == self.selected_index {
                            let fg = if is_multi {
                                Color::White
                            } else {
                                self.theme.palette.swatch_indicator
                            };
                            Span::styled("██", Style::default().bg(color).fg(fg))
                        } else if is_multi {
                            Span::styled("██", Style::default().bg(color).fg(Color::White))
                        } else {
                            Span::styled("  ", Style::default().bg(color))
                        };
                        spans.push(swatch);
                    }
                }
                lines.push(Line::from(spans));
            }
            lines.push(Line::from(Span::raw(" < > pages")));
        } else {
            let flat = build_flat_palette();
            let dim_style = Style::default().fg(self.theme.general.secondary);
            let mut visual_idx = 0usize;

            for group in HueGroup::ordered() {
                let group_entries: Vec<&(usize, Color, &'static str)> = flat
                    .iter()
                    .filter(|(i, _, _)| hue_group_for_ansi(*i) == *group)
                    .collect();

                if group_entries.is_empty() {
                    continue;
                }

                lines.push(Line::from(Span::styled(
                    format!(" {}", group.display_name()),
                    dim_style,
                )));

                for chunk in group_entries.chunks(5) {
                    let mut spans = Vec::new();
                    for entry in chunk {
                        let (_orig_idx, color, _name) = **entry;
                        let is_multi = self.multi_select_indices.contains(&visual_idx);
                        let swatch = if visual_idx == self.selected_index {
                            let fg = if is_multi {
                                Color::White
                            } else {
                                self.theme.palette.swatch_indicator
                            };
                            Span::styled("██", Style::default().bg(color).fg(fg))
                        } else if is_multi {
                            Span::styled("██", Style::default().bg(color).fg(Color::White))
                        } else {
                            Span::styled("  ", Style::default().bg(color))
                        };
                        spans.push(swatch);
                        visual_idx += 1;
                    }
                    lines.push(Line::from(spans));
                }
            }
        }

        // Hover tooltip: show color name below swatches
        if let Some(hover) = self.hover_index {
            let name = self.color_name(hover);
            if !name.is_empty() {
                lines.push(Line::from(Span::styled(
                    format!(" {}", name),
                    Style::default().fg(self.theme.general.secondary),
                )));
            }
        }

        if self.custom_mode {
            let hex_display = format!(" #{}", self.custom_hex);
            lines.push(Line::from(Span::raw(hex_display)));
        } else {
            let custom_display = match self.selected_color {
                Some(Color::Rgb(r, g, b)) => format!(" #{:02X}{:02X}{:02X}", r, g, b),
                _ => " Cst:#......".to_string(),
            };
            lines.push(Line::from(Span::raw(custom_display)));
        }

        if !self.recent.is_empty() {
            lines.push(Line::from(Span::raw(" Recent:")));
            let mut recent_spans = Vec::new();
            for color in self.recent.iter().rev().take(8) {
                recent_spans.push(Span::styled("  ", Style::default().bg(*color)));
            }
            lines.push(Line::from(recent_spans));
        }

        let paragraph = Paragraph::new(lines);
        Widget::render(paragraph, inner, buf);
    }
}

impl Default for Palette {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::Palette;
    use super::CHAR_GROUPS;
    use ratatui::layout::Rect;

    #[test]
    fn test_braille_palette_group_length() {
        let braille = CHAR_GROUPS
            .iter()
            .find(|g| g.name == "braille")
            .expect("braille group should exist");
        let count = braille.chars.chars().count();
        assert_eq!(
            count, 256,
            "braille group should have exactly 256 chars, got {count}"
        );
    }

    #[test]
    fn test_braille_palette_all_in_range() {
        let braille = CHAR_GROUPS
            .iter()
            .find(|g| g.name == "braille")
            .expect("braille group should exist");
        for c in braille.chars.chars() {
            let cp = c as u32;
            assert!(
                (0x2800..=0x28FF).contains(&cp),
                "braille char U+{cp:04X} outside U+2800–U+28FF"
            );
        }
    }

    #[test]
    fn test_braille_palette_all_256_unique() {
        let braille = CHAR_GROUPS
            .iter()
            .find(|g| g.name == "braille")
            .expect("braille group should exist");
        let mut cps: Vec<u32> = braille.chars.chars().map(|c| c as u32).collect();
        assert_eq!(cps.len(), 256, "should have 256 braille chars");
        cps.sort_unstable();
        cps.dedup();
        assert_eq!(cps.len(), 256, "should have 256 unique braille codepoints");
        assert_eq!(cps[0], 0x2800, "first codepoint should be U+2800");
        assert_eq!(cps[255], 0x28FF, "last codepoint should be U+28FF");
        for (i, &cp) in cps.iter().enumerate() {
            assert_eq!(
                cp,
                0x2800 + i as u32,
                "missing codepoint U+{:04X}",
                0x2800 + i as u32
            );
        }
    }

    #[test]
    fn test_blocks_palette_count_32() {
        let blocks = CHAR_GROUPS
            .iter()
            .find(|g| g.name == "blocks")
            .expect("blocks group should exist");
        let count = blocks.chars.chars().count();
        assert_eq!(
            count, 32,
            "blocks group should have exactly 32 chars, got {count}"
        );
    }

    #[test]
    fn test_blocks_palette_all_in_range() {
        let blocks = CHAR_GROUPS
            .iter()
            .find(|g| g.name == "blocks")
            .expect("blocks group should exist");
        for c in blocks.chars.chars() {
            let cp = c as u32;
            assert!(
                (0x2580..=0x259F).contains(&cp),
                "blocks char U+{cp:04X} outside U+2580–U+259F"
            );
        }
    }

    #[test]
    fn test_blocks_palette_all_32_unique() {
        let blocks = CHAR_GROUPS
            .iter()
            .find(|g| g.name == "blocks")
            .expect("blocks group should exist");
        let mut cps: Vec<u32> = blocks.chars.chars().map(|c| c as u32).collect();
        assert_eq!(cps.len(), 32, "should have 32 blocks chars");
        cps.sort_unstable();
        cps.dedup();
        assert_eq!(cps.len(), 32, "should have 32 unique blocks codepoints");
        assert_eq!(cps[0], 0x2580, "first codepoint should be U+2580");
        assert_eq!(cps[31], 0x259F, "last codepoint should be U+259F");
        for (i, &cp) in cps.iter().enumerate() {
            assert_eq!(
                cp,
                0x2580 + i as u32,
                "missing codepoint U+{:04X}",
                0x2580 + i as u32
            );
        }
    }

    // --- Box palette tests ---

    #[test]
    fn test_box_palette_count_128() {
        let box_group = CHAR_GROUPS
            .iter()
            .find(|g| g.name == "box")
            .expect("box group should exist");
        let count = box_group.chars.chars().count();
        assert_eq!(
            count, 128,
            "box group should have exactly 128 chars, got {count}"
        );
    }

    #[test]
    fn test_box_palette_all_in_range() {
        let box_group = CHAR_GROUPS
            .iter()
            .find(|g| g.name == "box")
            .expect("box group should exist");
        for c in box_group.chars.chars() {
            let cp = c as u32;
            assert!(
                (0x2500..=0x257F).contains(&cp),
                "box char U+{cp:04X} outside U+2500–U+257F"
            );
        }
    }

    #[test]
    fn test_box_palette_all_128_unique() {
        let box_group = CHAR_GROUPS
            .iter()
            .find(|g| g.name == "box")
            .expect("box group should exist");
        let mut cps: Vec<u32> = box_group.chars.chars().map(|c| c as u32).collect();
        assert_eq!(cps.len(), 128, "should have 128 box chars");
        cps.sort_unstable();
        cps.dedup();
        assert_eq!(cps.len(), 128, "should have 128 unique box codepoints");
        assert_eq!(cps[0], 0x2500, "first codepoint should be U+2500");
        assert_eq!(cps[127], 0x257F, "last codepoint should be U+257F");
        for (i, &cp) in cps.iter().enumerate() {
            assert_eq!(
                cp,
                0x2500 + i as u32,
                "missing codepoint U+{:04X}",
                0x2500 + i as u32
            );
        }
    }

    // --- Dithered palette tests ---

    #[test]
    fn test_dithered_palette_count_3() {
        let dithered = CHAR_GROUPS
            .iter()
            .find(|g| g.name == "dithered")
            .expect("dithered group should exist");
        let count = dithered.chars.chars().count();
        assert_eq!(
            count, 3,
            "dithered group should have exactly 3 chars, got {count}"
        );
    }

    #[test]
    fn test_dithered_palette_all_in_range() {
        let dithered = CHAR_GROUPS
            .iter()
            .find(|g| g.name == "dithered")
            .expect("dithered group should exist");
        for c in dithered.chars.chars() {
            let cp = c as u32;
            assert!(
                (0x2591..=0x2593).contains(&cp),
                "dithered char U+{cp:04X} outside U+2591–U+2593"
            );
        }
    }

    #[test]
    fn test_dithered_palette_all_unique() {
        let dithered = CHAR_GROUPS
            .iter()
            .find(|g| g.name == "dithered")
            .expect("dithered group should exist");
        let mut cps: Vec<u32> = dithered.chars.chars().map(|c| c as u32).collect();
        assert_eq!(cps.len(), 3, "should have 3 dithered chars");
        cps.sort_unstable();
        cps.dedup();
        assert_eq!(cps.len(), 3, "should have 3 unique dithered codepoints");
        assert_eq!(cps[0], 0x2591, "first should be U+2591");
        assert_eq!(cps[2], 0x2593, "last should be U+2593");
    }

    // --- Geometric palette tests ---

    #[test]
    fn test_geometric_palette_count_23() {
        let geometric = CHAR_GROUPS
            .iter()
            .find(|g| g.name == "geometric")
            .expect("geometric group should exist");
        let count = geometric.chars.chars().count();
        assert_eq!(
            count, 23,
            "geometric group should have exactly 23 chars, got {count}"
        );
    }

    #[test]
    fn test_geometric_palette_all_in_range() {
        let geometric = CHAR_GROUPS
            .iter()
            .find(|g| g.name == "geometric")
            .expect("geometric group should exist");
        for c in geometric.chars.chars() {
            let cp = c as u32;
            assert!(
                (0x25A0..=0x25FF).contains(&cp),
                "geometric char U+{cp:04X} outside U+25A0–U+25FF"
            );
        }
    }

    #[test]
    fn test_geometric_palette_all_unique() {
        let geometric = CHAR_GROUPS
            .iter()
            .find(|g| g.name == "geometric")
            .expect("geometric group should exist");
        let mut cps: Vec<u32> = geometric.chars.chars().map(|c| c as u32).collect();
        assert_eq!(cps.len(), 23, "should have 23 geometric chars");
        cps.sort_unstable();
        cps.dedup();
        assert_eq!(cps.len(), 23, "should have 23 unique geometric codepoints");
    }

    // --- Ogham palette tests ---

    #[test]
    fn test_ogham_palette_count() {
        let ogham = CHAR_GROUPS
            .iter()
            .find(|g| g.name == "ogham")
            .expect("ogham group should exist");
        let count = ogham.chars.chars().count();
        assert_eq!(
            count, 29,
            "ogham group should have exactly 29 chars, got {count}"
        );
    }

    #[test]
    fn test_ogham_palette_all_in_range() {
        let ogham = CHAR_GROUPS
            .iter()
            .find(|g| g.name == "ogham")
            .expect("ogham group should exist");
        for c in ogham.chars.chars() {
            let cp = c as u32;
            assert!(
                (0x1680..=0x169F).contains(&cp),
                "ogham char U+{cp:04X} outside U+1680–U+169F"
            );
        }
    }

    #[test]
    fn test_ogham_palette_all_unique() {
        let ogham = CHAR_GROUPS
            .iter()
            .find(|g| g.name == "ogham")
            .expect("ogham group should exist");
        let mut cps: Vec<u32> = ogham.chars.chars().map(|c| c as u32).collect();
        assert_eq!(cps.len(), 29, "should have 29 ogham chars");
        cps.sort_unstable();
        cps.dedup();
        assert_eq!(cps.len(), 29, "should have 29 unique ogham codepoints");
        assert_eq!(cps[0], 0x1680, "first codepoint should be U+1680");
        assert_eq!(cps[28], 0x169C, "last codepoint should be U+169C");
        for (i, &cp) in cps.iter().enumerate() {
            assert_eq!(
                cp,
                0x1680 + i as u32,
                "missing codepoint U+{:04X}",
                0x1680 + i as u32
            );
        }
    }

    // --- Deluxe palette tests ---

    #[test]
    fn test_deluxe_palette_count() {
        let deluxe = CHAR_GROUPS
            .iter()
            .find(|g| g.name == "deluxe")
            .expect("deluxe group should exist");
        let count = deluxe.chars.chars().count();
        assert_eq!(
            count, 566,
            "deluxe group should have exactly 566 chars, got {count}"
        );
    }

    #[test]
    fn test_deluxe_palette_contains_all_subset_chars() {
        let deluxe = CHAR_GROUPS
            .iter()
            .find(|g| g.name == "deluxe")
            .expect("deluxe group should exist");
        let deluxe_set: std::collections::HashSet<char> = deluxe.chars.chars().collect();
        for group in CHAR_GROUPS.iter().filter(|g| g.name != "deluxe") {
            for c in group.chars.chars() {
                assert!(
                    deluxe_set.contains(&c),
                    "deluxe should contain char U+{:04X} from '{}' group",
                    c as u32,
                    group.name
                );
            }
        }
    }

    #[test]
    fn test_hover_on_swatch() {
        let mut p = Palette::new();
        let area = Rect::new(0, 0, 20, 12);
        // Row 0: FG/BG toggle, Row 1: "Neutrals" header, Row 2: Neutrals data row
        // Neutrals data row col 0 -> visual index 0 (Black)
        assert!(p.handle_hover(1, 3, area));
        assert_eq!(p.hover_index, Some(0));
    }

    #[test]
    fn test_hover_outside_clears() {
        let mut p = Palette::new();
        let area = Rect::new(0, 0, 20, 12);
        assert!(p.handle_hover(1, 3, area));
        assert_eq!(p.hover_index, Some(0));
        assert!(p.handle_hover(1, 0, area));
        assert_eq!(p.hover_index, None);
    }

    #[test]
    fn test_hover_outside_palette_clears() {
        let mut p = Palette::new();
        let area = Rect::new(0, 0, 20, 12);
        p.handle_hover(1, 3, area);
        assert!(p.handle_hover(30, 30, area));
        assert_eq!(p.hover_index, None);
    }

    #[test]
    fn test_color_name_standard() {
        let p = Palette::new();
        assert_eq!(p.color_name(0), "Black");
        assert_eq!(p.color_name(9), "Bright Green");
        assert_eq!(p.color_name(15), "Bright Magenta");
    }

    #[test]
    fn test_hue_group_mapping() {
        use super::HueGroup;
        assert_eq!(super::hue_group_for_ansi(0), HueGroup::Neutrals);
        assert_eq!(super::hue_group_for_ansi(7), HueGroup::Neutrals);
        assert_eq!(super::hue_group_for_ansi(8), HueGroup::Neutrals);
        assert_eq!(super::hue_group_for_ansi(15), HueGroup::Neutrals);
        assert_eq!(super::hue_group_for_ansi(1), HueGroup::Reds);
        assert_eq!(super::hue_group_for_ansi(9), HueGroup::Reds);
        assert_eq!(super::hue_group_for_ansi(3), HueGroup::Yellows);
        assert_eq!(super::hue_group_for_ansi(11), HueGroup::Yellows);
        assert_eq!(super::hue_group_for_ansi(2), HueGroup::Greens);
        assert_eq!(super::hue_group_for_ansi(10), HueGroup::Greens);
        assert_eq!(super::hue_group_for_ansi(6), HueGroup::Cyans);
        assert_eq!(super::hue_group_for_ansi(14), HueGroup::Cyans);
        assert_eq!(super::hue_group_for_ansi(4), HueGroup::Blues);
        assert_eq!(super::hue_group_for_ansi(12), HueGroup::Blues);
        assert_eq!(super::hue_group_for_ansi(5), HueGroup::Purples);
        assert_eq!(super::hue_group_for_ansi(13), HueGroup::Purples);
    }

    #[test]
    fn test_flat_palette_contains_all_16() {
        let flat = super::build_flat_palette();
        assert_eq!(flat.len(), 16, "flat palette must have exactly 16 entries");
        let mut ansi_indices: Vec<usize> = flat.iter().map(|(i, _, _)| *i).collect();
        ansi_indices.sort_unstable();
        assert_eq!(
            ansi_indices,
            (0..16).collect::<Vec<usize>>(),
            "flat palette must contain every ANSI index 0..15 exactly once"
        );
    }

    #[test]
    fn test_flat_palette_group_ordering() {
        use super::HueGroup;
        let flat = super::build_flat_palette();
        let mut seen_groups = Vec::new();
        let mut last_group: Option<HueGroup> = None;
        for (i, _, _) in &flat {
            let g = super::hue_group_for_ansi(*i);
            if Some(g) != last_group {
                seen_groups.push(g);
                last_group = Some(g);
            }
        }
        assert_eq!(
            seen_groups,
            vec![
                HueGroup::Neutrals,
                HueGroup::Reds,
                HueGroup::Yellows,
                HueGroup::Greens,
                HueGroup::Cyans,
                HueGroup::Blues,
                HueGroup::Purples,
            ],
            "groups must appear in HueGroup::ordered() sequence"
        );
    }

    #[test]
    fn test_navigation_offset_5() {
        use crossterm::event::KeyCode;
        let mut p = Palette::new();
        assert_eq!(p.selected_index, 0);
        p.handle_key(KeyCode::Down);
        assert_eq!(p.selected_index, 5);
        p.handle_key(KeyCode::Up);
        assert_eq!(p.selected_index, 0);
        p.selected_index = 15;
        p.handle_key(KeyCode::Down);
        assert_eq!(p.selected_index, 15);
        p.selected_index = 0;
        p.handle_key(KeyCode::Up);
        assert_eq!(p.selected_index, 0);
    }

    #[test]
    fn test_color_name_extended() {
        let mut p = Palette::new();
        p.show_extended = true;
        let name = p.color_name(0);
        assert!(name.contains("Color 16"));
    }

    #[test]
    fn test_deluxe_palette_all_unique() {
        let deluxe = CHAR_GROUPS
            .iter()
            .find(|g| g.name == "deluxe")
            .expect("deluxe group should exist");
        let mut cps: Vec<u32> = deluxe.chars.chars().map(|c| c as u32).collect();
        assert_eq!(cps.len(), 566, "should have 566 deluxe chars");
        cps.sort_unstable();
        cps.dedup();
        // Dups come from dithered subset of blocks (░▒▓ = U+2591-U+2593)
        assert_eq!(
            cps.len(),
            563,
            "should have 563 unique deluxe codepoints (3 dithered are subset of blocks)"
        );
    }
}
