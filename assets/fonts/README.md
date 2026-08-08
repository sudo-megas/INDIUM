# The embedded typeface

One face, two weights, embedded in the binary via `include_bytes!` in `src/theme.rs`.
CORE §2 counts these as bundled assets, not dependencies — nothing is fetched, and
nothing is read from the system at runtime. INDIUM links no fontconfig and asks the
machine for nothing.

## What is here

| File | Bytes | Role |
| --- | --- | --- |
| `JetBrainsMonoNLNerdFontMono-Regular.ttf` | 2 451 248 | every family — chrome and values alike |
| `JetBrainsMonoNLNerdFontMono-Bold.ttf` | 2 453 632 | the `jetbrains-bold` family, CORE §6's weight distinction |

## Where they came from

Copied verbatim, byte for byte, from the Arch package on the build machine:

```sh
pacman -Qi ttf-jetbrains-mono-nerd      # 3.5.0-1, licence OFL-1.1-no-RFN
cp /usr/share/fonts/TTF/JetBrainsMonoNLNerdFontMono-Regular.ttf assets/fonts/
cp /usr/share/fonts/TTF/JetBrainsMonoNLNerdFontMono-Bold.ttf    assets/fonts/
cp /usr/share/licenses/ttf-jetbrains-mono-nerd/OFL.txt          LICENSES/OFL-1.1.txt
```

Upstream: JetBrains Mono, patched by Nerd Fonts — <https://github.com/ryanoasis/nerd-fonts>.
Unmodified: no subsetting, no re-hinting, no renaming. What pacman installed is what
ships.

## Why these particular cuts

**`NL`** is the no-ligature cut. JetBrains Mono normally draws `->` as a single arrow and
`!=` as a crossed equals. INDIUM displays filenames, stored paths and checksums — literal
bytes out of somebody's archive — and a program that refuses to guess a compressed size
must not quietly redraw a filename either. Two characters stored, two characters shown.

**`…NerdFontMono…`** is the single-cell icon cut: every icon glyph is forced to one
column, so an icon in a column does not shove the column out of alignment. The entry
table depends on that.

## What it covers, and what it does not

12 218 codepoints. Latin and Latin-1, the arrows, box-drawing and geometric shapes
(`× ✕ ▸ ▾ ▶ ▼ → ✓ · — █`), and roughly ten and a half thousand Nerd Font icons across the
private-use area and plane 15.

It carries **no CJK and no emoji**, and there is no fallback face behind it —
`FontDefinitions::empty()` in `src/theme.rs`, because `eframe` is built without
`default_fonts`. A filename in Japanese, or one with an emoji in it, renders as tofu.
That is a known and accepted limit of embedding one face and linking nothing, written
down here so it is a limit and not a surprise.

It also carries no `★ U+2605`, which is why the bookmark pin is still a `+`. P1
Deviation 5 recorded the original substitutions; only the reason changed.

## Licence

**OFL-1.1** (SIL Open Font Licence 1.1), full text in `LICENSES/OFL-1.1.txt`. The Arch
package declares the `no-RFN` variant — no Reserved Font Name — so nothing here
constrains what INDIUM may be called. OFL-1.1 is compatible with GPL-3.0-only for
bundling; the font is not a derived work of the program, nor the program of the font.

Verifying a fresh copy against what is committed:

```sh
sha256sum assets/fonts/*.ttf /usr/share/fonts/TTF/JetBrainsMonoNLNerdFontMono-{Regular,Bold}.ttf
```
