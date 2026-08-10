# INDIUM — CORE

*The authoritative design document.* The P-documents in `build/docs/` implement this file;
when a P-document and this file disagree, this file wins. Changes to this file are made by
the maker's hand, recorded here, and nowhere else.

---

## 1. WHAT INDIUM IS

INDIUM is an archive manager for Linux on Wayland, written in Rust, drawn with egui, and
shaped around one idea: **the metadata is the main event.** Every other archiver on this
platform treats an archive's contents as a name column and hides the rest behind a
Properties dialog. INDIUM keeps a permanent Inspector pane on screen — sizes, packed sizes,
ratio, method, checksums, four timestamps, ownership, mode, link targets, encryption state —
because the stated ambition of this program is to be one of the most verbose archiver
applications in the industry.

All format work happens **inside the process.** INDIUM never runs `7z`, `tar`, `unzip`,
`zstd`, or any other external compressor. When a format is listed as supported, it is
supported by code linked into the binary, on every machine, whether or not any archive
tool is installed.

One archive per window. Opening a second archive opens a second window. There are no tabs.

---

## 2. DEPENDENCIES

The rule of this section is simple: a dependency enters INDIUM only when it does genuine
work for the program, and this section must say **in one sentence** what that work is.
A dependency that cannot fill in its sentence does not get in. The count was never the
point — an archiver that installs a password manager it will never open is bloated at
five dependencies, and honest at fifty.

### Direct crates

| Crate | Enters at | Its sentence |
| --- | --- | --- |
| `eframe` (features: `glow`, `wayland` — nothing else) | P1 | Draws the entire program: window, input, GL context. The only UI dependency. |
| `egui_extras` | P1 | Provides the virtualized table that lets a 100,000-entry archive scroll like a 100-entry one. |
| `sevenz-rust2` | P4 | Writes 7z with AES-256, which libarchive cannot do; also the source of 7z-specific detail (solid blocks) the generic reader does not expose. |
| `wl-clipboard-rs` | P3 | Puts `text/uri-list` on the Wayland clipboard so copy-out works into any file manager. |
| `image` (via `egui_extras` `image` feature) | already linked; formats chosen at P5 | Decodes the image formats the Preview tab shows. It is not a new dependency — `eframe` pulls it through its clipboard path already, with PNG on — so P5 names the formats rather than adding the crate. |
| `serde` + `toml` | P2 | Read and write the settings, bookmarks, and recent-files TOML files. |
| `ashpd` (+ `zbus`, `futures-lite`) | P11 | The desktop's own file picker, over `xdg-desktop-portal`. The only alternative was drawing a file dialog, and §6 has no vocabulary for one. It brings a D-Bus stack with it — the largest dependency INDIUM has taken since libarchive, and taken deliberately: the picker a user has already chosen beats one INDIUM invents, and it is the only kind that survives a sandbox. P13 turns on its `open_uri` feature as well, for *"show me this folder"* — the feature is `open_uri = []` upstream, so it adds no crate and no linkage, only D-Bus calls over the `zbus` already here. |

Everything not in this table is the standard library or hand-written: CRC32 is a
twenty-line table, byte formatting is ten lines, argument handling is `std::env::args`.
`clap` arrives only if V1.3's headless subcommands justify its sentence.

### System libraries

| Library | Its sentence |
| --- | --- |
| `libarchive` | Reads and writes every supported container and filter in-process. It is a hard dependency of pacman itself, so it is present on every Arch machine that can install software; on Debian the package declares `libarchive13t64 \| libarchive13`, naming both because the time64 transition renamed it in trixie and a package that spans both suites has to say so. |
| `libwayland-client`, `libxkbcommon`, `libEGL`/GL | What the compositor session already provides; winit needs them to exist, and they do. |
| `glibc`, `libgcc_s`, `libm` | The floor. |

No GTK. No Qt. No KF6. No portal. `build/check-deps.sh` runs
`ldd target/release/indium` and fails if the output contains `gtk`, `Qt`, `KF6`, `X11`,
or `portal`. It runs by hand until V1.4 wires it into CI, and it runs before every release.

Bundled assets, not dependencies: Fira Mono Nerd Font Mono, regular and bold, embedded in
the binary, with the SIL Open Font Licence 1.1 alongside the GPL in `LICENSES/`. Fira Mono
carries **no ligatures at all**, which is the property that matters here — a filename
holding `->` must render as the two characters the archive stores, and a face that cannot
form the ligature cannot get that wrong. `Mono` is the single-cell icon cut, so a glyph in
a name never widens a table column.

