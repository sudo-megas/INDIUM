//! The sidebar — CORE §4's first zone.
//!
//! "the wordmark at top, then *Recent files* `1`, *Bookmarks* `2`, *Archive* `3`; at
//! the bottom *New* `N`, *Settings* `,`, *About* `A`. Numbers and letters are bare
//! keypresses, as in JADEITE."

use eframe::egui;

use super::{Indium, Popup, Section};
use crate::theme;

/// What the sidebar's contents get, and what P1 gave them: `190 − 12 − 12`.
const CONTENT: f32 = 166.0;
/// The sidebar's inner margin.
const ZONE_PAD: egui::Margin = egui::Margin::symmetric(12, 14);

pub fn show(app: &mut Indium, root: &mut egui::Ui) {
    // `exact_size` is the panel's *outer* width, so it has to pay for the whole frame —
    // and `Frame::total_margin` is `inner_margin + stroke.width + outer_margin` (egui 0.36
    // `frame.rs`), which means the 2px edge is **not** free. Asked rather than written down:
    // a hand-summed 198 (inner + gutter, edge forgotten) is exactly the four-pixel error
    // this line exists to make impossible, and it is invisible from outside — `Panel` clamps
    // the rect it reports to the size it was given and paints any overflow regardless.
    let frame = theme::zone(theme::PANEL).inner_margin(ZONE_PAD);
    egui::Panel::left("sidebar")
        .resizable(false)
        .exact_size(CONTENT + frame.total_margin().sum().x)
        .frame(frame)
        // egui draws its own hairline between panels, and it would stack with the card's
        // 2px edge — two lines a pixel apart, which is worse than either alone.
        .show_separator_line(false)
        .show(root, |ui| {
            ui.vertical(|ui| {
                // CORE §4 draws this zone "(family style)", and the family puts the mark
                // above the wordmark, centred, with the sections left-aligned beneath it.
                // The header block is the only centred thing in the window.
                ui.vertical_centered(|ui| {
                    ui.add_space(2.0);
                    ui.add(theme::mark(84.0));
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new("INDIUM")
                            .size(23.0)
                            .color(theme::TEXT)
                            .family(theme::bold()),
                    );
                    ui.label(
                        egui::RichText::new("archive manager")
                            .size(13.0)
                            .color(theme::TEXT_MUTED),
                    );
                });
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
                action_item(ui, app, "New", "N", Some(Popup::NewArchive), true);
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
        // No cursor reset. Each section has kept its own since P11, so leaving Archive for
        // Bookmarks and coming back lands where you were rather than at the top — and
        // resetting here would have written the *archive's* cursor on the way to a list,
        // which is how the one shared field used to corrupt itself.
        app.section = section;
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
    // Every sidebar action is live as of P4, so nothing carries a "not yet" tag any
    // more. The parameter stays because the next milestone will want it again.
    let tag = if enabled { None } else { Some("soon") };
    let response = row(ui, label, key, false, enabled, tag);
    if response.clicked() {
        match &popup {
            // New Archive needs its fields seeded, which is what `N` does too.
            Some(Popup::NewArchive) => app.open_new_archive(),
            Some(p) => app.popup = Some(p.clone()),
            None => app.status = format!("{label} is not available yet."),
        }
    }
}

/// The padding of one sidebar line, shared by the live and the unavailable arm so the two
/// cannot drift apart in height.
const ROW_PAD: egui::Margin = egui::Margin::symmetric(8, 6);

/// The contents of one sidebar line — label on the left, bare-key hint on the right —
/// shared by the live arm and the unavailable one so the two cannot drift apart.
///
/// **`selectable_labels` is turned off here, and that is load-bearing rather than
/// tidiness.** egui's default is `true` (`style.rs`
/// `Interaction::default`), which makes every plain `ui.label` allocate its rect with
/// `Sense::click_and_drag()` so its text can be selected. `theme::row` registers its own
/// sense *below* its content by design, and egui's hit test hands a click to the topmost
/// sensing widget under the pointer — so a selectable label would swallow every click that
/// landed on the words "Recent files" and only the padding around them would still work.
/// Nothing in a sidebar line is text anyone wants to select, so the flag goes off; the
/// `Ui`'s style is clone-on-write, so this dies with the row.
fn row_body(ui: &mut egui::Ui, label: &str, key: &str, ink: egui::Color32, tag: Option<&str>) {
    ui.style_mut().interaction.selectable_labels = false;
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).color(ink));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if let Some(t) = tag {
                ui.label(
                    egui::RichText::new(t)
                        .size(12.0)
                        .family(theme::MONO)
                        .color(theme::TEXT_MUTED),
                );
            } else {
                ui.label(
                    egui::RichText::new(key)
                        .family(theme::MONO)
                        .size(13.0)
                        .color(theme::TEXT_MUTED),
                );
            }
        });
    });
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
    // A section that is not available is not a control. `theme::row` has exactly one mode
    // — "this is clickable" — so the unavailable line keeps a plain frame: muted ink, no
    // hover fill, no pointing hand, and a `Sense::hover()` response that can never report
    // a click. That is the disabled behaviour P1 gave it, preserved rather than restyled.
    if !enabled {
        return egui::Frame::NONE
            .inner_margin(ROW_PAD)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                row_body(ui, label, key, theme::TEXT_MUTED, tag);
            })
            .response;
    }

    // `theme::row` paints the Aubergine active fill itself, and Aubergine is the
    // active-item colour (CORE §6), never orange: nothing is about to happen when you
    // merely change section. It also owns the hover fill, the held fill and the pointing
    // hand, which is why the trailing `ui.interact` and `on_hover_cursor` this function
    // used to end with are gone.
    theme::row(ui, active, ROW_PAD, |ui| {
        let ink = if active {
            theme::TEXT
        } else {
            theme::TEXT_SECONDARY
        };
        row_body(ui, label, key, ink, tag);
    })
}
