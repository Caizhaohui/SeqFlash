//! FASTA/FASTQ incremental index throughput (M8).

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use seqflash_index::{FastaIndex, FastqIndex, DEFAULT_INDEX_SCAN_BUDGET, FASTQ_INDEX_BUDGET};

fn sample_fasta(records: usize, bases: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(records * (bases + 32));
    for i in 0..records {
        out.extend_from_slice(format!(">seq_{i}\n").as_bytes());
        out.resize(out.len() + bases, b'C');
        out.push(b'\n');
    }
    out
}

fn sample_fastq(records: usize, bases: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(records * (bases * 2 + 32));
    for i in 0..records {
        out.extend_from_slice(format!("@r{i}\n").as_bytes());
        out.resize(out.len() + bases, b'T');
        out.extend_from_slice(b"\n+\n");
        out.resize(out.len() + bases, b'I');
        out.push(b'\n');
    }
    out
}

fn index_fasta_full(data: &[u8]) -> usize {
    let mut idx = FastaIndex::new(data.len() as u64);
    while !idx.is_complete() {
        idx.scan_chunk(data, DEFAULT_INDEX_SCAN_BUDGET);
    }
    idx.entry_count()
}

fn index_fastq_full(data: &[u8]) -> usize {
    let mut idx = FastqIndex::new(data.len() as u64);
    while !idx.is_complete() {
        idx.scan_chunk(data, FASTQ_INDEX_BUDGET);
    }
    idx.entry_count()
}

fn bench_index(c: &mut Criterion) {
    let fa = sample_fasta(2_000, 120);
    let fq = sample_fastq(1_000, 100);
    let mut g = c.benchmark_group("index");
    g.throughput(Throughput::Bytes(fa.len() as u64));
    g.bench_function("fasta_2k_records", |b| {
        b.iter(|| index_fasta_full(black_box(&fa)));
    });
    g.throughput(Throughput::Bytes(fq.len() as u64));
    g.bench_function("fastq_1k_records", |b| {
        b.iter(|| index_fastq_full(black_box(&fq)));
    });
    g.finish();
}

criterion_group!(benches, bench_index);
criterion_main!(benches);
