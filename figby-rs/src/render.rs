// Character rendering, kerning, smushing

use crate::font::{FIGcharacter, FIGfont};
use crate::smush::{smush_horizontal, SmushMode};

static BLANK_GLYPH: std::sync::OnceLock<FIGcharacter> = std::sync::OnceLock::new();

pub fn lookup_char<'a>(
    font: &'a FIGfont,
    code: u32,
    current_width: &mut usize,
) -> &'a FIGcharacter {
    // Try requested char, then char 0 (font-defined fallback glyph), then static blank.
    // BLANK_GLYPH is only reached for fonts with no chars at all.
    let blank = BLANK_GLYPH.get_or_init(FIGcharacter::default);
    let ch = font
        .chars
        .get(&code)
        .or_else(|| font.chars.get(&0))
        .unwrap_or(blank);
    *current_width = ch.width();
    ch
}

fn last_non_space(s: &str, fallback_pos: usize, fallback_char: char) -> (usize, char) {
    let chars: Vec<char> = s.chars().collect();
    for (i, &c) in chars.iter().enumerate().rev() {
        if c != ' ' {
            return (i, c);
        }
    }
    (fallback_pos, fallback_char)
}

fn first_non_space(s: &str, fallback_pos: usize, fallback_char: char) -> (usize, char) {
    for (i, c) in s.chars().enumerate() {
        if c != ' ' {
            return (i, c);
        }
    }
    (fallback_pos, fallback_char)
}

/// Maximum overlap between current character and output line.
///
/// For each row, find last non-space in output and first non-space
/// in current character. Minimum across all rows determines the
/// smush amount. Handles left-to-right and right-to-left.
///
/// Mirror of C `smushamt()` in figlet.c:1446-1485.
#[allow(clippy::too_many_arguments)]
pub fn calc_smush_amount(
    output_rows: &[String],
    curr_rows: &[String],
    outlinelen: usize,
    currcharwidth: usize,
    prevcharwidth: usize,
    mode: SmushMode,
    hardblank: char,
    right2left: bool,
) -> usize {
    if !mode.contains(SmushMode::KERN) && !mode.contains(SmushMode::SMUSH) {
        return 0;
    }

    let mut maxsmush = currcharwidth;

    for (output_row, curr_row) in output_rows.iter().zip(curr_rows.iter()) {
        let (linebd, ch1, charbd, ch2) = if right2left {
            let out_len = output_row.chars().count();
            if maxsmush > out_len {
                maxsmush = out_len;
            }

            let (charbd, c1) = last_non_space(curr_row, 0, ' ');
            let (linebd, c2) = first_non_space(output_row, outlinelen, '\0');

            (linebd, c1, charbd, c2)
        } else {
            let (linebd, c1) = last_non_space(output_row, 0, ' ');
            let (charbd, c2) = first_non_space(curr_row, currcharwidth, '\0');

            (linebd, c1, charbd, c2)
        };

        let amt_base = if right2left {
            (linebd as isize + currcharwidth as isize).saturating_sub(1 + charbd as isize)
        } else {
            (charbd as isize + outlinelen as isize).saturating_sub(1 + linebd as isize)
        };

        let amt = if ch1 == ' '
            || ch1 == '\0'
            || (ch2 != '\0'
                && prevcharwidth >= 2
                && currcharwidth >= 2
                && smush_horizontal(ch1, ch2, mode, hardblank, right2left).is_some())
        {
            amt_base + 1
        } else {
            amt_base
        };

        let amt = amt.max(0) as usize;

        if amt < maxsmush {
            maxsmush = amt;
        }
    }

    maxsmush
}

