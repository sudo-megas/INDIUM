//! Archive state and the pure logic behind what the table shows.
//!
//! P1 §3: "directory structure is derived from entry paths, not trusted from the
//! archive." Everything in this file is pure and unit-tested, so the table's
//! behaviour can be proven without opening a window.

use std::collections::{BTreeMap, BTreeSet};

use crate::arch::Entry;
use crate::util;

/// Where a row came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    /// A real member of the archive, by index into the entry list.
    Entry(usize),
    /// A directory nothing stored explicitly, inferred because entries live under it.
    ImplicitDir,
}

/// One line of the entry table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// What the Name column shows: a bare name when browsing, a full path when
    /// filtering.
    pub display: String,
    /// The full archive path this row stands for.
    pub path: String,
    pub is_dir: bool,
    pub kind: RowKind,
}

impl Row {
    /// The entry behind this row, if it is a real one.
    pub fn entry_index(&self) -> Option<usize> {
        match self.kind {
            RowKind::Entry(i) => Some(i),
            RowKind::ImplicitDir => None,
        }
    }
}

/// Rows for a directory inside the archive. `cwd` is `""` at the root.
///
/// Directories sort before files, then case-insensitively by name — the order a
/// person expects, not the order the archive happens to store.
pub fn rows_for(entries: &[Entry], cwd: &str) -> Vec<Row> {
    let prefix = if cwd.is_empty() {
        String::new()
    } else {
        format!("{cwd}/")
    };

    let mut direct: BTreeMap<String, Row> = BTreeMap::new();
    let mut implicit: BTreeSet<String> = BTreeSet::new();

    for (i, entry) in entries.iter().enumerate() {
        if !prefix.is_empty() && !entry.path.starts_with(&prefix) {
            continue;
        }
        let rest = &entry.path[prefix.len()..];
        if rest.is_empty() {
            continue; // the directory itself
        }
        match rest.split_once('/') {
            Some((first, _)) => {
                implicit.insert(first.to_string());
            }
            None => {
                direct.insert(
                    rest.to_string(),
                    Row {
                        display: rest.to_string(),
                        path: entry.path.clone(),
                        is_dir: entry.is_dir,
                        kind: RowKind::Entry(i),
                    },
                );
            }
        }
    }

    let mut rows: Vec<Row> = Vec::with_capacity(direct.len() + implicit.len());
    rows.extend(direct.values().cloned());
    for name in implicit {
        if direct.contains_key(&name) {
            continue; // the archive stored this directory explicitly
        }
        let path = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}{name}")
        };
        rows.push(Row {
            display: name,
            path,
            is_dir: true,
            kind: RowKind::ImplicitDir,
        });
    }

    rows.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.display.to_lowercase().cmp(&b.display.to_lowercase()))
            .then_with(|| a.display.cmp(&b.display))
    });
    rows
}

/// Rows for the filter bar.
///
/// P2 §4: while filtering the table goes **flat and whole-archive** — full paths,
/// case-insensitive substring, "because the moment you reach for a filter in an
/// 11,000-entry archive, the directory you happen to be standing in is the wrong
/// scope."
pub fn rows_for_filter(entries: &[Entry], needle: &str) -> Vec<Row> {
    let needle = needle.to_lowercase();
    let mut rows: Vec<Row> = entries
        .iter()
        .enumerate()
        .filter(|(_, e)| e.path.to_lowercase().contains(&needle))
        .map(|(i, e)| Row {
            display: e.path.clone(),
            path: e.path.clone(),
            is_dir: e.is_dir,
            kind: RowKind::Entry(i),
        })
        .collect();
    rows.sort_by_key(|r| r.path.to_lowercase());
    rows
}

/// Every ancestor of `cwd`, root first, for the breadcrumb.
pub fn breadcrumb(cwd: &str) -> Vec<(String, String)> {
    let mut out = vec![("Archive".to_string(), String::new())];
    if cwd.is_empty() {
        return out;
    }
    let mut acc = String::new();
    for part in cwd.split('/') {
        if part.is_empty() {
            continue;
        }
        if acc.is_empty() {
            acc = part.to_string();
        } else {
            acc = format!("{acc}/{part}");
        }
        out.push((part.to_string(), acc.clone()));
    }
    out
}

/// The parent of a directory path, or `None` at the root.
pub fn parent_of(cwd: &str) -> Option<String> {
    if cwd.is_empty() {
        return None;
    }
    Some(util::parent_dir(cwd).to_string())
}

/// What the Inspector shows when more than one row is selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Aggregate {
    pub count: usize,
    pub files: usize,
    pub dirs: usize,
    pub total_real: u64,
    /// Sum of the packed sizes actually reported. `None` while no format reports any,
    /// which with the generic reader is always — see `Entry::packed`.
    pub total_packed: Option<u64>,
}

