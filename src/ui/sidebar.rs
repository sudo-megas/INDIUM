//! The sidebar — CORE §4's first zone.
//!
//! "the wordmark at top, then *File* `1`, *Draft* `2` and *Create* `N`; a rule; then
//! *Open file* `O`, *Recent files* `3` and *Bookmarks* `4`; at the bottom *Settings* `,`
//! and *About* `A`. Numbers and letters are bare keypresses, as in JADEITE."
//!
//! The order used to run the other way — *Recent files* first and the archive last — and a
//! testing round said what was wrong with it in one line: *"as a person's first focus
//! usually the archive he/she in."* The rule below it is from the same note. P22 kept that
//! finding and moved two rows around it, per CORE §4's three groups.

use eframe::egui;

use super::{Indium, Popup, Section};
use crate::platform::picker::PickerFor;
use crate::theme;

/// What the sidebar's contents get, and what P1 gave them: `190 − 12 − 12`.
const CONTENT: f32 = 166.0;
/// The sidebar's inner margin.
const ZONE_PAD: egui::Margin = egui::Margin::symmetric(12, 14);

/// What a sidebar row does when it is clicked.
///
/// Three kinds, because the sidebar has always had three: a section it shows in the centre
/// zone, a popup it opens, and the one row that is neither — *Open file*, which raises the
/// desktop's own picker and shows nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Does {
    Show(Section),
    Open(Popup),
    Pick,
}

/// One line of the sidebar: what it does, its glyph, its label, its bare key, and which of
/// CORE §4's three groups it belongs to.
pub struct Row {
    pub does: Does,
    pub icon: &'static str,
    pub label: &'static str,
    pub key: &'static str,
    /// 1 above the rule, 2 below it, 3 at the foot of the panel.
    pub group: u8,
}

/// CORE §4's sidebar, in its order. The one list this zone draws.
///
/// It was **eight hand-written call sites** until P22, with nothing between them and the
/// document: the labels, the keys and the groups could all drift silently, and the keys had
/// already drifted into a second copy in `mod.rs`'s bare-key `match` besides, flagged there
/// in a comment as two lists that must be read together and held by nothing.
/// `the_sidebar_rows_are_the_ones_core_lists` below reads §4's own paragraph out of
/// `CORE.md` at test time and fails if the two disagree — the `keys::ROWS` idiom, for the
/// same reason and in the same direction: the document moves first, and this follows.
pub const ROWS: &[Row] = &[
    Row {
        does: Does::Show(Section::File),
        icon: theme::icon::ARCHIVE,
        label: "File",
        key: "1",
        group: 1,
    },
    Row {
        does: Does::Show(Section::Draft),
        icon: theme::icon::DRAFT,
        label: "Draft",
        key: "2",
        group: 1,
    },
    Row {
        does: Does::Open(Popup::Create),
        icon: theme::icon::NEW,
        label: "Create",
        key: "N",
        group: 1,
    },
    Row {
        does: Does::Pick,
        icon: theme::icon::FOLDER_OPEN,
        label: "Open file",
        key: "O",
        group: 2,
    },
    Row {
        does: Does::Show(Section::Recents),
        icon: theme::icon::RECENT,
        label: "Recent files",
        key: "3",
        group: 2,
    },
    Row {
        does: Does::Show(Section::Bookmarks),
        icon: theme::icon::BOOKMARK,
        label: "Bookmarks",
        key: "4",
        group: 2,
    },
    Row {
        does: Does::Open(Popup::Settings),
        icon: theme::icon::SETTINGS,
        label: "Settings",
        key: ",",
        group: 3,
    },
    Row {
        does: Does::Open(Popup::About),
        icon: theme::icon::ABOUT,
        label: "About",
        key: "A",
        group: 3,
    },
];

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
                    draw_group(ui, app, 3);
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

                    draw_group(ui, app, 1);
                    // CORE §4: "a rule". Above it is the archive you are in or making — the
                    // file, the draft, and the control that builds it; below it is how you
                    // reach another one, and the line says which is which. It is legible now
                    // — `theme::HAIRLINE` was 8% white and measured 1.2:1, which is the
                    // *other* half of the note this order came from: "The separator staying
                    // on the New button is so faded."
                    ui.add_space(1.0);
                    ui.separator();
                    ui.add_space(1.0);
                    draw_group(ui, app, 2);
                });
        });
}

