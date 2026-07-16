//! FASTA/FASTQ record boundary indexing and line checkpoints.
//!
//! Per `DEVELOPMENT_PLAN.md` section 9.4 / 13.2 / 14.3, this crate builds
//! record-boundary indexes for file bytes, mapping record numbers ↔ file
//! offsets. Indexing is incremental (a few MiB per frame) so it never blocks
//! the first screen. Cancellation stops further progress without rolling back.
//!
//! M3 scope: FASTA record index. FASTQ indexing arrives in M4.

#![forbid(unsafe_code)]

mod fasta_index;

pub use fasta_index::{FastaIndex, FastaRecordEntry, DEFAULT_INDEX_SCAN_BUDGET};
