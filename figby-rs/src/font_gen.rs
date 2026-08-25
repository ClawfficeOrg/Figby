use crate::font::{FIGcharacter, FIGfont, FontFormat, DEUTSCH_CHARS};
use font_kit::canvas::{Canvas, Format, RasterizationOptions};
use font_kit::error::{FontLoadingError, GlyphLoadingError, SelectionError};
use font_kit::font::Font;
use font_kit::handle::Handle;
use font_kit::hinting::HintingOptions;
use font_kit::source::SystemSource;
use image::{DynamicImage, GrayImage, Luma};
use pathfinder_geometry::transform2d::Transform2F;
use pathfinder_geometry::vector::{Vector2F, Vector2I};
use rascii_art::{charsets, RenderOptions};
use std::collections::HashMap;
use std::fmt;
use std::sync::OnceLock;

#[derive(Debug)]
pub enum FontGenError {
    Selection(SelectionError),
    FontLoading(FontLoadingError),
    GlyphLoading(GlyphLoadingError),
    FontNotFound(String),
    NoGlyph(u32),
}

impl fmt::Display for FontGenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FontGenError::Selection(e) => write!(f, "font selection error: {e}"),
            FontGenError::FontLoading(e) => write!(f, "font loading error: {e}"),
            FontGenError::GlyphLoading(e) => write!(f, "glyph loading error: {e}"),
            FontGenError::FontNotFound(name) => write!(f, "font not found: {name}"),
            FontGenError::NoGlyph(code) => write!(f, "no glyph for char code {code}"),
        }
    }
}

impl std::error::Error for FontGenError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            FontGenError::Selection(e) => Some(e),
            FontGenError::FontLoading(e) => Some(e),
            FontGenError::GlyphLoading(e) => Some(e),
            _ => None,
        }
    }
}

impl From<SelectionError> for FontGenError {
    fn from(e: SelectionError) -> Self {
        FontGenError::Selection(e)
    }
}

impl From<FontLoadingError> for FontGenError {
    fn from(e: FontLoadingError) -> Self {
        FontGenError::FontLoading(e)
    }
}

impl From<GlyphLoadingError> for FontGenError {
    fn from(e: GlyphLoadingError) -> Self {
        FontGenError::GlyphLoading(e)
    }
}

#[derive(Debug, Clone)]
pub struct FontFamilyInfo {
    pub family: String,
    pub styles: Vec<String>,
}

fn describe_style(font: &Font) -> String {
    let props = font.properties();
    format!(
        "Weight: {}, Style: {:?}",
        props.weight.0 as u32, props.style
    )
}

fn family_is_monospace(name: &str, source: &SystemSource) -> bool {
    if name.to_lowercase().contains("mono") {
        return true;
    }
    if let Ok(family_handle) = source.select_family_by_name(name) {
        let handles = family_handle.fonts();
        if let Some(handle) = handles.first() {
            if let Ok(font) = handle.load() {
                return font.is_monospace();
            }
        }
    }
    false
}

fn load_styles(handles: &[Handle]) -> Vec<String> {
    handles
        .iter()
        .filter_map(|handle| {
            let font = handle.load().ok()?;
            Some(describe_style(&font))
        })
        .collect()
}

pub fn list_system_fonts() -> Result<Vec<FontFamilyInfo>, FontGenError> {
    let source = SystemSource::new();
    let family_names = source.all_families()?;
    let mut result = Vec::with_capacity(family_names.len());

    for name in family_names {
        let styles = match source.select_family_by_name(&name) {
            Ok(family_handle) => load_styles(family_handle.fonts()),
            Err(_) => Vec::new(),
        };
        result.push(FontFamilyInfo {
            family: name,
            styles,
        });
    }

    Ok(result)
}

pub fn list_monospace_fonts() -> Result<Vec<FontFamilyInfo>, FontGenError> {
    let source = SystemSource::new();
    let family_names = source.all_families()?;
    let mut result = Vec::new();

    for name in family_names {
        if family_is_monospace(&name, &source) {
            let styles = match source.select_family_by_name(&name) {
                Ok(family_handle) => load_styles(family_handle.fonts()),
                Err(_) => Vec::new(),
            };
            result.push(FontFamilyInfo {
                family: name,
                styles,
            });
        }
    }

    Ok(result)
}

/// Generate FIGfont header line from font metrics.
///
/// Format: `flf2a<hardblank> <height> <baseline> <max_length> <old_layout> <comment_lines> [<print_direction> [<full_layout> [<codetag_count>]]]`
///
/// Generated header uses old_layout=0 (full-size), comment_lines=0,
/// print_direction (from the font struct), and codetag_count=0.
pub fn generate_figfont_header(font: &FIGfont) -> String {
    format!(
        "flf2a{hb} {h} {b} {ml} 0 0 {pd} {fl} 0",
        hb = font.hardblank,
        h = font.charheight,
        b = font.baseline,
        ml = font.maxlength,
        pd = font.print_direction,
        fl = font.full_layout,
    )
}

