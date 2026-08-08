# INDIUM

An archive manager for Linux on Wayland where **the metadata is the main event.**

The stated ambition of this program is to be one of the most verbose archiver applications
in the industry. Every other archiver on this platform treats an archive's contents as a
name column and hides the rest behind a Properties dialog.

## What it is

INDIUM is written in Rust and drawn with egui. The Inspector is a permanent pane on the
right of the window, not a dialog you go and find: sizes, packed sizes, ratio, method,
checksums, four timestamps, ownership, mode, link targets, encryption state. Selecting
nothing shows the archive-level card; selecting several entries shows aggregates. It has
two tabs, *Details* and *Preview*, and `Space` moves between them.

Two sourcing notes belong here rather than in a bug report. libarchive does not expose an
entry's *stored* CRC, so INDIUM computes CRC32 on demand and labels it computed. Preview
renders text and images only — a file is judged by its bytes and never by its extension,
and anything that is neither gets a plain sentence rather than a half-built hex view, which
arrives in V1.1.

All format work happens inside the process. INDIUM never runs `7z`, `tar`, `unzip`, `zstd`,
or any other external compressor. When a format is listed as supported below, it is
supported by code linked into the binary, on every machine, whether or not any archive tool
is installed.

One archive per window. Opening a second archive opens a second window. There are no tabs,
and the title bar names the archive so a row of INDIUM windows can be told apart.

Nothing is written in place. Every mutation — add, remove, rename, create — is a task in a
queue, and nothing reaches the disk until **Apply**. Apply builds the new archive in a temp
file beside the target, verifies it by walking its entries, and then atomically renames it
over the original. The original is never touched until the replacement is proven.

Wayland only. There is no X11 path and there will not be one.

## Install

Two packages, and nothing else.

### Arch

```sh
sudo pacman -U indium-1.0.0-1-x86_64.pkg.tar.zst
```

### Debian / Ubuntu

```sh
sudo apt install ./indium_1.0.0-1_amd64.deb
```

The three artefacts do not have the same floor, and the difference matters more than the
file extension does. The `.deb` is built on Debian bookworm and needs **glibc 2.36 or
newer**. The `.pkg.tar.zst` and the plain `indium-1.0-x86_64.tar.gz` are both built on
Arch, and the binary in them requires **glibc 2.43** — measured from the binary's own
symbol versions, not guessed. So the tarball is not a way to run INDIUM on Debian. It is
the Arch binary without the package around it, and on bookworm it will refuse to start.

No AppImage, no Flatpak, no Snap. That is CORE §9, and it is not open for discussion.

## Build from source

### Arch

```sh
sudo pacman -S libarchive rust
```

### Debian / Ubuntu

```sh
sudo apt install libarchive-dev pkg-config build-essential
```

Rust comes from rustup — <https://rustup.rs> — on Debian and Ubuntu rather than from
`apt`. Bookworm packages `rustc` 1.63, which cannot build `eframe` 0.36.

Then, on either:

```sh
cargo build --release
```

The binary lands at `target/release/indium` and runs from there. To register it on a
development machine — the icon sizes, the desktop entry, and the two caches, all in user
scope:

```sh
./build/install-desktop.sh
```

That command touches nothing about existing file associations. The second mode does:

```sh
./build/install-desktop.sh --set-default
```

It rewrites your real MIME associations so that archives open in INDIUM. It is left for
you to run, deliberately, because that is not a decision an install script should make on
your behalf.

## Dependencies

A dependency enters INDIUM only when it does genuine work for the program, and it must be
describable in one sentence. The count was never the point — an archiver that installs a
password manager it will never open is bloated at five dependencies, and honest at fifty.

### Crates

| Crate | Its sentence |
| --- | --- |
| `eframe` (features: `glow`, `wayland` — nothing else) | Draws the entire program: window, input, GL context. The only UI dependency. |
| `egui_extras` | Provides the virtualized table that lets a 100,000-entry archive scroll like a 100-entry one. |
| `sevenz-rust2` | Writes 7z with AES-256, which libarchive cannot do; also the source of 7z-specific detail the generic reader does not expose. |
| `wl-clipboard-rs` | Puts `text/uri-list` on the Wayland clipboard so copy-out works into any file manager. |
| `image` | Decodes the image formats the Preview tab shows; not a new dependency, since `eframe` already links it through its clipboard path. |
| `serde` + `toml` | Read and write the settings, bookmarks, and recent-files TOML files. |

Everything not in that table is the standard library or hand-written. CRC32 is a
twenty-line table, byte formatting is ten lines, and argument handling is
`std::env::args`.

### System libraries

