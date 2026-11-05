use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, StatefulWidget, Widget};
use unicode_width::UnicodeWidthStr;

use super::theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Brush,
    Move,
    Rotate,
    Marquee,
    Lasso,
    CircleSelect,
    PolygonSelect,
    Fill,
    Line,
    Eraser,
    Eyedropper,
    Spray,
    Text,
    Emitter,
    Lighting,
    Braille,
}

impl Tool {
    pub fn display_name(&self) -> &str {
        match self {
            Tool::Brush => "Br",
            Tool::Move => "Mv",
            Tool::Rotate => "Rt",
            Tool::Marquee => "Ma",
            Tool::Lasso => "La",
            Tool::CircleSelect => "Ci",
            Tool::PolygonSelect => "Po",
            Tool::Fill => "Fi",
            Tool::Line => "Li",
            Tool::Eraser => "Er",
            Tool::Eyedropper => "Ey",
            Tool::Spray => "Sp",
            Tool::Text => "Te",
            Tool::Emitter => "Em",
            Tool::Lighting => "Lg",
            Tool::Braille => "Bl",
        }
    }

    pub fn full_name(&self) -> &str {
        match self {
            Tool::Brush => "Brush",
            Tool::Move => "Move",
            Tool::Rotate => "Rotate",
            Tool::Marquee => "Select",
            Tool::Lasso => "Lasso",
            Tool::CircleSelect => "Circle",
            Tool::PolygonSelect => "Polygon",
            Tool::Fill => "Fill",
            Tool::Line => "Line",
            Tool::Eraser => "Eraser",
            Tool::Eyedropper => "Eyedrop",
            Tool::Spray => "Spray",
            Tool::Text => "Text",
            Tool::Emitter => "Emitter",
            Tool::Lighting => "Lighting",
            Tool::Braille => "Braille",
        }
    }

    pub fn key_shortcut(&self) -> KeyCode {
        match self {
            Tool::Brush => KeyCode::Char('b'),
            Tool::Move => KeyCode::Char('u'),
            Tool::Rotate => KeyCode::Char('r'),
            Tool::Marquee => KeyCode::Char('v'),
            Tool::Lasso => KeyCode::Char('l'),
            Tool::CircleSelect => KeyCode::Char('c'),
            Tool::PolygonSelect => KeyCode::Char('p'),
            Tool::Fill => KeyCode::Char('g'),
            Tool::Line => KeyCode::Char('i'),
            Tool::Eraser => KeyCode::Char('e'),
            Tool::Eyedropper => KeyCode::Char('d'),
            Tool::Spray => KeyCode::Char('a'),
            Tool::Text => KeyCode::Char('t'),
            Tool::Emitter => KeyCode::Char('m'),
            Tool::Lighting => KeyCode::Char('n'),
            Tool::Braille => KeyCode::Char('k'),
        }
    }

    pub fn icon_key(&self) -> &str {
        match self {
            Tool::Brush => "tool_brush",
            Tool::Move => "tool_move",
            Tool::Rotate => "tool_rotate",
            Tool::Marquee => "tool_marquee",
            Tool::Lasso => "tool_lasso",
            Tool::CircleSelect => "tool_circle",
            Tool::PolygonSelect => "tool_polygon",
            Tool::Fill => "tool_fill",
            Tool::Line => "tool_line",
            Tool::Eraser => "tool_eraser",
            Tool::Eyedropper => "tool_eyedropper",
            Tool::Spray => "tool_spray",
            Tool::Text => "tool_text",
            Tool::Emitter => "tool_emitter",
            Tool::Lighting => "tool_lighting",
            Tool::Braille => "tool_braille",
        }
    }

    /// Tools that paint/erase directly on Space/Enter from the keyboard
    /// (as opposed to navigation, selection, or modal tools).
    pub fn is_paint_tool(tool: Tool) -> bool {
        matches!(
            tool,
            Tool::Brush | Tool::Eraser | Tool::Line | Tool::Fill | Tool::Spray
        )
    }

    pub fn all() -> &'static [Tool] {
        &[
            Tool::Brush,
            Tool::Move,
            Tool::Rotate,
            Tool::Marquee,
            Tool::Lasso,
            Tool::CircleSelect,
            Tool::PolygonSelect,
            Tool::Fill,
            Tool::Line,
            Tool::Eraser,
            Tool::Eyedropper,
            Tool::Spray,
            Tool::Text,
            Tool::Emitter,
            Tool::Lighting,
            Tool::Braille,
        ]
    }
}

