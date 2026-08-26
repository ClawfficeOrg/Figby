use ratatui::style::Color;
use serde::Deserialize;

const DEFAULT_THEME_YAML: &str = include_str!("../../assets/themes/default.yaml");

/// Parse an RGB triple from a hex color string (`#RGB` or `#RRGGBB`).
///
/// ASCII-validates before any slicing, so non-ASCII input (e.g. `"éx"`)
/// returns `None` instead of panicking on an interior byte boundary
/// (GPT review F-17).
pub fn parse_hex_rgb(hex: &str) -> Option<[u8; 3]> {
    let hex = hex.trim().trim_start_matches('#');
    if hex.is_empty() || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    match hex.len() {
        6 => Some([
            u8::from_str_radix(&hex[0..2], 16).ok()?,
            u8::from_str_radix(&hex[2..4], 16).ok()?,
            u8::from_str_radix(&hex[4..6], 16).ok()?,
        ]),
        3 => {
            let bytes = hex.as_bytes();
            let expand = |c: u8| -> Option<u8> {
                let pair = [c, c];
                let s = core::str::from_utf8(&pair).ok()?;
                u8::from_str_radix(s, 16).ok()
            };
            Some([expand(bytes[0])?, expand(bytes[1])?, expand(bytes[2])?])
        }
        _ => None,
    }
}

pub fn color_from_hex(hex: &str) -> Color {
    parse_hex_rgb(hex)
        .map(|[r, g, b]| Color::Rgb(r, g, b))
        .unwrap_or(Color::Reset)
}

#[derive(Debug, Clone)]
pub struct LayerTheme {
    pub bg: Color,
    pub fg: Color,
    pub active_bg: Color,
    pub border: Color,
}