---

## 3. ARCHITECTURE

One binary crate, `indium`, with modules. No workspace, no premature abstraction.

| Module | Owns |
| --- | --- |
| `arch` | Hand-written FFI over system libarchive (~15 functions) and the safe wrapper around it. Listing streams entries over a channel from a worker thread; extraction runs with libarchive's secure flags (`SECURE_SYMLINKS`, `SECURE_NODOTDOT`) so a hostile archive cannot write outside its target. |
| `model` | Archive state: entries, selection, the open archive's identity. |
| `tasks` | The staging engine. Every mutation — add, remove, rename, create — is a task in a queue. **Apply** builds the new archive in a temp file beside the target, verifies it by walking its entries, then atomically renames over the original. The original is never touched until the replacement is proven. |
| `ui` | The window: sidebar, table, Inspector, tray, status bar, and every popup. |
| `platform` | The Linux specifics: clipboard, `.desktop` parsing for Open With, default-app registration, XDG paths, the second window — on this platform a window is a process, and opening one is a Linux specific like the rest — and handing a directory to the desktop's file manager, which is the portal's job for the same reason the picker is. |
| `theme` | The Aubergine palette, the fonts, and nothing configurable. |

Threading: the UI thread and one worker. The worker opens, lists, extracts, and rebuilds;
it reports progress over a channel and honours a cancellation flag. egui runs in reactive
mode — an idle INDIUM repaints nothing and costs nothing, on any refresh rate.

Passwords are requested at the moment of use, passed down, and zeroed when the operation
ends. They are never written to settings, recents, or anywhere else.

---

## 4. THE WINDOW

Five fixed zones and nine popups. Nothing else appears, ever.

**Sidebar** (family style): the wordmark at top, then *Open file* `O` and *Archive* `1`; a
rule; then *Bookmarks* `2` and *Recent files* `3`; at the bottom *New* `N`, *Settings* `,`,
*About* `A`. Numbers and letters are bare keypresses, as in JADEITE. Every row carries a
leading glyph in the same ink as its label, so the column can be found by shape before it is
read; §6 says which glyphs and what they are allowed to do.

The archive sits above the rule and the two lists below it because the first thing a person
looks for is the archive they are already inside — the order used to run the other way, and a
testing round said so plainly. *Open file* keeps the archive's company for the same reason: it
is how you get into one. The rule is a rule and not a gap; §6 fixes what it has to be to be
seen.

**Entry table**: virtualized; columns Name, Size, Packed, Method; a breadcrumb path above
it, with *Add files…* at the far end of that row — the picker adds into the directory the
breadcrumb names, which is the one placement that needs no explanation. `Enter` descends
into a directory, `Backspace` goes up. `Ctrl+F` opens a filter bar — there is deliberately
no type-to-jump, because bare letters are shortcuts.

**Inspector** (right, permanent): two tabs, *Details* and *Preview*, toggled with `Space`.
Details shows everything the reader can know about the selection; multi-select shows
aggregates; no selection shows the archive-level card. Preview renders text and images
(hex arrives at V1.1). Two honest sourcing notes, so nobody files a bug about them:
libarchive does not expose an entry's *stored* CRC, so INDIUM computes CRC32 on demand
and labels it computed; 7z solid-block detail arrives with `sevenz-rust2` in P4, and until
then the Inspector shows what the generic reader provides.

**Staging tray**: hidden until the first staged change, then a one-line strip above the
status bar — count, a summary of the first tasks, *Discard*, **Apply**. The strip itself
is a button.

**Status bar**: three rows, each of a fixed height, so the floor of the window never moves
between idle and working. *What is open* — the archive's name, its format and filter, and its
directory. **The directory is elided in the middle, never at the end**, because the end is the
folder the archive is actually in and the start is the tree it belongs to; a path that keeps
only one of those has kept the wrong half. The whole path is on hover, and clicking it hands
that folder to the desktop's file manager. *The numbers and the voice* — entry count,
real → packed with ratio, the selection count, and whatever INDIUM is currently saying, drawn
whether or not something is running. *Progress* — the phase, the count and its cancel during
long work; a hairline when nothing is running, because an empty row must still say something.
**The proportion done is drawn as a 2px line along the bar's own top edge**, not as a track
inside the row: it is the one measurement in the window that wants the whole width, and the
edge is already there. The line carries no text of its own — `#EEEEEC` on Ubuntu Orange
measures 2.4:1 — so the phase and the count are read beside it and never on it.

