use std::fs::File;
use std::io::{self, BufReader, Read};
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub struct ControlCommand {
    pub thecommand: u8,
    pub rangelo: u32,
    pub rangehi: u32,
    pub offset: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ControlState {
    pub commands: Vec<ControlCommand>,
    pub multibyte: u32,
    pub gn: [u32; 4],
    pub gndbl: [bool; 4],
    pub gl: u8,
    pub gr: u8,
}

impl Default for ControlState {
    fn default() -> Self {
        Self {
            commands: Vec::new(),
            multibyte: 0,
            gn: [0x00, 0x01, 0, 0],
            gndbl: [false; 4],
            gl: 0,
            gr: 1,
        }
    }
}

#[derive(Debug)]
pub enum ControlError {
    IoError(io::Error),
    ParseError(String),
}

impl From<io::Error> for ControlError {
    fn from(e: io::Error) -> Self {
        ControlError::IoError(e)
    }
}

impl std::fmt::Display for ControlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ControlError::IoError(e) => write!(f, "I/O error: {}", e),
            ControlError::ParseError(s) => write!(f, "Parse error: {}", s),
        }
    }
}

impl std::error::Error for ControlError {}

pub trait CharReader {
    fn next(&mut self) -> Option<u32>;
    fn unget(&mut self, c: u32);
}

impl ControlState {
    /// ISO-2022 state machine, iterative.
    ///
    /// An earlier version recursed once per control byte, so input with a
    /// long run of shift/control sequences could overflow the stack
    /// (GPT review F-16). The explicit loop below is behavior-equivalent:
    /// control bytes update state and re-loop; the SS2/SS3-style handlers
    /// (0x8E/0x8F/0x14E/0x14F) push the previous GL/GR onto a stack that
    /// unwinds when the next printable char is produced.
    pub fn iso2022(&mut self, input: &mut impl CharReader) -> Option<u32> {
        let mut saved_gl_gr: Vec<(u8, u8)> = Vec::new();
        loop {
            let mut ch = input.next()?;
            if ch == 0x1B {
                ch = input.next()? + 0x100;
            }
            if ch == 0x124 {
                ch = input.next()? + 0x200;
            }
            match ch {
                0x0E => {
                    self.gl = 1;
                    continue;
                }
                0x0F => {
                    self.gl = 0;
                    continue;
                }
                0x8E | 0x14E | 0x8F | 0x14F => {
                    saved_gl_gr.push((self.gl, self.gr));
                    if ch == 0x8E || ch == 0x14E {
                        self.gl = 2;
                        self.gr = 2;
                    } else {
                        self.gl = 3;
                        self.gr = 3;
                    }
                    continue;
                }
                0x16E => {
                    self.gl = 2;
                    continue;
                }
                0x16F => {
                    self.gl = 3;
                    continue;
                }
                0x17E => {
                    self.gr = 1;
                    continue;
                }
                0x17D => {
                    self.gr = 2;
                    continue;
                }
                0x17C => {
                    self.gr = 3;
                    continue;
                }
                0x128..=0x12B => {
                    let c = input.next().unwrap_or(0);
                    let c = if c == 0x42 { 0 } else { c };
                    let idx = (ch - 0x128) as usize;
                    self.gn[idx] = c << 16;
                    self.gndbl[idx] = false;
                    continue;
                }
                0x12D..=0x12F => {
                    let c = input.next().unwrap_or(0);
                    let c = if c == 0x41 { 0 } else { c };
                    let idx = (ch - 0x12C) as usize;
                    self.gn[idx] = (c << 16) | 0x80;
                    self.gndbl[idx] = false;
                    continue;
                }
                0x228..=0x22B => {
                    let c = input.next().unwrap_or(0);
                    let idx = (ch - 0x228) as usize;
                    self.gn[idx] = c << 16;
                    self.gndbl[idx] = true;
                    continue;
                }
                _ => {
                    if ch & 0x200 != 0 {
                        self.gn[0] = (ch & !0x200) << 16;
                        self.gndbl[0] = true;
                        continue;
                    }
                }
            }

            // A printable character: unwind any pending GL/GR saves
            // (LIFO — outermost save wins), then emit it.
            while let Some((gl, gr)) = saved_gl_gr.pop() {
                self.gl = gl;
                self.gr = gr;
            }
            if (0x21..=0x7E).contains(&ch) {
                if self.gndbl[self.gl as usize] {
                    let ch2 = input.next().unwrap_or(0);
                    return Some(self.gn[self.gl as usize] | (ch << 8) | ch2);
                }
                return Some(self.gn[self.gl as usize] | ch);
            }
            if (0xA0..=0xFF).contains(&ch) {
                if self.gndbl[self.gr as usize] {
                    let ch2 = input.next().unwrap_or(0);
                    return Some(self.gn[self.gr as usize] | (ch << 8) | ch2);
                }
                return Some(self.gn[self.gr as usize] | (ch & !0x80));
            }
            return Some(ch);
        }
    }
}

struct ByteReader<R: Read> {
    reader: BufReader<R>,
    pushback: Vec<u8>,
}

impl<R: Read> ByteReader<R> {
    fn new(reader: R) -> Self {
        Self {
            reader: BufReader::new(reader),
            pushback: Vec::new(),
        }
    }

