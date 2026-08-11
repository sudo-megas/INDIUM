//! The centre zone: the entry table, and the two list views the sidebar can select.
//!
//! CORE §4: "virtualized; columns Name, Size, Packed, Method; a breadcrumb path above
//! it." P1 §3 requires the virtualisation to be real — "the virtualized table that
//! lets a 100,000-entry archive scroll like a 100-entry one" (CORE §2).

use eframe::egui;
use egui_extras::{Column, TableBuilder};

use super::{filter, Indium, Section, Status};
use crate::model::Row;
use crate::platform::picker::PickerFor;
use crate::theme;
use crate::util;

const ROW_HEIGHT: f32 = 20.0;

/// The padding of one Recents or Bookmarks line. Both lists wear the same one, because they
/// are the same shape of thing and the sidebar switches between them.
const LIST_PAD: egui::Margin = egui::Margin::symmetric(12, 7);

pub fn show(app: &mut Indium, root: &mut egui::Ui, rows: &[Row]) {
    let ctx = root.ctx().clone();
    egui::CentralPanel::default()
        // The table is a well rather than a raised zone, so this card is `WINDOW` where the
        // other three are `PANEL` (P7 §1).
        //
        // Inner margin 4, not 0 and not 2. `StrokeKind::Inside` puts the 2px edge within the
        // frame rect, so at an inner margin of 0 or 2 the scrollbar and the first row's
        // full-bleed selection fill would sit on the rim; 4 leaves two clear pixels inside it.
        //
        // The price is **20px of table width, not 16**: `Frame::total_margin` is
        // `inner_margin + stroke.width + outer_margin` (egui 0.36 `frame.rs`), so each side
        // costs 4 + 2 + 4 and the edge is not free. Every pixel of it comes out of Name,
        // because Name is the only `Column::remainder()` and Size/Packed/Method are
        // `Column::exact`. That is the accepted cost of the floating-card treatment.
        //
        // A `CentralPanel` draws no separator line, so there is none to turn off here.
        .frame(theme::zone(theme::WINDOW).inner_margin(egui::Margin::same(4)))
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

    let ui_ctx = ui.ctx().clone();
    let mut clicked: Option<(usize, bool)> = None;
    let mut descend_into: Option<usize> = None;
    let mut commit_rename = false;

    // Scoped, and never global. `widgets.hovered.bg_fill` is what `egui_extras` paints a
    // hovered row with, and it is also the fill behind a hovered checkbox and a hovered
    // slider rail — setting it in `install_visuals` would wash all three. And it is
    // deliberately `ROW_HOVER` rather than `AUBERGINE`. CORE §6 gives Aubergine the
    // pointer's resting place and names this table as the one exception, which is what
    // the exception is for: a sidebar has six rows and a hover is an event, while this
    // list is full-height and virtualised, so a pointer crossing it would flare Aubergine
    // at 1.72:1 over the ground on row after row. `ROW_HOVER` is the same meaning at a
    // weight a hundred rows can carry, and it stays out of the way of the two signals
    // that matter here — the orange selection wash and the cursor's edge.
    //
    // `selectable_labels` off is the other half, and without it this whole section is
    // dead. egui's default is `true`, which makes every plain `ui.label` allocate with
    // `Sense::click_and_drag()` so its text can be selected; a row's hover comes from
    // `TableRow`'s union of its *cell* responses, and egui marks only the topmost sensing
    // widget under the pointer as hovered. A selectable label therefore blanks the
    // highlight over exactly the text — which is most of every row. Nothing in the entry
    // table is text to select; the Inspector's explicit `.selectable(true)` fields are
    // where selecting a path belongs, and they are untouched.
    ui.scope(|ui| {
        ui.visuals_mut().widgets.hovered.bg_fill = theme::ROW_HOVER;
        ui.style_mut().interaction.selectable_labels = false;

        // Where the cursor row landed, so its ring can be drawn after the table rather than
        // inside it. `egui_extras` paints striped → selected → hovered → content and offers
        // no cursor layer at all, so there is nowhere inside a cell to put this: a stroke
        // drawn there would be painted over by the next cell's fill.
        let mut cursor_rect: Option<egui::Rect> = None;

        let mut table = TableBuilder::new(ui)
            .striped(false)
            // **Not resizable, and that is what makes the columns track the window.**
            // `egui_extras` remembers a *resizable* column as `Size::exact(previous width)`
            // and then ignores the available width entirely — so the one column that must
            // follow the window, `Name`, was the one being frozen at whatever width it had
            // on the first frame. Widening left a dead strip to the right of `Method` with
            // the row's wash running under it; narrowing clipped instead of giving way.
            // P14 first set this on the three *exact* columns, which changed nothing,
            // because those already had nothing to solve. Solving from the spec every frame
            // is what `Column::remainder` meant, and hand-dragging a column boundary is not
            // something CORE asks for anywhere.
            .resizable(false)
            // `egui_extras` gates its hover fill on `self.sense.interactive()`, and the default
            // is `Sense::hover()`, which is not. This one line is what switches row hover on.
            .sense(egui::Sense::click())
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::remainder().at_least(120.0).clip(true))
            .column(Column::exact(84.0))
            .column(Column::exact(84.0))
            .column(Column::exact(72.0));

        // Only when the keyboard moved it, and only for the one frame the flag is up.
        // Asking every frame would fight the wheel: scroll away to read something and the
        // view would snap back before the pointer stopped moving.
        if std::mem::take(&mut app.scroll_to_cursor) && app.cursor < rows.len() {
            table = table.scroll_to_row(app.cursor, Some(egui::Align::Center));
        }

        table
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
                        // `F2` turns this cell into a text field rather than opening a
                        // tenth popup — CORE §4 fixes the count at nine. A focused field
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
                            // No `.sense(Sense::click())` any more, and that is the whole of
                            // why hover works. The cell's own sense — `Sense::click()`, from
                            // the builder above — is registered *below* whatever the cell
                            // contains, so a click-sensing label sat on top of it and took both
                            // the click and the hover, leaving the row unhighlighted over
                            // exactly the filename. The click and the double-click now come off
                            // the row's unioned cell response below, which is strictly more:
                            // the whole line answers, not just the width of the text.
                            ui.add(egui::Label::new(text).truncate());
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

                    // The union of the four cells, which is the whole line. Until P7 only the
                    // Name *text* was clickable, so clicking a row's size selected nothing;
                    // `TableRow::response` ORs every cell's response, so the line answers
                    // wherever you land on it. Safe to call: `col` has run four times, and
                    // that is the API's only precondition.
                    //
                    // The double-click reaches this the same way, so descending into a
                    // directory now works from any column rather than from the name alone.
                    let line = tr.response();
                    if focused {
                        // **The row, not the cells.** `TableRow::col` hands back each cell's
                        // *content* rect — the extent of the text drawn in it, not the column
                        // it was given — so a union of the first and last was only ever as
                        // wide as the words: measured, `212..266` for a name against a row of
                        // `212..645`.
                        //
                        // P13 changed this line the other way, from the row to the cells, on
                        // the strength of a report that the ring stuck out past the wash. The
                        // report was real and the diagnosis was not: the columns had frozen
                        // at their first-frame widths and it was the *wash* that was stopping
                        // short. P14 fixed the columns; with them filling the row again the
                        // row's own rect is exactly what `egui_extras` paints, which is what
                        // this asked for in the first place.
                        cursor_rect = Some(line.rect);
                    }
                    if line.clicked() {
                        clicked = Some((i, ui_ctx.input(|inp| inp.modifiers.ctrl)));
                    }
                    if line.double_clicked() && row.is_dir {
                        descend_into = Some(i);
                    }
                });
            });

        // CORE §6: the keyboard's position in a list is "a line, not a colour".
        //
        // It used to be the filename turning `ORANGE` and nothing else. But moving the
        // cursor also sets the selection (`mod.rs`, the movement block), and the selection
        // is `ORANGE.linear_multiply(0.35)` — so the cursor was orange ink on an orange
        // wash at **2.06:1**, which the testing round reported not as faint but as absent:
        // *"dont know what orange row cursor you talk about. i see none orange thing."*
        // A line and a wash can be read at the same time; two washes cannot.
        //
        // The rect matches `egui_extras`' own `gapless_rect` — `expand2(0.5 * item_spacing)`
        // — so the ring sits exactly on the selection fill rather than a few pixels inside
        // it, and it is square for the same reason: that fill is `CornerRadius::ZERO`, and a
        // rounded ring around a square wash reads as a mistake. `Inside` keeps the 2px
        // within the row instead of bleeding onto its neighbours.
        if let Some(r) = cursor_rect {
            // The trait `round_ui` hangs off; `egui_extras` rounds its fill the same way, and
            // half a pixel of disagreement between ring and wash is visible on a 20px row.
            use egui::emath::GuiRounding as _;
            let gapless = r.expand2(0.5 * ui.spacing().item_spacing).round_ui();
            ui.painter().with_clip_rect(ui.clip_rect()).rect_stroke(
                gapless,
                theme::R_ZONE,
                theme::edge_hot(),
                egui::StrokeKind::Inside,
            );
        }
    });

    if commit_rename {
        app.commit_rename();
    }

    if let Some((i, additive)) = clicked {
        app.cursor = i;
        app.crc_of = None;
        app.forget_preview(&ui_ctx);
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

/// A breadcrumb segment's padding — tight, because a crumb is a chip in a run and not a
/// line in a list.
const CRUMB_PAD: egui::Margin = egui::Margin::symmetric(4, 1);

fn breadcrumb_bar(app: &mut Indium, ui: &mut egui::Ui) {
    let crumbs = crate::model::breadcrumb(&app.cwd);
    let mut go: Option<String> = None;
    // Set inside the closure, acted on after it: `request_picker` needs `&mut app`, which
    // the layout closure is already holding immutably.
    let mut pick = false;

    // The crumb font, resolved rather than written down: `RichText::family` keeps the
    // current text style's size and swaps only the family, and this is that by hand,
    // because the galley has to be built before the row that will hold it.
    let mut font = egui::TextStyle::Body.resolve(ui.style());
    font.family = theme::MONO;

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
                    let galley = ui
                        .ctx()
                        .fonts_mut(|f| f.layout_no_wrap(label.clone(), font.clone(), colour));

                    if last {
                        // Where you already are, which is not somewhere you can go. A row
                        // here would light up and offer a pointing hand for a click that
                        // does nothing, so the last crumb stays a plain label.
                        ui.add(egui::Label::new(galley));
                        continue;
                    }

                    // `theme::row` ends with `set_width(available_width())` — correct for a
                    // full-width list line, and enough to make the first crumb swallow the
                    // whole bar. Allocating exactly the space the galley needs makes that
                    // claim a no-op, and the galley goes straight into the label, so the
                    // text is laid out once rather than twice.
                    let size = galley.size() + CRUMB_PAD.sum();
                    let crumb = ui.allocate_ui(size, |ui| {
                        theme::row(ui, false, CRUMB_PAD, |ui| {
                            ui.add(egui::Label::new(galley));
                        })
                    });
                    if crumb.inner.clicked() {
                        go = Some(path.clone());
                    }
                }

                // **Add files…**, on the breadcrumb row and nowhere else.
                //
                // Until P11 there were two ways to put a file into an archive and both
                // were dead: `Ctrl+V`, which had never once fired, and a drop, which
                // `winit-0.30.13` cannot deliver on Wayland at all. `Ctrl+V` works now,
                // but a chord nobody is told about is not an affordance, and the testing
                // round's flattest sentence was "cannot add files to archive".
                //
                // Here rather than in the tray, which CORE §4 keeps hidden until something
                // is already staged — a control for making the first change cannot live in
                // a zone that only exists after it. And here rather than in the sidebar,
                // because this row already names the directory an add lands in: the button
                // adds *to where the breadcrumb says you are*, which is the one placement
                // that needs no explanation.
                //
                // CORE §4 lists it, as `build/docs/P11.md` ordered; the maker landed the
                // change in his own hand, as with every zone since P6.
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if theme::small_button(ui, egui::RichText::new("Add files…"), true)
                        .on_hover_text("Choose files to stage into this directory")
                        .clicked()
                    {
                        pick = true;
                    }
                });
            });
        });

    if pick {
        app.request_picker(ui.ctx(), PickerFor::Add);
    }
    if let Some(path) = go {
        app.cwd = path;
        app.cursor = 0;
    }
}

