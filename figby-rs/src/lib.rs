#![doc = "Figby — Rust port of FIGlet (Frank, Ian & Glenn's Letters)\n\nRenders text in large ASCII art characters using FIGfont (.flf)\nand TOIlet (.tlf) font files with kerning, smushing, and multi-byte\ncharacter support."]

pub use ::figseq::CanvasCell;

pub mod atomic_io;
pub mod bounded_io;
pub mod config;
pub mod control;
#[cfg(not(target_arch = "wasm32"))]
pub mod figmap;
pub mod figseq;
pub mod font;
#[cfg(not(target_arch = "wasm32"))]
pub mod font_gen;
pub mod gif_import;
pub mod image_input;
pub mod input;
pub mod output;
pub mod palette_import;
pub mod render;
pub mod sanitize;
pub mod smush;
pub mod template;
#[cfg(not(target_arch = "wasm32"))]
pub mod tui;
#[cfg(target_arch = "wasm32")]
pub mod web;
