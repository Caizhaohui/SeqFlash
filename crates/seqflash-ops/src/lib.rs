//! Sequence statistics and lightweight record operations.
//!
//! **Status (M0): placeholder.** No implementation yet.
//!
//! Per `DEVELOPMENT_PLAN.md` section 9.7 / 17 / 19, this crate provides length,
//! GC, N-count and base composition stats, reverse complement (for FASTQ it
//! must reverse *both* the sequence and the quality), case conversion,
//! wrap/unwrap, FASTQ→FASTA, export, extract-by-ID, and length filtering. All
//! operations are GUI-free and unit-testable in isolation.
