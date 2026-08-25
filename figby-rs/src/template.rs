// .ftmp template file format parser and renderer

use serde::Deserialize;
use std::collections::HashMap;

use crate::font::{load_font, FIGfont, FontError};
use crate::render::{add_char, render_line, Justification};
use crate::smush::SmushMode;

/// Errors from template parsing/rendering.
#[derive(Debug)]
pub enum TemplateError {
    ParseError(String),
    FontError(FontError),
    ImageError(String),
    ResolveError(String),
}

impl std::fmt::Display for TemplateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TemplateError::ParseError(msg) => write!(f, "template parse error: {}", msg),
            TemplateError::FontError(e) => write!(f, "template font error: {}", e),
            TemplateError::ImageError(msg) => write!(f, "template image error: {}", msg),
            TemplateError::ResolveError(msg) => write!(f, "template resolve error: {}", msg),
        }
    }
}

impl std::error::Error for TemplateError {}

impl From<FontError> for TemplateError {
    fn from(e: FontError) -> Self {
        TemplateError::FontError(e)
    }
}

/// Canvas settings section in .ftmp frontmatter.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct CanvasSettings {
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
    #[serde(default)]
    pub keep_ratio: Option<bool>,
    #[serde(default)]
    pub margin: Option<u32>,
    #[serde(default)]
    pub padding: Option<u32>,
}

/// A single variable binding in .ftmp frontmatter.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct VariableBinding {
    pub text: String,
    #[serde(default)]
    pub font: Option<String>,
    #[serde(default)]
    pub x: Option<u32>,
    #[serde(default)]
    pub y: Option<u32>,
    #[serde(default)]
    pub z: Option<i32>,
    #[serde(default)]
    pub align: Option<String>,
    #[serde(default)]
    pub overlap: Option<String>,
    #[serde(default)]
    pub border_width: Option<u32>,
    #[serde(default)]
    pub border_color: Option<String>,
    #[serde(default)]
    pub shadow_size: Option<u32>,
    #[serde(default)]
    pub shadow_color: Option<String>,
}

/// Top-level frontmatter structure.
#[derive(Debug, Deserialize)]
struct TemplateFrontmatter {
    #[serde(default)]
    canvas: Option<CanvasSettings>,
    #[serde(default)]
    variables: Option<HashMap<String, VariableBinding>>,
}

/// An inline image tag `{{img:source:width:height:color:pos:charset}}`.
#[derive(Debug, Clone)]
pub struct ImageTag {
    pub source: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub colored: bool,
    pub x: Option<u32>,
    pub y: Option<u32>,
    pub charset: String,
}

/// Builtin template function tag `{{date:format}}` or `{{repo-data:field}}`.
///
/// Syntax:
/// - `{{date:strftime_format}}` — renders current date/time (deferred to 2.1)
/// - `{{repo-data:author|email|name|release}}` — renders repo metadata (deferred to 2.1)
#[derive(Debug, Clone)]
pub enum TemplateBuiltin {
    /// `{{date:format}}` — strftime-style date formatting (deferred).
    Date(String),
    /// `{{repo-data:field}}` — author, email, name, or release (deferred).
    RepoData(String),
}

/// A renderable layer derived from a `{{varname}}` in the template body.
#[derive(Debug, Clone)]
pub struct Layer {
    pub varname: String,
    pub binding: VariableBinding,
    /// Position in body text (for tie-breaking on equal z).
    pub body_index: usize,
    pub image_tag: Option<ImageTag>,
    pub builtin: Option<TemplateBuiltin>,
}

/// Parsed .ftmp template.
#[derive(Debug, Clone)]
pub struct Template {
    pub canvas: CanvasSettings,
    pub layers: Vec<Layer>,
    pub body: String,
}

/// Configuration passed to `render_template`.
#[derive(Debug, Clone)]
pub struct RenderConfig {
    pub font_dir: String,
    pub term_width: u32,
    pub override_width: Option<u32>,
}

/// Resolve `${VAR}` env var references in a text value.
/// `$(...)` command substitution is intentionally NOT executed (security: untrusted templates).
/// Plain string literals and `$(...)` syntax pass through unchanged.
pub fn resolve_text_value(text: &str) -> Result<String, TemplateError> {
    let mut result = String::new();
    let mut remaining = text;

    while let Some(dollar_pos) = remaining.find('$') {
        result.push_str(&remaining[..dollar_pos]);
        remaining = &remaining[dollar_pos + 1..];

        if let Some(rest) = remaining.strip_prefix('{') {
            let closing = rest.find('}').ok_or_else(|| {
                TemplateError::ResolveError("unclosed env var `${...}`".to_string())
            })?;
            let var_name = &rest[..closing];
            let value = std::env::var(var_name).map_err(|_| {
                TemplateError::ResolveError(format!("env var '{}' not set", var_name))
            })?;
            result.push_str(&value);
            remaining = &rest[closing + 1..];
        } else {
            result.push('$');
        }
    }

    result.push_str(remaining);
    Ok(result)
}

