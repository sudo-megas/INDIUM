# The embedded typeface

One face, two weights, embedded in the binary via `include_bytes!` in `src/theme.rs`.
CORE §2 counts these as bundled assets, not dependencies — nothing is fetched, and
nothing is read from the system at runtime. INDIUM links no fontconfig and asks the
machine for nothing.

## What is here

| File | Bytes | Role |
| --- | --- | --- |
| `CaskaydiaMonoNerdFontMono-Regular.ttf` | 2 856 256 | every family — chrome and values alike |
| `CaskaydiaMonoNerdFontMono-Bold.ttf` | 2 856 328 | the `caskaydia-bold` family, CORE §6's weight distinction |

## Where they came from

Copied verbatim, byte for byte, from the Arch package on the build machine:

```sh
pacman -Qi ttf-cascadia-mono-nerd                              # 3.5.0-1, licence OFL-1.1-RFN
cp /usr/share/fonts/TTF/CaskaydiaMonoNerdFontMono-Regular.ttf  assets/fonts/
cp /usr/share/fonts/TTF/CaskaydiaMonoNerdFontMono-Bold.ttf     assets/fonts/
cp /usr/share/licenses/ttf-cascadia-mono-nerd/LICENSE          LICENSES/OFL-1.1.txt
```

Upstream: Cascadia Mono — copyright © 2019–present Microsoft Corporation, with Reserved
Font Name *Cascadia Code* — patched by Nerd Fonts,
<https://github.com/ryanoasis/nerd-fonts>. Unmodified: no subsetting, no re-hinting, no
renaming. What pacman installed is what ships.

`build/package/verify.sh` derives the expected copyright holder from the first block of
`LICENSES/OFL-1.1.txt` and fails the `.deb` if `build/package/deb/copyright.header`
disagrees, or if the face it names matches no file in this directory. That gate exists
because P12's JetBrains → Fira swap shipped a copyright naming the wrong font for three
releases. **Copy the licence in the same commit as the font, or the build says so.**

## Why these particular cuts

**`Mono`** is the single-cell icon cut. A Nerd Font's icons are double-width in the
default cut, and one in a filename would widen a table column and break the alignment the
entry table is for. Verified: `'0'`, `U+F187` and `U+F071` all measure advance 1200 on a
2048-unit em — 0.586 em — in both weights.

**Cascadia `Mono`, not Cascadia `Code`** — the Nerd Font names these `CaskaydiaMono` and
`CaskaydiaCove`, which differ by one word in a filename and by whether an archive's
contents are drawn correctly. Cascadia **Code** is the ligature cut. A filename holding
`->` must render as the two characters the archive stores, not as an arrow.

**This is the reason P23 nearly shipped the wrong face, and it is worth writing down.** The
swap was first justified on the grounds that the face's ligatures could not matter, because
egui applies no OpenType shaping. That is false: `epaint` 0.36 shapes through **`harfrust`**,
a HarfBuzz port, calling `shaper.shape(buffer, ShapeOptions::new())` — HarfBuzz's *default*
horizontal feature set, which has `calt` enabled, and `calt` is where Cascadia keeps its
ligatures. Measured on the Cove cut: all twenty probed sequences substitute, and `www`
collapses into a single 23-pixel glyph followed by two zero-width continuations.

The guarantee therefore comes from the face, exactly as CORE §2 has always said — and it is
now held by a test, `a_filename_is_the_characters_it_holds`, which renders each sequence in
both weights and asserts every glyph is the one that face draws standing alone.

**These are TrueType (`glyf`), where the previous face was OTF/CFF.** Not a problem, and
checked rather than assumed: egui 0.36 rasterizes through Fontations — `skrifa`,
`read-fonts` and `harfrust` — which reads both natively.

## Coverage, measured

Both faces were read out of their `cmap` tables and compared, rather than trusted. The two
weights carry **identical** codepoint sets, which is not a given and is why both are read.

| | Codepoints |
| --- | --- |
| CaskaydiaMono Nerd Font Mono | 12 938 |
| Fira Mono Nerd Font Mono (previous) | 12 132 |
| JetBrains Mono NL Nerd Font Mono (before that) | 12 121 |

Present and verified: the full Turkish set (`ı ş ğ ç ö ü İ`), accented Latin, box drawing
(`U+2500`–`U+257F` complete), `·` and `—`, modern Greek, modern Russian and Ukrainian
Cyrillic. `köpek.txt`, `AŞÇALIKĞA.txt` and `résumé.pdf` all render whole.

**What the swap recovers.** 1 296 codepoints arrive that Fira did not have, and they
include *every one this file previously recorded as a loss*:

- **Vietnamese** — the 89-odd Latin Extended Additional precomposed set (`ạ ả ấ …`) and the
  horned vowels `ơ ư`. This directory's previous note said "no Vietnamese"; it is wrong now
  and the sentence in `install_fonts` was corrected with it.
- **`ə` / `Ə` (U+0259, U+018F), the schwa** — Azerbaijani, the neighbour language most
  likely to appear beside Turkish.
- **`ẞ` (U+1E9E), capital sharp s.**

**What it costs.** 490 codepoints Fira carried are absent here. The modern alphabets are
not among them — Russian, Ukrainian and modern Greek are complete, and the one gap in the
Greek block is `U+03A2`, which Unicode does not assign. What is gone is historic:

- **114 Cyrillic**, from `U+0460` up — Old Church Slavonic.
- **44 Greek**, the archaic letters and the polytonic `U+1F00` range — Ancient Greek.
- **102 of the 112 arrows** in `U+2190`–`U+21FF`. The ten that remain are
  `← ↑ → ↓ ↔ ↕ ↨ ↲ ⇡ ⇣`.

**One drawn glyph is not in that set, and it was not in Fira's either.** `keys.rs:38` draws
*"Details ⇄ Preview"* in the Keys popup, mirroring `CORE.md:301`, and `U+21C4` is absent
from **both** faces — so that row has been painting tofu in every release, not because of
this swap. It is recorded here because this is the file where face coverage is measured; the
fix belongs to P23 §2d, and it needs `CORE.md:301` to move, which is the maker's hand.

A name INDIUM cannot draw is not a name INDIUM has lost: since P11 the *reading* of every
name is locale-correct whatever the face can draw, and a missing glyph shows as tofu rather
than vanishing. This is a legibility cost, not a data one — but it is a cost, and it is
written down here rather than discovered.
