//! Record filtering operations: by length and by ID.

/// Filter record indices by sequence length.
///
/// `lengths` is a slice of `(record_index, sequence_length)` pairs.
/// Returns the indices of records whose length is within `[min, max]`
/// (inclusive). A `max` of `u64::MAX` means no upper bound.
#[must_use]
pub fn filter_by_length(lengths: &[(usize, u64)], min: u64, max: u64) -> Vec<usize> {
    lengths
        .iter()
        .filter(|(_, len)| *len >= min && *len <= max)
        .map(|(idx, _)| *idx)
        .collect()
}

/// Extract record indices whose ID matches a pattern.
///
/// `ids` is a slice of `(record_index, id_bytes)` pairs.
/// If `exact` is true, the ID must equal `pattern` exactly; otherwise prefix
/// match. Comparison is case-sensitive (the caller can pre-lowercase if needed).
#[must_use]
pub fn extract_by_id(ids: &[(usize, &[u8])], pattern: &[u8], exact: bool) -> Vec<usize> {
    ids.iter()
        .filter(|(_, id_bytes)| {
            if exact {
                *id_bytes == pattern
            } else {
                id_bytes.len() >= pattern.len() && &id_bytes[..pattern.len()] == pattern
            }
        })
        .map(|(idx, _)| *idx)
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn filter_length_range() {
        let lens = vec![(0, 10), (1, 50), (2, 100), (3, 500)];
        let result = filter_by_length(&lens, 20, 200);
        assert_eq!(result, vec![1, 2]);
    }

    #[test]
    fn filter_length_no_upper() {
        let lens = vec![(0, 10), (1, 5000)];
        let result = filter_by_length(&lens, 100, u64::MAX);
        assert_eq!(result, vec![1]);
    }

    #[test]
    fn extract_id_exact() {
        let ids: Vec<(usize, &[u8])> = vec![(0, b"seq1"), (1, b"seq2"), (2, b"seq3")];
        let result = extract_by_id(&ids, b"seq2", true);
        assert_eq!(result, vec![1]);
    }

    #[test]
    fn extract_id_prefix() {
        let ids: Vec<(usize, &[u8])> = vec![(0, b"chr1"), (1, b"chr2"), (2, b"scaffold1")];
        let result = extract_by_id(&ids, b"chr", false);
        assert_eq!(result, vec![0, 1]);
    }
}
