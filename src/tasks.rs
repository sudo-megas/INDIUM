//! The staging engine — CORE §3's `tasks`, implemented by P4 §1.
//!
//! CORE §3: "Every mutation — add, remove, rename, create — is a task in a queue.
//! **Apply** builds the new archive in a temp file beside the target, verifies it by
//! walking its entries, then atomically renames over the original. The original is never
//! touched until the replacement is proven."
//!
//! This file is the pure half: the queue you edit, the fold that turns it into a plan,
//! and the judgements that decide whether a rebuild is allowed at all. It performs no
//! I/O and opens no archive, which is why nearly every rule below is directly testable.
//!
//! Copyright © sudo-megas. GPL-3.0-only.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::arch::{path_escapes, ArchiveInfo, Entry};
use crate::util::normalize_archive_path;

// ---------------------------------------------------------------------------
// Methods, containers, and the recipe
// ---------------------------------------------------------------------------

/// A compression method. CORE §5 lists exactly these on the write side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Method {
    Store,
    Lz4,
    Gzip,
    Zstd,
    Bzip2,
    Xz,
    Lzma2,
    Deflate,
}

/// A container. CORE §5's write list is tar with five filters, zip, and 7z.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Container {
    Tar,
    Zip,
    SevenZ,
}

impl Method {
    /// CORE §5's verdict for this method, **verbatim**.
    ///
    /// CORE §5: "This copy ships in the New Archive popup, one honest sentence each,
    /// static in v1.x — the live estimator that measures *your* data on *your* CPU is
    /// V2.0." Copied by hand from the table there and pinned by a test, so a future
    /// edit to CORE cannot drift away from the window without someone noticing.
    pub fn verdict(self) -> &'static str {
        match self {
            Method::Store => "No compression — instant, and as large as the input.",
            Method::Lz4 => "The fastest real compression there is, and the largest result.",
            Method::Gzip => "Fast, everywhere, and beaten in both speed and size by zstd.",
            Method::Zstd => "Very fast with a small archive — the sane default.",
            Method::Bzip2 => {
                "Slower than gzip for a somewhat smaller file; kept for compatibility."
            }
            Method::Xz => "Among the smallest archives, built slowly; extraction is quick enough.",
            Method::Lzma2 => {
                "Smallest for mixed content, slow to build — and the only road to AES-256."
            }
            Method::Deflate => "Not the smallest or fastest, but opens absolutely anywhere.",
        }
    }

    /// The name shown in the method list and in the live sentence.
    pub fn label(self) -> &'static str {
        match self {
            Method::Store => "Store",
            Method::Lz4 => "lz4",
            Method::Gzip => "gzip",
            Method::Zstd => "zstd",
            Method::Bzip2 => "bzip2",
            Method::Xz => "xz",
            Method::Lzma2 => "LZMA2",
            Method::Deflate => "Deflate",
        }
    }

    /// The levels this method accepts, or `None` where a level is meaningless.
    ///
    /// The libarchive ranges come from `archive_write_set_options(3)` on the build
    /// machine, and then from asking libarchive itself, because on one filter the two
    /// disagree: the manual gives **lz4** as 0–9, and libarchive refuses
    /// `lz4:compression-level=0` outright. Every range below has been offered to
    /// libarchive at both ends and accepted — `every_level_a_method_offers_is_one_
    /// libarchive_accepts` in `tests/write_path.rs` is what keeps that true, and it is
    /// the test that caught the lz4 case. LZMA2's range is `sevenz-rust2`'s, where an
    /// out-of-range level is clamped rather than refused.
    pub fn levels(self) -> Option<std::ops::RangeInclusive<u32>> {
        match self {
            Method::Store => None,
            Method::Lz4 => Some(1..=9),
            Method::Gzip => Some(0..=9),
            Method::Zstd => Some(1..=22),
            Method::Bzip2 => Some(1..=9),
            Method::Xz => Some(0..=9),
            Method::Lzma2 => Some(0..=9),
            Method::Deflate => Some(0..=9),
        }
    }

    /// The level used when nobody has touched the slider.
    pub fn default_level(self) -> u32 {
        match self {
            Method::Store => 0,
            Method::Lz4 => 1,
            Method::Gzip => 6,
            Method::Zstd => 3,
            Method::Bzip2 => 9,
            Method::Xz => 6,
            Method::Lzma2 => 6,
            Method::Deflate => 6,
        }
    }

    /// Clamp a level into this method's range, so a method change cannot strand the
    /// slider at a value the backend would reject.
    pub fn clamp_level(self, level: u32) -> u32 {
        match self.levels() {
            None => 0,
            Some(r) => level.clamp(*r.start(), *r.end()),
        }
    }

    /// The container this method belongs to. Each method writes exactly one.
    pub fn container(self) -> Container {
        match self {
            Method::Store
            | Method::Lz4
            | Method::Gzip
            | Method::Zstd
            | Method::Bzip2
            | Method::Xz => Container::Tar,
            Method::Deflate => Container::Zip,
            Method::Lzma2 => Container::SevenZ,
        }
    }
}

/// Every method, in the order CORE §5 lists them.
pub const METHODS: [Method; 8] = [
    Method::Store,
    Method::Lz4,
    Method::Gzip,
    Method::Zstd,
    Method::Bzip2,
    Method::Xz,
    Method::Lzma2,
    Method::Deflate,
];

/// CORE §4.1's four preset chips.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    Fastest,
    Balanced,
    Smallest,
    Encrypted,
}

impl Preset {
    /// The method and encryption each preset selects.
    ///
    /// None of these is arbitrary: each preset picks the method whose own CORE §5
    /// verdict claims that exact virtue — lz4 "the fastest real compression there is",
    /// zstd "the sane default", LZMA2 "smallest for mixed content" and "the only road
    /// to AES-256".
    pub fn recipe_parts(self) -> (Method, bool) {
        match self {
            Preset::Fastest => (Method::Lz4, false),
            Preset::Balanced => (Method::Zstd, false),
            Preset::Smallest => (Method::Lzma2, false),
            Preset::Encrypted => (Method::Lzma2, true),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Preset::Fastest => "Fastest",
            Preset::Balanced => "Balanced",
            Preset::Smallest => "Smallest",
            Preset::Encrypted => "Encrypted",
        }
    }
}

