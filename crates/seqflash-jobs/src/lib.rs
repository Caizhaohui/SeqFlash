//! Background job management, progress, and cancellation.
//!
//! **Status (M0): placeholder.** No implementation yet.
//!
//! Per `DEVELOPMENT_PLAN.md` section 21, this crate owns background task
//! lifecycle: launching, status, progress reporting, cancellation tokens, and
//! thread communication. Every completing job verifies the document revision
//! it was started with; stale results never overwrite newer state.
