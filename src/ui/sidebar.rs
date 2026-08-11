//! The sidebar — CORE §4's first zone.
//!
//! "the wordmark at top, then *Open file* `O` and *Archive* `1`; a rule; then
//! *Bookmarks* `2` and *Recent files* `3`; at the bottom *New* `N`, *Settings* `,`,
//! *About* `A`. Numbers and letters are bare keypresses, as in JADEITE."
//!
//! The order used to run the other way — *Recent files* first and *Archive* last — and a
//! testing round said what was wrong with it in one line: *"as a person's first focus
//! usually the archive he/she in."* The rule below the archive is from the same note.

use eframe::egui;

use super::{Indium, Popup, Section, Status};
use crate::platform::picker::PickerFor;
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
            // The bottom group sits at the foot of the panel, as CORE draws it — and it
            // **reserves its space before the sections above are laid out**, which is the
            // whole of P11's fix here.
            //
            // It used to be a `bottom_up` layout in the same `Ui` as the group above it.
            // Two layouts sharing one rect, each measuring from a different edge, agree
            // only while the rect is taller than both of them put together: shorten the
            // window past that and they simply draw over one another, wordmark through
            // buttons, with nothing to notice it. A panel cannot do that. What it takes is
            // gone from `available_height` before the sections ask, and the `ScrollArea`
            // below turns "does not fit" into a scrollbar rather than a collision.
            egui::Panel::bottom("sidebar-actions")
                // `Panel::resizable` defaults to **true**, and this one never said
                // otherwise — so it carried an invisible drag handle across its top edge,
                // right on the rule above *New*, and remembered whatever height it was
                // dragged to. Every other panel in the program sets this; this one was the
                // omission.
                .resizable(false)
                .frame(egui::Frame::NONE)
                .show_separator_line(false)
                .show(ui, |ui| {
                    // Tighter than the group this replaced, and for a measured reason: a
                    // panel reserves its height before the sections are laid out, so every
                    // point spent here is a point the sections above cannot have. The
                    // compositor is under no obligation to honour `min_inner_size` — KWin
                    // was observed handing INDIUM a 540pt client area against a declared
                    // 600 — so how little this costs decides how short a window still
                    // shows all three sections without scrolling.
                    ui.separator();
                    ui.add_space(1.0);
                    action_item(
                        ui,
                        app,
                        theme::icon::NEW,
                        "New",
                        "N",
                        Some(Popup::NewArchive),
                        true,
                    );
                    action_item(
                        ui,
                        app,
                        theme::icon::SETTINGS,
                        "Settings",
                        ",",
                        Some(Popup::Settings),
                        true,
                    );
                    action_item(
                        ui,
                        app,
                        theme::icon::ABOUT,
                        "About",
                        "A",
                        Some(Popup::About),
                        true,
                    );
                });

            // **The bar floats here, and only here.** egui decides whether a `ScrollArea`
            // needs a scrollbar with a bare comparison — no hysteresis — and the program's
            // `ScrollStyle::solid()` makes a visible bar cost 10 points of *width*, ramped
            // over 0.2s. In this zone the content sits within a few points of the viewport
            // at ordinary window heights, so that decision flips back and forth and takes
            // the whole sidebar with it.
            //
            // A floating bar has `floating_allocated_width: 0.0`, so it overlays the content
            // instead of displacing it: the decision can flip as often as it likes and the
            // layout never moves. `AlwaysVisible` was tried first and worked, but it leaves
            // a track drawn down a zone with nothing to scroll, which is a worse thing to
            // look at than the problem it solved.
            //
            // Scoped to this `Ui`, whose style is clone-on-write, so the wells and the
            // Inspector keep the solid bars §6 wants them to have.
            ui.spacing_mut().scroll = egui::style::ScrollStyle::floating();
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    // CORE §4 draws this zone "(family style)", and the family puts the
                    // mark above the wordmark, centred, with the sections left-aligned
                    // beneath it. The header block is the only centred thing in the window.
                    //
                    // **It is fixed, and that is the point.** P13 spent an afternoon making
                    // it adapt — three arrangements chosen by how much room the zone had —
                    // and every version of it was worse than a header that simply does not
                    // move: a first launch and a one-pixel drag could show two different
                    // layouts, which is a worse thing to look at than any one of them. If
                    // the window is too short to hold this and the four rows beneath it,
                    // those four scroll — the foot reserves its height first and stays put —
                    // exactly as they did before and exactly as every other program handles
                    // a window smaller than its contents.
                    ui.vertical_centered(|ui| {
                        ui.add(theme::mark(50.0));
                        ui.add_space(4.0);
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
                    ui.add_space(6.0);

                    open_item(ui, app);
                    section_item(
                        ui,
                        app,
                        Section::Archive,
                        theme::icon::ARCHIVE,
                        "Archive",
                        "1",
                        app.has_archive(),
                    );
                    // CORE §4: "a rule". The archive is what you are inside; the two lists
                    // are ways of getting somewhere else, and the line says which is which.
                    // It is legible now — `theme::HAIRLINE` was 8% white and measured
                    // 1.2:1, which is the *other* half of the note this order came from:
                    // "The separator staying on the New button is so faded."
                    ui.add_space(1.0);
                    ui.separator();
                    ui.add_space(1.0);
                    section_item(
                        ui,
                        app,
                        Section::Bookmarks,
                        theme::icon::BOOKMARK,
                        "Bookmarks",
                        "2",
                        true,
                    );
                    section_item(
                        ui,
                        app,
                        Section::Recents,
                        theme::icon::RECENT,
                        "Recent files",
                        "3",
                        true,
                    );
                });
        });
}