/// Everything needed to build an archive: what it is called, how it is compressed, and
/// whether it is encrypted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recipe {
    pub path: PathBuf,
    pub method: Method,
    pub level: u32,
    pub encrypt: bool,
}

impl Recipe {
    pub fn container(&self) -> Container {
        self.method.container()
    }

    /// CORE §9: "No zip encryption — 7z AES-256 is the only encryption."
    pub fn encryption_is_legal(&self) -> bool {
        !self.encrypt || self.container() == Container::SevenZ
    }

    /// The recipe an already-open archive implies, or `None` when CORE §5 does not list
    /// its format as writable.
    ///
    /// P4 §1: "A format INDIUM reads but CORE §5 does not list as writable cannot be
    /// staged against." The level cannot be recovered from a finished archive — nothing
    /// records it — so the method's default stands in.
    pub fn from_info(info: &ArchiveInfo, path: &Path, encrypted: bool) -> Option<Recipe> {
        let method = method_for(info)?;
        Some(Recipe {
            path: path.to_path_buf(),
            method,
            level: method.default_level(),
            encrypt: encrypted && method.container() == Container::SevenZ,
        })
    }
}

/// Map libarchive's format and filter names onto a writable method.
///
/// Kept separate and pure so the mapping is testable without an archive. Anything not
/// named here is a format INDIUM reads and does not write, and the caller refuses.
fn method_for(info: &ArchiveInfo) -> Option<Method> {
    let format = info.format.to_ascii_uppercase();
    let filter = info.filter.to_ascii_lowercase();

    if format.contains("7-ZIP") || format.contains("7ZIP") {
        return Some(Method::Lzma2);
    }
    if format.contains("ZIP") {
        return Some(Method::Deflate);
    }
    if format.contains("TAR") || format.contains("USTAR") || format.contains("PAX") {
        return Some(match filter.as_str() {
            "gzip" => Method::Gzip,
            "bzip2" => Method::Bzip2,
            "xz" | "lzma" => Method::Xz,
            "zstd" => Method::Zstd,
            "lz4" => Method::Lz4,
            "none" | "" => Method::Store,
            // A tar under a filter INDIUM does not write — lzip, lzop, lrzip, compress.
            // Rebuilding would silently change the filter, so refuse instead.
            _ => return None,
        });
    }
    None
}

/// The live sentence at the foot of the New Archive popup.
///
/// CORE §4.1: "a live sentence states exactly what will be built: *"Building
/// photos-2026.7z — 7z, LZMA2:19, AES-256."*"
pub fn recipe_sentence(recipe: &Recipe) -> String {
    let name = recipe
        .path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "the archive".to_string());

    let container = match recipe.container() {
        Container::Tar => "tar",
        Container::Zip => "zip",
        Container::SevenZ => "7z",
    };

    let method = match recipe.method.levels() {
        None => recipe.method.label().to_string(),
        Some(_) => format!("{}:{}", recipe.method.label(), recipe.level),
    };

    if recipe.encrypt {
        format!("Building {name} — {container}, {method}, AES-256.")
    } else {
        format!("Building {name} — {container}, {method}.")
    }
}

// ---------------------------------------------------------------------------
// The queue
// ---------------------------------------------------------------------------

/// One staged mutation. CORE §3 names exactly these four.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Task {
    /// A path from the filesystem, landing at `dest` inside the archive. A directory
    /// expands into its members at Apply time, not here — expansion touches the disk.
    Add { source: PathBuf, dest: String },
    /// A staged path. A directory takes its subtree with it.
    Remove { path: String },
    /// The last component only; see P4 §5 for why.
    Rename { from: String, to: String },
    /// Only ever first, and only ever one.
    Create { recipe: Recipe },
}

impl Task {
    /// The verb shown in the tray summary and the `W` popup.
    pub fn verb(&self) -> &'static str {
        match self {
            Task::Add { .. } => "Add",
            Task::Remove { .. } => "Remove",
            Task::Rename { .. } => "Rename",
            Task::Create { .. } => "Create",
        }
    }

    /// The one-line description of this task, for the tray and the task list.
    pub fn summary(&self) -> String {
        match self {
            Task::Add { source, dest } => {
                let name = source
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| source.to_string_lossy().to_string());
                if dest.is_empty() {
                    format!("Add {name}")
                } else {
                    format!("Add {name} as {dest}")
                }
            }
            Task::Remove { path } => format!("Remove {path}"),
            Task::Rename { from, to } => format!("Rename {from} to {to}"),
            Task::Create { recipe } => format!(
                "Create {}",
                recipe
                    .path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default()
            ),
        }
    }
}

/// The ordered list of staged changes. This is what the `W` popup shows and edits.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Queue {
    tasks: Vec<Task>,
}

