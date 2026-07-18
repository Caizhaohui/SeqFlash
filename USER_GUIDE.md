# SeqFlash — User Guide

SeqFlash is a Windows desktop browser for large FASTA/FASTQ files. It lets you
open, browse, search, inspect, and export biosequence files — even those
several gigabytes in size — without copying the whole file into memory.

## Installation

SeqFlash is distributed as a **portable ZIP**. No setup, no install.

1. Download `SeqFlash-portable-x86_64.zip` from the
   [Releases](https://github.com/Caizhaohui/SeqFlash/releases) page.
2. Extract the ZIP to any folder (e.g. `C:\Tools\SeqFlash`).
3. Run `SeqFlash.exe`.

**To associate file extensions** so double-clicking `.fa` / `.fastq` opens
SeqFlash:

```powershell
.\scripts\register-file-assoc.ps1 -SeqFlashPath "C:\Tools\SeqFlash\SeqFlash.exe"
```

To remove the association:

```powershell
.\scripts\register-file-assoc.ps1 -Unregister
```

## Supported Formats

| Format | Extensions | Notes |
|---|---|---|
| FASTA | `.fa`, `.fasta`, `.fna`, `.ffn`, `.faa`, `.frn` | Single/multi-line sequences, IUPAC codes |
| FASTQ | `.fq`, `.fastq` | Single/multi-line seq & quality, Phred+33 |

Files are always opened **read-only**. SeqFlash never modifies the original
file.

## Basic Operations

### Opening Files
- **Menu**: Click "Open…" in the toolbar.
- **Drag & drop**: Drag a `.fasta` or `.fastq` file onto the window.
- **Command line**: `SeqFlash.exe path/to/file.fasta`
- **Double-click**: If file associations are registered.

### Browsing
- **Scroll**: Mouse wheel / Page Up / Page Down / Home / End.
- **Jump to offset**: Click "Go to offset…" in the toolbar, enter a byte
  position.
- **Record navigation**: Use the left panel to see indexed records. Click a
  record to jump to it. Use ◀/▶ to move between records.

### View Modes
- **Raw text view** (center): The virtual-scrolling viewer shows file contents
  line-by-line, with byte offsets.
- **Record info** (right panel): Shows statistics for the current record
  (length, GC%, N count, quality for FASTQ).
- **Record list** (left panel): Indexed records with their IDs.

### Searching
- Use the search bar in the left panel. Modes:
  - **Bytes**: Raw byte search across the whole file.
  - **ID**: Exact or prefix match on record IDs (requires FASTA/FASTQ index).
- Results appear in the search results list. Click a result to jump.
- Use ◀/▶ to navigate between results.

### Exporting
- **Copy**: Click "Copy Header" / "Copy Seq" / "Copy Qual" in the right panel
  to copy to the clipboard.
- **Save As**: Click "Save As…" to export the current record to a new file.
  Use `Transform.None` for original data, or select a transform
  (ReverseComplement, Uppercase, Lowercase).

### Editing (record-level)
- SeqFlash supports limited record-level edits through an in-memory overlay:
  - **Edit Header**: Modify the current record's header.
  - **Edit Sequence**: Modify the current record's sequence.
  - **Delete / Replace / Insert**: Structural edits.
  - **Undo / Redo**: Reverse or re-apply recent edits.
- **Save with changes**: Writes a new file with overlay edits applied. The
  original file is never modified.

## Performance Tips

- SeqFlash uses **memory mapping** (mmap). Files up to 4 GiB can be opened
  without significant memory overhead (~100 MiB private memory for a 1 GiB
  file).
- The FASTA/FASTQ index builds incrementally in the background while you
  browse. The first screen appears immediately.
- For best performance, place large files on an SSD.

## Settings

Settings are stored in `%LOCALAPPDATA%\SeqFlash\seqflash-settings.json`:
- **Theme**: Light, Dark, or System (follows Windows).
- **Reopen previous session**: Opens files from the last session on startup.
- **Wrap width**: Line width for FASTA export wrap mode.
- **Edit limit**: Maximum record size allowed for direct editing (default 64
  MiB).

## Troubleshooting

- **File won't open?** Check that the file is not exclusively locked by another
  program.
- **Indexing is slow?** Large files (4 GiB+) may take a few minutes to fully
  index. You can still browse while indexing is in progress.
- **App crashes?** Check the log files in `%LOCALAPPDATA%\SeqFlash\logs`. File
  a bug report at https://github.com/Caizhaohui/SeqFlash/issues.

## License

SeqFlash is released under the MIT License. See [LICENSE](./LICENSE).
