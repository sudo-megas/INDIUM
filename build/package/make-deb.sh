#!/bin/sh
# INDIUM — build a .deb from an already-built binary. CORE §8: "Packages: `.pkg.tar.zst`
# (Arch) and `.deb` (Debian/Ubuntu), from P6 onward. Nothing else."
#
#   ./build/package/make-deb.sh <indium-binary> [outdir]      # from the repository root
#
# The binary is an ARGUMENT and never an assumption. The .deb that ships is built inside
# a Debian bookworm container (`.github/workflows/release.yml`) so that its glibc floor is
# a fact rather than a promise, and this script has to be able to wrap a binary that some
# other machine produced.
#
# WHY THIS IS HAND-WRITTEN: `dpkg-deb` does not exist on the build machine and cannot be
# made to — CORE §2 admits a tool only when it can fill in its sentence, and "the thing
# that writes a 60-byte header" cannot. A .deb is an `ar` archive of exactly three members
# in a fixed order, which is a format small enough to write out. It is the same posture as
# the hand-written CRC32 table and the hand-written libarchive FFI: the format is small,
# so INDIUM owns it.
set -e

BIN=$1
[ -n "$BIN" ] || { echo "usage: $0 <indium-binary> [outdir]" >&2; exit 2; }
[ -f "$BIN" ] || { echo "make-deb: no such binary: $BIN" >&2; exit 2; }
[ -f Cargo.toml ] && [ -f CORE.md ] || { echo "make-deb: run from the repository root" >&2; exit 2; }

OUT=${2:-build/package/out}

# The four tools this script is built out of, named where a missing one is cheap to
# explain rather than at the point where it produces something subtly wrong. `bsdtar` is
# the one that catches people: on Arch it comes with the `libarchive` package itself, and
# on Debian it is in `libarchive-tools` — `libarchive-dev` alone does not bring it.
for tool in bsdtar xz readelf ar; do
  command -v "$tool" >/dev/null || { echo "make-deb: $tool is not installed" >&2; exit 1; }
done

# The version is read out of Cargo.toml and never written down a second time. `^version =`
# matches only the [package] field: every dependency states its version inside an inline
# table, so no line of theirs begins with that word.
VERSION=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -1)
[ -n "$VERSION" ] || { echo "make-deb: no version in Cargo.toml" >&2; exit 1; }

# The Debian revision, read out of the PKGBUILD's `pkgrel` and never written down a second
# time — the same discipline as the version above, for the same reason. Debian and Arch
# happen to spell this number identically, so one line can serve both and the .deb and the
# .pkg.tar.zst cannot come out claiming to be different builds of one tree.
#
# It counts rebuilds of the same upstream version. Debian's convention reserves that for
# packaging-only changes; P10 spends it on a source fix instead, at the maker's word, which
# is why `1.0.0-2` is a different binary from `1.0.0-1` and not merely a different wrapper.
REVISION=$(sed -n 's/^pkgrel=\([0-9][0-9]*\)$/\1/p' build/package/PKGBUILD | head -1)
[ -n "$REVISION" ] || { echo "make-deb: no pkgrel in build/package/PKGBUILD" >&2; exit 1; }
ARCH=amd64
MAINTAINER='sudo-megas <sudo-megas@users.noreply.github.com>'
HOMEPAGE=https://github.com/sudo-megas/INDIUM

# Reproducibility: one timestamp for everything in the package. SOURCE_DATE_EPOCH if the
# environment sets it, otherwise the date of the commit being packaged — so two builds of
# one commit agree about every timestamp they write, without anyone passing anything. That
# is as far as the claim goes: xz's encoder output moves between its own releases, so
# byte-identical rebuilds need the same xz as well, not just the same commit.
#
# git is asked, but not required, and its failure is not allowed to be silent. A container
# checkout is owned by a different uid than the step that runs in it, so git refuses the
# repository outright ("detected dubious ownership") — which killed this script's first run
# under the release workflow. Falling back to the current time would have been worse than
# failing: the package would build, and its reproducibility claim would quietly stop being
# true. So git may fail, and the script then insists on being told.
EPOCH=$SOURCE_DATE_EPOCH
[ -n "$EPOCH" ] || EPOCH=$(git log -1 --format=%ct 2>/dev/null || true)
[ -n "$EPOCH" ] || {
  echo "make-deb: no commit date available (not a usable git checkout), and" >&2
  echo "          SOURCE_DATE_EPOCH is unset. Set it to build reproducibly." >&2
  exit 2
}

