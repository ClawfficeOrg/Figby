//! Syntax colouring for the tachyonfx DSL in the FX script editor.
//!
//! A small, forgiving lexer: it never fails, it only sorts chars into
//! token classes for [`TextArea`](crate::tui::components::multiline::TextArea),
//! which stays language-agnostic. Positions are chars, per line, like the
//! widget's. Block comments and strings may span lines; they are split into
//! one span per line.

use ratatui::style::{Modifier, Style};

use super::graph::FxInterp;
use crate::tui::components::multiline::{TokenSpan, TOKEN_CLASSES};
use crate::tui::theme::Theme;

/// Token classes (indices into `TextAreaStyle::tokens`).
pub const IDENT: usize = 0;
/// A name right after `.`: a method such as `with_area`.
pub const METHOD: usize = 1;
pub const NUMBER: usize = 2;
pub const STRING: usize = 3;
/// `// ...` and `/* ... */`.
pub const COMMENT: usize = 4;
pub const PUNCT: usize = 5;
/// The `fx` namespace, the effect name after `fx::`, and `let`.
pub const EFFECT: usize = 6;
/// Known names: interpolations, colours (and `Color`), motions,
/// `true`/`false`.
pub const CONSTANT: usize = 7;

const COLOURS: &[&str] = &[
    "Color",
    "Black",
    "Red",
    "Green",
    "Yellow",
    "Blue",
    "Magenta",
    "Cyan",
    "Gray",
    "DarkGray",
    "LightRed",
    "LightGreen",
    "LightYellow",
    "LightBlue",
    "LightMagenta",
    "LightCyan",
    "White",
    "Reset",
    "Rgb",
    "Indexed",
];

const MOTIONS: &[&str] = &["LeftToRight", "RightToLeft", "UpToDown", "DownToUp"];

fn is_constant(name: &str) -> bool {
    matches!(name, "true" | "false")
        || COLOURS.contains(&name)
        || MOTIONS.contains(&name)
        || FxInterp::ALL.iter().any(|i| i.dsl_name() == name)
}

struct Lexer {
    chars: Vec<char>,
    i: usize,
    row: usize,
    col: usize,
    out: Vec<TokenSpan>,
}

impl Lexer {
    fn peek(&self, k: usize) -> Option<char> {
        self.chars.get(self.i + k).copied()
    }

    fn bump(&mut self) {
        if self.chars.get(self.i) == Some(&'\n') {
            self.row += 1;
            self.col = 0;
        } else {
            self.col += 1;
        }
        self.i += 1;
    }

    /// Emit `class` from `(row, col)` to the current position, one span
    /// per line.
    fn emit(&mut self, (row, col): (usize, usize), class: usize) {
        if row == self.row {
            if self.col > col {
                self.out.push(TokenSpan {
                    row,
                    start: col,
                    end: self.col,
                    class,
                });
            }
            return;
        }
        // Multi-line: the first line to its end, whole middle lines, then
        // the last line up to the current column.
        let mut line_ends = vec![];
        let mut c = 0;
        let mut r = 0;
        for ch in &self.chars[..self.i] {
            if *ch == '\n' {
                line_ends.push((r, c));
                r += 1;
                c = 0;
            } else {
                c += 1;
            }
        }
        for &(r, len) in line_ends.iter().filter(|(r, _)| *r >= row) {
            let start = if r == row { col } else { 0 };
            if len > start {
                self.out.push(TokenSpan {
                    row: r,
                    start,
                    end: len,
                    class,
                });
            }
        }
        if self.col > 0 {
            self.out.push(TokenSpan {
                row: self.row,
                start: 0,
                end: self.col,
                class,
            });
        }
    }

    fn run(mut self) -> Vec<TokenSpan> {
        // The last significant token, for `.method` and `fx::name`.
        let mut prev_dot = false;
        let mut after_path: Option<usize> = None;
        while let Some(c) = self.peek(0) {
            let start = (self.row, self.col);
            if c.is_whitespace() {
                self.bump();
                continue;
            }
            if c == '/' && self.peek(1) == Some('/') {
                while self.peek(0).is_some_and(|c| c != '\n') {
                    self.bump();
                }
                self.emit(start, COMMENT);
                continue;
            }
            if c == '/' && self.peek(1) == Some('*') {
                self.bump();
                self.bump();
                while let Some(c) = self.peek(0) {
                    if c == '*' && self.peek(1) == Some('/') {
                        self.bump();
                        self.bump();
                        break;
                    }
                    self.bump();
                }
                self.emit(start, COMMENT);
                continue;
            }
            if c == '"' {
                self.bump();
                while let Some(c) = self.peek(0) {
                    self.bump();
                    if c == '\\' {
                        self.bump();
                    } else if c == '"' {
                        break;
                    }
                }
                self.emit(start, STRING);
                prev_dot = false;
                after_path = None;
                continue;
            }
            if c.is_ascii_digit() {
                while self
                    .peek(0)
                    .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
                {
                    // `1.with_x` is not a float: stop at a dot before a
                    // letter.
                    if self.peek(0) == Some('.')
                        && !self.peek(1).is_some_and(|c| c.is_ascii_digit())
                    {
                        break;
                    }
                    self.bump();
                }
                self.emit(start, NUMBER);
                prev_dot = false;
                after_path = None;
                continue;
            }
            if c.is_alphabetic() || c == '_' {
                let from = self.i;
                while self
                    .peek(0)
                    .is_some_and(|c| c.is_alphanumeric() || c == '_')
                {
                    self.bump();
                }
                let name: String = self.chars[from..self.i].iter().collect();
                let path_next = self.peek(0) == Some(':') && self.peek(1) == Some(':');
                let class = if let Some(class) = after_path {
                    class
                } else if prev_dot {
                    METHOD
                } else if name == "fx" && path_next || name == "let" {
                    EFFECT
                } else if is_constant(&name) {
                    CONSTANT
                } else {
                    IDENT
                };
                self.emit(start, class);
                // `fx::name` and `Color::Name` colour the name after `::`
                // like the path.
                after_path = path_next.then_some(class).filter(|c| *c != IDENT);
                if path_next {
                    self.bump();
                    self.bump();
                    self.emit((start.0, self.col - 2), PUNCT);
                }
                prev_dot = false;
                continue;
            }
            self.bump();
            self.emit(start, PUNCT);
            prev_dot = c == '.';
            after_path = None;
        }
        self.out
    }
}

