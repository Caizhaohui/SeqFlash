//! M3 window layout: toolbar, tab strip, left record panel, central viewer,
//! right info panel with record stats, and a status bar with format/index
//! progress/record count.

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

    // Left record-navigation panel.
    egui::SidePanel::left("record_nav")
        .default_width(220.0)
        .resizable(true)
        .show(ctx, |ui| {
            record_nav_panel(app, ui);
        });

    // Right info/stats panel.
    egui::SidePanel::right("info_panel")
        .default_width(240.0)
        .resizable(true)
        .show(ctx, |ui| {
            info_panel(app, ui);
        });

    egui::CentralPanel::default().show(ctx, |ui| {
        let active_id = app.active_document_id();
        if let Some(doc_id) = active_id {
            app.viewer_for(doc_id);
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

    if let Some(text) = app.take_pending_clipboard() {
        ctx.copy_text(text);
    }
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

/// Left panel: record list, search, prev/next, jump-to-record.
const LIST_LIMIT: usize = 500;
fn record_nav_panel(app: &mut SeqFlashApp, ui: &mut egui::Ui) {
    ui.heading("Records");
    ui.add_space(4.0);

    let Some(doc_id) = app.active_document_id() else {
        ui.label("No document open.");
        return;
    };
    // Lazily create the index.
    app.index_for(doc_id);

    let idx = match app.active_fasta_index() {
        Some(idx) if idx.entry_count() > 0 => idx,
        _ => {
            if let Some(idx) = app.active_fasta_index() {
                let pct = if idx.is_complete() {
                    100
                } else {
                    u8::try_from(idx.scan_progress() * 100 / app.active_file_size().max(1))
                        .unwrap_or(0)
                };
                ui.label(format!("Indexing… {pct}%"));
            } else {
                ui.label("No FASTA index.");
            }
            return;
        }
    };

    // Navigation buttons — operate before borrowing idx info via shared
    // reference to avoid conflicts with the mutable closures below.
    let (total, complete, rec_entries, progress_pct) = {
        let idx_pct =
            u8::try_from(idx.scan_progress() * 100 / app.active_file_size().max(1)).unwrap_or(0);
        (
            idx.entry_count(),
            idx.is_complete(),
            idx.entries().to_vec(),
            idx_pct,
        )
    };

    let done_str = if complete {
        " ✓".to_string()
    } else {
        format!(" ({progress_pct}% scanned)")
    };
    ui.label(format!("{total} record(s).{done_str}"));
    ui.add_space(4.0);

    // Navigation buttons.
    ui.horizontal(|ui| {
        if ui.button("◀ Prev").clicked() {
            app.prev_record();
        }
        if ui.button("Next ▶").clicked() {
            app.next_record();
        }
    });

    // Record-number jump.
    ui.horizontal(|ui| {
        ui.label("Go to rec:");
        let mut rec_input = String::new();
        // We use a simple text edit; the value is consumed immediately on Enter.
        let resp = ui.add(
            egui::TextEdit::singleline(&mut rec_input)
                .hint_text("1")
                .desired_width(60.0),
        );
        if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            if let Ok(n) = rec_input.trim().parse::<u64>() {
                if n > 0 {
                    app.go_to_record(n - 1);
                }
            }
        }
    });

    ui.separator();

    // Scrollable record list (limited to avoid rendering 100k+ labels).
    let shown = rec_entries.len().min(LIST_LIMIT);
    let current = app.current_record_number();
    // Pre-compute the displayed labels so the `.show()` closure doesn't need to
    // immutably borrow `app` while the mutable `app.go_to_record` calls require
    // unique access.
    let doc_bytes_slice: &[u8] = app.active_document().map_or(&[][..], |d| d.bytes());
    let shown_labels: Vec<(usize, u64, String)> = rec_entries
        .iter()
        .take(shown)
        .enumerate()
        .map(|(i, entry)| {
            let id_bytes = id_slice_from_entry(entry, doc_bytes_slice);
            let id_str = String::from_utf8_lossy(&id_bytes).into_owned();
            (i, i as u64, format!("{}. {}", i + 1, id_str))
        })
        .collect();

    egui::ScrollArea::vertical()
        .id_salt("rec_list")
        .show(ui, |ui| {
            for (_i, rec_num, label) in &shown_labels {
                let is_current = current == Some(*rec_num);
                if ui.selectable_label(is_current, label).clicked() {
                    app.go_to_record(*rec_num);
                }
            }
        });
    if rec_entries.len() > LIST_LIMIT {
        ui.label(format!(
            "… showing first {LIST_LIMIT} of {} records",
            rec_entries.len()
        ));
    }
}

/// Extract the ID bytes for an entry from the document byte buffer.
fn id_slice_from_entry(entry: &seqflash_index::FastaRecordEntry, doc_bytes: &[u8]) -> Vec<u8> {
    let start = usize::try_from(entry.id_range.start).unwrap_or(0);
    let end = usize::try_from(entry.id_range.end)
        .unwrap_or(doc_bytes.len())
        .min(doc_bytes.len());
    doc_bytes[start..end].to_vec()
}

/// Right panel: record statistics.
fn info_panel(app: &mut SeqFlashApp, ui: &mut egui::Ui) {
    ui.heading("Record Info");
    ui.add_space(4.0);

    let Some(doc_id) = app.active_document_id() else {
        ui.label("No document.");
        return;
    };
    let Some(rec) = app.current_record_number() else {
        ui.label("Click a record to view stats.");
        return;
    };
    let Some((counts, gc)) = app.record_stats(doc_id, rec) else {
        ui.label("Stats unavailable.");
        return;
    };

    ui.label(format!("Record #: {}", rec + 1));
    ui.label(format!("Length: {} bases", counts.total()));
    ui.label(format!("GC: {gc:.1}%"));
    ui.separator();
    ui.label(format!("A: {}", counts.a));
    ui.label(format!("C: {}", counts.c));
    ui.label(format!("G: {}", counts.g));
    ui.label(format!("T: {}", counts.t));
    if counts.u > 0 {
        ui.label(format!("U: {}", counts.u));
    }
    ui.label(format!("N: {}", counts.n));
    ui.label(format!("Other (IUPAC/gap): {}", counts.other));
    if counts.illegal > 0 {
        ui.label(
            egui::RichText::new(format!("Illegal chars: {}", counts.illegal))
                .color(egui::Color32::RED),
        );
    }
    let total_acgt = counts.a + counts.c + counts.g + counts.t + counts.u;
    if total_acgt == 0 {
        ui.label("Empty sequence");
    }
}

/// Bottom status bar: path, size, format, record count, offset.
fn status_bar(app: &mut SeqFlashApp, ui: &mut egui::Ui) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 18.0;
        let count = app.document_count();
        match app.active_document() {
            Some(doc) => {
                let meta = doc.metadata();
                ui.label(format!("📄 {}", display_path(&meta.path)));
                ui.label(byte_size_label(meta.size));
                // Format label (was hardcoded "Unknown" in M2; M3 uses detection).
                ui.label(doc.format().label());
                // Record count + indexing progress.
                if let Some(idx) = app.active_fasta_index() {
                    let rec = idx.entry_count();
                    if idx.is_complete() {
                        ui.label(format!(
                            "Record {}/{}",
                            app.current_record_number().map_or(0, |n| n + 1),
                            rec
                        ));
                    } else {
                        let pct =
                            u8::try_from(idx.scan_progress() * 100 / meta.size.max(1)).unwrap_or(0);
                        ui.label(format!(
                            "Indexing {pct}% ({} records+dead)",
                            idx.entry_count()
                        ));
                    }
                }
                ui.label(format!(
                    "offset {} / {}",
                    app.active_top_offset(),
                    meta.size
                ));
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
