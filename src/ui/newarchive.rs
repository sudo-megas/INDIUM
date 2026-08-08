//! New Archive — CORE §4's first popup, `N`, built by P4 §5.
//!
//! CORE §4.1: "An instruction line at top. Four preset chips — *Fastest*, *Balanced*
//! (default), *Smallest*, *Encrypted* — each highlighting a row in the method list below,
//! where **every method carries its one-sentence verdict** (§5). An *Advanced* disclosure
//! holds the level slider. At the foot, a live sentence states exactly what will be
//! built."
//!
//! Copyright © sudo-megas. GPL-3.0-only.

use std::path::PathBuf;

use eframe::egui;

use super::{extract, Indium, Popup};
use crate::tasks::{self, Method, Preset, Recipe, Task, METHODS};
use crate::theme;

/// CORE §4.1's instruction line, verbatim.
const INSTRUCTION: &str = "Choose how INDIUM should compress. If unsure, keep the default.";

pub fn show(app: &mut Indium, ctx: &egui::Context) {
    if app.popup != Some(Popup::NewArchive) {
        return;
    }
    let mut open = true;
    let mut create = false;

    egui::Window::new("New Archive")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .open(&mut open)
        .show(ctx, |ui| {
            ui.set_min_width(620.0);

            ui.label(
                egui::RichText::new(INSTRUCTION)
                    .size(13.0)
                    .color(theme::TEXT_SECONDARY),
            );
            ui.add_space(8.0);

            // --- name and destination ---
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Name")
                        .size(12.0)
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
                        .size(13.0)
                        .color(theme::TEXT_MUTED),
                );
            });
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("In")
                        .size(12.0)
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
                                .size(12.0)
                                .color(theme::TEXT_MUTED),
                        );
                        ui.label(
                            egui::RichText::new(&completed)
                                .family(theme::MONO)
                                .size(12.0)
                                .color(theme::TEXT_MUTED),
                        );
                    });
                    if ui.input(|i| i.key_pressed(egui::Key::Tab)) {
                        app.new_dir = completed;
                    }
                }
            }

            ui.add_space(10.0);
            group(ui, "PRESET");
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

            ui.add_space(10.0);
            group(ui, "METHOD");
            egui::ScrollArea::vertical()
                .max_height(210.0)
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
                                    .size(12.0)
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
                                .size(12.0)
                                .italics()
                                .color(theme::TEXT_MUTED),
                            );
                        }
                    }
                });
            app.new_advanced = advanced.openness > 0.0;

            ui.add_space(10.0);
            ui.separator();

            // CORE §4.1's live sentence: "states exactly what will be built".
            let recipe = recipe_of(app);
            ui.label(
                egui::RichText::new(tasks::recipe_sentence(&recipe))
                    .family(theme::MONO)
                    .size(13.0)
                    .color(theme::TEXT_SECONDARY),
            );
            if app.new_encrypt {
                ui.label(
                    egui::RichText::new(
                        "You will be asked for the password when you Apply, and again to \
                         confirm it. INDIUM never stores it.",
                    )
                    .size(12.0)
                    .italics()
                    .color(theme::TEXT_MUTED),
                );
            }

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let ready = !app.new_name.trim().is_empty() && !app.new_dir.trim().is_empty();
                create = ui
                    .add_enabled(
                        ready,
                        egui::Button::new(egui::RichText::new("Create").color(theme::ORANGE)),
                    )
                    .clicked();
                ui.label(
                    egui::RichText::new("Nothing is written until you Apply · Esc closes")
                        .size(12.0)
                        .color(theme::TEXT_MUTED),
                );
            });
        });

    if create {
        let recipe = recipe_of(app);
        app.stage_creation(recipe);
        // A staged creation adopts an archive that does not exist yet, and the title
        // should name it just as an opened one does.
        app.set_window_title(ctx);
        app.popup = None;
    }
    if !open {
        app.close_popup();
    }
}

/// One method, with its CORE §5 verdict underneath.
fn method_row(ui: &mut egui::Ui, app: &mut Indium, method: Method) {
    let selected = app.new_method == method;
    let frame = egui::Frame::NONE
        .inner_margin(egui::Margin::symmetric(8, 5))
        .fill(if selected {
            theme::AUBERGINE
        } else {
            theme::PANEL
        });

    let inner = frame.show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(method.label())
                        .family(theme::bold())
                        .size(14.0)
                        .color(theme::TEXT),
                );
                if method == Method::Lzma2 {
                    ui.label(
                        egui::RichText::new("AES-256")
                            .family(theme::MONO)
                            .size(11.0)
                            .color(theme::TEXT_MUTED),
                    );
                }
            });
            // Verbatim from CORE §5, held once in `tasks` and pinned by a test.
            ui.label(
                egui::RichText::new(method.verdict())
                    .size(12.0)
                    .italics()
                    .color(theme::TEXT_SECONDARY),
            );
        });
    });

    let response = ui.interact(
        inner.response.rect,
        ui.id().with(("method", method.label())),
        egui::Sense::click(),
    );
    if response.clicked() {
        app.new_method = method;
        app.new_level = method.default_level();
        // Encryption belongs to 7z alone (CORE §9), so choosing anything else drops it.
        if method != Method::Lzma2 {
            app.new_encrypt = false;
        }
        app.new_preset = preset_for(method, app.new_encrypt);
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand);
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

fn group(ui: &mut egui::Ui, title: &str) {
    ui.label(
        egui::RichText::new(title)
            .size(12.0)
            .color(theme::TEXT)
            .family(theme::bold()),
    );
    ui.add_space(3.0);
}

impl Indium {
    /// Stage `Task::Create`, and adopt the archive that does not exist yet.
    ///
    /// P4 §1: New Archive writes nothing. The window takes the chosen path, the entry
    /// list is empty, the tray appears, and Apply builds it through the same lock,
    /// verify and rename as every other rebuild. Writing an empty archive here instead
    /// would touch the disk before anything had been staged, which is the surprise CORE
    /// §3's ethic exists to prevent.
    pub fn stage_creation(&mut self, recipe: Recipe) {
        let name = recipe
            .path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        self.archive_path = Some(recipe.path.clone());
        self.archive_bytes = 0;
        self.archive_info = None;
        self.entries.clear();
        self.selection.clear();
        self.cwd.clear();
        self.cursor = 0;
        self.section = super::Section::Archive;
        self.staged_against.clear();
        self.tasks.clear();
        self.tasks.push(Task::Create { recipe });
        self.status = format!("Staged: create {name}. Add files, then Apply.");
    }
}
