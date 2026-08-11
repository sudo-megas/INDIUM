//! P17: the terminal half, driven through `indium::cli::run`.
//!
//! Every test here calls `run` inside the test process with `Vec<u8>` for both streams,
//! which is why `run` returns a code instead of calling `exit` and writes through
//! `dyn Write` instead of `println!`. Nothing spawns a process except the one test that
//! must, and that one says why.
//!
//! P16's lesson was that a test can be weaker than its name — one of its own asserted the
//! standard library and reached no INDIUM code at all. So each test below is chosen for
//! how it *fails*, and where the wrong implementation would slip past a more obvious
//! check, the comment says which wrong implementation.
//!
//! Copyright © sudo-megas. GPL-3.0-only.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use indium::cli;

// ---------------------------------------------------------------------------
// A hand-written temporary directory — the same twenty lines as read_path.rs and
// write_path.rs, copied rather than shared for the reason package_path.rs gives.
// ---------------------------------------------------------------------------

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> TempDir {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("indium-cli-{}-{}-{}", tag, std::process::id(), n));
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

/// What a run produced: the code, and both streams.
struct Run {
    code: i32,
    out: Vec<u8>,
    err: String,
}

impl Run {
    fn stdout(&self) -> String {
        String::from_utf8_lossy(&self.out).into_owned()
    }
    fn lines(&self) -> Vec<String> {
        self.stdout().lines().map(|s| s.to_string()).collect()
    }
}

fn run(args: &[&str]) -> Run {
    let owned: Vec<OsString> = args.iter().map(OsString::from).collect();
    let mut out: Vec<u8> = Vec::new();
    let mut err: Vec<u8> = Vec::new();
    let code = cli::run(&owned, &mut out, &mut err);
    Run {
        code,
        out,
        err: String::from_utf8_lossy(&err).into_owned(),
    }
}

const BASIC: [&str; 4] = ["basic.zip", "basic.7z", "basic.tar.gz", "basic.tar.zst"];
const FOUR: [&str; 4] = ["alpha.txt", "beta.txt", "sub", "sub/gamma.txt"];

// ---------------------------------------------------------------------------
// The surface
// ---------------------------------------------------------------------------

/// CORE §4 puts this rule on the Keys popup — "generated from the bindings, never typed
/// twice" — because a help text that has drifted from the program is worse than none.
/// The same rule, applied to `USAGE`: a fourth subcommand added without a line here fails.
///
/// It also asserts the promise is *gone*. `main.rs` printed "Headless subcommands
/// (extract, list, single-file open) arrive in V1.3." from P1 until this round, and a
/// round that pays a debt while leaving its IOU in the window has not paid it.
#[test]
fn the_usage_text_names_every_subcommand_it_accepts() {
    for word in cli::SUBCOMMANDS {
        assert!(
            cli::USAGE.contains(word),
            "USAGE never mentions the {word} subcommand"
        );
    }
    assert!(
        !cli::USAGE.contains("V1.3") && !cli::USAGE.contains("arrive"),
        "USAGE still promises headless subcommands as a future thing"
    );
}

/// `indium listing.zip` and `indium ./list` are archives; only `indium list` is the
/// subcommand. A `starts_with` or `contains` implementation passes the first two cases
/// below and fails the last two, which is exactly why they are here.
#[test]
fn a_word_that_is_not_a_subcommand_is_an_archive_name() {
    let yes = |a: &str| cli::takes_the_terminal(&[OsString::from(a)]);
    assert!(yes("list"));
    assert!(yes("extract"));
    assert!(yes("cat"));
    assert!(!yes("frobnicate"));
    assert!(!yes("./list"), "a file called list is a file");
    assert!(!yes("listing.zip"), "a prefix match would take this");
    assert!(!yes("catalogue.tar"), "so would this one");
    assert!(!cli::takes_the_terminal(&[]), "no arguments is the window");
}

/// Exit 2 and not 1: a command line that cannot be obeyed is a different failure from an
/// operation that failed. And the usage goes to stderr, so a script reading `list`'s
/// stdout never receives help text where it expected paths.
#[test]
fn a_subcommand_with_no_archive_is_a_usage_error() {
    for word in cli::SUBCOMMANDS {
        let r = run(&[word]);
        assert_eq!(r.code, 2, "{word} with no archive should be a usage error");
        assert!(r.out.is_empty(), "{word}: usage must not reach stdout");
        assert!(r.err.contains("indium"), "{word}: nothing said on stderr");
    }
}

