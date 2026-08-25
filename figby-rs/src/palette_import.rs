use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Swatch {
    pub name: String,
    pub hex: String,
    #[serde(default)]
    pub lit_hex: Option<String>,
    #[serde(default)]
    pub shadow_hex: Option<String>,
    #[serde(default)]
    pub specular: Option<bool>,
    #[serde(default)]
    pub shininess: Option<f32>,
}

impl Swatch {
    pub fn new(name: String, hex: String) -> Self {
        Swatch {
            name,
            hex,
            lit_hex: None,
            shadow_hex: None,
            specular: None,
            shininess: None,
        }
    }

    /// Compute default shadow color as fg * 0.3
    pub fn default_shadow_hex(hex: &str) -> String {
        let [r, g, b] = match crate::tui::theme::parse_hex_rgb(hex) {
            Some(rgb) => rgb,
            None => return "#000000".to_string(),
        };
        let shadow_r = (r as f32 * 0.3).round() as u8;
        let shadow_g = (g as f32 * 0.3).round() as u8;
        let shadow_b = (b as f32 * 0.3).round() as u8;
        format!("#{:02X}{:02X}{:02X}", shadow_r, shadow_g, shadow_b)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ImportFormat {
    PalettyJson,
    AdobeAse,
    WezTermJson,
    WindowsTerminalJson,
    Native,
}

impl ImportFormat {
    pub fn display_name(&self) -> &'static str {
        match self {
            ImportFormat::PalettyJson => "Paletty JSON",
            ImportFormat::AdobeAse => "Adobe ASE",
            ImportFormat::WezTermJson => "WezTerm JSON",
            ImportFormat::WindowsTerminalJson => "Windows Terminal JSON",
            ImportFormat::Native => "Native Figby",
        }
    }

    pub fn all() -> &'static [ImportFormat] {
        &[
            ImportFormat::PalettyJson,
            ImportFormat::AdobeAse,
            ImportFormat::WezTermJson,
            ImportFormat::WindowsTerminalJson,
            ImportFormat::Native,
        ]
    }
}

pub fn auto_detect_format(content: &[u8], ext: Option<&str>) -> Option<ImportFormat> {
    if content.len() >= 4 && &content[0..4] == b"ASEF" {
        return Some(ImportFormat::AdobeAse);
    }
    if let Some(ext) = ext {
        let lower = ext.to_ascii_lowercase();
        if lower == "ase" {
            return Some(ImportFormat::AdobeAse);
        }
    }
    let s = std::str::from_utf8(content).ok()?;
    let value: serde_json::Value = serde_json::from_str(s).ok()?;
    match &value {
        serde_json::Value::Array(_) => Some(ImportFormat::PalettyJson),
        serde_json::Value::Object(map) => {
            if map.contains_key("colors") {
                Some(ImportFormat::WezTermJson)
            } else if map.contains_key("schemes") {
                Some(ImportFormat::WindowsTerminalJson)
            } else if map.contains_key("name") && map.contains_key("swatches") {
                Some(ImportFormat::Native)
            } else {
                None
            }
        }
        _ => None,
    }
}

pub fn import_swatches(content: &[u8], format: ImportFormat) -> Result<Vec<Swatch>, String> {
    match format {
        ImportFormat::PalettyJson => {
            let s = std::str::from_utf8(content).map_err(|e| format!("UTF-8 error: {e}"))?;
            parse_paletty_json(s)
        }
        ImportFormat::AdobeAse => parse_ase(content),
        ImportFormat::WezTermJson => {
            let s = std::str::from_utf8(content).map_err(|e| format!("UTF-8 error: {e}"))?;
            parse_wezterm(s)
        }
        ImportFormat::WindowsTerminalJson => {
            let s = std::str::from_utf8(content).map_err(|e| format!("UTF-8 error: {e}"))?;
            parse_windows_terminal(s)
        }
        ImportFormat::Native => {
            Err("Native format should be handled by palette_editor directly".to_string())
        }
    }
}

