//! The `Ctrl+F` filter bar — P2 §4.
//!
//! "While active, the table switches to **flat, whole-archive** view — full paths,
//! case-insensitive substring match, live as you type — because the moment you reach
//! for a filter in an 11,000-entry archive, the directory you happen to be standing in
//! is the wrong scope."
//!
//! "Plain substring only in v1; globs are a temptation, not a requirement."

use eframe::egui;

use super::Indium;
use crate::theme;

/// Draw the bar, if it is open. Returns the number of matching rows so the status
/// line can report *"214 of 11,482 match"*.
pub fn show(app: &mut Indium, ui: &mut egui::Ui, matched: usize) {
    let Some(needle) = app.filter.as_mut() else {
        return;
    };

    let total = app.entries.len();

    egui::Frame::NONE
        .fill(theme::PANEL)
        .inner_margin(egui::Margin::symmetric(8, 5))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Filter")
                        .family(theme::MONO)
                        .color(theme::TEXT_MUTED),
                );

                let resp = ui.add(
                    egui::TextEdit::singleline(needle)
                        .font(egui::TextStyle::Monospace)
                        .hint_text("substring")
                        .desired_width(260.0),
                );

                if app.filter_focus_requested {
                    resp.request_focus();
                    app.filter_focus_requested = false;
                }

                ui.label(
                    egui::RichText::new(format!("{matched} of {total} match"))
                        .family(theme::MONO)
                        .color(theme::TEXT_SECONDARY),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new("Esc clears")
                            .size(11.0)
                            .color(theme::TEXT_MUTED),
                    );
                });
            });
        });
}
