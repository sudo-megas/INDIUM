#!/bin/sh
# INDIUM — package verification. CORE §8 ships two packages and nothing else; this script
# is what stands between a built one and a release.
#
# The division of labour is deliberate, and it is the one P3 Deviation 7 and P5 Deviation
# 10 already drew: a step that touches the maker's real system — `pacman -U`, `dpkg -i`,
# the MIME associations that follow one — is his to run, by hand, with the reason recorded
# rather than the box silently ticked. So: **this script proves package CONTENTS; the
# maker proves INSTALLATION.** Nothing here installs anything and nothing here needs root.
#
# Run from the repository root:
#
#   ./build/package/verify.sh
#
# Artefacts are located by `Cargo.toml`'s version, which is also how a package that
# disagrees with the tree it came from gets caught rather than shipped. `INDIUM_PKG` and
# `INDIUM_DEB` override the two paths, because the shippable .deb is not built here at
# all — this is an Arch machine, its glibc floor is far above Debian's, and CI builds that
# one inside a bookworm container (see the README beside this file).
#
# The three outcomes are distinct and mean different things:
#
#   OK    the check ran and passed
#   SKIP  the artefact is not there yet; the message names the command that makes it
#   FAIL  the artefact is there and wrong — or a tool the check needs is missing, which
#         is a failure and not a skip, because a gate that skips proves nothing
set -e

[ -f Cargo.toml ] && [ -f CORE.md ] || {
  echo "FAIL: run this from the repository root" >&2
  exit 2
}

pass=0
fails=0
skips=0
ok()   { pass=$((pass + 1));   echo "OK: $*"; }
bad()  { fails=$((fails + 1)); echo "FAIL: $*"; }
nope() { skips=$((skips + 1)); echo "SKIP: $*"; }

# A tool the checks need. Absent, the check that wanted it fails.
need() {
  command -v "$1" >/dev/null 2>&1 && return 0
  bad "$1 is not installed — $2 cannot be verified"
  return 1
}

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT INT HUP TERM

ver=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -1)
[ -n "$ver" ] || { echo "FAIL: no version in Cargo.toml" >&2; exit 2; }

# The package revision, from the one line that holds it. Hardcoding `-1` here meant that
# the day a revision was bumped, both default paths pointed at artefacts that no longer
# existed and every check that opens a package would have gone to SKIP — which reads as
# "nothing wrong" and is the worst way for a check to fail.
rel=$(sed -n 's/^pkgrel=\([0-9][0-9]*\)$/\1/p' build/package/PKGBUILD | head -1)
[ -n "$rel" ] || { echo "FAIL: no pkgrel in build/package/PKGBUILD" >&2; exit 2; }

BIN=target/release/indium
PKG=${INDIUM_PKG:-build/package/indium-${ver}-${rel}-x86_64.pkg.tar.zst}
DEB=${INDIUM_DEB:-build/package/out/indium_${ver}-${rel}_amd64.deb}

# Debian 12 bookworm ships glibc 2.36, and it is the oldest release the .deb targets.
# Overridable, because the floor moves when the target does — and only then.
TARGET_GLIBC=${DEB_TARGET_GLIBC:-2.36}

echo "INDIUM package verification — version $ver-$rel"
echo "  pkg: $PKG"
echo "  deb: $DEB"

# Find an artefact, and tell absence apart from wrongness. An overridden path that does
# not exist is a failure: the caller pointed at it, so being wrong about it matters.
# A differently-named artefact sitting in the same directory is a failure too — it is
# present, and it is not the one this tree describes.
locate() {
  path=$1 overridden=$2 glob=$3 make=$4
  if [ -f "$path" ]; then
    return 0
  elif [ -n "$overridden" ]; then
    bad "$path does not exist (path came from the environment)"
    return 1
  else
    other=$(ls $glob 2>/dev/null | head -1 || true)
    if [ -n "$other" ]; then
      bad "expected $path, found $other — the package and Cargo.toml disagree about the version"
    else
      nope "no $path — \`$make\` makes it"
    fi
    return 1
  fi
}

# `sort -V` and not a string compare: "2.9" is greater than "2.36" alphabetically and
# smaller in every way that matters here.
ver_le() { [ "$(printf '%s\n%s\n' "$1" "$2" | sort -V | head -1)" = "$1" ]; }


echo
echo "-- 1. the desktop entry is valid"

