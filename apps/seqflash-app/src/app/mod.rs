//! The `eframe::App` for the SeqFlash main window.
//!
//! M1: open files via dialog or drag-and-drop, show one tab per document, and
//! render a small byte-level preview plus a status bar. The full virtual
//! scrolling text/record viewer, search, statistics, and editing arrive in
//! later milestones (plan section 22 describes the eventual layout).

mod ui;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use eframe::egui;

use seqflash_document::{Document, DocumentList};
use seqflash_formats::detect_format;
use seqflash_index::{FastaIndex, FastqIndex};
use seqflash_ops::{
    count_bases, export_fasta_records, export_fastq_records, gc_percent, phred33_quality_stats,
    save_fasta_with_overlay_ex, save_fastq_with_overlay_ex, BaseCounts, EditOverlay,
    FastaExportRecord, FastaOverlayEntry, FastqExportRecord, FastqOverlayEntry, QualityStats,
    RecordEdit, Transform, RECORD_EDIT_LIMIT_BYTES,
};
use seqflash_search::{SearchMode, SearchSession};
use seqflash_settings::AppSettings;
use seqflash_types::{ByteRange, DocumentId, SequenceFormat};
use seqflash_viewer::RawTextViewer;

/// In-flight background overlay save (plan 20.3: progress + cancel).
struct OverlaySaveJob {
    document_id: DocumentId,
    dest: PathBuf,
    cancel: Arc<AtomicBool>,
    done: Arc<AtomicU64>,
    total: Arc<AtomicU64>,
    /// Filled by the worker when finished; `None` while running.
    result: Arc<Mutex<Option<Result<(), String>>>>,
}

/// Owned index snapshot for a background overlay save.
enum OverlaySaveEntries {
    Fasta(Vec<FastaOverlayEntry>),
    Fastq(Vec<FastqOverlayEntry>),
}

/// Maximum number of entries kept in the in-memory recent-files list.
const RECENT_FILES_LIMIT: usize = 10;

/// Max sequence/quality characters shown in overlay text previews.
const PREVIEW_FIELD_CHARS: usize = 480;

/// Per-record overlay flags for list badges and preview chrome.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct RecordEditFlags {
    pub deleted: bool,
    pub replaced: bool,
    pub inserts_before: usize,
    pub inserts_after: usize,
}

impl RecordEditFlags {
    #[must_use]
    pub(crate) const fn has_any(self) -> bool {
        self.deleted || self.replaced || self.inserts_before > 0 || self.inserts_after > 0
    }

    /// Short badge text for the record list, e.g. `[DEL]`, `[EDIT +B1]`.
    #[must_use]
    pub(crate) fn badge(self) -> Option<String> {
        if !self.has_any() {
            return None;
        }
        let mut parts: Vec<String> = Vec::new();
        if self.deleted {
            parts.push("DEL".into());
        } else if self.replaced {
            parts.push("EDIT".into());
        }
        if self.inserts_before > 0 {
            parts.push(format!("+B{}", self.inserts_before));
        }
        if self.inserts_after > 0 {
            parts.push(format!("+A{}", self.inserts_after));
        }
        Some(format!("[{}]", parts.join(" ")))
    }
}

/// Overlay-resolved view of one record for UI preview (plan M7: delete/replace preview).
#[derive(Clone, Debug)]
pub(crate) struct OverlayRecordPreview {
    pub flags: RecordEditFlags,
    pub header: Option<String>,
    pub sequence: Option<String>,
    pub quality: Option<String>,
    /// Full effective record body (or a deleted placeholder). Truncated for display.
    pub body_preview: String,
    pub inserts_before: Vec<String>,
    pub inserts_after: Vec<String>,
}

/// Top-level SeqFlash egui application.
#[allow(clippy::struct_excessive_bools)] // dialog visibility flags; kept explicit for UI wiring
pub(crate) struct SeqFlashApp {
    settings: AppSettings,
    /// Path where settings are persisted (so `record_recent` can auto-save).
    settings_path: Option<PathBuf>,
    documents: DocumentList,
    active_document: Option<DocumentId>,
    recent_files: Vec<PathBuf>,
    /// Transient user-facing notice (error opening a file, change detected, …).
    notice: Option<String>,
    /// Path picked by an async file-dialog worker thread, awaited on the UI
    /// thread each frame. Shared (cloned) into the spawned dialog task.
    pending_open: Arc<Mutex<Option<PathBuf>>>,
    /// One persistent raw-text viewer per open document (holds its line index
    /// and scroll state). Removed when the document is closed.
    viewers: HashMap<DocumentId, RawTextViewer>,
    /// One FASTA record index per open document (lazy, built incrementally).
    fasta_indexes: HashMap<DocumentId, FastaIndex>,
    /// One FASTQ record index per open document (lazy).
    fastq_indexes: HashMap<DocumentId, FastqIndex>,
    /// Currently selected record number (0-indexed), or None if no record is
    /// active. Navigation buttons update this; clicking a record in the list
    /// sets it and scrolls the viewer.
    current_record_number: Option<u64>,
    /// Byte offset currently at the top of the active viewer's viewport.
    active_top_offset: u64,
    /// Text staged for clipboard copy; drained by the UI layer each frame.
    pending_clipboard: Option<String>,
    /// Whether the "Go to offset" dialog is open.
    show_goto_offset: bool,
    /// Current text in the "Go to offset" input.
    goto_offset_input: String,
    /// One search session per open document (if a search was started).
    search_sessions: HashMap<DocumentId, SearchSession>,
    /// Search input text (shared across documents).
    search_input: String,
    /// Selected search mode.
    search_mode: SearchMode,
    /// Currently selected search result index (for prev/next navigation).
    current_search_result: Option<usize>,
    /// One edit overlay per open document.
    overlays: HashMap<DocumentId, EditOverlay>,
    /// Text input for editing a record header.
    edit_header_input: String,
    /// Text input for editing a record sequence.
    edit_seq_input: String,
    /// Text input for editing a FASTQ quality string.
    edit_qual_input: String,
    /// Whether the edit-header dialog is open.
    show_edit_header: bool,
    /// Whether the edit-sequence dialog is open.
    show_edit_seq: bool,
    /// Whether the edit-quality dialog is open.
    show_edit_qual: bool,
    /// Whether the insert-record dialog is open.
    show_insert: bool,
    /// Insert placement: `true` = before current record, `false` = after.
    insert_before: bool,
    /// Background overlay save, if any.
    save_job: Option<OverlaySaveJob>,
}

impl SeqFlashApp {
    /// Construct the application from the already-loaded settings, optionally
    /// opening `initial_file` right away (e.g. from a command-line argument).
    pub(crate) fn new(
        settings: AppSettings,
        settings_path: Option<PathBuf>,
        open_files: Vec<PathBuf>,
    ) -> Self {
        // Seed recent-files list from persisted settings.
        let recent_files: Vec<PathBuf> = settings.recent_files.clone();
        let mut app = Self {
            settings,
            documents: DocumentList::new(),
            active_document: None,
            recent_files,
            settings_path,
            notice: None,
            pending_open: Arc::new(Mutex::new(None)),
            viewers: HashMap::new(),
            fasta_indexes: HashMap::new(),
            fastq_indexes: HashMap::new(),
            current_record_number: None,
            active_top_offset: 0,
            pending_clipboard: None,
            show_goto_offset: false,
            goto_offset_input: String::new(),
            search_sessions: HashMap::new(),
            search_input: String::new(),
            search_mode: SearchMode::RawBytes,
            current_search_result: None,
            overlays: HashMap::new(),
            edit_header_input: String::new(),
            edit_seq_input: String::new(),
            edit_qual_input: String::new(),
            show_edit_header: false,
            show_edit_seq: false,
            show_edit_qual: false,
            show_insert: false,
            insert_before: true,
            save_job: None,
        };
        for path in open_files {
            app.open_path(&path);
        }
        app
    }

    /// Drain a path produced by an async file-dialog worker, if any.
    /// Called once per frame from `update` so the UI thread never blocks on
    /// the native dialog.
    fn take_pending_open(&self) -> Option<PathBuf> {
        // Lock failures are treated as "no pending path" rather than panicking.
        self.pending_open
            .lock()
            .ok()
            .and_then(|mut guard| guard.take())
    }

