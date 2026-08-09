# The embedded typeface

One face, two weights, embedded in the binary via `include_bytes!` in `src/theme.rs`.
CORE §2 counts these as bundled assets, not dependencies — nothing is fetched, and
nothing is read from the system at runtime. INDIUM links no fontconfig and asks the
machine for nothing.

## What is here

| File | Bytes | Role |
| --- | --- | --- |
| `FiraMonoNerdFontMono-Regular.otf` | 2 362 764 | every family — chrome and values alike |
| `FiraMonoNerdFontMono-Bold.otf` | 2 366 400 | the `fira-bold` family, CORE §6's weight distinction |

## Where they came from

Copied verbatim, byte for byte, from the Arch package on the build machine:

```sh
pacman -Qi otf-firamono-nerd          # 3.5.0-1, licence OFL-1.1
cp /usr/share/fonts/OTF/FiraMonoNerdFontMono-Regular.otf assets/fonts/
cp /usr/share/fonts/OTF/FiraMonoNerdFontMono-Bold.otf    assets/fonts/
cp /usr/share/licenses/otf-firamono-nerd/LICENSE         LICENSES/OFL-1.1.txt
```

Upstream: Fira Mono — digitized data copyright © 2012–2015 The Mozilla Foundation and
Telefónica S.A. — patched by Nerd Fonts, <https://github.com/ryanoasis/nerd-fonts>.
Unmodified: no subsetting, no re-hinting, no renaming. What pacman installed is what ships.

## Why these particular cuts

**`Mono`** is the single-cell icon cut. A Nerd Font's icons are double-width in the
default cut, and one in a filename would widen a table column and break the alignment the
entry table is for.

**There is no `NL` cut, and none is needed.** The previous face was JetBrains Mono **NL** —
the no-ligature cut — because a filename holding `->` must render as the two characters the
archive stores, not as an arrow. Fira Mono carries **no ligatures at all**; that is Fira
*Code*'s job, and this is not Fira Code. A face that cannot form the ligature cannot get it
wrong.

**These are OTF, with CFF outlines (`OTTO`), where the previous face was TrueType.** That is
not a problem and it was checked rather than assumed: egui 0.36 rasterizes through
Fontations — `skrifa`, `read-fonts` and `harfrust`, all visible in `epaint`'s dependencies —
which reads CFF natively.

## Coverage, measured

Both faces were read out of their `cmap` tables and compared, rather than trusted:

| | Codepoints |
| --- | --- |
| Fira Mono Nerd Font Mono | 12 132 |
| JetBrains Mono NL Nerd Font Mono (previous) | 12 121 |

Present and verified: the full Turkish set (`ı ş ğ ç ö ü İ`), accented Latin, box drawing,
arrows, `·` and `—`, Greek, Cyrillic. `köpek.txt`, `AŞÇALIKĞA.txt` and `résumé.pdf` all
render whole.

**Honestly, what the swap costs.** 524 codepoints the old face carried are absent from this
one — against 535 in the other direction, so the totals are near enough identical but the
sets are not. The losses that could matter to a real filename:

- **89 in Latin Extended Additional**, which is mostly **Vietnamese** (`ạ ả ấ …`).
- **`ə` / `Ə` (U+0259, U+018F), the schwa** — Azerbaijani, and the neighbour language most
  likely to turn up beside Turkish.
- `ẞ` (U+1E9E, capital sharp s), and `ơ ư` — the Vietnamese horned vowels.

A name INDIUM cannot draw is not a name INDIUM has lost: since P11 the *reading* of every
name is locale-correct whatever the face can draw, and a missing glyph shows as tofu rather
than vanishing. This is a legibility cost, not a data one — but it is a cost, and it is
written down here rather than discovered.
