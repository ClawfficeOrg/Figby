use crate::control::CharReader;
use crate::font::DEUTSCH_CHARS;

pub fn deutsch_reroute(c: u32, deutschflag: bool) -> u32 {
    if !deutschflag {
        return c;
    }
    if (0x5B..=0x5D).contains(&c) {
        let idx = (c - 0x5B) as usize;
        DEUTSCH_CHARS[idx]
    } else if (0x7B..=0x7E).contains(&c) {
        let idx = (c - 0x7B) as usize;
        DEUTSCH_CHARS[3 + idx]
    } else {
        c
    }
}

pub fn read_dbcs_char(input: &mut impl CharReader) -> Option<u32> {
    let b = input.next()?;
    if (0x80..=0x9F).contains(&b) || (0xE0..=0xEF).contains(&b) {
        match input.next() {
            Some(trail) => Some((b << 8) | trail),
            None => Some(b),
        }
    } else {
        Some(b)
    }
}

#[derive(Default)]
pub struct HZState {
    pub hzmode: bool,
}

/// HZ (GB2312) reader — iterative.
///
/// Previously recursed per `~~` / `~X` escape sequence, so a long run of
/// tilde sequences could overflow the stack (GPT review F-16).
pub fn read_hz_char(input: &mut impl CharReader, state: &mut HZState) -> Option<u32> {
    loop {
        let b = input.next()?;
        if state.hzmode {
            if b == b'}' as u32 {
                if let Some(c) = input.next() {
                    if c == b'~' as u32 {
                        state.hzmode = false;
                        continue;
                    }
                    input.unget(c);
                }
            }
            match input.next() {
                Some(b2) => return Some((b << 8) | b2),
                None => return Some(b),
            }
        } else if b == b'~' as u32 {
            match input.next() {
                Some(c) if c == b'{' as u32 => {
                    state.hzmode = true;
                    continue;
                }
                Some(c) if c == b'~' as u32 => return Some(b'~' as u32),
                // Any other escape after '~' is skipped; keep scanning.
                Some(_) => continue,
                None => return Some(b'~' as u32),
            }
        } else {
            return Some(b);
        }
    }
}

