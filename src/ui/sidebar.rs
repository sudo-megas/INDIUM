//! The sidebar — CORE §4's first zone.
//!
//! "the wordmark at top, then *Recent files* `1`, *Bookmarks* `2`, *Archive* `3`; at
//! the bottom *New* `N`, *Settings* `,`, *About* `A`. Numbers and letters are bare
//! keypresses, as in JADEITE."

use eframe::egui;

use super::{Indium, Popup, Section};
use crate::theme;

pub fn show(app: &mut Indium, root: &mut egui::Ui) {
    egui::Panel::left("sidebar")
        .resizable(false)
        .exact_size(190.0)
        .frame(
            egui::Frame::NONE
                .fill(theme::PANEL)
                .inner_margin(egui::Margin::symmetric(12, 14)),
        )
        .show(root, |ui| {
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new("INDIUM")
                        .size(21.0)
                        .color(theme::TEXT)
                        .family(theme::bold()),
                );
                ui.label(
                    egui::RichText::new("archive manager")
                        .size(11.0)
                        .color(theme::TEXT_MUTED),
                );
                ui.add_space(18.0);

                section_item(ui, app, Section::Recents, "Recent files", "1", true);
                section_item(ui, app, Section::Bookmarks, "Bookmarks", "2", true);
                section_item(ui, app, Section::Archive, "Archive", "3", app.has_archive());
            });

            // The bottom group sits at the foot of the panel, as CORE draws it.
            ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                ui.add_space(4.0);
                action_item(ui, app, "About", "A", Some(Popup::About), true);
                action_item(ui, app, "Settings", ",", Some(Popup::Settings), true);
                // New Archive is P4's popup. Rendered, honest, and inert.
                action_item(ui, app, "New", "N", None, false);
                ui.add_space(10.0);
                ui.separator();
            });
        });
}

fn section_item(
    ui: &mut egui::Ui,
    app: &mut Indium,
    section: Section,
    label: &str,
    key: &str,
    enabled: bool,
) {
    let active = app.section == section;
    let response = row(ui, label, key, active, enabled, None);
    if response.clicked() && enabled {
        app.section = section;
        app.cursor = 0;
    }
}

fn action_item(
    ui: &mut egui::Ui,
    app: &mut Indium,
    label: &str,
    key: &str,
    popup: Option<Popup>,
    enabled: bool,
) {
    let tag = if enabled { None } else { Some("P4") };
    let response = row(ui, label, key, false, enabled, tag);
    if response.clicked() {
        match &popup {
            Some(p) => app.popup = Some(p.clone()),
            None => app.status = format!("{label} arrives in P4."),
        }
    }
}

/// One sidebar line: label on the left, its bare-key hint on the right.
fn row(
    ui: &mut egui::Ui,
    label: &str,
    key: &str,
    active: bool,
    enabled: bool,
    tag: Option<&str>,
) -> egui::Response {
    let text_colour = if !enabled {
        theme::TEXT_MUTED
    } else if active {
        theme::TEXT
    } else {
        theme::TEXT_SECONDARY
    };

    let frame = egui::Frame::NONE
        .inner_margin(egui::Margin::symmetric(8, 6))
        // Aubergine is the active-item colour (CORE §6), never orange: nothing is
        // about to happen when you merely change section.
        .fill(if active {
            theme::AUBERGINE
        } else {
            egui::Color32::TRANSPARENT
        })
        .corner_radius(3.0);

    let inner = frame.show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(label).color(text_colour));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(t) = tag {
                    ui.label(
                        egui::RichText::new(t)
                            .size(10.0)
                            .family(theme::MONO)
                            .color(theme::TEXT_MUTED),
                    );
                } else {
                    ui.label(
                        egui::RichText::new(key)
                            .family(theme::MONO)
                            .size(11.0)
                            .color(theme::TEXT_MUTED),
                    );
                }
            });
        });
    });

    let response = ui.interact(
        inner.response.rect,
        ui.id().with(label),
        egui::Sense::click(),
    );
    if enabled {
        response
            .clone()
            .on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        response
    }
}
