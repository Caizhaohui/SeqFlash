//! FASTA/FASTQ record boundary indexing and line checkpoints.
//!
//! **Status (M0): placeholder.** No implementation yet.
//!
//! Per `DEVELOPMENT_PLAN.md` section 9.4 / 13.2 / 14.3, this crate will build
//! record-boundary and line-checkpoint indexes in the background, mapping
//! record numbers ↔ file offsets, reporting incremental progress, and allowing
//! cancellation. Indexing must never be a prerequisite for showing the first
//! screen of a file.