/// *Open file* — a row that is an action rather than a section.
///
/// It sits with *Archive* above the rule because both are about the archive you are in or
/// about to be in, and it raises the desktop's own picker through `xdg-desktop-portal`
/// rather than a dialog INDIUM draws. `Ctrl+O`'s path field is unchanged and still there;
/// this is the route for people who do not know a path by heart, which is what the first
/// note back from testing asked for: *"we need an open file option ... must use xdg-portal
/// file picker."*
fn open_item(ui: &mut egui::Ui, app: &mut Indium) {
    if row(
        ui,
        theme::icon::FOLDER_OPEN,
        "Open file",
        "O",
        false,
        true,
        None,
    )
    .clicked()
    {
        let ctx = ui.ctx().clone();
        app.request_picker(&ctx, PickerFor::Open);
    }
}

fn section_item(
    ui: &mut egui::Ui,
    app: &mut Indium,
    section: Section,
    icon: &str,
    label: &str,
    key: &str,
    enabled: bool,
) {
    let active = app.section == section;
    let response = row(ui, icon, label, key, active, enabled, None);
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
    icon: &str,
    label: &str,
    key: &str,
    popup: Option<Popup>,
    enabled: bool,
) {
    // Every sidebar action is live as of P4, so nothing carries a "not yet" tag any
    // more. The parameter stays because the next milestone will want it again.
    let tag = if enabled { None } else { Some("soon") };
    let response = row(ui, icon, label, key, false, enabled, tag);
    if response.clicked() {
        match &popup {
            // New Archive needs its fields seeded, which is what `N` does too.
            Some(Popup::NewArchive) => app.open_new_archive(),
            Some(p) => app.popup = Some(p.clone()),
            None => app.status = Status::bad(format!("{label} is not available yet.")),
        }
    }
}

/// The padding of one sidebar line, shared by the live and the unavailable arm so the two
/// cannot drift apart in height.
const ROW_PAD: egui::Margin = egui::Margin::symmetric(8, 3);

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
fn row_body(
    ui: &mut egui::Ui,
    icon: &str,
    label: &str,
    key: &str,
    ink: egui::Color32,
    dim: egui::Color32,
    tag: Option<&str>,
) {
    ui.style_mut().interaction.selectable_labels = false;
    ui.horizontal(|ui| {
        // CORE §4: "Every row carries a leading glyph in the same ink as its label."
        //
        // The same ink, and not a muted one, so an active row brightens as a single object
        // rather than as a label with a dimmer thing stuck to it. At `ICON_SCALE` the glyph
        // is the tallest thing in the line and sets the row's height; what it does not cost
        // is *alignment* — §2's `Mono` cut is single-cell, so every row spends exactly one
        // column on its glyph and the labels stay flush down the sidebar.
        ui.label(
            egui::RichText::new(icon)
                .family(theme::MONO)
                .size(13.0 * theme::ICON_SCALE)
                .color(ink),
        );
        ui.label(egui::RichText::new(label).color(ink));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if let Some(t) = tag {
                ui.label(
                    egui::RichText::new(t)
                        .size(12.0)
                        .family(theme::MONO)
                        .color(dim),
                );
            } else {
                ui.label(
                    egui::RichText::new(key)
                        .family(theme::MONO)
                        .size(13.0)
                        .color(dim),
                );
            }
        });
    });
}

/// One sidebar line: label on the left, its bare-key hint on the right.
fn row(
    ui: &mut egui::Ui,
    icon: &str,
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
                row_body(
                    ui,
                    icon,
                    label,
                    key,
                    theme::TEXT_MUTED,
                    theme::TEXT_MUTED,
                    tag,
                );
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
        // The tag and the key hint are the row's quiet half, and on an active row the ground
        // under them is Aubergine, where TEXT_MUTED measures 3.30:1 — under AA, and nothing
        // measured it until P18 because AUBERGINE was not in `GROUNDS`. One tier up is 5.01:1
        // and still reads as the quiet half. The resting row keeps TEXT_MUTED at 4.67:1 or
        // better, so the ladder is unchanged where it was already legible.
        let dim = if active {
            theme::TEXT_SECONDARY
        } else {
            theme::TEXT_MUTED
        };
        row_body(ui, icon, label, key, ink, dim, tag);
    })
}
