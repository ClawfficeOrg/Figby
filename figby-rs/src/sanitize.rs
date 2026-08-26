//! Display sanitization for untrusted strings.
//!
//! Palette names, filenames, ZIP entry names and typed paths are
//! rendered as terminal text. Control characters, C1 bytes, bidi marks
//! and zero-width characters can hide content, spoof labels or inject
//! escape sequences — [`sanitize_display`] replaces them all with a
//! visible replacement character and bounds the displayed length
//! (GPT review F-24). Keep the original value separately for any real
//! filesystem operation.

/// Maximum characters shown for an untrusted label.
const MAX_DISPLAY_CHARS: usize = 256;

/// Sanitize an untrusted string for safe terminal display.
pub fn sanitize_display(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut truncated = false;
    for c in s.chars() {
        if out.chars().count() >= MAX_DISPLAY_CHARS {
            truncated = true;
            break;
        }
        out.push(sanitize_char(c));
    }
    if truncated {
        out.push('…');
    }
    out
}

fn sanitize_char(c: char) -> char {
    match c {
        // ASCII control chars + DEL (covers ESC, OSC intros, CR/LF games)
        c if (c as u32) < 0x20 || c as u32 == 0x7F => '\u{FFFD}',
        // C1 control range
        c if (0x80..=0x9F).contains(&(c as u32)) => '\u{FFFD}',
        // Bidi isolates/overrides
        c if (0x202A..=0x202E).contains(&(c as u32)) => '\u{FFFD}',
        c if (0x2066..=0x2069).contains(&(c as u32)) => '\u{FFFD}',
        // Zero-width / invisible formatting
        c if (0x200B..=0x200F).contains(&(c as u32)) => '\u{FFFD}',
        c if (0x2060..=0x2064).contains(&(c as u32)) => '\u{FFFD}',
        '\u{FEFF}' | '\u{00AD}' => '\u{FFFD}',
        _ => c,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passes_plain_text_through() {
        assert_eq!(sanitize_display("font.flf"), "font.flf");
        assert_eq!(sanitize_display("Warm Palette"), "Warm Palette");
        assert_eq!(sanitize_display(""), "");
        // Unicode that is safe stays as-is.
        assert_eq!(sanitize_display("日本語フォント"), "日本語フォント");
    }

    #[test]
    fn escapes_control_and_c1() {
        assert_eq!(sanitize_display("a\u{1B}[31mred"), "a\u{FFFD}[31mred");
        assert_eq!(sanitize_display("x\u{7F}y"), "x\u{FFFD}y");
        assert_eq!(sanitize_display("n\u{85}ext"), "n\u{FFFD}ext"); // NEL (C1)
    }

    #[test]
    fn escapes_bidi_and_zero_width() {
        // RLO override used for spoofing.
        assert_eq!(sanitize_display("tab\u{202E}gpj.exe"), "tab\u{FFFD}gpj.exe");
        assert_eq!(sanitize_display("zero\u{200B}width"), "zero\u{FFFD}width");
        assert_eq!(sanitize_display("bom\u{FEFF}name"), "bom\u{FFFD}name");
    }

    #[test]
    fn bounds_length() {
        let long = "a".repeat(500);
        let out = sanitize_display(&long);
        assert_eq!(out.chars().count(), MAX_DISPLAY_CHARS + 1); // + ellipsis
        assert!(out.ends_with('…'));
    }
}