    /// Number of open documents (used by the UI for empty-state checks).
    pub(crate) fn document_count(&self) -> usize {
        self.documents.len()
    }

    /// Iterate over open documents for tab rendering.
    ///
    /// Returns `(id, path, size)` triples; collected up front so the tab strip
    /// does not borrow `self` while click handlers mutate it.
    pub(crate) fn document_entries(&self) -> Vec<(DocumentId, std::path::PathBuf, u64)> {
        self.documents
            .iter()
            .map(|d| {
                let meta = d.metadata();
                (d.id(), meta.path.clone(), meta.size)
            })
            .collect()
    }

    /// Borrow the current transient notice text, if any.
    pub(crate) fn notice_text(&self) -> Option<&str> {
        self.notice.as_deref()
    }

    /// Set a user-facing notice (errors, save results, …).
    pub(crate) fn set_notice(&mut self, msg: impl Into<String>) {
        self.notice = Some(msg.into());
    }

    /// The active document id, if any.
    pub(crate) fn active_document_id(&self) -> Option<DocumentId> {
        self.active_document
    }

    /// The active document, if any.
    pub(crate) fn active_document(&self) -> Option<&Document> {
        self.active_document.and_then(|id| self.documents.get(id))
    }

    /// Open (or re-activate) a file by path.
    ///
    /// If the file is already open, switch to its tab instead of opening again.
    /// On failure, set a notice and do **not** create a broken tab.
    pub(crate) fn open_path(&mut self, path: &Path) {
        if let Some(id) = self.documents.find_by_path(path) {
            self.active_document = Some(id);
            return;
        }

        match self.documents.open(path) {
            Ok(id) => {
                self.active_document = Some(id);
                // Detect FASTA/FASTQ format from the first few bytes.
                let sample_end = self
                    .documents
                    .get(id)
                    .map_or(0, |d| d.bytes().len().min(65536));
                if let Some(doc) = self.documents.get_mut(id) {
                    let format = detect_format(&doc.bytes()[..sample_end]);
                    doc.set_format(format);
                    tracing::info!(
                        path = %path.display(),
                        format = ?format,
                        "detected format"
                    );
                }
                self.record_recent(path.to_path_buf());
                self.notice = None;
                tracing::info!(path = %path.display(), "opened document");
            }
            Err(err) => {
                let msg = user_facing_open_error(path, &err);
                tracing::warn!(path = %path.display(), %err, "open failed");
                self.notice = Some(msg);
            }
        }
    }

