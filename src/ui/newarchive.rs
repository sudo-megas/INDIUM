//! Create — CORE §4's first popup, `N`, built by P4 §5.
//!
//! CORE §4.1: "An instruction line at top. Four preset chips — *Fastest*, *Balanced*
//! (default), *Smallest*, *Encrypted* — each highlighting a row in the method list below,
//! where **every method carries its one-sentence verdict** (§5). An *Advanced* disclosure
//! holds the level slider. At the foot, a live sentence states exactly what will be
//! built."
//!
//! **The method list carries its verdicts and nothing else.** P21 wrote the estimator's
//! figures into a lane on each row and P21b took them back out again: they were 11 px in
//! `TEXT_MUTED` beside a sentence that had already claimed the width, which is the least
//! readable place in the popup for the one thing the round was about. Measure now opens a
//! popup of its own — §4.10, [`super::measure`] — and this file is what it was before.
//!
//! Copyright © sudo-megas. GPL-3.0-only.

use std::path::PathBuf;

use eframe::egui;

use super::{extract, Indium, Popup};
use crate::tasks::{self, Method, Preset, Recipe, METHODS};
use crate::theme;

/// CORE §4.1's instruction line, verbatim.
const INSTRUCTION: &str = "Choose how INDIUM should compress. If unsure, keep the default.";

