use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, StatefulWidget, Widget};

use super::keymap::{self, GlobalAction};
use super::theme::Theme;
use super::toolbox::Tool;

#[derive(Debug, Clone, PartialEq)]
pub enum MenuAction {
    FileNew,
    FileOpen,
    FileSave,
    FileSaveAs,
    FileSaveAsFigmap,
    FileExport,
    FileImportGif,
    FileQuit,
    FontNewFromFile,
    FontNewFromSystem,
    EditUndo,
    EditRedo,
    EditCut,
    EditCopy,
    EditPaste,
    ViewZoomIn,
    ViewZoomOut,
    ViewToggleGrid,
    ViewToggleUndoPanel,
    ViewToggleTimeline,
    ViewToggleSidePanel,
    ViewLoadBuiltinPalette(&'static str),
    ViewPaletteEditor,
    LayerNew,
    LayerDuplicate,
    LayerDelete,
    LayerMergeDown,
    LayerMoveUp,
    LayerMoveDown,
    LayerToggleVisibility,
    LayerToggleLock,
    AnimFrameAdd,
    AnimFrameDelete,
    AnimPlay,
    AnimToggleTimeline,
    ImageResizeCanvas,
    ToolsSelect(Tool),
    HelpAbout,
    HelpKeybindings,
}

struct TopMenu {
    label: &'static str,
    items: Vec<(&'static str, Option<String>, MenuAction)>,
}

/// Persistent + per-frame state for `MenuBar` (held separately so `MenuBar`
/// can implement `StatefulWidget for &MenuBar`).
pub struct MenuBarState {
    /// Index of the open dropdown, None = closed.
    pub active_menu: Option<usize>,
    focused_item: usize,
    /// Header rects recorded during last render pass.
    pub header_rects: Vec<Rect>,
    /// Item rects for the currently open dropdown.
    pub item_rects: Vec<Rect>,
    frame_area: Rect,
    pending_action: Option<MenuAction>,
}

impl MenuBarState {
    pub fn new() -> Self {
        Self {
            active_menu: None,
            focused_item: 0,
            header_rects: Vec::new(),
            item_rects: Vec::new(),
            frame_area: Rect::default(),
            pending_action: None,
        }
    }

    pub fn is_active(&self) -> bool {
        self.active_menu.is_some()
    }

    pub fn reset(&mut self) {
        self.active_menu = None;
        self.focused_item = 0;
        self.item_rects.clear();
    }

    pub fn drain_actions(&mut self) -> Option<MenuAction> {
        self.pending_action.take()
    }
}

impl Default for MenuBarState {
    fn default() -> Self {
        Self::new()
    }
}

/// Static configuration for the menu bar (menus + theme).
/// Dynamic state lives in `MenuBarState`.
pub struct MenuBar {
    menus: Vec<TopMenu>,
    pub theme: Theme,
}

impl MenuBar {
    pub fn new() -> Self {
        Self {
            menus: build_menus(),
            theme: Theme::default(),
        }
    }

    pub fn handle_key_event(&self, key: KeyEvent, state: &mut MenuBarState) -> bool {
        let code = key.code;
        let modifiers = key.modifiers;

        if modifiers == KeyModifiers::ALT {
            if let KeyCode::Char(c) = code {
                if let Some(idx) = alt_menu_index(c) {
                    self.open_menu(state, idx);
                    return true;
                }
            }
        }

        if state.active_menu.is_some() {
            match code {
                KeyCode::Esc => {
                    state.reset();
                    true
                }
                KeyCode::Enter => {
                    self.select_focused(state);
                    true
                }
                KeyCode::Up => {
                    if state.focused_item > 0 {
                        state.focused_item -= 1;
                    } else if let Some(idx) = state.active_menu {
                        state.focused_item = self.menus[idx].items.len().saturating_sub(1);
                    }
                    true
                }
                KeyCode::Down => {
                    if let Some(idx) = state.active_menu {
                        let max = self.menus[idx].items.len().saturating_sub(1);
                        if state.focused_item < max {
                            state.focused_item += 1;
                        } else {
                            state.focused_item = 0;
                        }
                    }
                    true
                }
                KeyCode::Left => {
                    if let Some(idx) = state.active_menu {
                        let prev = if idx == 0 {
                            self.menus.len() - 1
                        } else {
                            idx - 1
                        };
                        self.open_menu(state, prev);
                    }
                    true
                }
                KeyCode::Right => {
                    if let Some(idx) = state.active_menu {
                        let next = (idx + 1) % self.menus.len();
                        self.open_menu(state, next);
                    }
                    true
                }
                _ => true,
            }
        } else {
            false
        }
    }