/// Append a character to the output line, applying kerning/smushing.
///
/// Mirrors C `addchar()` in figlet.c:1497-1537.
/// `output_rows` must be pre-initialized with `font.charheight` empty strings
/// before the first call. Returns `true` if the character was added,
/// `false` if the output line length would exceed `outlinelen_limit`.
#[allow(clippy::too_many_arguments)]
pub fn add_char(
    font: &FIGfont,
    code: u32,
    output_rows: &mut Vec<String>,
    outlinelen: &mut usize,
    prev_width: &mut usize,
    mode: SmushMode,
    right2left: bool,
    outlinelen_limit: usize,
) -> bool {
    let old_prev_width = *prev_width;
    let ch = lookup_char(font, code, prev_width);
    let curr_width = *prev_width;
    let curr_rows = ch.rows();

    let smush = calc_smush_amount(
        output_rows,
        curr_rows,
        *outlinelen,
        curr_width,
        old_prev_width,
        mode,
        font.hardblank,
        right2left,
    );

    if *outlinelen + curr_width - smush > outlinelen_limit {
        *prev_width = old_prev_width;
        return false;
    }

    for (row_idx, curr_row) in curr_rows.iter().enumerate() {
        let out_chars: Vec<char> = if row_idx < output_rows.len() {
            output_rows[row_idx].chars().collect()
        } else {
            Vec::new()
        };
        let curr_chars: Vec<char> = curr_row.chars().collect();

        let result: String = if right2left {
            let mut temp = curr_chars.clone();
            let overlap = smush.min(curr_chars.len()).min(out_chars.len());
            for (k, rch) in out_chars.iter().enumerate().take(overlap) {
                let col = curr_width.saturating_sub(smush).saturating_add(k);
                if col < temp.len() {
                    let lch = temp[col];
                    if let Some(smushed) =
                        smush_horizontal(lch, *rch, mode, font.hardblank, right2left)
                    {
                        temp[col] = smushed;
                    }
                }
            }
            if smush < out_chars.len() {
                temp.extend(&out_chars[smush..]);
            }
            temp.into_iter().collect()
        } else {
            let mut out = out_chars;
            let overlap = smush.min(curr_chars.len());
            for (k, rch) in curr_chars.iter().enumerate().take(overlap) {
                let col = ((*outlinelen as isize)
                    .saturating_sub(smush as isize)
                    .saturating_add(k as isize))
                .max(0) as usize;
                if col < out.len() {
                    let lch = out[col];
                    if let Some(smushed) =
                        smush_horizontal(lch, *rch, mode, font.hardblank, right2left)
                    {
                        out[col] = smushed;
                    }
                }
            }
            if smush < curr_chars.len() {
                out.extend(&curr_chars[smush..]);
            }
            out.into_iter().collect()
        };

        if row_idx < output_rows.len() {
            output_rows[row_idx] = result;
        } else {
            output_rows.push(result);
        }
    }

    *outlinelen = output_rows[0].chars().count();
    true
}

/// Split `char_buffer` at the last word break (run of consecutive spaces).
///
/// Mirrors C `splitline()` in figlet.c:1623-1658.
/// Scans backward for the last run of spaces, splits the buffer, rebuilds
/// the first part into returned rows, and the second part into `output_rows`.
/// Returns `None` if no word break was found.
/// On success, returns `Some((part1_rows, part2_start))` where `part2_start`
/// is the index in `char_buffer` where the second part begins (caller uses
/// this to truncate its buffer). `output_rows`, `outlinelen`, and `prev_width`
/// are updated to reflect only part2.
#[allow(clippy::too_many_arguments)]
pub fn split_line(
    font: &FIGfont,
    char_buffer: &[u32],
    output_rows: &mut Vec<String>,
    outlinelen: &mut usize,
    prev_width: &mut usize,
    mode: SmushMode,
    right2left: bool,
    outlinelen_limit: usize,
) -> Option<(Vec<String>, usize)> {
    let buflen = char_buffer.len();
    if buflen == 0 {
        return None;
    }

    let mut gotspace = false;
    let mut lastspace = buflen - 1;
    let mut part1_end: isize = -1;

    for i in (0..buflen).rev() {
        if !gotspace && char_buffer[i] == b' ' as u32 {
            gotspace = true;
            lastspace = i;
        }
        if gotspace && char_buffer[i] != b' ' as u32 {
            part1_end = i as isize;
            break;
        }
    }

    if !gotspace {
        return None;
    }

    let part1_len = (part1_end + 1) as usize;
    let part2_start = lastspace + 1;

    let part1_codes = &char_buffer[..part1_len];
    let part2_codes = &char_buffer[part2_start..];

    let height = font.charheight as usize;
    let mut part1_rows = vec![String::new(); height];
    let mut p1_len = 0;
    let mut p1_prev = 0;
    for &code in part1_codes {
        add_char(
            font,
            code,
            &mut part1_rows,
            &mut p1_len,
            &mut p1_prev,
            mode,
            right2left,
            outlinelen_limit,
        );
    }

    *outlinelen = 0;
    *prev_width = 0;
    for row in output_rows.iter_mut() {
        row.clear();
    }
    for &code in part2_codes {
        add_char(
            font,
            code,
            output_rows,
            outlinelen,
            prev_width,
            mode,
            right2left,
            outlinelen_limit,
        );
    }

    Some((part1_rows, part2_start))
}

/// Output justification mode.
///
/// Matches C `justification` global: 0=left, 1=center, 2=right.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Justification {
    Left,
    Center,
    Right,
}

impl Justification {
    pub fn from_i32(v: i32) -> Self {
        match v {
            0 => Justification::Left,
            1 => Justification::Center,
            2 => Justification::Right,
            _ => Justification::Left,
        }
    }
}

