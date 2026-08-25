use super::canvas::CanvasBuffer;

#[derive(Debug, Clone)]
pub struct UndoEntry {
    pub buffer: CanvasBuffer,
    pub label: String,
    /// Index of the layer this snapshot was taken from. Undo/redo must
    /// restore into THIS layer, not whichever layer happens to be active
    /// later — otherwise editing layer A, switching to B and undoing
    /// corrupts B with A's pixels (GPT review F-09).
    pub layer_index: usize,
}

pub struct UndoSystem {
    undo: Vec<UndoEntry>,
    redo: Vec<UndoEntry>,
    limit: usize,
    in_batch: bool,
    batch_has_snapshot: bool,
}

impl UndoSystem {
    pub fn new(limit: usize) -> Self {
        Self {
            undo: Vec::with_capacity(limit),
            redo: Vec::new(),
            limit,
            in_batch: false,
            batch_has_snapshot: false,
        }
    }

    pub fn push_snapshot(&mut self, buffer: CanvasBuffer, layer_index: usize, label: String) {
        if self.in_batch {
            if self.batch_has_snapshot {
                return;
            }
            self.batch_has_snapshot = true;
        }
        self.undo.push(UndoEntry {
            buffer,
            label,
            layer_index,
        });
        self.redo.clear();
        while self.undo.len() > self.limit {
            self.undo.remove(0);
        }
    }

    pub fn begin_batch(&mut self) {
        self.in_batch = true;
        self.batch_has_snapshot = false;
    }

    pub fn end_batch(&mut self) {
        self.in_batch = false;
        self.batch_has_snapshot = false;
    }

    /// Layer index of the entry that [`Self::undo`] would pop, so the
    /// caller can pass that layer's CURRENT buffer for the redo stack.
    pub fn undo_top_layer(&self) -> Option<usize> {
        self.undo.last().map(|e| e.layer_index)
    }

    /// Layer index of the entry that [`Self::redo`] would pop.
    pub fn redo_top_layer(&self) -> Option<usize> {
        self.redo.last().map(|e| e.layer_index)
    }

    /// Pop the most recent undo entry. Returns the snapshot buffer, the
    /// layer it belongs to, and the label. The caller must restore the
    /// buffer into `layer_index` — never into the currently-active layer.
    pub fn undo(
        &mut self,
        current_buffer: CanvasBuffer,
        current_layer_index: usize,
    ) -> Option<(CanvasBuffer, usize, String)> {
        let entry = self.undo.pop()?;
        self.redo.push(UndoEntry {
            buffer: current_buffer,
            label: "Redo".to_string(),
            layer_index: current_layer_index,
        });
        Some((entry.buffer, entry.layer_index, entry.label))
    }

    /// Pop the most recent redo entry (same contract as [`Self::undo`]).
    pub fn redo(
        &mut self,
        current_buffer: CanvasBuffer,
        current_layer_index: usize,
    ) -> Option<(CanvasBuffer, usize, String)> {
        let entry = self.redo.pop()?;
        self.undo.push(UndoEntry {
            buffer: current_buffer,
            label: "Redo".to_string(),
            layer_index: current_layer_index,
        });
        Some((entry.buffer, entry.layer_index, entry.label))
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.in_batch = false;
        self.batch_has_snapshot = false;
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn history_len(&self) -> usize {
        self.undo.len()
    }

    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }

    pub fn history_entries(&self) -> &[UndoEntry] {
        &self.undo
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_buffer(w: usize, h: usize) -> CanvasBuffer {
        CanvasBuffer::new(w, h)
    }

    #[test]
    fn test_new_empty() {
        let us = UndoSystem::new(50);
        assert!(!us.can_undo());
        assert!(!us.can_redo());
        assert_eq!(us.history_len(), 0);
        assert_eq!(us.redo_len(), 0);
    }

    #[test]
    fn test_push_undo() {
        let mut us = UndoSystem::new(50);
        let buf = make_buffer(5, 5);
        us.push_snapshot(buf, 0, "Brush".to_string());
        assert!(us.can_undo());
        assert!(!us.can_redo());
        assert_eq!(us.history_len(), 1);
    }

    #[test]
    fn test_undo_restores_buffer() {
        let mut us = UndoSystem::new(50);
        let before = make_buffer(5, 5);
        us.push_snapshot(before.clone(), 0, "Brush".to_string());
        let after = make_buffer(5, 5);
        let result = us.undo(after, 0);
        assert!(result.is_some());
        let (restored, _, _) = result.unwrap();
        assert_eq!(restored.width(), before.width());
        assert_eq!(restored.height(), before.height());
        assert!(us.can_redo());
        assert!(!us.can_undo());
    }

    #[test]
    fn test_undo_redo_cycle() {
        let mut us = UndoSystem::new(50);
        let buf_a = make_buffer(3, 3);
        us.push_snapshot(buf_a, 0, "Action 1".to_string());
        let buf_b = make_buffer(3, 3);
        let (restored, _, _) = us.undo(buf_b, 0).unwrap();
        assert_eq!(restored.width(), 3);
        assert!(us.can_redo());
        let buf_c = make_buffer(5, 5);
        let (redone, _, _) = us.redo(buf_c, 0).unwrap();
        assert_eq!(redone.width(), 3); // buf_b was 3x3
        assert!(!us.can_redo());
    }

    #[test]
    fn test_undo_multiple_actions() {
        let mut us = UndoSystem::new(50);
        us.push_snapshot(make_buffer(1, 1), 0, "1".to_string());
        us.push_snapshot(make_buffer(2, 2), 0, "2".to_string());
        us.push_snapshot(make_buffer(3, 3), 0, "3".to_string());
        assert_eq!(us.history_len(), 3);
        let cur = make_buffer(4, 4);
        let (buf3, _, _) = us.undo(cur, 0).unwrap();
        assert_eq!(buf3.width(), 3);
        assert_eq!(us.history_len(), 2);
        let cur2 = make_buffer(5, 5);
        let (buf2, _, _) = us.undo(cur2, 0).unwrap();
        assert_eq!(buf2.width(), 2);
        assert_eq!(us.history_len(), 1);
    }

    #[test]
    fn test_undo_limit_enforcement() {
        let mut us = UndoSystem::new(5);
        for i in 0..10 {
            us.push_snapshot(make_buffer((i + 1) as usize, 1), 0, i.to_string());
        }
        assert_eq!(us.history_len(), 5);
        let cur = make_buffer(1, 1);
        let (buf, _, _) = us.undo(cur, 0).unwrap();
        assert_eq!(buf.width(), 10);
    }

    #[test]
    fn test_undo_clears_redo() {
        let mut us = UndoSystem::new(50);
        us.push_snapshot(make_buffer(1, 1), 0, "1".to_string());
        let cur = make_buffer(2, 2);
        us.undo(cur, 0);
        assert!(us.can_redo());
        us.push_snapshot(make_buffer(3, 3), 0, "2".to_string());
        assert!(!us.can_redo());
    }

    #[test]
    fn test_clear() {
        let mut us = UndoSystem::new(50);
        us.push_snapshot(make_buffer(1, 1), 0, "1".to_string());
        us.push_snapshot(make_buffer(2, 2), 0, "2".to_string());
        us.clear();
        assert!(!us.can_undo());
        assert!(!us.can_redo());
        assert_eq!(us.history_len(), 0);
    }

    #[test]
    fn test_batch_first_pushes_rest_discarded() {
        let mut us = UndoSystem::new(50);
        us.begin_batch();
        us.push_snapshot(make_buffer(1, 1), 0, "first".to_string());
        us.push_snapshot(make_buffer(2, 2), 0, "second".to_string());
        us.push_snapshot(make_buffer(3, 3), 0, "third".to_string());
        us.end_batch();
        assert_eq!(us.history_len(), 1);
    }

    #[test]
    fn test_batch_no_snapshot_ok() {
        let mut us = UndoSystem::new(50);
        us.begin_batch();
        us.end_batch();
        assert_eq!(us.history_len(), 0);
        us.push_snapshot(make_buffer(1, 1), 0, "after".to_string());
        assert_eq!(us.history_len(), 1);
    }

    #[test]
    fn test_undo_on_empty_returns_none() {
        let mut us = UndoSystem::new(50);
        assert!(us.undo(make_buffer(1, 1), 0).is_none());
    }

    #[test]
    fn test_redo_on_empty_returns_none() {
        let mut us = UndoSystem::new(50);
        assert!(us.redo(make_buffer(1, 1), 0).is_none());
    }

    #[test]
    fn test_history_entries_order() {
        let mut us = UndoSystem::new(50);
        us.push_snapshot(make_buffer(1, 1), 0, "A".to_string());
        us.push_snapshot(make_buffer(2, 2), 0, "B".to_string());
        us.push_snapshot(make_buffer(3, 3), 0, "C".to_string());
        let entries = us.history_entries();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].label, "A");
        assert_eq!(entries[1].label, "B");
        assert_eq!(entries[2].label, "C");
    }

    #[test]
    fn test_batch_preserves_two_separate_batches() {
        let mut us = UndoSystem::new(50);
        us.begin_batch();
        us.push_snapshot(make_buffer(1, 1), 0, "batch1".to_string());
        us.end_batch();
        us.begin_batch();
        us.push_snapshot(make_buffer(2, 2), 0, "batch2".to_string());
        us.end_batch();
        assert_eq!(us.history_len(), 2);
    }

    #[test]
    fn test_redo_label() {
        let mut us = UndoSystem::new(50);
        us.push_snapshot(make_buffer(1, 1), 0, "draw".to_string());
        us.undo(make_buffer(2, 2), 0);
        let (buf, _, label) = us.redo(make_buffer(3, 3), 0).unwrap();
        assert_eq!(label, "Redo");
        assert_eq!(buf.width(), 2);
    }

    /// F-09 (GPT review): an undo entry must remember which layer it came
    /// from, independent of what is active when undo runs.
    #[test]
    fn test_undo_carries_layer_identity() {
        let mut us = UndoSystem::new(50);
        // Edit layer 2, then layer 5.
        us.push_snapshot(make_buffer(1, 1), 2, "L2 edit".to_string());
        us.push_snapshot(make_buffer(2, 2), 5, "L5 edit".to_string());

        // Undoing while layer 9 is active still targets layer 5's state.
        let (buf, idx, label) = us.undo(make_buffer(3, 3), 9).unwrap();
        assert_eq!(idx, 5, "undo must target the recorded layer");
        assert_eq!(label, "L5 edit");

        // Next undo targets layer 2; the caller passes layer 2's own
        // current buffer so redo can restore it into layer 2 later.
        assert_eq!(us.undo_top_layer(), Some(2));
        let (_, idx2, _) = us.undo(make_buffer(4, 4), 2).unwrap();
        assert_eq!(idx2, 2);

        // Redo re-applies the last-undone action: the layer-2 edit,
        // restoring into layer 2 regardless of what is active now.
        assert_eq!(us.redo_top_layer(), Some(2));
        let (_, idx3, _) = us.redo(make_buffer(6, 6), 9).unwrap();
        assert_eq!(idx3, 2);
        assert_eq!(buf.width(), 2);
    }
}
