# The icon

Supplied by the maker, as CORE §6 requires: *"The icon is photorealistic PNG, supplied by
the maker, installed at the hicolor sizes provided. No SVG."*

These are **masters**, not shipped assets. They live under `build/` rather than `assets/`
because the set runs to 4096×4096 and most of it is never installed —
`build/install-desktop.sh` picks out the sizes a desktop actually looks in, and the rest
stay here as sources. The one exception is `indium-256.png`, which `src/main.rs` embeds
with `include_bytes!` for the window icon.

## The set

| File | Origin |
| --- | --- |
| `indium-4096.png` | the maker's, and now **the master every other size comes from** — see P6 below |
| every other size | **derived** from it, Lanczos, see below |

All are 8-bit RGBA PNG, square, and every one of them now has a real alpha channel.

## P6: the corners were opaque white

The whole set shipped with an alpha channel that was **entirely opaque** — `min = max` on
every file, 16 through 4096 — and the four corners outside the rounded square were opaque
white pixels rather than nothing. On a light desktop nobody noticed. INDIUM's own window is
`#300A24`, so the moment the mark was drawn in the sidebar and in About (P6 §6.4b) it
appeared in a white box, and so had the window and taskbar icon all along.

Nothing was redesigned. The corners were flood-filled to transparent on the 4096 master and
every other size was regenerated from it:

```sh
cd build/icons
m=4095
magick indium-4096.png -alpha set -fuzz 40% \
  -fill none -floodfill +0+0     white  -fill none -floodfill +${m}+0     white \
  -fill none -floodfill +0+${m}  white  -fill none -floodfill +${m}+${m}  white \
  PNG32:indium-4096.png
for s in 2048 1024 512 256 128 96 64 48 32 24 22 16; do
  magick indium-4096.png -filter Lanczos -resize ${s}x${s} -strip PNG32:indium-${s}.png
done
```

**40% fuzz, and the number was measured rather than guessed.** At 10% a white fringe
survives along the outside of the copper border and is plainly visible once the icon is
composited over the panel colour; at 40% the edge goes copper-to-nothing with no halo, and
the flood stops at the border either way, so the artwork inside is untouched. The result is
**4.96% transparent** — which is the four corners of a squircle and nothing else.

Regenerating every size from one master was safe to do because the sizes were already the
same artwork: `indium-256.png` against a Lanczos downscale of `indium-1024.png` measured an
RMSE of **0.001**. They were downscales before and they are downscales now, from one file
whose alpha is right.

## The four derived sizes

freedesktop's hicolor theme has directories the maker's set did not cover, and **48×48 is
the one size the specification actually mandates**. They were downscaled from
`indium-512.png` rather than drawn — mechanical, and recorded here so it is not mistaken
for design:

```sh
cd build/icons
for s in 22 24 48 96; do
  magick indium-512.png -filter Lanczos -resize ${s}x${s} -strip PNG32:indium-${s}.png
done
```

ImageMagick 7 on the build machine. Replace any of the four with a hand-tuned version and
nothing else has to change — the install script takes whatever is there.

## Installing

```sh
./build/install-desktop.sh              # icons + desktop entry, user scope
./build/install-desktop.sh --set-default   # and take the MIME types
```

Each size installs to `~/.local/share/icons/hicolor/<n>x<n>/apps/indium.png`, matching
`Icon=indium` in `assets/org.indium.desktop`. The script also runs
`gtk-update-icon-cache`: `update-desktop-database` rebuilds MIME associations and does
nothing at all for icons, and where an `icon-theme.cache` already exists GTK trusts it and
will not see a new icon until it is regenerated.

## What is deliberately absent

**No SVG**, per CORE §6. **No `@2` HiDPI directories** — the convention here is one
integer per file and 1× only; a compositor scales from the nearest larger size, and the
set goes to 512 installed and 4096 on disk, so there is plenty to scale from.