# CORE §8: installers create the launcher entry. desktop-file-validate is the reference
# implementation's own parser, it is installed on this machine, and so this is a real
# gate rather than an aspiration: an entry that fails here fails in every launcher.
if need desktop-file-validate "the desktop entry"; then
  if out=$(desktop-file-validate assets/org.indium.desktop 2>&1) && [ -z "$out" ]; then
    ok "desktop-file-validate assets/org.indium.desktop"
  else
    bad "desktop-file-validate: $out"
  fi
fi


echo
echo "-- 2. MimeType matches CORE §8 exactly"

# CORE §8 names ten types verbatim and then "the compressed-tar aliases", which are the
# four below — the same four `build/install-desktop.sh` hands to `xdg-mime default`.
# Fourteen, no more and no fewer, and set-wise: an extra type is as wrong as a missing
# one, because the entry is a promise about what INDIUM opens.
cat > "$tmp/mime.want" <<'EOF'
application/gzip
application/x-7z-compressed
application/x-bzip2
application/x-bzip2-compressed-tar
application/x-compressed-tar
application/x-cpio
application/x-iso9660-image
application/x-lzip
application/x-tar
application/x-xz
application/x-xz-compressed-tar
application/x-zstd-compressed-tar
application/zip
application/zstd
EOF
sort -o "$tmp/mime.want" "$tmp/mime.want"

mimeline=$(grep '^MimeType=' assets/org.indium.desktop || true)
if [ -z "$mimeline" ]; then
  bad "no MimeType= line in assets/org.indium.desktop"
else
  # The list ends in `;` as the spec requires, which leaves one empty field behind it.
  printf '%s' "${mimeline#MimeType=}" | tr ';' '\n' | grep -v '^$' | sort > "$tmp/mime.got"
  missing=$(comm -23 "$tmp/mime.want" "$tmp/mime.got" | tr '\n' ' ')
  extra=$(comm -13 "$tmp/mime.want" "$tmp/mime.got" | tr '\n' ' ')
  if [ -z "$missing" ] && [ -z "$extra" ]; then
    ok "MimeType lists CORE §8's fourteen types and nothing else"
  else
    [ -n "$missing" ] && bad "MimeType is missing: $missing"
    [ -n "$extra" ] && bad "MimeType carries types CORE §8 does not list: $extra"
  fi
fi

# CORE §5 and §9: "No RAR — not read, not written." A registration is a claim to open the
# format, so it gets a line of its own rather than being left to the set comparison above.
#
# Scoped to the MimeType value and not the whole file, deliberately: this project writes
# down what it deliberately leaves out, and the desktop entry's own header already points
# at the sections that do it. A comment saying RAR is absent must not read as RAR being
# present.
if printf '%s' "$mimeline" | grep -qi 'rar'; then
  bad "MimeType registers a rar type — CORE §9 forbids it: $mimeline"
else
  ok "no rar type registered"
fi


have_bin=0
[ -f "$BIN" ] && have_bin=1

echo
echo "-- 3. ELF facts on the release binary"

# Every one of these is a default of the Rust toolchain on x86_64-unknown-linux-gnu, and
# that is precisely why they are asserted here instead of configured anywhere: nothing in
# this repository turns them on, so nothing in this repository would notice them going
# away. A toolchain change, a stray RUSTFLAGS, a `-C link-arg` added for some other
# reason — this is where that shows up.
#
# `build/check-deps.sh` asserts the first two of them, because it runs before every
# release and inside makepkg's check(). The full set is here, where a release artefact is
# what is being judged rather than a build.
if [ "$have_bin" = 0 ]; then
  nope "no $BIN — \`cargo build --release\` makes it (checks 3, 4 and 10)"
