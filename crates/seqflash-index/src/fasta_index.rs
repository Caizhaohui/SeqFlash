//! FASTA record boundary indexing with incremental scanning.

use seqflash_formats::parse_fasta_header;
use seqflash_types::ByteRange;

/// Default bytes to scan per frame during incremental indexing.
pub const DEFAULT_INDEX_SCAN_BUDGET: u64 = 4 * 1024 * 1024;

/// One indexed FASTA record, as defined in plan section 13.2.
#[derive(Clone, Debug)]
pub struct FastaRecordEntry {
    pub record_number: u64,
    /// Absolute file offset of the record's first byte (the `>`).
    pub start_offset: u64,
    /// Exclusive end offset — the first byte past the record (next `>` or EOF).
    pub end_offset: u64,
    /// Byte range of the header line (absolute file offsets of `>`..`\n`).
    pub header_range: ByteRange,
    /// Byte range of the record ID (absolute file offsets within the header).
    pub id_range: ByteRange,
}

/// Incrementally-built FASTA record index.
///
/// Scans the buffer in chunks (default [`DEFAULT_INDEX_SCAN_BUDGET`]) per call
/// to [`scan_chunk`], emitting [`FastaRecordEntry`] records as new `>`-starting
/// lines are found. Designed to be called once per UI frame so the first screen
/// appears immediately and indexing progresses behind it.
///
/// State machine: when a `>` at line start is encountered, a pending record is
/// opened (recording its start offset, header range, and parsed ID). When the
/// next `>` at line start (or EOF) is reached, the pending record is finalized
/// with its end offset.
#[derive(Clone, Debug)]
pub struct FastaIndex {
    entries: Vec<FastaRecordEntry>,
    scan_progress: u64,
    file_size: u64,
    scan_complete: bool,
    cancelled: bool,
    /// Start offset of the record being accumulated, or `None` between records.
    pending_start: Option<u64>,
    /// End of the pending record's header line (absolute offset past `\n`).
    pending_header_end: Option<u64>,
    /// Parsed ID range of the pending record (absolute file offsets).
    pending_id_range: Option<ByteRange>,
}

impl FastaIndex {
    /// Create a fresh index for a file of `file_size` bytes.
    #[must_use]
    pub fn new(file_size: u64) -> Self {
        Self {
            entries: Vec::new(),
            scan_progress: 0,
            file_size,
            scan_complete: file_size == 0,
            cancelled: false,
            pending_start: None,
            pending_header_end: None,
            pending_id_range: None,
        }
    }

    /// Signal cancellation; future calls to [`scan_chunk`] become no-ops.
    pub fn cancel(&mut self) {
        self.cancelled = true;
    }

    // ---- Accessors ----

