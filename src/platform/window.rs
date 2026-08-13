//! The second window — P8 §1.
//!
//! CORE §1 said, from the first line of the first document until P22: *"One archive per
//! window. Opening a second archive opens a second window. There are no tabs."* Every
//! consequence of that sentence was built long ago — P4's advisory lock is keyed to the
//! archive path and lives in `$XDG_RUNTIME_DIR` so that *two processes* cannot rebuild
//! one file, P5 put the archive's name in the title so two windows can be told apart,
//! and P3 recorded that "multiple windows on one archive are permitted and safe today".
//! The one thing never built was the window itself. Until P8 the program answered a
//! second archive with a sentence telling the *user* to go and open one by hand.
//!
//! **P22 amended the rule that quote comes from.** §1 now says a window holds one archive
//! *at a time*: a file manager or a command line naming a second archive still gets a
//! second window, and that is what `open_new` below is for and all it is for. But a person
//! already inside INDIUM who opens another archive is not asking for two — they are done
//! with the one they have. So an in-program open closes what this window holds and takes
//! the next one here, and the question this module answers shrank to the only one left
//! worth asking: **is the archive being handed to us the one already here?**
//!
//! A window is a process. That is not the only way egui can do it — eframe 0.36 will
//! open genuine `xdg_toplevel`s for deferred viewports on Wayland, and the glow backend
//! implements them — but three of its properties disqualify it here, and none of them is
//! a matter of taste:
//!
//! - **The root window's close takes every other window with it.** eframe keys the
//!   event loop's exit on `ViewportId::ROOT` alone and never consults how many child
//!   viewports are alive; the close handler drops the whole running context, and every
//!   child's window is dropped with it. Windows that die when their eldest sibling dies
//!   are not the peers CORE §1 describes.
//! - **One archive per window becomes one worker per viewport.** CORE §3 gives INDIUM
//!   "the UI thread and one worker", and a process per window is what keeps that
//!   sentence literally true.
//! - **A deferred viewport's callback may not borrow the app.** egui requires it to be
//!   `Send + Sync + 'static`, so every field of a 2,200-line window would move behind a
//!   lock to buy windows that die together anyway.
//!
//! Spawning INDIUM is not the external-binary violation CORE §9 forbids: the ban is on
//! external *compressors* — `7z`, `tar`, `unzip` — because format work must happen in
//! this process. INDIUM is not a compressor INDIUM shells out to, and the second window
//! does its own format work in its own address space, which is the rule rather than an
//! exception to it.

use std::path::{Path, PathBuf};

/// Is `requested` the archive this window already holds?
///
/// `open_archive` is reached from seven places — the command line, a drop, `Ctrl+O`, a
/// click or `Enter` on a recent, the password prompt's resume, and Apply's own re-open.
/// Two of those seven hand back the archive that is already open: the password prompt
/// re-opens it with the passphrase, and Apply re-opens it after the rebuild. Neither is
/// *leaving* anything, and under P22's §1 that distinction became load-bearing: a
/// replacing open closes what is here first, and closing here would throw away the
/// passphrase the prompt has just taken and re-empty a tray Apply has already emptied.
/// Neither is an exception written down anywhere — both fall out of this one question,
/// because the archive they name is the archive already there.
///
/// Paths are compared **canonicalised**, for the reason `tasks::lock_name_for` gives for
/// doing the same: paths arrive from `std::env::args_os`, a drop, and a hand-typed field,
/// so `./photos.7z` and `/home/megas/photos.7z` are the ordinary case rather than the
/// exotic one. A path that cannot be canonicalised — an archive that is not there —
/// falls back to the name as given, which is correct: nothing that does not exist can be
/// the archive this window already holds.
pub fn already_open(current: Option<&Path>, requested: &Path) -> bool {
    match current {
        None => false,
        Some(current) => resolve(current) == resolve(requested),
    }
}