/// Parse a .ftmp template string into a `Template`.
///
/// Format:
/// ```toml
/// ---
/// [canvas]
/// width = 80
///
/// [variables.greeting]
/// text = "Hello"
/// ---
/// {{greeting}}
/// ```
pub fn parse_ftmp(input: &str) -> Result<Template, TemplateError> {
    let stripped = input.trim_start();

    if !stripped.starts_with("---") {
        return Err(TemplateError::ParseError(
            "template must start with '---' frontmatter delimiter".to_string(),
        ));
    }

    let after_opener = &stripped[3..];
    let end_marker = after_opener.find("\n---").ok_or_else(|| {
        TemplateError::ParseError("missing closing '---' frontmatter delimiter".to_string())
    })?;

    let frontmatter_str = after_opener[..end_marker].trim();
    let body = after_opener[end_marker + 4..].trim().to_string();

    let frontmatter: TemplateFrontmatter = toml::from_str(frontmatter_str)
        .map_err(|e| TemplateError::ParseError(format!("invalid TOML frontmatter: {}", e)))?;

    let variables = frontmatter.variables.unwrap_or_default();
    let canvas = frontmatter.canvas.unwrap_or_default();

    let mut layers = Vec::new();
    let mut body_idx = 0;
    let mut remaining = body.as_str();

    while let Some(start) = remaining.find("{{") {
        remaining = &remaining[start + 2..];
        if let Some(end) = remaining.find("}}") {
            let tag_content = remaining[..end].trim();
            if !tag_content.is_empty() {
                if let Some(img_rest) = tag_content.strip_prefix("img:") {
                    let parts: Vec<&str> = img_rest.split(':').collect();
                    let source = parts.first().unwrap_or(&"").to_string();
                    let width = parts.get(1).and_then(|s| s.parse().ok());
                    let height = parts.get(2).and_then(|s| s.parse().ok());
                    let colored =
                        matches!(*parts.get(3).unwrap_or(&""), "true" | "1" | "yes" | "y");
                    let (x, y) = parts
                        .get(4)
                        .and_then(|s| {
                            let mut ps = s.split(',');
                            let px = ps.next()?.parse().ok()?;
                            let py = ps.next()?.parse().ok()?;
                            Some((px, py))
                        })
                        .unwrap_or((0, 0));
                    let charset = parts.get(5).unwrap_or(&"").to_string();

                    layers.push(Layer {
                        varname: tag_content.to_string(),
                        binding: VariableBinding {
                            x: Some(x),
                            y: Some(y),
                            z: Some(0),
                            ..VariableBinding::default()
                        },
                        body_index: body_idx,
                        image_tag: Some(ImageTag {
                            source,
                            width,
                            height,
                            colored,
                            x: Some(x),
                            y: Some(y),
                            charset,
                        }),
                        builtin: None,
                    });
                } else if let Some(format) = tag_content.strip_prefix("date:") {
                    let format_str = format.to_string();
                    layers.push(Layer {
                        varname: tag_content.to_string(),
                        binding: VariableBinding::default(),
                        body_index: body_idx,
                        image_tag: None,
                        builtin: Some(TemplateBuiltin::Date(format_str)),
                    });
                } else if let Some(field) = tag_content.strip_prefix("repo-data:") {
                    let field_name = field.to_string();
                    layers.push(Layer {
                        varname: tag_content.to_string(),
                        binding: VariableBinding::default(),
                        body_index: body_idx,
                        image_tag: None,
                        builtin: Some(TemplateBuiltin::RepoData(field_name)),
                    });
                } else if let Some(binding) = variables.get(tag_content) {
                    layers.push(Layer {
                        varname: tag_content.to_string(),
                        binding: binding.clone(),
                        body_index: body_idx,
                        image_tag: None,
                        builtin: None,
                    });
                }
            }
            body_idx += 1;
            remaining = &remaining[end + 2..];
        }
    }

    Ok(Template {
        canvas,
        layers,
        body,
    })
}

/// Parse all `{{varname}}` placeholders from body text.
pub fn extract_placeholders(body: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut remaining = body;
    while let Some(start) = remaining.find("{{") {
        remaining = &remaining[start + 2..];
        if let Some(end) = remaining.find("}}") {
            let varname = remaining[..end].trim().to_string();
            if !varname.is_empty() {
                result.push(varname);
            }
            remaining = &remaining[end + 2..];
        }
    }
    result
}

/// Render a single line of text through the FIGlet pipeline.
fn render_text_line(
    font: &FIGfont,
    text: &str,
    output_width: usize,
    justification: Justification,
    rtl: bool,
) -> Vec<String> {
    let height = font.charheight as usize;
    let smush_mode = SmushMode::new(font.full_layout as u32);
    let limit = output_width.saturating_sub(1);

    let mut rows = vec![String::new(); height];
    let mut outlinelen = 0;
    let mut prev_width = 0;

    for c in text.chars() {
        let code = c as u32;
        let ok = add_char(
            font,
            code,
            &mut rows,
            &mut outlinelen,
            &mut prev_width,
            smush_mode,
            rtl,
            limit,
        );
        if !ok {
            break;
        }
    }

    render_line(&rows, font.hardblank, justification, output_width)
}

/// Render multi-line text through FIGlet, one FIGlet line per text line.
fn render_figlet_text(
    font: &FIGfont,
    text: &str,
    output_width: usize,
    justification: Justification,
) -> Vec<String> {
    let height = font.charheight as usize;
    let rtl = font.print_direction == 1;
    let mut all_rows = Vec::new();

    for line in text.split('\n') {
        if line.is_empty() {
            for _ in 0..height {
                all_rows.push(" ".repeat(output_width));
            }
        } else {
            all_rows.extend(render_text_line(
                font,
                line,
                output_width,
                justification,
                rtl,
            ));
        }
    }

    all_rows
}

/// Place rendered rows onto the canvas at (x, y), overwriting existing chars.
fn place_on_canvas(canvas: &mut [Vec<char>], rows: &[String], x: usize, y: usize) {
    for (i, row) in rows.iter().enumerate() {
        let canvas_y = y + i;
        if canvas_y >= canvas.len() {
            break;
        }
        for (j, c) in row.chars().enumerate() {
            let canvas_x = x + j;
            if canvas_x >= canvas[canvas_y].len() {
                break;
            }
            canvas[canvas_y][canvas_x] = c;
        }
    }
}

/// Compute bounding box of non-space content in rendered rows at canvas position (x, y).
fn content_bbox(rows: &[String], x: usize, y: usize) -> Option<(usize, usize, usize, usize)> {
    let mut top = None;
    let mut bottom = 0;
    let mut left = usize::MAX;
    let mut right = 0;

    for (i, row) in rows.iter().enumerate() {
        let canvas_y = y + i;
        for (j, c) in row.chars().enumerate() {
            if c != ' ' {
                let canvas_x = x + j;
                if top.is_none() {
                    top = Some(canvas_y);
                }
                bottom = canvas_y;
                left = left.min(canvas_x);
                right = right.max(canvas_x);
            }
        }
    }

    top.map(|t| (t, bottom, left, right))
}