impl Queue {
    pub fn new() -> Queue {
        Queue::default()
    }

    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }

    pub fn len(&self) -> usize {
        self.tasks.len()
    }

    pub fn tasks(&self) -> &[Task] {
        &self.tasks
    }

    pub fn push(&mut self, task: Task) {
        self.tasks.push(task);
    }

    pub fn remove(&mut self, index: usize) -> Option<Task> {
        if index < self.tasks.len() {
            Some(self.tasks.remove(index))
        } else {
            None
        }
    }

    pub fn clear(&mut self) {
        self.tasks.clear();
    }

    /// The recipe staged by a `Create`, if this queue creates an archive.
    pub fn creation(&self) -> Option<&Recipe> {
        self.tasks.iter().find_map(|t| match t {
            Task::Create { recipe } => Some(recipe),
            _ => None,
        })
    }

    /// Does Apply need a password typed twice? Only a fresh encrypted archive does:
    /// there is nothing to check a typo against, and a typo would build something
    /// nobody can open.
    pub fn creates_encrypted(&self) -> bool {
        self.creation().map(|r| r.encrypt).unwrap_or(false)
    }

    /// CORE §4: the tray shows "count, a summary of the first tasks".
    pub fn tray_summary(&self) -> String {
        match self.tasks.len() {
            0 => String::new(),
            1 => format!("1 change — {}", self.tasks[0].summary().to_lowercase()),
            n => {
                let first = self.tasks[0].summary().to_lowercase();
                format!("{n} changes — {first}, …")
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The plan
// ---------------------------------------------------------------------------

/// What becomes of one member of the source archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Disposition {
    /// Written to the new archive under `out_path`, with `hardlink` already retargeted.
    Keep {
        out_path: String,
        hardlink: Option<String>,
    },
    /// Not written.
    Drop,
}

/// One new member, from the filesystem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddItem {
    pub source: PathBuf,
    pub out_path: String,
}

/// The folded queue: exactly what Apply will write, and in what order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// One entry per member of the source listing, **in listing order**.
    ///
    /// Indexed by position rather than keyed by path on purpose: a tar may legally hold
    /// two members with the same stored name, and a path-keyed lookup breaks on that
    /// silently. Apply walks the source and this vector in step.
    pub source: Vec<Disposition>,
    /// New members, appended after everything kept.
    pub adds: Vec<AddItem>,
}

impl Plan {
    /// How many members the new archive will hold.
    pub fn out_count(&self) -> usize {
        self.source
            .iter()
            .filter(|d| matches!(d, Disposition::Keep { .. }))
            .count()
            + self.adds.len()
    }

    /// Does this plan change anything at all?
    pub fn is_identity(&self, source: &[Entry]) -> bool {
        self.adds.is_empty()
            && self.source.len() == source.len()
            && self.source.iter().zip(source).all(|(d, e)| match d {
                Disposition::Keep { out_path, hardlink } => {
                    *out_path == e.raw_path && *hardlink == e.hardlink
                }
                Disposition::Drop => false,
            })
    }
}

/// Why a queue cannot be applied.
///
/// The `Display` strings are the exact sentences the window shows, following
/// `ArchiveError`'s precedent — the user-facing wording lives with the rule it reports,
/// not scattered through the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Conflict {
    NoSuchPath(String),
    NameTaken(String),
    UnsafeName(String),
    HardlinkTargetRemoved { link: String, target: String },
    EncryptedSourceCannotBeRewritten,
    FormatCannotBeWritten(String),
    NothingToCreateInto,
}

impl std::fmt::Display for Conflict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Conflict::NoSuchPath(p) => {
                write!(f, "{p} is no longer in this archive.")
            }
            Conflict::NameTaken(p) => {
                write!(f, "{p} already exists in this archive.")
            }
            Conflict::UnsafeName(p) => {
                write!(f, "Refused: {p} is not a name an archive member may have.")
            }
            Conflict::HardlinkTargetRemoved { link, target } => write!(
                f,
                "{link} is a hard link to {target}; remove them together."
            ),
            Conflict::EncryptedSourceCannotBeRewritten => write!(
                f,
                "This archive has encrypted entries, and INDIUM does not write encrypted \
                 zip. Rebuilding it would store them unprotected, so nothing can be \
                 staged here."
            ),
            Conflict::FormatCannotBeWritten(name) => write!(
                f,
                "INDIUM reads {name} but does not write it. Nothing can be staged here."
            ),
            Conflict::NothingToCreateInto => {
                write!(f, "A new archive cannot be built over an existing listing.")
            }
        }
    }
}

/// Is this a name an archive member may have?
///
/// Reuses `arch::path_escapes`, which is the same judgement extraction makes, so a name
/// that could never be safely extracted can never be staged either. Empty names and
/// bare directory markers are refused here as well.
fn name_is_safe(path: &str) -> bool {
    !path.is_empty() && !path_escapes(path) && !normalize_archive_path(path).is_empty()
}

