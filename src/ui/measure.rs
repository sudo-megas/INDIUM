//! Measure — CORE §4's tenth popup, and the first that stands over another.
//!
//! §4.10: "Opened by **Measure** on the New Archive popup, and drawn over it. It holds
//! nothing but the measurements: one row per method, with the level each was built at, the
//! time it took, the size it produced and the ratio that follows. It runs when it opens and
//! keeps its figures for as long as New Archive lives. Clicking a row chooses that method.
//! A `~` marks a figure the sample could not promise."
//!
//! **Why it is a popup of its own.** P21 wrote these figures into a lane on the method row —
//! 11 px in `TEXT_MUTED`, the smallest and dimmest text in the popup, beside a verdict
//! sentence that had already claimed the width. The round's whole payload landed in the least
//! readable element on screen, and the maker said so the first time he saw it. A row cannot be
//! made to hold five columns and a sentence; a surface of its own can hold the five columns at
//! a size worth reading. The method list went back to what it was before P21 touched it.
//!
//! **What holds the columns.** Every row is one monospace string, laid out by
//! [`cells`] against fixed widths — so a column's position is a property of the format and not
//! of the widest value that happened to land in it. CORE §4: "Numbers hold their columns, and
//! do not move as their digits change." A row that has not landed yet, and a method this
//! build of libarchive will not write, are the same width as a measured one.
//!
//! Copyright © sudo-megas. GPL-3.0-only.

use eframe::egui;

use super::{newarchive, EstimateOf, Indium, Popup};
use crate::estimate::Measurement;
use crate::tasks::{Method, METHODS};
use crate::theme;

pub fn show(app: &mut Indium, ctx: &egui::Context) {
    // **Both slots, deliberately.** `over` alone is not enough: sixteen sites assign
    // `app.popup` by hand, the New Archive window is an `egui::Window` rather than a modal, so
    // the sidebar beneath it stays clickable, and `Ctrl+O` fires whatever is open. The frame
    // that swaps the popup underneath is the frame this must not draw on. `ui()` sweeps the
    // stale `over` immediately afterwards; this is the half that keeps the picture honest
    // within the frame itself.
    if app.over != Some(Popup::Measure) || app.popup != Some(Popup::NewArchive) {
        return;
    }

    // Set inside the body and acted on after it, for the same reason `newarchive`'s `measure`
    // is: spawning the worker needs `&mut self` methods the closure cannot hold.
    let mut again = false;
    let mut chose: Option<Method> = None;
    let mut close = false;

    // A `Modal`, following the password prompt — the one other popup in the program that
    // draws over rather than beside. It paints above every `egui::Window` without this file
    // having to reason about `Order`, and it takes the input beneath it, so a click meant for
    // this table cannot land on the method list it is covering. `SCRIM` rather than egui's
    // black, for the reason `password.rs` gives: the window stays in its own family.
    egui::Modal::new(egui::Id::new("measure-popup"))
        .backdrop_color(theme::SCRIM)
        .show(ctx, |ui| {
            // A minimum, not a width. `set_width` fixes both bounds, and a table wider than
            // its bound does not overflow in egui — it *wraps*, which turns a grid into
            // rubble. The floor is low enough that **the table sets the width**: the grid
            // below is wider than this, so the popup is exactly as wide as its own columns
            // and there is no empty lane down the right-hand side for the table to be
            // dwarfed by. Only a sentence longer than the grid can widen it further.
            ui.set_min_width(460.0);
            // A `Modal` draws no title bar, so this label is the title, in the same Heading
            // style every other popup's bar carries.
            ui.label(egui::RichText::new("Measure").heading().color(theme::TEXT));
            ui.add_space(4.0);
            // What the figures rest on, stated above them rather than left to be assumed.
            // This sentence used to sit on the New Archive heading line, beside a button; it
            // belongs with the numbers it describes.
            ui.label(
                egui::RichText::new(statement(app.estimate_of.as_ref(), app.estimate_running))
                    .size(13.0)
                    .color(theme::TEXT_SECONDARY),
            );
            ui.add_space(10.0);

            // The header rides the same grid as the rows: one family, one size, so a column
            // heading stands over its column at every display scale. Two tiers of ink is what
            // tells it from the data, not weight and not size.
            //
            // **And the same left inset**, which is not decoration: `theme::row` frames its
            // content in `Margin::symmetric(8, 5)`, so a header drawn straight onto the popup
            // sits eight points to the *left* of the column it names. At the window that read
            // as every heading being a character adrift of its own numbers.
            egui::Frame::NONE
                .inner_margin(egui::Margin::symmetric(8, 0))
                .show(ui, |ui| {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(header())
                                .family(theme::bold())
                                .size(TABLE_PT)
                                .color(theme::TEXT_MUTED),
                        )
                        .wrap_mode(egui::TextWrapMode::Extend),
                    );
                });
            ui.add_space(2.0);
            ui.add(egui::Separator::default().horizontal().spacing(6.0));

            egui::ScrollArea::vertical()
                .max_height(theme::list_height(ctx, 300.0, 340.0))
                // Vertically it shrinks to what it holds, which costs nothing in stability
                // here: all eight rows stand from the first frame, so the list is the same
                // height before the figures land as after. Refusing to shrink left a hand's
                // width of empty ground between the last method and the foot.
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    // **All eight rows, from the first frame.** Growing the table as figures
                    // land would reflow it eight times and move every row under the pointer
                    // while the user was reading it — motion in all but name, and CORE §6's
                    // fourth dated refusal is against exactly that. The rows stand still and
                    // their cells fill in, which is what "one row at a time as they land"
                    // means when the table has a fixed height.
                    for method in METHODS {
                        method_row(ui, app, method, &mut chose);
                    }
                });

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                // Re-running is the whole of E1's other half: the figures are kept while New
                // Archive lives, so the only way to spend the CPU again is to ask for it.
                let can = !app.estimate_running && app.estimate_refusal().is_none();
                let label = if app.estimate_running {
                    "Measuring…"
                } else {
                    "Measure again"
                };
                if theme::button(ui, egui::RichText::new(label), can).clicked() {
                    again = true;
                }
                ui.label(
                    egui::RichText::new("Click a method to choose it · Esc closes")
                        .size(12.0)
                        .color(theme::TEXT_MUTED),
                );
            });
        });

    if let Some(method) = chose {
        // The same four assignments the method list makes, through the same function, so
        // "which method is chosen" cannot come to mean two different things in two popups.
        newarchive::choose_method(app, method);
        close = true;
    }
    if again {
        app.request_estimate(ctx);
    }
    if close {
        app.over = None;
    }
}

