//! Search types: mode, request, result.

use seqflash_types::{ByteRange, DocumentId, Revision};

/// Default maximum number of results retained (plan 16.3).
pub const MAX_RESULTS: usize = 10_000;

/// The kind of search to perform (plan 16.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SearchMode {
    /// Exact match on a record ID.
    RecordIdExact,
    /// Prefix match on a record ID.
    RecordIdPrefix,
    /// Raw byte search across the entire file.
    RawBytes,
    /// Sequence-fragment search (byte search including newlines).
    SequenceFragment,
    /// Search within the current record only.
    CurrentRecord,
    /// Search forward from the current viewport offset.
    FromPosition,
}

/// A search request (plan 16.2).
#[derive(Clone, Debug)]
pub struct SearchRequest {
    pub document_id: DocumentId,
    pub revision: Revision,
    pub mode: SearchMode,
    pub pattern: Vec<u8>,
    pub start_offset: u64,
    pub case_sensitive: bool,
    pub max_results: usize,
}

/// One search match (plan 16.3).
#[derive(Clone, Debug)]
pub struct SearchResult {
    /// Byte range of the match in the file.
    pub byte_range: ByteRange,
    /// Record number containing the match, if the index is available.
    pub record_number: Option<u64>,
    /// A short preview of the surrounding bytes.
    pub preview: Vec<u8>,
}

impl SearchResult {
    /// Build a result with a preview of up to `preview_len` bytes centered on
    /// the match.
    pub(crate) fn with_preview(
        bytes: &[u8],
        range: ByteRange,
        rec: Option<u64>,
        preview_len: usize,
    ) -> Self {
        let start = usize::try_from(range.start).unwrap_or(0).min(bytes.len());
        let end = usize::try_from(range.end)
            .unwrap_or(bytes.len())
            .min(bytes.len());
        let ctx_start = start.saturating_sub(preview_len / 2);
        let ctx_end = (end + preview_len / 2).min(bytes.len());
        let preview = bytes[ctx_start..ctx_end].to_vec();
        Self {
            byte_range: range,
            record_number: rec,
            preview,
        }
    }
}
