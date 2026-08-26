// FIGfont/TLF font parser
//
// Core data types for parsed font data.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::path::Path;

/// A single character glyph in a FIGfont.
///
/// Stores each row of the character as a `String`. The number of rows
/// equals the font's `charheight`. Width is derived from the first row.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct FIGcharacter {
    rows: Vec<String>,
}

impl FIGcharacter {
    /// Width of the character in display columns (first row char count), or 0 if empty.
    pub fn width(&self) -> usize {
        self.rows.first().map_or(0, |r| r.chars().count())
    }

    /// Access the character's rows.
    pub fn rows(&self) -> &[String] {
        &self.rows
    }

    /// Replace the character's rows.
    pub fn set_rows(&mut self, rows: Vec<String>) {
        self.rows = rows;
    }
}

impl From<Vec<String>> for FIGcharacter {
    fn from(rows: Vec<String>) -> Self {
        FIGcharacter { rows }
    }
}

/// Font format variant for the font file.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontFormat {
    /// Standard FIGfont (.flf) format
    #[default]
    Figfont,
    /// TOIlet (.tlf) format — UTF-8 encoded rows
    Tlf,
}

/// A character node mapping a character code to its glyph.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FCharnode {
    pub ord: u32,
    pub character: FIGcharacter,
}

/// A parsed FIGfont or TOIlet font.
///
/// Owns all character glyphs and font metadata extracted from the
/// font file header.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FIGfont {
    /// Font format (FIGfont or TLF)
    pub format: FontFormat,
    /// The hardblank character (e.g., `$`)
    pub hardblank: char,
    /// Font height in lines
    pub charheight: u32,
    /// Baseline offset
    pub baseline: u32,
    /// Maximum character width
    pub maxlength: u32,
    /// Legacy layout mode (smush)
    pub old_layout: i32,
    /// Full layout flags (smush2)
    pub full_layout: i32,
    /// Print direction: -1 = unset, 0 = left-to-right, 1 = right-to-left
    pub print_direction: i32,
    /// Number of comment lines after header
    pub comment_lines: u32,
    /// Character storage: code → glyph
    pub chars: HashMap<u32, FIGcharacter>,
    /// Number of expected codetagged characters
    pub codetag_count: u32,
}

impl Default for FIGfont {
    fn default() -> Self {
        FIGfont {
            format: FontFormat::Figfont,
            hardblank: '$',
            charheight: 1,
            baseline: 0,
            maxlength: 1,
            old_layout: 0,
            full_layout: 0,
            print_direction: -1,
            comment_lines: 0,
            chars: HashMap::new(),
            codetag_count: 0,
        }
    }
}

/// Errors that can occur during font parsing.
#[derive(Debug)]
pub enum FontError {
    /// The magic number doesn't match a known font format.
    InvalidMagic,
    /// A general parsing error occurred.
    ParseError(String),
    /// An I/O error occurred (file not found, permission denied, etc.).
    IoError(std::io::Error),
    /// A ZIP archive processing error occurred.
    ZipError(String),
    /// The input exceeded a size limit (GPT review F-21).
    TooLarge(String),
}

impl PartialEq for FontError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::InvalidMagic, Self::InvalidMagic) => true,
            (Self::ParseError(a), Self::ParseError(b)) => a == b,
            (Self::ZipError(a), Self::ZipError(b)) => a == b,
            _ => false,
        }
    }
}

impl fmt::Display for FontError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FontError::InvalidMagic => write!(f, "invalid font magic number"),
            FontError::ParseError(msg) => write!(f, "font parse error: {}", msg),
            FontError::IoError(e) => write!(f, "I/O error: {}", e),
            FontError::ZipError(msg) => write!(f, "ZIP error: {}", msg),
            FontError::TooLarge(msg) => write!(f, "input too large: {}", msg),
        }
    }
}

impl std::error::Error for FontError {}

impl From<std::io::Error> for FontError {
    fn from(e: std::io::Error) -> Self {
        FontError::IoError(e)
    }
}

/// The 7 Deutsch characters supported by FIGlet: Ä, Ö, Ü, ä, ö, ü, ß.
pub const DEUTSCH_CHARS: [u32; 7] = [196, 214, 220, 228, 246, 252, 223];

/// Strip trailing endmark characters from a font file line.
///
/// Follows `figlet.c:1155-1165` algorithm:
/// 1. Strip trailing whitespace
/// 2. Last remaining char is the endmark
/// 3. Remove all consecutive endmark chars from the right
///
/// Trailing whitespace before endmarks is preserved (width correctness).
/// Returns empty string if the line is all endmarks/whitespace.
fn strip_endmarks(line: &str) -> String {
    let trimmed = line.trim_end_matches(|c: char| c.is_ascii_whitespace());
    let endmark = match trimmed.chars().last() {
        Some(c) => c,
        None => return String::new(),
    };
    let mut end = trimmed.len();
    for c in trimmed.chars().rev() {
        if c == endmark {
            end -= c.len_utf8();
        } else {
            break;
        }
    }
    trimmed[..end].to_string()
}

/// Parse the 95 required ASCII FIGcharacters (codes 32–126) and 7 Deutsch chars.
///
/// Returns the unconsumed slice of lines (for subsequent codetag parsing).
/// Each character reads `font.charheight` rows, stripping endmarks via
/// `strip_endmarks()`. Stores parsed glyphs in `font.chars` keyed by char code.
pub fn parse_char_data<'a>(
    font: &mut FIGfont,
    lines: &'a [String],
) -> Result<&'a [String], FontError> {
    let height = font.charheight as usize;
    let mut cursor = 0;

    // ASCII chars 32–126 (95 characters)
    for code in 32..=126 {
        if cursor + height > lines.len() {
            let parsed = cursor / height;
            return Err(FontError::ParseError(format!(
                "unexpected end of font data: parsed {} ASCII chars, need {} more rows for code {}",
                parsed, height, code
            )));
        }
        let rows: Vec<String> = lines[cursor..cursor + height]
            .iter()
            .map(|l| strip_endmarks(l))
            .collect();
        font.chars.insert(code, FIGcharacter::from(rows));
        cursor += height;
    }

    // Deutsch chars
    for &code in &DEUTSCH_CHARS {
        if cursor + height > lines.len() {
            let parsed = cursor / height - 95; // subtract ASCII chars
            return Err(FontError::ParseError(format!(
                "unexpected end of font data: parsed {} Deutsch chars, need {} more rows for code {}",
                parsed, height, code
            )));
        }
        let rows: Vec<String> = lines[cursor..cursor + height]
            .iter()
            .map(|l| strip_endmarks(l))
            .collect();
        font.chars.insert(code, FIGcharacter::from(rows));
        cursor += height;
    }

    Ok(&lines[cursor..])
}

