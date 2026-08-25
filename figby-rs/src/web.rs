use ratzilla::event::KeyCode;
use ratzilla::ratatui::layout::{Constraint, Layout};
use ratzilla::ratatui::style::{Color, Modifier, Style};
use ratzilla::ratatui::text::{Line, Span, Text};
use ratzilla::ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratzilla::ratatui::Terminal;
use ratzilla::{DomBackend, WebRenderer};
use std::cell::RefCell;
use std::io;
use std::rc::Rc;

use crate::font::{parse_tlf_font, FIGfont};
use crate::render::render_string;

struct FontEntry {
    name: &'static str,
    font: FIGfont,
}

struct WebApp {
    fonts: Vec<FontEntry>,
    selected: usize,
    sample_text: String,
    /// Cursor position as a *character* index (not a byte offset) —
    /// converting per-char movement to byte indices directly panicked on
    /// multibyte input (GPT review F-15).
    cursor_pos: usize,
}

impl WebApp {
    fn new(fonts: Vec<FontEntry>) -> Self {
        Self {
            fonts,
            selected: 0,
            sample_text: String::from("Hello, World!"),
            cursor_pos: 13,
        }
    }

    /// Number of characters in the sample text.
    fn char_len(&self) -> usize {
        self.sample_text.chars().count()
    }

    /// Byte offset of the cursor's character index within `sample_text`.
    fn byte_offset(&self) -> usize {
        self.sample_text
            .char_indices()
            .nth(self.cursor_pos)
            .map(|(i, _)| i)
            .unwrap_or(self.sample_text.len())
    }

    fn handle_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char(c) => {
                let at_end = self.cursor_pos >= self.char_len();
                if at_end {
                    self.sample_text.push(c);
                } else {
                    let byte = self.byte_offset();
                    self.sample_text.insert(byte, c);
                }
                self.cursor_pos += 1;
            }
            KeyCode::Backspace if self.cursor_pos > 0 && !self.sample_text.is_empty() => {
                let byte = self.byte_offset();
                // Width (in bytes) of the character before the cursor.
                let prev_len = self.sample_text[..byte]
                    .chars()
                    .next_back()
                    .map(|c| c.len_utf8())
                    .unwrap_or(0);
                if prev_len > 0 {
                    self.sample_text.remove(byte - prev_len);
                }
                self.cursor_pos -= 1;
            }
            KeyCode::Delete if self.cursor_pos < self.char_len() => {
                let byte = self.byte_offset();
                self.sample_text.remove(byte);
            }
            KeyCode::Left => {
                self.cursor_pos = self.cursor_pos.saturating_sub(1);
            }
            KeyCode::Right if self.cursor_pos < self.char_len() => {
                self.cursor_pos += 1;
            }
            KeyCode::Up if self.selected > 0 => {
                self.selected -= 1;
            }
            KeyCode::Down if self.selected + 1 < self.fonts.len() => {
                self.selected += 1;
            }
            KeyCode::Home => {
                self.cursor_pos = 0;
            }
            KeyCode::End => {
                self.cursor_pos = self.char_len();
            }
            _ => {}
        }
    }

    fn render(&self, f: &mut ratzilla::ratatui::Frame) {
        let area = f.area();

        let chunks = Layout::default()
            .direction(ratzilla::ratatui::layout::Direction::Horizontal)
            .constraints([Constraint::Length(24), Constraint::Min(0)])
            .split(area);

        let font_items: Vec<ListItem> = self
            .fonts
            .iter()
            .enumerate()
            .map(|(i, entry)| {
                let style = if i == self.selected {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                ListItem::new(entry.name.to_string()).style(style)
            })
            .collect();

        let font_list = List::new(font_items)
            .block(Block::new().title(" Fonts ").borders(Borders::ALL))
            .highlight_style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            );
        f.render_widget(font_list, chunks[0]);

        let right_chunks = Layout::default()
            .direction(ratzilla::ratatui::layout::Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .split(chunks[1]);

        let input = Paragraph::new(self.sample_text.as_str())
            .block(Block::new().title(" Text ").borders(Borders::ALL))
            .style(Style::default().fg(Color::White));
        f.render_widget(input, right_chunks[0]);

        let font = &self.fonts[self.selected].font;
        let rendered = render_string(font, &self.sample_text);
        let output_lines: Vec<Line> = rendered
            .iter()
            .map(|row| Line::from(Span::raw(row.as_str())))
            .collect();

        let output = Paragraph::new(Text::from(output_lines))
            .block(Block::new().title(" Output ").borders(Borders::ALL))
            .wrap(Wrap { trim: false });

        f.render_widget(output, right_chunks[1]);
    }
}

