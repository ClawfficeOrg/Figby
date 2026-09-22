use crossterm::event::{KeyCode, KeyModifiers};

/// Machine-dispatchable global actions — one-to-one with entries in [`GLOBAL_DISPATCH`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlobalAction {
    FileNew,
    FileOpen,
    FileSave,
    FileSaveAs,
    Export,
    Undo,
    Redo,
    ToggleUndoPanel,
    ToggleRenderMode,
    ToggleZenMode,
    CycleDrawer,
    ToggleKeybindings,
    ToggleTimeline,
    OpenTweenPanel,
    CycleTabPrev,
    CycleTabNext,
    DocNext,
    DocPrev,
    DocClose,
    NextMode,
    PrevMode,
    Quit,
}

pub struct KeyDispatch {
    pub modifiers: KeyModifiers,
    pub key_code: KeyCode,
    pub action: GlobalAction,
}

pub static GLOBAL_DISPATCH: &[KeyDispatch] = &[
    // File operations
    KeyDispatch {
        modifiers: KeyModifiers::CONTROL,
        key_code: KeyCode::Char('n'),
        action: GlobalAction::FileNew,
    },
    KeyDispatch {
        modifiers: KeyModifiers::CONTROL,
        key_code: KeyCode::Char('o'),
        action: GlobalAction::FileOpen,
    },
    KeyDispatch {
        modifiers: KeyModifiers::CONTROL,
        key_code: KeyCode::Char('s'),
        action: GlobalAction::FileSave,
    },
    KeyDispatch {
        modifiers: KeyModifiers::CONTROL.union(KeyModifiers::SHIFT),
        key_code: KeyCode::Char('s'),
        action: GlobalAction::FileSaveAs,
    },
    KeyDispatch {
        modifiers: KeyModifiers::CONTROL,
        key_code: KeyCode::Char('e'),
        action: GlobalAction::Export,
    },
    // Undo / redo
    KeyDispatch {
        modifiers: KeyModifiers::CONTROL,
        key_code: KeyCode::Char('z'),
        action: GlobalAction::Undo,
    },
    KeyDispatch {
        modifiers: KeyModifiers::CONTROL.union(KeyModifiers::SHIFT),
        key_code: KeyCode::Char('z'),
        action: GlobalAction::Redo,
    },
    KeyDispatch {
        modifiers: KeyModifiers::CONTROL,
        key_code: KeyCode::Char('y'),
        action: GlobalAction::Redo,
    },
    // Panels
    KeyDispatch {
        modifiers: KeyModifiers::CONTROL.union(KeyModifiers::SHIFT),
        key_code: KeyCode::Char('h'),
        action: GlobalAction::ToggleUndoPanel,
    },
    KeyDispatch {
        modifiers: KeyModifiers::CONTROL,
        key_code: KeyCode::Char('k'),
        action: GlobalAction::ToggleKeybindings,
    },
    // View toggles
    KeyDispatch {
        modifiers: KeyModifiers::NONE,
        key_code: KeyCode::F(5),
        action: GlobalAction::ToggleRenderMode,
    },
    KeyDispatch {
        modifiers: KeyModifiers::NONE,
        key_code: KeyCode::F(11),
        action: GlobalAction::ToggleZenMode,
    },
    // '?' may arrive with NONE or SHIFT depending on terminal
    KeyDispatch {
        modifiers: KeyModifiers::NONE,
        key_code: KeyCode::Char('?'),
        action: GlobalAction::CycleDrawer,
    },
    KeyDispatch {
        modifiers: KeyModifiers::SHIFT,
        key_code: KeyCode::Char('?'),
        action: GlobalAction::CycleDrawer,
    },
    // Timeline
    KeyDispatch {
        modifiers: KeyModifiers::NONE,
        key_code: KeyCode::Char('T'),
        action: GlobalAction::ToggleTimeline,
    },
    KeyDispatch {
        modifiers: KeyModifiers::SHIFT,
        key_code: KeyCode::Char('T'),
        action: GlobalAction::OpenTweenPanel,
    },
    // Side-panel tab cycling (Alt+arrows)
    KeyDispatch {
        modifiers: KeyModifiers::ALT,
        key_code: KeyCode::Left,
        action: GlobalAction::CycleTabPrev,
    },
    KeyDispatch {
        modifiers: KeyModifiers::ALT,
        key_code: KeyCode::Right,
        action: GlobalAction::CycleTabNext,
    },
    // Document tabs (8.5.3) — Ctrl+PageUp/PageDown, more portable than
    // Ctrl+Tab (which many terminals swallow for focus switching).
    KeyDispatch {
        modifiers: KeyModifiers::CONTROL,
        key_code: KeyCode::PageDown,
        action: GlobalAction::DocNext,
    },
    KeyDispatch {
        modifiers: KeyModifiers::CONTROL,
        key_code: KeyCode::PageUp,
        action: GlobalAction::DocPrev,
    },
    KeyDispatch {
        modifiers: KeyModifiers::CONTROL,
        key_code: KeyCode::Char('w'),
        action: GlobalAction::DocClose,
    },
    // Mode cycling
    KeyDispatch {
        modifiers: KeyModifiers::NONE,
        key_code: KeyCode::Tab,
        action: GlobalAction::NextMode,
    },
    KeyDispatch {
        modifiers: KeyModifiers::SHIFT,
        key_code: KeyCode::Tab,
        action: GlobalAction::PrevMode,
    },
    KeyDispatch {
        modifiers: KeyModifiers::CONTROL,
        key_code: KeyCode::Tab,
        action: GlobalAction::NextMode,
    },
    KeyDispatch {
        modifiers: KeyModifiers::CONTROL.union(KeyModifiers::SHIFT),
        key_code: KeyCode::Tab,
        action: GlobalAction::PrevMode,
    },
    // Quit — Esc removed; use Q / q / Ctrl+C (SIGINT)
    KeyDispatch {
        modifiers: KeyModifiers::NONE,
        key_code: KeyCode::Char('q'),
        action: GlobalAction::Quit,
    },
    KeyDispatch {
        modifiers: KeyModifiers::NONE,
        key_code: KeyCode::Char('Q'),
        action: GlobalAction::Quit,
    },
    KeyDispatch {
        modifiers: KeyModifiers::CONTROL,
        key_code: KeyCode::Char('q'),
        action: GlobalAction::Quit,
    },
];

