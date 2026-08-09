//! The packages, read by INDIUM itself — P6.
//!
//! CORE §8: "Packages: `.pkg.tar.zst` (Arch) and `.deb` (Debian/Ubuntu), from P6 onward.
//! Nothing else." This file checks the shipped artefacts rather than the scripts that
//! build them, and it does so through `indium::arch` — the program's own reader.
//! `Reader::open` calls `archive_read_support_format_all` and
//! `archive_read_support_filter_all`, so libarchive reads a `.pkg.tar.zst` (a
//! zstd-filtered tar) and a `.deb` (an `ar` archive, and CORE §5 lists `ar` and `deb`
//! among the containers INDIUM reads) with no external tool anywhere in sight. The
//! packaging is verified by the thing being packaged, which is the cheapest honest test
//! available and the only one that needs no dependency at all.
//!
//! **Every test here is `#[ignore]`d.** The artefacts do not exist during an ordinary
//! `cargo test` — they are made by a release, not by a build — and a test that silently
//! passes when its subject is absent is worse than no test. The precedent is
//! `src/platform/clipboard.rs`, whose `#[ignore = "needs a live Wayland session and
//! wl-paste"]` test is run deliberately, by hand. Each attribute below names what
//! produces the artefact it wants; a test that *is* run and finds nothing panics saying
//! the same thing, so nothing here can pass by absence.
//!
//! Run them with `cargo test --test package_path -- --ignored`.
//!
//! Copyright © sudo-megas. GPL-3.0-only.

use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use indium::arch::{self, Entry};