pub fn show(app: &mut Indium, ctx: &egui::Context) {
    if app.popup != Some(Popup::Create) {
        return;
    }
    let mut open = true;
    let mut create = false;
    // Set inside the window body and acted on after it, for the same reason `create` is:
    // spawning the worker needs `&mut self` methods the closure cannot hold.
    let mut measure = false;

    theme::floating(ctx, "Create")
        .max_height(theme::popup_max_height(ctx))
        .collapsible(false)
        .resizable(false)
        .open(&mut open)
        .show(ctx, |ui| {
            ui.set_min_width(620.0);

            ui.label(
                egui::RichText::new(INSTRUCTION)
                    .size(theme::BODY)
                    .color(theme::TEXT_SECONDARY),
            );
            ui.add_space(8.0);

            // --- name and destination ---
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Name")
                        .size(theme::SMALL)
                        .color(theme::TEXT_MUTED),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut app.new_name)
                        .font(egui::TextStyle::Monospace)
                        .desired_width(220.0),
                );
                ui.label(
                    egui::RichText::new(extension_for(app.new_method, app.new_encrypt))
                        .family(theme::MONO)
                        .size(theme::BODY)
                        .color(theme::TEXT_MUTED),
                );
            });
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("In")
                        .size(theme::SMALL)
                        .color(theme::TEXT_MUTED),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut app.new_dir)
                        .font(egui::TextStyle::Monospace)
                        .desired_width(430.0),
                );
            });
            // The same completion the Extract popover uses, rather than a second one.
            if let Some(completed) = extract::complete_path(&app.new_dir) {
                if completed != app.new_dir {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("Tab ->")
                                .size(theme::SMALL)
                                .color(theme::TEXT_MUTED),
                        );
                        ui.label(
                            egui::RichText::new(&completed)
                                .family(theme::MONO)
                                .size(theme::SMALL)
                                .color(theme::TEXT_MUTED),
                        );
                    });
                    if ui.input(|i| i.key_pressed(egui::Key::Tab)) {
                        app.new_dir = completed;
                    }
                }
            }

            // No `add_space` before either heading any more: `theme::section` carries
            // `SECTION_ABOVE` itself, so the gap is declared in `theme.rs` rather than
            // hand-tuned here. P7 §1.
            theme::section(ui, "PRESET");
            ui.horizontal_wrapped(|ui| {
                // The method rows below already paint Aubergine by hand for this same
                // "which one is chosen" meaning; the chips painted orange for it. Two
                // colours, one meaning, one popup. P6 §6.6.
                theme::active_fill(ui);
                for preset in [
                    Preset::Fastest,
                    Preset::Balanced,
                    Preset::Smallest,
                    Preset::Encrypted,
                ] {
                    let selected = app.new_preset == preset;
                    // Aubergine sits 1.72:1 against the panel, so the fill cannot be the
                    // only cue. The ink carries it too, as the Inspector tabs already do.
                    let label = egui::RichText::new(preset.label()).color(if selected {
                        theme::TEXT
                    } else {
                        theme::TEXT_MUTED
                    });
                    if ui.selectable_label(selected, label).clicked() {
                        app.new_preset = preset;
                        let (method, encrypt) = preset.recipe_parts();
                        app.new_method = method;
                        app.new_encrypt = encrypt;
                        app.new_level = method.default_level();
                    }
                }
            });

            // `theme::section` unrolled, so Measure can sit on the heading's own line.
            // A row of its own would have cost the method list a row of height at every
            // display scale, and `list_height`'s chrome figure with it; the heading line
            // was already there and had nothing on its right.
            ui.add_space(theme::SECTION_ABOVE);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("METHOD")
                        .size(theme::SECTION)
                        .family(theme::bold())
                        .color(theme::TEXT),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // CORE §7's V2.0, and the only control in the window that spends
                    // CPU on being asked. It is a button rather than something the
                    // popup does on opening because the eight candidates run in
                    // sequence — CORE §3 has one worker — and three seconds is a great
                    // deal to spend on a question nobody asked.
                    //
                    // Since P21b it opens §4.10's popup and the figures are drawn there.
                    // It stays live **while a run is in flight**, so a person who pressed
                    // `Esc` two seconds in can get back to the table the run is still
                    // filling; it is dead only when there is nothing to measure at all.
                    let refusal = app.estimate_refusal();
                    let label = if app.estimate_running {
                        "Measuring…"
                    } else {
                        "Measure"
                    };
                    if theme::button(ui, egui::RichText::new(label), refusal.is_none()).clicked() {
                        measure = true;
                    }
                    // Why it is dead, beside it. Drawn rather than pushed to the status
                    // bar on the click, because the click never comes: a disabled button
                    // reports none, so a sentence waiting for one would never be read by
                    // the one person who needs it.
                    if let Some(why) = refusal {
                        ui.label(
                            egui::RichText::new(why)
                                .size(theme::SMALL)
                                .color(theme::TEXT_MUTED),
                        );
                    }
                });
            });
            ui.add_space(3.0);
            ui.add(egui::Separator::default().horizontal().spacing(6.0));

            egui::ScrollArea::vertical()
                .max_height(theme::list_height(ctx, 452.0, 210.0))
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for method in METHODS {
                        method_row(ui, app, method);
                    }
                });

            ui.add_space(6.0);
            // egui paints this triangle as a vector polygon rather than a glyph, so it
            // does not depend on the embedded face carrying one. `show_background` is
            // not optional under this theme: without it the header paints nothing at all
            // and reads as a static label with no hint that it can be clicked.
            let advanced = egui::CollapsingHeader::new("Advanced")
                .id_salt("new-archive-advanced")
                .default_open(app.new_advanced)
                .show_background(true)
                .show(ui, |ui| {
                    match app.new_method.levels() {
                        None => {
                            ui.label(
                                egui::RichText::new("Store has no level to choose.")
                                    .size(theme::SMALL)
                                    .italics()
                                    .color(theme::TEXT_MUTED),
                            );
                        }
                        Some(range) => {
                            // No local visuals patch here any more: P4 darkened the rail
                            // in this one popup because it was PANEL on PANEL, and P5
                            // fixed that in `theme.rs` where it belonged. `trailing_fill`
                            // stays off, though — it paints the selection colour, and
                            // "how far along a slider is" is not one of the three
                            // meanings CORE §6 reserves orange for.
                            ui.add(
                                egui::Slider::new(&mut app.new_level, range.clone())
                                    .text("Level")
                                    .clamping(egui::SliderClamping::Always),
                            );
                            ui.label(
                                egui::RichText::new(format!(
                                    "{} accepts {} to {}.",
                                    app.new_method.label(),
                                    range.start(),
                                    range.end()
                                ))
                                .size(theme::SMALL)
                                .italics()
                                .color(theme::TEXT_MUTED),
                            );
                        }
                    }
                });
            app.new_advanced = advanced.openness > 0.0;

            ui.add_space(10.0);

            // The separator that used to divide these off is gone: the foot band is the
            // division, and a hairline drawn on top of a change of ground is a rule that
            // separates nothing from nothing.
            theme::foot(ui, |ui| {
                // CORE §4.1's live sentence: "states exactly what will be built".
                let recipe = recipe_of(app);
                ui.label(
                    egui::RichText::new(tasks::recipe_sentence(&recipe))
                        .family(theme::MONO)
                        .size(theme::BODY)
                        .color(theme::TEXT_SECONDARY),
                );
                if app.new_encrypt {
                    ui.label(
                        egui::RichText::new(
                            "You will be asked for the password when you Apply, and again to \
                             confirm it. INDIUM never stores it.",
                        )
                        .size(theme::SMALL)
                        .italics()
                        .color(theme::TEXT_MUTED),
                    );
                }

                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let refusal = create_refusal_for(
                        app.draft.is_empty(),
                        app.new_name.trim().is_empty(),
                        app.new_dir.trim().is_empty(),
                    );
                    // Orange while it can be pressed, and the helper's muted ghost when it
                    // cannot: CORE §6 gives orange to "something *will* happen", and a Create
                    // over an empty draft is exactly the case where nothing will. Orange as
                    // the *fill* since P23 §2f — as ink on this button's own resting face it
                    // measured 3.65:1, and the sentence above was true of a control nobody
                    // could read it on.
                    create =
                        theme::button_primary(ui, egui::RichText::new("Create"), refusal.is_none())
                            .clicked();
                    // Why it is dead, beside it — the same discipline Measure has followed
                    // since P21b, and for the same reason: a disabled button reports no
                    // click, so a sentence waiting for one never reaches the person who
                    // needs it.
                    ui.label(
                        egui::RichText::new(
                            refusal.unwrap_or("Nothing is written until you Apply · Esc closes"),
                        )
                        .size(theme::SMALL)
                        .color(theme::TEXT_MUTED),
                    );
                });
            });
        });

    if measure {
        app.over = Some(Popup::Measure);
        // E1: it runs when it opens — but only when there is nothing to show. Re-opening a
        // popup whose figures are still good must not spend three seconds of CPU proving
        // they were right; that is what the popup's own *Measure again* is for, and it is
        // also what stops a click landing on an in-flight run and starting a second one.
        if !app.holds_estimate() {
            app.request_estimate(ctx);
        }
    }

    if create {
        let recipe = recipe_of(app);
        app.stage_creation(recipe);
        // No `set_window_title` here any more. A staged creation used to adopt an archive
        // that did not exist yet and the title named it; P22 adopts nothing, so the title
        // goes on naming whatever is actually open — which is the truth it was invented to
        // tell.
        // The second of the popup's two close paths, and the reason `cancel_estimate` is
        // called here as well as in `close_popup`: leaving `popup` assigned by hand skips
        // everything that one does, and a measurement outliving its popup is three seconds
        // of CPU spent on figures with nowhere left to appear.
        app.cancel_estimate();
        app.popup = None;
    }
    if !open {
        app.close_popup();
    }
}

