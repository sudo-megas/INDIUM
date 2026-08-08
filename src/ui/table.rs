//! The centre zone: the entry table, and the two list views the sidebar can select.
//!
//! CORE §4: "virtualized; columns Name, Size, Packed, Method; a breadcrumb path above
//! it." P1 §3 requires the virtualisation to be real — "the virtualized table that
//! lets a 100,000-entry archive scroll like a 100-entry one" (CORE §2).

use eframe::egui;
use egui_extras::{Column, TableBuilder};

use super::{filter, Indium, Section};
use crate::model::Row;
use crate::theme;
use crate::util;

const ROW_HEIGHT: f32 = 20.0;

pub fn show(app: &mut Indium, root: &mut egui::Ui, rows: &[Row]) {
    let ctx = root.ctx().clone();
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(theme::WINDOW))
        .show(root, |ui| match app.section {
            Section::Archive => archive_view(app, ui, rows),
            Section::Recents => recents_view(app, &ctx, ui),
            Section::Bookmarks => bookmarks_view(app, ui),
        });
}

// ---------------------------------------------------------------------------
// The archive table
// ---------------------------------------------------------------------------

fn archive_view(app: &mut Indium, ui: &mut egui::Ui, rows: &[Row]) {
    if !app.has_archive() {
        empty_state(ui, "No archive open.", "Drop one here, or press Ctrl+O.");
        return;
    }

    breadcrumb_bar(app, ui);
    filter::show(app, ui, rows.len());
    ui.separator();

    if rows.is_empty() {
        let (title, hint) = if app.filter.is_some() {
            ("Nothing matches.", "Esc clears the filter.")
        } else if app.listing {
            ("Reading…", "Entries appear as they are read.")
        } else {
            ("This directory is empty.", "Backspace goes up.")
        };
        empty_state(ui, title, hint);
        return;
    }

    let mut clicked: Option<(usize, bool)> = None;
    let mut descend_into: Option<usize> = None;
    let mut commit_rename = false;

    TableBuilder::new(ui)
        .striped(false)
        .resizable(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::remainder().at_least(120.0).clip(true))
        .column(Column::exact(84.0))
        .column(Column::exact(84.0))
        .column(Column::exact(72.0))
        .header(22.0, |mut header| {
            for name in ["Name", "Size", "Packed", "Method"] {
                header.col(|ui| {
                    ui.label(
                        egui::RichText::new(name)
                            .size(13.0)
                            .color(theme::TEXT_MUTED),
                    );
                });
            }
        })
        .body(|body| {
            // `rows` here is egui_extras' virtualised body: only visible lines are
            // built, which is what keeps a 100,000-entry archive responsive.
            body.rows(ROW_HEIGHT, rows.len(), |mut tr| {
                let i = tr.index();
                let row = &rows[i];
                let selected = app.selection.contains(&row.path);
                let focused = i == app.cursor;
                tr.set_selected(selected);

                // Copied out rather than borrowed: the Name cell needs `&mut` access to
                // the rename field, and a live `&Entry` would hold `app` immutably
                // across it.
                let entry: Option<(bool, bool, u64, Option<u64>, String)> = app
                    .entry(&row.path)
                    .map(|e| (e.encrypted, e.is_dir, e.size, e.packed, e.method.clone()));

                tr.col(|ui| {
                    let colour = if focused {
                        theme::ORANGE
                    } else if row.is_dir {
                        theme::TEXT
                    } else {
                        theme::TEXT_SECONDARY
                    };
                    // A trailing slash marks a directory, as `ls -F` has since forever.
                    // CORE names no marker at all — the triangle this comment used to
                    // credit it with was never in the document — so the slash is
                    // INDIUM's own choice and stays one: it sorts with the name, costs
                    // no column, and needs no glyph. P1 Deviation 5 records the
                    // original reason (the Ubuntu faces carried no `▸`); the face
                    // carries one now, and the answer is still the slash.
                    let shown = if row.is_dir {
                        format!("{}/", row.display)
                    } else {
                        row.display.clone()
                    };
                    // `F2` turns this cell into a text field rather than opening an
                    // eighth popup — CORE §4 fixes the count at seven. A focused field
                    // also makes the existing `typing` guard suppress bare keys, so
                    // `Del` cannot fire into a half-typed name.
                    if app.rename_target.as_deref() == Some(row.path.as_str()) {
                        let field = ui.add(
                            egui::TextEdit::singleline(&mut app.rename_input)
                                .font(egui::TextStyle::Monospace)
                                .desired_width(240.0),
                        );
                        field.request_focus();
                        if ui.input(|inp| inp.key_pressed(egui::Key::Enter)) {
                            commit_rename = true;
                        }
                    } else {
                        // Named explicitly: without these it inherited egui's 13.0
                        // proportional default and sat 18% larger than the three mono
                        // columns beside it, in every row of the program's main view.
                        let mut text = egui::RichText::new(shown)
                            .family(theme::MONO)
                            .size(13.0)
                            .color(colour);
                        if row.is_dir {
                            text = text.family(theme::bold());
                        }
                        let resp = ui.add(
                            egui::Label::new(text)
                                .sense(egui::Sense::click())
                                .truncate(),
                        );
                        if resp.clicked() {
                            clicked = Some((i, ui.input(|inp| inp.modifiers.ctrl)));
                        }
                        if resp.double_clicked() && row.is_dir {
                            descend_into = Some(i);
                        }
                    }
                    if entry.as_ref().map(|e| e.0).unwrap_or(false) {
                        ui.label(
                            egui::RichText::new("enc")
                                .size(12.0)
                                .color(theme::TEXT_MUTED),
                        );
                    }
                });

                tr.col(|ui| {
                    let text = match entry.as_ref() {
                        Some((_, is_dir, size, _, _)) if !*is_dir => util::format_bytes(*size),
                        _ => "—".to_string(),
                    };
                    mono_right(ui, &text, theme::TEXT_SECONDARY);
                });

                tr.col(|ui| {
                    // Reported only where it is knowable. libarchive exposes no
                    // per-entry compressed size at all, and a 7z gives one only where an
                    // entry owns its compression block outright — a shared block's total
                    // belongs to no single member of it, so "—" is the honest answer
                    // rather than a share of someone else's bytes.
                    let text = match entry.as_ref().and_then(|e| e.3) {
                        Some(p) => util::format_bytes(p),
                        None => "—".to_string(),
                    };
                    mono_right(ui, &text, theme::TEXT_MUTED);
                });

                tr.col(|ui| {
                    let text = entry
                        .as_ref()
                        .map(|e| e.4.clone())
                        .unwrap_or_else(|| "—".into());
                    ui.label(
                        egui::RichText::new(text)
                            .family(theme::MONO)
                            .size(13.0)
                            .color(theme::TEXT_MUTED),
                    );
                });
            });
        });

    if commit_rename {
        app.commit_rename();
    }

    if let Some((i, additive)) = clicked {
        app.cursor = i;
        app.crc_of = None;
        let path = rows[i].path.clone();
        if additive {
            if !app.selection.remove(&path) {
                app.selection.insert(path);
            }
        } else {
            app.selection.clear();
            app.selection.insert(path);
        }
    }
    if let Some(i) = descend_into {
        app.cursor = i;
        app.descend(rows);
    }
}