Three rows of fixed height is not the same as three rows anyone can read, and the first testing
round said the bar *"looks like a mess. Cant really track whats going on there."* It was right:
ten fields in one size, one weight and three greys, with nothing to say which mattered. So the
bar has a hierarchy, and it is part of this document rather than a matter of taste:

- **One thing per row is the subject, and it is bold.** The archive's name on the first row, the
  entry count on the second, the phase on the third. Everything else on that row is its
  qualifier, and secondary.
- **A rule separates the rows.** They are three statements, not one paragraph in three pieces.
- **Numbers hold their columns.** Sizes, counts and ratios are right-aligned to fixed positions
  and do not move as their digits change; a number that jumps as it counts cannot be read while
  it counts.
- **A failure is `#FFD800`.** What INDIUM says is the only text in the window that reports both
  triumph and disaster in the same place, and until now it reported them in the same colour.
  Warning is already the document's word for *something has gone wrong*, and this is that.
  A refusal is a failure; a confirmation is not.

### The popups

1. **New Archive** (`N`). A subwindow, Clonezilla in content, and wearing the popup's own
   three grounds rather than the window's. An instruction line at top ("Choose how INDIUM
   should compress. If unsure, keep the default."). Four preset chips — *Fastest*,
   *Balanced* (default), *Smallest*, *Encrypted* — each highlighting a row in the method
   list below, where **every method
   carries its one-sentence verdict** (§5). An *Advanced* disclosure holds the level
   slider. At the foot, a live sentence states exactly what will be built:
   *"Building photos-2026.7z — 7z, LZMA2:19, AES-256."*
2. **Pending tasks** (`W`, or clicking the tray). The full task list: one row per staged
   operation with its own remove ✕, then *Discard all* and **Apply**.
3. **Extract** (`E`). A popover: *Extract here*, *Extract to `<name>/`*, a path field with
   tab completion, bookmarks beneath. Enter-driven.
4. **Open With** (`Enter` on a file, after extraction to the runtime dir). Applications
   from parsed `.desktop` files, ranked by MIME match, filter-as-you-type.
5. **Settings** (`,`). One small panel: bookmarks, default extract behaviour. TOML behind it.
6. **About** (`A`). The mark, the maker, the version and date, the source address and the
   licence in full. Addresses are text you can select but not click — INDIUM opens no
   browser and follows no link, by design.
7. **Password** (modal). Appears at the moment of use, per use, and nowhere survives it.
8. **Open** (`Ctrl+O`). A path field with tab completion, a *Browse…* button beside it
   raising the desktop's own picker through `xdg-desktop-portal`, and the only popup that
   is not about the archive already open. Naming an archive this window does not hold
   opens it in a window of its own, per §1.
9. **Keys** (`F1`). The table below, drawn in the window. It exists because a person who had
   used the program for an afternoon wrote *"I didn't know `Ctrl+O` opens a file, and still
   don't know how to exit from the archive"* — a program whose whole interface is bare
   keypresses owes the reader the list. It is **generated from the bindings, never typed
   twice**: a keys popup that has drifted from the keys is worse than no keys popup.

### Keyboard

| Key | Does |
| --- | --- |
| `1` `2` `3` | Sidebar sections |
| `O` / `I` | Open file · Add files — both raise the desktop's own picker |
| `N` `W` `E` `A` `,` | New Archive · Pending tasks · Extract · About · Settings |
| `F1` | Keys — this table, in the window |
| Arrows, `PgUp/PgDn`, `Home/End` | Move in the table |
| `Enter` / `Backspace` | Descend / go up |
| `Space` | Details ⇄ Preview |
| `Ctrl+F` | Filter bar |
| `Ctrl+A` | Select all |
| `Ctrl+C` | Copy out (extract to runtime dir, URIs to clipboard) |
| `Ctrl+V` / drop files | Stage an add. A drop needs X11: winit has no Wayland drag-and-drop, so on Wayland the compositor never delivers one. `Ctrl+V` and *Add files…* are the routes that always work. |
| `Del` / `F2` | Stage a remove / a rename |
| `Ctrl+O` | Open (path field) |
| `Esc` | Close the topmost popup |

No modal editing, no `hjkl`, no `:` commands — anywhere, ever.

---

## 5. FORMATS

