//! Incremental FASTQ record index using the state-machine parser.

use seqflash_formats::{parse_single_record, FastqRecordEntry};

/// Default bytes to scan per frame.
pub const FASTQ_INDEX_BUDGET: u64 = 2 * 1024 * 1024;

/// Incrementally-built FASTQ record index.
#[derive(Clone, Debug)]
pub struct FastqIndex {
    entries: Vec<FastqRecordEntry>,
    scan_progress: u64,
    file_size: u64,
    scan_complete: bool,
    cancelled: bool,
}

impl FastqIndex {
    #[must_use]
    pub fn new(file_size: u64) -> Self {
        Self {
            entries: Vec::new(),
            scan_progress: 0,
            file_size,
            scan_complete: file_size == 0,
            cancelled: false,
        }
    }

    pub fn cancel(&mut self) {
        self.cancelled = true;
    }

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
    pub fn entries(&self) -> &[FastqRecordEntry] {
        &self.entries
    }
    #[must_use]
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }
    #[must_use]
    pub fn errors(&self) -> Vec<&FastqRecordEntry> {
        self.entries
            .iter()
            .filter(|e| !e.validation.valid)
            .collect()
    }

    #[must_use]
    pub fn entry_at_offset(&self, byte_offset: u64) -> Option<&FastqRecordEntry> {
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

    /// Incrementally scan up to `budget` bytes.
    pub fn scan_chunk(&mut self, bytes: &[u8], budget: u64) {
        if self.scan_complete || self.cancelled {
            return;
        }
        let start = self.scan_progress;
        let limit = self
            .file_size
            .min(start.saturating_add(budget))
            .min(bytes.len() as u64);
        let mut pos = usize::try_from(start).unwrap_or(0);
        let limit_us = usize::try_from(limit)
            .unwrap_or(bytes.len())
            .min(bytes.len());
        if pos >= limit_us || pos >= bytes.len() {
            self.scan_complete = self.scan_progress >= self.file_size;
            return;
        }
        // Scan for '@' at line start, then parse a complete record.
        while pos < limit_us && pos < bytes.len() {
            let b = bytes[pos];
            if b == b'@' && (pos == 0 || bytes[pos - 1] == b'\n' || bytes[pos - 1] == b'\r') {
                let rec_num = self.entries.len() as u64;
                match parse_single_record(bytes, pos, rec_num) {
                    Ok((entry, next)) => {
                        self.entries.push(entry);
                        pos = next;
                    }
                    Err(_) => {
                        // Skip past this position
                        pos += 1;
                    }
                }
            } else {
                pos += 1;
            }
        }
        self.scan_progress = pos as u64;
        if self.scan_progress >= self.file_size || self.scan_progress >= bytes.len() as u64 {
            self.scan_complete = true;
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn build(data: &[u8]) -> FastqIndex {
        let mut idx = FastqIndex::new(data.len() as u64);
        idx.scan_chunk(data, u64::MAX);
        idx
    }

    #[test]
    fn indexes_two_records() {
        let data = b"@r1\nA\n+\nI\n@r2\nC\n+\nJ\n";
        let idx = build(data);
        assert_eq!(idx.entry_count(), 2);
        assert!(idx.entries[0].validation.valid);
        assert!(idx.entries[1].validation.valid);
    }

    #[test]
    fn errors_filters_invalid() {
        let data = b"@r1\nA\n+\nI\n@r2\nA\n+\n\n"; // r2 has empty quality
        let idx = build(data);
        assert_eq!(idx.errors().len(), 1);
        assert_eq!(idx.errors()[0].record_number, 1);
    }

    #[test]
    fn entry_at_offset() {
        let data = b"@r1\nA\n+\nI\n@r2\nC\n+\nJ\n";
        let idx = build(data);
        assert!(idx.entry_at_offset(0).is_some());
        assert!(idx.entry_at_offset(12).is_some());
        assert!(idx.entry_at_offset(u64::MAX).is_none());
    }

    #[test]
    fn incremental_scan() {
        let data = b"@r1\nA\n+\nI\n@r2\nC\n+\nJ\n";
        let mut idx = FastqIndex::new(data.len() as u64);
        idx.scan_chunk(data, 5);
        assert_eq!(idx.entry_count(), 1);
        idx.scan_chunk(data, u64::MAX);
        assert_eq!(idx.entry_count(), 2);
    }

    #[test]
    fn empty_index_for_non_fastq() {
        let data = b"just plain\n";
        let idx = build(data);
        assert_eq!(idx.entry_count(), 0);
    }

    #[test]
    fn cancel_stops() {
        let data = b"@r1\nA\n+\nI\n@r2\nC\n+\nJ\n";
        let mut idx = FastqIndex::new(data.len() as u64);
        idx.cancel();
        idx.scan_chunk(data, u64::MAX);
        assert_eq!(idx.entry_count(), 0);
    }
}
