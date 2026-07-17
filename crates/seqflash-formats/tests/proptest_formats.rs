//! Property tests for format detection and parsers (M8 / plan §26.3).
//!
//! Goals: no panic, no inverted ranges, graceful handling of random bytes.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use proptest::prelude::*;
use seqflash_formats::{detect_format, parse_fasta_header, parse_single_record};
use seqflash_types::SequenceFormat;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// detect_format never panics and only returns known variants.
    #[test]
    fn detect_never_panics(data in prop::collection::vec(any::<u8>(), 0..4096)) {
        let fmt = detect_format(&data);
        prop_assert!(matches!(
            fmt,
            SequenceFormat::Fasta | SequenceFormat::Fastq | SequenceFormat::Unknown
        ));
    }

    /// FASTA header parser accepts arbitrary first-line-ish bytes.
    #[test]
    fn fasta_header_never_panics(data in prop::collection::vec(any::<u8>(), 0..512)) {
        let _ = parse_fasta_header(&data);
    }

    /// FASTQ single-record parse never panics; ranges stay ordered when Ok.
    #[test]
    fn fastq_parse_never_panics_and_ranges_ordered(
        data in prop::collection::vec(any::<u8>(), 0..2048),
        offset in 0usize..64,
    ) {
        let offset = offset.min(data.len());
        if let Ok((entry, next)) = parse_single_record(&data, offset, 0) {
            prop_assert!(entry.start_offset <= entry.end_offset);
            prop_assert!(entry.header_range.start <= entry.header_range.end);
            prop_assert!(entry.sequence_range.start <= entry.sequence_range.end);
            prop_assert!(entry.quality_range.start <= entry.quality_range.end);
            prop_assert!(next as u64 >= entry.end_offset || next <= data.len());
        }
    }

    /// Well-formed random FASTA is detected as FASTA (or stays valid).
    #[test]
    fn random_fasta_detects(
        id in "[A-Za-z0-9_]{1,16}",
        seq in "[ACGTNacgtn]{0,200}",
    ) {
        let mut body = format!(">{id}\n");
        for chunk in seq.as_bytes().chunks(60) {
            body.push_str(std::str::from_utf8(chunk).unwrap_or(""));
            body.push('\n');
        }
        let fmt = detect_format(body.as_bytes());
        prop_assert_eq!(fmt, SequenceFormat::Fasta);
    }

    /// Well-formed four-line FASTQ is detected as FASTQ.
    #[test]
    fn random_fastq_detects(
        id in "[A-Za-z0-9_]{1,16}",
        seq in "[ACGTN]{1,80}",
    ) {
        let qual: String = std::iter::repeat_n('I', seq.len()).collect();
        let body = format!("@{id}\n{seq}\n+\n{qual}\n");
        let fmt = detect_format(body.as_bytes());
        prop_assert_eq!(fmt, SequenceFormat::Fastq);
        let (entry, _) = parse_single_record(body.as_bytes(), 0, 0).expect("parse");
        prop_assert_eq!(entry.sequence_length, seq.len() as u64);
        prop_assert_eq!(entry.quality_length, seq.len() as u64);
        prop_assert!(entry.validation.valid);
    }
}
