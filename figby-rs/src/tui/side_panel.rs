use std::collections::BTreeMap;

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use super::brush::BrushState;
use super::canvas::CanvasCell;
use super::layers::{LayerPanel, LayerStack};
use super::light_panel::LightPanel;
use super::lighting::Scene;
use super::particles::ParticleConfig;
use super::props_panel::{PropAction, PropsWidgetRect};
use super::theme::Theme;
use super::toolbox::Tool;
use super::tools::line::LineState;
use super::tools::move_tool::MoveState;
use super::tools::rotate_tool::RotateState;
use super::tools::selection::SelectionState;
use super::tools::text::TextToolState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabId {
    Layers,
    Props,
    Text,
    Libraries,
    Effects,
}

impl TabId {
    pub fn icon_key(&self) -> &str {
        match self {
            TabId::Layers => "layer_new",
            TabId::Props => "settings_open",
            TabId::Text => "tool_text",
            TabId::Libraries => "brush_shape_custom",
            TabId::Effects => "image_contrast",
        }
    }

    pub fn display_name(&self) -> &str {
        match self {
            TabId::Layers => "Layers",
            TabId::Props => "Props",
            TabId::Text => "Text",
            TabId::Libraries => "Libraries",
            TabId::Effects => "Effects",
        }
    }

    pub fn all() -> &'static [TabId] {
        &[
            TabId::Layers,
            TabId::Props,
            TabId::Text,
            TabId::Libraries,
            TabId::Effects,
        ]
    }

    pub fn next(self) -> Self {
        match self {
            TabId::Layers => TabId::Props,
            TabId::Props => TabId::Text,
            TabId::Text => TabId::Libraries,
            TabId::Libraries => TabId::Effects,
            TabId::Effects => TabId::Layers,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            TabId::Layers => TabId::Effects,
            TabId::Props => TabId::Layers,
            TabId::Text => TabId::Props,
            TabId::Libraries => TabId::Text,
            TabId::Effects => TabId::Libraries,
        }
    }
}

pub struct SidePanel {
    pub open: bool,
    pub active_tab: TabId,
    pub icons: BTreeMap<String, String>,
    pub theme: Theme,
}

impl SidePanel {
    pub fn new(icons: BTreeMap<String, String>, theme: Theme) -> Self {
        Self {
            open: false,
            active_tab: TabId::Layers,
            icons,
            theme,
        }
    }

    pub fn toggle_open(&mut self) {
        self.open = !self.open;
    }

    pub fn set_active_tab(&mut self, id: TabId) {
        self.active_tab = id;
    }

    pub fn cycle_tab(&mut self, forward: bool) {
        self.active_tab = if forward {
            self.active_tab.next()
        } else {
            self.active_tab.prev()
        };
    }

    pub fn tab_rects(&self, area: Rect) -> Vec<Rect> {
        let tab_count = TabId::all().len() as u16;
        if tab_count == 0 || area.width < 2 {
            return Vec::new();
        }
        let tab_w = (area.width - 2).max(tab_count) / tab_count;
        (0..tab_count)
            .map(|i| Rect {
                x: area.x + 1 + i * tab_w,
                y: area.y + 1,
                width: tab_w,
                height: 1,
            })
            .collect()
    }

    pub fn tab_at_pos(&self, col: u16, row: u16, area: Rect) -> Option<TabId> {
        let rects = self.tab_rects(area);
        for (i, r) in rects.iter().enumerate() {
            if col >= r.x && col < r.x + r.width && row == r.y {
                return TabId::all().get(i).copied();
            }
        }
        None
    }