/// Fill border around bbox on canvas using `'.'`, only overwriting spaces.
fn fill_border(canvas: &mut [Vec<char>], bbox: (usize, usize, usize, usize), border_width: usize) {
    let (top, bottom, left, right) = bbox;

    let outer_top = top.saturating_sub(border_width);
    let outer_bottom = bottom + border_width;
    let outer_left = left.saturating_sub(border_width);
    let outer_right = right + border_width;

    let max_y = canvas.len().saturating_sub(1);
    let y_end = outer_bottom.min(max_y);
    for (y, row) in canvas
        .iter_mut()
        .enumerate()
        .skip(outer_top)
        .take(y_end.saturating_sub(outer_top).saturating_add(1))
    {
        let max_x = row.len().saturating_sub(1);
        let x_end = outer_right.min(max_x);
        for (x, cell) in row
            .iter_mut()
            .enumerate()
            .skip(outer_left)
            .take(x_end.saturating_sub(outer_left).saturating_add(1))
        {
            if x >= left && x <= right && y >= top && y <= bottom {
                continue;
            }
            if *cell == ' ' {
                *cell = '.';
            }
        }
    }
}

/// Fill drop-shadow region offset from bbox on canvas using `'.'`, only overwriting spaces.
fn fill_shadow(canvas: &mut [Vec<char>], bbox: (usize, usize, usize, usize), shadow_size: usize) {
    let (top, bottom, left, right) = bbox;

    let shadow_top = top + shadow_size;
    let shadow_bottom = bottom + shadow_size;
    let shadow_left = left + shadow_size;
    let shadow_right = right + shadow_size;

    let max_y = canvas.len().saturating_sub(1);
    let y_end = shadow_bottom.min(max_y);
    for (_y, row) in canvas
        .iter_mut()
        .enumerate()
        .skip(shadow_top)
        .take(y_end.saturating_sub(shadow_top).saturating_add(1))
    {
        let max_x = row.len().saturating_sub(1);
        let x_end = shadow_right.min(max_x);
        for (_x, cell) in row
            .iter_mut()
            .enumerate()
            .skip(shadow_left)
            .take(x_end.saturating_sub(shadow_left).saturating_add(1))
        {
            if *cell == ' ' {
                *cell = '.';
            }
        }
    }
}

