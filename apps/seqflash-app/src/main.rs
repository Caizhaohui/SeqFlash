//! SeqFlash application entry point.
//!
//! Responsibilities at this layer (M0):
//! 1. Compute and create the per-user log directory.
//! 2. Install a `tracing` subscriber that writes rolling JSON-ish logs to that
//!    directory, plus the console in debug builds.
//! 3. Load `AppSettings` (falling back to defaults on any failure — first
//!    launch has no settings file yet).
//! 4. Open the `eframe` window.
//!
//! No FASTA/FASTQ functionality lives here yet; that arrives in later
//! milestones. See `DEVELOPMENT_PLAN.md` sections 9, 22, 24.

#![forbid(unsafe_code)]
// Release builds run as a pure GUI app (no console window). Debug builds keep
// the console so tracing output to stderr is visible during development.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;

use std::path::PathBuf;

use anyhow::{Context, Result};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::EnvFilter;

use seqflash_settings::AppSettings;

/// The friendly application name used for the window title and data directory.
const APP_NAME: &str = "SeqFlash";

fn main() -> Result<()> {
    // Keep the guard alive for the whole process; dropping it flushes/closes
    // the non-blocking writer. Bind to `_guard` *before* installing the
    // subscriber so the writer is in place when the first log line fires.
    let log_dir = log_dir()?;
    let _guard = install_tracing(&log_dir)?;
    install_panic_hook(log_dir.clone());

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        ?log_dir,
        "SeqFlash starting (M8)"
    );

    let settings = load_settings();
    tracing::info!(?settings.theme, "settings loaded");

    // Optional: a file path passed as the first CLI argument is opened on
    // startup (basis for double-click / file-association launching).
    let initial_file = std::env::args_os().nth(1).map(std::path::PathBuf::from);

    run_window(settings, initial_file)
}

/// Resolve the per-user log directory and make sure it exists.
///
/// Uses the OS convention for "local app data" on Windows, e.g.
/// `C:\Users\<user>\AppData\Local\SeqFlash\logs`.
fn log_dir() -> Result<PathBuf> {
    let project_dirs = directories::ProjectDirs::from("", "", APP_NAME)
        .context("could not resolve a per-user application data directory")?;
    let dir = project_dirs.data_local_dir().join("logs");
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("failed to create log directory: {}", dir.display()))?;
    Ok(dir)
}

/// Install a panic hook that logs to the rolling file logger and stderr (M8).
///
/// The default Rust hook only prints to stderr, which is invisible in release
/// GUI builds (`windows_subsystem = "windows"`). We also write a one-shot
/// `crash-*.log` next to the daily logs for support.
fn install_panic_hook(log_dir: PathBuf) {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map_or_else(|| "unknown".to_string(), |l| format!("{l}"));
        let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "non-string panic payload".to_string()
        };
        let backtrace = std::backtrace::Backtrace::force_capture();
        let message = format!("SeqFlash panic at {location}\n{payload}\n\nBacktrace:\n{backtrace}");
        tracing::error!(%location, %payload, "panic");
        // Best-effort crash file for release GUI (stderr may be discarded).
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let crash_path = log_dir.join(format!("crash-{stamp}.log"));
        let _ = std::fs::write(&crash_path, &message);
        // Still invoke the default hook (prints to stderr in debug).
        default_hook(info);
    }));
}

/// Install the global tracing subscriber.
///
/// Returns a [`WorkerGuard`] that must be held for the lifetime of the program;
/// dropping it flushes the async log writer.
fn install_tracing(log_dir: &std::path::Path) -> Result<WorkerGuard> {
    // Rolling daily file appender: `seqflash.log.YYYY-MM-DD`.
    let file_appender = tracing_appender::rolling::daily(log_dir, "seqflash.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,eframe=warn,egui=warn,wgpu=warn"));

    // Rolling-file layer: always on, never with ANSI escapes.
    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false);

    // Console layer: always writes to stderr. In debug builds this shows logs
    // in the attached console; in release builds (`windows_subsystem =
    // "windows"`) there is no console and stderr is silently discarded by the
    // OS — both cases are safe, and using a single writer type keeps the
    // layered subscriber monomorphized consistently.
    let console_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_ansi(cfg!(debug_assertions));

    let subscriber = tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .with(console_layer);

    // set_global_default returns Err if a subscriber is already installed
    // (e.g. by a test harness). That is harmless here.
    tracing::subscriber::set_global_default(subscriber)
        .context("failed to install tracing subscriber")?;

    Ok(guard)
}

/// Load settings from the conventional location, falling back to defaults.
///
/// Settings failure must never prevent the app from starting.
fn load_settings() -> AppSettings {
    let Some(project_dirs) = directories::ProjectDirs::from("", "", APP_NAME) else {
        tracing::warn!("could not resolve app data dir; using default settings");
        return AppSettings::default();
    };
    let path = project_dirs.data_local_dir().join(AppSettings::FILE_NAME);
    match AppSettings::load_from_path(&path) {
        Ok(s) => s,
        Err(err) => {
            tracing::warn!(%err, path = %path.display(), "settings load failed; using defaults");
            AppSettings::default()
        }
    }
}

/// Create and run the `eframe` window.
fn run_window(settings: AppSettings, initial_file: Option<PathBuf>) -> Result<()> {
    let viewport = egui::ViewportBuilder::default()
        .with_inner_size(egui::vec2(960.0, 600.0))
        .with_min_inner_size(egui::vec2(640.0, 400.0))
        .with_active(true)
        // Enable native drag-and-drop on Windows so dropped files reach egui's
        // `raw.dropped_files`. (No-op on other platforms.)
        .with_drag_and_drop(true);
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        APP_NAME,
        options,
        Box::new(move |cc| {
            // Apply the configured theme to the egui context.
            apply_theme(cc, &settings);
            Ok(Box::new(app::SeqFlashApp::new(settings, initial_file)))
        }),
    )
    .map_err(|err| anyhow::anyhow!("eframe window loop exited with an error: {err}"))?;

    tracing::info!("SeqFlash exited cleanly");
    Ok(())
}

/// Map `Theme` onto the egui `Visuals` for the given context.
fn apply_theme(cc: &eframe::CreationContext<'_>, settings: &AppSettings) {
    use seqflash_settings::Theme;
    match settings.theme {
        Theme::Dark => cc.egui_ctx.set_visuals(egui::Visuals::dark()),
        Theme::Light => cc.egui_ctx.set_visuals(egui::Visuals::light()),
        Theme::System => {
            // eframe already follows the system preference by default.
        }
    }
}
