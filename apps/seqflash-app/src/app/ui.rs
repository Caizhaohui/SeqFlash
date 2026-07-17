//! M3 window layout: toolbar, tab strip, left record panel, central viewer,
//! right info panel with record stats, and a status bar with format/index
//! progress/record count.

use std::path::Path;

use eframe::egui;

use crate::app::SeqFlashApp;
use seqflash_document::Document;
use seqflash_ops::Transform;
use seqflash_search::SearchMode;
use seqflash_types::DocumentId;

/// Render the whole window for one frame.
pub(crate) fn draw(app: &mut SeqFlashApp, ctx: &egui::Context) {
    handle_edit_shortcuts(app, ctx);

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
            // Overlay-resolved preview for the selected record (delete/replace/insert).
            overlay_preview_panel(app, ui);
            ui.separator();
            ui.label(
                egui::RichText::new("Source (read-only mmap — pre-overlay bytes)")
                    .weak()
                    .small(),
            );
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
    if app.show_edit_header() {
        edit_header_dialog(app, ctx);
    }
    if app.show_edit_seq() {
        edit_seq_dialog(app, ctx);
    }
    if app.show_edit_qual() {
        edit_qual_dialog(app, ctx);
    }
    if app.show_insert() {
        insert_record_dialog(app, ctx);
    }
    if app.save_in_progress() {
        save_progress_panel(app, ctx);
    }
}

