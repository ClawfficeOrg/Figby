//! `.figfx.toml` effect scripts: several named effects, each with a trigger
//! and a slot (docs/fx-scripting-plan.md, section 3 and phase 2).
//!
//! ```toml
//! [[effect]]
//! name    = "intro"
//! trigger = "on_open"     # manual | on_open | on_save | loop | on_key:F8
//! slot    = "intro"       # optional, defaults to the name
//! source  = '''
//! fx::sequence(&[fx::coalesce((500, QuadOut)), fx::sleep(300), fx::glitch(200)])
//!     .with_area(canvas)
//! '''
//! ```
//!
//! Parsing works on `&str` only; [`super::script_file`] is the one part that
//! touches the filesystem. Every `source` is test-compiled at load so a bad
//! script is reported once, at the line and column in the file, rather than
//! each time a trigger fires.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::Deserialize;

use super::compile::{compile, FxTargets};
use super::runtime::FxSlot;

/// The file extension scripts use.
pub const SCRIPT_EXTENSION: &str = ".figfx.toml";

/// Whether `path` names an effect script (case-insensitive).
pub fn is_script_path(path: &std::path::Path) -> bool {
    path.to_str()
        .is_some_and(|s| s.to_ascii_lowercase().ends_with(SCRIPT_EXTENSION))
}

/// When an effect starts by itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FxTrigger {
    /// Only from the Effects tab (or another explicit play).
    Manual,
    /// After a document (font, figmap, figseq) opens.
    OnOpen,
    /// After the document saves.
    OnSave,
    /// When this key is pressed. The key is consumed.
    OnKey(FxKey),
    /// From load on, restarted each time it ends, until stopped.
    Loop,
}

impl FxTrigger {
    /// Parse `manual`, `on_open`, `on_save`, `loop` or `on_key:<key>`.
    pub fn parse(s: &str) -> Result<Self, String> {
        let s = s.trim();
        match s {
            "manual" => return Ok(Self::Manual),
            "on_open" => return Ok(Self::OnOpen),
            "on_save" => return Ok(Self::OnSave),
            "loop" => return Ok(Self::Loop),
            _ => {}
        }
        if let Some(key) = s.strip_prefix("on_key:") {
            return FxKey::parse(key.trim().trim_matches('"')).map(Self::OnKey);
        }
        Err(format!(
            "unknown trigger `{s}` (expected manual, on_open, on_save, loop or on_key:<key>)"
        ))
    }

    /// The trigger as written in a script.
    pub fn label(&self) -> String {
        match self {
            Self::Manual => "manual".into(),
            Self::OnOpen => "on_open".into(),
            Self::OnSave => "on_save".into(),
            Self::Loop => "loop".into(),
            Self::OnKey(k) => format!("on_key:{k}"),
        }
    }
}

/// A key chord for `on_key`: `F8`, `x`, `ctrl+e`, `alt+shift+Up`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FxKey {
    pub code: KeyCode,
    /// Ctrl and Alt always count; Shift counts only for non-character keys
    /// (a character already carries its case).
    pub modifiers: KeyModifiers,
}

