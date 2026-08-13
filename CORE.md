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

One archive per window, and no tabs. A window holds one archive at a time; a *file manager*
asking INDIUM to open a second gets a second window, because that is what the desktop means
by opening a file. From inside the program the rule runs the other way: you close the file
you have, and the next archive you name takes this window. Until P22 there was no way to
leave an archive at all — the second half of this rule had to be written before it could
be true.

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
twenty-line table, byte formatting is ten lines, argument handling is **`std::env::args_os`**
— `args` panics outright on an argument that is not valid Unicode, and a path on Linux is
bytes, which this program argues at length about libarchive and then ignored in its own
`main` from P1 until P17 found it.

**`clap` was put to the test this section named, and refused — P17.** The condition was
that the headless subcommands justify its sentence — §7 numbered them V1.3 when that
condition was written and V1.2 when it was met — and they do not: the terminal half
is three subcommands, one string option and two flags, which is forty lines of `match`
beside the thirty `main` already hand-rolls. Its sentence would read *"parses six flags"*,
and that is not a sentence — while the derive path brings `syn`, `quote`, `proc-macro2`
and a colour-negotiation stack to a program that admits one palette and no theme setting.
The refusal carries a date for the same reason §6's dated refusals do: so it is not
reproposed as a discovery. The count is deliberately not repeated here — a number in one
section describing a list in another is a sentence that goes stale the next time the list
grows, and this one did.

### System libraries

| Library | Its sentence |
| --- | --- |
| `libarchive` | Reads and writes every supported container and filter in-process. It is a hard dependency of pacman itself, so it is present on every Arch machine that can install software; on Debian the package declares `libarchive13t64 \| libarchive13`, naming both because the time64 transition renamed it in trixie and a package that spans both suites has to say so. |
| `libwayland-client`, `libwayland-egl`, `libxkbcommon`, `libEGL`/GL | What the compositor session already provides; winit needs them to exist, and they do. **All four are `dlopen`ed by soname**, so none appears in `ldd` output and no shlibs machinery can find them — a package names them by hand or ships a program that fails at its first window. This row is the list every package is written from, and `libwayland-egl` was missing from it until P19. |
| `glibc`, `libgcc_s`, `libm` | The floor. |

