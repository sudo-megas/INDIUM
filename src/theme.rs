//! Ubuntu Canonical Aubergine, and the embedded typeface.
//!
//! CORE §6: "One theme: Ubuntu Canonical Aubergine. There is no second theme and no
//! theme setting." Every value below is transcribed from that section; none of them is
//! configurable, and CORE §9 forbids adding a control that would make them so.
//!
//! The typeface is CaskaydiaMono Nerd Font Mono, in two weights, and it is the only one:
//! CORE §6 puts the whole window in monospace rather than splitting chrome from values,
//! so chrome and values are told apart by weight and colour instead of by family.

use eframe::egui;
use egui::{Color32, FontData, FontDefinitions, FontFamily, Stroke};
use std::sync::Arc;

// --- Base ---------------------------------------------------------------------
//
// Five grounds, each between 1.37 and 1.87 times the *linear* luminance of the one below,
// all on hue 318–319° — the same family as Canonical Aubergine's 328°, which is the reason
// CORE §6 gives for `WINDOW` in the first place. It said *six* until P18, here and in §6
// both: it was six when P7 built the ladder, and P9 moved the popup off it and out of
// aubergine altogether without moving the number. `GROUNDS` below still holds six, and
// correctly — it is the list of *resting* surfaces, the ones a zone sits on when nothing is
// happening to it, which includes the popup and is a different question from which grounds
// are on the ladder. It is deliberately not every surface text is drawn on: a row under the
// pointer or inside the selection is painted on a fill mixed at that moment from a colour
// that is not a ground at all, and those are named where they are measured, not here.
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
/// **The wells** — the entry table and every text field. Anything you look *into* rather
/// than *at*.
///
/// The maker's own terminal ground, adopted at P5. It sits at hue 319° against the old
/// value's 288°, which puts it in the same family as Canonical Aubergine (328°) — so the
/// window became *more* faithful to CORE §6's structural colour, not less.
pub const WINDOW: Color32 = Color32::from_rgb(0x30, 0x0A, 0x24);
/// The raised zones: sidebar, Inspector, tray.
pub const PANEL: Color32 = Color32::from_rgb(0x3D, 0x0D, 0x2E);
/// Popups. A popup covers every zone, so it is lighter than every zone — and it is now the
/// one surface in INDIUM that is **not** aubergine.
///
/// P7 §3 made a popup lighter than the zones and stopped there, and it was not enough: at
/// one step of luminance inside a single hue family, a popup over the window read as more
/// window. The maker's verdict after looking at it was that the recolouring "still failed
/// to distinguish" the popup at all. So the separation moved from lightness to **hue**.
/// This is steel blue — the maker's own choice — at exactly the luminance the aubergine
/// popup had, which is what lets the ground ladder and every contrast test below stand
/// unchanged while the surface becomes unmistakable.
///
/// The luminance is not a matter of taste. The colour as first picked, `#4682B4`, measures
/// **1.44:1 against `TEXT_MUTED`** and 3.54:1 against `TEXT`; on a ground that light the
/// brightest text that exists — pure white — still reaches only **4.11:1**, so no choice of
/// text colour could have rescued it. Scaling linear RGB by a constant preserves
/// chromaticity exactly, so this is that blue, and only its lightness was spent.
pub const POPUP: Color32 = Color32::from_rgb(0x13, 0x2A, 0x3D);
/// The band across the top of a popup, carrying its title.
///
/// Sapphire, from the same scaling as `POPUP`, and darker than the popup's own ground for
/// the reason the maker drew it that way: the band is a lid, not a highlight. It replaces
/// Aubergine, which P7 put here to mean *this is the active item* — a meaning that stopped
/// being available the moment the popup stopped being aubergine, because an aubergine band
/// on a blue popup would have been the window's colour sitting on top of the thing covering
/// the window.
pub const POPUP_HEAD: Color32 = Color32::from_rgb(0x02, 0x17, 0x3F);
/// The band across the foot of a popup, carrying what it is about to do.
///
/// Cobalt, from the same scaling, and the darkest of the three: the foot is where the
/// sentence naming the consequence lives, and it is read against the deepest ground in the
/// popup so that the Orange of an Apply or a Create has the most to push against.
pub const POPUP_FOOT: Color32 = Color32::from_rgb(0x00, 0x13, 0x3A);
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
/// CORE §6's fifth colour, and the narrowest: a wrong password, two passwords that
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

/// 1px at 20% white — every rule *inside* a zone: beneath a heading, above a footer,
/// between the archive and the two lists.
///
/// **It was 8%, and 8% is invisible.** Composited it measured 1.18–1.27:1 over the grounds,
/// which is to say it measured nothing, and two separate testing notes said so in the same
/// round — *"the separator staying on the New button is so faded. cant distinguish it"* and
/// the same complaint again about the rule under the filter bar. A rule is 1px so it does
/// not compete with an edge, not so that it cannot be seen. At 20% it clears **1.68:1 on
/// the worst ground it is drawn on**, and CORE §6 makes that floor a rule; `a_rule_can_be_seen`
/// is the test that holds it there. It stays under [`EDGE`]'s 22% so the two weights are
/// still two weights and not one weight at two thicknesses.
pub const HAIRLINE: Color32 = Color32::from_rgba_premultiplied(0x33, 0x33, 0x33, 0x33);
/// 2px at 22% white — every boundary *around* a zone, a popup or a control.
///
/// Composited over the six grounds it measures **1.88–1.95:1**. These are translucent, so
/// the figure is the composite against each ground and not the raw byte value, which would
/// mean nothing at all.
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

/// Zones are rounded, and the number is the one the tightest zone can afford.
///
/// **The hazard is real and has not gone away.** `egui::Frame` does **not** clip its
/// children: it builds one `RectShape` and paints it *behind* the content. Nothing stops a
/// header row or a selected first row's full-bleed orange fill from overhanging an arc — the
/// frame will happily paint the fill across a corner it has already cut away. Zones were
/// square for sixteen milestones because that made the question moot.
///
/// What P23 §2a changed is not the hazard but the arithmetic. A rounded rect of radius `R`
/// admits a point `(c, c)` in from its corner when `c ≥ R(1 − 1/√2)` — **1.76px at R = 6** —
/// and every zone insets its content by `inner_margin + 2` for the stroke: sidebar 14 and
/// 16, inspector 14, tray 12 and 8, status bar 14 and 12, and the table, which is the
/// tightest at **4 + 2 = 6**. Six clears 1.76 by a factor of three, and the table's content
/// corner lands exactly on the arc's own centre — the furthest inside the shape a corner can
/// be. So no corner covers, no inset first row, and no rounded clipping, which epaint 0.36
/// does not offer in any case.
///
/// **The ceiling is 8.46, and it is not the fill that sets it.** The obvious sum — the table's
/// content inset against `R(1 − 1/√2)` — gives R ≈ 20.5, and that number is wrong, because the
/// table draws one thing *outside* its content lane. The cursor ring is `gapless_rect`, the row
/// expanded by `0.5 * item_spacing` — which is **4 horizontally and 2.5 vertically, the latter
/// rounded to 3 on the pixel grid** by `round_ui`. Four is exactly the table's inner margin, so
/// the ring reaches `frame.left + 2`, the fill's own boundary; three is more than half of it, so
/// the ring also hangs **3px below** the lane it was drawn in and stops at `frame.bottom − 3`.
///
/// Its worst corner is bottom-left, at `(left + 2, bottom − 3)` against an arc centred
/// `(left + R, bottom − R)`, which stays inside while `(R−2)² + (R−3)² ≤ R²` — that is
/// **R ≤ 8.46**. At 6 the ring's corner is 5.0 from a centre 6 away: one pixel of radial slack,
/// not two. A later hand raising this constant re-runs *that* comparison, not the content-inset
/// one, and re-checks the class below. It does not have to remember to:
/// `the_cursor_ring_s_corner_stays_inside_the_zone_s_arc` (`ui/table.rs`) runs that comparison
/// on every build, from the margins rather than from literals.
///
/// **This number was written down three times before it was measured once.** The groundwork said
/// R ≤ 8 by modelling the *fill's* corner at an inset of 4 — it omitted the 2px stroke, and it
/// judged against the stroke's inner arc rather than the shape's edge — two errors pointing
/// opposite ways, landing near a true answer on a shape that does not bind at all. A first draft
/// of this comment said 20.5, from that same fill with the inset corrected. §2a then committed
/// 12.9, which found the right shape and then credited the ring with a bottom inset of 6 that
/// the ring does not take. The 8.46 above is the fourth number and the only one read off a
/// screenshot: at scale 1, with fifty entries and the last row cursored, the table's rect is
/// `left 271, bottom 685`, its arc centre is `(277, 679)`, and the ring's bottom-left outer
/// corner is `(273, 682)` — 5.00 away.
///
/// **What the inset argument does not cover is anything painted against a zone's own rect.**
/// Content is inset; a painter is not. The status bar's progress track is the one member of
/// that class — it backs out of the inner margin deliberately, to sit on the line the stroke
/// draws — and it is pulled in by this constant at both ends where it is drawn.
pub const R_ZONE: u8 = 6;
/// Hand-rolled rows, text fields, checkboxes, and any button not built through [`button`] —
/// the two raw `ui.button()` calls in the tray still land here. Buttons and chips made the
/// theme's way take [`R_PILL`] instead. The status bar's progress track used this until P13
/// moved the proportion to a square-ended line on the panel's edge, which PXX thickened.
pub const R_CTRL: u8 = 3;
/// Popups, menus, the modal.
pub const R_POPUP: u8 = 10;
/// A control's ends are semicircles. CORE §6: *"Controls are capsules."*
///
/// The same number as [`R_POPUP`], and that is the point rather than an accident, because it
/// is what keeps the vocabulary at three values and no fourth. A control's height floor is
/// `interact_size.y` = [`CONTROL_H`] = 20, so half of it is 10; and epaint clamps a corner
/// radius to half the rect it is drawn in (`clamp_corner_radius`), so the same 10 is still a
/// true pill on a `small()` button, which is shorter. One number, a pill at every height.
///
/// It is applied **at the control**, never in [`install_visuals`]. `widgets.inactive`,
/// `.hovered` and `.active` are also what a `TextEdit` and a checkbox draw through;
/// moving the constant there would turn the path field into a
/// lozenge, and §6 gives the capsule to things you press.
pub const R_PILL: u8 = R_POPUP;

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
/// The height floor of anything you press, and the number [`R_PILL`] is half of.
///
/// **This used to be [`SB_ROW`], and P13 split them.** They were the same 20 for eleven
/// milestones and the coupling read as economy rather than as a claim — but the moment the
/// status bar grew to carry a double-size glyph, "a button is a row tall" would have made
/// every button in the program 32 tall and turned R_PILL's 10 into a merely rounded corner,
/// which is exactly the shape CORE §6 refuses. A control's height is its own number now.
pub const CONTROL_H: f32 = 20.0;
/// The reading spinner's diameter.
///
/// **A length, not a type size, and it is here so that it cannot be mistaken for one.** It
/// takes `.size()` on an `egui::Spinner` — the same method name the type scale uses on
/// `RichText`, which is the whole reason it is written down: left inline it was a bare `14.0`
/// in the middle of a status bar full of type literals, indistinguishable from a seventh size
/// by any reader, human or test.
///
/// Fourteen because a `Spinner` left to itself takes `interact_size.y`, which is
/// [`CONTROL_H`] — 20, and taller than the [`BODY`] text beside it, which made the spinner the
/// loudest thing in a bar of words.
pub const SPINNER_D: f32 = 14.0;
/// One status-bar row.
///
/// Tall enough for an icon at [`ICON_SCALE`], which is 1.4 and not the 2 this comment
/// claimed until P16 — CORE §6 said "twice" until the same round corrected it, and quoting
/// a sentence that had already been rolled back is how the number survived here. The
/// arithmetic that matters is not 13 × 1.4 = 18.2, which would fit the 20 this was until
/// P13: it is the *line box* egui gives an 18.2pt glyph, which does not. The bar grew for
/// that, which is the trade the maker asked for in as many words: "icons only sensible when
/// they are big enough."
///
/// Held by [`the_status_bar_row_fits_the_icon_it_was_grown_for`], which P23 added when it
/// swapped the face: this number was fitted to Fira Mono's line box and pinned by nothing,
/// so it was one `include_bytes!` away from being wrong in silence.
pub const SB_ROW: f32 = 24.0;
/// Between status-bar rows.
pub const SB_GAP: f32 = 4.0;

