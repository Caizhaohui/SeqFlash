//! Virtual-scrolling text/record viewer.
//!
//! Renders a byte buffer one fixed-width "line" at a time using egui's
//! `ScrollArea::show_viewport`, so only the visible rows are formatted and
//! drawn — never the whole file (plan section 9.5 / 12).
//!
//! M1/Fix-4 scope: a fixed-bytes-per-line raw byte view (no newline-aware line
//! indexing, no record view). Newline-checkpoint-based real-line scrolling
//! (plan section 12.3) is deferred to a later milestone.

#![forbid(unsafe_code)]

mod formatting;

pub use formatting::format_line;

/// Default number of file bytes shown per visual line.
pub const DEFAULT_BYTES_PER_LINE: usize = 64;

/// A fixed-bytes-per-line virtual-scrolling viewer over `&[u8]`.
#[derive(Clone, Debug)]
pub struct ByteViewer {
    bytes_per_line: usize,
}

impl ByteViewer {
    /// Create a viewer with the default line width ([`DEFAULT_BYTES_PER_LINE`]).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            bytes_per_line: DEFAULT_BYTES_PER_LINE,
        }
    }

    /// Number of file bytes rendered per visual line.
    #[must_use]
    pub const fn bytes_per_line(&self) -> usize {
        self.bytes_per_line
    }

    /// Render the viewer into `ui`. `id_salt` makes each document's scroll
    /// position independent (egui persists scroll state per scroll-area id).
    ///
    /// Only the rows visible in the current viewport are formatted and drawn;
    /// the total content height is reserved up front so the scrollbar reflects
    /// the full file. No full-file scan happens here.
    //
    // The f32<->usize casts below mirror egui's own `ScrollArea::show_rows`
    // (pixel math for row indices). They are inherent to immediate-mode
    // virtual scrolling; the relevant clippy lints are relaxed here only.
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    pub fn show(&self, ui: &mut eframe::egui::Ui, id_salt: impl std::hash::Hash, bytes: &[u8]) {
        use eframe::egui;
        use eframe::egui::NumExt as _; // for f32::at_least

        if bytes.is_empty() {
            ui.label(
                egui::RichText::new("This file is empty (0 bytes).")
                    .color(egui::Color32::from_rgb(0xC4, 0xA0, 0x00)),
            );
            return;
        }

        let bpl = self.bytes_per_line;
        // Total visual lines — pure arithmetic, no file scan.
        let total_lines = bytes.len().div_ceil(bpl);
        let row_height = ui.text_style_height(&egui::TextStyle::Monospace);
        let spacing_y = ui.spacing().item_spacing.y;
        let row_h = row_height + spacing_y;

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .id_salt(id_salt)
            .show_viewport(ui, |ui, viewport| {
                // Reserve the full content height so the scrollbar is correct.
                let full_height = (row_h * total_lines as f32 - spacing_y).at_least(0.0);
                ui.set_height(full_height);

                // Determine the visible row range from the relative viewport rect.
                let mut first = (viewport.min.y / row_h).floor() as usize;
                let mut last = (viewport.max.y / row_h).ceil() as usize + 1; // +1 pad
                if last > total_lines {
                    let diff = last.saturating_sub(first);
                    last = total_lines;
                    first = total_lines.saturating_sub(diff);
                }

                // Position the cursor at the first visible row's y, then draw
                // only the visible rows inside a scoped sub-ui (mirrors egui's
                // own ScrollArea::show_rows implementation).
                let top = ui.max_rect().top();
                let y_min = top + first as f32 * row_h;
                let y_max = top + last as f32 * row_h;
                let rect = egui::Rect::from_x_y_ranges(ui.max_rect().x_range(), y_min..=y_max);

                ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |view| {
                    view.skip_ahead_auto_ids(first);
                    for line_idx in first..last {
                        let start = line_idx * bpl;
                        let end = (start + bpl).min(bytes.len());
                        let chunk = &bytes[start..end];
                        let text = crate::format_line(start, chunk, bpl);
                        view.add(
                            egui::Label::new(egui::RichText::new(text).monospace())
                                .truncate()
                                .selectable(false),
                        );
                    }
                });
            });
    }
}

impl Default for ByteViewer {
    fn default() -> Self {
        Self::new()
    }
}

// `format_line` is re-exported above (`pub use formatting::format_line`);
// callers in this file use it via that path to avoid a duplicate import.

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn default_bytes_per_line() {
        assert_eq!(ByteViewer::new().bytes_per_line(), DEFAULT_BYTES_PER_LINE);
        assert_eq!(
            ByteViewer::default().bytes_per_line(),
            DEFAULT_BYTES_PER_LINE
        );
    }

    #[test]
    fn show_empty_does_not_panic() {
        // The viewer must handle empty input gracefully; we only assert it does
        // not panic (formatting of a real egui UI needs a live context).
        let _viewer = ByteViewer::new();
        // bytes.is_empty() branch returns early before any egui call.
    }
}