    #[must_use]
    pub const fn scan_progress(&self) -> u64 {
        self.scan_progress
    }
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.scan_complete
    }
    #[must_use]
    pub const fn is_cancelled(&self) -> bool {
        self.cancelled
    }
    #[must_use]
    pub fn entries(&self) -> &[FastaRecordEntry] {
        &self.entries
    }
    #[must_use]
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Binary search for the record that contains `byte_offset`.
    /// Returns `None` when no records have been indexed yet or the offset
    /// falls past the last record.
    #[must_use]
    pub fn entry_at_offset(&self, byte_offset: u64) -> Option<&FastaRecordEntry> {
        if self.entries.is_empty() {
            return None;
        }
        let idx = self
            .entries
            .partition_point(|e| e.start_offset <= byte_offset);
        let idx = idx.saturating_sub(1);
        let e = &self.entries[idx];
        if byte_offset >= e.start_offset && byte_offset < e.end_offset {
            Some(e)
        } else {
            None
        }
    }

    /// Incrementally scan `budget` bytes from the current position.
    ///
    /// May be called repeatedly; each invocation makes progress until the file
    /// is fully indexed or [`cancel`] is called. Appends new entries.
    ///
    /// # Panics
    /// Never — all index operations are bounds-checked.
    pub fn scan_chunk(&mut self, bytes: &[u8], budget: u64) {
        if self.scan_complete || self.cancelled {
            return;
        }

        let start = self.scan_progress;
        // We need one byte before `start` for is_line_start detection; that
        // is always available because we have the full `bytes` buffer.
        let start_us = usize::try_from(start).unwrap_or(0);
        let limit = self
            .file_size
            .min(start.saturating_add(budget))
            .min(bytes.len() as u64);
        let limit_us = usize::try_from(limit)
            .unwrap_or(bytes.len())
            .min(bytes.len());
        if start_us >= limit_us || start_us >= bytes.len() {
            self.scan_complete = self.scan_progress >= self.file_size;
            return;
        }
        let window = &bytes[start_us..limit_us];
        let mut i = 0usize;
        while i < window.len() {
            let abs_pos = start + i as u64;
            let b = window[i];

            if b == b'>'
                && (abs_pos == 0 || bytes[usize::try_from(abs_pos - 1).unwrap_or(0)] == b'\n')
            {
                // Close the previous pending record (if any) at this boundary.
                if let Some(prev_start) = self.pending_start {
                    self.entries.push(FastaRecordEntry {
                        record_number: self.entries.len() as u64,
                        start_offset: prev_start,
                        end_offset: abs_pos,
                        header_range: ByteRange::new(
                            prev_start,
                            self.pending_header_end.unwrap_or(abs_pos).min(abs_pos),
                        )
                        .unwrap_or(ByteRange {
                            start: prev_start,
                            end: prev_start + 1,
                        }),
                        id_range: self
                            .pending_id_range
                            .unwrap_or(ByteRange { start: 0, end: 0 }),
                    });
                }

                // Open a new pending record.
                self.pending_start = Some(abs_pos);
                self.pending_header_end = None;
                self.pending_id_range = None;

                // Scan forward within the window (or a bit beyond) to find
                // the end of the header line for ID extraction.
                let header_limit = (abs_pos + 4096) // headers are rarely >4 KiB
                    .min(self.file_size)
                    .min(bytes.len() as u64);
                let scan_from = abs_pos + 1; // skip '>'
                let header_bytes_end = (scan_from..header_limit)
                    .map(|o| usize::try_from(o).unwrap_or(0))
                    .find(|&o| o < bytes.len() && (bytes[o] == b'\n' || bytes[o] == b'\r'))
                    .map_or_else(
                        || usize::try_from(header_limit).unwrap_or(0),
                        |o| {
                            if bytes[o] == b'\r' && o + 1 < bytes.len() && bytes[o + 1] == b'\n' {
                                o + 2
                            } else {
                                o + 1
                            }
                        },
                    );

                self.pending_header_end = Some(header_bytes_end as u64);

                // Parse the header line for the ID.
                let hdr_slice = &bytes
                    [usize::try_from(abs_pos).unwrap_or(0)..header_bytes_end.min(bytes.len())];
                let parsed = parse_fasta_header(hdr_slice);
                let abs_id_start = abs_pos + parsed.id_range.start;
                let abs_id_end = abs_pos + parsed.id_range.end;
                self.pending_id_range = Some(
                    ByteRange::new(abs_id_start, abs_id_end)
                        .unwrap_or(ByteRange { start: 0, end: 0 }),
                );
            }

            i += 1;
        }

        self.scan_progress = start + i as u64;

        // At EOF, finalize the last pending record.
        if self.scan_progress >= self.file_size || self.scan_progress >= bytes.len() as u64 {
            if let Some(prev_start) = self.pending_start.take() {
                let header_end = self.pending_header_end.unwrap_or(prev_start + 1);
                self.entries.push(FastaRecordEntry {
                    record_number: self.entries.len() as u64,
                    start_offset: prev_start,
                    end_offset: self.scan_progress,
                    header_range: ByteRange::new(prev_start, header_end).unwrap_or(ByteRange {
                        start: prev_start,
                        end: prev_start + 1,
                    }),
                    id_range: self
                        .pending_id_range
                        .unwrap_or(ByteRange { start: 0, end: 0 }),
                });
                self.pending_start = None;
                self.pending_header_end = None;
                self.pending_id_range = None;
            }
            self.scan_complete = true;
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn build_index(data: &[u8]) -> FastaIndex {
        let mut idx = FastaIndex::new(data.len() as u64);
        idx.scan_chunk(data, u64::MAX);
        idx
    }

    #[test]
    fn indexes_single_record() {
        let data = b">seq1\nACGT\n";
        let idx = build_index(data);
        assert!(idx.is_complete());
        assert_eq!(idx.entry_count(), 1);
        let e = &idx.entries[0];
        assert_eq!(e.start_offset, 0);
    }

    #[test]
    fn indexes_two_records() {
        let data = b">seq1\nACGT\n>seq2\nTGCA\n";
        let idx = build_index(data);
        assert_eq!(idx.entry_count(), 2);
        assert_eq!(idx.entries[0].start_offset, 0);
        assert_eq!(idx.entries[1].start_offset, 11, ">seq2 is at byte 11");
        assert_eq!(idx.entries[0].end_offset, 11);
        assert_eq!(idx.entries[1].end_offset, data.len() as u64);
    }

    #[test]
    fn entry_at_offset_finds_record() {
        let data = b">seq1\nAAAA\n>seq2\nCCCC\n";
        let idx = build_index(data);
        assert_eq!(idx.entry_at_offset(0).unwrap().record_number, 0);
        assert_eq!(idx.entry_at_offset(6).unwrap().record_number, 0); // in seq1
        assert_eq!(idx.entry_at_offset(12).unwrap().record_number, 1); // at seq2 start
        assert!(idx.entry_at_offset(u64::MAX).is_none());
    }

    #[test]
    fn incremental_scan_is_consistent() {
        let data = b">seq1\nAAAA\n>seq2\nCCCC\n>seq3\nGGGG\n";
        let mut idx = FastaIndex::new(data.len() as u64);
        idx.scan_chunk(data, 10);
        idx.scan_chunk(data, 10);
        idx.scan_chunk(data, u64::MAX);
        assert_eq!(idx.entry_count(), 3);
        assert!(idx.is_complete());
    }

    #[test]
    fn empty_file() {
        let idx = FastaIndex::new(0);
        assert!(idx.is_complete());
        assert_eq!(idx.entry_count(), 0);
    }

    #[test]
    fn no_records_yields_empty_index() {
        let idx = build_index(b"just some text\nno fasta here\n");
        assert!(idx.is_complete());
        assert_eq!(idx.entry_count(), 0);
    }

    #[test]
    fn cancel_stops_scanning() {
        let data = b">seq1\nAAAA\n>seq2\nCCCC\n";
        let mut idx = FastaIndex::new(data.len() as u64);
        idx.cancel();
        idx.scan_chunk(data, u64::MAX);
        assert!(idx.is_cancelled());
        assert_eq!(idx.entry_count(), 0);
    }

    #[test]
    fn id_range_from_header() {
        let data = b">chr1 description\nAAAA\n";
        let idx = build_index(data);
        let e = &idx.entries[0];
        // ID "chr1" starts 1 byte after '>' (at offset 1).
        assert_eq!(e.id_range.start, 1);
        assert_eq!(e.id_range.end, 5);
        // header_range covers the whole ">chr1 description\n"
        assert_eq!(e.header_range.start, 0);
        assert!(e.header_range.end > 5);
    }

    #[test]
    fn handles_crlf() {
        let data = b">seq1\r\nACGT\r\n>seq2\r\n";
        let idx = build_index(data);
        assert_eq!(idx.entry_count(), 2);
    }

    #[test]
    fn no_trailing_newline() {
        let data = b">seq1\nACGT";
        let idx = build_index(data);
        assert_eq!(idx.entry_count(), 1);
        assert_eq!(idx.entries[0].end_offset, data.len() as u64);
    }
}
