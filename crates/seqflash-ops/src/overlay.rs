//! Record-level edit overlay (plan section 18.2).
//!
//! The source file is always read-only. Edits are stored in an in-memory
//! [`EditOverlay`] keyed by record number. The overlay supports undo/redo
//! without corrupting edit ordering.

use std::collections::BTreeMap;

use seqflash_types::Revision;

/// Default maximum record size allowed for direct editing (plan 18.3).
pub const RECORD_EDIT_LIMIT_BYTES: u64 = 64 * 1024 * 1024;

/// One record-level edit operation (plan 18.2).
#[derive(Clone, Debug)]
pub enum RecordEdit {
    Delete { record_number: u64 },
    Replace { record_number: u64, data: Vec<u8> },
    InsertBefore { record_number: u64, data: Vec<u8> },
    InsertAfter { record_number: u64, data: Vec<u8> },
}

impl RecordEdit {
    /// The record number this edit targets.
    #[must_use]
    pub const fn record_number(&self) -> u64 {
        match self {
            Self::Delete { record_number }
            | Self::Replace { record_number, .. }
            | Self::InsertBefore { record_number, .. }
            | Self::InsertAfter { record_number, .. } => *record_number,
        }
    }

    /// True if this edit deletes the record entirely.
    #[must_use]
    pub const fn is_delete(&self) -> bool {
        matches!(self, Self::Delete { .. })
    }

    /// The replacement data, if this is a Replace/Insert edit.
    #[must_use]
    pub fn data(&self) -> Option<&[u8]> {
        match self {
            Self::Replace { data, .. }
            | Self::InsertBefore { data, .. }
            | Self::InsertAfter { data, .. } => Some(data),
            Self::Delete { .. } => None,
        }
    }
}

/// In-memory edit overlay for one document.
///
/// Edits are stored in a `BTreeMap<u64, Vec<RecordEdit>>` keyed by record
/// number. An undo/redo stack tracks the history of applied edits so the user
/// can reverse operations without corrupting the overlay's ordering.
#[derive(Clone, Debug)]
pub struct EditOverlay {
    edits: BTreeMap<u64, Vec<RecordEdit>>,
    revision: Revision,
    undo_stack: Vec<RecordEdit>,
    redo_stack: Vec<RecordEdit>,
}

impl Default for EditOverlay {
    fn default() -> Self {
        Self::new()
    }
}

impl EditOverlay {
    /// Create an empty overlay.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            edits: BTreeMap::new(),
            revision: Revision::initial(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    /// Apply an edit to the overlay. Bumps revision and pushes to undo stack.
    /// Clears the redo stack (a new edit invalidates redo history).
    pub fn apply(&mut self, edit: RecordEdit) {
        let rn = edit.record_number();
        self.edits.entry(rn).or_default().push(edit.clone());
        self.undo_stack.push(edit);
        self.redo_stack.clear();
        self.revision = self.revision.bump();
    }

    /// Undo the last applied edit. Returns `true` if an edit was undone.
    pub fn undo(&mut self) -> bool {
        let Some(edit) = self.undo_stack.pop() else {
            return false;
        };
        // Remove the edit from the overlay's edits map.
        let rn = edit.record_number();
        if let Some(vec) = self.edits.get_mut(&rn) {
            // Remove the last edit matching this record (the one we just popped).
            if let Some(pos) = vec.iter().rposition(|e| matches_same_type(e, &edit)) {
                vec.remove(pos);
            }
            if vec.is_empty() {
                self.edits.remove(&rn);
            }
        }
        self.redo_stack.push(edit);
        self.revision = self.revision.bump();
        true
    }

    /// Redo the last undone edit. Returns `true` if an edit was redone.
    pub fn redo(&mut self) -> bool {
        let Some(edit) = self.redo_stack.pop() else {
            return false;
        };
        let rn = edit.record_number();
        self.edits.entry(rn).or_default().push(edit.clone());
        self.undo_stack.push(edit);
        self.revision = self.revision.bump();
        true
    }

    /// True if there are unsaved edits.
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        !self.edits.is_empty()
    }

    /// Current revision (bumped on every apply/undo/redo).
    #[must_use]
    pub const fn revision(&self) -> Revision {
        self.revision
    }

