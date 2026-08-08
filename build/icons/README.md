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
| `indium-16.png` `indium-32.png` `indium-64.png` | the maker's |
| `indium-128.png` `indium-256.png` `indium-512.png` | the maker's |
| `indium-1024.png` `indium-2048.png` `indium-4096.png` | the maker's — masters, not installed |
| `indium-22.png` `indium-24.png` `indium-48.png` `indium-96.png` | **derived**, see below |

All are 8-bit RGBA PNG, square.

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