**Read** — everything system libarchive reads, with one deliberate hole: tar in all its
variants, zip, 7z, cpio, ar, xar, mtree, iso9660, cab, lha, deb and rpm as containers,
through every filter (gzip, bzip2, xz, lzma, lzip, lz4, zstd, lzop, lrzip, compress).
Encrypted zip and 7z entries are read after a per-use password prompt.

**RAR is deliberately absent — not read, not written.** The format's owner permits no one
to create RAR archives, and the maker has ruled the format out entirely rather than carry
half of it. Opening one produces a plain sentence: *"RAR is not supported."* The reader
capability sits unused inside libarchive; INDIUM checks the detected format after open and
refuses. ACE is absent for the same family of reasons and its security history.

**Write** — `tar`, plain or with the filters `gz`, `bz2`, `xz`, `zst`, `lz4`; `zip` (Deflate);
`7z` (LZMA2, via `sevenz-rust2`). Encryption is **7z AES-256 and nothing else.**

### The method verdicts

This copy ships in the New Archive popup, one honest sentence each, static in v1.x — the
live estimator that measures *your* data on *your* CPU is V2.0.

| Method | The sentence |
| --- | --- |
| Store | No compression — instant, and as large as the input. |
| lz4 | The fastest real compression there is, and the largest result. |
| gzip | Fast, everywhere, and beaten in both speed and size by zstd. |
| zstd | Very fast with a small archive — the sane default. |
| bzip2 | Slower than gzip for a somewhat smaller file; kept for compatibility. |
| xz | Among the smallest archives, built slowly; extraction is quick enough. |
| 7z / LZMA2 | Smallest for mixed content, slow to build — and the only road to AES-256. |
| zip / Deflate | Not the smallest or fastest, but opens absolutely anywhere. |

---

## 6. LOOK

One theme: **Ubuntu Canonical Aubergine.** There is no second theme and no theme setting. The
window is aubergine throughout; popups are not, and that is the one deliberate exception —
contrast where a surface has to be told apart from the surface it covers, not decoration.

| Role | Value |
| --- | --- |
| Base | Six grounds in aubergine, darkest first: `#180412` the gutter every zone floats in · `#24071B` the status bar · `#300A24` the wells — the entry table and text fields · `#3D0D2E` the raised zones — sidebar, Inspector, tray · `#571342` the resting face of a control, so a button reads the same wherever it stands. Each about one and a half times the linear luminance of the one below it. A popup is not on this ladder at all — see the Popup row. |
| Popup | Not aubergine, and deliberately: a popup covers every zone, and five milestones proved that a sixth shade of the window's own colour reads as the window slightly lit rather than as a different kind of surface. Three grounds, in steel blue at the luminance the aubergine popup used to have — `#02173F` the band across the top, naming it · `#132A3D` the body · `#00133A` the band across the foot, carrying what is about to happen, and the darkest of the three so Orange has the most to push against. Scaled from `#0F52BA` / `#4682B4` / `#0047AB` by a single constant on linear RGB, which preserves the hue exactly and spends only the lightness: as picked, the body measured 1.44:1 against muted text, and no ink — not even pure white, at 4.11:1 — could have been read on it. |
| Structure | Canonical Aubergine `#772953` — selection context, the active item, and whatever the pointer is resting on; `#8F3164` the same colour with the light on, alive only for as long as a control is held down. One meaning at two intensities, never two meanings. **The entry table is the one exception**, and deliberately: it is full-height and virtualised, so a pointer crossing it would flare Aubergine on row after row where a sidebar flares once. There the pointer's place is a 7% white wash instead — the same meaning at a weight a hundred rows can carry. |
| Accent | Ubuntu Orange `#E95420` — reserved for exactly three meanings: the current selection, staged changes, and Apply/progress. Orange means *something will happen.* |
| Text | `#EEEEEC` primary · `#BDBDBB` secondary · `#999997` muted |
| Warning | `#FFD800` — and only where something has gone wrong: a wrong password, two passwords that differ, a settings file that would not parse. It is not an accent and never decorates. |
| Lines | Two weights and no third. **Inside** a zone, a 1px rule — beneath a heading, above a footer, between the archive and the lists. **Around** a zone, a popup or a control, a 2px edge at 22% white, rising to 40% white under the pointer or while a control is held. Nothing thicker than 2px, anywhere. A rule is 1px so that it does not compete with an edge, **not so that it cannot be seen**: at 8% white it measured 1.2:1 on a raised zone, which is to say it measured nothing, and two separate testing notes said so. A rule clears **1.6:1 against the ground it is drawn on**, and that floor is a test, not an intention. |
| Cursor | The keyboard's position in a list is **a line, not a colour** — the 2px edge above, at its 40% weight, drawn around the row. Orange already means the selection, and the cursor row is almost always also the selected row, so a cursor painted in Orange is Orange on Orange: it measured 2.06:1 and a testing round reported it as simply absent. A line and a wash can be read at the same time; two washes cannot. The cursor is also **kept on screen** — a row scrolled out of view is a cursor nobody can see, by a different route. |

