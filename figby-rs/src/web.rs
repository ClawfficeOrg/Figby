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

    fn handle_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char(c) => {
                if self.cursor_pos >= self.sample_text.len() {
                    self.sample_text.push(c);
                } else {
                    self.sample_text.insert(self.cursor_pos, c);
                }
                self.cursor_pos += 1;
            }
            KeyCode::Backspace if self.cursor_pos > 0 && !self.sample_text.is_empty() => {
                self.cursor_pos -= 1;
                self.sample_text.remove(self.cursor_pos);
            }
            KeyCode::Delete if self.cursor_pos < self.sample_text.len() => {
                self.sample_text.remove(self.cursor_pos);
            }
            KeyCode::Left => {
                self.cursor_pos = self.cursor_pos.saturating_sub(1);
            }
            KeyCode::Right if self.cursor_pos < self.sample_text.len() => {
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
                self.cursor_pos = self.sample_text.len();
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
