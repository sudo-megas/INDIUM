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

    let panel = egui::Panel::bottom("tray")
        .frame(theme::zone(theme::PANEL).inner_margin(egui::Margin::symmetric(10, 6)))
        // The card's own 2px edge is the boundary; egui's panel hairline would stack with it.
        .show_separator_line(false);

    panel.show(ui, |ui| {
        // "The strip itself is a button" (CORE §4), and now it is one you can feel: the
        // fill answers the pointer and the cursor becomes a hand, neither of which the
        // hand-rolled `ui.interact` at the foot of this function ever did.
        //
        // Pad zero, because the zone's own `symmetric(10, 6)` inner margin has already
        // spaced this line; a second pad here would only inset the hover fill from the
        // card rim by twice as much as it needs.
        //
        // **The `if !apply && !discard` guard that used to wrap the strip's `interact` is
        // gone, and nothing replaces it.** `theme::row` builds on `UiBuilder::sense`, which
        // registers the row's sense *below* everything added inside it (egui's `Ui::new_child`
        // says so in as many words: "Register in the widget stack early, to ensure we are
        // behind all widgets we contain"), and egui's hit test hands the click to the
        // topmost sensing widget under the pointer. Apply and Discard are real `Button`s,
        // so they beat the strip by construction rather than by a bool the next edit could
        // forget to update.
        open_tasks = theme::row(ui, false, egui::Margin::ZERO, |ui| {
            // Off for the same reason as everywhere else a `theme::row` holds a label: a
            // selectable label senses click-and-drag and would out-rank the row beneath it,
            // so clicking the summary text would do nothing. See `sidebar::row_body`.
            ui.style_mut().interaction.selectable_labels = false;
            ui.horizontal(|ui| {
                // Orange, and legitimately so: CORE §6 reserves it for the current
                // selection, staged changes, and Apply/progress. This is the second of the
                // three, and the first time INDIUM has had cause to use it.
                ui.label(
                    egui::RichText::new(app.tasks.tray_summary())
                        .family(theme::MONO)
                        .size(13.0)
                        .color(theme::ORANGE),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    apply = theme::button(
                        ui,
                        egui::RichText::new("Apply")
                            .color(theme::ORANGE)
                            .family(theme::bold()),
                        true,
                    )
                    .clicked();
                    discard = theme::button(ui, egui::RichText::new("Discard"), true).clicked();
                });
            });
        })
        .clicked();
    });

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
