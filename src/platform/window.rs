//! The second window — P8 §1.
//!
//! CORE §1 has said since the first line of the first document: *"One archive per
//! window. Opening a second archive opens a second window. There are no tabs."* Every
//! consequence of that sentence was built long ago — P4's advisory lock is keyed to the
//! archive path and lives in `$XDG_RUNTIME_DIR` so that *two processes* cannot rebuild
//! one file, P5 put the archive's name in the title so two windows can be told apart,
//! and P3 recorded that "multiple windows on one archive are permitted and safe today".
//! The one thing never built was the window itself. Until P8 the program answered a
//! second archive with a sentence telling the *user* to go and open one by hand.
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

/// Where an archive the user asked for should open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Destination {
    /// Here. Either this window holds nothing yet, or it already holds this same
    /// archive and is being handed it again.
    ThisWindow,
    /// A second window, because this one is already spoken for.
    NewWindow,
}

/// The whole of CORE §1, as one function.
///
/// `open_archive` is reached from seven places — the command line, a drop, `Ctrl+O`, a
/// click or `Enter` on a recent, the password prompt's resume, and Apply's own re-open.
/// Two of those seven hand back the archive that is already open: the password prompt
/// re-opens it with the passphrase, and Apply re-opens it after the rebuild. Both must
/// keep the window they are in, and neither is an exception written down anywhere — they
/// fall out of the same rule everything else follows, because the archive they name is
/// the archive already there.
///
/// Paths are compared **canonicalised**, for the reason `tasks::lock_name_for` gives for
/// doing the same: paths arrive from `std::env::args`, a drop, and a hand-typed field,
/// so `./photos.7z` and `/home/megas/photos.7z` are the ordinary case rather than the
/// exotic one. A path that cannot be canonicalised — an archive that is not there —
/// falls back to the name as given, which is correct: nothing that does not exist can be
/// the archive this window already holds.
pub fn destination(current: Option<&Path>, requested: &Path) -> Destination {
    match current {
        None => Destination::ThisWindow,
        Some(current) if resolve(current) == resolve(requested) => Destination::ThisWindow,
        Some(_) => Destination::NewWindow,
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
pub fn open_new(path: &Path) -> Result<(), String> {
    let exe = std::env::current_exe()
        .map_err(|e| format!("Could not find INDIUM's own program to open a window with: {e}"))?;

    let child = std::process::Command::new(exe)
        .arg(path)
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
    use super::*;

    #[test]
    fn an_empty_window_takes_the_archive_itself() {
        assert_eq!(
            destination(None, Path::new("/tmp/photos.7z")),
            Destination::ThisWindow,
            "the command line's archive must not open a second window and leave an empty first one"
        );
    }

    #[test]
    fn a_window_that_holds_one_archive_opens_another_elsewhere() {
        assert_eq!(
            destination(
                Some(Path::new("/tmp/photos.7z")),
                Path::new("/tmp/docs.zip")
            ),
            Destination::NewWindow
        );
    }

    /// Apply re-opens the archive it just rebuilt, and the password prompt re-opens the
    /// archive it just unlocked. Neither may spawn a window: the first would leave the
    /// rebuilt archive listed twice, and the second would put the unlocked archive in a
    /// window that never got the passphrase.
    #[test]
    fn handing_a_window_the_archive_it_already_holds_keeps_the_window() {
        assert_eq!(
            destination(
                Some(Path::new("/tmp/photos.7z")),
                Path::new("/tmp/photos.7z")
            ),
            Destination::ThisWindow
        );
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
        assert_eq!(
            destination(Some(&archive), &indirect),
            Destination::ThisWindow,
            "a second spelling of the open archive must not open a second window"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The fallback for a path that cannot be canonicalised must not collapse two
    /// different missing archives into one, or a window holding a deleted archive would
    /// refuse to open anything.
    #[test]
    fn two_archives_that_are_not_there_are_still_two_archives() {
        assert_eq!(
            destination(
                Some(Path::new("/nowhere/gone-a.7z")),
                Path::new("/nowhere/gone-b.7z")
            ),
            Destination::NewWindow
        );
    }
}
