//! The Keys popup — CORE §4.9.
//!
//! "The table below, drawn in the window. It exists because a person who had used the
//! program for an afternoon wrote *'I didn't know `Ctrl+O` opens a file, and still don't
//! know how to exit from the archive'* — a program whose whole interface is bare keypresses
//! owes the reader the list."
//!
//! CORE §4 also says it is "**generated from the bindings, never typed twice**: a keys popup
//! that has drifted from the keys is worse than no keys popup." That is a hard thing to
//! mean literally — the bindings are a `match` over `egui::Key` in [`super::Indium`], and a
//! `match` is not iterable. So the rule is enforced from the other end: [`ROWS`] below is
//! the one list this popup draws, and `the_popup_and_core_agree_about_the_keys` reads CORE
//! §4's own keyboard table out of `CORE.md` at test time and fails if the two disagree by so
//! much as a row. The document is the source; this is a copy that cannot drift silently.

use eframe::egui;

use super::{Indium, Popup};
use crate::theme;

/// CORE §4's keyboard table, in its order.
///
/// Kept as `(key, does)` pairs rather than as prose so the test can compare them cell by
/// cell against the document.
pub const ROWS: &[(&str, &str)] = &[
    ("1 2 3 4", "Sidebar sections"),
    (
        "O / I",
        "Open file · Add files — both raise the desktop's own picker",
    ),
    (
        "N W E A ,",
        "Create · Pending tasks · Extract · About · Settings",
    ),
    ("F1", "Keys — this table, in the window"),
    ("Arrows, PgUp/PgDn, Home/End", "Move in the table"),
    ("Enter / Backspace", "Descend / go up"),
    ("Space", "Details ⇄ Preview"),
    ("Ctrl+F", "Filter bar"),
    ("Ctrl+A", "Select all"),
    (
        "Ctrl+C",
        "Copy out (extract to runtime dir, URIs to clipboard)",
    ),
    ("Ctrl+V / drop files", "Stage an add"),
    ("Del / F2", "Stage a remove / a rename"),
    ("Ctrl+O", "Open (path field)"),
    ("Esc", "Close the topmost popup"),
];

/// The one line the testing round asked for that is not a keybinding.
///
/// *"still dont know how to exit from the archive"* — and until P22 the honest answer was
/// that there was no way, only a window to close. There is a control now, and it is on the
/// breadcrumb row rather than in this table because leaving an archive throws staged work
/// away and a key is too easy to hit. Saying where it is, is the whole point of the popup,
/// so it is said here rather than left to be inferred from a table that does not mention it.
const NO_KEY_FOR: &str =
    "There is no key for leaving an archive — Close, on the breadcrumb row, is how you \
     leave it. Opening another archive leaves this one too, and takes this same window.";

pub fn show(app: &mut Indium, ctx: &egui::Context) {
    if app.popup != Some(Popup::Keys) {
        return;
    }
    let mut open = true;
    theme::floating(ctx, "Keys")
        .max_height(theme::popup_max_height(ctx))
        .collapsible(false)
        .resizable(true)
        .default_size([560.0, 460.0])
        .open(&mut open)
        .show(ctx, |ui| {
            ui.set_min_width(520.0);

            egui::ScrollArea::vertical()
                .max_height(theme::list_height(ctx, 300.0, f32::INFINITY))
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for (key, does) in ROWS {
                        ui.horizontal(|ui| {
                            // A fixed key column, so the descriptions line up in one edge
                            // rather than stepping in and out with the length of the chord.
                            // The status bar's own unreadability was this same fault.
                            let (rect, _) = ui.allocate_exact_size(
                                egui::vec2(KEY_COL, theme::SB_ROW),
                                egui::Sense::hover(),
                            );
                            ui.painter().text(
                                rect.left_center(),
                                egui::Align2::LEFT_CENTER,
                                key,
                                egui::FontId::new(theme::BODY, theme::MONO),
                                theme::TEXT,
                            );
                            ui.label(
                                egui::RichText::new(*does)
                                    .size(theme::BODY)
                                    .color(theme::TEXT_SECONDARY),
                            );
                        });
                    }
                });

            theme::foot(ui, |ui| {
                ui.label(
                    egui::RichText::new(NO_KEY_FOR)
                        .size(theme::SMALL)
                        .color(theme::TEXT_MUTED),
                );
            });
        });

    if !open {
        app.popup = None;
    }
}

/// Wide enough for the longest chord in [`ROWS`] at 13pt mono, with room to spare.
const KEY_COL: f32 = 218.0;

#[cfg(test)]
mod tests {
    use super::*;

    /// CORE §4's keyboard table and [`ROWS`] are the same list, in the same order.
    ///
    /// This is the whole of "never typed twice" that a `match` statement permits. It reads
    /// the document rather than a fixture, so an edit to CORE §4 that forgets this popup
    /// fails here — which is the direction drift actually travels: the document moves first
    /// in this project, and the code follows.
    #[test]
    fn the_popup_and_core_agree_about_the_keys() {
        let core = include_str!("../../CORE.md");

        // The one table under "### Keyboard", up to the blank line that ends it.
        let after = core
            .split_once("### Keyboard")
            .expect("CORE §4 has a Keyboard heading")
            .1;
        let rows: Vec<(String, String)> = after
            .lines()
            .skip_while(|l| !l.starts_with('|'))
            .take_while(|l| l.starts_with('|'))
            .filter(|l| !l.contains("---"))
            .map(|l| {
                let cells: Vec<&str> = l.trim().trim_matches('|').split('|').collect();
                (clean(cells[0]), clean(cells[1]))
            })
            .filter(|(k, _)| k != "Key")
            .collect();

        assert_eq!(
            rows.len(),
            ROWS.len(),
            "CORE §4 lists {} keys and the popup draws {}",
            rows.len(),
            ROWS.len()
        );
        for (i, ((ck, cd), (pk, pd))) in rows.iter().zip(ROWS.iter()).enumerate() {
            assert_eq!(
                ck, pk,
                "row {i}: CORE says key {ck:?}, the popup says {pk:?}"
            );
            // CORE's cell may carry a longer explanation than a popup row wants to draw, so
            // the popup's text must be a prefix of the document's rather than equal to it.
            assert!(
                cd.starts_with(*pd),
                "row {i} ({ck}): CORE says {cd:?}, which does not begin with the popup's {pd:?}"
            );
        }
    }

    /// Strip the document's markup so a cell compares as the words a person reads.
    fn clean(cell: &str) -> String {
        cell.replace(['`', '*'], "")
            .replace("\\|", "|")
            .trim()
            .to_string()
    }
}
