# M7 Acceptance — Record-level editing and Overlay

**Plan reference:** `SeqFlash_DEVELOPMENT_PLAN.md` §30 M7  
**Status:** Accepted for M7 surface area (2026-07-17)

## Goals

Implement limited, safe record-level editing: edits live in an in-memory
`EditOverlay`; the source mmap stays read-only; persistence is **Save as new
file** only, with progress and cancel.

## Work items

| Item | Done | Notes |
|------|:----:|-------|
| Modify Header | ✅ | Dialog → `RecordEdit::Replace` |
| Modify Sequence | ✅ | Dialog; FASTQ length must match quality |
| Modify Quality | ✅ | FASTQ only |
| Delete current record | ✅ | `RecordEdit::Delete` |
| Replace current record | ✅ | Via field edits / rebuild |
| Insert record | ✅ | Insert before/after UI |
| `EditOverlay` | ✅ | `seqflash-ops::overlay` |
| Dirty / edit status display | ✅ | Tab `*`, status bar, list badges |
| Undo last record-level op | ✅ | Toolbar + panel + Ctrl+Z |
| Redo | ✅ | Toolbar + panel + Ctrl+Y |
| Streaming Save As | ✅ | `save_*_with_overlay_ex` |
| Save progress | ✅ | Progress window + status bar % |
| Save cancel | ✅ | Cancels worker; deletes temp file |
| External file change check | ✅ | Before start + after write (fingerprint) |
| Overlay preview (delete/replace) | ✅ | Central panel + list badges + stats |

## Acceptance criteria (plan)

| Criterion | Evidence |
|-----------|----------|
| Original mmap always read-only | `Document` / `FileBytes` never write; edits only in overlay |
| Overlay does not modify source | Save refuses open source path; worker re-opens read-only |
| Delete and replace preview correctly | List `[DEL]`/`[EDIT]`; central **Overlay preview**; right-panel effective fields/stats |
| Save produces a new file | Temp sibling + atomic rename |
| Save failure does not corrupt source | Errors clean temp; source untouched |
| Cancel cleans temp file | `ExportError::Cancelled` + remove temp; unit test |
| Over-threshold records stay read-only | `record_edit_limit_bytes` / `RECORD_EDIT_LIMIT_BYTES` gate |
| Undo/Redo does not break overlay order | Stacked undo/redo unit tests; export last-Replace-wins |

## Manual smoke checklist

1. Open a small FASTA and a small FASTQ; wait until indexing completes.  
2. Select a record → **Edit Header…** / **Edit Seq…** → Apply → list shows `[EDIT]`, central preview updates, source view still shows original bytes.  
3. **Delete record** → list strikethrough + `[DEL]`; preview says deleted.  
4. **Undo** / **Redo** (buttons and Ctrl+Z / Ctrl+Y).  
5. **Insert…** before/after → green insert blocks in central preview.  
6. **Save edits…** to a **new** path → progress window; open the new file in SeqFlash and confirm edits.  
7. Start save again and **Cancel** → no target file (or incomplete temp removed); overlay still dirty.  
8. Attempt save to the **same** path as the open source → error, no write.  
9. Edit a huge synthetic record only if limit is raised; with default 64 MiB limit, oversized records refuse edit dialogs.  
10. Modify source on disk while document open → **Check source** / save start reports external change.

## Automated checks

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo build --workspace --release
```

Relevant unit coverage:

- `seqflash-ops` overlay apply/undo/redo/clear  
- `seqflash-ops` overlay export delete/replace/insert/last-wins  
- `seqflash-ops` overlay export progress + cancel cleanup  

## Known limitations (defer to M8+)

- Main virtual scroller still shows **source** bytes only; effective content is in the overlay preview panel (by design for M7 safety/perf).  
- No full-file visual “diff” or multi-record selection editor.  
- Background save is a dedicated worker, not yet the general `seqflash-jobs` system.  
- Insert/replace of multi-line original FASTQ is normalized on edit to a simple layout when rebuilding from fields.

## Next milestone

**M8 — Stability and performance:** benches, fuzzing, long-run memory, task revision races, error UX.