/// The type scale — six roles, and the fact that there was never a seventh worth having.
///
/// **These are roles, not sizes.** Before P23 the window drew text at eight different numbers
/// across a hundred and eleven call sites — plus two more behind a file-local constant in
/// `measure.rs` — while `install_spacing` below set five text styles between **three** distinct
/// sizes. So the scale was not a decision the
/// program stated anywhere; it was the sum of a hundred and thirteen separate ones. Two of those numbers were the *same role drawn at two sizes*, which is the
/// defect this section exists to close and not a matter of taste:
///
/// - **The subject's name** — a popup's title, the Inspector's file, "17 selected" — was 17
///   in four places in the Inspector and in every `TextStyle::Heading`, and **16** in Open
///   With's app name and in the table's empty state. One role, two numbers, no reason.
/// - **A subordinate sentence** — the muted line under a heading that says what the thing is
///   for — was 13 in the Bookmarks and Draft panes and **11** twice in Create. Eleven is not
///   a level; it is a popup that was crowded and got quieter to fit.
///
/// **[`BODY`] is 13 because CORE says 13, not because it shipped that way.** §6 argues the
/// icon scale from "about nine points of ink next to **a thirteen-point capital**"
/// (`CORE.md:394`), and §6's file-type-icon ruling from "at `ICON_SCALE` **a 13pt glyph**
/// does not fit a 20px row" (`CORE.md:425`). Moving the body size falsifies two sentences of
/// a document that is the maker's to edit, so it is the one value here that is not this
/// round's to choose. Everything else in the scale is measured against it.
///
/// The gaps are the rethink. Thirteen, twelve and eleven were three sizes inside two points,
/// which reads as unevenness rather than as hierarchy; eleven is gone. A section heading was
/// 14 — one point over body — so it was told from the text beneath it by weight alone, and
/// CORE §5's own account of what was wrong with the Inspector is "ten fields in one size, one
/// weight and three greys, with nothing to say which mattered" (`CORE.md:204`). Fifteen gives
/// the heading two points and a weight instead of a weight and a rounding error.
///
/// Held by `every_drawn_size_comes_from_the_type_scale` — the same shape as the icon
/// registry's guard, and for the same reason: the scale is only one decision for as long as
/// nothing can quietly add a seventh number to it.
pub const WORDMARK: f32 = 28.0;
/// The wordmark where it stands beside the 50px mark rather than the 150px one.
///
/// Two sizes because there are two marks, not because the brand has two voices: the sidebar's
/// head is a fifth of About's and a 28pt word beside a 50px mark reads as a caption that got
/// away.
pub const WORDMARK_SM: f32 = 23.0;
/// The name of the thing on screen — a popup's title, the Inspector's subject, the empty
/// state's one line.
///
/// This is what `TextStyle::Heading` resolves to as well, so a popup's own title bar and a
/// pane's subject are the same size by construction rather than by two hands agreeing.
pub const TITLE: f32 = 17.0;
/// A heading *inside* a pane — what [`section`] and [`section_bare`] draw.
///
/// Not a title: a title names the thing, a section names a group of fields within it.
pub const SECTION: f32 = 15.0;
/// A value, a label, a button, a table cell, a table header.
///
/// **Pinned by CORE §6 at 13** — see the scale's note above. Every other size here is chosen
/// against this one, and the icon step is literally derived from it.
pub const BODY: f32 = 13.0;
/// Subordinate to the value beside it — a badge, a unit, a hint, the status bar's second row.
///
/// One point under [`BODY`] and never two: the third size that used to live below this one
/// was 11, and it was crowding rather than hierarchy.
pub const SMALL: f32 = 12.0;

/// How much bigger an icon is than the text it stands beside.
///
/// CORE §6 gives icons the same face as everything else, which means they also inherit its
/// sizes — and at 1× they were simply too small to be worth drawing: a Font Awesome glyph
/// carries padding inside its em box, so a 13pt icon puts about nine points of ink beside a
/// thirteen-point capital, which is why 1× read as no icon at all.
///
/// **It was 2× for an afternoon and that was too much.** At double the text the glyph sets
/// every row's height, which added 36 points to the status bar and about 120 to the sidebar
/// — and a window that had fitted everything for twelve milestones stopped fitting anything.
/// Every problem that followed was a consequence of this number. 1.4 is large enough to read
/// as a glyph and small enough that the rows are the height they always were.
pub const ICON_SCALE: f32 = 1.4;
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
/// repository actually holds. It is drawn at 50px in the sidebar and 150px in About, so even
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
/// CORE §6 gives Aubergine "selection context, the active item, and whatever the pointer is
/// resting on", and reserves orange for the current selection, staged changes, and
/// Apply/progress — orange means *something will happen*. A tab, a preset chip and a toggle say
/// only *which mode is active*; nothing is about to happen because a tab is open. So they are
/// Aubergine's work, and always were: the sidebar, the Create method rows and the focused
/// Recents and Bookmarks rows have hand-painted Aubergine for this exact meaning since P1.
/// P6 §6.6.
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
    // CORE §6 gives the capsule to "a button, a chip, *Cancel* — anything you press", and a
    // preset chip and an Inspector tab are pressed. A `selectable_label` takes its shape from
    // the widget state rather than from a builder, so unlike [`button`] the radius has to be
    // set here; this scope is the same clone-on-write child the fills above live in, so it
    // reaches the chips and nothing else.
    for w in [
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
    ] {
        w.corner_radius = R_PILL.into();
    }
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
    FontFamily::Name("caskaydia-bold".into())
}

/// The two faces, named once so [`install_fonts`] and the tests read the same bytes.
///
/// Same shape as [`MARK`], and for the same reason: a second `include_bytes!` on the same
/// path is a second copy in the binary, and a test that parses a *different* copy from the
/// one the window draws with is a test that proves nothing.
pub const FACE_REGULAR: &[u8] =
    include_bytes!("../assets/fonts/CaskaydiaMonoNerdFontMono-Regular.ttf");
/// The bold cut; see [`FACE_REGULAR`].
pub const FACE_BOLD: &[u8] = include_bytes!("../assets/fonts/CaskaydiaMonoNerdFontMono-Bold.ttf");

