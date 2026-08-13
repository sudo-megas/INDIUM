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
#
# --uninstall takes it all back. PXX's certification walk found the desktop entry and ten
# hicolor icons still in `~/.local` after `pacman -R indium`, and the package was right to
# leave them: it never owned them, this script installed them, and until now this script
# had no way to remove them. A dev-machine installer with no uninstaller is a machine that
# accumulates.
set -e
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)

if [ "$1" = "--uninstall" ]; then
  "$here/install-payload.sh" --remove "$HOME/.local"
else
  "$here/install-payload.sh" "$HOME/.local"
fi

command -v update-desktop-database >/dev/null &&
  update-desktop-database "$HOME/.local/share/applications" || true

# update-desktop-database rebuilds MIME associations and does nothing whatever for
# icons. Where an icon-theme.cache already exists — and one usually does — GTK trusts
# the cache and will not see a newly installed indium.png until it is regenerated. `-f`
# forces past the mtime check, `-t` skips the missing-index.theme refusal.
#
# Both caches are refreshed in both directions. An uninstall that removes the files and
# leaves the caches naming them is an uninstall a desktop has not heard about yet.
command -v gtk-update-icon-cache >/dev/null &&
  gtk-update-icon-cache -q -t -f "$HOME/.local/share/icons/hicolor" || true

# **`--uninstall` does not undo this, and that is deliberate.** `xdg-mime` has no inverse —
# there is no "unset default" — so taking it back would mean this script hand-editing the
# user's `mimeapps.list`, which is their file and holds every other association they have
# ever made. A default naming a desktop entry that no longer exists is inert: the desktop
# falls through to the next handler. Rewriting someone's preferences to tidy up after
# ourselves would be the larger damage, and a freeze is the wrong moment to be clever.
if [ "$1" = "--set-default" ]; then
  xdg-mime default org.indium.desktop \
    application/zip application/x-7z-compressed application/x-tar \
    application/gzip application/x-xz application/zstd application/x-bzip2 \
    application/x-lzip application/x-cpio application/x-iso9660-image \
    application/x-compressed-tar application/x-xz-compressed-tar \
    application/x-zstd-compressed-tar application/x-bzip2-compressed-tar
fi
