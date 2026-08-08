#!/bin/sh
# CORE §8: installers create the launcher entry. This script is that payload — the desktop
# entry and the icon sizes — written down once, in one place, for everything that installs
# INDIUM onto a system.
#
# `build/install-desktop.sh:3` used to say "Packages redo this system-wide in P6." They do
# not redo it: the Arch package, the .deb and the dev machine all call this script with
# their own root. So there is exactly one icon-size list in the tree, and no way for a
# package and a dev install to drift apart about what INDIUM puts on a machine.
#
# The icon loop tolerates a missing file and installs what exists rather than failing.
# P3 §4 made that deliberate and it is still true — but the tolerance ends at
# `build/package/verify.sh`, where a *package* missing a size fails the release. A dev
# machine may run with a partial set; a release may not ship with one.
#
# Usage: install-payload.sh ROOT [BINARY]
#   ROOT    the directory that holds bin/ and share/ — "$pkgdir/usr" for makepkg,
#           "$work/data/usr" for the .deb, "$HOME/.local" for the dev machine.
#   BINARY  installed to ROOT/bin/indium, and only when given: a package owns /usr/bin,
#           cargo owns target/release, and the dev-machine install passes no binary.
#
# Run from the repository root.
set -e
[ -n "$1" ] || { echo "usage: install-payload.sh ROOT [BINARY]" >&2; exit 2; }
root=$1

if [ -n "$2" ]; then
  install -Dm755 "$2" "$root/bin/indium"
fi

install -Dm644 assets/org.indium.desktop \
  "$root/share/applications/org.indium.desktop"

# P5: the maker's icons live in build/icons/ as `indium-<size>.png`. They are masters
# rather than shipped assets — the set runs to 4096 — so only the sizes a desktop
# actually looks in are installed, and the rest stay put as sources. Every one is
# installed as `apps/indium.png`, matching `Icon=indium` in the desktop entry.
for size in 16 22 24 32 48 64 96 128 256 512; do
  png="build/icons/indium-${size}.png"
  [ -e "$png" ] || continue
  install -Dm644 "$png" \
    "$root/share/icons/hicolor/${size}x${size}/apps/indium.png"
done