/// One method, with its CORE §5 verdict underneath.
///
/// Through `theme::row`, which reads its own `Response`. The hand-rolled version painted
/// its fill from `app.new_method` alone, so eight rows sat there taking the pointer and
/// answering nothing — and the unselected fill was `PANEL`, which used to be the popup's
/// own ground, so an unselected row was a box you could not see. P7 §2.
fn method_row(ui: &mut egui::Ui, app: &mut Indium, method: Method) {
    let selected = app.new_method == method;

    let response = theme::row(ui, selected, egui::Margin::symmetric(8, 5), |ui| {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(method.label())
                        .family(theme::bold())
                        .size(theme::SECTION)
                        .color(theme::TEXT),
                );
                if method == Method::Lzma2 {
                    // The selected row's ground is Aubergine, where TEXT_MUTED measures
                    // 3.30:1 — under AA, and unmeasured until P18. One tier up is 5.01:1.
                    ui.label(
                        egui::RichText::new("AES-256")
                            .family(theme::MONO)
                            .size(theme::SMALL)
                            .color(if selected {
                                theme::TEXT_SECONDARY
                            } else {
                                theme::TEXT_MUTED
                            }),
                    );
                }
            });
            // Verbatim from CORE §5, held once in `tasks` and pinned by a test.
            ui.label(
                egui::RichText::new(method.verdict())
                    .size(theme::SMALL)
                    .italics()
                    .color(theme::TEXT_SECONDARY),
            );
        });
    });

    if response.clicked() {
        choose_method(app, method);
    }
}

