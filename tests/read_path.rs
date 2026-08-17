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

/// PXX: `tar -cf x.tar -C dir .` stores a leading `./`, and until this round INDIUM
/// could neither list nor extract any archive shaped that way.
///
/// `normalize_archive_path("./")` is the empty string, which is also what an entry whose
/// name could not be read normalises to — so the listing grew a nameless row, and
/// `extract`'s pre-flight refused the **whole archive** with *"this archive holds an
/// entry whose name could not be read on this system"*. The name was `./`. It was plain
/// ASCII and it was read perfectly.
///
/// It went unnoticed for twenty-two rounds because not one committed fixture was rooted
/// that way, which is why `rooted.tar` now exists and carries the same payload as every
/// `basic.*`. Both halves are asserted here: the root does not become a row, and the four
/// real members all come out.
#[test]
fn a_dot_slash_rooted_tar_lists_and_extracts_like_any_other() {
    let entries = arch::list_all(&fixture("rooted.tar"), None).expect("rooted.tar failed to list");

    for e in &entries {
        assert!(
            !e.path.is_empty(),
            "the archive root became a nameless row: {:?}",
            paths_of(&entries)
        );
    }

    let got: HashSet<&str> = paths_of(&entries).into_iter().collect();
    let expect: HashSet<&str> = ["alpha.txt", "beta.txt", "sub", "sub/gamma.txt"]
        .into_iter()
        .collect();
    assert_eq!(got, expect, "rooted.tar entry paths");
    assert_eq!(entries.len(), 4, "the `./` root is not a fifth member");

    // Sizes come through the stripped prefix unharmed.
    assert_eq!(find(&entries, "alpha.txt").size, ALPHA.len() as u64);
    assert_eq!(find(&entries, "sub/gamma.txt").size, GAMMA.len() as u64);

    let dir = TempDir::new("rooted");
    let n = arch::extract(
        &fixture("rooted.tar"),
        &wanted(&["alpha.txt", "beta.txt", "sub"]),
        dir.path(),
        None,
        None,
        &no_cancel(),
    )
    .expect("a `./`-rooted archive must extract like any other");

    // The same four `extraction_reproduces_bytes_exactly` gets from `basic.zip` for the
    // same selection — alpha, beta, sub/ and sub/gamma.txt. That equality is the claim.
    assert_eq!(n, 4, "alpha, beta, sub/ and sub/gamma.txt");
    assert_eq!(std::fs::read(dir.path().join("alpha.txt")).unwrap(), ALPHA);
    assert_eq!(std::fs::read(dir.path().join("beta.txt")).unwrap(), BETA);
    assert_eq!(
        std::fs::read(dir.path().join("sub/gamma.txt")).unwrap(),
        GAMMA,
        "selecting the directory takes what is under it"
    );
}

