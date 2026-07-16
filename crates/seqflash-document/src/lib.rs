//! Document lifecycle, read-only memory mapping, and edit overlay.
//!
//! **Status (M0): placeholder.** No implementation yet.
//!
//! Once implemented this crate will own, per `DEVELOPMENT_PLAN.md` section 9.2:
//! file open/close, the **read-only** memory map (the only place `unsafe` mmap
//! lives, fully encapsulated), file metadata, document IDs and revisions,
//! external-change detection, the viewport cache, and the edit overlay.
//!
//! Other crates must never create their own memory mappings.

#![forbid(unsafe_code)]
