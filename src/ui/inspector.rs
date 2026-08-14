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

use super::table;
use super::{Indium, InspectorTab};
use crate::arch::Entry;
use crate::model::{self, Row};
use crate::theme;
use crate::util;

/// The Inspector's inner margin.
const ZONE_PAD: egui::Margin = egui::Margin::same(12);
/// What the pane's contents get at rest and at its narrowest: `330 − 24` and `260 − 24`,
/// which is exactly what P1 gave them.
const CONTENT: f32 = 306.0;
const MIN_CONTENT: f32 = 236.0;

/// What the table must be left with before this pane may keep another point of width.
///
/// The centre is the one zone that cannot give, and that asymmetry is the whole reason this
/// number exists. The sidebar's rows scroll under a bar that floats over them; this pane has
/// [`MIN_CONTENT`]; the table has [`table::COLUMNS`], whose widths are a specification rather
/// than a preference. Summed, that is **360** that no amount of narrowing will reduce — and it
/// is summed here rather than written down, because a copy of it is exactly what drifted.
///
/// Three costs sit on top of it, none of them guessable from the column list, and all three
/// are pinned by `the_centre_floor_is_the_sum_it_claims` rather than trusted:
/// `egui_extras` puts `item_spacing.x` between the columns — `to_lengths` charges
/// `spacing * (n − 1)`, so 8.0 × 3 = **24**; it subtracts the scrollbar's lane from what it
/// lays out into whenever `vscroll` is on, shown or not, which `ScrollStyle::solid` makes
/// `bar_inner_margin + bar_width + bar_outer_margin` = 4 + 6 + 0 = **10**; and the
/// `CentralPanel`'s own frame takes **20**, because `Frame::total_margin` is
/// `inner_margin + stroke.width + outer_margin` and `table::show` pays 4 + 2 + 4 a side.
///
/// 360 + 24 + 10 + 20 = **414**.
pub const CENTRE_MIN: f32 = table::COLUMNS[0]
    + table::COLUMNS[1]
    + table::COLUMNS[2]
    + table::COLUMNS[3]
    + GAPS
    + SCROLLBAR
    + CHROME;

/// `item_spacing.x` × (`COLUMNS.len()` − 1), per `egui_extras`' `sizing.rs`.
const GAPS: f32 = 24.0;
/// `ScrollStyle::solid().allocated_width()`, subtracted whenever the table has `vscroll`.
const SCROLLBAR: f32 = 10.0;
/// `table::show`'s `CentralPanel` frame: 4 inner + 2 stroke + 4 outer, a side.
const CHROME: f32 = 20.0;

