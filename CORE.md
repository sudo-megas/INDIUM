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
| `image` (via `egui_extras` `image` feature) | P5 | Decodes the image formats the Preview tab shows. |
| `serde` + `toml` | P2 | Read and write the settings, bookmarks, and recent-files TOML files. |

Everything not in this table is the standard library or hand-written: CRC32 is a
twenty-line table, byte formatting is ten lines, argument handling is `std::env::args`.
`clap` arrives only if V1.3's headless subcommands justify its sentence.

### System libraries

| Library | Its sentence |
| --- | --- |
| `libarchive` | Reads and writes every supported container and filter in-process. It is a hard dependency of pacman itself, so it is present on every Arch machine that can install software; on Debian the package declares `libarchive13`. |
| `libwayland-client`, `libxkbcommon`, `libEGL`/GL | What the compositor session already provides; winit needs them to exist, and they do. |
| `glibc`, `libgcc_s`, `libm` | The floor. |

No GTK. No Qt. No KF6. No portal. `build/check-deps.sh` runs
`ldd target/release/indium` and fails if the output contains `gtk`, `Qt`, `KF6`, `X11`,
or `portal`. It runs by hand until V1.4 wires it into CI, and it runs before every release.

Bundled assets, not dependencies: JetBrains Mono NL Nerd Font, regular and bold, embedded
in the binary, with the SIL Open Font Licence 1.1 alongside the GPL in `LICENSES/`. `NL`
is the no-ligature cut, because a filename holding `->` must render as the two characters
the archive stores.

---

## 3. ARCHITECTURE

One binary crate, `indium`, with modules. No workspace, no premature abstraction.

| Module | Owns |
| --- | --- |
| `arch` | Hand-written FFI over system libarchive (~15 functions) and the safe wrapper around it. Listing streams entries over a channel from a worker thread; extraction runs with libarchive's secure flags (`SECURE_SYMLINKS`, `SECURE_NODOTDOT`) so a hostile archive cannot write outside its target. |
| `model` | Archive state: entries, selection, the open archive's identity. |
| `tasks` | The staging engine. Every mutation — add, remove, rename, create — is a task in a queue. **Apply** builds the new archive in a temp file beside the target, verifies it by walking its entries, then atomically renames over the original. The original is never touched until the replacement is proven. |
| `ui` | The window: sidebar, table, Inspector, tray, status bar, and every popup. |
| `platform` | The Linux specifics: clipboard, `.desktop` parsing for Open With, default-app registration, XDG paths. |
| `theme` | The Aubergine palette, the fonts, and nothing configurable. |

Threading: the UI thread and one worker. The worker opens, lists, extracts, and rebuilds;
it reports progress over a channel and honours a cancellation flag. egui runs in reactive
mode — an idle INDIUM repaints nothing and costs nothing, on any refresh rate.

Passwords are requested at the moment of use, passed down, and zeroed when the operation
ends. They are never written to settings, recents, or anywhere else.

---

## 4. THE WINDOW

Five fixed zones and seven popups. Nothing else appears, ever.

**Sidebar** (family style): the wordmark at top, then *Recent files* `1`, *Bookmarks* `2`,
*Archive* `3`; at the bottom *New* `N`, *Settings* `,`, *About* `A`. Numbers and letters
are bare keypresses, as in JADEITE.

**Entry table**: virtualized; columns Name, Size, Packed, Method; a breadcrumb path above
it. `Enter` descends into a directory, `Backspace` goes up. `Ctrl+F` opens a filter bar —
there is deliberately no type-to-jump, because bare letters are shortcuts.

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

**Status bar**: entry count, real → packed with ratio, format badges, and the progress bar
with its cancel during long work.

### The popups

1. **New Archive** (`N`). A subwindow, Clonezilla in content and Aubergine in dress. An
   instruction line at top ("Choose how INDIUM should compress. If unsure, keep the
   default."). Four preset chips — *Fastest*, *Balanced* (default), *Smallest*,
   *Encrypted* — each highlighting a row in the method list below, where **every method
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

### Keyboard

| Key | Does |
| --- | --- |
| `1` `2` `3` | Sidebar sections |
| `N` `W` `E` `A` `,` | New Archive · Pending tasks · Extract · About · Settings |
| Arrows, `PgUp/PgDn`, `Home/End` | Move in the table |
| `Enter` / `Backspace` | Descend / go up |
| `Space` | Details ⇄ Preview |
| `Ctrl+F` | Filter bar |
| `Ctrl+A` | Select all |
| `Ctrl+C` | Copy out (extract to runtime dir, URIs to clipboard) |
| `Ctrl+V` / drop files | Stage an add |
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

**Write** — `tar` with the filters `gz`, `bz2`, `xz`, `zst`, `lz4`; `zip` (Deflate);
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

One theme: **Ubuntu Canonical Aubergine.** There is no second theme and no theme setting.

| Role | Value |
| --- | --- |
| Base | `#221226` window · `#2B1830` raised panels · `#1C0F20` status bar |
| Structure | Canonical Aubergine `#772953` — selection context, active sidebar item |
| Accent | Ubuntu Orange `#E95420` — reserved for exactly three meanings: the current selection, staged changes, and Apply/progress. Orange means *something will happen.* |
| Text | `#F0E6EE` primary · `#C9B3C4` secondary · `#A98BA3` muted |
| Lines | 1px hairlines at 8% white. Nothing thicker, anywhere. |

**One typeface, monospace, everywhere** — chrome and values alike; sizes, checksums,
paths, the whole Inspector. Monospace is what makes a verbose pane scannable instead of
noisy, and the pane is the program, so the window wears it throughout. Chrome and values
are told apart by weight and colour, never by family. There is no second face and no font
setting.
Motion is functional only: progress moves, panels appear; nothing decorates.

The icon is photorealistic PNG, supplied by the maker, installed at the hicolor sizes
provided. No SVG.

---

## 7. VERSIONS

Tags are two-numeral: `v0.1`, `v0.2`, … `v1.0`, `v1.1`.

### The road to v1.0

| P | Ships | Tag |
| --- | --- | --- |
| P1 | Window skeleton, open/list/inspect/extract, fixtures and tests, palette and fonts | `v0.1` |
| P2 | Recent files, bookmarks, Settings panel (TOML), `Ctrl+F` filter, encrypted-read prompts | `v0.2` |
| P3 | Clipboard copy-out, Open With picker, default-app registration | `v0.3` |
| P4 | Staging engine, `W` popup, Apply rebuild, New Archive popup, 7z AES-256 | `v0.4` |
| P5 | Preview tab (text, images), icon integration, look-and-feel pass | `v0.5` |
| P6 | `.pkg.tar.zst` and `.deb`, README, hardening | **`v1.0`** |

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
