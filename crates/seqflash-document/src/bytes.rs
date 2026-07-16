//! The read-only backing store for a document's bytes.
//!
//! Large files are served from a memory map without copying; empty files fall
//! back to a tiny inline buffer. Both variants deref to `&[u8]` so callers do
//! not need to care which one is in use.

use std::fs::File;
use std::ops::Deref;
use std::sync::Arc;

use memmap2::Mmap;

/// The bytes of an opened document.
///
/// Constructed via [`FileBytes::from_file`], which picks the right backend
/// based on the file size.
pub enum FileBytes {
    /// A read-only memory map of a non-empty file. Cheap to create regardless
    /// of file size — the OS pages data in on demand.
    Mmap(Mmap),
    /// An inline buffer, used for empty files where a mapping is pointless.
    Inline(Arc<[u8]>),
}

impl FileBytes {
    /// Build the byte store for `file`, which must already be open **read-only**.
    ///
    /// - Empty file → [`FileBytes::Inline`] with no bytes.
    /// - Non-empty file → [`FileBytes::Mmap`].
    ///
    /// This is the single `unsafe` site in the whole workspace; see the
    /// crate-root note and the SAFETY comment below.
    ///
    /// # Errors
    ///
    /// Returns an [`std::io::Error`] only if the memory map cannot be created
    /// (empty files never error — they use the inline path).
    #[allow(unsafe_code)] // one encapsulated mmap; see SAFETY below
    pub fn from_file(file: &File, len: u64) -> Result<Self, std::io::Error> {
        if len == 0 {
            // memmap2 tolerates empty files, but an inline buffer is version-
            // proof and avoids even attempting a mapping.
            return Ok(Self::Inline(Arc::from(Vec::new().into_boxed_slice())));
        }

        // SAFETY: `file` was opened read-only by the caller (`Document::open`)
        // and we never expose a writable handle to it. The only soundness risk
        // of a file-backed mmap is a concurrent external writer mutating or
        // truncating the file while the mapping is live; for SeqFlash's
        // read-only FASTA/FASTQ inputs that is an acceptable, documented
        // precondition, and external changes are surfaced separately via
        // `Document::has_external_changes`.
        let mmap = unsafe { Mmap::map(file)? };
        Ok(Self::Mmap(mmap))
    }
}

impl Deref for FileBytes {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &[u8] {
        match self {
            Self::Mmap(m) => m,
            Self::Inline(v) => v,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    fn open_readonly(path: &std::path::Path) -> File {
        File::open(path).unwrap_or_else(|e| panic!("open {path:?}: {e}"))
    }

    #[test]
    fn empty_file_uses_inline() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("empty.fa");
        File::create(&path).unwrap();

        let file = open_readonly(&path);
        let bytes = FileBytes::from_file(&file, 0).unwrap();

        assert!(matches!(bytes, FileBytes::Inline(_)));
        assert!(bytes.is_empty());
        assert_eq!(bytes.len(), 0);
    }

    #[test]
    fn non_empty_file_uses_mmap() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("seq.fa");
        let mut f = File::create(&path).unwrap();
        f.write_all(b">seq1\nACGTACGT\n").unwrap();
        f.sync_all().unwrap();
        drop(f);

        let file = open_readonly(&path);
        let len = file.metadata().unwrap().len();
        let bytes = FileBytes::from_file(&file, len).unwrap();

        assert!(matches!(bytes, FileBytes::Mmap(_)));
        assert_eq!(&bytes[..], b">seq1\nACGTACGT\n");
        assert_eq!(bytes.len(), usize::try_from(len).unwrap());
    }

    #[test]
    fn invalid_utf8_bytes_are_preserved() {
        // FASTA/FASTQ files are byte streams, not guaranteed UTF-8.
        let dir = tempdir().unwrap();
        let path = dir.path().join("binary.fa");
        let mut f = File::create(&path).unwrap();
        // 0xFF / 0xFE are invalid standalone UTF-8.
        f.write_all(&[b'>', b'x', b'\n', 0xFF, 0xFE, b'\n'])
            .unwrap();
        f.sync_all().unwrap();
        drop(f);

        let file = open_readonly(&path);
        let len = file.metadata().unwrap().len();
        let bytes = FileBytes::from_file(&file, len).unwrap();

        assert_eq!(&bytes[..], &[b'>', b'x', b'\n', 0xFF, 0xFE, b'\n']);
    }
}
