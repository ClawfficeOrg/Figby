use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct FigbyConfig {
    #[serde(default)]
    pub cli: CliSection,
    #[serde(default)]
    pub tui: TuiSection,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CliSection {
    pub font: Option<String>,
    pub output_width: Option<u32>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct TuiSection {
    pub theme: Option<String>,
    pub recent_files_max: Option<usize>,
    pub undo_limit: Option<usize>,
    pub render_mode: Option<String>,
    /// `"nerd"` (default) or `"ascii"`; `FIGBY_ICONS` overrides it.
    pub icons: Option<String>,
    #[serde(default)]
    pub brush: BrushSection,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct BrushSection {
    pub shape: Option<String>,
    pub size: Option<u8>,
    pub density: Option<u8>,
    pub ch: Option<String>,
}

fn config_file_path() -> Option<PathBuf> {
    let base = if let Ok(val) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(val)
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".config")
    } else {
        return None;
    };
    Some(base.join("figby").join("config.toml"))
}

/// Serialises tests that set `XDG_CONFIG_HOME`; the env is process-global
/// and cargo runs tests in parallel.
#[cfg(test)]
pub(crate) fn xdg_env_lock() -> &'static std::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
}

pub fn config_dir() -> Option<PathBuf> {
    config_file_path().and_then(|p| p.parent().map(|p| p.to_path_buf()))
}

pub fn load_config() -> FigbyConfig {
    let path = match config_file_path() {
        Some(p) => p,
        None => return FigbyConfig::default(),
    };
    let content =
        match crate::bounded_io::read_bounded_string(&path, crate::bounded_io::MAX_TEXT_BYTES) {
            Ok(c) => c,
            // Oversized or invalid config: fall back to defaults (GPT review F-21).
            Err(_) => return FigbyConfig::default(),
        };
    toml::from_str(&content).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_parse_full() {
        let toml_str = r##"
[cli]
font = "big"
output_width = 100
color_mode = "always"

[tui]
theme = "dark"
recent_files_max = 20

[tui.brush]
shape = "circle"
size = 5
density = 50
ch = "#"
"##;
        let config: FigbyConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.cli.font.as_deref(), Some("big"));
        assert_eq!(config.cli.output_width, Some(100));
        assert_eq!(config.tui.theme.as_deref(), Some("dark"));
        assert_eq!(config.tui.recent_files_max, Some(20));
        assert_eq!(config.tui.undo_limit, None);
        assert_eq!(config.tui.brush.shape.as_deref(), Some("circle"));
        assert_eq!(config.tui.brush.size, Some(5));
        assert_eq!(config.tui.brush.density, Some(50));
        assert_eq!(config.tui.brush.ch.as_deref(), Some("#"));
    }

    #[test]
    fn test_config_parse_partial_cli_only() {
        let toml_str = r#"
[cli]
font = "big"
"#;
        let config: FigbyConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.cli.font.as_deref(), Some("big"));
        assert_eq!(config.cli.output_width, None);
        assert_eq!(config.tui.theme, None);
        assert_eq!(config.tui.brush.shape, None);
    }

    #[test]
    fn test_config_parse_partial_tui_brush_only() {
        let toml_str = r#"
[tui.brush]
shape = "circle"
size = 5
"#;
        let config: FigbyConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.cli.font, None);
        assert_eq!(config.tui.brush.shape.as_deref(), Some("circle"));
        assert_eq!(config.tui.brush.size, Some(5));
        assert_eq!(config.tui.brush.density, None);
    }

    #[test]
    fn test_config_undo_limit() {
        let toml_str = r#"
[tui]
undo_limit = 100
"#;
        let config: FigbyConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.tui.undo_limit, Some(100));
    }

    #[test]
    fn test_config_defaults_on_empty_toml() {
        let config: FigbyConfig = toml::from_str("").unwrap();
        assert_eq!(config.cli.font, None);
        assert_eq!(config.cli.output_width, None);
        assert_eq!(config.tui.theme, None);
        assert_eq!(config.tui.brush.size, None);
    }

    /// `cli.color_mode` and `[tui.particles]` were parsed but never used,
    /// so they were removed (review A3). Old config files that still set
    /// them must keep loading, with the rest of their settings intact.
    #[test]
    fn test_config_removed_keys_still_parse() {
        let toml_str = r#"
[cli]
font = "big"
color_mode = "always"

[tui]
theme = "dark"

[tui.particles]
spawn_rate = 4.0
character = "*"
"#;
        let config: FigbyConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.cli.font.as_deref(), Some("big"));
        assert_eq!(config.tui.theme.as_deref(), Some("dark"));
    }

    #[test]
    fn test_config_defaults_on_missing_file() {
        // load_config should return defaults when file doesn't exist
        let config = load_config();
        // Should not panic, returns default
        assert_eq!(config.cli.font, None);
    }

    #[test]
    fn test_config_defaults_on_bad_toml() {
        let dir = std::env::temp_dir().join("figby-test-bad-config");
        let figby_dir = dir.join("figby");
        let _ = std::fs::create_dir_all(&figby_dir);
        let path = figby_dir.join("config.toml");
        std::fs::write(&path, "<<<bad toml>>>").ok();

        let _lock = xdg_env_lock().lock().unwrap_or_else(|e| e.into_inner());
        let orig = std::env::var("XDG_CONFIG_HOME").ok();
        std::env::set_var("XDG_CONFIG_HOME", dir.to_str().unwrap());
        let config = load_config();
        assert_eq!(config.cli.font, None);

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir_all(&dir);
        match orig {
            Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
    }
}
