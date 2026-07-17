//! Sequence statistics and lightweight record operations.
//!
//! M3 scope: base counting + GC%. M4 scope: FASTQ quality statistics.
//! M6 scope: reverse complement, case conversion, wrap/unwrap, FASTQ→FASTA,
//! filtering, and streaming export.

#![forbid(unsafe_code)]

mod convert;
mod export;
mod fastq_stats;
mod filter;
mod stats;
mod transform;

pub use convert::fastq_to_fasta;
pub use export::{
    export_fasta_records, export_fastq_records, ExportError, FastaExportRecord, FastqExportRecord,
    Transform,
};
pub use fastq_stats::{phred33_quality_stats, QualityStats};
pub use filter::{extract_by_id, filter_by_length};
pub use stats::{count_bases, gc_percent, BaseCounts};
pub use transform::{
    complement_base, reverse_complement, reverse_quality, to_lowercase, to_uppercase,
    unwrap_sequence, wrap_sequence,
};
