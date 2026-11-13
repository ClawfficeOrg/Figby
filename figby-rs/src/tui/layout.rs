use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::widgets::Borders;

pub const TIMELINE_HEIGHT: u16 = 8;
pub const DRAWER_WIDTH: u16 = 28;

/// Full borders for the toolbox list panel.
pub fn toolbox_list_borders() -> Borders {
    Borders::ALL
}

/// All widget Rects for one frame, computed in a single pass.
/// Stored on TuiApp so mouse handlers can use last-frame geometry.
#[derive(Debug, Clone, Copy)]
pub struct FrameLayout {
    pub menu: Rect,
    pub tabs: Rect,
    pub main: Rect,
    pub status: Rect,
    /// Full toolbox column (dynamic). None in zen mode.
    pub toolbox_full: Option<Rect>,
    /// Upper portion of toolbox column (for mouse hit-testing tool list items).
    pub toolbox_list: Option<Rect>,

    /// Palette panel below the toolbox in the left column. None in zen mode.
    pub palette: Option<Rect>,
    /// Center canvas area.
    pub canvas: Rect,
    /// Right drawer. None when drawer closed or zen mode.
    pub right_panel: Option<Rect>,
    /// Timeline panel at bottom of canvas. None when timeline hidden.
    pub timeline: Option<Rect>,
}

impl FrameLayout {
    /// Compute layout for the given terminal area.
    ///
    /// Collapsed-border convention (ratatui recipe):
    ///   - Toolbox list block uses `toolbox_list_borders()`: ALL borders.
    ///   - Canvas block omits LEFT when toolbox is visible (shares toolbox's
    ///     right border) and omits RIGHT when right panel is visible.
    ///   - Right panel block uses Borders::ALL (canvas omits its RIGHT when
    ///     right panel is open, so right panel owns its LEFT).
    ///   - All layouts use `spacing(0)` so adjacent rects share edges exactly.
    ///
    /// A `toolbox_width` of 0 hides the whole left column (toolbox and
    /// palette): `toolbox_full`, `toolbox_list` and `palette` are `None`
    /// and the canvas takes the width.
    pub fn compute(
        area: Rect,
        zen_mode: bool,
        side_panel_open: bool,
        toolbox_width: u16,
        toolbox_h: u16,
        timeline_visible: bool,
    ) -> Self {
        let vert = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .spacing(0)
        .split(area);

        let menu = vert[0];
        let tabs = vert[1];
        let main = vert[2];
        let status = vert[3];

        if zen_mode {
            return Self {
                menu,
                tabs,
                main,
                status,
                toolbox_full: None,
                toolbox_list: None,
                palette: None,
                canvas: area,
                right_panel: None,
                timeline: None,
            };
        }

        let (main_area, timeline_area) = if timeline_visible {
            let v = Layout::vertical([Constraint::Fill(1), Constraint::Length(TIMELINE_HEIGHT)])
                .spacing(0)
                .split(main);
            (v[0], Some(v[1]))
        } else {
            (main, None)
        };

        let h_areas = if side_panel_open {
            Layout::horizontal([
                Constraint::Length(toolbox_width),
                Constraint::Fill(1),
                Constraint::Length(DRAWER_WIDTH),
            ])
            .spacing(0)
            .split(main_area)
        } else {
            Layout::horizontal([Constraint::Length(toolbox_width), Constraint::Fill(1)])
                .spacing(0)
                .split(main_area)
        };

        let toolbox_full = h_areas[0];
        let canvas = h_areas[1];
        let right_panel = if side_panel_open {
            Some(h_areas[2])
        } else {
            None
        };

        // Split the toolbox column: top=toolbox, bottom=palette
        let (toolbox_full, toolbox_list, palette_rect) = if toolbox_width == 0 {
            (None, None, None)
        } else {
            let left_vert = Layout::vertical([Constraint::Length(toolbox_h), Constraint::Min(0)])
                .spacing(0)
                .split(toolbox_full);
            (Some(toolbox_full), Some(left_vert[0]), Some(left_vert[1]))
        };

        Self {
            menu,
            tabs,
            main,
            status,
            toolbox_full,
            toolbox_list,
            palette: palette_rect,
            canvas,
            right_panel,
            timeline: timeline_area,
        }
    }

    /// Collapsed border flags for the canvas block.
    ///
    /// Omits LEFT when toolbox is present (toolbox provides that edge),
    /// omits RIGHT when right panel is present.
    /// Shared TOP/BOTTOM edges with toolbox/right panel panels are drawn
    /// by both (inherent to ratatui's block model for side-by-side blocks).
    pub fn canvas_borders(&self) -> Borders {
        match (
            self.toolbox_full.is_some(),
            self.right_panel.is_some(),
            self.timeline.is_some(),
        ) {
            (true, true, false) => Borders::TOP | Borders::BOTTOM,
            (true, true, true) => Borders::TOP,
            (true, false, false) => Borders::TOP | Borders::RIGHT | Borders::BOTTOM,
            (true, false, true) => Borders::TOP | Borders::RIGHT,
            (false, true, false) => Borders::TOP | Borders::LEFT | Borders::BOTTOM,
            (false, true, true) => Borders::TOP | Borders::LEFT,
            (false, false, false) => Borders::ALL,
            (false, false, true) => Borders::TOP | Borders::LEFT | Borders::RIGHT,
        }
    }

    /// Borders for the timeline panel block.
    /// Omits TOP — shared with canvas's bottom edge.
    pub fn timeline_borders(&self) -> Borders {
        Borders::LEFT | Borders::RIGHT | Borders::BOTTOM
    }
}

impl Default for FrameLayout {
    fn default() -> Self {
        Self::compute(Rect::new(0, 0, 80, 24), false, false, 20, 15, false)
    }
}

/// Centered overlay for the palette editor panel.
/// 42 columns wide, roughly half terminal height, centered.
pub fn palette_editor_overlay(area: Rect) -> Rect {
    let width = 42u16.min(area.width.saturating_sub(4));
    let height = (area.height / 2).max(12).min(area.height.saturating_sub(4));
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect {
        x,
        y,
        width,
        height,
    }
}