fn normalize_hex(hex: &str) -> String {
    let hex = hex.trim().trim_start_matches('#');
    match crate::tui::theme::parse_hex_rgb(hex) {
        Some([r, g, b]) => format!("#{r:02X}{g:02X}{b:02X}"),
        None => "#000000".to_string(),
    }
}

fn parse_paletty_json(s: &str) -> Result<Vec<Swatch>, String> {
    let entries: Vec<serde_json::Value> =
        serde_json::from_str(s).map_err(|e| format!("Paletty JSON error: {e}"))?;
    let swatches: Vec<Swatch> = entries
        .into_iter()
        .map(|entry| {
            let hex = entry
                .get("hex")
                .and_then(|v| v.as_str())
                .unwrap_or("000000");
            let name = entry
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            Swatch::new(name, normalize_hex(hex))
        })
        .collect();
    if swatches.is_empty() {
        return Err("Paletty JSON contains no swatches".to_string());
    }
    Ok(swatches)
}

fn parse_ase(buf: &[u8]) -> Result<Vec<Swatch>, String> {
    if buf.len() < 12 {
        return Err("ASE file too short".to_string());
    }
    if &buf[0..4] != b"ASEF" {
        return Err("Invalid ASE magic number".to_string());
    }
    let _major = u16::from_be_bytes([buf[4], buf[5]]);
    let _minor = u16::from_be_bytes([buf[6], buf[7]]);
    let block_count = u32::from_be_bytes([buf[8], buf[9], buf[10], buf[11]]);
    let mut offset = 12usize;
    let mut swatches = Vec::new();
    let max_blocks = block_count.min(10000) as usize;
    for _ in 0..max_blocks {
        if offset + 6 > buf.len() {
            break;
        }
        let block_type = u16::from_be_bytes([buf[offset], buf[offset + 1]]);
        let block_length = u32::from_be_bytes([
            buf[offset + 2],
            buf[offset + 3],
            buf[offset + 4],
            buf[offset + 5],
        ]);
        offset += 6;
        let block_end = offset
            .checked_add(block_length as usize)
            .ok_or("Overflow in ASE block")?;
        if block_end > buf.len() {
            return Err("ASE block extends beyond file".to_string());
        }
        if block_type == 0x0001 || block_type == 0xC001 {
            if block_length < 8 {
                offset = block_end;
                continue;
            }
            let name_len = u16::from_be_bytes([buf[offset], buf[offset + 1]]);
            let name_bytes_len = name_len as usize * 2;
            if offset + 2 + name_bytes_len + 4 > block_end {
                offset = block_end;
                continue;
            }
            let name = if name_len > 0 {
                let name_data = &buf[offset + 2..offset + 2 + name_bytes_len];
                let utf16: Vec<u16> = name_data
                    .chunks(2)
                    .filter(|c| c.len() == 2)
                    .map(|c| u16::from_be_bytes([c[0], c[1]]))
                    .take_while(|&c| c != 0)
                    .collect();
                String::from_utf16(&utf16).unwrap_or_default()
            } else {
                String::new()
            };
            let color_data_start = offset + 2 + name_bytes_len;
            if color_data_start + 4 > block_end {
                offset = block_end;
                continue;
            }
            let model = &buf[color_data_start..color_data_start + 4];
            let hex = match model {
                b"RGB " | b"RGB" => {
                    let values_start = color_data_start + 4;
                    if values_start + 12 > block_end {
                        offset = block_end;
                        continue;
                    }
                    let r = f32::from_be_bytes([
                        buf[values_start],
                        buf[values_start + 1],
                        buf[values_start + 2],
                        buf[values_start + 3],
                    ]);
                    let g = f32::from_be_bytes([
                        buf[values_start + 4],
                        buf[values_start + 5],
                        buf[values_start + 6],
                        buf[values_start + 7],
                    ]);
                    let b = f32::from_be_bytes([
                        buf[values_start + 8],
                        buf[values_start + 9],
                        buf[values_start + 10],
                        buf[values_start + 11],
                    ]);
                    Some(format!(
                        "#{:02X}{:02X}{:02X}",
                        (r.clamp(0.0, 1.0) * 255.0) as u8,
                        (g.clamp(0.0, 1.0) * 255.0) as u8,
                        (b.clamp(0.0, 1.0) * 255.0) as u8
                    ))
                }
                b"Gray" => {
                    let values_start = color_data_start + 4;
                    if values_start + 4 > block_end {
                        offset = block_end;
                        continue;
                    }
                    let v = f32::from_be_bytes([
                        buf[values_start],
                        buf[values_start + 1],
                        buf[values_start + 2],
                        buf[values_start + 3],
                    ]);
                    let val = (v.clamp(0.0, 1.0) * 255.0) as u8;
                    Some(format!("#{:02X}{:02X}{:02X}", val, val, val))
                }
                _ => None,
            };
            if let Some(hex) = hex {
                let swatch_name = if name.is_empty() {
                    format!("Color {}", swatches.len() + 1)
                } else {
                    name
                };
                swatches.push(Swatch::new(swatch_name, hex));
            }
        }
        offset = block_end;
    }
    if swatches.is_empty() {
        return Err("No color swatches found in ASE file".to_string());
    }
    Ok(swatches)
}

