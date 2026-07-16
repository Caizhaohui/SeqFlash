//! Record-ID, byte, and sequence search.
//!
//! **Status (M0): placeholder.** No implementation yet.
//!
//! Per `DEVELOPMENT_PLAN.md` section 16, this crate provides exact and prefix
//! record-ID search, raw byte search, and sequence-fragment search. All
//! whole-file searches run in the background: chunked, cancellable, with
//! bounded result sets (default 10 000) and incremental first-result delivery.
