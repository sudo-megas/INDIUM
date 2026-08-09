#!/bin/sh
# P3 §4: dev-machine registration, user scope. Run from the repository root.
# The payload — the desktop entry and the icons — lives in build/install-payload.sh, which
# the packages call with their own roots; the two installs differ only in that root. What
# is left here is what only a dev machine needs: the caches, and the MIME types. Without
# --set-default the script installs and touches nothing about the user's existing choices.
#
# Runs from anywhere. It used to say "run from the repository root" and mean it: the line
# below was `./build/install-payload.sh`, relative to whatever directory the caller
# happened to be standing in, so the one invocation a user is most likely to type —
# `~/INDIUM/build/install-desktop.sh --set-default`, from `$HOME` — died on
# "No such file or directory". A script that only works from one directory should resolve
# its own, and this one now does.
set -e
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
"$here/install-payload.sh" "$HOME/.local"

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
