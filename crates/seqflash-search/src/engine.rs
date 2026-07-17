//! Incremental search engine.
//!
//! The engine processes file bytes in chunks (called once per UI frame) and
//! appends matches to a bounded result list. Byte search uses `memchr` for
//! the first byte then verifies the rest; ID search operates on already-built
//! record index entries and completes in one pass.

use seqflash_types::ByteRange;

use crate::types::{SearchMode, SearchResult, MAX_RESULTS};

/// Preview length (bytes) shown alongside each result.
const PREVIEW_BYTES: usize = 48;

/// One incremental search session for a single document.
pub struct SearchSession {
    mode: SearchMode,
    pattern: Vec<u8>,
    case_sensitive: bool,
    results: Vec<SearchResult>,
    /// Byte position up to which the file has been scanned.
    scan_pos: u64,
    file_size: u64,
    complete: bool,
    cancelled: bool,
    max_results: usize,
}

impl SearchSession {
    /// Create a new byte-search session (RawBytes / SequenceFragment /
    /// CurrentRecord / FromPosition).
    #[must_use]
    pub fn new(mode: SearchMode, pattern: Vec<u8>, case_sensitive: bool, file_size: u64) -> Self {
        let is_empty = pattern.is_empty();
        Self {
            mode,
            pattern,
            case_sensitive,
            results: Vec::new(),
            scan_pos: 0,
            file_size,
            complete: is_empty || file_size == 0,
            cancelled: false,
            max_results: MAX_RESULTS,
        }
    }

    /// Create a session that starts scanning from a given offset (for
    /// FromPosition mode).
    #[must_use]
    pub fn from_offset(
        mode: SearchMode,
        pattern: Vec<u8>,
        case_sensitive: bool,
        start: u64,
        file_size: u64,
    ) -> Self {
        let mut s = Self::new(mode, pattern, case_sensitive, file_size);
        s.scan_pos = start;
        s
    }

    /// Limit the number of results retained.
    pub fn set_max_results(&mut self, max: usize) {
        self.max_results = max;
    }

    #[must_use]
    pub fn mode(&self) -> SearchMode {
        self.mode
    }

    #[must_use]
    pub fn results(&self) -> &[SearchResult] {
        &self.results
    }

    #[must_use]
    pub const fn scan_progress(&self) -> u64 {
        self.scan_pos
    }

    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.complete
    }

    #[must_use]
    pub const fn is_cancelled(&self) -> bool {
        self.cancelled
    }

    pub fn cancel(&mut self) {
        self.cancelled = true;
    }

    #[must_use]
    pub fn result_count(&self) -> usize {
        self.results.len()
    }

    /// Advance the byte search by at most `budget` bytes from the current
    /// position. Uses `memchr` for the first pattern byte, then verifies the
    /// full pattern. Called once per UI frame.
    pub fn search_chunk(&mut self, bytes: &[u8], budget: u64) {
        if self.complete || self.cancelled || self.pattern.is_empty() {
            return;
        }
        if self.results.len() >= self.max_results {
            self.complete = true;
            return;
        }

        let start = usize::try_from(self.scan_pos).unwrap_or(0).min(bytes.len());
        let pat_len = self.pattern.len();
        let limit = self
            .file_size
            .min(self.scan_pos.saturating_add(budget))
            .min(bytes.len() as u64);
        // Extend the search window past `limit` by pattern_len so matches that
        // straddle the chunk boundary are not missed. `scan_pos` still advances
        // to `limit`, so the overlap region is re-scanned next chunk (harmless).
        let search_limit_us =
            (usize::try_from(limit).unwrap_or(bytes.len()) + pat_len).min(bytes.len());
        let limit_us = search_limit_us;

        if start >= limit_us {
            self.complete = true;
            return;
        }

        let pat = if self.case_sensitive {
            self.pattern.clone()
        } else {
            self.pattern
                .iter()
                .map(|&b| b.to_ascii_lowercase())
                .collect()
        };

        // We need to look slightly past `limit_us` to catch matches that start
        // inside the window but extend beyond it. Extend by pattern length.
        let search_end = limit_us.min(bytes.len());
        let extended = &bytes[start..search_end];

        let mut offset = 0usize;
        loop {
            if self.results.len() >= self.max_results {
                self.complete = true;
                break;
            }
            let abs_base = start + offset;
            let remaining = &extended[offset..];

            let found = if self.case_sensitive {
                find_substring(remaining, &self.pattern)
            } else {
                find_substring_caseless(remaining, &pat)
            };

            match found {
                Some(rel) => {
                    let match_start = abs_base + rel;
                    let match_end = match_start + self.pattern.len();
                    let range = ByteRange::new(match_start as u64, match_end as u64)
                        .unwrap_or(ByteRange { start: 0, end: 0 });
                    self.results.push(SearchResult::with_preview(
                        bytes,
                        range,
                        None,
                        PREVIEW_BYTES,
                    ));
                    offset += rel + 1; // advance past match (allow overlaps)
                    if offset >= extended.len() {
                        break;
                    }
                }
                None => break,
            }
        }

        self.scan_pos = limit;
        if self.scan_pos >= self.file_size || limit_us >= bytes.len() {
            self.complete = true;
        }
    }

    /// Search record IDs against already-indexed entries.
    /// `id_extractor` returns the ID bytes for entry `i`.
    /// Completes in one pass (ID search is fast — no chunking needed).
    pub fn search_ids<F>(&mut self, entry_count: usize, id_extractor: F)
    where
        F: Fn(usize) -> Vec<u8>,
    {
        if self.cancelled || self.pattern.is_empty() {
            self.complete = true;
            return;
        }
        let pat = if self.case_sensitive {
            self.pattern.clone()
        } else {
            self.pattern
                .iter()
                .map(|&b| b.to_ascii_lowercase())
                .collect()
        };
        for i in 0..entry_count {
            if self.results.len() >= self.max_results {
                break;
            }
            let id_bytes = id_extractor(i);
            let matches = match self.mode {
                SearchMode::RecordIdExact => {
                    if self.case_sensitive {
                        id_bytes == self.pattern
                    } else {
                        id_bytes.eq_ignore_ascii_case(&self.pattern)
                    }
                }
                SearchMode::RecordIdPrefix => {
                    if id_bytes.len() < pat.len() {
                        false
                    } else if self.case_sensitive {
                        id_bytes[..pat.len()] == self.pattern[..]
                    } else {
                        id_bytes[..pat.len()].eq_ignore_ascii_case(&self.pattern[..pat.len()])
                    }
                }
                _ => false,
            };
            if matches {
                // We don't have byte offsets here; the caller resolves them.
                self.results.push(SearchResult {
                    byte_range: ByteRange { start: 0, end: 0 },
                    record_number: Some(i as u64),
                    preview: id_bytes,
                });
            }
        }
        self.complete = true;
    }
}

