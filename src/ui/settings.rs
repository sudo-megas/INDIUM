//! The Settings panel — CORE §4's popup 5, and P2 §3.
//!
//! "Exactly three groups, nothing else, and no room grows later without a CORE edit:
//! Extract, Bookmarks, Recent files."
//!
//! That sentence is P2's, and it was right about this file for seventeen rounds while
//! CORE's own popup 5 named two of the three. The citation above used to read *"CORE
//! §4.5"* — a subsection number CORE has not had since §4 was reorganised, so the one
//! place carrying the correct count pointed at a section that could not confirm it.
//! P19 corrected both ends and pinned them together;
//! [`tests::the_settings_panel_has_the_groups_core_says_it_has`] is what now fails if
//! either moves.
//!
//! CORE §9 has already decided the absent ones: no theme controls, no language
//! controls, no anything-else.

use eframe::egui;

use super::{Indium, Popup};
use crate::platform::picker::PickerFor;
use crate::platform::store::{Bookmark, ExtractDefault};
use crate::theme;

pub fn show(app: &mut Indium, ctx: &egui::Context) {
    if app.popup != Some(Popup::Settings) {
        return;
    }
    let mut open = true;

    egui::Window::new("Settings")
        .max_height(theme::popup_max_height(ctx))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .open(&mut open)
        .show(ctx, |ui| {
            ui.set_min_width(500.0);

            if app.settings_broken {
                ui.label(
                    egui::RichText::new(
                        "settings.toml could not be parsed. INDIUM is running on defaults and \
                         will not overwrite the file.",
                    )
                    .size(theme::BODY)
                    .color(theme::WARNING),
                );
            }

            // --- 1. Extract ---------------------------------------------------
            // No `add_space` before any of the three headings any more: `theme::section`
            // carries `SECTION_ABOVE` itself, so the gap is declared once in `theme.rs`
            // rather than hand-tuned three times here. P7 §1.
            theme::section(ui, "Extract");
            // Set inside the row's closure, applied after it: the change is made to the
            // settings file rather than to this window's copy of it, which needs `app`
            // mutably and cannot have it while the row is drawing.
            let mut changed: Option<ExtractDefault> = None;
            let mut want_preselect = false;
            ui.horizontal(|ui| {
                // Which default is chosen is "this mode is active", not "something will
                // happen". The ink carries it too, because Aubergine alone sits 1.72:1
                // against the panel. P6 §6.6.
                theme::active_fill(ui);
                let cur = app.settings.extract.default;
                let toggle = |ui: &mut egui::Ui, on: bool, text: &str| {
                    let text = egui::RichText::new(text).color(if on {
                        theme::TEXT
                    } else {
                        theme::TEXT_MUTED
                    });
                    ui.selectable_label(on, text).clicked()
                };
                if toggle(ui, cur == ExtractDefault::Here, "here") && cur != ExtractDefault::Here {
                    changed = Some(ExtractDefault::Here);
                }
                if toggle(ui, cur == ExtractDefault::Subdir, "into a subdirectory")
                    && cur != ExtractDefault::Subdir
                {
                    changed = Some(ExtractDefault::Subdir);
                }
                // PXX 8.11. This was the row's *label*, sitting where the row's label goes,
                // and it was read as a button — by the maker, who wrote the rule it was
                // obeying. A word set beside two pressable words, in the same row and at the
                // same size, is a third pressable word no matter what it was meant to be.
                // The fix is to mean it rather than to restyle it into something quieter:
                // the word it was misread as is the word it keeps.
                //
                // **Third, and not first.** Leading the row it would still read as the
                // label it used to be, whatever it now does on a click; the maker asked for
                // it as "an 3rd option" and third is where a third option goes. The group's
                // heading is what names the row now, the way it already does for Bookmarks.
                //
                // It raises the picker on every click, including while it is already the
                // active mode, because that is the only route to changing the directory it
                // points at. The mode itself is not taken up here — that happens when a
                // directory comes back, in `ui::mod`'s `PickerFor::Preselect` arm, since a
                // cancelled dialog must leave the setting exactly as it found it.
                if toggle(ui, cur == ExtractDefault::Preselect, "Preselect") {
                    want_preselect = true;
                }
            });
            // The directory gets its own line. A path is as long as the filesystem is deep
            // and this row does not wrap — P7 §1 — so putting it beside the toggles is how
            // the toggles leave the panel. Shown whenever one is set rather than only while
            // Preselect is lit, because it is what pressing *Preselect* would return to.
            if !app.settings.extract.preselect.is_empty() {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(&app.settings.extract.preselect)
                            .family(theme::MONO)
                            .size(theme::BODY)
                            .color(
                                if app.settings.extract.default == ExtractDefault::Preselect {
                                    theme::TEXT
                                } else {
                                    theme::TEXT_MUTED
                                },
                            ),
                    );
                });
            }
            if let Some(want) = changed {
                app.change_settings(move |s| s.extract.default = want);
                app.extract_to_subdir = app.settings.extract.default == ExtractDefault::Subdir;
            }
            // After the row and never inside it: `request_picker` takes `app` mutably, and
            // the closure above is still holding it while it draws.
            if want_preselect {
                app.request_picker(ctx, PickerFor::Preselect);
            }

            // --- 2. Bookmarks -------------------------------------------------
            theme::section(ui, "Bookmarks");

            let mut remove: Option<usize> = None;
            for (i, b) in app.settings.bookmarks.clone().iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(&b.name)
                            .family(theme::MONO)
                            .size(theme::BODY)
                            .color(theme::TEXT),
                    );
                    ui.label(
                        egui::RichText::new(&b.path)
                            .family(theme::MONO)
                            .size(theme::BODY)
                            .color(theme::TEXT_MUTED),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if theme::small_button(ui, egui::RichText::new(theme::REMOVE), true)
                            .clicked()
                        {
                            remove = Some(i);
                        }
                    });
                });
            }
            // Removed by identity rather than by index: the index came from this
            // window's list, and the change is applied to the file, which another
            // window may have reordered or shortened since.
            if let Some(gone) = remove.and_then(|i| app.settings.bookmarks.get(i).cloned()) {
                app.change_settings(move |s| s.bookmarks.retain(|b| *b != gone));
            }

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut app.bookmark_name)
                        .hint_text("name")
                        .desired_width(130.0),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut app.bookmark_path)
                        .font(egui::TextStyle::Monospace)
                        .hint_text("/path/to/directory")
                        .desired_width(250.0),
                );
                let ready =
                    !app.bookmark_name.trim().is_empty() && !app.bookmark_path.trim().is_empty();
                if theme::button(ui, egui::RichText::new("Add"), ready).clicked() {
                    let added = Bookmark {
                        name: app.bookmark_name.trim().to_string(),
                        path: app.bookmark_path.trim().to_string(),
                    };
                    app.bookmark_name.clear();
                    app.bookmark_path.clear();
                    app.change_settings(move |s| s.bookmarks.push(added));
                }
            });

            // --- 3. Recent files ----------------------------------------------
            theme::section(ui, "Recent files");
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("{} remembered", app.recents.items.len()))
                        .size(theme::BODY)
                        .color(theme::TEXT_MUTED),
                );
                if theme::button(ui, egui::RichText::new("Clear list"), true).clicked() {
                    // Status first, save last, so a refusal or a write error is what the
                    // status bar carries rather than a cheerful line about a file that is
                    // still full of what it always held.
                    app.status = "Recent files cleared.".to_string().into();
                    app.change_recents(|r| r.items.clear());
                }
            });

            ui.add_space(12.0);
            theme::foot(ui, |ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "settings.toml · {}",
                        app.store.settings_path().display()
                    ))
                    .size(theme::SMALL)
                    .color(theme::TEXT_MUTED),
                );
                ui.label(
                    egui::RichText::new("Hand-editable. INDIUM respects what you write there.")
                        .size(theme::SMALL)
                        .color(theme::TEXT_MUTED),
                );
            });
        });

    if !open {
        app.popup = None;
    }
}