    fn next(&mut self) -> io::Result<Option<u8>> {
        if let Some(b) = self.pushback.pop() {
            return Ok(Some(b));
        }
        let mut buf = [0u8; 1];
        match self.reader.read(&mut buf) {
            Ok(0) => Ok(None),
            Ok(_) => Ok(Some(buf[0])),
            Err(e) => Err(e),
        }
    }

    fn unget(&mut self, b: u8) {
        self.pushback.push(b);
    }
}

fn skip_ws<R: Read>(reader: &mut ByteReader<R>) -> io::Result<()> {
    loop {
        match reader.next()? {
            Some(b) if b.is_ascii_whitespace() => continue,
            Some(b) => {
                reader.unget(b);
                return Ok(());
            }
            None => return Ok(()),
        }
    }
}

fn skip_to_eol<R: Read>(reader: &mut ByteReader<R>) -> io::Result<()> {
    loop {
        match reader.next()? {
            Some(b'\n') => return Ok(()),
            Some(b'\r') => {
                match reader.next()? {
                    Some(b'\n') => {}
                    Some(b) => reader.unget(b),
                    None => {}
                }
                return Ok(());
            }
            Some(_) => continue,
            None => return Ok(()),
        }
    }
}

fn read_num<R: Read>(reader: &mut ByteReader<R>) -> io::Result<i64> {
    skip_ws(reader)?;
    let mut sign = 1i64;
    match reader.next()? {
        Some(b'-') => sign = -1,
        Some(b) => reader.unget(b),
        None => return Ok(0),
    }

    let mut acc: u32 = 0;
    let hex_digits = b"0123456789ABCDEF";

    match reader.next()? {
        Some(b'0') => match reader.next()? {
            Some(b'x' | b'X') => loop {
                match reader.next()? {
                    Some(b @ b'0'..=b'9') => {
                        acc = acc * 16 + (b - b'0') as u32;
                    }
                    Some(b @ b'a'..=b'f') => {
                        acc = acc * 16 + (b - b'a' + 10) as u32;
                    }
                    Some(b @ b'A'..=b'F') => {
                        acc = acc * 16 + (b - b'A' + 10) as u32;
                    }
                    Some(b) => {
                        reader.unget(b);
                        break;
                    }
                    None => break,
                }
            },
            Some(b) => {
                reader.unget(b);
                loop {
                    match reader.next()? {
                        Some(b @ b'0'..=b'7') => {
                            acc = acc * 8 + (b - b'0') as u32;
                        }
                        Some(b) => {
                            reader.unget(b);
                            break;
                        }
                        None => break,
                    }
                }
            }
            None => {}
        },
        Some(b) => {
            reader.unget(b);
            while let Some(c) = reader.next()? {
                let c_upper = c.to_ascii_uppercase();
                let pos = hex_digits.iter().position(|&d| d == c_upper);
                match pos {
                    Some(d) => acc = acc * 10 + d as u32,
                    None => {
                        reader.unget(c);
                        break;
                    }
                }
            }
        }
        None => {}
    }

    Ok((acc as i64) * sign)
}

fn read_tchar<R: Read>(reader: &mut ByteReader<R>) -> io::Result<i64> {
    match reader.next()? {
        None => Ok(0),
        Some(b @ (b'\n' | b'\r')) => {
            reader.unget(b);
            Ok(0)
        }
        Some(b'\\') => match reader.next()? {
            None => Ok(0),
            Some(b'a') => Ok(7),
            Some(b'b') => Ok(8),
            Some(b'e') => Ok(27),
            Some(b'f') => Ok(12),
            Some(b'n') => Ok(10),
            Some(b'r') => Ok(13),
            Some(b't') => Ok(9),
            Some(b'v') => Ok(11),
            Some(next) if next == b'-' || next == b'x' || next.is_ascii_digit() => {
                reader.unget(next);
                read_num(reader)
            }
            Some(next) => Ok(next as i64),
        },
        Some(b) => Ok(b as i64),
    }
}

fn charset_name<R: Read>(reader: &mut ByteReader<R>) -> io::Result<i64> {
    let result = read_tchar(reader)?;
    if result == '\n' as i64 || result == '\r' as i64 {
        Ok(0)
    } else {
        Ok(result)
    }
}

fn charset_define<R: Read>(
    n: usize,
    reader: &mut ByteReader<R>,
    state: &mut ControlState,
) -> io::Result<()> {
    skip_ws(reader)?;
    match reader.next()? {
        Some(b'9') => {}
        Some(b) => {
            reader.unget(b);
            skip_to_eol(reader)?;
            return Ok(());
        }
        None => return Ok(()),
    }

    let ch = match reader.next()? {
        Some(b) => b,
        None => return Ok(()),
    };

    if ch == b'6' {
        let cn = charset_name(reader)?;
        state.gn[n] = (65536u64 * cn as u64) as u32 + 0x80;
        state.gndbl[n] = false;
        skip_to_eol(reader)?;
        return Ok(());
    }

    if ch != b'4' {
        skip_to_eol(reader)?;
        return Ok(());
    }

    let ch = match reader.next()? {
        Some(b) => b,
        None => return Ok(()),
    };

    if ch == b'x' {
        match reader.next()? {
            Some(b'9') => {}
            Some(b) => {
                reader.unget(b);
                return Ok(());
            }
            None => return Ok(()),
        }
        match reader.next()? {
            Some(b'4') => {}
            Some(b) => {
                reader.unget(b);
                return Ok(());
            }
            None => return Ok(()),
        }
        skip_ws(reader)?;
        let cn = charset_name(reader)?;
        state.gn[n] = (65536u64 * cn as u64) as u32;
        state.gndbl[n] = true;
        skip_to_eol(reader)?;
        return Ok(());
    }

    reader.unget(ch);
    skip_ws(reader)?;
    let cn = charset_name(reader)?;
    state.gn[n] = (65536u64 * cn as u64) as u32;
    state.gndbl[n] = false;
    Ok(())
}