    /// Show the native file-open dialog on a worker thread so the UI thread
    /// does not block while the user picks a file. The picked path is
    /// delivered back via `pending_open` and applied on a subsequent frame.
    pub(crate) fn open_from_dialog(&mut self, ctx: &egui::Context) {
        // If a dialog is already in flight, ignore the request.
        if self.pending_open.lock().is_ok_and(|g| g.is_some()) {
            return;
        }
        let slot = Arc::clone(&self.pending_open);
        // egui::Context is cheaply clonable (Arc-backed); keep a copy so the
        // worker can wake the UI thread once a path is ready.
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let selection = rfd::FileDialog::new()
                .add_filter(
                    "Sequence files",
                    &["fa", "fasta", "fas", "fna", "fq", "fastq"],
                )
                .add_filter("All files", &["*"])
                .pick_file();
            if let Some(path) = selection {
                if let Ok(mut guard) = slot.lock() {
                    *guard = Some(path);
                }
                // Wake the UI so the pending path is applied promptly instead
                // of waiting for the next user input.
                ctx.request_repaint();
            }
        });
    }

    /// Close a document and release its memory map. If it was the active tab,
    /// move focus to another open document.
    pub(crate) fn close_document(&mut self, id: DocumentId) {
        let was_active = self.active_document == Some(id);
        self.documents.close(id);
        // Drop the viewer (and its line index) for the closed document.
        self.viewers.remove(&id);
        self.fasta_indexes.remove(&id);
        self.fastq_indexes.remove(&id);
        self.search_sessions.remove(&id);
        self.overlays.remove(&id);
        if was_active {
            self.active_document = self.documents.iter().next().map(Document::id);
            self.active_top_offset = 0;
            self.current_record_number = None;
        }
    }

    /// Borrow the viewer for `id`, creating it lazily on first access from the
    /// document's file size. Returns `None` only if the document is gone.
    pub(crate) fn viewer_for(&mut self, id: DocumentId) -> Option<&mut RawTextViewer> {
        let file_size = self.documents.get(id).map(|d| d.metadata().size)?;
        self.viewers
            .entry(id)
            .or_insert_with(|| RawTextViewer::new(file_size));
        self.viewers.get_mut(&id)
    }

    /// Borrow the FASTA index for `id`, creating it lazily.
    pub(crate) fn index_for(&mut self, id: DocumentId) -> Option<&mut FastaIndex> {
        let file_size = self.documents.get(id).map(|d| d.metadata().size)?;
        self.fasta_indexes
            .entry(id)
            .or_insert_with(|| FastaIndex::new(file_size));
        self.fasta_indexes.get_mut(&id)
    }

    /// Borrow the FASTQ index for `id`, creating it lazily.
    pub(crate) fn index_for_fastq(&mut self, id: DocumentId) -> Option<&mut FastqIndex> {
        let file_size = self.documents.get(id).map(|d| d.metadata().size)?;
        self.fastq_indexes
            .entry(id)
            .or_insert_with(|| FastqIndex::new(file_size));
        self.fastq_indexes.get_mut(&id)
    }

    /// Advance the background index scan (FASTA or FASTQ) for the active doc.
    fn advance_index_scan(&mut self) {
        let Some(id) = self.active_document else {
            return;
        };
        let Some(doc) = self.documents.get(id) else {
            return;
        };
        let bytes = doc.bytes();
        match doc.format() {
            seqflash_types::SequenceFormat::Fasta => {
                if let Some(idx) = self.fasta_indexes.get_mut(&id) {
                    idx.scan_chunk(bytes, seqflash_index::DEFAULT_INDEX_SCAN_BUDGET);
                }
            }
            seqflash_types::SequenceFormat::Fastq => {
                if let Some(idx) = self.fastq_indexes.get_mut(&id) {
                    idx.scan_chunk(bytes, seqflash_index::FASTQ_INDEX_BUDGET);
                }
            }
            seqflash_types::SequenceFormat::Unknown => {}
        }
    }

    /// Active FastqIndex for UI read.
    #[must_use]
    pub(crate) fn active_fastq_index(&self) -> Option<&FastqIndex> {
        self.active_document
            .and_then(|id| self.fastq_indexes.get(&id))
    }

    /// Quality stats for a FASTQ record (overlay-aware).
    #[must_use]
    pub(crate) fn fastq_quality_for(&self, id: DocumentId, rec_num: u64) -> Option<QualityStats> {
        let doc = self.documents.get(id)?;
        if doc.format() != SequenceFormat::Fastq {
            return None;
        }
        if self.record_edit_flags(id, rec_num).deleted {
            return None;
        }
        let (_h, _seq, qual) = self.fields_for(id, rec_num)?;
        let qual = qual?;
        Some(phred33_quality_stats(qual.as_bytes(), 20))
    }

    /// The currently selected record number (0-based), within the active
    /// document. None if no document is open or no record is selected.
    #[must_use]
    pub(crate) fn current_record_number(&self) -> Option<u64> {
        self.current_record_number
    }

    /// Select a record by number and scroll to it.
    pub(crate) fn go_to_record(&mut self, record_number: u64) {
        self.current_record_number = Some(record_number);
        let Some(id) = self.active_document else {
            return;
        };
        if let Some(idx) = self.fasta_indexes.get(&id) {
            if let Some(entry) = idx
                .entries()
                .get(usize::try_from(record_number).unwrap_or(usize::MAX))
            {
                self.scroll_active_to_byte(entry.start_offset);
                return;
            }
        }
        if let Some(idx) = self.fastq_indexes.get(&id) {
            if let Some(entry) = idx
                .entries()
                .get(usize::try_from(record_number).unwrap_or(usize::MAX))
            {
                self.scroll_active_to_byte(entry.start_offset);
            }
        }
    }

    /// Select the next record.
    pub(crate) fn next_record(&mut self) {
        let n = self.current_record_number.unwrap_or(0);
        self.go_to_record(n.saturating_add(1));
    }

    /// Select the previous record.
    pub(crate) fn prev_record(&mut self) {
        let n = self.current_record_number.unwrap_or(1);
        self.go_to_record(n.saturating_sub(1));
    }

    /// Compute statistics for the given record's sequence (overlay-aware).
    /// Returns None if the record is deleted, missing, or not FASTA.
    #[must_use]
    pub(crate) fn record_stats(&self, id: DocumentId, rec: u64) -> Option<(BaseCounts, f64)> {
        let doc = self.documents.get(id)?;
        if doc.format() != SequenceFormat::Fasta {
            return None;
        }
        if self.record_edit_flags(id, rec).deleted {
            return None;
        }
        let (_h, seq, _q) = self.fields_for(id, rec)?;
        let counts = count_bases(seq.as_bytes());
        let gc = gc_percent(&counts);
        Some((counts, gc))
    }

    /// Overlay edit flags for a specific record on `id`.
    #[must_use]
    pub(crate) fn record_edit_flags(&self, id: DocumentId, rec: u64) -> RecordEditFlags {
        let Some(ov) = self.overlays.get(&id) else {
            return RecordEditFlags::default();
        };
        let Some(edits) = ov.edits_for(rec) else {
            return RecordEditFlags::default();
        };
        let mut flags = RecordEditFlags::default();
        for edit in edits {
            match edit {
                RecordEdit::Delete { .. } => flags.deleted = true,
                RecordEdit::Replace { .. } => flags.replaced = true,
                RecordEdit::InsertBefore { .. } => flags.inserts_before += 1,
                RecordEdit::InsertAfter { .. } => flags.inserts_after += 1,
            }
        }
        flags
    }

    /// Overlay flags for the currently selected record.
    #[must_use]
    pub(crate) fn current_record_edit_flags(&self) -> RecordEditFlags {
        let (Some(id), Some(rec)) = (self.active_document, self.current_record_number) else {
            return RecordEditFlags::default();
        };
        self.record_edit_flags(id, rec)
    }

    /// Build an overlay-resolved preview for the current record (for UI).
    #[must_use]
    pub(crate) fn current_overlay_preview(&self) -> Option<OverlayRecordPreview> {
        let id = self.active_document?;
        let rec = self.current_record_number?;
        self.overlay_preview_for(id, rec)
    }

    /// Overlay-resolved preview for any indexed record.
    #[must_use]
    pub(crate) fn overlay_preview_for(
        &self,
        id: DocumentId,
        rec: u64,
    ) -> Option<OverlayRecordPreview> {
        let doc = self.documents.get(id)?;
        let format = doc.format();
        let flags = self.record_edit_flags(id, rec);

        let (inserts_before, inserts_after) = self.collect_insert_previews(id, rec);

        if flags.deleted {
            return Some(OverlayRecordPreview {
                flags,
                header: None,
                sequence: None,
                quality: None,
                body_preview: format!(
                    "(record {} deleted in overlay — omitted on Save edits)",
                    rec + 1
                ),
                inserts_before,
                inserts_after,
            });
        }

        // Ensure the record exists in an index (or has a replace payload).
        let fields = self.fields_for(id, rec);
        let Some((header, sequence, quality)) = fields else {
            if flags.has_any() {
                // Inserts only on a still-loading record edge case.
                return Some(OverlayRecordPreview {
                    flags,
                    header: None,
                    sequence: None,
                    quality: None,
                    body_preview: "(record not available yet)".into(),
                    inserts_before,
                    inserts_after,
                });
            }
            return None;
        };

        let body = match build_record_bytes(format, &header, &sequence, quality.as_deref()) {
            Ok(bytes) => truncate_preview(&String::from_utf8_lossy(&bytes)),
            Err(_) => truncate_preview(&format!(">{header}\n{sequence}\n")),
        };

        Some(OverlayRecordPreview {
            flags,
            header: Some(header),
            sequence: Some(truncate_preview(&sequence)),
            quality: quality.map(|q| truncate_preview(&q)),
            body_preview: body,
            inserts_before,
            inserts_after,
        })
    }

    /// The active FastaIndex for UI read (immutable borrow).
    #[must_use]
    pub(crate) fn active_fasta_index(&self) -> Option<&FastaIndex> {
        self.active_document
            .and_then(|id| self.fasta_indexes.get(&id))
    }

    /// Scroll the active viewer to a byte offset (Home/End / "Go to offset").
    pub(crate) fn scroll_active_to_byte(&mut self, byte_offset: u64) {
        if let Some(id) = self.active_document {
            if let Some(viewer) = self.viewers.get_mut(&id) {
                viewer.scroll_to_byte(byte_offset);
            }
        }
    }

    /// The byte offset at the top of the active viewport (for the status bar).
    #[must_use]
    pub(crate) const fn active_top_offset(&self) -> u64 {
        self.active_top_offset
    }

    /// Record the top offset reported by the viewer this frame.
    pub(crate) fn set_active_top_offset(&mut self, offset: u64) {
        self.active_top_offset = offset;
    }

    // ---- Search ----

    /// Borrow the search input text (mutably, for the UI text field).
    pub(crate) fn search_input_mut(&mut self) -> &mut String {
        &mut self.search_input
    }

    /// Read-only access to search input.
    #[must_use]
    pub(crate) fn search_input(&self) -> &str {
        &self.search_input
    }

    /// Set the current search mode.
    pub(crate) fn set_search_mode(&mut self, mode: SearchMode) {
        self.search_mode = mode;
    }

    /// Get the current search mode.
    #[must_use]
    pub(crate) fn search_mode(&self) -> SearchMode {
        self.search_mode
    }

    /// Start a new search on the active document with the current input/mode.
    pub(crate) fn start_search(&mut self) {
        let Some(id) = self.active_document else {
            return;
        };
        let Some(doc) = self.documents.get(id) else {
            return;
        };
        let file_size = doc.metadata().size;
        let pattern = self.search_input.as_bytes().to_vec();
        let mode = self.search_mode;
        let mut session = match mode {
            SearchMode::FromPosition => {
                SearchSession::from_offset(mode, pattern, true, self.active_top_offset, file_size)
            }
            _ => SearchSession::new(mode, pattern, true, file_size),
        };

        // For ID search, run against the index immediately.
        if matches!(mode, SearchMode::RecordIdExact | SearchMode::RecordIdPrefix) {
            if let Some(fasta_idx) = self.fasta_indexes.get(&id) {
                let doc_bytes = doc.bytes();
                let entries = fasta_idx.entries();
                session.search_ids(entries.len(), |i| {
                    let e = &entries[i];
                    let s = usize::try_from(e.id_range.start)
                        .unwrap_or(0)
                        .min(doc_bytes.len());
                    let en = usize::try_from(e.id_range.end)
                        .unwrap_or(s)
                        .min(doc_bytes.len());
                    doc_bytes[s..en].to_vec()
                });
            }
        }

        self.search_sessions.insert(id, session);
        self.current_search_result = None;
    }

    /// Advance the active search session by one chunk (called each frame).
    fn advance_search(&mut self) {
        let Some(id) = self.active_document else {
            return;
        };
        let Some(doc) = self.documents.get(id) else {
            return;
        };
        let bytes = doc.bytes();
        if let Some(session) = self.search_sessions.get_mut(&id) {
            if !session.is_complete() && !session.is_cancelled() {
                session.search_chunk(bytes, 4 * 1024 * 1024);
            }
        }
    }

    /// Cancel the active search.
    pub(crate) fn cancel_search(&mut self) {
        if let Some(id) = self.active_document {
            if let Some(session) = self.search_sessions.get_mut(&id) {
                session.cancel();
            }
        }
    }

    /// Get search results for the active document (pre-collected to avoid
    /// borrow conflicts with click handlers).
    pub(crate) fn search_results_snapshot(&self) -> Vec<(ByteRange, Option<u64>, String)> {
        let Some(id) = self.active_document else {
            return Vec::new();
        };
        let Some(session) = self.search_sessions.get(&id) else {
            return Vec::new();
        };
        session
            .results()
            .iter()
            .map(|r| {
                let preview = String::from_utf8_lossy(&r.preview)
                    .chars()
                    .take(60)
                    .collect();
                (r.byte_range, r.record_number, preview)
            })
            .collect()
    }

    /// Whether the active search is still running.
    #[must_use]
    pub(crate) fn search_is_running(&self) -> bool {
        self.active_document
            .and_then(|id| self.search_sessions.get(&id))
            .is_some_and(|s| !s.is_complete() && !s.is_cancelled())
    }

    /// Search progress percentage (0-100).
    #[must_use]
    pub(crate) fn search_progress_pct(&self) -> u8 {
        let Some(id) = self.active_document else {
            return 0;
        };
        let Some(session) = self.search_sessions.get(&id) else {
            return 0;
        };
        let file_size = self.active_file_size().max(1);
        u8::try_from(session.scan_progress() * 100 / file_size).unwrap_or(0)
    }

    /// Navigate to the next search result.
    pub(crate) fn next_search_result(&mut self) {
        let results = self.search_results_snapshot();
        if results.is_empty() {
            return;
        }
        let idx = self.current_search_result.unwrap_or(0);
        let next = (idx + 1).min(results.len() - 1);
        self.current_search_result = Some(next);
        self.scroll_active_to_byte(results[next].0.start);
    }

    /// Navigate to the previous search result.
    pub(crate) fn prev_search_result(&mut self) {
        let results = self.search_results_snapshot();
        if results.is_empty() {
            return;
        }
        let idx = self.current_search_result.unwrap_or(0);
        let prev = idx.saturating_sub(1);
        self.current_search_result = Some(prev);
        self.scroll_active_to_byte(results[prev].0.start);
    }

    /// Navigate to a specific search result by index.
    pub(crate) fn goto_search_result(&mut self, index: usize) {
        let results = self.search_results_snapshot();
        if index < results.len() {
            self.current_search_result = Some(index);
            self.scroll_active_to_byte(results[index].0.start);
        }
    }

    /// Highlight ranges for the active search (for the viewer).
    #[allow(dead_code)] // viewer highlight integration is a refinement
    #[must_use]
    pub(crate) fn search_highlights(&self) -> Vec<ByteRange> {
        let Some(id) = self.active_document else {
            return Vec::new();
        };
        let Some(session) = self.search_sessions.get(&id) else {
            return Vec::new();
        };
        session.results().iter().map(|r| r.byte_range).collect()
    }

    /// Index of the currently selected search result.
    #[must_use]
    pub(crate) fn current_search_result_index(&self) -> Option<usize> {
        self.current_search_result
    }

    /// Drain any text staged for clipboard copy.
    pub(crate) fn take_pending_clipboard(&mut self) -> Option<String> {
        self.pending_clipboard.take()
    }

    /// Open the "Go to offset" dialog.
    pub(crate) fn open_goto_offset_dialog(&mut self) {
        self.show_goto_offset = true;
        self.goto_offset_input.clear();
    }

    /// Whether the "Go to offset" dialog should be shown.
    #[must_use]
    pub(crate) const fn show_goto_offset(&self) -> bool {
        self.show_goto_offset
    }

    /// Borrow the "Go to offset" input text (mutably).
    pub(crate) fn goto_offset_input_mut(&mut self) -> &mut String {
        &mut self.goto_offset_input
    }

    /// Close the "Go to offset" dialog and, if `apply` is true, jump there.
    pub(crate) fn close_goto_offset_dialog(&mut self, apply: bool) {
        if apply {
            if let Ok(offset) = self.goto_offset_input.trim().parse::<u64>() {
                self.scroll_active_to_byte(offset);
            }
        }
        self.show_goto_offset = false;
    }

    /// The active document's file size (for the goto dialog's hint).
    #[must_use]
    pub(crate) fn active_file_size(&self) -> u64 {
        self.active_document().map_or(0, |d| d.metadata().size)
    }

    /// Copy the visible text of the active document to the clipboard.
    pub(crate) fn copy_active_visible_text(&mut self) {
        // M2 basic copy: copy a bounded prefix of the file (first 64 KiB) as
        // lossy text. Full visible-viewport copy is a refinement for later.
        const COPY_LIMIT: usize = 64 * 1024;

        let Some(id) = self.active_document else {
            return;
        };
        let Some(doc) = self.documents.get(id) else {
            return;
        };
        let bytes = doc.bytes();
        let end = bytes.len().min(COPY_LIMIT);
        let text = String::from_utf8_lossy(&bytes[..end]).into_owned();
        self.notice = Some(format!("Copied first {end} bytes to clipboard."));
        // Stash the text so the UI layer can place it on the clipboard.
        self.pending_clipboard = Some(text);
    }
    pub(crate) fn check_active_source(&mut self) {
        let Some(id) = self.active_document else {
            return;
        };
        let Some(doc) = self.documents.get(id) else {
            return;
        };
        match doc.has_external_changes() {
            Ok(true) => {
                self.notice = Some("The source file changed on disk.".to_string());
            }
            Ok(false) => {
                self.notice = Some("The source file is unchanged.".to_string());
            }
            Err(err) => {
                self.notice = Some(format!("Could not check source file: {err}"));
            }
        }
    }

    fn record_recent(&mut self, path: PathBuf) {
        self.recent_files.retain(|p| p != &path);
        self.recent_files.insert(0, path);
        self.recent_files.truncate(RECENT_FILES_LIMIT);
        self.settings.recent_files = self.recent_files.clone();
        // Auto-save if the settings path is known.
        if let Some(ref sp) = self.settings_path.clone() {
            let _ = self.settings.save_to_path(sp);
        }
    }

    pub(crate) fn export_current_record(
        &self,
        rec: u64,
        path: &std::path::Path,
        transform: Transform,
    ) -> Result<(), String> {
        use seqflash_types::SequenceFormat;
        let id = self.active_document.ok_or("No document.")?;
        let doc = self.documents.get(id).ok_or("No doc.")?;
        let bytes = doc.bytes();
        match doc.format() {
            SequenceFormat::Fasta => {
                let idx = self.fasta_indexes.get(&id).ok_or("No index.")?;
                let e = idx
                    .entries()
                    .get(usize::try_from(rec).unwrap_or(usize::MAX))
                    .ok_or("Bad idx")?;
                let hs = usize::try_from(e.header_range.start).unwrap_or(0);
                let he = usize::try_from(e.header_range.end).unwrap_or(0);
                let es = usize::try_from(e.end_offset)
                    .unwrap_or(bytes.len())
                    .min(bytes.len());
                let hdr = slice_header(&bytes[hs..he]);
                export_fasta_records(
                    &[FastaExportRecord {
                        header: hdr,
                        sequence: &bytes[he..es],
                    }],
                    path,
                    transform,
                )
                .map_err(|e| e.to_string())
            }
            SequenceFormat::Fastq => {
                let idx = self.fastq_indexes.get(&id).ok_or("No index.")?;
                let e = idx
                    .entries()
                    .get(usize::try_from(rec).unwrap_or(usize::MAX))
                    .ok_or("Bad idx")?;
                let hs = usize::try_from(e.header_range.start).unwrap_or(0);
                let he = usize::try_from(e.header_range.end).unwrap_or(0);
                let ss = usize::try_from(e.sequence_range.start).unwrap_or(0);
                let se = usize::try_from(e.sequence_range.end)
                    .unwrap_or(bytes.len())
                    .min(bytes.len());
                let qs = usize::try_from(e.quality_range.start).unwrap_or(0);
                let qe = usize::try_from(e.quality_range.end)
                    .unwrap_or(bytes.len())
                    .min(bytes.len());
                let hdr = slice_header(&bytes[hs..he]);
                export_fastq_records(
                    &[FastqExportRecord {
                        header: hdr,
                        sequence: &bytes[ss..se],
                        quality: &bytes[qs..qe],
                    }],
                    path,
                    transform,
                )
                .map_err(|e| e.to_string())
            }
            SequenceFormat::Unknown => Err("Unsupported format".to_string()),
        }
    }

    pub(crate) fn copy_current_header(&mut self) {
        if let Some(t) = self.current_header_text() {
            self.pending_clipboard = Some(t);
        }
    }
    pub(crate) fn copy_current_sequence(&mut self) {
        if let Some(t) = self.current_sequence_text() {
            self.pending_clipboard = Some(t);
        }
    }
    pub(crate) fn copy_current_quality(&mut self) {
        if let Some(t) = self.current_quality_text() {
            self.pending_clipboard = Some(t);
        }
    }

    // ---- M7: Edit overlay ----

    /// Active document has unsaved overlay edits.
    #[must_use]
    pub(crate) fn active_is_dirty(&self) -> bool {
        self.active_document
            .is_some_and(|id| self.document_is_dirty(id))
    }

    /// Whether `id` has unsaved overlay edits.
    #[must_use]
    pub(crate) fn document_is_dirty(&self, id: DocumentId) -> bool {
        self.overlays.get(&id).is_some_and(EditOverlay::is_dirty)
    }

    /// Number of records with pending edits on the active document.
    #[must_use]
    pub(crate) fn active_edit_count(&self) -> usize {
        self.active_document
            .and_then(|id| self.overlays.get(&id))
            .map_or(0, EditOverlay::edited_record_count)
    }

    /// Whether the currently selected record is marked deleted in the overlay.
    #[must_use]
    pub(crate) fn current_record_is_deleted(&self) -> bool {
        let (Some(id), Some(rec)) = (self.active_document, self.current_record_number) else {
            return false;
        };
        self.overlays
            .get(&id)
            .and_then(|ov| ov.edits_for(rec))
            .is_some_and(|edits| edits.iter().any(RecordEdit::is_delete))
    }

    #[must_use]
    pub(crate) fn can_undo(&self) -> bool {
        self.active_document
            .and_then(|id| self.overlays.get(&id))
            .is_some_and(EditOverlay::can_undo)
    }

    #[must_use]
    pub(crate) fn can_redo(&self) -> bool {
        self.active_document
            .and_then(|id| self.overlays.get(&id))
            .is_some_and(EditOverlay::can_redo)
    }

    pub(crate) fn undo_edit(&mut self) {
        let Some(id) = self.active_document else {
            return;
        };
        if self.overlays.entry(id).or_default().undo() {
            self.notice = Some("Undid last edit.".into());
        }
    }

    pub(crate) fn redo_edit(&mut self) {
        let Some(id) = self.active_document else {
            return;
        };
        if self.overlays.entry(id).or_default().redo() {
            self.notice = Some("Redid edit.".into());
        }
    }

    #[must_use]
    pub(crate) const fn show_edit_header(&self) -> bool {
        self.show_edit_header
    }
    #[must_use]
    pub(crate) const fn show_edit_seq(&self) -> bool {
        self.show_edit_seq
    }
    #[must_use]
    pub(crate) const fn show_edit_qual(&self) -> bool {
        self.show_edit_qual
    }

    pub(crate) fn edit_header_input_mut(&mut self) -> &mut String {
        &mut self.edit_header_input
    }
    pub(crate) fn edit_seq_input_mut(&mut self) -> &mut String {
        &mut self.edit_seq_input
    }
    pub(crate) fn edit_qual_input_mut(&mut self) -> &mut String {
        &mut self.edit_qual_input
    }

    /// Open the edit-header dialog, pre-filled from the current (overlay-aware) header.
    pub(crate) fn open_edit_header_dialog(&mut self) {
        if !self.ensure_record_editable() {
            return;
        }
        self.edit_header_input = self.current_header_text().unwrap_or_default();
        self.show_edit_header = true;
        self.show_edit_seq = false;
        self.show_edit_qual = false;
        self.show_insert = false;
    }

    /// Open the edit-sequence dialog.
    pub(crate) fn open_edit_seq_dialog(&mut self) {
        if !self.ensure_record_editable() {
            return;
        }
        self.edit_seq_input = self.current_sequence_text().unwrap_or_default();
        self.show_edit_seq = true;
        self.show_edit_header = false;
        self.show_edit_qual = false;
        self.show_insert = false;
    }

    /// Open the edit-quality dialog (FASTQ only).
    pub(crate) fn open_edit_qual_dialog(&mut self) {
        if !self.ensure_record_editable() {
            return;
        }
        let is_fastq = self
            .active_document()
            .is_some_and(|d| d.format() == SequenceFormat::Fastq);
        if !is_fastq {
            self.notice = Some("Quality editing is only available for FASTQ.".into());
            return;
        }
        self.edit_qual_input = self.current_quality_text().unwrap_or_default();
        self.show_edit_qual = true;
        self.show_edit_header = false;
        self.show_edit_seq = false;
        self.show_insert = false;
    }

    /// Open the insert-record dialog (before or after the current record).
    pub(crate) fn open_insert_dialog(&mut self, before: bool) {
        if self.current_record_number.is_none() {
            self.notice = Some("Select a record first.".into());
            return;
        }
        // Verify the anchor record exists in the index.
        if self.current_record_byte_size().is_none() {
            self.notice = Some("Record not found in the index yet.".into());
            return;
        }
        self.insert_before = before;
        self.edit_header_input = "new_record".into();
        self.edit_seq_input = "N".into();
        // Phred+33 Q40 placeholder so FASTQ length matches a single 'N'.
        self.edit_qual_input = "I".into();
        self.show_insert = true;
        self.show_edit_header = false;
        self.show_edit_seq = false;
        self.show_edit_qual = false;
    }

    #[must_use]
    pub(crate) const fn show_insert(&self) -> bool {
        self.show_insert
    }

    #[must_use]
    pub(crate) const fn insert_before(&self) -> bool {
        self.insert_before
    }

    pub(crate) fn set_insert_before(&mut self, before: bool) {
        self.insert_before = before;
    }

    pub(crate) fn close_insert_dialog(&mut self, apply: bool) {
        if apply {
            if let Err(msg) = self.apply_insert() {
                self.notice = Some(msg);
                return;
            }
            let where_ = if self.insert_before {
                "before"
            } else {
                "after"
            };
            self.notice = Some(format!(
                "Inserted record {where_} current (overlay). Save to write a new file."
            ));
        }
        self.show_insert = false;
    }

    pub(crate) fn close_edit_header_dialog(&mut self, apply: bool) {
        if apply {
            if let Err(msg) = self.apply_header_edit() {
                self.notice = Some(msg);
                return;
            }
            self.notice = Some("Header updated (overlay). Save to write a new file.".into());
        }
        self.show_edit_header = false;
    }

    pub(crate) fn close_edit_seq_dialog(&mut self, apply: bool) {
        if apply {
            if let Err(msg) = self.apply_sequence_edit() {
                self.notice = Some(msg);
                return;
            }
            self.notice = Some("Sequence updated (overlay). Save to write a new file.".into());
        }
        self.show_edit_seq = false;
    }

    pub(crate) fn close_edit_qual_dialog(&mut self, apply: bool) {
        if apply {
            if let Err(msg) = self.apply_quality_edit() {
                self.notice = Some(msg);
                return;
            }
            self.notice = Some("Quality updated (overlay). Save to write a new file.".into());
        }
        self.show_edit_qual = false;
    }

    /// Mark the current record as deleted in the overlay (source file untouched).
    pub(crate) fn delete_current_record(&mut self) {
        if !self.ensure_record_editable() {
            return;
        }
        let (Some(id), Some(rec)) = (self.active_document, self.current_record_number) else {
            return;
        };
        self.overlays
            .entry(id)
            .or_default()
            .apply(RecordEdit::Delete { record_number: rec });
        self.notice = Some(format!(
            "Record {} marked deleted (overlay). Save to write a new file.",
            rec + 1
        ));
    }

    /// Whether an overlay save is currently running in the background.
    #[must_use]
    pub(crate) fn save_in_progress(&self) -> bool {
        self.save_job.is_some()
    }

    /// `(done, total)` progress of the in-flight save, if any.
    #[must_use]
    pub(crate) fn save_progress(&self) -> Option<(u64, u64)> {
        self.save_job.as_ref().map(|j| {
            (
                j.done.load(Ordering::Relaxed),
                j.total.load(Ordering::Relaxed),
            )
        })
    }

    /// Request cancellation of the in-flight overlay save.
    pub(crate) fn cancel_save(&mut self) {
        if let Some(job) = &self.save_job {
            job.cancel.store(true, Ordering::Relaxed);
            self.notice = Some("Cancelling save…".into());
        }
    }

    /// Start streaming the active document through the overlay into `path`
    /// on a background thread (plan 20.3). Never overwrites the open source.
    ///
    /// # Errors
    ///
    /// Returns immediately if preconditions fail (no doc, overwrite source,
    /// indexing incomplete, save already running). The write itself reports
    /// via [`poll_save_job`].
    pub(crate) fn start_save_with_overlay(
        &mut self,
        path: &Path,
        ctx: &egui::Context,
    ) -> Result<(), String> {
        if self.save_job.is_some() {
            return Err("A save is already in progress.".into());
        }

        let id = self.active_document.ok_or("No document open.")?;
        let prep = self.prepare_overlay_save(id, path)?;
        let total_records = match &prep.entries {
            OverlaySaveEntries::Fasta(e) => e.len() as u64,
            OverlaySaveEntries::Fastq(e) => e.len() as u64,
        };

        let cancel = Arc::new(AtomicBool::new(false));
        let done = Arc::new(AtomicU64::new(0));
        let total = Arc::new(AtomicU64::new(total_records));
        let result: Arc<Mutex<Option<Result<(), String>>>> = Arc::new(Mutex::new(None));

        spawn_overlay_save_worker(
            prep,
            Arc::clone(&cancel),
            Arc::clone(&done),
            Arc::clone(&total),
            Arc::clone(&result),
            ctx.clone(),
        );

        self.save_job = Some(OverlaySaveJob {
            document_id: id,
            dest: path.to_path_buf(),
            cancel,
            done,
            total,
            result,
        });
        self.notice = Some(format!("Saving {total_records} record(s)…"));
        Ok(())
    }

    /// Validate preconditions and snapshot everything the worker needs.
    fn prepare_overlay_save(
        &mut self,
        id: DocumentId,
        path: &Path,
    ) -> Result<OverlaySavePrep, String> {
        let (source_path, format, opened_size, opened_modified) = {
            let doc = self.documents.get(id).ok_or("Document missing.")?;
            let meta = doc.metadata();
            (meta.path.clone(), doc.format(), meta.size, meta.modified)
        };
        if same_path(&source_path, path) {
            return Err(
                "Cannot overwrite the open source file. Choose a different path (Save As).".into(),
            );
        }
        {
            let doc = self.documents.get(id).ok_or("Document missing.")?;
            if doc
                .has_external_changes()
                .map_err(|e| format!("Could not check source file: {e}"))?
            {
                return Err(
                    "Source file changed on disk. Re-open the file before saving edits.".into(),
                );
            }
        }

        let overlay = self.overlays.entry(id).or_default().clone();
        let entries = match format {
            SequenceFormat::Fasta => {
                let idx = self.fasta_indexes.get(&id).ok_or("FASTA index missing.")?;
                if !idx.is_complete() {
                    return Err("Indexing is still running. Wait until it finishes.".into());
                }
                OverlaySaveEntries::Fasta(
                    idx.entries()
                        .iter()
                        .map(|e| FastaOverlayEntry {
                            record_number: e.record_number,
                            start_offset: e.start_offset,
                            end_offset: e.end_offset,
                        })
                        .collect(),
                )
            }
            SequenceFormat::Fastq => {
                let idx = self.fastq_indexes.get(&id).ok_or("FASTQ index missing.")?;
                if !idx.is_complete() {
                    return Err("Indexing is still running. Wait until it finishes.".into());
                }
                OverlaySaveEntries::Fastq(
                    idx.entries()
                        .iter()
                        .map(|e| FastqOverlayEntry {
                            record_number: e.record_number,
                            start_offset: e.start_offset,
                            end_offset: e.end_offset,
                        })
                        .collect(),
                )
            }
            SequenceFormat::Unknown => {
                return Err("Unsupported format for overlay save.".into());
            }
        };

        Ok(OverlaySavePrep {
            source_path,
            dest: path.to_path_buf(),
            opened_size,
            opened_modified,
            overlay,
            entries,
        })
    }

    /// Poll the background save job; apply success/failure notices and clear overlay.
    fn poll_save_job(&mut self) {
        let finished = {
            let Some(job) = &self.save_job else {
                return;
            };
            job.result.lock().ok().and_then(|mut g| g.take())
        };
        let Some(outcome) = finished else {
            return;
        };
        let Some(job) = self.save_job.take() else {
            return;
        };
        match outcome {
            Ok(()) => {
                if let Some(ov) = self.overlays.get_mut(&job.document_id) {
                    ov.clear();
                }
                self.notice = Some(format!("Saved with edits to {}.", job.dest.display()));
                tracing::info!(path = %job.dest.display(), "overlay save complete");
            }
            Err(msg) => {
                // Keep overlay so the user can retry or continue editing.
                if msg.to_ascii_lowercase().contains("cancel") {
                    self.notice = Some("Save cancelled. Overlay edits were kept.".into());
                } else {
                    self.notice = Some(format!("Save failed: {msg}"));
                }
                tracing::warn!(path = %job.dest.display(), error = %msg, "overlay save failed");
            }
        }
    }

    /// Header text for the current record (overlay-aware).
    #[must_use]
    pub(crate) fn current_header_text(&self) -> Option<String> {
        let id = self.active_document?;
        let rec = self.current_record_number?;
        self.fields_for(id, rec).map(|(h, _, _)| h)
    }

    /// Sequence text for the current record (overlay-aware, whitespace-stripped display).
    #[must_use]
    pub(crate) fn current_sequence_text(&self) -> Option<String> {
        let id = self.active_document?;
        let rec = self.current_record_number?;
        self.fields_for(id, rec).map(|(_, s, _)| s)
    }

    /// Quality text for the current FASTQ record (overlay-aware).
    #[must_use]
    pub(crate) fn current_quality_text(&self) -> Option<String> {
        let id = self.active_document?;
        let rec = self.current_record_number?;
        self.fields_for(id, rec).and_then(|(_, _, q)| q)
    }

    /// Reject oversized / missing / already-deleted records before edit UI opens.
    fn ensure_record_editable(&mut self) -> bool {
        let Some(rec) = self.current_record_number else {
            self.notice = Some("Select a record first.".into());
            return false;
        };
        if self.current_record_is_deleted() {
            self.notice = Some(format!(
                "Record {} is marked deleted. Undo to restore it.",
                rec + 1
            ));
            return false;
        }
        let Some(size) = self.current_record_byte_size() else {
            self.notice = Some("Record not found in the index yet.".into());
            return false;
        };
        // Configurable threshold (plan 18.3); fall back to the ops default.
        let limit = if self.settings.record_edit_limit_bytes == 0 {
            RECORD_EDIT_LIMIT_BYTES
        } else {
            self.settings.record_edit_limit_bytes
        };
        if size > limit {
            self.notice = Some(format!(
                "Record is too large to edit in-memory ({size} bytes > {limit} limit)."
            ));
            return false;
        }
        true
    }

    fn current_record_byte_size(&self) -> Option<u64> {
        let id = self.active_document?;
        let rec = self.current_record_number?;
        // Prefer last Replace payload size when present.
        if let Some(ov) = self.overlays.get(&id) {
            if let Some(edits) = ov.edits_for(rec) {
                if let Some(data) = edits.iter().rev().find_map(|e| e.data()) {
                    return Some(data.len() as u64);
                }
            }
        }
        let doc = self.documents.get(id)?;
        match doc.format() {
            seqflash_types::SequenceFormat::Fasta => {
                let e = self
                    .fasta_indexes
                    .get(&id)?
                    .entries()
                    .get(usize::try_from(rec).ok()?)?;
                Some(e.end_offset.saturating_sub(e.start_offset))
            }
            seqflash_types::SequenceFormat::Fastq => {
                let e = self
                    .fastq_indexes
                    .get(&id)?
                    .entries()
                    .get(usize::try_from(rec).ok()?)?;
                Some(e.end_offset.saturating_sub(e.start_offset))
            }
            seqflash_types::SequenceFormat::Unknown => None,
        }
    }

    /// Insert payloads (preview strings) for a record's overlay edits.
    fn collect_insert_previews(&self, id: DocumentId, rec: u64) -> (Vec<String>, Vec<String>) {
        let mut before = Vec::new();
        let mut after = Vec::new();
        let Some(ov) = self.overlays.get(&id) else {
            return (before, after);
        };
        let Some(edits) = ov.edits_for(rec) else {
            return (before, after);
        };
        for edit in edits {
            match edit {
                RecordEdit::InsertBefore { data, .. } => {
                    before.push(truncate_preview(&String::from_utf8_lossy(data)));
                }
                RecordEdit::InsertAfter { data, .. } => {
                    after.push(truncate_preview(&String::from_utf8_lossy(data)));
                }
                RecordEdit::Delete { .. } | RecordEdit::Replace { .. } => {}
            }
        }
        (before, after)
    }

    /// Overlay-aware (header, sequence, optional quality) for any record.
    fn fields_for(&self, id: DocumentId, rec: u64) -> Option<(String, String, Option<String>)> {
        let doc = self.documents.get(id)?;

        // If a Replace is present, parse fields from the replacement payload.
        if let Some(ov) = self.overlays.get(&id) {
            if let Some(edits) = ov.edits_for(rec) {
                if edits.iter().any(RecordEdit::is_delete) {
                    return None;
                }
                if let Some(data) = edits.iter().rev().find_map(|e| match e {
                    RecordEdit::Replace { data, .. } => Some(data.as_slice()),
                    _ => None,
                }) {
                    return parse_record_fields(doc.format(), data);
                }
            }
        }

        // Fall back to original index ranges.
        let bytes = doc.bytes();
        match doc.format() {
            SequenceFormat::Fasta => {
                let e = self
                    .fasta_indexes
                    .get(&id)?
                    .entries()
                    .get(usize::try_from(rec).ok()?)?;
                let hs = usize::try_from(e.header_range.start).ok()?;
                let he = usize::try_from(e.header_range.end).ok()?;
                let se = usize::try_from(e.end_offset).ok()?.min(bytes.len());
                let header = String::from_utf8_lossy(slice_header(&bytes[hs..he.min(bytes.len())]))
                    .into_owned();
                let seq = strip_ws(&bytes[he.min(bytes.len())..se]);
                Some((header, seq, None))
            }
            SequenceFormat::Fastq => {
                let e = self
                    .fastq_indexes
                    .get(&id)?
                    .entries()
                    .get(usize::try_from(rec).ok()?)?;
                let hs = usize::try_from(e.header_range.start).ok()?;
                let he = usize::try_from(e.header_range.end).ok()?;
                let ss = usize::try_from(e.sequence_range.start).ok()?;
                let se = usize::try_from(e.sequence_range.end).ok()?.min(bytes.len());
                let qs = usize::try_from(e.quality_range.start).ok()?;
                let qe = usize::try_from(e.quality_range.end).ok()?.min(bytes.len());
                let header = String::from_utf8_lossy(slice_header(&bytes[hs..he.min(bytes.len())]))
                    .into_owned();
                let seq = strip_ws(&bytes[ss..se]);
                let qual = strip_ws(&bytes[qs..qe]);
                Some((header, seq, Some(qual)))
            }
            SequenceFormat::Unknown => None,
        }
    }

    fn apply_header_edit(&mut self) -> Result<(), String> {
        let (Some(id), Some(rec)) = (self.active_document, self.current_record_number) else {
            return Err("No record selected.".into());
        };
        let format = self
            .documents
            .get(id)
            .map(Document::format)
            .ok_or("Document missing.")?;
        let (_old_header, seq, qual) = self
            .fields_for(id, rec)
            .ok_or("Cannot load current record fields.")?;
        let new_header = self.edit_header_input.trim();
        if new_header.is_empty() {
            return Err("Header must not be empty.".into());
        }
        let data = build_record_bytes(format, new_header, &seq, qual.as_deref())?;
        self.overlays
            .entry(id)
            .or_default()
            .apply(RecordEdit::Replace {
                record_number: rec,
                data,
            });
        Ok(())
    }

    fn apply_sequence_edit(&mut self) -> Result<(), String> {
        let (Some(id), Some(rec)) = (self.active_document, self.current_record_number) else {
            return Err("No record selected.".into());
        };
        let format = self
            .documents
            .get(id)
            .map(Document::format)
            .ok_or("Document missing.")?;
        let (header, _old_seq, qual) = self
            .fields_for(id, rec)
            .ok_or("Cannot load current record fields.")?;
        let new_seq = strip_ws(self.edit_seq_input.as_bytes());
        if new_seq.is_empty() {
            return Err("Sequence must not be empty.".into());
        }
        // For FASTQ, keep quality length in sync when possible: if lengths
        // diverge, require the user to edit quality as well.
        let qual = match (format, qual) {
            (SequenceFormat::Fastq, Some(q)) if q.len() != new_seq.len() => {
                return Err(format!(
                    "Sequence length ({}) does not match quality length ({}). Edit quality too, or keep lengths equal.",
                    new_seq.len(),
                    q.len()
                ));
            }
            (_, q) => q,
        };
        let data = build_record_bytes(format, &header, &new_seq, qual.as_deref())?;
        self.overlays
            .entry(id)
            .or_default()
            .apply(RecordEdit::Replace {
                record_number: rec,
                data,
            });
        Ok(())
    }

    fn apply_quality_edit(&mut self) -> Result<(), String> {
        let (Some(id), Some(rec)) = (self.active_document, self.current_record_number) else {
            return Err("No record selected.".into());
        };
        let format = self
            .documents
            .get(id)
            .map(Document::format)
            .ok_or("Document missing.")?;
        if format != SequenceFormat::Fastq {
            return Err("Quality editing is only available for FASTQ.".into());
        }
        let (header, seq, _old_q) = self
            .fields_for(id, rec)
            .ok_or("Cannot load current record fields.")?;
        let new_qual = strip_ws(self.edit_qual_input.as_bytes());
        if new_qual.len() != seq.len() {
            return Err(format!(
                "Quality length ({}) must equal sequence length ({}).",
                new_qual.len(),
                seq.len()
            ));
        }
        let data = build_record_bytes(format, &header, &seq, Some(&new_qual))?;
        self.overlays
            .entry(id)
            .or_default()
            .apply(RecordEdit::Replace {
                record_number: rec,
                data,
            });
        Ok(())
    }

    fn apply_insert(&mut self) -> Result<(), String> {
        let (Some(id), Some(rec)) = (self.active_document, self.current_record_number) else {
            return Err("No record selected.".into());
        };
        let format = self
            .documents
            .get(id)
            .map(Document::format)
            .ok_or("Document missing.")?;
        let header = self.edit_header_input.trim();
        if header.is_empty() {
            return Err("Header must not be empty.".into());
        }
        let seq = strip_ws(self.edit_seq_input.as_bytes());
        if seq.is_empty() {
            return Err("Sequence must not be empty.".into());
        }
        let qual = match format {
            SequenceFormat::Fastq => {
                let q = strip_ws(self.edit_qual_input.as_bytes());
                if q.len() != seq.len() {
                    return Err(format!(
                        "Quality length ({}) must equal sequence length ({}).",
                        q.len(),
                        seq.len()
                    ));
                }
                Some(q)
            }
            SequenceFormat::Fasta | SequenceFormat::Unknown => None,
        };
        let data = build_record_bytes(format, header, &seq, qual.as_deref())?;
        let edit = if self.insert_before {
            RecordEdit::InsertBefore {
                record_number: rec,
                data,
            }
        } else {
            RecordEdit::InsertAfter {
                record_number: rec,
                data,
            }
        };
        self.overlays.entry(id).or_default().apply(edit);
        Ok(())
    }
}

