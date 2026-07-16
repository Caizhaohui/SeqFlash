//! Virtual-scrolling raw-text viewer.
//!
//! Renders a byte buffer one *real* (newline-delimited) line at a time using
//! egui's `ScrollArea::show_viewport`, so only the visible rows are formatted
//! and drawn — never the whole file (plan section 9.5 / 12).
//!
//! A sparse [`LineIndex`] of checkpoints is built incrementally (a few MiB per
//! frame) so the scrollbar and line lookups stay accurate without a one-shot
//! full-file scan. Long lines are truncated and horizontally scrolled, keeping
//! the row height uniform (one logical line == one visual row).

#![forbid(unsafe_code)]

mod formatting;
mod line_index;

pub use formatting::{format_line, format_raw_line};
pub use line_index::{LineCheckpoint, LineIndex, CHECKPOINT_INTERVAL_BYTES};

/// Bytes of newline scanning performed per UI frame. Small enough to keep each
/// frame well under a frame budget (~ms); large enough to finish a 1 GiB file
/// in a few seconds of background churn.
pub const SCAN_BUDGET_BYTES_PER_FRAME: u64 = 4 * 1024 * 1024;

/// Conservative average bytes-per-line used to estimate total content height
/// before enough of the file has been scanned to measure it.
const DEFAULT_AVG_LINE_BYTES: f64 = 60.0;

/// A virtual-scrolling raw-text viewer over `&[u8]`.
///
/// Carries the persistent [`LineIndex`] for one document; construct once and
/// reuse across frames (store it keyed by document id in the app).
#[derive(Clone, Debug)]
pub struct RawTextViewer {
    index: LineIndex,
    /// Pending scroll target (a byte offset the user asked to jump to).
    /// Consumed on the next render; `None` means "follow the egui scroll state".
    pending_scroll_to_byte: Option<u64>,
}

impl RawTextViewer {
    /// Create a fresh viewer for a buffer of `file_size` bytes.
    #[must_use]
    pub fn new(file_size: u64) -> Self {
        Self {
            index: LineIndex::new(file_size),
            pending_scroll_to_byte: None,
        }
    }

    /// How far the background scan has progressed (bytes).
    #[must_use]
    pub const fn scan_progress(&self) -> u64 {
        self.index.scan_progress()
    }

    /// Whether the whole file has been scanned.
    #[must_use]
    pub const fn is_scan_complete(&self) -> bool {
        self.index.is_complete()
    }

    /// Ask the viewer to scroll so that `byte_offset` is at the top of the
    /// viewport on the next render. Used by Home/End and "Go to offset".
    pub fn scroll_to_byte(&mut self, byte_offset: u64) {
        self.pending_scroll_to_byte = Some(byte_offset);
    }