/// One method's row: the cells it has, on the row the method list already paints.
///
/// Selection is drawn exactly as `newarchive::method_row` draws it — `theme::row` on
/// Aubergine — because it means the same thing in both places and the program has one
/// appearance for it.
fn method_row(ui: &mut egui::Ui, app: &Indium, method: Method, chose: &mut Option<Method>) {
    let selected = app.new_method == method;
    let sampled = app.estimate_of.as_ref().is_some_and(|of| of.sampled);
    let measured = app.estimates.iter().find(|m| m.method == method);
    let failed = app.estimate_failed.iter().any(|(m, _)| *m == method);

    let response = theme::row(ui, selected, egui::Margin::symmetric(8, ROW_PAD), |ui| {
        ui.add(
            egui::Label::new(
                egui::RichText::new(row_text(method, measured, failed, sampled))
                    .family(theme::bold())
                    .size(TABLE_PT)
                    .color(if selected {
                        theme::TEXT
                    } else {
                        theme::TEXT_SECONDARY
                    }),
            )
            .wrap_mode(egui::TextWrapMode::Extend),
        );
    });

    if response.clicked() {
        *chose = Some(method);
    }
}

// --- the grid --------------------------------------------------------------
//
// Written out as constants rather than inlined, because the header and the three kinds of row
// have to agree about them and a width typed four times is a width that will drift.

/// The type size of the whole table, header and rows alike.
///
/// **Sixteen, and the number is the point of this popup.** The figures were 11 pt in a lane
/// on a method row when the maker first saw them, which is why they are here at all; putting
/// them on a surface of their own and then setting them at the body size would have moved the
/// problem rather than fixed it. The table is the largest text in the window after a title,
/// and the popup is sized by it rather than the other way round.
const TABLE_PT: f32 = 16.0;

/// The breathing room above and below a row's text.
///
/// Eight rather than five so eight rows fill the height the popup wants to be, instead of
/// standing in the top two-thirds of it with a hand's width of empty ground beneath.
const ROW_PAD: i8 = 8;