/// Strip whitespace / newlines from sequence or quality bytes into a String.
fn strip_ws(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect()
}

/// Truncate long preview strings for UI panels.
fn truncate_preview(s: &str) -> String {
    let mut out: String = s.chars().take(PREVIEW_FIELD_CHARS).collect();
    if s.chars().count() > PREVIEW_FIELD_CHARS {
        out.push('…');
    }
    out
}

/// Parse fields out of a full record payload (produced by our builders or original).
fn parse_record_fields(
    format: seqflash_types::SequenceFormat,
    data: &[u8],
) -> Option<(String, String, Option<String>)> {
    use seqflash_types::SequenceFormat;
    match format {
        SequenceFormat::Fasta => {
            let text = String::from_utf8_lossy(data);
            let mut lines = text.lines();
            let header_line = lines.next()?;
            let header = header_line
                .strip_prefix('>')
                .unwrap_or(header_line)
                .trim()
                .to_string();
            let seq: String = lines
                .flat_map(str::chars)
                .filter(|c| !c.is_whitespace())
                .collect();
            Some((header, seq, None))
        }
        SequenceFormat::Fastq => {
            let text = String::from_utf8_lossy(data);
            let mut lines = text.lines().filter(|l| !l.is_empty());
            let header_line = lines.next()?;
            let header = header_line
                .strip_prefix('@')
                .unwrap_or(header_line)
                .trim()
                .to_string();
            // After header: sequence lines until a '+' line, then quality lines.
            let mut seq = String::new();
            let mut qual = String::new();
            let mut in_qual = false;
            for line in lines {
                if !in_qual && line.starts_with('+') {
                    in_qual = true;
                    continue;
                }
                if in_qual {
                    qual.extend(line.chars().filter(|c| !c.is_whitespace()));
                } else {
                    seq.extend(line.chars().filter(|c| !c.is_whitespace()));
                }
            }
            Some((header, seq, Some(qual)))
        }
        SequenceFormat::Unknown => None,
    }
}