// ---------------------------------------------------------------------------
// Recent files — P2 §2
// ---------------------------------------------------------------------------

fn recents_view(app: &mut Indium, ctx: &egui::Context, ui: &mut egui::Ui) {
    // A heading over a list of siblings, so it takes the rule (`theme::section`). The local
    // `header` this replaces drew 17.0 unbolded with no rule at all — a *value's* size used
    // as a heading, which is the confusion P7 §4 exists to end.
    theme::section(ui, "Recent files");

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
    // A double-click on a row whose file is gone. It used to assign `None` over a `forget`
    // that was already `None` — nothing happened, and nothing was said. The `Enter` path
    // in `handle_keys` has answered this honestly since P2, and the two now agree.
    let mut missing: Option<String> = None;

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (i, (path, opened)) in items.iter().enumerate() {
                // P2 §2: "A missing file renders dimmed." No automatic pruning — the
                // list only loses entries by the user's hand or the cap.
                let exists = std::path::Path::new(path).exists();
                let focused = i == app.recents_cursor;

                // `theme::row` paints the focused Aubergine fill, the hover and held fills,
                // and the pointing hand; the frame plus trailing `ui.interact` this replaces
                // painted only the first of those, so the list never answered the pointer.
                let resp = theme::row(ui, focused, LIST_PAD, |ui| {
                    // A selectable label senses click-and-drag and would out-rank the row
                    // registered beneath it, so clicking a filename would do nothing. See
                    // `sidebar::row_body` for the mechanism.
                    ui.style_mut().interaction.selectable_labels = false;
                    // On the focused row the ground is Aubergine, where TEXT_MUTED measures
                    // 3.30:1 — under AA, and unmeasured until P18 because AUBERGINE was not
                    // in theme's `GROUNDS`. One tier up is 5.01:1 and still reads as the
                    // quiet half; a row that is not focused keeps the muted ink it always had.
                    let dim = if focused {
                        theme::TEXT_SECONDARY
                    } else {
                        theme::TEXT_MUTED
                    };
                    ui.vertical(|ui| {
                        let name = std::path::Path::new(path)
                            .file_name()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_else(|| path.clone());
                        ui.label(egui::RichText::new(name).color(if exists {
                            theme::TEXT
                        } else {
                            dim
                        }));
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(path)
                                    .family(theme::MONO)
                                    .size(13.0)
                                    .color(dim),
                            );
                            if !exists {
                                ui.label(egui::RichText::new("missing").size(12.0).color(dim));
                            }
                        });
                        ui.label(
                            egui::RichText::new(util::format_timestamp(*opened))
                                .family(theme::MONO)
                                .size(12.0)
                                .color(dim),
                        );
                    });
                });

                // **One click opens.** It took a double click until P11, and a recents list
                // is not a file manager's pane — a row here names one archive and has
                // exactly one thing it can do. The double-click requirement read as the
                // list being broken rather than as a list wanting a second click: single
                // clicks appeared to do nothing at all, and only clicking fast enough to
                // register a double ever opened anything.
                if resp.clicked() {
                    app.recents_cursor = i;
                    if exists {
                        open_this = Some(path.clone());
                    } else {
                        missing = Some(path.clone());
                    }
                }
                resp.context_menu(|ui| {
                    if theme::button(ui, egui::RichText::new("Remove from list"), true).clicked() {
                        forget = Some(path.clone());
                        ui.close();
                    }
                });
            }
        });

    if let Some(p) = open_this {
        app.open_archive(ctx, std::path::PathBuf::from(p), None);
    }
    if let Some(p) = missing {
        // P2 §2 keeps the row: the list loses entries by the user's hand or the cap, not
        // because a drive happened to be unmounted this morning.
        app.status = Status::bad(format!("{p} is no longer there."));
    }
    if let Some(p) = forget {
        // Status first, save last: a write that failed owns the line, rather than losing
        // it to a sentence about a removal the file on disk never heard of.
        app.status = format!("Removed {p} from recent files.").into();
        app.change_recents(|r| r.remove(&p));
    }
}

