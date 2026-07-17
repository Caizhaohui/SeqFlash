//! Streaming export pipeline (plan section 20).
//!
//! Records are written one at a time to a temporary file, then the file is
//! atomically renamed to the target path. The source file is never modified.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;

use seqflash_types::ByteRange;

use crate::transform::{
    reverse_complement, reverse_quality, to_lowercase, to_uppercase, wrap_sequence,
};

/// Optional transformation applied to each exported record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transform {
    None,
    ReverseComplement,
    Uppercase,
    Lowercase,
    /// Wrap sequence lines at the given width (FASTA only).
    Wrap(usize),
    /// Convert FASTQ to FASTA.
    FastqToFasta,
}

/// Errors that can occur during export.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ExportError {
    #[error("failed to create temporary file: {0}")]
    TempCreate(io::Error),
    #[error("write error: {0}")]
    Write(io::Error),
    #[error("failed to rename temporary file to target: {0}")]
    Rename(io::Error),
}

/// One FASTA record to export (sliced from the source file's index).
pub struct FastaExportRecord<'a> {
    pub header: &'a [u8],
    pub sequence: &'a [u8],
}

/// One FASTQ record to export.
pub struct FastqExportRecord<'a> {
    pub header: &'a [u8],
    pub sequence: &'a [u8],
    pub quality: &'a [u8],
}