/// Build full record bytes for a Replace edit.
fn build_record_bytes(
    format: seqflash_types::SequenceFormat,
    header: &str,
    sequence: &str,
    quality: Option<&str>,
) -> Result<Vec<u8>, String> {
    use seqflash_types::SequenceFormat;
    let header = header.trim();
    let seq: String = sequence.chars().filter(|c| !c.is_whitespace()).collect();
    match format {
        SequenceFormat::Fasta => {
            let mut out = Vec::with_capacity(header.len() + seq.len() + 16);
            out.push(b'>');
            out.extend_from_slice(header.as_bytes());
            out.push(b'\n');
            // Wrap at 80 columns for readability; empty seq still gets a newline.
            if seq.is_empty() {
                out.push(b'\n');
            } else {
                for chunk in seq.as_bytes().chunks(80) {
                    out.extend_from_slice(chunk);
                    out.push(b'\n');
                }
            }
            Ok(out)
        }
        SequenceFormat::Fastq => {
            let qual = quality.ok_or("FASTQ quality required.")?;
            let qual: String = qual.chars().filter(|c| !c.is_whitespace()).collect();
            if seq.len() != qual.len() {
                return Err(format!(
                    "Sequence length ({}) != quality length ({}).",
                    seq.len(),
                    qual.len()
                ));
            }
            let mut out = Vec::with_capacity(header.len() + seq.len() + qual.len() + 16);
            out.push(b'@');
            out.extend_from_slice(header.as_bytes());
            out.push(b'\n');
            out.extend_from_slice(seq.as_bytes());
            out.push(b'\n');
            out.push(b'+');
            out.push(b'\n');
            out.extend_from_slice(qual.as_bytes());
            out.push(b'\n');
            Ok(out)
        }
        SequenceFormat::Unknown => Err("Unsupported format.".into()),
    }
}