    pub fn handle_mouse_event(
        &self,
        col: u16,
        row: u16,
        kind: MouseEventKind,
        state: &mut MenuBarState,
    ) -> bool {
        match kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(item_idx) = self.hit_item(col, row, state) {
                    if let Some(menu_idx) = state.active_menu {
                        let menu = &self.menus[menu_idx];
                        if item_idx < menu.items.len() {
                            let action = menu.items[item_idx].2.clone();
                            state.pending_action = Some(action);
                        }
                    }
                    state.reset();
                    return true;
                }

                if let Some(header_idx) = self.hit_header(col, row, state) {
                    if state.active_menu == Some(header_idx) {
                        state.reset();
                    } else {
                        self.open_menu(state, header_idx);
                    }
                    return true;
                }

                if state.active_menu.is_some() {
                    state.reset();
                    return true;
                }

                false
            }
            _ => false,
        }
    }

    fn open_menu(&self, state: &mut MenuBarState, idx: usize) {
        state.active_menu = Some(idx);
        state.focused_item = 0;
    }

    fn select_focused(&self, state: &mut MenuBarState) {
        let Some(menu_idx) = state.active_menu else {
            return;
        };
        let menu = &self.menus[menu_idx];
        if state.focused_item < menu.items.len() {
            let action = menu.items[state.focused_item].2.clone();
            state.pending_action = Some(action);
        }
        state.reset();
    }

    fn hit_header(&self, col: u16, row: u16, state: &MenuBarState) -> Option<usize> {
        state
            .header_rects
            .iter()
            .enumerate()
            .find(|(_, r)| col >= r.x && col < r.x + r.width && row == r.y)
            .map(|(i, _)| i)
    }

    fn hit_item(&self, col: u16, row: u16, state: &MenuBarState) -> Option<usize> {
        state
            .item_rects
            .iter()
            .enumerate()
            .find(|(_, r)| col >= r.x && col < r.x + r.width && row >= r.y && row < r.y + r.height)
            .map(|(i, _)| i)
    }

    fn render_dropdown(&self, menu_idx: usize, buf: &mut Buffer, state: &mut MenuBarState) {
        let menu = &self.menus[menu_idx];
        let item_count = menu.items.len() as u16;

        let max_label_w: u16 = menu
            .items
            .iter()
            .map(|(label, _, _)| label.len() as u16)
            .max()
            .unwrap_or(10);
        let max_shortcut_w: u16 = menu
            .items
            .iter()
            .map(|(_, shortcut, _)| shortcut.as_deref().map(str::len).unwrap_or(0) as u16)
            .max()
            .unwrap_or(0);
        let shortcut_col_w = if max_shortcut_w > 0 {
            max_shortcut_w + 2
        } else {
            0
        };
        let dropdown_w = max_label_w + shortcut_col_w + 4;
        let dropdown_h = item_count + 2;

        let Some(&header_rect) = state.header_rects.get(menu_idx) else {
            return;
        };
        let drop_x = header_rect
            .x
            .min(state.frame_area.width.saturating_sub(dropdown_w));
        let drop_y = header_rect.y + 1;

        if drop_y >= state.frame_area.height {
            return;
        }
        let clamp_h = dropdown_h.min(state.frame_area.height.saturating_sub(drop_y));
        let dropdown_rect = Rect::new(drop_x, drop_y, dropdown_w, clamp_h);

        Widget::render(Clear, dropdown_rect, buf);
        let block = Block::default().borders(Borders::ALL).style(
            Style::default()
                .bg(self.theme.menu.dropdown_bg)
                .fg(self.theme.menu.fg),
        );
        Widget::render(block, dropdown_rect, buf);

        let inner_x = dropdown_rect.x + 1;
        let inner_y = dropdown_rect.y + 1;
        let inner_w = dropdown_rect.width.saturating_sub(2);
        let inner_h = dropdown_rect.height.saturating_sub(2);

        state.item_rects.clear();

        for (i, (label, shortcut, _)) in menu.items.iter().enumerate() {
            if i as u16 >= inner_h {
                break;
            }
            let item_rect = Rect::new(inner_x, inner_y + i as u16, inner_w, 1);
            state.item_rects.push(item_rect);

            let is_focused = i == state.focused_item;
            let style = if is_focused {
                Style::default()
                    .fg(self.theme.menu.fg)
                    .bg(self.theme.menu.highlight)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(self.theme.menu.fg)
                    .bg(self.theme.menu.dropdown_bg)
            };
            let content_w = (inner_w as usize).saturating_sub(2);
            let padded = if let Some(shortcut) = shortcut {
                let label_w = content_w.saturating_sub(shortcut.len() + 1);
                format!(" {label:<label_w$} {shortcut} ")
            } else {
                format!(" {label:<content_w$} ")
            };
            Widget::render(Paragraph::new(padded).style(style), item_rect, buf);
        }
    }
}