/// Ctrl+Z / Ctrl+Y (and Ctrl+Shift+Z) for overlay undo/redo.
fn handle_edit_shortcuts(app: &mut SeqFlashApp, ctx: &egui::Context) {
    // Skip while a modal text dialog is open so Ctrl+Z edits the text field.
    if app.show_edit_header()
        || app.show_edit_seq()
        || app.show_edit_qual()
        || app.show_insert()
        || app.show_goto_offset()
    {
        return;
    }
    let (undo, redo) = ctx.input(|i| {
        let ctrl = i.modifiers.command;
        let undo = ctrl && !i.modifiers.shift && i.key_pressed(egui::Key::Z);
        let redo = ctrl
            && (i.key_pressed(egui::Key::Y) || (i.modifiers.shift && i.key_pressed(egui::Key::Z)));
        (undo, redo)
    });
    if undo {
        app.undo_edit();
    } else if redo {
        app.redo_edit();
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

/// Toolbar: Open / Go to offset / Copy visible / Check source / Undo-Redo / Save edits.
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
            ui.separator();
            let can_undo = app.can_undo();
            let can_redo = app.can_redo();
            if ui
                .add_enabled(can_undo, egui::Button::new("Undo"))
                .on_hover_text("Ctrl+Z")
                .clicked()
            {
                app.undo_edit();
            }
            if ui
                .add_enabled(can_redo, egui::Button::new("Redo"))
                .on_hover_text("Ctrl+Y")
                .clicked()
            {
                app.redo_edit();
            }
            let can_save = app.active_is_dirty() && !app.save_in_progress();
            if ui
                .add_enabled(can_save, egui::Button::new("Save edits…"))
                .on_hover_text(
                    "Write a new file with overlay edits applied (never overwrites source)",
                )
                .clicked()
            {
                save_edits_dialog(app, ui.ctx());
            }
            if app.save_in_progress() && ui.button("Cancel save").clicked() {
                app.cancel_save();
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

/// Native save dialog for overlay-aware full-file export (starts a background job).
fn save_edits_dialog(app: &mut SeqFlashApp, ctx: &egui::Context) {
    let path = rfd::FileDialog::new()
        .add_filter("FASTA", &["fa", "fasta", "fna"])
        .add_filter("FASTQ", &["fq", "fastq"])
        .add_filter("All files", &["*"])
        .save_file();
    if let Some(p) = path {
        if let Err(msg) = app.start_save_with_overlay(&p, ctx) {
            tracing::warn!("overlay save failed to start: {msg}");
            app.set_notice(msg);
        }
    }
}

/// Floating progress window for the background overlay save.
fn save_progress_panel(app: &mut SeqFlashApp, ctx: &egui::Context) {
    let (done, total) = app.save_progress().unwrap_or((0, 0));
    let frac = if total == 0 {
        0.0
    } else {
        #[allow(clippy::cast_precision_loss)]
        {
            done as f32 / total as f32
        }
    };
    egui::Window::new("Saving…")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label(format!("Writing record {done} / {total}"));
            ui.add(egui::ProgressBar::new(frac).show_percentage());
            if ui.button("Cancel").clicked() {
                app.cancel_save();
            }
            ui.label(
                egui::RichText::new("Source file is never modified. Cancel deletes the temp file.")
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
        .map(|(id, path, _size)| {
            let mut label = tab_label(&path);
            if app.document_is_dirty(id) {
                label.push('*');
            }
            (id, label)
        })
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
const SEARCH_LIST_LIMIT: usize = 200;
type IndexMeta = Option<(usize, bool, u8, Vec<(String, bool)>)>;

#[allow(clippy::too_many_lines)]
fn record_nav_panel(app: &mut SeqFlashApp, ui: &mut egui::Ui) {
    ui.heading("Records");
    ui.add_space(4.0);

    // ---- Search bar ----
    search_bar(app, ui);
    ui.add_space(4.0);

    let Some(doc_id) = app.active_document_id() else {
        ui.label("No document open.");
        return;
    };
    // Determine format and create appropriate index.
    let doc = app.active_document();
    let is_fastq = doc.is_some_and(|d| d.format() == seqflash_types::SequenceFormat::Fastq);
    if is_fastq {
        app.index_for_fastq(doc_id);
    } else {
        app.index_for(doc_id);
    }

    // Collect index metadata, dispatching by format.
    // We store (total, complete, progress_pct, has_records, entry_data)
    // where entry_data is Vec<(label: String, is_error: bool)> for the record list.
    let idx_meta: IndexMeta = if is_fastq {
        app.active_fastq_index().map(|idx| {
            let pct = u8::try_from(idx.scan_progress() * 100 / app.active_file_size().max(1))
                .unwrap_or(0);
            let total = idx.entry_count();
            let complete = idx.is_complete();
            let entries: Vec<(String, bool)> = idx
                .entries()
                .iter()
                .enumerate()
                .map(|(i, e)| {
                    let label =
                        format!("{}. {}", i + 1, if e.validation.valid { "✓" } else { "⚠" });
                    (label, !e.validation.valid)
                })
                .collect();
            (total, complete, pct, entries)
        })
    } else {
        app.active_fasta_index().map(|idx| {
            let pct = u8::try_from(idx.scan_progress() * 100 / app.active_file_size().max(1))
                .unwrap_or(0);
            let total = idx.entry_count();
            let complete = idx.is_complete();
            let doc_bytes: &[u8] = app.active_document().map_or(&[][..], |d| d.bytes());
            let entries: Vec<(String, bool)> = idx
                .entries()
                .iter()
                .map(|e| {
                    let id_start = usize::try_from(e.id_range.start)
                        .unwrap_or(0)
                        .min(doc_bytes.len());
                    let id_end = usize::try_from(e.id_range.end)
                        .unwrap_or(id_start)
                        .min(doc_bytes.len());
                    let id_str = if id_end > id_start {
                        String::from_utf8_lossy(&doc_bytes[id_start..id_end]).into_owned()
                    } else {
                        String::new()
                    };
                    (format!("{}. {}", e.record_number + 1, id_str), false)
                })
                .collect();
            (total, complete, pct, entries)
        })
    };

    let Some((total, complete, progress_pct, rec_entries)) = idx_meta else {
        ui.label("Indexing…");
        return;
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
    // Rec_entries already has pre-computed (label, is_error) pairs.
    // Append overlay badges (DEL / EDIT / +B / +A).
    let display_items: Vec<(usize, bool, String, bool, bool)> = rec_entries
        .iter()
        .take(shown)
        .enumerate()
        .map(|(i, (label, is_error))| {
            let flags = app.record_edit_flags(doc_id, i as u64);
            let mut text = label.clone();
            if let Some(badge) = flags.badge() {
                text.push(' ');
                text.push_str(&badge);
            }
            (i, current == Some(i as u64), text, *is_error, flags.deleted)
        })
        .collect();

    egui::ScrollArea::vertical()
        .id_salt("rec_list")
        .show(ui, |ui| {
            for (idx, is_current, label, is_error, is_deleted) in &display_items {
                let rich = if *is_deleted {
                    egui::RichText::new(label.as_str())
                        .color(egui::Color32::from_rgb(220, 80, 80))
                        .strikethrough()
                } else if *is_error {
                    egui::RichText::new(label.as_str()).color(egui::Color32::RED)
                } else if label.contains("[EDIT") || label.contains("[+B") || label.contains("[+A")
                {
                    egui::RichText::new(label.as_str()).color(egui::Color32::from_rgb(220, 160, 40))
                } else {
                    egui::RichText::new(label.as_str())
                };
                let resp = ui.selectable_label(*is_current, rich);
                if resp.clicked() {
                    app.go_to_record(*idx as u64);
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

/// Search input + mode selector + results navigation.
fn search_bar(app: &mut SeqFlashApp, ui: &mut egui::Ui) {
    ui.label(egui::RichText::new("Search").strong());
    // Mode selector (individual buttons for compactness)
    let current_mode = app.search_mode();
    ui.horizontal_wrapped(|ui| {
        for (mode, label) in SEARCH_MODES {
            if ui.selectable_label(current_mode == *mode, *label).clicked() {
                app.set_search_mode(*mode);
            }
        }
    });

    // Search input + buttons
    ui.horizontal(|ui| {
        let resp = ui.add(
            egui::TextEdit::singleline(app.search_input_mut())
                .hint_text("pattern…")
                .desired_width(120.0),
        );
        if resp.lost_focus()
            && ui.input(|i| i.key_pressed(egui::Key::Enter))
            && !app.search_input_mut().is_empty()
        {
            app.start_search();
        }
        if ui.button("🔍").clicked() && !app.search_input().is_empty() {
            app.start_search();
        }
    });

    // Results summary + navigation
    let results = app.search_results_snapshot();
    if !results.is_empty() {
        ui.horizontal(|ui| {
            ui.label(format!("{} hits", results.len()));
            if ui.button("◀").on_hover_text("Prev result").clicked() {
                app.prev_search_result();
            }
            if ui.button("▶").on_hover_text("Next result").clicked() {
                app.next_search_result();
            }
            if app.search_is_running() && ui.button("Cancel").clicked() {
                app.cancel_search();
            }
        });

        // Results list (limited)
        let shown = results.len().min(SEARCH_LIST_LIMIT);
        let current = app.current_search_result_index();
        egui::ScrollArea::vertical()
            .id_salt("search_results")
            .max_height(150.0)
            .show(ui, |ui| {
                for (i, (range, rec, preview)) in results.iter().take(shown).enumerate() {
                    let is_current = current == Some(i);
                    let label = format!(
                        "@{} {}",
                        range.start,
                        if preview.is_empty() { "" } else { preview }
                    );
                    let rich = if is_current {
                        egui::RichText::new(&label).color(egui::Color32::YELLOW)
                    } else {
                        egui::RichText::new(&label)
                    };
                    let rec_info = rec.map_or(String::new(), |r| format!("rec {r}"));
                    let resp = ui.selectable_label(is_current, rich);
                    if resp.clicked() {
                        app.goto_search_result(i);
                    }
                    if !rec_info.is_empty() {
                        ui.label(egui::RichText::new(&rec_info).weak().small());
                    }
                }
            });
        if results.len() > SEARCH_LIST_LIMIT {
            ui.label(
                egui::RichText::new(format!(
                    "… showing first {SEARCH_LIST_LIMIT} of {}",
                    results.len()
                ))
                .weak()
                .small(),
            );
        }
    } else if app.search_is_running() {
        let pct = app.search_progress_pct();
        ui.label(format!("Searching… {pct}%"));
        if ui.button("Cancel").clicked() {
            app.cancel_search();
        }
    }
}

const SEARCH_MODES: &[(SearchMode, &str)] = &[
    (SearchMode::RawBytes, "Bytes"),
    (SearchMode::RecordIdExact, "ID"),
    (SearchMode::RecordIdPrefix, "ID*"),
    (SearchMode::SequenceFragment, "Seq"),
    (SearchMode::CurrentRecord, "Rec"),
    (SearchMode::FromPosition, "Pos"),
];

/// Right panel: record statistics + operations.
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
    let is_fastq = app
        .active_document()
        .is_some_and(|d| d.format() == seqflash_types::SequenceFormat::Fastq);

    overlay_status_section(app, ui);
    record_field_preview_section(app, ui);
    record_stats_section(app, ui, doc_id, rec, is_fastq);
    record_operations_section(app, ui, rec, is_fastq);
}

fn record_stats_section(
    app: &SeqFlashApp,
    ui: &mut egui::Ui,
    doc_id: DocumentId,
    rec: u64,
    is_fastq: bool,
) {
    if app.record_edit_flags(doc_id, rec).deleted {
        ui.label(
            egui::RichText::new("Stats: n/a (deleted in overlay)")
                .weak()
                .italics(),
        );
        return;
    }

    if is_fastq {
        if let Some(qs) = app.fastq_quality_for(doc_id, rec) {
            ui.label(format!("Record #: {}", rec + 1));
            ui.label(format!("Length: {}", qs.total));
            ui.label(format!("Min Q: {}", qs.min));
            ui.label(format!("Max Q: {}", qs.max));
            ui.label(format!("Avg Q: {:.1}", qs.avg));
            ui.separator();
            #[allow(clippy::cast_precision_loss)]
            let low_pct = if qs.total > 0 {
                (qs.low_quality_count as f64 / qs.total as f64) * 100.0
            } else {
                0.0
            };
            if qs.low_quality_count > 0 {
                ui.label(
                    egui::RichText::new(format!(
                        "Low qual (<Q20): {:.1}% ({})",
                        low_pct, qs.low_quality_count
                    ))
                    .color(egui::Color32::RED),
                );
            } else {
                ui.label("Low qual (<Q20): none");
            }
            if app.record_edit_flags(doc_id, rec).replaced {
                ui.label(egui::RichText::new("(stats from overlay)").weak().small());
            }
        } else {
            ui.label("Quality stats unavailable.");
        }
        return;
    }

    if let Some((counts, gc)) = app.record_stats(doc_id, rec) {
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
        if counts.a + counts.c + counts.g + counts.t + counts.u == 0 {
            ui.label("Empty sequence");
        }
        if app.record_edit_flags(doc_id, rec).replaced {
            ui.label(egui::RichText::new("(stats from overlay)").weak().small());
        }
    } else {
        ui.label("Stats unavailable.");
    }
}

fn overlay_status_section(app: &SeqFlashApp, ui: &mut egui::Ui) {
    let flags = app.current_record_edit_flags();
    if flags.deleted {
        ui.label(
            egui::RichText::new("Status: DELETED (pending save)")
                .color(egui::Color32::from_rgb(220, 80, 80))
                .strong(),
        );
    } else if flags.replaced {
        ui.label(
            egui::RichText::new("Status: EDITED (overlay)")
                .color(egui::Color32::from_rgb(220, 160, 40))
                .strong(),
        );
    }
    if flags.inserts_before > 0 || flags.inserts_after > 0 {
        ui.label(
            egui::RichText::new(format!(
                "Inserts: {} before, {} after",
                flags.inserts_before, flags.inserts_after
            ))
            .color(egui::Color32::from_rgb(100, 180, 100)),
        );
    }
    if app.active_is_dirty() {
        ui.label(
            egui::RichText::new(format!(
                "Unsaved edits: {} record(s)",
                app.active_edit_count()
            ))
            .color(egui::Color32::from_rgb(220, 160, 40)),
        );
    }
    if flags.has_any() || app.active_is_dirty() {
        ui.separator();
    }
}

/// Compact header/seq preview in the right panel (overlay-aware).
fn record_field_preview_section(app: &SeqFlashApp, ui: &mut egui::Ui) {
    let Some(preview) = app.current_overlay_preview() else {
        return;
    };
    if let Some(h) = &preview.header {
        ui.label(egui::RichText::new("Header (effective)").strong().small());
        ui.label(egui::RichText::new(h).monospace().small());
    }
    if let Some(s) = &preview.sequence {
        ui.label(egui::RichText::new("Sequence (effective)").strong().small());
        ui.label(egui::RichText::new(s).monospace().small());
    }
    if let Some(q) = &preview.quality {
        ui.label(egui::RichText::new("Quality (effective)").strong().small());
        ui.label(egui::RichText::new(q).monospace().small());
    }
    ui.separator();
}

/// Central-panel overlay preview: effective record body + inserts.
fn overlay_preview_panel(app: &SeqFlashApp, ui: &mut egui::Ui) {
    let Some(preview) = app.current_overlay_preview() else {
        ui.label(
            egui::RichText::new("Select a record to preview effective (overlay) content.")
                .weak()
                .small(),
        );
        return;
    };

    let title = if preview.flags.has_any() {
        "Overlay preview (what Save edits will write for this record)"
    } else {
        "Record preview (no overlay edits)"
    };

    egui::CollapsingHeader::new(title)
        .default_open(preview.flags.has_any())
        .show(ui, |ui| {
            if preview.flags.deleted {
                ui.label(
                    egui::RichText::new(&preview.body_preview)
                        .color(egui::Color32::from_rgb(220, 80, 80))
                        .strong(),
                );
            }

            for (i, text) in preview.inserts_before.iter().enumerate() {
                ui.label(
                    egui::RichText::new(format!("▸ Insert before #{}\n{text}", i + 1))
                        .color(egui::Color32::from_rgb(100, 180, 100))
                        .monospace(),
                );
                ui.separator();
            }

            if !preview.flags.deleted {
                let color = if preview.flags.replaced {
                    egui::Color32::from_rgb(240, 200, 120)
                } else {
                    ui.visuals().text_color()
                };
                ui.label(
                    egui::RichText::new(&preview.body_preview)
                        .monospace()
                        .color(color),
                );
            }

            for (i, text) in preview.inserts_after.iter().enumerate() {
                ui.separator();
                ui.label(
                    egui::RichText::new(format!("▸ Insert after #{}\n{text}", i + 1))
                        .color(egui::Color32::from_rgb(100, 180, 100))
                        .monospace(),
                );
            }

            if !preview.flags.has_any() {
                ui.label(
                    egui::RichText::new(
                        "Edits appear here immediately. Source view below stays read-only.",
                    )
                    .weak()
                    .small(),
                );
            }
        });
}

fn record_operations_section(app: &mut SeqFlashApp, ui: &mut egui::Ui, rec: u64, is_fastq: bool) {
    ui.separator();
    ui.label(egui::RichText::new("Operations").strong());
    ui.add_space(4.0);

    ui.horizontal_wrapped(|ui| {
        if ui.button("Copy Header").clicked() {
            app.copy_current_header();
        }
        if ui.button("Copy Seq").clicked() {
            app.copy_current_sequence();
        }
        if is_fastq && ui.button("Copy Qual").clicked() {
            app.copy_current_quality();
        }
    });

    ui.horizontal_wrapped(|ui| {
        if ui.button("Edit Header…").clicked() {
            app.open_edit_header_dialog();
        }
        if ui.button("Edit Seq…").clicked() {
            app.open_edit_seq_dialog();
        }
        if is_fastq && ui.button("Edit Qual…").clicked() {
            app.open_edit_qual_dialog();
        }
    });
    ui.horizontal_wrapped(|ui| {
        if ui
            .button("Delete record")
            .on_hover_text("Mark deleted in overlay; source file is not modified")
            .clicked()
        {
            app.delete_current_record();
        }
        if ui
            .button("Insert…")
            .on_hover_text("Insert a new record before or after the current one")
            .clicked()
        {
            app.open_insert_dialog(true);
        }
        if ui
            .add_enabled(app.can_undo(), egui::Button::new("Undo"))
            .clicked()
        {
            app.undo_edit();
        }
        if ui
            .add_enabled(app.can_redo(), egui::Button::new("Redo"))
            .clicked()
        {
            app.redo_edit();
        }
    });

    if ui.button("Export record…").clicked() {
        let path = rfd::FileDialog::new()
            .add_filter("FASTA", &["fa", "fasta"])
            .add_filter("FASTQ", &["fq", "fastq"])
            .add_filter("All files", &["*"])
            .save_file();
        if let Some(p) = path {
            if let Err(msg) = app.export_current_record(rec, &p, Transform::None) {
                tracing::warn!("export failed: {msg}");
                app.set_notice(msg);
            } else {
                tracing::info!("exported record {} to {}", rec + 1, p.display());
                app.set_notice(format!("Exported record {} to {}.", rec + 1, p.display()));
            }
        }
    }

    let can_save = app.active_is_dirty() && !app.save_in_progress();
    if ui
        .add_enabled(can_save, egui::Button::new("Save edits…"))
        .on_hover_text("Write a new file applying all overlay edits (background, cancellable)")
        .clicked()
    {
        save_edits_dialog(app, ui.ctx());
    }
}

/// Modal dialog: insert a new record before/after the current one.
fn insert_record_dialog(app: &mut SeqFlashApp, ctx: &egui::Context) {
    let mut open = true;
    let is_fastq = app
        .active_document()
        .is_some_and(|d| d.format() == seqflash_types::SequenceFormat::Fastq);
    let title = if app.insert_before() {
        "Insert record before"
    } else {
        "Insert record after"
    };
    egui::Window::new(title)
        .open(&mut open)
        .resizable(true)
        .collapsible(false)
        .default_width(480.0)
        .default_height(360.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(app.insert_before(), "Before current")
                    .clicked()
                {
                    app.set_insert_before(true);
                }
                if ui
                    .selectable_label(!app.insert_before(), "After current")
                    .clicked()
                {
                    app.set_insert_before(false);
                }
            });
            ui.separator();
            ui.label("Header (without leading > or @):");
            ui.add(
                egui::TextEdit::singleline(app.edit_header_input_mut())
                    .desired_width(f32::INFINITY)
                    .hint_text("new_record"),
            );
            ui.label("Sequence:");
            egui::ScrollArea::vertical()
                .id_salt("insert_seq")
                .max_height(120.0)
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(app.edit_seq_input_mut())
                            .desired_width(f32::INFINITY)
                            .desired_rows(6)
                            .code_editor(),
                    );
                });
            if is_fastq {
                ui.label("Quality (Phred+33; length must match sequence):");
                egui::ScrollArea::vertical()
                    .id_salt("insert_qual")
                    .max_height(80.0)
                    .show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(app.edit_qual_input_mut())
                                .desired_width(f32::INFINITY)
                                .desired_rows(4)
                                .code_editor(),
                        );
                    });
            }
            ui.horizontal(|ui| {
                if ui.button("Insert").clicked() {
                    app.close_insert_dialog(true);
                }
                if ui.button("Cancel").clicked() {
                    app.close_insert_dialog(false);
                }
            });
            ui.label(
                egui::RichText::new("Insert is stored in the overlay until Save edits…")
                    .weak()
                    .small(),
            );
        });
    if !open {
        app.close_insert_dialog(false);
    }
}

/// Modal dialog: edit the current record header.
fn edit_header_dialog(app: &mut SeqFlashApp, ctx: &egui::Context) {
    let mut open = true;
    egui::Window::new("Edit header")
        .open(&mut open)
        .resizable(true)
        .collapsible(false)
        .default_width(420.0)
        .show(ctx, |ui| {
            ui.label("Header (without leading > or @):");
            ui.add(
                egui::TextEdit::singleline(app.edit_header_input_mut())
                    .desired_width(f32::INFINITY)
                    .hint_text("record id and description"),
            );
            ui.horizontal(|ui| {
                if ui.button("Apply").clicked() {
                    app.close_edit_header_dialog(true);
                }
                if ui.button("Cancel").clicked() {
                    app.close_edit_header_dialog(false);
                }
            });
            ui.label(
                egui::RichText::new("Changes stay in memory until you use Save edits…")
                    .weak()
                    .small(),
            );
        });
    if !open {
        app.close_edit_header_dialog(false);
    }
}

/// Modal dialog: edit the current record sequence.
fn edit_seq_dialog(app: &mut SeqFlashApp, ctx: &egui::Context) {
    let mut open = true;
    egui::Window::new("Edit sequence")
        .open(&mut open)
        .resizable(true)
        .collapsible(false)
        .default_width(480.0)
        .default_height(280.0)
        .show(ctx, |ui| {
            ui.label("Sequence bases (whitespace ignored on apply):");
            egui::ScrollArea::vertical()
                .max_height(200.0)
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(app.edit_seq_input_mut())
                            .desired_width(f32::INFINITY)
                            .desired_rows(10)
                            .code_editor(),
                    );
                });
            ui.horizontal(|ui| {
                if ui.button("Apply").clicked() {
                    app.close_edit_seq_dialog(true);
                }
                if ui.button("Cancel").clicked() {
                    app.close_edit_seq_dialog(false);
                }
            });
            ui.label(
                egui::RichText::new(
                    "For FASTQ, sequence and quality lengths must match after edit.",
                )
                .weak()
                .small(),
            );
        });
    if !open {
        app.close_edit_seq_dialog(false);
    }
}

