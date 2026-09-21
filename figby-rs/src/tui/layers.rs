use std::str::FromStr;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders};
use ratatui::Frame;
use serde::{Deserialize, Serialize};

use super::canvas::{CanvasBuffer, CanvasCell};
use super::theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum BlendMode {
    #[default]
    Normal,
    Multiply,
    Overlay,
    Screen,
    Add,
    Subtract,
}

impl BlendMode {
    pub fn next(&self) -> Self {
        match self {
            BlendMode::Normal => BlendMode::Multiply,
            BlendMode::Multiply => BlendMode::Overlay,
            BlendMode::Overlay => BlendMode::Screen,
            BlendMode::Screen => BlendMode::Add,
            BlendMode::Add => BlendMode::Subtract,
            BlendMode::Subtract => BlendMode::Normal,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            BlendMode::Normal => BlendMode::Subtract,
            BlendMode::Multiply => BlendMode::Normal,
            BlendMode::Overlay => BlendMode::Multiply,
            BlendMode::Screen => BlendMode::Overlay,
            BlendMode::Add => BlendMode::Screen,
            BlendMode::Subtract => BlendMode::Add,
        }
    }

    pub fn icon_key(&self) -> &str {
        match self {
            BlendMode::Normal => "blend_normal",
            BlendMode::Multiply => "blend_multiply",
            BlendMode::Overlay => "blend_overlay",
            BlendMode::Screen => "blend_screen",
            BlendMode::Add => "blend_add",
            BlendMode::Subtract => "blend_subtract",
        }
    }

    pub fn display_name(&self) -> &str {
        match self {
            BlendMode::Normal => "Normal",
            BlendMode::Multiply => "Multiply",
            BlendMode::Overlay => "Overlay",
            BlendMode::Screen => "Screen",
            BlendMode::Add => "Add",
            BlendMode::Subtract => "Subtract",
        }
    }
}