/// Look up a global action by exact (modifiers, key_code) match.
pub fn lookup_global(code: KeyCode, modifiers: KeyModifiers) -> Option<GlobalAction> {
    GLOBAL_DISPATCH
        .iter()
        .find(|d| d.key_code == code && d.modifiers == modifiers)
        .map(|d| d.action)
}

/// Format a (modifiers, key_code) pair as a display string, e.g. "Ctrl+Shift+S".
pub fn format_shortcut(modifiers: KeyModifiers, code: KeyCode) -> String {
    let mut parts = Vec::new();
    if modifiers.contains(KeyModifiers::CONTROL) {
        parts.push("Ctrl".to_string());
    }
    if modifiers.contains(KeyModifiers::ALT) {
        parts.push("Alt".to_string());
    }
    if modifiers.contains(KeyModifiers::SHIFT) {
        parts.push("Shift".to_string());
    }
    let key_str = match code {
        KeyCode::Char(c) => c.to_ascii_uppercase().to_string(),
        KeyCode::F(n) => format!("F{n}"),
        KeyCode::Tab => "Tab".to_string(),
        KeyCode::Enter => "Enter".to_string(),
        KeyCode::Esc => "Esc".to_string(),
        KeyCode::Delete => "Delete".to_string(),
        other => format!("{other:?}"),
    };
    parts.push(key_str);
    parts.join("+")
}

/// Find the display shortcut string for a global action, derived directly
/// from [`GLOBAL_DISPATCH`] so it can't drift from the actual binding.
pub fn global_shortcut_label(action: GlobalAction) -> Option<String> {
    GLOBAL_DISPATCH
        .iter()
        .find(|d| d.action == action)
        .map(|d| format_shortcut(d.modifiers, d.key_code))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Global,
    Canvas,
    LayerPanel,
    TextTool,
    FontOverview,
    FontCharEditor,
    Dialog,
    Timeline,
    Lighting,
}

impl Scope {
    pub fn label(self) -> &'static str {
        match self {
            Scope::Global => "Global",
            Scope::Canvas => "Canvas",
            Scope::LayerPanel => "Layer Panel",
            Scope::TextTool => "Text Tool",
            Scope::FontOverview => "Font Overview",
            Scope::FontCharEditor => "Font Char Editor",
            Scope::Dialog => "Dialog",
            Scope::Timeline => "Timeline",
            Scope::Lighting => "Lighting",
        }
    }
}