pub fn read_control<P: AsRef<Path>>(path: P, state: &mut ControlState) -> Result<(), ControlError> {
    let file = File::open(path.as_ref())?;
    let mut reader = ByteReader::new(BufReader::new(file));

    loop {
        let command = match reader.next()? {
            None => break,
            Some(b) => b,
        };

        match command {
            b't' => {
                skip_ws(&mut reader)?;
                let firstch = read_tchar(&mut reader)?;
                let dashcheck = match reader.next()? {
                    Some(b) => b,
                    None => break,
                };
                let lastch = if dashcheck == b'-' {
                    read_tchar(&mut reader)?
                } else {
                    reader.unget(dashcheck);
                    firstch
                };
                skip_ws(&mut reader)?;
                let target = read_tchar(&mut reader)?;
                let offset = target - firstch;
                skip_to_eol(&mut reader)?;
                state.commands.push(ControlCommand {
                    thecommand: 1,
                    rangelo: firstch as u32,
                    rangehi: lastch as u32,
                    offset,
                });
            }
            b'0'..=b'9' | b'-' => {
                reader.unget(command);
                let firstch = read_num(&mut reader)?;
                skip_ws(&mut reader)?;
                let lastch = read_num(&mut reader)?;
                let offset = lastch - firstch;
                skip_to_eol(&mut reader)?;
                state.commands.push(ControlCommand {
                    thecommand: 1,
                    rangelo: firstch as u32,
                    rangehi: firstch as u32,
                    offset,
                });
            }
            b'f' => {
                skip_to_eol(&mut reader)?;
                state.commands.push(ControlCommand {
                    thecommand: 0,
                    rangelo: 0,
                    rangehi: 0,
                    offset: 0,
                });
            }
            b'b' => {
                state.multibyte = 1;
            }
            b'u' => {
                state.multibyte = 2;
            }
            b'h' => {
                state.multibyte = 3;
            }
            b'j' => {
                state.multibyte = 4;
            }
            b'g' => {
                state.multibyte = 0;
                skip_ws(&mut reader)?;
                let sub = match reader.next()? {
                    Some(b) => b,
                    None => continue,
                };
                match sub {
                    b'0' => charset_define(0, &mut reader, state)?,
                    b'1' => charset_define(1, &mut reader, state)?,
                    b'2' => charset_define(2, &mut reader, state)?,
                    b'3' => charset_define(3, &mut reader, state)?,
                    b'l' | b'L' => {
                        skip_ws(&mut reader)?;
                        if let Some(d) = reader.next()? {
                            // Validate range: gl/gr index into a [u32;4] array
                            if (b'0'..=b'3').contains(&d) {
                                state.gl = d - b'0';
                            }
                        }
                        skip_to_eol(&mut reader)?;
                    }
                    b'r' | b'R' => {
                        skip_ws(&mut reader)?;
                        if let Some(d) = reader.next()? {
                            if (b'0'..=b'3').contains(&d) {
                                state.gr = d - b'0';
                            }
                        }
                        skip_to_eol(&mut reader)?;
                    }
                    _ => {
                        skip_to_eol(&mut reader)?;
                    }
                }
            }
            b'\r' | b'\n' => {}
            _ => {
                skip_to_eol(&mut reader)?;
            }
        }
    }

    Ok(())
}

