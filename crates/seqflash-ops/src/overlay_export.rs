//! Streaming save with overlay applied (plan section 20.2).
//!
//! Iterates original records from the index, queries the overlay for edits,
//! and writes the result to a temp file → atomic rename.

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
    entries: &[crate::FastaOverlayEntry],
    overlay: &EditOverlay,
    path: &Path,
) -> Result<(), ExportError> {
    let tmp = temp_path_for(path);
    let mut file = File::create(&tmp).map_err(ExportError::TempCreate)?;
    let result = write_fasta_overlay(&mut file, bytes, entries, overlay);
    if let Err(e) = result {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    file.sync_all().map_err(ExportError::Write)?;
    drop(file);
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        ExportError::Rename(e)
    })
}

/// Save a FASTQ file with overlay edits applied.
///
/// # Errors
///
/// Returns [`ExportError`] on temp-file creation, write, or rename failure.
pub fn save_fastq_with_overlay(
    bytes: &[u8],
    entries: &[crate::FastqOverlayEntry],
    overlay: &EditOverlay,
    path: &Path,
) -> Result<(), ExportError> {
    let tmp = temp_path_for(path);
    let mut file = File::create(&tmp).map_err(ExportError::TempCreate)?;
    let result = write_fastq_overlay(&mut file, bytes, entries, overlay);
    if let Err(e) = result {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    file.sync_all().map_err(ExportError::Write)?;
    drop(file);
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        ExportError::Rename(e)
    })
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

fn write_fasta_overlay(
    file: &mut File,
    bytes: &[u8],
    entries: &[FastaOverlayEntry],
    overlay: &EditOverlay,
) -> Result<(), ExportError> {
    for entry in entries {
        let rn = entry.record_number;
        let start = usize::try_from(entry.start_offset)
            .unwrap_or(0)
            .min(bytes.len());
        let end = usize::try_from(entry.end_offset)
            .unwrap_or(bytes.len())
            .min(bytes.len());

        // Check overlay edits for this record.
        if let Some(edits) = overlay.edits_for(rn) {
            let mut deleted = false;
            for edit in edits {
                match edit {
                    RecordEdit::Delete { .. } => {
                        deleted = true;
                    }
                    RecordEdit::Replace { data, .. } => {
                        file.write_all(data).map_err(ExportError::Write)?;
                    }
                    RecordEdit::InsertBefore { data, .. } => {
                        file.write_all(data).map_err(ExportError::Write)?;
                        // Also write the original record after insertion.
                        file.write_all(&bytes[start..end])
                            .map_err(ExportError::Write)?;
                    }
                    RecordEdit::InsertAfter { data, .. } => {
                        // Write original first.
                        file.write_all(&bytes[start..end])
                            .map_err(ExportError::Write)?;
                        file.write_all(data).map_err(ExportError::Write)?;
                    }
                }
            }
            // If the record was deleted, skip writing the original.
            if deleted {
                continue;
            }
            // Otherwise the record had only Replace/Insert edits which already
            // wrote their data (and possibly the original too). Continue.
            continue;
        }

        // No overlay edit: write the original record bytes.
        file.write_all(&bytes[start..end])
            .map_err(ExportError::Write)?;
    }
    Ok(())
}

fn write_fastq_overlay(
    file: &mut File,
    bytes: &[u8],
    entries: &[FastqOverlayEntry],
    overlay: &EditOverlay,
) -> Result<(), ExportError> {
    // Same logic as FASTA — write raw record bytes or overlay data.
    write_fasta_overlay(
        file,
        bytes,
        // Reinterpret FASTQ entries as FASTA-shaped (same fields).
        &entries
            .iter()
            .map(|e| FastaOverlayEntry {
                record_number: e.record_number,
                start_offset: e.start_offset,
                end_offset: e.end_offset,
            })
            .collect::<Vec<_>>(),
        overlay,
    )
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
    use tempfile::tempdir;

    #[test]
    fn save_deletes_record() {
        let data = b">s1\nAC\n>s2\nGT\n>s3\nTT\n";
        let entries = vec![
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
        ];
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
}