fn load_embedded_fonts() -> Vec<FontEntry> {
    let embedded: &[(&str, &[u8])] = &[
        ("standard", include_bytes!("../assets/fonts/standard.flf")),
        ("banner", include_bytes!("../assets/fonts/banner.flf")),
        ("big", include_bytes!("../assets/fonts/big.flf")),
    ];

    let mut fonts = Vec::new();
    for (name, bytes) in embedded {
        let content = String::from_utf8_lossy(bytes);
        if let Ok(font) = parse_tlf_font(&content) {
            fonts.push(FontEntry { name, font });
        }
    }
    fonts
}

pub fn run_web() -> io::Result<()> {
    let fonts = load_embedded_fonts();
    if fonts.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "no fonts could be loaded",
        ));
    }

    let app = Rc::new(RefCell::new(WebApp::new(fonts)));
    let backend = DomBackend::new()?;
    let mut terminal = Terminal::new(backend)?;

    terminal.on_key_event({
        let app = app.clone();
        move |key_event| {
            app.borrow_mut().handle_key(key_event.code);
        }
    })?;

    terminal.draw_web(move |f| {
        app.borrow().render(f);
    });

    Ok(())
}

#[cfg(test)]
mod web_tests {
    use super::*;

    fn app_with(text: &str) -> WebApp {
        let fonts = vec![FontEntry {
            name: "standard",
            font: parse_tlf_font(include_str!("../assets/fonts/standard.flf"))
                .expect("standard.flf should parse"),
        }];
        let mut app = WebApp::new(fonts);
        app.sample_text = text.to_string();
        app.cursor_pos = text.chars().count();
        app
    }

    /// F-15 (GPT review): editing around multibyte characters must not
    /// panic on interior byte indices.
    #[test]
    fn test_multibyte_editing_no_panic() {
        let mut app = app_with("é");
        // Backspace over 'é' (2 bytes, 1 char).
        app.handle_key(KeyCode::Backspace);
        assert_eq!(app.sample_text, "");
        assert_eq!(app.cursor_pos, 0);

        // Insert before a multibyte char (cursor at start).
        let mut app = app_with("éx");
        app.cursor_pos = 0;
        app.handle_key(KeyCode::Char('a'));
        assert_eq!(app.sample_text, "aéx");

        // Delete a multibyte char under the cursor.
        let mut app = app_with("aéx");
        app.cursor_pos = 1;
        app.handle_key(KeyCode::Delete);
        assert_eq!(app.sample_text, "ax");

        // CJK and emoji round-trip: type, move left twice, backspace.
        let mut app = app_with("");
        for c in ['日', '本', '🦀'] {
            app.handle_key(KeyCode::Char(c));
        }
        assert_eq!(app.sample_text, "日本🦀");
        app.handle_key(KeyCode::Left);
        app.handle_key(KeyCode::Left);
        app.handle_key(KeyCode::Backspace);
        assert_eq!(app.sample_text, "日🦀");
        assert_eq!(app.cursor_pos, 1);

        // Combining sequence stays valid UTF-8 through edits.
        let mut app = app_with("e\u{301}x"); // e + combining acute
        app.cursor_pos = 2; // after the combining mark
        app.handle_key(KeyCode::Backspace); // remove combining mark
        assert_eq!(app.sample_text, "ex");

        // End/Home navigate by chars, not bytes.
        let mut app = app_with("éx");
        app.handle_key(KeyCode::End);
        assert_eq!(app.cursor_pos, 2);
        app.handle_key(KeyCode::Home);
        assert_eq!(app.cursor_pos, 0);
    }
}
