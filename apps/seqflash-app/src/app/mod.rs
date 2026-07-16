//! The `eframe::App` for the SeqFlash main window.
//!
//! M1: open files via dialog or drag-and-drop, show one tab per document, and
//! render a small byte-level preview plus a status bar. The full virtual
//! scrolling text/record viewer, search, statistics, and editing arrive in
//! later milestones (plan section 22 describes the eventual layout).

mod ui;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use eframe::egui;

use seqflash_document::{Document, DocumentList};
use seqflash_formats::detect_format;
use seqflash_index::{FastaIndex, FastqIndex};
use seqflash_ops::{count_bases, gc_percent, phred33_quality_stats, BaseCounts, QualityStats};
use seqflash_settings::AppSettings;
use seqflash_types::DocumentId;
use seqflash_viewer::RawTextViewer;

/// Maximum number of entries kept in the in-memory recent-files list.
const RECENT_FILES_LIMIT: usize = 10;

/// Top-level SeqFlash egui application.
pub(crate) struct SeqFlashApp {
    #[allow(dead_code)]
    settings: AppSettings,
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
}

impl SeqFlashApp {
    /// Construct the application from the already-loaded settings, optionally
    /// opening `initial_file` right away (e.g. from a command-line argument).
    pub(crate) fn new(settings: AppSettings, initial_file: Option<PathBuf>) -> Self {
        let mut app = Self {
            settings,
            documents: DocumentList::new(),
            active_document: None,
            recent_files: Vec::new(),
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
        };
        if let Some(path) = initial_file {
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
                let msg = format!("Could not open {}: {err}", path.display());
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

    /// Quality stats for a FASTQ record.
    #[must_use]
    pub(crate) fn fastq_quality_for(&self, id: DocumentId, rec_num: u64) -> Option<QualityStats> {
        let doc = self.documents.get(id)?;
        let idx = self.fastq_indexes.get(&id)?;
        let entry = idx
            .entries()
            .get(usize::try_from(rec_num).unwrap_or(usize::MAX))?;
        let bytes = doc.bytes();
        let qs = usize::try_from(entry.quality_range.start).unwrap_or(0);
        let qe = usize::try_from(entry.quality_range.end)
            .unwrap_or(bytes.len())
            .min(bytes.len());
        if qs >= qe {
            return None;
        }
        let qual: Vec<u8> = bytes[qs..qe]
            .iter()
            .copied()
            .filter(|&b| b != b'\n' && b != b'\r')
            .collect();
        Some(phred33_quality_stats(&qual, 20))
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
            if record_number < idx.entries().len() as u64 {
                let entry = &idx.entries()[usize::try_from(record_number).unwrap_or(usize::MAX)];
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

    /// Compute statistics for the given record's sequence bytes.
    /// Returns None if the record doesn't exist or is not FASTA.
    #[must_use]
    pub(crate) fn record_stats(&self, id: DocumentId, rec: u64) -> Option<(BaseCounts, f64)> {
        let doc = self.documents.get(id)?;
        let idx = self.fasta_indexes.get(&id)?;
        let entries = idx.entries();
        let entry = entries.get(usize::try_from(rec).unwrap_or(usize::MAX))?;
        let bytes = doc.bytes();
        // Extract sequence bytes: skip past header line, strip newlines.
        let hdr_end = entry.header_range.end;
        let seq_start = usize::try_from(hdr_end + 1).unwrap_or(0); // skip past \n
        let seq_end = usize::try_from(entry.end_offset)
            .unwrap_or(bytes.len())
            .min(bytes.len());
        // Only consider FASTA format for stats.
        if doc.format() != seqflash_types::SequenceFormat::Fasta {
            return None;
        }
        if seq_start >= seq_end {
            let counts = BaseCounts::default();
            return Some((counts, 0.0));
        }
        let raw: Vec<u8> = bytes[seq_start..seq_end]
            .iter()
            .copied()
            .filter(|&b| b != b'\n' && b != b'\r')
            .collect();
        let counts = count_bases(&raw);
        let gc = gc_percent(&counts);
        Some((counts, gc))
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
    }
}

impl eframe::App for SeqFlashApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Apply a path picked by an async file-dialog worker, if it has landed.
        if let Some(path) = self.take_pending_open() {
            self.open_path(&path);
        }

        // Advance the FASTA record index for the active document a little
        // each frame (incremental, non-blocking).
        self.advance_index_scan();

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