/// The flag must be rejected *before* the archive is opened, which the empty stdout is
/// what proves — an implementation that listed first and complained afterwards would
/// still exit 2, and would have printed four paths on the way.
#[test]
fn an_unknown_flag_is_a_usage_error_and_reads_nothing() {
    let path = fixture("basic.zip");
    let r = run(&["list", path.to_str().unwrap(), "--verbose"]);
    assert_eq!(r.code, 2);
    assert!(
        r.out.is_empty(),
        "the archive was read before the flag was judged: {:?}",
        r.stdout()
    );
}

// ---------------------------------------------------------------------------
// list
// ---------------------------------------------------------------------------

/// Every basic fixture, so zip, 7z and two tar filters are all exercised rather than
/// whichever one happened to be tried.
///
/// The paths are the *stored* paths. An implementation that reused `model::rows_for` —
/// the window's view — would print `gamma.txt` for `sub/gamma.txt` and fail here, and so
/// would one that decorated a directory with a trailing slash.
#[test]
fn listing_every_basic_fixture_prints_the_same_four_paths() {
    for name in BASIC {
        let path = fixture(name);
        let r = run(&["list", path.to_str().unwrap()]);
        assert_eq!(r.code, 0, "{name}: {}", r.err);

        let mut got = r.lines();
        got.sort();
        let mut want: Vec<String> = FOUR.iter().map(|s| s.to_string()).collect();
        want.sort();
        assert_eq!(got, want, "{name} listing");
    }
}

/// **The property that makes `list` useful rather than merely printable.** Its output is
/// fed straight back in as `extract`'s members, and all four must land. Any decoration,
/// quoting or normalisation difference between the two subcommands fails here and
/// nowhere else in this file.
#[test]
fn a_listed_path_is_a_path_extract_will_take() {
    let dir = TempDir::new("roundtrip");
    let archive = fixture("basic.zip");
    let listed = run(&["list", archive.to_str().unwrap()]);
    assert_eq!(listed.code, 0);

    let mut args: Vec<String> = vec![
        "extract".into(),
        archive.to_str().unwrap().into(),
        "--to".into(),
        dir.path().to_str().unwrap().into(),
        "--".into(),
    ];
    args.extend(listed.lines());
    let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

    let r = run(&refs);
    assert_eq!(r.code, 0, "extract refused list's own output: {}", r.err);
    for name in ["alpha.txt", "beta.txt", "sub/gamma.txt"] {
        assert!(
            dir.path().join(name).is_file(),
            "{name} did not survive the round trip"
        );
    }
}

/// The long listing is computed here from the same functions the window uses, never from
/// a literal. A CLI that grew its own mode or timestamp formatter would pass a
/// literal-based test — the literals would simply have been copied from its output — and
/// fails this one.
#[test]
fn the_long_listing_speaks_the_window_s_own_words() {
    let path = fixture("meta.tar");
    let r = run(&["list", path.to_str().unwrap(), "--long"]);
    assert_eq!(r.code, 0, "{}", r.err);
    let text = r.stdout();

    for e in indium::arch::list_all(&path, None).expect("listed") {
        assert!(
            text.contains(&indium::util::format_mode(e.mode, e.filetype)),
            "the mode of {} is not the window's: {text}",
            e.path
        );
        if let Some(t) = e.mtime {
            assert!(
                text.contains(&indium::util::format_timestamp(t)),
                "the time of {} is not the window's",
                e.path
            );
        }
        assert!(text.contains(&e.path), "{} is missing entirely", e.path);
    }

    // The footer is the status bar's second row. `format_bytes` is asked for the number
    // rather than the number being written down.
    let agg = indium::model::aggregate(&indium::arch::list_all(&path, None).unwrap());
    assert!(
        text.contains(&indium::util::format_bytes(agg.total_real)),
        "the total is not the window's total"
    );
}

