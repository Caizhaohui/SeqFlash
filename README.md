# SeqFlash

> A Windows-first FASTA/FASTQ browser for large sequence files, built in Rust with `eframe` + `egui`.

SeqFlash focuses on one job: opening very large (hundreds of MB to multiple GB)
FASTA/FASTQ files quickly, browsing them smoothly, navigating records, searching,
inspecting, and exporting safely — all with low memory on Windows 10/11.

## Status

Currently at milestone **M0 — Windows workspace initialization**.

The workspace, build tooling, minimal `egui` main window, logging, and error
handling are in place. FASTA/FASTQ parsing, indexing, search, editing, and
export are **not** implemented yet (planned for later milestones — see
[`SeqFlash_DEVELOPMENT_PLAN.md`](./SeqFlash_DEVELOPMENT_PLAN.md)).

## Tech stack

- Language: Rust (stable, `x86_64-pc-windows-msvc`)
- GUI: `eframe` + `egui`
- Large-file I/O strategy: read-only memory mapping + byte offsets (planned)

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

See `SeqFlash_DEVELOPMENT_PLAN.md` sections 7 and 8 for the full layered
architecture and repository structure.

## License

Licensed under the [MIT License](./LICENSE).
