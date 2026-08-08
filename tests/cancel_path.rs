//! P7 §7 — "A cancelled copy-out offered half a selection" — pinned against real
//! fixtures. `arch::extract` against `basic.zip`.
//!
//! The defect, in `build/docs/P7.md` §7's own words: `arch::extract` "returns
//! `Ok(written)` when the cancellation flag is set — a partial count, from a function
//! whose success and whose interruption are the same variant." `tasks.rs`'s
//! `ApplyMsg::Cancelled` — pinned by `tests/write_path.rs`'s
//! `a_cancelled_apply_leaves_the_original_untouched_and_says_so` — is the precedent
//! this file follows in shape and in name, per §8's own instruction to mirror it.
//!
//! One honesty note before the tests: `ExtractMsg::Cancelled` itself is never
//! constructed inside `arch::extract`. Grep confirms the only place that builds it is
//! `src/ui/mod.rs`'s `spawn_extract`, which calls `extract`, gets back `Ok(written)`,
//! and *then* decides — reading its own cloned `Arc`, never `self.cancel` — whether to
//! send `Done` or `Cancelled`. That decision is UI-thread code with no window to run it
//! against in a test. So instead of asserting a message `arch::extract` cannot send,
//! these tests pin the fact the variant exists to paper over: that `Ok(usize)` alone is
//! ambiguous, and that whatever a cancelled run leaves on disk is honest — complete
//! files only, never a truncated one, and a count that matches what is actually there.
//!
//! Copyright © sudo-megas. GPL-3.0-only.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::channel;
use std::sync::Arc;

use indium::arch::{self, ExtractMsg};