/// Fold the queue into a plan.
///
/// Pure: no archive is opened, no file is touched, nothing is allocated outside the
/// result. Every rule in P4 §1 lives here, which is why the tests below can prove the
/// staging semantics without a filesystem.
pub fn plan(source: &[Entry], tasks: &[Task], adds: &[AddItem]) -> Result<Plan, Conflict> {
    // `staged[i]` is the normalised path member `i` currently answers to, or `None`
    // once it has been dropped.
    let mut staged: Vec<Option<String>> = source.iter().map(|e| Some(e.path.clone())).collect();
    let mut pending: Vec<AddItem> = Vec::new();

    for task in tasks {
        match task {
            Task::Create { .. } => {
                if !source.is_empty() {
                    return Err(Conflict::NothingToCreateInto);
                }
            }

            Task::Remove { path } => {
                let target = normalize_archive_path(path);
                let mut hit = false;
                for slot in staged.iter_mut() {
                    if let Some(p) = slot.as_ref() {
                        if p == &target || is_under(p, &target) {
                            *slot = None;
                            hit = true;
                        }
                    }
                }
                // An add staged at or beneath a removed path is cancelled: staging an
                // add and then removing that path must leave it gone, which is the only
                // intuitive reading.
                let before = pending.len();
                pending.retain(|a| a.out_path != target && !is_under(&a.out_path, &target));
                if !hit && pending.len() == before {
                    return Err(Conflict::NoSuchPath(target));
                }
            }

            Task::Rename { from, to } => {
                let from_n = normalize_archive_path(from);
                let to_n = normalize_archive_path(to);
                if !name_is_safe(&to_n) {
                    return Err(Conflict::UnsafeName(to.clone()));
                }
                if from_n == to_n {
                    continue;
                }
                let Some(index) = staged.iter().position(|s| s.as_deref() == Some(&*from_n)) else {
                    return Err(Conflict::NoSuchPath(from_n));
                };
                let taken = staged
                    .iter()
                    .enumerate()
                    .any(|(i, s)| i != index && s.as_deref() == Some(&*to_n))
                    || pending.iter().any(|a| a.out_path == to_n);
                if taken {
                    return Err(Conflict::NameTaken(to_n));
                }

                // A directory takes its children with it: rewriting only the directory
                // entry would leave every child naming a parent that no longer exists.
                for slot in staged.iter_mut() {
                    if let Some(p) = slot.as_ref() {
                        if is_under(p, &from_n) {
                            let rest = &p[from_n.len()..];
                            *slot = Some(format!("{to_n}{rest}"));
                        }
                    }
                }
                staged[index] = Some(to_n);
            }

            Task::Add { source: src, dest } => {
                let dest_n = normalize_archive_path(dest);
                if !name_is_safe(&dest_n) {
                    return Err(Conflict::UnsafeName(dest.clone()));
                }
                // An add over an existing name replaces it — what dropping a file onto
                // an archive means everywhere else.
                for slot in staged.iter_mut() {
                    if slot.as_deref() == Some(&*dest_n) {
                        *slot = None;
                    }
                }
                pending.retain(|a| a.out_path != dest_n);
                pending.push(AddItem {
                    source: src.clone(),
                    out_path: dest_n,
                });
            }
        }
    }

    // Adds discovered by expanding a directory arrive already resolved; they replace
    // survivors by the same rule.
    for item in adds {
        if !name_is_safe(&item.out_path) {
            return Err(Conflict::UnsafeName(item.out_path.clone()));
        }
        for slot in staged.iter_mut() {
            if slot.as_deref() == Some(&*item.out_path) {
                *slot = None;
            }
        }
        pending.retain(|a| a.out_path != item.out_path);
        pending.push(item.clone());
    }

    // Hardlinks name a member written earlier in the same archive, so a rename must
    // retarget every link that pointed at the old name, and removing a target while a
    // link survives is refused: the data lives with the target, and the surviving link
    // would extract broken.
    let mut moved: BTreeMap<String, String> = BTreeMap::new();
    let mut gone: BTreeSet<String> = BTreeSet::new();
    for (i, entry) in source.iter().enumerate() {
        match staged[i].as_ref() {
            Some(now) if now != &entry.path => {
                moved.insert(entry.path.clone(), now.clone());
            }
            None => {
                gone.insert(entry.path.clone());
            }
            _ => {}
        }
    }

    let mut out: Vec<Disposition> = Vec::with_capacity(source.len());
    for (i, entry) in source.iter().enumerate() {
        let Some(now) = staged[i].clone() else {
            out.push(Disposition::Drop);
            continue;
        };

        let hardlink = match entry.hardlink.as_ref() {
            None => None,
            Some(target) => {
                let target_n = normalize_archive_path(target);
                if gone.contains(&target_n) {
                    return Err(Conflict::HardlinkTargetRemoved {
                        link: now,
                        target: target_n,
                    });
                }
                Some(moved.get(&target_n).cloned().unwrap_or(target_n))
            }
        };

        out.push(Disposition::Keep {
            out_path: out_path_for(entry, &now),
            hardlink,
        });
    }

    Ok(Plan {
        source: out,
        adds: pending,
    })
}

/// Is `path` inside directory `dir`?
///
/// The `/` boundary check is what stops `photos2` from being treated as a child of
/// `photos`. Mirrors `arch::selection_matches`, which makes the same distinction.
fn is_under(path: &str, dir: &str) -> bool {
    !dir.is_empty()
        && path.len() > dir.len()
        && path.starts_with(dir)
        && path.as_bytes()[dir.len()] == b'/'
}

/// The name a kept member is written under.
///
/// An unrenamed member keeps its **stored** name byte for byte — normalising it on the
/// way through would quietly rewrite archives that Apply was only asked to copy. A
/// renamed directory keeps its trailing slash, or the rebuild writes a file where a
/// directory was.
fn out_path_for(entry: &Entry, staged: &str) -> String {
    if staged == entry.path {
        return entry.raw_path.clone();
    }
    if entry.raw_path.ends_with('/') || entry.is_dir {
        format!("{staged}/")
    } else {
        staged.to_string()
    }
}

// ---------------------------------------------------------------------------
// What a backend is asked to write — P4 §3
// ---------------------------------------------------------------------------

/// One member, as handed to a writer.
///
/// This is `Entry` minus everything only a reader can know — no packed size, no stored
/// method, no encryption flag. Kept and added members reach the rebuild loop in the same
/// shape, which is what lets that loop stay four lines long.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Meta {
    pub out_path: String,
    pub size: u64,
    pub is_dir: bool,
    pub mode: u32,
    pub mtime: Option<i64>,
    pub atime: Option<i64>,
    pub ctime: Option<i64>,
    pub uid: i64,
    pub gid: i64,
    pub uname: Option<String>,
    pub gname: Option<String>,
    pub symlink: Option<String>,
    pub hardlink: Option<String>,
}

impl Meta {
    /// A member copied out of the source archive, under the name the plan gave it and
    /// with its hardlink already retargeted.
    pub fn from_entry(entry: &Entry, out_path: &str, hardlink: Option<&str>) -> Meta {
        Meta {
            out_path: out_path.to_string(),
            size: entry.size,
            is_dir: entry.is_dir,
            mode: entry.mode,
            mtime: entry.mtime,
            atime: entry.atime,
            ctime: entry.ctime,
            uid: entry.uid,
            gid: entry.gid,
            uname: entry.uname.clone(),
            gname: entry.gname.clone(),
            symlink: entry.symlink.clone(),
            hardlink: hardlink.map(|h| h.to_string()),
        }
    }

    /// Does this member carry a data stream?
    pub fn has_data(&self) -> bool {
        !self.is_dir && self.symlink.is_none() && self.hardlink.is_none() && self.size > 0
    }
}

/// What Apply writes into. One of these per container.
///
/// Two implementations: `arch::Writer` over libarchive for tar and zip, and
/// `sevenz::Writer` over `sevenz-rust2` for 7z, which is the only one that can write
/// AES-256. Apply never learns which it holds.
pub trait Sink {
    /// Write one member. `data` is `None` for a directory, a symlink, a hardlink, or an
    /// empty file.
    fn put(&mut self, meta: &Meta, data: Option<&mut dyn std::io::Read>) -> Result<(), String>;

