//! Ubuntu Canonical Aubergine, and the embedded typeface.
//!
//! CORE §6: "One theme: Ubuntu Canonical Aubergine. There is no second theme and no
//! theme setting." Every value below is transcribed from that section; none of them is
//! configurable, and CORE §9 forbids adding a control that would make them so.
//!
//! The typeface is JetBrains Mono NL Nerd Font, in two weights, and it is the only one:
//! CORE §6 puts the whole window in monospace rather than splitting chrome from values,
//! so chrome and values are told apart by weight and colour instead of by family.

use eframe::egui;
use egui::{Color32, FontData, FontDefinitions, FontFamily, Stroke};
use std::sync::Arc;

// --- Base ---------------------------------------------------------------------
/// The window itself.
///
/// The maker's own terminal ground, adopted at P5. It sits at hue 319° against the old
/// value's 288°, which puts it in the same family as Canonical Aubergine (328°) — so the
/// window became *more* faithful to CORE §6's structural colour, not less.
pub const WINDOW: Color32 = Color32::from_rgb(0x30, 0x0A, 0x24);
/// Raised panels: sidebar, Inspector, popups. One step lighter, same hue.
pub const PANEL: Color32 = Color32::from_rgb(0x3D, 0x0D, 0x2E);
/// The status bar, darker than the window so it reads as a floor.
pub const STATUS_BAR: Color32 = Color32::from_rgb(0x24, 0x07, 0x1B);

// --- Structure ----------------------------------------------------------------
/// Canonical Aubergine — selection context and the active sidebar item.
pub const AUBERGINE: Color32 = Color32::from_rgb(0x77, 0x29, 0x53);

// --- Accent -------------------------------------------------------------------
/// Ubuntu Orange. CORE §6 reserves it for exactly three meanings: the current
/// selection, staged changes, and Apply/progress. "Orange means *something will
/// happen.*" Nothing decorative may take this colour.
pub const ORANGE: Color32 = Color32::from_rgb(0xE9, 0x54, 0x20);

// --- Text ---------------------------------------------------------------------
/// Primary text. Neutral rather than tinted: sampled from the maker's terminal, where a
/// near-white on aubergine is what he actually reads all day.
pub const TEXT: Color32 = Color32::from_rgb(0xEE, 0xEE, 0xEC);
pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(0xBD, 0xBD, 0xBB);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(0x99, 0x99, 0x97);

// --- Warning ------------------------------------------------------------------
/// CORE §6's fourth colour, and the narrowest: a wrong password, two passwords that
/// differ, a settings file that would not parse. Nothing else.
///
/// It exists because those three messages used to be painted orange, which falsified §6's
/// own sentence — "Orange means *something will happen*" — at exactly the moments when
/// nothing could.
pub const WARNING: Color32 = Color32::from_rgb(0xFF, 0xD8, 0x00);

// --- Lines --------------------------------------------------------------------
/// 1px hairlines at 8% white. CORE §6: "Nothing thicker, anywhere."
pub const HAIRLINE: Color32 = Color32::from_rgba_premultiplied(0x14, 0x14, 0x14, 0x14);

pub fn hairline() -> Stroke {
    Stroke::new(1.0, HAIRLINE)
}

/// "This mode is active", in Aubergine, for every `selectable_label` in this `Ui`.
///
/// CORE §6 gives Aubergine "selection context, active sidebar item", and reserves orange for
/// the current selection, staged changes, and Apply/progress — orange means *something will
/// happen*. A tab, a preset chip and a toggle say only *which mode is active*; nothing is about
/// to happen because a tab is open. So they are Aubergine's work, and always were: the sidebar,
/// the New Archive method rows and the focused Recents and Bookmarks rows have hand-painted
/// Aubergine for this exact meaning since P1. P6 §6.6.
///
/// **It is scoped rather than set in [`install_visuals`], and that is not a preference.** egui
/// resolves a `selectable_label`'s selected fill from `Visuals::selection::bg_fill` — the same
/// field `egui_extras` reads to paint the entry table's row cursor, which genuinely *is* the
/// current selection and keeps orange. Setting it globally would repaint the one thing §6
/// requires to stay. `visuals_mut` is clone-on-write per `Ui`, and `horizontal` and
/// `horizontal_wrapped` already open a child `Ui`, so the override dies with the row.
///
/// `selection.stroke` is deliberately left alone: egui does not take a selected button's
/// outline from it, and every one of these labels sets its text colour explicitly, so the
/// field reaches nothing here.
///
/// The hover fill moves with it, and has to. `widgets.hovered.weak_bg_fill` is Aubergine for
/// the whole program, so the moment the *selected* fill became Aubergine a chip under the
/// pointer painted exactly like the chosen one — and with four preset chips in a row the
/// pointer is nearly always on one. A fill that means both "active" and "the mouse is here"
/// means neither.
pub fn active_fill(ui: &mut egui::Ui) {
    let v = ui.visuals_mut();
    v.selection.bg_fill = AUBERGINE;
    v.widgets.hovered.weak_bg_fill = WINDOW;
    v.widgets.hovered.bg_fill = WINDOW;
}

