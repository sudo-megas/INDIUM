//! Where copies live — P3 §1, corrected by P8 §3.
//!
//! Copy-out and Open With both hand real files to the outside world, so both need a
//! scratch area with rules. `$XDG_RUNTIME_DIR` is tmpfs — RAM — so a large selection
//! is routed to disk instead, and everything INDIUM made is removed on a clean exit.
//!
//! **The scratch roots are shared by every INDIUM on the machine, and until P8 the
//! names inside them were not.** P3 numbered a window's directories from one, so the
//! first copy-out of every window was `co-1` in one shared directory: a second window's
//! copy-out landed on top of the first window's files, and either window's next
//! operation removed a directory the other was still handing to a file manager. The
//! sweep was worse — it took every `co-*` and `ow-*` it found at launch, so opening a
//! second window deleted the first one's on-disk copy-out while the user was looking at
//! it. Neither needed a bug report to be a bug; both needed a second window, which
//! CORE §1 has told users to open since P1 and P8 finally opens for them. So the names
//! carry the process that made them, and the sweep asks whether that process is still
//! running before it removes anything.

use std::path::{Path, PathBuf};

/// P3 §1: above this, route to disk rather than filling RAM. "Constant in P3, not a
/// setting."
pub const RAM_LIMIT: u64 = 1024 * 1024 * 1024; // 1 GiB

/// Which operation a scratch directory belongs to. Starting a new operation of a kind
/// removes the previous directory of that kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    CopyOut,
    OpenWith,
}

impl Kind {
    pub fn prefix(self) -> &'static str {
        match self {
            Kind::CopyOut => "co",
            Kind::OpenWith => "ow",
        }
    }

    fn slot(self) -> usize {
        match self {
            Kind::CopyOut => 0,
            Kind::OpenWith => 1,
        }
    }
}

/// Where a scratch directory ended up, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placement {
    pub dir: PathBuf,
    /// True when the selection was too large for RAM and went to disk. The caller
    /// shows the one-line status notice P3 §1 asks for.
    pub on_disk: bool,
}

pub struct Scratch {
    /// `$XDG_RUNTIME_DIR/indium`. `None` when the variable is unset, which is
    /// legitimate — CORE §9 permits running as root, and root often has none.
    runtime_root: Option<PathBuf>,
    /// `$XDG_CACHE_HOME/indium/scratch`.
    cache_root: PathBuf,
    limit: u64,
    /// This window's process id, which is what keeps two windows' directory names
    /// apart in a root they share. Held rather than asked for each time, so every
    /// directory one window makes carries one number.
    pid: u32,
    counter: u32,
    current: [Option<PathBuf>; 2],
}

impl Scratch {
    pub fn new() -> Scratch {
        Scratch::with_roots(runtime_root(), cache_root(), RAM_LIMIT)
    }

    /// Explicit roots and threshold, so the routing rule can be tested without a
    /// gigabyte of anything.
    pub fn with_roots(runtime: Option<PathBuf>, cache: PathBuf, limit: u64) -> Scratch {
        Scratch {
            runtime_root: runtime,
            cache_root: cache,
            limit,
            pid: std::process::id(),
            counter: 0,
            current: [None, None],
        }
    }

    /// Which root a selection of `total_bytes` belongs in.
    ///
    /// Pure: no directory is created and nothing is removed, so the rule itself is
    /// testable. `on_disk` is true only when the *size* forced the choice — a missing
    /// `$XDG_RUNTIME_DIR` falls back "silently" (P3 §1) and is not worth a notice.
    pub fn route(&self, total_bytes: u64) -> (PathBuf, bool) {
        match &self.runtime_root {
            Some(rt) if total_bytes <= self.limit => (rt.clone(), false),
            Some(_) => (self.cache_root.clone(), true),
            None => (self.cache_root.clone(), false),
        }
    }