const METHOD_W: usize = 8;
const LEVEL_W: usize = 5;
/// Wide enough for `99999 ms` — two orders of magnitude past the slowest candidate a 2 MiB
/// budget has ever produced, which is xz at a little under a second.
const TIME_W: usize = 9;
/// Wide enough for the widest string [`crate::util::format_bytes`] can return.
///
/// That is **ten** characters and not nine, which is the sort of thing only arithmetic finds:
/// the formatter divides until the value is under 1024 and then prints it to one decimal, so
/// `1073741823` — one byte short of a gibibyte — divides to 1023.99998, rounds to `1024.0`,
/// and comes back as `1024.0 MiB`. Nine would have held every figure the estimator can
/// actually produce under its own budget and shifted the ratio column on the one it cannot.
const SIZE_W: usize = 10;
const RATIO_W: usize = 7;
/// The space between two columns.
///
/// Four rather than two, because the popup is the table's and the table should fill it. The
/// alternative was leaving the grid narrow and letting the popup stand wider than its own
/// content, which is the empty lane down the right-hand side the maker circled.
const GAP: &str = "    ";
/// The four numeric columns and the gaps between them, for the one cell that spans them all.
const FIGURES_W: usize = LEVEL_W + TIME_W + SIZE_W + RATIO_W + 3 * GAP.len();
/// Every row this file produces is exactly this many characters wide.
///
/// Nothing in the window needs the number — the grid is enforced by the format strings above,
/// not by measuring what they produce — so it exists for
/// `every_measured_column_holds_its_width`, which is where the rule is actually held.
#[cfg(test)]
const ROW_W: usize = METHOD_W + GAP.len() + FIGURES_W;

/// The five cells, in the grid.
fn cells(method: &str, level: &str, time: &str, size: &str, ratio: &str) -> String {
    format!(
        "{method:<mw$}{GAP}{level:>lw$}{GAP}{time:>tw$}{GAP}{size:>sw$}{GAP}{ratio:>rw$}",
        mw = METHOD_W,
        lw = LEVEL_W,
        tw = TIME_W,
        sw = SIZE_W,
        rw = RATIO_W,
    )
}

/// A method's name, and one word right-aligned across the whole numeric field.
///
/// Both the row that has nothing yet and the row that will never have anything go through
/// here, so a method waiting and a method refused are the same shape and the table does not
/// change width as the answers arrive.
fn spanned(method: &str, word: &str) -> String {
    format!(
        "{method:<mw$}{GAP}{word:>fw$}",
        mw = METHOD_W,
        fw = FIGURES_W,
    )
}

/// The column headings, on the row grid.
pub(super) fn header() -> String {
    cells("METHOD", "LEVEL", "TIME", "SIZE", "RATIO")
}

/// One row, in whichever of its three states it is in.
///
/// **The level is printed on purpose.** The slider goes on moving after a measurement, and a
/// figure that does not say which level it was taken at becomes quietly false the moment it is
/// left behind. Printing it makes a stale row describe itself instead, in the same
/// `method:level` idiom `recipe_sentence` already writes.
///
/// The mark is `~` when the input was sampled and a space when it was not, so the per-cent
/// signs stay in one column either way — CORE §4's "numbers hold their columns" is about the
/// marked case too.
pub(super) fn row_text(
    method: Method,
    measured: Option<&Measurement>,
    failed: bool,
    sampled: bool,
) -> String {
    match measured {
        Some(m) => {
            let level = match m.method.levels() {
                Some(_) => m.level.to_string(),
                // Store has no level to choose, so there is no number here to print.
                None => "—".to_string(),
            };
            cells(
                method.label(),
                &level,
                &format!("{} ms", m.millis),
                &crate::util::format_bytes(m.bytes),
                &ratio_cell(m.ratio(), sampled),
            )
        }
        // A method this build of libarchive will not write is a row that says so rather than a
        // row that quietly disappears. The reason itself goes to the status bar; the table
        // keeps the method and its width.
        None if failed => spanned(method.label(), "unavailable"),
        None => spanned(method.label(), ""),
    }
}