/// A name outside ASCII, through a route that had never been exercised before this round.
/// The locale guard lives in `Reader::open`, so `list` inherits it — but only because it
/// goes through the reader, and this is the test that says so. On a machine with no UTF-8
/// locale configured it is also the test that says *that*, exactly as the three utf8.zip
/// tests did in the debian:bookworm container.
#[test]
fn a_name_outside_ascii_survives_the_terminal_path() {
    let path = fixture("utf8.zip");
    let r = run(&["list", path.to_str().unwrap()]);
    assert_eq!(r.code, 0, "{}", r.err);

    let listed = r.lines();
    let expected = indium::arch::list_all(&path, None).expect("listed");
    for e in &expected {
        assert!(
            listed.contains(&e.path),
            "the terminal lost {:?} — the listing has {listed:?}",
            e.path
        );
        assert!(!e.path.is_empty(), "a nameless entry is P11's defect again");
    }
    assert!(
        listed.iter().any(|l| !l.is_ascii()),
        "utf8.zip should carry at least one name outside ASCII: {listed:?}"
    );
}

// ---------------------------------------------------------------------------
// extract
// ---------------------------------------------------------------------------

/// **The empty-selection trap, which is the reason this test exists at all.**
/// `arch::selection_matches` returns false for an empty `wanted` — so a `cat`-shaped
/// implementation that simply forwarded "no members named" to `arch::extract` would write
/// nothing, return `Ok(0)`, and report a clean success. The assertion is on the files, not
/// on the exit code, because the exit code is the part that lies.
#[test]
fn an_extraction_with_no_member_takes_everything() {
    let dir = TempDir::new("all");
    let archive = fixture("basic.zip");
    let r = run(&[
        "extract",
        archive.to_str().unwrap(),
        "--to",
        dir.path().to_str().unwrap(),
    ]);
    assert_eq!(r.code, 0, "{}", r.err);
    for name in ["alpha.txt", "beta.txt", "sub/gamma.txt"] {
        assert!(
            dir.path().join(name).is_file(),
            "{name} was not extracted — an empty selection matches nothing"
        );
    }
}

#[test]
fn an_extraction_takes_only_what_was_named() {
    let dir = TempDir::new("some");
    let archive = fixture("basic.zip");
    let r = run(&[
        "extract",
        archive.to_str().unwrap(),
        "--to",
        dir.path().to_str().unwrap(),
        "sub/gamma.txt",
    ]);
    assert_eq!(r.code, 0, "{}", r.err);
    assert!(dir.path().join("sub/gamma.txt").is_file());
    assert!(
        !dir.path().join("alpha.txt").exists(),
        "a superset was extracted"
    );
    assert!(
        !dir.path().join("beta.txt").exists(),
        "a superset was extracted"
    );
}

/// `arch::extract`'s pre-flight refuses a traversal before a byte is written, and this
/// proves the terminal half reaches that pre-flight unmodified. A CLI that had built its
/// own extraction loop would pass every unit test in `arch` and fail here.
#[test]
fn a_traversal_entry_is_refused_and_the_destination_stays_empty() {
    let dir = TempDir::new("evil");
    let r = run(&[
        "extract",
        fixture("evil.zip").to_str().unwrap(),
        "--to",
        dir.path().to_str().unwrap(),
    ]);
    assert_eq!(r.code, 1, "a traversal must fail: {}", r.err);
    let left: Vec<_> = std::fs::read_dir(dir.path())
        .expect("destination")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name())
        .collect();
    assert!(left.is_empty(), "the destination is not empty: {left:?}");
}

// ---------------------------------------------------------------------------
// cat
// ---------------------------------------------------------------------------

/// The bytes, and only the bytes. A progress line, a trailing newline or a "wrote N
/// bytes" note would all fail — `cat`'s stdout is a data channel.
#[test]
fn cat_writes_the_bytes_and_says_nothing_else() {
    for name in BASIC {
        let path = fixture(name);
        let r = run(&["cat", path.to_str().unwrap(), "alpha.txt"]);
        assert_eq!(r.code, 0, "{name}: {}", r.err);
        assert_eq!(
            r.out, b"INDIUM fixture alpha\n",
            "{name}: cat wrote something other than the member"
        );
        assert!(r.err.is_empty(), "{name}: cat said {:?} on stderr", r.err);
    }
}

#[test]
fn cat_refuses_a_directory_and_a_name_that_is_not_there() {
    let path = fixture("basic.zip");
    let dir = run(&["cat", path.to_str().unwrap(), "sub"]);
    assert_eq!(dir.code, 1);
    assert!(dir.out.is_empty(), "a directory produced bytes");

    let missing = run(&["cat", path.to_str().unwrap(), "nowhere.txt"]);
    assert_eq!(missing.code, 1);
    assert!(missing.out.is_empty());
    assert!(missing.err.contains("no such entry"), "{}", missing.err);
}