/// CORE §6: "monospace for every value — sizes, checksums, paths, the whole Inspector.
/// Monospace is what makes a verbose pane scannable instead of noisy."
///
/// `MONO` and `SANS` now resolve to the same face. Both names are kept because egui's
/// two default families are still distinct — `Monospace` is what the table and the
/// Inspector ask for — and naming a value's family at the call site keeps saying what it
/// means, whatever is installed behind it.
pub const MONO: FontFamily = FontFamily::Monospace;
/// Chrome. The same face as [`MONO`]; see its note.
pub const SANS: FontFamily = FontFamily::Proportional;

/// The bold weight.
///
/// This cannot be a `const` like [`MONO`] and [`SANS`] — `FontFamily::Name` holds an
/// `Arc<str>`. Reach for it instead of `RichText::strong()`, which is a colour change and
/// never a weight change: `strong_text_color()` resolves to `widgets.active`'s colour,
/// which this theme sets to the same `TEXT` that `override_text_color` already forces, so
/// `.strong()` renders identically to ordinary text and always did.
pub fn bold() -> FontFamily {
    FontFamily::Name("jetbrains-bold".into())
}

/// Embed the typeface and make it the only one.
///
/// The files are bundled assets, not dependencies (CORE §2), under the SIL Open Font
/// Licence 1.1 in `LICENSES/`. `NL` is the no-ligature cut: a filename holding `->` or
/// `!=` must render as the bytes the archive stores, not as the glyph a programmer's font
/// would rather show. `…NerdFontMono…` is the single-cell icon cut, so an icon occupies
/// one column and the entry table stays aligned.
pub fn install_fonts(ctx: &egui::Context) {
    // `empty()`, not `default()`. `eframe` is built without `default_fonts`, so
    // `default()` already returns nothing — but it returns nothing *by side effect of a
    // feature flag*, and saying `empty()` means the same thing on purpose.
    let mut fonts = FontDefinitions::empty();

    fonts.font_data.insert(
        "jetbrains".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../assets/fonts/JetBrainsMonoNLNerdFontMono-Regular.ttf"
        ))),
    );
    fonts.font_data.insert(
        "jetbrains-bold".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../assets/fonts/JetBrainsMonoNLNerdFontMono-Bold.ttf"
        ))),
    );

    // One face in both default families, because CORE §6 puts the whole window in
    // monospace. There is no fallback behind it and no pretending otherwise: this face
    // carries 12,218 codepoints — Latin, the arrows and box-drawing, and some ten
    // thousand Nerd icons — but no CJK and no emoji, so a filename in Japanese or with an
    // emoji in it renders as tofu. That is the honest cost of embedding one face and
    // linking no fontconfig, and it is stated here so it is a known limit, not a bug
    // report waiting to happen.
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .insert(0, "jetbrains".to_owned());
    }

    fonts.families.insert(
        FontFamily::Name("jetbrains-bold".into()),
        vec!["jetbrains-bold".to_owned(), "jetbrains".to_owned()],
    );

    ctx.set_fonts(fonts);
}