    /// Compute the content rect below the tab header + separator, matching
    /// the layout `render` actually draws into. Callers that need to
    /// translate mouse coordinates into a tab's content area (e.g. the
    /// layers panel's click handling) must use this instead of the raw
    /// panel `area`, since `render` draws its own border plus a two-row
    /// tab bar before handing off to the per-tab content.
    pub fn content_area(&self, area: Rect) -> Rect {
        let block = Block::default().borders(Borders::ALL);
        let inner = block.inner(area);
        let content_y = inner.y + 2;
        if content_y >= area.y + area.height {
            return Rect {
                x: inner.x,
                y: content_y,
                width: inner.width,
                height: 0,
            };
        }
        Rect {
            x: inner.x,
            y: content_y,
            width: inner.width,
            height: (area.y + area.height).saturating_sub(content_y),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &self,
        frame: &mut Frame<'_>,
        area: Rect,
        layer_panel: Option<&mut LayerPanel>,
        layer_stack: Option<&LayerStack>,
        active_tool: Tool,
        brush: &BrushState,
        text_tool: Option<&TextToolState>,
        eyedropper_sample: Option<CanvasCell>,
        fill_threshold: u8,
        emitter_config: Option<&ParticleConfig>,
        canvas_width: u16,
        canvas_height: u16,
        font_name: Option<&str>,
        zoom: u8,
        lighting_scene: Option<&Scene>,
        lighting_panel: Option<&LightPanel>,
        props_rects: &mut Vec<PropsWidgetRect>,
        move_state: &MoveState,
        rotate_state: &RotateState,
        selection_state: &SelectionState,
        line_state: &LineState,
    ) {
        let block = Block::default().borders(Borders::ALL).style(
            Style::default()
                .bg(self.theme.menu.dropdown_bg)
                .fg(self.theme.menu.fg),
        );
        let inner = block.inner(area);
        frame.render_widget(block, area);

        if inner.width < 4 || inner.height < 3 {
            return;
        }

        // Tab header row
        let tabs = TabId::all();
        let tab_count = tabs.len() as u16;
        let total_tab_w = inner.width.saturating_sub(2);
        let tab_w = total_tab_w / tab_count;

        let mut tab_x = inner.x;
        for tab in tabs {
            let icon = self
                .icons
                .get(tab.icon_key())
                .map(|s| s.as_str())
                .unwrap_or("");
            let label = if tab_w >= 6 {
                format!("{} {}", icon, tab.display_name())
            } else if tab_w >= 2 {
                icon.to_string()
            } else {
                String::new()
            };
            let is_active = *tab == self.active_tab;
            let style = if is_active {
                Style::default()
                    .fg(self.theme.general.primary)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(self.theme.general.secondary)
            };
            let tab_rect = Rect {
                x: tab_x,
                y: inner.y,
                width: tab_w.min(inner.x + inner.width - tab_x),
                height: 1,
            };
            if tab_rect.width >= label.len() as u16 {
                frame.render_widget(Paragraph::new(label).style(style), tab_rect);
            }
            tab_x += tab_w;
        }

        // Separator line below tabs
        let sep_style = Style::default().fg(self.theme.general.secondary);
        let sep_rect = Rect {
            x: inner.x,
            y: inner.y + 1,
            width: inner.width,
            height: 1,
        };
        if inner.width > 0 && inner.y + 1 < area.y + area.height {
            let hint = " ←/→ tabs ";
            let sep_label = if inner.width as usize >= hint.len() + 4 {
                format!("{:─^width$}", hint, width = inner.width as usize)
            } else {
                "─".repeat(inner.width as usize)
            };
            frame.render_widget(
                Paragraph::new(sep_label).style(sep_style.add_modifier(Modifier::DIM)),
                sep_rect,
            );
        }

        // Content area
        let content_area = self.content_area(area);
        if content_area.height == 0 {
            return;
        }

        match self.active_tab {
            TabId::Layers => {
                if let (Some(panel), Some(stack)) = (layer_panel, layer_stack) {
                    panel.render_with_stack(frame, content_area, stack);
                }
            }
            TabId::Props => {
                Self::render_tool_props(
                    frame,
                    content_area,
                    &self.theme,
                    active_tool,
                    brush,
                    text_tool,
                    eyedropper_sample,
                    fill_threshold,
                    emitter_config,
                    canvas_width,
                    canvas_height,
                    font_name,
                    zoom,
                    lighting_scene,
                    lighting_panel,
                    props_rects,
                    move_state,
                    rotate_state,
                    selection_state,
                    line_state,
                );
            }
            TabId::Text => {
                if let Some(tt) = text_tool {
                    Self::render_text_content(frame, content_area, tt, &self.theme, props_rects);
                }
            }
            TabId::Libraries => {
                Self::render_placeholder(frame, content_area, "Libraries", &self.theme);
            }
            TabId::Effects => {
                Self::render_placeholder(frame, content_area, "Effects", &self.theme);
            }
        }
    }

    /// Dispatch tool property content based on active tool.
    /// Image/font info always rendered at bottom.
    #[allow(clippy::too_many_arguments)]
    fn render_tool_props(
        frame: &mut Frame<'_>,
        area: Rect,
        theme: &Theme,
        active_tool: Tool,
        brush: &BrushState,
        text_tool: Option<&TextToolState>,
        eyedropper_sample: Option<CanvasCell>,
        fill_threshold: u8,
        emitter_config: Option<&ParticleConfig>,
        canvas_width: u16,
        canvas_height: u16,
        font_name: Option<&str>,
        zoom: u8,
        lighting_scene: Option<&Scene>,
        lighting_panel: Option<&LightPanel>,
        rects: &mut Vec<PropsWidgetRect>,
        move_state: &MoveState,
        rotate_state: &RotateState,
        selection_state: &SelectionState,
        line_state: &LineState,
    ) {
        let mut lines: Vec<Line> = Vec::new();
        let mut line_y: u16 = 0;

        match active_tool {
            Tool::Brush | Tool::Spray | Tool::Eraser => {
                Self::add_brush_props(&mut lines, brush, rects, area, &mut line_y);
            }
            Tool::Text => {
                if let Some(tt) = text_tool {
                    Self::add_text_props(&mut lines, tt, rects, area, &mut line_y);
                }
            }
            Tool::Eyedropper => {
                Self::add_eyedropper_props(&mut lines, eyedropper_sample, rects, area, &mut line_y);
            }
            Tool::Fill => {
                Self::add_fill_props(&mut lines, fill_threshold, rects, area, &mut line_y);
            }
            Tool::Emitter => {
                Self::add_emitter_props(&mut lines, emitter_config, rects, area, &mut line_y);
            }
            Tool::Lighting => {
                Self::add_lighting_props(
                    &mut lines,
                    lighting_scene,
                    lighting_panel,
                    theme,
                    rects,
                    area,
                    &mut line_y,
                );
            }
            Tool::Move => {
                Self::add_move_props(&mut lines, move_state, rects, area, &mut line_y);
            }
            Tool::Rotate => {
                Self::add_rotate_props(&mut lines, rotate_state, rects, area, &mut line_y);
            }
            Tool::Marquee | Tool::Lasso | Tool::CircleSelect | Tool::PolygonSelect => {
                Self::add_select_props(&mut lines, selection_state, rects, area, &mut line_y);
            }
            Tool::Line => {
                Self::add_line_props(&mut lines, line_state, rects, area, &mut line_y);
            }
            Tool::Braille => {
                Self::add_braille_props(&mut lines, brush, rects, area, &mut line_y);
            }
        }

        // Separator + image/font info at bottom
        lines.push(Line::from(""));
        Self::add_image_font_info(&mut lines, canvas_width, canvas_height, font_name, zoom);

        let para = Paragraph::new(lines).style(Style::default().fg(theme.menu.fg));
        frame.render_widget(para, area);
    }

    fn add_brush_props(
        lines: &mut Vec<Line>,
        brush: &BrushState,
        rects: &mut Vec<PropsWidgetRect>,
        area: Rect,
        line_y: &mut u16,
    ) {
        lines.push(Line::from(Span::styled(
            " Brush ",
            Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )));
        *line_y += 1;

        // Size: [-].Size: 3.[+]
        {
            let y = area.y + *line_y;
            let x = area.x;
            // [-] button at col 2
            rects.push(PropsWidgetRect {
                rect: Rect {
                    x: x + 2,
                    y,
                    width: 3,
                    height: 1,
                },
                action: PropAction::SizeDown,
            });
            // [+] button at col 15
            rects.push(PropsWidgetRect {
                rect: Rect {
                    x: x + 15,
                    y,
                    width: 3,
                    height: 1,
                },
                action: PropAction::SizeUp,
            });
            lines.push(Line::from(vec![
                Span::raw("[-] "),
                Span::styled("Size:", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!(" {} [+] ", brush.size)),
            ]));
            *line_y += 1;
        }

        // Shape: click on value to cycle
        {
            let y = area.y + *line_y;
            let x = area.x;
            let val_start = x + 7; // after "Shape: "
            let val_w = brush.shape.name().len() as u16;
            rects.push(PropsWidgetRect {
                rect: Rect {
                    x: val_start,
                    y,
                    width: val_w,
                    height: 1,
                },
                action: PropAction::CycleShape,
            });
            lines.push(Line::from(vec![
                Span::styled("Shape:", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!(" {}", brush.shape.name())),
            ]));
            *line_y += 1;
        }

        // Mode: click on value to cycle
        {
            let y = area.y + *line_y;
            let x = area.x;
            let val_start = x + 6; // after "Mode: "
            let val_w = brush.sub_mode.name().len() as u16;
            rects.push(PropsWidgetRect {
                rect: Rect {
                    x: val_start,
                    y,
                    width: val_w,
                    height: 1,
                },
                action: PropAction::CycleSubMode,
            });
            lines.push(Line::from(vec![
                Span::styled("Mode:", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!(" {}", brush.sub_mode.name())),
            ]));
            *line_y += 1;
        }

        // Density: [-].Density: 35%.[+]
        {
            let y = area.y + *line_y;
            let x = area.x;
            rects.push(PropsWidgetRect {
                rect: Rect {
                    x: x + 2,
                    y,
                    width: 3,
                    height: 1,
                },
                action: PropAction::DensityDown,
            });
            let density_str = format!("{}%", brush.density);
            let density_len = density_str.len() as u16;
            rects.push(PropsWidgetRect {
                rect: Rect {
                    x: x + 2 + 3 + 9 + 1 + density_len + 1,
                    y,
                    width: 3,
                    height: 1,
                },
                action: PropAction::DensityUp,
            });
            lines.push(Line::from(vec![
                Span::raw("[-] "),
                Span::styled("Density:", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!(" {} [+] ", density_str)),
            ]));
            *line_y += 1;
        }

        // Char: click to enter edit mode
        {
            let y = area.y + *line_y;
            let x = area.x;
            let val_start = x + 6; // after "Char: "
            let val_w = 3; // e.g. '█'
            rects.push(PropsWidgetRect {
                rect: Rect {
                    x: val_start,
                    y,
                    width: val_w,
                    height: 1,
                },
                action: PropAction::BeginEditChar,
            });
            lines.push(Line::from(vec![
                Span::styled("Char:", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!(" '{}'", brush.ch)),
            ]));
            *line_y += 1;
        }
    }

    fn add_text_props(
        lines: &mut Vec<Line>,
        tt: &TextToolState,
        rects: &mut Vec<PropsWidgetRect>,
        area: Rect,
        line_y: &mut u16,
    ) {
        use crate::tui::tools::text::TextMode;

        lines.push(Line::from(Span::styled(
            " Text Tool ",
            Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )));
        *line_y += 1;

        // Text buffer display
        {
            let display = if tt.text_buffer.is_empty() {
                "(empty — type below)".to_string()
            } else {
                tt.text_buffer.clone()
            };
            lines.push(Line::from(vec![
                Span::styled("Text:", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!(" {}", display)),
            ]));
            *line_y += 1;
        }

        // Mode: Figlet | Plain (click cycles)
        {
            let y = area.y + *line_y;
            let x = area.x;
            let val_start = x + 6;
            let val_w = tt.mode.name().len() as u16;
            rects.push(PropsWidgetRect {
                rect: Rect {
                    x: val_start,
                    y,
                    width: val_w,
                    height: 1,
                },
                action: PropAction::CycleSubMode,
            });
            lines.push(Line::from(vec![
                Span::styled("Mode:", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!(" {}", tt.mode.name())),
            ]));
            *line_y += 1;
        }

        if tt.mode == TextMode::Figlet {
            let font_name = if tt.font_index < tt.available_fonts.len() {
                &tt.available_fonts[tt.font_index]
            } else {
                "?"
            };

            // Font: [<] name [>]
            {
                let y = area.y + *line_y;
                let x = area.x;
                rects.push(PropsWidgetRect {
                    rect: Rect {
                        x: x + 6,
                        y,
                        width: 3,
                        height: 1,
                    },
                    action: PropAction::FontPrev,
                });
                let name_start = x + 6 + 3 + 1 + font_name.len() as u16 + 1;
                rects.push(PropsWidgetRect {
                    rect: Rect {
                        x: name_start,
                        y,
                        width: 3,
                        height: 1,
                    },
                    action: PropAction::FontNext,
                });
                lines.push(Line::from(vec![
                    Span::styled("Font:", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(format!(" [<] {} [>]", font_name)),
                ]));
                *line_y += 1;
            }
        } else {
            // Plain mode: FG / BG from palette selection
            let fg_label = match tt.text_color {
                Some(c) => format!("{:?}", c),
                None => "—".to_string(),
            };
            let bg_label = match tt.bg_color {
                Some(c) => format!("{:?}", c),
                None => "—".to_string(),
            };
            lines.push(Line::from(vec![
                Span::styled("FG:", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!(" {}", fg_label)),
            ]));
            *line_y += 1;
            {
                let y = area.y + *line_y;
                let x = area.x;
                rects.push(PropsWidgetRect {
                    rect: Rect {
                        x: x + 4,
                        y,
                        width: bg_label.len().max(1) as u16,
                        height: 1,
                    },
                    action: PropAction::TextBgToggle,
                });
                lines.push(Line::from(vec![
                    Span::styled("BG:", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(format!(" {} ", bg_label)),
                    Span::raw("(click clear)"),
                ]));
                *line_y += 1;
            }
        }

        // Just: click to cycle
        {
            let just_str = match tt.justification {
                crate::render::Justification::Left => "Left",
                crate::render::Justification::Center => "Center",
                crate::render::Justification::Right => "Right",
            };
            let y = area.y + *line_y;
            let x = area.x;
            let val_start = x + 6;
            let val_w = just_str.len() as u16;
            rects.push(PropsWidgetRect {
                rect: Rect {
                    x: val_start,
                    y,
                    width: val_w,
                    height: 1,
                },
                action: PropAction::CycleJust,
            });
            lines.push(Line::from(vec![
                Span::styled("Just:", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!(" {}", just_str)),
            ]));
            *line_y += 1;
        }

        // Scale: [-].Scale: 3.[+]
        {
            let y = area.y + *line_y;
            let x = area.x;
            rects.push(PropsWidgetRect {
                rect: Rect {
                    x: x + 2,
                    y,
                    width: 3,
                    height: 1,
                },
                action: PropAction::ScaleDown,
            });
            rects.push(PropsWidgetRect {
                rect: Rect {
                    x: x + 15,
                    y,
                    width: 3,
                    height: 1,
                },
                action: PropAction::ScaleUp,
            });
            lines.push(Line::from(vec![
                Span::raw("[-] "),
                Span::styled("Scale:", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!(" {} [+] ", tt.scale)),
            ]));
            *line_y += 1;
        }

        // Rasterize button if block selected. Push the blank line first,
        // then compute y from the updated line_y so the hit-rect lands on
        // the "[Rasterize]" row, not the blank line above it.
        if tt.selected_block.is_some() {
            lines.push(Line::from(""));
            *line_y += 1;
            let y = area.y + *line_y;
            let x = area.x;
            rects.push(PropsWidgetRect {
                rect: Rect {
                    x: x + 2,
                    y,
                    width: 11,
                    height: 1,
                },
                action: PropAction::RasterizeBlock,
            });
            lines.push(Line::from(Span::raw(" [Rasterize] ")));
            *line_y += 1;
        }

        lines.push(Line::from(""));
        *line_y += 1;
        lines.push(Line::from(Span::raw(" Hover canvas to preview")));
        *line_y += 1;
        lines.push(Line::from(Span::raw(" Click to place text")));
        if tt.mode == TextMode::Plain {
            *line_y += 1;
            lines.push(Line::from(Span::raw(" M: toggle Figlet/Plain")));
        }
    }

    fn add_braille_props(
        lines: &mut Vec<Line>,
        brush: &BrushState,
        _rects: &mut Vec<PropsWidgetRect>,
        _area: Rect,
        line_y: &mut u16,
    ) {
        lines.push(Line::from(Span::styled(
            " Braille ",
            Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )));
        *line_y += 1;
        lines.push(Line::from(vec![
            Span::styled("Dot:", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format!(" ({}, {}) of 2x4", brush.sub_x, brush.sub_y)),
        ]));
        *line_y += 1;
        lines.push(Line::from(""));
        *line_y += 1;
        lines.push(Line::from(Span::raw(" Arrows: move dot cursor")));
        *line_y += 1;
        lines.push(Line::from(Span::raw(" Space/Enter: toggle dot")));
        *line_y += 1;
    }

    fn add_eyedropper_props(
        lines: &mut Vec<Line>,
        sample: Option<CanvasCell>,
        _rects: &mut Vec<PropsWidgetRect>,
        _area: Rect,
        line_y: &mut u16,
    ) {
        lines.push(Line::from(Span::styled(
            " Eyedropper ",
            Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )));
        *line_y += 1;
        match sample {
            Some(cell) => {
                lines.push(Line::from(vec![
                    Span::styled("Char:", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(format!(" '{}'", cell.ch)),
                ]));
                *line_y += 1;
                lines.push(Line::from(vec![
                    Span::styled("FG:", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(format!(" {:?}", cell.fg)),
                ]));
                *line_y += 1;
                lines.push(Line::from(vec![
                    Span::styled("BG:", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(format!(" {:?}", cell.bg)),
                ]));
                *line_y += 1;
            }
            None => {
                lines.push(Line::from(Span::raw(" No sample yet")));
                *line_y += 1;
                lines.push(Line::from(Span::raw(" Click canvas to sample")));
                *line_y += 1;
            }
        }
    }

    fn add_fill_props(
        lines: &mut Vec<Line>,
        threshold: u8,
        rects: &mut Vec<PropsWidgetRect>,
        area: Rect,
        line_y: &mut u16,
    ) {
        lines.push(Line::from(Span::styled(
            " Fill ",
            Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )));
        *line_y += 1;

        // Threshold: [-].Threshold: 0.[+]
        {
            let y = area.y + *line_y;
            let x = area.x;
            rects.push(PropsWidgetRect {
                rect: Rect {
                    x: x + 2,
                    y,
                    width: 3,
                    height: 1,
                },
                action: PropAction::FillThresholdDown,
            });
            let thresh_str = format!("{}", threshold);
            let thresh_len = thresh_str.len() as u16;
            rects.push(PropsWidgetRect {
                rect: Rect {
                    x: x + 2 + 3 + 11 + 1 + thresh_len + 1,
                    y,
                    width: 3,
                    height: 1,
                },
                action: PropAction::FillThresholdUp,
            });
            lines.push(Line::from(vec![
                Span::raw("[-] "),
                Span::styled("Threshold:", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!(" {} [+] ", threshold)),
            ]));
            *line_y += 1;
        }

        lines.push(Line::from(""));
        *line_y += 1;
        lines.push(Line::from(Span::raw(" Click canvas to flood fill")));
    }

    fn add_emitter_props(
        lines: &mut Vec<Line>,
        config: Option<&ParticleConfig>,
        _rects: &mut Vec<PropsWidgetRect>,
        _area: Rect,
        line_y: &mut u16,
    ) {
        lines.push(Line::from(Span::styled(
            " Emitter ",
            Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )));
        *line_y += 1;
        if let Some(cfg) = config {
            lines.push(Line::from(vec![
                Span::styled("Rate:", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!(" {:.1}/s", cfg.spawn_rate)),
            ]));
            *line_y += 1;
            lines.push(Line::from(vec![
                Span::styled("Lifetime:", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!(" {:.1}–{:.1}s", cfg.lifetime_min, cfg.lifetime_max)),
            ]));
            *line_y += 1;
            lines.push(Line::from(vec![
                Span::styled("Shape:", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!(" {}", cfg.emission_shape.display_name())),
            ]));
            *line_y += 1;
            lines.push(Line::from(vec![
                Span::styled("Char:", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!(" '{}'", cfg.character)),
            ]));
            *line_y += 1;
            lines.push(Line::from(vec![
                Span::styled("Size:", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!(" {}", cfg.size)),
            ]));
            *line_y += 1;
        } else {
            lines.push(Line::from(Span::raw(" No config")));
            *line_y += 1;
        }
    }

    fn add_lighting_props(
        lines: &mut Vec<Line>,
        scene: Option<&Scene>,
        panel: Option<&LightPanel>,
        theme: &Theme,
        _rects: &mut Vec<PropsWidgetRect>,
        _area: Rect,
        line_y: &mut u16,
    ) {
        lines.push(Line::from(Span::styled(
            " Lighting ",
            Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )));
        *line_y += 1;
        match (scene, panel) {
            (Some(scene), Some(panel)) => {
                let bl = LightPanel::build_lines(scene, panel.selected_index(), theme);
                let count = bl.len() as u16;
                lines.extend(bl);
                *line_y += count;
            }
            _ => {
                lines.push(Line::from(Span::raw(" No scene yet")));
                *line_y += 1;
            }
        }
        lines.push(Line::from(""));
        *line_y += 1;
        lines.push(Line::from(Span::raw(" A/D/P  Add Amb/Dir/Point")));
        *line_y += 1;
        lines.push(Line::from(Span::raw(" ↑/↓  Select light")));
        *line_y += 1;
        lines.push(Line::from(Span::raw(" ←/→  Move (Point)")));
        *line_y += 1;
        lines.push(Line::from(Span::raw(" -/+  Intensity")));
        *line_y += 1;
        lines.push(Line::from(Span::raw(" Del  Remove light")));
        *line_y += 1;
    }

    fn add_move_props(
        lines: &mut Vec<Line>,
        state: &MoveState,
        rects: &mut Vec<PropsWidgetRect>,
        area: Rect,
        line_y: &mut u16,
    ) {
        lines.push(Line::from(Span::styled(
            " Move ",
            Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )));
        *line_y += 1;

        // Stride: [-].Stride: 1.[+]
        {
            let y = area.y + *line_y;
            let x = area.x;
            rects.push(PropsWidgetRect {
                rect: Rect {
                    x: x + 2,
                    y,
                    width: 3,
                    height: 1,
                },
                action: PropAction::MoveStrideDown,
            });
            let stride_str = format!("{}", state.stride);
            let stride_len = stride_str.len() as u16;
            rects.push(PropsWidgetRect {
                rect: Rect {
                    x: x + 2 + 3 + 8 + 1 + stride_len + 1,
                    y,
                    width: 3,
                    height: 1,
                },
                action: PropAction::MoveStrideUp,
            });
            lines.push(Line::from(vec![
                Span::raw("[-] "),
                Span::styled("Stride:", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!(" {} [+] ", stride_str)),
            ]));
            *line_y += 1;
        }

        // Snap: click on value to toggle
        {
            let y = area.y + *line_y;
            let x = area.x;
            let snap_str = if state.snap { "On" } else { "Off" };
            let val_start = x + 6;
            let val_w = snap_str.len() as u16;
            rects.push(PropsWidgetRect {
                rect: Rect {
                    x: val_start,
                    y,
                    width: val_w,
                    height: 1,
                },
                action: PropAction::MoveSnapToggle,
            });
            lines.push(Line::from(vec![
                Span::styled("Snap:", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!(" {}", snap_str)),
            ]));
            *line_y += 1;
        }

        // Wrap: click on value to toggle
        {
            let y = area.y + *line_y;
            let x = area.x;
            let wrap_str = if state.wrap { "On" } else { "Off" };
            let val_start = x + 6;
            let val_w = wrap_str.len() as u16;
            rects.push(PropsWidgetRect {
                rect: Rect {
                    x: val_start,
                    y,
                    width: val_w,
                    height: 1,
                },
                action: PropAction::MoveWrapToggle,
            });
            lines.push(Line::from(vec![
                Span::styled("Wrap:", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!(" {}", wrap_str)),
            ]));
            *line_y += 1;
        }
    }

    fn add_rotate_props(
        lines: &mut Vec<Line>,
        state: &RotateState,
        rects: &mut Vec<PropsWidgetRect>,
        area: Rect,
        line_y: &mut u16,
    ) {
        lines.push(Line::from(Span::styled(
            " Rotate ",
            Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )));
        *line_y += 1;

        // Step angle: [-].Angle: 90.[+]
        {
            let y = area.y + *line_y;
            let x = area.x;
            rects.push(PropsWidgetRect {
                rect: Rect {
                    x: x + 2,
                    y,
                    width: 3,
                    height: 1,
                },
                action: PropAction::RotateStepDown,
            });
            let angle_str = format!("{}", state.step_angle);
            let angle_len = angle_str.len() as u16;
            rects.push(PropsWidgetRect {
                rect: Rect {
                    x: x + 2 + 3 + 7 + 1 + angle_len + 1,
                    y,
                    width: 3,
                    height: 1,
                },
                action: PropAction::RotateStepUp,
            });
            lines.push(Line::from(vec![
                Span::raw("[-] "),
                Span::styled("Angle:", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!(" {}° [+] ", angle_str)),
            ]));
            *line_y += 1;
        }

        // Direction: click to toggle
        {
            let y = area.y + *line_y;
            let x = area.x;
            let dir_str = state.direction.display_name();
            let val_start = x + 11;
            let val_w = dir_str.len() as u16;
            rects.push(PropsWidgetRect {
                rect: Rect {
                    x: val_start,
                    y,
                    width: val_w,
                    height: 1,
                },
                action: PropAction::RotateDirToggle,
            });
            lines.push(Line::from(vec![
                Span::styled("Direction:", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!(" {}", dir_str)),
            ]));
            *line_y += 1;
        }

        // Pivot: click to cycle
        {
            let y = area.y + *line_y;
            let x = area.x;
            let pivot_str = state.pivot.display_name();
            let val_start = x + 7;
            let val_w = pivot_str.len() as u16;
            rects.push(PropsWidgetRect {
                rect: Rect {
                    x: val_start,
                    y,
                    width: val_w,
                    height: 1,
                },
                action: PropAction::RotatePivotCycle,
            });
            lines.push(Line::from(vec![
                Span::styled("Pivot:", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!(" {}", pivot_str)),
            ]));
            *line_y += 1;
        }
    }

    fn add_select_props(
        lines: &mut Vec<Line>,
        state: &SelectionState,
        rects: &mut Vec<PropsWidgetRect>,
        area: Rect,
        line_y: &mut u16,
    ) {
        lines.push(Line::from(Span::styled(
            " Selection ",
            Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )));
        *line_y += 1;

        // Feather: [-].Feather: 0.[+]
        {
            let y = area.y + *line_y;
            let x = area.x;
            rects.push(PropsWidgetRect {
                rect: Rect {
                    x: x + 2,
                    y,
                    width: 3,
                    height: 1,
                },
                action: PropAction::SelectFeatherDown,
            });
            let feather_str = format!("{}", state.feather);
            let feather_len = feather_str.len() as u16;
            rects.push(PropsWidgetRect {
                rect: Rect {
                    x: x + 2 + 3 + 9 + 1 + feather_len + 1,
                    y,
                    width: 3,
                    height: 1,
                },
                action: PropAction::SelectFeatherUp,
            });
            lines.push(Line::from(vec![
                Span::raw("[-] "),
                Span::styled("Feather:", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!(" {} [+] ", feather_str)),
            ]));
            *line_y += 1;
        }

        // Additive: click to toggle
        {
            let y = area.y + *line_y;
            let x = area.x;
            let add_str = if state.additive { "On" } else { "Off" };
            let val_start = x + 10;
            let val_w = add_str.len() as u16;
            rects.push(PropsWidgetRect {
                rect: Rect {
                    x: val_start,
                    y,
                    width: val_w,
                    height: 1,
                },
                action: PropAction::SelectAdditiveToggle,
            });
            lines.push(Line::from(vec![
                Span::styled("Additive:", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!(" {}", add_str)),
            ]));
            *line_y += 1;
        }

        // Subtractive: click to toggle
        {
            let y = area.y + *line_y;
            let x = area.x;
            let sub_str = if state.subtractive { "On" } else { "Off" };
            let val_start = x + 12;
            let val_w = sub_str.len() as u16;
            rects.push(PropsWidgetRect {
                rect: Rect {
                    x: val_start,
                    y,
                    width: val_w,
                    height: 1,
                },
                action: PropAction::SelectSubtractiveToggle,
            });
            lines.push(Line::from(vec![
                Span::styled(
                    "Subtractive:",
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!(" {}", sub_str)),
            ]));
            *line_y += 1;
        }

        // Move with arrows: click to toggle
        {
            let y = area.y + *line_y;
            let x = area.x;
            let mov_str = if state.move_with_arrows { "On" } else { "Off" };
            let val_start = x + 17;
            let val_w = mov_str.len() as u16;
            rects.push(PropsWidgetRect {
                rect: Rect {
                    x: val_start,
                    y,
                    width: val_w,
                    height: 1,
                },
                action: PropAction::SelectMoveToggle,
            });
            lines.push(Line::from(vec![
                Span::styled(
                    "Move w/ Arrows:",
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!(" {}", mov_str)),
            ]));
            *line_y += 1;
        }
    }

    fn add_line_props(
        lines: &mut Vec<Line>,
        state: &LineState,
        rects: &mut Vec<PropsWidgetRect>,
        area: Rect,
        line_y: &mut u16,
    ) {
        lines.push(Line::from(Span::styled(
            " Line ",
            Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )));
        *line_y += 1;

        // Width: [-].Width: 1.[+]
        {
            let y = area.y + *line_y;
            let x = area.x;
            rects.push(PropsWidgetRect {
                rect: Rect {
                    x: x + 2,
                    y,
                    width: 3,
                    height: 1,
                },
                action: PropAction::LineWidthDown,
            });
            let width_str = format!("{}", state.width);
            let width_len = width_str.len() as u16;
            rects.push(PropsWidgetRect {
                rect: Rect {
                    x: x + 2 + 3 + 7 + 1 + width_len + 1,
                    y,
                    width: 3,
                    height: 1,
                },
                action: PropAction::LineWidthUp,
            });
            lines.push(Line::from(vec![
                Span::raw("[-] "),
                Span::styled("Width:", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!(" {} [+] ", width_str)),
            ]));
            *line_y += 1;
        }

        // Arrowhead: click to cycle
        {
            let y = area.y + *line_y;
            let x = area.x;
            let arrow_str = state.arrowhead.display_name();
            let val_start = x + 11;
            let val_w = arrow_str.len() as u16;
            rects.push(PropsWidgetRect {
                rect: Rect {
                    x: val_start,
                    y,
                    width: val_w,
                    height: 1,
                },
                action: PropAction::LineArrowCycle,
            });
            lines.push(Line::from(vec![
                Span::styled("Arrowhead:", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!(" {}", arrow_str)),
            ]));
            *line_y += 1;
        }

        // Curve: click to toggle
        {
            let y = area.y + *line_y;
            let x = area.x;
            let curve_str = state.curve.display_name();
            let val_start = x + 7;
            let val_w = curve_str.len() as u16;
            rects.push(PropsWidgetRect {
                rect: Rect {
                    x: val_start,
                    y,
                    width: val_w,
                    height: 1,
                },
                action: PropAction::LineCurveToggle,
            });
            lines.push(Line::from(vec![
                Span::styled("Curve:", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!(" {}", curve_str)),
            ]));
            *line_y += 1;
        }
    }

    fn add_image_font_info(
        lines: &mut Vec<Line>,
        canvas_width: u16,
        canvas_height: u16,
        font_name: Option<&str>,
        zoom: u8,
    ) {
        lines.push(Line::from(Span::styled(
            " Image / Font Info ",
            Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )));
        lines.push(Line::from(vec![
            Span::styled("Size:", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format!(" {}×{}", canvas_width, canvas_height)),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Zoom:", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format!(" {}×", zoom)),
        ]));
        if let Some(name) = font_name {
            lines.push(Line::from(vec![
                Span::styled("Font:", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!(" {}", name)),
            ]));
        }
    }

    fn render_text_content(
        frame: &mut Frame<'_>,
        area: Rect,
        tt: &TextToolState,
        _theme: &Theme,
        rects: &mut Vec<PropsWidgetRect>,
    ) {
        use crate::tui::tools::text::TextMode;

        let font_name = if tt.font_index < tt.available_fonts.len() {
            &tt.available_fonts[tt.font_index]
        } else {
            "?"
        };
        let just_str = match tt.justification {
            crate::render::Justification::Left => "Left",
            crate::render::Justification::Center => "Center",
            crate::render::Justification::Right => "Right",
        };

        let mut lines: Vec<Line> = Vec::new();
        let mut line_y: u16 = 0;

        // Text buffer
        {
            let display = if tt.text_buffer.is_empty() {
                "(empty)".to_string()
            } else {
                tt.text_buffer.clone()
            };
            lines.push(Line::from(vec![
                Span::styled("Text:", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!(" {}", display)),
            ]));
            line_y += 1;
        }
        lines.push(Line::from(Span::raw(if tt.editing {
            " [type in sidebar...]"
        } else {
            " [click text tab to edit]"
        })));
        line_y += 1;

        // Mode row
        {
            let y = area.y + line_y;
            let x = area.x;
            rects.push(PropsWidgetRect {
                rect: Rect {
                    x: x + 6,
                    y,
                    width: tt.mode.name().len() as u16,
                    height: 1,
                },
                action: PropAction::CycleSubMode,
            });
            lines.push(Line::from(vec![
                Span::styled("Mode:", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!(" {}", tt.mode.name())),
            ]));
            line_y += 1;
        }

        if tt.mode == TextMode::Figlet {
            // Font: [<] name [>]
            {
                let y = area.y + line_y;
                let x = area.x;
                rects.push(PropsWidgetRect {
                    rect: Rect {
                        x: x + 6,
                        y,
                        width: 3,
                        height: 1,
                    },
                    action: PropAction::FontPrev,
                });
                let name_end = x + 6 + 3 + 1 + font_name.len() as u16 + 1;
                rects.push(PropsWidgetRect {
                    rect: Rect {
                        x: name_end,
                        y,
                        width: 3,
                        height: 1,
                    },
                    action: PropAction::FontNext,
                });
                lines.push(Line::from(vec![
                    Span::styled("Font:", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(format!(" [<] {} [>]", font_name)),
                ]));
                line_y += 1;
            }
        } else {
            let fg_label = match tt.text_color {
                Some(c) => format!("{:?}", c),
                None => "—".to_string(),
            };
            let bg_label = match tt.bg_color {
                Some(c) => format!("{:?}", c),
                None => "—".to_string(),
            };
            lines.push(Line::from(vec![
                Span::styled("FG:", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!(" {}", fg_label)),
            ]));
            line_y += 1;
            lines.push(Line::from(vec![
                Span::styled("BG:", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!(" {}", bg_label)),
            ]));
            line_y += 1;
        }

        // Just: click to cycle
        {
            let y = area.y + line_y;
            let x = area.x;
            let val_start = x + 6;
            let val_w = just_str.len() as u16;
            rects.push(PropsWidgetRect {
                rect: Rect {
                    x: val_start,
                    y,
                    width: val_w,
                    height: 1,
                },
                action: PropAction::CycleJust,
            });
            lines.push(Line::from(vec![
                Span::styled("Just:", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!(" {}", just_str)),
            ]));
            line_y += 1;
        }

        // Scale: [-].Scale: 3.[+]
        {
            let y = area.y + line_y;
            let x = area.x;
            rects.push(PropsWidgetRect {
                rect: Rect {
                    x: x + 2,
                    y,
                    width: 3,
                    height: 1,
                },
                action: PropAction::ScaleDown,
            });
            rects.push(PropsWidgetRect {
                rect: Rect {
                    x: x + 15,
                    y,
                    width: 3,
                    height: 1,
                },
                action: PropAction::ScaleUp,
            });
            lines.push(Line::from(vec![
                Span::raw("[-] "),
                Span::styled("Scale:", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!(" {} [+] ", tt.scale)),
            ]));
            line_y += 1;
        }

        // Rasterize button if block selected
        if tt.selected_block.is_some() {
            lines.push(Line::from(""));
            let y = area.y + line_y + 1;
            let x = area.x;
            rects.push(PropsWidgetRect {
                rect: Rect {
                    x: x + 2,
                    y,
                    width: 11,
                    height: 1,
                },
                action: PropAction::RasterizeBlock,
            });
            lines.push(Line::from(Span::raw(" [Rasterize] ")));
        }

        // Instructions
        lines.push(Line::from(""));
        lines.push(Line::from(Span::raw(" Mouse: hover→preview")));
        lines.push(Line::from(Span::raw(" Click→place text")));
        lines.push(Line::from(Span::raw(" Enter→commit block")));
        lines.push(Line::from(Span::raw(" Esc→exit editing")));

        frame.render_widget(Paragraph::new(lines), area);
    }

    fn render_placeholder(frame: &mut Frame<'_>, area: Rect, name: &str, theme: &Theme) {
        let text = format!(" {name} — coming soon ");
        let para = Paragraph::new(text).style(
            Style::default()
                .fg(theme.general.secondary)
                .add_modifier(Modifier::DIM),
        );
        frame.render_widget(para, area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_area_matches_render_layout() {
        // area: 30x20 panel. Outer border (1) + tab row (1) + separator (1)
        // = content starts 3 rows down from the panel's own y, 1 col in.
        let panel = SidePanel::new(BTreeMap::new(), Theme::default());
        let area = Rect::new(0, 0, 30, 20);
        let content = panel.content_area(area);
        assert_eq!(content.x, 1);
        assert_eq!(content.y, 3);
        assert_eq!(content.width, 28);
        assert_eq!(content.height, 17);
    }

    #[test]
    fn test_content_area_too_small_yields_zero_height() {
        let panel = SidePanel::new(BTreeMap::new(), Theme::default());
        let area = Rect::new(0, 0, 30, 3);
        let content = panel.content_area(area);
        assert_eq!(content.height, 0);
    }

    #[test]
    fn test_brush_props_populates_rects() {
        let brush = BrushState::new();
        let mut lines = Vec::new();
        let mut rects = Vec::new();
        let area = Rect::new(1, 3, 28, 20);
        let mut line_y = 0;

        SidePanel::add_brush_props(&mut lines, &brush, &mut rects, area, &mut line_y);

        // Should have rects for: size -, size +, shape cycle, mode cycle,
        // density -, density +, char click = 7 rects
        assert_eq!(rects.len(), 7);
        assert!(rects.iter().any(|r| r.action == PropAction::SizeDown));
        assert!(rects.iter().any(|r| r.action == PropAction::SizeUp));
        assert!(rects.iter().any(|r| r.action == PropAction::CycleShape));
        assert!(rects.iter().any(|r| r.action == PropAction::CycleSubMode));
        assert!(rects.iter().any(|r| r.action == PropAction::DensityDown));
        assert!(rects.iter().any(|r| r.action == PropAction::DensityUp));
        assert!(rects.iter().any(|r| r.action == PropAction::BeginEditChar));

        // Verify all rects have valid geometry (non-zero area, within content area)
        for r in &rects {
            assert!(r.rect.width > 0);
            assert!(r.rect.height > 0);
            assert!(r.rect.x >= area.x);
            assert!(r.rect.y >= area.y);
            assert!(r.rect.x + r.rect.width <= area.x + area.width);
        }
    }

    #[test]
    fn test_text_props_populates_rects() {
        let tt = TextToolState::new("fonts");
        let mut lines = Vec::new();
        let mut rects = Vec::new();
        let area = Rect::new(1, 3, 28, 20);
        let mut line_y = 0;

        SidePanel::add_text_props(&mut lines, &tt, &mut rects, area, &mut line_y);

        // Figlet mode rects: Mode, FontPrev, FontNext, Just, Scale-, Scale+
        assert!(rects.iter().any(|r| r.action == PropAction::CycleSubMode));
        assert!(rects.iter().any(|r| r.action == PropAction::FontPrev));
        assert!(rects.iter().any(|r| r.action == PropAction::FontNext));
        assert!(rects.iter().any(|r| r.action == PropAction::CycleJust));
        assert!(rects.iter().any(|r| r.action == PropAction::ScaleDown));
        assert!(rects.iter().any(|r| r.action == PropAction::ScaleUp));
        assert_eq!(rects.len(), 6);
    }

    #[test]
    fn test_text_props_rasterize_rect_matches_button_row() {
        // Regression: the hit-rect was computed before the blank line was
        // pushed, so it sat one row ABOVE the visible "[Rasterize]" label.
        use crate::render::Justification;
        use crate::tui::tools::text::TextBlock;
        use crate::tui::tools::text::TextToolState;
        let mut tt = TextToolState::new("fonts");
        // Need a selected block for the button to render. Seed one directly.
        tt.blocks.push(TextBlock {
            id: 0,
            text: "X".into(),
            font_name: None,
            font_index: 0,
            x: 0,
            y: 0,
            scale: 1,
            justification: Justification::Left,
            text_color: None,
            bg_color: None,
            mode: crate::tui::tools::text::TextMode::Figlet,
            rotation: 0,
            cached_rows: vec!["X".into()],
            width: 1,
            height: 1,
        });
        tt.selected_block = Some(0);

        let mut lines = Vec::new();
        let mut rects = Vec::new();
        let area = Rect::new(0, 0, 30, 24);
        let mut line_y = 0u16;
        SidePanel::add_text_props(&mut lines, &tt, &mut rects, area, &mut line_y);

        let raster = rects
            .iter()
            .find(|r| r.action == PropAction::RasterizeBlock)
            .expect("rasterize rect should exist");
        // Find the "[Rasterize]" line index in the rendered lines.
        let label_row = lines
            .iter()
            .position(|l| l.spans.iter().any(|s| s.content.contains("[Rasterize]")))
            .expect("rasterize label should be rendered") as u16;
        assert_eq!(
            raster.rect.y,
            area.y + label_row,
            "rasterize hit-rect y must match the label row"
        );
    }

    #[test]
    fn test_palette_click_accounts_for_multi_select_row() {
        use crate::tui::palette::Palette;
        use crossterm::event::KeyCode;
        use ratatui::layout::Rect;
        let mut p = Palette::new();
        let area = Rect::new(0, 0, 16, 20);
        // Without multi-select, Neutrals first swatch is inner row 2 → term row 3.
        let handled = p.handle_click(2, 3, area);
        assert!(handled, "swatch click without multi-select");
        let without = p.selected_color;

        // With multi-select active, render inserts " Sel:" at inner row 1,
        // pushing swatches down one row. Same physical row must miss; row+1 hits.
        // Tab is the public toggle for multi_select_active.
        let _ = p.handle_key(KeyCode::Tab);
        p.selected_color = None;
        let missed = p.handle_click(2, 3, area);
        assert!(
            !missed || p.selected_color == without,
            "click on pre-shift row must not select the post-shift swatch"
        );
        p.selected_color = None;
        let hit = p.handle_click(2, 4, area);
        assert!(hit, "swatch click with multi-select needs +1 row offset");
        assert!(p.selected_color.is_some());
    }

    #[test]
    fn test_fill_props_populates_rects() {
        let mut lines = Vec::new();
        let mut rects = Vec::new();
        let area = Rect::new(1, 3, 28, 20);
        let mut line_y = 0;

        SidePanel::add_fill_props(&mut lines, 12, &mut rects, area, &mut line_y);

        // Should have rects for: threshold -, threshold +
        assert_eq!(rects.len(), 2);
        assert!(rects
            .iter()
            .any(|r| r.action == PropAction::FillThresholdDown));
        assert!(rects
            .iter()
            .any(|r| r.action == PropAction::FillThresholdUp));
    }

    #[test]
    fn test_move_props_populates_rects() {
        let state = MoveState::default();
        let mut lines = Vec::new();
        let mut rects = Vec::new();
        let area = Rect::new(1, 3, 28, 20);
        let mut line_y = 0;

        SidePanel::add_move_props(&mut lines, &state, &mut rects, area, &mut line_y);

        assert_eq!(rects.len(), 4);
        assert!(rects.iter().any(|r| r.action == PropAction::MoveStrideDown));
        assert!(rects.iter().any(|r| r.action == PropAction::MoveStrideUp));
        assert!(rects.iter().any(|r| r.action == PropAction::MoveSnapToggle));
        assert!(rects.iter().any(|r| r.action == PropAction::MoveWrapToggle));
        for r in &rects {
            assert!(r.rect.width > 0);
            assert!(r.rect.height > 0);
        }
    }

    #[test]
    fn test_rotate_props_populates_rects() {
        let state = RotateState::default();
        let mut lines = Vec::new();
        let mut rects = Vec::new();
        let area = Rect::new(1, 3, 28, 20);
        let mut line_y = 0;

        SidePanel::add_rotate_props(&mut lines, &state, &mut rects, area, &mut line_y);

        assert_eq!(rects.len(), 4);
        assert!(rects.iter().any(|r| r.action == PropAction::RotateStepDown));
        assert!(rects.iter().any(|r| r.action == PropAction::RotateStepUp));
        assert!(rects
            .iter()
            .any(|r| r.action == PropAction::RotateDirToggle));
        assert!(rects
            .iter()
            .any(|r| r.action == PropAction::RotatePivotCycle));
        for r in &rects {
            assert!(r.rect.width > 0);
            assert!(r.rect.height > 0);
        }
    }

    #[test]
    fn test_select_props_populates_rects() {
        let state = SelectionState::default();
        let mut lines = Vec::new();
        let mut rects = Vec::new();
        let area = Rect::new(1, 3, 28, 20);
        let mut line_y = 0;

        SidePanel::add_select_props(&mut lines, &state, &mut rects, area, &mut line_y);

        assert_eq!(rects.len(), 5);
        assert!(rects
            .iter()
            .any(|r| r.action == PropAction::SelectFeatherDown));
        assert!(rects
            .iter()
            .any(|r| r.action == PropAction::SelectFeatherUp));
        assert!(rects
            .iter()
            .any(|r| r.action == PropAction::SelectAdditiveToggle));
        assert!(rects
            .iter()
            .any(|r| r.action == PropAction::SelectSubtractiveToggle));
        assert!(rects
            .iter()
            .any(|r| r.action == PropAction::SelectMoveToggle));
        for r in &rects {
            assert!(r.rect.width > 0);
            assert!(r.rect.height > 0);
        }
    }

    #[test]
    fn test_line_props_populates_rects() {
        let state = LineState::default();
        let mut lines = Vec::new();
        let mut rects = Vec::new();
        let area = Rect::new(1, 3, 28, 20);
        let mut line_y = 0;

        SidePanel::add_line_props(&mut lines, &state, &mut rects, area, &mut line_y);

        assert_eq!(rects.len(), 4);
        assert!(rects.iter().any(|r| r.action == PropAction::LineWidthDown));
        assert!(rects.iter().any(|r| r.action == PropAction::LineWidthUp));
        assert!(rects.iter().any(|r| r.action == PropAction::LineArrowCycle));
        assert!(rects
            .iter()
            .any(|r| r.action == PropAction::LineCurveToggle));
        for r in &rects {
            assert!(r.rect.width > 0);
            assert!(r.rect.height > 0);
        }
    }
}