pub fn remap_char(state: &ControlState, c: u32) -> u32 {
    let mut c = c;
    let mut i = 0;
    let commands = &state.commands;
    while i < commands.len() {
        let cmd = &commands[i];
        if cmd.thecommand != 0 && c >= cmd.rangelo && c <= cmd.rangehi {
            c = c.wrapping_add(cmd.offset as u32);
            i += 1;
            while i < commands.len() && commands[i].thecommand != 0 {
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::path::Path;

    const FONTS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../fonts/");

    struct TempFile {
        _dir: tempfile::TempDir,
        path: std::path::PathBuf,
    }

    impl AsRef<Path> for TempFile {
        fn as_ref(&self) -> &Path {
            &self.path
        }
    }

    fn write_temp_file(content: &str) -> TempFile {
        let dir = tempfile::TempDir::new().unwrap();
        let file_path = dir.path().join("test.flc");
        let mut file = std::fs::File::create(&file_path).unwrap();
        write!(file, "{}", content).unwrap();
        TempFile {
            _dir: dir,
            path: file_path,
        }
    }

    fn test_state() -> ControlState {
        ControlState::default()
    }

    #[test]
    fn test_state_defaults() {
        let state = ControlState::default();
        assert!(state.commands.is_empty());
        assert_eq!(state.multibyte, 0);
        assert_eq!(state.gn, [0x00, 0x01, 0, 0]);
        assert_eq!(state.gndbl, [false; 4]);
        assert_eq!(state.gl, 0);
        assert_eq!(state.gr, 1);
    }

    #[test]
    fn test_read_empty_file() {
        let file = write_temp_file("");
        let mut state = test_state();
        read_control(&file, &mut state).unwrap();
        assert!(state.commands.is_empty());
    }

    #[test]
    fn test_read_magic_header_freeze() {
        // C's readcontrol treats 'f' in "flc2a" as a freeze command
        let file = write_temp_file("flc2a\n");
        let mut state = test_state();
        read_control(&file, &mut state).unwrap();
        assert_eq!(state.commands.len(), 1);
        assert_eq!(state.commands[0].thecommand, 0);
    }

    #[test]
    fn test_read_magic_header_freeze_no_newline() {
        let file = write_temp_file("flc2a");
        let mut state = test_state();
        read_control(&file, &mut state).unwrap();
        assert_eq!(state.commands.len(), 1);
        assert_eq!(state.commands[0].thecommand, 0);
    }

    #[test]
    fn test_read_translate_single() {
        let file = write_temp_file("t a b\n");
        let mut state = test_state();
        read_control(&file, &mut state).unwrap();
        assert_eq!(state.commands.len(), 1);
        let cmd = &state.commands[0];
        assert_eq!(cmd.thecommand, 1);
        assert_eq!(cmd.rangelo, b'a' as u32);
        assert_eq!(cmd.rangehi, b'a' as u32);
        assert_eq!(cmd.offset, 1);
    }

    #[test]
    fn test_read_translate_range() {
        let file = write_temp_file("t a-z A-Z\n");
        let mut state = test_state();
        read_control(&file, &mut state).unwrap();
        assert_eq!(state.commands.len(), 1);
        let cmd = &state.commands[0];
        assert_eq!(cmd.thecommand, 1);
        assert_eq!(cmd.rangelo, b'a' as u32);
        assert_eq!(cmd.rangehi, b'z' as u32);
        assert_eq!(cmd.offset, -32i64);
    }

    #[test]
    fn test_read_translate_negative_offset() {
        let file = write_temp_file("t = \\-6\n");
        let mut state = test_state();
        read_control(&file, &mut state).unwrap();
        assert_eq!(state.commands.len(), 1);
        let cmd = &state.commands[0];
        assert_eq!(cmd.thecommand, 1);
        assert_eq!(cmd.rangelo, b'=' as u32);
        assert_eq!(cmd.rangehi, b'=' as u32);
        assert_eq!(cmd.offset, -6 - b'=' as i64);
    }

    #[test]
    fn test_read_freeze() {
        let file = write_temp_file("f\n");
        let mut state = test_state();
        read_control(&file, &mut state).unwrap();
        assert_eq!(state.commands.len(), 1);
        let cmd = &state.commands[0];
        assert_eq!(cmd.thecommand, 0);
    }

    #[test]
    fn test_read_backslash_line_skipped() {
        let file = write_temp_file("\\0x037A \\0x0399\n");
        let mut state = test_state();
        read_control(&file, &mut state).unwrap();
        // starts with \, not 0-9 or -, so skipped by default case
        assert_eq!(state.commands.len(), 0);
    }

    #[test]
    fn test_read_mapping_table_decimal() {
        let file = write_temp_file("65 90\n");
        let mut state = test_state();
        read_control(&file, &mut state).unwrap();
        assert_eq!(state.commands.len(), 1);
        let cmd = &state.commands[0];
        assert_eq!(cmd.thecommand, 1);
        assert_eq!(cmd.rangelo, 65);
        assert_eq!(cmd.rangehi, 65);
        assert_eq!(cmd.offset, 25);
    }

    #[test]
    fn test_read_mapping_table_negative() {
        let file = write_temp_file("65 \\-1\n");
        let mut state = test_state();
        read_control(&file, &mut state).unwrap();
        // starts with '6', so unget, readnum gets 65, skipws, readnum reads \ -> ?
        // Actually, after skipws we read \, but readnum doesn't handle backslash
        // readnum's first char after skip_ws is \, which is not -, not 0, so base=10, unget
        // Then readnum reads \, which is not in hex_digits, so unget and return 0
        // So firstch=65, lastch=0, offset=-65
        assert_eq!(state.commands.len(), 1);
        let cmd = &state.commands[0];
        assert_eq!(cmd.rangelo, 65);
        assert_eq!(cmd.rangehi, 65);
    }

    #[test]
    fn test_read_comment() {
        let file = write_temp_file("# comment line\n");
        let mut state = test_state();
        read_control(&file, &mut state).unwrap();
        assert!(state.commands.is_empty());
    }

    #[test]
    fn test_read_blank_line() {
        let file = write_temp_file("\n");
        let mut state = test_state();
        read_control(&file, &mut state).unwrap();
        assert!(state.commands.is_empty());
    }

    #[test]
    fn test_read_blank_line_crlf() {
        let file = write_temp_file("\r\n");
        let mut state = test_state();
        read_control(&file, &mut state).unwrap();
        assert!(state.commands.is_empty());
    }

    #[test]
    fn test_read_multibyte_commands() {
        let mut state;

        state = test_state();
        read_control(write_temp_file("b\n"), &mut state).unwrap();
        assert_eq!(state.multibyte, 1);

        state = test_state();
        read_control(write_temp_file("u\n"), &mut state).unwrap();
        assert_eq!(state.multibyte, 2);

        state = test_state();
        read_control(write_temp_file("h\n"), &mut state).unwrap();
        assert_eq!(state.multibyte, 3);

        state = test_state();
        read_control(write_temp_file("j\n"), &mut state).unwrap();
        assert_eq!(state.multibyte, 4);
    }

    #[test]
    fn test_read_utf8_control_file() {
        let path = [FONTS_DIR, "utf8.flc"].concat();
        let mut state = test_state();
        read_control(&path, &mut state).unwrap();
        assert_eq!(state.multibyte, 2);
        // 'f' from "flc2a" adds 1 freeze command
        assert_eq!(state.commands.len(), 1);
    }

    #[test]
    fn test_read_hz_control_file() {
        let path = [FONTS_DIR, "hz.flc"].concat();
        let mut state = test_state();
        read_control(&path, &mut state).unwrap();
        assert_eq!(state.multibyte, 3);
        assert_eq!(state.commands.len(), 1);
    }

    #[test]
    fn test_read_charset_g0_94() {
        let file = write_temp_file("g094 J\n");
        let mut state = test_state();
        read_control(&file, &mut state).unwrap();
        assert_eq!(state.gn[0], 65536 * b'J' as u32);
        assert!(!state.gndbl[0]);
    }

    #[test]
    fn test_read_charset_g1_96() {
        // C readcontrol has NO skipws before charsetname in the 96 path
        // (matching C bug: readTchar reads the space = 32 instead of 'J')
        let file = write_temp_file("g196 J\n");
        let mut state = test_state();
        read_control(&file, &mut state).unwrap();
        assert_eq!(state.gn[1], 65536 * b' ' as u32 + 0x80);
        assert!(!state.gndbl[1]);
    }

    #[test]
    fn test_read_charset_g2_94x_double() {
        let file = write_temp_file("g294x94 J\n");
        let mut state = test_state();
        read_control(&file, &mut state).unwrap();
        assert_eq!(state.gn[2], 65536 * b'J' as u32);
        assert!(state.gndbl[2]);
    }

    #[test]
    fn test_read_gl_gr_selection() {
        let file = write_temp_file("g l 2\ng r 0\n");
        let mut state = test_state();
        read_control(&file, &mut state).unwrap();
        assert_eq!(state.gl, 2);
        assert_eq!(state.gr, 0);
    }

    #[test]
    fn test_read_unknown_command_skipped() {
        let file = write_temp_file("x\n");
        let mut state = test_state();
        read_control(&file, &mut state).unwrap();
        assert!(state.commands.is_empty());
    }

    #[test]
    fn test_read_escape_sequences() {
        let pairs = [
            (b'a', 7u8),
            (b'b', 8),
            (b'e', 27),
            (b'f', 12),
            (b'n', 10),
            (b'r', 13),
            (b't', 9),
            (b'v', 11),
        ];
        for &(esc, expected) in &pairs {
            let content = format!("t \\{} a\n", esc as char);
            let file = write_temp_file(&content);
            let mut state = test_state();
            read_control(&file, &mut state).unwrap();
            assert_eq!(state.commands.len(), 1, "escape \\{}", esc as char);
            let cmd = &state.commands[0];
            assert_eq!(cmd.rangelo, expected as u32, "escape \\{}", esc as char);
            assert_eq!(cmd.rangehi, expected as u32, "escape \\{}", esc as char);
        }
    }

    #[test]
    fn test_read_octal_escape() {
        // C readnum treats \377 as decimal 377, not octal 255
        let file = write_temp_file("t \\377 a\n");
        let mut state = test_state();
        read_control(&file, &mut state).unwrap();
        assert_eq!(state.commands.len(), 1);
        let cmd = &state.commands[0];
        assert_eq!(cmd.rangelo, 377);
    }

    #[test]
    fn test_read_hex_escape() {
        let file = write_temp_file("t \\0x1B a\n");
        let mut state = test_state();
        read_control(&file, &mut state).unwrap();
        assert_eq!(state.commands.len(), 1);
        let cmd = &state.commands[0];
        assert_eq!(cmd.rangelo, 27);
    }

    #[test]
    fn test_read_error_nonexistent_file() {
        let mut state = test_state();
        let result = read_control("/nonexistent/path.flc", &mut state);
        assert!(result.is_err());
        match result.unwrap_err() {
            ControlError::IoError(_) => {}
            _ => panic!("Expected IoError"),
        }
    }

    #[test]
    fn test_read_real_world_fixtures() {
        let fixtures = vec![
            [FONTS_DIR, "moscow.flc"].concat(),
            [FONTS_DIR, "koi8r.flc"].concat(),
            [FONTS_DIR, "646-cn.flc"].concat(),
        ];
        for fixture in &fixtures {
            let mut state = test_state();
            read_control(fixture, &mut state).unwrap();
            assert!(
                !state.commands.is_empty(),
                "{} should have at least one command",
                fixture
            );
        }
    }

    #[test]
    fn test_read_upper_flc_snapshot() {
        let mut state = test_state();
        read_control([FONTS_DIR, "upper.flc"].concat(), &mut state).unwrap();
        // upper.flc has 71 't' command lines + 1 freeze from 'f' in "flc2a"
        assert_eq!(state.commands.len(), 72);
    }

    #[test]
    fn test_read_8859_5_flc() {
        let mut state = test_state();
        read_control([FONTS_DIR, "8859-5.flc"].concat(), &mut state).unwrap();
        assert!(!state.commands.is_empty());
        let cmd = &state.commands[0];
        assert_eq!(cmd.thecommand, 1);
        // Lines in 8859-5.flc start with hex like "0x00\t0x0000"
        // These are mapping table entries: firstch reads 0 (then x00 = 0)
        // Actually the lines have tabs, but the first char is '0' (from 0x00)
        // Our parser handles this
    }

    #[test]
    fn test_read_jis0201_flc() {
        let mut state = test_state();
        read_control([FONTS_DIR, "jis0201.flc"].concat(), &mut state).unwrap();
        assert!(!state.commands.is_empty());
        assert_eq!(state.gl, 0);
        assert_eq!(state.gr, 1);
    }

    #[test]
    fn test_read_frango_flc() {
        let mut state = test_state();
        read_control([FONTS_DIR, "frango.flc"].concat(), &mut state).unwrap();
        assert!(!state.commands.is_empty());
    }

    #[test]
    fn test_freeze_then_translate() {
        let file = write_temp_file("f\nt a b\n");
        let mut state = test_state();
        read_control(&file, &mut state).unwrap();
        assert_eq!(state.commands.len(), 2);
        assert_eq!(state.commands[0].thecommand, 0);
        assert_eq!(state.commands[1].thecommand, 1);
    }

    #[test]
    fn test_multiple_translate_commands() {
        let file = write_temp_file("t a b\nt c d\n");
        let mut state = test_state();
        read_control(&file, &mut state).unwrap();
        assert_eq!(state.commands.len(), 2);
        assert_eq!(state.commands[0].offset, 1);
        assert_eq!(state.commands[1].offset, 1);
    }

    #[test]
    fn test_mapping_table_with_same_value() {
        let file = write_temp_file("65 65\n");
        let mut state = test_state();
        read_control(&file, &mut state).unwrap();
        assert_eq!(state.commands.len(), 1);
        assert_eq!(state.commands[0].offset, 0);
        assert_eq!(state.commands[0].rangelo, 65);
    }

    #[test]
    fn test_comment_ignored() {
        let file = write_temp_file("# This is a comment\n");
        let mut state = test_state();
        read_control(&file, &mut state).unwrap();
        assert!(state.commands.is_empty());
    }

    #[test]
    fn test_only_whitespace_lines() {
        let file = write_temp_file("  \n\t\n");
        let mut state = test_state();
        read_control(&file, &mut state).unwrap();
        assert!(state.commands.is_empty());
    }

    #[test]
    fn test_read_jis0201_flc_g_commands() {
        let mut state = test_state();
        read_control([FONTS_DIR, "jis0201.flc"].concat(), &mut state).unwrap();
        assert_eq!(state.gn[0], 65536 * b'J' as u32);
        assert_eq!(state.gn[1], 65536 * b'I' as u32);
        assert!(!state.gndbl[0]);
        assert!(!state.gndbl[1]);
    }

    #[test]
    fn test_read_mapping_table_hex_entry() {
        let file = write_temp_file("0x4A0020 0x20\n");
        let mut state = test_state();
        read_control(&file, &mut state).unwrap();
        assert_eq!(state.commands.len(), 1);
        let cmd = &state.commands[0];
        assert_eq!(cmd.rangelo, 0x4A0020);
        assert_eq!(cmd.rangehi, 0x4A0020);
        assert_eq!(cmd.offset, 0x20i64 - 0x4A0020i64);
    }

    #[test]
    fn test_translate_with_carriage_return() {
        let file = write_temp_file("t a b\r\n");
        let mut state = test_state();
        read_control(&file, &mut state).unwrap();
        assert_eq!(state.commands.len(), 1);
        assert_eq!(state.commands[0].rangelo, b'a' as u32);
        assert_eq!(state.commands[0].offset, 1);
    }

    fn build_remap_state(commands: &[u8]) -> ControlState {
        let content = String::from_utf8(commands.to_vec()).unwrap();
        let file = write_temp_file(&content);
        let mut state = test_state();
        read_control(&file, &mut state).unwrap();
        state
    }

    #[test]
    fn test_remap_empty_commands() {
        let state = test_state();
        assert_eq!(remap_char(&state, b'a' as u32), b'a' as u32);
        assert_eq!(remap_char(&state, b'z' as u32), b'z' as u32);
    }

    #[test]
    fn test_remap_single_char() {
        let state = build_remap_state(b"t a b\n");
        assert_eq!(remap_char(&state, b'a' as u32), b'b' as u32);
        assert_eq!(remap_char(&state, b'c' as u32), b'c' as u32);
    }

    #[test]
    fn test_remap_range() {
        let state = build_remap_state(b"t a-z A-Z\n");
        assert_eq!(remap_char(&state, b'm' as u32), b'M' as u32);
        assert_eq!(remap_char(&state, b'z' as u32), b'Z' as u32);
        assert_eq!(remap_char(&state, b'A' as u32), b'A' as u32);
    }

    #[test]
    fn test_remap_no_match() {
        let state = build_remap_state(b"t x y\n");
        assert_eq!(remap_char(&state, b'a' as u32), b'a' as u32);
    }

    #[test]
    fn test_remap_negative_offset() {
        let state = build_remap_state(b"t b a\n");
        assert_eq!(remap_char(&state, b'b' as u32), b'a' as u32);
        assert_eq!(remap_char(&state, b'a' as u32), b'a' as u32);
    }

    #[test]
    fn test_remap_out_of_range() {
        let state = build_remap_state(b"t a-z A-Z\n");
        assert_eq!(remap_char(&state, b'0' as u32), b'0' as u32);
    }

    #[test]
    fn test_remap_freeze_prevents_second_match_in_block() {
        let state = build_remap_state(b"f\nt a b\nt a c\n");
        // Both t a b and t a c are in same block (after freeze)
        // First match applies: 'a' -> 'b' (offset 1)
        assert_eq!(remap_char(&state, b'a' as u32), b'b' as u32);
    }

    #[test]
    fn test_remap_two_blocks_sequential_apply() {
        let state = build_remap_state(b"t a b\nf\nt b c\n");
        // Block 1: 'a' -> 'b'
        // Block 2: 'b' -> 'c'
        // 'a' -> block1: 'b', block2: 'b' in range 'b'-'c' -> 'c'
        assert_eq!(remap_char(&state, b'a' as u32), b'c' as u32);
        // 'b' -> block1: 'b' not in range 'a'-'a', block2: 'b' -> 'c'
        assert_eq!(remap_char(&state, b'b' as u32), b'c' as u32);
    }

    #[test]
    fn test_remap_three_blocks_chained() {
        let state = build_remap_state(b"t a b\nf\nt b c\nf\nt c d\n");
        // 'a' -> block1: 'b', block2: 'c', block3: 'd'
        assert_eq!(remap_char(&state, b'a' as u32), b'd' as u32);
        // 'b' -> block1: no match, block2: 'c', block3: 'd'
        assert_eq!(remap_char(&state, b'b' as u32), b'd' as u32);
        // 'c' -> block1: no match, block2: no match, block3: 'd'
        assert_eq!(remap_char(&state, b'c' as u32), b'd' as u32);
        assert_eq!(remap_char(&state, b'd' as u32), b'd' as u32);
    }

    #[test]
    fn test_remap_mapping_table_entry() {
        let state = build_remap_state(b"65 90\n");
        assert_eq!(remap_char(&state, 65), 90);
        assert_eq!(remap_char(&state, 66), 66);
    }

    #[test]
    fn test_remap_upper_flc_maps_a_to_a_upper() {
        let mut state = test_state();
        read_control([FONTS_DIR, "upper.flc"].concat(), &mut state).unwrap();
        assert_eq!(remap_char(&state, b'a' as u32), b'A' as u32);
    }

    #[test]
    fn test_remap_upper_flc_maps_z_to_z_upper() {
        let mut state = test_state();
        read_control([FONTS_DIR, "upper.flc"].concat(), &mut state).unwrap();
        assert_eq!(remap_char(&state, b'z' as u32), b'Z' as u32);
    }

    struct MockReader {
        data: Vec<u8>,
        pos: usize,
        buf: Option<u32>,
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

    impl MockReader {
        fn new(data: &[u8]) -> Self {
            Self {
                data: data.to_vec(),
                pos: 0,
                buf: None,
            }
        }
    }

    #[test]
    fn test_iso2022_so_si_gl_switch() {
        let mut state = test_state();
        let mut input = MockReader::new(&[b'A', 0x0E, b'B', 0x0F, b'C']);
        assert_eq!(state.iso2022(&mut input), Some(0x41));
        assert_eq!(state.iso2022(&mut input), Some(0x43));
        assert_eq!(state.iso2022(&mut input), Some(0x43));
        assert_eq!(state.iso2022(&mut input), None);
    }

    #[test]
    fn test_iso2022_esc_designate_94_set() {
        let mut state = test_state();
        let mut input = MockReader::new(&[0x1B, b'(', b'J', b'A']);
        assert_eq!(state.iso2022(&mut input), Some(0x4A0041));
        assert_eq!(state.iso2022(&mut input), None);
    }

    #[test]
    fn test_iso2022_esc_designate_96_set() {
        let mut state = test_state();
        let mut input = MockReader::new(&[0x1B, b'-', b'A', 0xA0]);
        assert_eq!(state.iso2022(&mut input), Some(0xA0));
        assert_eq!(state.iso2022(&mut input), None);
    }

    #[test]
    fn test_iso2022_esc_dollar_double_byte() {
        let mut state = test_state();
        let mut input = MockReader::new(&[0x1B, b'$', b'(', b'B', 0x21, 0x41]);
        assert_eq!(state.iso2022(&mut input), Some(0x422141));
        assert_eq!(state.iso2022(&mut input), None);
    }

    #[test]
    fn test_iso2022_ss2_single_invocation() {
        let mut state = test_state();
        let mut input = MockReader::new(&[0x8E, b'A']);
        assert_eq!(state.iso2022(&mut input), Some(0x41));
        assert_eq!(state.iso2022(&mut input), None);
    }

    #[test]
    fn test_iso2022_esc_n_ls2_permanent() {
        let mut state = test_state();
        let mut input = MockReader::new(&[0x1B, b'n', b'A']);
        assert_eq!(state.iso2022(&mut input), Some(0x41));
        assert_eq!(state.iso2022(&mut input), None);
    }

    #[test]
    fn test_iso2022_gr_invocation() {
        let mut state = test_state();
        let mut input = MockReader::new(&[0x1B, b'}', 0xA0]);
        assert_eq!(state.iso2022(&mut input), Some(0x20));
        assert_eq!(state.iso2022(&mut input), None);
    }

    #[test]
    fn test_iso2022_esc_b_ascii_reset() {
        let mut state = test_state();
        let mut input = MockReader::new(&[0x1B, b'(', b'J', 0x1B, b'(', b'B', b'A']);
        assert_eq!(state.iso2022(&mut input), Some(0x41));
        assert_eq!(state.iso2022(&mut input), None);
    }

    #[test]
    fn test_iso2022_eof_during_escape() {
        let mut state = test_state();
        let mut input = MockReader::new(&[0x1B]);
        assert_eq!(state.iso2022(&mut input), None);
    }

    #[test]
    fn test_iso2022_eof_during_double_byte_second_byte() {
        let mut state = test_state();
        state.gndbl[0] = true;
        let mut input = MockReader::new(&[0x21]);
        assert_eq!(state.iso2022(&mut input), Some(0x2100));
        assert_eq!(state.iso2022(&mut input), None);
    }

    #[test]
    fn test_iso2022_esc_dollar_lparen_double_byte_no_special_b() {
        let mut state = test_state();
        let mut input = MockReader::new(&[0x1B, b'$', b'(', b'J', 0x21, 0x41]);
        assert_eq!(state.iso2022(&mut input), Some(0x4A2141));
        assert_eq!(state.iso2022(&mut input), None);
    }

    #[test]
    fn test_iso2022_esc_b_remap_via_gn() {
        let mut state = test_state();
        let mut input = MockReader::new(&[0x1B, b'(', b'J', b'A']);
        assert_eq!(state.iso2022(&mut input), Some(0x4A0041));
        // After the call, gn[0] should still be 0x4A0000
        // (only changed by another escape sequence)
        assert_eq!(state.gn[0], 0x4A0000);
    }

    #[test]
    fn test_iso2022_ss2_restores_gl_gr() {
        let mut state = test_state();
        state.gl = 1;
        state.gr = 2;
        let save_gl = state.gl;
        let save_gr = state.gr;
        let mut input = MockReader::new(&[0x8E, b'A']);
        assert_eq!(state.iso2022(&mut input), Some(0x41));
        assert_eq!(state.gl, save_gl);
        assert_eq!(state.gr, save_gr);
    }

    #[test]
    fn test_iso2022_plain_char_passthrough() {
        let mut state = test_state();
        let mut input = MockReader::new(b"Hi");
        assert_eq!(state.iso2022(&mut input), Some(0x48));
        assert_eq!(state.iso2022(&mut input), Some(0x69));
        assert_eq!(state.iso2022(&mut input), None);
    }

    #[test]
    fn test_iso2022_newline_passthrough() {
        let mut state = test_state();
        let mut input = MockReader::new(b"\n");
        assert_eq!(state.iso2022(&mut input), Some(0x0A));
        assert_eq!(state.iso2022(&mut input), None);
    }

    #[test]
    fn test_iso2022_space_passthrough() {
        let mut state = test_state();
        let mut input = MockReader::new(b" ");
        assert_eq!(state.iso2022(&mut input), Some(0x20));
        assert_eq!(state.iso2022(&mut input), None);
    }

    use super::*;

    /// F-16 (GPT review): a long run of control bytes must not overflow
    /// the stack. 1M shift-in/shift-out pairs previously meant 1M stack
    /// frames; now it's a flat loop.
    #[test]
    fn test_iso2022_long_control_run_no_overflow() {
        let mut data = Vec::new();
        for _ in 0..500_000 {
            data.push(0x0E); // shift in
            data.push(0x0F); // shift out
        }
        data.push(b'A');
        let mut reader = MockReader::new(&data);
        let mut state = ControlState::default();
        let ch = state.iso2022(&mut reader).expect("should emit final char");
        assert_eq!(ch, b'A' as u32);
    }

    /// Nested SS2 saves must unwind correctly (outermost restore wins).
    #[test]
    fn test_iso2022_ss2_nesting() {
        // SS2 SS2 'A' → both saves pushed; gl/gr restored to pre-first-SS2.
        let mut reader = MockReader::new(&[0x8E, 0x8E, b'A']);
        let mut state = ControlState::default();
        let ch = state.iso2022(&mut reader).expect("should emit char");
        assert_eq!(state.gl, 0); // default gl restored
        assert_eq!(ch, b'A' as u32);
    }

    /// Deeply nested single-shift sequences still terminate and restore.
    #[test]
    fn test_iso2022_deep_ss2_run() {
        let mut data = vec![0x8E; 100_000];
        data.push(b'B');
        let mut reader = MockReader::new(&data);
        let mut state = ControlState::default();
        let ch = state.iso2022(&mut reader).expect("should emit char");
        assert_eq!(ch, b'B' as u32);
        assert_eq!(state.gl, 0);
    }
}
