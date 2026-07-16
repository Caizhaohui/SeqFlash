//! FASTQ state-machine parser and record types.
//!
//! Per plan section 14.1 / 14.2, the FASTQ parser uses a **state machine** to
//! handle single/multi-line sequences and quality, CRLF/LF, truncation at EOF,
//! and empty records — it never assumes "strictly four lines".

use seqflash_types::ByteRange;

/// States of the FASTQ state machine (plan 14.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FastqParserState {
    ExpectHeader,
    ReadSequence,
    ExpectPlus,
    ReadQuality,
    Complete,
    Error,
}

/// Validation flags for a single FASTQ record (plan 14.4).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct FastqValidation {
    pub header_ok: bool,
    pub sequence_ok: bool,
    pub plus_ok: bool,
    pub quality_ok: bool,
    pub length_match: bool,
    pub truncated: bool,
    pub illegal_qual_char: bool,
    pub valid: bool,
}

/// One fully indexed FASTQ record (plan 14.3).
#[derive(Clone, Debug)]
pub struct FastqRecordEntry {
    pub record_number: u64,
    pub start_offset: u64,
    pub end_offset: u64,
    pub header_range: ByteRange,
    pub id_range: ByteRange,
    pub sequence_range: ByteRange,
    pub plus_range: ByteRange,
    pub quality_range: ByteRange,
    pub sequence_length: u64,
    pub quality_length: u64,
    pub validation: FastqValidation,
}