No GTK. No Qt. No KF6. No portal. `build/check-deps.sh` runs
`ldd target/release/indium` and fails if the output contains `gtk`, `Qt`, `KF6`, `X11`,
or `portal`. It runs on every push in `ci.yml`, inside the Arch package's own `check()` so
no package can be built from a binary that grew a toolkit, and in the release workflow —
and still by hand before a release, which is the one of the four that proves nothing on its
own. §7 lists this as V1.4's gate and records that it was brought forward to P15; this
sentence went on saying *"until V1.4 wires it into CI"* for three releases after it had.

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
| `sevenz` | The 7z half, over `sevenz-rust2`: AES-256 writing, which libarchive cannot do, and the detail the generic reader does not expose — solid blocks, the per-entry method, and headers that are themselves encrypted. It sits beside `arch` rather than inside it because `arch`'s own first sentence is hand-written FFI over the system libarchive, and a crate-backed backend does not belong inside that sentence. |
| `model` | Archive state: entries, selection, the open archive's identity. |
| `tasks` | The staging engine, and the draft that feeds it. Every mutation — add, remove, rename, create — is a task in a queue; **Apply** builds the new archive in a temp file beside the target, verifies it by walking its entries, then atomically renames over the original, and the original is never touched until the replacement is proven. The **draft** is the other half, and deliberately not a second queue: a plain list of what the next archive will be made of, holding no mutation of anything, folding nothing, and becoming tasks only when *Create* is pressed. A queue describes changes to one archive and folds toward it; a draft names a thing that does not exist yet. The draft is the source of truth until Apply succeeds, and the queue's creation lane is a projection of it, recomputed every time *Create* is pressed. |
| `estimate` | Measures instead of asserting. It runs the real writers over the real input on the real CPU, so §5's eight sentences stand beside a time and a ratio from *this* machine rather than from folklore. It owns no format knowledge of its own — it drives the same `Sink` Apply drives, into a scratch file it deletes — and it is the one module in the program allowed to be wrong out loud: at or under its budget it measures the whole input and the figure is exact, above it the input is sampled and every figure says so, because chopping a stream costs LZMA the long-range matches its dictionary lives on and no amount of arithmetic gets them back. |
| `ui` | The window: sidebar, table, Inspector, tray, status bar, and every popup. |
| `platform` | The Linux specifics: clipboard, `.desktop` parsing for Open With, default-app registration, XDG paths, the second window — on this platform a window is a process, and opening one is a Linux specific like the rest — and handing a directory to the desktop's file manager, which is the portal's job for the same reason the picker is. |
| `cli` | The terminal half — `list`, `extract`, `cat`, their arguments, their output and their exit codes. It reads the archive through `arch` exactly as the window does, and it opens no window: the dispatch returns before `main` asks `eframe` for anything, so a subcommand on a machine with no compositor is an ordinary program reading a file. It touches nothing in `ui`, deliberately, because that is the way the previous sentence stops being true by accident. |
| `theme` | The Aubergine palette, the fonts, and nothing configurable. |
| `secret` | The password buffer: the only thing in the program that *carries* one, because every path that takes a password takes a `Secret`. It overwrites its bytes on drop through `write_volatile` behind a compiler fence, so the optimiser cannot decide the wipe is dead; it refuses to print itself; and the two copies it cannot follow are written down rather than papered over — the NUL-terminated string libarchive keeps for the reader's lifetime, and the plain `String` the prompt's text field must type into before the value can become a `Secret` at all, which is cleared on submit and on cancel but not overwritten. |
| `util` | The hand-written helpers §2 refuses a crate for: the CRC32 table and its streaming form, byte and ratio formatting, the hex view's row arithmetic, the mode string, a civil date from a unix timestamp, the path normalisation every stored name passes through before it is shown, the middle elision §4 requires of a path, and the byte sniffer the Inspector decides content with — which is the classifier §6 means above when it refuses a glyph that would decide by extension instead. Nothing here may grow a dependency. |

Threading: the UI thread and one worker. The worker opens, lists, extracts, and rebuilds;
it reports progress over a channel and honours a cancellation flag. egui runs in reactive
mode — an idle INDIUM repaints nothing and costs nothing, on any refresh rate.

Passwords are requested at the moment of use, passed down, and zeroed when the operation
ends. They are never written to settings, recents, or anywhere else.

---

## 4. THE WINDOW

Five fixed zones and ten popups. Nothing else appears, ever.

**Sidebar** (family style): the wordmark at top, then *File* `1`, *Draft* `2` and *Create*
`N`; a rule; then *Open file* `O`, *Recent files* `3` and *Bookmarks* `4`; at the bottom
*Settings* `,` and *About* `A`. Numbers and letters are bare keypresses, as in JADEITE. Every
row carries a leading glyph in the same ink as its label, so the column can be found by shape
before it is read; §6 says which glyphs and what they are allowed to do.

The three groups are three questions, in the order a person asks them. Above the rule is the
archive you are in or making — the file, the draft it will be built from, and the control that
builds it, which are one piece of work and sit together. Below the rule is how you reach
another one — the picker, the history, the shelf. At the bottom is the program itself, which
is about no archive at all. Two rows moved in P22 to make that true: *Create* came up from the
bottom, because it is the last step of the work above the rule and not a utility beside
Settings, and *Open file* went down, because it is a way in rather than something you are
holding. The rule is a rule and not a gap; §6 fixes what it has to be to be seen.

