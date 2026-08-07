//! P1 §4 and P2 §6: the read path, against the committed fixtures.
//!
//! Every constant here (sizes, CRCs, uids, mtimes) is recorded in
//! `tests/fixtures/README.md` alongside the command that produced the fixture.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use indium::arch::{self, ArchiveError, Entry};
use indium::secret::Secret;
use indium::util;

// ---------------------------------------------------------------------------
// A hand-written temporary directory.
//
// CORE §2: a crate must earn its sentence, and "makes a directory in /tmp for the
// tests" is not a sentence worth a dependency.
// ---------------------------------------------------------------------------

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> TempDir {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("indium-test-{}-{}-{}", tag, std::process::id(), n));
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

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn wanted(paths: &[&str]) -> HashSet<String> {
    paths.iter().map(|s| s.to_string()).collect()
}

fn no_cancel() -> Arc<AtomicBool> {
    Arc::new(AtomicBool::new(false))
}

fn find<'a>(entries: &'a [Entry], path: &str) -> &'a Entry {
    entries
        .iter()
        .find(|e| e.path == path)
        .unwrap_or_else(|| panic!("no entry {path:?} in {:?}", paths_of(entries)))
}

fn paths_of(entries: &[Entry]) -> Vec<&str> {
    entries.iter().map(|e| e.path.as_str()).collect()
}

// The payload shared by every `basic.*` fixture.
const ALPHA: &[u8] = b"INDIUM fixture alpha\n";
const BETA: &[u8] = b"INDIUM fixture beta\n";
const GAMMA: &[u8] = b"INDIUM fixture gamma\n";

// ---------------------------------------------------------------------------
// Listing
// ---------------------------------------------------------------------------

/// P1 §4: "each `basic.*` opens and lists the expected entry count and sizes".
#[test]
fn every_basic_fixture_lists_the_same_four_entries() {
    for name in ["basic.zip", "basic.tar.gz", "basic.tar.zst", "basic.7z"] {
        let entries = arch::list_all(&fixture(name), None)
            .unwrap_or_else(|e| panic!("{name} failed to list: {e}"));

        assert_eq!(entries.len(), 4, "{name} entry count");

        // Order differs by format — libarchive's 7z reader emits directories last —
        // so compare as a set.
        let got: HashSet<&str> = paths_of(&entries).into_iter().collect();
        let expect: HashSet<&str> = ["alpha.txt", "beta.txt", "sub", "sub/gamma.txt"]
            .into_iter()
            .collect();
        assert_eq!(got, expect, "{name} entry paths");

        assert_eq!(
            find(&entries, "alpha.txt").size,
            ALPHA.len() as u64,
            "{name} alpha size"
        );
        assert_eq!(
            find(&entries, "beta.txt").size,
            BETA.len() as u64,
            "{name} beta size"
        );
        assert_eq!(
            find(&entries, "sub/gamma.txt").size,
            GAMMA.len() as u64,
            "{name} gamma size"
        );

        assert!(
            find(&entries, "sub").is_dir,
            "{name}: sub must be a directory"
        );
        assert!(
            !find(&entries, "alpha.txt").is_dir,
            "{name}: alpha must be a file"
        );
    }
}

#[test]
fn methods_are_reported_per_format() {
    let cases = [
        ("basic.zip", "deflate"),
        ("basic.tar.gz", "gzip"),
        ("basic.tar.zst", "zstd"),
        ("basic.7z", "7z"),
    ];
    for (name, expect) in cases {
        let entries = arch::list_all(&fixture(name), None).expect("listed");
        let method = &find(&entries, "alpha.txt").method;
        assert_eq!(method, expect, "{name} method label");
    }
}

/// Per-entry compressed size is not available from the generic reader, and the
/// Inspector must not invent one. This test pins that honesty in place so a future
/// change cannot quietly start guessing.
#[test]
fn packed_size_is_absent_rather_than_guessed() {
    let entries = arch::list_all(&fixture("basic.zip"), None).expect("listed");
    assert!(
        entries.iter().all(|e| e.packed.is_none()),
        "libarchive exposes no per-entry compressed size; it must stay None until P4"
    );
}

// ---------------------------------------------------------------------------
// Metadata
// ---------------------------------------------------------------------------

/// P1 §4: "`meta.tar` round-trips symlink target, uid/gid, and mtime into `Entry`".
#[test]
fn meta_tar_round_trips_its_metadata() {
    let entries = arch::list_all(&fixture("meta.tar"), None).expect("meta.tar listed");
    assert_eq!(entries.len(), 5, "paths: {:?}", paths_of(&entries));

    let regular = find(&entries, "regular.txt");
    assert_eq!(regular.size, 28);
    assert_eq!(regular.uid, 1234);
    assert_eq!(regular.gid, 5678);
    assert_eq!(regular.uname.as_deref(), Some("indiumuser"));
    assert_eq!(regular.gname.as_deref(), Some("indiumgroup"));
    assert_eq!(regular.mtime, Some(981_158_400));
    assert_eq!(regular.mode & 0o777, 0o644);

    let link = find(&entries, "symlink.txt");
    assert_eq!(link.symlink.as_deref(), Some("regular.txt"));
    assert!(!link.is_dir);

    let hard = find(&entries, "hardlink.txt");
    assert_eq!(hard.hardlink.as_deref(), Some("regular.txt"));
    // A tar hardlink carries filetype 0, not AE_IFREG. `is_dir` must not misread
    // that as a directory.
    assert!(
        !hard.is_dir,
        "a hardlink with filetype 0 is not a directory"
    );

    let dir = find(&entries, "oldstuff");
    assert!(dir.is_dir);
    assert_eq!(dir.mtime, Some(946_598_400));
}

