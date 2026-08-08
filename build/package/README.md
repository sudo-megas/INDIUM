# The packages

CORE §8 names two, and only two: *"Packages: `.pkg.tar.zst` (Arch) and `.deb`
(Debian/Ubuntu), from P6 onward. Nothing else — no AppImage, no Flatpak, no Snap."*
This directory is both of them.

| File | What it does |
| --- | --- |
| `PKGBUILD` | The Arch package. Builds from the pushed git tag, not from the working tree. |
| `make-deb.sh` | The Debian package. Assembles a `.deb` around a binary it is handed. |
| `verify.sh` | Checks a built package before it ships. A package missing an icon size fails here. |
| `deb/` | What `make-deb.sh` builds the `.deb` from. |

Neither package carries its own copy of what INDIUM installs. Both call
`build/install-payload.sh`, which is also what the dev machine runs, so there is one
icon-size list in the tree and no way for a package and a dev install to drift apart.

## The one thing to know first

**Nothing that ships is built on the maker's machine.** From P8 onward
`.github/workflows/release.yml` builds all three release artefacts in containers — the
`.deb` in `debian:bookworm`, the `.pkg.tar.zst` in `archlinux:base-devel`, and the plain
tarball off the same binary the `.deb` wraps. Everything below still works by hand and is
the right way to test a change to this directory; the result is not a release artefact.

The `.deb` had to move first, and for a reason that has nothing to do with tidiness. This
is an Arch machine and its glibc floor is too high to be a Debian build host: the release
binary's dynamic imports reach `GLIBC_2.43` — `acosf` and `atan2f`, versioned by the local
glibc 2.44 — and Debian bookworm ships 2.36. A `.deb` built from an Arch-compiled binary
installs and then refuses to start. `make-deb.sh` takes the binary as an argument
precisely so it does not care where that binary came from.

The `.pkg.tar.zst` never had that problem — an Arch package built on Arch is exactly
right, and `makepkg` inside `archlinux:base-devel` is the same `makepkg`. It moved for the
other reason: a machine is a variable and a container is a written-down constant, so the
workflow file is now the whole provenance of every byte a user receives.

## By hand

```sh
( cd build/package && makepkg -f )         # .pkg.tar.zst, from the tag
./build/package/make-deb.sh <binary> [outdir]
./build/package/verify.sh
```

`makepkg` clones the tag named in `source=`, so the tag has to be pushed before this
works — a release is built from what GitHub has, never from uncommitted local state. The
workflow's `arch` job rewrites that fragment to the commit it was dispatched on, and only
on a dispatch, so a rehearsal has something to build before the tag exists; a tag run
builds the tag.

`makepkg` also runs `check()`, which is `cargo test` and `build/check-deps.sh`: CORE §2's
toolkit gate fails the build before there is anything to install, so no package can be
produced from a binary that grew GTK.

## Both at once

`verify.sh` takes `INDIUM_PKG` and `INDIUM_DEB` so it can judge artefacts built somewhere
other than this tree, and the workflow's third job is the first place both packages have
ever existed at the same moment. That is where the assertions worth the most are made —
`tests/package_path.rs`, run with `cargo test --test package_path -- --ignored`, opens
both packages *with INDIUM's own reader* and holds them to putting identical files under
`usr/`, differing only where Arch and Debian each command their own paperwork, and to
neither desktop entry so much as mentioning RAR.

`PKGEXT='.pkg.tar.zst'` is already the stock makepkg default, and nothing here overrides
it. The format CORE §8 names is what an untouched machine already produces.

## What is deliberately absent

**No `.install` file.** Arch ships `update-desktop-database.hook` and
`gtk-update-icon-cache.hook` in `/usr/share/libalpm/hooks/`, triggered on
`usr/share/applications/*.desktop` and `usr/share/icons/*/` — both paths this package
writes. An `.install` calling those two binaries would duplicate a hook that already
exists. `build/install-desktop.sh` runs them by hand only because `~/.local/share` has
no hooks watching it.

**No AppImage, Flatpak or Snap**, per CORE §9. Not now and not later.