elif need readelf "the ELF hardening facts"; then
  etype=$(readelf -h "$BIN" 2>/dev/null | sed -n 's/^  Type:[[:space:]]*\([A-Z]*\).*/\1/p' || true)
  if [ "$etype" = "DYN" ]; then
    ok "Type: DYN — position-independent executable"
  else
    bad "Type: ${etype:-unreadable} — not a PIE"
  fi

  if dyn=$(readelf -W -d "$BIN" 2>&1); then
    # The dynamic FLAGS carry BIND_NOW and FLAGS_1 carries NOW; either says the same
    # thing, that every relocation is resolved before main and the GOT can go read-only.
    if printf '%s\n' "$dyn" | grep -q 'BIND_NOW'; then
      ok "BIND_NOW — full RELRO"
    else
      bad "no BIND_NOW in the dynamic section"
    fi

    # A RUNPATH or RPATH in a package is a library search path burned into a binary that
    # a distribution's linker is supposed to own. There is no reason for one here.
    if printf '%s\n' "$dyn" | grep -qE '\((RUNPATH)\)'; then
      bad "RUNPATH present: $(printf '%s\n' "$dyn" | grep '(RUNPATH)')"
    else
      ok "no RUNPATH"
    fi
    if printf '%s\n' "$dyn" | grep -qE '\((RPATH)\)'; then
      bad "RPATH present: $(printf '%s\n' "$dyn" | grep '(RPATH)')"
    else
      ok "no RPATH"
    fi

    # TEXTREL means relocations write into executable pages, which defeats the point of
    # having a PIE at all.
    if printf '%s\n' "$dyn" | grep -q 'TEXTREL'; then
      bad "TEXTREL present — relocations write into the text segment"
    else
      ok "no TEXTREL"
    fi
  else
    bad "readelf -d failed: $dyn"
  fi

  # PT_GNU_STACK must exist *and* lack E. A missing header is not a pass by silence: the
  # kernel's fallback for an ELF with no PT_GNU_STACK is an executable stack.
  gs=$(readelf -W -l "$BIN" 2>/dev/null | grep -m1 'GNU_STACK' || true)
  if [ -z "$gs" ]; then
    bad "no PT_GNU_STACK header — the stack would be executable by default"
  else
    # Flags sit between MemSiz and Align, and readelf prints them as either `RW` or
    # `R E`, so they are joined rather than read as one field.
    gsflags=$(printf '%s\n' "$gs" | awk '{ f = ""; for (i = 7; i < NF; i++) f = f $i; print f }')
    case "$gsflags" in
      *E*) bad "GNU_STACK is $gsflags — executable stack" ;;
      *)   ok "GNU_STACK $gsflags — non-executable stack" ;;
    esac
  fi

  # Cargo.toml's [profile.release] sets strip = true. `not stripped` is tested first
  # because it contains the word the pass would match on.
  if need file "the strip check"; then
    finfo=$(file -b "$BIN" || true)
    case "$finfo" in
      *"not stripped"*) bad "binary is not stripped" ;;
      *", stripped"*)   ok "stripped" ;;
      *)                bad "file says neither stripped nor not stripped: $finfo" ;;
    esac
  fi
fi


echo
echo "-- 4. the glibc floor"

# THE gate. Everything else in this file catches a package that is untidy; this one
# catches a package that does not work. A .deb built from a binary compiled against a
# newer glibc than the target's installs perfectly, satisfies its dependencies, and then
# dies at exec with `version 'GLIBC_2.43' not found` — the failure arrives after the
# user has already been told the software is installed. Nothing about the .deb's own
# contents reveals it. This line does.
#
# The floor is computed exactly as make-deb.sh computes the `libc6 (>= x.y)` it writes,
# so the two can never disagree about what the binary needs.
#
# WEAK symbols are excluded deliberately. Rust imports `pidfd_spawnp@GLIBC_2.39` weakly
# and resolves it to NULL where the symbol is absent, falling back to fork/exec at
# runtime; counting a symbol the binary is built to live without would overstate the
# floor by three glibc releases and refuse a .deb that would have run.
# Which binary to measure, and it matters more than it looks. The gate exists to judge the
# binary that SHIPS, and that is the one inside the .deb. In CI the two are the same file —
# the container builds `target/release/indium` and packages it in the same job, which is why
# this gate is sound at the only moment it decides anything. But verifying a .deb built
# elsewhere, on a machine whose own binary has a different floor, would otherwise measure
# the local build and report a number belonging to no shipped artefact at all. So: if there
# is a .deb, its own `usr/bin/indium` is the subject, and the tree's binary is the fallback.
gate_bin=$BIN
gate_what="the release binary"
# `[ -f "$DEB" ]` rather than the have_deb flag: that flag is set by locate() in check 6,
# which has not run yet, so testing it here would silently never fire.
if [ -f "$DEB" ] && command -v ar >/dev/null 2>&1 && command -v bsdtar >/dev/null 2>&1; then
  if ar p "$DEB" data.tar.xz 2>/dev/null |
     bsdtar xOf - ./usr/bin/indium > "$tmp/gate-bin" 2>/dev/null && [ -s "$tmp/gate-bin" ]; then
    gate_bin=$tmp/gate-bin
    gate_what="the binary inside the .deb"
  fi
