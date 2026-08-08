//! The Inspector — CORE §4's third zone, and the reason INDIUM exists.
//!
//! "Details shows everything the reader can know about the selection; multi-select
//! shows aggregates; no selection shows the archive-level card."
//!
//! CORE §6: "One typeface, monospace, everywhere — chrome and values alike; sizes,
//! checksums, paths, the whole Inspector. Monospace is what makes a verbose pane
//! scannable instead of noisy, and the pane is the program, so the window wears it
//! throughout."
//!
//! Two honesty notes live here, because CORE §4 requires them stated rather than
//! discovered: libarchive exposes no *stored* CRC, so INDIUM computes one on demand
//! and labels it computed; and libarchive exposes no per-entry compressed size at all,
//! so Packed reads "—" for everything it reads. A 7z is read by `sevenz-rust2` instead,
//! which reports a packed size wherever an entry owns its compression block outright.

use eframe::egui;

use super::{Indium, InspectorTab};
use crate::arch::Entry;
use crate::model::{self, Row};
use crate::theme;
use crate::util;

pub fn show(app: &mut Indium, root: &mut egui::Ui, rows: &[Row]) {
    egui::Panel::right("inspector")
        .resizable(true)
        .default_size(330.0)
        .min_size(260.0)
        .frame(
            egui::Frame::NONE
                .fill(theme::PANEL)
                .inner_margin(egui::Margin::symmetric(12, 12)),
        )
        .show(root, |ui| {
            tabs(app, ui);
            ui.add_space(8.0);

            match app.inspector_tab {
                InspectorTab::Details => details(app, ui, rows),
                InspectorTab::Preview => preview(app, ui, rows),
            }
        });
}

fn tabs(app: &mut Indium, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        // Which tab is open is "this mode is active", not "something will happen". P6 §6.6.
        theme::active_fill(ui);
        for (tab, label) in [
            (InspectorTab::Details, "Details"),
            (InspectorTab::Preview, "Preview"),
        ] {
            let active = app.inspector_tab == tab;
            let text = egui::RichText::new(label).color(if active {
                theme::TEXT
            } else {
                theme::TEXT_MUTED
            });
            if ui.selectable_label(active, text).clicked() {
                app.inspector_tab = tab;
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new("Space")
                    .family(theme::MONO)
                    .size(12.0)
                    .color(theme::TEXT_MUTED),
            );
        });
    });
}

fn details(app: &mut Indium, ui: &mut egui::Ui, rows: &[Row]) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if !app.has_archive() {
                ui.label(egui::RichText::new("No archive open.").color(theme::TEXT_MUTED));
                return;
            }

            let selected: Vec<Entry> = app.selected_entries().into_iter().cloned().collect();

            match selected.len() {
                0 => archive_card(app, ui),
                1 => entry_card(app, ui, &selected[0]),
                _ => aggregate_card(ui, &selected),
            }

            // An implicit directory has no entry behind it; say so rather than
            // showing a blank pane.
            if selected.is_empty() {
                if let Some(row) = rows.get(app.cursor) {
                    if app.entry(&row.path).is_none() {
                        ui.add_space(10.0);
                        ui.separator();
                        ui.label(
                            egui::RichText::new(format!("{}/", row.display))
                                .family(theme::MONO)
                                .color(theme::TEXT),
                        );
                        ui.label(
                            egui::RichText::new(
                                "This directory is inferred from entry paths — the archive \
                                 stores no header for it, so there is no metadata to show.",
                            )
                            .size(13.0)
                            .color(theme::TEXT_MUTED),
                        );
                    }
                }
            }
        });
}

// ---------------------------------------------------------------------------
// One entry
// ---------------------------------------------------------------------------

