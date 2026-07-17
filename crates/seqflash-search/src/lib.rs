//! Record-ID, byte, and sequence search.
//!
//! Per `DEVELOPMENT_PLAN.md` section 16, this crate provides exact and prefix
//! record-ID search, raw byte search, and sequence-fragment search. All
//! whole-file searches run incrementally (chunked per frame), cancellable,
//! with bounded result sets (default 10 000).
//!
//! M5 scope: SearchMode enum, SearchRequest/Result types, SearchSession
//! incremental engine.

#![forbid(unsafe_code)]

mod engine;
mod types;

pub use engine::SearchSession;
pub use types::{SearchMode, SearchRequest, SearchResult, MAX_RESULTS};