impl FromStr for BlendMode {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "multiply" => Ok(BlendMode::Multiply),
            "overlay" => Ok(BlendMode::Overlay),
            "screen" => Ok(BlendMode::Screen),
            "add" => Ok(BlendMode::Add),
            "subtract" => Ok(BlendMode::Subtract),
            _ => Ok(BlendMode::Normal),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerMask {
    pub buffer: CanvasBuffer,
    pub enabled: bool,
}

impl LayerMask {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            buffer: CanvasBuffer::new(width, height),
            enabled: true,
        }
    }

    pub fn buffer(&self) -> &CanvasBuffer {
        &self.buffer
    }

    pub fn buffer_mut(&mut self) -> &mut CanvasBuffer {
        &mut self.buffer
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerGroup {
    pub name: String,
    pub collapsed: bool,
}

impl LayerGroup {
    pub fn new(name: String) -> Self {
        Self {
            name,
            collapsed: false,
        }
    }
}

/// A set of layers whose visibility and lock state stay in sync with each
/// other — toggling either on one member propagates to the rest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerLink {
    pub layer_indices: Vec<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Layer {
    pub buffer: CanvasBuffer,
    pub name: String,
    pub visible: bool,
    pub locked: bool,
    pub opacity: u8,
    pub blend_mode: BlendMode,
    pub mask: Option<LayerMask>,
    pub group: Option<usize>,
    pub link: Option<usize>,
    pub accepts_lighting: bool,
    pub casts_shadow: bool,
}

impl Layer {
    pub fn new(width: usize, height: usize, name: String) -> Self {
        Self {
            buffer: CanvasBuffer::new(width, height),
            name,
            visible: true,
            locked: false,
            opacity: 255,
            blend_mode: BlendMode::Normal,
            mask: None,
            group: None,
            link: None,
            accepts_lighting: true,
            casts_shadow: true,
        }
    }

    pub fn buffer_mut(&mut self) -> &mut CanvasBuffer {
        &mut self.buffer
    }

    pub fn buffer(&self) -> &CanvasBuffer {
        &self.buffer
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerStack {
    pub layers: Vec<Layer>,
    pub active: usize,
    pub groups: Vec<LayerGroup>,
    pub links: Vec<LayerLink>,
}

impl LayerStack {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            layers: vec![Layer::new(width, height, "Background".to_string())],
            active: 0,
            groups: Vec::new(),
            links: Vec::new(),
        }
    }

    pub fn with_capacity(width: usize, height: usize, initial_layers: usize) -> Self {
        let mut layers = Vec::with_capacity(initial_layers.max(1));
        for i in 0..initial_layers.max(1) {
            let name = if i == 0 {
                "Background".to_string()
            } else {
                format!("Layer {}", i + 1)
            };
            layers.push(Layer::new(width, height, name));
        }
        Self {
            layers,
            active: 0,
            groups: Vec::new(),
            links: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.layers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.layers.is_empty()
    }

    pub fn active_layer(&self) -> &Layer {
        &self.layers[self.active.min(self.layers.len().saturating_sub(1))]
    }

    pub fn active_layer_mut(&mut self) -> &mut Layer {
        let idx = self.active.min(self.layers.len().saturating_sub(1));
        &mut self.layers[idx]
    }

    pub fn add(&mut self, width: usize, height: usize) -> usize {
        let idx = self.layers.len();
        self.layers
            .push(Layer::new(width, height, format!("Layer {}", idx + 1)));
        self.active = idx;
        idx
    }

    pub fn delete(&mut self, index: usize) -> bool {
        if index >= self.layers.len() || self.layers.len() <= 1 {
            return false;
        }
        self.layers.remove(index);
        if self.active >= self.layers.len() {
            self.active = self.layers.len() - 1;
        } else if self.active > 0 && index <= self.active {
            self.active = self.active.saturating_sub(1);
        }
        true
    }

    pub fn duplicate(&mut self, index: usize) -> bool {
        if index >= self.layers.len() {
            return false;
        }
        let original = self.layers[index].clone();
        let idx = self.layers.len();
        self.layers.push(original);
        self.active = idx;
        true
    }

    pub fn merge_down(&mut self, index: usize) -> bool {
        if index == 0 || index >= self.layers.len() {
            return false;
        }
        if self.layers[index].locked {
            return false;
        }
        let above = self.layers.remove(index);
        let below = &mut self.layers[index - 1];
        for y in 0..below.buffer.height().min(above.buffer.height()) {
            for x in 0..below.buffer.width().min(above.buffer.width()) {
                if let Some(cell) = above.buffer.get(x, y) {
                    if cell.ch != ' ' {
                        below.buffer.set(x, y, *cell);
                    }
                }
            }
        }
        self.active = (index - 1).min(self.layers.len().saturating_sub(1));
        true
    }

    pub fn move_up(&mut self, index: usize) -> bool {
        if index + 1 >= self.layers.len() {
            return false;
        }
        self.layers.swap(index, index + 1);
        if self.active == index {
            self.active = index + 1;
        } else if self.active == index + 1 {
            self.active = index;
        }
        true
    }

    pub fn move_down(&mut self, index: usize) -> bool {
        if index == 0 || index >= self.layers.len() {
            return false;
        }
        self.layers.swap(index, index - 1);
        if self.active == index {
            self.active = index - 1;
        } else if self.active == index - 1 {
            self.active = index;
        }
        true
    }

    pub fn reorder(&mut self, from: usize, to: usize) -> bool {
        if from >= self.layers.len() || to >= self.layers.len() || from == to {
            return false;
        }
        let layer = self.layers.remove(from);
        self.layers.insert(to, layer);
        if self.active == from {
            self.active = to;
        } else {
            if from < self.active && to >= self.active {
                self.active = self.active.saturating_sub(1);
            } else if from > self.active && to <= self.active {
                self.active = self.active.saturating_add(1);
            }
        }
        true
    }

    pub fn set_active(&mut self, index: usize) {
        if index < self.layers.len() {
            self.active = index;
        }
    }

    pub fn resize_all(&mut self, width: usize, height: usize) {
        for layer in &mut self.layers {
            if layer.buffer.width() != width || layer.buffer.height() != height {
                let mut new_buf = CanvasBuffer::new(width, height);
                for y in 0..layer.buffer.height().min(height) {
                    for x in 0..layer.buffer.width().min(width) {
                        if let Some(cell) = layer.buffer.get(x, y) {
                            new_buf.set(x, y, *cell);
                        }
                    }
                }
                layer.buffer = new_buf;
            }
        }
    }

    pub fn create_group(&mut self, indices: &[usize], name: String) -> Option<usize> {
        if indices.is_empty() {
            return None;
        }
        for &idx in indices {
            if idx >= self.layers.len() {
                return None;
            }
        }
        let group_idx = self.groups.len();
        self.groups.push(LayerGroup::new(name));
        for &idx in indices {
            self.layers[idx].group = Some(group_idx);
        }
        Some(group_idx)
    }

    pub fn remove_group(&mut self, group_idx: usize) -> bool {
        if group_idx >= self.groups.len() {
            return false;
        }
        for layer in &mut self.layers {
            if layer.group == Some(group_idx) {
                layer.group = None;
            }
        }
        self.groups.remove(group_idx);
        for layer in &mut self.layers {
            if let Some(g) = layer.group {
                if g > group_idx {
                    layer.group = Some(g - 1);
                }
            }
        }
        true
    }

    pub fn toggle_group_collapsed(&mut self, group_idx: usize) -> bool {
        if let Some(group) = self.groups.get_mut(group_idx) {
            group.collapsed = !group.collapsed;
            true
        } else {
            false
        }
    }

    pub fn rename_group(&mut self, group_idx: usize, name: String) -> bool {
        if let Some(group) = self.groups.get_mut(group_idx) {
            group.name = name;
            true
        } else {
            false
        }
    }

    pub fn rename(&mut self, index: usize, name: String) -> bool {
        if name.trim().is_empty() {
            return false;
        }
        if let Some(layer) = self.layers.get_mut(index) {
            layer.name = name;
            true
        } else {
            false
        }
    }

    pub fn group_of_layer(&self, layer_idx: usize) -> Option<usize> {
        self.layers.get(layer_idx).and_then(|l| l.group)
    }

    pub fn layers_in_group(&self, group_idx: usize) -> Vec<usize> {
        self.layers
            .iter()
            .enumerate()
            .filter(|(_, l)| l.group == Some(group_idx))
            .map(|(i, _)| i)
            .collect()
    }

    /// Link the given layers so their visibility and lock state stay in
    /// sync (mirrors `create_group`). If any of the layers is already
    /// linked, it's moved into the new link set rather than double-linked.
    pub fn link_layers(&mut self, indices: &[usize]) -> Option<usize> {
        if indices.len() < 2 {
            return None;
        }
        for &idx in indices {
            if idx >= self.layers.len() {
                return None;
            }
        }
        for &idx in indices {
            if let Some(old_link) = self.layers[idx].link {
                self.unlink_layers(old_link);
            }
        }
        let link_idx = self.links.len();
        self.links.push(LayerLink {
            layer_indices: indices.to_vec(),
        });
        for &idx in indices {
            self.layers[idx].link = Some(link_idx);
        }
        Some(link_idx)
    }

    pub fn unlink_layers(&mut self, link_idx: usize) -> bool {
        if link_idx >= self.links.len() {
            return false;
        }
        for layer in &mut self.layers {
            if layer.link == Some(link_idx) {
                layer.link = None;
            }
        }
        self.links.remove(link_idx);
        for layer in &mut self.layers {
            if let Some(l) = layer.link {
                if l > link_idx {
                    layer.link = Some(l - 1);
                }
            }
        }
        true
    }

    pub fn link_of_layer(&self, layer_idx: usize) -> Option<usize> {
        self.layers.get(layer_idx).and_then(|l| l.link)
    }

    pub fn layers_in_link(&self, link_idx: usize) -> Vec<usize> {
        self.layers
            .iter()
            .enumerate()
            .filter(|(_, l)| l.link == Some(link_idx))
            .map(|(i, _)| i)
            .collect()
    }

    pub fn create_mask(&mut self, layer_idx: usize) -> bool {
        if let Some(layer) = self.layers.get_mut(layer_idx) {
            if layer.mask.is_some() {
                return false;
            }
            let w = layer.buffer.width();
            let h = layer.buffer.height();
            layer.mask = Some(LayerMask::new(w, h));
            true
        } else {
            false
        }
    }

    pub fn remove_mask(&mut self, layer_idx: usize) -> bool {
        if let Some(layer) = self.layers.get_mut(layer_idx) {
            if layer.mask.is_some() {
                layer.mask = None;
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    pub fn toggle_mask(&mut self, layer_idx: usize) -> bool {
        let layer = match self.layers.get_mut(layer_idx) {
            Some(l) => l,
            None => return false,
        };
        if layer.mask.is_some() {
            layer.mask = None;
        } else {
            let w = layer.buffer.width();
            let h = layer.buffer.height();
            layer.mask = Some(LayerMask::new(w, h));
        }
        true
    }

    pub fn toggle_mask_enabled(&mut self, layer_idx: usize) -> bool {
        if let Some(layer) = self.layers.get_mut(layer_idx) {
            if let Some(ref mut mask) = layer.mask {
                mask.enabled = !mask.enabled;
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    pub fn set_mask_pixel(&mut self, layer_idx: usize, x: usize, y: usize, cell: CanvasCell) {
        if let Some(layer) = self.layers.get_mut(layer_idx) {
            if let Some(ref mut mask) = layer.mask {
                if x < mask.buffer.width() && y < mask.buffer.height() {
                    mask.buffer.set(x, y, cell);
                }
            }
        }
    }

    pub fn get_mask_pixel(&self, layer_idx: usize, x: usize, y: usize) -> Option<CanvasCell> {
        self.layers
            .get(layer_idx)
            .and_then(|l| l.mask.as_ref())
            .and_then(|m| m.buffer.get(x, y))
            .copied()
    }

    pub fn composite(&self) -> CanvasBuffer {
        if self.layers.is_empty() {
            return CanvasBuffer::new(1, 1);
        }
        let w = self.layers[0].buffer.width();
        let h = self.layers[0].buffer.height();
        let mut result = CanvasBuffer::new(w, h);
        for layer in &self.layers {
            if !layer.visible {
                continue;
            }
            let opacity = layer.opacity;
            let blend_mode = layer.blend_mode;
            for y in 0..h.min(layer.buffer.height()) {
                for x in 0..w.min(layer.buffer.width()) {
                    if let Some(top) = layer.buffer.get(x, y) {
                        if top.ch == ' ' && top.fg.is_none() && top.bg.is_none() {
                            continue;
                        }
                        if let Some(ref mask) = layer.mask {
                            if mask.enabled {
                                if let Some(mask_cell) = mask.buffer.get(x, y) {
                                    if mask_cell.ch == ' ' {
                                        continue;
                                    }
                                }
                            }
                        }
                        if opacity == 255 && blend_mode == BlendMode::Normal {
                            result.set(x, y, *top);
                        } else if opacity > 0 {
                            let bottom = result.get(x, y).copied().unwrap_or_default();
                            let blended_fg = blend_mode_color(top.fg, bottom.fg, blend_mode);
                            let blended_bg = blend_mode_color(top.bg, bottom.bg, blend_mode);
                            let final_fg = blend_colors(blended_fg, bottom.fg, opacity);
                            let final_bg = blend_colors(blended_bg, bottom.bg, opacity);
                            result.set(
                                x,
                                y,
                                super::canvas::CanvasCell {
                                    ch: top.ch,
                                    fg: final_fg,
                                    bg: final_bg,
                                    height: None,
                                },
                            );
                        }
                    }
                }
            }
        }
        result
    }

    pub fn toggle_visibility(&mut self, index: usize) {
        let Some(layer) = self.layers.get_mut(index) else {
            return;
        };
        layer.visible = !layer.visible;
        let new_state = layer.visible;
        if let Some(link_idx) = self.link_of_layer(index) {
            for i in self.layers_in_link(link_idx) {
                if i != index {
                    if let Some(l) = self.layers.get_mut(i) {
                        l.visible = new_state;
                    }
                }
            }
        }
    }

    pub fn toggle_lock(&mut self, index: usize) {
        let Some(layer) = self.layers.get_mut(index) else {
            return;
        };
        layer.locked = !layer.locked;
        let new_state = layer.locked;
        if let Some(link_idx) = self.link_of_layer(index) {
            for i in self.layers_in_link(link_idx) {
                if i != index {
                    if let Some(l) = self.layers.get_mut(i) {
                        l.locked = new_state;
                    }
                }
            }
        }
    }

    pub fn toggle_accepts_lighting(&mut self, index: usize) {
        if let Some(layer) = self.layers.get_mut(index) {
            layer.accepts_lighting = !layer.accepts_lighting;
        }
    }

    pub fn toggle_casts_shadow(&mut self, index: usize) {
        if let Some(layer) = self.layers.get_mut(index) {
            layer.casts_shadow = !layer.casts_shadow;
        }
    }

    pub fn set_opacity(&mut self, index: usize, opacity: u8) {
        if let Some(layer) = self.layers.get_mut(index) {
            layer.opacity = opacity;
        }
    }

    pub fn set_blend_mode(&mut self, index: usize, mode: BlendMode) {
        if let Some(layer) = self.layers.get_mut(index) {
            layer.blend_mode = mode;
        }
    }

    pub fn add_frozen_frames(&mut self, frames: Vec<CanvasBuffer>, base_name: &str) -> Vec<usize> {
        let mut indices = Vec::with_capacity(frames.len());
        for (i, buffer) in frames.into_iter().enumerate() {
            let idx = self.layers.len();
            let name = format!("{} frame {}", base_name, i);
            let mut layer = Layer::new(buffer.width(), buffer.height(), name);
            layer.buffer = buffer;
            layer.visible = true;
            self.layers.push(layer);
            self.active = idx;
            indices.push(idx);
        }
        indices
    }
}

pub(crate) fn blend_colors(
    top: Option<Color>,
    bottom: Option<Color>,
    opacity: u8,
) -> Option<Color> {
    match (top, bottom) {
        (Some(t), Some(b)) => {
            let f = opacity as f32 / 255.0;
            match (t, b) {
                (Color::Rgb(tr, tg, tb), Color::Rgb(br, bg, bb)) => {
                    let r = (tr as f32 * f + br as f32 * (1.0 - f)).round() as u8;
                    let g = (tg as f32 * f + bg as f32 * (1.0 - f)).round() as u8;
                    let b = (tb as f32 * f + bb as f32 * (1.0 - f)).round() as u8;
                    Some(Color::Rgb(r, g, b))
                }
                _ => Some(t),
            }
        }
        (Some(t), None) => Some(t),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

fn blend_channel(top: u8, bottom: u8, mode: BlendMode) -> u8 {
    match mode {
        BlendMode::Normal => top,
        BlendMode::Multiply => ((top as u32) * (bottom as u32) / 255) as u8,
        BlendMode::Overlay => {
            if bottom < 128 {
                ((2u32 * top as u32 * bottom as u32) / 255) as u8
            } else {
                (255u32 - (2u32 * (255u32 - top as u32) * (255u32 - bottom as u32)) / 255) as u8
            }
        }
        BlendMode::Screen => {
            (255u32 - ((255u32 - top as u32) * (255u32 - bottom as u32)) / 255) as u8
        }
        BlendMode::Add => top.saturating_add(bottom),
        BlendMode::Subtract => bottom.saturating_sub(top),
    }
}

pub(crate) fn blend_mode_color(
    top: Option<Color>,
    bottom: Option<Color>,
    mode: BlendMode,
) -> Option<Color> {
    if mode == BlendMode::Normal {
        return top;
    }
    match (top, bottom) {
        (Some(t), Some(b)) => match (t, b) {
            (Color::Rgb(tr, tg, tb), Color::Rgb(br, bg, bb)) => {
                let r = blend_channel(tr, br, mode);
                let g = blend_channel(tg, bg, mode);
                let b = blend_channel(tb, bb, mode);
                Some(Color::Rgb(r, g, b))
            }
            _ => Some(t),
        },
        (Some(t), None) => Some(t),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

/// Toolbar button actions rendered on the layers panel's header row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerButton {
    New,
    Duplicate,
    Delete,
    Group,
    Link,
}

pub struct LayerPanel {
    pub scroll: u16,
    pub theme: Theme,
    pub icons: std::collections::BTreeMap<String, String>,
    drag_state: Option<(usize, usize)>,
    drag_hover_row: Option<usize>,
    pub renaming: Option<usize>,
    pub rename_buffer: String,
    /// Toolbar button hit-rects, recomputed each render and consulted by
    /// `handle_mouse` — same store/hit-test convention as `menu.rs`'s
    /// `MenuBarState.item_rects`.
    button_rects: Vec<(LayerButton, Rect)>,
    /// First layer picked for a link, armed by pressing `k`/`K` or clicking
    /// the Link button once; a second press/click on a different layer
    /// completes the pair (mirrors the one-at-a-time group-building flow).
    pub link_pending: Option<usize>,
}

impl LayerPanel {
    pub fn new() -> Self {
        Self {
            scroll: 0,
            theme: Theme::default(),
            icons: std::collections::BTreeMap::new(),
            drag_state: None,
            drag_hover_row: None,
            renaming: None,
            rename_buffer: String::new(),
            button_rects: Vec::new(),
            link_pending: None,
        }
    }

    /// Arm or complete a layer link, shared by the `k`/`K` keybind and the
    /// Link toolbar button.
    fn toggle_link_pending(&mut self, stack: &mut LayerStack) {
        match self.link_pending {
            None => {
                self.link_pending = Some(stack.active);
            }
            Some(from) if from == stack.active => {
                self.link_pending = None;
            }
            Some(from) => {
                stack.link_layers(&[from, stack.active]);
                self.link_pending = None;
            }
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent, stack: &mut LayerStack) -> bool {
        let code = key.code;
        let modifiers = key.modifiers;

        if let Some(idx) = self.renaming {
            match code {
                KeyCode::Enter => {
                    stack.rename(idx, self.rename_buffer.trim().to_string());
                    self.renaming = None;
                    self.rename_buffer.clear();
                }
                KeyCode::Esc => {
                    self.renaming = None;
                    self.rename_buffer.clear();
                }
                KeyCode::Backspace => {
                    self.rename_buffer.pop();
                }
                KeyCode::Char(c) => {
                    self.rename_buffer.push(c);
                }
                _ => {}
            }
            return true;
        }

        match code {
            KeyCode::F(2) => {
                self.renaming = Some(stack.active);
                self.rename_buffer = stack
                    .layers
                    .get(stack.active)
                    .map(|l| l.name.clone())
                    .unwrap_or_default();
                true
            }
            KeyCode::Up if modifiers == KeyModifiers::ALT | KeyModifiers::SHIFT => {
                stack.move_up(stack.active);
                true
            }
            KeyCode::Down if modifiers == KeyModifiers::ALT | KeyModifiers::SHIFT => {
                stack.move_down(stack.active);
                true
            }
            KeyCode::Up if modifiers == KeyModifiers::ALT => {
                if stack.active > 0 {
                    stack.active -= 1;
                }
                true
            }
            KeyCode::Down if modifiers == KeyModifiers::ALT => {
                if stack.active + 1 < stack.layers.len() {
                    stack.active += 1;
                }
                true
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                stack.toggle_visibility(stack.active);
                true
            }
            KeyCode::Char('l') => {
                stack.toggle_lock(stack.active);
                true
            }
            KeyCode::Char('L') => {
                stack.toggle_accepts_lighting(stack.active);
                true
            }
            KeyCode::Char('S') if modifiers == KeyModifiers::ALT => {
                stack.toggle_casts_shadow(stack.active);
                true
            }
            KeyCode::Char('+') | KeyCode::Char('=') => {
                let o = stack.layers[stack.active].opacity;
                stack.layers[stack.active].opacity = o.saturating_add(25);
                true
            }
            KeyCode::Char('-') | KeyCode::Char('_') => {
                let o = stack.layers[stack.active].opacity;
                stack.layers[stack.active].opacity = o.saturating_sub(25);
                true
            }
            KeyCode::Char('n') | KeyCode::Char('N') => {
                let w = stack.layers[0].buffer.width();
                let h = stack.layers[0].buffer.height();
                stack.add(w, h);
                true
            }
            KeyCode::Char('d') | KeyCode::Char('D') => {
                stack.duplicate(stack.active);
                true
            }
            KeyCode::Delete | KeyCode::Char('x') | KeyCode::Char('X') => {
                stack.delete(stack.active);
                true
            }
            KeyCode::Char('g') if modifiers == KeyModifiers::CONTROL => {
                let indices = [stack.active];
                let group_name = format!("Group {}", stack.groups.len() + 1);
                stack.create_group(&indices, group_name);
                true
            }
            KeyCode::Char('g') if modifiers == KeyModifiers::CONTROL | KeyModifiers::SHIFT => {
                if let Some(g) = stack.group_of_layer(stack.active) {
                    stack.remove_group(g);
                }
                true
            }
            KeyCode::Char('k') | KeyCode::Char('K') => {
                self.toggle_link_pending(stack);
                true
            }
            KeyCode::Right if modifiers == KeyModifiers::ALT => {
                if let Some(g) = stack.group_of_layer(stack.active) {
                    if let Some(grp) = stack.groups.get_mut(g) {
                        grp.collapsed = false;
                        return true;
                    }
                }
                false
            }
            KeyCode::Left if modifiers == KeyModifiers::ALT => {
                if let Some(g) = stack.group_of_layer(stack.active) {
                    if let Some(grp) = stack.groups.get_mut(g) {
                        grp.collapsed = true;
                        return true;
                    }
                }
                false
            }
            KeyCode::Char('M') => {
                stack.toggle_mask(stack.active);
                true
            }
            KeyCode::Char('m') => {
                if stack
                    .layers
                    .get(stack.active)
                    .and_then(|l| l.mask.as_ref())
                    .is_some()
                {
                    stack.toggle_mask_enabled(stack.active);
                } else {
                    stack.merge_down(stack.active);
                }
                true
            }
            KeyCode::Tab if modifiers == KeyModifiers::ALT => {
                if stack.groups.is_empty() {
                    return false;
                }
                if let Some(g) = stack.group_of_layer(stack.active) {
                    let members = stack.layers_in_group(g);
                    if let Some(&last) = members.last() {
                        if stack.active == last {
                            let next = last + 1;
                            if next < stack.layers.len() {
                                stack.active = next;
                            }
                        } else if let Some(&next) = members.iter().find(|&&i| i > stack.active) {
                            stack.active = next;
                        }
                    }
                } else {
                    let mut found = false;
                    for i in (0..stack.active).rev() {
                        if stack.layers[i].group.is_some() {
                            stack.active = i;
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        for i in stack.active + 1..stack.layers.len() {
                            if stack.layers[i].group.is_some() {
                                stack.active = i;
                                break;
                            }
                        }
                    }
                }
                true
            }
            KeyCode::Char(',') => {
                stack.move_up(stack.active);
                true
            }
            KeyCode::Char('.') => {
                stack.move_down(stack.active);
                true
            }
            KeyCode::Char('b') => {
                let mode = stack.layers[stack.active].blend_mode;
                stack.layers[stack.active].blend_mode = mode.next();
                true
            }
            KeyCode::Char('B') => {
                let mode = stack.layers[stack.active].blend_mode;
                stack.layers[stack.active].blend_mode = mode.prev();
                true
            }
            _ => false,
        }
    }
}

impl Default for LayerPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl LayerPanel {
    pub fn render_with_stack(&mut self, frame: &mut Frame, area: Rect, stack: &LayerStack) {
        let block = Block::default()
            .title(" Layers ")
            .borders(Borders::ALL)
            .style(
                Style::default()
                    .bg(self.theme.layers.bg)
                    .fg(self.theme.layers.fg),
            );
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let inner_y = inner.y;
        let inner_x = inner.x;
        let inner_h = inner.height as usize;

        self.button_rects.clear();
        if inner_h > 0 {
            let buttons: [(LayerButton, &str, &str); 5] = [
                (
                    LayerButton::New,
                    self.icons
                        .get("layer_new")
                        .map(|s| s.as_str())
                        .unwrap_or("+"),
                    "New",
                ),
                (
                    LayerButton::Duplicate,
                    self.icons
                        .get("layer_duplicate")
                        .map(|s| s.as_str())
                        .unwrap_or("D"),
                    "Dup",
                ),
                (
                    LayerButton::Delete,
                    self.icons
                        .get("layer_delete")
                        .map(|s| s.as_str())
                        .unwrap_or("x"),
                    "Del",
                ),
                (
                    LayerButton::Group,
                    self.icons
                        .get("layer_group")
                        .map(|s| s.as_str())
                        .unwrap_or("G"),
                    "Grp",
                ),
                (
                    LayerButton::Link,
                    self.icons
                        .get("layer_link")
                        .map(|s| s.as_str())
                        .unwrap_or("\u{1f517}"),
                    "Link",
                ),
            ];
            let btn_w = (inner.width / buttons.len() as u16).max(1);
            let mut bx = inner_x;
            let mut spans: Vec<Span> = Vec::new();
            for (action, icon, label) in buttons {
                let text = format!(" {icon} {label}");
                let rect = Rect {
                    x: bx,
                    y: inner_y,
                    width: btn_w.min(inner_x + inner.width - bx),
                    height: 1,
                };
                self.button_rects.push((action, rect));
                spans.push(Span::styled(
                    truncate_str(&text, btn_w as usize),
                    Style::default().fg(self.theme.general.secondary),
                ));
                bx += btn_w;
            }
            frame.render_widget(
                ratatui::widgets::Paragraph::new(Line::from(spans)),
                Rect {
                    x: inner_x,
                    y: inner_y,
                    width: inner.width,
                    height: 1,
                },
            );
        }

        let offset = 1;
        let visible_rows = inner_h.saturating_sub(offset);
        if visible_rows == 0 {
            return;
        }

        // First pass: count display rows (2 per layer, 1 per group header).
        let mut total_display_rows: usize = 0;
        let mut active_display_row: usize = 0;
        {
            let mut emitted = vec![false; stack.groups.len()];
            for rev_idx in 0..stack.layers.len() {
                let real_idx = stack.layers.len() - 1 - rev_idx;
                let layer = &stack.layers[real_idx];
                if let Some(g) = layer.group {
                    if g < emitted.len() && !emitted[g] {
                        emitted[g] = true;
                        total_display_rows += 1;
                        if let Some(group) = stack.groups.get(g) {
                            if group.collapsed {
                                continue;
                            }
                        }
                    }
                }
                if real_idx == stack.active {
                    active_display_row = total_display_rows;
                }
                total_display_rows += 2;
            }
        }

        // Clamp scroll so active layer name row is visible.
        if active_display_row < self.scroll as usize {
            self.scroll = active_display_row as u16;
        } else if active_display_row >= self.scroll as usize + visible_rows {
            self.scroll =
                (active_display_row.saturating_sub(visible_rows.saturating_sub(2))) as u16;
        }
        let scroll = self.scroll as usize;
        let has_more_below = total_display_rows > scroll + visible_rows;

        // Scroll indicators.
        if scroll > 0 {
            if let Some(cell) = frame.buffer_mut().cell_mut((
                inner_x + inner.width.saturating_sub(1),
                inner_y + offset as u16,
            )) {
                cell.set_char('▲');
                cell.set_style(Style::default().fg(self.theme.general.secondary));
            }
        }
        if has_more_below {
            if let Some(cell) = frame.buffer_mut().cell_mut((
                inner_x + inner.width.saturating_sub(1),
                inner_y + inner_h as u16 - 1,
            )) {
                cell.set_char('▼');
                cell.set_style(Style::default().fg(self.theme.general.secondary));
            }
        }

        // Second pass: render visible items.
        let mut display_row: usize = 0;
        let mut emitted_group_header = vec![false; stack.groups.len()];

        for rev_idx in 0..stack.layers.len() {
            let real_idx = stack.layers.len() - 1 - rev_idx;
            let layer = &stack.layers[real_idx];

            // Group header row.
            if let Some(g) = layer.group {
                if g < emitted_group_header.len() && !emitted_group_header[g] {
                    emitted_group_header[g] = true;
                    if display_row >= scroll {
                        let row = display_row - scroll;
                        if row < visible_rows {
                            if let Some(group) = stack.groups.get(g) {
                                let disclosure = if group.collapsed { "▶" } else { "▼" };
                                let count =
                                    stack.layers.iter().filter(|l| l.group == Some(g)).count();
                                let group_label =
                                    format!(" {} {} ({})", disclosure, group.name, count);
                                let row_style = Style::default()
                                    .bg(self.theme.layers.bg)
                                    .fg(self.theme.general.secondary)
                                    .add_modifier(Modifier::BOLD);
                                let para = ratatui::widgets::Paragraph::new(Line::from(
                                    Span::styled(group_label, row_style),
                                ));
                                frame.render_widget(
                                    para,
                                    Rect {
                                        x: inner_x,
                                        y: inner_y + offset as u16 + row as u16,
                                        width: inner.width,
                                        height: 1,
                                    },
                                );
                            }
                        }
                    }
                    display_row += 1;

                    if let Some(group) = stack.groups.get(g) {
                        if group.collapsed {
                            continue;
                        }
                    }
                }
            }

            let is_active = real_idx == stack.active;
            let row_bg = if is_active {
                self.theme.layers.active_bg
            } else {
                self.theme.layers.bg
            };
            let indent = if layer.group.is_some() { "  " } else { "" };
            let drag_handle = "⠿";
            let name_max = (inner.width as usize)
                .saturating_sub(indent.len() + 5)
                .max(4);

            // Row 1: drag handle + layer name.
            if display_row >= scroll {
                let row = display_row - scroll;
                if row < visible_rows {
                    let is_drag_target = self
                        .drag_state
                        .map(|(from, to)| from != to && real_idx == to)
                        .unwrap_or(false);
                    let active_marker = if is_active { "›" } else { " " };
                    let is_renaming = self.renaming == Some(real_idx);
                    let display_name = if is_renaming {
                        format!("{}\u{2588}", self.rename_buffer)
                    } else {
                        layer.name.clone()
                    };
                    let name_label = format!(
                        "{}{} {} {}",
                        indent,
                        drag_handle,
                        active_marker,
                        truncate_str(&display_name, name_max),
                    );
                    let fg = if is_renaming || is_drag_target {
                        self.theme.general.primary
                    } else {
                        self.theme.layers.fg
                    };
                    frame.render_widget(
                        ratatui::widgets::Paragraph::new(Line::from(Span::styled(
                            name_label,
                            Style::default().bg(row_bg).fg(fg),
                        ))),
                        Rect {
                            x: inner_x,
                            y: inner_y + offset as u16 + row as u16,
                            width: inner.width,
                            height: 1,
                        },
                    );
                }
            }
            display_row += 1;

            // Row 2: compact icon-based attributes.
            if display_row >= scroll {
                let row = display_row - scroll;
                if row >= visible_rows {
                    // No room for row 2 - still need to increment display_row
                    display_row += 1;
                    continue;
                }

                let vis_key = if layer.visible {
                    "layer_visibility_on"
                } else {
                    "layer_visibility_off"
                };
                let vis_icon = self
                    .icons
                    .get(vis_key)
                    .map(|s| s.as_str())
                    .unwrap_or(if layer.visible { "V" } else { "_" });

                let lock_key = if layer.locked {
                    "layer_lock"
                } else {
                    "layer_unlock"
                };
                let lock_icon = self
                    .icons
                    .get(lock_key)
                    .map(|s| s.as_str())
                    .unwrap_or(if layer.locked { "L" } else { "U" });

                let opa_pct = (layer.opacity as f32 / 255.0 * 100.0).round() as u8;
                let link_suffix = if layer.link.is_some() {
                    " \u{1f517}"
                } else {
                    ""
                };

                let attr_style = Style::default()
                    .bg(row_bg)
                    .fg(self.theme.general.secondary)
                    .add_modifier(Modifier::DIM);

                let attr_label = if inner.width >= 8 {
                    format!(
                        "{}{}  {}  {}%  {}{}",
                        indent,
                        vis_icon,
                        lock_icon,
                        opa_pct,
                        self.icons
                            .get(layer.blend_mode.icon_key())
                            .map(|s| s.as_str())
                            .unwrap_or(layer.blend_mode.display_name()),
                        link_suffix,
                    )
                } else {
                    format!(
                        "{}{}{} {}%{}",
                        indent,
                        if layer.visible { "V" } else { "_" },
                        if layer.locked { "L" } else { "_" },
                        opa_pct,
                        link_suffix,
                    )
                };

                frame.render_widget(
                    ratatui::widgets::Paragraph::new(Line::from(Span::styled(
                        attr_label, attr_style,
                    ))),
                    Rect {
                        x: inner_x,
                        y: inner_y + offset as u16 + row as u16,
                        width: inner.width,
                        height: 1,
                    },
                );
            }
            display_row += 1;
        }
    }

    /// Map screen (col, row) within the content area to a layer index.
    fn layer_at_pos(&self, col: u16, row: u16, area: Rect, stack: &LayerStack) -> Option<usize> {
        let inner = Block::default().borders(Borders::ALL).inner(area);
        let offset = 1usize;
        if row < inner.y + offset as u16 || col < inner.x {
            return None;
        }
        let vis_row = (row - inner.y) as usize;
        if vis_row < offset {
            return None;
        }
        let effective = vis_row.saturating_sub(offset) + self.scroll as usize;

        let mut display_row: usize = 0;
        let mut emitted = vec![false; stack.groups.len()];

        for rev_idx in 0..stack.layers.len() {
            let real_idx = stack.layers.len() - 1 - rev_idx;
            let layer = &stack.layers[real_idx];

            if let Some(g) = layer.group {
                if g < emitted.len() && !emitted[g] {
                    emitted[g] = true;
                    if effective == display_row {
                        return None;
                    }
                    display_row += 1;
                    if let Some(group) = stack.groups.get(g) {
                        if group.collapsed {
                            continue;
                        }
                    }
                }
            }

            if effective >= display_row && effective < display_row + 2 {
                return Some(real_idx);
            }
            display_row += 2;
        }
        None
    }

    pub fn handle_mouse(
        &mut self,
        col: u16,
        row: u16,
        kind: crossterm::event::MouseEventKind,
        area: Rect,
        stack: &mut LayerStack,
    ) -> bool {
        use crossterm::event::{MouseButton, MouseEventKind};
        let inner = Block::default().borders(Borders::ALL).inner(area);
        match kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(&(action, _)) = self
                    .button_rects
                    .iter()
                    .find(|(_, r)| col >= r.x && col < r.x + r.width && row == r.y)
                {
                    match action {
                        LayerButton::New => {
                            let w = stack.layers[0].buffer.width();
                            let h = stack.layers[0].buffer.height();
                            stack.add(w, h);
                        }
                        LayerButton::Duplicate => {
                            stack.duplicate(stack.active);
                        }
                        LayerButton::Delete => {
                            stack.delete(stack.active);
                        }
                        LayerButton::Group => {
                            let indices = [stack.active];
                            let group_name = format!("Group {}", stack.groups.len() + 1);
                            stack.create_group(&indices, group_name);
                        }
                        LayerButton::Link => {
                            self.toggle_link_pending(stack);
                        }
                    }
                    return true;
                }
                if let Some(idx) = self.layer_at_pos(col, row, area, stack) {
                    if col <= inner.x + 1 {
                        self.drag_state = Some((idx, idx));
                        self.drag_hover_row = None;
                        return true;
                    }
                    if stack.active != idx {
                        stack.set_active(idx);
                        return true;
                    }
                }
                false
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if let Some((from, _to)) = self.drag_state {
                    if let Some(idx) = self.layer_at_pos(col, row, area, stack) {
                        self.drag_hover_row = Some(idx);
                        if idx != from {
                            self.drag_state = Some((from, idx));
                        }
                    }
                    true
                } else {
                    false
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                if let Some((from, to)) = self.drag_state.take() {
                    self.drag_hover_row = None;
                    if from != to && to < stack.layers.len() {
                        stack.reorder(from, to);
                        return true;
                    }
                    return false;
                }
                false
            }
            _ => false,
        }
    }
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        format!(
            "{}…",
            s.chars()
                .take(max_len.saturating_sub(1))
                .collect::<String>()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::super::canvas::CanvasCell;
    use super::*;

    fn make_cell(ch: char) -> CanvasCell {
        CanvasCell {
            ch,
            fg: None,
            bg: None,
            height: None,
        }
    }

    fn make_stack(w: usize, h: usize) -> LayerStack {
        LayerStack::new(w, h)
    }

    #[test]
    fn test_new_layer() {
        let mut stack = make_stack(10, 10);
        assert_eq!(stack.len(), 1);
        assert_eq!(stack.active, 0);
        stack.add(10, 10);
        assert_eq!(stack.len(), 2);
        assert_eq!(stack.active, 1);
    }

    #[test]
    fn test_delete_layer() {
        let mut stack = make_stack(10, 10);
        stack.add(10, 10);
        stack.add(10, 10);
        assert_eq!(stack.len(), 3);
        stack.active = 1;
        assert!(stack.delete(1));
        assert_eq!(stack.len(), 2);
    }

    #[test]
    fn test_delete_cannot_remove_last() {
        let mut stack = make_stack(10, 10);
        assert!(!stack.delete(0));
        assert_eq!(stack.len(), 1);
    }

    #[test]
    fn test_duplicate_layer() {
        let mut stack = make_stack(10, 10);
        stack.layers[0].buffer.set(0, 0, make_cell('X'));
        assert!(stack.duplicate(0));
        assert_eq!(stack.len(), 2);
        assert_eq!(stack.layers[1].buffer.get(0, 0).unwrap().ch, 'X');
        stack.layers[1].buffer.set(0, 0, make_cell('Y'));
        assert_eq!(stack.layers[0].buffer.get(0, 0).unwrap().ch, 'X');
    }

    #[test]
    fn test_reorder_layers() {
        let mut stack = make_stack(10, 10);
        stack.add(10, 10);
        stack.add(10, 10);
        stack.layers[0].name = "A".to_string();
        stack.layers[1].name = "B".to_string();
        stack.layers[2].name = "C".to_string();
        assert!(stack.move_up(0));
        assert_eq!(stack.layers[0].name, "B");
        assert_eq!(stack.layers[1].name, "A");
        assert!(stack.move_down(1));
        assert_eq!(stack.layers[0].name, "A");
        assert_eq!(stack.layers[1].name, "B");
    }

    #[test]
    fn test_merge_down() {
        let mut stack = make_stack(5, 5);
        stack.add(5, 5);
        stack.layers[0].buffer.set(0, 0, make_cell('A'));
        stack.layers[1].buffer.set(1, 1, make_cell('B'));
        assert!(stack.merge_down(1));
        assert_eq!(stack.len(), 1);
        assert_eq!(stack.layers[0].buffer.get(0, 0).unwrap().ch, 'A');
        assert_eq!(stack.layers[0].buffer.get(1, 1).unwrap().ch, 'B');
    }

    #[test]
    fn test_merge_down_locked_top_noop() {
        let mut stack = make_stack(5, 5);
        stack.add(5, 5);
        stack.layers[1].locked = true;
        assert!(!stack.merge_down(1));
        assert_eq!(stack.len(), 2);
    }

    #[test]
    fn test_composite_visibility() {
        let mut stack = make_stack(3, 3);
        stack.add(3, 3);
        stack.layers[0].buffer.set(0, 0, make_cell('A'));
        stack.layers[1].buffer.set(1, 1, make_cell('B'));
        stack.layers[1].visible = false;
        let comp = stack.composite();
        assert_eq!(comp.get(0, 0).unwrap().ch, 'A');
        assert_eq!(comp.get(1, 1).unwrap().ch, ' ');
    }

    #[test]
    fn test_composite_order() {
        let mut stack = make_stack(3, 3);
        stack.add(3, 3);
        stack.layers[0].buffer.set(0, 0, make_cell('A'));
        stack.layers[1].buffer.set(0, 0, make_cell('B'));
        let comp = stack.composite();
        assert_eq!(comp.get(0, 0).unwrap().ch, 'B');
    }

    #[test]
    fn test_composite_opacity() {
        let mut stack = make_stack(3, 3);
        stack.add(3, 3);
        stack.layers[0].buffer.set(0, 0, make_cell('A'));
        stack.layers[1].buffer.set(0, 0, make_cell('B'));
        stack.layers[1].opacity = 128;
        let comp = stack.composite();
        assert_eq!(comp.get(0, 0).unwrap().ch, 'B');
    }

    #[test]
    fn test_active_mut_independence() {
        let mut stack = make_stack(5, 5);
        stack.add(5, 5);
        stack.active = 1;
        stack.active_layer_mut().buffer.set(0, 0, make_cell('X'));
        assert_eq!(stack.layers[1].buffer.get(0, 0).unwrap().ch, 'X');
        assert_eq!(stack.layers[0].buffer.get(0, 0).unwrap().ch, ' ');
    }

    #[test]
    fn test_resize_all() {
        let mut stack = make_stack(5, 5);
        stack.add(5, 5);
        stack.resize_all(10, 10);
        assert_eq!(stack.layers[0].buffer.width(), 10);
        assert_eq!(stack.layers[0].buffer.height(), 10);
        assert_eq!(stack.layers[1].buffer.width(), 10);
    }

    #[test]
    fn test_composite_empty_stack() {
        let mut stack = make_stack(1, 1);
        stack.layers.clear();
        let comp = stack.composite();
        assert_eq!(comp.width(), 1);
        assert_eq!(comp.height(), 1);
    }

    #[test]
    fn test_toggle_visibility() {
        let mut stack = make_stack(5, 5);
        assert!(stack.layers[0].visible);
        stack.toggle_visibility(0);
        assert!(!stack.layers[0].visible);
        stack.toggle_visibility(0);
        assert!(stack.layers[0].visible);
    }

    #[test]
    fn test_toggle_lock() {
        let mut stack = make_stack(5, 5);
        assert!(!stack.layers[0].locked);
        stack.toggle_lock(0);
        assert!(stack.layers[0].locked);
        stack.toggle_lock(0);
        assert!(!stack.layers[0].locked);
    }

    #[test]
    fn test_set_opacity() {
        let mut stack = make_stack(5, 5);
        stack.set_opacity(0, 128);
        assert_eq!(stack.layers[0].opacity, 128);
        stack.set_opacity(0, 255);
        assert_eq!(stack.layers[0].opacity, 255);
    }

    #[test]
    fn test_move_up_edge() {
        let mut stack = make_stack(5, 5);
        assert!(!stack.move_up(0));
    }

    #[test]
    fn test_move_down_edge() {
        let mut stack = make_stack(5, 5);
        assert!(!stack.move_down(0));
    }

    #[test]
    fn test_reorder_invalid() {
        let mut stack = make_stack(5, 5);
        stack.add(5, 5);
        assert!(!stack.reorder(0, 0));
    }

    #[test]
    fn test_with_capacity() {
        let stack = LayerStack::with_capacity(10, 10, 3);
        assert_eq!(stack.len(), 3);
        assert_eq!(stack.layers[0].name, "Background");
        assert_eq!(stack.layers[1].name, "Layer 2");
        assert_eq!(stack.layers[2].name, "Layer 3");
    }

    // --- Blend mode tests ---

    fn rgb_cell(ch: char, r: u8, g: u8, b: u8) -> CanvasCell {
        CanvasCell {
            ch,
            fg: Some(Color::Rgb(r, g, b)),
            bg: None,
            height: None,
        }
    }

    #[test]
    fn test_blend_multiply() {
        let top = rgb_cell('X', 200, 100, 50);
        let bottom = rgb_cell('Y', 100, 200, 50);
        let result = blend_mode_color(top.fg, bottom.fg, BlendMode::Multiply);
        match result {
            Some(Color::Rgb(r, g, b)) => {
                assert_eq!(r, 78);
                assert_eq!(g, 78);
                assert_eq!(b, 9);
            }
            _ => panic!("expected Some(Color::Rgb)"),
        }
    }

    #[test]
    fn test_blend_overlay_dark() {
        let top = rgb_cell('X', 200, 50, 50);
        let bottom = rgb_cell('Y', 50, 100, 150);
        let result = blend_mode_color(top.fg, bottom.fg, BlendMode::Overlay);
        match result {
            Some(Color::Rgb(r, g, b)) => {
                assert_eq!(r, 78);
                assert_eq!(g, 39);
                assert_eq!(b, 87);
            }
            _ => panic!("expected Some(Color::Rgb)"),
        }
    }

    #[test]
    fn test_blend_overlay_light() {
        let top = rgb_cell('X', 200, 50, 50);
        let bottom = rgb_cell('Y', 200, 100, 50);
        let result = blend_mode_color(top.fg, bottom.fg, BlendMode::Overlay);
        match result {
            Some(Color::Rgb(r, g, b)) => {
                assert_eq!(r, 232);
                assert_eq!(g, 39);
                assert_eq!(b, 19);
            }
            _ => panic!("expected Some(Color::Rgb)"),
        }
    }

    #[test]
    fn test_blend_screen() {
        let top = rgb_cell('X', 200, 100, 50);
        let bottom = rgb_cell('Y', 100, 200, 50);
        let result = blend_mode_color(top.fg, bottom.fg, BlendMode::Screen);
        match result {
            Some(Color::Rgb(r, g, b)) => {
                assert_eq!(r, 222);
                assert_eq!(g, 222);
                assert_eq!(b, 91);
            }
            _ => panic!("expected Some(Color::Rgb)"),
        }
    }

    #[test]
    fn test_blend_add() {
        let top = rgb_cell('X', 200, 100, 200);
        let bottom = rgb_cell('Y', 100, 200, 50);
        let result = blend_mode_color(top.fg, bottom.fg, BlendMode::Add);
        match result {
            Some(Color::Rgb(r, g, b)) => {
                assert_eq!(r, 255);
                assert_eq!(g, 255);
                assert_eq!(b, 250);
            }
            _ => panic!("expected Some(Color::Rgb)"),
        }
    }

    #[test]
    fn test_blend_subtract() {
        let top = rgb_cell('X', 200, 100, 50);
        let bottom = rgb_cell('Y', 100, 200, 50);
        let result = blend_mode_color(top.fg, bottom.fg, BlendMode::Subtract);
        match result {
            Some(Color::Rgb(r, g, b)) => {
                assert_eq!(r, 0);
                assert_eq!(g, 100);
                assert_eq!(b, 0);
            }
            _ => panic!("expected Some(Color::Rgb)"),
        }
    }

    #[test]
    fn test_composite_blend_mode() {
        let mut stack = make_stack(3, 3);
        stack.add(3, 3);
        stack.layers[0]
            .buffer
            .set(0, 0, rgb_cell('A', 200, 100, 50));
        stack.layers[1]
            .buffer
            .set(0, 0, rgb_cell('B', 100, 200, 50));
        stack.layers[1].blend_mode = BlendMode::Multiply;
        let comp = stack.composite();
        let cell = comp.get(0, 0).unwrap();
        assert_eq!(cell.ch, 'B');
        match cell.fg {
            Some(Color::Rgb(r, g, b)) => {
                assert_eq!(r, 78);
                assert_eq!(g, 78);
                assert_eq!(b, 9);
            }
            _ => panic!("expected Some(Color::Rgb)"),
        }
    }

    #[test]
    fn test_blend_mode_cycle() {
        let mut mode = BlendMode::Normal;
        mode = mode.next();
        assert_eq!(mode, BlendMode::Multiply);
        mode = mode.next();
        assert_eq!(mode, BlendMode::Overlay);
        mode = mode.next();
        assert_eq!(mode, BlendMode::Screen);
        mode = mode.next();
        assert_eq!(mode, BlendMode::Add);
        mode = mode.next();
        assert_eq!(mode, BlendMode::Subtract);
        mode = mode.next();
        assert_eq!(mode, BlendMode::Normal);
        // prev cycle
        mode = mode.prev();
        assert_eq!(mode, BlendMode::Subtract);
        mode = mode.prev();
        assert_eq!(mode, BlendMode::Add);
        mode = mode.prev();
        assert_eq!(mode, BlendMode::Screen);
        mode = mode.prev();
        assert_eq!(mode, BlendMode::Overlay);
        mode = mode.prev();
        assert_eq!(mode, BlendMode::Multiply);
        mode = mode.prev();
        assert_eq!(mode, BlendMode::Normal);
    }

    #[test]
    fn test_set_blend_mode() {
        let mut stack = make_stack(5, 5);
        assert_eq!(stack.layers[0].blend_mode, BlendMode::Normal);
        stack.set_blend_mode(0, BlendMode::Multiply);
        assert_eq!(stack.layers[0].blend_mode, BlendMode::Multiply);
        stack.set_blend_mode(0, BlendMode::Overlay);
        assert_eq!(stack.layers[0].blend_mode, BlendMode::Overlay);
    }

    #[test]
    fn test_blend_mode_icon_key() {
        assert_eq!(BlendMode::Normal.icon_key(), "blend_normal");
        assert_eq!(BlendMode::Multiply.icon_key(), "blend_multiply");
        assert_eq!(BlendMode::Overlay.icon_key(), "blend_overlay");
        assert_eq!(BlendMode::Screen.icon_key(), "blend_screen");
        assert_eq!(BlendMode::Add.icon_key(), "blend_add");
        assert_eq!(BlendMode::Subtract.icon_key(), "blend_subtract");
    }

    #[test]
    fn test_blend_mode_display_name() {
        assert_eq!(BlendMode::Normal.display_name(), "Normal");
        assert_eq!(BlendMode::Multiply.display_name(), "Multiply");
        assert_eq!(BlendMode::Overlay.display_name(), "Overlay");
        assert_eq!(BlendMode::Screen.display_name(), "Screen");
        assert_eq!(BlendMode::Add.display_name(), "Add");
        assert_eq!(BlendMode::Subtract.display_name(), "Subtract");
    }

    #[test]
    fn test_blend_mode_normal_returns_top() {
        let top = rgb_cell('X', 100, 150, 200);
        let bottom = rgb_cell('Y', 50, 100, 150);
        let result = blend_mode_color(top.fg, bottom.fg, BlendMode::Normal);
        assert_eq!(result, top.fg);
    }

    #[test]
    fn test_composite_blend_mode_with_opacity() {
        let mut stack = make_stack(3, 3);
        stack.add(3, 3);
        stack.layers[0]
            .buffer
            .set(0, 0, rgb_cell('A', 200, 100, 50));
        stack.layers[1]
            .buffer
            .set(0, 0, rgb_cell('B', 100, 200, 50));
        stack.layers[1].blend_mode = BlendMode::Multiply;
        stack.layers[1].opacity = 128;
        let comp = stack.composite();
        let cell = comp.get(0, 0).unwrap();
        assert_eq!(cell.ch, 'B');
        // With opacity=128, result is lerp between bottom and multiply blend
        match cell.fg {
            Some(Color::Rgb(r, g, b)) => {
                assert_eq!(r, 139);
                assert_eq!(g, 89);
                assert_eq!(b, 29);
            }
            _ => panic!("expected Some(Color::Rgb)"),
        }
    }

    // --- Layer group tests ---

    #[test]
    fn test_create_group() {
        let mut stack = make_stack(10, 10);
        stack.add(10, 10);
        stack.add(10, 10);
        let group_idx = stack.create_group(&[0, 1], "My Group".to_string());
        assert!(group_idx.is_some());
        let g = group_idx.unwrap();
        assert_eq!(stack.groups.len(), 1);
        assert_eq!(stack.groups[g].name, "My Group");
        assert!(!stack.groups[g].collapsed);
        assert_eq!(stack.layers[0].group, Some(g));
        assert_eq!(stack.layers[1].group, Some(g));
        assert!(stack.layers[2].group.is_none());
    }

    #[test]
    fn test_remove_group() {
        let mut stack = make_stack(10, 10);
        stack.add(10, 10);
        let g = stack.create_group(&[0, 1], "G".to_string()).unwrap();
        assert!(stack.remove_group(g));
        assert!(stack.layers[0].group.is_none());
        assert!(stack.layers[1].group.is_none());
        assert_eq!(stack.groups.len(), 0);
    }

    #[test]
    fn test_toggle_group_collapsed() {
        let mut stack = make_stack(10, 10);
        let g = stack.create_group(&[0], "G".to_string()).unwrap();
        assert!(!stack.groups[g].collapsed);
        assert!(stack.toggle_group_collapsed(g));
        assert!(stack.groups[g].collapsed);
        assert!(stack.toggle_group_collapsed(g));
        assert!(!stack.groups[g].collapsed);
    }

    // --- Layer link tests ---

    #[test]
    fn test_link_layers() {
        let mut stack = make_stack(10, 10);
        stack.add(10, 10);
        stack.add(10, 10);
        let link_idx = stack.link_layers(&[0, 1]);
        assert!(link_idx.is_some());
        let l = link_idx.unwrap();
        assert_eq!(stack.links.len(), 1);
        assert_eq!(stack.layers[0].link, Some(l));
        assert_eq!(stack.layers[1].link, Some(l));
        assert!(stack.layers[2].link.is_none());
    }

    #[test]
    fn test_link_layers_requires_at_least_two() {
        let mut stack = make_stack(10, 10);
        assert!(stack.link_layers(&[0]).is_none());
        assert!(stack.link_layers(&[]).is_none());
        assert!(stack.links.is_empty());
    }

    #[test]
    fn test_link_layers_invalid_index() {
        let mut stack = make_stack(10, 10);
        assert!(stack.link_layers(&[0, 5]).is_none());
        assert!(stack.links.is_empty());
    }

    #[test]
    fn test_unlink_layers() {
        let mut stack = make_stack(10, 10);
        stack.add(10, 10);
        let l = stack.link_layers(&[0, 1]).unwrap();
        assert!(stack.unlink_layers(l));
        assert!(stack.layers[0].link.is_none());
        assert!(stack.layers[1].link.is_none());
        assert!(stack.links.is_empty());
    }

    #[test]
    fn test_link_index_shifted_after_unlink() {
        let mut stack = make_stack(10, 10);
        stack.add(10, 10);
        stack.add(10, 10);
        stack.add(10, 10);
        let l0 = stack.link_layers(&[0, 1]).unwrap();
        let l1 = stack.link_layers(&[2, 3]).unwrap();
        assert_eq!(l0, 0);
        assert_eq!(l1, 1);
        assert!(stack.unlink_layers(l0));
        assert_eq!(stack.layers[2].link, Some(0));
        assert_eq!(stack.layers[3].link, Some(0));
    }

    #[test]
    fn test_relinking_moves_layer_out_of_old_link() {
        let mut stack = make_stack(10, 10);
        stack.add(10, 10);
        stack.add(10, 10);
        stack.link_layers(&[0, 1]).unwrap();
        let new_link = stack.link_layers(&[0, 2]).unwrap();
        assert_eq!(stack.layers[1].link, None, "layer 1 should be unlinked");
        assert_eq!(stack.layers[0].link, Some(new_link));
        assert_eq!(stack.layers[2].link, Some(new_link));
        assert_eq!(stack.links.len(), 1, "old (now-empty) link should be gone");
    }

    #[test]
    fn test_toggle_visibility_propagates_to_linked_layers() {
        let mut stack = make_stack(10, 10);
        stack.add(10, 10);
        stack.link_layers(&[0, 1]);
        assert!(stack.layers[0].visible);
        assert!(stack.layers[1].visible);
        stack.toggle_visibility(0);
        assert!(!stack.layers[0].visible);
        assert!(
            !stack.layers[1].visible,
            "linked layer's visibility should follow"
        );
        stack.toggle_visibility(1);
        assert!(stack.layers[0].visible);
        assert!(stack.layers[1].visible);
    }

    #[test]
    fn test_toggle_lock_propagates_to_linked_layers() {
        let mut stack = make_stack(10, 10);
        stack.add(10, 10);
        stack.link_layers(&[0, 1]);
        assert!(!stack.layers[0].locked);
        stack.toggle_lock(0);
        assert!(stack.layers[0].locked);
        assert!(stack.layers[1].locked, "linked layer's lock should follow");
    }

    #[test]
    fn test_toggle_visibility_unlinked_layer_does_not_propagate() {
        let mut stack = make_stack(10, 10);
        stack.add(10, 10);
        stack.toggle_visibility(0);
        assert!(!stack.layers[0].visible);
        assert!(stack.layers[1].visible);
    }

    #[test]
    fn test_layer_panel_link_pending_via_keyboard() {
        let mut stack = make_stack(10, 10);
        stack.add(10, 10);
        let mut panel = LayerPanel::new();

        stack.active = 0;
        let k = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE);
        assert!(panel.handle_key(k, &mut stack));
        assert_eq!(panel.link_pending, Some(0));

        stack.active = 1;
        assert!(panel.handle_key(k, &mut stack));
        assert_eq!(panel.link_pending, None);
        assert_eq!(stack.layers[0].link, stack.layers[1].link);
        assert!(stack.layers[0].link.is_some());
    }

    #[test]
    fn test_rename_layer() {
        let mut stack = make_stack(10, 10);
        assert!(stack.rename(0, "Sketch".to_string()));
        assert_eq!(stack.layers[0].name, "Sketch");
    }

    #[test]
    fn test_rename_layer_nonexistent() {
        let mut stack = make_stack(10, 10);
        assert!(!stack.rename(5, "X".to_string()));
    }

    #[test]
    fn test_rename_layer_empty_name_noop() {
        let mut stack = make_stack(10, 10);
        assert!(!stack.rename(0, "   ".to_string()));
        assert_eq!(stack.layers[0].name, "Background");
    }

    #[test]
    fn test_layer_panel_rename_via_f2() {
        let mut stack = make_stack(10, 10);
        let mut panel = LayerPanel::new();

        let f2 = KeyEvent::new(KeyCode::F(2), KeyModifiers::NONE);
        assert!(panel.handle_key(f2, &mut stack));
        assert_eq!(panel.renaming, Some(0));
        assert_eq!(panel.rename_buffer, "Background");

        for c in "Sky".chars() {
            let key = KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE);
            assert!(panel.handle_key(key, &mut stack));
        }
        assert_eq!(panel.rename_buffer, "BackgroundSky");

        let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        assert!(panel.handle_key(enter, &mut stack));
        assert_eq!(stack.layers[0].name, "BackgroundSky");
        assert_eq!(panel.renaming, None);
    }

    #[test]
    fn test_handle_mouse_selects_correct_layer_with_content_area() {
        use crossterm::event::{MouseButton, MouseEventKind};

        let mut stack = make_stack(10, 10);
        stack.add(10, 10); // index 0 "Background", index 1 "Layer 2"
        stack.active = 0;
        let mut panel = LayerPanel::new();

        // Mirrors SidePanel::content_area: a rect already offset past the
        // side panel's own border + tab bar, exactly as it's rendered. The
        // panel then draws one more border of its own inside this rect.
        let area = Rect::new(0, 5, 20, 10);
        // inner.y = area.y + 1 = 6; row offset = 1 => first layer row = 7.
        let row = 7;
        let col = 5;
        assert!(panel.handle_mouse(
            col,
            row,
            MouseEventKind::Down(MouseButton::Left),
            area,
            &mut stack
        ));
        assert_eq!(
            stack.active, 1,
            "clicking the first visible row should select the topmost layer (index 1), not be off by the tab-bar offset"
        );
    }

    #[test]
    fn test_layer_panel_rename_esc_cancels() {
        let mut stack = make_stack(10, 10);
        let mut panel = LayerPanel::new();
        panel.handle_key(KeyEvent::new(KeyCode::F(2), KeyModifiers::NONE), &mut stack);
        panel.handle_key(
            KeyEvent::new(KeyCode::Char('X'), KeyModifiers::NONE),
            &mut stack,
        );
        panel.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &mut stack);
        assert_eq!(panel.renaming, None);
        assert_eq!(stack.layers[0].name, "Background");
    }

    #[test]
    fn test_rename_group() {
        let mut stack = make_stack(10, 10);
        let g = stack.create_group(&[0], "Old".to_string()).unwrap();
        assert!(stack.rename_group(g, "New".to_string()));
        assert_eq!(stack.groups[g].name, "New");
    }

    #[test]
    fn test_group_of_layer() {
        let mut stack = make_stack(10, 10);
        stack.add(10, 10);
        let g = stack.create_group(&[1], "G".to_string()).unwrap();
        assert_eq!(stack.group_of_layer(1), Some(g));
        assert_eq!(stack.group_of_layer(0), None);
    }

    #[test]
    fn test_layers_in_group() {
        let mut stack = make_stack(10, 10);
        stack.add(10, 10);
        stack.add(10, 10);
        let g = stack.create_group(&[0, 2], "G".to_string()).unwrap();
        let members = stack.layers_in_group(g);
        assert_eq!(members.len(), 2);
        assert!(members.contains(&0));
        assert!(members.contains(&2));
    }

    #[test]
    fn test_create_group_empty_indices() {
        let mut stack = make_stack(10, 10);
        assert!(stack.create_group(&[], "Empty".to_string()).is_none());
        assert_eq!(stack.groups.len(), 0);
    }

    #[test]
    fn test_create_group_invalid_index() {
        let mut stack = make_stack(10, 10);
        assert!(stack.create_group(&[0, 5], "Bad".to_string()).is_none());
        assert_eq!(stack.groups.len(), 0);
    }

    #[test]
    fn test_remove_group_nonexistent() {
        let mut stack = make_stack(10, 10);
        assert!(!stack.remove_group(0));
    }

    #[test]
    fn test_toggle_group_collapsed_nonexistent() {
        let mut stack = make_stack(10, 10);
        assert!(!stack.toggle_group_collapsed(0));
    }

    #[test]
    fn test_rename_group_nonexistent() {
        let mut stack = make_stack(10, 10);
        assert!(!stack.rename_group(0, "X".to_string()));
    }

    #[test]
    fn test_group_preserved_after_delete() {
        let mut stack = make_stack(10, 10);
        stack.add(10, 10);
        stack.add(10, 10);
        let g = stack.create_group(&[0, 1], "G".to_string()).unwrap();
        stack.delete(0);
        assert_eq!(stack.layers[0].group, Some(g));
        assert_eq!(stack.groups.len(), 1);
    }

    #[test]
    fn test_group_index_shifted_after_remove() {
        let mut stack = make_stack(10, 10);
        stack.add(10, 10);
        stack.add(10, 10);
        let g0 = stack.create_group(&[0], "G0".to_string()).unwrap();
        let g1 = stack.create_group(&[1, 2], "G1".to_string()).unwrap();
        assert_eq!(g0, 0);
        assert_eq!(g1, 1);
        stack.remove_group(0);
        assert_eq!(stack.layers[0].group, None);
        assert_eq!(stack.layers[1].group, Some(0));
        assert_eq!(stack.layers[2].group, Some(0));
    }

    // --- Layer mask tests ---

    #[test]
    fn test_create_mask() {
        let mut stack = make_stack(10, 10);
        assert!(stack.layers[0].mask.is_none());
        assert!(stack.create_mask(0));
        assert!(stack.layers[0].mask.is_some());
        let mask = stack.layers[0].mask.as_ref().unwrap();
        assert!(mask.enabled);
        assert_eq!(mask.buffer.width(), 10);
        assert_eq!(mask.buffer.height(), 10);
    }

    #[test]
    fn test_create_mask_on_nonexistent_layer() {
        let mut stack = make_stack(10, 10);
        assert!(!stack.create_mask(5));
    }

    #[test]
    fn test_create_mask_twice_noop() {
        let mut stack = make_stack(10, 10);
        assert!(stack.create_mask(0));
        assert!(!stack.create_mask(0));
    }

    #[test]
    fn test_remove_mask() {
        let mut stack = make_stack(10, 10);
        stack.create_mask(0);
        assert!(stack.remove_mask(0));
        assert!(stack.layers[0].mask.is_none());
    }

    #[test]
    fn test_remove_mask_nonexistent() {
        let mut stack = make_stack(10, 10);
        assert!(!stack.remove_mask(0));
    }

    #[test]
    fn test_toggle_mask() {
        let mut stack = make_stack(10, 10);
        assert!(stack.toggle_mask(0));
        assert!(stack.layers[0].mask.is_some());
        assert!(stack.toggle_mask(0));
        assert!(stack.layers[0].mask.is_none());
    }

    #[test]
    fn test_layer_mask_toggle_enabled() {
        let mut stack = make_stack(10, 10);
        stack.create_mask(0);
        assert!(stack.layers[0].mask.as_ref().unwrap().enabled);
        assert!(stack.toggle_mask_enabled(0));
        assert!(!stack.layers[0].mask.as_ref().unwrap().enabled);
        assert!(stack.toggle_mask_enabled(0));
        assert!(stack.layers[0].mask.as_ref().unwrap().enabled);
    }

    #[test]
    fn test_mask_paint() {
        let mut stack = make_stack(10, 10);
        stack.create_mask(0);
        let cell = make_cell('X');
        stack.set_mask_pixel(0, 3, 4, cell);
        let got = stack.get_mask_pixel(0, 3, 4);
        assert!(got.is_some());
        assert_eq!(got.unwrap().ch, 'X');
    }

    #[test]
    fn test_set_mask_pixel_out_of_bounds() {
        let mut stack = make_stack(5, 5);
        stack.create_mask(0);
        stack.set_mask_pixel(0, 100, 100, make_cell('X'));
    }

    #[test]
    fn test_set_mask_pixel_no_mask() {
        let mut stack = make_stack(5, 5);
        stack.set_mask_pixel(0, 0, 0, make_cell('X'));
    }

    #[test]
    fn test_composite_with_mask() {
        let mut stack = make_stack(3, 3);
        stack.layers[0].buffer.set(0, 0, make_cell('A'));
        stack.create_mask(0);
        // Mask is all spaces by default -> layer is fully hidden
        let comp = stack.composite();
        assert_eq!(comp.get(0, 0).unwrap().ch, ' ');
    }

    #[test]
    fn test_composite_with_mask_painted() {
        let mut stack = make_stack(3, 3);
        stack.layers[0].buffer.set(0, 0, make_cell('A'));
        stack.create_mask(0);
        // Paint mask pixel at (0,0) to reveal it
        stack.set_mask_pixel(0, 0, 0, make_cell('▓'));
        let comp = stack.composite();
        assert_eq!(comp.get(0, 0).unwrap().ch, 'A');
    }

    #[test]
    fn test_composite_with_mask_disabled() {
        let mut stack = make_stack(3, 3);
        stack.layers[0].buffer.set(0, 0, make_cell('A'));
        stack.create_mask(0);
        stack.layers[0].mask.as_mut().unwrap().enabled = false;
        let comp = stack.composite();
        assert_eq!(comp.get(0, 0).unwrap().ch, 'A');
    }

    #[test]
    fn test_composite_two_layers_with_mask() {
        let mut stack = make_stack(3, 3);
        stack.add(3, 3);
        // Bottom layer has content at (0,0) and (1,1)
        stack.layers[0].buffer.set(0, 0, make_cell('A'));
        stack.layers[0].buffer.set(1, 1, make_cell('B'));
        // Top layer has content at (0,0) with mask revealing only part
        stack.layers[1].buffer.set(0, 0, make_cell('C'));
        stack.create_mask(1);
        stack.set_mask_pixel(1, 0, 0, make_cell('▓'));
        let comp = stack.composite();
        // (0,0): top layer visible through mask => 'C'
        assert_eq!(comp.get(0, 0).unwrap().ch, 'C');
        // (1,1): only bottom layer => 'B'
        assert_eq!(comp.get(1, 1).unwrap().ch, 'B');
    }

    // ── Frozen frames tests ──────────────────────────────────

    #[test]
    fn test_add_frozen_frames_to_layer_stack() {
        let mut stack = make_stack(5, 5);
        let initial_count = stack.len();

        let mut buf_a = CanvasBuffer::new(5, 5);
        buf_a.set(0, 0, make_cell('A'));
        let mut buf_b = CanvasBuffer::new(5, 5);
        buf_b.set(1, 1, make_cell('B'));
        let mut buf_c = CanvasBuffer::new(5, 5);
        buf_c.set(2, 2, make_cell('C'));

        let indices = stack.add_frozen_frames(vec![buf_a, buf_b, buf_c], "snapshot");
        assert_eq!(indices.len(), 3);
        assert_eq!(stack.len(), initial_count + 3);

        // Verify names
        assert_eq!(stack.layers[initial_count].name, "snapshot frame 0");
        assert_eq!(stack.layers[initial_count + 1].name, "snapshot frame 1");
        assert_eq!(stack.layers[initial_count + 2].name, "snapshot frame 2");

        // Verify independent content
        assert_eq!(
            stack.layers[initial_count].buffer.get(0, 0).unwrap().ch,
            'A'
        );
        assert_eq!(
            stack.layers[initial_count + 1].buffer.get(1, 1).unwrap().ch,
            'B'
        );
        assert_eq!(
            stack.layers[initial_count + 2].buffer.get(2, 2).unwrap().ch,
            'C'
        );

        // Verify independence: mutating one doesn't affect others
        stack
            .layers
            .get_mut(initial_count)
            .unwrap()
            .buffer
            .set(0, 0, make_cell(' '));
        assert_eq!(
            stack.layers[initial_count + 1].buffer.get(1, 1).unwrap().ch,
            'B'
        );
    }
}
