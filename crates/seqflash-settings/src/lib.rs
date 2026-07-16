//! User settings for SeqFlash and their on-disk persistence.
//!
//! This crate owns the *shape* of the settings and the (de)serialization, but
//! it does **not** decide *where* settings live on disk — that is the job of
//! the application / platform layer. Here you only get `load_from_path` and
//! `save_to_path`, both of which take an explicit path. See
//! `DEVELOPMENT_PLAN.md` section 9.9 / 23.
//!
//! The on-disk format is JSON for now (human-readable, easy to diff). The
//! schema is allowed to evolve; unknown fields are ignored on load so old
//! config files do not break newer builds.

#![forbid(unsafe_code)]

use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};

pub use error::SettingsError;

/// Errors that can occur while loading or saving settings.
mod error {
    use std::io;

    #[derive(Debug, thiserror::Error)]
    pub enum SettingsError {
        #[error("failed to read settings file")]
        Read(#[source] io::Error),
        #[error("failed to parse settings file")]
        Parse(#[from] serde_json::Error),
        #[error("failed to write settings file")]
        Write(#[from] io::Error),
    }
}

/// Visual theme preference.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    /// Always use the light theme.
    Light,
    /// Always use the dark theme.
    Dark,
    /// Follow the operating system preference.
    #[default]
    System,
}

/// Defaults for sequence display and caching.
///
/// Values are deliberately conservative; the viewer should still clamp runtime
/// inputs against safe bounds rather than trusting these blindly.
pub const DEFAULT_WRAP_WIDTH: usize = 80;
pub const DEFAULT_VIEWER_CACHE_LINES: usize = 400;
/// ~64 MiB cache budget for the visible viewport.
pub const DEFAULT_VIEWER_CACHE_BYTES: usize = 64 * 1024 * 1024;
pub const DEFAULT_MAX_SEARCH_RESULTS: usize = 10_000;
/// Single-record edit threshold (plan section 18.3).
pub const DEFAULT_RECORD_EDIT_LIMIT_BYTES: u64 = 64 * 1024 * 1024;

/// The complete, persisted user configuration.
///
/// Mirrors `DEVELOPMENT_PLAN.md` section 23. Add new fields with
/// `#[serde(default)]` so older config files keep loading.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppSettings {
    pub theme: Theme,
    pub font_family: String,
    pub font_size: f32,
    pub sequence_wrap_width: usize,
    pub viewer_cache_lines: usize,
    pub viewer_cache_bytes: usize,
    pub max_search_results: usize,
    pub worker_threads: usize,
    pub record_edit_limit_bytes: u64,
    #[serde(default)]
    pub default_export_directory: Option<std::path::PathBuf>,
    #[serde(default)]
    pub reopen_previous_session: bool,
}

impl AppSettings {
    /// The conventional on-disk filename (without directory).
    pub const FILE_NAME: &'static str = "seqflash-settings.json";

    /// Load settings from `path`.
    ///
    /// A missing file is treated as "use defaults" rather than an error: the
    /// first launch of the application has no settings file yet.
    ///
    /// # Errors
    ///
    /// Returns [`SettingsError::Read`] if the file exists but cannot be read,
    /// or [`SettingsError::Parse`] if the file is not valid JSON for this
    /// schema.
    pub fn load_from_path(path: &Path) -> Result<Self, SettingsError> {
        match std::fs::read(path) {
            Ok(bytes) => {
                let settings: Self = serde_json::from_slice(&bytes)?;
                Ok(settings)
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(err) => Err(SettingsError::Read(err)),
        }
    }

    /// Write settings to `path` via a sibling temp file plus an atomic rename,
    /// so a crash mid-write cannot leave a truncated settings file.
    ///
    /// # Errors
    ///
    /// Returns [`SettingsError::Write`] for any I/O failure (creating the temp
    /// file, writing, syncing, or renaming), or [`SettingsError::Parse`] if the
    /// in-memory settings cannot be serialized to JSON.
    pub fn save_to_path(&self, path: &Path) -> Result<(), SettingsError> {
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));

        let mut tmp_path = parent.to_path_buf();
        tmp_path.push(format!(
            ".{}.tmp",
            path.file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("seqflash-settings")
        ));

        let json = serde_json::to_vec_pretty(self)?;
        {
            let mut file = std::fs::File::create(&tmp_path)?;
            file.write_all(&json)?;
            file.sync_all()?;
        }
        // rename is atomic on the same filesystem; fall back to copy+remove.
        if std::fs::rename(&tmp_path, path).is_err() {
            std::fs::copy(&tmp_path, path)?;
            let _ = std::fs::remove_file(&tmp_path);
        }
        Ok(())
    }
}

