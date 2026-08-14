//! Pending tasks — CORE §4's second popup, `W`, built by P4 §5.
//!
//! CORE §4.2: "The full task list: one row per staged operation with its own remove ✕,
//! then *Discard all* and **Apply**."
//!
//! Copyright © sudo-megas. GPL-3.0-only.

use eframe::egui;

use super::{Indium, Popup};
use crate::tasks;
use crate::theme;

pub fn show(app: &mut Indium, ctx: &egui::Context) {
    if app.popup != Some(Popup::PendingTasks) {
        return;
    }
    let mut open = true;

    // The deferred-action idiom this file inherits from `settings.rs`: an intent is
    // collected inside the closure and acted on after it, because acting inside would
    // hold a borrow of `app` across calls that need it mutably.
    let mut remove: Option<usize> = None;
    let mut discard = false;
    let mut apply = false;

    theme::floating(ctx, "Pending tasks")
        .max_height(theme::popup_max_height(ctx))
        .collapsible(false)
        .resizable(false)
        .open(&mut open)
        .show(ctx, |ui| {
            ui.set_min_width(560.0);

            if app.tasks.is_empty() {
                // A popup that refuses to appear teaches nothing, so `W` always opens
                // and an empty queue says so.
                ui.label(
                    egui::RichText::new("Nothing staged.")
                        .size(theme::BODY)
                        .color(theme::TEXT_MUTED),
                );
                return;
            }

            egui::ScrollArea::vertical()
                .max_height(theme::list_height(ctx, 250.0, 320.0))
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for (i, task) in app.tasks.tasks().iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(task.verb())
                                    .size(theme::SMALL)
                                    .color(theme::TEXT)
                                    .family(theme::bold()),
                            );
                            ui.label(
                                egui::RichText::new(task.summary())
                                    .family(theme::MONO)
                                    .size(theme::BODY)
                                    .color(theme::TEXT),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    // The comment this replaces said the character was "what
                                    // `settings.rs` already uses for the same job", which was
                                    // true and was the problem: four rows had each typed it,
                                    // and none was checked by anything. [`theme::REMOVE`] is
                                    // where it is written now, and why it is that character.
                                    if theme::small_button(
                                        ui,
                                        egui::RichText::new(theme::REMOVE),
                                        true,
                                    )
                                    .clicked()
                                    {
                                        remove = Some(i);
                                    }
                                },
                            );
                        });
                    }
                });

            // What the rebuild will cost in metadata, said before Apply rather than
            // discovered after it.
            let losses = app
                .staging_container()
                .map(|c| tasks::metadata_losses(c, &app.entries))
                .unwrap_or_default();
            if !losses.is_empty() {
                ui.add_space(6.0);
                for note in &losses {
                    ui.label(
                        egui::RichText::new(note)
                            .size(theme::SMALL)
                            .italics()
                            .color(theme::TEXT_MUTED),
                    );
                }
            }

            ui.add_space(8.0);
            theme::foot(ui, |ui| {
                ui.horizontal(|ui| {
                    discard = theme::button(ui, egui::RichText::new("Discard all"), true).clicked();
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        apply = theme::button(
                            ui,
                            egui::RichText::new("Apply")
                                .color(theme::ORANGE)
                                .family(theme::bold()),
                            true,
                        )
                        .clicked();
                    });
                });
                ui.label(
                    egui::RichText::new("Esc closes · nothing is written until Apply")
                        .size(theme::SMALL)
                        .color(theme::TEXT_MUTED),
                );
            });
        });

    if let Some(i) = remove {
        app.remove_task(i);
    }
    if discard {
        app.discard_tasks();
        app.popup = None;
    }
    if apply {
        app.popup = None;
        app.request_apply(ctx);
    }
    if !open {
        app.close_popup();
    }
}
