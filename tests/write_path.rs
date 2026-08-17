//! The write path — P4 §7.
//!
//! These tests drive both write backends — `arch::Writer` and, for 7z, `sevenz::Writer` —
//! and the `tasks` fold against real archives on a real filesystem, and they exist to prove
//! one sentence from CORE §3: *"The original is never touched until the replacement is
//! proven."* The routing is `write_payload`'s, which is `tasks`' own two-arm match.
//!
//! Copyright © sudo-megas. GPL-3.0-only.

use std::fs;
use std::io::Cursor;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use indium::arch::{self, Entry};
use indium::tasks::{Container, Meta, Method, Recipe, Sink};

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

/// Write the standard payload into `path` through whichever backend the recipe's container
/// names, using the real `Sink`.
///
/// It reached `arch::Writer` unconditionally until P18, which is why every test built on it
/// silently excluded 7z — the one writable container libarchive does not handle. The routing
/// here is the same two-arm match `tasks` does at Apply, so a test that uses this exercises the
/// backend the window would have used.
fn write_payload(path: &Path, recipe: &Recipe) {
    let mut writer: Box<dyn Sink> = match recipe.container() {
        indium::tasks::Container::SevenZ => Box::new(
            indium::sevenz::Writer::create(path, recipe, None)
                .expect("could not open the 7z writer"),
        ),
        _ => Box::new(arch::Writer::create(path, recipe).expect("could not open the writer")),
    };
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

/// PXX. An archive with one member large enough that verifying it takes real time, and a
/// small one beside it for Apply to remove.
///
/// Zeros, deliberately, and that is the whole trick: gzip stores 64 MiB of them in a few
/// kilobytes, so the fixture costs almost nothing on disk and builds in a moment — while
/// `list_all` must still inflate every one of those bytes to walk to the end, because a
/// gzip stream cannot be seeked past. A cheap fixture with an expensive verify is exactly
/// what a test aiming at the verify window needs.
fn write_big_and_small(path: &Path, recipe: &Recipe, bytes: usize) {
    fn meta(out_path: &str, size: u64) -> Meta {
        Meta {
            out_path: out_path.to_string(),
            size,
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
        }
    }

    let mut writer = arch::Writer::create(path, recipe).expect("could not open the writer");
    let mut zeros = Cursor::new(vec![0u8; bytes]);
    writer
        .put(&meta("big.bin", bytes as u64), Some(&mut zeros))
        .expect("could not write the big member");
    let mut small = Cursor::new(ALPHA.to_vec());
    writer
        .put(&meta("small.txt", ALPHA.len() as u64), Some(&mut small))
        .expect("could not write the small member");
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

/// PXX 9.5: naming a folder that is not there must say so, in a sentence.
///
/// The walk chose `~/indium-test/large/` as the destination, which did not exist, and got
/// back:
///
/// ```text
/// could not open the 7z for writing: Io(Os { code: 2, kind: NotFound,
///   message: "No such file or directory" }, "…/large/.archivesadfad.7z.indium-new")
/// ```
///
/// Three faults in one line. It printed a Rust struct at a person. It named
/// `.indium-new` — an internal temp file nobody asked for and nobody can act on. And it
/// never mentioned the one thing that would have fixed it: the folder is missing.
///
/// **Every container, not just the one that was demonstrated.** The walk built a `.7z` and
/// so that is the branch with the Rust struct in it, but a `.tar.gz` into the same missing
/// folder went to libarchive and came back `Failed to open '…/.archive.tar.gz.indium-new'`
/// — a different sentence with the same two faults left in it. A round that freezes the
/// repository forever does not get to fix the format that was reported and leave the rest.
///
/// What is handed to each writer is the **temp path**, `.name.indium-new`, because that is
/// what `build_and_verify` hands it. Passing the plain name here would let the no-leak
/// assertion pass on a message that had no `.indium-new` available to leak.
#[test]
fn a_missing_destination_folder_is_named_plainly() {
    let dir = TempDir::new("nodir");
    let missing = dir.join("not-there");

    for (name, method) in [
        ("archive.7z", Method::Lzma2),
        ("archive.tar", Method::Store),
        ("archive.tar.gz", Method::Gzip),
        ("archive.tar.xz", Method::Xz),
        ("archive.tar.zst", Method::Zstd),
        ("archive.tar.bz2", Method::Bzip2),
        ("archive.zip", Method::Deflate),
    ] {
        // The recipe names the archive a person asked for; the writer is opened on the
        // temp file beside it. Both come from `Apply`, and only the first is speakable.
        let target = missing.join(name);
        let recipe = recipe(&target, method);
        let temp = missing.join(format!(".{name}.indium-new"));

        // `Writer` carries a live archive handle and deliberately implements no `Debug`, so
        // neither arm of this can be an `expect_err`.
        let err = match recipe.container() {
            Container::SevenZ => match indium::sevenz::Writer::create(&temp, &recipe, None) {
                Err(e) => e,
                Ok(_) => panic!("{name}: writing into a folder that does not exist must fail"),
            },
            _ => match arch::Writer::create(&temp, &recipe) {
                Err(e) => e,
                Ok(_) => panic!("{name}: writing into a folder that does not exist must fail"),
            },
        };

        assert!(
            err.contains(&missing.display().to_string()),
            "{name}: the message must name the missing folder: {err:?}"
        );
        for leak in ["Io(", "Os {", "code: 2", ".indium-new", "Failed to open"] {
            assert!(
                !err.contains(leak),
                "{name}: the message still leaks {leak:?}: {err:?}"
            );
        }
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

/// CORE §5's write list: "`tar`, plain or with the filters `gz`, `bz2`, `xz`, `zst`, `lz4`;
/// `zip` (Deflate); `7z` (LZMA2, via `sevenz-rust2`)". Every one of them must survive a write
/// and a read.
///
/// **That quotation used to stop one clause early.** It ended at `zip` (Deflate) and dropped
/// the 7z, which is what made "every writable format" true of a list of seven — the name was
/// kept honest by shortening the document instead of lengthening the test. 7z was covered
/// elsewhere in this file, so nothing was unproven; what was wrong was the citation, and a
/// doctored quotation of the authoritative document is worse than the gap it hides. P18.
///
/// The cases are now taken from `METHODS` through an exhaustive `match` with no `_` arm, so a
/// ninth method does not compile until somebody round-trips it.
#[test]
fn every_writable_format_round_trips_its_payload() {
    let dir = TempDir::new("formats");
    let cases = indium::tasks::METHODS.map(|method| {
        let name = match method {
            Method::Store => "plain.tar",
            Method::Gzip => "out.tar.gz",
            Method::Bzip2 => "out.tar.bz2",
            Method::Xz => "out.tar.xz",
            Method::Zstd => "out.tar.zst",
            Method::Lz4 => "out.tar.lz4",
            Method::Deflate => "out.zip",
            Method::Lzma2 => "out.7z",
        };
        (name, method)
    });

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
///
/// **Every** means every, since P18: this walked only `[start, end]` before, so zstd was
/// checked at 1 and 22 and never at the twenty levels between them. The full sweep costs
/// about six tenths of a second, and no new peak memory — level 22 was already one of the
/// two ends, and it is the expensive one.
///
/// The six here are the methods libarchive is handed. **`Method::Lzma2` is deliberately
/// absent**: it routes to `sevenz-rust2` and never reaches libarchive at all, so adding it
/// would make this test's own name false. Its range is checked by
/// `every_lzma2_level_the_slider_offers_builds_a_7z_that_reads_back` below, which is where
/// that claim can be true.
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
        for level in range {
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

/// The other half of the same claim, for the one method the test above cannot make it about.
///
/// `Method::Lzma2` goes to `sevenz-rust2`, so "libarchive accepts it" is not a sentence that
/// can be true of it — and until P18 that meant nothing anywhere checked LZMA2's advertised
/// `0..=9` at all, while the slider offered every one of them.
///
/// **What this cannot prove, said plainly, because it was tried.** Widening LZMA2's range to
/// `0..=99` and re-running this leaves it green. `clamp_level` clamps against `levels()`
/// itself, so it can only ever hold a level inside whatever range is declared, and
/// `sevenz-rust2` does not refuse the result — where libarchive refuses `zstd:23` at writer
/// creation, which is what gives the test above its teeth. So this proves every level the
/// window offers builds a 7z both readers can read; it does **not** prove the range is the
/// right one, and no test here can. Nor does it prove level 9 compresses harder than level 6:
/// asserting a size difference on a payload this small would be a lie dressed as a check.
#[test]
fn every_lzma2_level_the_slider_offers_builds_a_7z_that_reads_back() {
    let dir = TempDir::new("lzma2-levels");
    let range = Method::Lzma2
        .levels()
        .expect("LZMA2 takes a level, and the New Archive slider offers it");
    for level in range {
        let path = dir.join(&format!("lvl-{level}.7z"));
        let recipe = Recipe {
            path: path.clone(),
            method: Method::Lzma2,
            level,
            encrypt: false,
        };
        write_payload(&path, &recipe);

        let ours = indium::sevenz::list_all(&path, None)
            .unwrap_or_else(|e| panic!("LZMA2 at level {level}: our own reader refused it: {e}"));
        let theirs = arch::list_all(&path, None)
            .unwrap_or_else(|e| panic!("LZMA2 at level {level}: libarchive refused it: {e}"));
        assert_eq!(ours.len(), 3, "LZMA2 at level {level} lost an entry");
        assert_eq!(
            theirs.len(),
            3,
            "LZMA2 at level {level} lost an entry for the other reader"
        );
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
///
/// **`PXX-T3-023`. For its whole life this compared one reader with itself.** The second list came
/// from `arch::list_all`, and `arch::list_all` calls `list_7z` first for any 7z and only falls
/// through to libarchive on `None` — so both sides were `sevenz`, and the assertion below could not
/// fail on the only thing it claims to check. Class 4 in textbook form, and the claim it was
/// protecting is the exact routing decision at the heart of `PXX-2-002` and `9175a28`.
///
/// It now walks `arch::Reader`, which is libarchive and nothing else. The two readers do agree —
/// that was measured when the defect was found, and this is the gate that will notice if they ever
/// stop.
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

    // `Reader` and not `arch::list_all`: the latter routes any 7z to `sevenz` and would compare
    // `ours` with itself, which is what this gate did until v2.5.
    let mut theirs: Vec<String> = Vec::new();
    {
        let mut reader = arch::Reader::open(&path, None).expect("libarchive must open it too");
        while let Some(entry) = reader.next_entry().expect("libarchive must walk it") {
            theirs.push(entry.path.clone());
            reader.skip_data();
        }
    }
    assert!(
        !theirs.is_empty(),
        "libarchive returned no entries at all — the walk is the check, and an empty walk \
         would pass the comparison below while measuring nothing"
    );

    let mut a: Vec<&str> = ours.iter().map(|e| e.path.as_str()).collect();
    let mut b: Vec<&str> = theirs.iter().map(|s| s.as_str()).collect();
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

// ---------------------------------------------------------------------------
// Apply — P4 §2. "The original is never touched until the replacement is proven."
// ---------------------------------------------------------------------------

use indium::tasks::{self, AddItem, ApplyInput, ApplyMsg, Task};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::channel;
use std::sync::Arc;

fn no_cancel() -> Arc<AtomicBool> {
    Arc::new(AtomicBool::new(false))
}

/// Run Apply and collect what it reported.
fn run_apply(
    input: &ApplyInput,
    cancel: &Arc<AtomicBool>,
) -> (Result<usize, String>, Vec<ApplyMsg>) {
    let (tx, rx) = channel();
    let result = tasks::apply(input, &tx, cancel);
    drop(tx);
    (result, rx.into_iter().collect())
}

fn input_for(path: &Path, tasks: Vec<Task>) -> ApplyInput {
    ApplyInput {
        target: path.to_path_buf(),
        recipe: recipe(path, Method::Gzip),
        tasks,
        adds: Vec::new(),
        staged_against: Vec::new(),
        source_password: None,
        target_password: None,
    }
}

/// The load-bearing one. Apply with nothing staged must reproduce the archive it read,
/// or every stronger claim in this milestone rests on nothing.
#[test]
fn an_apply_with_no_tasks_reproduces_the_archive() {
    let dir = TempDir::new("identity");
    let path = dir.join("out.tar.gz");
    write_payload(&path, &recipe(&path, Method::Gzip));

    let before = arch::list_all(&path, None).expect("could not list before");
    let (result, _) = run_apply(&input_for(&path, Vec::new()), &no_cancel());
    result.expect("an empty Apply must succeed");

    let after = arch::list_all(&path, None).expect("could not list after");
    assert_eq!(
        before.len(),
        after.len(),
        "the entry count must be unchanged"
    );
    for old in &before {
        let new = find(&after, &old.path);
        assert_eq!(new.size, old.size, "{}: size", old.path);
        assert_eq!(new.mtime, old.mtime, "{}: mtime", old.path);
        assert_eq!(new.uid, old.uid, "{}: uid", old.path);
        assert_eq!(new.uname, old.uname, "{}: owner name", old.path);
        assert_eq!(new.mode & 0o7777, old.mode & 0o7777, "{}: mode", old.path);
    }
    assert!(
        !tasks::temp_path_for(&path).exists(),
        "the temp file must not survive a successful Apply"
    );
}

/// **`PXX-T2-015`, the loud half.** Apply over a `./`-rooted archive.
///
/// `apply` re-lists through `list_all`, which drops the archive root, so `plan.source` and
/// `expected()` are root-filtered — while the rebuild loop walked `next_entry()` raw, root
/// included, and paired the two by position. Two lists differing by one element, walking one and
/// indexing the other.
///
/// `rooted.tar` stores beta (20 bytes) before alpha (21). The record captured
/// `"alpha.txt was written at 20 bytes instead of 21"` from a real run rounds ago, which is why that
/// finding was misfiled as a `verify_against` defect. It was never a false alarm — the rebuild really
/// was wrong and `verify_against` was the only thing that noticed.
///
/// **But that is not the sentence this gate produces, and an earlier version of this comment said it
/// was.** Remove the skip today and it fails earlier and louder, inside `list_all`, with
/// `"Damaged tar archive (bad header checksum)"`: the shifted member is written under the stored name
/// `"./sub/"`, a trailing slash is a directory to libarchive as much as to `arch.rs:673`, so the
/// payload is never consumed as data and the next header is read from inside it.
///
/// So this gate proves the rebuild is wrong, not that it is wrong *silently*.
/// `a_rooted_archive_of_equal_length_members_is_not_silently_shuffled` is the only one of the three
/// root gates that shows Apply returning success over a corrupted archive.
#[test]
fn an_apply_over_a_dot_slash_rooted_archive_keeps_every_member() {
    let dir = TempDir::new("rooted-apply");
    let path = dir.join("rooted.tar");
    std::fs::copy(fixture("rooted.tar"), &path).expect("could not stage rooted.tar");

    let before = arch::list_all(&path, None).expect("could not list before");
    let input = ApplyInput {
        target: path.clone(),
        recipe: recipe(&path, Method::Store),
        tasks: Vec::new(),
        adds: Vec::new(),
        staged_against: Vec::new(),
        source_password: None,
        target_password: None,
    };
    let (result, _) = run_apply(&input, &no_cancel());
    result.expect("an empty Apply over a ./-rooted archive must succeed");

    let after = arch::list_all(&path, None).expect("could not list after");
    assert_eq!(
        after.len(),
        before.len(),
        "no member may be gained or lost: {before:?} became {after:?}"
    );
    for old in &before {
        let new = find(&after, &old.path);
        assert_eq!(new.size, old.size, "{}: size", old.path);
        assert_eq!(
            new.is_dir, old.is_dir,
            "{}: a file must not come back as a directory",
            old.path
        );
    }
}

/// **`PXX-T2-015`, the half that matters.** The same shift, with the sizes made to agree.
///
/// `rooted.tar` fails loudly by luck rather than by guard: it errors only because two shifted
/// members happen to differ in length. Give every member the *same* length and the shift becomes
/// invisible to every check `verify_against` makes — the path multiset still matches exactly, and
/// the size comparison is gated on `is_regular_file`, so the one member destroyed outright (turned
/// into a directory) is the one member the check skips. Apply then reports success and renames the
/// corrupted rebuild over the user's original.
///
/// So this gate asserts **bytes by name**, which is the only thing that can see it, and it
/// deliberately does not assert sizes: after the shift, sizes are the thing that coincides. It is
/// the experiment that takes the silent-commit claim from `probable` to measured, and it is built
/// rather than committed because a fixture is not something this repo puts in its history.
#[test]
fn a_rooted_archive_of_equal_length_members_is_not_silently_shuffled() {
    let dir = TempDir::new("rooted-equal");
    let payload = dir.join("payload");
    std::fs::create_dir_all(&payload).expect("payload dir");

    // Four members, all exactly nine bytes, distinct contents. Equal lengths are the point.
    let members: [(&str, &[u8]); 4] = [
        ("a.txt", b"aaaaaaaa\n"),
        ("b.txt", b"bbbbbbbb\n"),
        ("c.txt", b"cccccccc\n"),
        ("d.txt", b"dddddddd\n"),
    ];
    for (name, body) in members {
        std::fs::write(payload.join(name), body).expect("could not write a member");
    }

    // `tar -C dir .` is what writes a `./` root, and it is the ordinary way to make a tarball of
    // a directory's contents — which is why this shape is not exotic.
    let path = dir.join("equal.tar");
    let ok = std::process::Command::new("tar")
        .arg("-cf")
        .arg(&path)
        .arg("-C")
        .arg(&payload)
        .arg(".")
        .status()
        .expect("tar must be runnable")
        .success();
    assert!(ok, "tar could not build the fixture");

    // Independently of INDIUM's own filtering: the source really does carry a root member.
    let raw = std::process::Command::new("tar")
        .arg("-tf")
        .arg(&path)
        .output()
        .expect("tar -tf must run");
    let listed = String::from_utf8_lossy(&raw.stdout);
    assert!(
        listed.lines().any(|l| l == "./"),
        "the fixture is only the case if it carries a ./ root: {listed:?}"
    );

    let input = ApplyInput {
        target: path.clone(),
        recipe: recipe(&path, Method::Store),
        tasks: Vec::new(),
        adds: Vec::new(),
        staged_against: Vec::new(),
        source_password: None,
        target_password: None,
    };
    let (result, _) = run_apply(&input, &no_cancel());
    result.expect("an empty Apply must succeed");

    // The assertion that can see a shift. Before the fix every one of these four held its
    // neighbour's bytes, `a.txt` was a directory, and `d.txt` had been dropped — and Apply
    // returned success, because nothing it checked could tell.
    for (name, want) in members {
        let (got, _) = arch::head_of(&path, name, 64, None)
            .unwrap_or_else(|e| panic!("{name} could not be read back: {e:?}"));
        assert_eq!(
            got, want,
            "{name} came back holding another member's bytes — the rebuild is walking a \
             different list from the one it was planned against"
        );
    }

    let after = arch::list_all(&path, None).expect("could not list after");
    assert_eq!(
        after.len(),
        4,
        "four members in, four members out: {after:?}"
    );
    assert!(
        after.iter().all(|e| !e.is_dir),
        "not one of these is a directory, and a rebuild must not invent one: {after:?}"
    );
}

/// Build a `./`-rooted tar of equal-length members with `tar -C dir .`, which is both the ordinary
/// way to tar a directory's contents and the thing that writes the root member.
///
/// Equal lengths are the point everywhere this is used: after a misalignment, sizes are the thing
/// that coincides, so a gate built on them passes over a corrupted archive by construction.
fn rooted_equal_tar(dir: &TempDir, name: &str, members: &[(&str, &[u8])]) -> PathBuf {
    let payload = dir.join(&format!("payload-{name}"));
    fs::create_dir_all(payload.join("sub")).expect("payload dir");
    for (member, body) in members {
        fs::write(payload.join(member), body).expect("could not write a member");
    }
    let path = dir.join(name);
    let ok = std::process::Command::new("tar")
        .arg("-cf")
        .arg(&path)
        .arg("-C")
        .arg(&payload)
        .arg(".")
        .status()
        .expect("tar must be runnable — this gate needs it and must not vacate quietly")
        .success();
    assert!(ok, "tar could not build the fixture");

    let raw = std::process::Command::new("tar")
        .arg("-tf")
        .arg(&path)
        .output()
        .expect("tar -tf must run");
    let listed = String::from_utf8_lossy(&raw.stdout).to_string();
    assert!(
        listed.lines().any(|l| l == "./"),
        "the fixture is only the case if it carries a ./ root: {listed:?}"
    );
    path
}

/// **`PXX-T2-015`, the coverage hole its own fix left.** Apply with *staged mutations* over a
/// `./`-rooted archive.
///
/// Every measurement made of the root skip — both of its gates, and the probes a tier-3 reviewer
/// left behind before dying — used an **empty task list**. That exercises the rebuild loop's
/// alignment and nothing downstream of it. `Task::Rename` rewrites the `staged` vector by position
/// and carries a directory's children with it, and `expected()` is built from the same vector, so a
/// root that consumed a slot would misalign the *plan* as well as the walk. An empty queue cannot
/// see that, because with nothing renamed every `out_path` equals its source path and a shift of the
/// plan looks exactly like no shift at all.
///
/// Four nine-byte members, so no assertion here can lean on a size.
#[test]
fn a_rooted_archive_survives_staged_renames_and_removes() {
    let dir = TempDir::new("rooted-staged");
    let path = rooted_equal_tar(
        &dir,
        "staged.tar",
        &[
            ("a.txt", b"aaaaaaaa\n"),
            ("b.txt", b"bbbbbbbb\n"),
            ("sub/c.txt", b"cccccccc\n"),
            ("sub/d.txt", b"dddddddd\n"),
        ],
    );

    // A rename, a remove, and a directory rename that must take its two children with it.
    let tasks_list = vec![
        Task::Rename {
            from: "a.txt".to_string(),
            to: "z.txt".to_string(),
        },
        Task::Remove {
            path: "b.txt".to_string(),
        },
        Task::Rename {
            from: "sub".to_string(),
            to: "dir2".to_string(),
        },
    ];
    let input = ApplyInput {
        target: path.clone(),
        recipe: recipe(&path, Method::Store),
        tasks: tasks_list,
        adds: Vec::new(),
        staged_against: Vec::new(),
        source_password: None,
        target_password: None,
    };
    let (result, _) = run_apply(&input, &no_cancel());
    result.expect("a staged Apply over a ./-rooted archive must succeed");

    // Bytes by name, which is the only assertion that can see a shift.
    for (name, want) in [
        ("z.txt", &b"aaaaaaaa\n"[..]),
        ("dir2/c.txt", &b"cccccccc\n"[..]),
        ("dir2/d.txt", &b"dddddddd\n"[..]),
    ] {
        let (got, _) = arch::head_of(&path, name, 64, None)
            .unwrap_or_else(|e| panic!("{name} could not be read back: {e:?}"));
        assert_eq!(
            got, want,
            "{name} holds the wrong member's bytes — the rename moved a name but the rebuild \
             moved a different member's contents under it"
        );
    }

    let after = arch::list_all(&path, None).expect("could not list after");
    let names: std::collections::BTreeSet<&str> = after.iter().map(|e| e.path.as_str()).collect();
    assert!(
        !names.contains("b.txt") && !names.contains("a.txt"),
        "the removed member and the old name must both be gone: {names:?}"
    );
    assert!(
        names.contains("z.txt") && names.contains("dir2/c.txt") && names.contains("dir2/d.txt"),
        "the renamed member and both carried children must be present: {names:?}"
    );
    assert!(
        after.iter().all(|e| !e.is_dir || e.path == "dir2"),
        "nothing but the renamed directory may come back as a directory: {after:?}"
    );
}

/// A removed entry goes, and every survivor keeps its exact bytes.
#[test]
fn a_removed_entry_is_gone_and_every_survivor_is_byte_identical() {
    let dir = TempDir::new("remove");
    let path = dir.join("out.tar.gz");
    write_payload(&path, &recipe(&path, Method::Gzip));

    let before = arch::crc32_of(&path, "alpha.txt", None).expect("could not checksum before");

    let tasks_list = vec![Task::Remove {
        path: "sub/beta.txt".to_string(),
    }];
    let (result, _) = run_apply(&input_for(&path, tasks_list), &no_cancel());
    result.expect("the removal must apply");

    let after = arch::list_all(&path, None).expect("could not list after");
    assert!(
        after.iter().all(|e| e.path != "sub/beta.txt"),
        "the removed entry must be gone"
    );
    assert_eq!(
        arch::crc32_of(&path, "alpha.txt", None).expect("could not checksum after"),
        before,
        "a survivor's bytes must not change because a sibling was removed"
    );
}

/// CORE §3's sentence, tested directly. The failure is injected honestly — an add whose
/// source file does not exist — rather than by a fault harness.
#[test]
fn a_failed_apply_leaves_the_original_untouched() {
    let dir = TempDir::new("fail");
    let path = dir.join("out.tar.gz");
    write_payload(&path, &recipe(&path, Method::Gzip));

    let original = fs::read(&path).expect("could not snapshot the original");

    let mut input = input_for(&path, Vec::new());
    input.adds = vec![AddItem {
        source: dir.join("does-not-exist.txt"),
        out_path: "ghost.txt".to_string(),
    }];

    let (result, _) = run_apply(&input, &no_cancel());
    assert!(result.is_err(), "an add with no source must fail the Apply");

    assert_eq!(
        fs::read(&path).expect("the original must still be readable"),
        original,
        "the original must be byte-for-byte what it was"
    );
    assert!(
        !tasks::temp_path_for(&path).exists(),
        "a failed Apply must not leave its temp file behind"
    );
}

/// A cancelled Apply must be distinguishable from a finished one, and must leave the
/// original alone. This is the bug extraction still has and Apply must not inherit.
#[test]
fn a_cancelled_apply_leaves_the_original_untouched_and_says_so() {
    let dir = TempDir::new("cancel");
    let path = dir.join("out.tar.gz");
    write_payload(&path, &recipe(&path, Method::Gzip));

    let original = fs::read(&path).expect("could not snapshot the original");

    // Cancelled before the first member is written.
    let cancel = Arc::new(AtomicBool::new(true));
    let (result, messages) = run_apply(&input_for(&path, Vec::new()), &cancel);
    result.expect("a cancelled Apply is not an error");

    assert!(
        messages.iter().any(|m| matches!(m, ApplyMsg::Cancelled)),
        "a cancelled Apply must say so, and never look like a finished one"
    );
    assert_eq!(
        fs::read(&path).expect("the original must still be readable"),
        original,
        "the original must be untouched"
    );
    assert!(
        !tasks::temp_path_for(&path).exists(),
        "a cancelled Apply must remove its temp file"
    );
}

/// PXX. Its sibling above cancels before the first member is written. This one cancels
/// while Apply is **verifying**, which until this round committed the replacement anyway.
///
/// `build_and_verify` consulted the flag for the last time *before* the verify pass, and
/// that pass is a full decompress of what was just written — on a GB-scale rebuild,
/// minutes, and precisely the stretch during which someone who has changed their mind
/// reaches for the button. It returned `Ok(Some)` regardless and `apply` renamed, so the
/// archive was replaced while the window's Cancelled arm said "Nothing was written".
/// Ignoring a Cancel is a defect; doing the opposite of one is a worse defect.
///
/// The synchronisation is stated rather than hidden. `Phase::Verifying`'s first message
/// is sent *after* the pre-verify check and *before* the decompress, so a thread already
/// blocked on the channel is woken inside the window by construction — not by luck. What
/// remains is whether the flag is set before that decompress ends, and the margin is four
/// orders of magnitude: a thread wakeup in microseconds against the hundreds of
/// milliseconds it takes to inflate 64 MiB. That margin is why the fixture is large
/// rather than convenient, and it is the only reason this test is not flaky.
#[test]
fn a_cancel_arriving_during_verification_does_not_replace_the_archive() {
    let dir = TempDir::new("cancel-verify");
    let path = dir.join("out.tar.gz");
    write_big_and_small(&path, &recipe(&path, Method::Gzip), 64 << 20);

    let original = fs::read(&path).expect("could not snapshot the original");
    let cancel = Arc::new(AtomicBool::new(false));
    let (tx, rx) = channel();

    // Something to do, so the rebuild is real: with an empty task list Apply still
    // rebuilds, but a removal makes the "did it commit?" question visible in the listing
    // as well as in the bytes.
    let input = input_for(
        &path,
        vec![Task::Remove {
            path: "small.txt".to_string(),
        }],
    );

    let (result, mut seen) = std::thread::scope(|scope| {
        let handle = scope.spawn(|| tasks::apply(&input, &tx, &cancel));

        let mut seen = Vec::new();
        for msg in &rx {
            let verifying = matches!(
                msg,
                ApplyMsg::Progress {
                    phase: tasks::Phase::Verifying,
                    ..
                }
            );
            seen.push(msg);
            if verifying {
                break;
            }
        }
        cancel.store(true, Ordering::Relaxed);

        (
            handle.join().expect("the apply thread must not panic"),
            seen,
        )
    });
    drop(tx);
    seen.extend(rx);

    assert_eq!(
        result.expect("a cancelled Apply is not an error"),
        0,
        "a cancelled Apply reports nothing written"
    );
    assert!(
        seen.iter().any(|m| matches!(m, ApplyMsg::Cancelled)),
        "the cancel must be reported as a cancel"
    );
    assert!(
        !seen.iter().any(|m| matches!(m, ApplyMsg::Done { .. })),
        "a cancelled Apply must never also report Done — the window would show the \
         replacement as finished"
    );
    assert_eq!(
        fs::read(&path).expect("the original must still be readable"),
        original,
        "the original archive must be byte-identical: the Cancelled arm promises \
         'Nothing was written'"
    );
    assert!(
        arch::list_all(&path, None)
            .expect("the original must still list")
            .iter()
            .any(|e| e.path == "small.txt"),
        "the entry the cancelled Apply would have removed must still be in the archive"
    );
    assert!(
        !tasks::temp_path_for(&path).exists(),
        "a cancelled Apply must remove its temp file, however late the cancel arrived"
    );
}

/// P3 §4's carried-forward requirement: two windows must not rebuild one archive. The
/// test takes the lock itself, exactly as a second window would.
#[test]
fn a_second_apply_on_a_locked_archive_refuses() {
    let dir = TempDir::new("locked");
    let path = dir.join("out.tar.gz");
    write_payload(&path, &recipe(&path, Method::Gzip));
    let original = fs::read(&path).expect("could not snapshot");

    let held = tasks::Lock::take(&path).expect("the first lock must be granted");

    let (result, _) = run_apply(&input_for(&path, Vec::new()), &no_cancel());
    let err = result.expect_err("a second Apply must refuse while the first holds the lock");
    assert!(
        err.contains("Another INDIUM window"),
        "it must say why, in the sentence P4 §2 fixes: {err}"
    );
    assert_eq!(
        fs::read(&path).expect("the original must be readable"),
        original,
        "a refused Apply writes nothing"
    );

    drop(held);
    let (result, _) = run_apply(&input_for(&path, Vec::new()), &no_cancel());
    result.expect("once the lock is released, Apply must proceed");
}

/// **The same class as `PXX-T3-003`, on a target that matters more than a settings file.**
///
/// Apply commits by renaming a *fresh inode* over the user's archive (`tasks.rs:1545`), and
/// nothing in `apply` or `build_and_verify` captures the mode the archive had. So an archive the
/// user tightened to `0600` comes back at `0o666 & ~umask` — and for an AES-256 7z that means the
/// ciphertext sits world-readable after any rebuild, with nothing said.
///
/// Found by a blind tier-2 confirmer as an adjacent site while it was measuring the identical
/// mechanism in `store::atomic_write`, and filed `probable` because it had not been run against a
/// real Apply. This test is that run.
#[test]
fn an_apply_does_not_widen_the_mode_of_the_archive_it_rebuilds() {
    use std::os::unix::fs::PermissionsExt;

    let dir = TempDir::new("apply-mode");

    // Two modes, and the second is not decoration.
    //
    // `0o640` and not `0o600`: tier 3 mutated the shipping code to `from_mode(0o600)` and the
    // whole suite passed, because the fixture asked for exactly the constant a hardcoding
    // implementation would choose. `0o640` is neither that nor the umask default `0o644`.
    //
    // `0o666` adds a third triad — world-write — to what this gate proves about mode fidelity,
    // and that is all it does. It does **not** re-arm the exact restore, and the attempt to make
    // it do so is recorded here rather than dropped.
    //
    // Tier 3 found this gate disarmed: with the temp staged at the archive's mode, `umask 022`
    // lands a `0o640` fixture at `0o640` anyway, so the whole restore block deleted green at
    // `5ccdfcb` while reddening at its parent. `0o666` was added on the reasoning that bits
    // `umask 022`, `002` and `077` all clear would need the restore to come back. **Measured,
    // that is false**, and it is false because of the fix in the same commit: the staging now
    // chmods the descriptor to `mode | 0o600`, which for any mode already carrying owner
    // read-write *is* the archive's mode, so the restore has nothing left to do. The restore is
    // load-bearing only where owner bits are missing — and that is pinned, by
    // `an_apply_on_an_archive_its_owner_cannot_write_is_still_rebuilt`, which reds with
    // `asked for 400, got 600` when the restore is disabled.
    //
    // The lesson is the round's own: a fix aimed at one gate changed what a second gate could
    // see, and only sabotaging both revealed which one now holds the property.
    for want in [0o640u32, 0o666u32] {
        let path = dir.join(&format!("out-{want:o}.tar.gz"));
        write_payload(&path, &recipe(&path, Method::Gzip));
        fs::set_permissions(&path, PermissionsExt::from_mode(want))
            .expect("could not set the archive's mode");

        let (result, _) = run_apply(&input_for(&path, Vec::new()), &no_cancel());
        result.expect("an empty Apply must succeed");

        let mode = fs::metadata(&path)
            .expect("the archive must still be there")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(
            mode, want,
            "rebuilding must carry the archive's mode exactly; asked for {want:o}, got {mode:o}"
        );
    }
}

/// `PXX-T3-055`: an archive its owner may not write is still an archive INDIUM can rebuild.
///
/// The rebuild is staged beside the archive and renamed over it, and a rename needs permission on
/// the **directory**, not on the file — so a `0o444` archive was always rebuildable, and the
/// mode-carrying work exists precisely to preserve modes like that one. `5ccdfcb` then began
/// staging the temp at the archive's exact mode, and neither writer writes through the handle that
/// created it: libarchive reopens the name in `archive_write_open_filename` and `sevenz` in
/// `ArchiveWriter::create`, both `O_WRONLY|O_CREAT|O_TRUNC`. With no owner-write bit that reopen
/// answers `EACCES`, and every Apply on such an archive died — the fix for one freeze-blocking
/// finding creating another, on the very archives it was protecting.
///
/// A removal rather than an empty Apply, deliberately: an empty Apply can pass while proving only
/// that nothing crashed, and this gate has to show the rebuild reached the disk through a file the
/// account cannot write.
///
/// Sabotage-checked by dropping `| 0o600` from the staging chmod — this test reds and no other
/// does, at every one of the three modes below.
#[test]
fn an_apply_on_an_archive_its_owner_cannot_write_is_still_rebuilt() {
    use std::os::unix::fs::PermissionsExt;

    let dir = TempDir::new("apply-readonly");
    // `0o400` owner-read only, `0o444` the classic read-only file, `0o460` owner-read plus
    // group-write — three shapes with no owner-write bit and nothing else in common.
    //
    // **All three carry owner-read, and that is not incidental.** A mode that lacks it never
    // reaches the staging block at all: `arch::list_all(&input.target, …)` opens the source first
    // and dies, with a message naming the archive rather than the temp. Tier 3 measured the split
    // — `0o060 0o006 0o200 0o002 0o040 0o004 0o000` all fail at the read, `0o400 0o444 0o460
    // 0o440 0o404 0o406` all rebuild. So the reachable half of the regression is exactly the
    // owner-readable, owner-unwritable half, and these are three of it.
    for want in [0o400u32, 0o444u32, 0o460u32] {
        let path = dir.join(&format!("ro-{want:o}.tar.gz"));
        write_payload(&path, &recipe(&path, Method::Gzip));
        fs::set_permissions(&path, PermissionsExt::from_mode(want))
            .expect("could not set the archive's mode");

        let tasks_list = vec![Task::Remove {
            path: "sub/beta.txt".to_string(),
        }];
        let (result, _) = run_apply(&input_for(&path, tasks_list), &no_cancel());
        result.unwrap_or_else(|e| {
            panic!("an Apply on a {want:o} archive must succeed, and it said: {e}")
        });

        let after = arch::list_all(&path, None).expect("the rebuilt archive must list");
        assert!(
            after.iter().all(|e| e.path != "sub/beta.txt"),
            "the removal must have reached the disk on a {want:o} archive"
        );
        let mode = fs::metadata(&path)
            .expect("the archive must still be there")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(
            mode, want,
            "the archive's own mode must survive a rebuild it could not write; \
             asked for {want:o}, got {mode:o}"
        );
    }
}

/// P4 §2: "a crashed Apply leaves exactly one leftover per archive rather than an
/// accumulating pile, and the next Apply on that archive clears it."
#[test]
fn an_orphaned_temp_from_a_crashed_apply_is_overwritten_not_multiplied() {
    let dir = TempDir::new("orphan");
    let path = dir.join("out.tar.gz");
    write_payload(&path, &recipe(&path, Method::Gzip));

    let temp = tasks::temp_path_for(&path);
    fs::write(&temp, b"a leftover from an Apply that never finished")
        .expect("could not plant the orphan");

    let (result, _) = run_apply(&input_for(&path, Vec::new()), &no_cancel());
    result.expect("Apply must proceed over a leftover");

    assert!(!temp.exists(), "the leftover must be gone, not multiplied");
    let leftovers: Vec<_> = fs::read_dir(dir.join(""))
        .expect("could not read the directory")
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_str()
                .map(tasks::is_our_temp)
                .unwrap_or(false)
        })
        .collect();
    assert!(
        leftovers.is_empty(),
        "no temp file of ours may remain: {leftovers:?}"
    );
}

/// **`PXX-C9-001`.** The orphan test above cannot fail for the reason its name gives.
///
/// It plants a *regular* file, and libarchive truncates whatever it opens — so deleting the
/// removal block entirely leaves that test green. This one plants a **dangling symlink**, which
/// is the input `Path::exists()` answers `false` for, and therefore the one input the guard was
/// written to handle and the only one that skips it. With the guard skipped the archive is
/// written *through* the link into a file nobody named, and then `rename` moves the link rather
/// than the archive — so the user's `.tar.gz` becomes a symlink to somebody else's file.
///
/// This is also the discriminating run a blind confirmer asked for: it fails if the removal is
/// removed, which is what an orphan test is supposed to be able to say.
#[test]
fn a_dangling_link_at_the_apply_temp_is_unlinked_not_written_through() {
    let dir = TempDir::new("apply-temp-link");
    let path = dir.join("out.tar.gz");
    write_payload(&path, &recipe(&path, Method::Gzip));

    // Nothing is created here. A dangling link is the whole point: `exists()` traverses, so it
    // reports `false`, and the guard that was meant to clear this file steps over it.
    let victim = dir.join("victim");
    let temp = tasks::temp_path_for(&path);
    std::os::unix::fs::symlink(&victim, &temp).expect("could not plant the link");

    let (result, _) = run_apply(&input_for(&path, Vec::new()), &no_cancel());
    result.expect("Apply must proceed over a planted link");

    assert!(
        !victim.exists(),
        "the rebuild landed at {}, so the link was followed instead of unlinked",
        victim.display()
    );
    assert!(
        fs::symlink_metadata(&path)
            .expect("the archive must still be there")
            .file_type()
            .is_file(),
        "the archive must be a regular file, not the link the commit renamed into place"
    );
}

/// **`PXX-C9-002`.** A name that is not UTF-8 is an ordinary Linux name, and skipped the guard.
///
/// The removal was gated on `to_str()`, which returns `None` for any name carrying a byte
/// sequence Rust cannot decode — and the code then did nothing at all, quietly, for a file it
/// had built the name of itself. Same body as the test above, so the two differ in exactly one
/// variable: whether the target's name happens to decode.
#[test]
fn a_target_whose_name_is_not_utf8_still_has_its_temp_cleared() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let dir = TempDir::new("apply-temp-raw");
    // 0xFF cannot begin any UTF-8 sequence, and is a perfectly ordinary byte in a filename.
    let path = dir.join("").join(OsStr::from_bytes(b"out\xFF.tar.gz"));
    write_payload(&path, &recipe(&path, Method::Gzip));

    let victim = dir.join("victim-raw");
    let temp = tasks::temp_path_for(&path);
    std::os::unix::fs::symlink(&victim, &temp).expect("could not plant the link");

    let (result, _) = run_apply(&input_for(&path, Vec::new()), &no_cancel());
    result.expect("Apply must proceed over a planted link on an undecodable name");

    assert!(
        !victim.exists(),
        "the rebuild landed at {}, so an undecodable name skipped the removal",
        victim.display()
    );
}

/// **`PXX-C9-001`, the other half.** A leftover that cannot be cleared refuses the Apply.
///
/// Proceeding past a failed removal is the write-through with an extra step, so the removal is
/// fatal — and the message has to name the file, because a refusal the user cannot act on is
/// the fault this round filed against the lock file at `PXX-C9-014`.
///
/// **This test can vacate, and the honest statement of that is: it records a reason, and the
/// reason is only visible under `cargo test -- --show-output`.** It skips when the process can
/// still write into a directory it has just closed — which means root, and `CORE.md:657` allows
/// INDIUM to be run as root. The first draft of this comment claimed the skip was loud; libtest
/// captures the output of *passing* tests, so it is not, and tier 3 measured that rather than
/// taking the word for it.
///
/// Failing instead of skipping was the alternative, and it is rejected: it would turn an
/// explicitly permitted way of running INDIUM into a red suite. So the residual risk is stated
/// where the next reader will find it — under root this gate reports `ok` without having
/// tested anything, and that is the class this round audits, admitted rather than hidden.
#[test]
fn a_leftover_that_cannot_be_removed_refuses_the_apply_and_says_which_file() {
    let dir = TempDir::new("apply-temp-stuck");
    let path = dir.join("out.tar.gz");
    write_payload(&path, &recipe(&path, Method::Gzip));

    let temp = tasks::temp_path_for(&path);
    fs::write(
        &temp,
        b"a leftover this account will not be allowed to remove",
    )
    .expect("could not plant the orphan");
    let original = fs::read(&path).expect("the original must be readable");

    // Unlink permission is the directory's, so the directory is what has to be closed.
    let parent = dir.join("");
    fs::set_permissions(&parent, PermissionsExt::from_mode(0o500))
        .expect("could not close the directory");

    let probe = fs::File::create(parent.join("probe"));
    if probe.is_ok() {
        let _ = fs::remove_file(parent.join("probe"));
        fs::set_permissions(&parent, PermissionsExt::from_mode(0o700)).ok();
        eprintln!(
            "SKIPPED a_leftover_that_cannot_be_removed_refuses_the_apply_and_says_which_file: \
             this process writes into a directory with no write bit, so it is root (or the \
             filesystem ignores modes) and the precondition cannot be built here."
        );
        return;
    }

    let (result, _) = run_apply(&input_for(&path, Vec::new()), &no_cancel());
    fs::set_permissions(&parent, PermissionsExt::from_mode(0o700)).expect("could not reopen");

    let message = result.expect_err("Apply must refuse rather than write past a stuck leftover");
    assert!(
        message.contains(&temp.display().to_string()),
        "the refusal must name the file the user has to deal with; got: {message}"
    );
    assert_eq!(
        fs::read(&path).expect("the original must still be readable"),
        original,
        "a refused Apply writes nothing"
    );
}

/// **`PXX-T3B-002`.** The same `exists()` defect, one door over, in the same function.
///
/// Found by the tier-3 review of the commit that fixed the orphan removal: a hundred lines above
/// that fix, the refusal that stops a new archive replacing an existing file asks the identical
/// question the same identical way, and traverses for the same reason. A dangling symlink at the
/// destination is not "an existing file" to `exists()`, so Create proceeds — and the commit
/// `rename` then replaces the link with a regular file, which is the silent replacement the
/// guard exists to prevent, performed by the code that refuses to perform it.
///
/// A dangling link points at nothing, so nothing of the user's is destroyed. What is destroyed
/// is the guarantee, and a guarantee that holds for every input except the one shaped to defeat
/// it is not one.
#[test]
fn creating_over_a_dangling_symlink_is_refused_like_any_other_existing_name() {
    let dir = TempDir::new("create-over-link");
    let target = dir.join("out.tar.gz");
    let source = dir.join("payload.txt");
    fs::write(&source, ALPHA).expect("could not write the source file");

    // Points at nothing. `exists()` says false; the name is nonetheless taken.
    std::os::unix::fs::symlink(dir.join("nowhere"), &target).expect("could not plant the link");

    let input = ApplyInput {
        target: target.clone(),
        recipe: recipe(&target, Method::Gzip),
        tasks: vec![
            Task::Create {
                recipe: recipe(&target, Method::Gzip),
            },
            Task::Add {
                source: source.clone(),
                dest: "payload.txt".to_string(),
            },
        ],
        adds: Vec::new(),
        staged_against: Vec::new(),
        source_password: None,
        target_password: None,
    };

    let (result, _) = run_apply(&input, &no_cancel());
    let message = result.expect_err("creating over a name already taken must be refused");
    assert!(
        message.contains("already exists"),
        "the refusal must say the name is taken; got: {message}"
    );
    assert!(
        fs::symlink_metadata(&target)
            .expect("the link must still be there")
            .file_type()
            .is_symlink(),
        "a refused Create must leave the name exactly as it found it"
    );
}

/// **`PXX-T3B-001`.** A refusal must not describe a file that is not there.
///
/// `unlink` reports path-resolution and mount failures *before* it looks for the name, so
/// `EROFS`, `ENOTDIR`, `ENAMETOOLONG` and a missing search bit all come back for a name that
/// does not exist and never did. The first draft of the fatal arm had one sentence for every
/// errno, and it told that user a leftover was in the way and to go and remove it — a refusal
/// nobody can act on, which is the fault this round filed at `PXX-C9-014` wearing a friendlier
/// face.
///
/// Tier 3 measured it on a read-only bind mount inside a user namespace. This gate reproduces
/// the same false premise with no privileges and no mounts at all: `ENAMETOOLONG`. The temp
/// name is the target's plus twelve bytes, so a target close to `NAME_MAX` yields a temp name
/// that cannot exist on any ext4 filesystem — and nothing is planted anywhere.
#[test]
fn an_unclearable_workspace_does_not_claim_a_leftover_that_is_not_there() {
    let dir = TempDir::new("longname");
    // 243 + ".tar.gz" = 250, inside NAME_MAX. The temp adds "." and ".indium-new": 262, outside.
    let path = dir.join(&format!("{}.tar.gz", "n".repeat(243)));
    write_payload(&path, &recipe(&path, Method::Gzip));

    let temp = tasks::temp_path_for(&path);
    assert!(
        temp.file_name().map(|n| n.len()).unwrap_or(0) > 255,
        "the fixture only works if the temp name is over NAME_MAX"
    );
    assert!(
        fs::symlink_metadata(&temp).is_err(),
        "nothing may be planted here — the whole point is that no leftover exists"
    );

    let (result, _) = run_apply(&input_for(&path, Vec::new()), &no_cancel());
    let message = result.expect_err("a temp name that cannot exist must refuse the Apply");
    assert!(
        !message.contains("leftover"),
        "nothing is in the way, so the refusal must not say a leftover is; got: {message}"
    );
    assert!(
        message.contains(&temp.display().to_string()),
        "the refusal must still name the path it could not clear; got: {message}"
    );
}

/// A rename must move the member without touching its bytes.
#[test]
fn a_renamed_entry_keeps_its_bytes_and_its_metadata() {
    let dir = TempDir::new("rename");
    let path = dir.join("out.tar.gz");
    write_payload(&path, &recipe(&path, Method::Gzip));

    let before = arch::list_all(&path, None).expect("could not list before");
    let beta_before = find(&before, "sub/beta.txt").clone();

    let tasks_list = vec![Task::Rename {
        from: "sub/beta.txt".to_string(),
        to: "sub/renamed.txt".to_string(),
    }];
    let (result, _) = run_apply(&input_for(&path, tasks_list), &no_cancel());
    result.expect("the rename must apply");

    let after = arch::list_all(&path, None).expect("could not list after");
    let renamed = find(&after, "sub/renamed.txt");
    assert_eq!(renamed.size, beta_before.size, "size must survive a rename");
    assert_eq!(renamed.uid, beta_before.uid, "uid must survive");
    assert_eq!(renamed.uname, beta_before.uname, "owner name must survive");
    assert_eq!(renamed.mtime, beta_before.mtime, "mtime must survive");
    assert_eq!(
        arch::crc32_of(&path, "sub/renamed.txt", None).expect("could not checksum"),
        indium::util::crc32(BETA),
        "the bytes must be the bytes"
    );
}

/// The flagship. CORE opens with "the metadata is the main event", and `meta.tar` is the
/// fixture that holds the fields no other format carries — a symlink, a hardlink, a
/// non-root uid and gid, owner names, and two distinct mtimes. An Apply that reproduces
/// all of it is the strongest statement this milestone can make.
///
/// It is also the only test where the fold's hardlink retargeting meets a real archive.
#[test]
fn metadata_survives_a_tar_rebuild() {
    let dir = TempDir::new("metatar");
    let path = dir.join("meta.tar");
    fs::copy(fixture("meta.tar"), &path).expect("could not stage meta.tar");

    let before = arch::list_all(&path, None).expect("could not list meta.tar");
    assert!(
        before.iter().any(|e| e.symlink.is_some()),
        "the fixture must hold a symlink, or this test proves less than it claims"
    );
    assert!(
        before.iter().any(|e| e.hardlink.is_some()),
        "the fixture must hold a hardlink"
    );

    let mut input = input_for(&path, Vec::new());
    input.recipe = recipe(&path, Method::Store);
    let (result, _) = run_apply(&input, &no_cancel());
    result.expect("rebuilding meta.tar must succeed");

    let after = arch::list_all(&path, None).expect("could not list the rebuild");
    assert_eq!(after.len(), before.len(), "no member may be lost");

    for old in &before {
        let new = find(&after, &old.path);
        assert_eq!(new.uid, old.uid, "{}: uid", old.path);
        assert_eq!(new.gid, old.gid, "{}: gid", old.path);
        assert_eq!(new.uname, old.uname, "{}: owner name", old.path);
        assert_eq!(new.gname, old.gname, "{}: group name", old.path);
        assert_eq!(new.mtime, old.mtime, "{}: mtime", old.path);
        assert_eq!(new.symlink, old.symlink, "{}: symlink target", old.path);
        assert_eq!(new.hardlink, old.hardlink, "{}: hardlink target", old.path);
        assert_eq!(new.mode & 0o7777, old.mode & 0o7777, "{}: mode", old.path);
    }
}

/// P4 §1: "renaming a target must retarget every link that pointed at it." Proven in
/// `tasks` against a synthetic tree; proven here against a real tar, end to end.
#[test]
fn renaming_a_hardlink_target_retargets_the_link_in_the_rebuilt_archive() {
    let dir = TempDir::new("hardlink");
    let path = dir.join("meta.tar");
    fs::copy(fixture("meta.tar"), &path).expect("could not stage meta.tar");

    let before = arch::list_all(&path, None).expect("could not list");
    let link = before
        .iter()
        .find(|e| e.hardlink.is_some())
        .expect("the fixture holds a hardlink")
        .clone();
    let target = link.hardlink.clone().expect("it names a target");

    let mut input = input_for(
        &path,
        vec![Task::Rename {
            from: target.clone(),
            to: "renamed-target.txt".to_string(),
        }],
    );
    input.recipe = recipe(&path, Method::Store);
    let (result, _) = run_apply(&input, &no_cancel());
    result.expect("renaming a hardlink target must succeed");

    let after = arch::list_all(&path, None).expect("could not list the rebuild");
    let relinked = find(&after, &link.path);
    assert_eq!(
        relinked.hardlink.as_deref(),
        Some("renamed-target.txt"),
        "the link must name the target's new name, or it extracts broken"
    );
    assert!(
        after.iter().any(|e| e.path == "renamed-target.txt"),
        "the target itself must be there under its new name"
    );
}

/// 7z stores neither symlinks nor hardlinks, so its writer skips them — and verification
/// must know that, or every rebuild of an archive holding one fails looking for a member
/// that was never writable. The loss is the one the `W` popup warns about beforehand.
#[test]
fn rebuilding_into_7z_drops_links_without_failing_verification() {
    let dir = TempDir::new("sevenzlinks");
    let source = dir.join("meta.tar");
    fs::copy(fixture("meta.tar"), &source).expect("could not stage meta.tar");

    let before = arch::list_all(&source, None).expect("could not list");
    let links = before
        .iter()
        .filter(|e| e.symlink.is_some() || e.hardlink.is_some())
        .count();
    assert!(
        links > 0,
        "the fixture must hold links for this to mean anything"
    );

    // Rebuild the tar's contents into a 7z, which is what changing format does.
    let target = dir.join("out.7z");
    fs::copy(&source, &target).expect("could not seed the target");
    let mut input = input_for(&target, Vec::new());
    input.recipe = Recipe {
        path: target.clone(),
        method: Method::Lzma2,
        level: 6,
        encrypt: false,
    };

    let (result, _) = run_apply(&input, &no_cancel());
    result.expect("a 7z rebuild must not fail merely because links cannot be carried");

    let after = arch::list_all(&target, None).expect("the 7z must read back");
    assert!(
        after
            .iter()
            .all(|e| e.symlink.is_none() && e.hardlink.is_none()),
        "7z carries no links, and must not pretend to"
    );
    assert_eq!(
        after.len(),
        before.len() - links,
        "exactly the links are missing, and nothing else"
    );
}

// ---------------------------------------------------------------------------
// A destination that cannot be written
// ---------------------------------------------------------------------------

/// The second testing round extracted an entry into `/boot` — root-owned, mode `0700` —
/// and the window answered *"Extracted 1 entry."* over an empty directory.
///
/// libarchive is the reason. `archive_read_extract` answers `ARCHIVE_WARN` (-20) both for
/// a file it wrote but could not finish stamping and for a file it could not create at
/// all, and `extract` counted the second as written. A count that includes files which do
/// not exist makes every sentence built on it a lie, so this test holds the floor: a
/// destination that refuses the write must fail, and must leave nothing behind.
#[test]
fn a_destination_that_cannot_be_written_fails_rather_than_counts() {
    let dir = TempDir::new("unwritable");
    let source = dir.join("in.tar");
    write_payload(&source, &recipe(&source, Method::Store));

    let dest = dir.join("locked");
    fs::create_dir_all(&dest).expect("could not make the destination");
    fs::set_permissions(&dest, PermissionsExt::from_mode(0o500))
        .expect("could not close the destination");

    // Root ignores the mode bits, and so do a few filesystems. Rather than ask who this
    // process is, ask the directory whether it actually refuses — which is the condition
    // the test needs, and the only one it can assert against.
    if fs::write(dest.join(".probe"), b"").is_ok() {
        let _ = fs::remove_file(dest.join(".probe"));
        let _ = fs::set_permissions(&dest, PermissionsExt::from_mode(0o755));
        eprintln!("skipped: this filesystem lets this process write a 0500 directory");
        return;
    }

    let wanted: std::collections::HashSet<String> = ["alpha.txt".to_string()].into_iter().collect();
    let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let result = arch::extract(&source, &wanted, &dest, None, None, &cancel);

    // Put the mode back before asserting, or a failure here leaves a directory the
    // harness cannot clear.
    let _ = fs::set_permissions(&dest, PermissionsExt::from_mode(0o755));

    let landed = fs::read_dir(&dest)
        .expect("the destination must still be readable")
        .count();
    assert_eq!(
        landed, 0,
        "nothing can have been written into a 0500 directory"
    );
    let e = result.expect_err("a destination that refuses every write cannot report success");
    assert!(
        e.to_string().to_lowercase().contains("create")
            || e.to_string().to_lowercase().contains("write"),
        "the sentence must name the writing as what failed, not something else: {e}"
    );
}

/// **`PXX-T3-021`, the sentence half.** Apply on an encrypted 7z INDIUM wrote must not blame the
/// password.
///
/// `sevenz.rs`'s writer ties `set_encrypt_header` to the same flag that turns AES on, so every
/// archive the Encrypted preset produces has ciphertext headers. The rebuild streams its source
/// through `arch::Reader` — libarchive — which cannot read one at all. **So Apply fails on the
/// program's own archives, for every rename, removal and addition**, and it used to fail with
/// `EncryptedHeaders`'s own sentence, *"A password is needed to list it"*, shown to someone who had
/// supplied the right password and whose listing had just succeeded through the `sevenz` fallback.
/// It sent people to re-type a password that was correct.
///
/// The capability gap is recorded unfixed — repairing it means a `sevenz`-backed rebuild path,
/// which is a feature rather than a fix. This gate holds the sentence, in both directions: it must
/// not ask for a password, and it must say what is actually wrong.
#[test]
fn apply_on_an_encrypted_7z_does_not_blame_the_password() {
    let dir = TempDir::new("hdr-apply");
    let path = dir.join("secret.7z");
    let plan = Recipe {
        path: path.clone(),
        method: Method::Lzma2,
        level: 6,
        encrypt: true,
    };
    let secret = indium::secret::Secret::from_text("indium");

    {
        let mut writer = indium::sevenz::Writer::create(&path, &plan, Some(&secret))
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

    // The premise, pinned: this password reads the archive perfectly well. Without this the gate
    // below could pass on an archive that was simply broken.
    let listed =
        arch::list_all(&path, Some(&secret)).expect("the archive must list with this password");
    assert!(
        !listed.is_empty(),
        "the fixture must list some members, or there is nothing for Apply to fail at"
    );

    let input = ApplyInput {
        target: path.clone(),
        recipe: plan,
        tasks: Vec::new(),
        adds: Vec::new(),
        staged_against: Vec::new(),
        source_password: Some(secret.clone()),
        target_password: Some(secret.clone()),
    };
    let (result, _) = run_apply(&input, &no_cancel());
    let message = result.expect_err(
        "Apply cannot rebuild an encrypted-header 7z yet — if this starts succeeding, the \
         capability landed and this gate should be replaced by one that checks the bytes",
    );

    assert!(
        !message.contains("password is needed"),
        "the refusal must not ask for a password the user already supplied and which the listing \
         above proved correct: {message}"
    );
    assert!(
        message.contains("cannot rebuild"),
        "and it must name what is actually wrong, or the user is left guessing: {message}"
    );
}

/// **`PXX-T3-049`. The archive's bytes must never be on disk behind permissions the user did not
/// choose — including at the scratch name, which is where `25be01d` left them.**
///
/// That commit set the mode on the temp *after* `build_and_verify` returned, and its comment
/// claimed parity with `arch.rs`'s extraction write. Tier 3 measured the claim false: `arch.rs`
/// opens at the target mode (`create_new(true).mode(prior_mode…)`), while this site left the
/// **complete** rebuilt archive world-readable at a predictable name for the whole rebuild — 647 ms
/// on a 64 MiB source, scaling with size. For an AES-256 7z that is the ciphertext, and a `SIGKILL`
/// mid-rebuild leaves it there until the next Apply of the same archive.
///
/// The class the commit closed at the archive's own name was still open at the scratch name beside
/// it. This gate watches that name.
///
/// **It asserts it saw the temp at least once.** A poller that misses the window entirely would
/// otherwise pass while measuring nothing, which is the exact shape of the four mutations that
/// survived this commit's own gates.
#[test]
fn the_rebuilds_scratch_file_is_never_wider_than_the_archive() {
    use std::os::unix::fs::PermissionsExt;
    use std::sync::atomic::{AtomicBool, Ordering};

    let dir = TempDir::new("temp-mode");
    let path = dir.join("out.tar.gz");

    // Big enough that the rebuild takes long enough to watch, and incompressible so the writer
    // cannot finish it instantly.
    {
        let mut writer =
            arch::Writer::create(&path, &recipe(&path, Method::Gzip)).expect("could not open");
        let mut bytes = vec![0u8; 6 << 20];
        let mut x: u32 = 0x12345678;
        for b in bytes.iter_mut() {
            x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            *b = (x >> 24) as u8;
        }
        let meta = Meta {
            out_path: "bulk.bin".to_string(),
            size: bytes.len() as u64,
            is_dir: false,
            mode: 0o644,
            mtime: Some(1_704_164_645),
            atime: None,
            ctime: None,
            uid: 0,
            gid: 0,
            uname: None,
            gname: None,
            symlink: None,
            hardlink: None,
        };
        let mut cursor = Cursor::new(bytes);
        writer.put(&meta, Some(&mut cursor)).expect("could not put");
        writer.finish().expect("could not finish");
    }

    // The bound this gate measures against. Named, because the assertion below compares every
    // observed mode to *this* rather than to a hardcoded triad.
    //
    // `0o600` and not `0o640`, and the difference is the whole gate. Fixing the predicate to
    // compare against the archive's own bits was not enough: with a `0o640` fixture, a staging
    // widened by `| 0o040` adds a bit the archive already carries, so the mutation stays
    // invisible and the corrected predicate reports nothing. Measured — the sabotage passed
    // 36/36 until this line moved. A bound is only a bound where the fixture leaves room
    // beneath it.
    //
    // **That measurement no longer reproduces, and the reason is the fix in its own commit.**
    // At `5ccdfcb` the pre-create's `.mode()` was the only thing setting the temp's mode, so
    // `| 0o040` persisted for the whole rebuild and a spin-loop observer could not miss it. The
    // staging chmod now overwrites it one statement later, leaving a window of two syscalls —
    // tier 3 re-ran that sabotage fifteen times at HEAD and got nine reds. The gate is not flaky
    // in the direction that matters: every widening that *persists* is killed deterministically
    // (`| 0o700`, `| 0o640`, and deleting the block outright), and ten clean runs gave zero reds.
    // What became transient is the pre-create argument, which is now belt to the chmod's braces.
    // Recorded rather than re-fitted: pinning a two-syscall window wants a hook, not a poller,
    // and this round has spent four findings on gates that polled and measured nothing.
    const ARCHIVE_MODE: u32 = 0o600;
    fs::set_permissions(&path, PermissionsExt::from_mode(ARCHIVE_MODE)).expect("could not tighten");

    let temp = tasks::temp_path_for(&path);
    let stop = Arc::new(AtomicBool::new(false));
    let watcher_stop = Arc::clone(&stop);
    let watch_path = temp.clone();
    let watcher = std::thread::spawn(move || {
        let mut seen: Vec<u32> = Vec::new();
        while !watcher_stop.load(Ordering::Relaxed) {
            if let Ok(md) = fs::symlink_metadata(&watch_path) {
                let m = md.permissions().mode() & 0o777;
                if seen.last() != Some(&m) {
                    seen.push(m);
                }
            }
            std::hint::spin_loop();
        }
        seen
    });

    let (result, _) = run_apply(&input_for(&path, Vec::new()), &no_cancel());
    result.expect("an empty Apply must succeed");
    stop.store(true, Ordering::Relaxed);
    let seen = watcher.join().expect("the watcher must not panic");

    assert!(
        !seen.is_empty(),
        "the watcher never saw the scratch file at all, so this gate measured nothing — \
         raise the payload size rather than trusting the pass"
    );
    // Wider **than the archive**, which is what this test is named for — not "does the world
    // triad have anything in it", which is what it used to ask. Tier 3 mutated the pre-create to
    // `| 0o040` and this gate passed at 415 while the scratch file held the complete rebuilt
    // archive at `0640`. On a `0600` archive that is `PXX-T3-049`'s own class, at the site it was
    // filed against, invisible to the gate written to close it.
    //
    // A subset rather than an equality: the umask may legitimately land the staged file narrower
    // than the archive before the exact restore, so `0o640` seen on a `0o660` archive is a pass
    // and `0o644` is not.
    //
    // Owner bits are in the comparison too, and against this fixture that is sharper than it
    // sounds. The mask is `!0o600 & 0o777` = `0o177`, so the only owner widening that exists
    // here is `+x` — and it is inside the mask, which tier 3 confirmed by mutating the staging
    // to `| 0o700` and watching this gate red alone. Nothing in the owner triad is excused by
    // construction.
    let wide: Vec<String> = seen
        .iter()
        .filter(|m| *m & !ARCHIVE_MODE & 0o777 != 0)
        .map(|m| format!("{m:o}"))
        .collect();
    // Octal, because the sentence around it is octal. `{seen:?}` on a `Vec<u32>` prints decimal,
    // so the one list a reader needs in order to diagnose a widening read `[448, 384]` beside an
    // `(offending: ["700"])` and a `set to 600` — the failure message defeating its own purpose.
    let saw: Vec<String> = seen.iter().map(|m| format!("{m:o}")).collect();
    assert!(
        wide.is_empty(),
        "the rebuild's scratch file was wider than the archive while it held the archive's \
         bytes: saw modes {saw:?} (offending: {wide:?}) for an archive the user set to \
         {ARCHIVE_MODE:o}"
    );
}
