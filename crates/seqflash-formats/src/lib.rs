//! FASTA/FASTQ detection, parsing, and validation.
//!
//! Per `DEVELOPMENT_PLAN.md` section 9.3 / 13 / 14, this crate performs format
//! detection, FASTA record parsing, FASTQ state-machine parsing, and
//! header/ID extraction. All parsing is byte-oriented.
//!
//! M3 scope: FASTA detection + header/ID extraction.
//! M4 scope: FASTQ state-machine parser + record types.

#![forbid(unsafe_code)]

mod detect;
mod fasta;
mod fastq;

pub use detect::{detect_format, DETECT_SAMPLE_BYTES};
pub use fasta::{parse_fasta_header, FastaHeader};
pub use fastq::{parse_single_record, FastqParserState, FastqRecordEntry, FastqValidation};