fn entry_card(app: &mut Indium, ui: &mut egui::Ui, e: &Entry) {
    ui.label(
        egui::RichText::new(util::base_name(&e.path))
            .size(17.0)
            .color(theme::TEXT),
    );
    ui.add(
        egui::Label::new(
            egui::RichText::new(&e.path)
                .family(theme::MONO)
                .size(13.0)
                .color(theme::TEXT_MUTED),
        )
        .selectable(true)
        .wrap(),
    );
    ui.add_space(8.0);

    grid(ui, "entry-sizes", |ui| {
        field(ui, "Kind", kind_of(e));
        if !e.is_dir {
            field(
                ui,
                "Size",
                &format!(
                    "{} ({} bytes)",
                    util::format_bytes(e.size),
                    util::format_exact_bytes(e.size)
                ),
            );
            match e.packed {
                Some(p) => field(ui, "Packed", &util::format_bytes(p)),
                None => field_muted(ui, "Packed", "— not reported"),
            }
        }
        field(ui, "Method", &e.method);
    });

    if e.packed.is_none() && !e.is_dir {
        // Two different absences, and they are not the same fact. A 7z member that
        // shares its compression block genuinely has no packed size of its own — the
        // block's total belongs to no single member of it. Everything else is simply
        // libarchive not exposing one.
        note(
            ui,
            if app.archive_info.as_ref().and_then(|i| i.solid) == Some(true) {
                "This entry shares a compression block, so no packed size belongs to it \
                 alone. The archive card reports the block count."
            } else {
                "libarchive reports no per-entry compressed size."
            },
        );
    }

    // --- Checksum -----------------------------------------------------------
    ui.add_space(10.0);
    section_label(ui, "Checksum");
    let computed = app
        .crc_of
        .as_ref()
        .filter(|(p, _)| p == &e.path)
        .map(|(_, v)| *v);

    ui.horizontal(|ui| match computed {
        Some(v) => {
            ui.label(
                egui::RichText::new(format!("{v:08X}"))
                    .family(theme::MONO)
                    .color(theme::TEXT),
            );
            ui.label(
                egui::RichText::new("CRC32, computed")
                    .size(12.0)
                    .color(theme::TEXT_MUTED),
            );
        }
        None => {
            if e.is_dir {
                ui.label(
                    egui::RichText::new("—")
                        .family(theme::MONO)
                        .color(theme::TEXT_MUTED),
                );
            } else if ui.small_button("Compute CRC32").clicked() {
                let path = e.path.clone();
                app.compute_crc(&path);
            }
        }
    });
    if computed.is_none() && !e.is_dir {
        note(
            ui,
            "The format stores no CRC libarchive will hand over, so INDIUM reads the \
             entry and computes one. That is why it is on request.",
        );
    }

    // --- Times --------------------------------------------------------------
    ui.add_space(10.0);
    section_label(ui, "Times");
    grid(ui, "entry-times", |ui| {
        stamp(ui, "Modified", e.mtime);
        stamp(ui, "Accessed", e.atime);
        stamp(ui, "Changed", e.ctime);
        stamp(ui, "Created", e.birthtime);
    });

    // --- Ownership ----------------------------------------------------------
    ui.add_space(10.0);
    section_label(ui, "Ownership");
    grid(ui, "entry-own", |ui| {
        field(
            ui,
            "Owner",
            &match &e.uname {
                Some(n) => format!("{n} ({})", e.uid),
                None => e.uid.to_string(),
            },
        );
        field(
            ui,
            "Group",
            &match &e.gname {
                Some(n) => format!("{n} ({})", e.gid),
                None => e.gid.to_string(),
            },
        );
        field(ui, "Mode", &util::format_mode(e.mode, e.filetype));
    });

    // --- Links and encryption ----------------------------------------------
    if e.symlink.is_some() || e.hardlink.is_some() || e.encrypted {
        ui.add_space(10.0);
        section_label(ui, "Other");
        grid(ui, "entry-other", |ui| {
            if let Some(t) = &e.symlink {
                field(ui, "Symlink ->", t);
            }
            if let Some(t) = &e.hardlink {
                field(ui, "Hardlink ->", t);
            }
            if e.encrypted {
                field(ui, "Encrypted", "yes — a password is needed to read it");
            }
        });
    }
}

fn kind_of(e: &Entry) -> &'static str {
    if e.is_dir {
        "directory"
    } else if e.symlink.is_some() {
        "symbolic link"
    } else if e.hardlink.is_some() {
        "hard link"
    } else {
        "file"
    }
}

// ---------------------------------------------------------------------------
// Many entries
// ---------------------------------------------------------------------------

fn aggregate_card(ui: &mut egui::Ui, selected: &[Entry]) {
    let agg = model::aggregate(selected.iter());
    ui.label(
        egui::RichText::new(format!("{} selected", agg.count))
            .size(17.0)
            .color(theme::TEXT),
    );
    ui.add_space(8.0);
    grid(ui, "agg", |ui| {
        field(ui, "Files", &agg.files.to_string());
        field(ui, "Directories", &agg.dirs.to_string());
        field(
            ui,
            "Total size",
            &format!(
                "{} ({} bytes)",
                util::format_bytes(agg.total_real),
                util::format_exact_bytes(agg.total_real)
            ),
        );
        match agg.total_packed {
            Some(p) => {
                field(ui, "Total packed", &util::format_bytes(p));
                field(ui, "Ratio", &util::format_ratio(agg.total_real, p));
            }
            None => field_muted(ui, "Total packed", "— not reported"),
        }
    });
}

