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
//! **Every test here is `#[ignore]`d, save the two exceptions named below.** The artefacts
//! do not exist during an ordinary
//! `cargo test` — they are made by a release, not by a build — and a test that silently
//! passes when its subject is absent is worse than no test. The precedent is
//! `src/platform/clipboard.rs`, whose `#[ignore = "needs a live Wayland session and
//! wl-paste"]` test is run deliberately, by hand. Each attribute below names what
//! produces the artefact it wants; a test that *is* run and finds nothing panics saying
//! the same thing, so nothing here can pass by absence.
//!
//! Run them with `cargo test --test package_path -- --ignored`.
//!
//! **Some tests here are deliberately not `#[ignore]`d**, and each exception is written down
//! rather than left to look like an oversight. How many there are is not stated, for the
//! reason CORE §2 gives about counts: this opened *"Two tests here"* and P19 added a third,
//! so the sentence describing the list was falsified by the list growing — one line above the
//! list itself. The exceptions are named below and can be counted by reading them.
//! `the_copyright_header_names_the_font_that_ships`
//! reads two files that are in the repository at all times — `LICENSES/OFL-1.1.txt` and
//! `build/package/deb/copyright.header` — so it wants no artefact, and absence therefore
//! cannot make it pass. The reason it is here and not elsewhere is that its subject is
//! packaging: it is the tree half of `verify.sh`'s check 10. And the reason it is not left
//! to `verify.sh` alone is that `verify.sh` runs in `release.yml`, at a tag, while this runs
//! in `ci.yml` on every push — and the defect it guards against (P17: the .deb naming a font
//! INDIUM stopped embedding in P12) survived three releases precisely because nothing looked
//! at it until a person did.
//!
//! `core_and_the_deb_name_the_same_dlopened_libraries` is here on the same terms — it reads
//! `CORE.md` and `build/package/make-deb.sh`, both always in the tree. It is in this file
//! because its subject is what a package declares, and it exists because through `v1.2.0-2`
//! the `.deb` declared three of the four libraries the program opens by hand. Nothing could
//! have caught that: the four are `dlopen`ed, so `ldd` is silent and `verify.sh` had nothing
//! to compare. Two hand-written lists that must agree can at least be made to say so.
//!
//! Another is `the_pkgbuild_and_cargo_toml_agree_about_the_version`, on the same terms and
//! for the same reason: it reads `Cargo.toml` and `build/package/PKGBUILD`, both always in the
//! tree, so absence cannot make it pass. Nothing `cargo test` runs looked at `pkgver` at all
//! before P18 — `verify.sh` compares the two at package time, and `release.yml` derives the
//! tag it will accept from both, so a disagreement was caught by a workflow at a tag and by
//! nothing at all on a push. That is the shape P15 moved the toolkit gate forward to fix.
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

// ---------------------------------------------------------------------------
// The attribution, checked against the things that move with the font.
//
// Not `#[ignore]`d — see this file's header for why, and why it lives here.
// ---------------------------------------------------------------------------

/// The `.deb` is the only artefact carrying a hand-written attribution: the Arch package
/// installs `LICENSES/OFL-1.1.txt` verbatim and so cannot disagree with itself. This one
/// can, and did — P12 swapped the embedded face to Fira Mono and the DEP-5 header went on
/// naming JetBrains Mono through three releases, in a file `make-deb.sh` concatenates with
/// the OFL text that names the other holder.
///
/// Both expectations are **derived, never typed a second time.** The holder comes off the
/// licence's own first line, and the face is matched against the files actually embedded —
/// so swapping the font again moves both, and a header that did not move fails here.
#[test]
fn the_copyright_header_names_the_font_that_ships() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let ofl = std::fs::read_to_string(root.join("LICENSES/OFL-1.1.txt")).expect("no OFL text");
    let header = std::fs::read_to_string(root.join("build/package/deb/copyright.header"))
        .expect("no copyright header");

    // `trim_end` is load-bearing, not tidying: the OFL ships CRLF and every one of its
    // lines carries a carriage return. Comparing without it fails on an invisible byte.
    let want = ofl
        .lines()
        .next()
        .and_then(|l| l.trim_end().strip_prefix("Digitized data copyright (c) "))
        .expect(
            "LICENSES/OFL-1.1.txt no longer opens with the line this test derives from. \
             That means the font was swapped and nothing told this test — which is the \
             failure it exists to catch, so it is an error and not a reason to skip.",
        );

    // The `Files: assets/fonts/*` stanza, and only that one — `Files: *` above it carries
    // the maker's own copyright and matching it would make this test pass on the wrong line.
    let stanza = header
        .split("\n\n")
        .find(|p| p.starts_with("Files: assets/fonts/*"))
        .expect("copyright.header has no `Files: assets/fonts/*` stanza");

    let got = stanza
        .lines()
        .find_map(|l| l.strip_prefix("Copyright: "))
        .expect("that stanza names no Copyright:");
    assert_eq!(
        got, want,
        "the .deb copyright names a different holder than the licence it ships beside it"
    );

    // "Fira Mono Nerd Font Mono" -> "FiraMonoNerdFontMono" -> a real basename in
    // assets/fonts/. "JetBrains Mono NL Nerd Font" -> "JetBrainsMonoNLNerdFont" -> nothing,
    // which is the defect P17 found, caught mechanically rather than by reading.
    let face: String = stanza
        .lines()
        .find_map(|l| l.strip_prefix("Comment: "))
        .and_then(|c| c.split(", regular and bold").next())
        .expect("that stanza's Comment: names no face")
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();

    let embedded: Vec<String> = std::fs::read_dir(root.join("assets/fonts"))
        .expect("no assets/fonts")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".otf") || n.ends_with(".ttf"))
        .collect();
    assert!(
        !embedded.is_empty(),
        "no font files in assets/fonts/ — this test would pass by absence otherwise"
    );
    assert!(
        embedded.iter().any(|n| n.starts_with(&face)),
        "the .deb copyright names {face}, which is no font this binary embeds: {embedded:?}"
    );
}