/// Parse the header line of a FIGfont (.flf) file.
///
/// Expected format:
/// `flf2a<hardblank> <height> <baseline> <max_length> <old_layout> <comment_lines>`
/// `[<print_direction> [<full_layout> [<codetag_count>]]]`
///
/// Missing optional fields are defaulted following FIGlet 2.2.5 conventions.
pub fn parse_header(line: &str) -> Result<FIGfont, FontError> {
    let format = if line.starts_with("flf2a") {
        FontFormat::Figfont
    } else if line.starts_with("tlf2a") {
        FontFormat::Tlf
    } else {
        return Err(FontError::InvalidMagic);
    };

    let rest = &line[5..];
    if rest.is_empty() {
        return Err(FontError::InvalidMagic);
    }

    let hardblank = rest.chars().next().ok_or(FontError::InvalidMagic)?;
    let rest = rest[hardblank.len_utf8()..].trim_start();

    let tokens: Vec<&str> = rest.split_whitespace().collect();

    if tokens.len() < 5 {
        return Err(FontError::ParseError(format!(
            "expected at least 5 numeric fields after hardblank, got {}",
            tokens.len()
        )));
    }

    let height = tokens[0].parse::<i32>().map_err(|e| {
        FontError::ParseError(format!("invalid height value '{}': {}", tokens[0], e))
    })?;
    let baseline = tokens[1].parse::<i32>().map_err(|e| {
        FontError::ParseError(format!("invalid baseline value '{}': {}", tokens[1], e))
    })?;
    let max_length = tokens[2].parse::<i32>().map_err(|e| {
        FontError::ParseError(format!("invalid max_length value '{}': {}", tokens[2], e))
    })?;
    let old_layout = tokens[3].parse::<i32>().map_err(|e| {
        FontError::ParseError(format!("invalid old_layout value '{}': {}", tokens[3], e))
    })?;
    let comment_lines = tokens[4].parse::<i32>().map_err(|e| {
        FontError::ParseError(format!(
            "invalid comment_lines value '{}': {}",
            tokens[4], e
        ))
    })?;

    let print_direction = if tokens.len() > 5 {
        tokens[5].parse::<i32>().map_err(|e| {
            FontError::ParseError(format!(
                "invalid print_direction value '{}': {}",
                tokens[5], e
            ))
        })?
    } else {
        -1
    };

    let full_layout = if tokens.len() > 6 {
        tokens[6].parse::<i32>().map_err(|e| {
            FontError::ParseError(format!("invalid full_layout value '{}': {}", tokens[6], e))
        })?
    } else if old_layout == 0 {
        64
    } else if old_layout < 0 {
        0
    } else {
        (old_layout & 31) | 128
    };

    let codetag_count = if tokens.len() > 7 {
        tokens[7].parse::<i32>().map_err(|e| {
            FontError::ParseError(format!(
                "invalid codetag_count value '{}': {}",
                tokens[7], e
            ))
        })?
    } else {
        0
    };

    if !(1..=255).contains(&height) {
        return Err(FontError::ParseError(format!(
            "font height {} is out of range 1..=255",
            height
        )));
    }
    if baseline < 0 {
        return Err(FontError::ParseError(format!(
            "font baseline {} is negative",
            baseline
        )));
    }
    if max_length < 0 {
        return Err(FontError::ParseError(format!(
            "font max_length {} is negative",
            max_length
        )));
    }
    let max_length = max_length.min(4096);

    Ok(FIGfont {
        format,
        hardblank,
        charheight: height as u32,
        baseline: baseline as u32,
        maxlength: max_length as u32,
        old_layout,
        full_layout,
        print_direction,
        comment_lines: comment_lines as u32,
        chars: HashMap::new(),
        codetag_count: codetag_count as u32,
    })
}

/// Parse a codetag integer from a line, mirroring C's `sscanf(fileline,"%li",&theord)`.
///
/// Handles `0x`/`0X` prefix for hex. Returns `None` if the line doesn't contain
/// a valid integer (signals end of codetagged section — not an error).
fn parse_codetag_integer(line: &str) -> Option<i64> {
    let token = line.split_whitespace().next()?;
    if token.is_empty() {
        return None;
    }
    if (token.starts_with("0x") || token.starts_with("0X")) && token.len() > 2 {
        i64::from_str_radix(&token[2..], 16).ok()
    } else {
        token.parse::<i64>().ok()
    }
}

/// Parse code-tagged FIGcharacters from remaining lines after required chars.
///
/// Reads variable-length code-tagged chars. Each line starts with a numeric
/// code tag, followed by `font.charheight` rows of character data. Handles
/// negative codes via two's complement storage. Skips code `-1` (reserved).
/// Stops at first non-numeric line (end of codetagged section).
pub fn parse_codetagged(font: &mut FIGfont, lines: &[String]) -> Result<(), FontError> {
    let height = font.charheight as usize;
    let mut cursor = 0;

    while cursor < lines.len() {
        let Some(code) = parse_codetag_integer(&lines[cursor]) else {
            break;
        };

        if cursor + 1 + height > lines.len() {
            return Err(FontError::ParseError(format!(
                "truncated codetagged char: code {} at line {}, need {} rows, got {}",
                code,
                cursor,
                height,
                lines.len() - cursor - 1
            )));
        }

        cursor += 1;

        if code == -1 {
            cursor += height;
            continue;
        }

        let rows: Vec<String> = lines[cursor..cursor + height]
            .iter()
            .map(|l| strip_endmarks(l))
            .collect();
        font.chars.insert(code as u32, FIGcharacter::from(rows));
        cursor += height;
    }

    Ok(())
}

/// Parse a TLF font from its full file content.
///
/// Splits content into lines, parses header, skips comment lines,
/// then parses required ASCII + Deutsch chars and any codetagged chars.
pub fn parse_tlf_font(content: &str) -> Result<FIGfont, FontError> {
    let lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();

    if lines.is_empty() {
        return Err(FontError::ParseError("empty font file".to_string()));
    }

    let mut font = parse_header(&lines[0])?;

    let comment_count = font.comment_lines as usize;
    let data_start = 1 + comment_count;

    if data_start >= lines.len() {
        return Err(FontError::ParseError(
            "no character data in font file".to_string(),
        ));
    }

    let remaining = parse_char_data(&mut font, &lines[data_start..])?;
    parse_codetagged(&mut font, remaining)?;

    Ok(font)
}

/// Check if a font name contains a path separator.
fn has_path_separator(name: &str) -> bool {
    name.contains('/') || name.contains('\\')
}

/// Generate candidate font paths in the order FIGopen() tries them.
fn font_candidates(name: &str, fontdirs: &[&str]) -> Vec<String> {
    use std::path::Path;
    let mut candidates = Vec::new();
    if has_path_separator(name) {
        candidates.push(name.to_string());
    } else {
        for dir in fontdirs {
            let d = Path::new(dir);
            candidates.push(
                d.join(format!("{}.flf", name))
                    .to_string_lossy()
                    .into_owned(),
            );
        }
    }
    candidates.push(format!("{}.flf", name));
    if has_path_separator(name) {
        candidates.push(format!("{}.tlf", name));
    } else {
        for dir in fontdirs {
            let d = Path::new(dir);
            candidates.push(
                d.join(format!("{}.tlf", name))
                    .to_string_lossy()
                    .into_owned(),
            );
        }
    }
    candidates
}