/// Choose a method, and everything that follows from choosing it.
///
/// Four assignments, not one, and they are shared rather than copied because P21b gave the
/// program a **second** place a method can be picked from — §4.10's Measure popup, where a
/// row is clicked for the same reason it is clicked here. Two copies of this would have gone
/// stale in the usual way: the encryption drop and the preset recomputation are the two a
/// second copy forgets, and forgetting them leaves a lit chip describing a recipe nobody
/// chose and AES-256 riding on a container that cannot carry it.
pub(super) fn choose_method(app: &mut Indium, method: Method) {
    app.new_method = method;
    app.new_level = method.default_level();
    // Encryption belongs to 7z alone (CORE §9), so choosing anything else drops it.
    if method != Method::Lzma2 {
        app.new_encrypt = false;
    }
    app.new_preset = preset_for(method, app.new_encrypt);
}

/// Which preset a hand-picked method corresponds to, so the chips stay honest about what
/// is selected instead of showing a stale highlight.
fn preset_for(method: Method, encrypt: bool) -> Preset {
    for preset in [
        Preset::Fastest,
        Preset::Balanced,
        Preset::Smallest,
        Preset::Encrypted,
    ] {
        if preset.recipe_parts() == (method, encrypt) {
            return preset;
        }
    }
    // No chip claims this combination; Balanced stays lit as the default, and the method
    // list is where the truth is shown.
    Preset::Balanced
}

/// The Name field's contents for a recipe already staged — P21.
///
/// `Path::file_stem` is wrong here, and quietly: it takes `photos.tar.gz` to `photos.tar`,
/// so re-opening the popup over a staged creation would offer to build
/// `photos.tar.tar.gz`. Only the table below knows that `.tar.gz` is *one* extension, so
/// the split goes through it and not through `Path`.
pub(super) fn stem_of(path: &std::path::Path, method: Method, encrypt: bool) -> String {
    let file = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let ext = extension_for(method, encrypt);
    file.strip_suffix(ext).unwrap_or(&file).to_string()
}

/// The extension the chosen method implies.
fn extension_for(method: Method, _encrypt: bool) -> &'static str {
    match method {
        Method::Store => ".tar",
        Method::Gzip => ".tar.gz",
        Method::Bzip2 => ".tar.bz2",
        Method::Xz => ".tar.xz",
        Method::Zstd => ".tar.zst",
        Method::Lz4 => ".tar.lz4",
        Method::Deflate => ".zip",
        Method::Lzma2 => ".7z",
    }
}

fn recipe_of(app: &Indium) -> Recipe {
    let name = app.new_name.trim();
    let file = format!("{name}{}", extension_for(app.new_method, app.new_encrypt));
    Recipe {
        path: PathBuf::from(extract::expand_tilde(app.new_dir.trim())).join(file),
        method: app.new_method,
        level: app.new_method.clamp_level(app.new_level),
        encrypt: app.new_encrypt && app.new_method == Method::Lzma2,
    }
}

impl Indium {
    /// Project the draft onto the queue, and say what that displaced.
    ///
    /// P4 §1: Create writes nothing. **P22: it also adopts nothing.** Until this round the
    /// window took the chosen path right here — `archive_path`, the title, an emptied entry
    /// list — which made `has_archive()` answer true for a file that had never been written
    /// and put a phantom behind fourteen call sites. The archive that is open now stays open
    /// and stays itself; what appears is a queue and a tray, and the window opens the result
    /// once Apply has written it.
    ///
    /// There is no re-staging branch either. P21 needed one because the queue held the file
    /// list and a second Create would have thrown it away; the draft holds it now, and
    /// [`crate::tasks::Draft::project_onto`] rebuilds the queue from empty every press. That
    /// is strictly stronger than keeping the adds: a file dropped from the draft between two
    /// presses is dropped from the queue with it.
    pub fn stage_creation(&mut self, recipe: Recipe) {
        let name = recipe
            .path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        // Read before the projection, because it is what the projection is displacing.
        let open = self
            .archive_path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string());

        let displaced = self.draft.project_onto(&mut self.tasks, recipe);
        // The queue no longer describes the archive that is open, so the listing it was
        // staged against is not the listing Apply should refuse over.
        self.staged_against.clear();
        // The draft is what the queue now describes, so it is what to be looking at.
        self.section = super::Section::Draft;

        self.status = if displaced == 0 {
            format!("Staged: create {name}. Apply to build it.").into()
        } else {
            super::discarded_line(
                &format!("Staged: create {name}"),
                displaced,
                open.as_deref(),
            )
            .into()
        };
    }
}