pub fn read_utf8_char(input: &mut impl CharReader) -> Option<u32> {
    let b0 = input.next()?;

    if b0 < 0x80 {
        return Some(b0);
    }

    let length = if b0 & 0xE0 == 0xC0 {
        2
    } else if b0 & 0xF0 == 0xE0 {
        3
    } else if b0 & 0xF8 == 0xF0 {
        4
    } else if b0 & 0xFC == 0xF8 {
        5
    } else if b0 & 0xFE == 0xFC {
        6
    } else {
        return Some(0x0080);
    };

    let mut buf = [0u8; 6];
    buf[0] = b0 as u8;

    for slot in buf.iter_mut().take(length).skip(1) {
        match input.next() {
            Some(b) if b & 0xC0 == 0x80 => {
                *slot = b as u8;
            }
            _ => {
                return Some(0x0080);
            }
        }
    }

    match std::str::from_utf8(&buf[..length]) {
        Ok(s) => s.chars().next().map(|c| c as u32),
        Err(_) => Some(0x0080),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockReader {
        data: Vec<u8>,
        pos: usize,
        buf: Option<u32>,
    }

    impl MockReader {
        fn new(data: &[u8]) -> Self {
            Self {
                data: data.to_vec(),
                pos: 0,
                buf: None,
            }
        }
    }

    impl CharReader for MockReader {
        fn next(&mut self) -> Option<u32> {
            if let Some(c) = self.buf.take() {
                return Some(c);
            }
            if self.pos < self.data.len() {
                let c = self.data[self.pos] as u32;
                self.pos += 1;
                Some(c)
            } else {
                None
            }
        }

        fn unget(&mut self, c: u32) {
            self.buf = Some(c);
        }
    }

    #[test]
    fn test_utf8_ascii() {
        let mut input = MockReader::new(b"abc");
        assert_eq!(read_utf8_char(&mut input), Some('a' as u32));
        assert_eq!(read_utf8_char(&mut input), Some('b' as u32));
        assert_eq!(read_utf8_char(&mut input), Some('c' as u32));
        assert_eq!(read_utf8_char(&mut input), None);
    }

    #[test]
    fn test_utf8_2byte() {
        // U+00A9 © = 0xC2 0xA9
        let mut input = MockReader::new(&[0xC2, 0xA9]);
        assert_eq!(read_utf8_char(&mut input), Some(0x00A9));
        assert_eq!(read_utf8_char(&mut input), None);
    }

    #[test]
    fn test_utf8_3byte() {
        // U+4E2D 中 = 0xE4 0xB8 0xAD
        let mut input = MockReader::new(&[0xE4, 0xB8, 0xAD]);
        assert_eq!(read_utf8_char(&mut input), Some(0x4E2D));
        assert_eq!(read_utf8_char(&mut input), None);
    }

    #[test]
    fn test_utf8_4byte() {
        // U+1F600 😀 = 0xF0 0x9F 0x98 0x80
        let mut input = MockReader::new(&[0xF0, 0x9F, 0x98, 0x80]);
        assert_eq!(read_utf8_char(&mut input), Some(0x1F600));
        assert_eq!(read_utf8_char(&mut input), None);
    }

    #[test]
    fn test_utf8_overlong_c0() {
        // Overlong 2-byte encoding of NUL: 0xC0 0x80
        let mut input = MockReader::new(&[0xC0, 0x80]);
        assert_eq!(read_utf8_char(&mut input), Some(0x0080));
    }

    #[test]
    fn test_utf8_overlong_c1() {
        // Overlong 2-byte encoding of 0x7F: 0xC1 0xBF
        let mut input = MockReader::new(&[0xC1, 0xBF]);
        assert_eq!(read_utf8_char(&mut input), Some(0x0080));
    }

    #[test]
    fn test_utf8_surrogate() {
        // Surrogate U+D800 encoded as 0xED 0xA0 0x80
        let mut input = MockReader::new(&[0xED, 0xA0, 0x80]);
        assert_eq!(read_utf8_char(&mut input), Some(0x0080));
    }

    #[test]
    fn test_utf8_invalid_lead_byte() {
        // 0xFF
        let mut input = MockReader::new(&[0xFF]);
        assert_eq!(read_utf8_char(&mut input), Some(0x0080));
        // 0xFE
        let mut input = MockReader::new(&[0xFE]);
        assert_eq!(read_utf8_char(&mut input), Some(0x0080));
    }

    #[test]
    fn test_utf8_f5_plus() {
        // 0xF5 0x80 0x80 0x80 would decode to 0x140000 > U+10FFFF
        let mut input = MockReader::new(&[0xF5, 0x80, 0x80, 0x80]);
        assert_eq!(read_utf8_char(&mut input), Some(0x0080));
        // 0xF6
        let mut input = MockReader::new(&[0xF6, 0x80, 0x80, 0x80]);
        assert_eq!(read_utf8_char(&mut input), Some(0x0080));
        // 0xF7
        let mut input = MockReader::new(&[0xF7, 0x80, 0x80, 0x80]);
        assert_eq!(read_utf8_char(&mut input), Some(0x0080));
    }

    #[test]
    fn test_utf8_truncated() {
        // 2-byte sequence missing continuation byte
        let mut input = MockReader::new(&[0xC2]);
        assert_eq!(read_utf8_char(&mut input), Some(0x0080));
        // 3-byte sequence with only 1 continuation byte
        let mut input = MockReader::new(&[0xE4, 0xB8]);
        assert_eq!(read_utf8_char(&mut input), Some(0x0080));
        // Empty after partial
        assert_eq!(read_utf8_char(&mut input), None);
    }

    #[test]
    fn test_utf8_bad_continuation() {
        // Valid leading byte but invalid continuation (0x00 instead of 0x80..0xBF)
        let mut input = MockReader::new(&[0xC2, 0x00]);
        assert_eq!(read_utf8_char(&mut input), Some(0x0080));
    }

    #[test]
    fn test_utf8_eof_on_first_byte() {
        let mut input = MockReader::new(b"");
        assert_eq!(read_utf8_char(&mut input), None);
    }

    #[test]
    fn test_utf8_multiple_chars() {
        let mut input = MockReader::new("A©中😀".as_bytes());
        assert_eq!(read_utf8_char(&mut input), Some('A' as u32));
        assert_eq!(read_utf8_char(&mut input), Some(0x00A9));
        assert_eq!(read_utf8_char(&mut input), Some(0x4E2D));
        assert_eq!(read_utf8_char(&mut input), Some(0x1F600));
        assert_eq!(read_utf8_char(&mut input), None);
    }

    // --- DBCS tests ---

    #[test]
    fn test_dbcs_single_byte() {
        let mut input = MockReader::new(b"abc\xA0\xDF\xF0\xFF");
        assert_eq!(read_dbcs_char(&mut input), Some(b'a' as u32));
        assert_eq!(read_dbcs_char(&mut input), Some(b'b' as u32));
        assert_eq!(read_dbcs_char(&mut input), Some(b'c' as u32));
        assert_eq!(read_dbcs_char(&mut input), Some(0xA0));
        assert_eq!(read_dbcs_char(&mut input), Some(0xDF));
        assert_eq!(read_dbcs_char(&mut input), Some(0xF0));
        assert_eq!(read_dbcs_char(&mut input), Some(0xFF));
        assert_eq!(read_dbcs_char(&mut input), None);
    }

    #[test]
    fn test_dbcs_lead_80_9f() {
        let mut input = MockReader::new(&[0x81, 0x40]);
        assert_eq!(read_dbcs_char(&mut input), Some((0x81u32 << 8) | 0x40));
        assert_eq!(read_dbcs_char(&mut input), None);
    }

    #[test]
    fn test_dbcs_lead_e0_ef() {
        let mut input = MockReader::new(&[0xE0, 0x80]);
        assert_eq!(read_dbcs_char(&mut input), Some((0xE0u32 << 8) | 0x80));
        assert_eq!(read_dbcs_char(&mut input), None);
    }

    #[test]
    fn test_dbcs_eof_after_lead() {
        let mut input = MockReader::new(&[0x81]);
        assert_eq!(read_dbcs_char(&mut input), Some(0x81));
    }

    #[test]
    fn test_dbcs_eof_on_first() {
        let mut input = MockReader::new(b"");
        assert_eq!(read_dbcs_char(&mut input), None);
    }

    // --- HZ tests ---

    #[test]
    fn test_hz_ascii_passthrough() {
        let mut input = MockReader::new(b"hello");
        let mut state = HZState::default();
        assert_eq!(read_hz_char(&mut input, &mut state), Some(b'h' as u32));
        assert_eq!(read_hz_char(&mut input, &mut state), Some(b'e' as u32));
        assert_eq!(read_hz_char(&mut input, &mut state), Some(b'l' as u32));
        assert_eq!(read_hz_char(&mut input, &mut state), Some(b'l' as u32));
        assert_eq!(read_hz_char(&mut input, &mut state), Some(b'o' as u32));
        assert_eq!(read_hz_char(&mut input, &mut state), None);
    }

    #[test]
    fn test_hz_enter_exit() {
        let mut input = MockReader::new(b"a~{BC}~d");
        let mut state = HZState::default();
        assert_eq!(read_hz_char(&mut input, &mut state), Some(b'a' as u32));
        assert_eq!(
            read_hz_char(&mut input, &mut state),
            Some((b'B' as u32) << 8 | b'C' as u32)
        );
        assert_eq!(read_hz_char(&mut input, &mut state), Some(b'd' as u32));
        assert_eq!(read_hz_char(&mut input, &mut state), None);
    }

    #[test]
    fn test_hz_double_byte_content() {
        let mut input = MockReader::new(b"~{AB}~");
        let mut state = HZState::default();
        assert_eq!(
            read_hz_char(&mut input, &mut state),
            Some((b'A' as u32) << 8 | b'B' as u32)
        );
        assert!(read_hz_char(&mut input, &mut state).is_none());
    }

    #[test]
    fn test_hz_tilde_escape() {
        let mut input = MockReader::new(b"a~~b");
        let mut state = HZState::default();
        assert_eq!(read_hz_char(&mut input, &mut state), Some(b'a' as u32));
        assert_eq!(read_hz_char(&mut input, &mut state), Some(b'~' as u32));
        assert_eq!(read_hz_char(&mut input, &mut state), Some(b'b' as u32));
        assert_eq!(read_hz_char(&mut input, &mut state), None);
    }

    #[test]
    fn test_hz_skip_unknown() {
        let mut input = MockReader::new(b"a~xb");
        let mut state = HZState::default();
        assert_eq!(read_hz_char(&mut input, &mut state), Some(b'a' as u32));
        assert_eq!(read_hz_char(&mut input, &mut state), Some(b'b' as u32));
        assert_eq!(read_hz_char(&mut input, &mut state), None);
    }

    #[test]
    fn test_hz_eof_in_intro() {
        let mut input = MockReader::new(b"a~");
        let mut state = HZState::default();
        assert_eq!(read_hz_char(&mut input, &mut state), Some(b'a' as u32));
        assert_eq!(read_hz_char(&mut input, &mut state), Some(b'~' as u32));
        assert_eq!(read_hz_char(&mut input, &mut state), None);
    }

    #[test]
    fn test_hz_eof_in_exit() {
        // In hz_mode, }~ is the exit sequence. If we see } followed by EOF,
        // exit doesn't trigger, } becomes first byte of a pair, EOF ends it.
        let mut state = HZState { hzmode: true };
        let mut input = MockReader::new(b"}");
        assert_eq!(read_hz_char(&mut input, &mut state), Some(b'}' as u32));
        assert_eq!(read_hz_char(&mut input, &mut state), None);
    }

    #[test]
    fn test_hz_entry_eof_recurse() {
        // ~{ enters HZ mode then recurses. Recursive call hits EOF.
        let mut input = MockReader::new(b"~{");
        let mut state = HZState::default();
        assert_eq!(read_hz_char(&mut input, &mut state), None);
    }

    // --- Deutsch reroute tests ---

    #[test]
    fn test_deutsch_upper_a_umlaut() {
        assert_eq!(deutsch_reroute(0x5B, true), 196);
    }

    #[test]
    fn test_deutsch_upper_o_umlaut() {
        assert_eq!(deutsch_reroute(0x5C, true), 214);
    }

    #[test]
    fn test_deutsch_upper_u_umlaut() {
        assert_eq!(deutsch_reroute(0x5D, true), 220);
    }

    #[test]
    fn test_deutsch_lower_a_umlaut() {
        assert_eq!(deutsch_reroute(0x7B, true), 228);
    }

    #[test]
    fn test_deutsch_lower_o_umlaut() {
        assert_eq!(deutsch_reroute(0x7C, true), 246);
    }

    #[test]
    fn test_deutsch_lower_u_umlaut() {
        assert_eq!(deutsch_reroute(0x7D, true), 252);
    }

    #[test]
    fn test_deutsch_eszett() {
        assert_eq!(deutsch_reroute(0x7E, true), 223);
    }

    #[test]
    fn test_deutsch_disabled() {
        assert_eq!(deutsch_reroute(0x5B, false), 0x5B);
    }

    #[test]
    fn test_deutsch_out_of_range() {
        assert_eq!(deutsch_reroute(b'Z' as u32, true), b'Z' as u32);
        assert_eq!(deutsch_reroute(b'a' as u32, true), b'a' as u32);
        assert_eq!(deutsch_reroute(b' ' as u32, true), b' ' as u32);
    }

    // --- HZ recursion guard (GPT review F-16) ---

    #[test]
    fn test_hz_long_escape_run_no_overflow() {
        // 500k '~X' escape pairs previously meant 500k stack frames.
        let mut data = Vec::new();
        for _ in 0..500_000 {
            data.push(b'~');
            data.push(b'x');
        }
        data.push(b'A');
        let mut input = MockReader::new(&data);
        let mut state = HZState::default();
        let ch = read_hz_char(&mut input, &mut state);
        assert_eq!(ch, Some(b'A' as u32));
    }
}
