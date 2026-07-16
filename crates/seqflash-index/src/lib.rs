//! FASTA/FASTQ record boundary indexing and line checkpoints.
//!
//! Per `DEVELOPMENT_PLAN.md` section 9.4 / 13.2 / 14.3, this crate builds
//! record-boundary indexes for file bytes, mapping record numbers ↔ file
//! offsets. Indexing is incremental so the first screen appears immediately.
//!
//! M3 scope: FASTA index. M4 scope: FASTQ index.

#![forbid(unsafe_code)]

mod fasta_index;
mod fastq_index;

pub use fasta_index::{FastaIndex, FastaRecordEntry, DEFAULT_INDEX_SCAN_BUDGET};
pub use fastq_index::{FastqIndex, FASTQ_INDEX_BUDGET};
