//! Shared base types for the SeqFlash workspace.
//!
//! This crate intentionally depends on nothing and performs no I/O. It only
//! holds the small, stable value types that several other crates need to agree
//! on — identifiers, byte ranges, and the high-level file format enums.
//!
//! See `DEVELOPMENT_PLAN.md` section 9.1 for the intent and constraints:
//! no GUI, no file I/O, no scanning, no background threads, no Windows API.

#![forbid(unsafe_code)]

/// Identifies a single open document inside the running application.
///
/// IDs are assigned by the application layer and only need to be unique within
/// a single process lifetime.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DocumentId(pub u64);

impl DocumentId {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Identifies a background job running against a document.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct JobId(pub u64);

impl JobId {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// A monotonically increasing revision of a document's logical content.
///
/// The application bumps this whenever the document's view of its own data
/// changes (e.g. an edit overlay is applied). Background jobs record the
/// revision they were started with and discard their results if the document
/// has since moved on — see `DEVELOPMENT_PLAN.md` section 21.4.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Revision(pub u64);

impl Revision {
    #[must_use]
    pub const fn initial() -> Self {
        Self(0)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Advance to the next revision, returning the new value.
    #[must_use]
    pub const fn bump(self) -> Self {
        Self(self.0 + 1)
    }

    /// True if this revision still matches the document's current revision,
    /// i.e. results computed against `self` are still valid.
    #[must_use]
    pub const fn is_current(self, current: Revision) -> bool {
        self.0 == current.0
    }
}

/// A half-open byte range `[start, end)` within a file or buffer.
///
/// All offsets are absolute file offsets (`u64`). This matches the project's
/// "byte-first, `u64` everywhere" data model (plan section 10.2).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ByteRange {
    /// Inclusive start offset.
    pub start: u64,
    /// Exclusive end offset. Must satisfy `end >= start`.
    pub end: u64,
}

impl ByteRange {
    /// Create a new `[start, end)` range.
    ///
    /// Returns `None` if `end < start`.
    #[must_use]
    pub const fn new(start: u64, end: u64) -> Option<Self> {
        if end >= start {
            Some(Self { start, end })
        } else {
            None
        }
    }

    /// Length of the range in bytes. Returns `0` for an empty range.
    #[must_use]
    pub const fn len(self) -> u64 {
        self.end.saturating_sub(self.start)
    }

    /// True when the range covers zero bytes (`start == end`).
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }

    /// True if `offset` falls within `[start, end)`.
    #[must_use]
    pub const fn contains(self, offset: u64) -> bool {
        offset >= self.start && offset < self.end
    }
}

/// The high-level sequence file format of a document.
///
/// `Unknown` is a first-class value: the application must be able to open and
/// show files it cannot positively identify (plan section 11 / 13.1).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub enum SequenceFormat {
    /// FASTA.
    Fasta,
    /// FASTQ.
    Fastq,
    /// Could not be positively identified; still viewable as raw text.
    #[default]
    Unknown,
}

impl SequenceFormat {
    /// Lowercase, human-readable label suitable for the status bar.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Fasta => "FASTA",
            Self::Fastq => "FASTQ",
            Self::Unknown => "Unknown",
        }
    }
}

/// The newline convention observed in (part of) a file.
///
/// `Mixed` and `Unknown` exist because real-world files are messy and the
/// viewer must never assume "always LF" or "always CRLF" (plan section 9.3).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub enum NewlineStyle {
    /// Unix line feeds (`\n`).
    Lf,
    /// Windows carriage return + line feed (`\r\n`).
    CrLf,
    /// The file contains a mixture of LF and CRLF.
    Mixed,
    /// Not yet determined.
    #[default]
    Unknown,
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::needless_pass_by_value
)]
mod tests {
    use super::*;

    #[test]
    fn document_id_roundtrips() {
        let id = DocumentId::new(42);
        assert_eq!(id.get(), 42);
        assert_eq!(id, DocumentId(42));
        assert_ne!(id, DocumentId(43));
    }

    #[test]
    fn revision_bump_and_current() {
        let r0 = Revision::initial();
        assert_eq!(r0.get(), 0);
        let r1 = r0.bump();
        assert_eq!(r1.get(), 1);
        assert!(r0.is_current(r0));
        assert!(!r0.is_current(r1));
    }

    #[test]
    fn byte_range_basic() {
        let r = ByteRange::new(10, 20).expect("valid range");
        assert_eq!(r.len(), 10);
        assert!(!r.is_empty());
        assert!(r.contains(10));
        assert!(r.contains(19));
        assert!(!r.contains(20));
        assert!(!r.contains(9));
    }

    #[test]
    fn byte_range_empty_and_invalid() {
        let empty = ByteRange::new(7, 7).expect("empty range");
        assert!(empty.is_empty());
        assert_eq!(empty.len(), 0);
        // Inverted ranges are rejected, not silently fixed.
        assert!(ByteRange::new(20, 10).is_none());
    }

    #[test]
    fn sequence_format_default_and_labels() {
        assert_eq!(SequenceFormat::default(), SequenceFormat::Unknown);
        assert_eq!(SequenceFormat::Fasta.label(), "FASTA");
        assert_eq!(SequenceFormat::Fastq.label(), "FASTQ");
        assert_eq!(SequenceFormat::Unknown.label(), "Unknown");
    }

    #[test]
    fn newline_style_default_is_unknown() {
        assert_eq!(NewlineStyle::default(), NewlineStyle::Unknown);
    }
}