pub fn show(app: &mut Indium, root: &mut egui::Ui, rows: &[Row]) {
    // Both sizes are the panel's *outer* width, so both pay for the whole frame. Asked of
    // the frame rather than written down, for the reason `sidebar::show` gives at length:
    // `Frame::total_margin` is `inner_margin + stroke.width + outer_margin`, and a sum that
    // forgets the 2px edge is four pixels wrong on a pane CORE §1 calls the main event.
    let frame = theme::zone(theme::PANEL).inner_margin(ZONE_PAD);
    let chrome = frame.total_margin().sum().x;

    // And a ceiling, which the pane went without until PXX. A side panel keeps whatever width
    // it holds; only the `CentralPanel` after it absorbs the difference when the window
    // narrows. So without this line the pane sails through 560 wide at its full 330 and the
    // table is left with about sixteen points to draw four columns into — the state the walk
    // denied at step 6.3. The clamp is on *this* pane rather than on `MIN_W` because a floor
    // on the window forbids a size the maker may legitimately want; a ceiling here simply
    // makes the widest zone yield first, which is what a reader dragging an edge expects.
    //
    // `.max()` is load-bearing and not defensive: below `CENTRE_MIN + MIN_CONTENT + chrome`
    // of available width the subtraction goes under the minimum, and egui resolves a range
    // whose max is under its min by taking the max — so the pane would collapse past
    // [`MIN_CONTENT`] instead of stopping at it. Past that point the window is simply too
    // narrow for both, and the table clips, which is the failure it is built to survive.
    //
    // One consequence, stated rather than discovered: `Panel::outer_size` clamps the width it
    // loads from `PanelState` and then stores the clamped result, so a pane squeezed by a
    // narrow window stays narrow when the window widens again. It is a drag away from where it
    // was, and buying the spring-back would mean holding a second remembered width in `Indium`
    // and deciding when it goes stale — which is more machinery than this defect is worth on
    // the eve of a freeze.
    let ceiling =
        (root.available_rect_before_wrap().width() - CENTRE_MIN).max(MIN_CONTENT + chrome);

    egui::Panel::right("inspector")
        .resizable(true)
        .default_size(CONTENT + chrome)
        .min_size(MIN_CONTENT + chrome)
        .max_size(ceiling)
        .frame(frame)
        // The card's own 2px edge is the boundary; egui's panel hairline would stack with it.
        .show_separator_line(false)
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
                    .size(theme::SMALL)
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
                            .size(theme::BODY)
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
            .size(theme::TITLE)
            .color(theme::TEXT),
    );
    ui.add(
        egui::Label::new(
            egui::RichText::new(&e.path)
                .family(theme::MONO)
                .size(theme::BODY)
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
                Some(p) => {
                    field(ui, "Packed", &util::format_bytes(p));
                    // The aggregate card and the whole-archive card have both shown a ratio
                    // since P1; the single entry — the card a person reaches first — made
                    // them do the division themselves, while the README promised it among
                    // what the Inspector states about a selection. Only where a packed size
                    // is known: a 7z member sharing a compression block has none of its own,
                    // and a ratio computed from a number that was never reported would be an
                    // invention rather than a fact.
                    field(ui, "Ratio", &util::format_ratio(e.size, p));
                }
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
    //
    // Each of the four headings below opens a list of siblings, so each takes the rule
    // (P7 §4). The local `section_label` they replace drew 12.0 — *smaller* than the 13.0
    // body beneath it — and no rule; `theme::section` owns the leading space, which is why
    // the `add_space(10.0)` that used to precede each one is gone rather than doubled.
    theme::section(ui, "Checksum");
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
                    .size(theme::SMALL)
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
            } else if theme::small_button(ui, egui::RichText::new("Compute CRC32"), true).clicked()
            {
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
    theme::section(ui, "Times");
    grid(ui, "entry-times", |ui| {
        stamp(ui, "Modified", e.mtime);
        stamp(ui, "Accessed", e.atime);
        stamp(ui, "Changed", e.ctime);
        stamp(ui, "Created", e.birthtime);
    });

    // --- Ownership ----------------------------------------------------------
    theme::section(ui, "Ownership");
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
        theme::section(ui, "Other");
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
            .size(theme::TITLE)
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
        .size(theme::TITLE)
        .color(theme::TEXT),
    );
    ui.add(
        egui::Label::new(
            egui::RichText::new(path.to_string_lossy())
                .family(theme::MONO)
                .size(theme::BODY)
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
                .size(theme::TITLE)
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
                        .size(theme::BODY)
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
            .size(theme::SMALL)
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
                                .size(theme::BODY)
                                .color(theme::TEXT_SECONDARY),
                        )
                        .selectable(true)
                        .wrap(),
                    );
                });
        }
        util::Content::Binary => hex(ui, data),
        util::Content::Empty => note(ui, "Nothing was read for this entry."),
    }
}

