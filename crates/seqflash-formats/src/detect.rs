//! FASTA/FASTQ format detection.
//!
//! Per plan section 13.1, detection is *preliminary*: skip an optional BOM,
//! skip leading blank lines, then require the first meaningful byte to be `>`
//! (FASTA). A miss yields [`SequenceFormat::Unknown`] — we never force a parse.

use seqflash_types::SequenceFormat;

/// UTF-8 BOM, if present at the start of a file.
const BOM: &[u8] = &[0xEF, 0xBB, 0xBF];

/// Number of bytes sampled from the head of the file for detection. The header
/// alone is enough to tell FASTA from FASTQ; we never need the whole file here.
pub const DETECT_SAMPLE_BYTES: usize = 64 * 1024;

/// Preliminarily detect the sequence format of `bytes` (usually a head sample).
///
/// Rules (plan 13.1):
/// - ignore an optional UTF-8 BOM;
/// - skip leading blank lines (lines containing only `\r` / `\n`);
/// - if the first non-blank byte is `>` → [`SequenceFormat::Fasta`];
/// - if it is `@` → [`SequenceFormat::Fastq`] (recognized early; full parsing
///   arrives in M4);
/// - otherwise → [`SequenceFormat::Unknown`] (never force-parse).
#[must_use]
pub fn detect_format(bytes: &[u8]) -> SequenceFormat {
    let mut pos = skip_bom(bytes);
    pos = skip_blank_lines(bytes, pos);

    if pos >= bytes.len() {
        // Empty (after BOM/blanks) or only blanks — unknown, not FASTA.
        return SequenceFormat::Unknown;
    }

    match bytes[pos] {
        b'>' => SequenceFormat::Fasta,
        b'@' => SequenceFormat::Fastq,
        _ => SequenceFormat::Unknown,
    }
}

/// Return the index past a leading UTF-8 BOM, if present.
fn skip_bom(bytes: &[u8]) -> usize {
    if bytes.starts_with(BOM) {
        BOM.len()
    } else {
        0
    }
}

/// Return the index of the first byte that is not part of a leading run of
/// blank lines (lines of only CR/LF).
fn skip_blank_lines(bytes: &[u8], start: usize) -> usize {
    let mut pos = start;
    while pos < bytes.len() {
        let b = bytes[pos];
        if b == b'\n' {
            pos += 1;
        } else if b == b'\r' {
            // CRLF or lone CR.
            pos += 1;
            if pos < bytes.len() && bytes[pos] == b'\n' {
                pos += 1;
            }
        } else {
            break;
        }
    }
    pos
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn detects_fasta() {
        assert_eq!(detect_format(b">seq1\nACGT\n"), SequenceFormat::Fasta);
    }

    #[test]
    fn detects_fastq() {
        assert_eq!(
            detect_format(b"@read1\nACGT\n+\nIIII\n"),
            SequenceFormat::Fastq
        );
    }

    #[test]
    fn detects_unknown_for_plain_text() {
        assert_eq!(detect_format(b"hello world\n"), SequenceFormat::Unknown);
        assert_eq!(detect_format(b"ACGTACGT\n"), SequenceFormat::Unknown);
    }

    #[test]
    fn empty_is_unknown() {
        assert_eq!(detect_format(b""), SequenceFormat::Unknown);
    }

    #[test]
    fn skips_utf8_bom() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(BOM);
        bytes.extend_from_slice(b">seq1\nACGT\n");
        assert_eq!(detect_format(&bytes), SequenceFormat::Fasta);
    }

    #[test]
    fn skips_leading_blank_lines() {
        assert_eq!(detect_format(b"\n\n>seq1\nACGT\n"), SequenceFormat::Fasta);
        assert_eq!(detect_format(b"\r\n\r\n>seq1\n"), SequenceFormat::Fasta);
    }

    #[test]
    fn only_blanks_is_unknown() {
        assert_eq!(detect_format(b"\n\n\n"), SequenceFormat::Unknown);
    }

    #[test]
    fn fasta_without_trailing_newline() {
        assert_eq!(detect_format(b">seq1\nACGT"), SequenceFormat::Fasta);
    }
}
