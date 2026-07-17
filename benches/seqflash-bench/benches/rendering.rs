//! Viewer line formatting throughput (M8).

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use seqflash_viewer::{format_line, format_raw_line};

fn bench_render(c: &mut Criterion) {
    let line = vec![b'A'; 256];
    let long = vec![b'T'; 4096];
    let mut g = c.benchmark_group("render");
    g.throughput(Throughput::Bytes(line.len() as u64));
    g.bench_function("format_raw_line_256", |b| {
        b.iter(|| format_raw_line(black_box(0), black_box(&line)));
    });
    g.throughput(Throughput::Bytes(long.len() as u64));
    g.bench_function("format_raw_line_4k_truncated", |b| {
        b.iter(|| format_raw_line(black_box(1024), black_box(&long)));
    });
    g.bench_function("format_line_fixed_16", |b| {
        b.iter(|| format_line(black_box(0), black_box(&line[..16]), 16));
    });
    g.finish();
}

criterion_group!(benches, bench_render);
criterion_main!(benches);