**Entry table**: virtualized; columns Name, Size, Packed, Method; a breadcrumb path above
it, with *Close* and then *Add files…* at the far end of that row — the picker adds into the
directory the breadcrumb names, which is the one placement that needs no explanation, and
*Close* is beside it because leaving the file is an act on the file and belongs where the
file is. *Add files…* keeps the outer edge it has held since P11: the control pressed often
does not move so that the control that throws work away can have the place the hand knows. `Enter` descends into a directory, `Backspace` goes up. `Ctrl+F` opens a filter bar
— there is deliberately no type-to-jump, because bare letters are shortcuts. With nothing
open the table says so and offers the way in: it is a zone that is always there, not one
that appears with an archive.

**The Draft** is what this same zone shows when the second section is chosen — one row per
item with its own remove ✕, and two controls: *Add files…*, which is the same picker, and
*Bring from archive*, which pulls the entries selected in the open archive across into the
draft. It is not a sixth zone; the entry table is the zone, and the draft is one of the
things it shows, as the two lists already are. What *Bring from archive* pulls are **copies**:
an entry inside an archive is not a file until something makes it one, so the draft holds
files from that moment and the archive they came from can be closed without touching them.
Nothing in the draft is a mutation of anything and nothing in it is written; it becomes tasks
when *Create* is pressed, and not before. A *staged* creation survives a close for the same
reason the draft does — it names an archive the closed file was never the subject of — so what
closing discards is changes against the file being closed, and it says how many went.

**Inspector** (right, permanent): two tabs, *Details* and *Preview*, toggled with `Space`.
Details shows everything the reader can know about the selection; multi-select shows
aggregates; no selection shows the archive-level card. Preview renders text, images, and — for
anything that is neither — **hex**, sixteen bytes to a row with the printable gutter beside them,
which arrived at V1.1 as this line promised it would. **Sixteen is fixed** — the bytes on a row
are not fitted to the pane — so the same byte sits at the same offset however the Inspector is
dragged, and the pane scrolls sideways instead. (How many *rows* there are is the file's business
and varies with its length; it is the column count that is nailed down.) Two honest sourcing notes, so nobody files a bug about them:
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