fi
echo "      measuring: $gate_what"

if [ "$gate_bin" = "$BIN" ] && [ "$have_bin" = 0 ]; then
  nope "no $BIN and no .deb — the glibc floor cannot be computed"
elif need readelf "the glibc floor"; then
  # $gate_bin throughout, never $BIN: reassigning $BIN here would leak into the hint text
  # of later checks and name a temporary file as the thing to rebuild.
  floor=$(readelf -W --dyn-syms "$gate_bin" 2>/dev/null |
    awk '$5=="GLOBAL" && $7=="UND" && $8 ~ /GLIBC_/ {print $8}' |
    sed 's/.*@GLIBC_//' | sort -uV | tail -1)
  if [ -z "$floor" ]; then
    # An empty floor would sail through the comparison below and prove nothing, so it is
    # a failure in its own right: either the wrong file was read or readelf's column
    # layout moved under the awk.
    bad "no GLIBC_ versions found in $gate_what — wrong file, or readelf output has changed shape"
  elif ver_le "$floor" "$TARGET_GLIBC"; then
    ok "glibc floor $floor <= target $TARGET_GLIBC"
    sym=$(readelf -W --dyn-syms "$gate_bin" 2>/dev/null |
      awk -v f="GLIBC_$floor" '$5=="GLOBAL" && $7=="UND" && $8 ~ f {print $8}' | tr '\n' ' ')
    [ -n "$sym" ] && echo "      floor set by: $sym"
  else
    sym=$(readelf -W --dyn-syms "$gate_bin" 2>/dev/null |
      awk -v f="GLIBC_$floor" '$5=="GLOBAL" && $7=="UND" && $8 ~ f {print $8}' | tr '\n' ' ')
    bad "glibc floor $floor exceeds target $TARGET_GLIBC — a .deb from this binary would install and then fail at exec"
    echo "      floor set by: $sym"
    echo "      build the shippable binary on the target, or raise DEB_TARGET_GLIBC deliberately"
  fi
fi


echo
echo "-- 5. the Arch package"

if locate "$PKG" "$INDIUM_PKG" 'build/package/*.pkg.tar.zst' '(cd build/package && makepkg -f)' &&
   need bsdtar "the .pkg.tar.zst"; then
  if bsdtar tf "$PKG" 2>/dev/null | sed 's|^\./||' | sort > "$tmp/pkg.list" &&
     bsdtar xOf "$PKG" .PKGINFO > "$tmp/PKGINFO" 2>/dev/null; then

    for field in "pkgname = indium" "pkgver = ${ver}-1" "arch = x86_64"; do
      if grep -qx "$field" "$tmp/PKGINFO"; then
        ok ".PKGINFO $field"
      else
        bad ".PKGINFO has no '$field' (found: $(grep "^${field%% *} = " "$tmp/PKGINFO" | tr '\n' ' '))"
      fi
    done

    # CORE §2's system-library table, as pacman spells it. Compared set-wise: a seventh
    # dependency would mean the table and the package have parted company.
    printf '%s\n' glibc libarchive libgcc libglvnd libxkbcommon wayland | sort > "$tmp/dep.want"
    sed -n 's/^depend = //p' "$tmp/PKGINFO" | sort > "$tmp/dep.got"
    if cmp -s "$tmp/dep.want" "$tmp/dep.got"; then
      ok ".PKGINFO depends on exactly CORE §2's six: $(tr '\n' ' ' < "$tmp/dep.got")"
    else
      bad ".PKGINFO depends are wrong — want: $(tr '\n' ' ' < "$tmp/dep.want")/ got: $(tr '\n' ' ' < "$tmp/dep.got")"
    fi

    # CORE §8: GPL-3.0-only for the program, OFL-1.1 for the bundled font. Two, both named.
    nlic=$(grep -c '^license = ' "$tmp/PKGINFO" || true)
    if [ "$nlic" = 2 ] &&
       grep -qi '^license = .*GPL-3\.0' "$tmp/PKGINFO" &&
       grep -qi '^license = .*OFL' "$tmp/PKGINFO"; then
      ok ".PKGINFO declares both licences: $(sed -n 's/^license = //p' "$tmp/PKGINFO" | tr '\n' ' ')"
    else
      bad ".PKGINFO licences are wrong ($nlic lines): $(sed -n 's/^license = //p' "$tmp/PKGINFO" | tr '\n' ' ')"
    fi

    # build/install-payload.sh tolerates a missing icon and installs what it finds, which
    # is right for a dev machine and wrong for a release. This is where that tolerance
    # ends, in the words of that script's own comment.
    absent=
    for size in 16 22 24 32 48 64 96 128 256 512; do
      grep -qx "usr/share/icons/hicolor/${size}x${size}/apps/indium.png" "$tmp/pkg.list" ||
        absent="$absent $size"
    done
    if [ -z "$absent" ]; then
      ok "all ten hicolor icon sizes present"
    else
      bad "icon sizes missing from the package:$absent"
    fi

    if grep -qx 'usr/share/applications/org.indium.desktop' "$tmp/pkg.list"; then
      ok "desktop entry present"
    else
      bad "no usr/share/applications/org.indium.desktop in the package"
    fi

    lic=$(grep '^usr/share/licenses/indium/.' "$tmp/pkg.list" || true)
    if printf '%s\n' "$lic" | grep -qi 'license\|copying\|gpl' &&
       printf '%s\n' "$lic" | grep -qi 'ofl'; then
      ok "both licence files installed: $(printf '%s ' $lic)"
    else
      bad "usr/share/licenses/indium/ does not hold both licences: $(printf '%s ' $lic)"
    fi
  else
    bad "$PKG could not be read as a package archive"
  fi