/// Generate complete .flf file content from font metrics and glyph data.
///
/// Produces header + all 95 required ASCII chars (codes 32-126), 7 Deutsch
/// chars, and any codetagged chars. Missing required chars use space-padded
/// rows of `maxlength` width. Each row terminated with `@`.
pub fn generate_figfont(font: &FIGfont) -> String {
    let mut result = String::new();
    result.push_str(&generate_figfont_header(font));
    result.push('\n');

    let height = font.charheight as usize;
    let pad_row = " ".repeat(font.maxlength as usize);

    for code in 32..=126u32 {
        let ch = font.chars.get(&code);
        for row_i in 0..height {
            let row = ch
                .and_then(|c| c.rows().get(row_i))
                .map(|s| s.as_str())
                .unwrap_or(&pad_row);
            result.push_str(row);
            result.push_str("@\n");
        }
    }

    for &code in &DEUTSCH_CHARS {
        let ch = font.chars.get(&code);
        for row_i in 0..height {
            let row = ch
                .and_then(|c| c.rows().get(row_i))
                .map(|s| s.as_str())
                .unwrap_or(&pad_row);
            result.push_str(row);
            result.push_str("@\n");
        }
    }

    let mut codetagged: Vec<u32> = font
        .chars
        .keys()
        .copied()
        .filter(|code| !(32..=126).contains(code) && !DEUTSCH_CHARS.contains(code))
        .collect();
    codetagged.sort_unstable();

    for code in codetagged {
        let ch = &font.chars[&code];
        result.push_str(&code.to_string());
        result.push('\n');
        for row in ch.rows() {
            result.push_str(row);
            result.push_str("@\n");
        }
    }

    result
}

/// Built-in "smooth" charset: light marks + round chars for smooth antialiased edges.
/// Avoids `@` (FIGfont endmark) and `$` (hardblank) to prevent output corruption.
pub const SMOOTH_CHARSET: &[&str] = &[
    " ", ".", "'", "^", "\"", "~", ":", ";", "i", "r", "o", "O", "0", "Q", "#", "8", "&", "%",
];

// ── Extended charsets (Unicode) ────────────────────────────────────────────

/// Leak a `String` into a `&'static str`. Used once per char during init.
fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

fn make_charset_vec(codepoints: impl Iterator<Item = u32>) -> Vec<&'static str> {
    codepoints
        .filter_map(char::from_u32)
        .map(|c| leak(c.to_string()))
        .collect()
}

/// U+2800–U+28FF sorted by number of dots (bits set in the low byte), then code point.
fn braille_charset() -> &'static [&'static str] {
    static CELL: OnceLock<Vec<&'static str>> = OnceLock::new();
    CELL.get_or_init(|| {
        let mut cps: Vec<u32> = (0x2800u32..=0x28FFu32).collect();
        cps.sort_by_key(|&cp| ((cp & 0xFF).count_ones(), cp));
        make_charset_vec(cps.into_iter())
    })
}

/// Block elements U+2580–U+259F, ordered by luminance (light → dark).
fn blocks_charset() -> &'static [&'static str] {
    static CELL: OnceLock<Vec<&'static str>> = OnceLock::new();
    CELL.get_or_init(|| {
        let cps: Vec<u32> = vec![
            // ~12.5% coverage
            0x2581, 0x258F, 0x2594, 0x2595, // ~25% coverage
            0x2582, 0x258E, 0x2596, 0x2597, 0x2598, 0x259D, // ~37.5% coverage
            0x2583, 0x258D, // ~50% coverage
            0x2584, 0x2580, 0x258C, 0x2590, 0x2591, 0x259A, 0x259E, // ~62.5% coverage
            0x2585, 0x258B, // ~66% coverage
            0x2592, // ~75% coverage
            0x2586, 0x258A, 0x2593, 0x2599, 0x259B, 0x259C, 0x259F, // ~87.5% coverage
            0x2587, 0x2589, // 100% coverage
            0x2588,
        ];
        make_charset_vec(cps.into_iter())
    })
}

/// Dithered/shade chars U+2591–U+2593 (░▒▓).
fn dithered_charset() -> &'static [&'static str] {
    static CELL: OnceLock<Vec<&'static str>> = OnceLock::new();
    CELL.get_or_init(|| make_charset_vec(0x2591u32..=0x2593u32))
}

/// Geometric shapes subset U+25A0–U+25FF: squares, triangles, diamonds, circles.
fn geometric_charset() -> &'static [&'static str] {
    static CELL: OnceLock<Vec<&'static str>> = OnceLock::new();
    CELL.get_or_init(|| {
        let cps: Vec<u32> = vec![
            0x25A0, 0x25A1, 0x25AA, 0x25AB, // squares
            0x25B2, 0x25B3, 0x25B6, 0x25B7, // triangles
            0x25BC, 0x25BD, 0x25C0, 0x25C1, // triangles
            0x25C6, 0x25C7, 0x25C8, // diamonds
            0x25CA, // lozenge
            0x25CB, 0x25CE, 0x25CF, // circles
            0x25D0, 0x25D1, // half circles
            0x25E6, 0x25EF, // white bullet, large circle
        ];
        make_charset_vec(cps.into_iter())
    })
}

