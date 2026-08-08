//! The About popup — CORE §4.6.
//!
//! "The mark, the maker, the version and date, the source address and the licence in
//! full. Addresses are text you can select but not click — INDIUM opens no browser
//! and follows no link, by design." (CORE §9: "The app never opens a URL".)

use eframe::egui;

use super::{Indium, Popup};
use crate::theme;

const SOURCE: &str = "https://github.com/sudo-megas/INDIUM";
/// Updated by the maker at each tag, by hand, in the same commit as the version bump.
///
/// Deliberately not a build-time timestamp: CORE §8 ships this as a package, and a binary
/// that embeds the minute it was compiled cannot be built twice into the same bytes. A
/// constant is deterministic and is exactly how `LICENCE` below is already embedded.
const RELEASE_DATE: &str = "2026-08-08";
const LICENCE: &str = include_str!("../../LICENSE");

pub fn show(app: &mut Indium, ctx: &egui::Context) {
    if app.popup != Some(Popup::About) {
        return;
    }
    let mut open = true;
    egui::Window::new("About INDIUM")
        .collapsible(false)
        .resizable(true)
        .default_size([620.0, 480.0])
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .open(&mut open)
        .show(ctx, |ui| {
            ui.label(
                egui::RichText::new("INDIUM")
                    .size(28.0)
                    .color(theme::TEXT)
                    .family(theme::bold()),
            );
            ui.label(
                egui::RichText::new(
                    "An archive manager for Linux on Wayland where the metadata is the main event.",
                )
                .color(theme::TEXT_SECONDARY),
            );
            ui.add_space(10.0);

            egui::Grid::new("about-grid")
                .num_columns(2)
                .spacing([14.0, 4.0])
                .show(ui, |ui| {
                    field(ui, "Version", env!("CARGO_PKG_VERSION"));
                    // CORE §4.6 asks for "the version and date", and the date was missing
                    // from this grid from P1 until P6.
                    field(ui, "Date", RELEASE_DATE);
                    field(ui, "Maker", "sudo-megas");
                    field(ui, "Licence", "GPL-3.0-only");
                    ui.label(egui::RichText::new("Source").color(theme::TEXT_MUTED));
                    // Selectable, never clickable.
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(SOURCE)
                                .family(theme::MONO)
                                .color(theme::TEXT),
                        )
                        .selectable(true),
                    );
                    ui.end_row();
                });

            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(
                    "Addresses here are text you can select and copy. INDIUM opens no browser \
                     and follows no link.",
                )
                .size(13.0)
                .color(theme::TEXT_MUTED),
            );

            ui.add_space(10.0);
            ui.separator();
            ui.label(egui::RichText::new("The licence, in full").color(theme::TEXT_SECONDARY));
            ui.add_space(4.0);

            egui::ScrollArea::vertical()
                .max_height(240.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(LICENCE)
                                .family(theme::MONO)
                                .size(13.0)
                                .color(theme::TEXT_SECONDARY),
                        )
                        .selectable(true),
                    );
                });

            ui.add_space(8.0);
            ui.label(
                egui::RichText::new("Copyright © sudo-megas · Built with Reason and Passion.")
                    .size(13.0)
                    .color(theme::TEXT_MUTED),
            );
        });

    if !open {
        app.popup = None;
    }
}

fn field(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.label(egui::RichText::new(label).color(theme::TEXT_MUTED));
    ui.label(
        egui::RichText::new(value)
            .family(theme::MONO)
            .color(theme::TEXT),
    );
    ui.end_row();
}