fi


echo
echo "-- 6. the .deb is an ar archive of three members"

deb_ok=0
if locate "$DEB" "$INDIUM_DEB" 'build/package/out/*.deb' "./build/package/make-deb.sh $BIN"; then
  if need ar "the .deb"; then
    # A .deb is an ar archive whose members are these three, in this order. dpkg reads
    # `debian-binary` first, and the order is part of the format rather than a convention.
    #
    # Which members exist and what order they sit in are two separate failures, because
    # `ar p` pulls a member out regardless of where it is. A .deb with the right contents
    # in the wrong order should be told everything that is wrong with it in one run, not
    # have twenty content checks hidden behind one ordering fault.
    members=$(ar t "$DEB" 2>/dev/null | tr '\n' ' ' || true)
    sorted=$(ar t "$DEB" 2>/dev/null | sort | tr '\n' ' ' || true)
    if [ "$sorted" != "control.tar.xz data.tar.xz debian-binary " ]; then
      bad "ar members are '$members' — want exactly debian-binary, control.tar.xz, data.tar.xz"
    else
      deb_ok=1
      if [ "$members" = "debian-binary control.tar.xz data.tar.xz " ]; then
        ok "ar members in order: $members"
      else
        bad "ar members are in the wrong order: '$members'"
      fi
    fi
  fi
fi


echo
echo "-- 7. the .deb control file"

ctrl=
data_ok=0
# Both members are unpacked here, before any of them is judged, so that a fault in one
# cannot turn the checks on the other into skips — the same reason check 6 tells a missing
# member apart from a misordered one. `ar p` writes to stdout, so nothing depends on the
# working directory.
if [ "$deb_ok" = 1 ] && need bsdtar "the .deb members"; then
  mkdir -p "$tmp/control" "$tmp/data"
  if ar p "$DEB" control.tar.xz > "$tmp/control.tar.xz" 2>/dev/null &&
     bsdtar xf "$tmp/control.tar.xz" -C "$tmp/control" 2>/dev/null; then
    ctrl=$(find "$tmp/control" -name control -type f | head -1)
  fi
  if ar p "$DEB" data.tar.xz > "$tmp/data.tar.xz" 2>/dev/null &&
     bsdtar xf "$tmp/data.tar.xz" -C "$tmp/data" 2>/dev/null; then
    data_ok=1
  fi
fi

if [ "$deb_ok" != 1 ]; then
  nope "no readable .deb — the control file cannot be checked"
elif [ -z "$ctrl" ]; then
  bad "control.tar.xz holds no control file"
