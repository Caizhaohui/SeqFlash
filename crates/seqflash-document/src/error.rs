//! Errors raised while opening or inspecting documents.

use std::io;
use std::time::SystemTimeError;

/// Errors that can occur while opening a document or checking its state.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum DocumentError {
    /// The file could not be opened (missing, in use, permission denied, ...).
    #[error("failed to open file: {0}")]
    Open(#[source] io::Error),

    /// File metadata (size / modification time) could not be read.
    #[error("failed to read file metadata: {0}")]
    Metadata(#[source] io::Error),

    /// The memory map could not be created.
    #[error("failed to memory-map the file: {0}")]
    Mmap(#[source] io::Error),

    /// A modification time could not be compared (clock moved backwards, ...).
    #[error("failed to compare modification time: {0}")]
    Time(#[from] SystemTimeError),
}
