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

                // Elastic, where it used to be a flat 260. Four things share this line and
                // the last of them is right-aligned, so a field that does not fit does not
                // shrink — it pushes "214 of 11,482 match" underneath "Esc clears" and the
                // two render on top of each other. Measured at 1.25× on a 963pt-wide window
                // it already did, before P7 and after; `main.rs` sets the minimum window to
                // 840pt, so this was always reachable, and P7 §1's four zone cards cost the
                // bar about 30pt more. Taking what is left instead makes it unreachable.
                //
                // 250 is what the two items to the right need at their widest: a five-digit
                // count either side of " of " is 20 monospace columns, the hint is ten, and
                // there are two gaps of `item_spacing`.
                let field = (ui.available_width() - 250.0).clamp(120.0, 260.0);
                let resp = ui.add(
                    egui::TextEdit::singleline(needle)
                        .font(egui::TextStyle::Monospace)
                        .hint_text("substring")
                        .desired_width(field),
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
                            .size(13.0)
                            .color(theme::TEXT_MUTED),
                    );
                });
            });
        });
}