/// Read raw bytes from a font file path, bounded (GPT review F-21).
/// ZIP archives ride the same cap, bounding archive bytes too.
fn read_font_bytes(path: &str) -> Result<Vec<u8>, FontError> {
    use std::io::ErrorKind;
    crate::bounded_io::read_bounded(Path::new(path), crate::bounded_io::MAX_FONT_BYTES).map_err(
        |e| match e.kind() {
            ErrorKind::NotFound => {
                FontError::IoError(std::io::Error::new(ErrorKind::NotFound, e.to_string()))
            }
            _ => FontError::TooLarge(e.to_string()),
        },
    )
}

/// Check if byte slice starts with a ZIP local file header magic.
fn is_zip_bytes(bytes: &[u8]) -> bool {
    bytes.len() >= 4 && bytes[0] == 0x50 && bytes[1] == 0x4B && bytes[2] == 0x03 && bytes[3] == 0x04
}

/// Extract the first entry from a ZIP archive in memory.
fn extract_first_zip_entry(bytes: &[u8]) -> Result<Vec<u8>, FontError> {
    use std::io::Read;
    use zip::ZipArchive;

    let cursor = std::io::Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor)
        .map_err(|e| FontError::ZipError(format!("failed to open ZIP archive: {}", e)))?;

    if archive.is_empty() {
        return Err(FontError::ZipError("ZIP archive is empty".to_string()));
    }

    let entry = archive
        .by_index(0)
        .map_err(|e| FontError::ZipError(format!("failed to read first ZIP entry: {}", e)))?;

    const MAX_FONT_BYTES: u64 = 10 * 1024 * 1024; // 10 MiB cap against zip bombs
    if entry.size() > MAX_FONT_BYTES {
        return Err(FontError::ZipError(format!(
            "ZIP entry uncompressed size {} exceeds {} byte cap",
            entry.size(),
            MAX_FONT_BYTES
        )));
    }
    let mut content = Vec::new();
    entry
        .take(MAX_FONT_BYTES)
        .read_to_end(&mut content)
        .map_err(|e| FontError::ZipError(format!("failed to read ZIP entry contents: {}", e)))?;

    Ok(content)
}

/// Parse font content as either FLF or TLF (delegates to parse_tlf_font).
fn parse_font_bytes(content: &str) -> Result<FIGfont, FontError> {
    parse_tlf_font(content)
}

/// List font entries (.flf/.tlf) inside a ZIP archive.
///
/// Returns sorted list of entry names. Rejects entries with path separators
/// (path traversal defense).
pub fn list_zip_font_entries(path: &Path) -> Result<Vec<String>, FontError> {
    use zip::ZipArchive;

    const MAX_ZIP_ENTRIES: usize = 10_000;
    const MAX_ZIP_NAME_BYTES: usize = 4 * 1024 * 1024;

    let file = std::fs::File::open(path)?;
    let mut archive = ZipArchive::new(file)
        .map_err(|e| FontError::ZipError(format!("failed to open ZIP archive: {}", e)))?;
    if archive.len() > MAX_ZIP_ENTRIES {
        return Err(FontError::TooLarge(format!(
            "ZIP archive has {} entries, exceeding the {}-entry limit",
            archive.len(),
            MAX_ZIP_ENTRIES
        )));
    }

    let mut entries = Vec::new();
    let mut total_name_bytes = 0usize;
    for i in 0..archive.len() {
        let entry = archive
            .by_index(i)
            .map_err(|e| FontError::ZipError(format!("failed to read ZIP entry {}: {}", i, e)))?;
        let name = entry.name().to_string();
        total_name_bytes += name.len();
        if total_name_bytes > MAX_ZIP_NAME_BYTES {
            return Err(FontError::TooLarge(
                "ZIP central-directory names exceed the aggregate size limit".to_string(),
            ));
        }
        if name.contains('/') || name.contains('\\') {
            continue;
        }
        let lower = name.to_lowercase();
        if lower.ends_with(".flf") || lower.ends_with(".tlf") {
            entries.push(name);
        }
    }

    entries.sort();
    Ok(entries)
}

/// Read a specific entry from a ZIP archive by name.
///
/// Rejects entry names with path separators (path traversal defense).
pub fn read_zip_entry(path: &Path, entry_name: &str) -> Result<Vec<u8>, FontError> {
    use std::io::Read;
    use zip::ZipArchive;

    if entry_name.contains('/') || entry_name.contains('\\') {
        return Err(FontError::ZipError(
            "entry name contains path separator".to_string(),
        ));
    }

    let file = std::fs::File::open(path)?;
    let mut archive = ZipArchive::new(file)
        .map_err(|e| FontError::ZipError(format!("failed to open ZIP archive: {}", e)))?;

    let entry = archive
        .by_name(entry_name)
        .map_err(|e| FontError::ZipError(format!("entry '{}' not found: {}", entry_name, e)))?;

    const MAX_FONT_BYTES: u64 = 10 * 1024 * 1024; // 10 MiB cap against zip bombs
    if entry.size() > MAX_FONT_BYTES {
        return Err(FontError::ZipError(format!(
            "ZIP entry '{}' uncompressed size {} exceeds {} byte cap",
            entry_name,
            entry.size(),
            MAX_FONT_BYTES
        )));
    }
    let mut content = Vec::new();
    entry
        .take(MAX_FONT_BYTES)
        .read_to_end(&mut content)
        .map_err(|e| {
            FontError::ZipError(format!("failed to read entry '{}': {}", entry_name, e))
        })?;

    Ok(content)
}

/// Default font directories searched when no explicit font directory is given.
pub const DEFAULT_FONT_DIRS: &[&str] = &[
    "/usr/local/share/figlet", // macOS (Homebrew, user-installed)
    "/usr/share/figlet",       // Linux (system-wide)
];