/// Modal dialog: edit the current FASTQ quality string.
fn edit_qual_dialog(app: &mut SeqFlashApp, ctx: &egui::Context) {
    let mut open = true;
    egui::Window::new("Edit quality")
        .open(&mut open)
        .resizable(true)
        .collapsible(false)
        .default_width(480.0)
        .default_height(280.0)
        .show(ctx, |ui| {
            ui.label("Quality string (Phred+33 ASCII; whitespace ignored):");
            egui::ScrollArea::vertical()
                .max_height(200.0)
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(app.edit_qual_input_mut())
                            .desired_width(f32::INFINITY)
                            .desired_rows(10)
                            .code_editor(),
                    );
                });
            ui.horizontal(|ui| {
                if ui.button("Apply").clicked() {
                    app.close_edit_qual_dialog(true);
                }
                if ui.button("Cancel").clicked() {
                    app.close_edit_qual_dialog(false);
                }
            });
        });
    if !open {
        app.close_edit_qual_dialog(false);
    }
}

/// Bottom status bar: path, size, format, record count, offset, dirty flag.
fn status_bar(app: &mut SeqFlashApp, ui: &mut egui::Ui) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 18.0;
        let count = app.document_count();
        let dirty = app.active_is_dirty();
        let edit_count = app.active_edit_count();
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
                } else if let Some(idx) = app.active_fastq_index() {
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
                        ui.label(format!("Indexing {pct}% ({rec} records)"));
                    }
                }
                ui.label(format!(
                    "offset {} / {}",
                    app.active_top_offset(),
                    meta.size
                ));
                if dirty {
                    ui.label(
                        egui::RichText::new(format!("● modified ({edit_count})"))
                            .color(egui::Color32::from_rgb(220, 160, 40)),
                    );
                }
                if let Some((done, total)) = app.save_progress() {
                    let pct = done
                        .checked_mul(100)
                        .and_then(|n| n.checked_div(total))
                        .and_then(|n| u8::try_from(n).ok())
                        .unwrap_or(0);
                    ui.label(
                        egui::RichText::new(format!("Saving {pct}% ({done}/{total})"))
                            .color(egui::Color32::from_rgb(80, 160, 220)),
                    );
                }
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