// ---------------------------------------------------------------------------
// A temporary directory, hand-written.
//
// CORE §2's rule applies to test dependencies too: "makes a directory in /tmp for the
// tests" is not a sentence worth a crate. Same shape as `tests/read_path.rs` and
// `tests/write_path.rs`, copied rather than shared, because a test file that has to be
// read alongside another test file is harder to trust than a repeated twenty lines.
// ---------------------------------------------------------------------------

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> TempDir {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "indium-package-{}-{}-{}",
            tag,
            std::process::id(),
            n
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("could not create temp dir");
        TempDir(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

// ---------------------------------------------------------------------------
// Where the artefacts are
// ---------------------------------------------------------------------------

/// What to do when the Arch package is not there.
const PKG_HINT: &str = "run `cd build/package && makepkg -f`, or point INDIUM_PKG at the \
                        package wherever it was built";

/// What to do when the .deb is not there. The one that ships is built inside a Debian
/// container so its glibc floor is a fact rather than a promise, and lands wherever CI
/// put it — hence INDIUM_DEB as well as the command.
const DEB_HINT: &str = "run `./build/package/make-deb.sh target/release/indium`, or point \
                        INDIUM_DEB at the .deb CI built";

/// The package revision, read out of the PKGBUILD's `pkgrel` at run time.
///
/// The same rule as the version below, one line further down the same file: the number is
/// written once and read everywhere. It was a literal `1` in these two paths until P10
/// bumped the revision, at which point both defaults would have pointed at artefacts that
/// do not exist — and `artefact` panics with a hint about building one, which is a
/// confusing way to be told the path was simply out of date.
fn pkgrel() -> String {
    let pkgbuild = Path::new(env!("CARGO_MANIFEST_DIR")).join("build/package/PKGBUILD");
    let text = std::fs::read_to_string(&pkgbuild)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", pkgbuild.display()));
    text.lines()
        .find_map(|l| l.strip_prefix("pkgrel="))
        .map(|v| v.trim().to_string())
        .unwrap_or_else(|| panic!("no pkgrel in {}", pkgbuild.display()))
}

/// The Arch package: `build/package/indium-<ver>-<rel>-x86_64.pkg.tar.zst`, where makepkg
/// writes it — beside the PKGBUILD, not in a subdirectory.
///
/// The version comes from `CARGO_PKG_VERSION` so a version bump carries these tests with
/// it and there is no second copy of the number to forget — `pkgver` in the PKGBUILD is
/// the same number. Cargo's version is three-numeral (`1.0.0`) where CORE §7's tags are
/// two (`v1.0`), or carry the package revision when only the build changed (`v1.0.0-2`);
/// if the packaged version string ever parts company with Cargo's, `INDIUM_PKG` is the
/// answer, not a hand-written constant that would then be wrong in two places.
fn pkg_path() -> PathBuf {
    artefact(
        "INDIUM_PKG",
        &format!(
            "build/package/indium-{}-{}-x86_64.pkg.tar.zst",
            env!("CARGO_PKG_VERSION"),
            pkgrel()
        ),
        PKG_HINT,
    )
}

/// The Debian package: `build/package/out/indium_<ver>-<rel>_amd64.deb`, where
/// `build/package/make-deb.sh` writes it by default.
fn deb_path() -> PathBuf {
    artefact(
        "INDIUM_DEB",
        &format!(
            "build/package/out/indium_{}-{}_amd64.deb",
            env!("CARGO_PKG_VERSION"),
            pkgrel()
        ),
        DEB_HINT,
    )
}

/// An artefact path, from the environment or from the repository, and never a silent
/// pass. `var_os` rather than `var`: a path is bytes, not necessarily UTF-8.
fn artefact(var: &str, relative: &str, hint: &str) -> PathBuf {
    let path = match std::env::var_os(var) {
        Some(value) => PathBuf::from(value),
        None => Path::new(env!("CARGO_MANIFEST_DIR")).join(relative),
    };
    if !path.exists() {
        panic!("{} does not exist — {hint}", path.display());
    }
    path
}

// ---------------------------------------------------------------------------
// The payload
// ---------------------------------------------------------------------------

/// The icon sizes hicolor looks in, and the sizes `build/install-payload.sh` installs.
const ICON_SIZES: [u32; 10] = [16, 22, 24, 32, 48, 64, 96, 128, 256, 512];

const BINARY: &str = "usr/bin/indium";
const DESKTOP: &str = "usr/share/applications/org.indium.desktop";

/// The member of a `.deb` that holds everything a user gets.
const DATA_MEMBER: &str = "data.tar.xz";

fn icon_path(size: u32) -> String {
    format!("usr/share/icons/hicolor/{size}x{size}/apps/indium.png")
}

/// Every file a package puts on a machine.
///
/// `build/install-payload.sh` is the single source of truth for this list, and both
/// packages call it with their own root. Writing it down once more here, on the far side
/// of the packaging, is what turns "the script says so" into "the artefact does": a
/// package that stopped calling the script, or called it and then dropped something on
/// the way into the archive, fails here and nowhere else.
fn expected_payload() -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    out.insert(BINARY.to_string());
    out.insert(DESKTOP.to_string());
    for size in ICON_SIZES {
        out.insert(icon_path(size));
    }
    out
}

/// The payload, plus whatever else one package is entitled to carry.
fn payload_and(extra: &[&str]) -> BTreeSet<String> {
    let mut out = expected_payload();
    out.extend(extra.iter().map(|p| p.to_string()));
    out
}

// The paperwork. This is the one place the two packages differ, and neither difference is
// optional: Arch puts a package's licences in `/usr/share/licenses/$pkgname/`, and Debian
// Policy requires `/usr/share/doc/<pkg>/copyright` (§12.5) and a gzipped
// `changelog.Debian.gz` (§12.7) at those exact paths. A package without them is a broken
// package on its own system, so the difference is pinned here rather than waved away.

/// From `build/package/PKGBUILD`'s `package()`: the GPL text CORE §8 names, and the
/// font's OFL-1.1 — which, unlike the GPL, has no copy under `/usr/share/licenses/spdx/`
/// for the package to point at.
const PKG_PAPERWORK: [&str; 2] = [
    "usr/share/licenses/indium/LICENSE",
    "usr/share/licenses/indium/OFL-1.1.txt",
];

/// From `build/package/make-deb.sh`: DEP-5 copyright and the Debian changelog.
const DEB_PAPERWORK: [&str; 2] = [
    "usr/share/doc/indium/copyright",
    "usr/share/doc/indium/changelog.Debian.gz",
];

const PKG_PAPERWORK_DIR: &str = "usr/share/licenses/";
const DEB_PAPERWORK_DIR: &str = "usr/share/doc/";

/// The files an artefact ships, by normalised path.
///
/// Directories are left out deliberately. Whether a tar carries `usr/` as a member of its
/// own is a decision of whichever tool wrote it — makepkg's bsdtar does, a hand-rolled
/// `data.tar.xz` need not — and it says nothing about what lands on a machine. `e.path` is
/// the normalised form rather than `raw_path`, which is also what erases the `./` a deb's
/// data tar prefixes to every name; without it the two artefacts could not be compared.
fn files_of(entries: &[Entry]) -> BTreeSet<String> {
    entries
        .iter()
        .filter(|e| !e.is_dir)
        .map(|e| e.path.clone())
        .collect()
}

/// The same, minus pacman's own bookkeeping.
///
/// `.PKGINFO`, `.MTREE` and `.BUILDINFO` belong to pacman, not to INDIUM, and are put
/// there by makepkg rather than by anything in this repository. The rule is stated as
/// "a top-level name beginning with a dot" rather than as those three literals because
/// that is the actual convention — a PKGBUILD with an install script adds `.INSTALL`
/// too, and that would be pacman's business as well.
fn payload_of(entries: &[Entry]) -> BTreeSet<String> {
    files_of(entries)
        .into_iter()
        .filter(|p| !p.starts_with('.'))
        .collect()
}

/// Just the files that land under `/usr`, which is the part two different packaging
/// systems can be held to the same standard about.
fn under_usr(entries: &[Entry]) -> BTreeSet<String> {
    files_of(entries)
        .into_iter()
        .filter(|p| p.starts_with("usr/"))
        .collect()
}

// ---------------------------------------------------------------------------
// Small helpers
// ---------------------------------------------------------------------------

fn no_cancel() -> Arc<AtomicBool> {
    Arc::new(AtomicBool::new(false))
}

fn list(path: &Path) -> Vec<Entry> {
    arch::list_all(path, None)
        .unwrap_or_else(|e| panic!("{} could not be listed: {e}", path.display()))
}

fn find<'a>(entries: &'a [Entry], path: &str) -> &'a Entry {
    entries.iter().find(|e| e.path == path).unwrap_or_else(|| {
        panic!(
            "{path} is not there; the archive holds {:?}",
            files_of(entries)
        )
    })
}