| Library | Its sentence |
| --- | --- |
| `libarchive` | Reads and writes every supported container and filter in-process; it is a hard dependency of pacman itself, so it is present on every Arch machine that can install software. On Debian the package is `libarchive13t64` since the time64 transition, and `libarchive13` before it; the `.deb` declares both as alternatives so it installs on either. |
| `libwayland-client`, `libwayland-egl`, `libxkbcommon`, `libEGL` | What the compositor session already provides, and what the window cannot be drawn without. |
| `glibc`, `libgcc_s`, `libm` | The floor. |

### The sentence `ldd` cannot tell you

Run `ldd` on the release binary and the Wayland libraries are not in the output.
`libwayland-client`, `libwayland-egl`, `libxkbcommon` and `libEGL` are `dlopen`ed by name
at runtime, so they leave no `DT_NEEDED` entry to find. They are still hard requirements,
and both packages declare them anyway. A dependency list derived from `ldd` alone would
produce a package that installs cleanly and then fails to open a window.

### Bundled, not depended on

JetBrains Mono NL Nerd Font, regular and bold, embedded in the binary. Nothing is fetched
and nothing is read from the system — INDIUM links no fontconfig. The licence is OFL-1.1,
in `LICENSES/` alongside the GPL. `NL` is the no-ligature cut, and the reason is the whole
program in miniature: a filename holding `->` must render as the two characters the archive
stores, not as the single arrow a programmer's font would prefer to draw.

### The toolkit gate

No GTK. No Qt. No KF6. No portal. No X11. This is enforced rather than intended:
`build/check-deps.sh` runs `ldd target/release/indium` and fails if the output contains
`gtk`, `Qt`, `KF6`, `X11`, or `portal`, and it also asserts that the binary is PIE and
full-RELRO. It runs by hand before every release, inside the Arch package's own `check()`,
and inside the release workflow. What arrives at V1.4 is the rest of it: CI on every push,
with this script as a gate that blocks a merge.

## Formats

### Read

Everything system libarchive reads, with one deliberate hole.

| Level | Formats |
| --- | --- |
| Containers | tar in all its variants, zip, 7z, cpio, ar, xar, mtree, iso9660, cab, lha, and deb and rpm as containers |
| Filters | gzip, bzip2, xz, lzma, lzip, lz4, zstd, lzop, lrzip, compress |

Encrypted zip and 7z entries are read after a password prompt, asked at the moment of use.

### Write

| Method | Produces |
| --- | --- |
| Store | `.tar` — no compression |
| lz4 | `.tar.lz4` |
| gzip | `.tar.gz` |
| zstd | `.tar.zst` |
| bzip2 | `.tar.bz2` |
| xz | `.tar.xz` |
| LZMA2 | `.7z` — the only road to AES-256 |
| Deflate | `.zip` |

Encryption is **7z AES-256 and nothing else.** There is no zip encryption. CORE §5's write
sentence names the five tar filters and omits the plain uncompressed tar its own method
table lists; the program writes it, so it is in the table above.

### RAR is deliberately absent

Not read, not written. The format's owner permits no one to create RAR archives, and the
maker has ruled the format out entirely rather than carry half of it. The reader capability
sits unused inside libarchive; INDIUM checks the detected format after open and refuses.
Opening one produces a plain sentence:

> RAR is not supported.

`org.indium.desktop` registers every supported MIME type and deliberately not
`application/vnd.rar`.

ACE is absent for the same family of reasons and its security history.

## Keyboard

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

Bare letters are shortcuts, which is why there is deliberately no type-to-jump in the
table and why every bare letter is suppressed while a text field has focus. No modal
editing, no `hjkl`, no `:` commands — anywhere, ever.

## What INDIUM will never do

- **No network at all** — no update check, no telemetry, no analytics, no crash reporting.
- **The app never opens a URL.** Addresses in About are text you can select, never click.
- **No RAR** — not read, not written.
- **No plugin system** — not open for future arrangements.
- **No second theme.** Ubuntu Canonical Aubergine, and no theme setting.
- **No X11.** Wayland only.
- **Passwords are never stored or remembered** — typed per use, wiped after.

That is the short form. The full list is CORE.md §9, where it is authored by the maker's
hand rather than derived, and where items enter and leave the same way.

## Licence

**GPL-3.0-only.** The full text is in `LICENSE`, and it is readable inside the program
itself, from About.

JetBrains Mono NL is under the SIL Open Font Licence 1.1, in `LICENSES/`.

## The documents

`CORE.md` is the authoritative design document. When it and anything else in this
repository disagree — a P-document, a comment, this file — CORE.md wins.

`build/docs/P1.md` through `P6.md` are the record of how INDIUM was built, one milestone
each, and every one of them ends in a deviations ledger that is honest about what was done
differently and why. A document describing behaviour the code does not have is the one
failure this project treats as unforgivable, so the ledgers correct the record rather than
rewrite it.

---

Copyright © sudo-megas

*Built with Reason and Passion.*
