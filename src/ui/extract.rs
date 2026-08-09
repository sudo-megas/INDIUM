//! The Extract popover — CORE §4.3.
//!
//! "A popover: *Extract here*, *Extract to `<name>/`*, a path field with tab
//! completion, bookmarks beneath. Enter-driven."
//!
//! P2 §2 un-stubs the bookmarks row and adds the pin button, drawn as a `+`. CORE asks
//! for no particular mark — the star this comment used to credit it with was never in
//! the document — and `+` says "add this" without one. P1 Deviation 5 records the
//! original reason (the Ubuntu faces carried no `☆`); the face still carries no
//! `★ U+2605`, only Nerd Font's own star, and `+` remains the plainer answer.
//! "Adding happens where the path is already in your hands."

use std::path::PathBuf;

use eframe::egui;

use super::{archive_stem, Indium, Popup};
use crate::platform::store::Bookmark;
use crate::theme;

/// The path field's `Id`, named in one place so the widget and `caret_to_end` cannot
/// disagree about it.
fn path_field_id() -> egui::Id {
    egui::Id::new("extract-path-field")
}

pub fn show(app: &mut Indium, ctx: &egui::Context) {
    if app.popup != Some(Popup::Extract) {
        return;
    }
    let Some(archive) = app.archive_path.clone() else {
        app.popup = None;
        return;
    };

    let mut open = true;
    let mut go: Option<PathBuf> = None;

    egui::Window::new("Extract")
        .max_height(theme::popup_max_height(ctx))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .open(&mut open)
        .show(ctx, |ui| {
            ui.set_min_width(520.0);

            let count = if app.selection.is_empty() {
                app.entries.len()
            } else {
                app.selection.len()
            };
            ui.label(
                egui::RichText::new(if app.selection.is_empty() {
                    format!("Extracting the whole archive — {count} entries.")
                } else {
                    format!("Extracting {count} selected.")
                })
                .color(theme::TEXT_SECONDARY),
            );
            ui.add_space(8.0);

            let beside = archive
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("."));
            let subdir = beside.join(archive_stem(&archive));

            ui.horizontal(|ui| {
                if theme::button(ui, egui::RichText::new("Extract here"), true).clicked() {
                    go = Some(beside.clone());
                }
                if theme::button(
                    ui,
                    egui::RichText::new(format!("Extract to {}/", archive_stem(&archive))),
                    true,
                )
                .clicked()
                {
                    go = Some(subdir.clone());
                }
            });

            ui.add_space(10.0);
            ui.separator();
            ui.label(egui::RichText::new("Or a path").color(theme::TEXT_MUTED));

            ui.horizontal(|ui| {
                // Named so `caret_to_end` can find its state after Tab rewrites the text,
                // and `lock_focus` so Tab completes rather than leaving the field.
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut app.extract_path)
                        .id(path_field_id())
                        .lock_focus(true)
                        .font(egui::TextStyle::Monospace)
                        .desired_width(400.0),
                );
                // Once, on opening. Every frame meant nothing else in this popup — the ☆,
                // the bookmark chips, the buttons — could take focus from it.
                if app.wants_initial_focus(&Popup::Extract) {
                    resp.request_focus();
                }

                // P2 §2: "a small ☆ beside the popover's path field pins the typed
                // path (prompting only for a name)".
                if theme::small_button(ui, egui::RichText::new("+"), true)
                    .on_hover_text("Pin this path as a bookmark")
                    .clicked()
                {
                    let path = app.extract_path.trim().to_string();
                    if !path.is_empty() {
                        let name = PathBuf::from(&path)
                            .file_name()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_else(|| path.clone());
                        if !app.settings.bookmarks.iter().any(|b| b.path == path) {
                            let added = Bookmark { name, path };
                            app.change_settings(move |s| {
                                // Asked again of the file, not of this window's copy:
                                // another window may have pinned the same path since
                                // this one last read it.
                                if !s.bookmarks.iter().any(|b| b.path == added.path) {
                                    s.bookmarks.push(added);
                                }
                            });
                            app.status = "Bookmark pinned.".to_string();
                        }
                    }
                }
            });

            // Tab completion against the filesystem.
            if let Some(completed) = complete_path(&app.extract_path) {
                if completed != app.extract_path {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("Tab ->")
                                .size(12.0)
                                .color(theme::TEXT_MUTED),
                        );
                        ui.label(
                            egui::RichText::new(&completed)
                                .family(theme::MONO)
                                .size(13.0)
                                .color(theme::TEXT_MUTED),
                        );
                    });
                    if ui.input(|i| i.key_pressed(egui::Key::Tab)) {
                        app.extract_path = completed;
                        super::caret_to_end(ui.ctx(), path_field_id(), &app.extract_path);
                    }
                }
            }

            if !app.settings.bookmarks.is_empty() {
                ui.add_space(8.0);
                ui.label(egui::RichText::new("Bookmarks").color(theme::TEXT_MUTED));
                ui.horizontal_wrapped(|ui| {
                    for b in app.settings.bookmarks.clone() {
                        if theme::small_button(ui, egui::RichText::new(&b.name), true)
                            .on_hover_text(&b.path)
                            .clicked()
                        {
                            app.extract_path = b.path.clone();
                        }
                    }
                });
            }

            ui.add_space(10.0);
            theme::foot(ui, |ui| {
                ui.horizontal(|ui| {
                    let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if theme::button(ui, egui::RichText::new("Extract"), true).clicked() || enter {
                        let typed = app.extract_path.trim();
                        if !typed.is_empty() {
                            go = Some(PathBuf::from(expand_tilde(typed)));
                        }
                    }
                    ui.label(
                        egui::RichText::new("Enter extracts · Esc closes")
                            .size(12.0)
                            .color(theme::TEXT_MUTED),
                    );
                });
            });
        });

    if let Some(dest) = go {
        app.request_extract(ctx, dest);
    }
    if !open {
        app.popup = None;
    }
}

