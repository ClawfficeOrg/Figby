//! Multi-document model (todo-v8 8.5.1).
//!
//! `TuiApp` keeps `editor`/`animation`/`lighting` as real live fields so
//! existing call sites compile unchanged; `documents` holds one
//! [`Document`] per open file and tab switches hot-swap the live fields
//! with the parked slot via [`TuiApp::switch_document`]. The active slot
//! always holds a placeholder — live state is only ever in the hot
//! fields — so per-document titles for the active tab are derived from
//! the live editor, never read from the stale slot.
//!
//! Deviations from the todo-v8 sketch, all deliberate:
//! - No `path`/`unsaved` on `Document`: both derive from editor state
//!   (`font_editor.current_path`, `editor.unsaved`), so they can't go
//!   stale. `title` is frozen at switch-out for inactive tabs and
//!   live-derived for the active one.
//! - `mode`/`prev_mode` stay declared on `UiState` (zero call-site churn)
//!   and are synced into/out of the slot on switch — behaviorally the
//!   same as moving them.

use std::collections::BTreeMap;

use super::{brush, canvas, font_editor, image_editor, layers, lighting, palette, particles};
use super::{theme, timeline, toolbox, tools, undo};
use super::{AnimationState, AppMode, EditorState, LightingState, SessionType, TuiApp};

/// Which kind of content a document holds. Fixed at creation; drives the
/// default mode, the tab icon (8.5.2), and the Tab-cycle session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentKind {
    Font,
    Image,
}

impl DocumentKind {
    /// Mode a fresh document of this kind opens in.
    pub fn default_mode(self) -> AppMode {
        match self {
            DocumentKind::Font => AppMode::FontEditor,
            DocumentKind::Image => AppMode::ImageEditor,
        }
    }

    /// Tab-cycle session for this kind — kept in sync onto
    /// `UiState::session_type` whenever the active document changes, so
    /// the existing `next_for`/`prev_for` call sites behave per-document
    /// without being touched.
    pub fn session_type(self) -> SessionType {
        match self {
            DocumentKind::Font => SessionType::Font,
            DocumentKind::Image => SessionType::Image,
        }
    }

    /// Icon key into the shared icon map for the tab strip.
    pub fn icon_key(self) -> &'static str {
        match self {
            DocumentKind::Font => "mode_font_editor",
            DocumentKind::Image => "mode_image_editor",
        }
    }
}

/// One open file: parked per-document state plus the metadata the tab
/// strip needs. `editor`/`animation`/`lighting` hold this document's live
/// state while parked, or a placeholder while it is active (live state is
/// then in `TuiApp`'s hot fields).
pub struct Document {
    pub kind: DocumentKind,
    /// Display name, frozen when the document is parked. For the active
    /// document use `TuiApp::doc_title`, which derives it live.
    pub title: String,
    pub mode: AppMode,
    pub prev_mode: AppMode,
    pub editor: EditorState,
    pub animation: AnimationState,
    pub lighting: LightingState,
}

impl Document {
    /// Fresh document with themed states, mirroring the per-document
    /// blocks of `TuiApp::new` (config overrides apply to the initial
    /// document only — extra tabs start from plain defaults).
    pub fn new(kind: DocumentKind, theme: &theme::Theme, icons: &BTreeMap<String, String>) -> Self {
        let mut canvas = canvas::CanvasWidget::default();
        canvas.theme = theme.clone();
        let mut palette = palette::Palette::new();
        palette.theme = theme.clone();
        let mut toolbox = toolbox::Toolbox::new();
        toolbox.theme = theme.clone();
        toolbox.icons = icons.clone();
        let mut font_editor = font_editor::FontEditor::new();
        font_editor.theme = theme.clone();

        let (w, h) = (canvas.buffer.width(), canvas.buffer.height());
        let mut layer_panel = layers::LayerPanel::new();
        layer_panel.theme = theme.clone();
        layer_panel.icons = icons.clone();

        let mut editor = EditorState {
            canvas,
            toolbox,
            brush: brush::BrushState::new(),
            palette,
            font_editor,
            image_editor: image_editor::ImageEditor::new(),
            text_tool: tools::text::TextToolState::new("fonts"),
            undo: undo::UndoSystem::new(50),
            unsaved: false,
            revision: 0,
            selection: None,
            clipboard: None,
            layer_stack: layers::LayerStack::new(w, h),
            layer_panel,
            fill_threshold: 0,
            eyedropper_sample: None,
            move_state: tools::move_tool::MoveState::default(),
            rotate_state: tools::rotate_tool::RotateState::default(),
            selection_state: tools::selection::SelectionState::default(),
            line_state: tools::line::LineState::default(),
            selection_polygon_points: Vec::new(),
        };
        editor.recomposite_canvas();

        let mode = kind.default_mode();
        Self {
            kind,
            title: String::from("Untitled"),
            mode,
            prev_mode: mode,
            editor,
            animation: AnimationState {
                timeline_state: timeline::TimelineState::default(),
                particle_system: particles::ParticleSystem::new(
                    particles::ParticleConfig::default(),
                ),
                emitter_active: false,
                emitter_panel: particles::EmitterConfigPanel::new(),
                show_live_particles: true,
                baked_layer_indices: Vec::new(),
                timeline_visible: false,
                marker_accum: std::collections::HashMap::new(),
                loop_enabled: false,
                inline_player: None,
                transport_rects: Vec::new(),
            },
            lighting: LightingState {
                scene: None,
                max_shadow_distance: 50,
                height_scale: 0.5,
                lut: lighting::LightingLut::from_palette(
                    (0, 0, 0),
                    (255, 255, 255),
                    crate::image_input::DEFAULT_CHAR_MAP,
                ),
                panel: super::LightPanel::new(),
                light_keyframes: Vec::new(),
            },
        }
    }