/// Load a font by name from one or more font directories.
///
/// For each directory in `fontdirs`, tries `dir/name.flf`, then `dir/name.tlf`.
/// Also tries bare `name.flf` and `name.tlf` (relative/CWD) as a fallback.
///
/// If a candidate file is a ZIP archive, the first entry is extracted and parsed.
pub fn load_font(name: &str, fontdirs: &[&str]) -> Result<FIGfont, FontError> {
    let candidates = font_candidates(name, fontdirs);
    for path in &candidates {
        match read_font_bytes(path) {
            Ok(bytes) => {
                let content = if is_zip_bytes(&bytes) {
                    let extracted = extract_first_zip_entry(&bytes)?;
                    String::from_utf8_lossy(&extracted).into_owned()
                } else {
                    String::from_utf8_lossy(&bytes).into_owned()
                };
                return parse_font_bytes(&content);
            }
            Err(FontError::IoError(_)) => continue,
            Err(e) => return Err(e),
        }
    }
    Err(FontError::ParseError(format!(
        "could not find font '{}'",
        name
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_figcharacter_default() {
        let c = FIGcharacter::default();
        assert!(c.rows().is_empty());
        assert_eq!(c.width(), 0);
    }

    #[test]
    fn test_figcharacter_with_rows() {
        let rows = vec!["  __  ".to_string(), " / _ \\ ".to_string()];
        let c = FIGcharacter::from(rows);
        assert_eq!(c.width(), 6);
        assert_eq!(c.rows().len(), 2);
    }

    #[test]
    fn test_ficharnode_new() {
        let c = FIGcharacter::from(vec![" A ".to_string()]);
        let node = FCharnode {
            ord: 65,
            character: c,
        };
        assert_eq!(node.ord, 65);
        assert_eq!(node.character.width(), 3);
    }

    #[test]
    fn test_figfont_default() {
        let font = FIGfont::default();
        assert_eq!(font.format, FontFormat::Figfont);
        assert_eq!(font.hardblank, '$');
        assert_eq!(font.charheight, 1);
        assert_eq!(font.maxlength, 1);
        assert_eq!(font.old_layout, 0);
        assert_eq!(font.full_layout, 0);
        assert_eq!(font.print_direction, -1);
        assert_eq!(font.comment_lines, 0);
        assert!(font.chars.is_empty());
        assert_eq!(font.codetag_count, 0);
    }

    #[test]
    fn test_figfont_with_char() {
        let mut font = FIGfont::default();
        let c = FIGcharacter::from(vec![" X ".to_string()]);
        font.chars.insert(88, c);
        let retrieved = font.chars.get(&88);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().width(), 3);
    }

    #[test]
    fn test_figcharacter_serde_roundtrip() {
        let original = FIGcharacter::from(vec!["  __  ".to_string(), " / _ \\ ".to_string()]);
        let json = serde_json::to_string(&original).expect("serialize");
        let deserialized: FIGcharacter = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(original, deserialized);
    }

    #[test]
    fn test_ficharnode_serde_roundtrip() {
        let original = FCharnode {
            ord: 65,
            character: FIGcharacter::from(vec![" A ".to_string()]),
        };
        let json = serde_json::to_string(&original).expect("serialize");
        let deserialized: FCharnode = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(original, deserialized);
    }

    #[test]
    fn test_figfont_serde_roundtrip() {
        let original = FIGfont {
            charheight: 6,
            maxlength: 10,
            old_layout: 3,
            full_layout: 7,
            comment_lines: 5,
            chars: HashMap::from([(65, FIGcharacter::from(vec![" A ".to_string()]))]),
            ..FIGfont::default()
        };
        let json = serde_json::to_string(&original).expect("serialize");
        let deserialized: FIGfont = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(original, deserialized);
    }

    #[test]
    fn test_parse_header_full() {
        let result = parse_header("flf2a$ 6 5 20 15 3 0 143 229");
        assert!(result.is_ok());
        let font = result.unwrap();
        assert_eq!(font.hardblank, '$');
        assert_eq!(font.charheight, 6);
        assert_eq!(font.baseline, 5);
        assert_eq!(font.maxlength, 20);
        assert_eq!(font.old_layout, 15);
        assert_eq!(font.comment_lines, 3);
        assert_eq!(font.print_direction, 0);
        assert_eq!(font.full_layout, 143);
        assert_eq!(font.codetag_count, 229);
        assert!(font.chars.is_empty());
    }

    #[test]
    fn test_parse_header_minimal() {
        let result = parse_header("flf2a$ 6 5 20 15 3");
        assert!(result.is_ok());
        let font = result.unwrap();
        assert_eq!(font.hardblank, '$');
        assert_eq!(font.charheight, 6);
        assert_eq!(font.baseline, 5);
        assert_eq!(font.maxlength, 20);
        assert_eq!(font.old_layout, 15);
        assert_eq!(font.comment_lines, 3);
        assert_eq!(font.print_direction, -1);
        assert_eq!(font.full_layout, (15 & 31) | 128);
        assert_eq!(font.codetag_count, 0);
    }

    #[test]
    fn test_parse_header_old_layout_zero() {
        let result = parse_header("flf2a$ 6 5 20 0 3");
        assert!(result.is_ok());
        let font = result.unwrap();
        assert_eq!(font.old_layout, 0);
        assert_eq!(font.full_layout, 64);
    }

    #[test]
    fn test_parse_header_old_layout_negative() {
        let result = parse_header("flf2a$ 6 5 20 -1 3");
        assert!(result.is_ok());
        let font = result.unwrap();
        assert_eq!(font.old_layout, -1);
        assert_eq!(font.full_layout, 0);
    }

    #[test]
    fn test_parse_header_with_print_direction() {
        let result = parse_header("flf2a$ 8 3 15 5 2 1");
        assert!(result.is_ok());
        let font = result.unwrap();
        assert_eq!(font.charheight, 8);
        assert_eq!(font.baseline, 3);
        assert_eq!(font.maxlength, 15);
        assert_eq!(font.old_layout, 5);
        assert_eq!(font.comment_lines, 2);
        assert_eq!(font.print_direction, 1);
        assert_eq!(font.full_layout, (5 & 31) | 128);
    }

    #[test]
    fn test_parse_header_invalid_magic() {
        let result = parse_header("flf2b$ 6 5 20 15 3");
        assert_eq!(result, Err(FontError::InvalidMagic));
    }

    #[test]
    fn test_parse_header_wrong_prefix() {
        let result = parse_header("xyzzy$ 6 5 20 15 3");
        assert_eq!(result, Err(FontError::InvalidMagic));
    }

    #[test]
    fn test_parse_header_empty() {
        let result = parse_header("");
        assert_eq!(result, Err(FontError::InvalidMagic));
    }

    #[test]
    fn test_parse_header_truncated_magic() {
        let result = parse_header("flf");
        assert_eq!(result, Err(FontError::InvalidMagic));
    }

    #[test]
    fn test_parse_header_not_enough_fields() {
        let result = parse_header("flf2a$ 6 5 20");
        assert!(result.is_err());
        assert!(matches!(result, Err(FontError::ParseError(_))));
    }

    #[test]
    fn test_parse_header_non_numeric_field() {
        let result = parse_header("flf2a$ 6 x 20 15 3");
        assert!(result.is_err());
        assert!(matches!(result, Err(FontError::ParseError(_))));
    }

    // --- strip_endmarks tests ---

    #[test]
    fn test_strip_endmarks_typical() {
        assert_eq!(strip_endmarks("  __  @"), "  __  ");
        assert_eq!(strip_endmarks("$@@"), "$");
        assert_eq!(strip_endmarks(" @"), " ");
    }

    #[test]
    fn test_strip_endmarks_trailing_newline() {
        // With trailing newline: "  __  @\n"
        // trim_end_matches(whitespace) removes \n → "  __  @"
        // Then same as above
        assert_eq!(strip_endmarks("  __  @\n"), "  __  ");
        assert_eq!(strip_endmarks("$@@\n"), "$");
    }

    #[test]
    fn test_strip_endmarks_no_endmark() {
        // Per C algorithm: last non-whitespace char IS the endmark
        // So 'o' is endmark, stripped to "hell"
        assert_eq!(strip_endmarks("hello"), "hell");
    }

    #[test]
    fn test_strip_endmarks_empty() {
        assert_eq!(strip_endmarks(""), "");
        // "\n" → trim whitespace → "" → empty
        assert_eq!(strip_endmarks("\n"), "");
        // "\r\n" → trim whitespace → "" → empty
        assert_eq!(strip_endmarks("\r\n"), "");
    }

    #[test]
    fn test_strip_endmarks_whitespace_only() {
        assert_eq!(strip_endmarks("   \n"), "");
        assert_eq!(strip_endmarks("\t\n"), "");
    }

    #[test]
    fn test_strip_endmarks_multi_char_endmark() {
        // Endmark is '@', lines with multiple endmarks
        assert_eq!(strip_endmarks("AAA@@@@"), "AAA");
        assert_eq!(strip_endmarks("  X  @@@"), "  X  ");
    }

    #[test]
    fn test_strip_endmarks_trailing_spaces_preserved() {
        // Trailing spaces before endmarks are preserved per C spec
        assert_eq!(strip_endmarks("X  @"), "X  ");
        assert_eq!(strip_endmarks("  X  @"), "  X  ");
    }

    // --- parse_char_data tests ---

    fn build_102_char_fixture(height: u32) -> Vec<String> {
        let mut lines = Vec::new();
        for code in 32..=126u32 {
            let c = char::from_u32(code).expect("valid ASCII");
            for _ in 0..height {
                lines.push(format!("{}  @", c));
            }
        }
        for &code in &DEUTSCH_CHARS {
            let c = char::from_u32(code).expect("valid Deutsch char");
            for _ in 0..height {
                lines.push(format!("{}  @", c));
            }
        }
        lines
    }

    #[test]
    fn test_parse_char_data_102_chars() {
        let height = 2;
        let fixture = build_102_char_fixture(height);
        let mut font = FIGfont {
            charheight: height,
            ..FIGfont::default()
        };
        let remaining = parse_char_data(&mut font, &fixture).expect("parse should succeed");
        assert!(remaining.is_empty(), "all lines should be consumed");

        // Must have 95 + 7 = 102 chars
        assert_eq!(font.chars.len(), 102, "should have exactly 102 chars");

        // All ASCII keys 32..=126 present
        for code in 32..=126u32 {
            assert!(
                font.chars.contains_key(&code),
                "missing ASCII char code {code}"
            );
        }

        // All Deutsch keys present
        for &code in &DEUTSCH_CHARS {
            assert!(
                font.chars.contains_key(&code),
                "missing Deutsch char code {code}"
            );
        }
    }

    #[test]
    fn test_parse_char_data_endmarks_stripped() {
        let height = 1;
        let fixture = build_102_char_fixture(height);
        let mut font = FIGfont {
            charheight: height,
            ..FIGfont::default()
        };
        parse_char_data(&mut font, &fixture).expect("parse should succeed");

        for (code, ch) in &font.chars {
            for (i, row) in ch.rows().iter().enumerate() {
                assert!(
                    !row.ends_with('@'),
                    "char code {code} row {i} still ends with '@': '{row}'"
                );
            }
        }
    }

    #[test]
    fn test_parse_char_data_widths_consistent() {
        let height = 3;
        let fixture = build_102_char_fixture(height);
        let mut font = FIGfont {
            charheight: height,
            ..FIGfont::default()
        };
        parse_char_data(&mut font, &fixture).expect("parse should succeed");

        for (code, ch) in &font.chars {
            let rows = ch.rows();
            assert!(!rows.is_empty(), "char code {code} has no rows");
            let expected_width = rows[0].len();
            for (i, row) in rows.iter().enumerate() {
                assert_eq!(
                    row.len(),
                    expected_width,
                    "char code {code} row {i} width mismatch: expected {expected_width}, got {}",
                    row.len()
                );
            }
        }
    }

    #[test]
    fn test_parse_char_data_too_few_lines() {
        let height = 2;
        // Only provide 10 lines — not enough for even 1 char (needs height=2)
        let fixture: Vec<String> = (0..10).map(|i| format!("line{i}@")).collect();
        let mut font = FIGfont {
            charheight: height,
            ..FIGfont::default()
        };
        let result = parse_char_data(&mut font, &fixture);
        assert!(result.is_err(), "should fail with too few lines");
        match &result {
            Err(FontError::ParseError(msg)) => {
                assert!(
                    msg.contains("unexpected end of font data"),
                    "error should mention end of font data, got: {msg}"
                );
            }
            other => panic!("expected ParseError, got: {other:?}"),
        }
    }

    #[test]
    fn test_parse_char_data_returns_unconsumed() {
        // Provide extra lines beyond the 102 chars
        let height = 1;
        let mut fixture = build_102_char_fixture(height);
        fixture.push("codetag 0".to_string());
        fixture.push("row1@".to_string());

        let mut font = FIGfont {
            charheight: height,
            ..FIGfont::default()
        };
        let remaining = parse_char_data(&mut font, &fixture).expect("parse should succeed");

        assert_eq!(remaining.len(), 2, "should return 2 unconsumed lines");
        assert_eq!(remaining[0], "codetag 0");
        assert_eq!(remaining[1], "row1@");
    }

    // --- parse_codetag_integer tests ---

    #[test]
    fn test_parse_codetag_integer_decimal() {
        assert_eq!(parse_codetag_integer("200"), Some(200));
        assert_eq!(parse_codetag_integer("0"), Some(0));
        assert_eq!(parse_codetag_integer("-5"), Some(-5));
    }

    #[test]
    fn test_parse_codetag_integer_hex() {
        assert_eq!(parse_codetag_integer("0xCA0"), Some(3232));
        assert_eq!(parse_codetag_integer("0XCA0"), Some(3232));
        assert_eq!(parse_codetag_integer("0xff"), Some(255));
    }

    #[test]
    fn test_parse_codetag_integer_non_numeric() {
        assert_eq!(parse_codetag_integer("hello"), None);
        assert_eq!(parse_codetag_integer(""), None);
        assert_eq!(parse_codetag_integer("  "), None);
    }

    #[test]
    fn test_parse_codetag_integer_whitespace_prefix() {
        assert_eq!(parse_codetag_integer("  200  "), Some(200));
        assert_eq!(parse_codetag_integer("  -1  "), Some(-1));
    }

    // --- parse_codetagged tests ---

    fn build_codetag_fixture(_height: u32, entries: &[(i64, Vec<&str>)]) -> Vec<String> {
        let mut lines = Vec::new();
        for (code, rows) in entries {
            lines.push(code.to_string());
            for &row in rows {
                lines.push(format!("{}@", row));
            }
        }
        lines
    }

    #[test]
    fn test_parse_codetagged_basic() {
        let height = 2;
        let fixture = build_codetag_fixture(
            height,
            &[
                (200, vec!["row1_200", "row2_200"]),
                (300, vec!["row1_300", "row2_300"]),
            ],
        );
        let mut font = FIGfont {
            charheight: height,
            ..FIGfont::default()
        };
        parse_codetagged(&mut font, &fixture).expect("parse should succeed");
        assert_eq!(font.chars.len(), 2);

        let ch200 = font.chars.get(&200).expect("code 200 should exist");
        assert_eq!(ch200.rows().len(), height as usize);
        assert_eq!(ch200.rows()[0], "row1_200");
        assert_eq!(ch200.rows()[1], "row2_200");

        let ch300 = font.chars.get(&300).expect("code 300 should exist");
        assert_eq!(ch300.rows()[0], "row1_300");
    }

    #[test]
    fn test_parse_codetagged_skip_minus_one() {
        let height = 2;
        let fixture = build_codetag_fixture(
            height,
            &[
                (-1, vec!["skip1", "skip2"]),
                (200, vec!["actual1", "actual2"]),
            ],
        );
        let mut font = FIGfont {
            charheight: height,
            ..FIGfont::default()
        };
        parse_codetagged(&mut font, &fixture).expect("parse should succeed");
        assert_eq!(font.chars.len(), 1, "only code 200 should be inserted");
        assert!(
            !font.chars.contains_key(&(4294967295u32)),
            "code -1 should not be stored"
        );
        assert!(font.chars.contains_key(&200), "code 200 should exist");
    }

    #[test]
    fn test_parse_codetagged_hex_code() {
        let height = 1;
        let fixture: Vec<String> = vec!["0xCA0".to_string(), "hex_char@".to_string()];
        let mut font = FIGfont {
            charheight: height,
            ..FIGfont::default()
        };
        parse_codetagged(&mut font, &fixture).expect("parse should succeed");
        assert_eq!(font.chars.len(), 1);
        let ch = font
            .chars
            .get(&3232)
            .expect("code 3232 (0xCA0) should exist");
        assert_eq!(ch.rows()[0], "hex_char");
    }

    #[test]
    fn test_parse_codetagged_negative_code() {
        let height = 1;
        let fixture = build_codetag_fixture(height, &[(-5, vec!["negative_code"])]);
        let mut font = FIGfont {
            charheight: height,
            ..FIGfont::default()
        };
        parse_codetagged(&mut font, &fixture).expect("parse should succeed");

        // -5 as i32 cast to u32 = 4294967291
        let key = (-5i32) as u32;
        assert!(
            font.chars.contains_key(&key),
            "code -5 should exist as u32 key {key}"
        );
        let ch = font.chars.get(&key).expect("code -5 should exist");
        assert_eq!(ch.rows()[0], "negative_code");
    }

    #[test]
    fn test_parse_codetagged_truncated() {
        let height = 3;
        // Tag line + only 1 row instead of 3
        let fixture: Vec<String> = vec!["100".to_string(), "only_one_row@".to_string()];
        let mut font = FIGfont {
            charheight: height,
            ..FIGfont::default()
        };
        let result = parse_codetagged(&mut font, &fixture);
        assert!(result.is_err(), "should fail on truncated data");
        match &result {
            Err(FontError::ParseError(msg)) => {
                assert!(
                    msg.contains("truncated"),
                    "error should mention truncated, got: {msg}"
                );
            }
            other => panic!("expected ParseError, got: {other:?}"),
        }
    }

    #[test]
    fn test_parse_codetagged_no_codetags() {
        // Lines that don't start with a number → empty section
        let fixture: Vec<String> = vec![
            "some comment".to_string(),
            "another line".to_string(),
            "".to_string(),
        ];
        let mut font = FIGfont {
            charheight: 2,
            ..FIGfont::default()
        };
        parse_codetagged(&mut font, &fixture).expect("parse should succeed with no codetags");
        assert!(font.chars.is_empty(), "no chars should be inserted");
    }

    #[test]
    fn test_parse_codetagged_count_matches() {
        // Build fixture with known codetag_count
        let height = 1;
        let fixture = build_codetag_fixture(height, &[(100, vec!["a"]), (200, vec!["b"])]);
        let mut font = FIGfont {
            charheight: height,
            codetag_count: 2,
            ..FIGfont::default()
        };
        parse_codetagged(&mut font, &fixture).expect("parse should succeed");
        assert_eq!(
            font.chars.len() as u32,
            font.codetag_count,
            "parsed chars should match codetag_count"
        );
    }

    #[test]
    fn test_parse_codetagged_strips_endmarks() {
        let height = 2;
        let fixture = build_codetag_fixture(height, &[(42, vec!["content  @", "more  @"])]);
        let mut font = FIGfont {
            charheight: height,
            ..FIGfont::default()
        };
        parse_codetagged(&mut font, &fixture).expect("parse should succeed");
        let ch = font.chars.get(&42).expect("code 42 should exist");
        assert_eq!(
            ch.rows()[0],
            "content  ",
            "trailing spaces preserved, endmarks removed"
        );
        assert_eq!(ch.rows()[1], "more  ");
    }

    #[test]
    fn test_parse_codetagged_stops_at_non_numeric() {
        let height = 1;
        let mut fixture = build_codetag_fixture(height, &[(100, vec!["first"])]);
        fixture.push("trailing text".to_string());
        fixture.push("more trailing".to_string());
        let mut font = FIGfont {
            charheight: height,
            ..FIGfont::default()
        };
        parse_codetagged(&mut font, &fixture).expect("parse should succeed");
        assert_eq!(font.chars.len(), 1, "only one codetagged char parsed");
        assert!(font.chars.contains_key(&100));
    }

    #[test]
    fn test_parse_codetagged_integration_with_parse_char_data() {
        // Full flow: parse header → parse_char_data → parse_codetagged
        let height = 2;
        let mut all_lines: Vec<String> = Vec::new();
        // 102 required chars
        for code in 32..=126u32 {
            let c = char::from_u32(code).expect("valid ASCII");
            for _ in 0..height {
                all_lines.push(format!("{}  @", c));
            }
        }
        for &code in &DEUTSCH_CHARS {
            let c = char::from_u32(code).expect("valid Deutsch char");
            for _ in 0..height {
                all_lines.push(format!("{}  @", c));
            }
        }
        // codetagged section
        all_lines.push("0xCA0".to_string());
        all_lines.push("row1  @".to_string());
        all_lines.push("row2  @".to_string());
        all_lines.push("-1".to_string());
        all_lines.push("skip1@".to_string());
        all_lines.push("skip2@".to_string());
        all_lines.push("300".to_string());
        all_lines.push("extra1  @".to_string());
        all_lines.push("extra2  @".to_string());

        let mut font = FIGfont {
            charheight: height,
            codetag_count: 2,
            ..FIGfont::default()
        };

        let remaining =
            parse_char_data(&mut font, &all_lines).expect("parse_char_data should succeed");
        assert_eq!(
            remaining.len(),
            9,
            "9 lines should remain for codetagged section"
        );

        parse_codetagged(&mut font, remaining).expect("parse_codetagged should succeed");

        assert_eq!(
            font.chars.len(),
            104,
            "102 required + 2 codetagged = 104 total"
        );
        assert!(
            font.chars.contains_key(&3232),
            "hex code 0xCA0 should exist"
        );
        assert!(font.chars.contains_key(&300), "code 300 should exist");
        assert!(
            !font.chars.contains_key(&(4294967295u32)),
            "code -1 should be skipped"
        );

        let ch = font.chars.get(&3232).expect("code 3232");
        assert_eq!(ch.rows()[0], "row1  ");
    }

    // --- TLF font tests ---

    #[test]
    fn test_tlf_magic_detection() {
        let result = parse_header("tlf2a$ 6 5 20 -1 18");
        assert!(result.is_ok());
        let font = result.unwrap();
        assert_eq!(font.format, FontFormat::Tlf);
        assert_eq!(font.hardblank, '$');
        assert_eq!(font.charheight, 6);
        assert_eq!(font.old_layout, -1);
    }

    #[test]
    fn test_tlf_parse_header_fields() {
        // Header from tests/emboss.tlf: tlf2a<DEL> 3 3 8 -1 18 0 0 0
        let header = "tlf2a\u{7f} 3 3 8 -1 18 0 0 0";
        let result = parse_header(header);
        assert!(result.is_ok());
        let font = result.unwrap();
        assert_eq!(font.format, FontFormat::Tlf);
        assert_eq!(font.hardblank, '\u{7f}');
        assert_eq!(font.charheight, 3);
        assert_eq!(font.baseline, 3);
        assert_eq!(font.maxlength, 8);
        assert_eq!(font.old_layout, -1);
        assert_eq!(font.comment_lines, 18);
        assert_eq!(font.print_direction, 0);
        assert_eq!(font.full_layout, 0);
        assert_eq!(font.codetag_count, 0);
    }

    #[test]
    fn test_tlf_parse_full_font() {
        let content = include_str!("../../tests/emboss.tlf");
        let font = parse_tlf_font(content).expect("TLF font should parse");

        assert_eq!(font.format, FontFormat::Tlf);
        assert_eq!(font.charheight, 3);
        assert_eq!(
            font.chars.len(),
            102,
            "should have 102 chars (95 ASCII + 7 Deutsch)"
        );

        // All ASCII codes 32..=126 present
        for code in 32..=126u32 {
            assert!(
                font.chars.contains_key(&code),
                "missing ASCII char code {code}"
            );
        }

        // All Deutsch codes present
        for &code in &DEUTSCH_CHARS {
            assert!(
                font.chars.contains_key(&code),
                "missing Deutsch char code {code}"
            );
        }

        // Check specific glyph content: space (code 32)
        let space = font.chars.get(&32).expect("space");
        assert_eq!(space.rows().len(), 3);
        assert_eq!(space.rows()[0], " \u{7f}");
        assert_eq!(space.rows()[1], " \u{7f}");
        assert_eq!(space.rows()[2], " \u{7f}");

        // Check '!' (code 33)
        let excl = font.chars.get(&33).expect("exclamation");
        assert_eq!(excl.rows()[0], "\u{2503}");
        assert_eq!(excl.rows()[1], "\u{251b}");
        assert_eq!(excl.rows()[2], "\u{251b}");

        // Check '"' (code 34)
        let dquote = font.chars.get(&34).expect("double quote");
        assert_eq!(dquote.rows()[0], "\u{251b}\u{251b}");
        assert_eq!(dquote.rows()[1], "  ");
        assert_eq!(dquote.rows()[2], "  ",);
    }

    #[test]
    fn test_tlf_parse_char_data_endmarks_stripped() {
        let content = include_str!("../../tests/emboss.tlf");
        let font = parse_tlf_font(content).expect("TLF font should parse");

        for (code, ch) in &font.chars {
            for (i, row) in ch.rows().iter().enumerate() {
                // Endmarks are per-character in FIGfont/TLF: the last non-whitespace
                // char on each row. After 'strip_endmarks', no row should contain
                // its endmark at the end. Since endmark varies per char, just verify
                // no trailing '!', '"', '@', etc.
                if *code == 32 {
                    // Space uses '@' as endmark - rows end with DEL, which is fine
                    continue;
                }
                // All rows in emboss.tlf should have non-empty content after stripping
                assert!(
                    !row.is_empty(),
                    "char code {code} row {i} is empty after stripping endmarks"
                );
            }
        }
    }

    #[test]
    fn test_parse_header_dual_magic_rejection() {
        let result = parse_header("xyzzy$ 6 5 20 15 3");
        assert_eq!(result, Err(FontError::InvalidMagic));

        let result = parse_header("flf2b$ 6 5 20 15 3");
        assert_eq!(result, Err(FontError::InvalidMagic));

        let result = parse_header("tlf2b$ 6 5 20 15 3");
        assert_eq!(result, Err(FontError::InvalidMagic));
    }

    // --- load_font / ZIP support tests ---

    fn temp_dir_uniq() -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("figby_test_{}_{}_{n}", std::process::id(), ts))
    }

    fn write_standard_font(dir: &std::path::Path) {
        let bytes = include_bytes!("../assets/fonts/standard.flf");
        std::fs::write(dir.join("standard.flf"), bytes).unwrap();
    }

    #[test]
    fn test_is_zip_bytes() {
        // PK\x03\x04 is ZIP local file header
        assert!(is_zip_bytes(&[0x50, 0x4B, 0x03, 0x04, 0x00, 0x00]));
        assert!(is_zip_bytes(&[0x50, 0x4B, 0x03, 0x04]));
        assert!(!is_zip_bytes(&[]));
        assert!(!is_zip_bytes(&[0x00, 0x00, 0x00, 0x00]));
        assert!(!is_zip_bytes(&[0x50, 0x4B]));
        assert!(!is_zip_bytes(&[0x50, 0x4B, 0x03]));
    }

    #[test]
    fn test_has_path_separator() {
        assert!(!has_path_separator("standard"));
        assert!(has_path_separator("./standard"));
        assert!(has_path_separator("/abs/path/standard"));
        assert!(has_path_separator("subdir\\standard"));
        assert!(!has_path_separator(""));
        assert!(has_path_separator("a/b"));
    }

    #[test]
    fn test_load_font_plain_file() {
        let tmpdir = temp_dir_uniq();
        std::fs::create_dir_all(&tmpdir).unwrap();
        write_standard_font(&tmpdir);

        let dirs = [tmpdir.to_str().unwrap()];
        let font = load_font("standard", &dirs).expect("should load standard font from plain file");
        assert_eq!(font.charheight, 6);
        assert_eq!(font.chars.len(), 325);
        assert_eq!(font.hardblank, '$');

        std::fs::remove_file(tmpdir.join("standard.flf")).unwrap();
        std::fs::remove_dir(&tmpdir).unwrap();
    }

    #[test]
    fn test_load_font_from_zip() {
        use std::io::Write;

        let tmpdir = temp_dir_uniq();
        std::fs::create_dir_all(&tmpdir).unwrap();

        let font_bytes = include_bytes!("../assets/fonts/standard.flf");
        let zip_path = tmpdir.join("standard.flf");
        let file = std::fs::File::create(&zip_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        zip.start_file::<&str, ()>("standard.flf", Default::default())
            .unwrap();
        zip.write_all(font_bytes).unwrap();
        zip.finish().unwrap();

        let dirs = [tmpdir.to_str().unwrap()];
        let font =
            load_font("standard", &dirs).expect("should load standard font from ZIP archive");
        assert_eq!(font.charheight, 6);
        assert_eq!(font.chars.len(), 325);
        assert_eq!(font.hardblank, '$');

        std::fs::remove_file(&zip_path).unwrap();
        std::fs::remove_dir(&tmpdir).unwrap();
    }

    #[test]
    fn test_load_font_search_order() {
        // Create fontdir/standard.flf (stripped-down, height=1) and
        // plain standard.flf (full font, height=6).
        // load_font with fontdir pointing to parent of fontdir/
        // should pick up the fontdir version (height=1).
        let tmpdir = temp_dir_uniq();
        std::fs::create_dir_all(&tmpdir).unwrap();

        // Write the stripped-down font in fontdir/standard.flf
        let fontdir = tmpdir.join("fontdir");
        std::fs::create_dir_all(&fontdir).unwrap();
        let mut content = String::from("flf2a$ 1 0 1 0 0\n");
        for code in 32..=126u32 {
            let c = char::from_u32(code).unwrap();
            content.push(c);
            content.push_str("  @\n");
        }
        for &code in &DEUTSCH_CHARS {
            let c = char::from_u32(code).unwrap();
            content.push(c);
            content.push_str("  @\n");
        }
        std::fs::write(fontdir.join("standard.flf"), &content).unwrap();

        // Write the full font as bare path standard.flf
        let full_bytes = include_bytes!("../assets/fonts/standard.flf");
        std::fs::write(tmpdir.join("standard.flf"), full_bytes).unwrap();

        // Load with fontdir pointing to tmpdir/fontdir
        let dirs = [fontdir.to_str().unwrap()];
        let font = load_font("standard", &dirs).expect("should load font from fontdir");
        // Height=1 confirms fontdir version was picked
        assert_eq!(font.charheight, 1);
        assert_eq!(font.chars.len(), 102);

        std::fs::remove_file(fontdir.join("standard.flf")).unwrap();
        std::fs::remove_dir(&fontdir).unwrap();
        std::fs::remove_file(tmpdir.join("standard.flf")).unwrap();
        std::fs::remove_dir(&tmpdir).unwrap();
    }

    fn build_test_zip(path: &std::path::Path, entries: &[(&str, &[u8])]) {
        use std::io::Write;
        let file = std::fs::File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        for (name, data) in entries {
            zip.start_file::<&str, ()>(name, Default::default())
                .unwrap();
            zip.write_all(data).unwrap();
        }
        zip.finish().unwrap();
    }

    #[test]
    fn test_list_zip_font_entries() {
        let tmpdir = temp_dir_uniq();
        std::fs::create_dir_all(&tmpdir).unwrap();
        let zip_path = tmpdir.join("fonts.zip");
        build_test_zip(
            &zip_path,
            &[
                ("font1.flf", b"flf2a$ 1 0 1 0 0\nA  @\n"),
                ("font2.flf", b"flf2a$ 1 0 1 0 0\nB  @\n"),
                ("font3.tlf", b"tlf2a$ 1 0 1 0 0\nC  @\n"),
            ],
        );

        let entries = list_zip_font_entries(&zip_path).expect("should list entries");
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0], "font1.flf");
        assert_eq!(entries[1], "font2.flf");
        assert_eq!(entries[2], "font3.tlf");

        std::fs::remove_file(&zip_path).unwrap();
        std::fs::remove_dir(&tmpdir).unwrap();
    }

    #[test]
    fn test_list_zip_font_entries_skips_non_fonts() {
        let tmpdir = temp_dir_uniq();
        std::fs::create_dir_all(&tmpdir).unwrap();
        let zip_path = tmpdir.join("mixed.zip");
        build_test_zip(
            &zip_path,
            &[
                ("font.flf", b"data"),
                ("readme.txt", b"hello"),
                ("icon.tlf", b"data"),
                ("image.jpg", b"data"),
            ],
        );

        let entries = list_zip_font_entries(&zip_path).expect("should list entries");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0], "font.flf");
        assert_eq!(entries[1], "icon.tlf");

        std::fs::remove_file(&zip_path).unwrap();
        std::fs::remove_dir(&tmpdir).unwrap();
    }

    #[test]
    fn test_list_zip_font_entries_empty() {
        let tmpdir = temp_dir_uniq();
        std::fs::create_dir_all(&tmpdir).unwrap();
        let zip_path = tmpdir.join("empty.zip");
        build_test_zip(&zip_path, &[]);

        let entries = list_zip_font_entries(&zip_path).expect("should list entries");
        assert!(entries.is_empty());

        std::fs::remove_file(&zip_path).unwrap();
        std::fs::remove_dir(&tmpdir).unwrap();
    }

    #[test]
    fn test_list_zip_font_entries_rejects_path_separators() {
        let tmpdir = temp_dir_uniq();
        std::fs::create_dir_all(&tmpdir).unwrap();
        let zip_path = tmpdir.join("traversal.zip");
        let font_data = b"flf2a$ 1 0 1 0 0\nA  @\n";
        build_test_zip(
            &zip_path,
            &[
                ("../outside.flf", font_data),
                ("sub/escaped.flf", font_data),
                ("safe.flf", font_data),
            ],
        );

        let entries = list_zip_font_entries(&zip_path).expect("should list entries");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0], "safe.flf");

        std::fs::remove_file(&zip_path).unwrap();
        std::fs::remove_dir(&tmpdir).unwrap();
    }

    #[test]
    fn test_read_zip_entry() {
        let tmpdir = temp_dir_uniq();
        std::fs::create_dir_all(&tmpdir).unwrap();
        let zip_path = tmpdir.join("read_test.zip");
        build_test_zip(
            &zip_path,
            &[("alpha.flf", b"alpha data"), ("beta.flf", b"beta data")],
        );

        let alpha = read_zip_entry(&zip_path, "alpha.flf").expect("should read alpha");
        assert_eq!(alpha, b"alpha data");

        let beta = read_zip_entry(&zip_path, "beta.flf").expect("should read beta");
        assert_eq!(beta, b"beta data");

        std::fs::remove_file(&zip_path).unwrap();
        std::fs::remove_dir(&tmpdir).unwrap();
    }

    #[test]
    fn test_read_zip_entry_not_found() {
        let tmpdir = temp_dir_uniq();
        std::fs::create_dir_all(&tmpdir).unwrap();
        let zip_path = tmpdir.join("notfound.zip");
        build_test_zip(&zip_path, &[("existing.flf", b"data")]);

        let result = read_zip_entry(&zip_path, "nonexistent.flf");
        assert!(result.is_err());
        assert!(matches!(result, Err(FontError::ZipError(_))));

        std::fs::remove_file(&zip_path).unwrap();
        std::fs::remove_dir(&tmpdir).unwrap();
    }

    #[test]
    fn test_read_zip_entry_path_traversal() {
        let tmpdir = temp_dir_uniq();
        std::fs::create_dir_all(&tmpdir).unwrap();
        let zip_path = tmpdir.join("traversal_read.zip");
        build_test_zip(&zip_path, &[("safe.flf", b"data")]);

        let result = read_zip_entry(&zip_path, "../evil.flf");
        assert!(result.is_err());
        assert!(matches!(result, Err(FontError::ZipError(_))));
        assert!(
            result.unwrap_err().to_string().contains("path separator"),
            "error should mention path separator"
        );

        std::fs::remove_file(&zip_path).unwrap();
        std::fs::remove_dir(&tmpdir).unwrap();
    }

    #[test]
    fn test_load_font_nonexistent() {
        let tmpdir = temp_dir_uniq();
        std::fs::create_dir_all(&tmpdir).unwrap();

        let dirs = [tmpdir.to_str().unwrap()];
        let result = load_font("nonexistent_font_xyzzy", &dirs);
        assert!(result.is_err(), "should return error for nonexistent font");
        match &result {
            Err(FontError::ParseError(msg)) => {
                assert!(
                    msg.contains("could not find font"),
                    "error should mention could not find font, got: {msg}"
                );
            }
            other => panic!("expected ParseError, got: {other:?}"),
        }

        std::fs::remove_dir(&tmpdir).unwrap();
    }
}