/// Render output rows with hardblank replacement, justification, and width limit.
///
/// Port of C `putstring()` / `printline()` in figlet.c:1553-1610.
/// For each row:
/// 1. Replace hardblank characters with spaces
/// 2. If `outputwidth > 1`, truncate to `outputwidth - 1` characters
/// 3. If justification is Center or Right, prepend spaces per C formula
pub fn render_line(
    rows: &[String],
    hardblank: char,
    justification: Justification,
    outputwidth: usize,
) -> Vec<String> {
    rows.iter()
        .map(|row| {
            let mut s: String = row
                .chars()
                .map(|c| if c == hardblank { ' ' } else { c })
                .collect();

            if outputwidth > 1 {
                let max_len = outputwidth - 1;
                if s.chars().count() > max_len {
                    s = s.chars().take(max_len).collect();
                }

                let len = s.chars().count();
                let spaces = match justification {
                    Justification::Left => 0,
                    Justification::Center => {
                        let mut count = 0usize;
                        let mut i = 1usize;
                        while 2 * i + len - 1 < outputwidth {
                            count += 1;
                            i += 1;
                        }
                        count
                    }
                    Justification::Right => {
                        let mut count = 0usize;
                        let mut i = 1usize;
                        while i + len < outputwidth {
                            count += 1;
                            i += 1;
                        }
                        count
                    }
                };

                if spaces > 0 {
                    let pad = " ".repeat(spaces);
                    s = pad + &s;
                }
            }

            s
        })
        .collect()
}