#[cfg(test)]
mod tests {
    /// CORE §4's popup 5 and this panel are the same list of groups, in the same order.
    ///
    /// Same shape as `keys.rs`'s `the_popup_and_core_agree_about_the_keys`, and here for the
    /// same reason: the document moves first in this project and the code follows, so the
    /// document is what a test should read. What made this one necessary is the direction the
    /// drift actually took — the *code* moved first. P2 built three groups, CORE's popup 5
    /// described two, and it went on describing two for seventeen rounds while the third grew
    /// a **Clear list** button that empties the recents. A destructive control existed in the
    /// window, in the file, and in this module's own header, and in no governing document.
    ///
    /// The group names are read out of this file's own source rather than typed here, because
    /// a list typed beside the thing it describes is the hand-copy P18 spent a round removing.
    /// The scan stops at the test module so it cannot read itself.
    #[test]
    fn the_settings_panel_has_the_groups_core_says_it_has() {
        const NUMBER_WORDS: &[&str] = &["no", "one", "two", "three", "four", "five", "six"];

        // This file, up to the module attribute above — so the needle below, which is itself
        // the text being searched for, is never in the searched half.
        let src = include_str!("settings.rs");
        let code = src
            .split_once("#[cfg(test)]")
            .expect("this file has a test module")
            .0;

        let needle = "theme::section(ui, \"";
        let groups: Vec<&str> = code
            .match_indices(needle)
            .filter_map(|(i, _)| {
                let rest = &code[i + needle.len()..];
                rest.find('"').map(|end| &rest[..end])
            })
            .collect();
        assert!(
            !groups.is_empty(),
            "no `theme::section` heading was found in this file, so this test would pass by \
             absence — the panel's headings are no longer written that way"
        );

        // CORE §4's popup 5, from its numbered opening to the next item's.
        let core = include_str!("../../CORE.md");
        let item = core
            .split_once("5. **Settings** (`,`)")
            .expect("CORE §4 has no numbered Settings popup")
            .1
            .split_once("\n6.")
            .expect("CORE §4's Settings item runs into no item 6")
            .0;

        for group in &groups {
            assert!(
                item.contains(group),
                "the Settings panel draws a `{group}` group and CORE §4's popup 5 does not \
                 name it. The panel is what a person sees; the document is what the project \
                 is allowed to grow. CORE names: {item:?}"
            );
        }

        let word = NUMBER_WORDS
            .get(groups.len())
            .unwrap_or_else(|| panic!("{} groups is past the words this test knows", groups.len()));
        assert!(
            item.contains(&format!("{word} groups")),
            "the panel draws {} groups and CORE §4's popup 5 does not say \"{word} groups\". \
             The count is written there deliberately, because the third group holds the only \
             destructive control in the panel.",
            groups.len()
        );
    }
}