// ---------------------------------------------------------------------------
// Bookmarks — P2 §2
// ---------------------------------------------------------------------------

fn bookmarks_view(app: &mut Indium, ui: &mut egui::Ui) {
    theme::section(ui, "Bookmarks");
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
    // Set inside the loop, applied after it: the loop holds `app.settings.bookmarks`
    // immutably, and `app.cursor` cannot be written through that borrow.
    let mut focus: Option<usize> = None;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (i, b) in app.settings.bookmarks.iter().enumerate() {
                let focused = i == app.bookmarks_cursor;
                let resp = theme::row(ui, focused, LIST_PAD, |ui| {
                    // See `sidebar::row_body`: a selectable label would out-rank the row
                    // beneath it and eat the click that lands on a bookmark's name.
                    ui.style_mut().interaction.selectable_labels = false;
                    // A bookmark names a directory that may since have been deleted,
                    // renamed or unmounted. Recents have said so since P2; bookmarks said
                    // nothing at all, so a `settings.toml` naming a path that was never
                    // there read exactly like one that was. Same word, same dimming, same
                    // rule — the row stays until the user's own hand removes it.
                    let exists = std::path::Path::new(&b.path).is_dir();
                    // The focused row's ground is Aubergine; see the recents list above for
                    // the measurement and the reason nothing caught it until P18.
                    let dim = if focused {
                        theme::TEXT_SECONDARY
                    } else {
                        theme::TEXT_MUTED
                    };
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(&b.name).color(if exists {
                                    theme::TEXT
                                } else {
                                    dim
                                }));
                                if !exists {
                                    ui.label(egui::RichText::new("missing").size(12.0).color(dim));
                                }
                            });
                            ui.label(
                                egui::RichText::new(&b.path)
                                    .family(theme::MONO)
                                    .size(13.0)
                                    .color(dim),
                            );
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            // The `×` is a real `Button`, and `theme::row` registers its own
                            // sense *below* everything added inside it, so the button wins
                            // the hit test outright — removing a bookmark does not also
                            // move the cursor onto it. That is `UiBuilder::sense`'s stated
                            // property, not a coincidence of geometry.
                            if theme::small_button(ui, egui::RichText::new("×"), true).clicked() {
                                remove = Some(i);
                            }
                        });
                    });
                });
                // New at P7: the row had no click of its own before, which left it hovering
                // and offering a pointing hand for nothing. It now does what a Recents row
                // does — move the keyboard cursor onto the line you touched.
                if resp.clicked() {
                    focus = Some(i);
                }
            }
        });

    if let Some(i) = focus {
        app.bookmarks_cursor = i;
    }
    if let Some(gone) = remove.and_then(|i| app.settings.bookmarks.get(i).cloned()) {
        let name = gone.name.clone();
        app.change_settings(move |s| s.bookmarks.retain(|b| *b != gone));
        app.status = format!("Removed bookmark {name}.").into();
    }
}

// ---------------------------------------------------------------------------
// Shared bits
// ---------------------------------------------------------------------------

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
