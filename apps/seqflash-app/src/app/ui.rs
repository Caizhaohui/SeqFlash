//! M1 window layout: a toolbar, a tab strip, a byte-level preview, and a
//! status bar. This is a deliberate stepping stone — the full three-pane
//! layout (plan section 22) and the virtual-scrolling viewer (M2) come later.

use std::path::Path;

use eframe::egui;

use crate::app::SeqFlashApp;
use seqflash_document::Document;
use seqflash_types::DocumentId;

/// Render the whole window for one frame.
pub(crate) fn draw(app: &mut SeqFlashApp, ctx: &egui::Context) {
    egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
        toolbar(app, ui);
    });

    egui::TopBottomPanel::top("tab_strip")
        .exact_height(26.0)
        .show(ctx, |ui| {
            tab_strip(app, ui);
        });

    egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
        status_bar(app, ui);
    });

    egui::CentralPanel::default().show(ctx, |ui| {
        let active_id = app.active_document_id();
        if let (Some(doc_id), Some(doc)) = (active_id, app.active_document()) {
            preview(doc_id, doc, ui);
        } else {
            empty_state(ui);
        }
    });
}

/// Toolbar: Open button + change-check button + drag hint.
fn toolbar(app: &mut SeqFlashApp, ui: &mut egui::Ui) {
    ui.horizontal_wrapped(|ui| {
        if ui.button("Open…").clicked() {
            app.open_from_dialog(ui.ctx());
        }
        if app.active_document.is_some() && ui.button("Check source").clicked() {
            app.check_active_source();
        }
        ui.separator();
        ui.label(
            egui::RichText::new("Tip: drag .fasta / .fastq files onto the window")
                .weak()
                .small(),
        );
    });
}

/// One tab per open document, plus a close button on each.
fn tab_strip(app: &mut SeqFlashApp, ui: &mut egui::Ui) {
    // Collect tabs up front so we don't hold a borrow of `app` while the
    // click handlers below mutate it.
    let entries: Vec<(DocumentId, String)> = app
        .document_entries()
        .into_iter()
        .map(|(id, path, _size)| (id, tab_label(&path)))
        .collect();

    ui.horizontal_wrapped(|ui| {
        if entries.is_empty() {
            ui.label(egui::RichText::new("no document open").weak().small());
            return;
        }
        let active = app.active_document;
        for (id, label) in entries {
            let clicked = ui
                .selectable_label(active == Some(id), &label)
                .on_hover_text("Click to activate");
            if clicked.clicked() {
                app.active_document = Some(id);
            }
            if ui
                .button("×")
                .on_hover_text("Close this document")
                .clicked()
            {
                app.close_document(id);
            }
            ui.separator();
        }
    });
}

/// Central area when a document is open: the virtual-scrolling byte viewer.
///
/// Only the visible rows are formatted and drawn (`show_viewport`); the whole
/// file is never handed to a widget or scanned during rendering (plan 9.5).
fn preview(doc_id: DocumentId, doc: &Document, ui: &mut egui::Ui) {
    use seqflash_viewer::ByteViewer;

    ui.heading("Raw byte view");
    ui.add_space(4.0);

    // One ByteViewer per central panel; it is cheap (just a width config) and
    // stateless — scroll position is persisted by egui keyed on `id_salt`.
    let viewer = ByteViewer::new();
    viewer.show(ui, ("byte_view", doc_id.get()), doc.bytes());
}

/// Central area when no document is open.
fn empty_state(ui: &mut egui::Ui) {
    ui.vertical_centered(|ui| {
        ui.add_space(60.0);
        ui.heading("SeqFlash");
        ui.label("A FASTA/FASTQ browser for large sequence files.");
        ui.add_space(12.0);
        ui.label(egui::RichText::new("Click “Open…” or drag a .fasta / .fastq file here.").weak());
    });
}

/// Bottom status bar: path, size, format, active tab.
fn status_bar(app: &mut SeqFlashApp, ui: &mut egui::Ui) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 18.0;
        let count = app.document_count();
        match app.active_document() {
            Some(doc) => {
                let meta = doc.metadata();
                ui.label(format!("📄 {}", display_path(&meta.path)));
                ui.label(byte_size_label(meta.size));
                ui.label("Unknown"); // format detection arrives in M3/M4
                ui.label(format!("tab {}/{}", tab_index(app, doc.id()) + 1, count));
            }
            None => {
                ui.label(format!("{count} document(s) open"));
            }
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if let Some(notice) = app.notice_text() {
                ui.label(egui::RichText::new(notice).small().weak());
            }
        });
    });
}

// ---- small free helpers -------------------------------------------------

fn tab_label(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

fn display_path(path: &Path) -> String {
    path.display().to_string()
}

/// Human-readable file size using binary units, e.g. "1.5 GiB". Pure integer
/// arithmetic avoids any float-precision lint.
fn byte_size_label(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = KIB * 1024;
    const GIB: u64 = MIB * 1024;
    // One decimal digit via scaled integer division: tenths = (bytes * 10) / unit.
    match bytes {
        0..KIB => format!("{bytes} B"),
        KIB..MIB => format!("{}.{:01} KiB", bytes / KIB, (bytes % KIB) * 10 / KIB),
        MIB..GIB => format!("{}.{:01} MiB", bytes / MIB, (bytes % MIB) * 10 / MIB),
        _ => format!("{}.{:02} GiB", bytes / GIB, (bytes % GIB) * 100 / GIB),
    }
}

fn tab_index(app: &SeqFlashApp, id: DocumentId) -> usize {
    app.document_entries()
        .iter()
        .position(|(entry_id, _, _)| *entry_id == id)
        .unwrap_or(0)
}