/// Render a string through the full FIGlet pipeline.
///
/// Convenience wrapper: initializes output rows, calls `add_char` for each
/// codepoint in `text`, then calls `render_line` with left justification and
/// no width limit. Returns `charheight` rows of rendered text.
pub fn render_string(font: &FIGfont, text: &str) -> Vec<String> {
    let mode = SmushMode::new(font.full_layout as u32);
    let height = font.charheight as usize;
    let mut output_rows = vec![String::new(); height];
    let mut outlinelen = 0;
    let mut prev_width = 0;

    for ch in text.chars() {
        add_char(
            font,
            ch as u32,
            &mut output_rows,
            &mut outlinelen,
            &mut prev_width,
            mode,
            false,
            usize::MAX,
        );
    }

    render_line(
        &output_rows,
        font.hardblank,
        Justification::Left,
        usize::MAX,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn test_font() -> FIGfont {
        let mut chars = HashMap::new();
        chars.insert(0, FIGcharacter::from(vec!["#".to_string()]));
        chars.insert(65, FIGcharacter::from(vec![" A ".to_string()]));
        chars.insert(66, FIGcharacter::from(vec![" B  ".to_string()]));
        FIGfont {
            chars,
            ..FIGfont::default()
        }
    }

    #[test]
    fn test_lookup_char_known() {
        let font = test_font();
        let mut current_width = 0;
        let ch = lookup_char(&font, 65, &mut current_width);
        assert_eq!(ch.width(), 3);
        assert_eq!(current_width, 3);
        assert_eq!(ch.rows(), &[" A "]);
    }

    #[test]
    fn test_lookup_char_fallback() {
        let font = test_font();
        let mut current_width = 0;
        let ch = lookup_char(&font, 9999, &mut current_width);
        assert_eq!(ch.width(), 1);
        assert_eq!(current_width, 1);
        assert_eq!(ch.rows(), &["#"]);
    }

    #[test]
    fn test_lookup_char_previous_width() {
        let font = test_font();
        let mut current_width = 0;
        lookup_char(&font, 65, &mut current_width);
        assert_eq!(current_width, 3);
        let previous_width = current_width;
        lookup_char(&font, 66, &mut current_width);
        assert_eq!(current_width, 4);
        assert_eq!(previous_width, 3);
    }

    // --- calc_smush_amount tests ---

    const HB: char = '$';

    #[test]
    fn test_smush_no_mode() {
        let mode = SmushMode::new(0);
        let output = vec!["A".to_string()];
        let curr = vec!["B".to_string()];
        let result = calc_smush_amount(&output, &curr, 1, 1, 2, mode, HB, false);
        assert_eq!(result, 0);
    }

    #[test]
    fn test_smush_kerning_only() {
        let mode = SmushMode::new(SmushMode::KERN);
        let output = vec!["A".to_string()];
        let curr = vec!["A".to_string()];
        let result = calc_smush_amount(&output, &curr, 1, 1, 2, mode, HB, false);
        assert_eq!(result, 0);
    }

    #[test]
    fn test_smush_ltr_basic() {
        let mode = SmushMode::new(SmushMode::SMUSH);
        let output = vec!["A".to_string()];
        let curr = vec![" B ".to_string()];
        let result = calc_smush_amount(&output, &curr, 1, 3, 2, mode, HB, false);
        assert_eq!(result, 2);
    }

    #[test]
    fn test_smush_rtl_basic() {
        let mode = SmushMode::new(SmushMode::SMUSH);
        let output = vec![" A".to_string()];
        let curr = vec!["B ".to_string()];
        let result = calc_smush_amount(&output, &curr, 2, 2, 2, mode, HB, true);
        assert_eq!(result, 2);
    }

    #[test]
    fn test_smush_takes_row_min() {
        let mode = SmushMode::new(SmushMode::SMUSH);
        let output = vec!["AAA".to_string(), "A A".to_string()];
        let curr = vec!["  B".to_string(), "B  ".to_string()];
        let result = calc_smush_amount(&output, &curr, 3, 3, 2, mode, HB, false);
        assert_eq!(result, 1);
    }

    #[test]
    fn test_smush_boundary_smush() {
        let mode = SmushMode::new(SmushMode::SMUSH | SmushMode::EQUAL_CHARS);
        let output = vec!["A".to_string()];
        let curr = vec![" A".to_string()];
        let result = calc_smush_amount(&output, &curr, 1, 2, 2, mode, HB, false);
        assert_eq!(result, 2);
    }

    #[test]
    fn test_smush_boundary_no_smush() {
        let mode = SmushMode::new(SmushMode::SMUSH | SmushMode::EQUAL_CHARS);
        let output = vec!["A".to_string()];
        let curr = vec![" B".to_string()];
        let result = calc_smush_amount(&output, &curr, 1, 2, 2, mode, HB, false);
        assert_eq!(result, 1);
    }

    #[test]
    fn test_smush_output_all_spaces() {
        let mode = SmushMode::new(SmushMode::SMUSH);
        let output = vec!["  ".to_string()];
        let curr = vec!["A ".to_string()];
        let result = calc_smush_amount(&output, &curr, 2, 2, 2, mode, HB, false);
        assert_eq!(result, 2);
    }

    #[test]
    fn test_smush_curr_all_spaces() {
        let mode = SmushMode::new(SmushMode::SMUSH);
        let output = vec!["A ".to_string()];
        let curr = vec!["  ".to_string()];
        let result = calc_smush_amount(&output, &curr, 2, 2, 2, mode, HB, false);
        assert_eq!(result, 2);
    }

    // --- add_char tests ---

    fn add_char_font() -> FIGfont {
        let mut chars = HashMap::new();
        chars.insert(0, FIGcharacter::from(vec!["#".to_string()]));
        chars.insert(72, FIGcharacter::from(vec![" H ".to_string()])); // H
        chars.insert(73, FIGcharacter::from(vec![" I ".to_string()])); // I
        chars.insert(105, FIGcharacter::from(vec![" i ".to_string()])); // i
        chars.insert(65, FIGcharacter::from(vec!["AA".to_string()])); // A
        chars.insert(66, FIGcharacter::from(vec!["BB".to_string()])); // B
        chars.insert(33, FIGcharacter::from(vec!["!!".to_string()])); // !
        FIGfont {
            chars,
            ..FIGfont::default()
        }
    }

    fn setup_add_char() -> (Vec<String>, usize, usize) {
        (vec!["".to_string()], 0, 0)
    }

    #[test]
    fn test_add_char_10_as() {
        let font_bytes = include_bytes!("../assets/fonts/standard.flf");
        let font_str = String::from_utf8_lossy(font_bytes);
        let font = crate::font::parse_tlf_font(&font_str).unwrap();
        let mode = crate::smush::SmushMode::new(font.full_layout as u32);
        let mut output_rows = vec![String::new(); font.charheight as usize];
        let mut outlinelen = 0;
        let mut prev_width = 0;
        let limit = 79;
        for i in 0..10 {
            let ok = add_char(
                &font,
                'a' as u32,
                &mut output_rows,
                &mut outlinelen,
                &mut prev_width,
                mode,
                false,
                limit,
            );
            println!("add_char {}: ok={}, outlinelen={}", i, ok, outlinelen);
            if i < 9 {
                let ok2 = add_char(
                    &font,
                    ' ' as u32,
                    &mut output_rows,
                    &mut outlinelen,
                    &mut prev_width,
                    mode,
                    false,
                    limit,
                );
                println!("add_space {}: ok={}, outlinelen={}", i, ok2, outlinelen);
            }
        }
        println!("final outlinelen={}", outlinelen);
    }

    #[test]
    fn test_add_char_first_char_ltr() {
        let font = add_char_font();
        let mode = SmushMode::new(SmushMode::KERN);
        let (mut output_rows, mut outlinelen, mut prev_width) = setup_add_char();

        let ok = add_char(
            &font,
            72,
            &mut output_rows,
            &mut outlinelen,
            &mut prev_width,
            mode,
            false,
            200,
        );
        assert!(ok);
        assert_eq!(outlinelen, 2);
        assert_eq!(output_rows[0], "H ");
        assert_eq!(prev_width, 3);
    }

    #[test]
    fn test_add_char_two_chars_kerning() {
        let font = add_char_font();
        let mode = SmushMode::new(SmushMode::KERN);
        let (mut output_rows, mut outlinelen, mut prev_width) = setup_add_char();

        assert!(add_char(
            &font,
            72,
            &mut output_rows,
            &mut outlinelen,
            &mut prev_width,
            mode,
            false,
            200
        ));
        assert!(add_char(
            &font,
            105,
            &mut output_rows,
            &mut outlinelen,
            &mut prev_width,
            mode,
            false,
            200
        ));
        assert_eq!(outlinelen, 3);
        assert_eq!(output_rows[0], "Hi ");
    }

    #[test]
    fn test_add_char_two_chars_smush() {
        let font = add_char_font();
        let mode = SmushMode::new(SmushMode::SMUSH | SmushMode::EQUAL_CHARS);
        let (mut output_rows, mut outlinelen, mut prev_width) = setup_add_char();

        assert!(add_char(
            &font,
            65,
            &mut output_rows,
            &mut outlinelen,
            &mut prev_width,
            mode,
            false,
            200
        ));
        assert!(add_char(
            &font,
            65,
            &mut output_rows,
            &mut outlinelen,
            &mut prev_width,
            mode,
            false,
            200
        ));
        assert_eq!(output_rows[0], "AAA");
    }

    #[test]
    fn test_add_char_rtl_smush() {
        let font = add_char_font();
        let mode = SmushMode::new(SmushMode::SMUSH | SmushMode::EQUAL_CHARS);
        let (mut output_rows, mut outlinelen, mut prev_width) = setup_add_char();

        assert!(add_char(
            &font,
            65,
            &mut output_rows,
            &mut outlinelen,
            &mut prev_width,
            mode,
            true,
            200
        ));
        assert!(add_char(
            &font,
            65,
            &mut output_rows,
            &mut outlinelen,
            &mut prev_width,
            mode,
            true,
            200
        ));
        assert_eq!(output_rows[0], "AAA");
    }

    #[test]
    fn test_add_char_limit_bail() {
        let font = add_char_font();
        let mode = SmushMode::new(SmushMode::KERN);
        let (mut output_rows, mut outlinelen, mut prev_width) = setup_add_char();

        assert!(add_char(
            &font,
            72,
            &mut output_rows,
            &mut outlinelen,
            &mut prev_width,
            mode,
            false,
            3
        ));
        assert_eq!(outlinelen, 2);
        assert_eq!(prev_width, 3);

        let ok = add_char(
            &font,
            73,
            &mut output_rows,
            &mut outlinelen,
            &mut prev_width,
            mode,
            false,
            2,
        );
        assert!(!ok);
    }

    #[test]
    fn test_add_char_restores_prev_width_on_fail() {
        let font = add_char_font();
        let mode = SmushMode::new(SmushMode::KERN);
        let (mut output_rows, mut outlinelen, mut prev_width) = setup_add_char();

        assert!(add_char(
            &font,
            72,
            &mut output_rows,
            &mut outlinelen,
            &mut prev_width,
            mode,
            false,
            200
        ));
        assert_eq!(prev_width, 3);

        let prev_width_before = prev_width;
        let ok = add_char(
            &font,
            73,
            &mut output_rows,
            &mut outlinelen,
            &mut prev_width,
            mode,
            false,
            2,
        );
        assert!(!ok);
        assert_eq!(prev_width, prev_width_before);
    }

    #[test]
    fn test_add_char_single_word_c_output() {
        let font = add_char_font();
        let mode = SmushMode::new(SmushMode::KERN);
        let (mut output_rows, mut outlinelen, mut prev_width) = setup_add_char();

        // Build "Hi!"
        assert!(add_char(
            &font,
            72,
            &mut output_rows,
            &mut outlinelen,
            &mut prev_width,
            mode,
            false,
            200
        ));
        assert!(add_char(
            &font,
            105,
            &mut output_rows,
            &mut outlinelen,
            &mut prev_width,
            mode,
            false,
            200
        ));
        assert!(add_char(
            &font,
            33,
            &mut output_rows,
            &mut outlinelen,
            &mut prev_width,
            mode,
            false,
            200
        ));
        assert_eq!(output_rows[0], "Hi!!");
    }

    #[test]
    fn test_add_char_multi_row() {
        let mut chars = HashMap::new();
        chars.insert(0, FIGcharacter::from(vec!["#".to_string(); 2]));
        chars.insert(
            65,
            FIGcharacter::from(vec![" A ".to_string(), " A ".to_string()]),
        );
        chars.insert(
            66,
            FIGcharacter::from(vec![" B ".to_string(), " B ".to_string()]),
        );
        let font = FIGfont {
            chars,
            charheight: 2,
            ..FIGfont::default()
        };
        let mode = SmushMode::new(SmushMode::SMUSH | SmushMode::EQUAL_CHARS);
        let mut output_rows = vec!["".to_string(), "".to_string()];
        let mut outlinelen = 0;
        let mut prev_width = 0;

        assert!(add_char(
            &font,
            65,
            &mut output_rows,
            &mut outlinelen,
            &mut prev_width,
            mode,
            false,
            200
        ));
        assert!(add_char(
            &font,
            66,
            &mut output_rows,
            &mut outlinelen,
            &mut prev_width,
            mode,
            false,
            200
        ));

        assert_eq!(output_rows[0], "AB ");
        assert_eq!(output_rows[1], "AB ");
    }

    // --- render_line tests ---

    #[test]
    fn test_hardblank_to_space() {
        let rows = vec!["A$B".to_string()];
        let result = render_line(&rows, '$', Justification::Left, 80);
        assert_eq!(result[0], "A B");
    }

    #[test]
    fn test_left_justification() {
        let rows = vec!["Hello".to_string()];
        let result = render_line(&rows, '$', Justification::Left, 80);
        assert_eq!(result[0], "Hello");
    }

    #[test]
    fn test_center_justification() {
        let rows = vec!["Hi".to_string()];
        let result = render_line(&rows, '$', Justification::Center, 10);
        // 2*i + 2 - 1 < 10 → 2*i + 1 < 10 → i=1,2,3,4 → 4 spaces
        // "(outputwidth - len) / 2" = (10-2)/2 = 4
        assert_eq!(result[0], "    Hi");
    }

    #[test]
    fn test_right_justification() {
        let rows = vec!["Hi".to_string()];
        let result = render_line(&rows, '$', Justification::Right, 10);
        // i + 2 < 10 → i=1..7 → 7 spaces
        // outputwidth - len - 1 = 10-2-1 = 7
        assert_eq!(result[0], "       Hi");
    }

    #[test]
    fn test_width_truncation() {
        let rows = vec!["HelloWorld".to_string()];
        let result = render_line(&rows, '$', Justification::Left, 6);
        // outputwidth=6 > 1, so max_len=5
        assert_eq!(result[0], "Hello");
    }

    #[test]
    fn test_width_truncation_with_center() {
        let rows = vec!["HelloWorld".to_string()];
        let result = render_line(&rows, '$', Justification::Center, 8);
        // outputwidth=8 > 1, truncate to 7 → "HelloWo"
        // len=7, center: 2*i + 7 - 1 < 8 → 2*i + 6 < 8 → i=1: 8<8 false → 0 spaces
        assert_eq!(result[0], "HelloWo");
    }

    #[test]
    fn test_outputwidth_leq_one() {
        let rows = vec!["HelloWorld".to_string()];
        let result = render_line(&rows, '$', Justification::Center, 1);
        // outputwidth=1 → no truncation, no justification
        assert_eq!(result[0], "HelloWorld");
    }

    #[test]
    fn test_multi_row() {
        let rows = vec!["AAAA".to_string(), "BBBB".to_string()];
        let result = render_line(&rows, '$', Justification::Right, 10);
        // len=4, right: i + 4 < 10 → i=1..5 → 5 spaces
        assert_eq!(result[0], "     AAAA");
        assert_eq!(result[1], "     BBBB");
    }

    #[test]
    fn test_center_exact_formula() {
        // Trace C formula: outputwidth=15, len=5, center
        // 2*i + 5 - 1 < 15 → 2*i + 4 < 15 → i=1,2,3,4,5 → 5 spaces
        let rows = vec!["Hello".to_string()];
        let result = render_line(&rows, '$', Justification::Center, 15);
        assert_eq!(result[0], "     Hello");
        // visible width = 5 + 5 = 10, which is outputwidth-1=14?
        // No, the formula doesn't guarantee exact centering, it replicates C
        // Let's check: i=5 → 2*5+4=14<15, print 5th space. i=6 → 2*6+4=16<15 false
        // 5 spaces + 5 chars = 10 total.
        assert_eq!(result[0].chars().count(), 10);
    }

    #[test]
    fn test_right_exact_formula() {
        // Trace C formula: outputwidth=12, len=4, right
        // i + 4 < 12 → i=1..7 → 7 spaces
        let rows = vec!["test".to_string()];
        let result = render_line(&rows, '$', Justification::Right, 12);
        assert_eq!(result[0], "       test");
        assert_eq!(result[0].chars().count(), 11); // 7 + 4 = 11
    }

    #[test]
    fn test_hardblank_with_truncation() {
        let rows = vec!["A$B$C".to_string()];
        let result = render_line(&rows, '$', Justification::Left, 4);
        // hardblank → space: "A B C", truncated to 3: "A B"
        assert_eq!(result[0], "A B");
    }

    #[test]
    fn test_outputwidth_zero() {
        let rows = vec!["test".to_string()];
        let result = render_line(&rows, '$', Justification::Center, 0);
        // outputwidth=0, which is NOT > 1, so no truncation/justification
        assert_eq!(result[0], "test");
    }

    #[test]
    fn test_empty_rows() {
        let rows: Vec<String> = Vec::new();
        let result = render_line(&rows, '$', Justification::Center, 80);
        assert!(result.is_empty());
    }

    // --- split_line tests ---

    fn split_font() -> FIGfont {
        let mut chars = HashMap::new();
        chars.insert(0, FIGcharacter::from(vec!["#".to_string()]));
        chars.insert(32, FIGcharacter::from(vec![" ".to_string()]));
        chars.insert(65, FIGcharacter::from(vec![" A ".to_string()]));
        chars.insert(66, FIGcharacter::from(vec![" B  ".to_string()]));
        chars.insert(72, FIGcharacter::from(vec![" H ".to_string()]));
        chars.insert(105, FIGcharacter::from(vec![" i ".to_string()]));
        chars.insert(87, FIGcharacter::from(vec![" W ".to_string()]));
        chars.insert(111, FIGcharacter::from(vec![" o ".to_string()]));
        chars.insert(114, FIGcharacter::from(vec![" r ".to_string()]));
        chars.insert(108, FIGcharacter::from(vec![" l ".to_string()]));
        chars.insert(100, FIGcharacter::from(vec![" d ".to_string()]));
        chars.insert(67, FIGcharacter::from(vec![" C ".to_string()]));
        FIGfont {
            chars,
            ..FIGfont::default()
        }
    }

    fn build_expected(
        part_codes: &[u32],
        font: &FIGfont,
        mode: SmushMode,
        limit: usize,
    ) -> (Vec<String>, usize, usize) {
        let height = font.charheight as usize;
        let mut rows = vec![String::new(); height];
        let mut len = 0;
        let mut prev = 0;
        for &code in part_codes {
            add_char(
                font, code, &mut rows, &mut len, &mut prev, mode, false, limit,
            );
        }
        (rows, len, prev)
    }

    #[test]
    fn test_split_line_basic_multiword() {
        let font = split_font();
        let mode = SmushMode::new(SmushMode::KERN);
        let limit = 200;
        let char_buffer = vec![72, 105, 32, 87, 111, 114, 108, 100]; // "Hi World"

        let height = font.charheight as usize;
        let mut output_rows = vec![String::new(); height];
        let mut outlinelen = 0;
        let mut prev_width = 0;

        let result = split_line(
            &font,
            &char_buffer,
            &mut output_rows,
            &mut outlinelen,
            &mut prev_width,
            mode,
            false,
            limit,
        );

        assert!(result.is_some());
        let (part1_rows, part2_start) = result.unwrap();
        assert_eq!(part2_start, 3);

        let (expected_p1, _, _) = build_expected(&[72, 105], &font, mode, limit);
        assert_eq!(part1_rows, expected_p1);

        let (expected_p2, expected_len, expected_prev) =
            build_expected(&[87, 111, 114, 108, 100], &font, mode, limit);
        assert_eq!(output_rows, expected_p2);
        assert_eq!(outlinelen, expected_len);
        assert_eq!(prev_width, expected_prev);
    }

    #[test]
    fn test_split_line_multiple_spaces() {
        let font = split_font();
        let mode = SmushMode::new(SmushMode::KERN);
        let limit = 200;
        let char_buffer = vec![65, 32, 32, 32, 66]; // "A   B"

        let height = font.charheight as usize;
        let mut output_rows = vec![String::new(); height];
        let mut outlinelen = 0;
        let mut prev_width = 0;

        let result = split_line(
            &font,
            &char_buffer,
            &mut output_rows,
            &mut outlinelen,
            &mut prev_width,
            mode,
            false,
            limit,
        );

        assert!(result.is_some());
        let (part1_rows, part2_start) = result.unwrap();
        assert_eq!(part2_start, 4);

        let (expected_p1, _, _) = build_expected(&[65], &font, mode, limit);
        assert_eq!(part1_rows, expected_p1);

        let (expected_p2, expected_len, expected_prev) = build_expected(&[66], &font, mode, limit);
        assert_eq!(output_rows, expected_p2);
        assert_eq!(outlinelen, expected_len);
        assert_eq!(prev_width, expected_prev);
    }

    #[test]
    fn test_split_line_no_word_break() {
        let font = split_font();
        let mode = SmushMode::new(SmushMode::KERN);
        let limit = 200;
        let char_buffer = vec![72, 105]; // "Hi" — no space

        let mut output_rows = vec!["".to_string()];
        let mut outlinelen = 0;
        let mut prev_width = 0;

        let result = split_line(
            &font,
            &char_buffer,
            &mut output_rows,
            &mut outlinelen,
            &mut prev_width,
            mode,
            false,
            limit,
        );

        assert!(result.is_none());
    }

    #[test]
    fn test_split_line_single_char_after_space() {
        let font = split_font();
        let mode = SmushMode::new(SmushMode::KERN);
        let limit = 200;
        let char_buffer = vec![72, 32, 105]; // "H i"

        let height = font.charheight as usize;
        let mut output_rows = vec![String::new(); height];
        let mut outlinelen = 0;
        let mut prev_width = 0;

        let result = split_line(
            &font,
            &char_buffer,
            &mut output_rows,
            &mut outlinelen,
            &mut prev_width,
            mode,
            false,
            limit,
        );

        assert!(result.is_some());
        let (part1_rows, part2_start) = result.unwrap();
        assert_eq!(part2_start, 2);

        let (expected_p1, _, _) = build_expected(&[72], &font, mode, limit);
        assert_eq!(part1_rows, expected_p1);

        let (expected_p2, expected_len, expected_prev) = build_expected(&[105], &font, mode, limit);
        assert_eq!(output_rows, expected_p2);
        assert_eq!(outlinelen, expected_len);
        assert_eq!(prev_width, expected_prev);
    }

    #[test]
    fn test_split_line_leading_spaces() {
        let font = split_font();
        let mode = SmushMode::new(SmushMode::KERN);
        let limit = 200;
        let char_buffer = vec![32, 65, 32, 66]; // " A B" (no trailing space)

        let height = font.charheight as usize;
        let mut output_rows = vec![String::new(); height];
        let mut outlinelen = 0;
        let mut prev_width = 0;

        let result = split_line(
            &font,
            &char_buffer,
            &mut output_rows,
            &mut outlinelen,
            &mut prev_width,
            mode,
            false,
            limit,
        );

        assert!(result.is_some());
        let (part1_rows, part2_start) = result.unwrap();
        assert_eq!(part2_start, 3);

        let (expected_p1, _, _) = build_expected(&[32, 65], &font, mode, limit);
        assert_eq!(part1_rows, expected_p1);

        let (expected_p2, expected_len, expected_prev) = build_expected(&[66], &font, mode, limit);
        assert_eq!(output_rows, expected_p2);
        assert_eq!(outlinelen, expected_len);
        assert_eq!(prev_width, expected_prev);
    }

    #[test]
    fn test_split_line_all_spaces() {
        let font = split_font();
        let mode = SmushMode::new(SmushMode::KERN);
        let limit = 200;
        let char_buffer = vec![32, 32, 32]; // "   "

        let height = font.charheight as usize;
        let mut output_rows = vec![String::new(); height];
        let mut outlinelen = 0;
        let mut prev_width = 0;

        let result = split_line(
            &font,
            &char_buffer,
            &mut output_rows,
            &mut outlinelen,
            &mut prev_width,
            mode,
            false,
            limit,
        );

        assert!(result.is_some());
        let (part1_rows, part2_start) = result.unwrap();
        assert_eq!(part2_start, 3);

        let (expected_p1, _, _) = build_expected(&[], &font, mode, limit);
        assert_eq!(part1_rows, expected_p1);

        let (expected_p2, expected_len, expected_prev) = build_expected(&[], &font, mode, limit);
        assert_eq!(output_rows, expected_p2);
        assert_eq!(outlinelen, expected_len);
        assert_eq!(prev_width, expected_prev);
    }

    #[test]
    fn test_split_line_multirow() {
        let mut chars = HashMap::new();
        chars.insert(0, FIGcharacter::from(vec!["#".to_string(); 2]));
        chars.insert(32, FIGcharacter::from(vec![" ".to_string(); 2]));
        chars.insert(
            65,
            FIGcharacter::from(vec![" A ".to_string(), " A ".to_string()]),
        );
        chars.insert(
            66,
            FIGcharacter::from(vec![" B  ".to_string(), " B  ".to_string()]),
        );
        let font = FIGfont {
            chars,
            charheight: 2,
            ..FIGfont::default()
        };
        let mode = SmushMode::new(SmushMode::KERN);
        let limit = 200;
        let char_buffer = vec![65, 32, 66]; // "A B"

        let mut output_rows = vec![String::new(); 2];
        let mut outlinelen = 0;
        let mut prev_width = 0;

        let result = split_line(
            &font,
            &char_buffer,
            &mut output_rows,
            &mut outlinelen,
            &mut prev_width,
            mode,
            false,
            limit,
        );

        assert!(result.is_some());
        let (part1_rows, part2_start) = result.unwrap();
        assert_eq!(part2_start, 2);

        let (expected_p1, _, _) = build_expected(&[65], &font, mode, limit);
        assert_eq!(part1_rows, expected_p1);

        let (expected_p2, expected_len, expected_prev) = build_expected(&[66], &font, mode, limit);
        assert_eq!(output_rows, expected_p2);
        assert_eq!(outlinelen, expected_len);
        assert_eq!(prev_width, expected_prev);
    }

    #[test]
    fn test_split_line_empty_buffer() {
        let font = split_font();
        let mode = SmushMode::new(SmushMode::KERN);
        let limit = 200;
        let char_buffer: Vec<u32> = vec![];

        let mut output_rows = vec!["".to_string()];
        let mut outlinelen = 0;
        let mut prev_width = 0;

        let result = split_line(
            &font,
            &char_buffer,
            &mut output_rows,
            &mut outlinelen,
            &mut prev_width,
            mode,
            false,
            limit,
        );

        assert!(result.is_none());
    }

    #[test]
    fn test_render_string_basic() {
        let font_bytes = include_bytes!("../assets/fonts/standard.flf");
        let font_str = String::from_utf8_lossy(font_bytes);
        let font = crate::font::parse_tlf_font(&font_str).unwrap();
        let rows = render_string(&font, "AaBbCc123!?");
        assert!(!rows.is_empty());
        assert_eq!(rows.len(), font.charheight as usize);
        for row in &rows {
            assert!(!row.is_empty());
        }
    }

    #[test]
    fn test_render_string_empty() {
        let font_bytes = include_bytes!("../assets/fonts/standard.flf");
        let font_str = String::from_utf8_lossy(font_bytes);
        let font = crate::font::parse_tlf_font(&font_str).unwrap();
        let rows = render_string(&font, "");
        assert!(!rows.is_empty());
        assert_eq!(rows.len(), font.charheight as usize);
    }
}