    /// Make a fresh directory for this kind of operation, removing the previous one.
    pub fn begin(&mut self, kind: Kind, total_bytes: u64) -> std::io::Result<Placement> {
        self.discard(kind);

        let (root, on_disk) = self.route(total_bytes);
        self.counter += 1;
        let dir = root.join(format!("{}-{}-{}", kind.prefix(), self.pid, self.counter));
        std::fs::create_dir_all(&dir)?;
        self.current[kind.slot()] = Some(dir.clone());
        Ok(Placement { dir, on_disk })
    }

    /// Remove this kind's current directory, if there is one.
    pub fn discard(&mut self, kind: Kind) {
        if let Some(old) = self.current[kind.slot()].take() {
            let _ = std::fs::remove_dir_all(&old);
        }
    }

    pub fn current(&self, kind: Kind) -> Option<&Path> {
        self.current[kind.slot()].as_deref()
    }

    /// Sweep leftovers from a previous run. P3 §1: "Stale `scratch/` cache entries are
    /// swept at launch" — the runtime dir's logout wipe is the backstop for the other
    /// root, but the cache is on disk and nothing else will ever clear it.
    ///
    /// **"Stale" now means the window that made it is gone.** P3's sweep read "ours" and
    /// removed it, which was right while only one INDIUM could be running and became a
    /// deletion of a live window's files the moment a second one could — see this
    /// module's own note. A directory is removed when its name says it belongs to no
    /// running process, and kept in every other case, including every case the name
    /// cannot answer.
    pub fn sweep_stale(&self) {
        let Ok(entries) = std::fs::read_dir(&self.cache_root) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let stale = match owner(&name) {
                // Not ours, and so not ours to delete.
                Owner::Stranger => false,
                // Ours, from a version that did not say whose. Nothing else will ever
                // clear it, and no running window is using a name in that form.
                Owner::Anonymous => true,
                Owner::Process(pid) => !is_running(pid),
            };
            if stale {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
    }
}

/// What a directory name under a scratch root says about who made it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Owner {
    /// Ours, and it names the window that made it: `co-<pid>-<n>`.
    Process(u32),
    /// Ours, but from a version before P8 put the process id in the name: `co-<n>`.
    Anonymous,
    /// Not ours. Never to be removed, whatever else is true of it.
    Stranger,
}

/// Read a directory name. Deliberately strict, for the reason P3 gave when the only
/// forms were `co-<digits>` and `ow-<digits>`: a sweep must never be able to delete
/// something INDIUM did not create. Anything that does not parse exactly is a
/// `Stranger`, including a process id too large to be one.
pub fn owner(name: &str) -> Owner {
    let Some((prefix, rest)) = name.split_once('-') else {
        return Owner::Stranger;
    };
    if !matches!(prefix, "co" | "ow") {
        return Owner::Stranger;
    }
    match rest.split_once('-') {
        Some((pid, n)) if digits(pid) && digits(n) => match pid.parse::<u32>() {
            Ok(pid) => Owner::Process(pid),
            Err(_) => Owner::Stranger,
        },
        Some(_) => Owner::Stranger,
        None if digits(rest) => Owner::Anonymous,
        None => Owner::Stranger,
    }
}

fn digits(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
}

/// Does this directory name look like one of ours?
pub fn is_ours(name: &str) -> bool {
    owner(name) != Owner::Stranger
}

/// Is that process id still on this machine?
///
/// `/proc/<pid>` is the whole test — CORE §9 is Linux only, and the directory is there
/// on every machine INDIUM runs on. A process id the kernel has since handed to some
/// other program reads as running and the directory is kept: the two mistakes are not
/// the same size, and one leftover directory swept on a later launch is the small one.
fn is_running(pid: u32) -> bool {
    Path::new("/proc").join(pid.to_string()).exists()
}

/// P3 §1's drop guard: "removes everything of ours on clean exit".
impl Drop for Scratch {
    fn drop(&mut self) {
        self.discard(Kind::CopyOut);
        self.discard(Kind::OpenWith);
    }
}

