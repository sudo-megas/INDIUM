//! Where copies live — P3 §1.
//!
//! Copy-out and Open With both hand real files to the outside world, so both need a
//! scratch area with rules. `$XDG_RUNTIME_DIR` is tmpfs — RAM — so a large selection
//! is routed to disk instead, and everything INDIUM made is removed on a clean exit.

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
        let dir = root.join(format!("{}-{}", kind.prefix(), self.counter));
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
    pub fn sweep_stale(&self) {
        let Ok(entries) = std::fs::read_dir(&self.cache_root) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if is_ours(&name) {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
    }
}

/// Does this directory name look like one of ours? Deliberately strict: only
/// `co-<digits>` and `ow-<digits>`, so a sweep can never delete something it did not
/// create.
pub fn is_ours(name: &str) -> bool {
    let Some((prefix, n)) = name.split_once('-') else {
        return false;
    };
    matches!(prefix, "co" | "ow") && !n.is_empty() && n.chars().all(|c| c.is_ascii_digit())
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
