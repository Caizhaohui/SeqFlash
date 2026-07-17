//! Streaming save with overlay applied (plan section 20.2).
//!
//! Iterates original records from the index, queries the overlay for edits,
//! and writes the result to a temp file → atomic rename. Supports progress
//! reporting and cooperative cancellation (plan 20.3).

use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

use crate::export::ExportError;
use crate::overlay::{EditOverlay, RecordEdit};

/// Save a FASTA file with overlay edits applied.
///
/// Iterates `entries` in order, querying `overlay` for each record's edits.
/// Deleted records are skipped; replaced records use the new data; insertions
/// are written at the appropriate position. Writes to a temp file then renames.
///
/// # Errors
///
/// Returns [`ExportError`] on temp-file creation, write, or rename failure.
pub fn save_fasta_with_overlay(
    bytes: &[u8],
    entries: &[FastaOverlayEntry],
    overlay: &EditOverlay,
    path: &Path,
) -> Result<(), ExportError> {
    save_fasta_with_overlay_ex(bytes, entries, overlay, path, || false, |_, _| {})
}

/// FASTA overlay save with cooperative cancel + progress (records done / total).
///
/// `should_cancel` is polled once per source record. On cancel the temp file is
/// deleted and [`ExportError::Cancelled`] is returned. `on_progress(done, total)`
/// is invoked after each record is processed (including deleted ones).
///
/// # Errors
///
/// See [`save_fasta_with_overlay`]; also [`ExportError::Cancelled`].
pub fn save_fasta_with_overlay_ex(
    bytes: &[u8],
    entries: &[FastaOverlayEntry],
    overlay: &EditOverlay,
    path: &Path,
    should_cancel: impl FnMut() -> bool,
    on_progress: impl FnMut(u64, u64),
) -> Result<(), ExportError> {
    save_with_overlay_ex(bytes, entries, overlay, path, should_cancel, on_progress)
}

/// Save a FASTQ file with overlay edits applied.
///
/// # Errors
///
/// Returns [`ExportError`] on temp-file creation, write, or rename failure.
pub fn save_fastq_with_overlay(
    bytes: &[u8],
    entries: &[FastqOverlayEntry],
    overlay: &EditOverlay,
    path: &Path,
) -> Result<(), ExportError> {
    save_fastq_with_overlay_ex(bytes, entries, overlay, path, || false, |_, _| {})
}

/// FASTQ overlay save with cooperative cancel + progress.
///
/// # Errors
///
/// See [`save_fastq_with_overlay`]; also [`ExportError::Cancelled`].
pub fn save_fastq_with_overlay_ex(
    bytes: &[u8],
    entries: &[FastqOverlayEntry],
    overlay: &EditOverlay,
    path: &Path,
    should_cancel: impl FnMut() -> bool,
    on_progress: impl FnMut(u64, u64),
) -> Result<(), ExportError> {
    // Same layout as FASTA entries for the shared writer.
    let mapped: Vec<FastaOverlayEntry> = entries
        .iter()
        .map(|e| FastaOverlayEntry {
            record_number: e.record_number,
            start_offset: e.start_offset,
            end_offset: e.end_offset,
        })
        .collect();
    save_with_overlay_ex(bytes, &mapped, overlay, path, should_cancel, on_progress)
}

/// A FASTA record's byte ranges for overlay-aware export.
pub struct FastaOverlayEntry {
    pub record_number: u64,
    pub start_offset: u64,
    pub end_offset: u64,
}

/// A FASTQ record's byte ranges for overlay-aware export.
pub struct FastqOverlayEntry {
    pub record_number: u64,
    pub start_offset: u64,
    pub end_offset: u64,
}

fn save_with_overlay_ex(
    bytes: &[u8],
    entries: &[FastaOverlayEntry],
    overlay: &EditOverlay,
    path: &Path,
    mut should_cancel: impl FnMut() -> bool,
    mut on_progress: impl FnMut(u64, u64),
) -> Result<(), ExportError> {
    let tmp = temp_path_for(path);
    let mut file = File::create(&tmp).map_err(ExportError::TempCreate)?;
    let result = write_fasta_overlay(
        &mut file,
        bytes,
        entries,
        overlay,
        &mut should_cancel,
        &mut on_progress,
    );
    if let Err(e) = result {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    // Final cancel check before rename so we never publish a partial file.
    if should_cancel() {
        let _ = fs::remove_file(&tmp);
        return Err(ExportError::Cancelled);
    }
    file.sync_all().map_err(ExportError::Write)?;
    drop(file);
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        ExportError::Rename(e)
    })
}