/// Why *Create* cannot be pressed, or `None` if it can.
///
/// The draft comes first because it is the round's whole subject, and because it is the one
/// a person fixes by leaving the popup: the name and the directory are seeded when the popup
/// opens, so an empty draft is the refusal this will actually show.
///
/// Free rather than a method, for [`crate::ui::estimate_refusal_for`]'s reason — `Indium` cannot
/// be built in a unit test, and a gate written as a method on it is held by eye.
fn create_refusal_for(empty_draft: bool, no_name: bool, no_dir: bool) -> Option<&'static str> {
    if empty_draft {
        return Some("The draft is empty — add files to it first.");
    }
    if no_name {
        return Some("Give it a name.");
    }
    if no_dir {
        return Some("Say which directory it goes in.");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// CORE §4.1: *"The popup's own button is dead, with a sentence, while the draft is
    /// empty."*
    ///
    /// The draft is asked before the name and the directory because it is the refusal this
    /// will actually show — `open_create` seeds both fields before the popup is drawn, so an
    /// empty draft is the ordinary way to arrive here with nothing to build. It is also the
    /// only one of the three you leave the popup to fix.
    #[test]
    fn create_is_dead_over_an_empty_draft_and_says_which() {
        // empty_draft, no_name, no_dir
        assert_eq!(create_refusal_for(false, false, false), None);

        assert_eq!(
            create_refusal_for(true, false, false),
            Some("The draft is empty — add files to it first.")
        );
        assert_eq!(
            create_refusal_for(true, true, true),
            Some("The draft is empty — add files to it first."),
            "with everything missing, the draft is still what to say"
        );
        assert!(create_refusal_for(false, true, false)
            .expect("a nameless archive cannot be created")
            .contains("name"));
        assert!(create_refusal_for(false, false, true)
            .expect("an archive with nowhere to go cannot be created")
            .contains("directory"));
    }

    /// The trap P21 walked into on paper before it walked into it in code.
    ///
    /// Re-opening the popup over a staged creation has to put the *stem* back in the Name
    /// field, and every compressed tar carries two dots. `Path::file_stem` removes one of
    /// them, so the popup would re-stage `photos.tar` + `.tar.gz` and offer to build
    /// `photos.tar.tar.gz` — a name nobody typed, from a field nobody edited.
    #[test]
    fn a_reopened_new_archive_shows_the_name_that_was_staged() {
        let cases = [
            ("/home/m/photos.tar.gz", Method::Gzip, "photos"),
            ("/home/m/photos.tar.zst", Method::Zstd, "photos"),
            ("/home/m/photos.tar.bz2", Method::Bzip2, "photos"),
            ("/home/m/photos.tar.xz", Method::Xz, "photos"),
            ("/home/m/photos.tar", Method::Store, "photos"),
            ("/home/m/backup.7z", Method::Lzma2, "backup"),
            ("/home/m/backup.zip", Method::Deflate, "backup"),
            // Dots inside the stem are the user's, and stay.
            ("/home/m/site.v2.tar.gz", Method::Gzip, "site.v2"),
        ];
        for (path, method, want) in cases {
            assert_eq!(
                stem_of(std::path::Path::new(path), method, false),
                want,
                "{path}"
            );
        }
    }

    /// Every method's own extension round-trips, so no method can be added with a suffix
    /// this split does not undo.
    #[test]
    fn every_extension_the_popup_writes_is_one_the_name_field_can_take_back() {
        for method in crate::tasks::METHODS {
            let file = format!("archive{}", extension_for(method, false));
            assert_eq!(
                stem_of(std::path::Path::new(&file), method, false),
                "archive",
                "{} does not round-trip",
                method.label()
            );
        }
    }

    /// A name whose stem happens to end in the extension is still only stripped once.
    #[test]
    fn only_the_trailing_extension_is_taken_off() {
        assert_eq!(
            stem_of(
                std::path::Path::new("/x/archive.tar.gz.tar.gz"),
                Method::Gzip,
                false
            ),
            "archive.tar.gz"
        );
    }

    /// The METHOD line holds three things, and P23 §2c spent most of the room it had.
    ///
    /// The heading, the refusal sentence and the *Measure* button share one `ui.horizontal`
    /// — the comment at the top of that row explains why they were folded onto one line —
    /// and the type scale moved two of the three: the heading 14 → [`theme::SECTION`] and the
    /// sentence 11 → [`theme::SMALL`]. **Measured at the popup's real width, that took the
    /// clearance between heading and sentence from 62.0px to 18.9px** — the round spent 70%
    /// of the margin, and the state it is tightest in is the encrypted one, which no
    /// screenshot in §2c reached. Nothing failed, and nothing was pinned either.
    ///
    /// **The failure this guards is an overrun, not a wrap**, and that is worth stating
    /// because the obvious assumption is wrong: `Ui::wrap_mode` returns `Extend` for a
    /// horizontal layout that is not `main_wrap` (egui-0.36.1 `ui.rs:588-605`), so a
    /// sentence with no room does not take a second line — it keeps going left, out of the
    /// popup. Lengthened by eighteen characters it starts at **−71.3**, past the window's
    /// own edge, with nothing on screen to say it was cut. So the left edge is the
    /// assertion that carries this test; the height one below it is the leg that catches
    /// the row being changed to a wrapping layout, where the overflow would go the other
    /// way and push the method list down instead.
    ///
    /// The sentences come from [`crate::ui::estimate_refusal_for`] rather than being quoted,
    /// so a reword is measured the day it is written.
    #[test]
    fn the_method_line_holds_heading_sentence_and_button_on_one_line() {
        let ctx = egui::Context::default();
        theme::install(&ctx);

        // The three sentences the button can be dead behind, in the order §4.1 produces
        // them: nothing staged and nothing open, an archive not yet read, an encrypted one.
        let sentences = [
            crate::ui::estimate_refusal_for(false, false, false, false),
            crate::ui::estimate_refusal_for(false, true, true, false),
            crate::ui::estimate_refusal_for(false, true, false, true),
        ];

        for why in sentences {
            let why = why.expect("each of these three states refuses to measure");
            let mut heading = egui::Rect::NOTHING;
            let mut sentence = egui::Rect::NOTHING;
            // What one line of `SMALL` is, measured in the same frame by the same face
            // rather than asserted as a number. Drawn after the row, so it cannot move it.
            let mut line = 0.0f32;
            let mut full = ctx.run_ui(
                egui::RawInput {
                    // 656 leaves the panel's content exactly the 620 `show` sets as the
                    // popup's minimum, and the popup is `resizable(false)`, so 620 is not a
                    // floor the window grows off — it is the width.
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(656.0, 500.0),
                    )),
                    ..Default::default()
                },
                |root| {
                    egui::CentralPanel::default()
                        .frame(theme::zone(theme::PANEL))
                        .show(root, |ui| {
                            ui.set_min_width(620.0);
                            ui.horizontal(|ui| {
                                heading = ui
                                    .label(
                                        egui::RichText::new("METHOD")
                                            .size(theme::SECTION)
                                            .family(theme::bold())
                                            .color(theme::TEXT),
                                    )
                                    .rect;
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        theme::button(ui, egui::RichText::new("Measure"), false);
                                        sentence = ui
                                            .label(
                                                egui::RichText::new(why)
                                                    .size(theme::SMALL)
                                                    .color(theme::TEXT_MUTED),
                                            )
                                            .rect;
                                    },
                                );
                            });
                            line = ui
                                .label(egui::RichText::new("reference").size(theme::SMALL))
                                .rect
                                .height();
                        });
                },
            );
            // `FullOutput` panics on drop with an unapplied delta; nothing here paints it.
            full.textures_delta.clear();

            assert!(
                sentence.left() > heading.right(),
                "\"{why}\" starts at {:.1}, which is not clear of where the METHOD heading \
                 ends ({:.1}). This row extends rather than wraps, so a sentence with no \
                 room runs off the popup's left edge and says nothing about it — either \
                 shorten the sentence, quieten one of the two sizes, or give the row its \
                 own line and pay `list_height` the row of chrome that costs.",
                sentence.left(),
                heading.right()
            );
            assert!(
                sentence.height() <= line + 0.5,
                "\"{why}\" is {:.1}px tall where one line of SMALL is {line:.1}, so this row \
                 now wraps. That is a different failure from the one above and needs a \
                 different fix: the popup is resizable(false) at 620, so a second line \
                 pushes the method list down rather than widening anything.",
                sentence.height()
            );
        }
    }
}