// ---------------------------------------------------------------------------
// Tab completion
// ---------------------------------------------------------------------------

/// Expand a leading `~` against `$HOME`.
pub fn expand_tilde(input: &str) -> String {
    if input == "~" {
        return crate::platform::home().to_string_lossy().to_string();
    }
    match input.strip_prefix("~/") {
        Some(rest) => crate::platform::home()
            .join(rest)
            .to_string_lossy()
            .to_string(),
        None => input.to_string(),
    }
}

/// The longest string every candidate starts with.
///
/// Pure and byte-safe: the split point is only accepted on a character boundary, so a
/// completion can never cut a multi-byte name in half.
pub fn longest_common_prefix(candidates: &[String]) -> String {
    let Some(first) = candidates.first() else {
        return String::new();
    };
    let mut len = first.len();
    for c in &candidates[1..] {
        let common = first
            .as_bytes()
            .iter()
            .zip(c.as_bytes())
            .take_while(|(a, b)| a == b)
            .count();
        len = len.min(common);
    }
    while len > 0 && !first.is_char_boundary(len) {
        len -= 1;
    }
    first[..len].to_string()
}

/// Complete a partially typed filesystem path against what is actually on disk.
///
/// Returns the completed path, or `None` when there is nothing to add. Directories
/// gain a trailing `/` so the next Tab continues into them.
pub fn complete_path(input: &str) -> Option<String> {
    if input.is_empty() {
        return None;
    }
    let expanded = expand_tilde(input);

    let (dir, prefix) = match expanded.rsplit_once('/') {
        Some((d, p)) => {
            let d = if d.is_empty() { "/" } else { d };
            (d.to_string(), p.to_string())
        }
        None => (".".to_string(), expanded.clone()),
    };

    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            if !name.starts_with(&prefix) {
                return None;
            }
            // Only directories are useful as an extract destination, but completing
            // through files would be surprising, so both are offered and directories
            // are marked.
            let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
            Some(if is_dir { format!("{name}/") } else { name })
        })
        .collect();

    if names.is_empty() {
        return None;
    }
    names.sort();

    let common = longest_common_prefix(&names);
    if common.len() <= prefix.len() {
        return None;
    }

    let joined = if dir == "/" {
        format!("/{common}")
    } else {
        format!("{dir}/{common}")
    };
    Some(joined)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_prefix_of_one_is_itself() {
        assert_eq!(longest_common_prefix(&["alpha".to_string()]), "alpha");
    }

    #[test]
    fn common_prefix_stops_at_the_first_difference() {
        let names = vec!["report-a.txt".to_string(), "report-b.txt".to_string()];
        assert_eq!(longest_common_prefix(&names), "report-");
    }

    #[test]
    fn common_prefix_of_nothing_is_empty() {
        assert_eq!(longest_common_prefix(&[]), "");
        let names = vec!["a".to_string(), "b".to_string()];
        assert_eq!(longest_common_prefix(&names), "");
    }

    /// A completion must never split a multi-byte character.
    #[test]
    fn common_prefix_respects_character_boundaries() {
        let names = vec!["ödev-a".to_string(), "ödev-b".to_string()];
        let got = longest_common_prefix(&names);
        assert_eq!(got, "ödev-");
        assert!(got.chars().all(|c| c != '\u{FFFD}'));
    }

    #[test]
    fn tilde_expands_to_home() {
        let home = crate::platform::home().to_string_lossy().to_string();
        assert_eq!(expand_tilde("~"), home);
        assert_eq!(expand_tilde("~/Downloads"), format!("{home}/Downloads"));
        // A tilde that is not a home reference is left alone.
        assert_eq!(expand_tilde("~user/x"), "~user/x");
        assert_eq!(expand_tilde("/tmp"), "/tmp");
    }

    #[test]
    fn completion_finds_a_real_directory() {
        let base = std::env::temp_dir().join(format!("indium-complete-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("destination-one")).unwrap();
        std::fs::create_dir_all(base.join("destination-two")).unwrap();

        let partial = format!("{}/dest", base.display());
        let got = complete_path(&partial).expect("should complete");
        assert_eq!(got, format!("{}/destination-", base.display()));

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn completion_of_a_unique_directory_adds_a_slash() {
        let base = std::env::temp_dir().join(format!("indium-unique-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("only-one")).unwrap();

        let partial = format!("{}/onl", base.display());
        let got = complete_path(&partial).expect("should complete");
        assert_eq!(got, format!("{}/only-one/", base.display()));

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn completion_returns_none_when_nothing_matches() {
        let base = std::env::temp_dir().join(format!("indium-none-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        assert!(complete_path(&format!("{}/zzz", base.display())).is_none());
        assert!(complete_path("").is_none());
        let _ = std::fs::remove_dir_all(&base);
    }
}
