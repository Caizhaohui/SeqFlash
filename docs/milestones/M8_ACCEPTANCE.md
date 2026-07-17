# M8 Acceptance — Stability and performance

**Plan reference:** `SeqFlash_DEVELOPMENT_PLAN.md` §30 M8  
**Status:** In progress (foundation landed)

## Goals

Beta-quality stability: no panic on bad input, measurable performance baselines,
revision-safe background work, crash logs for GUI builds.

## Work items

| Item | Status | Notes |
|------|:------:|-------|
| 性能基准 (Criterion) | ✅ | `benches/seqflash-bench` + `scripts/benchmark.ps1` |
| 内存分析 | 🔶 | Stress open/close; full 1–4 GiB RSS still manual |
| 长时间滚动测试 | ⬜ | Manual soak (2h) |
| 超长单行处理 | ✅ | Viewer stress + format truncation |
| 超长单条序列处理 | ✅ | Index stress (2 MiB line / 256 KiB FASTQ) |
| 多文件标签测试 | ✅ | DocumentList open/close stress |
| 后台任务竞争 / Revision | ✅ | `seqflash-jobs` tokens + unit tests |
| mmap 生命周期 | ✅ | Rapid open/close stress |
| 模糊 / 属性测试 | ✅ | proptest + stdin fuzz harnesses |
| 崩溃日志 | ✅ | Panic hook → `crash-*.log` under app logs |
| 错误提示优化 | ✅ | Open-file user-facing messages |

## Acceptance criteria (plan)

| Criterion | How we address it |
|-----------|-------------------|
| 达到主要性能目标 | Methodology + benches; **large-file numbers must be measured on device** |
| 两小时无明显内存增长 | Manual soak checklist |
| 频繁开关文件无句柄泄漏 | Automated stress |
| 快速切换标签不串任务结果 | Per-document maps; JobHandle revision gate |
| 取消任务不死锁 | Cancel flags + cooperative save cancel (M7) |
| 格式错误不 panic | proptest + fuzz harnesses |
| 解析器模糊测试 | proptest + `fuzz/seqflash-fuzz` stdin tools |
| Release 无 Clippy 警告 | CI gate `clippy -D warnings` |

## Automated checks

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo build --workspace --release
cargo bench -p seqflash-bench -- --quick   # optional faster local
```

## Manual remaining

1. Run `.\scripts\benchmark.ps1` and archive Criterion HTML.  
2. Generate 100 MiB / 1 GiB FASTA; measure first paint + RSS.  
3. GUI soak: scroll + tab switch ~2 hours; watch Task Manager.  
4. Force a panic in debug to confirm `crash-*.log` under `%LOCALAPPDATA%\SeqFlash\logs`.

## Next after M8

**M9 — Windows productization:** icon, installer, file association, portable ZIP, DPI, release workflow.