    /// Get the edits for a specific record number.
    #[must_use]
    pub fn edits_for(&self, record_number: u64) -> Option<&Vec<RecordEdit>> {
        self.edits.get(&record_number)
    }

    /// Iterate over all (record_number, edits) pairs in order.
    pub fn iter(&self) -> impl Iterator<Item = (&u64, &Vec<RecordEdit>)> {
        self.edits.iter()
    }

    /// Number of distinct records that have edits.
    #[must_use]
    pub fn edited_record_count(&self) -> usize {
        self.edits.len()
    }

    /// Can undo?
    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Can redo?
    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Clear all edits (e.g. after a successful save).
    pub fn clear(&mut self) {
        self.edits.clear();
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.revision = self.revision.bump();
    }
}

/// Check if two edits are the same type targeting the same record.
fn matches_same_type(a: &RecordEdit, b: &RecordEdit) -> bool {
    matches!(
        (a, b),
        (RecordEdit::Delete { .. }, RecordEdit::Delete { .. })
            | (RecordEdit::Replace { .. }, RecordEdit::Replace { .. })
            | (
                RecordEdit::InsertBefore { .. },
                RecordEdit::InsertBefore { .. }
            )
            | (
                RecordEdit::InsertAfter { .. },
                RecordEdit::InsertAfter { .. }
            )
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn apply_and_dirty() {
        let mut ov = EditOverlay::new();
        assert!(!ov.is_dirty());
        ov.apply(RecordEdit::Delete { record_number: 2 });
        assert!(ov.is_dirty());
        assert_eq!(ov.edited_record_count(), 1);
    }

    #[test]
    fn undo_redo() {
        let mut ov = EditOverlay::new();
        ov.apply(RecordEdit::Delete { record_number: 1 });
        ov.apply(RecordEdit::Replace {
            record_number: 2,
            data: b"new".to_vec(),
        });
        assert!(ov.can_undo());
        assert!(!ov.can_redo());

        // Undo both
        assert!(ov.undo());
        assert_eq!(ov.edited_record_count(), 1); // only record 1 left
        assert!(ov.undo());
        assert!(!ov.is_dirty());

        // Redo both
        assert!(ov.can_redo());
        assert!(ov.redo());
        assert!(ov.redo());
        assert!(ov.is_dirty());
        assert_eq!(ov.edited_record_count(), 2);
    }

    #[test]
    fn new_edit_clears_redo() {
        let mut ov = EditOverlay::new();
        ov.apply(RecordEdit::Delete { record_number: 0 });
        ov.undo();
        assert!(ov.can_redo());
        ov.apply(RecordEdit::Delete { record_number: 1 });
        assert!(!ov.can_redo()); // redo cleared
    }

    #[test]
    fn revision_bumps() {
        let mut ov = EditOverlay::new();
        let r0 = ov.revision();
        ov.apply(RecordEdit::Delete { record_number: 0 });
        let r1 = ov.revision();
        assert_ne!(r0, r1);
        ov.undo();
        let r2 = ov.revision();
        assert_ne!(r1, r2);
    }

    #[test]
    fn clear_resets() {
        let mut ov = EditOverlay::new();
        ov.apply(RecordEdit::Delete { record_number: 0 });
        ov.apply(RecordEdit::Replace {
            record_number: 1,
            data: vec![],
        });
        ov.clear();
        assert!(!ov.is_dirty());
        assert!(!ov.can_undo());
        assert!(!ov.can_redo());
    }

    #[test]
    fn insert_edits() {
        let mut ov = EditOverlay::new();
        ov.apply(RecordEdit::InsertBefore {
            record_number: 0,
            data: b"before".to_vec(),
        });
        ov.apply(RecordEdit::InsertAfter {
            record_number: 0,
            data: b"after".to_vec(),
        });
        let edits = ov.edits_for(0).unwrap();
        assert_eq!(edits.len(), 2);
    }

    #[test]
    fn multiple_edits_same_record() {
        let mut ov = EditOverlay::new();
        ov.apply(RecordEdit::Delete { record_number: 0 });
        // Undo and re-apply to same record
        ov.undo();
        ov.apply(RecordEdit::Replace {
            record_number: 0,
            data: b"x".to_vec(),
        });
        assert_eq!(ov.edited_record_count(), 1);
    }
}