/// Box drawing + selected geometric shapes.
fn box_charset() -> &'static [&'static str] {
    static CELL: OnceLock<Vec<&'static str>> = OnceLock::new();
    CELL.get_or_init(|| {
        let mut cps: Vec<u32> = (0x2500u32..=0x257Fu32).collect();
        // Add selected geometric shapes
        cps.extend([
            0x25A0u32, 0x25A1, 0x25AA, 0x25AB, 0x25B2, 0x25B3, 0x25C6, 0x25C7,
        ]);
        make_charset_vec(cps.into_iter())
    })
}

/// Ogham script block U+1680–U+169C.
fn ogham_charset() -> &'static [&'static str] {
    static CELL: OnceLock<Vec<&'static str>> = OnceLock::new();
    CELL.get_or_init(|| make_charset_vec(0x1680u32..=0x169Cu32))
}

/// Full: ASCII printable + blocks, with full block at the end.
/// Clean gradient from space → █ without cluttering Unicode symbols.
fn full_charset() -> &'static [&'static str] {
    static CELL: OnceLock<Vec<&'static str>> = OnceLock::new();
    CELL.get_or_init(|| {
        let mut v: Vec<&'static str> = Vec::new();
        v.extend(make_charset_vec(0x0020u32..=0x007Eu32));
        v.extend_from_slice(blocks_charset());
        v
    })
}

/// Deluxe: ASCII printable + blocks + box + braille + ogham.
/// Full block U+2588 is forced to the end so darkest pixels fill solid.
fn deluxe_charset() -> &'static [&'static str] {
    static CELL: OnceLock<Vec<&'static str>> = OnceLock::new();
    CELL.get_or_init(|| {
        let mut v: Vec<&'static str> = Vec::new();
        // ASCII printable (space through ~)
        v.extend(make_charset_vec(0x0020u32..=0x007Eu32));
        v.extend_from_slice(blocks_charset());
        v.extend_from_slice(box_charset());
        v.extend_from_slice(braille_charset());
        v.extend_from_slice(ogham_charset());
        v.extend_from_slice(dithered_charset());
        v.extend_from_slice(geometric_charset());
        // Full block last so darkest pixels always fill solid
        v.push("█");
        v
    })
}

/// Resolve a charset name to a character slice for font generation.
/// Built-in names: `block`, `default`, `slight`, `smooth`,
/// `braille`, `blocks`, `box`, `ogham`, `deluxe`, `full`,
/// `dithered`, `geometric`.
pub fn resolve_charset(name: &str) -> Option<&'static [&'static str]> {
    Some(match name {
        "block" => charsets::BLOCK,
        "default" => charsets::DEFAULT,
        "slight" => charsets::SLIGHT,
        "smooth" => SMOOTH_CHARSET,
        "braille" => braille_charset(),
        "blocks" => blocks_charset(),
        "box" => box_charset(),
        "ogham" => ogham_charset(),
        "dithered" => dithered_charset(),
        "geometric" => geometric_charset(),
        "deluxe" => deluxe_charset(),
        "full" => full_charset(),
        _ => return None,
    })
}

/// Convert a cell-sized canvas to a FIGcharacter using rascii_art.
///
/// The canvas is already sized to the FIGfont cell (charheight × glyph_width).
/// Uses the given `charset` for luminance-to-character mapping.
fn canvas_to_figcharacter_cell(
    canvas: &Canvas,
    charheight: usize,
    charset: &[&str],
) -> FIGcharacter {
    let canvas_h = canvas.size.y() as usize;
    let canvas_w = canvas.size.x() as usize;
    let stride = canvas.stride;

    let mut img = GrayImage::new(canvas_w as u32, canvas_h as u32);
    for r in 0..canvas_h.min(charheight) {
        let row_start = r * stride;
        for c in 0..canvas_w {
            let alpha = canvas.pixels[row_start + c];
            img.put_pixel(c as u32, r as u32, Luma([alpha]));
        }
    }

    let options = RenderOptions::new()
        .width(canvas_w as u32)
        .height(canvas_h as u32)
        .charset(charset);

    let mut buf = String::new();
    if rascii_art::render_image_to(&DynamicImage::ImageLuma8(img), &mut buf, &options).is_err() {
        let rows = vec![" ".repeat(canvas_w); charheight];
        return FIGcharacter::from(rows);
    }

    let mut rows: Vec<String> = buf.lines().map(|s| s.to_string()).collect();
    while rows.len() < charheight {
        rows.push(" ".repeat(canvas_w));
    }
    FIGcharacter::from(rows)
}