#[test]
fn methods_are_reported_per_format() {
    let cases = [
        ("basic.zip", "deflate"),
        ("basic.tar.gz", "gzip"),
        ("basic.tar.zst", "zstd"),
        // Since P4 a 7z is listed through `sevenz-rust2`, which names the coder its
        // block actually uses rather than the container. That is strictly more than
        // libarchive's "7z" could say, and it is what CORE §4 meant by 7z-specific
        // detail arriving in P4 — note this fixture is **LZMA**, not LZMA2, because
        // `bsdtar --format 7zip` defaults to LZMA1. INDIUM writes LZMA2; it reads what
        // is actually there, and says which.
        ("basic.7z", "LZMA"),
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
        "libarchive exposes no per-entry compressed size, and the Inspector must not \
         invent one. A 7z reports one where an entry owns its block; a zip never does."
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

/// One ustar member: a 512-byte header, the body, and the padding up to the next block.
///
/// Hand-built rather than shelled out to `tar`, for two reasons. `bsdtar` and GNU `tar`
/// both refuse to *store* the names this test is about — that refusal is theirs, and a
/// fixture that has to argue with its own generator is a fixture nobody will maintain. And
/// written out here the hostile names are **visible in the test**, where a checked-in blob
/// would make them opaque bytes that nothing in the tree explains.
fn ustar_member(name: &str, body: &[u8]) -> Vec<u8> {
    fn put(h: &mut [u8; 512], at: usize, s: &str) {
        h[at..at + s.len()].copy_from_slice(s.as_bytes());
    }

    let mut h = [0u8; 512];
    let nb = name.as_bytes();
    assert!(nb.len() < 100, "every name here fits the ustar name field");
    h[..nb.len()].copy_from_slice(nb);
    put(&mut h, 100, "0000644\0"); // mode
    put(&mut h, 108, "0000000\0"); // uid
    put(&mut h, 116, "0000000\0"); // gid
    put(&mut h, 124, &format!("{:011o}\0", body.len()));
    put(&mut h, 136, "00000000000\0"); // mtime
    h[156] = b'0'; // typeflag: a regular file
    put(&mut h, 257, "ustar\0");
    put(&mut h, 263, "00");

    // The checksum is taken with its own field read as spaces, then written back into it
    // as six octal digits, a NUL and a space. That trailing pair is the format's.
    h[148..156].fill(b' ');
    let sum: u32 = h.iter().map(|&b| b as u32).sum();
    put(&mut h, 148, &format!("{sum:06o}\0 "));

    let mut out = h.to_vec();
    out.extend_from_slice(body);
    out.resize(out.len().div_ceil(512) * 512, 0);
    out
}

/// A whole tar: the members, then the two zero blocks that end one.
fn ustar_tar(members: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    for (name, body) in members {
        out.extend_from_slice(&ustar_member(name, body));
    }
    out.extend_from_slice(&[0u8; 1024]);
    out
}

/// PXX, and it closes the gap the certification walk's step 3.13 left open.
///
/// The walk approved 3.13 on the refusal of the **first** traversal member, and the refusal
/// is deliberately whole-archive — it returns on the first one it finds — so members 2, 3
/// and 4 of that fixture were never reached, let alone judged. `path_escapes` has unit tests
/// for all four shapes in isolation, but a shape refused by the predicate and a shape refused
/// by `extract` are two different claims, and only the second one is the promise §3 makes.
///
/// So: one archive per shape, each with a harmless member **ahead** of the hostile one, which
/// is what proves the loop actually reaches the member under test rather than stopping short.
///
/// **The absolute member aims inside the tempdir, not at `$HOME`.** That is the one deliberate
/// departure from the fixture the walk used, and it is not a weakening: `path_escapes` takes
/// the same `starts_with('/')` branch either way, and `dest.join()` replaces the destination
/// wholesale for any absolute path, so the code under test cannot tell the difference. What
/// changes is only where the damage lands if this test ever fails — inside a directory the
/// test already owns, rather than in the home directory of whoever ran `cargo test`.
#[test]
fn every_traversal_shape_is_refused_end_to_end_and_writes_nothing() {
    let dir = TempDir::new("traversal");
    let inside = dir.path().join("dest");
    std::fs::create_dir_all(&inside).unwrap();

    // Absolute, and pointed somewhere this test is entitled to break.
    let absolute = dir.path().join("escaped-absolute.txt");
    let absolute = absolute
        .to_str()
        .expect("a tempdir path is UTF-8")
        .to_string();

    let shapes: Vec<(&str, String)> = vec![
        ("one up", "../escaped-one-up.txt".to_string()),
        ("two up", "../../escaped-two-up.txt".to_string()),
        (
            "via a middle component",
            "safe/../../escaped-via-middle.txt".to_string(),
        ),
        ("absolute", absolute.clone()),
    ];

    for (what, hostile) in &shapes {
        let tar = dir.path().join(format!("{}.tar", what.replace(' ', "-")));
        std::fs::write(
            &tar,
            ustar_tar(&[
                ("harmless.txt", b"in front of the hostile one\n".as_slice()),
                (hostile.as_str(), b"should never be written\n".as_slice()),
            ]),
        )
        .unwrap();

        // Listing is not where the refusal belongs: the Inspector must be able to show a
        // person what an archive claims to hold, including the parts of it that are a lie.
        let entries = arch::list_all(&tar, None)
            .unwrap_or_else(|e| panic!("the {what} archive must still list: {e}"));
        assert_eq!(
            paths_of(&entries).len(),
            2,
            "the {what} archive holds both members"
        );

        let result = arch::extract(
            &tar,
            &wanted(&["harmless.txt", hostile.as_str()]),
            &inside,
            None,
            None,
            &no_cancel(),
        );
        assert!(
            result.is_err(),
            "the {what} member must be refused, got {result:?}"
        );

        // And it must say so. A member silently dropped from an otherwise successful
        // extraction is the failure mode §3 is written against — the user is told the
        // archive came out, and one file of it quietly did not.
        let msg = format!("{}", result.unwrap_err());
        assert!(
            msg.contains(hostile.as_str()),
            "the refusal must name the {what} member; it said {msg:?}"
        );
    }

    // Nothing reached any of the four targets. Checked after the whole loop rather than
    // inside it, so a leak from an earlier shape cannot be masked by a later assertion.
    for suffix in [
        "escaped-one-up.txt",
        "escaped-two-up.txt",
        "escaped-via-middle.txt",
    ] {
        for base in [dir.path(), dir.path().parent().unwrap()] {
            let leaked = base.join(suffix);
            assert!(!leaked.exists(), "a file escaped to {}", leaked.display());
        }
    }
    assert!(
        !Path::new(&absolute).exists(),
        "the absolute member was written to {absolute}"
    );
    assert!(
        !inside.join("escaped-one-up.txt").exists(),
        "nothing lands inside the destination either — the archive is refused whole"
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

/// PXX 10.9: a wrong password on an **encrypted-header** 7z must say so.
///
/// The walk ran `indium cat secret.7z …`, gave the wrong password, and got
/// `indium: Other("Broken or unsupported archive: no Header")` — a Rust enum, a crate's
/// internal wording, and no hint that the password was the problem.
///
/// The mechanism is worth naming, because it is why this could not just be left to
/// `MaybeBadPassword`. AES has nothing to check a key against: a wrong one decrypts the
/// header to noise perfectly happily, and only the *parser* then objects — in whatever
/// way that particular noise happens to break it. So the crate never reports a password
/// problem at all here; it reports a broken file. `classify` reads it back as what it is.
///
/// The zip case above goes through libarchive and was already covered. This is the 7z
/// header path, which is `sevenz-rust2`'s alone, and nothing had ever tested it.
#[test]
fn a_wrong_password_on_encrypted_headers_says_password_not_broken_archive() {
    let path = fixture("secret-headers.7z");

    let err = arch::list_all(&path, Some(&Secret::from_text("totallywrong")))
        .expect_err("a wrong password must not list an encrypted-header archive");

    assert!(
        matches!(err, ArchiveError::WrongPassword),
        "expected WrongPassword, got {err:?}"
    );

    // The sentence a person actually reads must name neither a Rust type nor the crate.
    let shown = err.to_string();
    for leak in ["Other(", "no Header", "Broken or unsupported"] {
        assert!(
            !shown.contains(leak),
            "the message still leaks {leak:?}: {shown:?}"
        );
    }

    // And the right password still works, so the mapping did not simply swallow every
    // failure into "wrong password".
    let entries = arch::list_all(&path, Some(&Secret::from_text("indium")))
        .expect("the right password must still list");
    assert!(!entries.is_empty(), "the fixture holds at least one member");
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
/// P2 §6 asked for "with the passphrase, lists and extracts" and could not have it:
/// libarchive 3.8.9 answers every `next_header` with "The archive header is encrypted,
/// but currently not supported", with or without a passphrase, which P2 recorded as its
/// first Deviation. P4 routes 7z listing through `sevenz-rust2`, which parses encrypted
/// headers natively — so **this test now pins the requirement rather than the excuse**,
/// and the deviation it used to guard is closed.
#[test]
fn encrypted_headers_list_with_the_passphrase_and_refuse_without_it() {
    let path = fixture("secret-headers.7z");

    let err = arch::list_all(&path, None)
        .expect_err("without the password the names are ciphertext and must not be listed");
    assert!(
        matches!(
            err,
            ArchiveError::NeedPassword
                | ArchiveError::WrongPassword
                | ArchiveError::EncryptedHeaders
                | ArchiveError::Other(_)
        ),
        "unexpected error {err:?}"
    );
    assert!(
        !err.to_string().is_empty(),
        "whatever the wording, it must never be silently empty"
    );

    let entries = arch::list_all(&path, Some(&Secret::from_text("indium")))
        .expect("with the passphrase it must list — P2 §6, reachable at last");
    assert_eq!(entries.len(), 1, "the fixture holds one member");
    assert_eq!(entries[0].path, "f.txt");
    assert!(entries[0].encrypted, "its block is AES-256");
}

// ---------------------------------------------------------------------------
// The streaming list — P5 §A1
//
// `arch::list_all` is what the tests reach for and `arch::list` is what the window
// actually runs. They routed 7z differently until P5, so everything sevenz-rust2 knew
// reached this file and never reached a user. These tests drive the streaming path
// specifically, because that is the one that was wrong.
// ---------------------------------------------------------------------------

/// Collect a streaming listing the way `Indium::drain_worker` does.
fn stream(
    path: &Path,
    passphrase: Option<&Secret>,
) -> Result<(arch::ArchiveInfo, Vec<Entry>), ArchiveError> {
    use indium::arch::ListMsg;
    let (tx, rx) = std::sync::mpsc::channel();
    arch::list(path, passphrase, &tx, &no_cancel());
    drop(tx);

    let mut info = arch::ArchiveInfo::default();
    let mut entries = Vec::new();
    for msg in rx {
        match msg {
            ListMsg::Opened(i) => info = i,
            ListMsg::Entry(e) => entries.push(*e),
            ListMsg::Done { .. } => {}
            ListMsg::Failed(e) => return Err(e),
        }
    }
    Ok((info, entries))
}

/// P5 §A1. The discriminating assertion is the **method**, not the packed size:
/// `basic.7z` is solid, so packed is `None` under either routing and an assertion on it
/// would have passed while the bug was still there.
#[test]
fn the_streaming_list_names_the_coder_a_7z_actually_uses() {
    let (_, entries) = stream(&fixture("basic.7z"), None).expect("basic.7z must list");
    assert_eq!(
        find(&entries, "alpha.txt").method,
        "LZMA",
        "the streaming list must route 7z through sevenz-rust2, which names the coder; \
         libarchive would say \"7z\" and know nothing of the block"
    );
}

/// CORE §4's solid-block detail, reaching the window at last rather than only a test.
#[test]
fn a_solid_archive_reports_itself_solid_through_the_streaming_list() {
    let (info, _) = stream(&fixture("basic.7z"), None).expect("basic.7z must list");
    assert_eq!(
        info.solid,
        Some(true),
        "basic.7z is one block holding three files"
    );
    assert_eq!(info.blocks, Some(1));

    // Everything libarchive reads has no block structure to report, and must not invent
    // one rather than leaving it unknown.
    let (zip, _) = stream(&fixture("basic.zip"), None).expect("basic.zip must list");
    assert_eq!(zip.solid, None, "a zip has no blocks to be solid about");
    assert_eq!(zip.blocks, None);
}

/// P2 §6's requirement, through the path the window actually uses. Until P5 this failed:
/// `open_archive` runs the streaming list, which went straight to libarchive.
#[test]
fn an_encrypted_header_7z_opens_through_the_streaming_list() {
    let path = fixture("secret-headers.7z");

    let err = stream(&path, None).expect_err("without the password there is nothing to show");
    assert!(
        matches!(
            err,
            ArchiveError::NeedPassword
                | ArchiveError::WrongPassword
                | ArchiveError::EncryptedHeaders
        ),
        "the error must be one the window turns into a prompt, not a dead end: {err:?}"
    );

    let (_, entries) =
        stream(&path, Some(&Secret::from_text("indium"))).expect("with the password it must list");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].path, "f.txt");
    assert_eq!(
        entries[0].packed,
        Some(48),
        "f.txt owns its block outright, so its packed size is knowable"
    );
}

/// P5 §A1b. P4 §4 promised libarchive-first-then-sevenz for data and never built it;
/// making an encrypted-header archive listable is what turned that from latent into a
/// user-visible hole. The payload is recorded in `tests/fixtures/README.md`.
#[test]
fn an_encrypted_header_7z_entry_can_be_read_after_the_prompt() {
    let path = fixture("secret-headers.7z");
    let secret = Secret::from_text("indium");
    const PAYLOAD: &[u8] = b"INDIUM header-encrypted payload\n";

    let got = arch::crc32_of(&path, "f.txt", Some(&secret))
        .expect("an archive that lists must also be readable");
    assert_eq!(
        got,
        util::crc32(PAYLOAD),
        "the bytes must be the payload the fixtures README records"
    );

    assert!(
        arch::crc32_of(&path, "f.txt", None).is_err(),
        "and without the password it must still refuse"
    );
}

/// The third read path. An archive that lists must also extract, or listing it was a
/// promise INDIUM could not keep.
#[test]
fn an_encrypted_header_7z_extracts_after_the_prompt() {
    let dir = TempDir::new("hdr7z");
    let n = arch::extract(
        &fixture("secret-headers.7z"),
        &wanted(&["f.txt"]),
        dir.path(),
        Some(&Secret::from_text("indium")),
        None,
        &no_cancel(),
    )
    .expect("an archive that lists must also extract");

    assert_eq!(n, 1);
    assert_eq!(
        std::fs::read(dir.path().join("f.txt")).expect("f.txt must be on disk"),
        b"INDIUM header-encrypted payload\n",
        "the bytes must be the payload the fixtures README records"
    );
}

/// A link already on the disk cannot redirect an encrypted-header write. **PXX-2-001.**
///
/// The encrypted-header branch never reaches libarchive, so `SECURE_SYMLINKS` and
/// `SECURE_NODOTDOT` — which `CORE.md:102` says extraction runs under — are not in play on it at
/// all. `path_escapes` vets the *stored* name, and the only name here is `f.txt`, which escapes
/// nothing: the vector is not a hostile path but a link already sitting in the destination, and
/// INDIUM plants such a link itself, because an ordinary tar carrying one extracts with exit 0.
///
/// Both variants, because they fail differently and only one of them fails the obvious way.
/// `O_NOFOLLOW` refuses the symlink and **opens the hardlink** — a hardlink is not a link the
/// kernel resolves, it is a second name for one inode — so the write has to be unlink-then-create
/// rather than open-with-a-flag. Severing is not refusing: extraction must still succeed, because
/// a destination holding a stale link is not a hostile archive and the user asked for their files.
#[test]
fn a_link_planted_in_the_destination_cannot_redirect_an_encrypted_header_write() {
    const PAYLOAD: &[u8] = b"INDIUM header-encrypted payload\n";

    // ---- the symlink variant: the target does not exist, so a followed write creates it ----
    let dir = TempDir::new("pxx2001-sym");
    let dest = dir.path().join("dest");
    let outside = dir.path().join("outside");
    std::fs::create_dir_all(&dest).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    let pwned = outside.join("pwned.txt");
    std::os::unix::fs::symlink(&pwned, dest.join("f.txt")).unwrap();

    let result = arch::extract(
        &fixture("secret-headers.7z"),
        &wanted(&["f.txt"]),
        &dest,
        Some(&Secret::from_text("indium")),
        None,
        &no_cancel(),
    );

    assert!(
        !pwned.exists(),
        "the payload was written through a symlink to {}, outside the destination",
        pwned.display()
    );
    assert_eq!(
        result.expect("severing a stale link is not a refusal — extraction must still succeed"),
        1
    );
    assert_eq!(
        std::fs::read(dest.join("f.txt")).expect("f.txt must be in the destination"),
        PAYLOAD
    );
    assert!(
        std::fs::symlink_metadata(dest.join("f.txt"))
            .unwrap()
            .is_file(),
        "the destination entry must be a real file now, not still a link"
    );

    // ---- the hardlink variant: one inode, two names, and O_NOFOLLOW sees nothing wrong ----
    let dir = TempDir::new("pxx2001-hard");
    let dest = dir.path().join("dest");
    let outside = dir.path().join("outside");
    std::fs::create_dir_all(&dest).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    let victim = outside.join("victim.txt");
    const ALREADY: &[u8] = b"the file that was already here\n";
    std::fs::write(&victim, ALREADY).unwrap();
    std::fs::hard_link(&victim, dest.join("f.txt")).unwrap();

    let result = arch::extract(
        &fixture("secret-headers.7z"),
        &wanted(&["f.txt"]),
        &dest,
        Some(&Secret::from_text("indium")),
        None,
        &no_cancel(),
    );

    assert_eq!(
        std::fs::read(&victim).unwrap(),
        ALREADY,
        "the payload was written through a hardlink into {}, outside the destination",
        victim.display()
    );
    assert_eq!(
        result.expect("severing a stale link is not a refusal — extraction must still succeed"),
        1
    );
    assert_eq!(
        std::fs::read(dest.join("f.txt")).expect("f.txt must be in the destination"),
        PAYLOAD
    );

    // ---- and a linked *directory* component, which is the same hole one level up ----
    let dir = TempDir::new("pxx2001-dir");
    let dest = dir.path().join("dest");
    let outside = dir.path().join("outside");
    std::fs::create_dir_all(&dest).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    // `d` inside the destination is a link to a directory outside it. Nothing the archive says
    // is unsafe; `create_dir_all` would walk straight through this and write beyond `dest`.
    std::os::unix::fs::symlink(&outside, dest.join("d")).unwrap();
    let escaped = outside.join("f.txt");
    let result = arch::extract(
        &fixture("secret-headers.7z"),
        &wanted(&["f.txt"]),
        &dest.join("d"),
        Some(&Secret::from_text("indium")),
        None,
        &no_cancel(),
    );
    // `dest/d` was named as the destination itself, which is the user's own choice and not the
    // archive's — so this one is allowed, and is here to pin that the fix did not overreach into
    // refusing a destination the user pointed at through their own link.
    assert!(
        result.is_ok() && escaped.exists(),
        "a destination the *user* named through their own link must still work, got {result:?}"
    );
}

// ---------------------------------------------------------------------------
// Names outside ASCII — P11
// ---------------------------------------------------------------------------

/// The names `utf8.zip` stores, in the order `list_all` reports them.
const UTF8_NAMES: [&str; 4] = ["Ünlü", "köpek.txt", "日本語.txt", "Ünlü/naïve.txt"];

/// **The regression test for P11's worst find**, and the one no earlier milestone had.
///
/// libarchive converts a stored name into the *current locale's* charset as it reads the
/// header. A Rust program never calls `setlocale`, so INDIUM ran its whole life in the `C`
/// locale, every name with a byte outside ASCII failed to convert, and
/// `archive_entry_pathname` returned **NULL** — which arrived here as an empty string.
///
/// Every fixture before this one is pure ASCII, which is exactly why seven milestones of
/// tests all passed while `köpek.txt` was unreachable in a shipped binary.
#[test]
fn every_name_survives_the_read_whatever_alphabet_it_is_in() {
    let entries = arch::list_all(&fixture("utf8.zip"), None).expect("utf8.zip lists");
    assert_eq!(paths_of(&entries), UTF8_NAMES);
    for e in &entries {
        assert!(
            !e.path.is_empty(),
            "a nameless entry means the locale conversion failed again: {e:?}"
        );
    }
}

/// The half that lost data. A name that did not survive the read matches no selection, so
/// `extract` skipped it exactly as it skips a file nobody asked for — silently, and
/// reporting success for the files that happened to be ASCII.
#[test]
fn extraction_writes_every_name_rather_than_the_ascii_ones() {
    let dir = TempDir::new("utf8");
    let n = arch::extract(
        &fixture("utf8.zip"),
        &wanted(&UTF8_NAMES),
        dir.path(),
        None,
        None,
        &no_cancel(),
    )
    .expect("extracted");

    assert_eq!(n, 4, "one directory and three files");
    for (name, payload) in [
        ("köpek.txt", &b"INDIUM utf8 kopek\n"[..]),
        ("日本語.txt", &b"INDIUM utf8 nihongo\n"[..]),
        ("Ünlü/naïve.txt", &b"INDIUM utf8 naive\n"[..]),
    ] {
        let on_disk = dir.path().join(name);
        assert!(on_disk.exists(), "{name} never reached disk");
        assert_eq!(
            std::fs::read(&on_disk).expect("readable"),
            payload,
            "{name} has the wrong bytes"
        );
    }
}

/// Selecting the directory must pull the child beneath it, which is the path `Ctrl+C` on a
/// folder takes. `selection_matches` is pure `str` work, so a name that arrived empty made
/// it answer `false` for a child it should have claimed.
#[test]
fn selecting_a_directory_outside_ascii_takes_what_is_under_it() {
    let dir = TempDir::new("utf8sel");
    let n = arch::extract(
        &fixture("utf8.zip"),
        &wanted(&["Ünlü"]),
        dir.path(),
        None,
        None,
        &no_cancel(),
    )
    .expect("extracted");

    assert_eq!(n, 2, "the directory and the one file inside it");
    assert!(dir.path().join("Ünlü/naïve.txt").exists());
    assert!(
        !dir.path().join("köpek.txt").exists(),
        "nothing outside the selection may be written"
    );
}

// ---------------------------------------------------------------------------
// `stream_entry` — P17 §2. The uncapped read `indium cat` is built on.
// ---------------------------------------------------------------------------

/// Below the cap the two readers must be indistinguishable, or `cat` and Preview are
/// showing different files and only one of them can be right.
///
/// Every basic fixture, so the libarchive branch is exercised through zip, 7z and two
/// tar filters rather than through whichever one happened to be tried.
#[test]
fn stream_entry_writes_the_bytes_head_of_reads() {
    for name in ["basic.zip", "basic.7z", "basic.tar.gz", "basic.tar.zst"] {
        let path = fixture(name);
        for entry in arch::list_all(&path, None).expect("could not list") {
            if entry.is_dir {
                continue;
            }
            let (head, truncated) =
                arch::head_of(&path, &entry.path, 8 * 1024 * 1024, None).expect("head_of failed");
            assert!(
                !truncated,
                "{name}: a fixture member should not reach the cap"
            );

            let mut streamed = Vec::new();
            let n = arch::stream_entry(&path, &entry.path, None, &mut streamed)
                .unwrap_or_else(|e| panic!("{name}: stream_entry failed on {}: {e}", entry.path));

            assert_eq!(
                streamed, head,
                "{name}: stream_entry and head_of disagree about {}",
                entry.path
            );
            assert_eq!(
                n,
                streamed.len() as u64,
                "{name}: the returned count is not what was written for {}",
                entry.path
            );
        }
    }
}

/// **The test that catches the obvious wrong build.** `cat` implemented as
/// `head_of(.., PREVIEW_CAP, ..)` returns exactly 8 MiB and looks entirely correct on
/// every committed fixture, because none of them is anywhere near that size. So the
/// oversized member is built here rather than taken — a fixture cannot prove this.
///
/// Gzip, so nine megabytes of compressible bytes cost a few kilobytes on disk; and the
/// content is a counter rather than zeros, so a reader that silently produced a hole of
/// the right length would still fail.
#[test]
fn stream_entry_does_not_stop_where_the_preview_stops() {
    use std::io::Cursor;

    // `Sink` is the trait carrying `put` and `finish`; the writer is useless without it.
    use indium::tasks::{Meta, Method, Recipe, Sink};

    const BIG: usize = 9 * 1024 * 1024;
    let dir = TempDir::new("stream-big");
    let path = dir.path().join("big.tar.gz");

    let body: Vec<u8> = (0..BIG).map(|i| (i % 251) as u8).collect();
    let recipe = Recipe {
        path: path.clone(),
        method: Method::Gzip,
        level: Method::Gzip.default_level(),
        encrypt: false,
    };
    {
        let mut writer = arch::Writer::create(&path, &recipe).expect("could not open the writer");
        let meta = Meta {
            out_path: "big.bin".to_string(),
            size: BIG as u64,
            is_dir: false,
            mode: 0o644,
            mtime: Some(1_704_164_645),
            atime: None,
            ctime: None,
            uid: 0,
            gid: 0,
            uname: Some("root".to_string()),
            gname: Some("root".to_string()),
            symlink: None,
            hardlink: None,
        };
        let mut cursor = Cursor::new(body.clone());
        writer.put(&meta, Some(&mut cursor)).expect("could not put");
        writer.finish().expect("could not finish");
    }

    // The cap this must not inherit, named here so the test says what it is about.
    const PREVIEW_CAP: usize = 8 * 1024 * 1024;
    let (head, truncated) = arch::head_of(&path, "big.bin", PREVIEW_CAP, None).expect("head_of");
    assert_eq!(head.len(), PREVIEW_CAP, "head_of should stop at its cap");
    assert!(truncated, "head_of should report the member as truncated");

    let mut out = Vec::new();
    let n = arch::stream_entry(&path, "big.bin", None, &mut out).expect("stream_entry");

    assert_eq!(
        out.len(),
        BIG,
        "stream_entry stopped at {} bytes — a cat built on head_of returns {PREVIEW_CAP}",
        out.len()
    );
    assert_eq!(
        n, BIG as u64,
        "the returned count disagrees with what was written"
    );
    assert_eq!(
        indium::util::crc32(&out),
        indium::util::crc32(&body),
        "the bytes came back changed, not merely complete"
    );
}

/// A directory is an error rather than an empty success: `cat` on one is a mistake
/// everywhere else too, and returning `Ok(0)` would let a script mistake it for a file.
#[test]
fn stream_entry_refuses_a_directory_rather_than_writing_nothing() {
    let mut out = Vec::new();
    let err = arch::stream_entry(&fixture("basic.zip"), "sub", None, &mut out)
        .expect_err("a directory has no bytes to write");
    assert!(
        err.to_string().contains("directory"),
        "the refusal should say what sub is: {err}"
    );
    assert!(out.is_empty(), "nothing may be written for a directory");
}

/// RAR is refused by this path too, with CORE §5's exact sentence — not by accident but
/// because `stream_entry` goes through `Reader::open`, where the gate lives. A future
/// variant that opened the file another way would pass every other test and fail here.
#[test]
fn stream_entry_refuses_rar_with_the_exact_sentence() {
    let mut out = Vec::new();
    let err = arch::stream_entry(&fixture("notrar.rar"), "anything", None, &mut out)
        .expect_err("RAR must be refused");
    assert_eq!(err.to_string(), arch::RAR_REFUSAL);
    assert!(out.is_empty());
}
