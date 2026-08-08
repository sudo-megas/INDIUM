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
        .max_height(theme::popup_max_height(ctx))
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
            // Set inside the row's closure, applied after it: the change is made to the
            // settings file rather than to this window's copy of it, which needs `app`
            // mutably and cannot have it while the row is drawing.
            let mut changed: Option<ExtractDefault> = None;
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
                    changed = Some(ExtractDefault::Here);
                }
                if toggle(ui, cur == ExtractDefault::Subdir, "into a subdirectory")
                    && cur != ExtractDefault::Subdir
                {
                    changed = Some(ExtractDefault::Subdir);
                }
            });
            if let Some(want) = changed {
                app.change_settings(move |s| s.extract.default = want);
                app.extract_to_subdir = app.settings.extract.default == ExtractDefault::Subdir;
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
            // Removed by identity rather than by index: the index came from this
            // window's list, and the change is applied to the file, which another
            // window may have reordered or shortened since.
            if let Some(gone) = remove.and_then(|i| app.settings.bookmarks.get(i).cloned()) {
                app.change_settings(move |s| s.bookmarks.retain(|b| *b != gone));
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
                    let added = Bookmark {
                        name: app.bookmark_name.trim().to_string(),
                        path: app.bookmark_path.trim().to_string(),
                    };
                    app.bookmark_name.clear();
                    app.bookmark_path.clear();
                    app.change_settings(move |s| s.bookmarks.push(added));
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
                    // Status first, save last, so a refusal or a write error is what the
                    // status bar carries rather than a cheerful line about a file that is
                    // still full of what it always held.
                    app.status = "Recent files cleared.".to_string();
                    app.change_recents(|r| r.items.clear());
                }
            });

            ui.add_space(12.0);
            theme::foot(ui, |ui| {
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
        });

    if !open {
        app.popup = None;
    }
}