/// The ratio, marked, and bounded so it cannot push its own column sideways.
///
/// **A ratio can genuinely exceed a thousand per cent.** `input_bytes` is the sum of the
/// members' own bytes, and every container writes a fixed frame around them: stage a one-byte
/// file and `Store` produces a 10 KiB tar, which is 1 024 000% — a true figure, and a useless
/// one. It is about a header's fixed cost measured against nothing, not about compression, and
/// its exact digits are noise. Above `999.9%` the cell says it is off the scale instead, which
/// is the honest report *and* the one that fits: `>999%` is bounded where the number is not.
fn ratio_cell(ratio: f32, sampled: bool) -> String {
    let mark = if sampled { '~' } else { ' ' };
    if ratio > 999.9 {
        format!("{mark} >999%")
    } else {
        format!("{mark}{ratio:>5.1}%")
    }
}

/// What the figures were drawn from, in a sentence, above the table.
///
/// A ratio with no statement of what was weighed is the same species of claim as the folklore
/// CORE §7 sent V2.0 to replace, so this is never absent: before the worker has reported what
/// it resolved, the sentence says that instead of saying nothing.
pub(super) fn statement(of: Option<&EstimateOf>, running: bool) -> String {
    match of {
        Some(of) if of.sampled => format!(
            "~ estimated from {} of {}",
            crate::util::format_bytes(of.bytes),
            of.describe
        ),
        Some(of) => format!(
            "measured on all {} of {}",
            crate::util::format_bytes(of.bytes),
            of.describe
        ),
        None if running => "Reading the input…".to_string(),
        None => "Nothing has been measured yet.".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::estimate::Measurement;

    fn m(method: Method, level: u32, millis: u64, bytes: u64, input: u64) -> Measurement {
        Measurement {
            method,
            level,
            millis,
            bytes,
            input_bytes: input,
        }
    }

    /// CORE §4: "Numbers hold their columns, and do not move as their digits change."
    ///
    /// The popup itself cannot be built in a test — `Indium::new` wants an
    /// `eframe::CreationContext` — but the rule it has to obey is a property of these pure
    /// functions, and this is where it is held. Every method, all three states of a row, both
    /// marks, and **every power of two the size column can be handed**, come out one width.
    ///
    /// The size sweep is exhaustive rather than sampled because the case that breaks a column
    /// is not one anybody would think to type: a byte short of a power of two rounds *up* a
    /// unit — `1073741823` prints as `1024.0 MiB` — and that one string is a character wider
    /// than every figure the estimator can produce under its own budget. Three hand-picked
    /// sizes would have missed it and the column would have moved on somebody's screen.
    ///
    /// Time is the one column with a **stated domain** rather than an exhaustive one: `u64`
    /// milliseconds run to six hundred million years, and a cell wide enough for that would be
    /// a cell nine characters of empty. The domain is the estimator's own — it hands each
    /// candidate at most `estimate::BUDGET`, two mebibytes, on which the slowest of the eight
    /// takes a little under a second — and the bound tested here is a hundred times that.
    #[test]
    fn every_measured_column_holds_its_width() {
        assert_eq!(header().chars().count(), ROW_W, "the header");
        for method in METHODS {
            // Waiting, and refused: the two rows that carry no figures.
            for row in [
                row_text(method, None, false, false),
                row_text(method, None, true, false),
            ] {
                assert_eq!(row.chars().count(), ROW_W, "{}: {row:?}", method.label());
            }
            // Every magnitude of size, and its neighbour one byte below — which is the one
            // that rounds up into the next unit's widest form.
            for shift in 0..64u32 {
                let at = 1u64 << shift;
                for bytes in [at, at - 1] {
                    for sampled in [true, false] {
                        // The ratio rides the size: an input fixed at the budget makes the
                        // sweep walk the ratio column from 0.0% to far past its own bound.
                        let one = m(method, 22, 999, bytes, crate::estimate::BUDGET);
                        let row = row_text(method, Some(&one), false, sampled);
                        assert_eq!(
                            row.chars().count(),
                            ROW_W,
                            "{} at {bytes} B: {row:?}",
                            method.label()
                        );
                    }
                }
            }
            // And every magnitude of level and time inside the stated domain.
            for level in [0u32, 1, 9, 22, 99999] {
                for millis in [0u64, 1, 7, 99, 999, 9999, 99999] {
                    let one = m(method, level, millis, 803 * 1024, 2 * 1024 * 1024);
                    let row = row_text(method, Some(&one), false, true);
                    assert_eq!(
                        row.chars().count(),
                        ROW_W,
                        "{} at {level}/{millis} ms: {row:?}",
                        method.label()
                    );
                }
            }
        }
    }

    /// A ratio big enough to break the column says so instead of printing itself.
    ///
    /// Not a hypothetical: `input_bytes` counts the members' own bytes, so one staged byte
    /// against `Store`'s ten-kibibyte tar frame is 1 024 000% — a true figure about a header's
    /// fixed cost and not about compression at all.
    #[test]
    fn a_ratio_too_large_to_be_about_compression_is_told_it_is_off_the_scale() {
        let tiny = m(Method::Store, 0, 2, 10240, 1);
        let row = row_text(Method::Store, Some(&tiny), false, false);
        assert!(row.contains(">999%"), "{row:?}");
        assert_eq!(row.chars().count(), ROW_W, "{row:?}");
        // And one just under the bound still prints its digits.
        assert!(ratio_cell(999.4, false).contains("999.4%"));
    }

    /// A figure the sample could not promise is marked, and one taken whole is not.
    ///
    /// Carried over from `newarchive::figure_of`, which P21b deleted with the lane it fed.
    /// The `~` is the only thing in the popup that distinguishes an estimate from a
    /// measurement, so it is the one character in the row worth its own test.
    #[test]
    fn a_sampled_figure_is_marked_and_an_exact_one_is_not() {
        let one = m(Method::Zstd, 3, 23, 803 * 1024, 2 * 1024 * 1024);
        let sampled = row_text(Method::Zstd, Some(&one), false, true);
        let exact = row_text(Method::Zstd, Some(&one), false, false);

        assert!(sampled.contains('~'), "{sampled:?}");
        assert!(!exact.contains('~'), "{exact:?}");
        // And the two are the same width, so the mark costs the column nothing.
        assert_eq!(sampled.chars().count(), exact.chars().count());
        // The mark sits immediately before the digits it qualifies, not adrift in the gap.
        assert!(sampled.contains("~ 39.2%"), "{sampled:?}");
    }

    /// Store has no level to choose, and says so rather than printing one it does not have.
    #[test]
    fn the_one_method_with_no_level_prints_no_number_for_it() {
        let row = row_text(
            Method::Store,
            Some(&m(Method::Store, 0, 2, 2048, 2048)),
            false,
            false,
        );
        // Read out of the level column itself rather than out of the whole row: the sizes and
        // the ratio carry digits of their own, so "the row has no zero in it" would have been
        // a test of the wrong string.
        let level: String = row
            .chars()
            .skip(METHOD_W + GAP.len())
            .take(LEVEL_W)
            .collect();
        assert_eq!(level, "    —", "{row:?}");
    }

    /// The size column goes through the program's one byte formatter rather than a second
    /// one grown here — the same argument CORE §3 makes about a value having one home.
    #[test]
    fn the_size_column_uses_the_programs_one_byte_formatter() {
        for bytes in [0u64, 1, 1023, 1024, 803 * 1024, 3 * 1024 * 1024 + 512] {
            let row = row_text(
                Method::Gzip,
                Some(&m(Method::Gzip, 6, 118, bytes, 2 * 1024 * 1024)),
                false,
                false,
            );
            assert!(
                row.contains(&crate::util::format_bytes(bytes)),
                "{bytes} B is not written as {:?} in {row:?}",
                crate::util::format_bytes(bytes)
            );
        }
    }

    /// The sentence above the table is never absent, and never claims more than it has.
    #[test]
    fn the_popup_always_states_what_it_weighed() {
        let of = EstimateOf {
            describe: "photos.tar.zst".to_string(),
            sampled: true,
            bytes: 2 * 1024 * 1024,
        };
        assert_eq!(
            statement(Some(&of), false),
            "~ estimated from 2.0 MiB of photos.tar.zst"
        );

        let whole = EstimateOf {
            describe: "12 staged items".to_string(),
            sampled: false,
            bytes: 640 * 1024,
        };
        assert_eq!(
            statement(Some(&whole), false),
            "measured on all 640.0 KiB of 12 staged items"
        );

        // Before the worker has said what it resolved, and after a run that never started.
        assert!(!statement(None, true).is_empty());
        assert!(!statement(None, false).is_empty());
        assert_ne!(statement(None, true), statement(None, false));
    }
}