    /// Render the viewer into `ui`. `id_salt` makes each document's scroll
    /// position independent (egui persists scroll state per scroll-area id).
    ///
    /// Returns the byte offset currently at the top of the viewport, for the
    /// status bar.
    #[must_use]
    pub fn show(
        &mut self,
        ui: &mut eframe::egui::Ui,
        id_salt: impl std::hash::Hash,
        bytes: &[u8],
    ) -> u64 {
        self.show_impl(ui, id_salt, bytes)
    }

    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    fn show_impl(
        &mut self,
        ui: &mut eframe::egui::Ui,
        id_salt: impl std::hash::Hash,
        bytes: &[u8],
    ) -> u64 {
        use eframe::egui;
        use eframe::egui::NumExt as _; // for f32::at_least

        if bytes.is_empty() {
            ui.label(
                egui::RichText::new("This file is empty (0 bytes).")
                    .color(egui::Color32::from_rgb(0xC4, 0xA0, 0x00)),
            );
            return 0;
        }

        // 1. Advance the background scan a little this frame.
        self.index.scan_chunk(bytes, SCAN_BUDGET_BYTES_PER_FRAME);
        // Keep scanning if we're far behind, so the first-screen experience
        // converges quickly; each chunk is cheap.
        let file_size = self.index.file_size();
        let row_height = ui.text_style_height(&egui::TextStyle::Monospace);
        let spacing_y = ui.spacing().item_spacing.y;
        let row_h = row_height + spacing_y;

        // 2. Estimate total content height from the average line length observed
        //    so far (or a conservative default before scanning measures it).
        let avg_line_bytes = measured_or_default_avg_line_bytes(&self.index);
        let estimated_total_lines = (file_size as f64 / avg_line_bytes).max(1.0);
        let full_height = (row_h * estimated_total_lines as f32 - spacing_y).at_least(0.0);

        // Byte offset at the top of the viewport, returned to the caller.
        let mut top_byte_offset = 0u64;

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .id_salt(id_salt)
            .show_viewport(ui, |ui, viewport| {
                ui.set_height(full_height);

                // Map the visible pixel range to a byte-offset range using the
                // average line length estimate.
                let est_first_line = (viewport.min.y / row_h).floor().max(0.0) as u64;
                let est_last_line = ((viewport.max.y / row_h).ceil() as u64) + 2;
                let est_start_byte = (est_first_line as f64 * avg_line_bytes) as u64;

                // Snap to the nearest preceding checkpoint, then scan locally
                // to the real line boundary at/after est_start_byte.
                let cp = self.index.checkpoint_before(est_start_byte.min(file_size));
                let line_start_byte = locate_line_start(bytes, cp.byte_offset, est_start_byte);

                top_byte_offset = line_start_byte as u64;

                // Render visible lines by walking newlines forward from
                // line_start_byte, emitting rows until we pass the viewport.
                let top = ui.max_rect().top();
                // Pixel y where the first rendered line sits. Use the estimate
                // so the scrollbar stays smooth; the line content itself is real.
                let y_cursor_start = top + est_first_line as f32 * row_h;
                let mut y = y_cursor_start;
                let mut byte_cursor = line_start_byte;
                let mut lines_drawn = 0u64;
                let max_lines = est_last_line.saturating_sub(est_first_line) + 4;

                let x_range = ui.max_rect().x_range();
                while y < viewport.max.y + row_h
                    && byte_cursor < bytes.len()
                    && lines_drawn < max_lines
                {
                    let (line_end, next_start) = next_line_bounds(bytes, byte_cursor);
                    let line_bytes = &bytes[byte_cursor..line_end];
                    let text = crate::format_raw_line(byte_cursor, line_bytes);
                    let row_rect = egui::Rect::from_x_y_ranges(x_range, y..=(y + row_height));
                    ui.scope_builder(egui::UiBuilder::new().max_rect(row_rect), |view| {
                        view.add(
                            egui::Label::new(egui::RichText::new(text).monospace())
                                .truncate()
                                .selectable(true),
                        );
                    });
                    y += row_h;
                    byte_cursor = next_start;
                    lines_drawn += 1;
                }

                // 3. Apply a pending "scroll to byte offset" request by scrolling
                //    the egui area to the matching pixel.
                if let Some(target) = self.pending_scroll_to_byte.take() {
                    let target_line = target as f64 / avg_line_bytes;
                    let target_y = target_line as f32 * row_h;
                    ui.scroll_with_delta(egui::vec2(0.0, target_y - viewport.min.y));
                }
            });

        // Request a repaint while scanning is still in progress so the index
        // (and thus the scrollbar estimate) keeps improving without user input.
        if !self.index.is_complete() {
            ui.ctx().request_repaint();
        }

        // Keyboard navigation: Home/End jump to file start/end; PageUp/PageDown
        // move by one viewport of bytes. These set a pending scroll target that
        // the next render applies. The egui ScrollArea already handles mouse
        // wheel and drag scrolling natively.
        let ctx = ui.ctx();
        let avg = measured_or_default_avg_line_bytes(&self.index);
        let viewport_lines = (full_height / row_h).max(1.0) as u64;
        let viewport_bytes = (viewport_lines as f64 * avg) as u64;
        if ctx.input(|i| i.key_pressed(egui::Key::Home)) {
            self.scroll_to_byte(0);
        } else if ctx.input(|i| i.key_pressed(egui::Key::End)) {
            self.scroll_to_byte(file_size);
        } else if ctx.input(|i| i.key_pressed(egui::Key::PageUp)) {
            let target = top_byte_offset.saturating_sub(viewport_bytes);
            self.scroll_to_byte(target);
        } else if ctx.input(|i| i.key_pressed(egui::Key::PageDown)) {
            let target = top_byte_offset
                .saturating_add(viewport_bytes)
                .min(file_size);
            self.scroll_to_byte(target);
        }

        top_byte_offset
    }
}

/// Average bytes per completed line, measured from the scan so far; falls back
/// to [`DEFAULT_AVG_LINE_BYTES`] when no lines have been counted yet.
#[allow(clippy::cast_precision_loss)]
fn measured_or_default_avg_line_bytes(index: &LineIndex) -> f64 {
    let lines = index.lines_seen();
    if lines == 0 {
        return DEFAULT_AVG_LINE_BYTES;
    }
    let scanned = index.scan_progress().max(1) as f64;
    scanned / lines as f64
}

