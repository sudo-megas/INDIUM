//! The staging tray — CORE §4's fourth zone, built by P4 §5.
//!
//! CORE §4: "hidden until the first staged change, then a one-line strip above the
//! status bar — count, a summary of the first tasks, *Discard*, **Apply**. The strip
//! itself is a button."
//!
//! Copyright © sudo-megas. GPL-3.0-only.

use eframe::egui;

use super::{Indium, Popup};
use crate::theme;

pub fn show(app: &mut Indium, ui: &mut egui::Ui) {
    // "Hidden until the first staged change." Not a zero-height panel — a bottom panel
    // draws its own separator hairline, and an empty one would leave a visible seam.
    if app.tasks.is_empty() {
        return;
    }

    let mut open_tasks = false;
    let mut discard = false;
    let mut apply = false;

    let panel = egui::Panel::bottom("tray").frame(
        egui::Frame::NONE
            .fill(theme::PANEL)
            .inner_margin(egui::Margin::symmetric(10, 5)),
    );

    let inner = panel.show(ui, |ui| {
        ui.horizontal(|ui| {
            // Orange, and legitimately so: CORE §6 reserves it for the current
            // selection, staged changes, and Apply/progress. This is the second of the
            // three, and the first time INDIUM has had cause to use it.
            ui.label(
                egui::RichText::new(app.tasks.tray_summary())
                    .family(theme::MONO)
                    .size(11.0)
                    .color(theme::ORANGE),
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                apply = ui
                    .button(
                        egui::RichText::new("Apply")
                            .color(theme::ORANGE)
                            .family(theme::bold()),
                    )
                    .clicked();
                discard = ui.button("Discard").clicked();
            });
        });
    });

    // "The strip itself is a button." The two buttons above are consulted first, so a
    // click that landed on one of them does not also open the task list behind it.
    if !apply && !discard {
        let response = ui.interact(
            inner.response.rect,
            ui.id().with("tray-strip"),
            egui::Sense::click(),
        );
        if response.clicked() {
            open_tasks = true;
        }
        response.on_hover_cursor(egui::CursorIcon::PointingHand);
    }

    if open_tasks {
        app.popup = Some(Popup::PendingTasks);
    }
    if discard {
        app.discard_tasks();
    }
    if apply {
        app.request_apply(ui.ctx());
    }
}
