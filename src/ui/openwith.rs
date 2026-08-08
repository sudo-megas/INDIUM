//! The Open With picker — CORE §4.4 and P3 §3.
//!
//! "Applications from parsed `.desktop` files, ranked by MIME match,
//! filter-as-you-type."
//!
//! Icons are deliberately absent (P3 §3). The footer sentence is permanent, "so it
//! never becomes a bug report".

use eframe::egui;

use super::{Indium, Popup};
use crate::platform::apps::{self, Candidate};
use crate::theme;

/// P3 §3: "One permanent line in the popup footer, always visible."
const FOOTER: &str = "Opens a copy — changes won't return to the archive.";

pub fn show(app: &mut Indium, ctx: &egui::Context) {
    if app.popup != Some(Popup::OpenWith) {
        return;
    }

    let mut open = true;
    let mut launch: Option<Candidate> = None;

    egui::Window::new("Open With")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .open(&mut open)
        .show(ctx, |ui| {
            ui.set_min_width(520.0);

            ui.label(
                egui::RichText::new(&app.openwith_name)
                    .size(16.0)
                    .color(theme::TEXT),
            );
            ui.label(
                egui::RichText::new(&app.openwith_mime)
                    .family(theme::MONO)
                    .size(13.0)
                    .color(theme::TEXT_MUTED),
            );
            ui.add_space(8.0);

            let resp = ui.add(
                egui::TextEdit::singleline(&mut app.openwith_filter)
                    .hint_text("filter")
                    .desired_width(f32::INFINITY),
            );
            resp.request_focus();

            ui.add_space(6.0);

            let needle = app.openwith_filter.to_lowercase();
            let show_all = app.openwith_show_all;

            let visible: Vec<&Candidate> = app
                .openwith_candidates
                .iter()
                // Until "Show all applications" is on, only the ones that actually
                // claim the type are offered (P3 §3).
                .filter(|c| show_all || c.exact || c.is_default)
                .filter(|c| needle.is_empty() || c.app.name.to_lowercase().contains(&needle))
                .collect();

            if visible.is_empty() {
                ui.label(
                    egui::RichText::new(if app.openwith_candidates.is_empty() {
                        "No applications found."
                    } else {
                        "Nothing matches. Try Show all applications."
                    })
                    .color(theme::TEXT_MUTED),
                );
            }

            egui::ScrollArea::vertical()
                .max_height(300.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for c in &visible {
                        // Through `theme::row`, which reads its own `Response`. These rows
                        // had no fill in any state and no cursor at all, so a list built to
                        // be clicked gave no sign that it could be. P7 §2.
                        //
                        // `active` is always false: Aubergine means *the active item*, and
                        // nothing in this picker is chosen yet. The default application is
                        // already named by its own chip, which is the honest place for it.
                        let r = theme::row(ui, false, egui::Margin::symmetric(8, 5), |ui| {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(&c.app.name).color(theme::TEXT));
                                if c.is_default {
                                    ui.label(
                                        egui::RichText::new("default")
                                            .size(12.0)
                                            .family(theme::MONO)
                                            .color(theme::TEXT),
                                    );
                                }
                                if c.app.terminal {
                                    ui.label(
                                        egui::RichText::new("terminal")
                                            .size(12.0)
                                            .color(theme::TEXT_MUTED),
                                    );
                                }
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label(
                                            egui::RichText::new(&c.app.id)
                                                .size(12.0)
                                                .family(theme::MONO)
                                                .color(theme::TEXT_MUTED),
                                        );
                                    },
                                );
                            });
                        });
                        if r.clicked() {
                            launch = Some((*c).clone());
                        }
                    }
                });

            ui.add_space(6.0);
            ui.checkbox(&mut app.openwith_show_all, "Show all applications");

            ui.add_space(8.0);
            ui.separator();
            ui.label(
                egui::RichText::new(FOOTER)
                    .size(13.0)
                    .color(theme::TEXT_MUTED),
            );
        });

    if let Some(c) = launch {
        match app.openwith_path.clone() {
            Some(path) => match apps::launch(&c.app, &path) {
                Ok(()) => {
                    app.popup = None;
                    app.status = format!("Opened in {}.", c.app.name);
                }
                Err(e) => app.status = e,
            },
            None => app.status = "Nothing to open.".to_string(),
        }
    }

    if !open {
        app.popup = None;
    }
}