    /// Placeholder for the active slot: never rendered (live state is in
    /// the hot fields while parked here), only holding metadata. Uses the
    /// default theme; swapped out before anything can draw it.
    pub fn placeholder(kind: DocumentKind) -> Self {
        Self::new(kind, &theme::Theme::default(), &BTreeMap::new())
    }
}

/// Outcome of [`TuiApp::close_document`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseResult {
    Closed,
    /// Refused: the last document can't be closed.
    LastDocument,
    /// Refused: the target has unsaved changes (8.5.3 prompts first).
    HasUnsaved,
}

impl TuiApp {
    /// Live display title for a tab: derived from the hot editor for the
    /// active document (the slot copy is stale by design), frozen copy
    /// for parked ones.
    pub fn doc_title(&self, idx: usize) -> String {
        if idx >= self.documents.len() {
            return String::from("Untitled");
        }
        if idx == self.active_doc {
            Self::live_title(&self.editor)
        } else if self.documents[idx].title.is_empty() {
            String::from("Untitled")
        } else {
            self.documents[idx].title.clone()
        }
    }

    fn live_title(editor: &EditorState) -> String {
        let name = editor.font_editor.font_storage_name.trim();
        if name.is_empty() {
            String::from("Untitled")
        } else {
            name.to_string()
        }
    }

    pub fn document_count(&self) -> usize {
        self.documents.len()
    }

    /// Unsaved dot for a tab: live editor state for the active document,
    /// parked state otherwise.
    pub fn doc_unsaved(&self, idx: usize) -> bool {
        if idx >= self.documents.len() {
            return false;
        }
        if idx == self.active_doc {
            self.editor.unsaved
        } else {
            self.documents[idx].editor.unsaved
        }
    }

    /// Park the active document: sync UI metadata into its slot, then swap
    /// the live fields out. After this the hot fields hold a placeholder.
    fn park_active(&mut self) {
        let idx = self.active_doc;
        let title = Self::live_title(&self.editor);
        let doc = &mut self.documents[idx];
        doc.title = title;
        doc.mode = self.ui.mode;
        doc.prev_mode = self.ui.prev_mode;
        std::mem::swap(&mut self.editor, &mut doc.editor);
        std::mem::swap(&mut self.animation, &mut doc.animation);
        std::mem::swap(&mut self.lighting, &mut doc.lighting);
    }

    /// Load the active slot into the live fields and restore its UI
    /// metadata. Inverse of [`TuiApp::park_active`].
    fn unpark_active(&mut self) {
        let idx = self.active_doc;
        let doc = &mut self.documents[idx];
        std::mem::swap(&mut self.editor, &mut doc.editor);
        std::mem::swap(&mut self.animation, &mut doc.animation);
        std::mem::swap(&mut self.lighting, &mut doc.lighting);
        self.ui.mode = doc.mode;
        self.ui.prev_mode = doc.prev_mode;
        self.ui.session_type = doc.kind.session_type();
    }

    /// Switch tabs. No-op (returns `false`) for the active or an
    /// out-of-range index. The inline player travels inside
    /// `AnimationState`, so playback state is per-document automatically.
    pub fn switch_document(&mut self, idx: usize) -> bool {
        if idx >= self.documents.len() || idx == self.active_doc {
            return false;
        }
        self.park_active();
        self.active_doc = idx;
        self.unpark_active();
        self.frame.dirty = true;
        self.frame.force_full_redraw = true;
        true
    }

    /// Open a fresh document in a new tab and switch to it. Returns its index.
    pub fn new_document(&mut self, kind: DocumentKind) -> usize {
        self.park_active();
        let theme = self.ctx.theme.clone();
        let icons = self.ctx.icons.clone();
        self.documents.push(Document::new(kind, &theme, &icons));
        self.active_doc = self.documents.len() - 1;
        self.unpark_active();
        self.frame.dirty = true;
        self.frame.force_full_redraw = true;
        self.active_doc
    }

