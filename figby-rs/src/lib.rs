#![doc = "Figby — Rust port of FIGlet (Frank, Ian & Glenn's Letters)\n\nRenders text in large ASCII art characters using FIGfont (.flf)\nand TOIlet (.tlf) font files with kerning, smushing, and multi-byte\ncharacter support."]

mod canvas_inner {
    use ratatui::style::Color;
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct CanvasCell {
        pub ch: char,
        pub fg: Option<Color>,
        pub bg: Option<Color>,
        pub height: Option<u8>,
    }
    impl Default for CanvasCell {
        fn default() -> Self {
            Self {
                ch: ' ',
                fg: None,
                bg: None,
                height: None,
            }
        }
    }
}
pub use canvas_inner::CanvasCell;

pub mod atomic_io;
pub mod config;
pub mod control;
pub mod font;
#[cfg(not(target_arch = "wasm32"))]
pub mod font_gen;
pub mod gif_import;
pub mod image_input;
pub mod input;
pub mod output;
pub mod palette_import;
pub mod render;
pub mod smush;
pub mod template;
#[cfg(not(target_arch = "wasm32"))]
pub mod tui;
#[cfg(target_arch = "wasm32")]
pub mod web;