W=$(mktemp -d)
trap 'rm -rf "$W"' EXIT INT HUP TERM
mkdir -p "$W/data/usr" "$W/control"

# ---------------------------------------------------------------- data.tar contents ----

# The desktop entry and the ten hicolor icons are one script's job, shared with the Arch
# package. Listing them here as well is how the two packages start to disagree.
./build/install-payload.sh "$W/data/usr" "$BIN"

DOC=$W/data/usr/share/doc/indium
install -d -m 755 "$DOC"

# /usr/share/doc/indium/copyright, in machine-readable DEP-5. The GPL paragraph points at
# /usr/share/common-licenses/GPL-3 as Policy 12.5 asks, rather than repeating 35 kB that
# every Debian system already has. The OFL has no common-licenses entry, so it is inlined
# — and inlined *from* LICENSES/OFL-1.1.txt, so the packaged copy and the repository copy
# cannot drift apart. The sed is the whole of DEP-5's line syntax: a blank line becomes a
# lone `.`, and every line gains one leading space.
{
  cat build/package/deb/copyright.header
  sed 's/^$/./; s/^/ /' LICENSES/OFL-1.1.txt
} > "$DOC/copyright"
chmod 644 "$DOC/copyright"

# The Debian changelog, gzipped as Policy requires. `-n` leaves the original name and
# mtime out of the gzip header — the one thing inside a .gz that would otherwise differ
# between two identical builds.
gzip -9nc build/package/deb/changelog.Debian > "$DOC/changelog.Debian.gz"
chmod 644 "$DOC/changelog.Debian.gz"

# ------------------------------------------------------------------- the glibc floor ----

# Depends must state the oldest glibc that can run this exact binary, and the binary is
# the only honest source for it: every versioned symbol it imports names a glibc release,
# and the newest of those is the floor. Computed here, never written down — a hardcoded
# floor is a promise that goes stale the first time a build machine updates.
#
# WEAK symbols are excluded deliberately. Rust's std imports `pidfd_spawnp@GLIBC_2.39`
# weakly and resolves it to NULL where the running glibc has no such symbol, taking the
# fork/exec path instead. Counting it would raise a bookworm-built binary's floor from
# 2.36 to 2.39 — three releases' worth of Debians that can in fact run it — in exchange
# for nothing. GLOBAL UND symbols are the ones the loader must actually find.
#
# Columns of `readelf -W --dyn-syms`: 5 is the binding, 7 is the section index, 8 is the
# name. Checked against real output on this binutils before being relied on.
floor() {   # floor <binary> <prefix-regex>
  readelf -W --dyn-syms "$1" \
    | awk -v v="$2" '$5 == "GLOBAL" && $7 == "UND" && $8 ~ v {print $8}' \
    | sed "s/.*@$2//" | sort -uV | tail -1
}
LIBC_MIN=$(floor "$BIN" GLIBC_)
GCC_MIN=$(floor "$BIN" GCC_)
[ -n "$LIBC_MIN" ] && [ -n "$GCC_MIN" ] || { echo "make-deb: no versioned symbols in $BIN" >&2; exit 1; }

# ------------------------------------------------------------- control.tar contents ----

# Installed-Size is in KiB and Policy calls it an estimate, which is exactly what
# `du -ks --apparent-size` of the staged tree is.
INSTALLED=$(du -ks --apparent-size "$W/data" | awk '{print $1}')