impl Default for AppSettings {
    fn default() -> Self {
        // Fall back to a single worker if the platform refuses to report
        // parallelism (e.g. some sandboxed environments). Never panic.
        let worker_threads =
            std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get);

        Self {
            theme: Theme::default(),
            font_family: "monospace".to_string(),
            font_size: 14.0,
            sequence_wrap_width: DEFAULT_WRAP_WIDTH,
            viewer_cache_lines: DEFAULT_VIEWER_CACHE_LINES,
            viewer_cache_bytes: DEFAULT_VIEWER_CACHE_BYTES,
            max_search_results: DEFAULT_MAX_SEARCH_RESULTS,
            worker_threads,
            record_edit_limit_bytes: DEFAULT_RECORD_EDIT_LIMIT_BYTES,
            default_export_directory: None,
            reopen_previous_session: false,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn defaults_are_sane() {
        let s = AppSettings::default();
        assert_eq!(s.theme, Theme::System);
        assert!(!s.font_family.is_empty());
        assert!(s.font_size > 0.0);
        assert_eq!(s.sequence_wrap_width, DEFAULT_WRAP_WIDTH);
        assert_eq!(s.max_search_results, DEFAULT_MAX_SEARCH_RESULTS);
        assert_eq!(s.record_edit_limit_bytes, DEFAULT_RECORD_EDIT_LIMIT_BYTES);
        assert!(s.worker_threads >= 1);
        assert!(s.default_export_directory.is_none());
        assert!(!s.reopen_previous_session);
    }

    #[test]
    fn missing_file_yields_defaults() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("does-not-exist.json");
        let s = AppSettings::load_from_path(&path).expect("missing file -> defaults");
        assert_eq!(s.theme, Theme::System);
    }

    #[test]
    fn roundtrip_preserves_values() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join(AppSettings::FILE_NAME);

        let original = AppSettings {
            font_size: 18.5,
            sequence_wrap_width: 60,
            theme: Theme::Dark,
            default_export_directory: Some(PathBuf::from("D:/seqflash/exports")),
            reopen_previous_session: true,
            ..AppSettings::default()
        };

        original.save_to_path(&path).expect("save");
        let loaded = AppSettings::load_from_path(&path).expect("load");

        // A value that round-trips through JSON bit-for-bit is exactly equal.
        #[allow(clippy::float_cmp)]
        {
            assert_eq!(loaded.font_size, 18.5);
        }
        assert_eq!(loaded.sequence_wrap_width, 60);
        assert_eq!(loaded.theme, Theme::Dark);
        assert_eq!(
            loaded.default_export_directory.as_deref(),
            Some(std::path::Path::new("D:/seqflash/exports"))
        );
        assert!(loaded.reopen_previous_session);
    }

    #[test]
    fn json_uses_lowercase_theme() {
        // Guard the serde rename against accidental regression — the status bar
        // and config docs treat themes as lowercase.
        let json = serde_json::to_string(&AppSettings::default()).expect("serialize");
        assert!(json.contains("\"theme\":\"system\""), "got: {json}");
    }

    #[test]
    fn unknown_fields_are_ignored() {
        // Forward-compatibility: a future field must not break this build.
        let json = r#"{
            "theme": "dark",
            "font_family": "monospace",
            "font_size": 12.0,
            "sequence_wrap_width": 80,
            "viewer_cache_lines": 400,
            "viewer_cache_bytes": 67108864,
            "max_search_results": 10000,
            "worker_threads": 4,
            "record_edit_limit_bytes": 67108864,
            "future_unknown_field": true
        }"#;
        let s: AppSettings = serde_json::from_str(json).expect("forward-compatible");
        assert_eq!(s.theme, Theme::Dark);
        #[allow(clippy::float_cmp)]
        {
            assert_eq!(s.font_size, 12.0);
        }
    }

    #[test]
    fn default_export_directory_is_optional() {
        // A config that simply omits the optional directory must load.
        let json = r#"{
            "theme": "system",
            "font_family": "monospace",
            "font_size": 14.0,
            "sequence_wrap_width": 80,
            "viewer_cache_lines": 400,
            "viewer_cache_bytes": 67108864,
            "max_search_results": 10000,
            "worker_threads": 2,
            "record_edit_limit_bytes": 67108864
        }"#;
        let s: AppSettings = serde_json::from_str(json).expect("optional dir");
        assert!(s.default_export_directory.is_none());
        assert!(!s.reopen_previous_session);
    }
}
