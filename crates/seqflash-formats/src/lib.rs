//! FASTA/FASTQ detection, parsing, and validation.
//!
//! Per `DEVELOPMENT_PLAN.md` section 9.3 / 13, this crate performs format
//! detection, FASTA record parsing, and header/ID extraction. It never assumes
//! files are UTF-8, sequences are single-line, files end with a newline, or
//! files only contain LF — all parsing is byte-oriented.
//!
//! M3 scope: FASTA detection + header/ID extraction. FASTQ state-machine
//! parsing arrives in M4.

#![forbid(unsafe_code)]

mod detect;
mod fasta;

pub use detect::detect_format;
pub use fasta::{parse_fasta_header, FastaHeader};