else
  field() { sed -n "s/^$1: //p" "$ctrl" | head -1; }

  for f in "Package:indium" "Version:${ver}-${rel}" "Architecture:amd64" \
           "Section:utils" "Priority:optional"; do
    name=${f%%:*} want=${f#*:} got=$(field "${f%%:*}")
    if [ "$got" = "$want" ]; then
      ok "control $name: $got"
    else
      bad "control $name is '$got', want '$want'"
    fi
  done

  # CORE §8: releases come from the sudo-megas account and no other.
  maint=$(field Maintainer)
  case "$maint" in
    *sudo-megas*) ok "control Maintainer: $maint" ;;
    "")           bad "control has no Maintainer" ;;
    *)            bad "control Maintainer is '$maint' — CORE §8 names sudo-megas" ;;
  esac

  home=$(field Homepage)
  case "$home" in
    http*INDIUM*) ok "control Homepage: $home" ;;
    *)            bad "control Homepage is '$home'" ;;
  esac

  # Installed-Size is in kibibytes and dpkg's front ends do arithmetic on it, so a
  # non-numeric value is not a cosmetic problem.
  isize=$(field Installed-Size)
  case "$isize" in
    ''|*[!0-9]*) bad "control Installed-Size is '$isize' — not a plain number" ;;
    *)           ok "control Installed-Size: $isize KiB" ;;
  esac

  # CORE §2's six system libraries, under the names Debian gives them. What is asserted is
  # that each library is declared, not how it is spelled: a version relation is stripped,
  # and `a | b` alternatives are read as the alternatives they are, because Debian's
  # 64-bit-time_t transition renamed libarchive13 to libarchive13t64 mid-suite and a
  # package that spans both suites has to say so.
  deps=$(field Depends)
  if [ -z "$deps" ]; then
    bad "control has no Depends"
  else
    printf '%s\n' "$deps" | tr ',|' '\n\n' |
      sed 's/([^)]*)//g; s/[[:space:]]//g' | grep -v '^$' > "$tmp/deb.deps"
    dmissing=
    for pat in 'libc6' 'libgcc-s1' 'libarchive13t64|libarchive13' \
               'libwayland-client0' 'libxkbcommon0' 'libegl1|libgl1|libglvnd0'; do
      grep -qxE "$pat" "$tmp/deb.deps" || dmissing="$dmissing [$pat]"
    done
    if [ -z "$dmissing" ]; then
      ok "control Depends covers CORE §2's six: $deps"
    else
      bad "control Depends declares nothing for:$dmissing"
      echo "      Depends: $deps"
    fi
  fi

  # Debian Policy 3.4: the synopsis is a phrase, not a sentence — no article in front, no
  # full stop behind, and short enough for `apt search` to print in one column.
  syn=$(field Description)
  len=$(printf '%s' "$syn" | wc -c)
  synbad=
  [ -n "$syn" ] || synbad="empty"
  [ "$len" -lt 64 ] || synbad="$synbad ${len}chars"
  case "$syn" in *.) synbad="$synbad trailing-full-stop" ;; esac
  case "$syn" in [Aa]\ *|[Aa]n\ *|[Tt]he\ *) synbad="$synbad leading-article" ;; esac
  if [ -z "$synbad" ]; then
    ok "control synopsis ($len chars): $syn"
  else
    bad "control synopsis breaks Policy 3.4 ($synbad): $syn"
  fi
fi


echo
echo "-- 8. md5sums covers the payload, hash for hash"

# The point of re-hashing rather than counting lines: an md5sums file that lists the right
# names and the wrong digests is exactly as broken as one with a file missing, and dpkg
# will not tell anybody until `debsums` is run years later.
if [ "$deb_ok" != 1 ] || [ -z "$ctrl" ]; then
  nope "no readable .deb control member — md5sums cannot be checked"