    /// Close a tab. The last document and unsaved documents are refused;
    /// prompting for the latter arrives with the 8.5.3 keybinding.
    pub fn close_document(&mut self, idx: usize) -> CloseResult {
        if self.documents.len() <= 1 || idx >= self.documents.len() {
            return CloseResult::LastDocument;
        }
        let unsaved = if idx == self.active_doc {
            self.editor.unsaved
        } else {
            self.documents[idx].editor.unsaved
        };
        if unsaved {
            return CloseResult::HasUnsaved;
        }
        if idx == self.active_doc {
            // Hot fields hold the closed document: park them into its
            // slot so `remove` drops the right state, then load a neighbor.
            self.park_active();
            self.documents.remove(idx);
            self.active_doc = idx.min(self.documents.len() - 1);
            self.unpark_active();
        } else {
            self.documents.remove(idx);
            if idx < self.active_doc {
                self.active_doc -= 1;
            }
        }
        self.frame.dirty = true;
        self.frame.force_full_redraw = true;
        CloseResult::Closed
    }

    /// Confirmed close after Save/Discard in the unsaved-changes dialog:
    /// drops the unsaved flag (the user just approved losing or saving
    /// the changes) so [`TuiApp::close_document`]'s guard doesn't block
    /// the very close that was confirmed.
    pub fn finish_close_document(&mut self, idx: usize) -> CloseResult {
        if idx < self.documents.len() {
            if idx == self.active_doc {
                self.editor.unsaved = false;
            } else {
                self.documents[idx].editor.unsaved = false;
            }
        }
        self.close_document(idx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_switch_preserves_per_document_state() {
        let mut app = TuiApp::new();
        assert_eq!(app.document_count(), 1);
        // Dirty the first document, then open a second one.
        app.editor.unsaved = true;
        app.editor.brush.set_size(5);
        let second = app.new_document(DocumentKind::Image);
        assert_eq!(second, 1);
        assert_eq!(app.document_count(), 2);
        // Fresh doc: clean, default brush, image session.
        assert!(!app.editor.unsaved);
        assert_eq!(app.ui.mode, AppMode::ImageEditor);
        assert_eq!(app.ui.session_type, SessionType::Image);
        // Switch back: first doc's state restored.
        assert!(app.switch_document(0));
        assert!(app.editor.unsaved);
        assert_eq!(app.editor.brush.size, 5);
        assert_eq!(app.ui.session_type, SessionType::Font);
        // No-op cases.
        assert!(!app.switch_document(0));
        assert!(!app.switch_document(99));
    }

    #[test]
    fn test_close_document_guards_and_index_fixup() {
        let mut app = TuiApp::new();
        assert_eq!(app.close_document(0), CloseResult::LastDocument);
        app.new_document(DocumentKind::Image);
        app.new_document(DocumentKind::Font);
        assert_eq!(app.document_count(), 3);
        // Close the middle (parked) tab; active index shifts down.
        assert_eq!(app.active_doc, 2);
        assert_eq!(app.close_document(1), CloseResult::Closed);
        assert_eq!(app.document_count(), 2);
        assert_eq!(app.active_doc, 1);
        // Unsaved active doc is refused.
        app.editor.unsaved = true;
        assert_eq!(app.close_document(1), CloseResult::HasUnsaved);
        assert_eq!(app.document_count(), 2);
        // Saved: closing the active tab loads the neighbor.
        app.editor.unsaved = false;
        assert_eq!(app.close_document(1), CloseResult::Closed);
        assert_eq!(app.document_count(), 1);
        assert_eq!(app.active_doc, 0);
    }

    #[test]
    fn test_doc_title_live_for_active_frozen_for_parked() {
        let mut app = TuiApp::new();
        assert_eq!(app.doc_title(0), "Untitled");
        app.editor.font_editor.font_storage_name = String::from("banner");
        assert_eq!(app.doc_title(0), "banner");
        app.new_document(DocumentKind::Image);
        // First tab parked with the name it had at switch-out.
        assert_eq!(app.doc_title(0), "banner");
        assert_eq!(app.doc_title(1), "Untitled");
        assert_eq!(app.doc_title(99), "Untitled");
    }
}

#[cfg(test)]
mod roundtrip_tests {
    use super::super::{AppMode, TuiApp};
    use super::*;

    #[test]
    fn test_canvas_content_survives_switch_round_trip() {
        let mut app = TuiApp::new();
        let mark = crate::tui::canvas::CanvasCell {
            ch: 'Z',
            fg: None,
            bg: None,
            height: None,
        };
        app.editor.canvas.buffer.set(3, 2, mark);
        app.editor.font_editor.font_storage_name = String::from("first");
        app.new_document(DocumentKind::Image);
        // Fresh tab is blank and independent.
        assert_eq!(app.editor.canvas.buffer.get(3, 2).map(|c| c.ch), Some(' '));
        assert_eq!(app.ui.mode, AppMode::ImageEditor);
        // Switch back: canvas mark, name, and mode all restored.
        assert!(app.switch_document(0));
        assert_eq!(app.editor.canvas.buffer.get(3, 2).map(|c| c.ch), Some('Z'));
        assert_eq!(app.doc_title(0), "first");
        assert_eq!(app.ui.mode, AppMode::FontEditor);
    }
}
