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

**The shippable `.deb`'s binary is not built here.** This is an Arch machine and its
glibc floor is too high to be a Debian build host. The release binary's dynamic imports
reach `GLIBC_2.43` — `acosf` and `atan2f`, versioned by the local glibc 2.44 — and Debian
bookworm ships 2.36. A `.deb` built from an Arch-compiled binary installs and then
refuses to start.

So the `.deb` that reaches users is built by `.github/workflows/release.yml` inside a
`debian:bookworm` container, and `make-deb.sh` takes the binary as an argument precisely
so it does not care where that binary came from. Running it locally is a legitimate way
to test the packaging; the result is not a release artefact.

The `.pkg.tar.zst` has no such problem — an Arch package built on Arch is exactly right.

## By hand

```sh
( cd build/package && makepkg -f )         # .pkg.tar.zst, from the tag
./build/package/make-deb.sh <binary> [outdir]
./build/package/verify.sh
```

`makepkg` clones the tag named in `source=`, so the tag has to be pushed before this
works — a release is built from what GitHub has, never from uncommitted local state.
`makepkg` also runs `check()`, which is `cargo test` and `build/check-deps.sh`: CORE §2's
toolkit gate fails the build before there is anything to install, so no package can be
produced from a binary that grew GTK.

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