    /// Close the archive out. Errors here are as fatal as errors anywhere else — a
    /// half-flushed archive must never be renamed over a good one.
    fn finish(&mut self) -> Result<(), String>;

    /// Abandon the build without flushing. Called on cancel and on failure, before the
    /// temp file is removed.
    fn abandon(&mut self);
}

// ---------------------------------------------------------------------------
// Verification — P4 §2 step 6
// ---------------------------------------------------------------------------

/// What the rebuilt archive must contain for Apply to commit it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expected {
    /// Normalised paths, as a multiset — order is not compared, because INDIUM does not
    /// promise one and the table sorts anyway.
    pub paths: Vec<String>,
    /// Sizes for regular files only; see `verify_against`.
    pub sizes: BTreeMap<String, u64>,
}

impl Plan {
    /// The shape the rebuilt archive must have.
    pub fn expected(&self, source: &[Entry], added: &[(String, u64)]) -> Expected {
        let mut paths = Vec::new();
        let mut sizes = BTreeMap::new();

        for (i, disposition) in self.source.iter().enumerate() {
            let Disposition::Keep { out_path, .. } = disposition else {
                continue;
            };
            let normalised = normalize_archive_path(out_path);
            paths.push(normalised.clone());
            if let Some(entry) = source.get(i) {
                if is_regular_file(entry) {
                    sizes.insert(normalised, entry.size);
                }
            }
        }
        for (path, size) in added {
            let normalised = normalize_archive_path(path);
            paths.push(normalised.clone());
            sizes.insert(normalised, *size);
        }

        Expected { paths, sizes }
    }
}

/// A member whose size is worth comparing.
///
/// Directories have no data; a tar hardlink entry carries size 0 because the bytes live
/// with its target (the fixtures README records this); and a zip symlink stores its
/// target string as data, so its size is not the size of anything. Comparing those
/// would fail `meta.tar` for reasons that are not corruption.
fn is_regular_file(entry: &Entry) -> bool {
    !entry.is_dir && entry.symlink.is_none() && entry.hardlink.is_none()
}