pub struct KeyBinding {
    pub keys: &'static str,
    pub scope: Scope,
    pub description: &'static str,
}

pub const KEYMAP: &[KeyBinding] = &[
    // Global
    KeyBinding {
        keys: "Ctrl+N",
        scope: Scope::Global,
        description: "New image",
    },
    KeyBinding {
        keys: "Ctrl+O",
        scope: Scope::Global,
        description: "Open file",
    },
    KeyBinding {
        keys: "Ctrl+S",
        scope: Scope::Global,
        description: "Save file",
    },
    KeyBinding {
        keys: "Ctrl+Shift+S",
        scope: Scope::Global,
        description: "Save As",
    },
    KeyBinding {
        keys: "Ctrl+E",
        scope: Scope::Global,
        description: "Export",
    },
    KeyBinding {
        keys: "q / Q",
        scope: Scope::Global,
        description: "Quit",
    },
    KeyBinding {
        keys: "Tab",
        scope: Scope::Global,
        description: "Next mode",
    },
    KeyBinding {
        keys: "Ctrl+PageDown / Ctrl+PageUp",
        scope: Scope::Global,
        description: "Next / previous document tab",
    },
    KeyBinding {
        keys: "Ctrl+W",
        scope: Scope::Global,
        description: "Close document tab",
    },
    KeyBinding {
        keys: "Shift+Tab",
        scope: Scope::Global,
        description: "Prev mode",
    },
    KeyBinding {
        keys: "F5",
        scope: Scope::Global,
        description: "Toggle render mode (Fast/Dirty)",
    },
    KeyBinding {
        keys: "F11",
        scope: Scope::Global,
        description: "Toggle zen mode (full canvas)",
    },
    KeyBinding {
        keys: "?",
        scope: Scope::Global,
        description: "Toggle side panel (Layers, Props, Tools, Effects)",
    },
    KeyBinding {
        keys: "T",
        scope: Scope::Global,
        description: "Toggle animation timeline",
    },
    KeyBinding {
        keys: "Ctrl+K",
        scope: Scope::Global,
        description: "Toggle keybindings overlay",
    },
    KeyBinding {
        keys: "Ctrl+Shift+H",
        scope: Scope::Global,
        description: "Toggle undo history panel",
    },
    KeyBinding {
        keys: "Ctrl+Shift+P",
        scope: Scope::Global,
        description: "Open palette editor",
    },
    KeyBinding {
        keys: "Alt+F",
        scope: Scope::Global,
        description: "Open File menu",
    },
    KeyBinding {
        keys: "Alt+E",
        scope: Scope::Global,
        description: "Open Edit menu",
    },
    KeyBinding {
        keys: "Alt+V",
        scope: Scope::Global,
        description: "Open View menu",
    },
    KeyBinding {
        keys: "Alt+T",
        scope: Scope::Global,
        description: "Open Tools menu",
    },
    KeyBinding {
        keys: "Alt+H",
        scope: Scope::Global,
        description: "Open Help menu",
    },
    // Canvas
    KeyBinding {
        keys: "Ctrl+Z",
        scope: Scope::Canvas,
        description: "Undo",
    },
    KeyBinding {
        keys: "Ctrl+Y",
        scope: Scope::Canvas,
        description: "Redo",
    },
    KeyBinding {
        keys: "Ctrl+Shift+Z",
        scope: Scope::Canvas,
        description: "Redo (alternate)",
    },
    KeyBinding {
        keys: "+ / -",
        scope: Scope::Canvas,
        description: "Zoom in / out",
    },
    KeyBinding {
        keys: "b/e/l/v/c/p",
        scope: Scope::Canvas,
        description: "Brush/Eraser/Lasso/Select/Circle/Polygon",
    },
    KeyBinding {
        keys: "g/i/d/a/t/u/r",
        scope: Scope::Canvas,
        description: "Fill/Line/Eyedropper/Spray/Text/Move/Rotate",
    },
    KeyBinding {
        keys: "arrows (Move tool)",
        scope: Scope::Canvas,
        description: "Nudge selection contents, or whole layer if none",
    },
    KeyBinding {
        keys: "[ / ]",
        scope: Scope::Canvas,
        description: "Brush size down / up",
    },
    KeyBinding {
        keys: "; / '",
        scope: Scope::Canvas,
        description: "Brush density down / up",
    },
    KeyBinding {
        keys: r"\",
        scope: Scope::Canvas,
        description: "Cycle brush shape",
    },
    KeyBinding {
        keys: "M",
        scope: Scope::Canvas,
        description: "Toggle marker sub-mode (brush tool)",
    },
    KeyBinding {
        keys: "drag (Rotate tool)",
        scope: Scope::Canvas,
        description: "Rotate selection, or whole layer if none, 90° per drag step",
    },
    KeyBinding {
        keys: "left/right (Rotate tool)",
        scope: Scope::Canvas,
        description: "Rotate selection, or whole layer if none, one 90° step",
    },
    KeyBinding {
        keys: "Ctrl+A",
        scope: Scope::Canvas,
        description: "Select all",
    },
    KeyBinding {
        keys: "Ctrl+X",
        scope: Scope::Canvas,
        description: "Cut selection",
    },
    KeyBinding {
        keys: "Ctrl+C",
        scope: Scope::Canvas,
        description: "Copy selection",
    },
    KeyBinding {
        keys: "Ctrl+V",
        scope: Scope::Canvas,
        description: "Paste",
    },
    KeyBinding {
        keys: "Delete",
        scope: Scope::Canvas,
        description: "Delete selection",
    },
    // Font Overview
    KeyBinding {
        keys: "↑↓←→",
        scope: Scope::FontOverview,
        description: "Navigate glyph grid",
    },
    KeyBinding {
        keys: "Enter",
        scope: Scope::FontOverview,
        description: "Open glyph editor",
    },
    KeyBinding {
        keys: "Type chars",
        scope: Scope::FontOverview,
        description: "Activate search / filter glyphs",
    },
    KeyBinding {
        keys: "Esc",
        scope: Scope::FontOverview,
        description: "Clear search",
    },
    KeyBinding {
        keys: "A",
        scope: Scope::FontOverview,
        description: "Add glyph",
    },
    KeyBinding {
        keys: "D",
        scope: Scope::FontOverview,
        description: "Delete glyph",
    },
    KeyBinding {
        keys: "C",
        scope: Scope::FontOverview,
        description: "Copy glyph",
    },
    KeyBinding {
        keys: "H",
        scope: Scope::FontOverview,
        description: "Header editor",
    },
    KeyBinding {
        keys: "S",
        scope: Scope::FontOverview,
        description: "Smushing rule editor",
    },
    KeyBinding {
        keys: "T",
        scope: Scope::FontOverview,
        description: "Transform editor",
    },
    // Font Char Editor
    KeyBinding {
        keys: "↑↓←→",
        scope: Scope::FontCharEditor,
        description: "Move cursor in glyph",
    },
    KeyBinding {
        keys: "Space",
        scope: Scope::FontCharEditor,
        description: "Toggle cell",
    },
    KeyBinding {
        keys: "M",
        scope: Scope::FontCharEditor,
        description: "Mirror",
    },
    KeyBinding {
        keys: "F",
        scope: Scope::FontCharEditor,
        description: "Flip",
    },
    KeyBinding {
        keys: "G",
        scope: Scope::FontCharEditor,
        description: "Generate from system font",
    },
    // Layer Panel
    KeyBinding {
        keys: "↑ / ↓",
        scope: Scope::LayerPanel,
        description: "Select layer",
    },
    KeyBinding {
        keys: "Enter / Space",
        scope: Scope::LayerPanel,
        description: "Toggle layer visibility",
    },
    KeyBinding {
        keys: "n / N",
        scope: Scope::LayerPanel,
        description: "New layer",
    },
    KeyBinding {
        keys: "d / D",
        scope: Scope::LayerPanel,
        description: "Duplicate layer",
    },
    KeyBinding {
        keys: "x / Delete",
        scope: Scope::LayerPanel,
        description: "Delete layer",
    },
    KeyBinding {
        keys: "l",
        scope: Scope::LayerPanel,
        description: "Toggle layer lock",
    },
    KeyBinding {
        keys: "m",
        scope: Scope::LayerPanel,
        description: "Merge down (or toggle mask enabled)",
    },
    KeyBinding {
        keys: "M",
        scope: Scope::LayerPanel,
        description: "Toggle mask",
    },
    KeyBinding {
        keys: "+ / -",
        scope: Scope::LayerPanel,
        description: "Opacity up / down",
    },
    KeyBinding {
        keys: "Ctrl+G",
        scope: Scope::LayerPanel,
        description: "Group selected layer",
    },
    KeyBinding {
        keys: "k / K",
        scope: Scope::LayerPanel,
        description: "Link layer (press again on another layer to pair)",
    },
    KeyBinding {
        keys: "F2",
        scope: Scope::LayerPanel,
        description: "Rename layer",
    },
    // Text Tool
    KeyBinding {
        keys: "t",
        scope: Scope::TextTool,
        description: "Activate text tool",
    },
    KeyBinding {
        keys: "↑ / ↓",
        scope: Scope::TextTool,
        description: "Previous / next font",
    },
    KeyBinding {
        keys: "Enter",
        scope: Scope::TextTool,
        description: "Commit text block to canvas",
    },
    KeyBinding {
        keys: "Esc",
        scope: Scope::TextTool,
        description: "Cancel / clear text buffer",
    },
    KeyBinding {
        keys: "[ / ]",
        scope: Scope::TextTool,
        description: "Scale text down / up",
    },
    // Timeline
    KeyBinding {
        keys: "T",
        scope: Scope::Global,
        description: "Toggle timeline panel",
    },
    KeyBinding {
        keys: "Shift+T",
        scope: Scope::Global,
        description: "Open tween panel",
    },
    KeyBinding {
        keys: "← / →",
        scope: Scope::Timeline,
        description: "Switch frame",
    },
    // Side-panel chrome
    KeyBinding {
        keys: "Alt+← / Alt+→",
        scope: Scope::Global,
        description: "Cycle side panel tabs",
    },
    // Layer panel (Alt-gated)
    KeyBinding {
        keys: "Alt+↑ / Alt+↓",
        scope: Scope::LayerPanel,
        description: "Select layer",
    },
    KeyBinding {
        keys: "Alt+Shift+↑ / Alt+Shift+↓",
        scope: Scope::LayerPanel,
        description: "Reorder layer",
    },
    KeyBinding {
        keys: "Alt+← / Alt+→",
        scope: Scope::LayerPanel,
        description: "Collapse / expand layer group",
    },
    KeyBinding {
        keys: "Alt+S",
        scope: Scope::LayerPanel,
        description: "Toggle layer cast shadow",
    },
    KeyBinding {
        keys: "Alt+Tab",
        scope: Scope::LayerPanel,
        description: "Cycle through group layers",
    },
    // Settings
    KeyBinding {
        keys: "S",
        scope: Scope::Global,
        description: "Open settings dialog",
    },
    // Lighting mode
    KeyBinding {
        keys: "G",
        scope: Scope::Lighting,
        description: "Enter lighting mode",
    },
    KeyBinding {
        keys: "Esc",
        scope: Scope::Lighting,
        description: "Exit lighting mode",
    },
    KeyBinding {
        keys: "↑ / ↓",
        scope: Scope::Lighting,
        description: "Select previous / next light",
    },
    KeyBinding {
        keys: "← / →",
        scope: Scope::Lighting,
        description: "Move point light horizontally",
    },
    KeyBinding {
        keys: "Shift+↑ / Shift+↓",
        scope: Scope::Lighting,
        description: "Move point light vertically",
    },
    KeyBinding {
        keys: "+ / -",
        scope: Scope::Lighting,
        description: "Adjust selected light intensity",
    },
    KeyBinding {
        keys: "A / D / P",
        scope: Scope::Lighting,
        description: "Add ambient / directional / point light",
    },
    KeyBinding {
        keys: "Delete",
        scope: Scope::Lighting,
        description: "Remove selected light",
    },
    KeyBinding {
        keys: "A",
        scope: Scope::Timeline,
        description: "Add frame",
    },
    KeyBinding {
        keys: "Delete",
        scope: Scope::Timeline,
        description: "Delete frame",
    },
    KeyBinding {
        keys: "Enter",
        scope: Scope::Timeline,
        description: "Play animation",
    },
    // Dialog
    KeyBinding {
        keys: "Esc",
        scope: Scope::Dialog,
        description: "Close / cancel",
    },
    KeyBinding {
        keys: "Enter",
        scope: Scope::Dialog,
        description: "Confirm",
    },
    KeyBinding {
        keys: "↑↓",
        scope: Scope::Dialog,
        description: "Navigate items",
    },
];