/// Export selected FASTA records to `path`, applying an optional transform.
///
/// Writes records one at a time to a sibling temp file, then atomically renames.
/// The source file is never modified. On any error, the temp file is deleted.
///
/// # Errors
///
/// Returns [`ExportError::TempCreate`] if the temp file cannot be created,
/// [`ExportError::Write`] for any write/sync failure, or
/// [`ExportError::Rename`] if the final atomic rename fails.
pub fn export_fasta_records(
    records: &[FastaExportRecord<'_>],
    path: &Path,
    transform: Transform,
) -> Result<(), ExportError> {
    let tmp = temp_path_for(path);
    let mut file = File::create(&tmp).map_err(ExportError::TempCreate)?;
    let write_result = write_fasta_records(&mut file, records, transform);
    if let Err(e) = write_result {
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

/// Export selected FASTQ records to `path`.
///
/// # Errors
///
/// See [`export_fasta_records`].
pub fn export_fastq_records(
    records: &[FastqExportRecord<'_>],
    path: &Path,
    transform: Transform,
) -> Result<(), ExportError> {
    let tmp = temp_path_for(path);
    let mut file = File::create(&tmp).map_err(ExportError::TempCreate)?;
    let write_result = write_fastq_records(&mut file, records, transform);
    if let Err(e) = write_result {
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

fn write_fasta_records(
    file: &mut File,
    records: &[FastaExportRecord<'_>],
    transform: Transform,
) -> Result<(), ExportError> {
    for rec in records {
        file.write_all(b">").map_err(ExportError::Write)?;
        file.write_all(rec.header).map_err(ExportError::Write)?;
        file.write_all(b"\n").map_err(ExportError::Write)?;
        match transform {
            Transform::None | Transform::FastqToFasta => {
                file.write_all(rec.sequence).map_err(ExportError::Write)?;
            }
            Transform::ReverseComplement => {
                let rc = reverse_complement(rec.sequence);
                file.write_all(&rc).map_err(ExportError::Write)?;
            }
            Transform::Uppercase => {
                let up = to_uppercase(rec.sequence);
                file.write_all(&up).map_err(ExportError::Write)?;
            }
            Transform::Lowercase => {
                let lo = to_lowercase(rec.sequence);
                file.write_all(&lo).map_err(ExportError::Write)?;
            }
            Transform::Wrap(w) => {
                let wrapped = wrap_sequence(rec.sequence, w);
                file.write_all(&wrapped).map_err(ExportError::Write)?;
            }
        }
        if !rec.sequence.ends_with(b"\n") && !matches!(transform, Transform::Wrap(_)) {
            file.write_all(b"\n").map_err(ExportError::Write)?;
        }
    }
    Ok(())
}

fn write_fastq_records(
    file: &mut File,
    records: &[FastqExportRecord<'_>],
    transform: Transform,
) -> Result<(), ExportError> {
    for rec in records {
        if transform == Transform::FastqToFasta {
            // Convert to FASTA
            file.write_all(b">").map_err(ExportError::Write)?;
            file.write_all(rec.header).map_err(ExportError::Write)?;
            file.write_all(b"\n").map_err(ExportError::Write)?;
            file.write_all(rec.sequence).map_err(ExportError::Write)?;
            file.write_all(b"\n").map_err(ExportError::Write)?;
        } else {
            file.write_all(b"@").map_err(ExportError::Write)?;
            file.write_all(rec.header).map_err(ExportError::Write)?;
            file.write_all(b"\n").map_err(ExportError::Write)?;

            let (seq, qual) = match transform {
                Transform::ReverseComplement => (
                    reverse_complement(rec.sequence),
                    reverse_quality(rec.quality),
                ),
                Transform::Uppercase => (to_uppercase(rec.sequence), rec.quality.to_vec()),
                Transform::Lowercase => (to_lowercase(rec.sequence), rec.quality.to_vec()),
                _ => (rec.sequence.to_vec(), rec.quality.to_vec()),
            };

            file.write_all(&seq).map_err(ExportError::Write)?;
            file.write_all(b"\n+\n").map_err(ExportError::Write)?;
            file.write_all(&qual).map_err(ExportError::Write)?;
            file.write_all(b"\n").map_err(ExportError::Write)?;
        }
    }
    Ok(())
}

/// Generate a sibling temporary file path: `dir/.filename.tmp`.
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

/// Helper: convert a [`ByteRange`] to a byte slice from the source buffer.
/// Strips the leading `>` or `@` from the header line.
#[allow(dead_code)]
pub(crate) fn slice_header(bytes: &[u8], range: ByteRange) -> &[u8] {
    let start = usize::try_from(range.start).unwrap_or(0).min(bytes.len());
    let end = usize::try_from(range.end)
        .unwrap_or(bytes.len())
        .min(bytes.len());
    let slice = &bytes[start..end];
    // Strip leading '>' or '@' and trailing newline.
    let s = if slice.first() == Some(&b'>') || slice.first() == Some(&b'@') {
        &slice[1..]
    } else {
        slice
    };
    // Strip trailing \n/\r
    let trim_end = s
        .iter()
        .rposition(|&b| b != b'\n' && b != b'\r')
        .map_or(0, |p| p + 1);
    &s[..trim_end]
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn export_fasta_no_transform() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("out.fa");
        let records = vec![FastaExportRecord {
            header: b"seq1 desc",
            sequence: b"ACGT",
        }];
        export_fasta_records(&records, &path, Transform::None).unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, ">seq1 desc\nACGT\n");
    }

    #[test]
    fn export_fasta_revcomp() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("rc.fa");
        let records = vec![FastaExportRecord {
            header: b"seq1",
            sequence: b"AAAA",
        }];
        export_fasta_records(&records, &path, Transform::ReverseComplement).unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, ">seq1\nTTTT\n");
    }

    #[test]
    fn export_fasta_wrap() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("wrap.fa");
        let records = vec![FastaExportRecord {
            header: b"seq1",
            sequence: b"AAAAAAAAAA", // 10 bases
        }];
        export_fasta_records(&records, &path, Transform::Wrap(4)).unwrap();
        let content = fs::read_to_string(&path).unwrap();
        // 10/4 = AAAA\nAAAA\nAA\n
        assert_eq!(content, ">seq1\nAAAA\nAAAA\nAA\n");
    }

    #[test]
    fn export_fastq_revcomp() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("rc.fq");
        let records = vec![FastqExportRecord {
            header: b"read1",
            sequence: b"ACGTAA",
            quality: b"abcdef",
        }];
        export_fastq_records(&records, &path, Transform::ReverseComplement).unwrap();
        let content = fs::read_to_string(&path).unwrap();
        // rc(ACGTAA) = TTACGT, reversed quality = fedcba
        assert_eq!(content, "@read1\nTTACGT\n+\nfedcba\n");
    }

    #[test]
    fn export_fastq_to_fasta() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("conv.fa");
        let records = vec![FastqExportRecord {
            header: b"read1",
            sequence: b"ACGT",
            quality: b"IIII",
        }];
        export_fastq_records(&records, &path, Transform::FastqToFasta).unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, ">read1\nACGT\n");
    }

    #[test]
    fn export_cleans_up_temp_on_failure() {
        // Export to a path in a non-existent directory → should fail and clean up.
        let path = Path::new("/nonexistent_dir_xyz/output.fa");
        let records = vec![FastaExportRecord {
            header: b"x",
            sequence: b"A",
        }];
        let result = export_fasta_records(&records, path, Transform::None);
        assert!(result.is_err());
        // Temp file should not exist
        assert!(!Path::new("/nonexistent_dir_xyz/.output.fa.tmp").exists());
    }

    #[test]
    fn export_multiple_records() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("multi.fa");
        let records = vec![
            FastaExportRecord {
                header: b"s1",
                sequence: b"AC",
            },
            FastaExportRecord {
                header: b"s2",
                sequence: b"GT",
            },
        ];
        export_fasta_records(&records, &path, Transform::None).unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, ">s1\nAC\n>s2\nGT\n");
    }
}
