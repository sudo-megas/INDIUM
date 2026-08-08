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
//
// Six grounds, each between 1.37 and 1.87 times the *linear* luminance of the one below,
// all on hue 318–319° — the same family as Canonical Aubergine's 328°, which is the reason
// CORE §6 gives for `WINDOW` in the first place.
//
// P7 measured the old three and found the arithmetic behind every complaint the maker made
// about the look: adjacent grounds were **1.06–1.10:1** apart and the hairline between them
// **1.23:1**. INDIUM had been separating one zone from the next with a step of fill and an
// 8%-white line, and neither is strong enough to separate anything. The ladder is now the
// quiet voice that says *which way is up*; the gap and the 2px edge do the separating.

/// The gutter every zone floats in. Not a surface — the absence of one, and the darkest
/// thing in the window so that everything else reads as sitting above it.
pub const VOID: Color32 = Color32::from_rgb(0x18, 0x04, 0x12);
/// The status bar, darker than the window so it reads as a floor.
pub const STATUS_BAR: Color32 = Color32::from_rgb(0x24, 0x07, 0x1B);
/// **The wells** — the entry table, every text field, the progress track. Anything you look
/// *into* rather than *at*.
///
/// The maker's own terminal ground, adopted at P5. It sits at hue 319° against the old
/// value's 288°, which puts it in the same family as Canonical Aubergine (328°) — so the
/// window became *more* faithful to CORE §6's structural colour, not less.
pub const WINDOW: Color32 = Color32::from_rgb(0x30, 0x0A, 0x24);
/// The raised zones: sidebar, Inspector, tray.
pub const PANEL: Color32 = Color32::from_rgb(0x3D, 0x0D, 0x2E);
/// Popups. A popup covers every zone, so it is lighter than every zone.
pub const POPUP: Color32 = Color32::from_rgb(0x4A, 0x10, 0x38);
/// The resting face of every button, and the lightest resting surface in the window — so a
/// button reads the same on the status bar, on a panel and inside a popup, without knowing
/// which one it is standing on.
pub const CONTROL: Color32 = Color32::from_rgb(0x57, 0x13, 0x42);

// --- Structure ----------------------------------------------------------------
/// Canonical Aubergine — selection context, the active item, and whatever the pointer is
/// resting on. 2.13× `CONTROL` in linear luminance, and that step is what makes hover felt.
pub const AUBERGINE: Color32 = Color32::from_rgb(0x77, 0x29, 0x53);
/// The same colour with the light on, alive only for as long as a control is held down.
///
/// One meaning at two intensities, never two meanings. Pressed-as-*darker* was tried first
/// and rejected on measurement: the natural darker aubergine `#561D3C` comes out at
/// **1.038:1 against `CONTROL`**, so a pressed button would have been indistinguishable
/// from a resting one. At this end of the scale there is only room upward.
pub const AUBERGINE_LIT: Color32 = Color32::from_rgb(0x8F, 0x31, 0x64);

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
//
// Two weights and no third. **Inside** a zone, a hairline. **Around** a zone, a popup or a
// control, an edge. P7 ordered the CORE §6 change that permits the second one; before it,
// the rule was "1px hairlines at 8% white, nothing thicker, anywhere", and the whole of
// the maker's "section borders are not obvious" is that sentence being obeyed.

/// 1px at 8% white — every rule *inside* a zone: beneath a heading, above a footer.
pub const HAIRLINE: Color32 = Color32::from_rgba_premultiplied(0x14, 0x14, 0x14, 0x14);
/// 2px at 22% white — every boundary *around* a zone, a popup or a control.
///
/// Composited over the six grounds it measures **1.88–1.95:1**, where the hairline manages
/// **1.18–1.23:1**. These are translucent, so the figure is the composite against each
/// ground and not the raw byte value, which would mean nothing at all.
pub const EDGE: Color32 = Color32::from_rgba_premultiplied(0x38, 0x38, 0x38, 0x38);
/// The same boundary under the pointer, or while a control is held: **3.26–3.74:1** over the
/// six grounds, and **2.81:1** over Aubergine, which is the ground a hovered control has.
pub const EDGE_HOT: Color32 = Color32::from_rgba_premultiplied(0x66, 0x66, 0x66, 0x66);

