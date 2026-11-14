use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;
use std::collections::BTreeMap;
use unicode_width::UnicodeWidthStr;

use super::super::theme::Theme;
use super::super::AppMode;

/// Longest status message shown, in chars.
const MESSAGE_MAX: usize = 72;

struct StatusItem<'a> {
    spans: Vec<Span<'a>>,
    width: u16,
    keep: bool,
}

pub struct StatusBarWidget<'a> {
    mode: AppMode,
    mode_name: &'a str,
    cursor: (u16, u16),
    zoom: u8,
    tool_name: &'a str,
    unsaved: bool,
    font_name: Option<&'a str>,
    glyph_count: Option<usize>,
    git_branch: Option<&'a str>,
    fps: f64,
    render_mode: &'a str,
    clock_str: &'a str,
    layer_count: u8,
    undo_count: usize,
    throbber_text: &'a str,
    icons: &'a BTreeMap<String, String>,
    theme: &'a Theme,
    lighting_active: bool,
    light_type: Option<&'a str>,
    light_intensity: Option<f32>,
    canvas_size: Option<(u16, u16)>,
    brush_size: Option<u8>,
    brush_shape: Option<&'a str>,
    /// Transient message (an fx compile error, say) and whether it is an
    /// error.
    message: Option<(&'a str, bool)>,
}