impl FxKey {
    pub fn parse(s: &str) -> Result<Self, String> {
        let mut modifiers = KeyModifiers::NONE;
        let mut rest = s;
        loop {
            let lower = rest.to_ascii_lowercase();
            let (m, len) = if lower.starts_with("ctrl+") {
                (KeyModifiers::CONTROL, 5)
            } else if lower.starts_with("alt+") {
                (KeyModifiers::ALT, 4)
            } else if lower.starts_with("shift+") {
                (KeyModifiers::SHIFT, 6)
            } else {
                break;
            };
            modifiers |= m;
            rest = &rest[len..];
        }
        let lower = rest.to_ascii_lowercase();
        let code = match lower.as_str() {
            "enter" => KeyCode::Enter,
            "tab" => KeyCode::Tab,
            "esc" | "escape" => KeyCode::Esc,
            "space" => KeyCode::Char(' '),
            "backspace" => KeyCode::Backspace,
            "delete" | "del" => KeyCode::Delete,
            "insert" | "ins" => KeyCode::Insert,
            "home" => KeyCode::Home,
            "end" => KeyCode::End,
            "pageup" => KeyCode::PageUp,
            "pagedown" => KeyCode::PageDown,
            "up" => KeyCode::Up,
            "down" => KeyCode::Down,
            "left" => KeyCode::Left,
            "right" => KeyCode::Right,
            f if f.len() > 1 && f.starts_with('f') && f[1..].parse::<u8>().is_ok() => {
                let n: u8 = f[1..].parse().unwrap_or(0);
                if !(1..=24).contains(&n) {
                    return Err(format!("no such function key `{rest}`"));
                }
                KeyCode::F(n)
            }
            _ => {
                let mut chars = rest.chars();
                match (chars.next(), chars.next()) {
                    (Some(c), None) => KeyCode::Char(c),
                    _ => return Err(format!("unknown key `{s}`")),
                }
            }
        };
        if matches!(code, KeyCode::Char(_)) {
            modifiers.remove(KeyModifiers::SHIFT);
        }
        Ok(Self { code, modifiers })
    }

    /// Whether a key press is this chord.
    pub fn matches(&self, key: &KeyEvent) -> bool {
        let relevant = |m: KeyModifiers, code: KeyCode| {
            let mut mask = KeyModifiers::CONTROL | KeyModifiers::ALT;
            if !matches!(code, KeyCode::Char(_)) {
                mask |= KeyModifiers::SHIFT;
            }
            m & mask
        };
        let code_eq = match (self.code, key.code) {
            // A terminal may report Ctrl+E as 'E' or 'e'.
            (KeyCode::Char(a), KeyCode::Char(b))
                if self.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                a.eq_ignore_ascii_case(&b)
            }
            (a, b) => a == b,
        };
        code_eq && relevant(self.modifiers, self.code) == relevant(key.modifiers, key.code)
    }
}

impl std::fmt::Display for FxKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.modifiers.contains(KeyModifiers::CONTROL) {
            f.write_str("ctrl+")?;
        }
        if self.modifiers.contains(KeyModifiers::ALT) {
            f.write_str("alt+")?;
        }
        if self.modifiers.contains(KeyModifiers::SHIFT) {
            f.write_str("shift+")?;
        }
        match self.code {
            KeyCode::Char(' ') => f.write_str("Space"),
            KeyCode::Char(c) => write!(f, "{c}"),
            KeyCode::F(n) => write!(f, "F{n}"),
            other => write!(f, "{other:?}"),
        }
    }
}

/// One named effect of a script.
#[derive(Debug, Clone, PartialEq)]
pub struct FxScriptEffect {
    pub name: String,
    pub trigger: FxTrigger,
    /// Slot name; effects sharing a slot replace each other.
    pub slot: String,
    /// DSL source, compiled each time the effect starts (targets move).
    pub source: String,
}

/// A parsed, validated script.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FxScript {
    pub effects: Vec<FxScriptEffect>,
    /// Distinct slot names, in first-use order; index `i` is
    /// `FxSlot::Script(i)`.
    pub slots: Vec<String>,
}

impl FxScript {
    /// The runtime slot effect `idx` runs in.
    pub fn slot_of(&self, idx: usize) -> FxSlot {
        let name = &self.effects[idx].slot;
        let i = self.slots.iter().position(|s| s == name).unwrap_or(0);
        FxSlot::Script(i as u16)
    }

    /// Indices of the effects whose trigger is `trigger` (for `OnKey`, use
    /// [`Self::for_key`]).
    pub fn with_trigger(&self, trigger: FxTrigger) -> Vec<usize> {
        (0..self.effects.len())
            .filter(|&i| self.effects[i].trigger == trigger)
            .collect()
    }

    /// Indices of the `on_key` effects bound to `key`.
    pub fn for_key(&self, key: &KeyEvent) -> Vec<usize> {
        (0..self.effects.len())
            .filter(|&i| matches!(self.effects[i].trigger, FxTrigger::OnKey(k) if k.matches(key)))
            .collect()
    }

    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.effects.iter().position(|e| e.name == name)
    }
}

