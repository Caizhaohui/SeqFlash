//! The `eframe::App` for the SeqFlash main window.
//!
//! M0 renders a minimal placeholder UI: a title, a short status line, and the
//! current milestone. Real file opening, the virtual viewer, panels, and the
//! status bar arrive in later milestones (see `DEVELOPMENT_PLAN.md` section 22).

use eframe::egui;

use seqflash_settings::{AppSettings, Theme};

/// Top-level SeqFlash egui application.
pub(crate) struct SeqFlashApp {
    settings: AppSettings,
}

impl SeqFlashApp {
    /// Construct the application from the already-loaded settings.
    pub(crate) fn new(settings: AppSettings) -> Self {
        Self { settings }
    }

    /// Human-readable label for the active theme.
    fn theme_label(&self) -> &'static str {
        match self.settings.theme {
            Theme::Light => "Light",
            Theme::Dark => "Dark",
            Theme::System => "System",
        }
    }
}

impl eframe::App for SeqFlashApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(12.0);
            ui.heading("SeqFlash");
            ui.label(egui::RichText::new("A FASTA/FASTQ browser for large sequence files.").weak());
            ui.add_space(18.0);

            ui.label(
                egui::RichText::new("Milestone M0 — workspace initialized")
                    .color(egui::Color32::from_rgb(0x4E, 0x9A, 0x06)),
            );
            ui.label(
                egui::RichText::new(
                    "FASTA/FASTQ features are not implemented yet (planned for later milestones).",
                )
                .weak(),
            );

            ui.add_space(24.0);
            ui.separator();
            ui.add_space(8.0);

            ui.horizontal_wrapped(|ui| {
                ui.label("Theme:");
                ui.label(self.theme_label());
            });
            ui.horizontal_wrapped(|ui| {
                ui.label("Wrap width:");
                ui.label(format!("{}", self.settings.sequence_wrap_width));
            });
            ui.horizontal_wrapped(|ui| {
                ui.label("Worker threads:");
                ui.label(format!("{}", self.settings.worker_threads));
            });

            ui.add_space(24.0);
            ui.label(
                egui::RichText::new("File > Open is coming in M1.")
                    .italics()
                    .weak(),
            );
        });
    }
}
