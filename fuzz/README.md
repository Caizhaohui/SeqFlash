# Fuzz targets (M8)

Full `cargo-fuzz` + libFuzzer is awkward on Windows MSVC. SeqFlash covers the
same parsers with:

1. **Always-on property tests** (`proptest`) in:
   - `crates/seqflash-formats/tests/proptest_formats.rs`
   - `crates/seqflash-ops/tests/proptest_overlay.rs`
2. **Stdin harness binaries** under this folder for optional external fuzzers
   (AFL, Honggfuzz, libFuzzer via nightly + clang) on CI agents that support them.

## Property tests (default)

```powershell
cargo test -p seqflash-formats --test proptest_formats
cargo test -p seqflash-ops --test proptest_overlay
```

## Stdin harnesses

Build:

```powershell
cargo build -p seqflash-fuzz --release
```

Feed a file:

```powershell
Get-Content -Encoding Byte sample.fa | .\target\release\fuzz_detect.exe
Get-Content -Encoding Byte sample.fq | .\target\release\fuzz_fastq.exe
```

These binaries must not panic on any input; they exit 0 after parsing.
