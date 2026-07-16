//! Document lifecycle, read-only memory mapping, and file metadata.
//!
//! This crate owns the lifetime of a read-only file mapping. Per
//! `DEVELOPMENT_PLAN.md` section 9.2 / 10.3 / 11:
//!
//! - The original file is always opened **read-only**.
//! - Large files are viewed through a memory map ([`FileBytes::Mmap`]); the
//!   whole file is never copied into a `Vec<u8>`.
//! - The only `unsafe` in the workspace lives here, encapsulated behind a safe
//!   API (see [`FileBytes`]).
//! - Empty files are legal and map to an empty [`FileBytes::Inline`].
//!
//! M1 scope: open / close / bytes view / metadata / basic external-change
//! detection. Format detection, indexing, search, statistics, editing, and
//! export are NOT implemented here (later milestones).

// The whole workspace forbids `unsafe_code`. This crate is the single,
// documented exception: memory-mapping a file requires one `unsafe` call to
// `memmap2::Mmap::map`. To keep that exception as tight as possible we keep
// `deny(unsafe_code)` at the crate root (rather than `forbid`, which cannot be
// locally relaxed) and only relax it on the one function that performs the
// mapping (`FileBytes::from_file`, in `bytes.rs`), which carries a SAFETY
// comment. Any other `unsafe` added elsewhere in this crate will fail to build.
#![deny(unsafe_code)]

mod bytes;
mod document;
mod error;
mod list;

pub use bytes::FileBytes;
pub use document::{Document, DocumentMetadata, FileFingerprint};
pub use error::DocumentError;
pub use list::DocumentList;
