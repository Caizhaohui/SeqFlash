//! M2 window layout: a toolbar (open / go-to-offset / copy / check), a tab
//! strip, the virtual-scrolling raw-text viewer, a "Go to offset" modal, and
//! a status bar with the current byte offset. The full three-pane layout
//! (plan section 22) comes in a later milestone.

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
        if let Some(doc_id) = active_id {
            // Ensure a viewer exists for this document (lazily created).
            app.viewer_for(doc_id);
            // Take the viewer out of the map so we can borrow `app.documents`
            // for the bytes slice at the same time (avoids a borrow conflict).
            let mut viewer = app.viewers.remove(&doc_id);
            let bytes = app.documents.get(doc_id).map_or(&[][..], Document::bytes);
            let top_offset = match &mut viewer {
                Some(v) => v.show(ui, ("raw_text_view", doc_id.get()), bytes),
                None => 0,
            };
            app.set_active_top_offset(top_offset);
            if let Some(v) = viewer {
                app.viewers.insert(doc_id, v);
            }
        } else {
            empty_state(ui);
        }
    });

    // Drain any pending clipboard copy onto the egui command queue.
    if let Some(text) = app.take_pending_clipboard() {
        ctx.copy_text(text);
    }

    // "Go to offset" modal dialog.
    if app.show_goto_offset() {
        goto_offset_dialog(app, ctx);
    }
}

/// Modal dialog for jumping to a byte offset.
fn goto_offset_dialog(app: &mut SeqFlashApp, ctx: &egui::Context) {
    let mut open = true;
    let file_size = app.active_file_size();
    egui::Window::new("Go to offset")
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.label(format!("Enter a byte offset (0 – {file_size}):"));
            ui.add(
                egui::TextEdit::singleline(app.goto_offset_input_mut())
                    .hint_text("e.g. 1048576")
                    .desired_width(220.0),
            );
            ui.horizontal(|ui| {
                if ui.button("Go").clicked() {
                    app.close_goto_offset_dialog(true);
                }
                if ui.button("Cancel").clicked() {
                    app.close_goto_offset_dialog(false);
                }
            });
        });
    if !open {
        app.close_goto_offset_dialog(false);
    }
}

/// Toolbar: Open / Go to offset / Copy visible / Check source + drag hint.
fn toolbar(app: &mut SeqFlashApp, ui: &mut egui::Ui) {
    ui.horizontal_wrapped(|ui| {
        if ui.button("Open…").clicked() {
            app.open_from_dialog(ui.ctx());
        }
        if app.active_document.is_some() {
            if ui.button("Go to offset…").clicked() {
                app.open_goto_offset_dialog();
            }
            if ui.button("Copy visible").clicked() {
                app.copy_active_visible_text();
            }
            if ui.button("Check source").clicked() {
                app.check_active_source();
            }
        }
        ui.separator();
        ui.label(
            egui::RichText::new(
                "Tip: drag .fasta / .fastq files onto the window; Home/End/PgUp/PgDn to navigate",
            )
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
                ui.label(format!(
                    "offset {} / {}",
                    app.active_top_offset(),
                    meta.size
                ));
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