/// From `scan_from`, scan forward (or backward) to the start of the line that
/// contains or follows `target_byte`. Returns the byte offset of that line
/// start. Keeps the local scan bounded to a reasonable window.
fn locate_line_start(bytes: &[u8], scan_from: u64, target_byte: u64) -> usize {
    let target = usize::try_from(target_byte).unwrap_or(0).min(bytes.len());
    let mut from = usize::try_from(scan_from).unwrap_or(0).min(bytes.len());
    if from > target {
        from = target;
    }
    // Walk forward from `from` until we reach/pass `target`, splitting on '\n'.
    let mut cur = from;
    while cur < target {
        // Find next newline at/after cur.
        match bytes[cur..].iter().position(|&b| b == b'\n') {
            Some(rel) => {
                let nl = cur + rel;
                if nl >= target {
                    // target lies within the current line; its start is `cur`.
                    return cur;
                }
                cur = nl + 1;
            }
            None => return cur,
        }
    }
    cur
}

/// Given a cursor at a line start, return `(line_content_end, next_line_start)`:
/// the content end excludes the trailing newline; the next start skips past it.
fn next_line_bounds(bytes: &[u8], cursor: usize) -> (usize, usize) {
    match bytes[cursor..].iter().position(|&b| b == b'\n') {
        Some(rel) => {
            let end = cursor + rel;
            // Skip a trailing '\r' (CRLF) when reporting content, but advance
            // past the full CRLF for the next start.
            let content_end = if end > cursor && bytes[end - 1] == b'\r' {
                end - 1
            } else {
                end
            };
            (content_end, end + 1)
        }
        None => (bytes.len(), bytes.len()),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn new_viewer_seeds_empty_or_complete_index() {
        let v0 = RawTextViewer::new(0);
        assert!(v0.is_scan_complete());
        let vbig = RawTextViewer::new(1_000_000);
        assert!(!vbig.is_scan_complete());
        assert_eq!(vbig.scan_progress(), 0);
    }

    #[test]
    fn scroll_to_byte_sets_pending_target() {
        let mut v = RawTextViewer::new(1024);
        assert!(v.pending_scroll_to_byte.is_none());
        v.scroll_to_byte(500);
        assert_eq!(v.pending_scroll_to_byte, Some(500));
    }

    #[test]
    fn next_line_bounds_handles_lf() {
        let bytes = b"abc\ndef\n";
        assert_eq!(next_line_bounds(bytes, 0), (3, 4));
        assert_eq!(next_line_bounds(bytes, 4), (7, 8));
    }

    #[test]
    fn next_line_bounds_handles_crlf() {
        let bytes = b"abc\r\ndef\r\n";
        // content excludes the '\r'; next start skips "\r\n".
        assert_eq!(next_line_bounds(bytes, 0), (3, 5));
        assert_eq!(next_line_bounds(bytes, 5), (8, 10));
    }

    #[test]
    fn next_line_bounds_at_eof_without_newline() {
        let bytes = b"abc";
        assert_eq!(next_line_bounds(bytes, 0), (3, 3));
    }

    #[test]
    fn locate_line_start_finds_current_line() {
        // "ab\ncdef\nghi" — line starts at 0, 3, 8.
        let bytes = b"ab\ncdef\nghi";
        assert_eq!(locate_line_start(bytes, 0, 0), 0); // line "ab"
        assert_eq!(locate_line_start(bytes, 0, 5), 3); // inside "cdef"
        assert_eq!(locate_line_start(bytes, 0, 9), 8); // inside "ghi"
    }

    #[test]
    fn locate_line_start_from_checkpoint() {
        // Simulate a checkpoint at offset 3 (start of "cdef").
        let bytes = b"ab\ncdef\nghi";
        assert_eq!(locate_line_start(bytes, 3, 5), 3);
        // Target before the checkpoint: clamp back to target itself.
        assert_eq!(locate_line_start(bytes, 3, 1), 1);
    }

    #[test]
    fn measured_avg_falls_back_then_converges() {
        let v = RawTextViewer::new(10_000);
        assert!(
            (measured_or_default_avg_line_bytes(&v.index) - DEFAULT_AVG_LINE_BYTES).abs() < 1e-9
        );
        // After scanning a small input the average reflects reality.
        let mut v2 = RawTextViewer::new(9);
        v2.index.scan_chunk(b"aaaa\nbbbb\n", u64::MAX);
        let avg = measured_or_default_avg_line_bytes(&v2.index);
        assert!(avg > 0.0);
    }
}
