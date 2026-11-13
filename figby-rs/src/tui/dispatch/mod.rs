//! Event dispatch for [`TuiApp`]: a router that hands each key, mouse and
//! menu event to the component that owns it. Split by concern from the
//! former single-file `dispatch.rs` (review A1).

use rand::rngs::StdRng;
use rand::Rng;
use rand::SeedableRng;
use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::Rect;
use std::sync::mpsc;

use super::dialogs;
use super::*;
use super::{capture_thumbnail, rotate_drag_steps};

mod actions;
mod canvas_mouse;
mod file_io;
mod global;
mod import_export;
mod keys;
mod lifecycle;
mod menu_actions;
mod mouse;
mod playback;
