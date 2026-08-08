//! The write path — P4 §7.
//!
//! These tests drive `arch::Writer` and the `tasks` fold against real archives on a real
//! filesystem, and they exist to prove one sentence from CORE §3: *"The original is never
//! touched until the replacement is proven."*
//!
//! Copyright © sudo-megas. GPL-3.0-only.

use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use indium::arch::{self, Entry};
use indium::tasks::{Meta, Method, Recipe, Sink};

// ---------------------------------------------------------------------------
// A temporary directory, hand-written.
//
// CORE §2's rule applies to test dependencies too: "makes a directory in /tmp for the
// tests" is not a sentence worth a crate. Same shape as `tests/read_path.rs`.
// ---------------------------------------------------------------------------

struct TempDir(PathBuf);

static COUNTER: AtomicU32 = AtomicU32::new(0);

impl TempDir {
    fn new(tag: &str) -> TempDir {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("indium-write-{tag}-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("could not make the test directory");
        TempDir(dir)
    }

    fn join(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const ALPHA: &[u8] = b"INDIUM fixture alpha\n";
const BETA: &[u8] = b"INDIUM fixture beta\n";

/// The payload every round-trip test writes: a directory, two files inside and out.
fn payload() -> Vec<(Meta, Option<Vec<u8>>)> {
    vec![
        (
            Meta {
                out_path: "alpha.txt".to_string(),
                size: ALPHA.len() as u64,
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
            },
            Some(ALPHA.to_vec()),
        ),
        (
            Meta {
                out_path: "sub".to_string(),
                size: 0,
                is_dir: true,
                mode: 0o755,
                mtime: Some(1_704_164_645),
                atime: None,
                ctime: None,
                uid: 0,
                gid: 0,
                uname: Some("root".to_string()),
                gname: Some("root".to_string()),
                symlink: None,
                hardlink: None,
            },
            None,
        ),
        (
            Meta {
                out_path: "sub/beta.txt".to_string(),
                size: BETA.len() as u64,
                is_dir: false,
                mode: 0o600,
                mtime: Some(1_704_164_645),
                atime: None,
                ctime: None,
                uid: 1234,
                gid: 5678,
                uname: Some("indiumuser".to_string()),
                gname: Some("indiumgroup".to_string()),
                symlink: None,
                hardlink: None,
            },
            Some(BETA.to_vec()),
        ),
    ]
}

/// Write the payload into `path` under `recipe`, through the real `Sink`.
fn write_payload(path: &Path, recipe: &Recipe) {
    let mut writer = arch::Writer::create(path, recipe).expect("could not open the writer");
    for (meta, data) in payload() {
        match data {
            Some(bytes) => {
                let mut cursor = Cursor::new(bytes);
                writer
                    .put(&meta, Some(&mut cursor))
                    .unwrap_or_else(|e| panic!("could not write {}: {e}", meta.out_path));
            }
            None => writer
                .put(&meta, None)
                .unwrap_or_else(|e| panic!("could not write {}: {e}", meta.out_path)),
        }
    }
    writer.finish().expect("could not finish the archive");
}

fn recipe(path: &Path, method: Method) -> Recipe {
    Recipe {
        path: path.to_path_buf(),
        method,
        level: method.default_level(),
        encrypt: false,
    }
}

fn find<'a>(entries: &'a [Entry], path: &str) -> &'a Entry {
    entries.iter().find(|e| e.path == path).unwrap_or_else(|| {
        panic!(
            "{path} is not in the archive; it holds {:?}",
            entries.iter().map(|e| &e.path).collect::<Vec<_>>()
        )
    })
}

// ---------------------------------------------------------------------------
// Round trips
// ---------------------------------------------------------------------------

/// CORE §5's write list: "`tar` with the filters `gz`, `bz2`, `xz`, `zst`, `lz4`; `zip`
/// (Deflate)". Every one of them must survive a write and a read.
#[test]
fn every_writable_format_round_trips_its_payload() {
    let dir = TempDir::new("formats");
    let cases = [
        ("plain.tar", Method::Store),
        ("out.tar.gz", Method::Gzip),
        ("out.tar.bz2", Method::Bzip2),
        ("out.tar.xz", Method::Xz),
        ("out.tar.zst", Method::Zstd),
        ("out.tar.lz4", Method::Lz4),
        ("out.zip", Method::Deflate),
    ];

    for (name, method) in cases {
        let path = dir.join(name);
        write_payload(&path, &recipe(&path, method));
        assert!(path.exists(), "{name} was not created");

        let entries = arch::list_all(&path, None)
            .unwrap_or_else(|e| panic!("{name} could not be read back: {e}"));

        let alpha = find(&entries, "alpha.txt");
        assert_eq!(alpha.size, ALPHA.len() as u64, "{name}: alpha.txt size");
        let beta = find(&entries, "sub/beta.txt");
        assert_eq!(beta.size, BETA.len() as u64, "{name}: sub/beta.txt size");
        assert!(
            find(&entries, "sub").is_dir,
            "{name}: sub must come back a directory"
        );
    }
}

/// CORE §1: "the metadata is the main event." A tar must carry every field INDIUM can
/// show, including the two that only tar can hold — owner names and numeric ids.
#[test]
fn a_tar_rebuild_carries_every_field_indium_can_show() {
    let dir = TempDir::new("meta");
    let path = dir.join("meta.tar");
    write_payload(&path, &recipe(&path, Method::Store));

    let entries = arch::list_all(&path, None).expect("could not read the tar back");
    let beta = find(&entries, "sub/beta.txt");

    assert_eq!(beta.uid, 1234, "uid must survive");
    assert_eq!(beta.gid, 5678, "gid must survive");
    assert_eq!(beta.uname.as_deref(), Some("indiumuser"), "owner name");
    assert_eq!(beta.gname.as_deref(), Some("indiumgroup"), "group name");
    assert_eq!(beta.mtime, Some(1_704_164_645), "mtime must survive");
    assert_eq!(beta.mode & 0o7777, 0o600, "mode must survive");
}

/// P4 §3's table says zip carries no owner names. The claim is asserted rather than
/// assumed, so the note the `W` popup shows before Apply stays true.
#[test]
fn a_zip_rebuild_loses_owner_names_as_the_table_says() {
    let dir = TempDir::new("ziploss");
    let path = dir.join("out.zip");
    write_payload(&path, &recipe(&path, Method::Deflate));

    let entries = arch::list_all(&path, None).expect("could not read the zip back");
    let beta = find(&entries, "sub/beta.txt");

    assert_eq!(
        beta.uname, None,
        "zip stores no owner name; the popup says so and must be right"
    );
    assert_eq!(beta.gname, None, "zip stores no group name");
    assert_eq!(
        beta.mode & 0o7777,
        0o600,
        "the mode does survive a zip, and is worth keeping true"
    );
}

/// P4 §3: "A failure to set options is a hard error, never swallowed. Silently dropping
/// the compression level you chose is the kind of quiet lie this program does not tell."
///
/// Every level in every method's own range must be accepted by libarchive. A range this
/// file claims but libarchive rejects would otherwise surface as a mystery at Apply.
#[test]
fn every_level_a_method_offers_is_one_libarchive_accepts() {
    let dir = TempDir::new("levels");
    for method in [
        Method::Gzip,
        Method::Bzip2,
        Method::Xz,
        Method::Zstd,
        Method::Lz4,
        Method::Deflate,
    ] {
        let range = method.levels().expect("these methods all take a level");
        for level in [*range.start(), *range.end()] {
            let path = dir.join(&format!("lvl-{}-{level}", method.label()));
            let recipe = Recipe {
                path: path.clone(),
                method,
                level,
                encrypt: false,
            };
            write_payload(&path, &recipe);
            let entries = arch::list_all(&path, None).unwrap_or_else(|e| {
                panic!("{} at level {level} could not be read: {e}", method.label())
            });
            assert_eq!(
                entries.len(),
                3,
                "{} at level {level} lost an entry",
                method.label()
            );
        }
    }
}

/// A member whose data is streamed in must arrive byte-identical, not merely the right
/// length. CRC32 is INDIUM's own, so this checks the bytes and not the header.
#[test]
fn a_written_members_bytes_are_the_bytes_that_went_in() {
    let dir = TempDir::new("bytes");
    let path = dir.join("out.tar.zst");
    write_payload(&path, &recipe(&path, Method::Zstd));

    let got = arch::crc32_of(&path, "alpha.txt", None).expect("could not checksum alpha.txt");
    assert_eq!(
        got,
        indium::util::crc32(ALPHA),
        "the bytes read back must be the bytes written"
    );
}

/// P4 §2: a build that fails must leave nothing usable behind, and must never be
/// mistaken for one that succeeded.
#[test]
fn a_writer_that_is_abandoned_does_not_leave_a_readable_archive() {
    let dir = TempDir::new("abandon");
    let path = dir.join("partial.tar.gz");

    {
        let mut writer =
            arch::Writer::create(&path, &recipe(&path, Method::Gzip)).expect("could not open");
        let (meta, data) = payload().into_iter().next().unwrap();
        let mut cursor = Cursor::new(data.unwrap());
        writer.put(&meta, Some(&mut cursor)).expect("could not put");
        writer.abandon();
    }

    // Whatever is on disk, it must not read back as a complete archive holding the
    // entry we wrote. Apply deletes this file; the point here is that abandoning never
    // produces something that could be mistaken for a finished build.
    match arch::list_all(&path, None) {
        Err(_) => {}
        Ok(entries) => assert!(
            entries.is_empty(),
            "an abandoned build must not read back as a finished archive"
        ),
    }
}

// ---------------------------------------------------------------------------
// 7z, in both directions — P4 §4
// ---------------------------------------------------------------------------

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// P4 §4: "An entry's packed size is reported when that entry is the sole occupant of
/// its block. When a block holds several entries, apportioning it between them would be
/// a guess, and INDIUM shows nothing rather than guess."
///
/// `basic.7z` is solid — one LZMA block holding all three files — and the crate stamps
/// the whole block's packed size onto whichever entry comes first. This is the fixture
/// that catches an implementation which checks for a non-zero size instead of counting
/// the block's occupants.
#[test]
fn a_solid_7z_reports_no_packed_size_for_any_entry() {
    let entries = indium::sevenz::list_all(&fixture("basic.7z"), None)
        .expect("basic.7z should list through sevenz-rust2");

    for entry in &entries {
        assert_eq!(
            entry.packed, None,
            "{} shares its block, so its packed size is not knowable",
            entry.path
        );
    }

    let (solid, blocks) = indium::sevenz::solid_info(&fixture("basic.7z"), None)
        .expect("basic.7z should report its block layout");
    assert!(solid, "basic.7z is a solid archive");
    assert_eq!(blocks, 1, "and it holds exactly one block");
}

/// P2 §6 wanted "with the passphrase, lists and extracts" and could not have it:
/// libarchive answers every header on this fixture with "currently not supported",
/// with or without the password, which P2 recorded as its first Deviation. P4 puts a
/// reader behind the prompt that has been waiting since then.
#[test]
fn secret_headers_7z_lists_with_its_password_at_last() {
    let path = fixture("secret-headers.7z");
    let secret = indium::secret::Secret::from_text("indium");

    let entries = indium::sevenz::list_all(&path, Some(&secret))
        .expect("an encrypted-header 7z must list once the password is known");
    assert_eq!(entries.len(), 1, "the fixture holds one member");
    assert_eq!(entries[0].path, "f.txt");
    assert!(entries[0].encrypted, "its block is AES-256");

    // Sole occupant of its block, so this one *can* be reported.
    assert_eq!(
        entries[0].packed,
        Some(48),
        "f.txt owns its block outright, so its packed size is knowable"
    );

    assert!(
        indium::sevenz::list_all(&path, None).is_err(),
        "without the password the names are ciphertext and must not be listed"
    );
}

/// CORE §5: "Encryption is 7z AES-256 and nothing else." The archive INDIUM writes must
/// be unreadable without the password and correct with it.
#[test]
fn an_aes256_7z_indium_wrote_needs_its_password_to_open() {
    let dir = TempDir::new("aes");
    let path = dir.join("secret.7z");
    let secret = indium::secret::Secret::from_text("indium");

    let recipe = Recipe {
        path: path.clone(),
        method: Method::Lzma2,
        level: 6,
        encrypt: true,
    };

    {
        let mut writer = indium::sevenz::Writer::create(&path, &recipe, Some(&secret))
            .expect("could not open the 7z writer");
        for (meta, data) in payload() {
            match data {
                Some(bytes) => {
                    let mut cursor = Cursor::new(bytes);
                    writer.put(&meta, Some(&mut cursor)).expect("could not put");
                }
                None => writer.put(&meta, None).expect("could not put"),
            }
        }
        writer.finish().expect("could not finish the 7z");
    }

    assert!(
        indium::sevenz::list_all(&path, None).is_err(),
        "an encrypted-header archive must not list without its password"
    );

    let entries = indium::sevenz::list_all(&path, Some(&secret))
        .expect("it must list with the right password");
    let alpha = entries
        .iter()
        .find(|e| e.path == "alpha.txt")
        .expect("alpha.txt must be in the archive");
    assert_eq!(alpha.size, ALPHA.len() as u64);
    assert!(alpha.encrypted, "every member is AES-256");
}

/// A plain 7z INDIUM writes must carry its metadata back, and be readable by the other
/// reader too — libarchive, a genuinely independent implementation.
#[test]
fn a_plain_7z_round_trips_through_both_readers() {
    let dir = TempDir::new("sevenz");
    let path = dir.join("out.7z");
    let recipe = Recipe {
        path: path.clone(),
        method: Method::Lzma2,
        level: 6,
        encrypt: false,
    };

    {
        let mut writer = indium::sevenz::Writer::create(&path, &recipe, None)
            .expect("could not open the 7z writer");
        for (meta, data) in payload() {
            match data {
                Some(bytes) => {
                    let mut cursor = Cursor::new(bytes);
                    writer.put(&meta, Some(&mut cursor)).expect("could not put");
                }
                None => writer.put(&meta, None).expect("could not put"),
            }
        }
        writer.finish().expect("could not finish the 7z");
    }

    let ours = indium::sevenz::list_all(&path, None).expect("our own reader must read it");
    let theirs = arch::list_all(&path, None).expect("libarchive must read it too");

    let mut a: Vec<&str> = ours.iter().map(|e| e.path.as_str()).collect();
    let mut b: Vec<&str> = theirs.iter().map(|e| e.path.as_str()).collect();
    a.sort_unstable();
    b.sort_unstable();
    assert_eq!(
        a, b,
        "the two readers must agree on the entry list, or routing listing to one and \
         data to the other is unsound"
    );

    let beta = find(&ours, "sub/beta.txt");
    assert_eq!(beta.mtime, Some(1_704_164_645), "mtime must survive");
    assert_eq!(beta.mode & 0o7777, 0o600, "the unix mode must survive");
}