# Depends, and why each entry is there:
#
#   libc6, libgcc-s1     the floor, computed above from the binary's own symbols.
#   libarchive13t64 |    CORE §2's one hard system library. Debian's 64-bit-time_t
#     libarchive13       transition renamed the package to libarchive13t64 in trixie; the
#                        alternation covers bookworm, which still calls it libarchive13,
#                        without two packages or two control files.
#   libwayland-client0,  invisible to ldd. eframe reaches the compositor session through
#   libxkbcommon0,       dlopen, so none of the three appears in the dynamic table and no
#   libegl1              shlibs machinery could ever find them. They are named by hand
#                        because CORE §2 names them, and a package that omits them fails
#                        at the first window rather than at install time.
DEPENDS="libc6 (>= $LIBC_MIN), libgcc-s1 (>= $GCC_MIN), libarchive13t64 | libarchive13, libwayland-client0, libxkbcommon0, libegl1"

# There is no postinst and no postrm, on purpose. Debian ships dpkg triggers for both
# caches this package touches — desktop-file-utils declares interest in
# /usr/share/applications, and the GTK library packages declare interest in
# /usr/share/icons/hicolor — exactly as pacman has hooks for the same two. A maintainer
# script here would duplicate a trigger that already fires, and draw a lintian tag for it.
#
# The synopsis carries no leading article and no full stop, per Policy 3.4.1. The extended
# description is CORE §1 and §5 in the house voice, and it says what INDIUM will not do,
# because the absence is a design decision and not an oversight.
cat > "$W/control/control" <<EOF
Package: indium
Version: $VERSION-$REVISION
Architecture: $ARCH
Maintainer: $MAINTAINER
Installed-Size: $INSTALLED
Depends: $DEPENDS
Section: utils
Priority: optional
Homepage: $HOMEPAGE
Description: archive manager for Wayland where metadata is the main event
 Every other archiver on this platform treats an archive's contents as a name
 column and hides the rest behind a Properties dialog. INDIUM keeps a permanent
 Inspector pane on screen — sizes, packed sizes, ratio, method, checksums, four
 timestamps, ownership, mode, link targets, encryption state — because the
 stated ambition of the program is to be one of the most verbose archiver
 applications in the industry.
 .
 All format work happens inside the process. INDIUM never runs 7z, tar, unzip,
 zstd, or any other external compressor. When a format is listed as supported,
 it is supported by code linked into the binary, whether or not any archive tool
 is installed.
 .
 It reads everything system libarchive reads: tar in all its variants, zip, 7z,
 cpio, ar, xar, mtree, iso9660, cab, lha, and deb and rpm as containers, through
 gzip, bzip2, xz, lzma, lzip, lz4, zstd, lzop, lrzip and compress. It writes tar
 plain or with gz, bz2, xz, zst or lz4, zip with Deflate, and 7z with LZMA2.
 Encryption is 7z AES-256 and nothing else.
 .
 RAR is deliberately absent — not read, not written. The format's owner permits
 no one to create RAR archives, and the maker has ruled the format out entirely
 rather than carry half of it; opening one produces a plain sentence saying so.
 ACE is absent for the same family of reasons and its security history.
 .
 Wayland only, and toolkit-free: no GTK, no Qt, no KF6, no portal, no X11. No
 network of any kind — no update check, no telemetry, no analytics, no crash
 reporting. One archive per window; there are no tabs.
EOF
chmod 644 "$W/control/control"

# md5sums: one line per regular file, path relative to / with no leading `./`, in
# LC_ALL=C order. md5sum's own two-space separator is already the format dpkg reads, so
# nothing is reformatted on the way out.
( cd "$W/data" && find usr -type f | LC_ALL=C sort | xargs -r md5sum ) > "$W/control/md5sums"
chmod 644 "$W/control/md5sums"

# --------------------------------------------------------------------- the two tars ----

# Every mtime in both trees, set once. Directories carry mtimes into a tar as surely as
# files do, so they are touched too; -h touches a symlink rather than what it points at.
find "$W/data" "$W/control" -exec touch -h -d "@$EPOCH" {} +

