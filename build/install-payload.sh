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
# Usage: install-payload.sh [--remove] ROOT [BINARY]
#   ROOT      the directory that holds bin/ and share/ — "$pkgdir/usr" for makepkg,
#             "$work/data/usr" for the .deb, "$HOME/.local" for the dev machine.
#   BINARY    installed to ROOT/bin/indium, and only when given: a package owns /usr/bin,
#             cargo owns target/release, and the dev-machine install passes no binary.
#   --remove  take back exactly what an install into the same ROOT put there, and nothing
#             else. It belongs here rather than in `install-desktop.sh` for the same reason
#             the rest of this script does: the icon-size list is written down once. An
#             uninstaller carrying its own copy of that list is one that will eventually
#             miss a size the installer added — which is how ten orphaned icons are made.
#
# Runs from anywhere: the sources it copies are found relative to this script, not to the
# caller's working directory. The packages happen to call it from the repository root and
# are unaffected — `$0` resolves to the same tree either way — but a dev machine running
# `~/INDIUM/build/install-desktop.sh` from `$HOME` is not, and used to fail here.
set -e
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo=$(dirname -- "$here")
remove=
if [ "$1" = "--remove" ]; then
  remove=1
  shift
fi
[ -n "$1" ] || { echo "usage: install-payload.sh [--remove] ROOT [BINARY]" >&2; exit 2; }
root=$1

# Symmetry is the whole point: every path below is written once and read by both
# directions, so there is no list an uninstall can be behind on.
if [ -n "$2" ]; then
  if [ -n "$remove" ]; then
    rm -f "$root/bin/indium"
  else
    install -Dm755 "$2" "$root/bin/indium"
  fi
fi

desktop="$root/share/applications/org.indium.desktop"
if [ -n "$remove" ]; then
  rm -f "$desktop"
else
  install -Dm644 "$repo/assets/org.indium.desktop" "$desktop"
fi

# P5: the maker's icons live in build/icons/ as `indium-<size>.png`. They are masters
# rather than shipped assets — the set runs to 4096 — so only the sizes a desktop
# actually looks in are installed, and the rest stay put as sources. Every one is
# installed as `apps/indium.png`, matching `Icon=indium` in the desktop entry.
for size in 16 22 24 32 48 64 96 128 256 512; do
  dest="$root/share/icons/hicolor/${size}x${size}/apps/indium.png"
  if [ -n "$remove" ]; then
    rm -f "$dest"
    # `rmdir` is the only safe way to prune this: it refuses a directory that still
    # holds anything, so an `apps/` or a `48x48/` another program also installs into
    # survives untouched, and INDIUM cannot take a neighbour's icons down with it.
    rmdir "$root/share/icons/hicolor/${size}x${size}/apps" 2>/dev/null || true
    rmdir "$root/share/icons/hicolor/${size}x${size}" 2>/dev/null || true
  else
    png="$repo/build/icons/indium-${size}.png"
    [ -e "$png" ] || continue
    install -Dm644 "$png" "$dest"
  fi
done