impl Default for MenuBar {
    fn default() -> Self {
        Self::new()
    }
}

impl StatefulWidget for &MenuBar {
    type State = MenuBarState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        state.frame_area = buf.area;
        state.header_rects.clear();

        let bar_style = Style::default()
            .fg(self.theme.menu.fg)
            .bg(self.theme.menu.bg);

        Widget::render(
            Paragraph::new(Line::from(Span::styled(
                " ".repeat(area.width as usize),
                bar_style,
            ))),
            area,
            buf,
        );

        let mut x = area.x + 1;
        for (i, menu) in self.menus.iter().enumerate() {
            let label = format!(" {} ", menu.label);
            let label_w = label.len() as u16;

            if x + label_w > area.x + area.width {
                break;
            }

            let is_open = state.active_menu == Some(i);
            let style = if is_open {
                Style::default()
                    .fg(self.theme.menu.fg)
                    .bg(self.theme.menu.highlight)
                    .add_modifier(Modifier::BOLD)
            } else {
                bar_style
            };

            let header_rect = Rect::new(x, area.y, label_w, 1);
            state.header_rects.push(header_rect);

            Widget::render(
                Paragraph::new(Line::from(Span::styled(label, style))),
                header_rect,
                buf,
            );

            x += label_w;
        }

        if let Some(menu_idx) = state.active_menu {
            self.render_dropdown(menu_idx, buf, state);
        }
    }
}

fn alt_menu_index(c: char) -> Option<usize> {
    match c {
        'f' | 'F' => Some(0), // File
        'e' | 'E' => Some(1), // Edit
        'v' | 'V' => Some(2), // View
        'i' | 'I' => Some(3), // Image
        't' | 'T' => Some(4), // Tools
        'l' | 'L' => Some(5), // Layers
        'a' | 'A' => Some(6), // Animation
        'h' | 'H' => Some(7), // Help
        _ => None,
    }
}

/// Shortcut for a menu item that's derived from `GLOBAL_DISPATCH`, so it
/// can't drift from the actual binding.
fn g(action: GlobalAction) -> Option<String> {
    keymap::global_shortcut_label(action)
}

/// Shortcut for a menu item with no `GlobalAction` equivalent — the
/// binding lives in a scope-specific handler instead (verified against
/// keymap.rs's documented keybinds and the relevant handler).
fn s(shortcut: &str) -> Option<String> {
    Some(shortcut.to_string())
}

