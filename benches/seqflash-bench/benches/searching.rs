//! Byte-search throughput (M8).

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use seqflash_search::{SearchMode, SearchSession};

fn sample_fasta(records: usize, bases: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(records * (bases + 32));
    for i in 0..records {
        out.extend_from_slice(format!(">seq_{i}\n").as_bytes());
        // Embed a rare motif every 50 records.
        if i % 50 == 0 {
            out.extend_from_slice(b"ACGTMOTIF");
            out.resize(out.len() + bases.saturating_sub(9), b'G');
        } else {
            out.resize(out.len() + bases, b'G');
        }
        out.push(b'\n');
    }
    out
}

fn bench_search(c: &mut Criterion) {
    let data = sample_fasta(1_000, 200);
    let mut g = c.benchmark_group("search");
    g.throughput(Throughput::Bytes(data.len() as u64));
    g.bench_function("raw_bytes_motif", |b| {
        b.iter(|| {
            let mut s = SearchSession::new(
                SearchMode::RawBytes,
                b"MOTIF".to_vec(),
                true,
                data.len() as u64,
            );
            while !s.is_complete() && !s.is_cancelled() {
                s.search_chunk(black_box(&data), 4 * 1024 * 1024);
            }
            black_box(s.results().len())
        });
    });
    g.finish();
}

criterion_group!(benches, bench_search);
criterion_main!(benches);