/// A load failure, at a 1-based line and column in the script file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FxScriptError {
    /// The effect the error is in, when known.
    pub effect: Option<String>,
    pub line: usize,
    pub column: usize,
    pub message: String,
}

impl std::fmt::Display for FxScriptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}: ", self.line, self.column)?;
        if let Some(name) = &self.effect {
            write!(f, "effect `{name}`: ")?;
        }
        f.write_str(&self.message)
    }
}

impl std::error::Error for FxScriptError {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawScript {
    #[serde(default)]
    effect: Vec<RawEffect>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEffect {
    name: toml::Spanned<String>,
    #[serde(default)]
    trigger: Option<toml::Spanned<String>>,
    #[serde(default)]
    slot: Option<String>,
    source: toml::Spanned<String>,
}

/// 1-based (line, column) of byte `offset` in `text`.
fn line_col(text: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(text.len());
    let before = &text[..offset];
    let line = before.matches('\n').count() + 1;
    let col = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
    (line, col)
}

/// Byte offset in `text` where the content of the string literal spanning
/// `span` starts: after the quotes, and after the newline TOML trims right
/// after an opening `'''` / `"""`.
fn content_start(text: &str, span: &std::ops::Range<usize>) -> usize {
    let lit = &text[span.start.min(text.len())..span.end.min(text.len())];
    let quotes = if lit.starts_with("'''") || lit.starts_with("\"\"\"") {
        3
    } else {
        1
    };
    let after = &lit[quotes.min(lit.len())..];
    let nl = if quotes == 3 && after.starts_with("\r\n") {
        2
    } else if quotes == 3 && after.starts_with('\n') {
        1
    } else {
        0
    };
    span.start + quotes + nl
}

/// Map a DSL error position (1-based, within `source`) to the file. Exact
/// for literal strings (`'...'`, `'''...'''`); approximate if the source
/// uses escapes.
fn map_source_pos(text: &str, start: usize, line: usize, column: usize) -> (usize, usize) {
    let (l0, c0) = line_col(text, start);
    if line <= 1 {
        (l0, c0 + column.saturating_sub(1))
    } else {
        (l0 + line - 1, column)
    }
}

/// Parse and validate a `.figfx.toml` script. Each source is test-compiled
/// with every target bound to an empty rect.
pub fn parse_script(text: &str) -> Result<FxScript, FxScriptError> {
    let raw: RawScript = toml::from_str(text).map_err(|e| {
        let (line, column) = e.span().map_or((1, 1), |s| line_col(text, s.start));
        FxScriptError {
            effect: None,
            line,
            column,
            message: e.message().to_string(),
        }
    })?;
    let mut script = FxScript::default();
    let probe = FxTargets::default();
    for e in raw.effect {
        let name = e.name.get_ref().trim().to_string();
        let at = |offset: usize, message: String| {
            let (line, column) = line_col(text, offset);
            FxScriptError {
                effect: (!name.is_empty()).then(|| name.clone()),
                line,
                column,
                message,
            }
        };
        if name.is_empty() {
            return Err(at(e.name.span().start, "effect name is empty".into()));
        }
        if script.index_of(&name).is_some() {
            return Err(at(
                e.name.span().start,
                format!("duplicate effect name `{name}`"),
            ));
        }
        let trigger = match &e.trigger {
            None => FxTrigger::Manual,
            Some(t) => FxTrigger::parse(t.get_ref()).map_err(|m| at(t.span().start, m))?,
        };
        let source = e.source.get_ref().clone();
        if source.trim().is_empty() {
            return Err(at(e.source.span().start, "source is empty".into()));
        }
        if let Err(err) = compile(&source, &probe) {
            let start = content_start(text, &e.source.span());
            let (line, column) = map_source_pos(text, start, err.line, err.column);
            return Err(FxScriptError {
                effect: Some(name),
                line,
                column,
                message: err.message.lines().next().unwrap_or_default().to_string(),
            });
        }
        let slot = e
            .slot
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| name.clone());
        if !script.slots.contains(&slot) {
            if script.slots.len() >= usize::from(u16::MAX) {
                return Err(at(e.name.span().start, "too many slots".into()));
            }
            script.slots.push(slot.clone());
        }
        script.effects.push(FxScriptEffect {
            name,
            trigger,
            slot,
            source,
        });
    }
    Ok(script)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
# Two effects share the "intro" slot.
[[effect]]
name    = "intro"
trigger = "on_open"
source  = '''
fx::sequence(&[fx::coalesce((500, QuadOut)), fx::sleep(300), fx::glitch(200)])
    .with_area(canvas)
'''

[[effect]]
name    = "saved"
trigger = "on_save"
slot    = "intro"
source  = 'fx::fade_from_fg(Color::Green, (300, Linear))'

[[effect]]
name    = "flash"
trigger = "on_key:ctrl+e"
source  = "fx::parallel(&[fx::dissolve(200), fx::delay(100, fx::coalesce(200))])"

[[effect]]
name    = "spin"
trigger = "loop"
source  = "fx::hsl_shift_fg([30.0, 0.0, 0.0], (1000, Linear)).with_area(status)"

[[effect]]
name   = "poke"
source = "fx::glitch(100)"
"#;

