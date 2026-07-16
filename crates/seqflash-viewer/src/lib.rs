//! Virtual-scrolling text/record viewer.
//!
//! **Status (M0): placeholder.** No implementation yet.
//!
//! Per `DEVELOPMENT_PLAN.md` section 9.5 / 12, this crate draws only the
//! visible region (raw text and logical-record views), implements virtual
//! scrolling over the memory map, and handles selection, copy, search-result
//! highlights, and sequence-coordinate display. It must never hand the whole
//! file to `egui::TextEdit`, nor scan the whole file inside a draw call.