fn build_menus() -> Vec<TopMenu> {
    vec![
        TopMenu {
            label: "File",
            items: vec![
                ("New Image", g(GlobalAction::FileNew), MenuAction::FileNew),
                ("Open", g(GlobalAction::FileOpen), MenuAction::FileOpen),
                ("Save", g(GlobalAction::FileSave), MenuAction::FileSave),
                (
                    "Save As",
                    g(GlobalAction::FileSaveAs),
                    MenuAction::FileSaveAs,
                ),
                ("Save as Figmap", None, MenuAction::FileSaveAsFigmap),
                ("Export", g(GlobalAction::Export), MenuAction::FileExport),
                ("Import GIF", None, MenuAction::FileImportGif),
                ("New Font from File", None, MenuAction::FontNewFromFile),
                ("New Font from System", None, MenuAction::FontNewFromSystem),
                ("Quit", g(GlobalAction::Quit), MenuAction::FileQuit),
            ],
        },
        TopMenu {
            label: "Edit",
            items: vec![
                ("Undo", g(GlobalAction::Undo), MenuAction::EditUndo),
                ("Redo", g(GlobalAction::Redo), MenuAction::EditRedo),
                ("Cut", s("Ctrl+X"), MenuAction::EditCut),
                ("Copy", s("Ctrl+C"), MenuAction::EditCopy),
                ("Paste", s("Ctrl+V"), MenuAction::EditPaste),
            ],
        },
        TopMenu {
            label: "View",
            items: vec![
                ("Zoom In", s("+"), MenuAction::ViewZoomIn),
                ("Zoom Out", s("-"), MenuAction::ViewZoomOut),
                ("Toggle Grid", None, MenuAction::ViewToggleGrid),
                (
                    "Toggle Undo Panel",
                    g(GlobalAction::ToggleUndoPanel),
                    MenuAction::ViewToggleUndoPanel,
                ),
                (
                    "Toggle Timeline",
                    g(GlobalAction::ToggleTimeline),
                    MenuAction::ViewToggleTimeline,
                ),
                (
                    "Toggle Side Panel",
                    g(GlobalAction::CycleDrawer),
                    MenuAction::ViewToggleSidePanel,
                ),
                (
                    "Palette Editor",
                    s("Ctrl+Shift+P"),
                    MenuAction::ViewPaletteEditor,
                ),
                (
                    "Palette: Grayscale",
                    None,
                    MenuAction::ViewLoadBuiltinPalette("Grayscale"),
                ),
                (
                    "Palette: Primary",
                    None,
                    MenuAction::ViewLoadBuiltinPalette("Primary"),
                ),
                (
                    "Palette: Warm",
                    None,
                    MenuAction::ViewLoadBuiltinPalette("Warm"),
                ),
                (
                    "Palette: Cool",
                    None,
                    MenuAction::ViewLoadBuiltinPalette("Cool"),
                ),
            ],
        },
        TopMenu {
            label: "Image",
            items: vec![("Resize Canvas", None, MenuAction::ImageResizeCanvas)],
        },
        TopMenu {
            label: "Tools",
            items: vec![
                ("Brush", s("B"), MenuAction::ToolsSelect(Tool::Brush)),
                ("Eraser", s("E"), MenuAction::ToolsSelect(Tool::Eraser)),
                ("Line", s("I"), MenuAction::ToolsSelect(Tool::Line)),
                ("Fill", s("G"), MenuAction::ToolsSelect(Tool::Fill)),
                ("Marquee", s("V"), MenuAction::ToolsSelect(Tool::Marquee)),
                ("Lasso", s("L"), MenuAction::ToolsSelect(Tool::Lasso)),
                (
                    "Circle Select",
                    s("C"),
                    MenuAction::ToolsSelect(Tool::CircleSelect),
                ),
                (
                    "Polygon Select",
                    s("P"),
                    MenuAction::ToolsSelect(Tool::PolygonSelect),
                ),
                (
                    "Eyedropper",
                    s("D"),
                    MenuAction::ToolsSelect(Tool::Eyedropper),
                ),
                ("Spray", s("A"), MenuAction::ToolsSelect(Tool::Spray)),
                ("Text", s("T"), MenuAction::ToolsSelect(Tool::Text)),
                ("Lighting", None, MenuAction::ToolsSelect(Tool::Lighting)),
            ],
        },
        TopMenu {
            label: "Layers",
            items: vec![
                ("New Layer", s("N"), MenuAction::LayerNew),
                ("Duplicate Layer", s("D"), MenuAction::LayerDuplicate),
                ("Delete Layer", s("Delete"), MenuAction::LayerDelete),
                ("Merge Down", s("M"), MenuAction::LayerMergeDown),
                ("Move Up", s("Shift+Up"), MenuAction::LayerMoveUp),
                ("Move Down", s("Shift+Down"), MenuAction::LayerMoveDown),
                (
                    "Toggle Visibility",
                    s("Enter"),
                    MenuAction::LayerToggleVisibility,
                ),
                ("Toggle Lock", s("L"), MenuAction::LayerToggleLock),
            ],
        },
        TopMenu {
            label: "Animation",
            items: vec![
                ("Add Frame", s("A"), MenuAction::AnimFrameAdd),
                ("Delete Frame", s("Delete"), MenuAction::AnimFrameDelete),
                ("Play / Pause", s("Enter"), MenuAction::AnimPlay),
                (
                    "Toggle Timeline",
                    g(GlobalAction::ToggleTimeline),
                    MenuAction::AnimToggleTimeline,
                ),
            ],
        },
        TopMenu {
            label: "Help",
            items: vec![
                ("About", None, MenuAction::HelpAbout),
                (
                    "Keybindings",
                    g(GlobalAction::ToggleKeybindings),
                    MenuAction::HelpKeybindings,
                ),
            ],
        },
    ]
}