    #[test]
    fn sample_script_loads_with_triggers_and_slots() {
        let s = parse_script(SAMPLE).unwrap();
        let names: Vec<&str> = s.effects.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["intro", "saved", "flash", "spin", "poke"]);
        assert_eq!(s.effects[0].trigger, FxTrigger::OnOpen);
        assert_eq!(s.effects[1].trigger, FxTrigger::OnSave);
        assert_eq!(s.effects[3].trigger, FxTrigger::Loop);
        assert_eq!(s.effects[4].trigger, FxTrigger::Manual, "default trigger");
        assert_eq!(
            s.effects[2].trigger,
            FxTrigger::OnKey(FxKey {
                code: KeyCode::Char('e'),
                modifiers: KeyModifiers::CONTROL
            })
        );
        // `saved` shares `intro`'s slot; the rest default to their name.
        assert_eq!(s.slots, ["intro", "flash", "spin", "poke"]);
        assert_eq!(s.slot_of(0), s.slot_of(1));
        assert_ne!(s.slot_of(0), s.slot_of(2));
        assert_eq!(s.with_trigger(FxTrigger::OnOpen), [0]);
        assert_eq!(s.with_trigger(FxTrigger::Loop), [3]);
        assert!(s.effects[0].source.contains("fx::sequence"));
    }

    #[test]
    fn empty_file_is_an_empty_script() {
        assert_eq!(parse_script("").unwrap(), FxScript::default());
    }

    #[test]
    fn toml_syntax_error_has_file_position() {
        let err = parse_script("[[effect]]\nname = \"a\"\nsource = \n").unwrap_err();
        assert_eq!(err.line, 3, "{err}");
        assert_eq!(err.effect, None);
    }

    #[test]
    fn unknown_field_is_rejected() {
        let err = parse_script("[[effect]]\nname = \"a\"\nsource = \"fx::sleep(1)\"\ncolour = 1\n")
            .unwrap_err();
        assert!(err.message.contains("colour"), "{err}");
    }

    #[test]
    fn missing_source_is_rejected() {
        let err = parse_script("[[effect]]\nname = \"a\"\n").unwrap_err();
        assert!(err.message.contains("source"), "{err}");
    }

    #[test]
    fn duplicate_names_are_rejected() {
        let src = "[[effect]]\nname = \"a\"\nsource = \"fx::sleep(1)\"\n\
                   [[effect]]\nname = \"a\"\nsource = \"fx::sleep(2)\"\n";
        let err = parse_script(src).unwrap_err();
        assert_eq!(err.line, 5);
        assert!(err.message.contains("duplicate"), "{err}");
    }

