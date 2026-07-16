//! FASTQ quality-score statistics (Phred+33).

/// Statistics computed from a FASTQ quality string (Phred+33 encoding).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QualityStats {
    /// Minimum quality score observed (0–93).
    pub min: u8,
    /// Maximum quality score observed.
    pub max: u8,
    /// Mean quality score (rounded to 2 decimal places here, stored as f64).
    pub avg: f64,
    /// Number of bases with quality below `threshold`.
    pub low_quality_count: u64,
    /// Total bases counted.
    pub total: u64,
}

/// Compute quality statistics from raw Phred+33 quality bytes.
///
/// Phred+33 encoding: `qual_score = byte_value - 33`.
/// Valid ASCII range: 33 ('!') to 126 ('~'), yielding Q scores 0–93.
/// Bytes outside this range are treated as Q=0.
///
/// `threshold`: bases with Q-score < `threshold` are counted as "low quality".
/// Default Phred quality threshold is typically 20.
///
/// Returns all-zero stats for an empty slice.
#[must_use]
pub fn phred33_quality_stats(quality_bytes: &[u8], threshold: u8) -> QualityStats {
    if quality_bytes.is_empty() {
        return QualityStats {
            min: 0,
            max: 0,
            avg: 0.0,
            low_quality_count: 0,
            total: 0,
        };
    }
    let mut min = u8::MAX;
    let mut max = 0u8;
    let mut sum = 0u64;
    let mut low = 0u64;
    let mut total = 0u64;
    for &b in quality_bytes {
        let q = if (33u8..=126u8).contains(&b) {
            b - 33
        } else {
            0
        };
        min = min.min(q);
        max = max.max(q);
        sum += u64::from(q);
        total += 1;
        if q < threshold {
            low += 1;
        }
    }
    #[allow(clippy::cast_precision_loss)]
    {
        let avg = if total > 0 {
            sum as f64 / total as f64
        } else {
            0.0
        };
        QualityStats {
            min,
            max,
            avg,
            low_quality_count: low,
            total,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn perfect_quality() {
        // ASCII 126 '~' = Q93
        let stats = phred33_quality_stats(b"~~~", 20);
        assert_eq!(stats.min, 93);
        assert_eq!(stats.max, 93);
        assert_eq!(stats.total, 3);
        assert_eq!(stats.low_quality_count, 0);
    }

    #[test]
    fn typical_sanger() {
        // ASCII 73 'I' = Q40, 53 '5' = Q20, 43 '+' = Q10
        let stats = phred33_quality_stats(b"I+", 20);
        // I=Q40, +=Q10
        assert_eq!(stats.min, 10);
        assert_eq!(stats.max, 40);
        assert_eq!(stats.low_quality_count, 1); // Q10 < 20
    }

    #[test]
    fn low_quality_detected() {
        // Space (ASCII 32) is outside valid range → Q=0
        let stats = phred33_quality_stats(b" !\"#", 20);
        // ' '=Q0, '!'=Q0, '"'=Q1, '#'=Q2
        assert!(stats.avg < 1.0);
        assert_eq!(stats.low_quality_count, 4);
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn empty() {
        let stats = phred33_quality_stats(b"", 20);
        assert_eq!(stats.total, 0);
        assert_eq!(stats.avg, 0.0);
    }

    #[test]
    fn threshold_zero() {
        // Everything is low quality if threshold > max
        let stats = phred33_quality_stats(b"AAA", 100);
        assert_eq!(stats.low_quality_count, 3);
    }
}