// ---------------------------------------------------------------------------
// The archive itself
// ---------------------------------------------------------------------------

fn archive_card(app: &Indium, ui: &mut egui::Ui) {
    let Some(path) = &app.archive_path else {
        return;
    };
    let agg = model::aggregate(app.entries.iter());

    ui.label(
        egui::RichText::new(
            path.file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default(),
        )
        .size(17.0)
        .color(theme::TEXT),
    );
    ui.add(
        egui::Label::new(
            egui::RichText::new(path.to_string_lossy())
                .family(theme::MONO)
                .size(13.0)
                .color(theme::TEXT_MUTED),
        )
        .selectable(true)
        .wrap(),
    );
    ui.add_space(8.0);

    grid(ui, "archive", |ui| {
        if let Some(info) = &app.archive_info {
            field(ui, "Format", &info.format);
            if !info.filter.is_empty() && info.filter != "none" {
                field(ui, "Filter", &info.filter);
            }
            // CORE §4's solid-block detail. It belongs to the archive rather than to an
            // entry — which is exactly why a member of a shared block has no packed size
            // of its own to show.
            if let Some(solid) = info.solid {
                field(ui, "Solid", if solid { "yes" } else { "no" });
            }
            if let Some(blocks) = info.blocks {
                field(
                    ui,
                    "Blocks",
                    &format!("{blocks} {}", if blocks == 1 { "block" } else { "blocks" }),
                );
            }
        }
        field(ui, "Entries", &agg.count.to_string());
        field(ui, "Files", &agg.files.to_string());
        field(ui, "Directories", &agg.dirs.to_string());
        field(
            ui,
            "Contents",
            &format!(
                "{} ({} bytes)",
                util::format_bytes(agg.total_real),
                util::format_exact_bytes(agg.total_real)
            ),
        );
        field(ui, "On disk", &util::format_bytes(app.archive_bytes));
        field(
            ui,
            "Ratio",
            &util::format_ratio(agg.total_real, app.archive_bytes),
        );
    });

    ui.add_space(8.0);
    ratio_bar(ui, agg.total_real, app.archive_bytes);
}

/// The one piece of non-text in the pane: how much of the original the archive is.
fn ratio_bar(ui: &mut egui::Ui, real: u64, packed: u64) {
    let Some(r) = util::ratio(real, packed) else {
        return;
    };
    let frac = r.clamp(0.0, 1.0);
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 6.0), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 2.0, theme::WINDOW);
    let mut filled = rect;
    filled.set_width(rect.width() * frac);
    // Aubergine, not orange: this bar reports, it does not promise an action.
    painter.rect_filled(filled, 2.0, theme::AUBERGINE);
}

// ---------------------------------------------------------------------------
// Preview — P5
// ---------------------------------------------------------------------------