    #[test]
    fn bad_trigger_is_rejected_at_its_line() {
        let src = "[[effect]]\nname = \"a\"\ntrigger = \"on_click\"\nsource = \"fx::sleep(1)\"\n";
        let err = parse_script(src).unwrap_err();
        assert_eq!((err.line, err.column), (3, 11));
        assert!(err.message.contains("on_click"), "{err}");
        assert_eq!(err.effect.as_deref(), Some("a"));
    }

    #[test]
    fn dsl_error_maps_into_the_file() {
        // Multi-line literal: the error is on the source's 2nd line, which
        // is line 5 of the file.
        let src = "[[effect]]\nname = \"a\"\nsource = '''\nfx::dissolve(300)\n    .with_area(nowhere)\n'''\n";
        let err = parse_script(src).unwrap_err();
        assert_eq!(err.line, 5, "{err}");
        assert!(err.message.contains("nowhere"), "{err}");
        assert_eq!(err.effect.as_deref(), Some("a"));
        // Single-line literal: column offset by the key and the quote.
        let src = "[[effect]]\nname = \"b\"\nsource = 'fx::nope(1)'\n";
        let err = parse_script(src).unwrap_err();
        assert_eq!(err.line, 3);
        assert!(err.column >= 11, "{err}");
    }

    #[test]
    fn sequence_parallel_and_delay_compile() {
        for src in [
            "fx::sequence(&[fx::dissolve(100), fx::sleep(50), fx::coalesce(100)])",
            "fx::parallel(&[fx::dissolve(100), fx::fade_to_fg(Color::Red, (200, QuadOut))])",
            "fx::delay(150, fx::dissolve(100)).with_area(canvas)",
            "fx::sequence(&[fx::delay(100, fx::glitch(100)), fx::parallel(&[fx::dissolve(50), fx::sleep(80)])])",
        ] {
            let text = format!("[[effect]]\nname = \"x\"\nsource = '{src}'\n");
            assert!(parse_script(&text).is_ok(), "{src}: {:?}", parse_script(&text));
        }
    }

    #[test]
    fn key_specs_parse_and_match() {
        let k = |s| FxKey::parse(s).unwrap();
        let ev = |code, m| KeyEvent::new(code, m);
        assert!(k("F8").matches(&ev(KeyCode::F(8), KeyModifiers::NONE)));
        assert!(!k("F8").matches(&ev(KeyCode::F(8), KeyModifiers::SHIFT)));
        assert!(k("shift+F8").matches(&ev(KeyCode::F(8), KeyModifiers::SHIFT)));
        // Characters ignore Shift; their case is the character.
        assert!(k("X").matches(&ev(KeyCode::Char('X'), KeyModifiers::SHIFT)));
        assert!(!k("x").matches(&ev(KeyCode::Char('X'), KeyModifiers::SHIFT)));
        assert!(k("ctrl+e").matches(&ev(KeyCode::Char('e'), KeyModifiers::CONTROL)));
        assert!(k("Ctrl+E").matches(&ev(KeyCode::Char('e'), KeyModifiers::CONTROL)));
        assert!(!k("ctrl+e").matches(&ev(KeyCode::Char('e'), KeyModifiers::NONE)));
        assert!(k("alt+Up").matches(&ev(KeyCode::Up, KeyModifiers::ALT)));
        assert!(k("space").matches(&ev(KeyCode::Char(' '), KeyModifiers::NONE)));
        assert!(FxKey::parse("F99").is_err());
        assert!(FxKey::parse("banana").is_err());
        assert_eq!(k("ctrl+e").to_string(), "ctrl+e");
        assert_eq!(
            FxTrigger::parse("on_key:\"F7\"").unwrap().label(),
            "on_key:F7"
        );
    }

    #[test]
    fn script_paths_are_recognised() {
        assert!(is_script_path(std::path::Path::new("a/b/intro.figfx.toml")));
        assert!(is_script_path(std::path::Path::new("INTRO.FIGFX.TOML")));
        assert!(!is_script_path(std::path::Path::new("config.toml")));
    }
}
