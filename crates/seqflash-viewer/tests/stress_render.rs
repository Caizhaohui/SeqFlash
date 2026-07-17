//! Stress tests for ultra-long line rendering (M8).

use seqflash_viewer::{format_line, format_raw_line};

/// Formatting a multi-megabyte line must stay bounded (no multi-MB String).
#[test]
fn format_raw_line_bounds_allocation_for_huge_line() {
    let huge = vec![b'A'; 4 * 1024 * 1024]; // 4 MiB single "line"
    let rendered = format_raw_line(0, &huge);
    // Offset field + separator + at most MAX_RENDERED_LINE_BYTES + ellipsis.
    assert!(
        rendered.len() < 1024,
        "rendered line unexpectedly large: {}",
        rendered.len()
    );
    assert!(rendered.contains('…'), "expected truncation marker");
}

/// Fixed-width formatter stays proportional to bytes_per_line, not file size.
#[test]
fn format_line_fixed_width_small() {
    let chunk = b"ACGTACGT";
    let s = format_line(1_000_000, chunk, 16);
    assert!(s.len() < 64);
    assert!(s.contains('│'));
}

/// Empty and single-byte lines are fine.
#[test]
fn format_edge_cases() {
    assert!(!format_raw_line(0, b"").is_empty());
    assert!(!format_raw_line(42, b"X").is_empty());
}