1. **Create** (`N`). A subwindow, Clonezilla in content, and wearing the popup's own
   three grounds rather than the window's. **It is the last step and not the first**: it is
   reached with a draft already full, so every figure it can show is a figure about real
   bytes. Until P22 it was the only door into the state where files could exist, which put
   the recipe before the input it describes, and made a person press `N` twice to build one
   archive. An instruction line at top ("Choose how INDIUM
   should compress. If unsure, keep the default."). Four preset chips — *Fastest*,
   *Balanced* (default), *Smallest*, *Encrypted* — each highlighting a row in the method
   list below, where **every method
   carries its one-sentence verdict** (§5) and nothing else. Beside the heading, **Measure**
   — V2.0's estimator — opens the Measure popup (§4.10) and runs the eight real candidates
   over the real input; the figures are drawn there, on a surface with room for them, and
   not in a lane on a row that is already carrying a sentence. **Measure is live from the
   first frame**, because the draft is full before this popup opens; that is what moving it
   last was for. An *Advanced*
   disclosure holds the level slider. The popup's own button is dead, with a sentence, while
   the draft is empty. At the foot, a live sentence states exactly what will
   be built: *"Building photos-2026.7z — 7z, LZMA2:9, AES-256."*
2. **Pending tasks** (`W`, or clicking the tray). The full task list: one row per staged
   operation with its own remove ✕, then *Discard all* and **Apply**.
3. **Extract** (`E`). A popover: *Extract here*, *Extract to `<name>/`*, a path field with
   tab completion, bookmarks beneath. Enter-driven.
4. **Open With** (`Enter` on a file, after extraction to the runtime dir). Applications
   from parsed `.desktop` files, ranked by MIME match, filter-as-you-type.
5. **Settings** (`,`). One small panel, and **exactly three groups**: *Extract*'s default
   destination, *Bookmarks*, and *Recent files* — the last carrying a **Clear list** that
   empties the history, which is the only destructive control in the panel and the reason
   the count is written here rather than left to be discovered. TOML behind it. This item
   named two of the three from P2 until P19; §9 has already decided the absent ones.
6. **About** (`A`). The mark, the maker, the version and date, the source address and the
   licence in full. Addresses are text you can select but not click — INDIUM opens no
   browser and follows no link, by design.
7. **Password** (modal). Appears at the moment of use, per use, and nowhere survives it.
8. **Open** (`Ctrl+O`). A path field with tab completion, a *Browse…* button beside it
   raising the desktop's own picker through `xdg-desktop-portal`, and the only popup that
   is not about the archive already open. Naming an archive this window does not hold
   closes the one it holds and takes the new one here, per §1 — the same close the *Close*
   control performs, and it says what it discarded in the same words.
9. **Keys** (`F1`). The table below, drawn in the window. It exists because a person who had
   used the program for an afternoon wrote *"I didn't know `Ctrl+O` opens a file, and still
   don't know how to exit from the archive"* — a program whose whole interface is bare
   keypresses owes the reader the list. It is **generated from the bindings, never typed
   twice**: a keys popup that has drifted from the keys is worse than no keys popup.
10. **Measure**. Opened by *Measure* on the Create popup, and the only popup that is
    drawn **over** another — which is what makes `Esc`'s *"close the topmost popup"* below a
    description rather than an aspiration. It holds nothing but the measurements: one row per
    method, carrying the level it was built at, the time it took, the size it produced and the
    ratio that follows. All eight rows stand from the first frame and their cells fill in as
    the candidates land, because a table that grows is a table that moves. The figures are told
    in text and never in colour, a `~` marks a ratio the sample could not promise, and the
    popup always states what it weighed. It runs when it opens and **keeps its figures for as
    long as Create lives** — *Measure again* is how they are spent a second time, and
    closing Create discards them, because a figure that outlives its input is the folklore
    V2.0 was sent to replace. **Clicking a row chooses that method**: the measuring was to
    decide, so the answer is the control. It has no key of its own; the popup it stands on is
    the way in.

    It has this popup because it did not, and that was wrong: V2.0 first wrote the figures
    into a lane on each method row, at the smallest and dimmest type in the window, beside a
    verdict that had already taken the width. The round's whole payload landed in the least
    readable element on screen. A row cannot hold five columns and a sentence.

### Keyboard

| Key | Does |
| --- | --- |
| `1` `2` `3` `4` | Sidebar sections |
| `O` / `I` | Open file · Add files — both raise the desktop's own picker. `I` adds to whichever section is showing: the draft, or the directory the breadcrumb names. |
| `N` `W` `E` `A` `,` | Create · Pending tasks · Extract · About · Settings |
| `F1` | Keys — this table, in the window |
| Arrows, `PgUp/PgDn`, `Home/End` | Move in the table |
| `Enter` / `Backspace` | Descend / go up |
| `Space` | Details ⇄ Preview |
| `Ctrl+F` | Filter bar |
| `Ctrl+A` | Select all |
| `Ctrl+C` | Copy out (extract to runtime dir, URIs to clipboard) |
| `Ctrl+V` / drop files | Stage an add. A drop needs X11: winit has no Wayland drag-and-drop, so on Wayland the compositor never delivers one. `Ctrl+V` and *Add files…* are the routes that always work. Both land where `I` does, in the section that is showing. |
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

This copy ships in the Create popup, one honest sentence each. **They no longer stand
alone: since V2.0 the estimator will give any one of them a measured level, time, size and
ratio, one Measure away in §4.10's popup** — this data, this CPU, this moment. The sentences
stay because they say what a method is *for*, which a number cannot, and because a figure is
only ever about the input that was measured. Where that input was too large to weigh whole the figure is marked `~`
and is an estimate in the strict sense: a stratified sample predicts throughput well and
gzip and zstd's ratios closely, but it cannot predict LZMA's, whose dictionary earns its
ratio on exactly the long-range matches that chopping a stream destroys. The program says
which kind of figure it is showing rather than hoping nobody asks.

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
| Base | Five grounds in aubergine, darkest first: `#180412` the gutter every zone floats in · `#24071B` the status bar · `#300A24` the wells — the entry table and text fields · `#3D0D2E` the raised zones — sidebar, Inspector, tray · `#571342` the resting face of a control, so a button reads the same wherever it stands. Each about one and a half times the linear luminance of the one below it. A popup is not on this ladder at all — see the Popup row. **This row read *six* until P18 counted it.** It was six when P7 built it, and P9 took the popup off the ladder and out of aubergine without taking it out of the number — so the sentence has named a ground it then removes two clauses later, for nine milestones. The five are pinned to the palette by `the_ground_ladder_is_the_one_core_six_lists`, which is the only reason the next edit cannot do it again. |
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
one.** **An icon is drawn at 1.4× the size of the text it stands beside**, and the row
grows to carry it rather than the glyph shrinking to fit; a Font Awesome glyph carries padding
inside its em box, so at 1× it puts about nine points of ink next to a thirteen-point capital and
reads as an afterthought. This is why the status bar's rows are taller than the entry table's.
This line read *twice* until P16 corrected it: the code has said 1.4 since P13, which built 2×
for an afternoon and found that at double the text the glyph sets every row's height — it added
thirty-six points to the status bar and about a hundred and twenty to the sidebar, and a window
that had fitted everything for twelve milestones stopped fitting anything. A folder glyph beside a path that already looks like a path is decoration, and §6 has no
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

**Per-file-type icons in the entry table are refused — P18.** Nominated by P13, repeated by
P15, and left *"still nominated and still undecided"* by both P16 and P17: the most-repeated
open item in the whole record, and the reason it never moved is that it was never a build
task. This section already answers it. *An icon replaces a word; it never garnishes one* —
and a type glyph beside `photo.png` is a glyph beside a name that already says `png`, which
is very nearly the example this section gives against itself two sentences later. The entry
table has said the same thing in code since P1 and re-affirmed it after the face gained the
glyph: a directory is marked by a trailing slash, *"the face carries one now, and the answer
is still the slash."* A row would end up carrying four directory signals — slash, bold face,
ink, glyph.

Three costs were measured before ruling, so this is a decision and not a preference. The
glyphs exist: the whole Font Awesome file-type family ships in both faces already, so
availability was never the constraint. The row is: at `ICON_SCALE` a 13pt glyph does not fit
a 20px row — P13 measured exactly this when the status bar grew from 20 to 24 to carry one —
and the entry table is full-height and virtualised, so the same growth costs about a sixth of
every screenful. And the Name column would pay for it, being the only `remainder()` beside
three `exact` columns, with no slack anywhere. Last, the only affordable classifier decides by
**extension**, in a program whose Inspector deliberately decides by **bytes**, on the stated
ground that a PNG named `notes.txt` is a PNG. Putting a name-trusting glyph in the main table
of *this* program is the contradiction that settles it.

**Eight things this section has now refused, with a date on them.** The eighth is the
entry-table icon ruled on immediately above, put to the maker at P18 after four rounds of
standing nominated. The other seven a design round in P13 put to him, and each was declined,
so they are written down rather than left to be proposed again: **no second typeface** — the sentence above is not a default, it is a decision;
**no emoji** — they are colour bitmaps from another face and a fontconfig lookup INDIUM does not
make, so they arrive as tofu or not at all; **no translucency, blur or OS material** — the
grounds are opaque, the ladder's five and the popup's own three alike, and each is a flat
colour for a reason; **no motion beyond what this section
already permits** — progress moves, panels appear, a control answers the hand, nothing else,
and in an immediate-mode window every animation is a frame the machine cannot idle through;
**no sixth colour** — in particular no green for success, which is the outcome that needs the
least announcing; **`#FFD800` is not softened** — it measures 13.44:1 on the status bar's ground
and a failure should be the loudest thing in the window; **no third line weight** — 1px inside,
2px around, and `no_stroke_is_thicker_than_two_pixels` is the test that says so.

---

## 7. VERSIONS

Tags are two-numeral — a major and a minor, and no third: `v1.0`, `v1.1`, `v1.2`. A tag
that fixes a released version without changing it carries the package revision instead of
that third numeral — `v1.0.0-2` — because the thing being distinguished is the build and
not the version. The release workflow derives which of the two forms it will accept from
`pkgrel`, so the rule is enforced rather than remembered.

**The sequence begins at `v0.2`, and the hole below it is real.** `P1.md` asked for a
`v0.1` in its done-when block and it was never cut — not here, not on origin — so P1
shipped without one and the box has stayed open ever since. It is not the only open box in
the ledger, but it is the only one that cannot be closed by doing the work, for the reason
below. This rule used to open by naming `v0.1` among its examples, and the road table
carried it in P1's Tag column as though it had been cut; P18 stopped both. Two P-documents
still speak of it as a release point that happened — `P6.md:368` and `P7.md:521` — and they
are left standing, because a P-document records what its round believed and is annotated,
never rewritten. The tag is **not** cut retroactively: P17's argument against a hole in the
sequence was about a slot a future thing would never fill, and a tag dated two years after
the work, with no artefacts to attach to it, would be a claim about the past rather than a
repair to it.

### The road to v1.0

| P | Ships | Tag |
| --- | --- | --- |
| P1 | Window skeleton, open/list/inspect/extract, fixtures and tests, palette and fonts | — *(asked for, never cut)* |
| P2 | Recent files, bookmarks, Settings panel (TOML), `Ctrl+F` filter, encrypted-read prompts | `v0.2` |
| P3 | Clipboard copy-out, Open With picker, default-app registration | `v0.3` |
| P4 | Staging engine, `W` popup, Apply rebuild, New Archive popup, 7z AES-256 | `v0.4` |
| P5 | Preview tab (text, images), icon integration, look-and-feel pass | `v0.5` |
| P6 | `.pkg.tar.zst` and `.deb`, README, hardening | — *(the tag was held)* |
| P7 | The visual hierarchy: six grounds, two line weights, four control states, three-row status bar | — *(the tag was held again)* |
| P8 | The second window CORE §1 always described, and the two files it was overwriting | **`v1.0`** |
| P9 | The popup's own three grounds, and the popup that stopped outgrowing its window | **`v1.0`** |
| P10 | `Ctrl+C` and `Ctrl+V`, which had never once worked, and the gate that would not have let them ship | **`v1.0.0-2`** |
| P11 | The round that was actually run: a locale that can carry a name, the portal's file picker, and four defects a person found | **`v1.0.0-3`** |
| P12 | The design round the testing asked for: the sidebar's order, `F1`, a cursor that is a line, capsules, rules that can be seen, Fira Mono | **`v1.0.0-4`** |
| P13 | The design round decided: icons from the face already embedded, a path elided in the middle that opens its folder, progress on the bar's edge | **`v1.0.0-4`** |
| P14 | The sidebar's scrollbar threshold, and what a fractional display scale does to a window | **`v1.0.0-4`** |
| P15 | The audit's open ledger: five defects closed, a false line in the record straightened, and V1.4's gate brought forward | **`v1.0.0-5`** |
| P16 | The hex view, and the version number that goes with a feature rather than a fix | **`v1.1`** |
| P17 | The terminal half, and the copyright the `.deb` had been getting wrong since P12 | **`v1.2`** |
| P18 | The record round: the claims nothing could check, and the gates that now read them | **`v1.2.0-2`** |
| P19 | The front page read against the tree: a library three of four packages never declared, a panel's third group, and a password refused as the wrong one | **`v1.2.0-3`** |
| P20 | The yazi plugin, in a repository of its own because `contrib/` could never have been installed, and the description settled in all four places that state one | **`v1.2.0-4`** |
| P21 | V2.0, the live estimator: eight real candidates on the real CPU, exact under its budget and marked above it, the popup made re-openable so there is something to measure, and — once it had been seen on screen — a tenth popup to draw the figures in | **`v2.0`** |

The P-table is a plan, not scripture; P-documents may split or merge steps, but scope
only moves *out* of v1.0 by the maker's decision recorded here.

**The `1.0` line is a beta, and this is the condition for it stopping.** Every release
from `v1.0.0-4` onward has said so in its own notes, in these words: *"the `1.0` line stays
one until the design work it is named for has been in real hands."* `v1.0.0-3` announced
the beta before it, in different words, and every release since has carried it unchanged.
How many bodies that is stays out of this paragraph, for the reason §2 gives about counts:
the list grows at every release, so a number written here goes stale the first time the
rule it describes is obeyed — including by the release that shipped the paragraph. `P15.md`
quotes it and P16 and P17 paraphrase it in their closing steps, but until P18 wrote it here
it appeared in **no governing document** — not CORE, not the README — so the project's own
next state change was a condition a reader had to reconstruct from release prose and the
rounds that happened to repeat it. The design work it names is P12's and P13's, shipped at `v1.0.0-4`; the gate is a
testing round against a released build carrying it, and no such round has been run. **What
"real hands" means is deliberately left undefined**: it is a decision the maker has not
made, and recording the sentence is not the place to make it for him.

### After v1.0

**V1.1** hex view — **shipped in P16**, and the line stays here so the road reads as what
happened. **PDF was not taken up with it**: the condition was a pure-Rust renderer with a
GPL-3.0-compatible licence, none was viable, and the three refusals stand unchanged — INDIUM
will not link poppler, will not take AGPL code, and will not bundle a pdfium blob. It joins
the yazi plugin as a thing judged when it is reached rather than promised in advance —
named rather than numbered, because the numbers moved and a cross-reference by number
would not have survived it.

**V1.2** headless subcommands (extract, list, single-file open) for terminal use without
the GUI — **shipped in P17**, and the line stays here for the same reason V1.1's does.
**V1.3** the yazi preview plugin — **shipped, and not in `contrib/`, which is what this line
used to say.** The gate was measured rather than asserted, both halves written down, because a
gate that cannot return "no" is not one: yazi 26.5.6 ships a built-in archive previewer showing
a name, a size, an icon and a tree depth, and it gets them by running `7zz` or `7z` — the thing
§9 forbids this program from ever doing — so on a machine with no 7-zip installed it previews
nothing at all. `indium list --long` states seven fields and a total, and runs nothing. The
condition is met.

It lives at [`sudo-megas/indium.yazi`](https://github.com/sudo-megas/indium.yazi), a repository
of its own, because `contrib/` was never installable: `ya pkg` clones a repository's *default
branch* and expects the plugin at its **root**, and its `rev` takes a commit hash rather than a
branch or a tag. A directory in this tree could not have been installed, and neither could a
branch of it. So *versioned separately* is now literally true — its own history, its own tags,
and **INDIUM cuts no tag for it**, exactly as the paragraph below says it never could.

It is a plugin for yazi, written against yazi's previewer API, and it reaches INDIUM the way
anything reaches INDIUM: by running `indium list`, over the CLI §7 shipped at V1.2. Nothing here
is added to this program.

**Those two numbers were the other way round until P17, and the swap is recorded rather
than quietly made.** The maker's call was that the tag sequence stays contiguous: the
plugin is *versioned separately* and so can never take an INDIUM tag, and leaving a hole
at `v1.2` for something that will never fill it is worse than renumbering. The cost is
named honestly — **eight published releases print *"arrive in V1.3"*** in their `--help`,
for a thing that arrived at `v1.2`. Those tarballs cannot be edited, and a reader who
finds that sentence in one of them is owed this paragraph rather than a puzzle.

**V1.4** CI: build, test, `check-deps.sh` as a gate —
**brought forward and shipped in P15**, by the maker's decision, because a gate that fires
only on a tag is not a gate; it stays listed here so the road reads as what happened.
**V2.0** the live estimator: sample the actual input, run the real candidates on the
real CPU, report measured time and ratio instead of folklore — **shipped in P21**, as
`estimate` (§3), the Measure action in §4.1's first popup and the tenth popup it opens
(§4.10). It came out narrower than
the line above reads in one respect and wider in another. Narrower: the candidates run in
sequence, because §3 fixes threading at the UI thread and one worker, so eight of them cost
around three and a half seconds at the budget — xz and LZMA2 are five sixths of it — and
Measure is therefore a button rather than something the popup does on opening. Wider: at or under its budget it does not sample at all — it compresses the whole
input through the same writers Apply uses, so the figure is not an estimate but the size
Apply would produce. Only above the budget is anything sampled, and then it says so. §5
records what that mark means and why LZMA is the method it means it most about.

And wider again, after it was seen on screen: the figures needed a **popup of their own**.
V2.0 first wrote them into a lane on each method row, which is where they fit and not where
they could be read — 11 px of the dimmest ink in the window, beside a sentence already
holding the width. The maker lifted the popup cap for it in the same breath, so §4 counts ten
where it counted nine, and `Esc`'s "close the topmost popup" describes something the program
actually does.

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
- Every P-document ends with the full ritual spelled out: **bump the version or `pkgrel`,
  write the `changelog.Debian` stanza and move `RELEASE_DATE` to its date**, test, commit,
  push, tag, build `--release`, run `check-deps.sh`, package, upload. Nothing implicit.
  **Since P21 the last of those is not done by hand.** A release is drafted from the
  **sudo-megas** account — the title, the notes, the decision that this is a release — and
  the tag's own workflow attaches the three artefacts to that draft and publishes it. A
  release's author is fixed when it is created, so the draft is what keeps the rule above
  literally true: `release.yml` runs on `GITHUB_TOKEN`, and anything it *created* would be
  authored by `github-actions[bot]`. The job therefore **refuses to create one** — no
  draft, no release, and it says so — which makes the human half impossible to skip rather
  than merely expected. This finishes an argument the workflow had already half made: no
  byte a release ships is produced on a personal machine, and now none is uploaded from one
  either.
  **The two that now open the list were added at P19, and they are not paperwork.**
  `release.yml` derives which tag form it will accept from `pkgrel` — two-numeral at 1, a
  revision above it — so that number decides the tag's name rather than following it. And
  since P18 a test reads About's date back out of the changelog's top stanza, which makes
  writing the stanza a *build* prerequisite: a tag that moves the version without one fails
  `cargo test` rather than shipping a window stating the wrong date, as three consecutive
  tags did. §8 listed neither for eighteen rounds, because until that test existed neither
  was load-bearing enough to be forgotten expensively.

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
- No GTK, Qt, KF6, or portal linkage — enforced by `check-deps.sh`, and by CI on every push
  since P15.
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
- No signal handlers — not one, anywhere; refused at P18 rather than left undecided. *The
  cost is accepted and named*: `Ctrl-C` at the terminal password prompt skips the
  restore-on-drop guard and leaves the echo off until `stty sane`.
- No X11 — Wayland only.
- Allowed, for the record: a small Settings panel; a Recent Files list; running as root.
- No commits from any account other than **sudo-megas**; no AI attribution anywhere.

---

Copyright © sudo-megas

*Built with Reason and Passion.*