/// The wash under a hovered table row, 7% white.
///
/// Deliberately **not** Aubergine. Aubergine means "the active item", and a full-height
/// table where every row you pass turns Aubergine means nothing at all.
///
/// Against the orange selection fill the two separate by **2.24–2.45:1** depending on the
/// ground beneath, so a hovered row and a selected one never read as the same thing. (P7
/// first published 1.40:1 here, which was reproducible by no method at all; the real figure
/// is the safer one.)
pub const ROW_HOVER: Color32 = Color32::from_rgba_premultiplied(0x12, 0x12, 0x12, 0x12);

/// The scrim behind a modal: `VOID` at 78%.
///
/// egui's default is `from_black_alpha(100)`, and pure black over an aubergine window pulls
/// the whole thing grey at the exact moment the user is being asked for a secret. This
/// keeps the window in its own family and is nearly twice as opaque, which is what
/// "everything else is unavailable" should look like.
/// Premultiplied, because `from_rgba_unmultiplied` is not a `const fn`: `VOID`'s channels
/// each scaled by 200/255, which is the same colour written the way a `const` can hold it.
pub const SCRIM: Color32 = Color32::from_rgba_premultiplied(0x13, 0x03, 0x0E, 0xC8);

pub fn hairline() -> Stroke {
    Stroke::new(1.0, HAIRLINE)
}

/// The 2px boundary around a zone, a popup or a control.
pub fn edge() -> Stroke {
    Stroke::new(2.0, EDGE)
}

/// The same boundary, under the pointer or held.
pub fn edge_hot() -> Stroke {
    Stroke::new(2.0, EDGE_HOT)
}

// --- Corners ------------------------------------------------------------------
//
// Three radii, declared. Before P7 the window mixed five — 0, 2, 3, 6 and a 9px progress
// pill — not one of which was written down anywhere.

/// Zones are square, and that is a decision rather than an omission.
///
/// `egui::Frame` does **not** clip its children: it builds one `RectShape` and paints it
/// *behind* the content. A rounded table card would therefore let the header row, and a
/// selected first or last row's full-bleed orange fill, overhang the arc. Square corners
/// make that impossible, they suit a monospace window, and the gutter plus the 2px edge do
/// all the floating.
pub const R_ZONE: u8 = 0;
/// Buttons, chips, hand-rolled rows, the progress track.
pub const R_CTRL: u8 = 3;
/// Popups, menus, the modal.
pub const R_POPUP: u8 = 10;

// --- Spacing ------------------------------------------------------------------

/// The gap between two zones. Applied as `outer_margin` of half this on every zone, because
/// two neighbours each contribute half; the window's own rim is therefore half a gutter,
/// there being nothing on the other side of it.
pub const GUTTER: i8 = 8;
/// A zone's inner margin — the clear space between the edge and the content inside it.
///
/// **The edge costs layout, and P7 first said it did not.** `Frame::total_margin` is
/// `inner_margin + stroke.width + outer_margin`, so a 2px edge takes 2px per side on top of
/// this, and a panel given an `exact_size` that forgot it overflows and paints over its own
/// gutter. The status bar was measured at four pixels short before anyone noticed, because
/// `Panel` clamps the rect it *reports* to `exact_size` and paints the overflow anyway.
pub const PAD: i8 = 12;
/// One status-bar row: the same 20.0 the entry table uses for a row.
pub const SB_ROW: f32 = 20.0;
/// Between status-bar rows.
pub const SB_GAP: f32 = 4.0;
/// Above a section heading.
pub const SECTION_ABOVE: f32 = 14.0;

/// The maker's mark, embedded once and drawn in two places.
///
/// CORE §4's sixth popup asks for "**the mark**, the maker, the version and date" — the word
/// has been in the document since P1 and the popup never drew one. CORE §4 also calls the
/// sidebar "(family style)", and the family — JADEITE — puts the mark above the wordmark.
/// Both are that sentence being kept rather than a new idea.
///
/// **1024, and not the 2048 or 4096 masters**, for a reason that is not aesthetic: those two
/// are gitignored (P5 Deviation 6, 21 MB between them), and `build/package/PKGBUILD` builds
/// from a *git clone of the tag*. `include_bytes!` on a file git does not carry would fail
/// the build for every person who ever packages INDIUM, and would fail it at `cargo build`
/// with a missing-file error rather than anywhere informative. 1024 is the largest master the
/// repository actually holds. It is drawn at 84px in the sidebar and 150px in About, so even
/// a 2× display asks for less than a third of it.
pub const MARK: &[u8] = include_bytes!("../build/icons/indium-1024.png");