pub struct Toolbox {
    pub selected: Tool,
    pub theme: Theme,
    pub icons: std::collections::BTreeMap<String, String>,
    pub borders: Borders,
}

impl Toolbox {
    pub fn new() -> Self {
        Self {
            selected: Tool::Brush,
            theme: Theme::default(),
            icons: std::collections::BTreeMap::new(),
            borders: Borders::ALL,
        }
    }

    pub fn set_borders(&mut self, borders: Borders) {
        self.borders = borders;
    }

    pub fn handle_key(&mut self, code: KeyCode) -> bool {
        let lower = match code {
            KeyCode::Char(c) => c.to_ascii_lowercase(),
            _ => return false,
        };
        for tool in Tool::all() {
            if let KeyCode::Char(tc) = tool.key_shortcut() {
                if tc == lower {
                    self.selected = *tool;
                    return true;
                }
            }
        }
        false
    }

    pub fn next(&mut self) {
        let all = Tool::all();
        let idx = all.iter().position(|t| *t == self.selected).unwrap_or(0);
        self.selected = all[(idx + 1) % all.len()];
    }

    pub fn prev(&mut self) {
        let all = Tool::all();
        let idx = all.iter().position(|t| *t == self.selected).unwrap_or(0);
        self.selected = all[(idx + all.len() - 1) % all.len()];
    }
}

impl Widget for &Toolbox {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let tools = Tool::all();
        let items: Vec<ListItem<'_>> = tools
            .iter()
            .map(|t| {
                let icon = self
                    .icons
                    .get(t.icon_key())
                    .map(|s| s.as_str())
                    .unwrap_or(t.display_name());
                ListItem::new(format!(" {} {}", icon, t.full_name()))
            })
            .collect();

        let selected_idx = tools.iter().position(|t| *t == self.selected).unwrap_or(0);

        let list = List::new(items)
            .block(Block::default().title(" Tools ").borders(self.borders))
            .highlight_style(
                Style::default()
                    .fg(self.theme.toolbox.selected)
                    .add_modifier(Modifier::BOLD),
            );

        let mut state = ListState::default();
        state.select(Some(selected_idx));
        StatefulWidget::render(list, area, buf, &mut state);
    }
}

impl Toolbox {
    pub fn required_width(&self, brush_width: u16) -> u16 {
        let mut icon_width: usize = 0;
        for t in Tool::all() {
            let w = self
                .icons
                .get(t.icon_key())
                .map(|s| s.width())
                .unwrap_or_else(|| t.display_name().width());
            icon_width = icon_width.max(w);
        }
        let longest_name_len = Tool::all()
            .iter()
            .map(|t| t.full_name().width())
            .max()
            .unwrap_or(0);
        let tool_list_width = (icon_width + longest_name_len + 2) as u16;
        tool_list_width.max(brush_width).clamp(10, 20)
    }
}

impl Default for Toolbox {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_required_width_default() {
        let tb = Toolbox::new();
        let w = tb.required_width(15);
        assert!((10..=20).contains(&w));
    }

    #[test]
    fn test_required_width_with_icons() {
        let mut tb = Toolbox::new();
        tb.icons.insert("tool_brush".to_string(), "🖌".to_string());
        let w = tb.required_width(15);
        assert!((10..=20).contains(&w));
    }

    #[test]
    fn test_required_width_clamp_low() {
        let tb = Toolbox::new();
        // Content + padding gives at least 11, clamp(10, 20) keeps it >= 10.
        let w = tb.required_width(0);
        assert!(w >= 10);
    }

    #[test]
    fn test_required_width_clamp_high() {
        let tb = Toolbox::new();
        let w = tb.required_width(200);
        assert_eq!(w, 20);
    }

    #[test]
    fn test_tool_shortcuts_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for tool in Tool::all() {
            if let KeyCode::Char(c) = tool.key_shortcut() {
                assert!(
                    seen.insert(c),
                    "duplicate tool shortcut '{c}' (tool {:?})",
                    tool
                );
            }
        }
    }

    #[test]
    fn test_move_tool_shortcut_selects_move() {
        let mut tb = Toolbox::new();
        assert!(tb.handle_key(KeyCode::Char('u')));
        assert_eq!(tb.selected, Tool::Move);
    }

    #[test]
    fn test_rotate_tool_shortcut_selects_rotate() {
        let mut tb = Toolbox::new();
        assert!(tb.handle_key(KeyCode::Char('r')));
        assert_eq!(tb.selected, Tool::Rotate);
    }
}
