//! The Settings panel — CORE §4.5 and P2 §3.
//!
//! "Exactly three groups, nothing else, and no room grows later without a CORE edit:
//! Extract, Bookmarks, Recent files."
//!
//! CORE §9 has already decided the absent ones: no theme controls, no language
//! controls, no anything-else.

use eframe::egui;

use super::{Indium, Popup};
use crate::platform::store::{Bookmark, ExtractDefault};
use crate::theme;

pub fn show(app: &mut Indium, ctx: &egui::Context) {
    if app.popup != Some(Popup::Settings) {
        return;
    }
    let mut open = true;

    egui::Window::new("Settings")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .open(&mut open)
        .show(ctx, |ui| {
            ui.set_min_width(500.0);

            if app.settings_broken {
                ui.label(
                    egui::RichText::new(
                        "settings.toml could not be parsed. INDIUM is running on defaults and \
                         will not overwrite the file.",
                    )
                    .size(13.0)
                    .color(theme::WARNING),
                );
            }

            // --- 1. Extract ---------------------------------------------------
            // No `add_space` before any of the three headings any more: `theme::section`
            // carries `SECTION_ABOVE` itself, so the gap is declared once in `theme.rs`
            // rather than hand-tuned three times here. P7 §1.
            theme::section(ui, "Extract");
            let mut changed = false;
            ui.horizontal(|ui| {
                // Which default is chosen is "this mode is active", not "something will
                // happen". The ink carries it too, because Aubergine alone sits 1.72:1
                // against the panel. P6 §6.6.
                theme::active_fill(ui);
                ui.label(
                    egui::RichText::new("Preselect")
                        .size(13.0)
                        .color(theme::TEXT_MUTED),
                );
                let cur = app.settings.extract.default;
                let toggle = |ui: &mut egui::Ui, on: bool, text: &str| {
                    let text = egui::RichText::new(text).color(if on {
                        theme::TEXT
                    } else {
                        theme::TEXT_MUTED
                    });
                    ui.selectable_label(on, text).clicked()
                };
                if toggle(ui, cur == ExtractDefault::Here, "here") && cur != ExtractDefault::Here {
                    app.settings.extract.default = ExtractDefault::Here;
                    changed = true;
                }
                if toggle(ui, cur == ExtractDefault::Subdir, "into a subdirectory")
                    && cur != ExtractDefault::Subdir
                {
                    app.settings.extract.default = ExtractDefault::Subdir;
                    changed = true;
                }
            });
            if changed {
                app.extract_to_subdir = app.settings.extract.default == ExtractDefault::Subdir;
                app.save_settings();
            }

            // --- 2. Bookmarks -------------------------------------------------
            theme::section(ui, "Bookmarks");

            let mut remove: Option<usize> = None;
            for (i, b) in app.settings.bookmarks.clone().iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(&b.name)
                            .family(theme::MONO)
                            .size(13.0)
                            .color(theme::TEXT),
                    );
                    ui.label(
                        egui::RichText::new(&b.path)
                            .family(theme::MONO)
                            .size(13.0)
                            .color(theme::TEXT_MUTED),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if theme::small_button(ui, egui::RichText::new("×"), true).clicked() {
                            remove = Some(i);
                        }
                    });
                });
            }
            if let Some(i) = remove {
                app.settings.bookmarks.remove(i);
                app.save_settings();
            }

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut app.bookmark_name)
                        .hint_text("name")
                        .desired_width(130.0),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut app.bookmark_path)
                        .font(egui::TextStyle::Monospace)
                        .hint_text("/path/to/directory")
                        .desired_width(250.0),
                );
                let ready =
                    !app.bookmark_name.trim().is_empty() && !app.bookmark_path.trim().is_empty();
                if theme::button(ui, egui::RichText::new("Add"), ready).clicked() {
                    app.settings.bookmarks.push(Bookmark {
                        name: app.bookmark_name.trim().to_string(),
                        path: app.bookmark_path.trim().to_string(),
                    });
                    app.bookmark_name.clear();
                    app.bookmark_path.clear();
                    app.save_settings();
                }
            });

            // --- 3. Recent files ----------------------------------------------
            theme::section(ui, "Recent files");
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("{} remembered", app.recents.items.len()))
                        .size(13.0)
                        .color(theme::TEXT_MUTED),
                );
                if theme::button(ui, egui::RichText::new("Clear list"), true).clicked() {
                    app.recents.items.clear();
                    // Status first, save last, so a refusal or a write error is what the
                    // status bar carries rather than a cheerful line about a file that is
                    // still full of what it always held.
                    app.status = "Recent files cleared.".to_string();
                    app.save_recents();
                }
            });

            ui.add_space(12.0);
            ui.separator();
            ui.label(
                egui::RichText::new(format!(
                    "settings.toml · {}",
                    app.store.settings_path().display()
                ))
                .size(12.0)
                .color(theme::TEXT_MUTED),
            );
            ui.label(
                egui::RichText::new("Hand-editable. INDIUM respects what you write there.")
                    .size(12.0)
                    .color(theme::TEXT_MUTED),
            );
        });

    if !open {
        app.popup = None;
    }
}
