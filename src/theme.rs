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
pub const WINDOW: Color32 = Color32::from_rgb(0x22, 0x12, 0x26);
/// Raised panels: sidebar, Inspector, popups.
pub const PANEL: Color32 = Color32::from_rgb(0x2B, 0x18, 0x30);
/// The status bar, darker than the window so it reads as a floor.
pub const STATUS_BAR: Color32 = Color32::from_rgb(0x1C, 0x0F, 0x20);

// --- Structure ----------------------------------------------------------------
/// Canonical Aubergine — selection context and the active sidebar item.
pub const AUBERGINE: Color32 = Color32::from_rgb(0x77, 0x29, 0x53);

// --- Accent -------------------------------------------------------------------
/// Ubuntu Orange. CORE §6 reserves it for exactly three meanings: the current
/// selection, staged changes, and Apply/progress. "Orange means *something will
/// happen.*" Nothing decorative may take this colour.
pub const ORANGE: Color32 = Color32::from_rgb(0xE9, 0x54, 0x20);

// --- Text ---------------------------------------------------------------------
pub const TEXT: Color32 = Color32::from_rgb(0xF0, 0xE6, 0xEE);
pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(0xC9, 0xB3, 0xC4);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(0xA9, 0x8B, 0xA3);

// --- Lines --------------------------------------------------------------------
/// 1px hairlines at 8% white. CORE §6: "Nothing thicker, anywhere."
pub const HAIRLINE: Color32 = Color32::from_rgba_premultiplied(0x14, 0x14, 0x14, 0x14);

pub fn hairline() -> Stroke {
    Stroke::new(1.0, HAIRLINE)
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

    v.widgets.inactive.bg_fill = PANEL;
    v.widgets.inactive.weak_bg_fill = PANEL;
    v.widgets.inactive.bg_stroke = hairline();
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);

    v.widgets.hovered.bg_fill = AUBERGINE;
    v.widgets.hovered.weak_bg_fill = AUBERGINE;
    v.widgets.hovered.bg_stroke = hairline();
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT);

    v.widgets.active.bg_fill = AUBERGINE;
    v.widgets.active.weak_bg_fill = AUBERGINE;
    v.widgets.active.bg_stroke = Stroke::new(1.0, ORANGE);
    v.widgets.active.fg_stroke = Stroke::new(1.0, TEXT);

    // Selection is one of orange's three permitted meanings.
    v.selection.bg_fill = ORANGE.linear_multiply(0.35);
    v.selection.stroke = Stroke::new(1.0, ORANGE);

    v.window_shadow = egui::epaint::Shadow::NONE;
    v.popup_shadow = egui::epaint::Shadow::NONE;

    ctx.set_visuals(v);
}

pub fn install(ctx: &egui::Context) {
    install_fonts(ctx);
    install_visuals(ctx);
}