/// Turn a document-open failure into a short, actionable notice (M8 error UX).
fn user_facing_open_error(path: &Path, err: &seqflash_document::DocumentError) -> String {
    let name = path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    let detail = err.to_string();
    let hint = if detail.to_ascii_lowercase().contains("not found")
        || detail.to_ascii_lowercase().contains("cannot find")
    {
        " Check that the path exists and is readable."
    } else if detail.to_ascii_lowercase().contains("access")
        || detail.to_ascii_lowercase().contains("permission")
        || detail.to_ascii_lowercase().contains("denied")
    {
        " The file may be locked or you may lack permission."
    } else if detail.to_ascii_lowercase().contains("memory")
        || detail.to_ascii_lowercase().contains("map")
    {
        " The file could not be memory-mapped (disk or OS limit)."
    } else {
        ""
    };
    format!("Could not open “{name}”: {detail}.{hint}")
}

/// Best-effort path equality (case-insensitive on Windows).
fn same_path(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    // Canonicalize when possible so relative vs absolute compare fairly.
    if let (Ok(ca), Ok(cb)) = (a.canonicalize(), b.canonicalize()) {
        ca == cb
    } else {
        // Fall back to lowercase display compare on Windows.
        let sa = a.to_string_lossy().to_lowercase();
        let sb = b.to_string_lossy().to_lowercase();
        sa == sb
    }
}

