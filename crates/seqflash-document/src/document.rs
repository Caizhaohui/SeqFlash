//! A single open document: its identity, metadata, read-only bytes, and
//! external-change fingerprint.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use seqflash_types::{DocumentId, Revision, SequenceFormat};

use crate::bytes::FileBytes;
use crate::error::DocumentError;

/// Snapshot of the size and modification time recorded when the file was
/// opened. Used by [`Document::has_external_changes`] to detect that the file
/// was modified on disk after it was opened.
#[derive(Clone, Debug)]
pub struct FileFingerprint {
    pub size: u64,
    pub modified: SystemTime,
}

/// File metadata captured at open time. Immutable for the document's lifetime.
#[derive(Clone, Debug)]
pub struct DocumentMetadata {
    /// Absolute path the document was opened from.
    pub path: PathBuf,
    /// File size in bytes at open time.
    pub size: u64,
    /// File modification time at open time.
    pub modified: SystemTime,
}

/// One open document in the application.
///
/// The original file is opened read-only and never modified. Its bytes are
/// served either from a memory map (large files) or an inline buffer (empty
/// files); see [`FileBytes`].
pub struct Document {
    id: DocumentId,
    metadata: DocumentMetadata,
    bytes: FileBytes,
    /// Format is detected by later milestones; M1 leaves it `Unknown`.
    format: SequenceFormat,
    revision: Revision,
    opened_fingerprint: FileFingerprint,
}

impl Document {
    /// Open `path` read-only and build a document around it.
    ///
    /// On any failure this returns `Err` and creates **no** partial document —
    /// callers must not see a half-initialized entry.
    ///
    /// # Errors
    ///
    /// - [`DocumentError::Open`] if the file cannot be opened.
    /// - [`DocumentError::Metadata`] if size / mtime cannot be read.
    /// - [`DocumentError::Mmap`] if the memory map cannot be created.
    pub fn open(path: &Path, id: DocumentId) -> Result<Self, DocumentError> {
        let file = File::open(path).map_err(DocumentError::Open)?;
        let fs_meta = file.metadata().map_err(DocumentError::Metadata)?;

        // `modified()` can fail on exotic filesystems; treat that as a metadata
        // read failure rather than panicking.
        let modified = fs_meta.modified().map_err(DocumentError::Metadata)?;
        let size = fs_meta.len();

        let bytes = FileBytes::from_file(&file, size).map_err(DocumentError::Mmap)?;

        let metadata = DocumentMetadata {
            path: path.to_path_buf(),
            size,
            modified,
        };
        let opened_fingerprint = FileFingerprint { size, modified };

        Ok(Self {
            id,
            metadata,
            bytes,
            format: SequenceFormat::Unknown,
            revision: Revision::initial(),
            opened_fingerprint,
        })
    }

    #[must_use]
    pub const fn id(&self) -> DocumentId {
        self.id
    }

    /// The document's bytes, as a zero-copy `&[u8]` view over the mapping.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    #[must_use]
    pub fn metadata(&self) -> &DocumentMetadata {
        &self.metadata
    }

    #[must_use]
    pub const fn format(&self) -> SequenceFormat {
        self.format
    }

    /// Set the detected format. Reserved for later milestones that detect
    /// FASTA/FASTQ; M1 keeps the default `Unknown`.
    pub fn set_format(&mut self, format: SequenceFormat) {
        self.format = format;
    }

    #[must_use]
    pub const fn revision(&self) -> Revision {
        self.revision
    }

    /// Re-read the file's metadata and report whether it changed on disk since
    /// it was opened (different size **or** modification time).
    ///
    /// # Errors
    ///
    /// Returns [`DocumentError::Open`] if the file can no longer be opened
    /// (e.g. it was deleted or moved).
    pub fn has_external_changes(&self) -> Result<bool, DocumentError> {
        let file = File::open(&self.metadata.path).map_err(DocumentError::Open)?;
        let meta = file.metadata().map_err(DocumentError::Metadata)?;
        let size = meta.len();
        let modified = meta.modified().map_err(DocumentError::Metadata)?;

        let changed =
            size != self.opened_fingerprint.size || modified != self.opened_fingerprint.modified;
        // `!=` on SystemTime never returns Err; durations only error when
        // subtracting backwards, which we never do here.
        Ok(changed)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::thread;
    use std::time::Duration;
    use tempfile::tempdir;

    fn doc_with(path: &std::path::Path, contents: &[u8]) -> Document {
        let mut f = File::create(path).unwrap();
        f.write_all(contents).unwrap();
        f.sync_all().unwrap();
        drop(f);
        Document::open(path, DocumentId::new(1)).unwrap()
    }

    #[test]
    fn open_reads_bytes_and_metadata() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("seq.fa");
        let doc = doc_with(&path, b">x\nACGT\n");

        assert_eq!(doc.id(), DocumentId::new(1));
        assert_eq!(doc.bytes(), b">x\nACGT\n");
        assert_eq!(doc.metadata().path, path);
        assert_eq!(doc.metadata().size, 8);
        assert_eq!(doc.format(), SequenceFormat::Unknown);
        assert_eq!(doc.revision(), Revision::initial());
    }

    #[test]
    fn open_empty_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("empty.fa");
        let doc = doc_with(&path, b"");

        assert!(doc.bytes().is_empty());
        assert_eq!(doc.metadata().size, 0);
    }

    #[test]
    fn open_missing_file_is_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nope.fa");
        let result = Document::open(&path, DocumentId::new(1));
        assert!(matches!(result, Err(DocumentError::Open(_))));
    }

    #[test]
    fn no_external_change_when_untouched() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("seq.fa");
        let doc = doc_with(&path, b"data\n");

        assert!(!doc.has_external_changes().unwrap());
    }

    #[test]
    fn detects_external_size_change() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("seq.fa");
        let doc = doc_with(&path, b"original\n");

        // Simulate an external edit that grows the file. Delete + recreate to
        // stay compatible with Windows, where a live mmap locks the file
        // against in-place writes.
        std::fs::remove_file(&path).unwrap();
        doc_with(&path, b"original plus more bytes\n");

        assert!(doc.has_external_changes().unwrap());
    }

    #[test]
    fn detects_external_mtime_change() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("seq.fa");
        let doc = doc_with(&path, b"same content\n");

        // Simulate an external edit: delete then recreate the file with the
        // same size but a strictly newer mtime. (On Windows a live mmap locks
        // the file against write+truncate, so we delete+recreate instead —
        // which a third-party editor's "save" may also do.)
        thread::sleep(Duration::from_millis(50));
        std::fs::remove_file(&path).unwrap();
        doc_with(&path, b"same content\n");

        assert!(doc.has_external_changes().unwrap());
    }

    #[test]
    fn external_change_on_deleted_file_is_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("gone.fa");
        let doc = doc_with(&path, b"x\n");

        std::fs::remove_file(&path).unwrap();
        assert!(matches!(
            doc.has_external_changes(),
            Err(DocumentError::Open(_))
        ));
    }
}