impl Default for LayerTheme {
    fn default() -> Self {
        Self {
            bg: Color::Rgb(0x0d, 0x0d, 0x1a),
            fg: Color::Rgb(0xd4, 0xd4, 0xe8),
            active_bg: Color::Rgb(0x00, 0x34, 0x4d),
            border: Color::Rgb(0x1a, 0x1a, 0x2e),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Theme {
    pub toolbox: ToolboxTheme,
    pub canvas: CanvasTheme,
    pub palette: PaletteTheme,
    pub statusbar: StatusBarTheme,
    pub menu: MenuTheme,
    pub dialog: DialogTheme,
    pub general: GeneralTheme,
    pub layers: LayerTheme,
}

#[derive(Debug, Clone)]
pub struct ToolboxTheme {
    pub bg: Color,
    pub fg: Color,
    pub selected: Color,
}

#[derive(Debug, Clone)]
pub struct CanvasTheme {
    pub grid: Color,
    pub cursor: Color,
    pub selection: Color,
    pub edge: Color,
    pub text_block: Color,
    pub border: Color,
}

#[derive(Debug, Clone)]
pub struct PaletteTheme {
    pub border: Color,
    pub active_target: Color,
    pub swatch_indicator: Color,
    pub cell_bg: Color,
}

#[derive(Debug, Clone)]
pub struct StatusBarTheme {
    pub mode_font: Color,
    pub mode_image: Color,
    pub mode_ascii: Color,
    pub mode_lighting: Color,
    pub separator: Color,
    pub label: Color,
    pub git_branch: Color,
    pub font_name: Color,
    pub fps: Color,
    pub glyph_count: Color,
    pub unsaved: Color,
    pub saved: Color,
}

#[derive(Debug, Clone)]
pub struct MenuTheme {
    pub bg: Color,
    pub fg: Color,
    pub highlight: Color,
    pub dim: Color,
    pub dropdown_bg: Color,
}

#[derive(Debug, Clone)]
pub struct DialogTheme {
    pub border_success: Color,
    pub border_path: Color,
    pub label: Color,
    pub meta: Color,
    pub error: Color,
    pub highlight: Color,
    pub selected_bg: Color,
}

#[derive(Debug, Clone)]
pub struct GeneralTheme {
    pub primary: Color,
    pub secondary: Color,
    pub success: Color,
    pub error: Color,
    pub warning: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            toolbox: ToolboxTheme {
                bg: Color::Rgb(0x0d, 0x0d, 0x1a),
                fg: Color::Rgb(0xd4, 0xd4, 0xe8),
                selected: Color::Rgb(0x00, 0xd4, 0xff),
            },
            canvas: CanvasTheme {
                grid: Color::Rgb(0x1a, 0x1a, 0x2e),
                cursor: Color::Rgb(0x00, 0xd4, 0xff),
                selection: Color::Rgb(0x00, 0x34, 0x4d),
                edge: Color::Rgb(0x1a, 0x1a, 0x2e),
                text_block: Color::Rgb(0xff, 0x00, 0x99),
                border: Color::Rgb(0x00, 0xd4, 0xff),
            },
            palette: PaletteTheme {
                border: Color::Rgb(0x1a, 0x1a, 0x2e),
                active_target: Color::Rgb(0x00, 0xd4, 0xff),
                swatch_indicator: Color::Rgb(0xd4, 0xd4, 0xe8),
                cell_bg: Color::Rgb(0x12, 0x12, 0x1f),
            },
            statusbar: StatusBarTheme {
                mode_font: Color::Rgb(0x00, 0xd4, 0xff),
                mode_image: Color::Rgb(0xff, 0x00, 0x99),
                mode_ascii: Color::Rgb(0xff, 0xaa, 0x00),
                mode_lighting: Color::Rgb(0xff, 0xaa, 0x00),
                separator: Color::Rgb(0x55, 0x55, 0x77),
                label: Color::Rgb(0x88, 0x88, 0xaa),
                git_branch: Color::Rgb(0x55, 0x55, 0x77),
                font_name: Color::Rgb(0x88, 0x88, 0xaa),
                fps: Color::Rgb(0x88, 0x88, 0xaa),
                glyph_count: Color::Rgb(0x88, 0x88, 0xaa),
                unsaved: Color::Rgb(0xff, 0xaa, 0x00),
                saved: Color::Rgb(0x00, 0xff, 0x87),
            },
            menu: MenuTheme {
                bg: Color::Rgb(0x0d, 0x0d, 0x1a),
                fg: Color::Rgb(0xd4, 0xd4, 0xe8),
                highlight: Color::Rgb(0x00, 0xd4, 0xff),
                dim: Color::Rgb(0x55, 0x55, 0x77),
                dropdown_bg: Color::Rgb(0x12, 0x12, 0x1f),
            },
            dialog: DialogTheme {
                border_success: Color::Rgb(0x00, 0xff, 0x87),
                border_path: Color::Rgb(0x00, 0xd4, 0xff),
                label: Color::Rgb(0xd4, 0xd4, 0xe8),
                meta: Color::Rgb(0x88, 0x88, 0xaa),
                error: Color::Rgb(0xff, 0x00, 0x44),
                highlight: Color::Rgb(0xff, 0x00, 0x99),
                selected_bg: Color::Rgb(0x00, 0x34, 0x4d),
            },
            general: GeneralTheme {
                primary: Color::Rgb(0x00, 0xd4, 0xff),
                secondary: Color::Rgb(0xff, 0x00, 0x99),
                success: Color::Rgb(0x00, 0xff, 0x87),
                error: Color::Rgb(0xff, 0x00, 0x44),
                warning: Color::Rgb(0xff, 0xaa, 0x00),
            },
            layers: LayerTheme::default(),
        }
    }
}

// Intermediate YAML structs for serde deserialization
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct LayerYaml {
    bg: Option<String>,
    fg: Option<String>,
    active_bg: Option<String>,
    border: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ThemeYaml {
    toolbox: Option<ToolboxYaml>,
    canvas: Option<CanvasYaml>,
    palette: Option<PaletteYaml>,
    statusbar: Option<StatusBarYaml>,
    menu: Option<MenuYaml>,
    dialog: Option<DialogYaml>,
    general: Option<GeneralYaml>,
    layers: Option<LayerYaml>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ToolboxYaml {
    bg: Option<String>,
    fg: Option<String>,
    selected: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct CanvasYaml {
    grid: Option<String>,
    cursor: Option<String>,
    selection: Option<String>,
    edge: Option<String>,
    text_block: Option<String>,
    border: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct PaletteYaml {
    border: Option<String>,
    active_target: Option<String>,
    swatch_indicator: Option<String>,
    cell_bg: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct StatusBarYaml {
    mode_font: Option<String>,
    mode_image: Option<String>,
    mode_ascii: Option<String>,
    mode_lighting: Option<String>,
    separator: Option<String>,
    label: Option<String>,
    git_branch: Option<String>,
    font_name: Option<String>,
    fps: Option<String>,
    glyph_count: Option<String>,
    unsaved: Option<String>,
    saved: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct MenuYaml {
    bg: Option<String>,
    fg: Option<String>,
    highlight: Option<String>,
    dim: Option<String>,
    dropdown_bg: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct DialogYaml {
    border_success: Option<String>,
    border_path: Option<String>,
    label: Option<String>,
    meta: Option<String>,
    error: Option<String>,
    highlight: Option<String>,
    selected_bg: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct GeneralYaml {
    primary: Option<String>,
    secondary: Option<String>,
    success: Option<String>,
    error: Option<String>,
    warning: Option<String>,
}

fn merge_color(yaml: Option<&String>, default: Color) -> Color {
    match yaml {
        Some(h) => color_from_hex(h),
        None => default,
    }
}

impl From<ThemeYaml> for Theme {
    fn from(y: ThemeYaml) -> Self {
        let base = Theme::default();
        Self {
            toolbox: ToolboxTheme {
                bg: merge_color(
                    y.toolbox.as_ref().and_then(|t| t.bg.as_ref()),
                    base.toolbox.bg,
                ),
                fg: merge_color(
                    y.toolbox.as_ref().and_then(|t| t.fg.as_ref()),
                    base.toolbox.fg,
                ),
                selected: merge_color(
                    y.toolbox.as_ref().and_then(|t| t.selected.as_ref()),
                    base.toolbox.selected,
                ),
            },
            canvas: CanvasTheme {
                grid: merge_color(
                    y.canvas.as_ref().and_then(|t| t.grid.as_ref()),
                    base.canvas.grid,
                ),
                cursor: merge_color(
                    y.canvas.as_ref().and_then(|t| t.cursor.as_ref()),
                    base.canvas.cursor,
                ),
                selection: merge_color(
                    y.canvas.as_ref().and_then(|t| t.selection.as_ref()),
                    base.canvas.selection,
                ),
                edge: merge_color(
                    y.canvas.as_ref().and_then(|t| t.edge.as_ref()),
                    base.canvas.edge,
                ),
                text_block: merge_color(
                    y.canvas.as_ref().and_then(|t| t.text_block.as_ref()),
                    base.canvas.text_block,
                ),
                border: merge_color(
                    y.canvas.as_ref().and_then(|t| t.border.as_ref()),
                    base.canvas.border,
                ),
            },
            palette: PaletteTheme {
                border: merge_color(
                    y.palette.as_ref().and_then(|t| t.border.as_ref()),
                    base.palette.border,
                ),
                active_target: merge_color(
                    y.palette.as_ref().and_then(|t| t.active_target.as_ref()),
                    base.palette.active_target,
                ),
                swatch_indicator: merge_color(
                    y.palette.as_ref().and_then(|t| t.swatch_indicator.as_ref()),
                    base.palette.swatch_indicator,
                ),
                cell_bg: merge_color(
                    y.palette.as_ref().and_then(|t| t.cell_bg.as_ref()),
                    base.palette.cell_bg,
                ),
            },
            statusbar: StatusBarTheme {
                mode_font: merge_color(
                    y.statusbar.as_ref().and_then(|t| t.mode_font.as_ref()),
                    base.statusbar.mode_font,
                ),
                mode_image: merge_color(
                    y.statusbar.as_ref().and_then(|t| t.mode_image.as_ref()),
                    base.statusbar.mode_image,
                ),
                mode_ascii: merge_color(
                    y.statusbar.as_ref().and_then(|t| t.mode_ascii.as_ref()),
                    base.statusbar.mode_ascii,
                ),
                mode_lighting: merge_color(
                    y.statusbar.as_ref().and_then(|t| t.mode_lighting.as_ref()),
                    base.statusbar.mode_lighting,
                ),
                separator: merge_color(
                    y.statusbar.as_ref().and_then(|t| t.separator.as_ref()),
                    base.statusbar.separator,
                ),
                label: merge_color(
                    y.statusbar.as_ref().and_then(|t| t.label.as_ref()),
                    base.statusbar.label,
                ),
                git_branch: merge_color(
                    y.statusbar.as_ref().and_then(|t| t.git_branch.as_ref()),
                    base.statusbar.git_branch,
                ),
                font_name: merge_color(
                    y.statusbar.as_ref().and_then(|t| t.font_name.as_ref()),
                    base.statusbar.font_name,
                ),
                fps: merge_color(
                    y.statusbar.as_ref().and_then(|t| t.fps.as_ref()),
                    base.statusbar.fps,
                ),
                glyph_count: merge_color(
                    y.statusbar.as_ref().and_then(|t| t.glyph_count.as_ref()),
                    base.statusbar.glyph_count,
                ),
                unsaved: merge_color(
                    y.statusbar.as_ref().and_then(|t| t.unsaved.as_ref()),
                    base.statusbar.unsaved,
                ),
                saved: merge_color(
                    y.statusbar.as_ref().and_then(|t| t.saved.as_ref()),
                    base.statusbar.saved,
                ),
            },
            menu: MenuTheme {
                bg: merge_color(y.menu.as_ref().and_then(|t| t.bg.as_ref()), base.menu.bg),
                fg: merge_color(y.menu.as_ref().and_then(|t| t.fg.as_ref()), base.menu.fg),
                highlight: merge_color(
                    y.menu.as_ref().and_then(|t| t.highlight.as_ref()),
                    base.menu.highlight,
                ),
                dim: merge_color(y.menu.as_ref().and_then(|t| t.dim.as_ref()), base.menu.dim),
                dropdown_bg: merge_color(
                    y.menu.as_ref().and_then(|t| t.dropdown_bg.as_ref()),
                    base.menu.dropdown_bg,
                ),
            },
            dialog: DialogTheme {
                border_success: merge_color(
                    y.dialog.as_ref().and_then(|t| t.border_success.as_ref()),
                    base.dialog.border_success,
                ),
                border_path: merge_color(
                    y.dialog.as_ref().and_then(|t| t.border_path.as_ref()),
                    base.dialog.border_path,
                ),
                label: merge_color(
                    y.dialog.as_ref().and_then(|t| t.label.as_ref()),
                    base.dialog.label,
                ),
                meta: merge_color(
                    y.dialog.as_ref().and_then(|t| t.meta.as_ref()),
                    base.dialog.meta,
                ),
                error: merge_color(
                    y.dialog.as_ref().and_then(|t| t.error.as_ref()),
                    base.dialog.error,
                ),
                highlight: merge_color(
                    y.dialog.as_ref().and_then(|t| t.highlight.as_ref()),
                    base.dialog.highlight,
                ),
                selected_bg: merge_color(
                    y.dialog.as_ref().and_then(|t| t.selected_bg.as_ref()),
                    base.dialog.selected_bg,
                ),
            },
            general: GeneralTheme {
                primary: merge_color(
                    y.general.as_ref().and_then(|t| t.primary.as_ref()),
                    base.general.primary,
                ),
                secondary: merge_color(
                    y.general.as_ref().and_then(|t| t.secondary.as_ref()),
                    base.general.secondary,
                ),
                success: merge_color(
                    y.general.as_ref().and_then(|t| t.success.as_ref()),
                    base.general.success,
                ),
                error: merge_color(
                    y.general.as_ref().and_then(|t| t.error.as_ref()),
                    base.general.error,
                ),
                warning: merge_color(
                    y.general.as_ref().and_then(|t| t.warning.as_ref()),
                    base.general.warning,
                ),
            },
            layers: LayerTheme {
                bg: merge_color(
                    y.layers.as_ref().and_then(|t| t.bg.as_ref()),
                    base.layers.bg,
                ),
                fg: merge_color(
                    y.layers.as_ref().and_then(|t| t.fg.as_ref()),
                    base.layers.fg,
                ),
                active_bg: merge_color(
                    y.layers.as_ref().and_then(|t| t.active_bg.as_ref()),
                    base.layers.active_bg,
                ),
                border: merge_color(
                    y.layers.as_ref().and_then(|t| t.border.as_ref()),
                    base.layers.border,
                ),
            },
        }
    }
}

pub fn load_default() -> Theme {
    let yaml: ThemeYaml = serde_yaml::from_str(DEFAULT_THEME_YAML).unwrap_or_default();
    Theme::from(yaml)
}

pub fn load_custom(path: &str) -> Theme {
    let content = match crate::bounded_io::read_bounded_string(
        std::path::Path::new(path),
        crate::bounded_io::MAX_TEXT_BYTES,
    ) {
        Ok(c) => c,
        Err(_) => return load_default(),
    };
    let yaml: Result<ThemeYaml, _> = serde_yaml::from_str(&content);
    match yaml {
        Ok(y) => Theme::from(y),
        Err(_) => load_default(),
    }
}

pub fn load_theme(theme_opt: &Option<String>) -> Theme {
    match theme_opt.as_deref() {
        None | Some("default") => load_default(),
        Some(path) => load_custom(path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_load_default() {
        let theme = load_default();
        assert_ne!(theme.toolbox.bg, Color::Reset, "toolbox.bg should be set");
        assert_ne!(theme.canvas.grid, Color::Reset, "canvas.grid should be set");
        assert_ne!(
            theme.canvas.border,
            Color::Reset,
            "canvas.border should be set"
        );
        assert_ne!(
            theme.statusbar.mode_font,
            Color::Reset,
            "statusbar.mode_font should be set"
        );
        assert_ne!(
            theme.statusbar.mode_image,
            Color::Reset,
            "statusbar.mode_image should be set"
        );
        assert_ne!(
            theme.statusbar.mode_ascii,
            Color::Reset,
            "statusbar.mode_ascii should be set"
        );
        assert_ne!(
            theme.statusbar.mode_lighting,
            Color::Reset,
            "statusbar.mode_lighting should be set"
        );
        // Verify parsed to Rgb, not Reset
        if let Color::Rgb(r, g, b) = theme.toolbox.bg {
            assert_eq!(r, 0x0d);
            assert_eq!(g, 0x0d);
            assert_eq!(b, 0x1a);
        } else {
            panic!("toolbox.bg should be Color::Rgb");
        }
        if let Color::Rgb(r, g, b) = theme.statusbar.mode_font {
            assert_eq!(r, 0x00);
            assert_eq!(g, 0xd4);
            assert_eq!(b, 0xff);
        } else {
            panic!("statusbar.mode_font should be Color::Rgb");
        }
        // Verify non-overridden defaults remain
        assert_eq!(theme.general.primary, Color::Rgb(0x00, 0xd4, 0xff));
        assert_eq!(theme.general.error, Color::Rgb(0xff, 0x00, 0x44));
    }

    #[test]
    fn test_theme_load_custom() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_theme_custom.yaml");
        let yaml = r##"
toolbox:
  bg: "#ff0000"
canvas:
  grid: "#00ff00"
statusbar:
  mode_font: "#0000ff"
"##;
        std::fs::write(&path, yaml).expect("write test theme");
        let theme = load_custom(path.to_str().unwrap());
        assert_eq!(theme.toolbox.bg, Color::Rgb(255, 0, 0));
        assert_eq!(theme.canvas.grid, Color::Rgb(0, 255, 0));
        assert_eq!(theme.statusbar.mode_font, Color::Rgb(0, 0, 255));
        // Non-overridden fields should still be at loaded values
        assert_ne!(theme.statusbar.mode_image, Color::Reset);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_theme_invalid_path() {
        let theme = load_custom("/nonexistent/path/theme.yaml");
        assert_ne!(
            theme.toolbox.bg,
            Color::Reset,
            "should fall back to default"
        );
    }

    #[test]
    fn test_theme_invalid_yaml() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_theme_bad.yaml");
        std::fs::write(&path, b"<<<garbage>>>").expect("write bad theme");
        let theme = load_custom(path.to_str().unwrap());
        assert_ne!(
            theme.toolbox.bg,
            Color::Reset,
            "should fall back to default"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_load_theme_dispatcher() {
        let theme = load_theme(&None);
        assert_ne!(theme.toolbox.bg, Color::Reset);
        let theme = load_theme(&Some("default".to_string()));
        assert_ne!(theme.toolbox.bg, Color::Reset);
        let theme = load_theme(&Some("/nonexistent/theme.yaml".to_string()));
        assert_ne!(theme.toolbox.bg, Color::Reset, "fallback on bad path");
    }

    #[test]
    fn test_hex_parsing() {
        assert_eq!(color_from_hex("#ff0000"), Color::Rgb(255, 0, 0));
        assert_eq!(color_from_hex("#aabbcc"), Color::Rgb(170, 187, 204));
        assert_eq!(color_from_hex("#000000"), Color::Rgb(0, 0, 0));
        assert_eq!(color_from_hex(""), Color::Reset);
        assert_eq!(color_from_hex("nothex"), Color::Reset);
    }
}

#[cfg(test)]
mod theme_tests {
    use super::*;

    /// F-17 (GPT review): non-ASCII input must return None, not panic on
    /// a non-char-boundary byte slice.
    #[test]
    fn test_parse_hex_rgb_multibyte_no_panic() {
        assert_eq!(parse_hex_rgb("éx"), None);
        assert_eq!(parse_hex_rgb("#日本語"), None);
        assert_eq!(parse_hex_rgb("gggggg"), None);
        assert_eq!(parse_hex_rgb("#AABBCC"), Some([0xAA, 0xBB, 0xCC]));
        assert_eq!(parse_hex_rgb("#abc"), Some([0xAA, 0xBB, 0xCC]));
        assert_eq!(color_from_hex("éx"), Color::Reset);
    }
}