/// `PKGBUILD`'s `pkgver` is `Cargo.toml`'s version, and `pkgrel` is a number.
///
/// Not `#[ignore]`d — see the header. Both files are always in the tree, so absence cannot
/// make this pass.
///
/// Together these two numbers decide the only tag `release.yml` will accept: `pkgrel` of 1
/// gives CORE §7's two-numeral form, and anything higher gives the revision form. A drift
/// between them therefore does not produce a wrong package — it produces a tag the workflow
/// rejects, at the tag, which is the most expensive moment to find out. `verify.sh` has
/// compared them since P6, and `verify.sh` runs in `release.yml`; this runs on every push.
#[test]
fn the_pkgbuild_and_cargo_toml_agree_about_the_version() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cargo = std::fs::read_to_string(root.join("Cargo.toml")).expect("no Cargo.toml");
    let pkgbuild =
        std::fs::read_to_string(root.join("build/package/PKGBUILD")).expect("no PKGBUILD");

    // The first `version =` in the file is `[package]`'s. A dependency's version is indented
    // or inline in a table, never at column 0.
    let cargo_version = cargo
        .lines()
        .find_map(|l| l.strip_prefix("version = "))
        .map(|v| v.trim().trim_matches('"'))
        .expect("Cargo.toml has no top-level `version =` line");
    let pkgver = pkgbuild
        .lines()
        .find_map(|l| l.strip_prefix("pkgver="))
        .map(str::trim)
        .expect("the PKGBUILD has no `pkgver=` line");
    assert_eq!(
        pkgver, cargo_version,
        "the PKGBUILD says pkgver={pkgver} and Cargo.toml says {cargo_version}; \
         release.yml derives the tag it accepts from both"
    );

    // Through `pkgrel()`, which the artefact paths above already read the number with. A
    // second inline copy of the same three lines is the shape this round exists to remove,
    // and it would be a copy that cannot disagree in value but can in strictness.
    let rel = pkgrel();
    assert!(
        !rel.is_empty() && rel.bytes().all(|b| b.is_ascii_digit()),
        "pkgrel is {rel:?}, which release.yml cannot compare against 1"
    );

    // The version the binary will report, from the same source About prints.
    assert_eq!(
        cargo_version,
        env!("CARGO_PKG_VERSION"),
        "the Cargo.toml on disk and the version compiled into this test disagree"
    );
}

// ---------------------------------------------------------------------------
// The libraries nothing can see, named in two places that have to agree.
//
// Not `#[ignore]`d — see this file's header for why, and why it lives here.
// ---------------------------------------------------------------------------

