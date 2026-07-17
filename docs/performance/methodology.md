# Performance methodology (M8)

## Reference environment (plan §25)

Record these fields for every published result:

| Field | Example |
|-------|---------|
| CPU | e.g. AMD Ryzen 7 … |
| RAM | e.g. 16 GiB |
| Disk | NVMe SSD / model |
| Windows version | Windows 11 23H2 |
| Build | Release (`cargo build --release` / criterion) |
| File size / format | 1 GiB FASTA |
| Record count | … |
| Avg sequence length | … |
| Avg physical line length | … |

**Never invent or estimate numbers.** If a run was not measured, leave the cell empty.

## Micro-benchmarks

```powershell
.\scripts\benchmark.ps1
# or
cargo bench -p seqflash-bench
```

Groups:

| Bench | Crate surface |
|-------|----------------|
| `parsing` | `detect_format`, FASTQ `parse_single_record` |
| `indexing` | FASTA/FASTQ incremental index full scan |
| `searching` | `SearchSession` raw-byte search |
| `rendering` | `format_raw_line` / `format_line` |

Environment snapshot: `docs/performance/results/bench-env-*.txt`.

## Large-file scenarios

Generate data (never commit large files):

```powershell
.\scripts\generate-large-fasta.ps1 -SizeGB 0.1
.\scripts\generate-large-fasta.ps1 -SizeGB 1
```

Manual checks (stopwatch / Task Manager):

1. Cold start to first frame.
2. Open file → first paint.
3. Working set while browsing (1 GiB / 4 GiB targets in plan §25).
4. Jump to record after index complete.
5. Scroll input feel (subjective + frame timing if available).
6. Search time to first hit.
7. Overlay save throughput vs disk sequential write (rough).

## Memory / handle leak smoke

- Open/close 32 files × 4 cycles: covered by `seqflash-document` stress test.
- Long-run 2h GUI soak: manual (M8 acceptance checklist).

## Interpreting Criterion

- Prefer median / mean from the HTML report under `target/criterion/`.
- Compare only same machine + same rustc major version.
- Micro-benches do **not** replace large-file first-screen targets.
- Example archived run: [`results/criterion-20260717-221019.md`](./results/criterion-20260717-221019.md).

## Large-file GUI measurement template

Copy and fill:

[`results/LARGE_FILE_MEASUREMENT_TEMPLATE.md`](./results/LARGE_FILE_MEASUREMENT_TEMPLATE.md)

Generate data with `scripts/generate-large-fasta.ps1` (never commit multi‑GB files).