/// CORE §6: "Icons are glyphs of that same face … from the **Font Awesome** range the Nerd
/// Font patches in, and no second range is mixed with it."
///
/// Named by **role, never by codepoint**, so no call site anywhere holds a hex escape and
/// changing what *the archive* looks like is one edit rather than a search. Every one of
/// these is checked against the embedded `cmap` by `every_icon_exists_in_both_faces` —
/// which is the test that makes a missing glyph a build failure instead of a box on
/// somebody's screen — and against the block above by
/// `every_icon_comes_from_the_font_awesome_range_and_no_second_one`, which is a separate
/// test because the two questions have different answers: this face carries several Nerd
/// Font ranges, so *existing* and *belonging here* are not the same thing, and until P18
/// only the first was asked.
///
/// The rule these obey is §6's: **an icon replaces a word, it never garnishes one.** The
/// single deliberate exception is [`icon::WARNING`], which doubles the colour on purpose.
pub mod icon {
    /// Declare an icon and register it in [`ALL`] with the same words.
    ///
    /// `ALL` used to be a second hand-kept list beside the constants, so a glyph added to the
    /// module and forgotten here compiled, drew, and was checked by nothing — the two tests
    /// below only ever see what `ALL` names. It failed *open*, which is the worst way: the
    /// evidence was tofu on somebody's screen. Now the only syntax that defines an icon is the
    /// syntax that registers it. P18.
    ///
    /// `$(#[$meta:meta])*` carries each constant's doc comment through unchanged — a `///` is
    /// `#[doc = "…"]` — so the prose stays with the glyph it describes.
    macro_rules! icons {
        ($($(#[$meta:meta])* $name:ident = $glyph:expr;)*) => {
            $($(#[$meta])* pub const $name: &str = $glyph;)*

            /// Every icon this program draws, generated from the declarations above.
            ///
            /// Consumed only by the tests; drawing code reaches the constants by name.
            pub const ALL: &[(&str, &str)] = &[$((stringify!($name), $name)),*];
        };
    }

    icons! {
        /// The drawer. *The archive* — the same shape in the sidebar and the status bar, so
        /// one glyph has one meaning wherever it appears.
        ARCHIVE = "\u{f187}";
        /// A directory on disk, as opposed to one inside an archive.
        FOLDER = "\u{f07b}";
        /// *Open file* — the folder you are about to look in, so it is the open one.
        FOLDER_OPEN = "\u{f07c}";
        /// The ribbon, for *Bookmarks*.
        BOOKMARK = "\u{f02e}";
        /// A list, for *Draft* — what the next archive will be made of. Not the drawer:
        /// [`ARCHIVE`] means a file that exists, and the whole of P22 is that a draft is
        /// not one yet.
        DRAFT = "\u{f03a}";
        /// A clock, for *Recent files*.
        RECENT = "\u{f017}";
        /// A plus, for *New*.
        NEW = "\u{f067}";
        /// A gear, for *Settings*.
        SETTINGS = "\u{f013}";
        /// An `i` in a circle, for *About*.
        ABOUT = "\u{f05a}";
        /// The triangle, and **only** where something has gone wrong — the same restriction
        /// CORE §6 puts on [`super::WARNING`], the colour it is always drawn in.
        WARNING = "\u{f071}";
    }
}

/// Embed the typeface and make it the only one.
///
/// The files are bundled assets, not dependencies (CORE §2), under the SIL Open Font
/// Licence 1.1 in `LICENSES/`. Cascadia **Mono** carries no ligatures — that is Cascadia
/// *Code*'s job, and this is not Cascadia Code — so a filename holding `->` or `!=` renders
/// as the bytes the archive stores and cannot do otherwise. `…NerdFontMono…` is the
/// single-cell icon cut, so an icon occupies one column and the entry table stays aligned.
///
/// **That first sentence is load-bearing, and P23 nearly shipped the face that breaks it.**
/// The reasoning it replaced claimed the guarantee came from the toolkit — that egui applies
/// no OpenType shaping, so `liga` and `calt` could never fire whatever the face offered. That
/// is false. `epaint` 0.36 shapes through **`harfrust`**, a HarfBuzz port: `font.rs:361`
/// holds a `ShaperData` of the parsed GSUB/GPOS, and `text_layout.rs` calls
/// `shaper.shape(buffer, ShapeOptions::new())` — *default* options, which is HarfBuzz's
/// default horizontal feature set, `calt` included. Cascadia implements its ligatures in
/// `calt`, so they fire. Measured on the Cove cut: all twenty probes in
/// [`a_filename_is_the_characters_it_holds`] substitute, and `www` collapses to one
/// 23-pixel glyph followed by two zero-width continuations. In a program whose subject is
/// the names inside an archive, `a->b` drawn as `a⟶b` is not a cosmetic difference.
///
/// So the face is chosen for the property, and a test holds it rather than a comment.
///
/// These are TrueType, with `glyf` outlines, where the face before them was OTF/CFF. egui
/// rasterizes through Fontations — `skrifa`, `read-fonts`, `harfrust` — which reads both
/// natively, so the format is not a special case. `assets/fonts/README.md` records the
/// provenance and the measured coverage of both faces.
pub fn install_fonts(ctx: &egui::Context) {
    // `empty()`, not `default()`. `eframe` is built without `default_fonts`, so
    // `default()` already returns nothing — but it returns nothing *by side effect of a
    // feature flag*, and saying `empty()` means the same thing on purpose.
    let mut fonts = FontDefinitions::empty();

    fonts.font_data.insert(
        "caskaydia".to_owned(),
        Arc::new(FontData::from_static(FACE_REGULAR)),
    );
    fonts.font_data.insert(
        "caskaydia-bold".to_owned(),
        Arc::new(FontData::from_static(FACE_BOLD)),
    );

    // One face in both default families, because CORE §6 puts the whole window in
    // monospace. There is no fallback behind it and no pretending otherwise: this face
    // carries 12,938 codepoints — Latin including the whole Turkish set, Greek, Cyrillic,
    // the arrows and box-drawing, and some ten thousand Nerd icons — but no CJK and no
    // emoji, so a filename using those renders as tofu. That is the honest cost of
    // embedding one face and linking no fontconfig, and it is stated here so it is a known
    // limit, not a bug report waiting to happen. A glyph the face cannot draw is still a
    // name INDIUM read correctly and will write back correctly; since P11 the reading does
    // not depend on the drawing.
    //
    // **Both numbers are re-measured at each face swap and neither was carried across this
    // one.** The count moved 12,132 → 12,938, and the limit got shorter rather than longer:
    // Fira Mono had no Vietnamese and this face does — `U+1EA1` and the rest of the
    // precomposed set are present in both weights — so a sentence that still said "no
    // Vietnamese" would now be understating the program.
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .insert(0, "caskaydia".to_owned());
    }

    fonts.families.insert(
        FontFamily::Name("caskaydia-bold".into()),
        vec!["caskaydia-bold".to_owned(), "caskaydia".to_owned()],
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
    // P7 put Aubergine here, meaning *this is the active item*. That meaning is no longer
    // available: the popup is blue, and an aubergine band on it would be the window's own
    // colour sitting on top of the thing covering the window — the exact confusion the
    // recolouring exists to end. The band is now the popup's own darkest-but-one ground.
    //
    // That egui only lights the band on the top layer is still worth keeping deliberately:
    // when the password modal opens over Create, Create's band drops back to
    // plain POPUP and the modal is unmistakably the thing holding the keyboard.
    v.widgets.open.bg_fill = POPUP_HEAD;
    v.widgets.open.weak_bg_fill = POPUP_HEAD;
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

    // P21. **Text was never un-antialiased**, and this line is not what turns it on: egui
    // rasterizes each glyph's outline through `vello_cpu` into a coverage pixmap, hinting on
    // (`Target::Smooth`), and pushes that coverage through `TwoCoverageMinusCoverageSq` —
    // which `Visuals::dark()` above already selected, being the dark-mode default. The knob
    // that *sounds* like the one is `TessellationOptions::feathering`, and epaint says of it
    // flatly: "This setting does not affect text." It smooths shape edges. Raising it would
    // blur the rules and leave every glyph exactly as it was.
    //
    // PXX. **Sub-pixel binning is left at egui's default, which is on**, and the line that
    // turned it off is gone. P21 set `text_options.subpixel_binning = false` here, arguing
    // that §6 puts the window in monospace on a fixed advance, so the even kerning binning
    // buys was already there and only its blur — epaint's own "It also lead to text looking
    // more blurry" — was being paid for. The argument is false, and the font file says so:
    // `CaskaydiaMonoNerdFontMono-Regular.ttf` reports `head.unitsPerEm` 2048 and an advance
    // of 1200, so the advance is fixed at 0.586 *em* and not at a whole pixel. At the six
    // sizes this window sets — the type scale, 12 / 13 / 15 / 17 / 23 / 28 — it is 7.031 /
    // 7.617 / 8.789 / 9.961 / 13.477 / 16.406 px, **not one of them integral**, so with
    // binning off each glyph origin rounds to a pixel and the gaps at 13px run 8, 7, 8, 7, 8,
    // 8. The window was paying uneven spacing for a benefit it had been told it could not
    // collect.
    //
    // **Every figure in that paragraph has now been re-measured twice, and the sizes were
    // wrong both times before it.** At 2b the comment said "12 / 13 / 18" while
    // `install_spacing` below had set Heading to 17 for as long as it had existed; at 2c it
    // said *three* sizes while the scale above names six. The conclusion survived the face
    // change untouched — Fira was 600/1000 and this face is 1200/2048, and neither lands on
    // a pixel — and it survives the scale, because every one of the six misses a pixel too.
    // That is the point: the argument is about the *shape* of the number, so a reader must
    // not have to wonder whether it was ever re-checked. Twice now it needed to be.
    //
    // P21 owed the line an experiment and never ran it — `P21.md:551`, *"if the A/B refuses
    // it, the line comes out"*, with `P21.md:673` left unticked. PXX ran it: two builds
    // differing in this line alone, the same archive open at the same scroll, no keypress in
    // either, the window landing pixel-identically (a title-bar strip differing by 0 pixels).
    // With binning off the Details panel wrapped `23.4 MiB (24 576 000 bytes)` onto a second
    // line; with it on the same string fits. Rounding every origin up widens a string enough
    // to wrap it, so the line was costing *layout* and not only sharpness — silently, in the
    // narrowest panel in the window. The line comes out, as P21 said it would, and CORE §6
    // is not touched.

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
        //
        // **The numbers are not here.** They are the type scale above, and this map is a
        // reader of it like every call site in `ui/`. It was the other way round until P23:
        // these five literals were the nearest thing the program had to a stated scale, and
        // being the nearest thing is not the same as being it — a hundred and eleven call sites
        // drew sizes this map had never heard of, three of them larger than anything in it.
        style.text_styles = [
            (
                TextStyle::Heading,
                FontId::new(TITLE, FontFamily::Proportional),
            ),
            (TextStyle::Body, FontId::new(BODY, FontFamily::Proportional)),
            (
                TextStyle::Button,
                FontId::new(BODY, FontFamily::Proportional),
            ),
            (
                TextStyle::Small,
                FontId::new(SMALL, FontFamily::Proportional),
            ),
            (
                TextStyle::Monospace,
                FontId::new(BODY, FontFamily::Monospace),
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
        // A popup's padding, a menu's padding, the `CollapsingHeader` offset in Create,
        // and the minimum height of every button — `CONTROL_H`, which matches a table row.
        // The status bar's row is `SB_ROW` and no longer agrees with either: P13 split the
        // two so that a taller bar could carry a double-size glyph without making every
        // button in the program taller with it.
        // A popup's padding is `PAD` and not a number of its own. It read 14 while `foot`
        // pulled its band back by `PAD`, and the two points between them showed as popup
        // ground down both sides of every foot and along its bottom edge. One of the two had
        // to give, and it is this one: `foot` cannot pull back by anything but the inset it is
        // escaping, whereas the inset itself has no reason to be a second number.
        style.spacing.window_margin = egui::Margin::same(PAD);
        style.spacing.menu_margin = egui::Margin::same(8);
        style.spacing.indent = 18.0;
        style.spacing.interact_size.y = CONTROL_H;
    });
}

// --- The shapes CORE §4 and §6 are made of ------------------------------------

/// One of CORE §4's five zones: a fill, a 2px edge all round, [`R_ZONE`] corners, and half a
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

/// A popup that opens in the middle of the window and can then be dragged anywhere.
///
/// **The maker asked whether popups could float and be moved by mouse freely. They could not,
/// and not by accident** — every one of them called
/// `.anchor(Align2::CENTER_CENTER, Vec2::ZERO)`, and `Area::anchor` ends
/// `self.movable(false)` (egui-0.36.1 `area.rs:333-336`). The centring and the immovability
/// were the same call, which is why they were never separated: nobody had asked for one
/// without the other.
///
/// **Deleting that line does not leave a centred popup, it leaves a cascading one.** With no
/// anchor and no default position, `Area::begin` falls through to `automatic_area_position`
/// (`area.rs:461`, `:702-719`), which returns the constrain rect's top-left plus 16 and then
/// steps sideways per already-open window. So the anchor is replaced rather than removed, by
/// the two calls that say the same thing without the third meaning: a default position, and
/// the pivot to hang it from.
///
/// **The position is [`egui::Context::content_rect`]'s centre because that is the rect the
/// anchor was centring in** — `constrain_rect` defaults to `ctx.content_rect()`
/// (`area.rs:439`) and the anchor aligned within it, so a popup opens exactly where it opened
/// before. It is the viewport less any safe-area inset, which on this compositor is the
/// viewport; the two are not the same call, and the inset one is the one already used by
/// [`popup_max_height`] and [`list_height`].
///
/// **Where a popup opens the second time is egui's answer, and it is kept.** `pivot_pos` is
/// set through `get_or_insert_with` (`area.rs:459-462`), so `default_pos` is read once and a
/// popup that has been dragged reopens where it was left — closed and reopened in the test
/// below it comes back at the same pixel, not near it. That state lives in egui's memory, and
/// `eframe` is built here with `default-features = false` — **no `persistence`** — so it dies
/// with the process: moved within a run, centred again on the next launch. Nothing in this
/// file makes that happen; it is written down because it is the behaviour, and the next hand
/// to add `persistence` for some other reason will change it without meaning to.
///
/// **The same cache has a second effect, and it is the one that will look like a bug.** Read
/// once means read once whether or not anybody dragged anything, so a popup that has never
/// been touched also stops re-centring: resize the window and reopen it and it returns to the
/// old centre. Measured, a popup centred in a 1200×800 window and reopened at 800×600 sits
/// **223.8px off centre**, where the anchored version it replaced sat at 0.5. It is not lost —
/// `constrain_window_rect_to_area` still clamps it into view — it is simply not centred until
/// the process restarts, which is the same sentence as the paragraph above and the same cause.
/// **That is the accepted cost of the maker's question**, and it is
/// stated here in full because stage 3's round-13 walk resizes the window to 125% and 150% and
/// reopens popups at each: without this paragraph that walk files a placement defect, and it
/// would be filing one against a decision rather than a mistake.
///
/// The two modals are not here. Password and Measure are `egui::Modal`, fixed by
/// construction, and CORE §4.10 rests on Measure being the one popup drawn *over* another.
pub fn floating<'a>(ctx: &egui::Context, title: &'a str) -> egui::Window<'a> {
    egui::Window::new(title)
        .default_pos(ctx.content_rect().center())
        .pivot(egui::Align2::CENTER_CENTER)
}

/// The tallest a popup may be before it starts losing its own edges.
///
/// `egui::Window` sizes itself from its content and is then **clipped** by the viewport
/// rather than shrunk to it. A popup whose content outgrew the window therefore lost its
/// title band off the top and its foot off the bottom — which is to say it lost its name
/// and the button it exists for, and kept the middle. The window's minimum inner height is
/// [`crate::ui::MIN_H`], and it is only ever a request — this compositor hands INDIUM less —
/// so a popup taller than the window it covers is not a small-screen edge case.
pub fn popup_max_height(ctx: &egui::Context) -> f32 {
    (ctx.content_rect().height() - 2.0 * GUTTER as f32).max(240.0)
}

/// How tall a popup's scrolling middle may be, in the window it actually has.
///
/// `chrome` is everything in the popup that is not this list — its title band, the fields
/// above, the foot below. `want` is the height the list was drawn to have and is never
/// exceeded: a taller window gets the list the popup was designed with, not a longer one.
///
/// **Asked of the viewport, not of the `Ui`.** Measuring looks like the better answer and is
/// not: an `egui::Window` sizes itself from its content, so inside one `available_height` is
/// effectively unbounded and every list quietly kept the height it already had. Anchoring
/// the sum on the window's own cursor instead makes it circular — a popup that centres on
/// the viewport hangs from its own midpoint, so where it sits depends on the height being
/// computed here. That was written of an `.anchor()` call and it survived that call's
/// removal unchanged: [`floating`]'s `CENTER_CENTER` pivot derives the rect from the size
/// the same way. The viewport is the one term in the arithmetic that does not move.
///
/// The floor of 56 is two rows. A popup that cannot show two rows is one the user has to
/// resize the window for, and two rows with a foot beats six rows and no way to act on them.
pub fn list_height(ctx: &egui::Context, chrome: f32, want: f32) -> f32 {
    want.min(ctx.content_rect().height() - chrome).max(56.0)
}

/// The band across the foot of a popup, holding what the popup is about to do.
///
/// The third of the popup's three grounds, and the one that carries the consequence — the
/// button that acts and the sentence saying what happens if it is pressed. It is drawn as a
/// full-width band rather than as a row on the popup's own ground so that the eye finds the
/// action without reading for it, which is the same argument P7 §5 made for giving the
/// status bar rows of fixed height.
///
/// The negative outer margin is what makes it a band. `egui::Window` resolves to
/// `Frame::window`, whose inner margin insets its whole content; a footer that respected that
/// inset would be a floating strip with popup-coloured gutters down both sides and along the
/// bottom, which is a panel, not a foot. Pulling back by that inset puts the band's edges
/// back on the popup's edges.
///
/// **It pulls back by `PAD`, and `window_margin` is `PAD`, because P15 made them one number
/// again.** They had drifted apart to 12 and 14, and the two points between them showed as
/// popup ground down both sides of every foot and along its bottom edge — a band with a
/// gutter, which is the panel this function exists to avoid being. An audit found it, P13
/// wrote it down here rather than move a milestone that had been asked to stop moving, and
/// P15 spent the one line. The two must stay the same number: the pull-back is only correct
/// while it is exactly the inset it escapes.
pub fn foot(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::NONE
        .fill(POPUP_FOOT)
        .inner_margin(egui::Margin::symmetric(PAD, PAD - 2))
        .outer_margin(egui::Margin {
            left: -PAD,
            right: -PAD,
            top: 0,
            bottom: -PAD,
        })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add_contents(ui);
        });
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
        let r = ui.add(egui::Button::new(text).corner_radius(R_PILL));
        if r.clicked() {
            r.surrender_focus();
        }
        r
    } else {
        ui.add_enabled(
            false,
            egui::Button::new(text.color(TEXT_MUTED))
                .corner_radius(R_PILL)
                .fill(Color32::TRANSPARENT)
                .stroke(edge()),
        )
    }
}