/// Take the `.deb`'s data member out into `dir` and hand back its path.
///
/// A `.deb` is an `ar` archive holding three files, so the payload is one archive inside
/// another and INDIUM reads both halves. Extraction goes through `arch::extract` — the
/// same secure-flagged path the window uses — rather than through anything special.
fn deb_data(dir: &TempDir) -> PathBuf {
    let deb = deb_path();
    let wanted: HashSet<String> = [DATA_MEMBER.to_string()].into_iter().collect();
    let n = arch::extract(&deb, &wanted, dir.path(), None, None, &no_cancel())
        .unwrap_or_else(|e| panic!("could not take {DATA_MEMBER} out of {}: {e}", deb.display()));
    assert_eq!(n, 1, "exactly one member of the .deb should have come out");
    dir.path().join(DATA_MEMBER)
}

/// The desktop entry's bytes, read straight out of a package.
fn desktop_bytes(archive: &Path) -> Vec<u8> {
    // A kilobyte would do; 64 is a round number that still cannot be used to make the
    // test process disappear if the file is ever replaced by something enormous.
    const CAP: usize = 64 * 1024;
    let (bytes, truncated) = arch::head_of(archive, DESKTOP, CAP, None)
        .unwrap_or_else(|e| panic!("could not read {DESKTOP} out of {}: {e}", archive.display()));
    assert!(
        !truncated,
        "the desktop entry is well under a kilobyte; a truncated read would make any \
         assertion about its contents vacuous"
    );
    bytes
}

// ---------------------------------------------------------------------------
// The Arch package
// ---------------------------------------------------------------------------

#[test]
#[ignore = "needs the Arch package: cd build/package && makepkg -f (or set INDIUM_PKG)"]
fn pkg_holds_exactly_the_payload() {
    let entries = list(&pkg_path());
    assert_eq!(
        payload_of(&entries),
        payload_and(&PKG_PAPERWORK),
        "the Arch package must ship exactly what build/install-payload.sh installs plus \
         the two licences PKGBUILD's package() adds — no more, because a package that \
         carries something nobody declared is a package nobody can reason about, and no \
         less"
    );
}

/// CORE §6: "The icon is photorealistic PNG, supplied by the maker, installed at the
/// hicolor sizes provided."
///
/// `build/install-payload.sh` skips an icon size whose file is missing, on purpose, so a
/// dev machine with a partial set installs what exists instead of failing. That tolerance
/// is right for a dev machine and wrong for a release: a shipped package quietly missing
/// the 48px size — the one size freedesktop actually mandates — would show a generic icon
/// on somebody else's desktop and nowhere else. **The tolerance lives in the script; the
/// loudness lives here.**
#[test]
#[ignore = "needs the Arch package: cd build/package && makepkg -f (or set INDIUM_PKG)"]
fn pkg_ships_every_hicolor_size() {
    let files = files_of(&list(&pkg_path()));
    for size in ICON_SIZES {
        let path = icon_path(size);
        assert!(
            files.contains(&path),
            "the package is missing the {size}px icon ({path}); install-payload.sh skips \
             a size whose master is absent, and a release must not"
        );
    }
}