/// True when the on-disk size or mtime differs from the values recorded at open.
fn source_changed_since(
    path: &Path,
    opened_size: u64,
    opened_modified: std::time::SystemTime,
) -> Result<bool, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("Could not stat source file: {e}"))?;
    let modified = meta
        .modified()
        .map_err(|e| format!("Could not read source mtime: {e}"))?;
    Ok(meta.len() != opened_size || modified != opened_modified)
}

/// Snapshot handed to the overlay-save worker thread.
struct OverlaySavePrep {
    source_path: PathBuf,
    dest: PathBuf,
    opened_size: u64,
    opened_modified: std::time::SystemTime,
    overlay: EditOverlay,
    entries: OverlaySaveEntries,
}

fn spawn_overlay_save_worker(
    prep: OverlaySavePrep,
    cancel: Arc<AtomicBool>,
    done: Arc<AtomicU64>,
    total: Arc<AtomicU64>,
    result: Arc<Mutex<Option<Result<(), String>>>>,
    ctx: egui::Context,
) {
    std::thread::spawn(move || {
        let outcome = run_overlay_save_worker(prep, &cancel, &done, &total, &ctx);
        if let Ok(mut guard) = result.lock() {
            *guard = Some(outcome);
        }
        ctx.request_repaint();
    });
}

