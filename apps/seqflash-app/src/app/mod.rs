//! The `eframe::App` for the SeqFlash main window.
//!
//! M1: open files via dialog or drag-and-drop, show one tab per document, and
//! render a small byte-level preview plus a status bar. The full virtual
//! scrolling text/record viewer, search, statistics, and editing arrive in
//! later milestones (plan section 22 describes the eventual layout).

mod ui;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use eframe::egui;

use seqflash_document::{Document, DocumentList};
use seqflash_settings::AppSettings;
use seqflash_types::DocumentId;

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
        if was_active {
            self.active_document = self.documents.iter().next().map(Document::id);
        }
    }

    /// Check the active document for on-disk changes since it was opened.
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