fn parse_wezterm(s: &str) -> Result<Vec<Swatch>, String> {
    #[derive(Deserialize)]
    struct WezTermColors {
        #[serde(default)]
        foreground: Option<String>,
        #[serde(default)]
        background: Option<String>,
        #[serde(default)]
        cursor_fg: Option<String>,
        #[serde(default)]
        cursor_bg: Option<String>,
        #[serde(default)]
        selection_fg: Option<String>,
        #[serde(default)]
        selection_bg: Option<String>,
        #[serde(default)]
        ansi: Option<Vec<String>>,
        #[serde(default)]
        brights: Option<Vec<String>>,
    }
    #[derive(Deserialize)]
    struct WezTermRoot {
        colors: WezTermColors,
    }
    let root: WezTermRoot =
        serde_json::from_str(s).map_err(|e| format!("WezTerm JSON error: {e}"))?;
    let c = root.colors;
    let mut swatches = Vec::new();
    let named_pairs: Vec<(&str, Option<String>)> = vec![
        ("foreground", c.foreground),
        ("background", c.background),
        ("cursor_fg", c.cursor_fg),
        ("cursor_bg", c.cursor_bg),
        ("selection_fg", c.selection_fg),
        ("selection_bg", c.selection_bg),
    ];
    for (name, color_opt) in named_pairs {
        if let Some(hex) = color_opt {
            swatches.push(Swatch::new(name.to_string(), normalize_hex(&hex)));
        }
    }
    if let Some(ansi) = c.ansi {
        for (i, color) in ansi.iter().enumerate().take(8) {
            swatches.push(Swatch::new(format!("ansi_{i}"), normalize_hex(color)));
        }
    }
    if let Some(brights) = c.brights {
        for (i, color) in brights.iter().enumerate().take(8) {
            swatches.push(Swatch::new(format!("bright_{i}"), normalize_hex(color)));
        }
    }
    if swatches.is_empty() {
        return Err("No colors found in WezTerm configuration".to_string());
    }
    Ok(swatches)
}