/// The section a bare digit selects, read out of [`ROWS`] rather than typed a second time.
///
/// `handle_keys` used to hold its own `Num1 => Archive, Num2 => Bookmarks, …` match, with a
/// comment saying it and the sidebar "have to be read together". Two hand-kept lists and a
/// comment between them is the arrangement this project keeps finding drifted, so P22 made
/// the second one ask the first.
pub fn section_for_key(key: &str) -> Option<Section> {
    ROWS.iter().find_map(|r| match &r.does {
        Does::Show(section) if r.key == key => Some(*section),
        _ => None,
    })
}

/// Draw one of CORE §4's three groups, in [`ROWS`] order.
///
/// **Every row is live.** Nothing in this zone has carried a "not yet" tag since P4, and P22
/// removed the last thing that was ever conditionally dead: *File* used to be disabled with
/// no archive open, which made the section you would go to *in order to* open one the single
/// section you could not enter. `row` keeps its unavailable mode — P4 wrote it down as
/// deliberate and a later round will want it — but nothing here asks for it.
fn draw_group(ui: &mut egui::Ui, app: &mut Indium, group: u8) {
    for r in ROWS.iter().filter(|r| r.group == group) {
        match &r.does {
            Does::Show(section) => section_item(ui, app, *section, r.icon, r.label, r.key),
            Does::Open(popup) => action_item(ui, app, r.icon, r.label, r.key, popup),
            Does::Pick => open_item(ui, app, r.icon, r.label, r.key),
        }
    }
}

