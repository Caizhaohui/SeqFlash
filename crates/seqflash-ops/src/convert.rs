//! Format conversion operations.

/// Convert a single FASTQ record to FASTA format.
///
/// `header` is the raw header line WITHOUT the leading `@` (the caller strips
/// it). The output prepends `>` instead. `sequence` should be the raw sequence
/// bytes (no newlines, or newlines will be preserved as-is).
///
/// Output: `>{header}\n{sequence}\n`
#[must_use]
pub fn fastq_to_fasta(header: &[u8], sequence: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + header.len() + 1 + sequence.len() + 1);
    out.push(b'>');
    out.extend_from_slice(header);
    out.push(b'\n');
    out.extend_from_slice(sequence);
    // Ensure trailing newline
    if sequence.last() != Some(&b'\n') {
        out.push(b'\n');
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn basic_conversion() {
        let out = fastq_to_fasta(b"read1 desc", b"ACGT");
        assert_eq!(out, b">read1 desc\nACGT\n");
    }

    #[test]
    fn preserves_header_text() {
        let out = fastq_to_fasta(b"read1", b"AAAA");
        assert!(out.starts_with(b">read1\n"));
    }

    #[test]
    fn empty_sequence() {
        let out = fastq_to_fasta(b"empty", b"");
        assert_eq!(out, b">empty\n\n");
    }
}
