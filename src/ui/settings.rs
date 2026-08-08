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
                    .size(11.0)
                    .color(theme::ORANGE),
                );
                ui.add_space(8.0);
            }

            // --- 1. Extract ---------------------------------------------------
            group(ui, "Extract");
            let mut changed = false;
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Preselect")
                        .size(11.0)
                        .color(theme::TEXT_MUTED),
                );
                let cur = app.settings.extract.default;
                if ui
                    .selectable_label(cur == ExtractDefault::Here, "here")
                    .clicked()
                    && cur != ExtractDefault::Here
                {
                    app.settings.extract.default = ExtractDefault::Here;
                    changed = true;
                }
                if ui
                    .selectable_label(cur == ExtractDefault::Subdir, "into a subdirectory")
                    .clicked()
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
            ui.add_space(14.0);
            group(ui, "Bookmarks");

            let mut remove: Option<usize> = None;
            for (i, b) in app.settings.bookmarks.clone().iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(&b.name)
                            .family(theme::MONO)
                            .size(11.0)
                            .color(theme::TEXT),
                    );
                    ui.label(
                        egui::RichText::new(&b.path)
                            .family(theme::MONO)
                            .size(11.0)
                            .color(theme::TEXT_MUTED),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("×").clicked() {
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
                if ui.add_enabled(ready, egui::Button::new("Add")).clicked() {
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
            ui.add_space(14.0);
            group(ui, "Recent files");
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("{} remembered", app.recents.items.len()))
                        .size(11.0)
                        .color(theme::TEXT_MUTED),
                );
                if ui.button("Clear list").clicked() {
                    app.recents.items.clear();
                    if !app.recents_broken {
                        let _ = app.store.save_recents(&app.recents);
                    }
                    app.status = "Recent files cleared.".to_string();
                }
            });

            ui.add_space(12.0);
            ui.separator();
            ui.label(
                egui::RichText::new(format!(
                    "settings.toml · {}",
                    app.store.settings_path().display()
                ))
                .size(10.0)
                .color(theme::TEXT_MUTED),
            );
            ui.label(
                egui::RichText::new("Hand-editable. INDIUM respects what you write there.")
                    .size(10.0)
                    .color(theme::TEXT_MUTED),
            );
        });

    if !open {
        app.popup = None;
    }
}

fn group(ui: &mut egui::Ui, title: &str) {
    ui.label(
        egui::RichText::new(title)
            .size(10.0)
            .color(theme::AUBERGINE.to_opaque())
            .family(theme::bold()),
    );
    ui.add_space(3.0);
}
