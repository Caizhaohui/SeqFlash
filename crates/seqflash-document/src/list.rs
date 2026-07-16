//! The collection of open documents (the basis for multi-tab support).

use std::path::Path;

use seqflash_types::DocumentId;

use crate::document::Document;
use crate::error::DocumentError;

/// All currently open documents, each addressed by a unique [`DocumentId`].
///
/// IDs are assigned monotonically and never reused, so a stale [`DocumentId`]
/// (from a closed document) safely resolves to `None`.
#[derive(Default)]
pub struct DocumentList {
    docs: Vec<Document>,
    next_id: u64,
}

impl DocumentList {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            docs: Vec::new(),
            next_id: 0,
        }
    }

    /// Number of open documents.
    #[must_use]
    pub fn len(&self) -> usize {
        self.docs.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.docs.is_empty()
    }

    /// Open `path` read-only and add it to the list.
    ///
    /// On failure no document is added.
    ///
    /// # Errors
    /// See [`Document::open`].
    pub fn open(&mut self, path: &Path) -> Result<DocumentId, DocumentError> {
        let id = DocumentId::new(self.next_id);
        self.next_id += 1;
        let document = Document::open(path, id)?;
        self.docs.push(document);
        Ok(id)
    }

    /// Close the document with the given id, releasing its memory map.
    ///
    /// Returns `true` if a document was removed.
    pub fn close(&mut self, id: DocumentId) -> bool {
        let before = self.docs.len();
        self.docs.retain(|d| d.id() != id);
        self.docs.len() != before
    }

    /// Look up a document by id.
    #[must_use]
    pub fn get(&self, id: DocumentId) -> Option<&Document> {
        self.docs.iter().find(|d| d.id() == id)
    }

    /// Look up a document by id, mutably.
    #[must_use]
    pub fn get_mut(&mut self, id: DocumentId) -> Option<&mut Document> {
        self.docs.iter_mut().find(|d| d.id() == id)
    }

    /// Iterate over all open documents.
    pub fn iter(&self) -> impl Iterator<Item = &Document> {
        self.docs.iter()
    }

    /// Find the id of an already-open document by path, if any.
    ///
    /// Used to avoid opening the same file twice — re-activating the existing
    /// tab instead.
    #[must_use]
    pub fn find_by_path(&self, path: &Path) -> Option<DocumentId> {
        self.docs
            .iter()
            .find(|d| d.metadata().path == path)
            .map(Document::id)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    fn write_file(path: &std::path::Path, contents: &[u8]) {
        let mut f = File::create(path).unwrap();
        f.write_all(contents).unwrap();
        f.sync_all().unwrap();
    }

    #[test]
    fn open_assigns_unique_incrementing_ids() {
        let dir = tempdir().unwrap();
        let p1 = dir.path().join("a.fa");
        let p2 = dir.path().join("b.fa");
        write_file(&p1, b"a\n");
        write_file(&p2, b"b\n");

        let mut list = DocumentList::new();
        let id1 = list.open(&p1).unwrap();
        let id2 = list.open(&p2).unwrap();

        assert_ne!(id1, id2);
        assert_eq!(id1.get() + 1, id2.get());
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn close_releases_and_returns_false_for_unknown() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("a.fa");
        write_file(&p, b"a\n");

        let mut list = DocumentList::new();
        let id = list.open(&p).unwrap();
        assert_eq!(list.len(), 1);

        assert!(list.close(id));
        assert!(list.is_empty());
        assert!(list.get(id).is_none());

        // Closing again (or an unknown id) is a no-op.
        assert!(!list.close(id));
    }

    #[test]
    fn find_by_path_locates_open_document() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("a.fa");
        write_file(&p, b"a\n");

        let mut list = DocumentList::new();
        let id = list.open(&p).unwrap();

        assert_eq!(list.find_by_path(&p), Some(id));
        let other = dir.path().join("other.fa");
        assert_eq!(list.find_by_path(&other), None);
    }

    #[test]
    fn open_failure_does_not_add() {
        let dir = tempdir().unwrap();
        let missing = dir.path().join("nope.fa");

        let mut list = DocumentList::new();
        assert!(list.open(&missing).is_err());
        assert!(list.is_empty());
    }
}
