# SeqFlash

> A Windows-first FASTA/FASTQ browser for large sequence files, built in Rust with `eframe` + `egui`.

SeqFlash focuses on one job: opening very large (hundreds of MB to multiple GB)
FASTA/FASTQ files quickly, browsing them smoothly, navigating records, searching,
inspecting, editing at record level via an in-memory overlay, and exporting
safely — all with low memory on Windows 10/11.

## Status

Currently at milestone **M8 — stability and performance** (foundation in place;
large-file timing still measured per machine).

| Milestone | Status |
|-----------|--------|
| M0 Workspace init | Done |
| M1 mmap document model | Done |
| M2 Virtual raw-text viewer | Done |
| M3 FASTA index & navigation | Done |
| M4 FASTQ index & validation | Done |
| M5 Search | Done |
| M6 Export & sequence ops | Done (library + partial UI) |
| M7 Overlay edit / undo / save | Done |
| **M8 Stability & performance** | **In progress** |
| M9 Windows productization | Planned |

See [`SeqFlash_DEVELOPMENT_PLAN.md`](./SeqFlash_DEVELOPMENT_PLAN.md) for the full
plan, [`docs/milestones/M7_ACCEPTANCE.md`](./docs/milestones/M7_ACCEPTANCE.md)
for M7, and [`docs/milestones/M8_ACCEPTANCE.md`](./docs/milestones/M8_ACCEPTANCE.md)
for M8.

### What works today

- Open large FASTA/FASTQ via dialog, CLI path, or drag-and-drop (read-only mmap)
- Virtual scrolling over source bytes (source file is never modified)
- Incremental FASTA/FASTQ record indexing and navigation
- Base / GC stats (FASTA) and quality stats (FASTQ), **overlay-aware**
- Incremental search (bytes / ID / sequence fragment)
- Record copy + single-record export
- **Record-level overlay edits**: header / sequence / quality, delete, insert before/after
- **Overlay preview** in the central panel and record list badges (`[DEL]`, `[EDIT]`, …)
- Undo / Redo (Ctrl+Z / Ctrl+Y)
- **Save edits…**: background streaming save with progress + cancel; never overwrites the open source

### Explicitly out of M7

- Free-form text editing of multi-GB files
- In-place overwrite of the open source file
- Full-file virtualized “overlay as the only view” (source view remains available; effective record content is previewed separately)
- Unified `seqflash-jobs` runtime for all background work (save uses a dedicated worker; M8 adds job tokens)

### M8 tooling

```powershell
# Micro-benchmarks (Criterion)
.\scripts\benchmark.ps1

# Property / stress tests (included in workspace test)
cargo test --workspace

# Optional stdin fuzz harnesses
cargo build -p seqflash-fuzz --release
```

See [`docs/performance/methodology.md`](./docs/performance/methodology.md).

## Tech stack

- Language: Rust (stable, `x86_64-pc-windows-msvc`)
- GUI: `eframe` + `egui`
- Large-file I/O: read-only memory mapping + byte offsets / record indexes

## Build

```bash
cargo build --workspace --release
```

The release binary is `target/release/seqflash-app.exe`.

## Miri on Windows

Install the Miri components explicitly before running it, so Cargo does not
need to synchronize a nightly toolchain during a validation run:

```powershell
rustup toolchain install nightly-x86_64-pc-windows-msvc
rustup component add miri rust-src --toolchain nightly-x86_64-pc-windows-msvc
$env:MIRIFLAGS = "-Zmiri-strict-provenance -Zmiri-symbolic-alignment-check"
cargo +nightly miri test -p seqflash-document
```

Miri does not currently support the Windows file operations used by real
temporary-file and memory-map tests. Those tests are skipped under Miri and
remain covered by the ordinary Windows test suite.

## Development checks

These four commands must all pass before any milestone is considered complete
(see `SeqFlash_DEVELOPMENT_PLAN.md` section 29):

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo build --workspace --release
```

## Project layout

```text
apps/seqflash-app/          # egui desktop application
crates/
  seqflash-document/        # read-only mmap documents
  seqflash-formats/         # FASTA/FASTQ detect & parse
  seqflash-index/           # incremental record indexes
  seqflash-viewer/          # virtual-scrolling raw viewer
  seqflash-search/          # incremental search
  seqflash-ops/             # stats, transforms, export, overlay
  seqflash-settings/        # persisted settings
  seqflash-types/           # shared IDs, ranges, formats
  seqflash-jobs/            # job tokens: cancel, progress, revision gate
  seqflash-platform-windows/# placeholder (M9)
benches/seqflash-bench/     # Criterion micro-benchmarks
fuzz/seqflash-fuzz/         # stdin fuzz harnesses
docs/                       # design notes, performance, milestone acceptance
```

See `SeqFlash_DEVELOPMENT_PLAN.md` sections 7 and 8 for the layered architecture.

## License

Licensed under the [MIT License](./LICENSE).
