//! Stress tests: ultra-long lines and many records (M8).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use seqflash_index::{FastaIndex, FastqIndex, DEFAULT_INDEX_SCAN_BUDGET, FASTQ_INDEX_BUDGET};

/// A single multi-megabyte sequence line must not hang indexing.
#[test]
fn indexes_ultra_long_fasta_sequence_line() {
    // 2 MiB of bases on one line after a short header.
    let seq_len = 2 * 1024 * 1024;
    let mut data = Vec::with_capacity(seq_len + 16);
    data.extend_from_slice(b">long\n");
    data.resize(data.len() + seq_len, b'A');
    data.push(b'\n');
    data.extend_from_slice(b">short\nACGT\n");

    let mut idx = FastaIndex::new(data.len() as u64);
    // Drive to completion with normal per-frame budgets.
    let mut safety = 0;
    while !idx.is_complete() && safety < 10_000 {
        idx.scan_chunk(&data, DEFAULT_INDEX_SCAN_BUDGET);
        safety += 1;
    }
    assert!(idx.is_complete(), "index did not complete");
    assert_eq!(idx.entry_count(), 2);
    let e0 = &idx.entries()[0];
    assert!(e0.end_offset > e0.start_offset);
    assert!(e0.end_offset - e0.start_offset > seq_len as u64);
}

/// Many small FASTA records stay within sane entry counts.
#[test]
fn indexes_many_small_fasta_records() {
    use std::fmt::Write;
    let n = 5_000usize;
    let mut data = String::new();
    for i in 0..n {
        let _ = write!(data, ">s{i}\nACGT\n");
    }
    let bytes = data.as_bytes();
    let mut idx = FastaIndex::new(bytes.len() as u64);
    let mut safety = 0;
    while !idx.is_complete() && safety < 10_000 {
        idx.scan_chunk(bytes, DEFAULT_INDEX_SCAN_BUDGET);
        safety += 1;
    }
    assert!(idx.is_complete());
    assert_eq!(idx.entry_count(), n);
}

/// FASTQ with a long sequence/quality pair indexes without panic.
#[test]
fn indexes_long_fastq_record() {
    let seq_len = 256 * 1024;
    let mut data = Vec::with_capacity(seq_len * 2 + 32);
    data.extend_from_slice(b"@r1\n");
    data.resize(data.len() + seq_len, b'A');
    data.extend_from_slice(b"\n+\n");
    data.resize(data.len() + seq_len, b'I');
    data.push(b'\n');

    let mut idx = FastqIndex::new(data.len() as u64);
    let mut safety = 0;
    while !idx.is_complete() && safety < 10_000 {
        idx.scan_chunk(&data, FASTQ_INDEX_BUDGET);
        safety += 1;
    }
    assert!(idx.is_complete());
    assert_eq!(idx.entry_count(), 1);
    let e = &idx.entries()[0];
    assert_eq!(e.sequence_length, seq_len as u64);
    assert_eq!(e.quality_length, seq_len as u64);
}