// ---------------------------------------------------------------------------
// A hand-written temporary directory.
//
// CORE §2's rule applies to test dependencies too: "makes a directory in /tmp for the
// tests" is not a sentence worth a crate. Same shape as `tests/read_path.rs` and
// `tests/write_path.rs`.
// ---------------------------------------------------------------------------

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> TempDir {
        use std::sync::atomic::{AtomicU32, Ordering as CounterOrdering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, CounterOrdering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "indium-cancel-{}-{}-{}",
            tag,
            std::process::id(),
            n
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("could not create temp dir");
        TempDir(path)
    }

    fn path(&self) -> &Path {
        &self.0
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

fn cancelled_already() -> Arc<AtomicBool> {
    Arc::new(AtomicBool::new(true))
}

// The shared payload every `basic.*` fixture carries — sizes and bytes recorded in
// `tests/fixtures/README.md`.
const ALPHA: &[u8] = b"INDIUM fixture alpha\n";
const BETA: &[u8] = b"INDIUM fixture beta\n";
const GAMMA: &[u8] = b"INDIUM fixture gamma\n";

/// `basic.zip`'s four selected entries, exactly as `read_path.rs`'s
/// `extraction_reproduces_bytes_exactly` selects them: `alpha.txt`, `beta.txt`,
/// `sub/` and `sub/gamma.txt` (naming `sub` recurses into it, per
/// `tests/fixtures/README.md`).
fn basic_wanted() -> HashSet<String> {
    wanted(&["alpha.txt", "beta.txt", "sub"])
}

/// Every path `basic_wanted()` selects, paired with the bytes it must hold if it is a
/// file. `sub` is a directory and has none of its own to check.
const SELECTED_PATHS: [(&str, Option<&[u8]>); 4] = [
    ("alpha.txt", Some(ALPHA)),
    ("beta.txt", Some(BETA)),
    ("sub", None),
    ("sub/gamma.txt", Some(GAMMA)),
];

/// How many of `basic_wanted()`'s entries exist under `dir` — and, along the way, proof
/// that every file among them is byte-identical to the fixture. A cancellation must
/// leave each entry complete or absent; this is what would fail if it ever left one
/// half-written instead.
fn count_intact_entries(dir: &Path) -> usize {
    let mut present = 0usize;
    for (rel, want_bytes) in SELECTED_PATHS {
        let target = dir.join(rel);
        if !target.exists() {
            continue;
        }
        present += 1;
        match want_bytes {
            Some(expected) => {
                let got = fs::read(&target)
                    .unwrap_or_else(|e| panic!("{rel} exists but could not be read: {e}"));
                assert_eq!(
                    got, expected,
                    "{rel} is on disk but is not byte-identical to the fixture — a \
                     cancellation must leave every file it touches complete or absent, \
                     never a truncated one wearing a whole file's name"
                );
            }
            None => assert!(target.is_dir(), "{rel} exists but is not a directory"),
        }
    }
    present
}

// ---------------------------------------------------------------------------
// The control: an uncancelled extraction is the baseline every claim below rests on.
// ---------------------------------------------------------------------------

/// If this fails, nothing below means anything — a cancelled run can only be judged
/// short against a run that is known to be complete.
#[test]
fn an_uncancelled_extraction_writes_every_selected_entry() {
    let dir = TempDir::new("full");
    let n = arch::extract(
        &fixture("basic.zip"),
        &basic_wanted(),
        dir.path(),
        None,
        None,
        &no_cancel(),
    )
    .expect("an uncancelled extraction must not fail");

    assert_eq!(n, 4, "alpha, beta, sub/ and sub/gamma.txt");
    assert_eq!(count_intact_entries(dir.path()), 4);
}

// ---------------------------------------------------------------------------
// The defect, stated as a fact — `build/docs/P7.md` §8's own description of this test:
// "With the flag set before the call it asserts that `arch::extract` returns `Ok` with
// a count below the selection, that the destination holds exactly that many entries and
// no more, and that nothing in the return value distinguishes the run from a completed
// one." §8 names it `a_cancelled_extraction_returns_ok_like_a_finished_one`; this test
// keeps that name.
//
// One divergence from §8, and it is deliberate rather than missed: §8 places this test
// in `tests/read_path.rs`. It lives here instead, in its own file, because this session
// owns `tests/cancel_path.rs` only — five other agents are editing `src/` and
// `tests/read_path.rs`'s neighbours concurrently, and a new file was the boundary drawn
// to keep this work from colliding with theirs.
// ---------------------------------------------------------------------------

/// `arch::extract` never sends `Done` or `Cancelled` (see the module doc comment), so
/// this cannot assert on a message it does not send. What it pins instead is the reason
/// the milestone's fix has to live in the worker: `Ok(usize)` on its own cannot tell a
/// cancelled run from a legitimately smaller one, because both are the same value.
#[test]
fn a_cancelled_extraction_returns_ok_like_a_finished_one() {
    let dir = TempDir::new("cancelled");
    let (tx, rx) = channel();

    // The flag is checked at the top of the loop in both the 7z branch and the
    // libarchive branch of `extract`, before either touches its current entry — so a
    // cancel set before the call stops it before a single byte reaches disk.
    let n = arch::extract(
        &fixture("basic.zip"),
        &basic_wanted(),
        dir.path(),
        None,
        Some(&tx),
        &cancelled_already(),
    )
    // `write_path.rs`'s `a_cancelled_apply_leaves_the_original_untouched_and_says_so`
    // establishes the same for Apply, in the same word: cancellation is not an error,
    // so `extract` must return `Ok`, never `Err`, when the flag stops it early.
    .expect("a cancelled extraction is not an error");
    drop(tx);

    // "returns `Ok` with a count below the selection"
    assert!(
        n < basic_wanted().len(),
        "four were selected; a preset cancel must write fewer, got {n}"
    );

    // Progress is only ever sent after a member is written, so a count this short
    // should have sent no more messages than that — here, none at all.
    let messages: Vec<ExtractMsg> = rx.into_iter().collect();
    assert_eq!(
        messages.len(),
        n,
        "extract sends one Progress per entry written: {messages:?}"
    );

    // "the destination holds exactly that many entries and no more"
    assert_eq!(
        count_intact_entries(dir.path()),
        n,
        "what is on disk must match what extract reports, exactly"
    );

    // "nothing in the return value distinguishes the run from a completed one" — made
    // concrete: a second call that asks for nothing at all, cancels nothing, and simply
    // finishes reports the identical `Ok` value. A caller holding only the `Result`
    // cannot tell "cut short after none" from "there was never anything to do" apart —
    // which is exactly why `ExtractMsg::Cancelled` has to say so in band instead.
    let legitimate_dir = TempDir::new("legitimately-empty");
    let legitimate = arch::extract(
        &fixture("basic.zip"),
        &wanted(&[]),
        legitimate_dir.path(),
        None,
        None,
        &no_cancel(),
    )
    .expect("asking for nothing and finishing is not an error either");

    assert_eq!(
        n, legitimate,
        "a cancelled run of four and a legitimate run of zero report the same Ok({n}) — \
         extract's own return type cannot carry the difference"
    );
}

// ---------------------------------------------------------------------------
// Beyond §8's minimum: what a cancellation that lands mid-run, rather than before it
// starts, leaves behind.
//
// A preset flag (above) always stops before entry one, since the check runs before any
// write. To exercise a cancel that actually interrupts a run in progress, `extract` has
// to be running while the flag flips — which needs a second thread, since `extract`
// itself blocks its caller until it returns (it does not spawn its own worker; that is
// `src/ui/mod.rs`'s `spawn_extract`'s job). `std::thread::scope` gives a joined,
// borrowing thread with no `'static` requirement, so the fixture path, the `HashSet`
// and the `Arc` below need no cloning beyond the `Arc` cancellation already requires.
//
// The assertion does not guess how many entries land before the flag is seen — that is
// a race, and asserting an exact count would make this test flaky on a loaded machine.
// What must hold regardless of where the race lands: every file actually on disk is
// byte-identical to the fixture, and the count `extract` returns is exactly the count
// of selected paths that exist — the same number `spawn_extract` would hand
// `ExtractMsg::Cancelled { written }` verbatim.
// ---------------------------------------------------------------------------

/// No half-file, and the count is honest, when the cancel genuinely races the write
/// loop instead of being decided before it ever starts.
#[test]
fn a_cancellation_landing_mid_run_never_leaves_a_truncated_file_and_its_count_matches_disk() {
    let dir = TempDir::new("mid-run");
    let cancel = Arc::new(AtomicBool::new(false));
    let (tx, rx) = channel();

    let written = std::thread::scope(|scope| {
        let handle = scope.spawn(|| {
            arch::extract(
                &fixture("basic.zip"),
                &basic_wanted(),
                dir.path(),
                None,
                Some(&tx),
                &cancel,
            )
        });

        // Block for the first Progress message: `extract` writes an entry's bytes,
        // increments `written`, and only then sends, so waiting for it guarantees at
        // least one complete file exists before the flag is ever set. Set it the
        // instant it arrives, so the window for further entries to land is as small as
        // this thread can make it — the remaining race is real, not manufactured, and
        // is what the assertions below are built to tolerate.
        rx.recv()
            .expect("extract must send at least one Progress before finishing four entries");
        cancel.store(true, Ordering::Relaxed);

        handle.join().expect("the extraction thread must not panic")
    })
    .expect("a cancelled extraction is not an error");
    drop(tx);
    let _drain: Vec<ExtractMsg> = rx.into_iter().collect();

    assert!(
        (1..=4).contains(&written),
        "the first Progress message proves at least one entry was written, and only \
         four were ever selected: got {written}"
    );

    assert_eq!(
        count_intact_entries(dir.path()),
        written,
        "extract's own return value must match what is actually on disk — this is the \
         number `ExtractMsg::Cancelled {{ written }}` would carry verbatim, per \
         `spawn_extract` in `src/ui/mod.rs`"
    );
}