/// Token spans for `src`, ordered, per line, in chars.
pub fn tokenize(src: &str) -> Vec<TokenSpan> {
    Lexer {
        chars: src.chars().collect(),
        i: 0,
        row: 0,
        col: 0,
        out: Vec::new(),
    }
    .run()
}

/// Theme colours per token class, for `TextAreaStyle::tokens`.
pub fn token_styles(theme: &Theme) -> [Style; TOKEN_CLASSES] {
    let g = &theme.general;
    let mut s = [Style::default(); TOKEN_CLASSES];
    s[IDENT] = Style::default();
    s[METHOD] = Style::default().fg(g.primary);
    s[NUMBER] = Style::default().fg(g.warning);
    s[STRING] = Style::default().fg(g.success);
    s[COMMENT] = Style::default()
        .fg(g.secondary)
        .add_modifier(Modifier::ITALIC);
    s[PUNCT] = Style::default().fg(g.secondary);
    s[EFFECT] = Style::default().fg(g.primary).add_modifier(Modifier::BOLD);
    s[CONSTANT] = Style::default().fg(g.warning).add_modifier(Modifier::BOLD);
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `(text, class)` pairs for one-line `src`.
    fn lexed(src: &str) -> Vec<(String, usize)> {
        let chars: Vec<char> = src.chars().collect();
        tokenize(src)
            .into_iter()
            .map(|t| (chars[t.start..t.end].iter().collect(), t.class))
            .collect()
    }

    fn pairs(v: &[(&str, usize)]) -> Vec<(String, usize)> {
        v.iter().map(|(s, c)| (s.to_string(), *c)).collect()
    }

    #[test]
    fn effects_methods_numbers_and_constants() {
        assert_eq!(
            lexed("fx::dissolve((300, QuadOut)).with_area(canvas)"),
            pairs(&[
                ("fx", EFFECT),
                ("::", PUNCT),
                ("dissolve", EFFECT),
                ("(", PUNCT),
                ("(", PUNCT),
                ("300", NUMBER),
                (",", PUNCT),
                ("QuadOut", CONSTANT),
                (")", PUNCT),
                (")", PUNCT),
                (".", PUNCT),
                ("with_area", METHOD),
                ("(", PUNCT),
                ("canvas", IDENT),
                (")", PUNCT),
            ])
        );
    }

    #[test]
    fn colour_paths_lets_floats_and_strings() {
        assert_eq!(
            lexed("let c = Color::LightRed; [40.5, 0.0] \"a\\\"b\" true"),
            pairs(&[
                ("let", EFFECT),
                ("c", IDENT),
                ("=", PUNCT),
                ("Color", CONSTANT),
                ("::", PUNCT),
                ("LightRed", CONSTANT),
                (";", PUNCT),
                ("[", PUNCT),
                ("40.5", NUMBER),
                (",", PUNCT),
                ("0.0", NUMBER),
                ("]", PUNCT),
                ("\"a\\\"b\"", STRING),
                ("true", CONSTANT),
            ])
        );
        // A user path (not fx/Color) colours nothing special.
        assert_eq!(lexed("my::x")[2], ("x".to_string(), IDENT));
        // `1.max` is a number then a method.
        assert_eq!(
            lexed("1.max"),
            pairs(&[("1", NUMBER), (".", PUNCT), ("max", METHOD)])
        );
    }

    #[test]
    fn comments_and_multi_line_spans() {
        assert_eq!(
            lexed("fx::glitch(1) // é note"),
            pairs(&[
                ("fx", EFFECT),
                ("::", PUNCT),
                ("glitch", EFFECT),
                ("(", PUNCT),
                ("1", NUMBER),
                (")", PUNCT),
                ("// é note", COMMENT),
            ])
        );
        let spans = tokenize("a /* one\ntwo\nthree */ b");
        let comments: Vec<_> = spans
            .iter()
            .filter(|t| t.class == COMMENT)
            .map(|t| (t.row, t.start, t.end))
            .collect();
        assert_eq!(comments, [(0, 2, 8), (1, 0, 3), (2, 0, 8)]);
        let b = spans.last().unwrap();
        assert_eq!((b.row, b.start, b.end, b.class), (2, 9, 10, IDENT));
        // Unterminated comment and string run to the end without panicking.
        assert_eq!(tokenize("/* x").len(), 1);
        assert_eq!(tokenize("\"x\ny").len(), 2);
    }

    #[test]
    fn every_interpolation_is_a_constant() {
        for i in FxInterp::ALL {
            assert_eq!(lexed(i.dsl_name()), pairs(&[(i.dsl_name(), CONSTANT)]));
        }
    }
}