// ---------------------------------------------------------------------------
// The refusals CORE fixes
// ---------------------------------------------------------------------------

/// CORE §5: "Opening one produces a plain sentence." All three subcommands, because the
/// gate lives in `Reader::open` and each reaches it by a different route — `list` through
/// `list_all`, `extract` through its pre-flight's listing, `cat` through `stream_entry`.
/// A future `stream_entry` variant that opened the file some other way would pass the
/// other two and fail here.
#[test]
fn rar_is_refused_by_every_subcommand_with_the_exact_sentence() {
    let path = fixture("notrar.rar");
    let p = path.to_str().unwrap();
    let dir = TempDir::new("rar");

    let cases = [
        run(&["list", p]),
        run(&["extract", p, "--to", dir.path().to_str().unwrap()]),
        run(&["cat", p, "anything"]),
    ];
    for (i, r) in cases.iter().enumerate() {
        assert_eq!(r.code, 1, "case {i} did not fail");
        assert_eq!(
            r.err.trim_end(),
            format!("indium: {}", indium::arch::RAR_REFUSAL),
            "case {i} said something other than CORE §5's sentence"
        );
        assert!(r.out.is_empty(), "case {i} wrote to stdout");
    }
}

/// **This one must spawn a process, and the reason is the whole point of the test.**
///
/// `cargo test` run from an interactive shell *inherits the controlling terminal*, so
/// `/dev/tty` opens and a prompt would block — passing in CI while hanging the maker's
/// own `cargo test`, which the release ritual requires by hand. So the no-terminal
/// condition is *created* rather than assumed: `setsid` detaches the child, and a
/// timeout makes "it blocked" a failure rather than a suite that never finishes.
#[test]
fn an_encrypted_archive_with_no_terminal_is_refused_and_does_not_hang() {
    use std::process::{Command, Stdio};

    let mut child = match Command::new("setsid")
        .arg(env!("CARGO_BIN_EXE_indium"))
        .args(["cat", fixture("secret.zip").to_str().unwrap(), "secret.txt"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        // `setsid` is util-linux and present on every machine this ships to, but a test
        // that silently passes when its tool is missing is the thing package_path.rs
        // refuses to do. Say so loudly instead.
        Err(e) => panic!("setsid could not be run, so this test proved nothing: {e}"),
    };

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    loop {
        match child.try_wait().expect("could not wait on the child") {
            Some(status) => {
                let out = child.wait_with_output().expect("output");
                assert_eq!(
                    status.code(),
                    Some(1),
                    "expected a refusal, got {status:?}: {}",
                    String::from_utf8_lossy(&out.stderr)
                );
                assert!(
                    out.stdout.is_empty(),
                    "bytes were written for an archive that was never unlocked"
                );
                let said = String::from_utf8_lossy(&out.stderr);
                assert!(
                    said.contains("terminal"),
                    "the refusal should name the missing terminal: {said}"
                );
                return;
            }
            None if std::time::Instant::now() > deadline => {
                let _ = child.kill();
                panic!(
                    "the terminal half blocked on a password with no terminal to type it \
                     on. That is the failure this test exists for: the prompt must be on \
                     /dev/tty and must refuse when there is none."
                );
            }
            None => std::thread::sleep(std::time::Duration::from_millis(50)),
        }
    }
}

// ---------------------------------------------------------------------------
// Hostile names
// ---------------------------------------------------------------------------

/// **The whole justification for `-0`.** A member name may contain a newline, and a
/// line-oriented listing of one produces more lines than there are entries — so the
/// default output is *not* correct for this archive and is not asserted to be. `-0` is,
/// and it is the only form that is.
///
/// The archive is built here because no committed fixture holds such a name, and one
/// that did would be a hazard to every other test in the tree.
#[test]
fn a_member_name_with_a_newline_survives_minus_zero() {
    use std::io::Cursor;

    use indium::tasks::{Meta, Method, Recipe, Sink};

    let dir = TempDir::new("newline");
    let path = dir.path().join("hostile.tar");
    let names = ["plain.txt", "two\nlines.txt", "three\nmore\nlines.txt"];

    {
        let recipe = Recipe {
            path: path.clone(),
            method: Method::Store,
            level: Method::Store.default_level(),
            encrypt: false,
        };
        let mut writer = indium::arch::Writer::create(&path, &recipe).expect("writer");
        for name in names {
            let body = b"x\n";
            let meta = Meta {
                out_path: name.to_string(),
                size: body.len() as u64,
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
            let mut cursor = Cursor::new(body.to_vec());
            writer.put(&meta, Some(&mut cursor)).expect("put");
        }
        writer.finish().expect("finish");
    }

    let p = path.to_str().unwrap();

    // The default form: more lines than entries. Recorded, not blessed — this is what
    // makes the flag necessary rather than decorative.
    let plain = run(&["list", p]);
    assert_eq!(plain.code, 0, "{}", plain.err);
    assert!(
        plain.lines().len() > names.len(),
        "a newline in a name should break a line-oriented listing; it did not, so this \
         test is no longer testing what it says"
    );

    // The NUL form: exactly one field per entry, each the whole stored name.
    let nul = run(&["list", p, "-0"]);
    assert_eq!(nul.code, 0, "{}", nul.err);
    let mut fields: Vec<&[u8]> = nul.out.split(|b| *b == 0).collect();
    assert_eq!(
        fields.pop(),
        Some(&b""[..]),
        "a NUL-separated listing ends with a separator"
    );
    assert_eq!(
        fields.len(),
        names.len(),
        "expected one NUL field per entry, got {fields:?}"
    );
    for name in names {
        assert!(
            fields.contains(&name.as_bytes()),
            "{name:?} did not survive -0 whole"
        );
    }
}

/// `--long` is for a person and `-0` is for a script. Asked for together one must lose,
/// and a flag accepted and then silently discarded is worse than one refused — the caller
/// believes they got what they asked for. An implementation where `--long` simply wins
/// exits 0 and prints a perfectly good long listing, so the exit code is what catches it.
#[test]
fn long_and_nul_together_are_refused_rather_than_one_being_ignored() {
    let path = fixture("basic.zip");
    let r = run(&["list", path.to_str().unwrap(), "--long", "-0"]);
    assert_eq!(
        r.code,
        2,
        "one of the two flags was silently ignored: {:?}",
        r.stdout()
    );
    assert!(r.out.is_empty(), "the archive was read before the refusal");
}

// ---------------------------------------------------------------------------
// What P17's own sweep found. Each of these passed nothing before the fix.
// ---------------------------------------------------------------------------

/// **`cat … | head` is correct behaviour, and used to exit 1 saying "Broken pipe".**
/// The guard in `run`'s flush could never fire for `cat`: the error surfaces inside
/// `io::copy`, deep in `arch`, and comes back as an `ArchiveError` — so the flush had
/// nothing left to fail on. Driven through the real binary because a `Vec<u8>` sink cannot
/// close a pipe.
#[test]
fn cat_into_a_closed_pipe_is_success_and_says_nothing() {
    use std::process::{Command, Stdio};

    let sh = format!(
        "{} cat {} alpha.txt | head -c 1 >/dev/null",
        env!("CARGO_BIN_EXE_indium"),
        fixture("basic.zip").display()
    );
    let out = Command::new("sh")
        .args(["-o", "pipefail", "-c", &sh])
        .stderr(Stdio::piped())
        .output()
        .expect("could not run the pipeline");

    assert!(
        out.status.success(),
        "a reader that walked away was reported as a failure: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.stderr.is_empty(),
        "nothing should be said about a closed pipe: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// A genuine write failure must still fail — the other half of the same rule, so the fix
/// above cannot have been "treat every write error as fine".
#[test]
fn cat_onto_a_full_disk_still_fails() {
    use std::process::{Command, Stdio};

    let sh = format!(
        "{} cat {} alpha.txt >/dev/full",
        env!("CARGO_BIN_EXE_indium"),
        fixture("basic.zip").display()
    );
    let out = Command::new("sh")
        .args(["-c", &sh])
        .stderr(Stdio::piped())
        .output()
        .expect("could not run");
    assert_eq!(out.status.code(), Some(1), "ENOSPC must be a failure");
    assert!(!out.stderr.is_empty(), "and it must say so");
}

/// **A named member that matches nothing used to exit 0 saying "Extracted 0 entries."**
/// The round guarded the *empty* selection and left the *unmatched* one, which is the same
/// lie in different clothes — and the sharpest form is a typo'd second archive, which
/// `list` already refuses outright.
#[test]
fn a_member_that_matches_nothing_is_a_failure_not_an_empty_success() {
    let dir = TempDir::new("unmatched");
    let archive = fixture("basic.zip");
    let cases = [
        "nosuchfile.txt",
        "ALPHA.TXT",               // wrong case
        "tests/fixtures/utf8.zip", // the typo'd second archive
        "",                        // normalises to nothing, matches nothing
        "/",                       // so does this
    ];
    for member in cases {
        let r = run(&[
            "extract",
            archive.to_str().unwrap(),
            "--to",
            dir.path().to_str().unwrap(),
            "--",
            member,
        ]);
        assert_ne!(
            r.code, 0,
            "extract reported success for a member that matches nothing: {member:?}"
        );
    }
}

/// **An empty archive path used to read stdin and report a clean empty listing.**
/// libarchive maps `""` to standard input; the window half has checked `exists()` since P8
/// and the terminal half had no equivalent, so a missing archive came back successful.
#[test]
fn an_archive_that_is_not_there_is_a_failure_on_every_subcommand() {
    for args in [
        vec!["list", ""],
        vec!["cat", "", "alpha.txt"],
        vec!["extract", ""],
        vec!["list", "/nonexistent/nowhere.zip"],
    ] {
        let r = run(&args);
        assert_ne!(r.code, 0, "{args:?} should have failed");
        assert!(r.out.is_empty(), "{args:?} wrote to stdout");
    }
}

/// **`list` promises its output feeds back into `cat`, and a member named `-0` broke it.**
/// `extract` had `--` from the start; `cat` did not, so it refused the very name `list`
/// had just printed. The round-trip test covered `extract` only, which is how it got past.
#[test]
fn a_member_named_like_a_flag_is_reachable_through_cat() {
    use std::io::Cursor;

    use indium::tasks::{Meta, Method, Recipe, Sink};

    let dir = TempDir::new("flagnames");
    let path = dir.path().join("names.tar");
    let names = ["-0", "--long", "--to", "--", "-"];
    {
        let recipe = Recipe {
            path: path.clone(),
            method: Method::Store,
            level: Method::Store.default_level(),
            encrypt: false,
        };
        let mut writer = indium::arch::Writer::create(&path, &recipe).expect("writer");
        for name in names {
            let meta = Meta {
                out_path: name.to_string(),
                size: 2,
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
            let mut cursor = Cursor::new(b"x\n".to_vec());
            writer.put(&meta, Some(&mut cursor)).expect("put");
        }
        writer.finish().expect("finish");
    }

    let p = path.to_str().unwrap();
    for name in names {
        let r = run(&["cat", p, "--", name]);
        assert_eq!(r.code, 0, "cat could not reach {name:?}: {}", r.err);
        assert_eq!(r.out, b"x\n", "wrong bytes for {name:?}");
    }
}

/// **`--to=` lost bytes.** The arm was reached through `to_str()`, so a directory that is
/// not valid UTF-8 skipped it and the whole token fell through to the positional arm —
/// making the *flag* the archive path. In the round that adopted `args_os` to stop losing
/// bytes, this was the one place they were still lost.
#[test]
fn a_destination_outside_utf8_is_still_a_destination() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let dir = TempDir::new("nonutf8");
    let mut raw = dir.path().as_os_str().to_os_string().into_vec();
    raw.extend_from_slice(b"/out\xe9");
    let dest = OsString::from_vec(raw);

    let mut flag = OsString::from("--to=");
    flag.push(&dest);

    let args = vec![
        OsString::from("extract"),
        fixture("basic.zip").into_os_string(),
        flag,
    ];
    let mut out: Vec<u8> = Vec::new();
    let mut err: Vec<u8> = Vec::new();
    let code = cli::run(&args, &mut out, &mut err);
    assert_eq!(code, 0, "{}", String::from_utf8_lossy(&err));
    assert!(
        Path::new(&dest).join("alpha.txt").is_file(),
        "nothing landed in the non-UTF-8 destination"
    );
}

/// **`--to=` with nothing after it emptied the archive into the working directory.**
/// `create_dir_all("")` is `Ok(())` by std's own empty-path case, so the guard never fired.
/// The bare `--to` at the end of the line was already refused; these must agree.
#[test]
fn an_empty_destination_is_a_usage_error_like_a_missing_one() {
    let archive = fixture("basic.zip");
    let empty = run(&["extract", archive.to_str().unwrap(), "--to="]);
    let missing = run(&["extract", archive.to_str().unwrap(), "--to"]);
    assert_eq!(empty.code, 2, "--to= should be a usage error");
    assert_eq!(missing.code, 2, "--to at the end should be a usage error");
}