/// Walk the rebuilt archive's entries against what the plan promised.
///
/// P4 §2: "verification proves the new archive is structurally complete and names every
/// member it should, at the size it should. It does not re-checksum the data; the
/// compressor's own integrity check does that on extraction." Stated as a reading of
/// CORE §3, and returning the exact sentence shown on failure.
pub fn verify_against(expected: &Expected, built: &[Entry]) -> Result<(), String> {
    let mut want: BTreeMap<&str, usize> = BTreeMap::new();
    for p in &expected.paths {
        *want.entry(p.as_str()).or_insert(0) += 1;
    }
    let mut have: BTreeMap<&str, usize> = BTreeMap::new();
    for e in built {
        *have.entry(e.path.as_str()).or_insert(0) += 1;
    }

    for (path, n) in &want {
        let got = have.get(path).copied().unwrap_or(0);
        if got < *n {
            return Err(format!(
                "the rebuilt archive is missing {path} — it was not written, so nothing was replaced."
            ));
        }
    }
    for (path, n) in &have {
        let wanted = want.get(path).copied().unwrap_or(0);
        if *n > wanted {
            return Err(format!(
                "the rebuilt archive holds {path}, which was not planned — nothing was replaced."
            ));
        }
    }

    for entry in built {
        if let Some(size) = expected.sizes.get(&entry.path) {
            if is_regular_file(entry) && entry.size != *size {
                return Err(format!(
                    "{} was written at {} bytes instead of {} — nothing was replaced.",
                    entry.path, entry.size, size
                ));
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// The temp file beside the target — P4 §2
// ---------------------------------------------------------------------------

/// The temp file Apply builds into, beside the target.
///
/// Deterministic on purpose: that determinism **is** the crash-orphan policy. A crashed
/// Apply leaves exactly one leftover per archive rather than an accumulating pile, and
/// the next Apply on that archive clears it. INDIUM sweeps no directory it was not
/// pointed at, and goes looking through nobody's disk for its own litter.
pub fn temp_path_for(target: &Path) -> PathBuf {
    let name = target
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_else(|| OsString::from("archive"));
    let mut temp = OsString::from(".");
    temp.push(&name);
    temp.push(".indium-new");
    match target.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir.join(temp),
        _ => PathBuf::from(temp),
    }
}

/// Is this file one of ours, and therefore ours to delete?
///
/// Strict, in the manner of `platform::scratch::is_ours`: nothing is ever removed on a
/// loose match. A file that merely resembles ours is not ours.
pub fn is_our_temp(name: &str) -> bool {
    name.starts_with('.') && name.ends_with(".indium-new") && name.len() > ".indium-new".len() + 1
}

// ---------------------------------------------------------------------------
// Metadata the target format cannot carry — P4 §3
// ---------------------------------------------------------------------------

/// What a rebuild into `container` will throw away, said before Apply rather than
/// discovered afterwards.
///
/// "The metadata is the main event" (CORE §1) has to mean telling the user when
/// metadata is about to die. Each sentence fires only when the loss is real, so a
/// rebuild that keeps its format stays quiet.
pub fn metadata_losses(container: Container, entries: &[Entry]) -> Vec<String> {
    let mut out = Vec::new();
    if container == Container::Tar {
        return out;
    }

    if let Some(entry) = entries
        .iter()
        .find(|e| e.uname.is_some() || e.gname.is_some())
    {
        let owner = format!(
            "{}:{}",
            entry.uname.as_deref().unwrap_or("?"),
            entry.gname.as_deref().unwrap_or("?")
        );
        out.push(format!(
            "{} does not store owner names — {owner} will not survive.",
            container_label(container)
        ));
    }
    if entries.iter().any(|e| e.hardlink.is_some()) {
        out.push(format!(
            "{} does not store hard links — they will be written as separate files.",
            container_label(container)
        ));
    }
    if container == Container::SevenZ && entries.iter().any(|e| e.symlink.is_some()) {
        out.push("7z does not store symbolic links in this version.".to_string());
    }
    out
}

fn container_label(container: Container) -> &'static str {
    match container {
        Container::Tar => "tar",
        Container::Zip => "zip",
        Container::SevenZ => "7z",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal entry. Only the fields a fold looks at need to be right.
    fn entry(path: &str, is_dir: bool) -> Entry {
        let raw = if is_dir {
            format!("{path}/")
        } else {
            path.to_string()
        };
        Entry {
            raw_path: raw,
            path: path.to_string(),
            is_dir,
            size: 10,
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
            filetype: 0,
            symlink: None,
            hardlink: None,
            encrypted: false,
        }
    }

    fn tree() -> Vec<Entry> {
        vec![
            entry("alpha.txt", false),
            entry("sub", true),
            entry("sub/beta.txt", false),
            entry("sub/gamma.txt", false),
        ]
    }

    fn kept(plan: &Plan) -> Vec<String> {
        plan.source
            .iter()
            .filter_map(|d| match d {
                Disposition::Keep { out_path, .. } => Some(out_path.clone()),
                Disposition::Drop => None,
            })
            .collect()
    }

    /// CORE §3: "The original is never touched until the replacement is proven." The
    /// weakest possible Apply — one with nothing staged — must still reproduce the
    /// archive exactly, or every stronger claim rests on nothing.
    #[test]
    fn a_plan_over_an_empty_queue_is_the_identity() {
        let source = tree();
        let plan = plan(&source, &[], &[]).unwrap();
        assert!(plan.is_identity(&source));
        assert_eq!(
            kept(&plan),
            vec!["alpha.txt", "sub/", "sub/beta.txt", "sub/gamma.txt"],
            "an untouched member must keep its stored name byte for byte"
        );
    }

    /// P4 §1: "A remove takes a directory's whole subtree."
    #[test]
    fn removing_a_directory_drops_everything_beneath_it() {
        let source = tree();
        let tasks = [Task::Remove {
            path: "sub".to_string(),
        }];
        let plan = plan(&source, &tasks, &[]).unwrap();
        assert_eq!(kept(&plan), vec!["alpha.txt"]);
    }

    /// A sibling whose name merely starts with the same letters is not a child.
    #[test]
    fn a_name_that_merely_starts_the_same_is_not_a_child() {
        let source = vec![entry("sub", true), entry("subtitle.txt", false)];
        let tasks = [Task::Remove {
            path: "sub".to_string(),
        }];
        let plan = plan(&source, &tasks, &[]).unwrap();
        assert_eq!(kept(&plan), vec!["subtitle.txt"]);
    }

    /// P4 §1: "A rename rewrites the path prefix of every live descendant."
    #[test]
    fn renaming_a_directory_rewrites_every_child_path() {
        let source = tree();
        let tasks = [Task::Rename {
            from: "sub".to_string(),
            to: "docs".to_string(),
        }];
        let plan = plan(&source, &tasks, &[]).unwrap();
        assert_eq!(
            kept(&plan),
            vec!["alpha.txt", "docs/", "docs/beta.txt", "docs/gamma.txt"]
        );
    }

    /// P4 §1: "preserves a directory's trailing `/` — write a file where a directory
    /// was and the archive is wrong."
    #[test]
    fn a_renamed_directory_keeps_its_trailing_slash() {
        let source = vec![entry("sub", true)];
        let tasks = [Task::Rename {
            from: "sub".to_string(),
            to: "docs".to_string(),
        }];
        let plan = plan(&source, &tasks, &[]).unwrap();
        assert_eq!(kept(&plan), vec!["docs/"]);
    }

    /// Tasks key on the *staged* path, so a second rename names what the first
    /// produced and the two compose without a special case.
    #[test]
    fn two_renames_of_one_path_compose_into_one() {
        let source = vec![entry("a.txt", false)];
        let tasks = [
            Task::Rename {
                from: "a.txt".to_string(),
                to: "b.txt".to_string(),
            },
            Task::Rename {
                from: "b.txt".to_string(),
                to: "c.txt".to_string(),
            },
        ];
        let plan = plan(&source, &tasks, &[]).unwrap();
        assert_eq!(kept(&plan), vec!["c.txt"]);
    }

    /// P4 §1: "An add whose name collides with a survivor replaces it."
    #[test]
    fn an_add_whose_name_already_exists_replaces_that_entry() {
        let source = vec![entry("alpha.txt", false)];
        let tasks = [Task::Add {
            source: PathBuf::from("/tmp/alpha.txt"),
            dest: "alpha.txt".to_string(),
        }];
        let plan = plan(&source, &tasks, &[]).unwrap();
        assert!(kept(&plan).is_empty(), "the old member must be dropped");
        assert_eq!(plan.adds.len(), 1);
        assert_eq!(plan.out_count(), 1, "replaced, not duplicated");
    }

    /// P4 §1: "Add-then-remove leaves the path gone, which is the only intuitive
    /// reading."
    #[test]
    fn an_add_then_a_remove_of_that_path_leaves_it_gone() {
        let source = vec![entry("alpha.txt", false)];
        let tasks = [
            Task::Add {
                source: PathBuf::from("/tmp/new.txt"),
                dest: "new.txt".to_string(),
            },
            Task::Remove {
                path: "new.txt".to_string(),
            },
        ];
        let plan = plan(&source, &tasks, &[]).unwrap();
        assert!(plan.adds.is_empty(), "the cancelled add must not survive");
        assert_eq!(kept(&plan), vec!["alpha.txt"]);
    }

    /// P4 §1: "renaming a target must retarget every link that pointed at it."
    #[test]
    fn a_rename_retargets_every_hardlink_that_pointed_at_it() {
        let mut source = vec![entry("regular.txt", false), entry("link.txt", false)];
        source[1].hardlink = Some("regular.txt".to_string());
        let tasks = [Task::Rename {
            from: "regular.txt".to_string(),
            to: "main.txt".to_string(),
        }];
        let plan = plan(&source, &tasks, &[]).unwrap();
        match &plan.source[1] {
            Disposition::Keep { hardlink, .. } => assert_eq!(
                hardlink.as_deref(),
                Some("main.txt"),
                "a link naming a member that no longer exists would extract broken"
            ),
            Disposition::Drop => panic!("the link must survive"),
        }
    }

    /// P4 §1: "removing a target while a link survives is refused, because the data
    /// lives with the target."
    #[test]
    fn removing_a_hardlink_target_while_a_link_survives_is_refused() {
        let mut source = vec![entry("regular.txt", false), entry("link.txt", false)];
        source[1].hardlink = Some("regular.txt".to_string());
        let tasks = [Task::Remove {
            path: "regular.txt".to_string(),
        }];
        let err = plan(&source, &tasks, &[]).unwrap_err();
        assert_eq!(
            err,
            Conflict::HardlinkTargetRemoved {
                link: "link.txt".to_string(),
                target: "regular.txt".to_string(),
            }
        );
    }

    /// P4 §1: "a tar may legally hold two members with the same stored name, and a
    /// path-keyed lookup breaks on that silently."
    #[test]
    fn duplicate_stored_names_are_dispositioned_independently() {
        let source = vec![
            entry("dup.txt", false),
            entry("dup.txt", false),
            entry("other.txt", false),
        ];
        let plan = plan(&source, &[], &[]).unwrap();
        assert_eq!(plan.source.len(), 3, "one slot per member, not per name");
        assert_eq!(kept(&plan).len(), 3);
    }

    #[test]
    fn a_rename_onto_an_existing_name_is_refused() {
        let source = vec![entry("a.txt", false), entry("b.txt", false)];
        let tasks = [Task::Rename {
            from: "a.txt".to_string(),
            to: "b.txt".to_string(),
        }];
        assert_eq!(
            plan(&source, &tasks, &[]).unwrap_err(),
            Conflict::NameTaken("b.txt".to_string())
        );
    }

    #[test]
    fn a_task_naming_a_path_that_is_gone_is_refused() {
        let source = tree();
        let tasks = [Task::Remove {
            path: "nowhere.txt".to_string(),
        }];
        assert_eq!(
            plan(&source, &tasks, &[]).unwrap_err(),
            Conflict::NoSuchPath("nowhere.txt".to_string())
        );
    }

    /// The same judgement extraction makes: a name that could never be safely
    /// extracted can never be staged either.
    #[test]
    fn an_add_of_an_unsafe_path_is_refused() {
        let source = vec![entry("a.txt", false)];
        for bad in ["../escape.txt", "/etc/passwd", "sub/../../out.txt", ""] {
            let tasks = [Task::Add {
                source: PathBuf::from("/tmp/x"),
                dest: bad.to_string(),
            }];
            assert!(
                matches!(plan(&source, &tasks, &[]), Err(Conflict::UnsafeName(_))),
                "{bad} must be refused"
            );
        }
    }

    /// P4 §2: verification "compares sizes for regular files only, because a tar
    /// hardlink has size zero and a zip symlink stores its target as data".
    #[test]
    fn verification_compares_sizes_only_for_regular_files() {
        let mut built = vec![entry("regular.txt", false), entry("link.txt", false)];
        built[1].hardlink = Some("regular.txt".to_string());
        built[1].size = 0;

        let expected = Expected {
            paths: vec!["regular.txt".to_string(), "link.txt".to_string()],
            sizes: BTreeMap::from([("regular.txt".to_string(), 10)]),
        };
        assert!(verify_against(&expected, &built).is_ok());
    }

    #[test]
    fn verification_names_a_missing_an_extra_and_a_resized_member() {
        let expected = Expected {
            paths: vec!["a.txt".to_string(), "b.txt".to_string()],
            sizes: BTreeMap::from([("a.txt".to_string(), 10), ("b.txt".to_string(), 10)]),
        };

        let missing = vec![entry("a.txt", false)];
        assert!(verify_against(&expected, &missing)
            .unwrap_err()
            .contains("missing b.txt"));

        let extra = vec![
            entry("a.txt", false),
            entry("b.txt", false),
            entry("c.txt", false),
        ];
        assert!(verify_against(&expected, &extra)
            .unwrap_err()
            .contains("c.txt"));

        let mut resized = vec![entry("a.txt", false), entry("b.txt", false)];
        resized[1].size = 99;
        assert!(verify_against(&expected, &resized)
            .unwrap_err()
            .contains("99 bytes instead of 10"));
    }

    /// CORE §5's verdicts ship in the New Archive popup. Pinned against a second
    /// hand-copy so an edit to one and not the other cannot pass unnoticed.
    #[test]
    fn every_method_verdict_is_core_section_five_verbatim() {
        let core: [(Method, &str); 8] = [
            (
                Method::Store,
                "No compression — instant, and as large as the input.",
            ),
            (
                Method::Lz4,
                "The fastest real compression there is, and the largest result.",
            ),
            (
                Method::Gzip,
                "Fast, everywhere, and beaten in both speed and size by zstd.",
            ),
            (
                Method::Zstd,
                "Very fast with a small archive — the sane default.",
            ),
            (
                Method::Bzip2,
                "Slower than gzip for a somewhat smaller file; kept for compatibility.",
            ),
            (
                Method::Xz,
                "Among the smallest archives, built slowly; extraction is quick enough.",
            ),
            (
                Method::Lzma2,
                "Smallest for mixed content, slow to build — and the only road to AES-256.",
            ),
            (
                Method::Deflate,
                "Not the smallest or fastest, but opens absolutely anywhere.",
            ),
        ];
        for (method, sentence) in core {
            assert_eq!(method.verdict(), sentence, "{}", method.label());
        }
        assert_eq!(METHODS.len(), 8, "CORE §5 lists eight methods");
    }

    /// CORE §4.1: "a live sentence states exactly what will be built".
    #[test]
    fn the_footer_sentence_reads_exactly_as_core_writes_it() {
        let recipe = Recipe {
            path: PathBuf::from("/home/megas/photos-2026.7z"),
            method: Method::Lzma2,
            level: 9,
            encrypt: true,
        };
        assert_eq!(
            recipe_sentence(&recipe),
            "Building photos-2026.7z — 7z, LZMA2:9, AES-256."
        );

        let plain = Recipe {
            path: PathBuf::from("backup.tar.zst"),
            method: Method::Zstd,
            level: 3,
            encrypt: false,
        };
        assert_eq!(
            recipe_sentence(&plain),
            "Building backup.tar.zst — tar, zstd:3."
        );

        let stored = Recipe {
            path: PathBuf::from("plain.tar"),
            method: Method::Store,
            level: 0,
            encrypt: false,
        };
        assert_eq!(
            recipe_sentence(&stored),
            "Building plain.tar — tar, Store.",
            "a method with no levels must not print one"
        );
    }

    /// CORE §9: "No zip encryption — 7z AES-256 is the only encryption."
    #[test]
    fn only_seven_zip_may_be_encrypted() {
        for method in METHODS {
            let recipe = Recipe {
                path: PathBuf::from("x"),
                method,
                level: method.default_level(),
                encrypt: true,
            };
            assert_eq!(
                recipe.encryption_is_legal(),
                method == Method::Lzma2,
                "{} must not offer encryption",
                method.label()
            );
        }
    }

    #[test]
    fn a_level_outside_a_methods_range_is_clamped_rather_than_passed_on() {
        assert_eq!(Method::Bzip2.clamp_level(0), 1, "bzip2 starts at 1");
        assert_eq!(Method::Zstd.clamp_level(99), 22, "zstd tops out at 22");
        assert_eq!(Method::Gzip.clamp_level(4), 4);
        assert_eq!(Method::Store.clamp_level(7), 0, "Store has no level");
    }

    /// P4 §1: a format INDIUM reads but CORE §5 does not write cannot be staged
    /// against.
    #[test]
    fn a_recipe_is_derived_only_for_formats_core_writes() {
        let cases = [
            ("POSIX ustar format", "gzip", Some(Method::Gzip)),
            ("POSIX ustar format", "zstd", Some(Method::Zstd)),
            ("POSIX ustar format", "none", Some(Method::Store)),
            ("ZIP 2.0 (deflation)", "none", Some(Method::Deflate)),
            ("7-Zip", "none", Some(Method::Lzma2)),
            ("POSIX cpio", "none", None),
            ("ISO9660", "none", None),
            ("POSIX ustar format", "lzip", None),
        ];
        for (format, filter, want) in cases {
            let info = ArchiveInfo {
                format: format.to_string(),
                filter: filter.to_string(),
            };
            let got = Recipe::from_info(&info, Path::new("/tmp/a"), false).map(|r| r.method);
            assert_eq!(got, want, "{format} / {filter}");
        }
    }

    /// P4 §2: the temp file sits beside the target, and nothing else is ever ours.
    #[test]
    fn the_temp_path_sits_beside_the_target_and_is_recognisably_ours() {
        let temp = temp_path_for(Path::new("/home/megas/photos.tar.gz"));
        assert_eq!(
            temp,
            PathBuf::from("/home/megas/.photos.tar.gz.indium-new"),
            "beside the target, not in a scratch directory"
        );
        assert!(is_our_temp(".photos.tar.gz.indium-new"));
    }

    #[test]
    fn a_file_that_is_not_ours_is_never_a_candidate_for_deletion() {
        for name in [
            "photos.tar.gz",
            ".indium-new",
            "indium-new",
            ".photos.tar.gz.indium-new.bak",
            "",
            ".hidden",
        ] {
            assert!(!is_our_temp(name), "{name} is not ours to delete");
        }
    }

    /// CORE §1: "the metadata is the main event" — which has to mean saying so before
    /// the metadata dies, not after.
    #[test]
    fn metadata_losses_name_what_the_target_format_cannot_carry() {
        let mut entries = vec![entry("a.txt", false), entry("link.txt", false)];
        entries[0].uname = Some("indiumuser".to_string());
        entries[0].gname = Some("indiumgroup".to_string());
        entries[1].hardlink = Some("a.txt".to_string());

        let zip = metadata_losses(Container::Zip, &entries);
        assert!(zip
            .iter()
            .any(|s| s.contains("owner names") && s.contains("indiumuser:indiumgroup")));
        assert!(zip.iter().any(|s| s.contains("hard links")));

        assert!(
            metadata_losses(Container::Tar, &entries).is_empty(),
            "a rebuild that keeps tar loses nothing, and must stay quiet"
        );
    }

    #[test]
    fn a_queue_summarises_itself_for_the_tray() {
        let mut queue = Queue::new();
        assert!(
            queue.is_empty(),
            "the tray is hidden until the first change"
        );
        queue.push(Task::Remove {
            path: "alpha.txt".to_string(),
        });
        assert!(queue.tray_summary().starts_with("1 change"));
        queue.push(Task::Remove {
            path: "beta.txt".to_string(),
        });
        assert!(queue.tray_summary().starts_with("2 changes"));
    }

    /// P4 §1: a fresh encrypted archive is the only case that needs the password twice.
    #[test]
    fn only_a_new_encrypted_archive_asks_for_the_password_twice() {
        let mut queue = Queue::new();
        assert!(!queue.creates_encrypted());
        queue.push(Task::Create {
            recipe: Recipe {
                path: PathBuf::from("/tmp/secret.7z"),
                method: Method::Lzma2,
                level: 6,
                encrypt: true,
            },
        });
        assert!(queue.creates_encrypted());
    }

    #[test]
    fn a_create_task_cannot_be_staged_over_an_existing_listing() {
        let source = tree();
        let tasks = [Task::Create {
            recipe: Recipe {
                path: PathBuf::from("/tmp/new.7z"),
                method: Method::Lzma2,
                level: 6,
                encrypt: false,
            },
        }];
        assert_eq!(
            plan(&source, &tasks, &[]).unwrap_err(),
            Conflict::NothingToCreateInto
        );
    }
}
