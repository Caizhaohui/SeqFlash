//! Background job tokens: cancel, progress, and revision gating (plan §21).
//!
//! M8 delivers a minimal, GUI-agnostic foundation. The app can adopt these
//! tokens for search/save/index workers so stale results never overwrite newer
//! document state. Full thread-pool scheduling remains out of scope for M8.

#![forbid(unsafe_code)]

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use seqflash_types::{DocumentId, JobId, Revision};

/// Kind of background work (plan §21.1).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum JobKind {
    BuildIndex,
    ValidateFile,
    Search,
    ComputeStatistics,
    ExportRecords,
    RebuildFile,
    ConvertFastqToFasta,
}

/// Lifecycle state of a job (plan §21.3).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum JobStatus {
    Pending,
    Running,
    Completed,
    Cancelled,
    Failed,
}

/// Shared control surface for one background job.
///
/// Clone is cheap (`Arc`); the worker and the UI both hold a handle.
#[derive(Clone, Debug)]
pub struct JobHandle {
    id: JobId,
    document_id: DocumentId,
    kind: JobKind,
    /// Document revision when the job was launched.
    started_revision: Revision,
    cancel: Arc<AtomicBool>,
    /// Units completed (records, bytes, …) — interpretation is job-specific.
    progress_done: Arc<AtomicU64>,
    progress_total: Arc<AtomicU64>,
    status: Arc<AtomicU64>,
}

impl JobHandle {
    /// Create a new running job handle.
    #[must_use]
    pub fn new(
        id: JobId,
        document_id: DocumentId,
        kind: JobKind,
        started_revision: Revision,
        total: u64,
    ) -> Self {
        Self {
            id,
            document_id,
            kind,
            started_revision,
            cancel: Arc::new(AtomicBool::new(false)),
            progress_done: Arc::new(AtomicU64::new(0)),
            progress_total: Arc::new(AtomicU64::new(total)),
            status: Arc::new(AtomicU64::new(status_code(JobStatus::Running))),
        }
    }

    #[must_use]
    pub const fn id(&self) -> JobId {
        self.id
    }

    #[must_use]
    pub const fn document_id(&self) -> DocumentId {
        self.document_id
    }

    #[must_use]
    pub const fn kind(&self) -> JobKind {
        self.kind
    }

    #[must_use]
    pub const fn started_revision(&self) -> Revision {
        self.started_revision
    }

    /// Request cooperative cancellation.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
        self.status
            .store(status_code(JobStatus::Cancelled), Ordering::Relaxed);
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    /// True when results computed for `started_revision` are still valid.
    #[must_use]
    pub fn is_current(&self, document_revision: Revision) -> bool {
        self.started_revision.is_current(document_revision)
    }

    /// Report progress; no-ops once cancelled.
    pub fn set_progress(&self, done: u64, total: u64) {
        self.progress_done.store(done, Ordering::Relaxed);
        self.progress_total.store(total, Ordering::Relaxed);
    }

    #[must_use]
    pub fn progress(&self) -> (u64, u64) {
        (
            self.progress_done.load(Ordering::Relaxed),
            self.progress_total.load(Ordering::Relaxed),
        )
    }

    pub fn mark_completed(&self) {
        if !self.is_cancelled() {
            self.status
                .store(status_code(JobStatus::Completed), Ordering::Relaxed);
        }
    }

    pub fn mark_failed(&self) {
        if !self.is_cancelled() {
            self.status
                .store(status_code(JobStatus::Failed), Ordering::Relaxed);
        }
    }

    #[must_use]
    pub fn status(&self) -> JobStatus {
        match self.status.load(Ordering::Relaxed) {
            0 => JobStatus::Pending,
            1 => JobStatus::Running,
            2 => JobStatus::Completed,
            3 => JobStatus::Cancelled,
            _ => JobStatus::Failed,
        }
    }
}

/// Apply a job result only if the document revision has not moved on.
///
/// Returns `Some(result)` when still current, `None` when stale (caller must
/// discard the payload).
#[must_use]
pub fn take_if_current<T>(
    started_revision: Revision,
    document_revision: Revision,
    result: T,
) -> Option<T> {
    if started_revision.is_current(document_revision) {
        Some(result)
    } else {
        None
    }
}

const fn status_code(s: JobStatus) -> u64 {
    match s {
        JobStatus::Pending => 0,
        JobStatus::Running => 1,
        JobStatus::Completed => 2,
        JobStatus::Cancelled => 3,
        JobStatus::Failed => 4,
    }
}

/// Monotonic job-id allocator for a single process.
#[derive(Debug, Default)]
pub struct JobIdGen {
    next: u64,
}

impl JobIdGen {
    #[must_use]
    pub const fn new() -> Self {
        Self { next: 1 }
    }

    /// Allocate the next unique job id.
    pub fn next_id(&mut self) -> JobId {
        let id = JobId::new(self.next);
        self.next = self.next.saturating_add(1);
        id
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn cancel_flags_and_status() {
        let h = JobHandle::new(
            JobId::new(1),
            DocumentId::new(1),
            JobKind::Search,
            Revision::initial(),
            100,
        );
        assert!(!h.is_cancelled());
        assert_eq!(h.status(), JobStatus::Running);
        h.cancel();
        assert!(h.is_cancelled());
        assert_eq!(h.status(), JobStatus::Cancelled);
    }

    #[test]
    fn progress_updates() {
        let h = JobHandle::new(
            JobId::new(2),
            DocumentId::new(1),
            JobKind::RebuildFile,
            Revision::initial(),
            10,
        );
        h.set_progress(3, 10);
        assert_eq!(h.progress(), (3, 10));
    }

    #[test]
    fn revision_gate_drops_stale() {
        let r0 = Revision::initial();
        let r1 = r0.bump();
        assert!(take_if_current(r0, r0, 42).is_some());
        assert!(take_if_current(r0, r1, 42).is_none());
    }

    #[test]
    fn is_current_matches_revision() {
        let r0 = Revision::initial();
        let h = JobHandle::new(
            JobId::new(3),
            DocumentId::new(9),
            JobKind::BuildIndex,
            r0,
            0,
        );
        assert!(h.is_current(r0));
        assert!(!h.is_current(r0.bump()));
    }

    #[test]
    fn id_gen_is_unique() {
        let mut gen = JobIdGen::new();
        let a = gen.next_id();
        let b = gen.next_id();
        assert_ne!(a, b);
    }

    #[test]
    fn completed_does_not_override_cancel() {
        let h = JobHandle::new(
            JobId::new(4),
            DocumentId::new(1),
            JobKind::ExportRecords,
            Revision::initial(),
            1,
        );
        h.cancel();
        h.mark_completed();
        assert_eq!(h.status(), JobStatus::Cancelled);
    }
}