fn resolve(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

/// Open `path` in a second INDIUM, or say why not.
///
/// The child is INDIUM's own binary with the archive as its one argument, which is the
/// command CORE §1 has been telling the user to type since P1. It inherits this
/// window's environment and streams deliberately: a second window launched from a
/// terminal should report to that terminal, exactly as a second window launched by hand
/// would.
///
/// **No passphrase is ever passed.** There is no argument for one and there will not be:
/// CORE §9 keeps passwords out of settings, recents "and anywhere else", and a command
/// line is world-readable in `/proc`. The two callers that hold a secret are the two
/// that re-open the archive already open, so they never reach this function — and
/// `open_archive` drops the secret rather than carry it here if that ever changes.
/// A path the child cannot mistake for a subcommand.
///
/// **P17 broke a P8 feature and the sweep caught it.** The terminal half claims `argv[1]`
/// when it is byte-exactly `list`, `extract` or `cat` — and this function hands the child a
/// bare path. So `indium photos.zip list`, with a real archive named `list` beside it,
/// spawned a child that printed a usage error onto the terminal they share and opened no
/// window at all. The archive was fine; its *name* was the whole defect.
///
/// `./list` is what a person is told to type for the same reason, and it is what the child
/// is handed here. Only a relative path with no directory part can collide, so only that
/// case is touched — an absolute path or anything holding a `/` already reads as a path.
fn unambiguous(path: &Path) -> std::path::PathBuf {
    if crate::cli::takes_the_terminal(&[path.as_os_str().to_os_string()]) {
        return std::path::PathBuf::from(".").join(path);
    }
    path.to_path_buf()
}

pub fn open_new(path: &Path) -> Result<(), String> {
    let exe = std::env::current_exe()
        .map_err(|e| format!("Could not find INDIUM's own program to open a window with: {e}"))?;

    let child = std::process::Command::new(exe)
        .arg(unambiguous(path))
        .spawn()
        .map_err(|e| format!("Could not open a second window: {e}"))?;

    reap(child);
    Ok(())
}

/// Wait for a child window in a thread of its own, so a closed window leaves no zombie.
///
/// A spawned child that is never waited for stays in the process table as `<defunct>`
/// until its parent exits, and INDIUM's parent window is expected to outlive many
/// children. The alternative — polling `try_wait` from the frame loop — would put a
/// syscall per child in the repaint path to save a thread that spends its whole life
/// blocked in `wait`, and CORE §3's "an idle INDIUM repaints nothing and costs nothing"
/// is the sentence that decides between them.
fn reap(mut child: std::process::Child) {
    std::thread::spawn(move || {
        let _ = child.wait();
    });
}

#[cfg(test)]
mod tests {
    /// P17 broke P8's second window for any archive whose bare name is a subcommand: the
    /// child claims `argv[1]` when it is byte-exactly `list`, `extract` or `cat`, and this
    /// function used to hand it one. The archive was fine; its *name* was the whole defect.
    ///
    /// Asserted against `cli::takes_the_terminal` rather than against a literal, so the day
    /// a fourth subcommand is added this test starts covering it without being edited.
    #[test]
    fn a_second_window_is_never_handed_a_path_its_child_would_read_as_a_subcommand() {
        use std::ffi::OsString;

        for word in crate::cli::SUBCOMMANDS {
            let given = std::path::Path::new(word);
            let passed = super::unambiguous(given);
            assert!(
                !crate::cli::takes_the_terminal(&[passed.as_os_str().to_os_string()]),
                "a child handed {passed:?} would print a usage error instead of opening a window"
            );
            assert_eq!(
                passed,
                std::path::PathBuf::from(format!("./{word}")),
                "the escape should be the one USAGE tells a person to type"
            );
        }

        // Everything else is passed through untouched — no path is rewritten for its own
        // sake, only the handful that would be misread.
        for ordinary in [
            "photos.zip",
            "./list",
            "/tmp/list",
            "sub/cat",
            "listing.zip",
        ] {
            let p = std::path::Path::new(ordinary);
            assert_eq!(
                super::unambiguous(p),
                p.to_path_buf(),
                "{ordinary} was rewritten"
            );
        }
        let _ = OsString::new();
    }

    use super::*;

    // `an_empty_window_takes_the_archive_itself` and
    // `a_window_that_holds_one_archive_opens_another_elsewhere` stood here until P22,
    // holding `destination`. Both are gone with it, and deliberately: the first is now
    // unconditional — *every* window takes the archive it is handed, so there is no
    // branch left to assert — and the second asserted the exact behaviour this round
    // reverses. What survives of the pair is the question below, which is the half of
    // `destination` that was doing real work.

    /// An empty window holds nothing, so an archive handed to it leaves nothing behind.
    /// The launch case, and the reason the answer here is a plain `false` rather than a
    /// third state: "nothing open" and "a different archive open" both end with this
    /// window holding the archive, and only the second has anything to close first.
    #[test]
    fn an_empty_window_has_nothing_to_leave() {
        assert!(!already_open(None, Path::new("/tmp/photos.7z")));
    }

    /// Apply re-opens the archive it just rebuilt, and the password prompt re-opens the
    /// archive it just unlocked. Neither may be read as leaving it: the first would clear
    /// a tray Apply has already emptied, and the second would wipe the passphrase the
    /// prompt has just taken and fail straight back to a second prompt.
    #[test]
    fn handing_a_window_the_archive_it_already_holds_is_not_leaving_it() {
        assert!(already_open(
            Some(Path::new("/tmp/photos.7z")),
            Path::new("/tmp/photos.7z")
        ));
    }

    /// The archive this window does not hold is the one it must close for.
    #[test]
    fn a_different_archive_is_a_different_archive() {
        assert!(!already_open(
            Some(Path::new("/tmp/photos.7z")),
            Path::new("/tmp/docs.zip")
        ));
    }

    /// `indium ./photos.7z` in one window and a recent naming `/tmp/.../photos.7z` are
    /// the same archive, and the rule must not be defeated by which way it was spelled —
    /// the same defeat `tasks::lock_name_for` canonicalises to avoid.
    #[test]
    fn the_same_archive_by_two_spellings_is_still_the_same_archive() {
        let dir = std::env::temp_dir().join(format!("indium-window-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let archive = dir.join("photos.7z");
        std::fs::write(&archive, b"not really an archive").unwrap();

        let indirect = dir.join(".").join("photos.7z");
        assert!(
            already_open(Some(&archive), &indirect),
            "a second spelling of the open archive must not be read as a different one"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The fallback for a path that cannot be canonicalised must not collapse two
    /// different missing archives into one, or a window holding a deleted archive would
    /// think it already held whatever it was handed next and skip the close.
    #[test]
    fn two_archives_that_are_not_there_are_still_two_archives() {
        assert!(!already_open(
            Some(Path::new("/nowhere/gone-a.7z")),
            Path::new("/nowhere/gone-b.7z")
        ));
    }
}