fn preview(app: &mut Indium, ui: &mut egui::Ui, rows: &[Row]) {
    if !app.has_archive() {
        ui.label(egui::RichText::new("No archive open.").color(theme::TEXT_MUTED));
        return;
    }

    // Preview follows the cursor rather than the multi-selection: there is one pane and
    // one file can be in it. `subject_paths` would give the whole selection, which is the
    // right subject for extraction and the wrong one for looking at something.
    let subject = app
        .selected_entries()
        .first()
        .map(|e| e.path.clone())
        .or_else(|| rows.get(app.cursor).map(|r| r.path.clone()));

    let Some(path) = subject else {
        empty_note(
            ui,
            "Nothing selected.",
            "Arrow keys move; Space returns to Details.",
        );
        return;
    };

    let Some(entry) = app.entry(&path).cloned() else {
        empty_note(
            ui,
            "Nothing to preview.",
            "This directory is inferred from entry paths and has no contents of its own.",
        );
        return;
    };

    if entry.is_dir {
        empty_note(ui, "A directory.", "Enter descends into it.");
        return;
    }
    if entry.size == 0 {
        empty_note(
            ui,
            "An empty file.",
            "Zero bytes, so there is nothing to show.",
        );
        return;
    }

    let ctx = ui.ctx().clone();
    app.request_preview(&ctx, &path);

    // The header names what is being looked at, whatever state the read is in.
    ui.add(
        egui::Label::new(
            egui::RichText::new(util::base_name(&path))
                .size(17.0)
                .color(theme::TEXT),
        )
        .truncate(),
    );

    let ready = app.preview.as_ref().filter(|p| p.path == path);
    let Some(data) = ready else {
        ui.add_space(6.0);
        if app.preview_loading.as_deref() == Some(path.as_str()) {
            ui.horizontal(|ui| {
                ui.add(egui::Spinner::new().color(theme::ORANGE));
                ui.label(
                    egui::RichText::new("Reading…")
                        .size(13.0)
                        .color(theme::TEXT_SECONDARY),
                );
            });
        } else {
            note(ui, "Nothing was read for this entry.");
        }
        return;
    };

    ui.label(
        egui::RichText::new(kind_line(data, entry.size))
            .size(12.0)
            .color(theme::TEXT_MUTED),
    );
    ui.add_space(8.0);

    match data.content {
        util::Content::Image(_) if data.truncated => {
            // A head is enough to sniff an image and never enough to decode one. Handing
            // a truncated PNG to the decoder would surface a loader error where an honest
            // sentence belongs.
            note(
                ui,
                "Too large to preview. INDIUM reads the first few megabytes of an entry, \
                 and an image cannot be decoded from part of itself.",
            );
        }
        util::Content::Image(_) => {
            egui::ScrollArea::both()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.add(
                        egui::Image::from_bytes(data.uri.clone(), data.bytes.clone())
                            .maintain_aspect_ratio(true)
                            .max_width(ui.available_width())
                            .show_loading_spinner(true),
                    );
                });
        }
        util::Content::Text => {
            // The About-licence idiom, plus `.wrap()`: the Inspector is a third the width
            // of that window.
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(String::from_utf8_lossy(&data.bytes))
                                .family(theme::MONO)
                                .size(13.0)
                                .color(theme::TEXT_SECONDARY),
                        )
                        .selectable(true)
                        .wrap(),
                    );
                });
        }
        util::Content::Binary => {
            // CORE §4 puts hex at V1.1, so the honest answer is a sentence and not a
            // half-built hex view.
            note(
                ui,
                "Neither text nor an image INDIUM decodes. A hex view arrives in V1.1; \
                 until then there is nothing truthful to show here.",
            );
        }
        util::Content::Empty => note(ui, "Nothing was read for this entry."),
    }
}

/// What Preview is looking at, in one line.
fn kind_line(data: &super::PreviewData, size: u64) -> String {
    let kind = match data.content {
        util::Content::Image(k) => k.to_string(),
        util::Content::Text => "text".to_string(),
        util::Content::Binary => "binary".to_string(),
        util::Content::Empty => "empty".to_string(),
    };
    if data.truncated {
        format!(
            "{kind} · {} · showing the first {}",
            util::format_bytes(size),
            util::format_bytes(data.bytes.len() as u64)
        )
    } else {
        format!("{kind} · {}", util::format_bytes(size))
    }
}

/// A centred two-line state, the shape the old Preview stub used.
fn empty_note(ui: &mut egui::Ui, title: &str, hint: &str) {
    ui.vertical_centered(|ui| {
        ui.add_space(50.0);
        ui.label(egui::RichText::new(title).color(theme::TEXT_SECONDARY));
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(hint)
                .size(13.0)
                .color(theme::TEXT_MUTED),
        );
    });
}

// ---------------------------------------------------------------------------
// Small helpers
// ---------------------------------------------------------------------------

fn grid(ui: &mut egui::Ui, id: &str, body: impl FnOnce(&mut egui::Ui)) {
    egui::Grid::new(id)
        .num_columns(2)
        .spacing([10.0, 3.0])
        .striped(false)
        .show(ui, body);
}

fn field(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.label(
        egui::RichText::new(label)
            .size(13.0)
            .color(theme::TEXT_MUTED),
    );
    ui.add(
        egui::Label::new(
            egui::RichText::new(value)
                .family(theme::MONO)
                .size(13.0)
                .color(theme::TEXT),
        )
        .selectable(true)
        .wrap(),
    );
    ui.end_row();
}

fn field_muted(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.label(
        egui::RichText::new(label)
            .size(13.0)
            .color(theme::TEXT_MUTED),
    );
    ui.label(
        egui::RichText::new(value)
            .family(theme::MONO)
            .size(13.0)
            .color(theme::TEXT_MUTED),
    );
    ui.end_row();
}

fn stamp(ui: &mut egui::Ui, label: &str, value: Option<i64>) {
    match value {
        Some(t) => field(ui, label, &util::format_timestamp(t)),
        None => field_muted(ui, label, "— not stored"),
    }
}

fn section_label(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .size(12.0)
            .color(theme::TEXT)
            .family(theme::bold()),
    );
    ui.add_space(2.0);
}

fn note(ui: &mut egui::Ui, text: &str) {
    ui.add_space(3.0);
    ui.label(
        egui::RichText::new(text)
            .size(12.0)
            .italics()
            .color(theme::TEXT_MUTED),
    );
}