fn run_overlay_save_worker(
    prep: OverlaySavePrep,
    cancel: &AtomicBool,
    done: &AtomicU64,
    total: &AtomicU64,
    ctx: &egui::Context,
) -> Result<(), String> {
    let OverlaySavePrep {
        source_path,
        dest,
        opened_size,
        opened_modified,
        overlay,
        entries,
    } = prep;

    // Compare against the UI-open fingerprint (plan 20.4).
    if source_changed_since(&source_path, opened_size, opened_modified)? {
        return Err("Source file changed on disk. Re-open the file before saving edits.".into());
    }
    // Re-open the source read-only so we do not share the UI-thread mmap.
    let src = Document::open(&source_path, DocumentId::new(0))
        .map_err(|e| format!("Could not re-open source for save: {e}"))?;
    let bytes = src.bytes();
    let cancel_fn = || cancel.load(Ordering::Relaxed);
    let progress_fn = |d: u64, t: u64| {
        done.store(d, Ordering::Relaxed);
        total.store(t, Ordering::Relaxed);
        ctx.request_repaint();
    };
    match entries {
        OverlaySaveEntries::Fasta(e) => {
            save_fasta_with_overlay_ex(bytes, &e, &overlay, &dest, cancel_fn, progress_fn)
                .map_err(|e| e.to_string())?;
        }
        OverlaySaveEntries::Fastq(e) => {
            save_fastq_with_overlay_ex(bytes, &e, &overlay, &dest, cancel_fn, progress_fn)
                .map_err(|e| e.to_string())?;
        }
    }
    // Final check before the user trusts the published file.
    if source_changed_since(&source_path, opened_size, opened_modified)? {
        let _ = std::fs::remove_file(&dest);
        return Err("Source file changed on disk during save. Overlay was kept.".into());
    }
    Ok(())
}

/// Strip leading >/@ and trailing newlines from a header line.
fn slice_header(hdr: &[u8]) -> &[u8] {
    let s = if hdr.first() == Some(&b'>') || hdr.first() == Some(&b'@') {
        &hdr[1..]
    } else {
        hdr
    };
    let t = s
        .iter()
        .rposition(|&b| b != b'\n' && b != b'\r')
        .map_or(s.len(), |p| p + 1);
    &s[..t]
}

impl eframe::App for SeqFlashApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Apply a path picked by an async file-dialog worker, if it has landed.
        if let Some(path) = self.take_pending_open() {
            self.open_path(&path);
        }

        // Complete background overlay saves (progress + cancel).
        self.poll_save_job();
        // Keep animating the progress indicator while a save runs.
        if self.save_job.is_some() {
            ctx.request_repaint();
        }

        // Advance the FASTA record index for the active document a little
        // each frame (incremental, non-blocking).
        self.advance_index_scan();
        self.advance_search();

        // Handle files dragged onto the window. On Windows the backend fills
        // `DroppedFile.path` only (no inline bytes), so we open from disk.
        let dropped: Vec<egui::DroppedFile> = ctx.input(|i| i.raw.dropped_files.clone());
        if !dropped.is_empty() {
            for file in dropped {
                if let Some(path) = file.path {
                    self.open_path(&path);
                }
            }
            // Consume so the same drop isn't handled every frame.
            ctx.input_mut(|i| i.raw.dropped_files.clear());
        }

        ui::draw(self, ctx);
    }
}