/// The mark, at a given edge length. The URI is what egui's texture cache keys on, so it is
/// fixed rather than derived from the size — one decode, one texture, both call sites.
pub fn mark(size: f32) -> egui::Image<'static> {
    egui::Image::from_bytes("bytes://indium-mark.png", MARK)
        .fit_to_exact_size(egui::vec2(size, size))
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
    v.widgets.hovered.weak_bg_fill = CONTROL;
    v.widgets.hovered.bg_fill = CONTROL;
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
    v.extreme_bg_color = WINDOW;
    v.faint_bg_color = PANEL;

    // Popups. `Frame::window` and `Frame::popup` both read `window_fill`, and `Modal` and
    // the context menu resolve to `Frame::popup`, so setting it here is the whole of P7 §3
    // — every popup, the password modal and the right-click menu, with no per-file work.
    //
    // Lighter than every zone, because a popup covers every zone. Recessed was considered
    // and rejected: the window already teaches "lighter is closer" with WINDOW → PANEL, and
    // a darker popup would have to un-teach it while costing contrast on the pane CORE §1
    // calls the main event.
    v.window_fill = POPUP;
    v.window_stroke = edge();
    v.window_corner_radius = R_POPUP.into();
    v.menu_corner_radius = R_POPUP.into();

    // A popup is the only thing in INDIUM that is genuinely above something else, so it is
    // the only thing that casts. P5 turned both of these off without recording a reason,
    // which reads as part of its "write the defaults down" sweep rather than a decision.
    // Zones cast nothing: a zone sits *in* the gutter, not above it, and a shadow there
    // would leak onto its neighbour — which is decoration, and CORE §6 forbids it.
    let shadow = egui::epaint::Shadow {
        offset: [0, 8],
        blur: 20,
        spread: 0,
        color: Color32::from_black_alpha(120),
    };
    v.window_shadow = shadow;
    v.popup_shadow = shadow;

    v.widgets.noninteractive.bg_fill = PANEL;
    v.widgets.noninteractive.weak_bg_fill = PANEL;
    // Stays a hairline, and must. This stroke is what every `ui.separator()` in the program
    // paints with, and those are all rules *inside* a zone. The 2px zone border is applied
    // per-frame by `zone()` instead — putting it here would thicken all nine internal rules
    // and flatten the hierarchy P7 exists to build.
    v.widgets.noninteractive.bg_stroke = hairline();
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_SECONDARY);

    // The four states of every control, and the answer to "clicks doesnt feel anything".
    //
    // Before P7 the whole of the press feedback was **one pixel of corner radius**: hovered
    // and held shared a fill, a stroke and a text colour, and nothing moved. Now three
    // channels change on every transition and two of them are geometry, which is what a
    // finger expects. Rest → hover the fill more than doubles in luminance, the rim goes
    // from 22% to 40% white, and the box grows a pixel on every side. Hover → held the fill
    // brightens by half again and the box contracts two pixels per side, ending a pixel
    // *smaller* than at rest.
    //
    // `expansion` is safe at this size: `item_spacing` is (8, 5) and every frame a button
    // sits in has an inner margin of at least 6, so ±1 never reaches a neighbour.
    v.widgets.inactive.bg_fill = CONTROL;
    v.widgets.inactive.weak_bg_fill = CONTROL;
    v.widgets.inactive.bg_stroke = edge();
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT_SECONDARY);
    v.widgets.inactive.expansion = 0.0;

    v.widgets.hovered.bg_fill = AUBERGINE;
    v.widgets.hovered.weak_bg_fill = AUBERGINE;
    v.widgets.hovered.bg_stroke = edge_hot();
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.hovered.expansion = 1.0;

    v.widgets.active.bg_fill = AUBERGINE_LIT;
    v.widgets.active.weak_bg_fill = AUBERGINE_LIT;
    // Not orange. `widgets.active` is the being-pressed state of *every* widget, so an
    // orange stroke here put CORE §6's accent around Cancel, Discard, Clear list and the
    // checkbox — orange as a generic press affordance, which is decoration, which the
    // comment on ORANGE above forbids in its own words.
    v.widgets.active.bg_stroke = edge_hot();
    v.widgets.active.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.active.expansion = -1.0;

    // The one widget state `install_visuals` never set, and the reason every popup wore a
    // neutral grey title bar: egui fills the header with `widgets.open.weak_bg_fill` when
    // the window is the topmost layer, and `Visuals::dark()`'s value is `#2D2D2D`.
    //
    // Aubergine, which is CORE §6's existing meaning used exactly — *this is the active
    // item*. That egui only lights the band on the top layer is worth keeping deliberately:
    // when the password modal opens over New Archive, New Archive's band drops back to
    // plain POPUP and the modal is unmistakably the thing holding the keyboard.
    v.widgets.open.bg_fill = AUBERGINE;
    v.widgets.open.weak_bg_fill = AUBERGINE;
    v.widgets.open.bg_stroke = edge();
    v.widgets.open.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.open.expansion = 0.0;

    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
    ] {
        w.corner_radius = R_CTRL.into();
    }
    v.widgets.open.corner_radius = R_POPUP.into();

    // `disable()` multiplies the painter's opacity, so at egui's 0.5 an inactive fill over
    // a popup blended away to nothing — the invisible-box bug this file was written to kill,
    // recurring in the one state nobody looks at. At 0.85 the outlined ghost `button()`
    // paints measures 1.73:1 on its rim and 4.07:1 on its label.
    v.disabled_alpha = 0.85;

    // egui's default caret is a 2px `#C0DEFF` pale blue — off-palette, and the one stroke in
    // the program that broke CORE §6's line rule before P7 rewrote it. A 1px caret renders
    // as a grey smear on a 1× display, so the weight stays and only the colour changes.
    v.text_cursor.stroke = Stroke::new(2.0, TEXT);

    // And the caret was not the only one. `ime_composition` carries two more strokes in the
    // same `#C0DEFF`, drawn under preedit text in every field the program has — the filter
    // bar, the rename cell, both path fields, the archive name, both password boxes. They
    // are exactly 2.0 wide, so the width rule never caught them; only the colour was wrong,
    // and a test that measures width alone will go on not catching them.
    v.ime_composition.active_underline_stroke = Stroke::new(2.0, TEXT);
    v.ime_composition.inactive_underline_stroke = Stroke::new(1.0, TEXT_MUTED);

    // Selection is one of orange's three permitted meanings.
    v.selection.bg_fill = ORANGE.linear_multiply(0.35);
    v.selection.stroke = Stroke::new(1.0, ORANGE);

    // Both styles, not just the current one. `set_visuals` writes only the theme egui thinks
    // it is in, and `Options::theme()` is refreshed from the platform on every pass — so a
    // compositor that reported "light" would have thrown away the entire palette and left
    // stock `Visuals::light()` wearing INDIUM's fonts. It is latent today only because winit
    // returns no system theme on Linux; CORE §6 says there is no second theme, and this is
    // what makes that structurally true rather than true by accident. `install_spacing`
    // below has always written both, which is where the discrepancy showed.
    ctx.all_styles_mut(|style| style.visuals = v.clone());
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

        // The four this function's own doc comment promised to write down and never did.
        // A popup's padding, a menu's padding, the `CollapsingHeader` offset in New Archive,
        // and the minimum height of every button — which now matches a status-bar row and a
        // table row, so the three cannot disagree about how tall one line is.
        style.spacing.window_margin = egui::Margin::same(14);
        style.spacing.menu_margin = egui::Margin::same(8);
        style.spacing.indent = 18.0;
        style.spacing.interact_size.y = SB_ROW;
    });
}

