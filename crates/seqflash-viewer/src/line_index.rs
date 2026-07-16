//! Sparse line-checkpoint index for virtual scrolling over a large byte buffer.
//!
//! Instead of recording every newline (which would cost ~1 entry/line and
//! explode for huge files), we keep one checkpoint roughly every
//! [`CHECKPOINT_INTERVAL_BYTES`]. To find the line at a given byte offset we
//! binary-search to the nearest preceding checkpoint, then scan forward from
//! there (a few hundred KB at most) — see plan section 12.3.

/// Record one checkpoint every ~1 MiB of scanned input. Tuned so a 4 GiB file
/// yields ~4096 entries (~50 KB of index), and the worst-case local rescan to
/// reach any offset is under 1 MiB.
pub const CHECKPOINT_INTERVAL_BYTES: u64 = 1024 * 1024;

/// One sparse checkpoint: "file offset `byte_offset` is the start of line
/// `line_index`" (0-based).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineCheckpoint {
    pub byte_offset: u64,
    pub line_index: u64,
}

/// Incrementally-built sparse line index.
///
/// Scanning happens in small chunks on the UI thread (see
/// [`LineIndex::scan_chunk`]); the index is append-only after construction.
#[derive(Clone, Debug)]
pub struct LineIndex {
    /// Always sorted ascending by `byte_offset`. The first entry is the
    /// implicit `{ offset: 0, line: 0 }` for the start of the file.
    checkpoints: Vec<LineCheckpoint>,
    /// Highest byte offset scanned so far.
    scan_progress: u64,
    /// Total size of the backing buffer; scanning stops here.
    file_size: u64,
    /// Number of newline bytes consumed so far (i.e. completed lines).
    lines_seen: u64,
    /// Byte offset of the next checkpoint we will emit.
    next_checkpoint_at: u64,
    scan_complete: bool,
}

impl LineIndex {
    /// Create a fresh index for a buffer of `file_size` bytes. The first
    /// checkpoint (offset 0, line 0) is seeded immediately.
    #[must_use]
    pub fn new(file_size: u64) -> Self {
        // Empty file: one checkpoint, already complete.
        let scan_complete = file_size == 0;
        Self {
            checkpoints: vec![LineCheckpoint {
                byte_offset: 0,
                line_index: 0,
            }],
            scan_progress: 0,
            file_size,
            lines_seen: 0,
            next_checkpoint_at: CHECKPOINT_INTERVAL_BYTES,
            scan_complete,
        }
    }

    /// Highest scanned byte offset.
    #[must_use]
    pub const fn scan_progress(&self) -> u64 {
        self.scan_progress
    }