/// A package installs into `/usr`, which is root's. An entry that arrived owned by the
/// building user, or without its executable bit, would install a binary nobody can run —
/// and would prove that whatever built it did not use `install -Dm755`.
#[test]
#[ignore = "needs the Arch package: cd build/package && makepkg -f (or set INDIUM_PKG)"]
fn pkg_binary_is_root_owned_and_executable() {
    let entries = list(&pkg_path());
    let binary = find(&entries, BINARY);

    // `Entry.mode` carries the filetype bits too, so the permission bits are masked out
    // rather than compared whole.
    assert_eq!(
        binary.mode & 0o777,
        0o755,
        "{BINARY} must be rwxr-xr-x, got {:o}",
        binary.mode & 0o777
    );
    assert_eq!(binary.uid, 0, "{BINARY} must be owned by root");
    assert_eq!(binary.gid, 0, "{BINARY} must belong to the root group");
}

// ---------------------------------------------------------------------------
// The Debian package
// ---------------------------------------------------------------------------

/// A `.deb` is an `ar` archive of exactly three members, and the order is not decoration:
/// `debian-binary` must come first because dpkg reads the stream in order and settles the
/// format version before it looks at anything else. The other two are named for the
/// compression they carry, which is what tells a reader how to open them.
#[test]
#[ignore = "needs the .deb: ./build/package/make-deb.sh target/release/indium (or INDIUM_DEB)"]
fn deb_has_three_members_in_order() {
    let entries = list(&deb_path());
    let members: Vec<&str> = entries.iter().map(|e| e.path.as_str()).collect();
    assert_eq!(
        members,
        ["debian-binary", "control.tar.xz", DATA_MEMBER],
        "a .deb is these three members in this order and nothing else"
    );
}

#[test]
#[ignore = "needs the .deb: ./build/package/make-deb.sh target/release/indium (or INDIUM_DEB)"]
fn deb_data_holds_exactly_the_payload() {
    let dir = TempDir::new("deb-payload");
    let entries = list(&deb_data(&dir));
    assert_eq!(
        files_of(&entries),
        payload_and(&DEB_PAPERWORK),
        "the .deb's data member must ship exactly what build/install-payload.sh installs \
         plus the two files Debian Policy requires of every package"
    );
}

/// The same standard the Arch package is held to, on the other side of the fence: a file
/// owned by whoever happened to run the build would install with that owner.
#[test]
#[ignore = "needs the .deb: ./build/package/make-deb.sh target/release/indium (or INDIUM_DEB)"]
fn deb_data_is_root_owned_with_sane_modes() {
    let dir = TempDir::new("deb-modes");
    let entries = list(&deb_data(&dir));

    let binary = find(&entries, BINARY);
    assert_eq!(
        binary.mode & 0o777,
        0o755,
        "{BINARY} must be rwxr-xr-x, got {:o}",
        binary.mode & 0o777
    );

    for entry in entries.iter().filter(|e| !e.is_dir && e.path != BINARY) {
        assert_eq!(
            entry.mode & 0o777,
            0o644,
            "{} is data, not a program, and must be rw-r--r--, got {:o}",
            entry.path,
            entry.mode & 0o777
        );
    }

    for entry in entries.iter().filter(|e| !e.is_dir) {
        assert_eq!(entry.uid, 0, "{} must be owned by root", entry.path);
        assert_eq!(entry.gid, 0, "{} must belong to root", entry.path);
        // tar carries the names as well as the numbers, and dpkg shows the names. An
        // entry with uid 0 and a build-user *name* would read as somebody else's file.
        assert_eq!(
            entry.uname.as_deref(),
            Some("root"),
            "{} must name root as its owner",
            entry.path
        );
        assert_eq!(
            entry.gname.as_deref(),
            Some("root"),
            "{} must name root as its group",
            entry.path
        );
    }
}

// ---------------------------------------------------------------------------
// The two of them together
// ---------------------------------------------------------------------------

