//! FASTA header parsing and ID extraction.
//!
//! A FASTA header line is `>` followed by an identifier and an optional
//! description, separated by whitespace. This module extracts byte ranges
//! (relative to the header content, i.e. excluding the leading `>`) so callers
//! can slice the original file bytes without copying.

use seqflash_types::ByteRange;

/// Parsed components of a single FASTA header line (the `>`-prefixed line).
///
/// All ranges are **relative offsets within `header_bytes`** passed to
/// [`parse_fasta_header`], where `header_bytes` is the full header line
/// *including* the leading `>` but excluding the trailing newline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FastaHeader {
    /// Range of the ID token (the first whitespace-delimited token after `>`).
    /// Relative to the start of `header_bytes` (so it starts at 1, past `>`).
    pub id_range: ByteRange,
    /// Range of the description (everything after the ID and its separating
    /// whitespace, trimmed). Empty when there is no description. Relative to
    /// the start of `header_bytes`.
    pub description_range: ByteRange,
}

/// Parse a FASTA header line into its ID and description byte ranges.
///
/// `header_bytes` is the full `>`-prefixed header line **without** the trailing
/// newline (the caller strips CR/LF first). The ranges returned are relative to
/// the start of `header_bytes`.
///
/// ID = the first run of non-whitespace bytes after the leading `>`.
/// Description = the remaining bytes after the ID, left-trimmed of whitespace.
///
/// Non-ASCII / invalid-UTF-8 bytes are handled transparently (byte comparison
/// only); a header with no description yields an empty `description_range`.
///
/// # Panics
/// Never. A `header_bytes` that does not start with `>` still yields a best-
/// effort parse (ID starting at byte 0).
#[must_use]
pub fn parse_fasta_header(header_bytes: &[u8]) -> FastaHeader {
    // Position past the leading '>'; tolerate a missing one.
    let mut pos = usize::from(header_bytes.first() == Some(&b'>'));

    // ID = run of non-whitespace starting at `pos`.
    let id_start = pos;
    while pos < header_bytes.len() && !is_header_whitespace(header_bytes[pos]) {
        pos += 1;
    }
    let id_end = pos;
    let id_range =
        ByteRange::new(id_start as u64, id_end as u64).unwrap_or(ByteRange { start: 0, end: 0 });

    // Skip the whitespace separating ID from description.
    while pos < header_bytes.len() && is_header_whitespace(header_bytes[pos]) {
        pos += 1;
    }
    let desc_start = pos;
    let desc_end = header_bytes.len();
    // Right-trim trailing CR (in case the caller didn't strip CRLF fully).
    let desc_end_trimmed = trim_trailing_cr(header_bytes, desc_start, desc_end);
    let description_range = if desc_end_trimmed > desc_start {
        ByteRange::new(desc_start as u64, desc_end_trimmed as u64)
            .unwrap_or(ByteRange { start: 0, end: 0 })
    } else {
        ByteRange { start: 0, end: 0 }
    };

    FastaHeader {
        id_range,
        description_range,
    }
}

/// Bytes treated as whitespace when splitting the header into ID/description.
fn is_header_whitespace(b: u8) -> bool {
    matches!(b, b' ' | b'\t')
}

/// Return `end` minus any trailing '\r' (for CRLF headers not fully stripped).
fn trim_trailing_cr(bytes: &[u8], start: usize, end: usize) -> usize {
    let mut e = end;
    while e > start && bytes[e - 1] == b'\r' {
        e -= 1;
    }
    e
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn id_str<'a>(header: &'a [u8], h: &FastaHeader) -> &'a [u8] {
        let s = usize::try_from(h.id_range.start).unwrap();
        let e = usize::try_from(h.id_range.end).unwrap();
        &header[s..e]
    }
    fn desc_str<'a>(header: &'a [u8], h: &FastaHeader) -> &'a [u8] {
        let s = usize::try_from(h.description_range.start).unwrap();
        let e = usize::try_from(h.description_range.end).unwrap();
        &header[s..e]
    }

    #[test]
    fn parses_id_and_description() {
        let h = parse_fasta_header(b">seq1 Some description here");
        assert_eq!(id_str(b">seq1 Some description here", &h), b"seq1");
        assert_eq!(
            desc_str(b">seq1 Some description here", &h),
            b"Some description here"
        );
    }

    #[test]
    fn parses_id_only() {
        let h = parse_fasta_header(b">seq1");
        assert_eq!(id_str(b">seq1", &h), b"seq1");
        assert!(h.description_range.is_empty());
    }

    #[test]
    fn id_stops_at_tab() {
        let h = parse_fasta_header(b">chr1\tdescription");
        assert_eq!(id_str(b">chr1\tdescription", &h), b"chr1");
    }

    #[test]
    fn empty_id_after_gt() {
        // "> " with just a space — id is empty.
        let h = parse_fasta_header(b"> ");
        assert!(h.id_range.is_empty());
    }

    #[test]
    fn strips_trailing_cr() {
        // CRLF header where the caller passed the full line including CR.
        let h = parse_fasta_header(b">seq1 description\r");
        assert_eq!(id_str(b">seq1 description\r", &h), b"seq1");
        assert_eq!(desc_str(b">seq1 description\r", &h), b"description");
    }

    #[test]
    fn handles_non_ascii_bytes() {
        // High bytes in the ID are preserved as-is (byte-level parsing).
        let header: &[u8] = b">\xff\xfe weird";
        let h = parse_fasta_header(header);
        assert_eq!(id_str(header, &h), b"\xff\xfe");
    }

    #[test]
    fn empty_header() {
        let h = parse_fasta_header(b">");
        assert!(h.id_range.is_empty());
        assert!(h.description_range.is_empty());
    }
}