/// The remove mark, `×` U+00D7 — the one place it is written.
///
/// Four rows carry a remove control: bookmarks and staged operations in the table, a watched
/// path in Settings, a queued task in Pending. Each typed this character inline, and because
/// it sits outside `mod icon` it escaped **all four** icon tests — including
/// `the_icon_registry_is_the_only_way_an_icon_is_defined`, which scans only inside the
/// module. That is the defect P23 §2d found, and this constant plus
/// `no_second_remove_mark_is_typed_into_the_source` is the whole of the fix.
///
/// **The other half of §2d's plan — moving it to `fa-times` U+F00D — measurement withdrew.**
/// The premise was that a mathematical operator drawn from the text portion of the face
/// "shares neither weight nor ink density with the Font Awesome marks beside it", and both
/// halves are false. Rastered at a common 18.2pt (`BODY * ICON_SCALE`) and summing coverage
/// over each glyph's own `uv_rect`, `×` fills **45.4%** of a 9×8 box and `fa-times` **46.1%**
/// of an 11×12 — the same weight, in a smaller box, which is what a glyph sized to text
/// rather than to an icon looks like. And no Font Awesome mark stands beside it in any of the
/// four rows: every one is a label or two and then this button, right-aligned, against 13pt
/// text. The swap would have made the mark *larger* than the row it ends.
///
/// `REMOVE` and not `CLOSE` because that is the job. CORE §4 writes "its own remove ✕" at
/// both places it names one, and none of the four closes anything.
///
/// **CORE writes `✕` U+2715 and the screen has always drawn `×` U+00D7.** The document's
/// character is in neither embedded face, so a literal reading of §4 would ship tofu, and
/// this silent divergence is the only reason those controls render at all. Which character
/// §4 should name is CORE's to settle and the amendment is drafted in `build/docs/P23.md`;
/// until it lands, the prose that quotes §4 keeps the document's `✕`, because a quotation
/// stays accurate even when the quoted text is wrong.
pub const REMOVE: &str = "×";

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
        let r = scoped.add(egui::Button::new(text).small().corner_radius(R_PILL));
        if r.clicked() {
            r.surrender_focus();
        }
        r
    } else {
        scoped.add_enabled(
            false,
            egui::Button::new(text.color(TEXT_MUTED))
                .small()
                .corner_radius(R_PILL)
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
                // **Without this the row is dead, and silently so.**
                //
                // `Interaction::selectable_labels` defaults to `true`, which makes every
                // plain `ui.label` allocate with `Sense::click_and_drag()` so its text can be
                // dragged out. A container registers its own sense *below* its content, and
                // the hit test takes the topmost click-sensing widget — so a row whose only
                // content is a label never hovers, never clicks, and gives no clue why.
                //
                // It lives here rather than at each call site because the first two rows
                // converted without it looked finished and did nothing. A row's whole purpose
                // is to be clicked; whatever is inside it is not competing for that.
                // Clone-on-write, so it dies with the row.
                ui.style_mut().interaction.selectable_labels = false;
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
            .size(SECTION)
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

    /// Composite a premultiplied translucent colour over an opaque ground.
    ///
    /// The palette's lines are translucent, so their declared byte value means nothing on
    /// its own — what a person sees is this, and it is what the figures are measured on.
    /// The entry table's selected fill is the same problem in another colour: it is `ORANGE`
    /// at 35%, and the ground a selected row's text sits on is only ever this mixture.
    ///
    /// The mixing is done in gamma space because that is where egui does it. Linear
    /// compositing gives a slightly kinder figure — for the selected row, 4.42:1 against
    /// 3.72:1 — so measuring here is the conservative of the two readings, not the flattering
    /// one.
    fn composite(over: Color32, ground: Color32) -> Color32 {
        let a = over.a() as f32 / 255.0;
        let mix = |o: u8, g: u8| (o as f32 + g as f32 * (1.0 - a)).round() as u8;
        Color32::from_rgb(
            mix(over.r(), ground.r()),
            mix(over.g(), ground.g()),
            mix(over.b(), ground.b()),
        )
    }

    /// CORE §6: a rule "clears **1.6:1 against the ground it is drawn on**, and that floor is
    /// a test, not an intention."
    ///
    /// This is that test. [`HAIRLINE`] spent eight milestones at 8% white, where it measured
    /// 1.18–1.27:1 — close enough to nothing that two separate notes in one testing round
    /// reported the rule above *New* and the rule under the filter bar as simply absent. The
    /// figure is easy to lose again by eye, because 8% and 20% white look much the same in a
    /// hex literal and not at all the same on a screen.
    ///
    /// Aubergine is in the list because a rule can be drawn inside a hovered row, which is
    /// the lightest ground any line in this program meets and therefore the worst case.
    #[test]
    fn a_rule_can_be_seen() {
        for (name, ground) in GROUNDS.iter().chain([&("AUBERGINE", AUBERGINE)]) {
            let seen = contrast(composite(HAIRLINE, *ground), *ground);
            assert!(
                seen >= 1.6,
                "a rule on {name} measures {seen:.2}:1, under the 1.6:1 CORE §6 sets"
            );
        }
    }

    /// The other half of *two weights and no third*: a rule must stay quieter than an edge,
    /// or the hierarchy is a thickness difference and nothing else.
    #[test]
    fn a_rule_is_quieter_than_an_edge() {
        assert!(
            HAIRLINE.a() < EDGE.a(),
            "the rule ({}) is not lighter than the edge ({})",
            HAIRLINE.a(),
            EDGE.a()
        );
    }

    fn visuals() -> egui::Visuals {
        let ctx = egui::Context::default();
        install_visuals(&ctx);
        ctx.style_of(egui::Theme::Dark).visuals.clone()
    }

    /// PXX: a fixed *em* advance is not a fixed *pixel* advance.
    ///
    /// P21 turned sub-pixel binning off and pinned it off here, on the reading that CORE §6's
    /// one monospace face means the advance is already whole-pixel and only binning's blur
    /// was being paid for. Cascadia Mono is 1200 units on a 2048-unit em — 0.586 em — so at
    /// this window's 12 / 13 / 17 px that is 7.031 / 7.617 / 9.961 px. None is integral, and
    /// with binning off the rounded origins at 13px sit 8, 7, 8, 7, 8, 8 apart. The evenness
    /// was never already there.
    ///
    /// **P23 changed the face and every number in that paragraph with it, and the conclusion
    /// did not move.** Fira was 600 on 1000; this face is 1200 on 2048. Two different
    /// fractions, neither of them a whole pixel at any size the window sets — which is the
    /// reason this test asserts the *setting* and not an arithmetic result. A face whose
    /// advance did land on a pixel would still not make binning-off correct at every zoom,
    /// and CORE §9's 100/125/150% are three more scales this would have to hold at.
    ///
    /// Pinned for the structural reason P21 pinned it the other way: the value is egui's
    /// default rather than something this function sets, so a toolkit upgrade that flipped
    /// that default would otherwise change how every glyph in the window is placed without
    /// one line of INDIUM saying so.
    #[test]
    fn a_fixed_em_advance_is_not_a_fixed_pixel_advance() {
        assert!(
            visuals().text_options.subpixel_binning,
            "sub-pixel binning is off — either something set it, or egui's default moved. \
             0.586 em x 13 px = 7.617 px, so the glyph origins round to gaps of 8, 7, 8, 7, \
             8, 8 and the window pays uneven spacing for even kerning it never had"
        );
    }

    /// The rest of the text pipeline is left exactly as egui ships it, and that is a
    /// decision rather than an omission: hinting is what makes stems land on pixels, and
    /// `TwoCoverageMinusCoverageSq` is the dark-mode gamma ramp this window wants. Both
    /// arrive through `Visuals::dark()`. If a future round is tempted to "add antialiasing",
    /// this is the test that says it is already there.
    #[test]
    fn glyphs_are_hinted_and_carry_the_dark_mode_ramp() {
        let v = visuals();
        assert!(v.text_options.font_hinting, "hinting was turned off");
        assert_eq!(
            v.text_options.color_transfer_function,
            egui::epaint::FontColorTransferFunction::DARK_MODE_DEFAULT,
            "the glyph coverage ramp is no longer the dark-mode one"
        );
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
    fn every_ink_is_legible_on_every_ground_it_is_allowed_on() {
        // AUBERGINE is `widgets.hovered.bg_fill` everywhere, and `selection.bg_fill` only
        // inside [`active_fill`]'s scope — the Inspector tabs, the Settings toggles and the
        // Create presets. It is the ground of every row `theme::row` draws active or
        // under the pointer, and it was not in GROUNDS, so nothing measured any ink on it.
        // `a_rule_can_be_seen` below chains it in by hand and calls it "the lightest ground
        // any line in this program meets" — the file knew.
        //
        // SELECTION_WASH is the ground the *entry table* selects onto, and it is a different
        // colour: `ui/table.rs` overrides the hover fill in its scope and leaves
        // `selection.bg_fill` global, which is `ORANGE` at 35% over the well beneath. It is
        // mixed at paint time rather than named in the palette, which is precisely why no
        // test reached it until P18's sweep did — the round fixed five Aubergine call sites
        // believing that was the selected ground, and the largest selected surface in the
        // program was somewhere else.
        //
        // egui composites this in gamma space, so that is what `over` reproduces. The linear
        // reading is the kinder one — 9.14/5.64/3.72 here against 10.85/6.70/4.42 — and the
        // verdict is the same in both: TEXT and TEXT_SECONDARY clear AA, TEXT_MUTED does not.
        // The floor is asserted against the gamma figure because it is the lower of the two.
        let grounds: Vec<(&str, Color32)> = GROUNDS
            .iter()
            .copied()
            .chain([
                ("AUBERGINE", AUBERGINE),
                ("AUBERGINE_LIT", AUBERGINE_LIT),
                (
                    "SELECTION_WASH",
                    composite(ORANGE.linear_multiply(0.35), WINDOW),
                ),
                // The entry table's own hover, which the comment below has cited by number
                // since P18 without any test computing it. `ui/table.rs:108` overrides
                // `widgets.hovered.bg_fill` inside its scope only, and the well it overrides
                // it in is `WINDOW` (`table.rs:48`), so this is the pair. It arrives measuring
                // TEXT_MUTED at 5.20:1 — the figure that comment already quotes.
                ("ROW_HOVER", composite(ROW_HOVER, WINDOW)),
            ])
            .collect();
        // ORANGE and WARNING are inks, and until this round nothing measured them as inks.
        // Both are painted as text: WARNING at `password.rs:113,127`, `settings.rs:45` and
        // `mod.rs:3871-3888`; ORANGE at `tray.rs:60` as a label and as the ink of the three
        // primary buttons — Apply in the tray and in Pending tasks, Create in the Create
        // popup (`tray.rs:67`, `pending.rs:113`, `newarchive.rs:281`).
        //
        // **The two spinners are not in this list and must not be.** `mod.rs:3956` and
        // `inspector.rs:557` paint ORANGE as a `Spinner`, which is a graphical object, not
        // text: WCAG asks 3:1 of it (1.4.11), not 4.5, and ORANGE clears that on both grounds
        // it spins over. Holding a spinner to a text floor would be measuring the wrong thing
        // and would put a fifth entry in the list below that no reader could act on.
        let inks = [
            ("TEXT", TEXT),
            ("TEXT_SECONDARY", TEXT_SECONDARY),
            ("TEXT_MUTED", TEXT_MUTED),
            ("ORANGE", ORANGE),
            ("WARNING", WARNING),
        ];

        // Pairs the program must never paint. AUBERGINE_LIT is a control being held down —
        // CORE §6, "alive only for as long as a control is held" — and holding it to the same
        // floor means the ink has to follow the press, which needs the `Response` inside
        // `theme::row`'s closure and all seven of its callers. Excluded deliberately, not
        // overlooked. TEXT_MUTED on AUBERGINE was painted in five places until P18, and on
        // SELECTION_WASH in three more the sweep found afterwards; the rows that persist in
        // either state now step one tier up, and both pairs stay listed here because the
        // palette still permits them and no test can see a call site.
        //
        // **What P18 did not reach, and it is the same obstacle:** `theme::row` gives
        // `widgets.hovered.bg_fill` the same Aubergine, so a row merely under the pointer has
        // the ground too — and in an immediate-mode frame the row is drawn before its own
        // `Response` exists, so a call site cannot know it is hovered in time to choose an
        // ink. What is fixed is every state that *persists* and that a call site can read
        // before it draws: active, focused, selected, on the cursor. A hover is transient and
        // still paints the quiet half at 3.30:1. The entry table's own hover is not affected
        // — it overrides the fill to ROW_HOVER, where TEXT_MUTED measures 5.20:1.
        //
        // ORANGE joins the list on three grounds it is never painted on. It is an accent,
        // not a text ramp: nothing draws orange words directly onto a popup's face, onto a
        // selected row, or onto a hovered one — the orange in those places is the *fill*, and
        // the ink over it is TEXT. Listed rather than omitted so that stays true.
        const FORBIDDEN: [(&str, &str); 7] = [
            ("TEXT_MUTED", "AUBERGINE"),
            ("TEXT_MUTED", "AUBERGINE_LIT"),
            ("TEXT_SECONDARY", "AUBERGINE_LIT"),
            ("TEXT_MUTED", "SELECTION_WASH"),
            ("ORANGE", "POPUP"),
            ("ORANGE", "SELECTION_WASH"),
            ("ORANGE", "ROW_HOVER"),
        ];

        // **These four are painted today, and they are under the floor.** That is a different
        // statement from the list above and it does not get to share its name: `FORBIDDEN`
        // means the program never does this, and the program does all four of these on every
        // launch. Measuring ORANGE as an ink for the first time is what surfaced them.
        //
        // | pair | measures | drawn by |
        // | --- | --- | --- |
        // | ORANGE on PANEL | 4.44 | `tray.rs:60` — the staging summary, [`BODY`] in [`MONO`] |
        // | ORANGE on CONTROL | 3.65 | Apply and Create at rest |
        // | ORANGE on AUBERGINE | 2.58 | the same three buttons, hovered |
        // | ORANGE on AUBERGINE_LIT | 2.06 | the same three, held down |
        //
        // The three buttons come from [`button`], which sets no fill when enabled, so the ink
        // rides egui's own widget states: `weak_bg_fill` is CONTROL inactive, AUBERGINE
        // hovered, AUBERGINE_LIT active (`install_visuals`).
        //
        // **The large-text exemption does not apply and cannot be made to.** WCAG's 3:1 tier
        // wants 18pt regular or 14pt bold; [`BODY`] is 13px, which is 9.75pt, so the bold
        // Apply is 9.75pt bold and short of the tier by a third. And the hovered and held
        // figures — 2.58 and 2.06 — are under 3:1 as well, so no reading of the exemption
        // reaches them even if the size did.
        //
        // **This is a finding, not a decision, and the decision is not mine.** ORANGE is
        // CORE §6's palette and CORE §6 gives it the meaning *something will happen*, so
        // every way out — darkening the token, giving the buttons an orange fill and a dark
        // ink, or accepting the figures on the record — edits a clause only the maker edits.
        // Until then the truth is pinned here rather than left unmeasured, and it is pinned
        // in **both** directions: heal one of these and this fires and says to move it up.
        const PAINTED_ANYWAY: [(&str, &str); 4] = [
            ("ORANGE", "PANEL"),
            ("ORANGE", "CONTROL"),
            ("ORANGE", "AUBERGINE"),
            ("ORANGE", "AUBERGINE_LIT"),
        ];

        for (gn, g) in &grounds {
            let resting = GROUNDS.iter().any(|(n, _)| n == gn);
            for (inn, ink) in inks {
                // TEXT is the subject of a line and is held higher than AA on the grounds a
                // window rests at; on a lit row it only has to be read.
                let floor = if inn == "TEXT" && resting { 7.0 } else { 4.5 };
                let seen = contrast(ink, *g);
                let forbidden = FORBIDDEN.contains(&(inn, gn));
                let painted_anyway = PAINTED_ANYWAY.contains(&(inn, gn));
                if forbidden {
                    // The list is checked in both directions so it cannot rot. If a palette
                    // change ever made one of these legible, this fires and says to take it
                    // off — an exclusion nobody rechecks is how the original hole was dug.
                    assert!(
                        seen < floor,
                        "{inn} on {gn} measures {seen:.2}:1 and clears {floor:.1} — it is no \
                         longer forbidden, so take it off the list"
                    );
                } else if painted_anyway {
                    assert!(
                        seen < floor,
                        "{inn} on {gn} measures {seen:.2}:1 and now clears {floor:.1} — the \
                         program paints this pair and it used to be under the floor, so \
                         whatever just fixed it means this comes off PAINTED_ANYWAY and the \
                         round document's open finding is closed"
                    );
                } else {
                    assert!(
                        seen >= floor,
                        "{inn} on {gn} is {seen:.2}:1, under the {floor:.1} this ground asks for"
                    );
                }
            }
        }

        // Measured minima, so the numbers are in the record and not only in the floors:
        // TEXT 11.46, TEXT_SECONDARY 7.08, TEXT_MUTED 4.67 — all three on CONTROL, the
        // lightest of the six. On AUBERGINE: 8.12, 5.01, 3.30. On AUBERGINE_LIT: 6.48, 4.00,
        // 2.64. TEXT_MUTED cannot reach 4.5 on AUBERGINE and stay muted: it would have to be
        // about #B3B3B1, which is TEXT_SECONDARY in all but name, and the ink ladder would
        // collapse from three tiers to two. So the call sites moved, not the palette.
    }

    /// [`SCRIM`]'s job is the opposite of every other figure in this file: the window behind
    /// a modal has to stop being readable.
    ///
    /// **The plan asked for the scrim as a *ground*, and that framing does not survive being
    /// run.** A backdrop is painted **over** the pixels behind it, so the ink is dimmed by the
    /// same wash as the surface under it — modelling it as a ground dims only the ground, and
    /// then the figures go the wrong way entirely: TEXT on WINDOW comes out at **16.65 against
    /// its bare 15.13**, and the scrim reads as having *improved* legibility. It is the fourth
    /// figure in this plan to change on contact with a measurement, and the most misleading,
    /// because the wrong model returns a plausible number instead of an obvious error.
    ///
    /// Modelled the way it is painted, it does what it is for. TEXT on WINDOW falls
    /// **15.13 → 1.72**, and the whole matrix lands between 1.27 and 1.75. So the assertion
    /// here is a **ceiling**: nothing behind the scrim may reach 2.0, which is under even the
    /// 1.6 CORE §6 asks of a hairline. Weakening the alpha to make the backdrop prettier is
    /// the change this catches, and it is a plausible thing for someone to want.
    ///
    /// The floor legs elsewhere in this file are proved by a control that must fail them.
    /// This one carries its own: the same arithmetic at **half the scrim's alpha** has to come
    /// back **over** the ceiling, or the model is not responding to `SCRIM` at all and the
    /// ceiling is being cleared by something other than the wash.
    #[test]
    fn the_scrim_puts_the_window_behind_it_out_of_reach() {
        const CEILING: f32 = 2.0;
        let inks = [
            ("TEXT", TEXT),
            ("TEXT_SECONDARY", TEXT_SECONDARY),
            ("TEXT_MUTED", TEXT_MUTED),
        ];
        let mut worst: f32 = 0.0;
        for (gn, g) in GROUNDS.iter() {
            for (inn, ink) in inks {
                let dimmed = contrast(composite(SCRIM, ink), composite(SCRIM, *g));
                worst = worst.max(dimmed);
                assert!(
                    dimmed < CEILING,
                    "behind the scrim, {inn} on {gn} still reads at {dimmed:.2}:1 — a modal's \
                     backdrop is supposed to put the window out of reach, and at that figure \
                     the two Modals are a tint rather than a scrim"
                );
            }
        }
        // The control: the same sum with the wash at half strength must break the ceiling.
        let thin = SCRIM.linear_multiply(0.5);
        let leaked = contrast(composite(thin, TEXT), composite(thin, WINDOW));
        assert!(
            leaked > CEILING,
            "at half alpha the scrim still holds TEXT on WINDOW to {leaked:.2}:1, under the \
             {CEILING:.1} ceiling — so the ceiling above is not being cleared by SCRIM and \
             this test would pass with no scrim at all"
        );
        assert!(
            worst > 1.0,
            "nothing behind the scrim measured above 1.0:1, which is not suppression but an \
             arithmetic fault — 1.0 is two identical colours"
        );
    }

    /// Read a file under the crate root at test time.
    fn source(rel: &str) -> String {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{rel} is readable: {e}"))
    }

    /// Every `.rs` file under a directory, named relative to the crate root and sorted, with
    /// the floor that keeps a scan honest.
    ///
    /// `least` is not a formality. Every caller here is a test that passes by finding nothing,
    /// so a walk that silently returns an empty list turns each of them green for the one
    /// reason none of them is checking.
    fn sources_under(rel: &str, least: usize) -> Vec<(String, String)> {
        fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            for entry in std::fs::read_dir(dir)
                .unwrap_or_else(|e| panic!("{} is readable: {e}", dir.display()))
                .flatten()
            {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, out);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    out.push(path);
                }
            }
        }
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut paths = Vec::new();
        walk(&root.join(rel), &mut paths);
        paths.sort();
        assert!(
            paths.len() >= least,
            "only {} file(s) under {rel}, fewer than the {least} this scan expects — a scan \
             that found nothing must not read as a scan that found nothing wrong",
            paths.len()
        );
        paths
            .into_iter()
            .map(|p| {
                let name = p
                    .strip_prefix(root)
                    .unwrap_or(&p)
                    .to_string_lossy()
                    .into_owned();
                let body = std::fs::read_to_string(&p).expect("a listed file is readable");
                (name, body)
            })
            .collect()
    }

    const INK_TOKENS: [&str; 3] = ["TEXT_SECONDARY", "TEXT_MUTED", "TEXT"];

    /// Resonance's clause 13, which INDIUM nearly satisfies already: a text token is text ink
    /// and nothing else — never a fill, never a separator, never a rule.
    ///
    /// **This is the half that holds, and the census below is the half that does not.** Across
    /// every file under `src/ui/` the three tokens appear only as a colour: `.color(…)` on a
    /// `RichText`, a `painter.text` colour argument, or an `ink`/`dim` variable feeding one.
    /// Zero fills and zero strokes, which is the property worth keeping and the reason to
    /// spend a test on it.
    ///
    /// The scan normalises whitespace before looking, because the regression it is for does
    /// not have to fit on one line — `rustfmt` breaks a long `rect_filled` across three, and a
    /// line-at-a-time scan would read straight past it. What it cannot see is a text token put
    /// into a variable and *then* used as a fill; that is the gap, and it is stated rather
    /// than papered over. The census test below closes the other direction.
    #[test]
    fn a_text_token_in_the_window_is_only_ever_text() {
        // Sinks, spelled as they appear immediately before a colour argument. The window is
        // 32 characters because that is long enough to hold `rect_filled(rect, 0.0, ` and
        // `Stroke::new(1.0, ` and short enough not to reach back over a builder chain that
        // legitimately sets a fill and an ink in the same expression.
        const SINKS: [&str; 7] = [
            "fill(",
            "filled(",
            "stroke(",
            "Stroke::new(",
            "bg_fill",
            "hline(",
            "vline(",
        ];
        let mut seen = 0usize;
        for (name, body) in sources_under("src/ui", 10) {
            // Code only. A comment is free to name a sink and a token in one breath — several
            // in this crate do, explaining why the pairing would be wrong — and a scan that
            // cannot tell prose from code would fire on the very notes that agree with it.
            let flat: String = body
                .lines()
                .map(|l| l.trim())
                .filter(|l| !l.starts_with("//"))
                .collect::<Vec<_>>()
                .join(" ");
            for token in INK_TOKENS {
                let needle = format!("theme::{token}");
                let mut from = 0;
                while let Some(at) = flat[from..].find(&needle) {
                    let at = from + at;
                    from = at + needle.len();
                    // `TEXT` is a prefix of the other two; only count the exact token.
                    if flat[from..].starts_with(|c: char| c.is_alphanumeric() || c == '_') {
                        continue;
                    }
                    seen += 1;
                    // Back up to a char boundary: this file's neighbours are full of em
                    // dashes, and 32 bytes lands inside one sooner or later.
                    let mut lo = at.saturating_sub(32);
                    while lo < at && !flat.is_char_boundary(lo) {
                        lo += 1;
                    }
                    let back = &flat[lo..at];
                    if let Some(sink) = SINKS.iter().find(|s| back.contains(**s)) {
                        panic!(
                            "{name} puts {token} into `{sink}` — \"…{back}{needle}\". The three \
                             text tokens are text ink and nothing else: a fill or a rule that \
                             wants this weight takes it from the ground ramp or from \
                             `HAIRLINE`, which are measured for the job"
                        );
                    }
                }
            }
        }
        // The scan must have found the tokens it was scanning for.
        assert!(
            seen >= 100,
            "only {seen} text-token uses across src/ui — this scan is not reaching the code it \
             is supposed to be checking"
        );
    }

    /// The other direction: the places a text token is *not* text, all of which are here.
    ///
    /// The plan said **six** and then listed eight, which is the fifth figure in this round to
    /// need re-deriving rather than transcribing — its line numbers had moved too, by the
    /// width of everything 2c and 2e added. So the census is taken from the file at test time
    /// and pinned by field name, and it is the field names that matter: a ninth entry means
    /// someone has given a text token a job that is not text, and it should have to be written
    /// down here to do it.
    ///
    /// They are not all the same kind of thing, and the plan's flat count is what hid that:
    ///
    /// - **Five `fg_stroke`s** are text in stroke's clothing. `fg_stroke` is what egui draws a
    ///   widget's own label and glyphs with, so these are the ink following the widget state —
    ///   the role, not a departure from it.
    /// - **Three marks** genuinely are not text: the caret, and the two IME underlines. A
    ///   caret is the ink's own cursor and an IME underline sits under the text it belongs to,
    ///   so both take the ink deliberately; that is the argument, and it is worth having
    ///   written down because it is the one a reviewer would otherwise have to reconstruct.
    /// - **`override_text_color`** is listed because it is where the ink is *installed*, and a
    ///   census with the origin missing would look complete while being the one line that
    ///   matters most.
    #[test]
    fn the_only_text_tokens_that_are_not_text_are_these_nine() {
        const CENSUS: [(&str, &str); 9] = [
            ("override_text_color", "TEXT"),
            ("widgets.noninteractive.fg_stroke", "TEXT_SECONDARY"),
            ("widgets.inactive.fg_stroke", "TEXT_SECONDARY"),
            ("widgets.hovered.fg_stroke", "TEXT"),
            ("widgets.active.fg_stroke", "TEXT"),
            ("widgets.open.fg_stroke", "TEXT"),
            ("text_cursor.stroke", "TEXT"),
            ("ime_composition.active_underline_stroke", "TEXT"),
            ("ime_composition.inactive_underline_stroke", "TEXT_MUTED"),
        ];

        let body = source("src/theme.rs");
        let start = body
            .find("pub fn install_visuals(")
            .expect("install_visuals is in this file");
        let len = body[start..]
            .find("\n}\n")
            .expect("install_visuals has an end");
        let scope = &body[start..start + len];

        let mut found: Vec<(String, String)> = Vec::new();
        for line in scope.lines() {
            let line = line.trim();
            if line.starts_with("//") || !line.starts_with("v.") {
                continue;
            }
            let Some((lhs, rhs)) = line.split_once('=') else {
                continue;
            };
            // `TEXT` is a prefix of the other two, so the longest match wins.
            let Some(token) = INK_TOKENS.iter().find(|t| {
                rhs.split(|c: char| !(c.is_alphanumeric() || c == '_'))
                    .any(|w| w == **t)
            }) else {
                continue;
            };
            found.push((
                lhs.trim().trim_start_matches("v.").to_string(),
                (*token).to_string(),
            ));
        }

        let expected: Vec<(String, String)> = CENSUS
            .iter()
            .map(|(f, t)| ((*f).to_string(), (*t).to_string()))
            .collect();
        assert_eq!(
            found, expected,
            "the census of text tokens inside `install_visuals` has changed. Every entry here \
             is a place one of the three inks is something other than a `RichText` colour, and \
             the list is exhaustive on purpose — a new one is a text token being given a \
             second job, which is the thing clause 13 is about. If the new entry is right, add \
             it with the reason; if it is a fill or a rule, it wants a ground token instead"
        );
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
        // and it belongs to the popup's own grounds rather than to the four faces of a
        // button. `the_popup_title_bar_is_not_egui_grey` and `the_popup_wears_its_own_three
        // _grounds` are what guard that field.
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

    /// Named after the verdict that ordered it: the popup "still failed to distinguish"
    /// itself from the window behind it while both were aubergine one step apart.
    ///
    /// The three grounds must be a popup's own, must be legible, and must not be the
    /// window's colour — and the last of those is checked on **hue**, because the whole
    /// point of the change is that luminance was never going to do it. The popup body and
    /// the window sit 1.19:1 apart by luminance and always will; what tells them apart is
    /// that one is blue-dominant and the other red-dominant.
    #[test]
    fn the_popup_wears_its_own_three_grounds() {
        let bands = [
            ("POPUP_HEAD", POPUP_HEAD),
            ("POPUP", POPUP),
            ("POPUP_FOOT", POPUP_FOOT),
        ];

        for (name, g) in bands {
            assert!(
                contrast(TEXT, g) >= 7.0,
                "TEXT on {name} is {:.2}:1",
                contrast(TEXT, g)
            );
            // The middle tier, checked here from P18. It was the omission one test over as
            // well, and it passes comfortably — 7.82 to 9.67 across the three bands.
            assert!(
                contrast(TEXT_SECONDARY, g) >= 4.5,
                "TEXT_SECONDARY on {name} is {:.2}:1",
                contrast(TEXT_SECONDARY, g)
            );
            assert!(
                contrast(TEXT_MUTED, g) >= 4.5,
                "TEXT_MUTED on {name} is {:.2}:1 — the first pick of this palette failed \
                 here at 1.44:1, and that is the whole reason these values are not it",
                contrast(TEXT_MUTED, g)
            );
            assert!(
                g.b() > g.r(),
                "{name} must be blue-dominant, or it is the window wearing a different hat"
            );
        }

        for (name, g) in bands {
            for (wn, w) in [("WINDOW", WINDOW), ("PANEL", PANEL), ("CONTROL", CONTROL)] {
                assert!(
                    w.r() > w.b() && g.b() > g.r(),
                    "{name} and {wn} must not share a hue family"
                );
            }
        }

        // The band is a lid, not a highlight: both bands sit below the popup's own ground.
        assert!(luminance(POPUP) > luminance(POPUP_HEAD));
        assert!(luminance(POPUP_HEAD) > luminance(POPUP_FOOT));
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

    // -----------------------------------------------------------------------
    // The icons exist
    // -----------------------------------------------------------------------

    /// Read a big-endian `u16` / `u32` out of a font table.
    fn be16(d: &[u8], o: usize) -> u32 {
        u16::from_be_bytes([d[o], d[o + 1]]) as u32
    }
    fn be32(d: &[u8], o: usize) -> u32 {
        u32::from_be_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]])
    }

    /// Does this face have a glyph for `cp`?
    ///
    /// A hand-rolled `cmap` walk, and deliberately so: the alternative is a font-parsing
    /// crate as a dev-dependency to answer one yes-or-no question, which is exactly what
    /// CORE §2 exists to refuse. Format 12 is preferred over format 4 because it is exact
    /// — a codepoint inside a format-12 group *has* a glyph, where a format-4 segment can
    /// still map to glyph 0 — and this face carries one.
    fn face_covers(face: &[u8], cp: u32) -> bool {
        let tables = be16(face, 4) as usize;
        let mut cmap = None;
        for i in 0..tables {
            let rec = 12 + i * 16;
            if &face[rec..rec + 4] == b"cmap" {
                cmap = Some(be32(face, rec + 8) as usize);
            }
        }
        let cmap = cmap.expect("the face has no cmap table");

        let subs = be16(face, cmap + 2) as usize;
        let mut chosen: Option<(u32, usize)> = None;
        for i in 0..subs {
            let sub = cmap + be32(face, cmap + 4 + i * 8 + 4) as usize;
            let fmt = be16(face, sub);
            if fmt == 12 {
                chosen = Some((12, sub));
                break;
            }
            if fmt == 4 && chosen.is_none() {
                chosen = Some((4, sub));
            }
        }
        match chosen.expect("the face has no format 4 or 12 cmap subtable") {
            (12, sub) => {
                let groups = be32(face, sub + 12) as usize;
                (0..groups).any(|g| {
                    let p = sub + 16 + g * 12;
                    cp >= be32(face, p) && cp <= be32(face, p + 4)
                })
            }
            (_, sub) => {
                let segx2 = be16(face, sub + 6) as usize;
                let ends = sub + 14;
                let starts = sub + 16 + segx2;
                (0..segx2 / 2).any(|i| {
                    let s = be16(face, starts + i * 2);
                    s != 0xFFFF && cp >= s && cp <= be16(face, ends + i * 2)
                })
            }
        }
    }

    /// CORE §6: "Icons are glyphs of that same face."
    ///
    /// The whole point of the icon work is that it costs no new asset — which is only true
    /// while every glyph it names is actually in the two files the binary carries. A
    /// codepoint that is not there does not fail loudly; it draws a box, and a box is
    /// indistinguishable from a font that failed to load. So it is checked here, against
    /// the same bytes `install_fonts` hands to egui, in **both** weights — the bold cut is
    /// a separate file and a separate `cmap`, and a bold sidebar row would be the place a
    /// one-weight assumption showed up.
    #[test]
    fn every_icon_exists_in_both_faces() {
        // Every test that consumes the generated list needs this floor: `icons! {}` with an
        // empty body expands to an empty `ALL`, and a loop over nothing passes.
        assert!(!icon::ALL.is_empty(), "icon::ALL is empty");
        for (name, glyph) in icon::ALL {
            let mut chars = glyph.chars();
            let cp = chars.next().expect("an icon cannot be the empty string") as u32;
            assert!(
                chars.next().is_none(),
                "icon::{name} is more than one glyph, which breaks the single-cell rule"
            );
            for (weight, face) in [("regular", FACE_REGULAR), ("bold", FACE_BOLD)] {
                assert!(
                    face_covers(face, cp),
                    "icon::{name} (U+{cp:04X}) is missing from the {weight} face; \
                     it would draw as tofu"
                );
            }
        }
    }

    /// The general form of [`no_second_remove_mark_is_typed_into_the_source`], which that test
    /// names as owed to this section: **every character the program draws has to be a
    /// character the face can draw.**
    ///
    /// A missing codepoint does not fail — it draws a box, and a box looks exactly like a font
    /// that did not load. `icon::ALL` has been checked against both faces since P16, but only
    /// the icons: an ordinary sentence with an arrow in it went through no check at all, and
    /// **that is how `⇄` reached a release.** `keys.rs:38` draws `"Details ⇄ Preview"` from
    /// CORE §4's own keyboard table, U+21C4 is in neither weight, and it has been a tofu box
    /// in the Keys popup in every version that had one. Verified twice in P23 §2f — this
    /// crate's `cmap` reader and `fc-query` over the two `.ttf` files from outside the
    /// codebase, agreeing on every codepoint.
    ///
    /// **So it lands with one named exception rather than red.** The fix is CORE draft E —
    /// `⇄` → `↔` U+2194, which *is* in both weights — and CORE is the maker's hand, so the
    /// alternative to naming it is a suite that stays red until he gets to it. A named
    /// exception still fails on a *second* one, which is the whole job; a red suite fails at
    /// everything and is therefore read as failing at nothing. It is asserted rather than
    /// skipped, so the day `keys.rs:38` is corrected this test says to delete the line.
    ///
    /// Test modules are cut at their `#[cfg(test)]`, because an assertion message is not
    /// something the window draws — `table.rs` quotes `✕` in one, and it is right to.
    #[test]
    fn every_drawn_character_exists_in_both_faces() {
        // (file, codepoint). One entry, and it is expected to become zero.
        const EXCEPTIONS: [(&str, u32); 1] = [("src/ui/keys.rs", 0x21C4)];

        /// The string literals on one line, escapes skipped.
        ///
        /// It counts quotes, so a `char` literal that *is* a quote — `'"'` — opens a string
        /// that never closes and the rest of that line is lost. The damage stops at the
        /// newline: `inside` does not outlive the call. Two such lines are in scanned code,
        /// both in `src/platform/apps.rs`, which parses `.desktop` `Exec=` lines and draws
        /// nothing at all; the rest are inside test modules, which are cut. Raw strings
        /// escape differently and are likewise all in test modules. The python survey this
        /// was cross-checked against shares the same blindness, so its agreement is not
        /// coverage of this.
        fn literals(line: &str) -> Vec<String> {
            let (mut out, mut cur, mut inside) = (Vec::new(), String::new(), false);
            let mut chars = line.chars();
            while let Some(c) = chars.next() {
                if !inside {
                    inside = c == '"';
                } else {
                    match c {
                        '\\' => {
                            chars.next();
                        }
                        '"' => {
                            out.push(std::mem::take(&mut cur));
                            inside = false;
                        }
                        _ => cur.push(c),
                    }
                }
            }
            out
        }

        let mut checked = 0usize;
        for (name, body) in sources_under("src", 30) {
            for line in body.lines() {
                if line.starts_with("#[cfg(test)]") {
                    break;
                }
                if line.trim_start().starts_with("//") {
                    continue;
                }
                for lit in literals(line) {
                    for ch in lit.chars().filter(|c| !c.is_ascii()) {
                        let cp = ch as u32;
                        checked += 1;
                        if EXCEPTIONS.contains(&(name.as_str(), cp)) {
                            continue;
                        }
                        assert!(
                            face_covers(FACE_REGULAR, cp) && face_covers(FACE_BOLD, cp),
                            "{name} draws U+{cp:04X} `{ch}`, which is not in both embedded \
                             weights and will render as a tofu box. Either use a character \
                             the face carries, or — if CORE names this one — draft the \
                             amendment and add it to EXCEPTIONS with the reason"
                        );
                    }
                }
            }
        }

        // Both legs of the exception list, so it cannot rot into a permanent excuse.
        for (file, cp) in EXCEPTIONS {
            assert!(
                !(face_covers(FACE_REGULAR, cp) && face_covers(FACE_BOLD, cp)),
                "U+{cp:04X} is in both faces now, so {file} is no longer an exception — take \
                 it off the list"
            );
            let body = source(file);
            let drawn = body
                .lines()
                .take_while(|l| !l.starts_with("#[cfg(test)]"))
                .any(|l| {
                    !l.trim_start().starts_with("//")
                        && literals(l)
                            .iter()
                            .any(|s| s.chars().any(|c| c as u32 == cp))
                });
            assert!(
                drawn,
                "{file} no longer draws U+{cp:04X}, so the exception is stale — CORE draft E \
                 has landed or the string was reworded, and this line comes out"
            );
        }
        assert!(
            checked >= 50,
            "only {checked} non-ASCII character(s) found across src/ — this scan is not \
             reaching the literals it is supposed to be reading"
        );
    }

    /// Named-by-role is only worth anything if the names are distinct: two roles sharing a
    /// codepoint is a copy-paste slip that reads as a deliberate choice on screen.
    #[test]
    fn no_two_icons_are_the_same_glyph() {
        assert!(!icon::ALL.is_empty(), "icon::ALL is empty");
        for (i, (na, a)) in icon::ALL.iter().enumerate() {
            for (nb, b) in &icon::ALL[i + 1..] {
                assert_ne!(a, b, "icon::{na} and icon::{nb} are the same glyph");
            }
        }
    }

    /// CORE §6's Base row names the aubergine ladder, and this is that ladder.
    ///
    /// The row said *six* and listed *five* for nine milestones: it was six when P7 built it,
    /// and P9 took the popup off the ladder — *"a popup is not on this ladder at all"*, two
    /// clauses further along the same row — without taking it out of the count. Both the word
    /// and the hexes are read here, so neither can move without the other.
    ///
    /// `GROUNDS` above is deliberately **not** this list: it is every surface text is drawn on,
    /// popup included, which is a different question and rightly has a different answer.
    #[test]
    fn the_ground_ladder_is_the_one_core_six_lists() {
        let core = include_str!("../CORE.md");
        // Anchored to §6 rather than swept for across the whole document. `| Base |` is
        // unique in CORE today, but the row is quoted in two P-documents already, and nothing
        // stops a later section opening a table whose first cell reads the same.
        let section = core
            .split_once("## 6. LOOK")
            .expect("CORE has a section 6 heading")
            .1;
        let row = section
            .lines()
            .find(|l| l.starts_with("| Base |"))
            .expect("CORE §6 has a Base row");

        let hexes: Vec<String> = row
            .split('`')
            .filter(|s| s.len() == 7 && s.starts_with('#'))
            .map(|s| s.to_ascii_uppercase())
            .collect();
        let ladder = [
            ("VOID", VOID),
            ("STATUS_BAR", STATUS_BAR),
            ("WINDOW", WINDOW),
            ("PANEL", PANEL),
            ("CONTROL", CONTROL),
        ];
        assert_eq!(
            hexes.len(),
            ladder.len(),
            "CORE §6's Base row names {} colours and the ladder has {}: {hexes:?}",
            hexes.len(),
            ladder.len()
        );
        for (i, (hex, (name, c))) in hexes.iter().zip(ladder.iter()).enumerate() {
            let want = format!("#{:02X}{:02X}{:02X}", c.r(), c.g(), c.b());
            assert_eq!(
                hex, &want,
                "rung {i}: CORE §6 says {hex}, and {name} is {want}"
            );
        }

        // The row opens by counting itself. A number word and a list that disagree is the whole
        // defect this test exists for, so the word is read too rather than trusted.
        const WORDS: [(&str, usize); 8] = [
            ("Three", 3),
            ("Four", 4),
            ("Five", 5),
            ("Six", 6),
            ("Seven", 7),
            ("Eight", 8),
            ("Nine", 9),
            ("Ten", 10),
        ];
        let word = row
            .trim_start_matches("| Base |")
            .split_whitespace()
            .next()
            .expect("the Base row's cell is not empty");
        let counted = WORDS
            .iter()
            .find(|(w, _)| *w == word)
            .unwrap_or_else(|| panic!("the Base row opens with {word:?}, which is not a count"))
            .1;
        assert_eq!(
            counted,
            ladder.len(),
            "CORE §6's Base row says {word} and lists {} grounds",
            ladder.len()
        );
    }

    /// CORE §6: icons "come from the **Font Awesome** range the Nerd Font patches in, and no
    /// second range is mixed with it: mixing icon families reads exactly like mixing typefaces."
    ///
    /// **This bound is typed, and it is the one number in this file that could not be derived.**
    /// The embedded face covers `U+F000`–`U+F385` as a single unbroken run of 902 codepoints —
    /// Font Awesome's historical block and Font Logos' distro marks, with no gap between them —
    /// so the `cmap`, the only source in the tree that moves when the face does, cannot tell the
    /// two families apart. `every_icon_exists_in_both_faces` will happily pass a distro logo at
    /// `U+F31A` or a Codicon at `U+EA60`, because both are genuinely in both faces. This is the
    /// only test that will not.
    ///
    /// Where the boundary is honestly fuzzy: Nerd Fonts v3 patches in a *second* Font Awesome
    /// span as well — the v6 icons, measured in this face at `U+ED00`–`U+EFCF`. §6 says
    /// "range", singular, and every glyph in [`icon`] was chosen from the v4 block. Admitting
    /// the newer span is a decision made by editing this constant and writing down why, not by
    /// a glyph quietly passing.
    #[test]
    fn every_icon_comes_from_the_font_awesome_range_and_no_second_one() {
        const FONT_AWESOME: std::ops::RangeInclusive<u32> = 0xF000..=0xF2E0;
        assert!(!icon::ALL.is_empty(), "icon::ALL is empty");
        for (name, glyph) in icon::ALL {
            let cp = glyph
                .chars()
                .next()
                .expect("an icon cannot be the empty string") as u32;
            assert!(
                FONT_AWESOME.contains(&cp),
                "icon::{name} (U+{cp:04X}) is outside the Font Awesome block \
                 U+{:04X}–U+{:04X}; CORE §6 mixes no second range",
                FONT_AWESOME.start(),
                FONT_AWESOME.end()
            );
        }
    }

    /// The `icons!` macro is the only way an icon is defined, so [`icon::ALL`] cannot go stale.
    ///
    /// The macro closes the drift it was written for — a constant declared through it is
    /// registered by the same syntax — but a `pub const` typed *beside* the invocation would
    /// still escape both other tests. Nothing but the module's own source can see that, which
    /// is why this test reads the file it lives in. Unusual, and deliberate.
    ///
    /// The macro *definition* is cut out before the scan, and nothing else is: its body holds
    /// the literal text `pub const $name` and `pub const ALL`, so a naive search finds the
    /// machinery instead of a bypass. Everything else inside `mod icon` — including the gap
    /// between the macro and its invocation, which is exactly where a stray constant would sit
    /// — is scanned.
    #[test]
    fn the_icon_registry_is_the_only_way_an_icon_is_defined() {
        // Line-based throughout, so a checkout with CRLF endings reads the same as this one:
        // `include_str!` embeds a file's bytes verbatim — rustc's newline normalisation is for
        // source literals, not included files — and nothing in the tree pins `eol=lf`. A byte
        // scan for "\n}\n" would fail on such a checkout and blame the one thing that is
        // certainly true, that `mod icon` is closed. `lines()` drops the `\r` and the question
        // does not arise.
        let lines: Vec<&str> = include_str!("theme.rs").lines().collect();
        let open = lines
            .iter()
            .position(|l| l.trim_end() == "pub mod icon {")
            .expect("theme.rs declares `pub mod icon`");
        // `mod icon` closes with a `}` in the first column; every brace inside it is indented.
        let close = open
            + 1
            + lines[open + 1..]
                .iter()
                .position(|l| l.trim_end() == "}")
                .expect("`mod icon` is closed");
        let body = &lines[open + 1..close];

        let mac = body
            .iter()
            .position(|l| l.trim_start().starts_with("macro_rules! icons {"))
            .expect("`mod icon` defines the icons! macro");
        let mac_end = mac
            + 1
            + body[mac + 1..]
                .iter()
                .position(|l| l.trim_end() == "    }")
                .expect("the icons! macro definition is closed");

        // What is scanned is the module minus the macro's own definition — whose body holds
        // the literal text `pub const $name` and `pub const ALL`, so a naive search finds the
        // machinery instead of a bypass — and minus every comment line. An icon's `///`, and
        // the macro's own doc above it, are entitled to say "pub const"; prose about a
        // declaration is not a declaration, and a test that cannot tell the two apart fails
        // the next time someone documents the macro. What remains is the gap between the
        // macro and its invocation, which is exactly where a stray constant would sit.
        let code: Vec<&str> = body[..mac]
            .iter()
            .chain(&body[mac_end + 1..])
            .copied()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect();

        assert!(
            code.iter().any(|l| l.contains("icons! {")),
            "the icons! invocation was cut away with the definition — this test would \
             otherwise pass by absence"
        );
        // Any public item, not merely a `pub const`: a `pub static`, or a `pub use` renaming
        // one of the generated constants, would ship a glyph past `ALL` just as well, and the
        // claim this test is named for is that the macro is the *only* way an icon is defined.
        for l in &code {
            assert!(
                !l.trim_start().starts_with("pub "),
                "`mod icon` declares {:?} outside the icons! invocation; it would be drawn but \
                 checked by nothing, because every other icon test sees only what ALL names",
                l.trim()
            );
        }
    }

    /// [`REMOVE`] is where the remove mark is written, and the only place.
    ///
    /// The four rows that draw one had each typed `×` inline, and because the character lives
    /// outside `mod icon` every icon test looked straight past it —
    /// [`the_icon_registry_is_the_only_way_an_icon_is_defined`] scans between `pub mod icon {`
    /// and its closing brace and cannot see anything else by construction. This scans the
    /// whole of `src/` from disk instead, so a fifth copy is caught in a file that does not
    /// exist yet as readily as in one of the four that started it.
    ///
    /// It matches the quoted token `"×"` rather than the bare character, and skips comment
    /// lines, so prose stays free to name the mark: every mention in the tree writes it in
    /// backticks, and the two rules together let a doc comment quote CORE's own sentence
    /// verbatim without tripping a test about what the screen draws.
    ///
    /// **`✕` U+2715 is asserted absent for a harder reason than tidiness: neither embedded
    /// face carries it.** CORE §4 names it in two places, so a hand correcting the code to
    /// match the document would ship a tofu box in four rows and see nothing wrong in the
    /// diff. That is not hypothetical elsewhere — `keys.rs` draws `⇄` U+21C4 from §4's own
    /// keyboard table, which is absent too and therefore ships as tofu today. The general
    /// form of this check, over every non-ASCII character in every drawn literal, is owed to
    /// P23 §2f, where it can land green behind a named exception; here the scope is the one
    /// mark this constant replaced.
    #[test]
    fn no_second_remove_mark_is_typed_into_the_source() {
        fn rs_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            for e in std::fs::read_dir(dir).expect("a readable directory under src/") {
                let p = e.expect("a readable entry").path();
                // The dotfile skip is lib.rs's, for lib.rs's reason: an editor's lock file is
                // `.#theme.rs`, whose extension is `rs` and whose contents are not source.
                let hidden = p
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with('.'));
                if p.is_dir() {
                    rs_files(&p, out);
                } else if p.extension().is_some_and(|x| x == "rs") && !hidden {
                    out.push(p);
                }
            }
        }

        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        rs_files(&root, &mut files);
        files.sort();
        assert!(
            files.len() > 10,
            "only {} .rs files found under {} — this test would otherwise pass by absence",
            files.len(),
            root.display()
        );

        let mut drawn: Vec<(String, String)> = Vec::new();
        let mut tofu: Vec<String> = Vec::new();
        for p in &files {
            let text = std::fs::read_to_string(p).expect("a readable .rs file");
            let name = p.strip_prefix(&root).unwrap_or(p).display().to_string();
            for (i, l) in text.lines().enumerate() {
                if l.trim_start().starts_with("//") {
                    continue;
                }
                if l.contains("\"\u{00d7}\"") {
                    drawn.push((format!("src/{name}:{}", i + 1), l.trim().to_string()));
                }
                if l.contains("\"\u{2715}\"") {
                    tofu.push(format!("src/{name}:{}", i + 1));
                }
            }
        }

        assert!(
            tofu.is_empty(),
            "`✕` U+2715 is in neither embedded face and would draw as a tofu box; CORE §4 \
             names it but the screen has never drawn it. Written at {tofu:?}"
        );
        assert_eq!(
            drawn.len(),
            1,
            "`×` should be written once, in theme.rs's REMOVE; found it at {:?}",
            drawn.iter().map(|(w, _)| w).collect::<Vec<_>>()
        );
        let (where_, line) = &drawn[0];
        assert!(
            line.starts_with("pub const REMOVE"),
            "the one `×` in the tree is at {where_}, which is {line:?} rather than REMOVE's \
             own declaration"
        );
    }

    /// CORE §2: *"a filename holding `->` must render as the two characters the archive
    /// stores, and a face that cannot form the ligature cannot get that wrong."*
    ///
    /// **The clause had no test for sixteen milestones, and P23 is the round that found out
    /// why it needed one.** The redesign set out to swap in CaskaydiaCove — Cascadia *Code*,
    /// the ligature cut — on the reasoning that egui applies no OpenType shaping, so the
    /// face's ligatures could never fire. That reasoning was wrong. `epaint` 0.36 shapes
    /// through `harfrust`, a HarfBuzz port, with `ShapeOptions::new()` — HarfBuzz's *default*
    /// feature set, which has `calt` on, and `calt` is where Cascadia keeps its ligatures.
    /// The Cove cut substitutes on every sequence below. `www` is the clearest: one glyph
    /// 23 pixels wide followed by two zero-width continuations, in place of three `w`s.
    ///
    /// So the guarantee comes from the face after all, exactly as §2 always said — and it is
    /// held here rather than asserted in a comment, because the next swap will be made by
    /// someone who did not run this experiment. Both weights, because the bold cut is a
    /// separate file with a separate GSUB.
    ///
    /// **Sub-pixel binning is switched off inside this test and nowhere else.** With it on —
    /// which is what ships, and what
    /// [`a_fixed_em_advance_is_not_a_fixed_pixel_advance`] pins — one glyph is cached as
    /// several rasters, one per fractional origin, so the same `w` at x=7.62 and at x=15.24
    /// occupies different atlas slots at different pixel sizes. That is *position*, and this
    /// test is about *identity*: without switching it off, every glyph after the first
    /// reports as substituted and the check is worthless. It cost an hour to find.
    #[test]
    fn a_filename_is_the_characters_it_holds() {
        // Every sequence Cascadia Code ligates that can legally appear in a filename. `/`
        // cannot be in a name, but it can be in an archive *path*, which the entry table
        // draws — and `//` ligates too.
        const SEQUENCES: &[&str] = &[
            "->", "=>", "<-", "!=", "==", "===", ">=", "<=", "|>", "::", "++", "--", "&&", "||",
            "//", "/*", "*/", "<>", ">>", "<<", "?.", "??", "~~", "www", "a->b", ".hpp", "0xFF",
            "#!", "__",
        ];

        for weight in ["regular", "bold"] {
            let ctx = egui::Context::default();
            install(&ctx);
            // See the note above: identity, not position.
            ctx.all_styles_mut(|st| st.visuals.text_options.subpixel_binning = false);
            let mut warm = ctx.run_ui(Default::default(), |_| {});
            warm.textures_delta.clear();

            let family = if weight == "bold" { bold() } else { MONO };
            let font = egui::FontId::new(BODY, family);
            let render = |s: &str| -> Vec<(char, f32, String)> {
                let galley =
                    ctx.fonts_mut(|f| f.layout_no_wrap(s.to_owned(), font.clone(), Color32::WHITE));
                galley.rows[0]
                    .glyphs
                    .iter()
                    .map(|g| (g.chr, g.advance_width, format!("{:?}", g.uv_rect)))
                    .collect()
            };

            for seq in SEQUENCES {
                let drawn = render(seq);

                // One glyph per character. A ligature that replaced two with one would fail
                // here first.
                assert_eq!(
                    drawn.len(),
                    seq.chars().count(),
                    "the {weight} face draws {seq:?} as {} glyphs for {} characters",
                    drawn.len(),
                    seq.chars().count()
                );

                for (i, (chr, advance, uv)) in drawn.iter().enumerate() {
                    // A ligature in a monospace face keeps the cell count by emitting
                    // zero-width continuation glyphs — `epaint`'s own mechanism, named in
                    // `text_layout.rs`. So the width is not evidence and the *ink* is.
                    assert!(
                        *advance > 0.0,
                        "the {weight} face draws {seq:?} glyph #{i} ({chr:?}) with no \
                         advance — a zero-width continuation, which is what a ligature \
                         leaves behind"
                    );

                    // The decisive one: the glyph drawn in context is the glyph drawn
                    // alone. Anything else is a substitution, whatever it looks like.
                    let alone = render(&chr.to_string());
                    assert_eq!(
                        &alone[0].2, uv,
                        "the {weight} face draws {chr:?} differently inside {seq:?} than on \
                         its own — GSUB substituted it, and a name is no longer the \
                         characters the archive stores"
                    );
                }
            }
        }
    }

    /// [`SB_ROW`] is tall enough for the glyph it was sized for — and now says so.
    ///
    /// **It was fitted to a face and pinned by nothing.** P13 grew the status-bar row from 20
    /// to 24 because an icon at [`ICON_SCALE`] does not fit a 20px row — not the 18.2pt glyph
    /// itself, but the *line box* egui gives it, which is the number that actually drives
    /// layout. That measurement was taken against Fira Mono and then left as a literal, so
    /// the next face swap could have shrunk the bar's headroom to nothing without one test
    /// noticing. P23 is that swap.
    ///
    /// Measured rather than assumed, and the swap made it roomier rather than tighter:
    /// Fira's line box at 18.2px was 21.844 and Cascadia Mono's is 21.125, so the slack went
    /// from 2.156px to 2.875px. **`SB_ROW` therefore does not move** — the bar keeps the
    /// height thirteen milestones of screenshots were taken at, and this test is what makes
    /// that a decision rather than a coincidence nobody re-checked.
    #[test]
    fn the_status_bar_row_fits_the_icon_it_was_grown_for() {
        let ctx = egui::Context::default();
        install(&ctx);
        let mut warm = ctx.run_ui(Default::default(), |_| {});
        warm.textures_delta.clear();

        // The tallest thing the bar draws: an icon beside Body text, which is 13.0.
        let icon = egui::FontId::new(BODY * ICON_SCALE, MONO);
        let line_box = ctx.fonts_mut(|f| f.row_height(&icon));

        assert!(
            line_box <= SB_ROW,
            "an icon at {}px has a {line_box}px line box and SB_ROW is {SB_ROW} — the status \
             bar is shorter than the glyph it exists to carry, which is the defect P13 grew \
             the row to fix",
            BODY * ICON_SCALE
        );

        // The other half, and the reason this is not simply `<=`: a bar with a great deal of
        // slack is a bar nobody re-derived. Two pixels of headroom is the row doing its job;
        // eight would mean the constant had stopped tracking the face entirely.
        assert!(
            SB_ROW - line_box <= 6.0,
            "SB_ROW is {SB_ROW} and the icon's line box is only {line_box}px — that is \
             {}px of unexplained headroom, so the constant has drifted away from the face \
             it is supposed to be fitted to",
            SB_ROW - line_box
        );
    }

    /// The type scale is the only place a drawn size is written down.
    ///
    /// **This is the test that makes §2c a decision rather than a tidy-up.** Naming six roles
    /// is worth nothing on its own: the program had five named sizes in `install_spacing`
    /// before this round and still drew text at eight, because a call site can always type a
    /// number. A hundred and eleven of them had. Renaming without a guard buys one clean
    /// afternoon and the same drift by the next round.
    ///
    /// It reads the tree rather than a list of files, so a `ui/` module added later is scanned
    /// the day it appears — the one thing an `include_str!` sweep of named files cannot do,
    /// and the way [`the_icon_registry_is_the_only_way_an_icon_is_defined`]'s narrower scan
    /// would have missed the four inline `×` marks §2d found.
    ///
    /// **There are no exceptions, and that cost a constant.** The status bar's spinner takes
    /// `.size()` too, on an `egui::Spinner`, where it means a diameter — so the honest fix was
    /// [`SPINNER_D`], not a name on an allow-list. An exception list is where a scale goes to
    /// die: the seventh size arrives as the second exception and nobody re-reads why the first
    /// one was granted.
    #[test]
    fn every_drawn_size_comes_from_the_type_scale() {
        // The tree, walked at test time. `include_str!` would pin the file list at compile
        // time, which is the failure this is written against.
        fn rs_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            let read = std::fs::read_dir(dir)
                .unwrap_or_else(|e| panic!("{} is readable: {e}", dir.display()));
            for entry in read {
                let path = entry.expect("a readable directory entry").path();
                if path.is_dir() {
                    rs_files(&path, out);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    out.push(path);
                }
            }
        }
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        rs_files(&root, &mut files);
        files.sort();
        // A tree that reads as empty is a test that passes by accident. `src/` has had more
        // than thirty files since P16 and cannot plausibly shrink to five.
        assert!(
            files.len() >= 20,
            "only {} .rs files found under {} — the scan is not seeing the tree it is \
             supposed to be guarding",
            files.len(),
            root.display()
        );

        // Two spellings reach the font: `RichText::size` and `FontId::new`. Both are matched
        // only where a *digit* follows, so `.size(theme::BODY)` and this file's own mentions
        // of the method by name are not hits.
        const SPELLINGS: [&str; 2] = [".size(", "FontId::new("];
        let mut strays = Vec::new();
        for file in &files {
            let text = std::fs::read_to_string(file)
                .unwrap_or_else(|e| panic!("{} is readable: {e}", file.display()));
            for (n, line) in text.lines().enumerate() {
                // Prose is entitled to quote a number; a comment is not a call. The icon
                // registry's guard learned this the same way — a test that cannot tell a
                // declaration from a sentence about one fails the next time someone
                // documents the thing it guards.
                if line.trim_start().starts_with("//") {
                    continue;
                }
                for spelling in SPELLINGS {
                    let mut rest = line;
                    while let Some(at) = rest.find(spelling) {
                        let after = &rest[at + spelling.len()..];
                        if after.starts_with(|c: char| c.is_ascii_digit()) {
                            strays.push(format!(
                                "{}:{}: {}",
                                file.strip_prefix(&root).unwrap_or(file).display(),
                                n + 1,
                                line.trim()
                            ));
                        }
                        rest = &rest[at + spelling.len()..];
                    }
                }
            }
        }

        assert!(
            strays.is_empty(),
            "{} drawn size(s) bypass the type scale:\n{}\n\nEvery size in the window is one \
             of WORDMARK {WORDMARK}, WORDMARK_SM {WORDMARK_SM}, TITLE {TITLE}, SECTION \
             {SECTION}, BODY {BODY} or SMALL {SMALL} — or, if it is a length rather than \
             type, a named one like SPINNER_D. A seventh number needs a role and a reason, \
             both of them next to the other six.",
            strays.len(),
            strays.join("\n")
        );
    }

    /// [`BODY`] is thirteen because CORE §6 argues from thirteen, twice.
    ///
    /// The maker asked this round to rethink every size in the window, and every size in the
    /// window was this round's to move — except one. §6 builds the icon scale on *"about nine
    /// points of ink next to a **thirteen-point** capital"*, and its file-type-icon refusal on
    /// *"at `ICON_SCALE` a **13pt** glyph does not fit a 20px row"*. Both sentences are
    /// arguments, not descriptions: change the body size and they stop being true, in a
    /// document that is the maker's to edit and nobody else's.
    ///
    /// So this is the binding rather than a comment saying so. It fails from either end — a
    /// hand moving `BODY`, or a hand editing those sentences without moving `BODY` — which is
    /// the only arrangement that survives someone who has read neither.
    #[test]
    fn core_s_icon_argument_is_the_reason_body_is_thirteen() {
        let core = include_str!("../CORE.md");
        let look = core
            .split_once("## 6. LOOK")
            .expect("CORE has a section 6 heading")
            .1;
        // Two independent sentences, so a single edit cannot quietly unpin the constant.
        for phrase in ["thirteen-point capital", "a 13pt glyph does not fit"] {
            assert!(
                look.contains(phrase),
                "CORE §6 no longer says \"{phrase}\" — the sentence BODY = {BODY} was pinned \
                 to has been edited. If the maker moved the body size, move BODY with it and \
                 re-derive SB_ROW and the icon step; if the wording merely changed, re-anchor \
                 this test. Do not delete it: the constant is load-bearing for CORE's own \
                 argument about icons."
            );
        }
        assert_eq!(
            BODY, 13.0,
            "BODY is {BODY} while CORE §6 argues the icon scale from a thirteen-point capital \
             and refuses file-type icons because a 13pt glyph will not fit a 20px row. Those \
             two sentences are the maker's to change (CORE.md:3-5), so this constant cannot \
             move ahead of them."
        );
    }

    /// One popup on a bench: a context, a window, and frames you can vary.
    ///
    /// **The pointer goes in through `RawInput::events`, which is the only door that
    /// works here.** P23 spent an afternoon on `ydotool` before this: `--absolute` is inert
    /// on this compositor, relative moves are accelerated, and no synthetic press ever
    /// reached the window. All of that is the OS layer. `Event::PointerButton` is on the
    /// far side of it, and egui cannot tell it from a hand.
    ///
    /// `anchored` swaps [`floating`] back for the `.anchor()` call it replaced. It is the
    /// control leg: the assertions are also run against the popup as it shipped, so a
    /// harness that cannot drag anything fails the test it is supposed to pass.
    ///
    /// `screen` is public because one of the tests below resizes the window mid-run.
    struct Bench {
        ctx: egui::Context,
        screen: egui::Rect,
        anchored: bool,
    }

    impl Bench {
        fn new(anchored: bool) -> Self {
            let ctx = egui::Context::default();
            install(&ctx);
            Self {
                ctx,
                screen: egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0)),
                anchored,
            }
        }

        /// One frame. With `show` false the window is not raised at all, which is what a
        /// closed popup looks like from egui's side — `open: bool` guards the `show` call
        /// at all eight sites.
        fn frame(&self, events: Vec<egui::Event>, show: bool) -> egui::Rect {
            let mut rect = egui::Rect::NOTHING;
            let mut full = self.ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(self.screen),
                    events,
                    ..Default::default()
                },
                |root| {
                    if !show {
                        return;
                    }
                    let ctx = root.ctx().clone();
                    let w = if self.anchored {
                        egui::Window::new("Drag me")
                            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                    } else {
                        floating(&ctx, "Drag me")
                    };
                    if let Some(r) = w.collapsible(false).resizable(false).show(&ctx, |ui| {
                        ui.set_min_width(300.0);
                        ui.label("something to give the window a size");
                    }) {
                        rect = r.response.rect;
                    }
                },
            );
            // `FullOutput` panics on drop with an unapplied delta; nothing here paints it.
            full.textures_delta.clear();
            rect
        }

        /// Raise it and let the sizing pass settle: the first frame is uninteractable on
        /// purpose — `Area::begin` marks it so — and the popup is only really open on the
        /// second.
        fn open(&self) -> egui::Rect {
            self.frame(vec![], true);
            self.frame(vec![], true)
        }

        /// Grab the middle of the title band, a few pixels down from the top edge, and pull.
        fn drag(&self, from: egui::Rect, by: egui::Vec2) -> egui::Rect {
            let grab = from.center_top() + egui::vec2(0.0, 8.0);
            self.frame(vec![egui::Event::PointerMoved(grab)], true);
            self.frame(
                vec![egui::Event::PointerButton {
                    pos: grab,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::default(),
                }],
                true,
            );
            self.frame(vec![egui::Event::PointerMoved(grab + by)], true);
            self.frame(vec![], true)
        }

        fn content(&self) -> egui::Rect {
            self.ctx.input(|i| i.content_rect())
        }
    }

    /// The maker's question — *"Can popups float and be moved by mouse freely?"* — as an
    /// assertion rather than an answer.
    ///
    /// Two properties, because [`floating`] replaced one call that did two things. The popup
    /// still opens where the anchor put it: dead centre of the content rect, which is the
    /// rect the anchor was aligning within. And it now moves when dragged, which the anchored
    /// leg proves is not something this harness would report either way.
    #[test]
    fn a_popup_opens_centred_and_can_then_be_dragged_off_centre() {
        let drag = egui::vec2(120.0, -60.0);

        let bench = Bench::new(false);
        let opened = bench.open();
        let content = bench.content();
        let moved = bench.drag(opened, drag);
        assert!(
            (opened.center() - content.center()).length() < 1.0,
            "a floating popup opens at {:?}, which is not the centre of {:?} — \
             `default_pos` and the pivot are supposed to reproduce exactly where \
             `.anchor(CENTER_CENTER, ZERO)` put it, and dropping the anchor without them \
             leaves egui's cascading top-left instead",
            opened.center(),
            content.center()
        );
        assert!(
            (moved.min - opened.min - drag).length() < 1.0,
            "dragged by {drag:?} the popup went from {:?} to {:?} — it is supposed to \
             follow the pointer, and `Area::anchor` ending in `movable(false)` is the only \
             reason it ever did not",
            opened.min,
            moved.min
        );

        let control = Bench::new(true);
        let anchored_open = control.open();
        let anchored_moved = control.drag(anchored_open, drag);
        assert_eq!(
            anchored_moved.min, anchored_open.min,
            "the anchored control moved, so this harness cannot tell a movable popup from a \
             fixed one and neither assertion above means anything"
        );
    }

    /// Where a popup opens the *second* time — measured, because [`floating`]'s doc claims it.
    ///
    /// The claim was read off egui's source before it was ever run: `Area::begin` fills
    /// `pivot_pos` through `get_or_insert_with` (egui-0.36.1 `area.rs:459-462`), so
    /// `default_pos` is consulted once per `Id` and a popup that has been dragged reopens
    /// where it was left. That is an inference, and this round's own gate row says
    /// *measured rather than argued*, so here it is closed and reopened instead.
    ///
    /// The same cache has a **second consequence, and it is the one worth knowing about**:
    /// a popup that was never touched also stops re-centring. Resize the window mid-run and
    /// reopen it and it comes back at the old centre, because the cached position is not
    /// recomputed — under `.anchor()` it was, every frame. Nothing is lost off-screen
    /// (`constrain_window_rect_to_area` still clamps it), it is simply no longer centred.
    ///
    /// **This is the chosen behaviour, not a defect**, and it is written down here because
    /// stage 3's round-13 walk resizes the window to 125% and 150% and reopens popups at
    /// each — without this paragraph the walker files "popup not centred" and is right to.
    /// The anchored control leg measures the old behaviour beside it, so the difference is
    /// on the record as a difference rather than as a description.
    #[test]
    fn a_dragged_popup_reopens_where_it_was_left_and_a_resize_no_longer_re_centres_one() {
        let drag = egui::vec2(120.0, -60.0);
        let bench = Bench::new(false);
        let opened = bench.open();
        let moved = bench.drag(opened, drag);

        // Without this leg the one below is vacuous: a popup that never moved reopens at the
        // position it never left, and the assertion holds while measuring nothing.
        assert!(
            (moved.min - opened.min).length() > 1.0,
            "the drag did not move the popup, so \"it reopens where it was left\" is being \
             asserted about a popup that was never anywhere else"
        );

        // Close it — three frames with nothing raised — and open it again.
        for _ in 0..3 {
            bench.frame(vec![], false);
        }
        let reopened = bench.open();
        assert!(
            (reopened.min - moved.min).length() < 1.0,
            "dragged to {:?} and closed, the popup reopened at {:?} — `default_pos` is \
             supposed to be consulted once per `Id`, so this is either egui dropping the \
             `Area` state of a window that missed a frame or `floating` being handed a \
             position every time",
            moved.min,
            reopened.min
        );

        // A popup nobody touched, and a window that changes size under it.
        for (anchored, label) in [(false, "floating"), (true, "anchored")] {
            let mut bench = Bench::new(anchored);
            let first = bench.open();
            assert!(
                (first.center() - bench.content().center()).length() < 1.0,
                "the {label} popup did not open centred, so the resize below proves nothing"
            );
            bench.screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
            for _ in 0..3 {
                bench.frame(vec![], false);
            }
            let after = bench.open();
            let off = (after.center() - bench.content().center()).length();
            if anchored {
                assert!(
                    off < 1.0,
                    "the anchored control did not re-centre after the resize either, so the \
                     floating leg below is not measuring a change — it is measuring a harness \
                     that cannot see one"
                );
            } else {
                assert!(
                    off > 1.0,
                    "a floating popup re-centred itself after the window resized, {off:.1}px \
                     from centre. That is the anchor's behaviour, and `floating` does not have \
                     the anchor: `pivot_pos` is cached per `Id`. If egui has started \
                     recomputing it, this test's doc — and P23 §2e's note to the round-13 \
                     walker — are describing something that no longer happens"
                );
            }
        }
    }

    /// Every popup is built by [`floating`], and the next one will be too.
    ///
    /// The test above proves a popup *can* be dragged; this one is why the ninth popup will be
    /// draggable without anybody remembering to make it so. A `Window` raised directly in
    /// `src/ui/` gets egui's defaults — the cascading corner from `automatic_area_position`,
    /// and none of the reasoning in `floating`'s doc — and it would look like a placement bug
    /// long before anyone traced it back to a missing call.
    ///
    /// Scoped to `src/ui/` on purpose, and with **no exceptions**: the two `Window::new` calls
    /// in this file are `floating` itself and the control leg above, and neither is a popup.
    #[test]
    fn every_popup_is_raised_through_floating() {
        // The walk, and the "a scan that found nothing is not a scan that found nothing
        // wrong" guard with it, are [`sources_under`]. This test wrote both first; 2f needed the
        // same two and copying them a second time is the shape 2c was deleting.
        let mut strays = Vec::new();
        for (name, text) in sources_under("src/ui", 10) {
            for (n, line) in text.lines().enumerate() {
                if line.trim_start().starts_with("//") {
                    continue;
                }
                if line.contains("egui::Window::new") {
                    strays.push(format!("{name}:{}: {}", n + 1, line.trim()));
                }
            }
        }
        assert!(
            strays.is_empty(),
            "{} popup(s) raise an egui::Window directly instead of theme::floating:\n{}\n\n\
             `floating` is what centres a popup and lets it be dragged; a bare Window gets \
             egui's cascading top-left and cannot be moved into place because it was never \
             put in one. If this is deliberately not a popup, it does not belong in src/ui.",
            strays.len(),
            strays.join("\n")
        );
    }
}