fn parse_windows_terminal(s: &str) -> Result<Vec<Swatch>, String> {
    #[derive(Deserialize)]
    struct WindowsTerminalScheme {
        #[serde(default)]
        #[expect(dead_code)]
        name: Option<String>,
        #[serde(default)]
        background: Option<String>,
        #[serde(default)]
        foreground: Option<String>,
        #[serde(rename = "cursorColor", default)]
        cursor_color: Option<String>,
        #[serde(rename = "selectionBackground", default)]
        selection_background: Option<String>,
        #[serde(default)]
        black: Option<String>,
        #[serde(default)]
        red: Option<String>,
        #[serde(default)]
        green: Option<String>,
        #[serde(default)]
        yellow: Option<String>,
        #[serde(default)]
        blue: Option<String>,
        #[serde(default)]
        purple: Option<String>,
        #[serde(default)]
        cyan: Option<String>,
        #[serde(default)]
        white: Option<String>,
        #[serde(rename = "brightBlack", default)]
        bright_black: Option<String>,
        #[serde(rename = "brightRed", default)]
        bright_red: Option<String>,
        #[serde(rename = "brightGreen", default)]
        bright_green: Option<String>,
        #[serde(rename = "brightYellow", default)]
        bright_yellow: Option<String>,
        #[serde(rename = "brightBlue", default)]
        bright_blue: Option<String>,
        #[serde(rename = "brightPurple", default)]
        bright_purple: Option<String>,
        #[serde(rename = "brightCyan", default)]
        bright_cyan: Option<String>,
        #[serde(rename = "brightWhite", default)]
        bright_white: Option<String>,
    }
    #[derive(Deserialize)]
    struct WindowsTerminalRoot {
        schemes: Option<Vec<WindowsTerminalScheme>>,
    }
    let root: WindowsTerminalRoot =
        serde_json::from_str(s).map_err(|e| format!("Windows Terminal JSON error: {e}"))?;
    let schemes = root
        .schemes
        .ok_or("No 'schemes' array found in Windows Terminal JSON")?;
    let scheme = schemes.first().ok_or("Empty 'schemes' array")?;
    let mut swatches = Vec::new();
    let named_pairs: Vec<(&str, Option<&String>)> = vec![
        ("background", scheme.background.as_ref()),
        ("foreground", scheme.foreground.as_ref()),
        ("cursorColor", scheme.cursor_color.as_ref()),
        ("selectionBackground", scheme.selection_background.as_ref()),
        ("black", scheme.black.as_ref()),
        ("red", scheme.red.as_ref()),
        ("green", scheme.green.as_ref()),
        ("yellow", scheme.yellow.as_ref()),
        ("blue", scheme.blue.as_ref()),
        ("purple", scheme.purple.as_ref()),
        ("cyan", scheme.cyan.as_ref()),
        ("white", scheme.white.as_ref()),
        ("brightBlack", scheme.bright_black.as_ref()),
        ("brightRed", scheme.bright_red.as_ref()),
        ("brightGreen", scheme.bright_green.as_ref()),
        ("brightYellow", scheme.bright_yellow.as_ref()),
        ("brightBlue", scheme.bright_blue.as_ref()),
        ("brightPurple", scheme.bright_purple.as_ref()),
        ("brightCyan", scheme.bright_cyan.as_ref()),
        ("brightWhite", scheme.bright_white.as_ref()),
    ];
    for (name, color_opt) in named_pairs {
        if let Some(hex) = color_opt {
            swatches.push(Swatch::new(name.to_string(), normalize_hex(hex)));
        }
    }
    if swatches.is_empty() {
        return Err("No colors found in Windows Terminal scheme".to_string());
    }
    Ok(swatches)
}

fn swatches(pairs: &[(&'static str, &'static str)]) -> Vec<Swatch> {
    pairs
        .iter()
        .map(|(name, hex)| Swatch::new(name.to_string(), hex.to_string()))
        .collect()
}