// --- The shapes CORE §4 and §6 are made of ------------------------------------

/// One of CORE §4's five zones: a fill, a 2px edge all round, square corners, and half a
/// gutter of `VOID` outside it.
///
/// **Budget for the edge.** `Frame::total_margin()` is `inner_margin + stroke.width +
/// outer_margin`, so this frame consumes `inner + 2 + 4` on every side. Any panel given an
/// `exact_size` must add all three, or it overflows and paints across the gutter it was
/// supposed to float in — and it does so invisibly, because `Panel` clamps the rect it
/// reports to `exact_size` regardless of what it drew.
///
/// Every panel that uses this must also call `.show_separator_line(false)`: egui draws its
/// own hairline between panels, and it would stack with this border.
pub fn zone(fill: Color32) -> egui::Frame {
    egui::Frame::NONE
        .fill(fill)
        .stroke(edge())
        .corner_radius(R_ZONE)
        .inner_margin(egui::Margin::same(PAD))
        .outer_margin(egui::Margin::same(GUTTER / 2))
}

/// A button, with the focus problem solved.
///
/// egui maps `has_focus() || clicked()` to the *active* state and offers no `widgets.focused`
/// to separate them, so a button used to stay lit long after the click that lit it. INDIUM
/// has no Tab navigation — CORE §4's keyboard table is all bare keys — so a button has no
/// business keeping focus, and surrendering it costs nothing and fixes it exactly.
///
/// The disabled arm is an outlined ghost rather than a filled box: a fill at
/// `disabled_alpha` blends toward whatever is behind it, which is how the invisible-box bug
/// got in the first time. The absent fill is what says "off"; `disabled_alpha` dims the rest.
///
/// **The ghost's stroke is 2px, and it has to be.** `Style::button_style` computes the
/// button's inner margin as `button_padding - bg_stroke.width`, having already budgeted for
/// the state's 2px edge; `Button::stroke` is then applied *over* that, without recomputing
/// it. A 1px override — which is what this arm carried when it was written — leaves a pixel
/// unaccounted on each side, so the button was **2px narrower disabled than enabled** and
/// jumped sideways the moment its field filled in, shoving whatever stood beside it. The
/// weight stays matched; only the fill and the ink say the button is off.
pub fn button(ui: &mut egui::Ui, text: egui::RichText, enabled: bool) -> egui::Response {
    if enabled {
        let r = ui.add(egui::Button::new(text));
        if r.clicked() {
            r.surrender_focus();
        }
        r
    } else {
        ui.add_enabled(
            false,
            egui::Button::new(text.color(TEXT_MUTED))
                .fill(Color32::TRANSPARENT)
                .stroke(edge()),
        )
    }
}