fn breadcrumb_bar(app: &mut Indium, ui: &mut egui::Ui) {
    let crumbs = crate::model::breadcrumb(&app.cwd);
    let mut go: Option<String> = None;

    egui::Frame::NONE
        .inner_margin(egui::Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                for (n, (label, path)) in crumbs.iter().enumerate() {
                    if n > 0 {
                        ui.label(egui::RichText::new("/").color(theme::TEXT_MUTED));
                    }
                    let last = n + 1 == crumbs.len();
                    let colour = if last {
                        theme::TEXT
                    } else {
                        theme::TEXT_SECONDARY
                    };
                    let resp = ui.add(
                        egui::Label::new(
                            egui::RichText::new(label).family(theme::MONO).color(colour),
                        )
                        .sense(egui::Sense::click()),
                    );
                    if resp.clicked() && !last {
                        go = Some(path.clone());
                    }
                }
            });
        });

    if let Some(path) = go {
        app.cwd = path;
        app.cursor = 0;
    }
}

// ---------------------------------------------------------------------------
// Recent files — P2 §2
// ---------------------------------------------------------------------------

fn recents_view(app: &mut Indium, ctx: &egui::Context, ui: &mut egui::Ui) {
    header(ui, "Recent files");

    let items: Vec<(String, i64)> = app
        .recents
        .sorted()
        .iter()
        .map(|r| (r.path.clone(), r.opened))
        .collect();

    if items.is_empty() {
        empty_state(
            ui,
            "Nothing opened yet.",
            "Drop an archive here, or press Ctrl+O.",
        );
        return;
    }

    let mut open_this: Option<String> = None;
    let mut forget: Option<String> = None;

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (i, (path, opened)) in items.iter().enumerate() {
                // P2 §2: "A missing file renders dimmed." No automatic pruning — the
                // list only loses entries by the user's hand or the cap.
                let exists = std::path::Path::new(path).exists();
                let focused = i == app.cursor;

                let frame = egui::Frame::NONE
                    .inner_margin(egui::Margin::symmetric(12, 7))
                    .fill(if focused {
                        theme::AUBERGINE
                    } else {
                        egui::Color32::TRANSPARENT
                    });

                let inner = frame.show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.vertical(|ui| {
                        let name = std::path::Path::new(path)
                            .file_name()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_else(|| path.clone());
                        ui.label(egui::RichText::new(name).color(if exists {
                            theme::TEXT
                        } else {
                            theme::TEXT_MUTED
                        }));
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(path)
                                    .family(theme::MONO)
                                    .size(13.0)
                                    .color(theme::TEXT_MUTED),
                            );
                            if !exists {
                                ui.label(
                                    egui::RichText::new("missing")
                                        .size(12.0)
                                        .color(theme::TEXT_MUTED),
                                );
                            }
                        });
                        ui.label(
                            egui::RichText::new(util::format_timestamp(*opened))
                                .family(theme::MONO)
                                .size(12.0)
                                .color(theme::TEXT_MUTED),
                        );
                    });
                });

                let resp = ui.interact(
                    inner.response.rect,
                    ui.id().with(("recent", i)),
                    egui::Sense::click(),
                );
                if resp.clicked() {
                    app.cursor = i;
                }
                if resp.double_clicked() {
                    if exists {
                        open_this = Some(path.clone());
                    } else {
                        forget = None;
                    }
                }
                resp.context_menu(|ui| {
                    if ui.button("Remove from list").clicked() {
                        forget = Some(path.clone());
                        ui.close();
                    }
                });
            }
        });

    if let Some(p) = open_this {
        app.open_archive(ctx, std::path::PathBuf::from(p), None);
    }
    if let Some(p) = forget {
        app.recents.remove(&p);
        if !app.recents_broken {
            let _ = app.store.save_recents(&app.recents);
        }
        app.status = format!("Removed {p} from recent files.");
    }
}