fn write_fasta_overlay(
    file: &mut File,
    bytes: &[u8],
    entries: &[FastaOverlayEntry],
    overlay: &EditOverlay,
    should_cancel: &mut dyn FnMut() -> bool,
    on_progress: &mut dyn FnMut(u64, u64),
) -> Result<(), ExportError> {
    let total = entries.len() as u64;
    for (i, entry) in entries.iter().enumerate() {
        if should_cancel() {
            return Err(ExportError::Cancelled);
        }

        let rn = entry.record_number;
        let start = usize::try_from(entry.start_offset)
            .unwrap_or(0)
            .min(bytes.len());
        let end = usize::try_from(entry.end_offset)
            .unwrap_or(bytes.len())
            .min(bytes.len());
        let original = &bytes[start..end];

        // Resolve stacked edits: last Replace wins; Delete skips the body;
        // InsertBefore/After are written around the resolved body.
        if let Some(edits) = overlay.edits_for(rn) {
            let mut deleted = false;
            let mut replace: Option<&[u8]> = None;
            let mut inserts_before: Vec<&[u8]> = Vec::new();
            let mut inserts_after: Vec<&[u8]> = Vec::new();
            for edit in edits {
                match edit {
                    RecordEdit::Delete { .. } => deleted = true,
                    RecordEdit::Replace { data, .. } => replace = Some(data.as_slice()),
                    RecordEdit::InsertBefore { data, .. } => inserts_before.push(data.as_slice()),
                    RecordEdit::InsertAfter { data, .. } => inserts_after.push(data.as_slice()),
                }
            }
            for data in inserts_before {
                file.write_all(data).map_err(ExportError::Write)?;
            }
            if !deleted {
                let body = replace.unwrap_or(original);
                file.write_all(body).map_err(ExportError::Write)?;
            }
            for data in inserts_after {
                file.write_all(data).map_err(ExportError::Write)?;
            }
        } else {
            // No overlay edit: write the original record bytes.
            file.write_all(original).map_err(ExportError::Write)?;
        }

        let done = (i as u64).saturating_add(1);
        on_progress(done, total);
    }
    Ok(())
}

