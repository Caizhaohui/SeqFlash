//! Sequence statistics and lightweight record operations.
//!
//! M3 scope: base counting + GC%. M4 scope: FASTQ quality statistics.
//! All operations are pure functions over `&[u8]` slices.

#![forbid(unsafe_code)]

mod fastq_stats;
mod stats;

pub use fastq_stats::{phred33_quality_stats, QualityStats};
pub use stats::{count_bases, gc_percent, BaseCounts};