/// Apply the palette.
pub fn install_visuals(ctx: &egui::Context) {
    let mut v = egui::Visuals::dark();

    v.override_text_color = Some(TEXT);
    v.panel_fill = PANEL;
    v.window_fill = PANEL;
    v.extreme_bg_color = WINDOW;
    v.faint_bg_color = PANEL;
    v.window_stroke = hairline();

    v.widgets.noninteractive.bg_fill = PANEL;
    v.widgets.noninteractive.weak_bg_fill = PANEL;
    v.widgets.noninteractive.bg_stroke = hairline();
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_SECONDARY);

    // An interactive surface must not be the colour of the thing behind it. These were
    // all `PANEL`, and popups are `PANEL`, so every button was a bare label inside an
    // invisible box, every unfocused text field had no boundary at all, and the checkbox
    // was a panel-coloured square on a panel. P4 diagnosed exactly this on the slider and
    // patched it in one popup; the cause was always here.
    v.widgets.inactive.bg_fill = WINDOW;
    v.widgets.inactive.weak_bg_fill = WINDOW;
    v.widgets.inactive.bg_stroke = hairline();
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);

    v.widgets.hovered.bg_fill = AUBERGINE;
    v.widgets.hovered.weak_bg_fill = AUBERGINE;
    v.widgets.hovered.bg_stroke = hairline();
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT);

    v.widgets.active.bg_fill = AUBERGINE;
    v.widgets.active.weak_bg_fill = AUBERGINE;
    // Not orange. `widgets.active` is the being-pressed state of *every* widget, so an
    // orange stroke here put CORE §6's accent around Cancel, Discard, Clear list and the
    // checkbox — orange as a generic press affordance, which is decoration, which the
    // comment on ORANGE above forbids in its own words.
    v.widgets.active.bg_stroke = hairline();
    v.widgets.active.fg_stroke = Stroke::new(1.0, TEXT);

    // Selection is one of orange's three permitted meanings.
    v.selection.bg_fill = ORANGE.linear_multiply(0.35);
    v.selection.stroke = Stroke::new(1.0, ORANGE);

    v.window_shadow = egui::epaint::Shadow::NONE;
    v.popup_shadow = egui::epaint::Shadow::NONE;

    ctx.set_visuals(v);
    install_spacing(ctx);
}

/// The shape of the window: type scale, spacing, and scrollbars that exist.
///
/// None of this was ever set, so egui's defaults have been quietly in charge of the two
/// most prominent sizes in the program — 13.0 for every unstyled label and button, 18.0
/// for every popup title — neither of which appears anywhere in the source. Writing them
/// down is most of the fix.
fn install_spacing(ctx: &egui::Context) {
    use egui::{FontId, TextStyle};

    // `all_styles_mut` rather than `set_style`: egui 0.36 keeps a style per theme, and
    // INDIUM has exactly one look (CORE §6), so both get the same one.
    ctx.all_styles_mut(|style| {
        // Four roles, not ten sizes. Body is the value you read; Button matches it so a
        // button never shouts; Small is a label or a hint; Heading is a popup title, which
        // egui resolves for us and which had been 18.0 by nobody's decision.
        style.text_styles = [
            (
                TextStyle::Heading,
                FontId::new(17.0, FontFamily::Proportional),
            ),
            (TextStyle::Body, FontId::new(13.0, FontFamily::Proportional)),
            (
                TextStyle::Button,
                FontId::new(13.0, FontFamily::Proportional),
            ),
            (
                TextStyle::Small,
                FontId::new(12.0, FontFamily::Proportional),
            ),
            (
                TextStyle::Monospace,
                FontId::new(13.0, FontFamily::Monospace),
            ),
        ]
        .into();

        // egui's defaults are 3px between stacked widgets and 1px of vertical button padding,
        // which is why every popup read as a dense block. A verbose pane has to breathe or the
        // verbosity is just noise.
        style.spacing.item_spacing = egui::vec2(8.0, 5.0);
        style.spacing.button_padding = egui::vec2(8.0, 3.0);

        // Scrollbars were floating with zero dormant opacity and zero allocated width —
        // invisible until hovered, and painted *over* the content rather than beside it. The
        // Inspector is the pane this program exists for and had no cue that it scrolled.
        style.spacing.scroll = egui::style::ScrollStyle::solid();
    });
}

pub fn install(ctx: &egui::Context) {
    install_fonts(ctx);
    install_visuals(ctx);
}