#[test]
fn timestamps_render_from_a_real_archive() {
    let entries = arch::list_all(&fixture("meta.tar"), None).expect("listed");
    let mtime = find(&entries, "regular.txt").mtime.expect("mtime set");
    assert_eq!(util::format_timestamp(mtime), "2001-02-03 00:00:00 UTC");
}

// ---------------------------------------------------------------------------
// Extraction
// ---------------------------------------------------------------------------

/// P1 §4: "extraction of `basic.zip` to a tempdir reproduces byte-identical files".
#[test]
fn extraction_reproduces_bytes_exactly() {
    let dir = TempDir::new("extract");
    let n = arch::extract(
        &fixture("basic.zip"),
        &wanted(&["alpha.txt", "beta.txt", "sub"]),
        dir.path(),
        None,
        None,
        &no_cancel(),
    )
    .expect("extraction failed");

    assert_eq!(n, 4, "alpha, beta, sub/ and sub/gamma.txt");
    assert_eq!(std::fs::read(dir.path().join("alpha.txt")).unwrap(), ALPHA);
    assert_eq!(std::fs::read(dir.path().join("beta.txt")).unwrap(), BETA);
    assert_eq!(
        std::fs::read(dir.path().join("sub/gamma.txt")).unwrap(),
        GAMMA
    );
}

#[test]
fn extraction_takes_only_what_was_selected() {
    let dir = TempDir::new("subset");
    arch::extract(
        &fixture("basic.tar.gz"),
        &wanted(&["alpha.txt"]),
        dir.path(),
        None,
        None,
        &no_cancel(),
    )
    .expect("extraction failed");

    assert!(dir.path().join("alpha.txt").exists());
    assert!(
        !dir.path().join("beta.txt").exists(),
        "beta was not selected"
    );
    assert!(!dir.path().join("sub").exists(), "sub was not selected");
}

/// P1 §4: "`evil.zip` extraction **fails** and `escape.txt` does not exist outside
/// the tempdir". This is the test that proves the secure flags are actually on.
#[test]
fn a_traversal_entry_is_refused_and_writes_nothing() {
    let dir = TempDir::new("evil");
    let inside = dir.path().join("dest");
    std::fs::create_dir_all(&inside).unwrap();

    let entries = arch::list_all(&fixture("evil.zip"), None).expect("evil.zip lists fine");
    assert_eq!(paths_of(&entries), vec!["../escape.txt"]);

    let result = arch::extract(
        &fixture("evil.zip"),
        &wanted(&["../escape.txt"]),
        &inside,
        None,
        None,
        &no_cancel(),
    );

    assert!(
        result.is_err(),
        "extraction of a '..' entry must fail, got {result:?}"
    );

    // The escape target: one level up from the destination.
    let escaped = dir.path().join("escape.txt");
    assert!(!escaped.exists(), "a file escaped to {}", escaped.display());
    assert!(!inside.join("escape.txt").exists());
    assert!(
        !Path::new("/tmp/escape.txt").exists(),
        "something reached /tmp/escape.txt"
    );
}

// ---------------------------------------------------------------------------
// CRC32
// ---------------------------------------------------------------------------

/// P1 §4: "`crc32` of a fixture file matches a precomputed constant."
#[test]
fn crc32_of_an_entry_matches_the_precomputed_constant() {
    let path = fixture("basic.zip");
    assert_eq!(
        arch::crc32_of(&path, "alpha.txt", None).unwrap(),
        0xF28E_C54D
    );
    assert_eq!(
        arch::crc32_of(&path, "beta.txt", None).unwrap(),
        0xD5AC_ED60
    );
    assert_eq!(
        arch::crc32_of(&path, "sub/gamma.txt", None).unwrap(),
        0x78FF_AF48
    );
}

#[test]
fn crc32_agrees_across_containers() {
    // The same bytes, however they were packed, must checksum the same.
    for name in ["basic.zip", "basic.tar.gz", "basic.tar.zst", "basic.7z"] {
        assert_eq!(
            arch::crc32_of(&fixture(name), "alpha.txt", None).unwrap(),
            0xF28E_C54D,
            "{name}"
        );
    }
}

#[test]
fn crc32_of_a_missing_entry_is_an_error_not_a_panic() {
    let e = arch::crc32_of(&fixture("basic.zip"), "nope.txt", None);
    assert!(matches!(e, Err(ArchiveError::Other(_))));
}

