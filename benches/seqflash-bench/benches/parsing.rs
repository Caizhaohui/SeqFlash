//! Format detection and FASTQ parse throughput (M8).

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use seqflash_formats::{detect_format, parse_single_record};

fn sample_fasta(records: usize, bases: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(records * (bases + 32));
    for i in 0..records {
        out.extend_from_slice(format!(">seq_{i}\n").as_bytes());
        out.resize(out.len() + bases, b'A');
        out.push(b'\n');
    }
    out
}

fn sample_fastq(records: usize, bases: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(records * (bases * 2 + 32));
    for i in 0..records {
        out.extend_from_slice(format!("@read_{i}\n").as_bytes());
        out.resize(out.len() + bases, b'A');
        out.extend_from_slice(b"\n+\n");
        out.resize(out.len() + bases, b'I');
        out.push(b'\n');
    }
    out
}

fn bench_detect(c: &mut Criterion) {
    let fa = sample_fasta(100, 200);
    let fq = sample_fastq(100, 150);
    let mut g = c.benchmark_group("detect_format");
    g.throughput(Throughput::Bytes(fa.len() as u64));
    g.bench_function("fasta_sample", |b| {
        b.iter(|| detect_format(black_box(&fa[..fa.len().min(65536)])));
    });
    g.throughput(Throughput::Bytes(fq.len() as u64));
    g.bench_function("fastq_sample", |b| {
        b.iter(|| detect_format(black_box(&fq[..fq.len().min(65536)])));
    });
    g.finish();
}

fn bench_fastq_parse(c: &mut Criterion) {
    let fq = sample_fastq(500, 100);
    let mut g = c.benchmark_group("fastq_parse");
    g.throughput(Throughput::Elements(500));
    g.bench_function("500_records", |b| {
        b.iter(|| {
            let mut pos = 0usize;
            let mut n = 0u64;
            while pos < fq.len() {
                match parse_single_record(black_box(&fq), pos, n) {
                    Ok((e, next)) => {
                        pos = next;
                        n = e.record_number.saturating_add(1);
                    }
                    Err(_) => break,
                }
            }
            black_box(n)
        });
    });
    g.finish();
}

criterion_group!(benches, bench_detect, bench_fastq_parse);
criterion_main!(benches);
