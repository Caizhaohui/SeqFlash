//! Sequence transformation operations: reverse complement, case conversion,
//! wrap/unwrap. All functions are pure `&[u8] → Vec<u8>` and byte-oriented.

/// Complement a single DNA base (case-preserving). Non-ACGT bases pass through
/// unchanged (IUPAC codes like R/Y/S etc. are not complemented — only standard
/// ACGT/U/N map).
#[must_use]
#[allow(clippy::match_same_arms)] // T→A and U→A both return A (RNA compat)
pub fn complement_base(b: u8) -> u8 {
    match b {
        b'A' => b'T',
        b'T' => b'A',
        b'C' => b'G',
        b'G' => b'C',
        b'a' => b't',
        b't' => b'a',
        b'c' => b'g',
        b'g' => b'c',
        b'U' => b'A',
        b'u' => b'a',
        other => other,
    }
}

/// Reverse-complement a DNA sequence: complement each base, then reverse the
/// order. Newlines in the input are stripped before transformation so
/// multi-line FASTA sequences produce a single contiguous result.
#[must_use]
pub fn reverse_complement(seq: &[u8]) -> Vec<u8> {
    seq.iter()
        .rev()
        .filter(|&&b| b != b'\n' && b != b'\r')
        .map(|&b| complement_base(b))
        .collect()
}

/// Reverse a quality string (for FASTQ reverse complement). The quality bytes
/// are simply reversed — no complement is applied to quality values.
#[must_use]
pub fn reverse_quality(qual: &[u8]) -> Vec<u8> {
    qual.iter().rev().copied().collect()
}

/// Convert sequence bytes to uppercase (ASCII only).
#[must_use]
pub fn to_uppercase(seq: &[u8]) -> Vec<u8> {
    seq.iter().map(|&b| b.to_ascii_uppercase()).collect()
}

/// Convert sequence bytes to lowercase (ASCII only).
#[must_use]
pub fn to_lowercase(seq: &[u8]) -> Vec<u8> {
    seq.iter().map(|&b| b.to_ascii_lowercase()).collect()
}

/// Wrap a sequence into fixed-width lines (FASTA style). Newlines in the input
/// are stripped first. Each output line is at most `width` bytes, terminated by
/// `\n`. A `width` of 0 is treated as 1.
#[must_use]
pub fn wrap_sequence(seq: &[u8], width: usize) -> Vec<u8> {
    let w = width.max(1);
    let cleaned: Vec<u8> = seq
        .iter()
        .copied()
        .filter(|&b| b != b'\n' && b != b'\r')
        .collect();
    let mut out = Vec::with_capacity(cleaned.len() + cleaned.len() / w + 1);
    for chunk in cleaned.chunks(w) {
        out.extend_from_slice(chunk);
        out.push(b'\n');
    }
    out
}

/// Remove all newlines from a sequence (FASTA unwrap). The result is a single
/// contiguous byte string with no `\n` or `\r`.
#[must_use]
pub fn unwrap_sequence(seq: &[u8]) -> Vec<u8> {
    seq.iter()
        .copied()
        .filter(|&b| b != b'\n' && b != b'\r')
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn revcomp_simple() {
        assert_eq!(reverse_complement(b"ACGT"), b"ACGT");
        assert_eq!(reverse_complement(b"AAAA"), b"TTTT");
        assert_eq!(reverse_complement(b"ACGTACGT"), b"ACGTACGT");
    }

    #[test]
    fn revcomp_case_preserved() {
        // aCgT → reversed TgCa → complement AcGt
        assert_eq!(reverse_complement(b"aCgT"), b"AcGt");
    }

    #[test]
    fn revcomp_strips_newlines() {
        assert_eq!(reverse_complement(b"AC\nGT"), reverse_complement(b"ACGT"));
    }

    #[test]
    fn revcomp_non_acgt_passthrough() {
        // N and gap pass through (position still reversed).
        let r = reverse_complement(b"AN");
        assert_eq!(r, b"NT"); // N→N, A→T, reversed
    }

    #[test]
    fn reverse_quality_simple() {
        assert_eq!(reverse_quality(b"IIJI"), b"IJII");
    }

    #[test]
    fn uppercase() {
        assert_eq!(to_uppercase(b"acgTNNn"), b"ACGTNNN");
    }

    #[test]
    fn lowercase() {
        assert_eq!(to_lowercase(b"ACGtNNn"), b"acgtnnn");
    }

    #[test]
    #[allow(clippy::naive_bytecount)]
    fn wrap_60() {
        let seq = b"A".repeat(130);
        let wrapped = wrap_sequence(&seq, 60);
        // 130/60 = 2 full lines + 1 partial = 3 newlines
        assert_eq!(wrapped.iter().filter(|&&b| b == b'\n').count(), 3);
    }

    #[test]
    fn wrap_strips_input_newlines() {
        let wrapped = wrap_sequence(b"AB\nCD\nEF", 4);
        assert_eq!(wrapped, b"ABCD\nEF\n");
    }

    #[test]
    fn unwrap_removes_newlines() {
        assert_eq!(unwrap_sequence(b"AB\nCD\r\nEF"), b"ABCDEF");
    }

    #[test]
    fn fastq_revcomp_consistency() {
        // FASTQ reverse complement must reverse BOTH sequence and quality.
        let seq = b"ACGTAA";
        let qual = b"abcdef";
        let rc_seq = reverse_complement(seq);
        let rc_qual = reverse_quality(qual);
        // rc of ACGTAA = TTACGT
        assert_eq!(rc_seq, b"TTACGT");
        // quality reversed
        assert_eq!(rc_qual, b"fedcba");
        // lengths match
        assert_eq!(rc_seq.len(), rc_qual.len());
    }
}