# `--format=gnutar`, never bsdtar's default. bsdtar writes restricted pax by default and
# emits `x`-type extended headers whenever it feels the need — a sub-second mtime is
# enough to trigger one. dpkg's tar reader handles ustar and the GNU extensions, and GNU
# tar format is what dpkg-deb itself writes, so it is the format with no argument attached
# to it.
#
# --uid/--gid/--uname/--gname write root ownership into the archive instead of reading the
# building process's own, which is why no fakeroot appears anywhere in this script.
# --no-acls --no-xattrs --no-fflags keep the build machine's filesystem metadata out.
#
# The member list is sorted and fed in with -n (no recursion), so member order is sort
# order rather than readdir order — the last thing between this and a byte-identical
# rebuild. The `./` prefix on every name is what dpkg-deb writes too.
TARFLAGS="--format=gnutar --uid 0 --gid 0 --uname root --gname root --no-acls --no-xattrs --no-fflags"

# xz for both members, not zstd. A zstd-compressed data member needs dpkg >= 1.21.18 to be
# unpacked at all, which would exclude bookworm in exchange for nothing this package
# needs. xz has been accepted for data.tar since dpkg 1.15.6 and for control.tar since
# 1.17.6, so it is the choice that costs no user anything.
#
# The tar is written to a file and compressed as a second step rather than piped straight
# into xz. POSIX sh has no `pipefail`: in `bsdtar ... | xz > out`, a bsdtar that dies
# leaves xz compressing nothing, xz exits 0, `set -e` sees success, and the .deb ships with
# an empty data member that `ar t` is perfectly happy about. Making bsdtar the last command
# of its own pipeline is what puts its exit status back where `set -e` can see it.
tarball() {   # tarball <tree> <output.tar.xz>
  ( cd "$1" && find . | LC_ALL=C sort | bsdtar $TARFLAGS -cnf "$W/staged.tar" -T - )
  xz -9 -c "$W/staged.tar" > "$2"
  rm -f "$W/staged.tar"
}
tarball "$W/control" "$W/control.tar.xz"
tarball "$W/data" "$W/data.tar.xz"

printf '2.0\n' > "$W/debian-binary"

# ------------------------------------------------------------------------ the `ar` ----

# THE ar STEP, BY HAND. `ar`(1) is installed and could nearly write this, but its GNU
# dialect appends a slash to every member name and a .deb's names carry none — so the
# archive would have to be written and then corrected, which is more work than writing it.
# The whole format is a magic line and 60 bytes of header per member:
#
#     name  16   mtime 12   uid 6   gid 6   mode 8   size 10   magic 2
#
# All fields decimal, left-justified, space-padded; the magic is 0x60 0x0A. Members are
# padded to an even length with a newline, and the size field records the *unpadded*
# size. The archive opens with `!<arch>\n`. A .deb holds exactly three members in exactly
# this order: debian-binary, control.tar.xz, data.tar.xz.
#
# The trailing-slash question — the SysV dialect that distinguishes `control.tar.xz/` from
# `control.tar.xz` — does not arise here, because writing the header by hand removes it.
mkdir -p "$OUT"
DEB="$OUT/indium_${VERSION}-${REVISION}_${ARCH}.deb"

member() {   # member <file> <name-in-archive>
  sz=$(wc -c < "$1" | tr -d ' ')
  # \140 is the backtick, the first byte of the two-byte header magic.
  printf '%-16s%-12s%-6s%-6s%-8s%-10s\140\n' "$2" "$EPOCH" 0 0 100644 "$sz" >> "$DEB"
  cat "$1" >> "$DEB"
  [ $((sz % 2)) -eq 0 ] || printf '\n' >> "$DEB"
}

printf '!<arch>\n' > "$DEB"
member "$W/debian-binary"  debian-binary
member "$W/control.tar.xz" control.tar.xz
member "$W/data.tar.xz"    data.tar.xz

# The cheapest possible proof that the headers are well formed: a reader that is not this
# script walks them and prints the three names back. If a size field or the magic were
# wrong, `ar` would stop here rather than at somebody's install.
echo "--- ar t $DEB"
ar t "$DEB"
echo "--- indium $VERSION-$REVISION, $ARCH, ${INSTALLED} KiB installed"
echo "--- libc6 (>= $LIBC_MIN), libgcc-s1 (>= $GCC_MIN)"
ls -l "$DEB"