/// Parse one complete FASTQ record starting at `offset` in `bytes`.
///
/// Returns `(FastqRecordEntry, next_record_offset)` where `next_record_offset`
/// is the byte just past the completed record. The `record_number` is filled
/// by the caller.
///
/// The parser is robust: it handles single/multi-line sequences and quality,
/// CRLF/LF, empty records, and truncated files without panicking.
///
/// # Errors
///
/// Returns `Err` with a description only when the parser cannot identify a
/// valid FASTQ record header at `offset` (e.g. offset points to non-FASTQ
/// content). Parsing continues past structural issues when possible so the
/// caller gets partial data.
#[allow(clippy::too_many_lines)]
pub fn parse_single_record(
    bytes: &[u8],
    offset: usize,
    record_number: u64,
) -> Result<(FastqRecordEntry, usize), String> {
    let end = bytes.len();
    let mut i = offset;
    let mut state = FastqParserState::ExpectHeader;
    let mut v = FastqValidation::default();
    let mut seq_len: u64 = 0;
    let mut qual_len: u64 = 0;
    // Byte-range boundaries (absolute offsets).
    let mut hdr_start = 0u64;
    let mut hdr_end = 0u64;
    let mut id_start = 0u64;
    let mut id_end = 0u64;
    let mut seq_end = 0u64;
    let mut plus_start = 0u64;
    let mut plus_end = 0u64;
    let mut qual_end = 0u64;

    while i < end {
        let b = bytes[i];
        match state {
            FastqParserState::ExpectHeader => {
                if b == b'@' && (i == offset || bytes[i - 1] == b'\n' || bytes[i - 1] == b'\r') {
                    // Found header start
                    hdr_start = i as u64;
                    // Advance to end of header line
                    let line_end = find_line_end_or_eof(bytes, i);
                    hdr_end = line_end as u64;
                    // Extract ID (first token after '@')
                    let hdr_bytes = &bytes[i + 1..line_end];
                    let token_end = hdr_bytes
                        .iter()
                        .position(|&b| b == b' ' || b == b'\t')
                        .unwrap_or(hdr_bytes.len());
                    id_start = i as u64 + 1;
                    id_end = id_start + token_end as u64;
                    i = line_end;
                    v.header_ok = true;
                    state = FastqParserState::ReadSequence;
                } else {
                    // Skip non-fastq content before header
                    i += 1;
                }
            }
            FastqParserState::ReadSequence => {
                if b == b'+' && (i == 0 || bytes[i - 1] == b'\n' || bytes[i - 1] == b'\r') {
                    // Found the '+' line
                    if seq_len > 0 {
                        v.sequence_ok = true;
                    }
                    // The current position i includes any trailing newline content?
                    // No, `i` points to '+'. The preceding newline was consumed
                    // as part of the last sequence line.
                    plus_start = i as u64;
                    let line_end = find_line_end_or_eof(bytes, i);
                    plus_end = line_end as u64;
                    i = line_end;
                    v.plus_ok = true;
                    state = FastqParserState::ReadQuality;
                } else {
                    // Accumulate sequence bytes (skip newlines for length)
                    if b != b'\n' && b != b'\r' {
                        seq_len += 1;
                    }
                    seq_end = (i + 1) as u64;
                    i += 1;
                }
            }
            FastqParserState::ReadQuality => {
                // Accumulate quality bytes. '@' at line start does NOT start a
                // new record here — we are in the middle of a FASTQ record.
                if b != b'\n' && b != b'\r' {
                    qual_len += 1;
                    qual_end = (i + 1) as u64;
                    if !((33u8..=126u8).contains(&b)) {
                        v.illegal_qual_char = true;
                    }
                }
                i += 1;
                // Check if we have enough quality
                if qual_len >= seq_len {
                    v.quality_ok = seq_len > 0;
                    // Skip trailing newline if present
                    if i < end && (bytes[i] == b'\n' || bytes[i] == b'\r') {
                        i += 1;
                        if bytes[i - 1] == b'\r' && i < end && bytes[i] == b'\n' {
                            i += 1;
                        }
                    }
                    break;
                }
            }
            FastqParserState::Complete | FastqParserState::Error => break,
            // ExpectPlus is a conceptual state — in practice detection of '+'
            // happens inside ReadSequence and transitions directly to ReadQuality.
            FastqParserState::ExpectPlus => {
                // Should not be reached in normal parsing; if we somehow get
                // here, look for '+' at line start or skip.
                if b == b'+' && (i == 0 || bytes[i - 1] == b'\n' || bytes[i - 1] == b'\r') {
                    v.plus_ok = true;
                    plus_start = i as u64;
                    i = find_line_end_or_eof(bytes, i);
                    plus_end = i as u64;
                    state = FastqParserState::ReadQuality;
                } else {
                    i += 1;
                }
            }
        }
    }

    // Post-loop handling
    if state == FastqParserState::ReadQuality && qual_len < seq_len {
        v.truncated = true;
    }
    v.length_match = seq_len == qual_len;

    // Empty sequence handling
    if seq_len == 0 && v.header_ok {
        v.sequence_ok = false;
        v.length_match = qual_len == 0;
        if qual_len > 0 {
            v.truncated = true; // quality with no sequence is nonsense
        }
    }

    v.valid = v.header_ok && v.plus_ok && v.length_match && !v.truncated && seq_len > 0;

    // If we hit end of file without closing, still report the record
    if state != FastqParserState::Complete && state != FastqParserState::Error && !v.truncated {
        // Nothing — we've already set validity flags
    }

    let next_offset = if v.valid || v.truncated || v.plus_ok {
        // After a valid or partial record, next record starts at the position
        // after the quality (which is i)
        i
    } else {
        i.max(offset + 1)
    };

    let entry = FastqRecordEntry {
        record_number,
        start_offset: offset as u64,
        end_offset: i as u64,
        header_range: ByteRange::new(hdr_start, hdr_end).unwrap_or(ByteRange {
            start: offset as u64,
            end: offset as u64 + 1,
        }),
        id_range: ByteRange::new(id_start, id_end).unwrap_or(ByteRange {
            start: offset as u64 + 1,
            end: offset as u64 + 1,
        }),
        sequence_range: ByteRange::new(hdr_end, seq_end).unwrap_or(ByteRange {
            start: hdr_end,
            end: hdr_end,
        }),
        plus_range: ByteRange::new(plus_start, plus_end).unwrap_or(ByteRange { start: 0, end: 0 }),
        quality_range: ByteRange::new(plus_end, qual_end).unwrap_or(ByteRange { start: 0, end: 0 }),
        sequence_length: seq_len,
        quality_length: qual_len,
        validation: v,
    };

    Ok((entry, next_offset))
}