impl<'a> StatusBarWidget<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        mode: AppMode,
        mode_name: &'a str,
        cursor: (u16, u16),
        zoom: u8,
        tool_name: &'a str,
        unsaved: bool,
        font_name: Option<&'a str>,
        glyph_count: Option<usize>,
        git_branch: Option<&'a str>,
        fps: f64,
        render_mode: &'a str,
        clock_str: &'a str,
        layer_count: u8,
        undo_count: usize,
        throbber_text: &'a str,
        icons: &'a BTreeMap<String, String>,
        theme: &'a Theme,
    ) -> Self {
        Self {
            mode,
            mode_name,
            cursor,
            zoom,
            tool_name,
            unsaved,
            font_name,
            glyph_count,
            git_branch,
            fps,
            render_mode,
            clock_str,
            layer_count,
            undo_count,
            throbber_text,
            icons,
            theme,
            lighting_active: false,
            light_type: None,
            light_intensity: None,
            canvas_size: None,
            brush_size: None,
            brush_shape: None,
            message: None,
        }
    }

    /// [`Self::with_message`] when `message` is `Some`.
    pub fn with_message_opt(self, message: Option<(&'a str, bool)>) -> Self {
        match message {
            Some((text, is_error)) => self.with_message(text, is_error),
            None => self,
        }
    }

    /// Show `text` after the cursor position; errors use the theme's error
    /// colour. Kept when the bar is narrow, but cut to fit.
    pub fn with_message(mut self, text: &'a str, is_error: bool) -> Self {
        self.message = Some((text, is_error));
        self
    }

    pub fn with_canvas_size(mut self, width: u16, height: u16) -> Self {
        self.canvas_size = Some((width, height));
        self
    }

    pub fn with_brush(mut self, size: u8, shape: &'a str) -> Self {
        self.brush_size = Some(size);
        self.brush_shape = Some(shape);
        self
    }

    pub fn with_lighting(
        mut self,
        active: bool,
        light_type: Option<&'a str>,
        intensity: Option<f32>,
    ) -> Self {
        self.lighting_active = active;
        self.light_type = light_type;
        self.light_intensity = intensity;
        self
    }

    fn icon(&self, key: &str, fallback: &'static str) -> &str {
        self.icons.get(key).map_or(fallback, |s| s.as_str())
    }

    fn mode_color(&self) -> ratatui::style::Color {
        match self.mode {
            AppMode::FontEditor => self.theme.statusbar.mode_font,
            AppMode::ImageEditor => self.theme.statusbar.mode_image,
            AppMode::AsciiPreview => self.theme.statusbar.mode_ascii,
            AppMode::Lighting => self.theme.statusbar.mode_lighting,
        }
    }

    fn make_item(spans: Vec<Span<'a>>, keep: bool) -> StatusItem<'a> {
        let width = spans.iter().map(|s| s.content.width()).sum::<usize>() as u16;
        StatusItem { spans, width, keep }
    }

    fn build_all_items(&self) -> Vec<StatusItem<'a>> {
        if self.lighting_active {
            return self.build_lighting_items();
        }

        let mut items: Vec<StatusItem> = Vec::new();

        let mode_icon = self.icon("status_mode", "M");
        items.push(Self::make_item(
            vec![Span::styled(
                format!(" {} {} ", mode_icon, self.mode_name),
                Style::default()
                    .fg(self.mode_color())
                    .add_modifier(Modifier::BOLD),
            )],
            true,
        ));

        let tool_icon = self.icon("status_tool", "T");
        items.push(Self::make_item(
            vec![Span::raw(format!(" {} {} ", tool_icon, self.tool_name))],
            true,
        ));

        let pos_icon = self.icon("status_position", "+");
        items.push(Self::make_item(
            vec![Span::raw(format!(
                " {} X:{} Y:{} ",
                pos_icon, self.cursor.0, self.cursor.1
            ))],
            true,
        ));

        if let Some((text, is_error)) = self.message {
            let color = if is_error {
                self.theme.general.error
            } else {
                self.theme.general.primary
            };
            let cut: String = text.chars().take(MESSAGE_MAX).collect();
            items.push(Self::make_item(
                vec![Span::styled(
                    format!(" {cut} "),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                )],
                true,
            ));
        }

        let zoom_icon = self.icon("status_zoom", "Z");
        items.push(Self::make_item(
            vec![Span::styled(
                format!(" {} {}x ", zoom_icon, self.zoom),
                Style::default().fg(self.theme.statusbar.label),
            )],
            false,
        ));

        let has_font = self.font_name.is_some();
        let mut font_spans: Vec<Span<'a>> = Vec::new();
        if let Some(name) = self.font_name {
            let font_icon = self.icon("status_font", "F");
            font_spans.push(Span::styled(
                format!(" {} {} ", font_icon, name),
                Style::default().fg(self.theme.statusbar.font_name),
            ));
        }
        if self.unsaved {
            let unsaved_icon = self.icon("status_unsaved", "!");
            font_spans.push(Span::styled(
                format!(" {} ", unsaved_icon),
                Style::default().fg(self.theme.statusbar.unsaved),
            ));
        } else if has_font {
            let saved_icon = self.icon("status_saved", "*");
            font_spans.push(Span::styled(
                format!(" {} ", saved_icon),
                Style::default().fg(self.theme.statusbar.saved),
            ));
        }
        if has_font {
            if let Some(count) = self.glyph_count {
                let glyph_icon = self.icon("status_glyph", "#");
                font_spans.push(Span::styled(
                    format!(" {} {} ", glyph_icon, count),
                    Style::default().fg(self.theme.statusbar.glyph_count),
                ));
            }
        }
        if !font_spans.is_empty() {
            items.push(Self::make_item(font_spans, false));
        }

        if let Some(branch) = self.git_branch {
            let branch_icon = self.icon("status_git_branch", "\u{2387}");
            items.push(Self::make_item(
                vec![Span::styled(
                    format!(" {} {} ", branch_icon, branch),
                    Style::default().fg(self.theme.statusbar.git_branch),
                )],
                false,
            ));
        }

        items.push(Self::make_item(
            vec![Span::styled(
                format!(" FPS:{:.0} ", self.fps),
                Style::default().fg(self.theme.statusbar.fps),
            )],
            false,
        ));

        let clock_icon = self.icon("status_clock", "\u{1f550}");
        items.push(Self::make_item(
            vec![Span::styled(
                format!(" {} {} ", clock_icon, self.clock_str),
                Style::default().fg(self.theme.statusbar.label),
            )],
            false,
        ));

        if !self.render_mode.is_empty() {
            items.push(Self::make_item(
                vec![Span::styled(
                    format!(" {} ", self.render_mode),
                    Style::default().fg(self.theme.statusbar.label),
                )],
                false,
            ));
        }

        if self.layer_count > 0 {
            let layer_icon = self.icon("status_layer", "L");
            items.push(Self::make_item(
                vec![Span::styled(
                    format!(" {}:{} ", layer_icon, self.layer_count),
                    Style::default().fg(self.theme.statusbar.label),
                )],
                false,
            ));
        }

        if self.undo_count > 0 {
            let undo_icon = self.icon("status_undo", "U");
            items.push(Self::make_item(
                vec![Span::styled(
                    format!(" {}:{} ", undo_icon, self.undo_count),
                    Style::default().fg(self.theme.statusbar.label),
                )],
                false,
            ));
        }

        if let Some((w, h)) = self.canvas_size {
            items.push(Self::make_item(
                vec![Span::styled(
                    format!(" {w}×{h} "),
                    Style::default().fg(self.theme.statusbar.label),
                )],
                false,
            ));
        }

        if let (Some(size), Some(shape)) = (self.brush_size, self.brush_shape) {
            items.push(Self::make_item(
                vec![Span::styled(
                    format!(" {shape}:{size} "),
                    Style::default().fg(self.theme.statusbar.label),
                )],
                false,
            ));
        }

        if !self.throbber_text.is_empty() {
            items.push(Self::make_item(
                vec![Span::raw(format!(" {} ", self.throbber_text))],
                false,
            ));
        }

        items
    }

    fn build_lighting_items(&self) -> Vec<StatusItem<'a>> {
        let mut items: Vec<StatusItem> = Vec::new();

        items.push(Self::make_item(
            vec![Span::styled(
                " LIGHTING ",
                Style::default()
                    .fg(self.mode_color())
                    .add_modifier(Modifier::BOLD),
            )],
            true,
        ));

        if let Some(lt) = self.light_type {
            let intensity_str = self
                .light_intensity
                .map(|i| format!(" {:.2} ", i))
                .unwrap_or_default();
            items.push(Self::make_item(
                vec![Span::styled(
                    format!(" {} {} ", lt, intensity_str),
                    Style::default().fg(self.theme.statusbar.label),
                )],
                true,
            ));
        }

        items.push(Self::make_item(
            vec![Span::styled(
                format!(" FPS:{:.0} ", self.fps),
                Style::default().fg(self.theme.statusbar.fps),
            )],
            false,
        ));

        let clock_icon = self.icon("status_clock", "\u{1f550}");
        items.push(Self::make_item(
            vec![Span::styled(
                format!(" {} {} ", clock_icon, self.clock_str),
                Style::default().fg(self.theme.statusbar.label),
            )],
            false,
        ));

        items
    }
}

