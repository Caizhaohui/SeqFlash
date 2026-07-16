//! Formatting of one fixed-width byte line for the viewer.
//!
//! Pure functions, fully unit-testable without an egui context.

/// Format one visual line of the byte view.
///
/// `byte_offset` is the absolute file offset of the first byte in `chunk`;
/// `chunk` is the (possibly short, for the last line) slice to render;
/// `bytes_per_line` is the full line width used to pad short trailing chunks.
///
/// Output shape: `{byte_offset:>12} │ {ascii}` where non-printable bytes are
/// shown as `·`. Example (bytes_per_line = 8):
///
/// ```text
///           0 │ >seq_1↵·
/// ```
#[must_use]
pub fn format_line(byte_offset: usize, chunk: &[u8], bytes_per_line: usize) -> String {
    use std::fmt::Write;
    // Pre-size: offset field (12) + separator (3) + one char per byte + NUL.
    let mut out = String::with_capacity(16 + bytes_per_line);
    let _ = write!(out, "{byte_offset:>12} │ ");

    for &b in chunk {
        out.push(printable_ascii(b));
    }
    // Pad the trailing line so column alignment stays stable at EOF.
    if chunk.len() < bytes_per_line {
        for _ in 0..(bytes_per_line - chunk.len()) {
            out.push(' ');
        }
    }
    out
}

/// Map a single byte to a display glyph: printable ASCII passes through; all
/// other bytes (control, high-bit, NUL) become `·`.
fn printable_ascii(b: u8) -> char {
    // Printable ASCII range is 0x20..=0x7E. Everything else is shown as a dot.
    if (0x20..=0x7E).contains(&b) {
        char::from(b)
    } else {
        '·'
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn printable_bytes_pass_through() {
        // Pure printable ASCII, full-width line (no padding).
        let line = format_line(0, b">seq_1", 6);
        assert_eq!(line, format!("{:>12} │ >seq_1", 0));
    }

    #[test]
    fn newline_shown_as_dot() {
        // 0x0A (newline) is a control char, so it renders as `·`.
        let line = format_line(0, b">seq\n", 8);
        assert_eq!(line, format!("{:>12} │ >seq·   ", 0));
    }

    #[test]
    fn last_short_line_is_padded() {
        // A trailing chunk shorter than bytes_per_line is space-padded so
        // columns stay aligned.
        let line = format_line(8, b"AB", 8);
        assert_eq!(line, format!("{:>12} │ AB      ", 8));
    }

    #[test]
    fn high_bytes_become_dot() {
        // 0xFF / 0x80 are invalid UTF-8 / non-printable — render as `·`.
        let line = format_line(0, &[0xFF, b'A', 0x80], 4);
        assert_eq!(line, format!("{:>12} │ ·A· ", 0));
    }

    #[test]
    fn offset_is_right_aligned() {
        let line = format_line(1_234_567, b"X", 4);
        assert_eq!(line, format!("{:>12} │ X   ", 1_234_567));
    }
}