fn temp_path_for(path: &Path) -> std::path::PathBuf {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let name = path.file_name().map_or_else(
        || "export".to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    parent.join(format!(".{name}.tmp"))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;
    use tempfile::tempdir;

    fn three_entries() -> Vec<FastaOverlayEntry> {
        vec![
            FastaOverlayEntry {
                record_number: 0,
                start_offset: 0,
                end_offset: 7,
            },
            FastaOverlayEntry {
                record_number: 1,
                start_offset: 7,
                end_offset: 14,
            },
            FastaOverlayEntry {
                record_number: 2,
                start_offset: 14,
                end_offset: 21,
            },
        ]
    }

    #[test]
    fn save_deletes_record() {
        let data = b">s1\nAC\n>s2\nGT\n>s3\nTT\n";
        let entries = three_entries();
        let mut ov = EditOverlay::new();
        ov.apply(RecordEdit::Delete { record_number: 1 });

        let dir = tempdir().unwrap();
        let path = dir.path().join("out.fa");
        save_fasta_with_overlay(data, &entries, &ov, &path).unwrap();
        let result = fs::read_to_string(&path).unwrap();
        assert!(result.contains(">s1"));
        assert!(!result.contains(">s2"));
        assert!(result.contains(">s3"));
    }

    #[test]
    fn save_replaces_record() {
        let data = b">s1\nAC\n>s2\nGT\n";
        let entries = vec![
            FastaOverlayEntry {
                record_number: 0,
                start_offset: 0,
                end_offset: 8,
            },
            FastaOverlayEntry {
                record_number: 1,
                start_offset: 8,
                end_offset: 16,
            },
        ];
        let mut ov = EditOverlay::new();
        ov.apply(RecordEdit::Replace {
            record_number: 1,
            data: b">s2_replaced\nXXXX\n".to_vec(),
        });

        let dir = tempdir().unwrap();
        let path = dir.path().join("rep.fa");
        save_fasta_with_overlay(data, &entries, &ov, &path).unwrap();
        let result = fs::read_to_string(&path).unwrap();
        assert!(result.contains(">s2_replaced"));
        assert!(result.contains("XXXX"));
    }

    #[test]
    fn save_inserts_before() {
        let data = b">s1\nAC\n";
        let entries = vec![FastaOverlayEntry {
            record_number: 0,
            start_offset: 0,
            end_offset: 8,
        }];
        let mut ov = EditOverlay::new();
        ov.apply(RecordEdit::InsertBefore {
            record_number: 0,
            data: b">s0_inserted\nTT\n".to_vec(),
        });

        let dir = tempdir().unwrap();
        let path = dir.path().join("ins.fa");
        save_fasta_with_overlay(data, &entries, &ov, &path).unwrap();
        let result = fs::read_to_string(&path).unwrap();
        assert!(result.starts_with(">s0_inserted"));
        assert!(result.contains(">s1"));
    }

    #[test]
    fn save_no_overlay_writes_original() {
        let data = b">s1\nAC\n>s2\nGT\n";
        let entries = vec![
            FastaOverlayEntry {
                record_number: 0,
                start_offset: 0,
                end_offset: 8,
            },
            FastaOverlayEntry {
                record_number: 1,
                start_offset: 8,
                end_offset: 16,
            },
        ];
        let ov = EditOverlay::new();

        let dir = tempdir().unwrap();
        let path = dir.path().join("orig.fa");
        save_fasta_with_overlay(data, &entries, &ov, &path).unwrap();
        let result = fs::read_to_string(&path).unwrap();
        assert_eq!(result, ">s1\nAC\n>s2\nGT\n");
    }

    #[test]
    fn stacked_replaces_last_wins() {
        let data = b">s1\nAC\n";
        let entries = vec![FastaOverlayEntry {
            record_number: 0,
            start_offset: 0,
            end_offset: 7,
        }];
        let mut ov = EditOverlay::new();
        ov.apply(RecordEdit::Replace {
            record_number: 0,
            data: b">s1\nXXXX\n".to_vec(),
        });
        ov.apply(RecordEdit::Replace {
            record_number: 0,
            data: b">s1\nYYYY\n".to_vec(),
        });

        let dir = tempdir().unwrap();
        let path = dir.path().join("stack.fa");
        save_fasta_with_overlay(data, &entries, &ov, &path).unwrap();
        let result = fs::read_to_string(&path).unwrap();
        assert_eq!(result, ">s1\nYYYY\n");
        assert!(!result.contains("XXXX"));
    }

    #[test]
    fn progress_reports_each_record() {
        let data = b">s1\nAC\n>s2\nGT\n>s3\nTT\n";
        let entries = three_entries();
        let ov = EditOverlay::new();
        let last = Arc::new(AtomicU64::new(0));
        let total_seen = Arc::new(AtomicU64::new(0));
        let last_c = Arc::clone(&last);
        let total_c = Arc::clone(&total_seen);

        let dir = tempdir().unwrap();
        let path = dir.path().join("prog.fa");
        save_fasta_with_overlay_ex(
            data,
            &entries,
            &ov,
            &path,
            || false,
            move |done, total| {
                last_c.store(done, Ordering::Relaxed);
                total_c.store(total, Ordering::Relaxed);
            },
        )
        .unwrap();
        assert_eq!(last.load(Ordering::Relaxed), 3);
        assert_eq!(total_seen.load(Ordering::Relaxed), 3);
    }

    #[test]
    fn cancel_cleans_temp_and_skips_target() {
        let data = b">s1\nAC\n>s2\nGT\n>s3\nTT\n";
        let entries = three_entries();
        let ov = EditOverlay::new();
        let dir = tempdir().unwrap();
        let path = dir.path().join("cancel.fa");
        let mut calls = 0u64;
        let err = save_fasta_with_overlay_ex(
            data,
            &entries,
            &ov,
            &path,
            || {
                calls += 1;
                // Cancel before writing the second record.
                calls > 1
            },
            |_, _| {},
        )
        .unwrap_err();
        assert!(matches!(err, ExportError::Cancelled));
        assert!(!path.exists());
        // Sibling temp must also be gone.
        let tmp = dir.path().join(".cancel.fa.tmp");
        assert!(!tmp.exists());
    }
}