    /// Whether the whole file has been scanned.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.scan_complete
    }

    /// Total file size the index was built for.
    #[must_use]
    pub const fn file_size(&self) -> u64 {
        self.file_size
    }

    /// Number of checkpoints currently stored (mostly for tests/diagnostics).
    #[must_use]
    pub fn checkpoint_count(&self) -> usize {
        self.checkpoints.len()
    }

    /// Total number of lines counted so far (approximate until complete).
    #[must_use]
    pub const fn lines_seen(&self) -> u64 {
        self.lines_seen
    }

    /// Advance the scan over `bytes` by at most `budget` bytes from the current
    /// cursor. Updates checkpoints and line counts. Cheap to call each frame.
    pub fn scan_chunk(&mut self, bytes: &[u8], budget: u64) {
        if self.scan_complete {
            return;
        }
        let start = self.scan_progress;
        let limit = self.file_size.min(start.saturating_add(budget));
        // Bounds: file_size <= bytes.len(); convert once, checked.
        let start_us = usize::try_from(start).unwrap_or(usize::MAX);
        let limit_us = usize::try_from(limit)
            .unwrap_or(bytes.len())
            .min(bytes.len());
        if start_us >= limit_us || start_us >= bytes.len() {
            self.scan_complete = self.scan_progress >= self.file_size;
            return;
        }

        let window = &bytes[start_us..limit_us];
        // memchr-style scan for b'\n'. We do it bytewise here (no extra dep) —
        // the budget is small enough (~4 MiB) that this is a few ms.
        let mut local_offset = 0u64;
        for (i, &b) in window.iter().enumerate() {
            if b == b'\n' {
                self.lines_seen += 1;
                let here = start + i as u64 + 1; // byte after the newline
                if here >= self.next_checkpoint_at {
                    self.checkpoints.push(LineCheckpoint {
                        byte_offset: here,
                        line_index: self.lines_seen,
                    });
                    self.next_checkpoint_at = here + CHECKPOINT_INTERVAL_BYTES;
                }
                local_offset = (i as u64) + 1;
            } else {
                local_offset = (i as u64) + 1;
            }
        }

        self.scan_progress = start + local_offset;
        if self.scan_progress >= self.file_size {
            self.scan_complete = true;
        }
    }

    /// Find the checkpoint at or before `byte_offset` (binary search).
    /// Returns the implicit `{0,0}` checkpoint if `byte_offset` is before the
    /// first recorded checkpoint.
    #[must_use]
    pub fn checkpoint_before(&self, byte_offset: u64) -> LineCheckpoint {
        // checkpoints are sorted ascending by byte_offset; find the rightmost
        // entry whose byte_offset <= byte_offset.
        let idx = self
            .checkpoints
            .partition_point(|c| c.byte_offset <= byte_offset);
        // idx is the first entry > byte_offset; so idx-1 is the answer (>= 0).
        self.checkpoints[idx.saturating_sub(1).min(self.checkpoints.len() - 1)]
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn build_full_index(contents: &[u8]) -> LineIndex {
        let mut idx = LineIndex::new(contents.len() as u64);
        // Scan everything in one big chunk.
        idx.scan_chunk(contents, u64::MAX);
        idx
    }

    #[test]
    fn empty_file_is_immediately_complete() {
        let idx = LineIndex::new(0);
        assert!(idx.is_complete());
        assert_eq!(idx.checkpoint_count(), 1);
        assert_eq!(idx.checkpoint_before(0).byte_offset, 0);
    }

    #[test]
    fn counts_lines_in_small_input() {
        // 3 newlines => 3 completed lines; the trailing "def" is a 4th
        // incomplete line but not counted as a completed line.
        let idx = build_full_index(b"abc\ndef\nghi\ndef");
        assert!(idx.is_complete());
        assert_eq!(idx.lines_seen(), 3);
    }

    #[test]
    fn checkpoint_before_clamps_to_first() {
        let idx = build_full_index(b"abc\ndef\n");
        // Offset 0 is at/before the first checkpoint (also offset 0).
        let cp = idx.checkpoint_before(0);
        assert_eq!(cp.byte_offset, 0);
        assert_eq!(cp.line_index, 0);
    }

    #[test]
    fn checkpoint_before_finds_preceding_entry() {
        // Need input spanning multiple CHECKPOINT_INTERVAL_BYTES so that more
        // than the initial checkpoint is recorded.
        let mut data = Vec::new();
        // ~7 bytes/line * 400_000 lines ≈ 2.8 MiB => >= 2 checkpoints.
        for i in 0..400_000 {
            data.extend_from_slice(format!("line{i}\n").as_bytes());
        }
        let idx = build_full_index(&data);
        assert!(
            idx.checkpoint_count() > 1,
            "expected multiple checkpoints, got {}",
            idx.checkpoint_count()
        );
        // A probe past the first checkpoint must not return offset 0 unless
        // it's genuinely before the second checkpoint.
        let probe = (CHECKPOINT_INTERVAL_BYTES + 10).min(data.len() as u64);
        let cp = idx.checkpoint_before(probe);
        assert!(cp.byte_offset <= probe);
        assert!(cp.byte_offset > 0, "should have advanced past offset 0");
    }

    #[test]
    fn scan_chunk_is_incremental_and_idempotent() {
        // Scanning the same data in two halves yields the same line count and
        // final progress as scanning it all at once.
        let data = b"aaaa\nbbbb\ncccc\ndddd\n";
        let mut all_at_once = LineIndex::new(data.len() as u64);
        all_at_once.scan_chunk(data, u64::MAX);

        let mut half_half = LineIndex::new(data.len() as u64);
        half_half.scan_chunk(data, 8);
        half_half.scan_chunk(data, u64::MAX);

        assert_eq!(all_at_once.lines_seen(), half_half.lines_seen());
        assert_eq!(all_at_once.scan_progress(), half_half.scan_progress());
        assert!(half_half.is_complete());
    }

    #[test]
    fn checkpoint_before_past_end_clamps_to_last() {
        let data = b"abc\ndef\n";
        let idx = build_full_index(data);
        let cp = idx.checkpoint_before(u64::MAX);
        // Last checkpoint is offset 0 here (small input); must still be valid.
        assert!(cp.byte_offset <= data.len() as u64);
    }
}