/// Find the position of the first `\n` at or after `start`; return `bytes.len()` if none.
fn find_line_end_or_eof(bytes: &[u8], start: usize) -> usize {
    bytes[start..]
        .iter()
        .position(|&b| b == b'\n')
        .map_or(bytes.len(), |rel| {
            let pos = start + rel;
            // For CRLF, include the \r in the line, but return position of \n
            pos + 1 // include the \n
        })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn standard_four_line() {
        let data = b"@read1\nACGT\n+\nIIII\n";
        let (e, _) = parse_single_record(data, 0, 1).unwrap();
        assert!(e.validation.valid);
        assert_eq!(e.sequence_length, 4);
        assert_eq!(e.quality_length, 4);
    }

    #[test]
    fn crlf() {
        let data = b"@read1\r\nACGT\r\n+\r\nIIII\r\n";
        let (e, _) = parse_single_record(data, 0, 1).unwrap();
        assert!(e.validation.valid);
        assert_eq!(e.sequence_length, 4);
    }

    #[test]
    fn truncated() {
        let data = b"@read1\nACGT\n+\nIII";
        let (e, _) = parse_single_record(data, 0, 1).unwrap();
        assert!(!e.validation.valid);
        assert!(e.validation.truncated);
    }

    #[test]
    fn length_mismatch() {
        let data = b"@read1\nACGT\n+\nIII\n";
        let (e, _) = parse_single_record(data, 0, 1).unwrap();
        assert!(!e.validation.length_match);
        assert_eq!(e.sequence_length, 4);
        assert_eq!(e.quality_length, 3);
    }

    #[test]
    fn multi_line_sequence() {
        let data = b"@read1\nAC\nGT\n+\nIIII\n";
        let (e, _) = parse_single_record(data, 0, 1).unwrap();
        assert!(e.validation.valid);
        assert_eq!(e.sequence_length, 4);
    }

    #[test]
    fn multi_line_quality() {
        let data = b"@read1\nACGT\n+\nII\nII\n";
        let (e, _) = parse_single_record(data, 0, 1).unwrap();
        assert!(e.validation.valid, "multi-line quality valid");
        assert_eq!(e.quality_length, 4);
    }

    #[test]
    fn empty_sequence_after_header() {
        // Two records where the first has an empty sequence
        let data = b"@read1\n+\nIIII\n@read2\nACGT\n+\nIIII\n";
        let (e, next) = parse_single_record(data, 0, 1).unwrap();
        assert!(!e.validation.sequence_ok, "empty seq flag");
        assert!(!e.validation.valid, "empty seq -> invalid");
        // The next record should parse from `next`
        let (e2, _) = parse_single_record(data, next, 2).unwrap();
        assert!(e2.validation.valid);
        assert_eq!(e2.record_number, 2);
    }

    #[test]
    fn no_trailing_newline() {
        let data = b"@read1\nACGT\n+\nIIII";
        let (e, _) = parse_single_record(data, 0, 1).unwrap();
        assert!(e.validation.valid, "no trailing newline");
    }

    #[test]
    fn id_from_header() {
        let data = b"@read1 long description\nACGT\n+\nIIII\n";
        let (e, _) = parse_single_record(data, 0, 1).unwrap();
        let id_slice = &data
            [usize::try_from(e.id_range.start).unwrap()..usize::try_from(e.id_range.end).unwrap()];
        assert_eq!(id_slice, b"read1");
    }

    #[test]
    fn illegal_quality_chars() {
        // ' ' (ASCII 32) is outside Phred+33 valid range (33-126)
        let data = b"@read1\nACGT\n+\n   \n";
        let (e, _) = parse_single_record(data, 0, 1).unwrap();
        assert!(e.validation.illegal_qual_char);
    }

    #[test]
    fn multiple_records() {
        let data = b"@r1\nA\n+\nI\n@r2\nC\n+\nJ\n";
        let (e1, next) = parse_single_record(data, 0, 1).unwrap();
        assert!(e1.validation.valid);
        let (e2, _) = parse_single_record(data, next, 2).unwrap();
        assert!(e2.validation.valid);
        assert_eq!(e2.record_number, 2);
    }
}
