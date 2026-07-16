//! Pure-function base-counting and statistics.

/// Per-base counts for a nucleotide sequence.
///
/// `other` (IUPAC ambiguity codes) and `illegal` (anything not in the
/// supported set) are tracked separately from A/C/G/T/U/N so callers can
/// distinguish "allowed non-ACGT" from true garbage.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BaseCounts {
    pub a: u64,
    pub c: u64,
    pub g: u64,
    pub t: u64,
    pub u: u64,
    pub n: u64,
    /// IUPAC ambiguity codes within the allowed character set
    /// (R/Y/S/W/K/M/B/D/H/V), plus gap `-` and indeterminate `.` — these are
    /// valid but not one of the six primary bases.
    pub other: u64,
    /// Bytes that are none of the above (digits, symbols, non-printable, …).
    pub illegal: u64,
}

impl BaseCounts {
    /// Total bases counted (sum of all fields).
    #[must_use]
    pub const fn total(&self) -> u64 {
        self.a + self.c + self.g + self.t + self.u + self.n + self.other + self.illegal
    }
}

/// Count distinct base categories in `seq_bytes` (case-insensitive).
///
/// The allowed character set mirrors plan section 13.4:
/// `A C G T U  R Y S W K M  B D H V  N  -  .` (both cases).
/// Everything else is counted as [`BaseCounts::illegal`].
/// Newlines and other whitespace are NOT stripped — the caller is expected to
/// pass only the raw sequence bytes (after removing header lines and
/// newlines).
#[must_use]
pub fn count_bases(seq_bytes: &[u8]) -> BaseCounts {
    let mut counts = BaseCounts::default();
    for &b in seq_bytes {
        match b {
            b'A' | b'a' => counts.a += 1,
            b'C' | b'c' => counts.c += 1,
            b'G' | b'g' => counts.g += 1,
            b'T' | b't' => counts.t += 1,
            b'U' | b'u' => counts.u += 1,
            b'N' | b'n' => counts.n += 1,
            // IUPAC extended + gaps
            b'R' | b'r' | b'Y' | b'y' | b'S' | b's' | b'W' | b'w' | b'K' | b'k' | b'M' | b'm'
            | b'B' | b'b' | b'D' | b'd' | b'H' | b'h' | b'V' | b'v' | b'-' | b'.' => {
                counts.other += 1;
            }
            _ => counts.illegal += 1,
        }
    }
    counts
}

/// GC percentage based on `(G + C) / (A + C + G + T + U) * 100`.
///
/// Returns `0.0` when the denominator is 0 (empty or all-N sequence).
#[must_use]
pub fn gc_percent(counts: &BaseCounts) -> f64 {
    let gc = counts.g + counts.c;
    let atgcu = counts.a + counts.t + counts.g + counts.c + counts.u;
    if atgcu == 0 {
        0.0
    } else {
        #[allow(clippy::cast_precision_loss)]
        {
            (gc as f64 / atgcu as f64) * 100.0
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn assert_close(a: f64, b: f64) {
        assert!((a - b).abs() < 0.01, "{a} !≈ {b}");
    }

    #[test]
    fn counts_simple_acgt() {
        let c = count_bases(b"ACGTacgt");
        assert_eq!(c.a, 2);
        assert_eq!(c.c, 2);
        assert_eq!(c.g, 2);
        assert_eq!(c.t, 2);
        assert_eq!(c.u, 0);
        assert_eq!(c.n, 0);
        assert_eq!(c.other, 0);
        assert_eq!(c.illegal, 0);
        assert_eq!(c.total(), 8);
    }

    #[test]
    fn counts_n_and_other() {
        let c = count_bases(b"ACGTNNNnRYSWKMBDHV-.");
        // A=1 C=1 G=1 T=1 N=3+n=1=4
        assert_eq!(c.n, 4);
        assert_eq!(c.other, 12); // RY..HV(10) + -(1) + .(1)
        assert_eq!(c.illegal, 0);
    }

    #[test]
    fn illegal_characters() {
        let c = count_bases(b"ACGT123!@#");
        assert_eq!(c.illegal, 6); // 1 2 3 ! @ #
    }

    #[test]
    fn gc_half() {
        let c = BaseCounts {
            a: 1,
            c: 1,
            g: 0,
            t: 0,
            u: 0,
            n: 0,
            other: 0,
            illegal: 0,
        };
        assert_close(gc_percent(&c), 50.0);
    }

    #[test]
    fn gc_zero_when_all_n() {
        let c = BaseCounts {
            a: 0,
            c: 0,
            g: 0,
            t: 0,
            u: 0,
            n: 100,
            other: 0,
            illegal: 0,
        };
        assert_close(gc_percent(&c), 0.0);
    }

    #[test]
    fn empty_sequence_is_all_zero() {
        let c = count_bases(b"");
        assert_eq!(c.total(), 0);
        assert_close(gc_percent(&c), 0.0);
    }

    #[test]
    fn gc_includes_uraci() {
        let c = count_bases(b"ACGUacgu");
        assert_close(gc_percent(&c), 50.0); // C+G=4, A+C+G+U=8
    }
}