impl Default for Scratch {
    fn default() -> Self {
        Scratch::new()
    }
}

fn runtime_root() -> Option<PathBuf> {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .map(|p| p.join("indium"))
}

fn cache_root() -> PathBuf {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| super::home().join(".cache"));
    base.join("indium").join("scratch")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static N: AtomicU32 = AtomicU32::new(0);
        let p = std::env::temp_dir().join(format!(
            "indium-scratch-{}-{}-{}",
            tag,
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    /// P3 §5: "selection size above the guard routes to cache (threshold injected in
    /// the test), below routes to runtime".
    #[test]
    fn a_small_selection_stays_in_ram() {
        let base = tmp("small");
        let s = Scratch::with_roots(Some(base.join("run")), base.join("cache"), 1000);
        let (root, on_disk) = s.route(999);
        assert_eq!(root, base.join("run"));
        assert!(!on_disk);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_large_selection_goes_to_disk_with_a_notice() {
        let base = tmp("large");
        let s = Scratch::with_roots(Some(base.join("run")), base.join("cache"), 1000);
        let (root, on_disk) = s.route(1001);
        assert_eq!(root, base.join("cache"));
        assert!(on_disk, "the caller needs to know, to show the notice");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn exactly_the_limit_still_fits_in_ram() {
        let base = tmp("edge");
        let s = Scratch::with_roots(Some(base.join("run")), base.join("cache"), 1000);
        assert!(!s.route(1000).1);
        let _ = std::fs::remove_dir_all(&base);
    }

    /// P3 §1: a missing `$XDG_RUNTIME_DIR` is legitimate (root often has none) and
    /// falls back "silently" — no notice, because nothing surprising happened.
    #[test]
    fn a_missing_runtime_dir_falls_back_without_a_notice() {
        let base = tmp("noruntime");
        let s = Scratch::with_roots(None, base.join("cache"), 1000);
        let (root, on_disk) = s.route(10);
        assert_eq!(root, base.join("cache"));
        assert!(!on_disk, "the fallback is silent, not a size warning");
        let _ = std::fs::remove_dir_all(&base);
    }

    /// P3 §5: "new `co-<n>` removes `co-<n-1>`".
    #[test]
    fn starting_an_operation_removes_the_previous_one_of_that_kind() {
        let base = tmp("rotate");
        let mut s = Scratch::with_roots(Some(base.join("run")), base.join("cache"), 1000);

        let first = s.begin(Kind::CopyOut, 1).unwrap().dir;
        std::fs::write(first.join("a.txt"), b"x").unwrap();
        assert!(first.exists());

        let second = s.begin(Kind::CopyOut, 1).unwrap().dir;
        assert_ne!(first, second);
        assert!(
            !first.exists(),
            "the previous copy-out directory must be gone"
        );
        assert!(second.exists());

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn the_two_kinds_do_not_disturb_each_other() {
        let base = tmp("kinds");
        let mut s = Scratch::with_roots(Some(base.join("run")), base.join("cache"), 1000);

        let co = s.begin(Kind::CopyOut, 1).unwrap().dir;
        let ow = s.begin(Kind::OpenWith, 1).unwrap().dir;
        assert!(
            co.exists(),
            "starting an Open With must not remove the copy-out"
        );
        assert!(ow.exists());
        assert!(co.file_name().unwrap().to_string_lossy().starts_with("co-"));
        assert!(ow.file_name().unwrap().to_string_lossy().starts_with("ow-"));

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn dropping_the_scratch_removes_everything_of_ours() {
        let base = tmp("drop");
        let (co, ow);
        {
            let mut s = Scratch::with_roots(Some(base.join("run")), base.join("cache"), 1000);
            co = s.begin(Kind::CopyOut, 1).unwrap().dir;
            ow = s.begin(Kind::OpenWith, 1).unwrap().dir;
            assert!(co.exists() && ow.exists());
        }
        assert!(
            !co.exists(),
            "clean exit must remove the copy-out directory"
        );
        assert!(
            !ow.exists(),
            "clean exit must remove the Open With directory"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn the_sweep_only_touches_our_own_directories() {
        let base = tmp("sweep");
        let cache = base.join("cache");
        std::fs::create_dir_all(cache.join("co-1")).unwrap();
        std::fs::create_dir_all(cache.join("ow-42")).unwrap();
        std::fs::create_dir_all(cache.join("someone-elses")).unwrap();
        std::fs::create_dir_all(cache.join("co-not-a-number")).unwrap();

        let s = Scratch::with_roots(Some(base.join("run")), cache.clone(), 1000);
        s.sweep_stale();

        assert!(!cache.join("co-1").exists());
        assert!(!cache.join("ow-42").exists());
        assert!(cache.join("someone-elses").exists(), "not ours to delete");
        assert!(cache.join("co-not-a-number").exists(), "not our naming");

        let _ = std::fs::remove_dir_all(&base);
    }

    /// The collision P3 could not have: every window numbers its copy-outs from one, so
    /// without the process id in the name the first copy-out of every window on the
    /// machine is the same directory in the same shared root.
    #[test]
    fn a_scratch_directory_says_which_window_made_it() {
        let base = tmp("owner");
        let mut s = Scratch::with_roots(Some(base.join("run")), base.join("cache"), 1000);

        let dir = s.begin(Kind::CopyOut, 1).unwrap().dir;
        let name = dir.file_name().unwrap().to_string_lossy().to_string();
        assert_eq!(
            owner(&name),
            Owner::Process(std::process::id()),
            "a name that does not carry the window's process id collides with every other window's"
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    /// The bug this replaces: launching a second window swept the first one's on-disk
    /// copy-out while the user was still looking at it.
    #[test]
    fn the_sweep_leaves_a_running_windows_copy_out_alone() {
        let base = tmp("live");
        let cache = base.join("cache");
        let mine = cache.join(format!("co-{}-1", std::process::id()));
        std::fs::create_dir_all(&mine).unwrap();
        std::fs::write(mine.join("held.txt"), b"a file handed to a file manager").unwrap();

        let s = Scratch::with_roots(Some(base.join("run")), cache.clone(), 1000);
        s.sweep_stale();

        assert!(
            mine.join("held.txt").exists(),
            "a second window must not delete the files a first window is still handing out"
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    /// The other half of the same rule: a window that died still has to be cleaned up
    /// after, or the cache grows for ever. The dead process id is a real one — a child
    /// this test starts and reaps — rather than a number assumed to be free.
    #[test]
    fn the_sweep_removes_a_dead_windows_copy_out() {
        let base = tmp("dead");
        let cache = base.join("cache");

        let mut child = std::process::Command::new("/bin/true").spawn().unwrap();
        let gone = child.id();
        child.wait().unwrap();

        let theirs = cache.join(format!("co-{gone}-1"));
        std::fs::create_dir_all(&theirs).unwrap();

        let s = Scratch::with_roots(Some(base.join("run")), cache.clone(), 1000);
        s.sweep_stale();

        assert!(
            !theirs.exists(),
            "nothing else will ever clear the cache root, so a dead window's directory must go"
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    /// A process id too large to be one is not a licence to delete the directory.
    #[test]
    fn a_name_we_cannot_read_is_never_swept() {
        assert_eq!(owner("co-99999999999999-1"), Owner::Stranger);
        assert_eq!(owner("co-1-2-3"), Owner::Stranger);
        assert_eq!(owner("co--1"), Owner::Stranger);
    }

    #[test]
    fn our_directory_names_are_recognised_strictly() {
        assert!(is_ours("co-1"));
        assert!(is_ours("ow-999"));
        assert!(!is_ours("co-"));
        assert!(!is_ours("co"));
        assert!(!is_ours("xx-1"));
        assert!(!is_ours("co-1a"));
        assert!(!is_ours("important-data"));
    }
}