/// CORE §2's system-library row is the list every package is written from, and the `.deb`'s
/// `Depends` is one of the things written from it. Neither can be checked against the binary
/// by any ordinary means: winit and glutin open these by soname at runtime, so they appear in
/// no `ldd` output and no shlibs machinery will ever find them. `make-deb.sh` says as much
/// where it declares them — *"named by hand because CORE §2 names them."*
///
/// Named by hand from a row that was short one entry. Through `v1.2.0-2` CORE §2 listed three
/// where the binary opens four, and the `.deb` inherited the omission: its `Depends` named
/// `libwayland-client0`, `libxkbcommon0` and `libegl1` while the binary it wrapped opened
/// `libwayland-egl.so.1` as well. The `PKGBUILD`'s comment carried it correctly the whole
/// time, and on Arch it could not show — one `wayland` package ships both sonames, so only
/// Debian, which splits them, could ever have been wrong.
///
/// So the two hand-written lists are pinned to each other, in both directions. **The Debian
/// package name is matched by stem rather than typed here**: `libwayland-egl` has to be the
/// prefix of some `Depends` entry, which `libwayland-egl1` is, and the soversion digit stays
/// out of this test because it belongs to Debian rather than to INDIUM. A fifth library added
/// to CORE §2 fails this until `make-deb.sh` names it; one added to `make-deb.sh` fails it
/// until CORE does.
///
/// **What it cannot prove, said plainly rather than left looking covered:** if the program
/// gains a `dlopen` that *no* document mentions, both lists agree and both are wrong. Only
/// the running program can show that. P19 records it as the limit rather than inventing a
/// weak gate for it, because a gate that cannot fail is worse than a note that it is absent.
#[test]
fn core_and_the_deb_name_the_same_dlopened_libraries() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let core = std::fs::read_to_string(root.join("CORE.md")).expect("no CORE.md");
    let make_deb = std::fs::read_to_string(root.join("build/package/make-deb.sh"))
        .expect("no build/package/make-deb.sh");

    // The three entries Debian spells differently from CORE, and therefore the three this
    // test cannot match by stem. They are typed, and the reason is that no derivation exists:
    // `libc6` is CORE's `glibc`, `libgcc-s1` is its `libgcc_s`, and `libarchive13t64` is a
    // package name carrying a soversion and a time64 suffix that appear in no document. They
    // are the *linked* libraries — `ldd` prints all three — so they are the ones this test is
    // not about.
    const LINKED: &[&str] = &["libc6", "libgcc-s1", "libarchive13t64", "libarchive13"];

    // CORE §2's row for what the compositor session provides. Found by the first name in it
    // rather than by line number, so the row may move.
    let row = core
        .lines()
        .find(|l| l.starts_with("| `libwayland-client`"))
        .expect("CORE §2 has no row starting with `libwayland-client` — has the table moved?");
    let first_cell = row
        .trim_start_matches('|')
        .split('|')
        .next()
        .expect("the row has no first cell");
    let core_names: Vec<String> = first_cell
        .split('`')
        .skip(1)
        .step_by(2)
        .map(|n| n.trim().to_ascii_lowercase())
        .collect();
    assert!(
        !core_names.is_empty(),
        "CORE §2's dlopen row parsed to no library names at all, so this test would pass by \
         absence — the row's backtick quoting has changed"
    );

    // `DEPENDS="libc6 (>= 2.35), …"` — one line, the only one in the file that starts that way.
    let depends_line = make_deb
        .lines()
        .find_map(|l| l.strip_prefix("DEPENDS=\""))
        .and_then(|l| l.strip_suffix('"'))
        .expect("make-deb.sh has no single-line `DEPENDS=\"…\"` assignment");
    // A `Depends` entry may carry a version relation or an alternation; neither is a name.
    let deb_names: Vec<String> = depends_line
        .split(',')
        .map(|e| {
            e.split('(')
                .next()
                .unwrap_or(e)
                .split('|')
                .next()
                .unwrap_or(e)
                .trim()
                .to_ascii_lowercase()
        })
        .filter(|e| !e.is_empty())
        .collect();
    assert!(
        !deb_names.is_empty(),
        "the .deb's Depends parsed to nothing, so this test would pass by absence"
    );

    // Forward: every library CORE names as dlopen'd is declared by the package.
    for name in &core_names {
        assert!(
            deb_names.iter().any(|d| d.starts_with(name.as_str())),
            "CORE §2 names `{name}` among the libraries the program opens by hand, and the \
             .deb's Depends declares no package beginning with it. A .deb that omits one of \
             these installs cleanly and fails at its first window. Depends is: {deb_names:?}"
        );
    }

    // Reverse: every declared package that is not one of the linked three is a library CORE
    // names. This is the direction that keeps the row honest when the package grows first.
    for dep in &deb_names {
        if LINKED.contains(&dep.as_str()) {
            continue;
        }
        assert!(
            core_names.iter().any(|n| dep.starts_with(n.as_str())),
            "the .deb declares `{dep}`, which is neither one of the linked libraries nor a \
             library CORE §2's row names. Either the row is short an entry, as it was through \
             v1.2.0-2, or this package declares something nothing has written down."
        );
    }
}