pub fn builtin_palettes() -> Vec<(&'static str, Vec<Swatch>)> {
    vec![
        (
            "Grayscale",
            swatches(&[
                ("White", "#F0F0F0"),
                ("Light Gray", "#C0C0C0"),
                ("Mid Gray", "#808080"),
                ("Dark Gray", "#404040"),
                ("Near Black", "#141414"),
            ]),
        ),
        (
            "Primary",
            swatches(&[
                ("Red Light", "#FF9999"),
                ("Red", "#FF3333"),
                ("Red Dark", "#CC0000"),
                ("Red Deep", "#880000"),
                ("Red Darkest", "#440000"),
                ("Blue Light", "#99CCFF"),
                ("Blue", "#3399FF"),
                ("Blue Dark", "#0066CC"),
                ("Blue Deep", "#003388"),
                ("Blue Darkest", "#001144"),
                ("Green Light", "#99EE88"),
                ("Green", "#33BB44"),
                ("Green Dark", "#008822"),
                ("Green Deep", "#005511"),
                ("Green Darkest", "#002208"),
                ("Yellow Light", "#FFEE88"),
                ("Yellow", "#FFD700"),
                ("Yellow Dark", "#CC9900"),
                ("Yellow Deep", "#886600"),
                ("Yellow Darkest", "#443300"),
            ]),
        ),
        (
            "Warm",
            swatches(&[
                ("Red Pale", "#FFCCCC"),
                ("Red Soft", "#FF8888"),
                ("Red", "#FF3333"),
                ("Red Dark", "#CC0000"),
                ("Red Deep", "#770000"),
                ("Orange Pale", "#FFDDBB"),
                ("Orange Soft", "#FFAA66"),
                ("Orange", "#FF6600"),
                ("Orange Dark", "#CC4400"),
                ("Orange Deep", "#772200"),
                ("Yellow Pale", "#FFEECC"),
                ("Yellow Soft", "#FFD966"),
                ("Yellow", "#FFC200"),
                ("Yellow Dark", "#CC8800"),
                ("Yellow Deep", "#664400"),
            ]),
        ),
        (
            "Cool",
            swatches(&[
                ("Blue Pale", "#CCE5FF"),
                ("Blue Soft", "#88BBFF"),
                ("Blue", "#3377EE"),
                ("Blue Dark", "#0044AA"),
                ("Blue Deep", "#001155"),
                ("Teal Pale", "#CCEEEE"),
                ("Teal Soft", "#66CCBB"),
                ("Teal", "#009988"),
                ("Teal Dark", "#006655"),
                ("Teal Deep", "#002233"),
                ("Purple Pale", "#DDCCFF"),
                ("Purple Soft", "#AA88EE"),
                ("Purple", "#7744CC"),
                ("Purple Dark", "#440088"),
                ("Purple Deep", "#220044"),
            ]),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// F-17 (GPT review): valid UTF-8 with multibyte chars must not panic
    /// on byte-offset slicing; it should parse or error cleanly.
    #[test]
    fn test_normalize_hex_multibyte_no_panic() {
        assert_eq!(normalize_hex("éx"), "#000000");
        assert_eq!(normalize_hex("#éx"), "#000000");
        assert_eq!(normalize_hex("日本語"), "#000000");
        assert_eq!(normalize_hex("abc"), "#AABBCC");
        assert_eq!(normalize_hex("#aabbcc"), "#AABBCC");
        assert_eq!(Swatch::default_shadow_hex("éxxxx"), "#000000");
    }

    #[test]
    fn test_paletty_json_parses() {
        let json = r##"[
            {"hex": "#FF0000", "name": "Red"},
            {"hex": "#00FF00", "name": "Green"},
            {"hex": "#0000FF", "name": "Blue"}
        ]"##;
        let swatches = parse_paletty_json(json).unwrap();
        assert_eq!(swatches.len(), 3);
        assert_eq!(swatches[0].hex, "#FF0000");
        assert_eq!(swatches[0].name, "Red");
        assert_eq!(swatches[1].hex, "#00FF00");
        assert_eq!(swatches[2].hex, "#0000FF");
    }

    #[test]
    fn test_paletty_json_mixed_hex_formats() {
        let json = r##"[
            {"hex": "#ff0000", "name": "Red"},
            {"hex": "ff0000", "name": "NoHash"}
        ]"##;
        let swatches = parse_paletty_json(json).unwrap();
        assert_eq!(swatches.len(), 2);
        assert_eq!(swatches[0].hex, "#FF0000");
        assert_eq!(swatches[1].hex, "#FF0000");
    }

    #[test]
    fn test_paletty_json_rejects_non_array() {
        let json = r#"{"name": "test"}"#;
        assert!(parse_paletty_json(json).is_err());
    }

    #[test]
    fn test_paletty_json_empty_array() {
        let json = "[]";
        assert!(parse_paletty_json(json).is_err());
    }

    #[test]
    fn test_ase_parses_correctly() {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"ASEF");
        buf.extend_from_slice(&1u16.to_be_bytes());
        buf.extend_from_slice(&0u16.to_be_bytes());
        buf.extend_from_slice(&1u32.to_be_bytes());
        buf.extend_from_slice(&0x0001u16.to_be_bytes());
        let name_utf16: Vec<u16> = "Red\0".encode_utf16().collect();
        let name_len = name_utf16.len() as u16;
        let block_data_len = 2u32 + name_len as u32 * 2 + 4 + 12 + 2;
        buf.extend_from_slice(&block_data_len.to_be_bytes());
        buf.extend_from_slice(&name_len.to_be_bytes());
        for &c in &name_utf16 {
            buf.extend_from_slice(&c.to_be_bytes());
        }
        buf.extend_from_slice(b"RGB ");
        buf.extend_from_slice(&(0.5f32).to_be_bytes());
        buf.extend_from_slice(&(0.0f32).to_be_bytes());
        buf.extend_from_slice(&(1.0f32).to_be_bytes());
        buf.extend_from_slice(&0u16.to_be_bytes());
        let swatches = parse_ase(&buf).unwrap();
        assert_eq!(swatches.len(), 1);
        assert_eq!(swatches[0].name, "Red");
        assert_eq!(swatches[0].hex, "#7F00FF");
    }

    #[test]
    fn test_ase_rejects_bad_magic() {
        let buf = b"NOTASEF";
        assert!(parse_ase(buf).is_err());
    }

    #[test]
    fn test_ase_rejects_truncated() {
        let buf = b"ASEF\x00\x01\x00\x00\x00\x00\x00\x00";
        assert!(parse_ase(buf).is_err());
    }

    #[test]
    fn test_wezterm_json_parses() {
        let json = r##"{
            "colors": {
                "foreground": "#ffffff",
                "background": "#000000",
                "ansi": ["#000000","#cc0000","#4e9a06","#c4a000","#3465a4","#75507b","#06989a","#d3d7cf"],
                "brights": ["#555753","#ef2929","#8ae234","#fce94f","#729fcf","#ad7fa8","#34e2e2","#eeeeee"]
            }
        }"##;
        let swatches = parse_wezterm(json).unwrap();
        assert_eq!(swatches.len(), 18);
        assert_eq!(swatches[0].name, "foreground");
        assert_eq!(swatches[0].hex, "#FFFFFF");
        assert_eq!(swatches[2].name, "ansi_0");
        assert_eq!(swatches[10].name, "bright_0");
    }

    #[test]
    fn test_wezterm_json_partial_data() {
        let json = r##"{
            "colors": {
                "foreground": "#ffffff"
            }
        }"##;
        let swatches = parse_wezterm(json).unwrap();
        assert_eq!(swatches.len(), 1);
        assert_eq!(swatches[0].name, "foreground");
    }

    #[test]
    fn test_wezterm_json_missing_colors_key() {
        let json = r#"{"not_colors": {}}"#;
        assert!(parse_wezterm(json).is_err());
    }

    #[test]
    fn test_windows_terminal_json_parses() {
        let json = r##"{
            "schemes": [{
                "name": "My Scheme",
                "background": "#0C0C0C",
                "foreground": "#F2F2F2",
                "black": "#0C0C0C",
                "red": "#C50F1F",
                "green": "#13A10E",
                "yellow": "#C19C00",
                "blue": "#0037DA",
                "purple": "#881798",
                "cyan": "#3A96DD",
                "white": "#CCCCCC"
            }]
        }"##;
        let swatches = parse_windows_terminal(json).unwrap();
        assert_eq!(swatches.len(), 10);
        assert_eq!(swatches[0].name, "background");
        assert_eq!(swatches[1].name, "foreground");
        assert_eq!(swatches[2].name, "black");
    }

    #[test]
    fn test_windows_terminal_picks_first_scheme() {
        let json = r##"{
            "schemes": [
                {"background": "#000000", "foreground": "#ffffff"},
                {"background": "#111111", "foreground": "#eeeeee"}
            ]
        }"##;
        let swatches = parse_windows_terminal(json).unwrap();
        assert_eq!(swatches[0].hex, "#000000");
        assert_eq!(swatches[1].hex, "#FFFFFF");
    }

    #[test]
    fn test_auto_detect_paletty() {
        let content = b"[{\"hex\":\"#f00\",\"name\":\"Red\"}]";
        assert_eq!(
            auto_detect_format(content, Some("json")),
            Some(ImportFormat::PalettyJson)
        );
    }

    #[test]
    fn test_auto_detect_wezterm() {
        let content = b"{\"colors\":{\"foreground\":\"#fff\"}}";
        assert_eq!(
            auto_detect_format(content, Some("json")),
            Some(ImportFormat::WezTermJson)
        );
    }

    #[test]
    fn test_auto_detect_windows_terminal() {
        let content = b"{\"schemes\":[{\"name\":\"x\"}]}";
        assert_eq!(
            auto_detect_format(content, Some("json")),
            Some(ImportFormat::WindowsTerminalJson)
        );
    }

    #[test]
    fn test_auto_detect_native() {
        let content = b"{\"name\":\"x\",\"swatches\":[]}";
        assert_eq!(
            auto_detect_format(content, Some("json")),
            Some(ImportFormat::Native)
        );
    }

    #[test]
    fn test_auto_detect_ase() {
        let content = b"ASEF\x00\x01\x00\x00\x00\x00\x00\x00";
        assert_eq!(
            auto_detect_format(content, Some("ase")),
            Some(ImportFormat::AdobeAse)
        );
    }

    #[test]
    fn test_auto_detect_unknown() {
        let content = b"garbage content";
        assert_eq!(auto_detect_format(content, Some("txt")), None);
    }

    #[test]
    fn test_auto_detect_ase_by_magic() {
        let content = b"ASEF\x00\x01\x00\x00\x00\x00\x00\x01";
        assert_eq!(
            auto_detect_format(content, None),
            Some(ImportFormat::AdobeAse)
        );
    }

    #[test]
    fn test_auto_detect_empty_content() {
        let content = b"";
        assert_eq!(auto_detect_format(content, None), None);
    }

    #[test]
    fn test_import_switches_dispatch() {
        let paletty = b"[{\"hex\":\"#f00\",\"name\":\"R\"}]";
        let r = import_swatches(paletty, ImportFormat::PalettyJson).unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].hex, "#FF0000");
    }

    #[test]
    fn test_builtin_palettes_names_and_counts() {
        let palettes = builtin_palettes();
        assert_eq!(palettes.len(), 4);
        let names: Vec<&str> = palettes.iter().map(|(n, _)| *n).collect();
        assert!(names.contains(&"Grayscale"));
        assert!(names.contains(&"Primary"));
        assert!(names.contains(&"Warm"));
        assert!(names.contains(&"Cool"));
        for (name, swatches) in &palettes {
            assert!(!swatches.is_empty(), "{name} palette should be non-empty");
            for s in swatches {
                assert!(s.hex.starts_with('#'), "{name}/{}: hex missing #", s.name);
            }
        }
    }
}