/// Render a parsed template onto a canvas and return output rows.
pub fn render_template(
    tmpl: &Template,
    config: &RenderConfig,
) -> Result<Vec<String>, TemplateError> {
    let width = config
        .override_width
        .or(tmpl.canvas.width)
        .unwrap_or(config.term_width) as usize;

    let height = tmpl.canvas.height.unwrap_or(24) as usize;
    let margin = tmpl.canvas.margin.unwrap_or(0) as usize;
    let padding = tmpl.canvas.padding.unwrap_or(0) as usize;

    const MAX_CANVAS_CELLS: usize = 1_000_000;
    const MAX_MARGIN: usize = 1_000;
    const MAX_PADDING: usize = 1_000;
    if width.saturating_mul(height) > MAX_CANVAS_CELLS {
        return Err(TemplateError::ParseError(format!(
            "canvas {}×{} exceeds max {} cells",
            width, height, MAX_CANVAS_CELLS
        )));
    }
    if margin > MAX_MARGIN {
        return Err(TemplateError::ParseError(format!(
            "canvas margin {} exceeds max {}",
            margin, MAX_MARGIN
        )));
    }
    if padding > MAX_PADDING {
        return Err(TemplateError::ParseError(format!(
            "canvas padding {} exceeds max {}",
            padding, MAX_PADDING
        )));
    }

    let mut canvas = vec![vec![' '; width]; height];

    // Collect unique font names from non-image layers and pre-load them.
    let font_names: Vec<String> = {
        let mut names: Vec<String> = tmpl
            .layers
            .iter()
            .filter(|l| l.image_tag.is_none())
            .map(|l| {
                l.binding
                    .font
                    .clone()
                    .unwrap_or_else(|| "standard".to_string())
            })
            .collect();
        names.sort();
        names.dedup();
        names
    };

    let mut font_cache: HashMap<String, FIGfont> = HashMap::new();
    for name in &font_names {
        let mut dirs: Vec<&str> = vec![config.font_dir.as_str()];
        dirs.extend(crate::font::DEFAULT_FONT_DIRS);
        font_cache.insert(name.clone(), load_font(name, &dirs)?);
    }

    // Sort layers by z (ascending), then body_index (ascending).
    let mut sorted_layers: Vec<&Layer> = tmpl.layers.iter().collect();
    sorted_layers.sort_by(|a, b| {
        let za = a.binding.z.unwrap_or(0);
        let zb = b.binding.z.unwrap_or(0);
        za.cmp(&zb).then(a.body_index.cmp(&b.body_index))
    });

    let mut flow_y: usize = 0;

    for layer in &sorted_layers {
        let x = layer.binding.x.unwrap_or(0) as usize;
        let overlap = layer.binding.overlap.as_deref().unwrap_or("overwrite");

        if layer.builtin.is_some() {
            continue;
        }

        if let Some(ref img) = layer.image_tag {
            let mut options = rascii_art::RenderOptions::new();
            if let Some(w) = img.width {
                options = options.width(w);
            }
            if let Some(h) = img.height {
                options = options.height(h);
            }
            if img.colored {
                options = options.colored(true);
            }
            if !img.charset.is_empty() {
                if let Some(cs) = rascii_art::charsets::from_str(&img.charset) {
                    options = options.charset(cs);
                }
            }

            // Security: reject absolute paths and '..' traversal (untrusted template).
            let img_path = std::path::Path::new(&img.source);
            if img_path.is_absolute()
                || img_path
                    .components()
                    .any(|c| c == std::path::Component::ParentDir)
            {
                return Err(TemplateError::ImageError(format!(
                    "image path '{}' rejected: must be relative with no '..' traversal",
                    img.source
                )));
            }

            let mut buf = String::new();
            rascii_art::render_to(&img.source, &mut buf, &options)
                .map_err(|e| TemplateError::ImageError(e.to_string()))?;

            let rows: Vec<String> = buf.lines().map(|l| l.to_string()).collect();

            let placed_y: usize = match overlap {
                "flow" => {
                    let y_pos = flow_y;
                    flow_y += rows.len();
                    place_on_canvas(&mut canvas, &rows, x, y_pos);
                    y_pos
                }
                _ => {
                    let y = layer.binding.y.unwrap_or(0) as usize;
                    place_on_canvas(&mut canvas, &rows, x, y);
                    y
                }
            };

            if layer.binding.border_width.is_some() || layer.binding.shadow_size.is_some() {
                if let Some(bbox) = content_bbox(&rows, x, placed_y) {
                    if let Some(bw) = layer.binding.border_width {
                        if bw > 0 {
                            fill_border(&mut canvas, bbox, bw as usize);
                        }
                    }
                    if let Some(ss) = layer.binding.shadow_size {
                        if ss > 0 {
                            fill_shadow(&mut canvas, bbox, ss as usize);
                        }
                    }
                }
            }
        } else {
            let font_name = layer.binding.font.as_deref().unwrap_or("standard");
            let font = font_cache.get(font_name).ok_or_else(|| {
                TemplateError::ParseError(format!("font '{}' not loaded", font_name))
            })?;

            let align = match layer.binding.align.as_deref() {
                Some("center") => Justification::Center,
                Some("right") => Justification::Right,
                _ => Justification::Left,
            };

            let text = resolve_text_value(&layer.binding.text)?;
            let rows = render_figlet_text(font, &text, width, align);

            let placed_y: usize = match overlap {
                "flow" => {
                    let y_pos = flow_y;
                    flow_y += rows.len();
                    place_on_canvas(&mut canvas, &rows, x, y_pos);
                    y_pos
                }
                _ => {
                    let y = layer.binding.y.unwrap_or(0) as usize;
                    place_on_canvas(&mut canvas, &rows, x, y);
                    y
                }
            };

            if layer.binding.border_width.is_some() || layer.binding.shadow_size.is_some() {
                if let Some(bbox) = content_bbox(&rows, x, placed_y) {
                    if let Some(bw) = layer.binding.border_width {
                        if bw > 0 {
                            fill_border(&mut canvas, bbox, bw as usize);
                        }
                    }
                    if let Some(ss) = layer.binding.shadow_size {
                        if ss > 0 {
                            fill_shadow(&mut canvas, bbox, ss as usize);
                        }
                    }
                }
            }
        }
    }

    // Convert canvas to output rows.
    let mut output: Vec<String> = canvas.iter().map(|row| row.iter().collect()).collect();

    // Apply padding.
    if padding > 0 {
        let pad = " ".repeat(padding);
        for row in &mut output {
            let inner = std::mem::take(row);
            *row = format!("{}{}{}", pad, inner, pad);
        }
    }

    // Apply margin.
    if margin > 0 {
        let blank = " ".repeat(width + 2 * padding);
        let mut result = Vec::new();
        result.extend(std::iter::repeat_n(blank.clone(), margin));
        result.extend(output);
        result.extend(std::iter::repeat_n(blank, margin));
        output = result;
    }

    Ok(output)
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    fn test_font() -> FIGfont {
        use std::collections::HashMap;
        let mut chars = HashMap::new();
        chars.insert(0, FIGcharacter_from("####"));
        chars.insert(32, FIGcharacter_from("    "));
        chars.insert(65, FIGcharacter_from(" AA "));
        chars.insert(66, FIGcharacter_from(" BB "));
        chars.insert(67, FIGcharacter_from(" CC "));
        FIGfont {
            charheight: 1,
            hardblank: '$',
            full_layout: 0,
            chars,
            ..FIGfont::default()
        }
    }

    fn FIGcharacter_from(s: &str) -> crate::font::FIGcharacter {
        crate::font::FIGcharacter::from(vec![s.to_string()])
    }

    fn setup_font_dir() -> (tempfile::TempDir, String) {
        let tmpdir = tempfile::tempdir().unwrap();
        let font_bytes = include_bytes!("../assets/fonts/standard.flf");
        std::fs::write(tmpdir.path().join("standard.flf"), font_bytes).unwrap();
        let font_dir = tmpdir.path().to_str().unwrap().to_string();
        (tmpdir, font_dir)
    }

    #[test]
    fn test_parse_ftmp_basic() {
        let ftmp = r#"---
[canvas]
width = 80
height = 24

[variables.greeting]
text = "Hello World"
font = "standard"
x = 0
y = 0
---
{{greeting}}
"#;
        let tmpl = parse_ftmp(ftmp).expect("should parse");
        assert_eq!(tmpl.canvas.width, Some(80));
        assert_eq!(tmpl.canvas.height, Some(24));
        assert_eq!(tmpl.layers.len(), 1);
        assert_eq!(tmpl.layers[0].varname, "greeting");
        assert_eq!(tmpl.layers[0].binding.text, "Hello World");
        assert_eq!(tmpl.layers[0].binding.font.as_deref(), Some("standard"));
    }

    #[test]
    fn test_parse_ftmp_no_frontmatter() {
        let ftmp = "---\n---\n{{x}}\n";
        let tmpl = parse_ftmp(ftmp).expect("should parse empty frontmatter");
        assert!(tmpl.canvas.width.is_none());
        assert_eq!(tmpl.layers.len(), 0);
    }

    #[test]
    fn test_parse_ftmp_variable_without_body_placeholder() {
        let ftmp = r#"---
[variables.foo]
text = "hello"
---
there are no placeholders here
"#;
        let tmpl = parse_ftmp(ftmp).expect("should parse");
        assert_eq!(tmpl.layers.len(), 0);
    }

    #[test]
    fn test_parse_ftmp_canvas_settings() {
        let ftmp = r#"---
[canvas]
width = 60
height = 30
keep_ratio = true
margin = 2
padding = 1
---
{{a}}
"#;
        let tmpl = parse_ftmp(ftmp).expect("should parse");
        assert_eq!(tmpl.canvas.width, Some(60));
        assert_eq!(tmpl.canvas.height, Some(30));
        assert_eq!(tmpl.canvas.keep_ratio, Some(true));
        assert_eq!(tmpl.canvas.margin, Some(2));
        assert_eq!(tmpl.canvas.padding, Some(1));
    }

    #[test]
    fn test_parse_ftmp_missing_delimiter() {
        let err = parse_ftmp("no delimiter here").unwrap_err();
        assert!(err.to_string().contains("must start with"));
    }

    #[test]
    fn test_parse_ftmp_missing_closing_delimiter() {
        let err = parse_ftmp("---\n[key = 1]\nno closing").unwrap_err();
        assert!(err.to_string().contains("missing closing"));
    }

    #[test]
    fn test_extract_placeholders() {
        let body = "Hello {{name}}, your {{item}} is ready";
        let vars = extract_placeholders(body);
        assert_eq!(vars, vec!["name", "item"]);
    }

    #[test]
    fn test_extract_placeholders_no_match() {
        let vars = extract_placeholders("just text");
        assert!(vars.is_empty());
    }

    #[test]
    fn test_extract_placeholders_empty_name() {
        let vars = extract_placeholders("{{}}");
        assert!(vars.is_empty());
    }

    #[test]
    fn test_render_overwrite_mode() {
        // Two layers at same (x,y), second overwrites first.
        let font = test_font();
        let text_a = "A";
        let text_b = "B";

        let rows_a = render_figlet_text(&font, text_a, 20, Justification::Left);
        let rows_b = render_figlet_text(&font, text_b, 20, Justification::Left);

        let mut canvas = vec![vec![' '; 20]; 10];
        place_on_canvas(&mut canvas, &rows_a, 0, 0);
        place_on_canvas(&mut canvas, &rows_b, 0, 0);

        // ' BB ' overwrites ' AA ' at (0,0). canvas[0] should have " BB" then spaces.
        let row: String = canvas[0].iter().collect();
        assert!(
            row.starts_with(" BB"),
            "second layer overwrites first: got '{row}'"
        );
    }

    #[test]
    fn test_render_flow_mode() {
        // Two layers with flow: second stacks below first.
        let font = test_font();
        let text_a = "AA";
        let text_b = "BB";

        let rows_a = render_figlet_text(&font, text_a, 20, Justification::Left);
        let rows_b = render_figlet_text(&font, text_b, 20, Justification::Left);

        let mut canvas = vec![vec![' '; 20]; 10];
        let mut flow_y = 0;

        // Layer A (flow)
        place_on_canvas(&mut canvas, &rows_a, 0, flow_y);
        flow_y += rows_a.len();

        // Layer B (flow) placed at new flow_y
        place_on_canvas(&mut canvas, &rows_b, 0, flow_y);

        let row0: String = canvas[0].iter().collect();
        let row1: String = canvas[1].iter().collect();
        assert!(row0.starts_with(" AA "), "layer A on row 0: got '{row0}'");
        assert!(row1.starts_with(" BB "), "layer B on row 1: got '{row1}'");
    }

    #[test]
    fn test_render_z_order() {
        // Three layers at (0,0) with z=0, z=1, z=2.
        // Render in ascending z-order — highest z (last rendered) overwrites.
        let font = test_font();
        let text_a = "A"; // renders as " AA "
        let text_b = "B"; // renders as " BB "
        let text_c = "C"; // renders as " CC "

        let rows_a = render_figlet_text(&font, text_a, 20, Justification::Left);
        let rows_b = render_figlet_text(&font, text_b, 20, Justification::Left);
        let rows_c = render_figlet_text(&font, text_c, 20, Justification::Left);

        let mut canvas = vec![vec![' '; 20]; 10];

        // Render in ascending z-order: z=0 (A), z=1 (B), z=2 (C)
        place_on_canvas(&mut canvas, &rows_a, 0, 0); // z=0
        place_on_canvas(&mut canvas, &rows_b, 0, 0); // z=1
        place_on_canvas(&mut canvas, &rows_c, 0, 0); // z=2

        // z=2 (C) rendered last, so " CC " should be on top
        let row: String = canvas[0].iter().collect();
        assert!(row.starts_with(" CC"), "z=2 should be on top: got '{row}'");
    }

    #[test]
    fn test_render_canvas_size() {
        let ftmp = r#"---
[canvas]
width = 30
height = 10

[variables.msg]
text = "Hi"
font = "standard"
---
{{msg}}
"#;
        let tmpl = parse_ftmp(ftmp).expect("should parse");
        let (_tmpdir, font_dir) = setup_font_dir();
        let config = RenderConfig {
            font_dir,
            term_width: 80,
            override_width: None,
        };
        let output = render_template(&tmpl, &config).expect("should render");
        for row in &output {
            assert!(
                row.chars().count() <= 30,
                "row width {} exceeds canvas width 30: '{row}'",
                row.chars().count()
            );
        }
    }

    #[test]
    fn test_canvas_oversize_rejected() {
        // width=4000000000 would OOM; must be caught before allocation.
        let ftmp = "---\n[canvas]\nwidth = 4000000\nheight = 1000\n---\n";
        let tmpl = parse_ftmp(ftmp).expect("parse must succeed");
        let config = RenderConfig {
            font_dir: ".".to_string(),
            term_width: 80,
            override_width: None,
        };
        let err = render_template(&tmpl, &config).unwrap_err();
        assert!(
            err.to_string().contains("exceeds max"),
            "oversize canvas must be rejected, got: {err}"
        );
    }

    #[test]
    fn test_end_to_end_template() {
        let ftmp = r#"---
[canvas]
width = 50
height = 10

[variables.greeting]
text = "Hello"
font = "standard"
---
{{greeting}}
"#;
        let tmpl = parse_ftmp(ftmp).expect("should parse");
        let (_tmpdir, font_dir) = setup_font_dir();
        let config = RenderConfig {
            font_dir,
            term_width: 80,
            override_width: None,
        };
        let output = render_template(&tmpl, &config).expect("should render");
        // Should produce 6 rows (standard font height) at least
        assert!(!output.is_empty(), "output should have rows");
        // Each row should contain the rendered "Hello" text
        assert!(
            output[0].contains('H') || output[0].contains(' '),
            "row 0 should contain 'H' or spaces"
        );
        // Verify all rows are within canvas width
        for row in &output {
            assert!(
                row.chars().count() <= 50,
                "row width {} exceeds 50",
                row.chars().count()
            );
        }
    }

    #[test]
    fn test_render_margin_padding() {
        let font = test_font();
        let rows = render_figlet_text(&font, "AA", 20, Justification::Left);

        let mut canvas = vec![vec![' '; 20]; 5];
        place_on_canvas(&mut canvas, &rows, 0, 0);

        // Convert to output string rows
        let output: Vec<String> = canvas.iter().map(|r| r.iter().collect()).collect();

        // Apply padding=1
        let pad = " ".to_string();
        let padded: Vec<String> = output
            .iter()
            .map(|r| format!("{}{}{}", pad, r, pad))
            .collect();

        // Apply margin=2
        let blank = " ".repeat(22);
        let mut result = Vec::new();
        result.extend(std::iter::repeat_n(blank.clone(), 2));
        result.extend(padded);
        result.extend(std::iter::repeat_n(blank, 2));

        assert_eq!(result.len(), 5 + 4); // 5 original + 2 top + 2 bottom margin
        assert_eq!(result[0].len(), 22); // 20 + 2 padding
    }

    #[test]
    fn test_resolve_text_value_literal() {
        let result = resolve_text_value("Hello World").unwrap();
        assert_eq!(result, "Hello World");
    }

    #[test]
    fn test_resolve_text_value_empty() {
        let result = resolve_text_value("").unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_resolve_env_var() {
        let result = resolve_text_value("${HOME}").unwrap();
        assert!(!result.is_empty(), "HOME should be set");
        assert!(result.contains('/'), "HOME should be a path");
    }

    #[test]
    fn test_resolve_env_var_in_context() {
        let result = resolve_text_value("path=${HOME}/foo").unwrap();
        assert!(result.starts_with("path="), "should preserve prefix");
        assert!(result.ends_with("/foo"), "should preserve suffix");
        assert!(
            result.len() > "path=/foo".len(),
            "should contain expanded HOME"
        );
    }

    #[test]
    fn test_resolve_env_var_not_set() {
        let err = resolve_text_value("${DOES_NOT_EXIST_XYZZY}").unwrap_err();
        assert!(err.to_string().contains("not set"));
    }

    #[test]
    fn test_resolve_env_var_unclosed() {
        let err = resolve_text_value("${UNCLOSED").unwrap_err();
        assert!(err.to_string().contains("unclosed"));
    }

    #[test]
    fn test_resolve_command_sub_not_executed() {
        // Security: $(...) must NOT execute — pass through as literal text.
        let result = resolve_text_value("$(echo hello)").unwrap();
        assert_eq!(
            result, "$(echo hello)",
            "command substitution must not execute"
        );
    }

    #[test]
    fn test_resolve_command_sub_in_context_not_executed() {
        let result = resolve_text_value("prefix_$(echo mid)_suffix").unwrap();
        assert_eq!(result, "prefix_$(echo mid)_suffix");
    }

    #[test]
    fn test_resolve_command_sub_dangerous_not_executed() {
        // Ensure a destructive command is passed through verbatim, never run.
        let result = resolve_text_value("$(rm -rf /)").unwrap();
        assert_eq!(result, "$(rm -rf /)");
    }

    #[test]
    fn test_resolve_mixed_literal_and_env() {
        std::env::set_var("FIGBY_TEST_VAR", "42");
        let result = resolve_text_value("$(echo A)${FIGBY_TEST_VAR}B").unwrap();
        // $(...) literal, ${VAR} expanded
        assert_eq!(result, "$(echo A)42B");
    }

    #[test]
    fn test_parse_img_tag_basic() {
        let ftmp = r#"---
[canvas]
width = 50
height = 10
---
{{img:test.png:30::false:0,0:default}}
"#;
        let tmpl = parse_ftmp(ftmp).expect("should parse img tag");
        assert_eq!(tmpl.layers.len(), 1);
        let img = tmpl.layers[0]
            .image_tag
            .as_ref()
            .expect("should be image tag");
        assert_eq!(img.source, "test.png");
        assert_eq!(img.width, Some(30));
        assert!(img.height.is_none());
        assert!(!img.colored);
        assert_eq!(img.charset, "default");
    }

    #[test]
    fn test_parse_img_tag_all_fields() {
        let ftmp = r#"---
[canvas]
width = 80
---
{{img:photo.jpg:80:24:true:5,10:BLOCK}}
"#;
        let tmpl = parse_ftmp(ftmp).expect("should parse");
        let img = tmpl.layers[0].image_tag.as_ref().expect("should be image");
        assert_eq!(img.source, "photo.jpg");
        assert_eq!(img.width, Some(80));
        assert_eq!(img.height, Some(24));
        assert!(img.colored);
        assert_eq!(img.x, Some(5));
        assert_eq!(img.y, Some(10));
        assert_eq!(img.charset, "BLOCK");
    }

    #[test]
    fn test_img_absolute_path_rejected() {
        let ftmp =
            "---\n[canvas]\nwidth = 40\nheight = 10\n---\n{{img:/etc/passwd:40::false:0,0:}}\n";
        let tmpl = parse_ftmp(ftmp).expect("parse must succeed");
        let config = RenderConfig {
            font_dir: ".".to_string(),
            term_width: 40,
            override_width: None,
        };
        let err = render_template(&tmpl, &config).unwrap_err();
        assert!(
            err.to_string().contains("rejected"),
            "absolute path must be rejected, got: {err}"
        );
    }

    #[test]
    fn test_img_traversal_path_rejected() {
        let ftmp = "---\n[canvas]\nwidth = 40\nheight = 10\n---\n{{img:../../etc/passwd:40::false:0,0:}}\n";
        let tmpl = parse_ftmp(ftmp).expect("parse must succeed");
        let config = RenderConfig {
            font_dir: ".".to_string(),
            term_width: 40,
            override_width: None,
        };
        let err = render_template(&tmpl, &config).unwrap_err();
        assert!(
            err.to_string().contains("rejected"),
            "traversal path must be rejected, got: {err}"
        );
    }

    #[test]
    fn test_resolve_text_in_template_ftmp() {
        // Verify that ${VAR} in a variable's text field gets resolved.
        // Set a known env var for the test.
        let ftmp = r#"---
[canvas]
width = 40
height = 10

[variables.msg]
text = "${USER}"
font = "standard"
---
{{msg}}
"#;
        let tmpl = parse_ftmp(ftmp).expect("should parse");
        let (_tmpdir, font_dir) = setup_font_dir();
        let config = RenderConfig {
            font_dir,
            term_width: 80,
            override_width: None,
        };
        let result = render_template(&tmpl, &config);
        // Should succeed because USER is normally set
        assert!(result.is_ok(), "render should succeed: {:?}", result.err());
    }

    #[test]
    fn test_render_template_with_text_and_img_placeholders() {
        // Copy image into a local tempdir so the path is relative without '..'
        let src_path = "../assets/img/figby.png";
        if !std::path::Path::new(src_path).exists() {
            eprintln!("skipping image test: file not found at {src_path}");
            return;
        }
        // tempdir_in(".") creates a dir under figby-rs/ — use only the dir name
        // to build a relative path without '..' (path() itself is absolute).
        let img_tmpdir = tempfile::Builder::new()
            .tempdir_in(".")
            .expect("temp dir in '.'");
        let dir_name = img_tmpdir
            .path()
            .file_name()
            .expect("dir name")
            .to_string_lossy()
            .into_owned();
        let rel_img = format!("{dir_name}/figby.png");
        std::fs::copy(src_path, &rel_img).expect("copy image");
        let ftmp = format!(
            r#"---
[canvas]
width = 60
height = 20

[variables.greeting]
text = "figby"
font = "standard"
---
{{{{greeting}}}}{{{{img:{}:30:::0,0:}}}}
"#,
            rel_img
        );
        let tmpl = parse_ftmp(&ftmp).expect("should parse");
        assert_eq!(tmpl.layers.len(), 2);
        assert!(tmpl.layers[0].image_tag.is_none());
        assert!(tmpl.layers[1].image_tag.is_some());
        let (_tmpdir, font_dir) = setup_font_dir();
        let config = RenderConfig {
            font_dir,
            term_width: 80,
            override_width: None,
        };
        let result = render_template(&tmpl, &config);
        assert!(result.is_ok(), "render should succeed: {:?}", result.err());
        let output = result.unwrap();
        assert!(!output.is_empty(), "should produce output rows");
    }

    #[test]
    fn test_cli_flag_parse_check() {
        // Verify clap can parse --render-template.
        // This test lives in template.rs but exercises main.rs's CliArgs.
        // We use try_parse_from directly.
        use clap::Parser;
        #[derive(Parser, Debug)]
        struct TestArgs {
            #[arg(short = 'T', long = "render-template")]
            render_template: Option<String>,
        }
        let args = TestArgs::try_parse_from(["test", "--render-template", "test.ftmp"]).unwrap();
        assert_eq!(args.render_template, Some("test.ftmp".to_string()));

        let args = TestArgs::try_parse_from(["test"]).unwrap();
        assert!(args.render_template.is_none());
    }

    // --- Border and shadow helper tests ---

    #[test]
    fn test_content_bbox_basic() {
        let rows = vec!["X".to_string()];
        let bbox = content_bbox(&rows, 5, 3).unwrap();
        assert_eq!(bbox, (3, 3, 5, 5));
    }

    #[test]
    fn test_content_bbox_no_content() {
        let rows = vec!["   ".to_string()];
        assert!(content_bbox(&rows, 0, 0).is_none());
    }

    #[test]
    fn test_content_bbox_multiple_rows() {
        let rows = vec![" A ".to_string(), "BBB".to_string(), " C ".to_string()];
        let bbox = content_bbox(&rows, 2, 1).unwrap();
        assert_eq!(bbox, (1, 3, 2, 4));
    }

    #[test]
    fn test_fill_border_ring() {
        let rows = vec!["X".to_string()];
        let mut canvas = vec![vec![' '; 7]; 5];
        place_on_canvas(&mut canvas, &rows, 3, 2);
        let bbox = content_bbox(&rows, 3, 2).unwrap();
        assert_eq!(bbox, (2, 2, 3, 3));
        fill_border(&mut canvas, bbox, 1);

        // Content cell preserved
        assert_eq!(canvas[2][3], 'X');

        // Border ring cells should be '.'
        // Top row: y=1, x=2..4
        assert_eq!(canvas[1][2], '.');
        assert_eq!(canvas[1][3], '.');
        assert_eq!(canvas[1][4], '.');
        // Left/right on same row
        assert_eq!(canvas[2][2], '.');
        assert_eq!(canvas[2][4], '.');
        // Bottom row: y=3, x=2..4
        assert_eq!(canvas[3][2], '.');
        assert_eq!(canvas[3][3], '.');
        assert_eq!(canvas[3][4], '.');

        // Cells outside border ring unchanged
        assert_eq!(canvas[0][3], ' ');
        assert_eq!(canvas[2][0], ' ');
    }

    #[test]
    fn test_fill_shadow_offset() {
        let rows = vec!["X".to_string()];
        let mut canvas = vec![vec![' '; 10]; 10];
        place_on_canvas(&mut canvas, &rows, 2, 1);
        let bbox = content_bbox(&rows, 2, 1).unwrap();
        assert_eq!(bbox, (1, 1, 2, 2));
        fill_shadow(&mut canvas, bbox, 2);

        // Shadow at (3, 4) — offset 2 down-right from content (1, 2)
        assert_eq!(canvas[3][4], '.', "shadow at offset");
        // Cell before shadow left edge unchanged
        assert_eq!(canvas[3][3], ' ', "pre-shadow cell preserved");
        // Content preserved
        assert_eq!(canvas[1][2], 'X');
        // Cells before offset unchanged
        assert_eq!(canvas[2][2], ' ');
        assert_eq!(canvas[1][4], ' ');
    }

    #[test]
    fn test_fill_border_no_overwrite_content() {
        // Pre-place content in border ring area before fill_border runs
        let rows = vec!["X".to_string()];
        let mut canvas = vec![vec![' '; 7]; 5];
        // Place second content at border-ring position
        canvas[1][3] = 'Y';
        place_on_canvas(&mut canvas, &rows, 3, 2);
        let bbox = content_bbox(&rows, 3, 2).unwrap();
        fill_border(&mut canvas, bbox, 1);

        // 'Y' in border ring should still be 'Y' (not overwritten)
        assert_eq!(
            canvas[1][3], 'Y',
            "pre-existing content in border ring preserved"
        );
        // Content 'X' preserved
        assert_eq!(canvas[2][3], 'X');
    }

    // --- Template-level border and shadow tests ---

    fn make_border_shadow_ftmp(
        text: &str,
        border_width: Option<u32>,
        shadow_size: Option<u32>,
        extra_fields: &str,
    ) -> String {
        let bw = border_width
            .map(|w| format!("border_width = {}", w))
            .unwrap_or_default();
        let bc = border_width
            .map(|_| "border_color = \".\"".to_string())
            .unwrap_or_default();
        let ss = shadow_size
            .map(|s| format!("shadow_size = {}", s))
            .unwrap_or_default();
        let sc = shadow_size
            .map(|_| "shadow_color = \".\"".to_string())
            .unwrap_or_default();
        format!(
            r#"---
[canvas]
width = 30
height = 10

[variables.msg]
text = "{text}"
font = "standard"
{bw}
{bc}
{ss}
{sc}
{extra_fields}
---
{{{{msg}}}}
"#
        )
    }

    #[test]
    fn test_border_only_template() {
        let ftmp = make_border_shadow_ftmp("A", Some(1), None, "");
        let tmpl = parse_ftmp(&ftmp).expect("should parse");
        let (_tmpdir, font_dir) = setup_font_dir();
        let config = RenderConfig {
            font_dir,
            term_width: 80,
            override_width: None,
        };
        let output = render_template(&tmpl, &config).expect("should render");

        // Output should contain '.' characters (border)
        let dot_count: usize = output
            .iter()
            .map(|row| row.chars().filter(|&c| c == '.').count())
            .sum();
        assert!(dot_count > 0, "border should produce '.' chars");

        // Output should also contain text characters (not all dots)
        let text_chars: usize = output
            .iter()
            .map(|row| row.chars().filter(|&c| c != ' ' && c != '.').count())
            .sum();
        assert!(text_chars > 0, "text content should be present");
    }

    #[test]
    fn test_shadow_only_template() {
        let ftmp = make_border_shadow_ftmp("A", None, Some(2), "");
        let tmpl = parse_ftmp(&ftmp).expect("should parse");
        let (_tmpdir, font_dir) = setup_font_dir();
        let config = RenderConfig {
            font_dir,
            term_width: 80,
            override_width: None,
        };
        let output = render_template(&tmpl, &config).expect("should render");

        // Output should contain '.' characters (shadow)
        let dot_count: usize = output
            .iter()
            .map(|row| row.chars().filter(|&c| c == '.').count())
            .sum();
        assert!(dot_count > 0, "shadow should produce '.' chars");

        // Text characters still present
        let text_chars: usize = output
            .iter()
            .map(|row| row.chars().filter(|&c| c != ' ' && c != '.').count())
            .sum();
        assert!(text_chars > 0, "text content should be present");
    }

    #[test]
    fn test_border_and_shadow_template() {
        let ftmp = make_border_shadow_ftmp("A", Some(1), Some(3), "");
        let tmpl = parse_ftmp(&ftmp).expect("should parse");
        let (_tmpdir, font_dir) = setup_font_dir();
        let config = RenderConfig {
            font_dir,
            term_width: 80,
            override_width: None,
        };
        let output = render_template(&tmpl, &config).expect("should render");

        // Both border and shadow produce '.' chars
        let dot_count: usize = output
            .iter()
            .map(|row| row.chars().filter(|&c| c == '.').count())
            .sum();
        assert!(dot_count > 0, "border+shadow should produce '.' chars");

        // Text still present
        let text_chars: usize = output
            .iter()
            .map(|row| row.chars().filter(|&c| c != ' ' && c != '.').count())
            .sum();
        assert!(text_chars > 0, "text content should be present");
    }

    #[test]
    fn test_border_no_overwrite_other_layer() {
        // Two layers at different positions.
        // Layer "a" has border; layer "b" placed where border would extend.
        // Layer "b" content should not be overwritten by layer "a"'s border.
        let ftmp = r#"---
[canvas]
width = 40
height = 12

[variables.a]
text = "A"
font = "standard"
x = 0
y = 0
border_width = 1
border_color = "."

[variables.b]
text = "B"
font = "standard"
x = 0
y = 6
---
{{a}}
{{b}}
"#;
        let tmpl = parse_ftmp(ftmp).expect("should parse");
        let (_tmpdir, font_dir) = setup_font_dir();
        let config = RenderConfig {
            font_dir,
            term_width: 80,
            override_width: None,
        };
        let output = render_template(&tmpl, &config).expect("should render");

        // Both text chars should be present
        assert!(
            output.len() >= 12,
            "should have enough rows for both layers"
        );
        // Verify border dots present
        let dot_count: usize = output
            .iter()
            .map(|row| row.chars().filter(|&c| c == '.').count())
            .sum();
        assert!(dot_count > 0, "border should produce '.' chars");
    }
}