/// The same, for a `×` or a `+` that must not be a full-height button.
///
/// **It gives up the one-pixel geometry and keeps the other two channels, deliberately.**
/// egui makes `expansion` layout-neutral by pairing a negative *outer* margin with a
/// positive *inner* one, so the total is always `button_padding`. `Button::small()` then
/// zeroes the vertical padding and **leaves the negative outer margin in place** — which
/// left these buttons 2px shorter on hover and 4px taller while held, moving every label
/// beside them and reflowing the wrapped row of bookmark chips as the pointer crossed it.
///
/// Zeroing the expansion for this scope only is what stops it. A `×` still brightens from
/// `CONTROL` to Aubergine and its rim still goes hot; it simply does not breathe. The full
/// three-channel press stays on every button big enough to show it without disturbing its
/// neighbours.
pub fn small_button(ui: &mut egui::Ui, text: egui::RichText, enabled: bool) -> egui::Response {
    let mut scoped = ui.new_child(egui::UiBuilder::new().max_rect(ui.available_rect_before_wrap()));
    let v = scoped.visuals_mut();
    v.widgets.hovered.expansion = 0.0;
    v.widgets.active.expansion = 0.0;

    let r = if enabled {
        let r = scoped.add(egui::Button::new(text).small());
        if r.clicked() {
            r.surrender_focus();
        }
        r
    } else {
        scoped.add_enabled(
            false,
            egui::Button::new(text.color(TEXT_MUTED))
                .small()
                .fill(Color32::TRANSPARENT)
                .stroke(edge()),
        )
    };
    ui.advance_cursor_after_rect(r.rect);
    r
}

/// A clickable row that reacts to the pointer.
///
/// Seven of these were hand-rolled across the program and not one of them read its own
/// `Response`, so their fill was a function of application state only and hovering painted
/// nothing. Three had no cursor change either.
///
/// Built on `UiBuilder::sense`, whose sense registers *below* any widget inside the row — so
/// a `×` button within a row still takes its own click, and the row does not also fire.
pub fn row(
    ui: &mut egui::Ui,
    active: bool,
    pad: egui::Margin,
    add: impl FnOnce(&mut egui::Ui),
) -> egui::Response {
    ui.scope_builder(egui::UiBuilder::new().sense(egui::Sense::click()), |ui| {
        let r = ui.response();
        let fill = if active {
            AUBERGINE
        } else if r.is_pointer_button_down_on() {
            AUBERGINE_LIT
        } else if r.hovered() {
            CONTROL
        } else {
            Color32::TRANSPARENT
        };
        egui::Frame::NONE
            .fill(fill)
            .corner_radius(R_CTRL)
            .inner_margin(pad)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                add(ui);
            });
    })
    .response
    .on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A section heading, and the rule that separates it from what it opens.