// ---------------------------------------------------------------------------
// Bookmarks — P2 §2
// ---------------------------------------------------------------------------

fn bookmarks_view(app: &mut Indium, ui: &mut egui::Ui) {
    header(ui, "Bookmarks");
    ui.label(
        egui::RichText::new("Named directories to extract into.")
            .size(13.0)
            .color(theme::TEXT_MUTED),
    );
    ui.add_space(6.0);

    if app.settings.bookmarks.is_empty() {
        empty_state(
            ui,
            "No bookmarks yet.",
            "Pin one with the + in the Extract popover, or add it in Settings.",
        );
        return;
    }

    let mut remove: Option<usize> = None;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (i, b) in app.settings.bookmarks.iter().enumerate() {
                let focused = i == app.cursor;
                egui::Frame::NONE
                    .inner_margin(egui::Margin::symmetric(12, 7))
                    .fill(if focused {
                        theme::AUBERGINE
                    } else {
                        egui::Color32::TRANSPARENT
                    })
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new(&b.name).color(theme::TEXT));
                                ui.label(
                                    egui::RichText::new(&b.path)
                                        .family(theme::MONO)
                                        .size(13.0)
                                        .color(theme::TEXT_MUTED),
                                );
                            });
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.small_button("×").clicked() {
                                        remove = Some(i);
                                    }
                                },
                            );
                        });
                    });
            }
        });

    if let Some(i) = remove {
        let b = app.settings.bookmarks.remove(i);
        app.save_settings();
        app.status = format!("Removed bookmark {}.", b.name);
    }
}

// ---------------------------------------------------------------------------
// Shared bits
// ---------------------------------------------------------------------------

fn header(ui: &mut egui::Ui, text: &str) {
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.add_space(12.0);
        ui.label(egui::RichText::new(text).size(17.0).color(theme::TEXT));
    });
    ui.add_space(4.0);
}

fn empty_state(ui: &mut egui::Ui, title: &str, hint: &str) {
    ui.vertical_centered(|ui| {
        ui.add_space(70.0);
        ui.label(
            egui::RichText::new(title)
                .size(16.0)
                .color(theme::TEXT_SECONDARY),
        );
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(hint)
                .size(13.0)
                .color(theme::TEXT_MUTED),
        );
    });
}

fn mono_right(ui: &mut egui::Ui, text: &str, colour: egui::Color32) {
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.label(
            egui::RichText::new(text)
                .family(theme::MONO)
                .size(13.0)
                .color(colour),
        );
    });
}