/// Find the first occurrence of `needle` in `haystack` (case-sensitive).
fn find_substring(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    // Use memchr to find the first byte, then verify the full pattern.
    let first = needle[0];
    let limit = haystack.len() - needle.len();
    let mut search_start = 0;
    loop {
        if search_start > limit {
            return None;
        }
        let found = memchr::memchr(first, &haystack[search_start..])?;
        let candidate = search_start + found;
        if candidate <= limit && &haystack[candidate..candidate + needle.len()] == needle {
            return Some(candidate);
        }
        search_start = candidate + 1;
    }
}

/// Case-insensitive substring search (both args pre-lowercased).
fn find_substring_caseless(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }
    let lower: Vec<u8> = haystack.iter().map(|&b| b.to_ascii_lowercase()).collect();
    find_substring(&lower, needle)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn byte_search_finds_matches() {
        let data = b"hello world hello again";
        let mut s = SearchSession::new(
            SearchMode::RawBytes,
            b"hello".to_vec(),
            true,
            data.len() as u64,
        );
        s.search_chunk(data, u64::MAX);
        assert_eq!(s.result_count(), 2);
        assert!(s.is_complete());
    }

    #[test]
    fn byte_search_case_insensitive() {
        let data = b"Hello HELLO hello";
        let mut s = SearchSession::new(
            SearchMode::RawBytes,
            b"hello".to_vec(),
            false,
            data.len() as u64,
        );
        s.search_chunk(data, u64::MAX);
        assert_eq!(s.result_count(), 3);
    }

    #[test]
    fn result_limit() {
        let data = b"aaaa aa aa aa aa aa aa aa";
        let mut s =
            SearchSession::new(SearchMode::RawBytes, b"a".to_vec(), true, data.len() as u64);
        s.set_max_results(3);
        s.search_chunk(data, u64::MAX);
        assert!(s.result_count() <= 3);
        assert!(s.is_complete());
    }

    #[test]
    fn incremental_consistency() {
        let data = b"xxx hello yyy hello zzz";
        let mut all = SearchSession::new(
            SearchMode::RawBytes,
            b"hello".to_vec(),
            true,
            data.len() as u64,
        );
        all.search_chunk(data, u64::MAX);

        let mut incr = SearchSession::new(
            SearchMode::RawBytes,
            b"hello".to_vec(),
            true,
            data.len() as u64,
        );
        incr.search_chunk(data, 8);
        incr.search_chunk(data, 8);
        incr.search_chunk(data, u64::MAX);
        assert_eq!(incr.result_count(), all.result_count());
    }

    #[test]
    fn cancel_stops() {
        let data = b"hello hello hello";
        let mut s = SearchSession::new(
            SearchMode::RawBytes,
            b"hello".to_vec(),
            true,
            data.len() as u64,
        );
        s.cancel();
        s.search_chunk(data, u64::MAX);
        assert_eq!(s.result_count(), 0);
    }

    #[test]
    fn empty_pattern_completes_immediately() {
        let s = SearchSession::new(SearchMode::RawBytes, vec![], true, 100);
        assert!(s.is_complete());
    }

    #[test]
    fn id_exact_match() {
        let ids = [b"seq1".to_vec(), b"seq2".to_vec(), b"seq3".to_vec()];
        let mut s = SearchSession::new(SearchMode::RecordIdExact, b"seq2".to_vec(), true, 0);
        s.search_ids(3, |i| ids[i].clone());
        assert_eq!(s.result_count(), 1);
        assert_eq!(s.results()[0].record_number, Some(1));
    }

    #[test]
    fn id_prefix_match() {
        let ids = [b"chr1".to_vec(), b"chr2".to_vec(), b"scaffold1".to_vec()];
        let mut s = SearchSession::new(SearchMode::RecordIdPrefix, b"chr".to_vec(), true, 0);
        s.search_ids(3, |i| ids[i].clone());
        assert_eq!(s.result_count(), 2); // chr1 + chr2
    }

    #[test]
    fn from_position_search() {
        let data = b"AAA hello BBB hello CCC";
        let mut s = SearchSession::from_offset(
            SearchMode::FromPosition,
            b"hello".to_vec(),
            true,
            10, // skip past first "hello"
            data.len() as u64,
        );
        s.search_chunk(data, u64::MAX);
        assert_eq!(s.result_count(), 1); // only the second "hello"
    }
}
