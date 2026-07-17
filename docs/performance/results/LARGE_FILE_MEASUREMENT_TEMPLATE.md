# Large-file measurement record (template)

Copy this file to a dated name, e.g. `large-file-YYYYMMDD-machine.md`, fill every
measured field, and leave unmeasured cells as `—`. **Do not invent numbers.**

See methodology: [`../methodology.md`](../methodology.md) and plan §25.

---

## Environment

| Field | Value |
|-------|--------|
| Date (local) | |
| Operator | |
| Machine hostname | |
| CPU | |
| Physical RAM | |
| Disk type / model | |
| Free disk space | |
| Windows version | |
| rustc (`rustc -V`) | |
| SeqFlash commit / branch | |
| Build | Release (`cargo build --workspace --release`) |
| Binary path | `target/release/seqflash-app.exe` |
| Notes (power plan, AV, etc.) | |

---

## Test data

Generate with (do **not** commit large files):

```powershell
.\scripts\generate-large-fasta.ps1 -SizeGB 0.1
.\scripts\generate-large-fasta.ps1 -SizeGB 1
# optional: -SizeGB 4 if disk and time allow
```

| Scenario ID | Path | Format | Size (bytes / GiB) | Records | Avg seq length | Avg physical line length | Generator params |
|-------------|------|--------|--------------------|---------|----------------|--------------------------|------------------|
| A-100MiB | | FASTA | | | | | `-SizeGB 0.1` |
| B-1GiB | | FASTA | | | | | `-SizeGB 1` |
| C-4GiB | | FASTA | | | | | `-SizeGB 4` (optional) |
| D-FASTQ (optional) | | FASTQ | | | | | |

How to fill metadata after generation:

```powershell
# size
(Get-Item $path).Length
# rough record count (FASTA headers)
(Select-String -Path $path -Pattern '^>' -SimpleMatch).Count
```

---

## Targets (plan §25) — reference only

| Scenario | Target |
|----------|--------|
| App cold start | &lt; 1.5 s |
| 100 MiB FASTA first paint | &lt; 1 s |
| 1 GiB FASTA first paint | &lt; 2 s |
| 4 GiB FASTA first paint | &lt; 3 s |
| 1 GiB view working set | &lt; 300 MiB |
| 4 GiB view working set | &lt; 500 MiB |
| Jump to indexed record | &lt; 100 ms |
| Scroll input delay P95 | &lt; 50 ms |
| FPS while indexing | ≥ 30 |
| Search first hit | preferably &lt; 1 s |
| Streaming export throughput | ≥ 60% of sequential disk write |

---

## Measurements

### Cold start

| Run | Stopwatch (s) | Method | Pass? |
|-----|---------------|--------|-------|
| 1 | | Launch exe, first window frame visible | |
| 2 | | | |
| 3 | | | |
| Median | | | |

### First paint after open

Start timing when confirming Open / dropping file; stop when first sequence
lines are visible (not when index reaches 100%).

| Scenario | Run 1 (s) | Run 2 (s) | Run 3 (s) | Median (s) | Target | Pass? |
|----------|-----------|-----------|-----------|------------|--------|-------|
| A-100MiB | | | | | &lt; 1 s | |
| B-1GiB | | | | | &lt; 2 s | |
| C-4GiB | | | | | &lt; 3 s | |

### Working set (Task Manager → Memory)

Sample after first paint + ~10 s idle browse (scroll a few pages).

| Scenario | Working set (MiB) | Private bytes (if available) | Target | Pass? |
|----------|-------------------|------------------------------|--------|-------|
| A-100MiB | | | — | |
| B-1GiB | | | &lt; 300 MiB | |
| C-4GiB | | | &lt; 500 MiB | |

### Index complete (optional)

| Scenario | Time to index 100% (s) | Records shown | Notes |
|----------|------------------------|---------------|-------|
| A-100MiB | | | |
| B-1GiB | | | |

### Record jump (after index complete)

| Scenario | From rec → to rec | Time (ms) | Target | Pass? |
|----------|-------------------|-----------|--------|-------|
| B-1GiB | e.g. 1 → mid | | &lt; 100 ms | |

### Search first hit

| Scenario | Pattern | Mode | Time to first hit (s) | Notes |
|----------|---------|------|----------------------|-------|
| B-1GiB | | Bytes / ID | | |

### Overlay save (optional)

| Scenario | Edits applied | Output path | Duration (s) | Output size | Cancel test OK? |
|----------|---------------|-------------|--------------|-------------|-----------------|
| small | | | | | |

### Soak (optional, plan: 2 h)

| Duration | Actions | Start WS (MiB) | End WS (MiB) | Leaks / hangs? |
|----------|---------|----------------|--------------|----------------|
| | scroll + tab switch | | | |

---

## Open / close handle smoke (automated)

```powershell
cargo test -p seqflash-document --test stress_lifecycle
```

| Result | Date |
|--------|------|
| pass / fail | |

---

## Criterion micro-benchmarks (same session)

```powershell
.\scripts\benchmark.ps1
```

| Field | Value |
|-------|--------|
| Env snapshot file | `docs/performance/results/bench-env-*.txt` |
| Criterion report dir | `target/criterion/` |
| Filter used | (all) / … |
| Notes | |

Paste or attach notable medians (from HTML or console), e.g.:

| Group / bench | Median time | Throughput |
|---------------|-------------|------------|
| detect_format/fasta_sample | | |
| index/fasta_2k_records | | |
| search/raw_bytes_motif | | |
| render/format_raw_line_256 | | |

---

## Summary

| Area | Verdict (pass / fail / partial) | Notes |
|------|----------------------------------|-------|
| Cold start | | |
| First paint 100 MiB | | |
| First paint 1 GiB | | |
| Memory 1 GiB | | |
| Micro-benchmarks recorded | | |
| Blockers | | |

**Signed off:**  
**Date:**