///
/// Headings used to render at 12.0 — *smaller* than the 13.0 body beneath them — and were
/// told from content by weight alone. Three helpers drew them, two of which were byte-
/// identical copies of each other.
///
/// **A heading takes a rule when it opens a list of siblings, and none when it names a
/// single object.** The rule *is* the top edge of the group; where there is no group, there
/// is no edge to draw.
pub fn section(ui: &mut egui::Ui, title: &str) {
    section_bare(ui, title);
    ui.add(egui::Separator::default().horizontal().spacing(6.0));
}

/// The same heading where the container already draws the boundary — a popup's own title
/// bar, or a card whose name is a value rather than a label.
pub fn section_bare(ui: &mut egui::Ui, title: &str) {
    ui.add_space(SECTION_ABOVE);
    ui.label(
        egui::RichText::new(title)
            .size(14.0)
            .family(bold())
            .color(TEXT),
    );
    ui.add_space(3.0);
}

pub fn install(ctx: &egui::Context) {
    install_fonts(ctx);
    install_visuals(ctx);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WCAG relative luminance. Ten lines, no dependency — CORE §2 would not admit a crate
    /// for arithmetic this small, and `util.rs` writes its own CRC32 table for the same
    /// reason.
    fn luminance(c: Color32) -> f32 {
        let f = |v: u8| {
            let v = v as f32 / 255.0;
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * f(c.r()) + 0.7152 * f(c.g()) + 0.0722 * f(c.b())
    }

    fn contrast(a: Color32, b: Color32) -> f32 {
        let (x, y) = (luminance(a), luminance(b));
        let (hi, lo) = if x > y { (x, y) } else { (y, x) };
        (hi + 0.05) / (lo + 0.05)
    }

    const GROUNDS: [(&str, Color32); 6] = [
        ("VOID", VOID),
        ("STATUS_BAR", STATUS_BAR),
        ("WINDOW", WINDOW),
        ("PANEL", PANEL),
        ("POPUP", POPUP),
        ("CONTROL", CONTROL),
    ];

    fn visuals() -> egui::Visuals {
        let ctx = egui::Context::default();
        install_visuals(&ctx);
        ctx.style_of(egui::Theme::Dark).visuals.clone()
    }

    #[test]
    fn the_ground_ladder_only_ever_goes_up() {
        for pair in GROUNDS.windows(2) {
            let (lo_name, lo) = pair[0];
            let (hi_name, hi) = pair[1];
            assert!(
                luminance(hi) > luminance(lo),
                "{hi_name} must be lighter than {lo_name}"
            );
        }
        assert!(luminance(AUBERGINE) > luminance(CONTROL));
        assert!(luminance(AUBERGINE_LIT) > luminance(AUBERGINE));
    }

    /// The regression that would bring back P5's invisible button: a surface the same
    /// colour as the thing behind it.
    #[test]
    fn no_two_grounds_are_the_same_colour() {
        // `assert_ne!` alone would pass at 1.001:1 — it would have let the very bug this
        // test is named for straight through. Each rung must clear its neighbour by a real
        // margin in *linear* luminance, which is the axis the eye reads at this end of the
        // scale; the WCAG ratio for a pair this dark is dominated by its own flare term and
        // says 1.05 for a step you can plainly see.
        for pair in GROUNDS.windows(2) {
            let (lo_name, lo) = pair[0];
            let (hi_name, hi) = pair[1];
            let step = luminance(hi) / luminance(lo);
            assert!(
                step >= 1.30,
                "{hi_name} is only {step:.3}x {lo_name} — too close to read as a step"
            );
        }
        for (i, (an, a)) in GROUNDS.iter().enumerate() {
            for (bn, b) in GROUNDS.iter().skip(i + 1) {
                assert_ne!(a, b, "{an} and {bn} are the same colour");
            }
        }
    }

    #[test]
    fn text_is_legible_on_every_ground() {
        for (name, g) in GROUNDS {
            assert!(
                contrast(TEXT, g) >= 7.0,
                "TEXT on {name} is {:.2}:1",
                contrast(TEXT, g)
            );
            assert!(
                contrast(TEXT_MUTED, g) >= 4.5,
                "TEXT_MUTED on {name} is {:.2}:1",
                contrast(TEXT_MUTED, g)
            );
        }
    }

    /// "clicks doesnt feel anything" — the four states must actually differ.
    #[test]
    fn the_four_control_states_are_four_colours() {
        let v = visuals();
        let fills = [
            v.widgets.inactive.weak_bg_fill,
            v.widgets.hovered.weak_bg_fill,
            v.widgets.active.weak_bg_fill,
        ];
        // `widgets.open` is deliberately not in that list, though `install_visuals` sets it
        // alongside the other three. It is not a control state — it is the popup title band,
        // and it is Aubergine for the same reason a hovered control is: CORE §6 gives
        // Aubergine exactly one meaning, *the active item*. Requiring it to differ would be
        // requiring the palette to break its own rule. `the_popup_title_bar_is_not_egui_grey`
        // is what guards that field.
        for (i, a) in fills.iter().enumerate() {
            for b in fills.iter().skip(i + 1) {
                assert_ne!(a, b, "two control states share a fill");
            }
        }
        assert!(v.widgets.hovered.expansion > v.widgets.inactive.expansion);
        assert!(v.widgets.active.expansion < v.widgets.inactive.expansion);
    }

    /// Named after the bug: every popup wore `#2D2D2D` because this state was never set.
    #[test]
    fn the_popup_title_bar_is_not_egui_grey() {
        let v = visuals();
        assert_ne!(
            v.widgets.open.weak_bg_fill,
            egui::Visuals::dark().widgets.open.weak_bg_fill
        );
        assert_ne!(v.widgets.open.weak_bg_fill, v.window_fill);
    }

    /// CORE §6's rewritten Lines rule: two weights, and nothing thicker than the second.
    #[test]
    fn no_stroke_is_thicker_than_two_pixels() {
        let v = visuals();
        let w = &v.widgets;
        for s in [
            w.noninteractive.bg_stroke,
            w.noninteractive.fg_stroke,
            w.inactive.bg_stroke,
            w.inactive.fg_stroke,
            w.hovered.bg_stroke,
            w.hovered.fg_stroke,
            w.active.bg_stroke,
            w.active.fg_stroke,
            w.open.bg_stroke,
            w.open.fg_stroke,
            v.window_stroke,
            v.selection.stroke,
            v.text_cursor.stroke,
            v.ime_composition.active_underline_stroke,
            v.ime_composition.inactive_underline_stroke,
        ] {
            assert!(s.width <= 2.0, "a stroke is {} wide", s.width);
            // Width alone let three 2.0px `#C0DEFF` strokes through — the caret and both
            // IME underlines. A line rule that does not check the colour is half a rule.
            assert_ne!(
                s.color,
                egui::Color32::from_rgb(192, 222, 255),
                "an off-palette egui default stroke survived"
            );
        }
    }

    /// Five undeclared radii became three declared ones.
    #[test]
    fn only_three_corner_radii_exist() {
        let v = visuals();
        let ok = |r: egui::CornerRadius| {
            [R_ZONE, R_CTRL, R_POPUP].contains(&r.nw)
                && r.nw == r.ne
                && r.nw == r.sw
                && r.nw == r.se
        };
        for r in [
            v.widgets.noninteractive.corner_radius,
            v.widgets.inactive.corner_radius,
            v.widgets.hovered.corner_radius,
            v.widgets.active.corner_radius,
            v.widgets.open.corner_radius,
            v.window_corner_radius,
            v.menu_corner_radius,
        ] {
            assert!(ok(r), "{r:?} is not one of the three declared radii");
        }
    }

    /// P5 §3 and P6 §6.6's invariant, pinned at last: orange means the current selection,
    /// staged changes and Apply/progress, and reaches the widget states through none of them.
    #[test]
    fn orange_has_not_spread_into_the_widget_states() {
        let v = visuals();
        let w = &v.widgets;
        for c in [
            w.inactive.weak_bg_fill,
            w.hovered.weak_bg_fill,
            w.active.weak_bg_fill,
            w.open.weak_bg_fill,
            w.inactive.bg_stroke.color,
            w.hovered.bg_stroke.color,
            w.active.bg_stroke.color,
        ] {
            assert_ne!(c, ORANGE, "orange reached a widget state");
        }
        assert_eq!(v.selection.stroke.color, ORANGE);
    }
}
