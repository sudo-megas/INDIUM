//! The About popup — CORE §4.6.
//!
//! "The mark, the maker, the version and date, the source address and the licence in
//! full. Addresses are text you can select but not click — INDIUM opens no browser
//! and follows no link, by design." (CORE §9: "The app never opens a URL".)

use eframe::egui;

use super::{Indium, Popup};
use crate::theme;

const SOURCE: &str = "https://github.com/sudo-megas/INDIUM";
/// Typed once, and read back out of `changelog.Debian`'s newest stanza by
/// `the_date_about_prints_is_the_one_the_changelog_stamped`, so it cannot drift: the
/// release that moves the changelog and forgets this line fails the build instead of
/// shipping a date from a previous tag. **It was stale across three of them** — this popup
/// printed `2026-08-10` beside a version reading `1.2.0`, because nothing in the repository
/// named the constant and the comment that used to sit here said it was updated at each tag
/// by hand, which the history flatly contradicts.
///
/// Deliberately not a build-time timestamp: CORE §8 ships this as a package, and a binary
/// that embeds the minute it was compiled cannot be built twice into the same bytes. A
/// constant is deterministic and is exactly how `LICENCE` below is already embedded.
///
/// The test is named in prose rather than linked: it lives in this file's `#[cfg(test)]`
/// module, which `cargo doc` does not compile, so an intra-doc link to it resolves to
/// nothing the first time a docs job is added.
const RELEASE_DATE: &str = "2026-08-12";
const LICENCE: &str = include_str!("../../LICENSE");

pub fn show(app: &mut Indium, ctx: &egui::Context) {
    if app.popup != Some(Popup::About) {
        return;
    }
    let mut open = true;
    egui::Window::new("About INDIUM")
        .max_height(theme::popup_max_height(ctx))
        .collapsible(false)
        .resizable(true)
        .default_size([620.0, 480.0])
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .open(&mut open)
        .show(ctx, |ui| {
            // About is the one popup whose fixed part alone — a 150px mark, a title, five
            // fields and a sourcing note — is taller than the window's own minimum height,
            // so no arithmetic over what is *left* can make it fit. The whole body scrolls
            // instead, and only the foot is held out of it: a popup may lose the bottom of
            // the licence to a small window, but it must never lose the line naming what it
            // is and who made it.
            egui::ScrollArea::vertical()
                .max_height(theme::list_height(ctx, 120.0, f32::INFINITY))
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    // "The mark, the maker, the version and date" — CORE §4.6, in that order. The
                    // mark was named in the document from P1 and drawn here for the first time in P6.
                    ui.vertical_centered(|ui| {
                        ui.add_space(4.0);
                        ui.add(theme::mark(150.0));
                        ui.add_space(10.0);
                        ui.label(
                            egui::RichText::new("INDIUM")
                                .size(28.0)
                                .color(theme::TEXT)
                                .family(theme::bold()),
                        );
                        ui.label(
                            egui::RichText::new(
                                "The Most Verbose Archive Manager for Linux on Wayland.",
                            )
                            .color(theme::TEXT_SECONDARY),
                        );
                    });
                    ui.add_space(14.0);

                    egui::Grid::new("about-grid")
                        .num_columns(2)
                        .spacing([14.0, 4.0])
                        .show(ui, |ui| {
                            field(ui, "Version", env!("CARGO_PKG_VERSION"));
                            // CORE §4.6 asks for "the version and date", and the date was missing
                            // from this grid from P1 until P6.
                            field(ui, "Date", RELEASE_DATE);
                            field(ui, "Maker", "sudo-megas");
                            field(ui, "Licence", "GPL-3.0-only");
                            ui.label(egui::RichText::new("Source").color(theme::TEXT_MUTED));
                            // Selectable, never clickable.
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(SOURCE)
                                        .family(theme::MONO)
                                        .color(theme::TEXT),
                                )
                                .selectable(true),
                            );
                            ui.end_row();
                        });

                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new(
                            "Addresses here are text you can select and copy. INDIUM opens no \
                             browser and follows no link.",
                        )
                        .size(13.0)
                        .color(theme::TEXT_MUTED),
                    );

                    ui.add_space(10.0);
                    // The separator stays and belongs to the popup, not to the heading: it divides
                    // "who made this" from "what it is licensed under". The heading itself is
                    // `section_bare`, because CORE's rule is that a heading takes a rule when it
                    // opens a *list of siblings* and none when it names a single object — and the
                    // licence is one document, not a list. The centred "INDIUM" above is a value
                    // rather than a heading, so it keeps its own size and is left alone. P7 §4.
                    ui.separator();
                    theme::section_bare(ui, "The licence, in full");

                    // No scroll area of its own any more: the body above is already one, and
                    // a licence that scrolls inside a popup that scrolls is two wheels under
                    // one finger.
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(LICENCE)
                                .family(theme::MONO)
                                .size(13.0)
                                .color(theme::TEXT_SECONDARY),
                        )
                        .selectable(true),
                    );
                });

            ui.add_space(8.0);
            theme::foot(ui, |ui| {
                ui.label(
                    egui::RichText::new("Copyright © sudo-megas · Built with Reason and Passion.")
                        .size(13.0)
                        .color(theme::TEXT_MUTED),
                );
            });
        });

    if !open {
        app.popup = None;
    }
}