else
  sums=$(dirname "$ctrl")/md5sums
  if [ ! -f "$sums" ]; then
    bad "control.tar.xz holds no md5sums"
  elif [ "$data_ok" = 0 ]; then
    bad "data.tar.xz could not be extracted"
  else
    # Compared as two sets of paths and not as two counts: a duplicated line would make
    # the counts agree while a payload file went uncovered, and `md5sum -c` would not
    # notice, because it verifies what it is given rather than what is there.
    find "$tmp/data" -type f | sed "s|^$tmp/data/||" | sort -u > "$tmp/data.files"
    sed 's/^[^ ]*  //; s|^\./||' "$sums" | grep -v '^$' > "$tmp/sums.raw"
    dups=$(sort "$tmp/sums.raw" | uniq -d | tr '\n' ' ')
    sort -u "$tmp/sums.raw" > "$tmp/sums.files"
    unlisted=$(comm -23 "$tmp/data.files" "$tmp/sums.files" | tr '\n' ' ')
    phantom=$(comm -13 "$tmp/data.files" "$tmp/sums.files" | tr '\n' ' ')
    if [ -z "$unlisted" ] && [ -z "$phantom" ] && [ -z "$dups" ]; then
      ok "md5sums has one line per regular file ($(grep -c . "$tmp/data.files"))"
    else
      [ -n "$unlisted" ] && bad "in data.tar.xz but not in md5sums: $unlisted"
      [ -n "$phantom" ] && bad "in md5sums but not in data.tar.xz: $phantom"
      [ -n "$dups" ] && bad "md5sums lists a path more than once: $dups"
    fi

    # Policy: the paths are relative to the filesystem root, so `usr/bin/indium` and
    # never `./usr/bin/indium`. `md5sum -c` accepts both, so the leading `./` has to be
    # rejected here or it is rejected nowhere.
    dotted=$(grep -c '  \./' "$sums" || true)
    if [ "$dotted" = 0 ]; then
      ok "md5sums paths are relative, with no leading ./"
    else
      bad "$dotted md5sums paths begin with ./"
    fi

    if out=$(cd "$tmp/data" && md5sum -c "$sums" 2>&1); then
      ok "every md5sum verifies against the extracted tree"
    else
      bad "md5sum -c failed:"
      printf '%s\n' "$out" | grep -v ': OK$' | sed 's/^/      /'
    fi
  fi
fi


echo
echo "-- 9. the Debian changelog"

if [ "$deb_ok" != 1 ] || [ "$data_ok" = 0 ]; then
  nope "no extracted .deb payload — the changelog cannot be checked"
else
  cl=$(find "$tmp/data" -name 'changelog.Debian.gz' | head -1)
  if [ -z "$cl" ]; then
    bad "no changelog.Debian.gz in the package payload"
  elif ! first=$(gzip -dc "$cl" 2>/dev/null | head -1); then
    bad "changelog.Debian.gz does not decompress"
  else
    want="indium (${ver}-${rel}) unstable; urgency=medium"
    if [ "$first" = "$want" ]; then
      ok "changelog first line: $first"
    else
      bad "changelog first line is '$first', want '$want'"
    fi

    # Bytes 4-7 of a gzip member are MTIME. `gzip -9n` zeroes them; plain `gzip -9`
    # writes the source file's timestamp, which makes the .deb differ from itself
    # between two builds of identical content. Reproducibility here costs one flag, and
    # this is the check that notices when the flag is dropped.
    mtime=$(od -An -tu1 -j4 -N4 "$cl" | tr -s ' ' | sed 's/^ *//;s/ *$//')
    if [ "$mtime" = "0 0 0 0" ]; then
      ok "changelog gzip MTIME is zeroed — gzip -9n held"
    else
      bad "changelog gzip MTIME bytes are '$mtime', not zero — gzip -9n was not used"
    fi
  fi
fi


echo
echo "-- 10. the CORE §2 gate"

# CORE §2: "No GTK. No Qt. No KF6. No portal." The gate lives in its own script and is
# called rather than reimplemented, so there is one definition of what INDIUM may link and
# a release cannot pass a rule the build was held to.
if [ "$have_bin" = 0 ]; then
  nope "no $BIN — \`cargo build --release\` makes it"
elif out=$(./build/check-deps.sh 2>&1); then
  ok "check-deps.sh — $(printf '%s\n' "$out" | grep '^OK:' | sed 's/^OK: //' | tr '\n' ';' | sed 's/;/; /g;s/; $//')"
else
  bad "check-deps.sh failed:"
  printf '%s\n' "$out" | sed 's/^/      /'
fi


echo
echo "-- summary: $pass passed, $fails failed, $skips skipped"
if [ "$fails" -gt 0 ]; then
  echo "NOT SHIPPABLE"
  exit 1
fi
if [ "$skips" -gt 0 ]; then
  echo "Contents verified as far as the built artefacts allow."
else
  echo "Contents verified. Installation is the maker's to run by hand."
fi