**Controls are capsules.** A button, a chip, *Cancel* — anything you press — is drawn as a
pill: the corner radius is half the control's height, so the ends are semicircles rather than
softened corners. This is the one shape rule in the document and it exists because a control
that is merely a rounded rectangle is a rectangle, and reads as one. Rows are **not** capsules:
a list is read as a column, and a stack of pills is read as a pile of separate objects. The
radius vocabulary stays at three values and no fourth — square for a zone, 3px for a row,
half-height for a control and for a popup, which at the sizes this window uses are the same
number.

**One typeface, monospace, everywhere** — chrome and values alike; sizes, checksums,
paths, the whole Inspector. Monospace is what makes a verbose pane scannable instead of
noisy, and the pane is the program, so the window wears it throughout. Chrome and values
are told apart by weight and colour, never by family. There is no second face and no font
setting. The face is **Fira Mono Nerd Font**, regular and bold.

**Icons are glyphs of that same face**, which is what keeps the sentence above literally true
rather than nearly true. They come from the **Font Awesome** range the Nerd Font patches in, and
no second range is mixed with it: mixing icon families reads exactly like mixing typefaces. The
`Mono` cut §2 names is the reason this costs nothing — every glyph is one cell, so an icon never
widens a column or moves a number off its own. **An icon replaces a word; it never garnishes
one.** **An icon is drawn at twice the size of the text it stands beside**, and the row grows to
carry it rather than the glyph shrinking to fit — a Font Awesome glyph carries padding inside its
em box, so at 1× it puts about nine points of ink next to a thirteen-point capital and reads as
an afterthought. This is why the status bar's rows are taller than the entry table's. A folder glyph beside a path that already looks like a path is decoration, and §6 has no
room for decoration; a glyph *instead of* a label is what the sidebar and the status bar use it
for. The one deliberate redundancy is the warning glyph before a failure: colour alone carrying
meaning fails anyone who cannot separate `#FFD800` from grey, so there the shape and the colour
say the same thing on purpose.
Motion is functional only: progress moves, panels appear, and a control grows by one pixel
under the pointer and contracts below its resting size while it is held. That last is
function, not decoration: a control that does not answer the hand is a control that looks
broken. Nothing else moves.

The icon is photorealistic PNG, supplied by the maker, installed at the hicolor sizes
provided. No SVG.

**Seven things this section has now refused, with a date on them.** A design round in P13 put
each to the maker and each was declined, so they are written down rather than left to be
proposed again: **no second typeface** — the sentence above is not a default, it is a decision;
**no emoji** — they are colour bitmaps from another face and a fontconfig lookup INDIUM does not
make, so they arrive as tofu or not at all; **no translucency, blur or OS material** — the
grounds are opaque and there are six of them for a reason; **no motion beyond what this section
already permits** — progress moves, panels appear, a control answers the hand, nothing else,
and in an immediate-mode window every animation is a frame the machine cannot idle through;
**no sixth colour** — in particular no green for success, which is the outcome that needs the
least announcing; **`#FFD800` is not softened** — it measures 13.44:1 on the status bar's ground
and a failure should be the loudest thing in the window; **no third line weight** — 1px inside,
2px around, and `no_stroke_is_thicker_than_two_pixels` is the test that says so.

---

## 7. VERSIONS

Tags are two-numeral: `v0.1`, `v0.2`, … `v1.0`, `v1.1`. A tag that fixes a released
version without changing it carries the package revision instead of a third numeral —
`v1.0.0-2` — because the thing being distinguished is the build and not the version.
The release workflow derives which of the two forms it will accept from `pkgrel`, so
the rule is enforced rather than remembered.

### The road to v1.0