/// The hex view CORE §4 has promised since P5, and the reader since P5's own copy.
///
/// **Virtualised, and it has to be.** The preview cap is 8 MiB, which at
/// [`util::HEX_COLUMNS`] bytes a row is 524,288 rows — so the text arm's idiom above, one
/// `Label` holding the whole blob, is exactly the thing that must not be copied here.
/// `show_rows` builds only the rows on screen and needs a row height it can trust, which is
/// why that height is asked of the font rather than written down: a literal that disagreed
/// with what is painted would drift a row out of place for every row scrolled.
///
/// **It scrolls sideways rather than reflowing.** A row is about 78 cells and the Inspector's
/// content is 306 at rest, so the line does not fit and is not made to: [`util::HEX_COLUMNS`]
/// says at length why the count is fixed, and the pane is resizable for a reader who wants
/// the whole width. `ScrollArea::both` is the image arm's idiom, one zone over.
fn hex(ui: &mut egui::Ui, data: &super::PreviewData) {
    let font = egui::FontId::new(theme::BODY, theme::MONO);
    // `fonts_mut` for the reason the status bar's lane gives: measuring can populate the
    // atlas, so the accessor that admits it is the correct one.
    let row_h = ui.ctx().fonts_mut(|f| f.row_height(&font));

    // Zeroed before `show_rows`, not inside it: the scroll area works out which rows are
    // visible from this spacing, so setting it in the closure would be a frame too late.
    // A dump is a grid, and a grid with gaps in it is not one.
    ui.spacing_mut().item_spacing = egui::Vec2::ZERO;

    let rows = util::hex_rows(data.bytes.len());
    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .show_rows(ui, row_h, rows, |ui, range| {
            for row in range {
                let start = row * util::HEX_COLUMNS;
                let end = (start + util::HEX_COLUMNS).min(data.bytes.len());
                ui.horizontal(|ui| {
                    // The offset is chrome and the bytes are the value, which §6 tells
                    // apart by colour. Two labels rather than one, for that alone.
                    ui.label(
                        egui::RichText::new(util::hex_offset(start))
                            .family(theme::MONO)
                            .size(theme::BODY)
                            .color(theme::TEXT_MUTED),
                    );
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(format!(
                                "  {}",
                                util::hex_body(&data.bytes[start..end])
                            ))
                            .family(theme::MONO)
                            .size(theme::BODY)
                            .color(theme::TEXT_SECONDARY),
                        )
                        // As the text arm is. A selection cannot run past the rows that
                        // exist, which is the price of virtualising half a million of them.
                        .selectable(true),
                    );
                });
            }
        });
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
                .size(theme::BODY)
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
            .size(theme::BODY)
            .color(theme::TEXT_MUTED),
    );
    ui.add(
        egui::Label::new(
            egui::RichText::new(value)
                .family(theme::MONO)
                .size(theme::BODY)
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
            .size(theme::BODY)
            .color(theme::TEXT_MUTED),
    );
    ui.label(
        egui::RichText::new(value)
            .family(theme::MONO)
            .size(theme::BODY)
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

fn note(ui: &mut egui::Ui, text: &str) {
    ui.add_space(3.0);
    ui.label(
        egui::RichText::new(text)
            .size(theme::SMALL)
            .italics()
            .color(theme::TEXT_MUTED),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// PXX 6.3: the centre's floor is arithmetic, and every term of it lives in another file.
    ///
    /// [`CENTRE_MIN`] sums [`table::COLUMNS`] directly, so the columns cannot drift from it.
    /// The other three terms can: they are egui's numbers, not INDIUM's, and an upgrade may
    /// change any of them without a word. That is precisely how this defect was born — the
    /// pane carried a `min_size` it never reached and a floor nobody had multiplied out — so
    /// the three are read back off a real style here rather than trusted.
    ///
    /// A failure is not a bug in the clamp. It means egui now charges a different price for
    /// the same layout and `CENTRE_MIN` owes the table more width than it is asking for.
    #[test]
    fn the_centre_floor_is_the_sum_it_claims() {
        let ctx = egui::Context::default();
        theme::install_visuals(&ctx);
        let spacing = ctx.style_of(egui::Theme::Dark).spacing.clone();

        assert_eq!(
            spacing.item_spacing.x * (table::COLUMNS.len() - 1) as f32,
            GAPS,
            "egui_extras charges `item_spacing * (n - 1)` between columns",
        );
        assert_eq!(
            spacing.scroll.allocated_width(),
            SCROLLBAR,
            "a solid scrollbar takes its lane out of the table's width, shown or not",
        );

        // The same frame `table::show` builds, asked rather than restated — the four-plus-two
        // -plus-four a side that its own comment insists is 20 and not 16.
        let chrome = theme::zone(theme::WINDOW)
            .inner_margin(egui::Margin::same(4))
            .total_margin()
            .sum()
            .x;
        assert_eq!(chrome, CHROME, "the CentralPanel's frame costs 20, not 16");

        assert_eq!(CENTRE_MIN, 414.0, "360 of columns + 24 + 10 + 20");
    }

    /// The `.max()` in `show`'s ceiling is load-bearing, and this is the arithmetic that says so.
    ///
    /// `Panel::max_size` sets the range to `(min.at_most(max), max)` — it *lowers the minimum*
    /// to meet a smaller maximum rather than refusing one. So a ceiling allowed to fall below
    /// [`MIN_CONTENT`] would not clamp the pane at its floor; it would carry the floor down
    /// with it and let the pane collapse to nothing.
    #[test]
    fn the_ceiling_never_falls_through_the_pane_s_own_floor() {
        let chrome = 24.0; // ZONE_PAD 12 a side + the 2px edge, as `show` computes it.
        let floor = MIN_CONTENT + chrome;
        let ceiling = |available: f32| (available - CENTRE_MIN).max(floor);

        // Wide: the pane keeps its default and the ceiling is not the binding constraint.
        assert!(
            ceiling(978.0) > CONTENT + chrome,
            "1180 wide leaves room for both"
        );
        // Narrow: the pane is pinned at its own minimum, never under it.
        assert_eq!(
            ceiling(358.0),
            floor,
            "560 wide: at the floor, not through it"
        );
        assert_eq!(
            ceiling(0.0),
            floor,
            "and it stays there however narrow it gets"
        );
    }
}