fn field(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.label(egui::RichText::new(label).color(theme::TEXT_MUTED));
    ui.label(
        egui::RichText::new(value)
            .family(theme::MONO)
            .color(theme::TEXT),
    );
    ui.end_row();
}

#[cfg(test)]
mod tests {
    use super::RELEASE_DATE;

    /// The packaging changelog, which the release ritual writes a stanza into at every tag.
    ///
    /// Read here rather than in `build/`, and behind `#[cfg(test)]` so none of it reaches the
    /// shipped binary — the same arrangement `keys.rs` uses for `CORE.md`.
    const CHANGELOG: &str = include_str!("../../build/package/deb/changelog.Debian");

    /// The newest stanza, and only that one.
    ///
    /// Stanzas begin at column 0 with `indium (`; a body bullet is indented. Splitting on the
    /// *second* occurrence rather than scanning for a prefix means nothing further down the file
    /// can ever be matched by accident — the version this asserts against is the top one or the
    /// parse fails.
    fn top_stanza() -> &'static str {
        let rest = CHANGELOG
            .strip_prefix("indium (")
            .expect("changelog.Debian begins with an `indium (` stanza");
        match rest.split_once("\nindium (") {
            Some((first, _)) => first,
            None => rest,
        }
    }

    /// `1.2.0-1` → `("1.2.0", "1")`.
    fn top_version() -> (&'static str, &'static str) {
        let full = top_stanza()
            .split_once(')')
            .expect("the top stanza's version is parenthesised")
            .0;
        full.rsplit_once('-')
            .unwrap_or_else(|| panic!("the top stanza's version {full:?} carries no -revision"))
    }

    /// CORE §4.6 asks About for "the version and date". Both come from the stanza the release
    /// ritual writes, so the two fields cannot disagree with the package they ship inside.
    ///
    /// It was stale across three tags before P18 read it: About said 2026-08-10 beside a version
    /// that read 1.2.0, and nothing in the repository named the constant at all.
    #[test]
    fn the_date_about_prints_is_the_one_the_changelog_stamped() {
        let trailer = top_stanza()
            .lines()
            .find(|l| l.starts_with(" -- "))
            .expect("the top stanza ends with a ` -- maintainer  date` trailer");

        // Debian policy separates the maintainer from the date by exactly two spaces, which is
        // the only reliable split: the address contains one space, the date contains four.
        // Trailing whitespace is stripped first — `rsplit_once` takes the *last* double space,
        // so two spaces left at the end of the line would otherwise hand back an empty date and
        // blame the format for what is a stray keystroke.
        let stamp = trailer
            .trim_end()
            .rsplit_once("  ")
            .expect("the trailer separates maintainer and date by two spaces")
            .1;

        let parts: Vec<&str> = stamp.split_whitespace().collect();
        assert!(
            parts.len() == 6,
            "the trailer's date {stamp:?} is not `Day, DD Mon YYYY HH:MM:SS +ZZZZ`"
        );
        let (day, month, year) = (parts[1], parts[2], parts[3]);

        // A month this table does not know is a FAIL and never a skip: it means the trailer has
        // stopped being the shape this test reads, which is exactly when it must speak up.
        const MONTHS: [(&str, &str); 12] = [
            ("Jan", "01"),
            ("Feb", "02"),
            ("Mar", "03"),
            ("Apr", "04"),
            ("May", "05"),
            ("Jun", "06"),
            ("Jul", "07"),
            ("Aug", "08"),
            ("Sep", "09"),
            ("Oct", "10"),
            ("Nov", "11"),
            ("Dec", "12"),
        ];
        let num = MONTHS
            .iter()
            .find(|(name, _)| *name == month)
            .unwrap_or_else(|| {
                panic!("the changelog names a month this test does not know: {month:?}")
            })
            .1;

        let want = format!("{year}-{num}-{day:0>2}");
        assert_eq!(
            RELEASE_DATE, want,
            "About prints {RELEASE_DATE}, and the changelog's newest stanza is stamped {want}"
        );
    }

    /// The other half of the same grid. `verify.sh` asserts this at package time; asserting it
    /// here puts it in front of every push, which is the lesson P15 and P17 both paid for.
    #[test]
    fn the_version_about_prints_is_the_one_the_changelog_declares() {
        let (upstream, revision) = top_version();
        assert_eq!(
            env!("CARGO_PKG_VERSION"),
            upstream,
            "About prints the Cargo version and the changelog's newest stanza declares {upstream}"
        );
        assert!(
            !revision.is_empty() && revision.bytes().all(|b| b.is_ascii_digit()),
            "the top stanza's package revision {revision:?} is not a number"
        );
    }
}
