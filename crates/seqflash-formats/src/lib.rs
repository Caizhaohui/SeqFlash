//! FASTA/FASTQ detection, parsing, and validation.
//!
//! **Status (M0): placeholder.** No implementation yet.
//!
//! Per `DEVELOPMENT_PLAN.md` section 9.3 / 13 / 14, this crate must never
//! assume: files are UTF-8, FASTA sequences are single-line, FASTQ records are
//! strictly four lines, files end with a newline, files only contain LF, or
//! headers are printable ASCII. The FASTQ parser will be a state machine that
//! tolerates multi-line sequence/quality, CRLF, truncation, and empty records.

#![forbid(unsafe_code)]
