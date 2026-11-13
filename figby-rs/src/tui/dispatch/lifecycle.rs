//! Undo/redo and document transitions (new, close, switch, quit) behind the unsaved-changes prompt.

use super::*;

impl TuiApp {
    /// Undo the most recent action, restoring into the RECORDED layer.
    /// The redo entry captures that layer's current buffer so redo is
    /// exact even across layer switches (GPT review F-09).
    pub(crate) fn perform_undo(&mut self) {
        if !self.editor.undo.can_undo() {
            return;
        }
        let target = match self.editor.undo.undo_top_layer() {
            Some(i) => i,
            None => return,
        };
        if target >= self.editor.layer_stack.layers.len() {
            // Layer was deleted after the snapshot; drop the stale entry.
            let _ = self.editor.undo.undo(canvas::CanvasBuffer::new(0, 0), 0);
            return;
        }
        let cur = self.editor.layer_stack.layers[target].buffer.clone();
        if let Some((buf, idx, _)) = self.editor.undo.undo(cur, target) {
            if self.editor.restore_undo_snapshot(idx, buf) {
                self.editor.mark_dirty();
            }
        }
    }

    /// Redo counterpart of [`Self::perform_undo`].
    pub(crate) fn perform_redo(&mut self) {
        if !self.editor.undo.can_redo() {
            return;
        }
        let target = match self.editor.undo.redo_top_layer() {
            Some(i) => i,
            None => return,
        };
        if target >= self.editor.layer_stack.layers.len() {
            let _ = self.editor.undo.redo(canvas::CanvasBuffer::new(0, 0), 0);
            return;
        }
        let cur = self.editor.layer_stack.layers[target].buffer.clone();
        if let Some((buf, idx, _)) = self.editor.undo.redo(cur, target) {
            if self.editor.restore_undo_snapshot(idx, buf) {
                self.editor.mark_dirty();
            }
        }
    }

    /// Route a destructive transition through the unsaved-changes dialog
    /// when needed (GPT review F-08). Clean documents proceed straight
    /// through.
    pub fn request_transition(&mut self, action: PendingDocAction) {
        if !self.editor.unsaved {
            self.execute_action(action);
            return;
        }
        self.dialogs.pending_transition = Some(action);
        self.dialogs.quit_confirm_dialog = true;
        self.frame.dirty = true;
    }

    /// Close a document tab, prompting through the unsaved-changes dialog
    /// when that tab has unsaved work (8.5.3). Clean tabs close at once.
    pub fn request_close_document(&mut self, idx: usize) {
        if idx >= self.document_count() {
            return;
        }
        let unsaved = if idx == self.active_doc {
            self.editor.unsaved
        } else {
            self.documents[idx].editor.unsaved
        };
        if !unsaved {
            let _ = self.close_document(idx);
            return;
        }
        self.dialogs.pending_transition = Some(PendingDocAction::CloseDocument(idx));
        self.dialogs.quit_confirm_dialog = true;
        self.frame.dirty = true;
    }

    /// Immediately perform a transition (no confirmation). For OpenFont
    /// this means the file picker, or the stashed path if one was already
    /// resolved.
    pub(crate) fn execute_action(&mut self, action: PendingDocAction) {
        match action {
            PendingDocAction::Quit => self.ui.should_quit = true,
            PendingDocAction::NewFileSession => self.do_new_file_session(self.ui.session_type),
            PendingDocAction::FontNewBlankSession => self.do_new_file_session(SessionType::Font),
            PendingDocAction::CloseDocument(idx) => {
                let _ = self.finish_close_document(idx);
            }
        }
    }

    /// Fresh blank document for the given session type (welcome "New").
    pub(crate) fn do_new_file_session(&mut self, session_type: SessionType) {
        // The new session gets its own tab, replacing the startup document
        // only if that one is still untouched (never an edited one).
        let kind = match session_type {
            SessionType::Image => crate::tui::documents::DocumentKind::Image,
            _ => crate::tui::documents::DocumentKind::Font,
        };
        self.new_document_or_reuse_pristine(kind);
        self.reset_document(32, 16);
        self.welcome.screen.show = false;
        self.welcome.fx = None;
        self.frame.dirty = true;
    }
}
