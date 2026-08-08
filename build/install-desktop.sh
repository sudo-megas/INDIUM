#!/bin/sh
# P3 §4: dev-machine registration, user scope. Run from the repository root.
# Packages redo this system-wide in P6. Without --set-default the script installs
# and touches nothing about the user's existing choices.
set -e
install -Dm644 assets/org.indium.desktop \
  "$HOME/.local/share/applications/org.indium.desktop"

# P5: the maker's icons live in build/icons/ as `indium-<size>.png`. They are masters
# rather than shipped assets — the set runs to 4096 — so only the sizes a desktop
# actually looks in are installed, and the rest stay put as sources.
#
# Every one is installed as `apps/indium.png`, matching `Icon=indium` in the desktop
# entry. The loop still tolerates a missing file, so a partial set installs what exists
# rather than failing: P3 §4 made that deliberate and it is still true.
for size in 16 22 24 32 48 64 96 128 256 512; do
  png="build/icons/indium-${size}.png"
  [ -e "$png" ] || continue
  install -Dm644 "$png" \
    "$HOME/.local/share/icons/hicolor/${size}x${size}/apps/indium.png"
done

command -v update-desktop-database >/dev/null &&
  update-desktop-database "$HOME/.local/share/applications" || true

# update-desktop-database rebuilds MIME associations and does nothing whatever for
# icons. Where an icon-theme.cache already exists — and one usually does — GTK trusts
# the cache and will not see a newly installed indium.png until it is regenerated. `-f`
# forces past the mtime check, `-t` skips the missing-index.theme refusal.
command -v gtk-update-icon-cache >/dev/null &&
  gtk-update-icon-cache -q -t -f "$HOME/.local/share/icons/hicolor" || true

if [ "$1" = "--set-default" ]; then
  xdg-mime default org.indium.desktop \
    application/zip application/x-7z-compressed application/x-tar \
    application/gzip application/x-xz application/zstd application/x-bzip2 \
    application/x-lzip application/x-cpio application/x-iso9660-image \
    application/x-compressed-tar application/x-xz-compressed-tar \
    application/x-zstd-compressed-tar application/x-bzip2-compressed-tar
fi