// ---------------------------------------------------------------------------
// The RAR gate
// ---------------------------------------------------------------------------

/// CORE §5 and P1 §4: a RAR is refused with the exact sentence, nothing else.
#[test]
fn rar_is_refused_with_the_exact_sentence() {
    let path = fixture("notrar.rar");
    assert!(arch::looks_like_rar(&path), "signature not recognised");

    let err = arch::list_all(&path, None).expect_err("a RAR must not list");
    assert_eq!(err, ArchiveError::Rar);
    assert_eq!(err.to_string(), "RAR is not supported.");
    assert_eq!(arch::RAR_REFUSAL, "RAR is not supported.");
}

#[test]
fn non_rar_archives_are_not_caught_by_the_gate() {
    for name in [
        "basic.zip",
        "basic.tar.gz",
        "basic.7z",
        "meta.tar",
        "evil.zip",
    ] {
        assert!(
            !arch::looks_like_rar(&fixture(name)),
            "{name} was mistaken for RAR"
        );
    }
}

// ---------------------------------------------------------------------------
// Encrypted reads (P2 §5, §6)
// ---------------------------------------------------------------------------

/// P1 §4: `secret.zip`'s *listing* works without a password.
#[test]
fn an_encrypted_zip_lists_without_a_password() {
    let entries = arch::list_all(&fixture("secret.zip"), None).expect("listing needs no password");
    assert_eq!(entries.len(), 1);
    let e = &entries[0];
    assert_eq!(e.path, "secret.txt");
    assert_eq!(e.size, 22);
    assert!(e.encrypted, "the entry must be flagged encrypted");
}

#[test]
fn encrypted_entries_are_detected() {
    assert_eq!(
        arch::has_encrypted_entries(&fixture("secret.zip")),
        Some(true)
    );
    // tar cannot encrypt at all: libarchive answers UNSUPPORTED, which is not `false`.
    assert_eq!(arch::has_encrypted_entries(&fixture("meta.tar")), None);
}

/// P2 §6: "`secret.zip` extracts byte-identical with `indium`".
#[test]
fn an_encrypted_zip_extracts_with_the_right_password() {
    let dir = TempDir::new("secret-ok");
    let pass = Secret::from_text("indium");
    arch::extract(
        &fixture("secret.zip"),
        &wanted(&["secret.txt"]),
        dir.path(),
        Some(&pass),
        None,
        &no_cancel(),
    )
    .expect("extraction with the right password failed");

    assert_eq!(
        std::fs::read(dir.path().join("secret.txt")).unwrap(),
        b"INDIUM secret payload\n"
    );
}

/// P2 §6: "a wrong passphrase returns an error, not a panic, and writes nothing."
#[test]
fn a_wrong_password_errors_and_writes_nothing() {
    let dir = TempDir::new("secret-bad");
    let pass = Secret::from_text("wrong");
    let result = arch::extract(
        &fixture("secret.zip"),
        &wanted(&["secret.txt"]),
        dir.path(),
        Some(&pass),
        None,
        &no_cancel(),
    );

    assert!(
        result.is_err(),
        "a wrong password must fail, got {result:?}"
    );
    let leftover: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.file_name())
        .collect();
    assert!(
        leftover.is_empty(),
        "the destination must be untouched, found {leftover:?}"
    );
}

/// P2 §5's verify-before-writing step: three wrong attempts must cost nothing.
#[test]
fn passwords_can_be_verified_without_writing() {
    let path = fixture("secret.zip");
    assert!(
        arch::verify_passphrase(&path, &Secret::from_text("indium")).unwrap(),
        "the correct password should verify"
    );
    assert!(
        !arch::verify_passphrase(&path, &Secret::from_text("wrong")).unwrap(),
        "a wrong password should not verify"
    );
}

/// `secret-headers.7z` — encrypted filenames.
///
/// DEVIATION, recorded in P2's log: libarchive 3.8.9 returns "The archive header is
/// encrypted, but currently not supported" from every `next_header`, with or without
/// a passphrase. So P2 §6's "with the passphrase, lists and extracts" is not reachable
/// from a pure-libarchive reader. What INDIUM *can* do — and what this test pins — is
/// detect the situation and say so plainly instead of showing an empty archive.
#[test]
fn encrypted_headers_are_detected_and_reported_not_silently_empty() {
    let path = fixture("secret-headers.7z");

    assert_eq!(
        arch::has_encrypted_entries(&path),
        Some(true),
        "encrypted headers must be detectable"
    );

    for attempt in [None, Some(Secret::from_text("indium"))] {
        let err = arch::list_all(&path, attempt.as_ref())
            .expect_err("libarchive cannot read encrypted 7z headers");
        assert!(
            matches!(err, ArchiveError::EncryptedHeaders | ArchiveError::Other(_)),
            "unexpected error {err:?}"
        );
        // Whatever the wording, it must never be silently empty.
        assert!(!err.to_string().is_empty());
    }
}