impl<'a> Widget for StatusBarWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 10 {
            return;
        }

        let area_w = area.width;
        let sep_style = Style::default().fg(self.theme.statusbar.separator);

        let mut items = self.build_all_items();

        let mode_w = items[0].width;
        let pos_w = items[2].width;
        if mode_w + pos_w + 3 >= area_w {
            let mode_icon = self.icon("status_mode", "M");
            let mode_text = format!(" {} {} ", mode_icon, self.mode_name);
            let truncated: String = mode_text
                .chars()
                .take((area_w as usize).saturating_sub(2))
                .collect();
            let line = Line::from(vec![Span::styled(
                truncated,
                Style::default()
                    .fg(self.mode_color())
                    .add_modifier(Modifier::BOLD),
            )]);
            buf.set_line(area.x, area.y, &line, area_w);
            return;
        }

        loop {
            let n = items.len();
            let sep_count = n.saturating_sub(1);
            let total_w: u16 = items.iter().map(|i| i.width).sum::<u16>() + sep_count as u16 * 3;
            if total_w <= area_w {
                break;
            }
            let drop_idx = items.iter().rposition(|i| !i.keep);
            match drop_idx {
                Some(idx) => {
                    items.remove(idx);
                }
                None => {
                    break;
                }
            }
        }

        let mut final_spans: Vec<Span<'a>> = Vec::new();
        for (i, item) in items.into_iter().enumerate() {
            if i > 0 {
                final_spans.push(Span::styled(" \u{2502} ", sep_style));
            }
            final_spans.extend(item.spans);
        }

        buf.set_line(area.x, area.y, &Line::from(final_spans), area_w);
    }
}
