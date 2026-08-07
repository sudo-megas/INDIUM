//! Ubuntu Canonical Aubergine, and the two embedded typefaces.
//!
//! CORE §6: "One theme: Ubuntu Canonical Aubergine. There is no second theme and no
//! theme setting." Every value below is transcribed from that section; none of them is
//! configurable, and CORE §9 forbids adding a control that would make them so.

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

/// The name of the monospace family. CORE §6: "Ubuntu Mono for every value — sizes,
/// checksums, paths, the whole Inspector. Monospace is what makes a verbose pane
/// scannable instead of noisy."
pub const MONO: FontFamily = FontFamily::Monospace;
/// Ubuntu Sans, for chrome.
pub const SANS: FontFamily = FontFamily::Proportional;

/// Embed the typefaces and make them the only ones.
///
/// The files are bundled assets, not dependencies (CORE §2), under the Ubuntu Font
/// Licence 1.0 in `LICENSES/`.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    fonts.font_data.insert(
        "ubuntu-sans".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../assets/fonts/UbuntuSans-Regular.ttf"
        ))),
    );
    fonts.font_data.insert(
        "ubuntu-sans-bold".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../assets/fonts/UbuntuSans-Bold.ttf"
        ))),
    );
    fonts.font_data.insert(
        "ubuntu-mono".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../assets/fonts/UbuntuSansMono-Regular.ttf"
        ))),
    );

    // Our faces go first; egui's bundled fallbacks stay behind them so a CJK or
    // emoji filename still renders something rather than tofu.
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "ubuntu-sans".to_owned());
    fonts
        .families
        .entry(FontFamily::Monospace)
        .or_default()
        .insert(0, "ubuntu-mono".to_owned());

    fonts.families.insert(
        FontFamily::Name("ubuntu-sans-bold".into()),
        vec!["ubuntu-sans-bold".to_owned(), "ubuntu-sans".to_owned()],
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