/// The important one.
///
/// `build/install-desktop.sh` used to say "Packages redo this system-wide in P6." They do
/// not redo it — P6 gave both packages and the dev install the one payload script — and
/// this is the test that makes that true in the artefacts rather than only in a comment,
/// and keeps it true. Two packaging systems drifting apart about what INDIUM puts on a
/// machine is the ordinary way this goes wrong, it goes wrong quietly, and it is invisible
/// to anyone who only ever installs one of them.
///
/// Only `usr/` is compared: pacman's `.PKGINFO` and Debian's control member are each
/// their own system's bookkeeping, and neither lands on a machine.
///
/// **Deviation.** P6 §8 asks for the two payloads to be "set-identical under `usr/`",
/// full stop. They cannot be, and P6 itself is why: §3 sends both licences to
/// `/usr/share/licenses/indium/` because that is where Arch keeps them, and §4 writes
/// `/usr/share/doc/indium/copyright` and `changelog.Debian.gz` because Debian Policy
/// §12.5 and §12.7 require exactly those paths. A package obeying only the other system's
/// convention is a broken package on its own system, so neither is removable and literal
/// identity was never available.
///
/// The two paperwork directories are therefore cut out of the comparison — and then
/// asserted, both ways round, so the exception is pinned rather than left as a hole to
/// grow in. What the test claims in the end is stronger than the set comparison §8 asked
/// for: **the packages differ exactly where their distributions command them to, and
/// nowhere else at all.**
#[test]
#[ignore = "needs both packages: makepkg -f in build/package/, and make-deb.sh for the .deb"]
fn deb_and_pkg_ship_the_same_payload() {
    let dir = TempDir::new("both");
    let deb_files = under_usr(&list(&deb_data(&dir)));
    let pkg_files = under_usr(&list(&pkg_path()));

    let shared = |files: &BTreeSet<String>| -> BTreeSet<String> {
        files
            .iter()
            .filter(|p| !p.starts_with(PKG_PAPERWORK_DIR) && !p.starts_with(DEB_PAPERWORK_DIR))
            .cloned()
            .collect()
    };

    assert!(
        !shared(&pkg_files).is_empty(),
        "neither package ships anything under usr/, so comparing them proves nothing"
    );
    assert_eq!(
        shared(&deb_files),
        shared(&pkg_files),
        "the .deb and the Arch package must put exactly the same files on a machine; \
         they are built from one payload script and must stay indistinguishable"
    );

    // And the difference is exactly the paperwork, in both directions.
    for path in PKG_PAPERWORK {
        assert!(
            pkg_files.contains(path),
            "the Arch package must carry {path}"
        );
    }
    for path in DEB_PAPERWORK {
        assert!(deb_files.contains(path), "the .deb must carry {path}");
    }
    assert!(
        !pkg_files.iter().any(|p| p.starts_with(DEB_PAPERWORK_DIR)),
        "the Arch package must not carry Debian's paperwork: {:?}",
        pkg_files
            .iter()
            .filter(|p| p.starts_with(DEB_PAPERWORK_DIR))
            .collect::<Vec<_>>()
    );
    assert!(
        !deb_files.iter().any(|p| p.starts_with(PKG_PAPERWORK_DIR)),
        "the .deb must not carry Arch's paperwork: {:?}",
        deb_files
            .iter()
            .filter(|p| p.starts_with(PKG_PAPERWORK_DIR))
            .collect::<Vec<_>>()
    );
}

/// CORE §5: "RAR is deliberately absent — not read, not written." CORE §8 spells out the
/// consequence for the launcher entry: it registers every supported MIME type "and
/// deliberately **not** `application/vnd.rar`". CORE §9 says it a third time.
///
/// `assets/org.indium.desktop` is checked by reading it; this checks the copy that
/// actually reaches a user's `/usr/share/applications`, which is the one that would make
/// a file manager hand INDIUM a RAR and get the refusal sentence for an answer. Source and
/// artefact are not the same claim, and only one of them is installed.
#[test]
#[ignore = "needs both packages: makepkg -f in build/package/, and make-deb.sh for the .deb"]
fn neither_package_ships_a_rar_association() {
    let dir = TempDir::new("rar");
    let cases = [
        ("the Arch package", desktop_bytes(&pkg_path())),
        ("the .deb", desktop_bytes(&deb_data(&dir))),
    ];

    for (what, bytes) in cases {
        let text = String::from_utf8_lossy(&bytes).to_ascii_lowercase();

        // Read the right file first, or "it does not mention RAR" is true of every empty
        // buffer ever produced.
        assert!(
            text.contains("mimetype="),
            "{what}: that is not a desktop entry — it carries no MimeType line"
        );
        assert!(
            text.contains("application/zip"),
            "{what}: the desktop entry lost the MIME types CORE §8 fixes"
        );

        assert!(
            !text.contains("rar"),
            "{what} ships a desktop entry that mentions RAR. CORE §5 and §9 make its \
             absence deliberate: INDIUM must never be offered as the application that \
             opens one, because it will only ever answer \"RAR is not supported.\""
        );
    }
}