/// Shared glyph rendering: rasterize all required chars from a loaded font-kit `Font`.
const POINT_SIZE_MIN: f32 = 4.0;
const POINT_SIZE_MAX: f32 = 200.0;

fn render_font_glyphs(
    font: &Font,
    point_size: f32,
    charset: &[&str],
) -> Result<(FIGfont, u32), FontGenError> {
    let point_size = point_size.clamp(POINT_SIZE_MIN, POINT_SIZE_MAX);
    let metrics = font.metrics();
    let upem = metrics.units_per_em as f32;
    let scale = point_size / upem;
    let ascent_px = (metrics.ascent * scale).ceil() as u32;
    let descent_px = ((-metrics.descent) * scale).ceil() as u32;
    let charheight = (ascent_px + descent_px).max(1);
    let baseline = ascent_px;

    let mut figchars = HashMap::new();
    let mut maxlength = 0u32;

    let all_chars: Vec<u32> = (32u32..=126).chain(DEUTSCH_CHARS.iter().copied()).collect();

    let hinting = HintingOptions::None;
    let raster_opts = RasterizationOptions::GrayscaleAa;

    for &code in &all_chars {
        let make_blank = |figchars: &mut HashMap<u32, FIGcharacter>, code: u32| {
            let blank_row = " ".to_string();
            figchars.insert(
                code,
                FIGcharacter::from(vec![blank_row; charheight as usize]),
            );
        };

        let c = match char::from_u32(code) {
            Some(c) => c,
            None => {
                make_blank(&mut figchars, code);
                continue;
            }
        };
        let glyph_id = match font.glyph_for_char(c) {
            Some(id) => id,
            None => {
                make_blank(&mut figchars, code);
                continue;
            }
        };

        // Use raster_bounds to check bounds, then get advance for cell width.
        // advance() returns font units (font-kit sets char size to upem during
        // font init). Scale by point_size / upem to get pixel advance.
        let bounds = match font.raster_bounds(
            glyph_id,
            point_size,
            Transform2F::default(),
            hinting,
            raster_opts,
        ) {
            Ok(b) => b,
            Err(_) => {
                make_blank(&mut figchars, code);
                continue;
            }
        };

        let advance_v = match font.advance(glyph_id) {
            Ok(v) => v,
            Err(_) => {
                make_blank(&mut figchars, code);
                continue;
            }
        };
        // advance() returns font units (char size set to upem during font init).
        // Scale by point_size / upem to get pixel advance.
        let advance_px = advance_v.x() * point_size / upem;
        let cell_w = (advance_px.ceil() as i32).max(1);

        let size = bounds.size();
        if size.x() <= 0 || size.y() <= 0 {
            let blank_row = " ".repeat(cell_w as usize);
            let ch = FIGcharacter::from(vec![blank_row; charheight as usize]);
            figchars.insert(code, ch);
            continue;
        }

        // Allocate canvas at advance width so all characters have consistent
        // cell dimensions matching the font's horizontal advance metric.
        // The transform shifts the baseline down by `baseline` pixels so the
        // rendered bitmap lands at the correct vertical position within the cell.
        let canvas_size = Vector2I::new(cell_w, charheight as i32);
        let mut canvas = Canvas::new(canvas_size, Format::A8);

        // Shift baseline to row `baseline` in the cell-sized canvas.
        // font-kit converts pathfinder +y=down to FreeType +y=up internally,
        // so a positive y shift here moves the render downward in the canvas.
        let shifted_transform = Transform2F {
            vector: Vector2F::new(0.0, baseline as f32),
            ..Default::default()
        };

        font.rasterize_glyph(
            &mut canvas,
            glyph_id,
            point_size,
            shifted_transform,
            hinting,
            raster_opts,
        )
        .map_err(|_| FontGenError::NoGlyph(code))?;

        let ch = canvas_to_figcharacter_cell(&canvas, charheight as usize, charset);
        let width = ch.width() as u32;
        if width > maxlength {
            maxlength = width;
        }
        figchars.insert(code, ch);
    }

    let hardblank = '$';
    Ok((
        FIGfont {
            format: FontFormat::Figfont,
            hardblank,
            charheight,
            baseline,
            maxlength,
            old_layout: 0,
            full_layout: 64,
            print_direction: 0,
            comment_lines: 0,
            chars: figchars,
            codetag_count: 0,
        },
        maxlength,
    ))
}

/// Load a system font by **family name** and render as a FIGfont.
///
/// Searches system-installed fonts via font-kit's `SystemSource`.
/// For font files (.ttf/.otf), use [`font_file_to_figfont`].
pub fn system_font_to_figfont(
    name: &str,
    point_size: f32,
    charset: &[&str],
) -> Result<FIGfont, FontGenError> {
    let source = SystemSource::new();
    let family = source
        .select_family_by_name(name)
        .map_err(|_| FontGenError::FontNotFound(name.to_string()))?;
    let handle = family
        .fonts()
        .first()
        .ok_or_else(|| FontGenError::FontNotFound(name.to_string()))?;
    let font = handle.load()?;
    let (figfont, _maxlength) = render_font_glyphs(&font, point_size, charset)?;
    Ok(figfont)
}