| P | Ships | Tag |
| --- | --- | --- |
| P1 | Window skeleton, open/list/inspect/extract, fixtures and tests, palette and fonts | `v0.1` |
| P2 | Recent files, bookmarks, Settings panel (TOML), `Ctrl+F` filter, encrypted-read prompts | `v0.2` |
| P3 | Clipboard copy-out, Open With picker, default-app registration | `v0.3` |
| P4 | Staging engine, `W` popup, Apply rebuild, New Archive popup, 7z AES-256 | `v0.4` |
| P5 | Preview tab (text, images), icon integration, look-and-feel pass | `v0.5` |
| P6 | `.pkg.tar.zst` and `.deb`, README, hardening | — *(the tag was held)* |
| P7 | The visual hierarchy: six grounds, two line weights, four control states, three-row status bar | — *(the tag was held again)* |
| P8 | The second window CORE §1 always described, and the two files it was overwriting | **`v1.0`** |
| P9 | The popup's own three grounds, and the popup that stopped outgrowing its window | **`v1.0`** |
| P10 | `Ctrl+C` and `Ctrl+V`, which had never once worked, and the gate that would not have let them ship | **`v1.0.0-2`** |

The P-table is a plan, not scripture; P-documents may split or merge steps, but scope
only moves *out* of v1.0 by the maker's decision recorded here.

### After v1.0

**V1.1** hex view; PDF joins only if a pure-Rust renderer with a GPL-3.0-compatible
licence is viable by then — INDIUM will not link poppler, will not take AGPL code, and
will not bundle a pdfium blob. **V1.2** the yazi preview plugin, built only if the
Inspector's verbosity genuinely beats a plain listing — judged then, in `contrib/`,
versioned separately. **V1.3** headless subcommands (extract, list, single-file open)
for terminal use without the GUI. **V1.4** CI: build, test, `check-deps.sh` as a gate.
**V2.0** the live estimator: sample the actual input, run the real candidates on the
real CPU, report measured time and ratio instead of folklore.

Second language: shelved for an undefined time. English is the language of v1.

---

## 8. RELEASE MECHANICS

- Packages: `.pkg.tar.zst` (Arch) and `.deb` (Debian/Ubuntu), from P6 onward. Nothing
  else — no AppImage, no Flatpak, no Snap.
- `org.indium.desktop` registers every supported MIME type — `application/zip`,
  `application/x-7z-compressed`, `application/x-tar`, `application/gzip`,
  `application/x-xz`, `application/zstd`, `application/x-bzip2`, `application/x-lzip`,
  `application/x-cpio`, `application/x-iso9660-image`, and the compressed-tar aliases —
  and deliberately **not** `application/vnd.rar`. `Exec=indium %f`. Installers create
  the launcher entry.
- Commits and releases come from the **sudo-megas** account and no other. No AI
  attribution anywhere: no trailers, no generated-by lines, nothing in files, docs, or
  release notes.
- Licence: **GPL-3.0-only**, full text in `LICENSE`, readable inside the app from About.
  Font licence (OFL-1.1) in `LICENSES/`.
- Every P-document ends with the full ritual spelled out: test, commit, push, tag, build
  `--release`, run `check-deps.sh`, package, upload. Nothing implicit.

---

## 9. DO NOT

This list is authored by the maker, not derived. Items enter and leave only by his hand.

- No drag-out — clipboard copy-out is the mechanism.
- No tray icon, ever.
- No file-manager context-menu integrations.
- No multi-theme — Ubuntu Canonical Aubergine only.
- No in-place archive writes — task list plus full rewrite only.
- No zip encryption — 7z AES-256 is the only encryption.
- **No RAR — not read, not written.**
- No external compressor binaries, ever — all format work in-process.
- No GTK, Qt, KF6, or portal linkage — enforced by `check-deps.sh`, and by CI from V1.4.
  *Linkage* is the word and it is meant: the file picker talks to `xdg-desktop-portal` over
  D-Bus, in Rust, and links nothing. `ldd` is the test, and it stays clean.
- No network at all: no update check, no telemetry, no analytics, no crash reporting.
- The app never opens a URL — About addresses are selectable, never clickable.
- No database — settings, bookmarks, and recents are TOML files.
- No system-locale detection — English default, changed only by hand if a second
  language ever ships.
- No plugin system — not open for future arrangements.
- No AppImage, Flatpak, or Snap — only `.pkg.tar.zst` and `.deb`.
- No Windows support.
- Passwords are never stored or remembered — typed per use, wiped after.
- No X11 — Wayland only.
- Allowed, for the record: a small Settings panel; a Recent Files list; running as root.
- No commits from any account other than **sudo-megas**; no AI attribution anywhere.

---

Copyright © sudo-megas

*Built with Reason and Passion.*