pub fn aggregate<'a>(entries: impl IntoIterator<Item = &'a Entry>) -> Aggregate {
    let mut agg = Aggregate::default();
    let mut packed_sum: Option<u64> = None;
    for e in entries {
        agg.count += 1;
        if e.is_dir {
            agg.dirs += 1;
        } else {
            agg.files += 1;
        }
        agg.total_real += e.size;
        if let Some(p) = e.packed {
            packed_sum = Some(packed_sum.unwrap_or(0) + p);
        }
    }
    agg.total_packed = packed_sum;
    agg
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str, is_dir: bool, size: u64) -> Entry {
        Entry {
            raw_path: path.to_string(),
            path: path.to_string(),
            is_dir,
            size,
            packed: None,
            method: "store".to_string(),
            mtime: None,
            atime: None,
            ctime: None,
            birthtime: None,
            uid: 0,
            gid: 0,
            uname: None,
            gname: None,
            mode: 0o644,
            filetype: if is_dir { 0o040000 } else { 0o100000 },
            symlink: None,
            hardlink: None,
            encrypted: false,
        }
    }

    fn sample() -> Vec<Entry> {
        vec![
            entry("alpha.txt", false, 21),
            entry("beta.txt", false, 20),
            entry("sub", true, 0),
            entry("sub/gamma.txt", false, 21),
        ]
    }

    #[test]
    fn root_shows_top_level_only() {
        let rows = rows_for(&sample(), "");
        let names: Vec<&str> = rows.iter().map(|r| r.display.as_str()).collect();
        assert_eq!(names, vec!["sub", "alpha.txt", "beta.txt"]);
    }

    #[test]
    fn descending_shows_the_children() {
        let rows = rows_for(&sample(), "sub");
        let names: Vec<&str> = rows.iter().map(|r| r.display.as_str()).collect();
        assert_eq!(names, vec!["gamma.txt"]);
        assert_eq!(rows[0].path, "sub/gamma.txt");
    }

    #[test]
    fn directories_sort_before_files() {
        let entries = vec![entry("zzz_dir", true, 0), entry("aaa_file.txt", false, 1)];
        let rows = rows_for(&entries, "");
        assert!(rows[0].is_dir, "a directory must come first");
        assert_eq!(rows[0].display, "zzz_dir");
    }

    /// P1 §3: structure is derived from paths, not trusted from the archive. An
    /// archive with no stored directory entry must still browse.
    #[test]
    fn directories_are_inferred_when_the_archive_stores_none() {
        let entries = vec![entry("deep/nested/file.txt", false, 10)];

        let root = rows_for(&entries, "");
        assert_eq!(root.len(), 1);
        assert_eq!(root[0].display, "deep");
        assert!(root[0].is_dir);
        assert_eq!(root[0].kind, RowKind::ImplicitDir);

        let mid = rows_for(&entries, "deep");
        assert_eq!(mid[0].display, "nested");
        assert!(mid[0].is_dir);

        let leaf = rows_for(&entries, "deep/nested");
        assert_eq!(leaf[0].display, "file.txt");
        assert_eq!(leaf[0].kind, RowKind::Entry(0));
    }

    #[test]
    fn a_stored_directory_is_not_duplicated_by_an_inferred_one() {
        let rows = rows_for(&sample(), "");
        let subs: Vec<_> = rows.iter().filter(|r| r.display == "sub").collect();
        assert_eq!(subs.len(), 1, "sub appeared twice");
        assert_eq!(subs[0].kind, RowKind::Entry(2), "the stored entry must win");
    }

    #[test]
    fn a_prefix_that_is_not_a_directory_boundary_does_not_match() {
        let entries = vec![entry("sub", true, 0), entry("subtle.txt", false, 5)];
        let rows = rows_for(&entries, "sub");
        assert!(rows.is_empty(), "subtle.txt is not inside sub/");
    }

    #[test]
    fn filtering_goes_flat_and_whole_archive() {
        let rows = rows_for_filter(&sample(), "gamma");
        assert_eq!(rows.len(), 1);
        // Full path, not a bare name — the point of the flat view.
        assert_eq!(rows[0].display, "sub/gamma.txt");
    }

    #[test]
    fn filtering_is_case_insensitive() {
        assert_eq!(rows_for_filter(&sample(), "GAMMA").len(), 1);
        assert_eq!(rows_for_filter(&sample(), "gAmMa").len(), 1);
    }

    #[test]
    fn filtering_matches_across_directories() {
        let rows = rows_for_filter(&sample(), ".txt");
        assert_eq!(rows.len(), 3, "alpha, beta and sub/gamma all match");
    }

    #[test]
    fn an_empty_filter_matches_everything() {
        assert_eq!(rows_for_filter(&sample(), "").len(), 4);
    }

    #[test]
    fn breadcrumbs_walk_back_to_the_root() {
        assert_eq!(breadcrumb(""), vec![("Archive".to_string(), String::new())]);
        assert_eq!(
            breadcrumb("a/b"),
            vec![
                ("Archive".to_string(), "".to_string()),
                ("a".to_string(), "a".to_string()),
                ("b".to_string(), "a/b".to_string()),
            ]
        );
    }

    #[test]
    fn going_up_stops_at_the_root() {
        assert_eq!(parent_of("a/b"), Some("a".to_string()));
        assert_eq!(parent_of("a"), Some(String::new()));
        assert_eq!(parent_of(""), None);
    }

    #[test]
    fn aggregates_count_and_total() {
        let entries = sample();
        let agg = aggregate(entries.iter());
        assert_eq!(agg.count, 4);
        assert_eq!(agg.files, 3);
        assert_eq!(agg.dirs, 1);
        assert_eq!(agg.total_real, 62);
        assert_eq!(agg.total_packed, None, "no format reported a packed size");
    }

    #[test]
    fn aggregating_nothing_is_zero_not_a_panic() {
        let agg = aggregate(std::iter::empty());
        assert_eq!(agg.count, 0);
        assert_eq!(agg.total_real, 0);
    }
}