/// Load a font from a **file path** (.ttf, .otf) and render as a FIGfont.
pub fn font_file_to_figfont(
    path: &std::path::Path,
    point_size: f32,
    charset: &[&str],
) -> Result<FIGfont, FontGenError> {
    use std::sync::Arc;
    let data = std::fs::read(path).map_err(|e| FontGenError::FontNotFound(format!("{e}")))?;
    let handle = font_kit::handle::Handle::from_memory(Arc::new(data), 0);
    let font = handle.load()?;
    let (figfont, _maxlength) = render_font_glyphs(&font, point_size, charset)?;
    Ok(figfont)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::{parse_header, parse_tlf_font, FIGcharacter};

    #[test]
    fn test_list_system_fonts_not_empty() {
        let fonts = list_system_fonts().expect("system font enumeration should succeed");
        assert!(
            !fonts.is_empty(),
            "at least one font family should be found"
        );
    }

    #[test]
    fn test_list_monospace_fonts_returns_subset() {
        let fonts = list_monospace_fonts().expect("monospace font enumeration should succeed");
        assert!(
            !fonts.is_empty(),
            "at least one monospace font family should be found"
        );
        for info in &fonts {
            let is_mono =
                info.family.to_lowercase().contains("mono") || is_any_font_monospace(&info.family);
            assert!(is_mono, "family '{}' should be monospace", info.family);
        }
    }

    #[test]
    fn test_font_family_info_has_styles() {
        let fonts = list_system_fonts().expect("system font enumeration should succeed");
        let any_with_styles = fonts.iter().any(|f| !f.styles.is_empty());
        assert!(
            any_with_styles,
            "at least one family should have non-empty styles"
        );
    }

    fn is_any_font_monospace(name: &str) -> bool {
        let source = SystemSource::new();
        if let Ok(family_handle) = source.select_family_by_name(name) {
            if let Some(handle) = family_handle.fonts().first() {
                if let Ok(font) = handle.load() {
                    return font.is_monospace();
                }
            }
        }
        false
    }

    // --- FIGfont header generation tests ---

    #[test]
    fn test_generate_header_roundtrip() {
        let font = FIGfont {
            hardblank: '$',
            charheight: 8,
            baseline: 7,
            maxlength: 15,
            full_layout: 64,
            ..FIGfont::default()
        };
        let header = generate_figfont_header(&font);
        let parsed = parse_header(&header).expect("should parse generated header");

        assert_eq!(parsed.hardblank, '$');
        assert_eq!(parsed.charheight, 8);
        assert_eq!(parsed.baseline, 7);
        assert_eq!(parsed.maxlength, 15);
        assert_eq!(parsed.old_layout, 0);
        assert_eq!(parsed.comment_lines, 0);
        assert_eq!(parsed.print_direction, -1);
        assert_eq!(parsed.full_layout, 64);
        assert_eq!(parsed.codetag_count, 0);
    }

    #[test]
    fn test_generate_header_defaults_full_size() {
        let font = FIGfont::default();
        let header = generate_figfont_header(&font);
        let parsed = parse_header(&header).expect("should parse generated header");

        assert_eq!(parsed.old_layout, 0);
        assert_eq!(parsed.full_layout, 0);
        assert_eq!(parsed.print_direction, -1);
        assert_eq!(parsed.codetag_count, 0);
    }

    #[test]
    fn test_generate_header_smush_layout() {
        let font = FIGfont {
            hardblank: '$',
            charheight: 6,
            baseline: 5,
            maxlength: 20,
            full_layout: 191,
            ..FIGfont::default()
        };
        let header = generate_figfont_header(&font);
        let parsed = parse_header(&header).expect("should parse generated header");
        assert_eq!(parsed.full_layout, 191);
    }

    #[test]
    fn test_generate_header_hardblank_multi_byte() {
        let font = FIGfont {
            hardblank: '\u{7f}',
            charheight: 3,
            baseline: 3,
            maxlength: 8,
            full_layout: 0,
            ..FIGfont::default()
        };
        let header = generate_figfont_header(&font);
        assert!(
            header.starts_with("flf2a\u{7f}"),
            "header should start with flf2a + hardblank DEL, got: {header:?}"
        );
        let parsed =
            parse_header(&header).expect("should parse generated header with DEL hardblank");
        assert_eq!(parsed.hardblank, '\u{7f}');
        assert_eq!(parsed.charheight, 3);
    }

    #[test]
    fn test_generate_figfont_full_roundtrip() {
        let mut font = FIGfont {
            hardblank: '$',
            charheight: 2,
            baseline: 1,
            maxlength: 10,
            full_layout: 64,
            ..FIGfont::default()
        };

        font.chars.insert(
            32,
            FIGcharacter::from(vec!["   ".to_string(), "   ".to_string()]),
        );
        font.chars.insert(
            65,
            FIGcharacter::from(vec!["  A  ".to_string(), " AAA ".to_string()]),
        );

        font.chars.insert(
            200,
            FIGcharacter::from(vec![" char ".to_string(), " 200  ".to_string()]),
        );

        let content = generate_figfont(&font);
        let parsed = parse_tlf_font(&content).expect("should parse generated .flf content");

        assert_eq!(parsed.hardblank, font.hardblank);
        assert_eq!(parsed.charheight, font.charheight);
        assert_eq!(parsed.baseline, font.baseline);
        assert_eq!(parsed.maxlength, font.maxlength);
        assert_eq!(parsed.old_layout, 0);
        assert_eq!(parsed.full_layout, font.full_layout);
        assert_eq!(parsed.print_direction, -1);
        assert_eq!(parsed.codetag_count, 0);

        // 102 required chars + 1 codetagged = 103
        assert!(
            parsed.chars.len() >= 103,
            "should have at least 103 chars, got {}",
            parsed.chars.len()
        );

        let ch65 = parsed
            .chars
            .get(&65)
            .expect("code 65 (A) should exist in output");
        assert_eq!(ch65.rows(), &["  A  ", " AAA "]);

        let ch200 = parsed
            .chars
            .get(&200)
            .expect("code 200 should exist in output");
        assert_eq!(ch200.rows(), &[" char ", " 200  "]);
    }

    #[test]
    fn test_generate_figfont_empty_chars() {
        let font = FIGfont {
            hardblank: '$',
            charheight: 3,
            baseline: 2,
            maxlength: 8,
            full_layout: 64,
            chars: std::collections::HashMap::new(),
            ..FIGfont::default()
        };

        let content = generate_figfont(&font);
        let parsed = parse_tlf_font(&content).expect("should parse generated font with no chars");

        assert_eq!(parsed.charheight, 3);
        assert_eq!(parsed.maxlength, 8);
        // all 102 required chars should exist as placeholders
        assert_eq!(parsed.chars.len(), 102);

        // Verify some placeholder chars have correct row count
        let ch32 = parsed.chars.get(&32).expect("space should exist");
        assert_eq!(ch32.rows().len(), 3);
        let ch126 = parsed.chars.get(&126).expect("tilde should exist");
        assert_eq!(ch126.rows().len(), 3);
    }

    // --- Font generation tests (require system fonts) ---

    fn test_charset() -> &'static [&'static str] {
        SMOOTH_CHARSET
    }

    #[test]
    fn test_create_font_roundtrip() {
        match system_font_to_figfont("Monospace", 12.0, test_charset()) {
            Ok(font) => {
                assert!(font.charheight > 0, "charheight should be > 0");
                assert!(font.baseline > 0, "baseline should be > 0");
                assert!(font.maxlength > 0, "maxlength should be > 0");
                assert!(font.charheight >= font.baseline, "charheight >= baseline");
            }
            Err(FontGenError::FontNotFound(_)) => {
                eprintln!("Monospace font not found, skipping test");
            }
            Err(e) => panic!("unexpected error: {e}"),
        }
    }

    #[test]
    fn test_create_font_parseable() {
        match system_font_to_figfont("Monospace", 12.0, test_charset()) {
            Ok(font) => {
                let content = generate_figfont(&font);
                let parsed = parse_tlf_font(&content).expect("generated font should parse");
                assert_eq!(parsed.chars.len(), 102, "should have 102 required chars");
                for code in 32..=126u32 {
                    assert!(
                        parsed.chars.contains_key(&code),
                        "missing ASCII char code {code}"
                    );
                }
            }
            Err(FontGenError::FontNotFound(_)) => {
                eprintln!("Monospace font not found, skipping test");
            }
            Err(e) => panic!("unexpected error: {e}"),
        }
    }

    #[test]
    fn test_create_font_renders_text() {
        match system_font_to_figfont("Monospace", 12.0, test_charset()) {
            Ok(font) => {
                let content = generate_figfont(&font);
                let parsed = parse_tlf_font(&content).expect("generated font should parse");

                // Try to render a simple character 'H' through the font
                let ch = parsed
                    .chars
                    .get(&72)
                    .expect("char code 72 (H) should exist");
                assert!(!ch.rows().is_empty(), "H character should have rows");
                for row in ch.rows() {
                    assert_eq!(row.len(), ch.width(), "row width should match");
                }
            }
            Err(FontGenError::FontNotFound(_)) => {
                eprintln!("Monospace font not found, skipping test");
            }
            Err(e) => panic!("unexpected error: {e}"),
        }
    }

    #[test]
    fn test_create_font_nonexistent_name() {
        let result = system_font_to_figfont("NonexistentFontXYZ_12345", 12.0, test_charset());
        assert!(
            matches!(result, Err(FontGenError::FontNotFound(_))),
            "expected FontNotFound error, got: {result:?}"
        );
    }

    #[test]
    fn test_create_font_size_changes_metrics() {
        let small = match system_font_to_figfont("Monospace", 8.0, test_charset()) {
            Ok(f) => f,
            Err(FontGenError::FontNotFound(_)) => {
                eprintln!("Monospace font not found, skipping test");
                return;
            }
            Err(e) => panic!("unexpected error: {e}"),
        };
        let large = match system_font_to_figfont("Monospace", 24.0, test_charset()) {
            Ok(f) => f,
            Err(FontGenError::FontNotFound(_)) => {
                eprintln!("Monospace font not found, skipping test");
                return;
            }
            Err(e) => panic!("unexpected error: {e}"),
        };

        assert!(
            large.charheight > small.charheight,
            "larger font size should have greater charheight ({} > {})",
            large.charheight,
            small.charheight
        );
    }

    // --- Braille charset tests ---

    #[test]
    fn test_braille_charset_count_256() {
        assert_eq!(braille_charset().len(), 256);
    }

    #[test]
    fn test_braille_charset_all_in_range() {
        for s in braille_charset() {
            let c = s.chars().next().expect("each entry should be one char");
            let cp = c as u32;
            assert!(
                (0x2800..=0x28FF).contains(&cp),
                "braille char U+{cp:04X} outside U+2800–U+28FF"
            );
        }
    }

    #[test]
    fn test_braille_charset_sorted_by_dots() {
        let chars = braille_charset();
        let cps: Vec<u32> = chars
            .iter()
            .map(|s| s.chars().next().unwrap() as u32)
            .collect();
        for pair in cps.windows(2) {
            let a = pair[0];
            let b = pair[1];
            let key_a = ((a & 0xFF).count_ones(), a);
            let key_b = ((b & 0xFF).count_ones(), b);
            assert!(
                key_a <= key_b,
                "sort violation: U+{:04X} (dots={}, code={}) before U+{:04X} (dots={}, code={})",
                a,
                (a & 0xFF).count_ones(),
                a,
                b,
                (b & 0xFF).count_ones(),
                b
            );
        }
    }

    #[test]
    fn test_braille_charset_all_256_unique() {
        let chars = braille_charset();
        let mut cps: Vec<u32> = chars
            .iter()
            .map(|s| s.chars().next().unwrap() as u32)
            .collect();
        cps.sort_unstable();
        cps.dedup();
        assert_eq!(cps.len(), 256, "should have 256 unique codepoints");
        assert_eq!(cps[0], 0x2800, "first codepoint should be U+2800");
        assert_eq!(cps[255], 0x28FF, "last codepoint should be U+28FF");
        // Verify no gaps
        for (i, &cp) in cps.iter().enumerate() {
            assert_eq!(
                cp,
                0x2800 + i as u32,
                "missing codepoint U+{:04X}",
                0x2800 + i as u32
            );
        }
    }

    #[test]
    fn test_blocks_count_32() {
        let chars = blocks_charset();
        assert_eq!(chars.len(), 32, "blocks charset should have 32 entries");
    }

    #[test]
    fn test_blocks_all_in_range() {
        let chars = blocks_charset();
        for s in chars {
            let cp = s.chars().next().unwrap() as u32;
            assert!(
                (0x2580..=0x259F).contains(&cp),
                "blocks char U+{cp:04X} outside U+2580–U+259F"
            );
        }
    }

    #[test]
    fn test_blocks_all_unique() {
        let chars = blocks_charset();
        let mut cps: Vec<u32> = chars
            .iter()
            .map(|s| s.chars().next().unwrap() as u32)
            .collect();
        assert_eq!(cps.len(), 32, "should have 32 entries");
        cps.sort_unstable();
        cps.dedup();
        assert_eq!(cps.len(), 32, "should have 32 unique codepoints");
        assert_eq!(cps[0], 0x2580, "first codepoint should be U+2580");
        assert_eq!(cps[31], 0x259F, "last codepoint should be U+259F");
        // Verify no gaps
        for (i, &cp) in cps.iter().enumerate() {
            assert_eq!(
                cp,
                0x2580 + i as u32,
                "missing codepoint U+{:04X}",
                0x2580 + i as u32
            );
        }
    }

    // --- Dithered charset tests ---

    #[test]
    fn test_dithered_count_3() {
        assert_eq!(dithered_charset().len(), 3);
    }

    #[test]
    fn test_dithered_all_in_range() {
        for s in dithered_charset() {
            let cp = s.chars().next().unwrap() as u32;
            assert!(
                (0x2591..=0x2593).contains(&cp),
                "dithered char U+{cp:04X} outside U+2591–U+2593"
            );
        }
    }

    #[test]
    fn test_dithered_all_unique() {
        let chars = dithered_charset();
        let mut cps: Vec<u32> = chars
            .iter()
            .map(|s| s.chars().next().unwrap() as u32)
            .collect();
        assert_eq!(cps.len(), 3, "should have 3 entries");
        cps.sort_unstable();
        cps.dedup();
        assert_eq!(cps.len(), 3, "should have 3 unique codepoints");
        assert_eq!(cps[0], 0x2591, "first should be U+2591");
        assert_eq!(cps[2], 0x2593, "last should be U+2593");
        for (i, &cp) in cps.iter().enumerate() {
            assert_eq!(
                cp,
                0x2591 + i as u32,
                "missing codepoint U+{:04X}",
                0x2591 + i as u32
            );
        }
    }

    // --- Geometric charset tests ---

    #[test]
    fn test_geometric_count_23() {
        assert_eq!(geometric_charset().len(), 23);
    }

    #[test]
    fn test_geometric_all_in_range() {
        for s in geometric_charset() {
            let cp = s.chars().next().unwrap() as u32;
            assert!(
                (0x25A0..=0x25FF).contains(&cp),
                "geometric char U+{cp:04X} outside U+25A0–U+25FF"
            );
        }
    }

    #[test]
    fn test_geometric_all_unique() {
        let chars = geometric_charset();
        let mut cps: Vec<u32> = chars
            .iter()
            .map(|s| s.chars().next().unwrap() as u32)
            .collect();
        assert_eq!(cps.len(), 23, "should have 23 entries");
        cps.sort_unstable();
        cps.dedup();
        assert_eq!(cps.len(), 23, "should have 23 unique codepoints");
    }

    // --- Ogham charset tests ---

    #[test]
    fn test_ogham_count() {
        assert_eq!(ogham_charset().len(), 29);
    }

    #[test]
    fn test_ogham_all_in_range() {
        for s in ogham_charset() {
            let cp = s.chars().next().unwrap() as u32;
            assert!(
                (0x1680..=0x169F).contains(&cp),
                "ogham char U+{cp:04X} outside U+1680–U+169F"
            );
        }
    }

    #[test]
    fn test_ogham_all_unique() {
        let chars = ogham_charset();
        let mut cps: Vec<u32> = chars
            .iter()
            .map(|s| s.chars().next().unwrap() as u32)
            .collect();
        assert_eq!(cps.len(), 29, "should have 29 entries");
        cps.sort_unstable();
        cps.dedup();
        assert_eq!(cps.len(), 29, "should have 29 unique codepoints");
        assert_eq!(cps[0], 0x1680, "first codepoint should be U+1680");
        assert_eq!(cps[28], 0x169C, "last codepoint should be U+169C");
        for (i, &cp) in cps.iter().enumerate() {
            assert_eq!(
                cp,
                0x1680 + i as u32,
                "missing codepoint U+{:04X}",
                0x1680 + i as u32
            );
        }
    }

    #[test]
    fn test_font_file_to_figfont_smoke() {
        use std::io::Write;
        static FONT_BYTES: &[u8] = include_bytes!("../tests/fixtures/FiraMono-Regular.ttf");
        let mut tmp = tempfile::NamedTempFile::new().expect("tempfile");
        tmp.write_all(FONT_BYTES).expect("write");
        let charset = test_charset();
        let font = font_file_to_figfont(tmp.path(), 16.0, charset)
            .expect("bundled TTF should convert to FIGfont");
        assert!(font.chars.len() >= 50, "should have at least 50 glyphs");
        assert!(font.charheight > 0, "charheight should be positive");
    }

    #[test]
    fn test_font_file_to_figfont_malformed() {
        use std::io::Write;
        let mut tmp = tempfile::NamedTempFile::new().expect("tempfile");
        tmp.write_all(b"not a font file \x00\x01\x02\x03")
            .expect("write");
        let charset = &["A", "B"];
        let result = font_file_to_figfont(tmp.path(), 16.0, charset);
        assert!(result.is_err(), "malformed bytes should return Err");
    }

    #[test]
    fn test_point_size_clamping() {
        use std::io::Write;
        static FONT_BYTES: &[u8] = include_bytes!("../tests/fixtures/FiraMono-Regular.ttf");
        let mut tmp = tempfile::NamedTempFile::new().expect("tempfile");
        tmp.write_all(FONT_BYTES).expect("write");
        let charset = test_charset();
        let font_tiny = font_file_to_figfont(tmp.path(), -999.0, charset)
            .expect("negative point_size should be clamped and succeed");
        let font_huge = font_file_to_figfont(tmp.path(), 99999.0, charset)
            .expect("huge point_size should be clamped and succeed");
        assert!(
            font_tiny.charheight <= 20,
            "clamped tiny (min=4.0) should produce small charheight, got {}",
            font_tiny.charheight
        );
        assert!(
            font_huge.charheight <= 500,
            "clamped huge (max=200.0) should produce bounded charheight, got {}",
            font_huge.charheight
        );
        assert!(
            font_huge.charheight > font_tiny.charheight,
            "max point_size should produce larger glyphs than min"
        );
    }
}
