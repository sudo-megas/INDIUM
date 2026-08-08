#!/bin/sh
# P3 §4: dev-machine registration, user scope. Run from the repository root.
# Packages redo this system-wide in P6. Without --set-default the script installs
# and touches nothing about the user's existing choices.
set -e
install -Dm644 assets/org.indium.desktop \
  "$HOME/.local/share/applications/org.indium.desktop"
for png in assets/icon/*.png; do
  [ -e "$png" ] || continue
  size=$(basename "$png" .png)
  install -Dm644 "$png" \
    "$HOME/.local/share/icons/hicolor/${size}x${size}/apps/indium.png"
done
command -v update-desktop-database >/dev/null &&
  update-desktop-database "$HOME/.local/share/applications" || true
if [ "$1" = "--set-default" ]; then
  xdg-mime default org.indium.desktop \
    application/zip application/x-7z-compressed application/x-tar \
    application/gzip application/x-xz application/zstd application/x-bzip2 \
    application/x-lzip application/x-cpio application/x-iso9660-image \
    application/x-compressed-tar application/x-xz-compressed-tar \
    application/x-zstd-compressed-tar application/x-bzip2-compressed-tar
fi
