//! The Open With picker — CORE §4.4 and P3 §3.
//!
//! "Applications from parsed `.desktop` files, ranked by MIME match,
//! filter-as-you-type."
//!
//! Icons are deliberately absent (P3 §3). The footer sentence is permanent, "so it
//! never becomes a bug report".

use eframe::egui;

use super::{Indium, Popup, Status};
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
        .max_height(theme::popup_max_height(ctx))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .open(&mut open)
        .show(ctx, |ui| {
            ui.set_min_width(520.0);

            ui.label(
                egui::RichText::new(&app.openwith_name)
                    .size(theme::TITLE)
                    .color(theme::TEXT),
            );
            ui.label(
                egui::RichText::new(&app.openwith_mime)
                    .family(theme::MONO)
                    .size(theme::BODY)
                    .color(theme::TEXT_MUTED),
            );
            ui.add_space(8.0);

            // Read before the field is built, so the answer cannot depend on what a
            // focused `TextEdit` decides to do with an arrow key.
            let (up, down, enter) = ui.input(|i| {
                (
                    i.key_pressed(egui::Key::ArrowUp),
                    i.key_pressed(egui::Key::ArrowDown),
                    i.key_pressed(egui::Key::Enter),
                )
            });

            let resp = ui.add(
                egui::TextEdit::singleline(&mut app.openwith_filter)
                    .hint_text("filter")
                    .desired_width(f32::INFINITY),
            );
            // Once. It used to be every frame, which is why the list below could be seen
            // but never reached: focus returned here before an arrow key could land.
            if app.wants_initial_focus(&Popup::OpenWith) {
                resp.request_focus();
            }
            // A narrowed list is a different list, so the cursor goes back to its head
            // rather than staying on whatever row happens to be at that index now.
            if resp.changed() {
                app.openwith_cursor = 0;
            }

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

            // The keyboard's place in the list, moved and clamped together so the cursor
            // can never point past a list the filter just shortened.
            if visible.is_empty() {
                app.openwith_cursor = 0;
            } else {
                let last = visible.len() - 1;
                if up {
                    app.openwith_cursor = app.openwith_cursor.saturating_sub(1);
                }
                if down {
                    app.openwith_cursor = (app.openwith_cursor + 1).min(last);
                }
                app.openwith_cursor = app.openwith_cursor.min(last);
                if enter {
                    launch = visible.get(app.openwith_cursor).map(|c| (*c).clone());
                }
            }

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
                .max_height(theme::list_height(ctx, 300.0, 300.0))
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for (i, c) in visible.iter().enumerate() {
                        // Through `theme::row`, which reads its own `Response`. These rows
                        // had no fill in any state and no cursor at all, so a list built to
                        // be clicked gave no sign that it could be. P7 §2.
                        //
                        // `active` was always false while nothing in the picker could be
                        // chosen without the pointer. P11 gave the list a keyboard cursor,
                        // and Aubergine means *the active item* — which is now exactly what
                        // `Enter` would open. The default application keeps its own chip;
                        // being the default and being under the cursor are different facts.
                        let on_cursor = i == app.openwith_cursor;
                        // Under the cursor the ground is Aubergine, where TEXT_MUTED measures
                        // 3.30:1 — under AA, and unmeasured until P18. One tier up is 5.01:1.
                        let dim = if on_cursor {
                            theme::TEXT_SECONDARY
                        } else {
                            theme::TEXT_MUTED
                        };
                        let r = theme::row(ui, on_cursor, egui::Margin::symmetric(8, 5), |ui| {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(&c.app.name).color(theme::TEXT));
                                if c.is_default {
                                    ui.label(
                                        egui::RichText::new("default")
                                            .size(theme::SMALL)
                                            .family(theme::MONO)
                                            .color(theme::TEXT),
                                    );
                                }
                                if c.app.terminal {
                                    ui.label(
                                        egui::RichText::new("terminal")
                                            .size(theme::SMALL)
                                            .color(dim),
                                    );
                                }
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label(
                                            egui::RichText::new(&c.app.id)
                                                .size(theme::SMALL)
                                                .family(theme::MONO)
                                                .color(dim),
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
            theme::foot(ui, |ui| {
                ui.label(
                    egui::RichText::new(FOOTER)
                        .size(theme::BODY)
                        .color(theme::TEXT_MUTED),
                );
            });
        });

    if let Some(c) = launch {
        match app.openwith_path.clone() {
            Some(path) => match apps::launch(&c.app, &path) {
                Ok(()) => {
                    app.popup = None;
                    app.status = format!("Opened in {}.", c.app.name).into();
                }
                Err(e) => app.status = Status::bad(e),
            },
            None => app.status = Status::bad("Nothing to open."),
        }
    }

    if !open {
        app.popup = None;
    }
}
