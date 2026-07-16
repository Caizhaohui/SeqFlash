//! Sequence statistics and lightweight record operations.
//!
//! Per `DEVELOPMENT_PLAN.md` section 9.7 / 17 / 19, this crate provides
//! sequence-length, GC, N-count, base-composition, and illegal-character
//! statistics. All operations are pure functions over `&[u8]` slices — no I/O,
//! no GUI, no FASTA structure knowledge. Fully unit-testable in isolation.
//!
//! M3 scope: base counting + GC%. Reverse complement, wrap/unwrap, export, and
//! other operations arrive in later milestones.

#![forbid(unsafe_code)]

mod stats;

pub use stats::{count_bases, gc_percent, BaseCounts};