/// *Open file* — the one row that is an action and shows nothing.
///
/// It raises the desktop's own picker through `xdg-desktop-portal` rather than a dialog
/// INDIUM draws. `Ctrl+O`'s path field is unchanged and still there; this is the route for
/// people who do not know a path by heart, which is what the first note back from testing
/// asked for: *"we need an open file option ... must use xdg-portal file picker."* It kept
/// the archive's company above the rule until P22 and now sits below it with the two lists,
/// because it is a way in rather than something you are holding.
fn open_item(ui: &mut egui::Ui, app: &mut Indium, icon: &str, label: &str, key: &str) {
    if row(ui, icon, label, key, false, true, None).clicked() {
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
) {
    let active = app.section == section;
    if row(ui, icon, label, key, active, true, None).clicked() {
        // No cursor reset. Each section has kept its own since P11, so leaving File for
        // Bookmarks and coming back lands where you were rather than at the top — and
        // resetting here would have written the *file's* cursor on the way to a list,
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
    popup: &Popup,
) {
    if row(ui, icon, label, key, false, true, None).clicked() {
        match popup {
            // Create needs its fields seeded, which is what `N` does too.
            Popup::Create => app.open_create(),
            p => app.popup = Some(p.clone()),
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

#[cfg(test)]
mod tests {
    use super::*;

    /// CORE §4's sidebar paragraph, as one run of words.
    ///
    /// Newlines in the document are wrapping and not structure — §4 wraps *Create* onto one
    /// line and its `N` onto the next — so the paragraph is flattened before it is read.
    fn sidebar_paragraph() -> String {
        let core = include_str!("../../CORE.md");
        let after = core
            .split_once("**Sidebar**")
            .expect("CORE §4 describes the Sidebar")
            .1;
        let para = after
            .split_once("**Entry table**")
            .expect("CORE §4 describes the Entry table after the Sidebar")
            .0;
        para.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    /// Read the rows out of that paragraph: an italic label, then a backticked key.
    ///
    /// Split on the backtick and every odd chunk is a key; the chunk before it ends with the
    /// label's closing `*` when — and only when — that key belongs to a row. A backtick that
    /// is not preceded by an italic run is prose and is skipped, which is what lets §4 write
    /// `Ctrl+O` in the same section without inventing a row.
    fn rows_core_lists(flat: &str) -> Vec<(String, String)> {
        let parts: Vec<&str> = flat.split('`').collect();
        let mut out = Vec::new();
        for k in (1..parts.len()).step_by(2) {
            let Some(head) = parts[k - 1].strip_suffix("* ") else {
                continue;
            };
            let Some(start) = head.rfind('*') else {
                continue;
            };
            let label = &head[start + 1..];
            if label.is_empty() || label.contains('*') {
                continue;
            }
            out.push((label.to_string(), parts[k].to_string()));
        }
        out
    }

    /// CORE §4's sidebar paragraph and [`ROWS`] are the same list, in the same order and in
    /// the same three groups.
    ///
    /// This zone was eight hand-written call sites and a paragraph with **nothing between
    /// them**, in a program whose whole method is that a list written twice is held by a
    /// test. `the_popup_and_core_agree_about_the_keys` has done this for the keyboard table
    /// since P12 and caught drift more than once; this is the same instrument pointed at the
    /// other list §4 keeps, in the round that rewrites both.
    ///
    /// It reads a row as *label* followed by `key`, which is a constraint on §4's prose and
    /// a deliberate one: a list a test can read is a list that cannot drift. The groups are
    /// checked as well as the order, because the maker's F3 ruling **is** the grouping — a
    /// rule that moved silently would leave *Create* filed with Settings again.
    #[test]
    fn the_sidebar_rows_are_the_ones_core_lists() {
        let flat = sidebar_paragraph();
        let listed = rows_core_lists(&flat);

        assert!(
            !listed.is_empty(),
            "CORE §4's sidebar paragraph did not parse — it no longer writes its rows as \
             *label* followed by `key`, or the paragraph has moved"
        );
        assert_eq!(
            listed.len(),
            ROWS.len(),
            "CORE §4 lists {} sidebar rows and the sidebar draws {}: {listed:?}",
            listed.len(),
            ROWS.len()
        );
        for (i, ((core_label, core_key), drawn)) in listed.iter().zip(ROWS.iter()).enumerate() {
            assert_eq!(
                core_label, drawn.label,
                "row {i}: CORE says {core_label:?}, the sidebar draws {:?}",
                drawn.label
            );
            assert_eq!(
                core_key, drawn.key,
                "row {i} ({core_label}): CORE says key {core_key:?}, the sidebar draws {:?}",
                drawn.key
            );
        }

        let rule = flat
            .find("a rule")
            .expect("CORE §4 puts a rule through the sidebar");
        let foot = flat
            .find("at the bottom")
            .expect("CORE §4 puts a group at the bottom of the sidebar");
        for drawn in ROWS {
            let at = flat
                .find(&format!("*{}*", drawn.label))
                .expect("every drawn row is named in CORE §4");
            let ok = match drawn.group {
                1 => at < rule,
                2 => at > rule && at < foot,
                _ => at > foot,
            };
            assert!(
                ok,
                "{} is drawn in group {} and CORE §4 puts it elsewhere",
                drawn.label, drawn.group
            );
        }
    }

    /// Every digit CORE §4's keyboard table offers selects a section, and the bare-key
    /// `match` in `handle_keys` reads them from here rather than keeping its own copy.
    #[test]
    fn every_sidebar_digit_selects_the_section_core_puts_on_it() {
        for drawn in ROWS {
            let Does::Show(section) = &drawn.does else {
                continue;
            };
            assert_eq!(
                section_for_key(drawn.key),
                Some(*section),
                "{} is drawn on key {:?} and that key selects something else",
                drawn.label,
                drawn.key
            );
        }
        assert_eq!(
            section_for_key("N"),
            None,
            "a key that opens a popup must not also select a section"
        );
    }
}
