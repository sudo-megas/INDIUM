//! The staging engine — CORE §3's `tasks`, implemented by P4 §1.
//!
//! CORE §3: "Every mutation — add, remove, rename, create — is a task in a queue.
//! **Apply** builds the new archive in a temp file beside the target, verifies it by
//! walking its entries, then atomically renames over the original. The original is never
//! touched until the replacement is proven."
//!
//! Most of this file is pure: the queue you edit, the fold that turns it into a plan, and
//! the judgements that decide whether a rebuild is allowed at all. None of that opens an
//! archive or touches a disk, which is why nearly every rule below is directly testable.
//! `apply` at the foot is the one part that does, and it is deliberately the last thing
//! here — by the time it runs, every decision has already been made.
//!
//! Copyright © sudo-megas. GPL-3.0-only.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;

use crate::arch::{path_escapes, ArchiveInfo, Entry};
use crate::secret::Secret;
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

/// A container. CORE §5's write list is tar — plain or with five filters — zip, and 7z.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Container {
    Tar,
    Zip,
    SevenZ,
}

impl Method {
    /// CORE §5's verdict for this method, **verbatim**.
    ///
    /// CORE §5: "This copy ships in the Create popup, one honest sentence each,
    /// static in v1.x — the live estimator that measures *your* data on *your* CPU is
    /// V2.0." Typed here and read back out of `CORE.md` itself by
    /// `every_method_verdict_is_core_section_five_verbatim`, so an edit to the document
    /// that this window does not follow fails the build.
    ///
    /// That sentence used to claim as much and was not true: until P18 the test compared
    /// this `match` against a second hand-copy of the same eight sentences **in this file**,
    /// so editing CORE §5 left `cargo test` green. A comment describing a gate that does not
    /// exist is worse than no gate, because it stops anyone building one.
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
    /// `lz4:compression-level=0` outright. Every level in every range below — not merely
    /// both ends, which is all it was until P18 — has been offered to libarchive and
    /// accepted: `every_level_a_method_offers_is_one_libarchive_accepts` in
    /// `tests/write_path.rs` is what keeps that true, and it is the test that caught the
    /// lz4 case. LZMA2's range is `sevenz-rust2`'s, where an out-of-range level is clamped
    /// rather than refused, so it is checked by
    /// `every_lzma2_level_the_slider_offers_builds_a_7z_that_reads_back` instead — the
    /// libarchive test cannot make a claim about a method that never reaches libarchive.
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

/// The live sentence at the foot of the Create popup.
///
/// CORE §4.1: "a live sentence states exactly what will be built". The example the document
/// prints there is read back out of it by `the_footer_sentence_reads_exactly_as_core_writes_it`
/// rather than copied down here — this doc carried `LZMA2:19` from P4 to P22 for want of that,
/// and so did §4.1 itself.
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

    /// Drop any task that no longer folds against `source`.
    ///
    /// A queue is valid as a *sequence*, so removing one row can orphan a later one — a
    /// rename of a name an earlier task produced, an add beneath a directory that is no
    /// longer being created. Re-folding after every edit is what keeps the list the user
    /// sees identical to the list Apply would run, instead of letting the difference
    /// surface as a failure much later.
    pub fn retain_foldable(&mut self, source: &[Entry]) {
        let mut kept: Vec<Task> = Vec::with_capacity(self.tasks.len());
        for task in std::mem::take(&mut self.tasks) {
            let mut trial = kept.clone();
            trial.push(task.clone());
            if plan(source, &trial, &[]).is_ok() {
                kept.push(task);
            }
        }
        self.tasks = kept;
    }

    /// The recipe staged by a `Create`, if this queue creates an archive.
    ///
    /// `set_creation` stood beside this until P22, swapping a staged recipe in place so a
    /// second Create would not discard the files already added to it. [`Draft::project_onto`]
    /// replaces it: the queue is rebuilt from the draft on every press, which preserves
    /// `Task::Create`'s *"only ever first, and only ever one"* by construction instead of by
    /// careful mutation, and drops a file the draft has dropped instead of keeping it.
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
// The draft
// ---------------------------------------------------------------------------

/// One thing that will go into the next archive: a file on disk, and the name it takes
/// inside the archive.
///
/// The same pair `Task::Add` carries, and deliberately so — the projection below turns one
/// into the other and has nothing to invent on the way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftItem {
    pub source: PathBuf,
    pub dest: String,
}

/// What the next archive will be made of.
///
/// CORE §3: *"a plain list of what the next archive will be made of, holding no mutation of
/// anything, folding nothing, and becoming tasks only when Create is pressed. A queue
/// describes changes to one archive and folds toward it; a draft names a thing that does not
/// exist yet."* That is the whole reason this is not a second [`Queue`]: `plan` folds toward
/// one target, and *"rename inside photos.zip"* and *"build backup.7z out of these"* are two.
///
/// **The draft is the source of truth until Apply succeeds**, and the queue's creation lane
/// is a projection of it, recomputed by [`Draft::project_onto`] every time *Create* is
/// pressed. That is what leaves the P21 drift nowhere to live: a method chosen in Measure
/// used to write the popup's fields while the staged recipe kept the old one, because the
/// two were separate states that looked like one. There is now no state in the queue's adds
/// that the draft did not put there this press.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Draft {
    items: Vec<DraftItem>,
}

impl Draft {
    pub fn new() -> Draft {
        Draft::default()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn items(&self) -> &[DraftItem] {
        &self.items
    }

    /// Put one file in the draft, and report what it displaced.
    ///
    /// **One item per `dest`.** Two items sharing a name inside the archive is a silent
    /// loss: the fold below resolves an add over an existing name by replacing it, so the
    /// second would win at Apply and the first would vanish having never been refused.
    /// Replacing here makes the same outcome visible at the moment it is chosen, and the
    /// displaced item comes back so the caller can say so. The row keeps its position
    /// rather than moving to the end, because a list that reorders under a person's hand
    /// is a list they have to re-read.
    pub fn add(&mut self, source: PathBuf, dest: String) -> Option<DraftItem> {
        if let Some(i) = self.items.iter().position(|it| it.dest == dest) {
            return Some(std::mem::replace(
                &mut self.items[i],
                DraftItem { source, dest },
            ));
        }
        self.items.push(DraftItem { source, dest });
        None
    }

    pub fn remove(&mut self, index: usize) -> Option<DraftItem> {
        if index < self.items.len() {
            Some(self.items.remove(index))
        } else {
            None
        }
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// Rewrite `queue` as `recipe` plus this draft, and report what that displaced.
    ///
    /// This is the projection. It runs on every *Create* press rather than merging into
    /// what is already there, which is what makes the draft the source of truth: the queue
    /// cannot hold an add the draft has since dropped, or miss one it has since gained.
    ///
    /// What it displaces depends on what the queue was. A queue that already creates is
    /// this draft's own previous projection, and its adds are regenerated below rather than
    /// lost — nothing went. A queue that does not create is a set of mutations of the
    /// archive that is open, and every one of them goes: `plan` folds toward one target and
    /// these name another. The count comes back so the caller can say what went, which is
    /// the maker's ruling for both doors that lose staged work.
    ///
    /// `Task::Create` is documented *"Only ever first, and only ever one"*, and building the
    /// queue from empty is what makes that true by construction rather than by care.
    pub fn project_onto(&self, queue: &mut Queue, recipe: Recipe) -> usize {
        let displaced = if queue.creation().is_some() {
            0
        } else {
            queue.len()
        };
        queue.clear();
        queue.push(Task::Create { recipe });
        for item in &self.items {
            queue.push(Task::Add {
                source: item.source.clone(),
                dest: item.dest.clone(),
            });
        }
        displaced
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

/// A [`Read`](std::io::Read) that counts what passes through it, says so, and can refuse
/// to go on.
///
/// PXX, and it closes both halves of one defect. Progress and cancel were **both
/// per-member**: the only `tx.send` and the only `cancel.load` in the build loop sit
/// *between* members, which is exactly right for an archive of nine files and useless for
/// one of one. The certification walk built a single encrypted 7z and got a bar that never
/// moved off zero and a Cancel that did nothing for ten minutes. Neither run was stuck —
/// systemd's own accounting puts one at 68% of a core sustained over 10m13s and the other
/// at 106% over 2m25s — they were computing in silence until the maker gave up on them.
/// The button was drawn and the flag was set; nothing read the flag until the member
/// finished, and the member was the whole job.
///
/// Wrapping the reader is what puts a report and a check *inside* the member without
/// either writer having to know: both pump a member through a `Read` in a loop —
/// libarchive's own at 64 KiB, `sevenz-rust2`'s at 4 KiB — so the wrapper is called
/// hundreds of thousands of times over a large one, and every call is an opportunity for
/// both.
struct Metered<'a> {
    inner: &'a mut dyn std::io::Read,
    tx: &'a Sender<ApplyMsg>,
    cancel: &'a Arc<AtomicBool>,
    /// Bytes through, and the count the last message carried.
    done: u64,
    sent: u64,
    total: u64,
    /// How far `done` must move past `sent` before it is worth saying again.
    step: u64,
}

impl<'a> Metered<'a> {
    fn new(
        inner: &'a mut dyn std::io::Read,
        total: u64,
        tx: &'a Sender<ApplyMsg>,
        cancel: &'a Arc<AtomicBool>,
    ) -> Metered<'a> {
        Metered {
            inner,
            tx,
            cancel,
            done: 0,
            sent: 0,
            total,
            // A percentage point of the member, floored at a mebibyte. The floor is what
            // keeps a directory of small files quiet — under a MiB nothing is sent at all,
            // and the per-member `Progress` already covers those — and the percentage is
            // what stops a 4 GiB member sending a million messages down a channel the UI
            // drains once a frame. A hundred either way.
            step: (total / 100).max(1 << 20),
        }
    }
}

impl std::io::Read for Metered<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        // Before the read rather than after, so a cancel arriving while the last chunk of
        // a member is in flight stops the *next* member from starting rather than being
        // noticed a whole member later.
        if self.cancel.load(Ordering::Relaxed) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "cancelled",
            ));
        }
        let n = self.inner.read(buf)?;
        self.done += n as u64;
        if self.done - self.sent >= self.step {
            self.sent = self.done;
            let _ = self.tx.send(ApplyMsg::Within {
                done: self.done,
                total: self.total,
            });
        }
        Ok(n)
    }
}

/// [`Sink::put`], with a cancel told apart from a failure.
///
/// `Ok(false)` means the flag was set. A [`Metered`] refusing to read surfaces here as an
/// ordinary write error — the writers cannot tell a cancelled read from a disk going away
/// and should not have to, and neither can be identified by its message without matching
/// on error strings, which is worse than asking. So the flag is consulted before the error
/// is believed, and it is the flag rather than the error that decides. Getting this wrong
/// would report a cancel as *"could not write …"* in yellow, which is a failure the user
/// did not have.
fn put_member(
    sink: &mut dyn Sink,
    meta: &Meta,
    data: Option<&mut dyn std::io::Read>,
    cancel: &Arc<AtomicBool>,
) -> Result<bool, String> {
    match sink.put(meta, data) {
        Ok(()) => Ok(true),
        Err(e) if cancel.load(Ordering::Relaxed) => {
            let _ = e;
            sink.abandon();
            Ok(false)
        }
        Err(e) => Err(e),
    }
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
    ///
    /// It must account for what the *target* format cannot carry, or verification asks
    /// for a member the writer was never able to write. 7z stores no symlink and no
    /// hardlink, so a symlink rebuilt into a 7z is legitimately absent — expecting it
    /// would fail every such Apply, and the loss is the one `metadata_losses` already
    /// warns about before the button is pressed.
    pub fn expected(
        &self,
        source: &[Entry],
        added: &[(String, u64)],
        container: Container,
    ) -> Expected {
        let mut paths = Vec::new();
        let mut sizes = BTreeMap::new();

        for (i, disposition) in self.source.iter().enumerate() {
            let Disposition::Keep { out_path, .. } = disposition else {
                continue;
            };
            if let Some(entry) = source.get(i) {
                if !container_keeps(container, entry) {
                    continue;
                }
            }
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

/// Will this member exist at all in the rebuilt archive?
///
/// The mirror of `metadata_losses`: what that sentence warns about, this one accounts
/// for. 7z carries neither a symlink nor a hardlink, so its writer skips them, and
/// verification must not then go looking for them.
fn container_keeps(container: Container, entry: &Entry) -> bool {
    match container {
        Container::SevenZ => entry.symlink.is_none() && entry.hardlink.is_none(),
        _ => true,
    }
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
        out.push(match container {
            // 7z's writer skips a link outright rather than writing a body it has no
            // way to point anywhere, so the member simply will not be there.
            Container::SevenZ => {
                "7z does not store hard links — they will not be kept.".to_string()
            }
            _ => "zip does not store hard links — they will not be kept.".to_string(),
        });
    }
    if container == Container::SevenZ && entries.iter().any(|e| e.symlink.is_some()) {
        out.push("7z does not store symbolic links — they will not be kept.".to_string());
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

// ---------------------------------------------------------------------------
// Apply — P4 §2. The only part of this file that touches a disk.
// ---------------------------------------------------------------------------

/// Which half of the work a progress report belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Building,
    Verifying,
}

impl Phase {
    pub fn label(self) -> &'static str {
        match self {
            Phase::Building => "Building",
            Phase::Verifying => "Verifying",
        }
    }
}

/// What Apply reports back to the window.
///
/// `Cancelled` is a variant of its own, unlike `ExtractMsg`, where its absence means a
/// cancelled extraction is indistinguishable from a finished one. An Apply that stopped
/// early must never look like an Apply that succeeded.
#[derive(Debug)]
pub enum ApplyMsg {
    Progress {
        phase: Phase,
        done: usize,
        total: usize,
    },
    /// How far into the member currently being written the worker has read.
    ///
    /// A separate variant rather than two more fields on `Progress`, because `done` and
    /// `total` there are **members** — the status bar prints them as "3/9" and that must
    /// stay true — while one member can be the whole archive. The window folds this into
    /// the bar's fraction and never into the count.
    Within {
        done: u64,
        total: u64,
    },
    Done {
        entries: usize,
    },
    Cancelled,
    Failed(String),
}

/// The advisory lock that stops two windows rebuilding one archive.
///
/// P3 §4 carried this requirement forward as "an advisory lock on the archive path", and
/// the obvious implementation — lock the archive file — **does not work**, which was
/// measured rather than reasoned about. `File::try_lock` is `flock(2)`, whose lock lives
/// on the inode; Apply finishes by renaming a different inode over the name; so the
/// holder keeps a lock on an unlinked inode nobody can reach, and the next window opens
/// the new one and locks it happily. The guard would hold for the first Apply and fail
/// silently for every one after — exactly the case it exists for.
///
/// So the lock is taken on a file named for the archive under `$XDG_RUNTIME_DIR`, which
/// nothing ever renames over. It sits there rather than beside the archive so INDIUM
/// leaves no litter in the user's folders, and so the session's own logout wipe clears
/// whatever a crash leaves behind.
pub struct Lock {
    _file: std::fs::File,
}

impl Lock {
    /// Take the lock for `target`, or report why not.
    pub fn take(target: &Path) -> Result<Lock, String> {
        let path = lock_path_for(target);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("could not make the lock directory: {e}"))?;
        }
        let file = std::fs::File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(|e| format!("could not open the lock file: {e}"))?;

        match file.try_lock() {
            Ok(()) => Ok(Lock { _file: file }),
            Err(std::fs::TryLockError::WouldBlock) => {
                Err("Another INDIUM window is rebuilding this archive.".to_string())
            }
            Err(std::fs::TryLockError::Error(e)) => {
                Err(format!("could not lock this archive: {e}"))
            }
        }
    }
}

/// Where an archive's lock file lives.
///
/// The name is derived from the full path so two archives of the same name in different
/// directories do not collide, and it is sanitised so it is always one flat filename.
/// Pure, so the rule is testable without a runtime directory.
pub fn lock_name_for(target: &Path) -> String {
    // Canonicalised first, or the guard is trivially defeated by how the archive was
    // named: `indium ./photos.7z` in one window and `indium /home/megas/photos.7z` in
    // another would take two different locks on one file, and paths come straight from
    // `std::env::args_os`, so the relative form is ordinary rather than exotic. This also
    // gives the symlink case for free. A path that cannot be canonicalised — a new
    // archive that does not exist yet — falls back to the name as given, which is
    // correct: nothing else can be holding a lock on a file that is not there.
    let resolved = target
        .canonicalize()
        .unwrap_or_else(|_| target.to_path_buf());
    // Escaped, not folded. Until P15 every character outside the set below became a bare
    // `%`, which threw away what it was escaping: `açık.zip` and `aşık.zip` in one directory
    // both came out `a%%k.zip.lock`, so one window told the other it was already rebuilding
    // an archive it had never opened. Writing the byte in hex costs two characters and makes
    // the mapping injective, which is the whole property a lock name needs. `%` is itself
    // outside the set and so becomes `%25`, or the escape would be ambiguous with a literal.
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let full = resolved.to_string_lossy();
    let mut name = String::with_capacity(full.len() + 5);
    for b in full.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'.' | b'-' | b'_' => name.push(b as char),
            _ => {
                name.push('%');
                name.push(HEX[(b >> 4) as usize] as char);
                name.push(HEX[(b & 0x0F) as usize] as char);
            }
        }
    }
    // A very deep path would make a filename no filesystem accepts. Keeping the tail
    // keeps the part that differs between neighbours. The cut is by byte and the name is
    // pure ASCII by construction, so it cannot land inside a character; it can land inside
    // an escape, which costs nothing — two paths alike for everything but their first bytes
    // shared a lock under the old scheme too, and the tail is still what tells them apart.
    if name.len() > 180 {
        name = name.split_off(name.len() - 180);
    }
    format!("{name}.lock")
}

fn lock_path_for(target: &Path) -> PathBuf {
    crate::platform::runtime_or_cache_dir()
        .join("indium")
        .join("locks")
        .join(lock_name_for(target))
}

/// Everything Apply needs, gathered before a worker thread is spawned.
pub struct ApplyInput {
    pub target: PathBuf,
    pub recipe: Recipe,
    pub tasks: Vec<Task>,
    /// Adds already resolved to individual members. A directory staged by `Task::Add`
    /// is expanded by Apply itself, so a caller may leave this empty; it exists for a
    /// caller that has already walked the tree.
    pub adds: Vec<AddItem>,
    /// The normalised paths the queue was staged against.
    ///
    /// Apply re-lists the source and refuses if this no longer matches — the archive can
    /// change on disk between staging and Apply, and a queue folded against a listing
    /// that is no longer true would rebuild the wrong thing. Empty means "do not check",
    /// which is right for a creation and for a caller with nothing staged.
    pub staged_against: Vec<String>,
    /// The password the source was read with, if it was encrypted.
    pub source_password: Option<Secret>,
    /// The password the rebuilt archive is encrypted with, if it is.
    pub target_password: Option<Secret>,
}

/// Rebuild the archive, and replace the original only once the replacement is proven.
///
/// CORE §3, in order: lock, re-list, fold, build beside the target, verify by walking the
/// new archive's entries, rename over, sync. Any failure and any cancellation removes the
/// temp file and leaves the original exactly as it was.
pub fn apply(
    input: &ApplyInput,
    tx: &Sender<ApplyMsg>,
    cancel: &Arc<AtomicBool>,
) -> Result<usize, String> {
    let creating = input.tasks.iter().any(|t| matches!(t, Task::Create { .. }));

    // 1. The lock, held for everything below.
    let _lock = Lock::take(&input.target)?;

    // A new archive must never silently replace an existing file. `create_new` failing
    // with `AlreadyExists` is the check, and it costs nothing.
    if creating && input.target.exists() {
        return Err(format!(
            "{} already exists.",
            input.target.to_string_lossy()
        ));
    }

    // The mode the archive already has, read before anything is built.
    //
    // The commit at step 7 renames a **fresh inode** over the archive, so without this the
    // archive's permissions are not rebuilt with it — they are replaced by `0o666 & ~umask`. An
    // archive the user had tightened to `0600` came back `0644`, and for an AES-256 7z that puts
    // the ciphertext where anyone on the machine can read it, silently, on every Apply. Measured:
    // `an_apply_does_not_widen_the_mode_of_the_archive_it_rebuilds`.
    //
    // `symlink_metadata`, so `is_file()` is false for a symlink and a mode is carried only from a
    // real file. Creating an archive leaves this `None`, which is the umask default and correct.
    let prior_mode = std::fs::symlink_metadata(&input.target)
        .ok()
        .filter(|md| md.is_file())
        .map(|md| std::os::unix::fs::PermissionsExt::mode(&md.permissions()));

    // 2. Re-list the source inside the worker. The archive may have changed since the
    //    queue was staged against it, and a guard that assumes otherwise is not a guard.
    let source: Vec<Entry> = if creating {
        Vec::new()
    } else {
        crate::arch::list_all(&input.target, input.source_password.as_ref())
            .map_err(|e| e.to_string())?
    };

    // The two refusals P4 §1 states, checked here as well as at stage time. The UI gate
    // is the convenience; this is the guard. Without it an `ApplyInput` naming an
    // encrypted source and an unencrypted recipe would stream decrypted members into a
    // plaintext archive — the silent security downgrade §1 exists to forbid — and the
    // only thing standing in the way would be a window that had not been written yet.
    if source.iter().any(|e| e.encrypted) && !input.recipe.encrypt {
        return Err(Conflict::EncryptedSourceCannotBeRewritten.to_string());
    }
    if input.recipe.container() == Container::Tar && input.recipe.method == Method::Lzma2 {
        return Err(Conflict::FormatCannotBeWritten("this archive".to_string()).to_string());
    }

    // The archive may have changed on disk since the queue was staged against it.
    if !input.staged_against.is_empty() {
        let mut now: Vec<&str> = source.iter().map(|e| e.path.as_str()).collect();
        let mut then: Vec<&str> = input.staged_against.iter().map(|s| s.as_str()).collect();
        now.sort_unstable();
        then.sort_unstable();
        if now != then {
            // Says what happened, what was done about it, and what to do next. The old
            // wording stopped after the first of those, which reads as a failure rather
            // than as the refusal it is — the staged changes are still here, and the only
            // thing lost is the assumption they were built on.
            return Err(
                "Another window changed this archive after these changes were \
                        staged, so nothing was written. Re-open it and stage them again."
                    .to_string(),
            );
        }
    }

    // Directory adds are expanded here, not in the fold, because walking a tree touches
    // the disk and `plan` must stay pure. Doing it inside Apply rather than leaving it
    // to the caller is what stops a directory being folded once unexpanded and once
    // expanded.
    let mut adds = input.adds.clone();
    for task in &input.tasks {
        if let Task::Add { source: src, dest } = task {
            if src.is_dir() {
                adds.extend(
                    expand_add(src, dest)
                        .map_err(|e| format!("could not read {}: {e}", src.display()))?,
                );
            }
        }
    }

    // 3. Fold. Any conflict fails here, before a byte is written.
    let plan = plan(&source, &input.tasks, &adds).map_err(|c| c.to_string())?;

    // 4. Build into a temp beside the target. A leftover from an interrupted Apply is
    //    removed first — that is the whole of the orphan policy, and it only ever
    //    touches a file whose name is provably ours.
    let temp = temp_path_for(&input.target);
    if let Some(name) = temp.file_name().and_then(|n| n.to_str()) {
        if is_our_temp(name) && temp.exists() {
            let _ = std::fs::remove_file(&temp);
        }
    }

    let outcome = build_and_verify(input, &plan, &source, &temp, tx, cancel);

    match outcome {
        Err(e) => {
            let _ = std::fs::remove_file(&temp);
            Err(e)
        }
        Ok(None) => {
            // Cancelled. The temp goes; the original was never touched.
            let _ = std::fs::remove_file(&temp);
            let _ = tx.send(ApplyMsg::Cancelled);
            Ok(0)
        }
        Ok(Some(count)) => {
            // The permissions go on the **temp**, before the rename, so the archive is never
            // present at its own name carrying a mode the user did not choose. Doing it after the
            // rename would leave a window — brief, but a window on a file whose whole point may
            // be that only one account can read it.
            //
            // It fails the Apply rather than shrugging. A rebuild that cannot carry the archive's
            // own permissions has not reproduced the archive, and `let _ =` here would be exactly
            // the silent-failure class this round is auditing for. The original is untouched on
            // this path, which is what makes failing safe.
            if let Some(mode) = prior_mode {
                let want =
                    <std::fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(mode);
                if let Err(e) = std::fs::set_permissions(&temp, want) {
                    let _ = std::fs::remove_file(&temp);
                    return Err(format!(
                        "the rebuild is good but its permissions could not be set to {mode:o}, so \
                         it was not committed: {e}"
                    ));
                }
            }
            // 7. Commit. The rename is atomic; the parent directory is synced because
            //    the durability of a rename needs it, exactly as `store::atomic_write`
            //    already does for the settings file.
            std::fs::rename(&temp, &input.target).map_err(|e| {
                let _ = std::fs::remove_file(&temp);
                format!("could not replace the archive: {e}")
            })?;
            if let Some(dir) = input.target.parent() {
                if let Ok(handle) = std::fs::File::open(dir) {
                    let _ = handle.sync_all();
                }
            }
            let _ = tx.send(ApplyMsg::Done { entries: count });
            Ok(count)
        }
    }
}

/// Build the replacement and prove it. `Ok(None)` means cancelled.
fn build_and_verify(
    input: &ApplyInput,
    plan: &Plan,
    source: &[Entry],
    temp: &Path,
    tx: &Sender<ApplyMsg>,
    cancel: &Arc<AtomicBool>,
) -> Result<Option<usize>, String> {
    let total = plan.out_count();
    let mut written = 0usize;
    let mut added_sizes: Vec<(String, u64)> = Vec::new();

    {
        let mut sink: Box<dyn Sink> = match input.recipe.container() {
            Container::SevenZ => Box::new(crate::sevenz::Writer::create(
                temp,
                &input.recipe,
                input.target_password.as_ref(),
            )?),
            _ => Box::new(crate::arch::Writer::create(temp, &input.recipe)?),
        };

        // The kept members, streamed one pass over the source.
        if !source.is_empty() {
            let mut reader =
                crate::arch::Reader::open(&input.target, input.source_password.as_ref())
                    .map_err(|e| e.to_string())?;
            let mut index = 0usize;

            while let Some(entry) = reader.next_entry().map_err(|e| e.to_string())? {
                if cancel.load(Ordering::Relaxed) {
                    sink.abandon();
                    return Ok(None);
                }

                let disposition = plan.source.get(index);
                index += 1;

                match disposition {
                    Some(Disposition::Keep { out_path, hardlink }) => {
                        let meta = Meta::from_entry(&entry, out_path, hardlink.as_deref());
                        if meta.has_data() {
                            // Metered here as well as over the adds below, and for the
                            // same reason: rebuilding a 3 GB archive to rename one member
                            // copies every byte of it, and that is the same ten minutes of
                            // silence whether the bytes are new or kept.
                            let mut data = crate::arch::EntryData::new(&mut reader);
                            let mut metered = Metered::new(&mut data, meta.size, tx, cancel);
                            if !put_member(sink.as_mut(), &meta, Some(&mut metered), cancel)? {
                                return Ok(None);
                            }
                        } else {
                            reader.skip_data();
                            sink.put(&meta, None)?;
                        }
                        written += 1;
                        let _ = tx.send(ApplyMsg::Progress {
                            phase: Phase::Building,
                            done: written,
                            total,
                        });
                    }
                    _ => reader.skip_data(),
                }
            }
        }

        // Then the new members, appended.
        for item in &plan.adds {
            if cancel.load(Ordering::Relaxed) {
                sink.abandon();
                return Ok(None);
            }
            let meta = meta_from_fs(&item.source, &item.out_path)?;
            added_sizes.push((item.out_path.clone(), meta.size));
            if meta.has_data() {
                let mut file = std::fs::File::open(&item.source)
                    .map_err(|e| format!("could not read {}: {e}", item.source.display()))?;
                let mut metered = Metered::new(&mut file, meta.size, tx, cancel);
                if !put_member(sink.as_mut(), &meta, Some(&mut metered), cancel)? {
                    return Ok(None);
                }
            } else {
                sink.put(&meta, None)?;
            }
            written += 1;
            let _ = tx.send(ApplyMsg::Progress {
                phase: Phase::Building,
                done: written,
                total,
            });
        }

        // 5. Finish and sync, so what is verified is what is on the disk.
        sink.finish()?;
    }

    if let Ok(handle) = std::fs::File::open(temp) {
        let _ = handle.sync_all();
    }

    if cancel.load(Ordering::Relaxed) {
        return Ok(None);
    }

    // 6. Verify by walking the new archive's entries.
    let _ = tx.send(ApplyMsg::Progress {
        phase: Phase::Verifying,
        done: 0,
        total,
    });
    // Read back through the *other* implementation wherever that is possible: a 7z this
    // program wrote with sevenz-rust2 is proven by libarchive, which shares none of its
    // code. Only an encrypted 7z is verified by its own writer's reader, because
    // libarchive cannot open one at all — and there that doubles as proof the password
    // written with is the password that reads.
    let built = if input.recipe.container() == Container::SevenZ && input.recipe.encrypt {
        crate::sevenz::list_all(temp, input.target_password.as_ref()).map_err(|e| e.to_string())?
    } else {
        crate::arch::list_all(temp, None).map_err(|e| e.to_string())?
    };
    let expected = plan.expected(source, &added_sizes, input.recipe.container());
    verify_against(&expected, &built)?;
    let _ = tx.send(ApplyMsg::Progress {
        phase: Phase::Verifying,
        done: total,
        total,
    });

    // PXX. The flag is consulted once more, and this is not belt-and-braces: the check
    // above is the *last* one before the verify pass, and that pass is the longest
    // uninterruptible stretch in the whole of Apply — `list_all` is a full decompress of
    // what was just written, which on a 3 GB rebuild is minutes. Without this a Cancel
    // pressed at any point during Verifying was not merely ignored; `Ok(Some)` returned,
    // the caller renamed, and the replacement committed. The window promises "Nothing was
    // written" on the Cancelled arm, so committing there is the one outcome worse than
    // doing nothing — it is the opposite of what was asked for.
    //
    // Honoured at the boundary rather than inside `list_all`: threading a flag through it
    // would touch the listing, the CLI and the package tests to shorten a wait, where this
    // keeps the promise exactly. The cost is that the press is acted on when the pass ends
    // rather than at once, and the built temp is thrown away after being proven good —
    // which is the correct trade, because the alternative spends the user's archive.
    if cancel.load(Ordering::Relaxed) {
        return Ok(None);
    }

    Ok(Some(written))
}

/// The metadata a file on disk brings into the archive.
///
/// `pub(crate)` since P21: the estimator builds the same members Apply will, and it must
/// build them the same way or it is describing a different archive from the one it is
/// timing.
pub(crate) fn meta_from_fs(source: &Path, out_path: &str) -> Result<Meta, String> {
    use std::os::unix::fs::MetadataExt;
    use std::os::unix::fs::PermissionsExt;

    // `symlink_metadata`, not `metadata`: a symlink staged for adding is added as a
    // symlink, not silently followed and stored as a copy of whatever it points at.
    let md = std::fs::symlink_metadata(source)
        .map_err(|e| format!("could not read {}: {e}", source.display()))?;

    let symlink = if md.file_type().is_symlink() {
        std::fs::read_link(source)
            .ok()
            .map(|t| t.to_string_lossy().to_string())
    } else {
        None
    };

    Ok(Meta {
        out_path: out_path.to_string(),
        size: if md.is_dir() || symlink.is_some() {
            0
        } else {
            md.len()
        },
        is_dir: md.is_dir(),
        mode: md.permissions().mode() & 0o7777,
        mtime: Some(md.mtime()),
        atime: None,
        ctime: None,
        uid: md.uid() as i64,
        gid: md.gid() as i64,
        uname: None,
        gname: None,
        symlink,
        hardlink: None,
    })
}

/// Expand a directory staged for adding into the members it contributes.
///
/// Side-effecting, and therefore kept out of `plan`, which must stay pure. The queue
/// keeps one row — *"Add photos/"* — rather than four thousand.
pub fn expand_add(source: &Path, dest: &str) -> std::io::Result<Vec<AddItem>> {
    let mut out = Vec::new();
    let md = std::fs::symlink_metadata(source)?;
    if !md.is_dir() {
        out.push(AddItem {
            source: source.to_path_buf(),
            out_path: normalize_archive_path(dest),
        });
        return Ok(out);
    }

    let mut stack = vec![(source.to_path_buf(), normalize_archive_path(dest))];
    while let Some((dir, prefix)) = stack.pop() {
        out.push(AddItem {
            source: dir.clone(),
            out_path: prefix.clone(),
        });
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            let child_out = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}/{name}")
            };
            let child_md = std::fs::symlink_metadata(entry.path())?;
            if child_md.is_dir() {
                stack.push((entry.path(), child_out));
            } else {
                out.push(AddItem {
                    source: entry.path(),
                    out_path: child_out,
                });
            }
        }
    }
    Ok(out)
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

    /// CORE §5's verdicts ship in the Create popup, and this reads them out of the
    /// document rather than out of a second hand-copy.
    ///
    /// Until P18 the copy is what it compared against — eight sentences typed twice in this
    /// one file — so an edit to CORE §5 left the suite green while the window went on saying
    /// something the document no longer said. `keys.rs` had the right shape since P12 and was
    /// the only test in the tree using it.
    ///
    /// The comparison is `assert_eq!` and not `starts_with`, unlike `keys.rs`: that popup
    /// draws an abbreviation of CORE's cell, and this one draws the sentence, which is what
    /// `verdict()`'s own doc means by **verbatim**.
    #[test]
    fn every_method_verdict_is_core_section_five_verbatim() {
        let core = include_str!("../CORE.md");
        let after = core
            .split_once("### The method verdicts")
            .expect("CORE §5 has a 'The method verdicts' heading")
            .1;
        let rows: Vec<(String, String)> = after
            .lines()
            // Bounded at the next heading first. Without it, deleting §5's table walks the
            // parse into §6's and reports a mismatch about §5 having read something else
            // entirely — the "did not parse" guard below would never fire for the case it is
            // worded for.
            .take_while(|l| !l.starts_with('#'))
            .skip_while(|l| !l.starts_with('|'))
            .take_while(|l| l.starts_with('|'))
            // Not `contains("---")`: a re-centred column writes `| :-: |`, which has no run
            // of three and would be parsed as a row.
            .filter(|l| !l.chars().all(|c| matches!(c, '|' | '-' | ':' | ' ')))
            .map(|l| {
                let cells: Vec<&str> = l.trim().trim_matches('|').split('|').collect();
                assert!(
                    cells.len() >= 2,
                    "CORE §5's verdict table has a row with fewer than two cells: {l:?}"
                );
                // CORE names two of these by container *and* codec — "7z / LZMA2", "zip /
                // Deflate" — where `label()` gives only the codec. A cell without a slash is
                // itself, so the six single-token rows pass through untouched.
                let method = cells[0].rsplit('/').next().unwrap_or("").trim().to_string();
                (method, cells[1].trim().to_string())
            })
            .filter(|(m, _)| m != "Method")
            .collect();

        assert_eq!(
            rows.len(),
            METHODS.len(),
            "CORE §5 lists {} methods and the popup offers {}",
            rows.len(),
            METHODS.len()
        );
        for (i, ((cm, cs), method)) in rows.iter().zip(METHODS.iter()).enumerate() {
            assert_eq!(
                cm,
                method.label(),
                "row {i}: CORE §5 names {cm:?}, and METHODS has {:?} in that place",
                method.label()
            );
            assert_eq!(
                cs,
                method.verdict(),
                "row {i} ({cm}): CORE §5 says {cs:?}, and the window says {:?}",
                method.verdict()
            );
        }
    }

    /// CORE §4.1: "a live sentence states exactly what will be built".
    ///
    /// The name has claimed *exactly as CORE writes it* since P4 while the sentence was typed
    /// here by hand, and that is how §4.1 came to read `LZMA2:19` — a level this program cannot
    /// be set to — with the suite green over it for eighteen rounds. P22 fixes the figure and
    /// this test, which is the half that matters: the document's own example is now parsed and
    /// compared, so the next such slip fails the build instead of shipping.
    ///
    /// Only the example CORE prints is read from the document. The two below it hold branches
    /// §4.1 does not exemplify — a method with no levels, and no encryption — and stay written
    /// out, because a test cannot read what the document does not say.
    #[test]
    fn the_footer_sentence_reads_exactly_as_core_writes_it() {
        let core = include_str!("../CORE.md");
        let popups = core
            .split_once("### The popups")
            .expect("CORE §4 has a 'The popups' heading")
            .1;
        // Bounded at the next heading, as §5's gate is: without it a deleted §4.1 walks the
        // search into §5 and reports on a sentence written somewhere else entirely. Flattened
        // because §4.1 wraps this sentence across two lines — the newline is wrapping, not
        // structure.
        let popups: String = popups
            .lines()
            .take_while(|l| !l.starts_with('#'))
            .flat_map(str::split_whitespace)
            .collect::<Vec<_>>()
            .join(" ");
        let quoted = popups
            .split_once("a live sentence states exactly what will be built: *\"")
            .expect("CORE §4.1 says what the live sentence reads, and gives an example")
            .1
            .split_once("\"*")
            .expect("CORE §4.1's example is closed with \"*")
            .0;

        // Every field read off that example: the name it prints, the `7z` its extension gives,
        // the codec and level of `LZMA2:9`, and `AES-256.` for the encryption.
        let recipe = Recipe {
            path: PathBuf::from("/home/megas/photos-2026.7z"),
            method: Method::Lzma2,
            level: 9,
            encrypt: true,
        };
        assert_eq!(
            recipe_sentence(&recipe),
            quoted,
            "CORE §4.1 writes the live sentence as {quoted:?}"
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
                solid: None,
                blocks: None,
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

    /// PXX finding 4. `Indium::on_exit` removes the in-flight temp on a deliberate close
    /// **without joining the worker**, because joining would hold the window on screen
    /// after the user asked it to go. That rests on one POSIX guarantee, and a guarantee
    /// asserted only in a comment is folklore — so it is checked here instead.
    ///
    /// The 159 MB the certification walk found was left by exactly this: an `exit(0)` one
    /// second after the temp last grew, with nothing on the way out that would take it back.
    #[test]
    fn removing_a_temp_a_worker_still_holds_open_takes_only_the_name() {
        use std::io::Write;

        let dir = std::env::temp_dir().join("indium-pxx-orphan");
        std::fs::create_dir_all(&dir).expect("scratch dir");
        let temp = temp_path_for(&dir.join("archive.7z"));

        let mut worker = std::fs::File::create(&temp).expect("the worker opens its temp");
        worker
            .write_all(b"half an archive")
            .expect("and starts writing");

        // The same two guards `on_exit` applies, in the same order.
        let name = temp.file_name().and_then(|n| n.to_str()).expect("a name");
        assert!(is_our_temp(name), "{name} must be provably ours to remove");
        std::fs::remove_file(&temp).expect("removed while the worker still holds it");

        assert!(!temp.exists(), "the name goes the instant it is unlinked");
        // And the worker never finds out — it is between reads, not being killed. If this
        // ever fails, `on_exit` is racing a live write instead of quietly outliving it.
        worker
            .write_all(b", which nobody will ever read")
            .expect("the writer's own handle keeps working");
        worker.sync_all().expect("and still syncs");
        drop(worker);
        assert!(
            !temp.exists(),
            "and nothing reappears when the handle closes"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    /// Collect what a [`Metered`] said, in order.
    fn within_sent(rx: &std::sync::mpsc::Receiver<ApplyMsg>) -> Vec<u64> {
        rx.try_iter()
            .filter_map(|m| match m {
                ApplyMsg::Within { done, .. } => Some(done),
                _ => None,
            })
            .collect()
    }

    /// The half of 12.8 that made Cancel look dead.
    ///
    /// The flag was set the moment the button was pressed; nothing read it until the member
    /// finished, and the walk's member was the whole job. This pins the new latency at one
    /// chunk — 4 KiB under `sevenz-rust2`, 64 KiB under libarchive — rather than one member.
    #[test]
    fn a_cancelled_metered_read_stops_at_the_very_next_chunk() {
        use std::io::Read;

        let (tx, _rx) = std::sync::mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let data = vec![7u8; 64 * 1024];
        let mut src = &data[..];
        let mut m = Metered::new(&mut src, data.len() as u64, &tx, &cancel);

        let mut buf = [0u8; 4096];
        assert_eq!(m.read(&mut buf).expect("reads before the cancel"), 4096);

        cancel.store(true, Ordering::Relaxed);
        let e = m
            .read(&mut buf)
            .expect_err("a cancelled read must not succeed");
        assert_eq!(
            e.kind(),
            std::io::ErrorKind::Interrupted,
            "and it must be an error the writer will propagate rather than a short read, \
             which every writer treats as end-of-member and would commit"
        );
    }

    /// The other half: it reports, and it does not flood.
    ///
    /// 400 MiB read at the 4 KiB `sevenz-rust2` uses is 102,400 calls to `read`. A message
    /// per call would put a hundred thousand of them down a channel the window drains once
    /// a frame; a message per member is what the walk got, which was none. The step lands it
    /// at a hundred, which is one per percentage point and one more than the bar can show.
    #[test]
    fn a_large_metered_member_reports_a_hundred_times_and_not_a_hundred_thousand() {
        use std::io::Read;

        const TOTAL: u64 = 400 << 20;
        let (tx, rx) = std::sync::mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let mut src = std::io::repeat(0u8).take(TOTAL);
        let mut m = Metered::new(&mut src, TOTAL, &tx, &cancel);

        let mut buf = [0u8; 4096];
        while m.read(&mut buf).expect("a repeat reader does not fail") > 0 {}

        let sent = within_sent(&rx);
        assert_eq!(sent.len(), 100, "one per percentage point of the member");
        assert!(
            sent.windows(2).all(|w| w[0] < w[1]),
            "a bar that goes backwards is worse than one that does not move: {sent:?}"
        );
        assert_eq!(
            *sent.last().expect("a hundred of them"),
            TOTAL,
            "and the last one lands on the whole member, so the bar reaches the end of it"
        );
    }

    /// And a small member says nothing at all, which is the right amount.
    ///
    /// The per-member `Progress` already covers an archive of small files exactly. The floor
    /// under the step is what stops the wrapper turning a folder of ten thousand thumbnails
    /// into ten thousand redundant messages.
    #[test]
    fn a_small_metered_member_says_nothing_the_member_count_has_not_already_said() {
        use std::io::Read;

        let (tx, rx) = std::sync::mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let data = vec![0u8; 900 * 1024];
        let mut src = &data[..];
        let mut m = Metered::new(&mut src, data.len() as u64, &tx, &cancel);

        let mut buf = [0u8; 4096];
        while m.read(&mut buf).expect("reads to the end") > 0 {}

        assert!(
            within_sent(&rx).is_empty(),
            "under the floor the member count is the whole report"
        );
    }

    // `restaging_a_creation_keeps_the_files_already_added` and its companion stood here
    // until P22, holding `Queue::set_creation`. Both are gone with it, and deliberately:
    // the premise changed. P21 needed the queue to keep its adds across a second Create
    // because the queue was the only place the file list lived. The draft holds it now, and
    // `create_projects_the_draft_onto_the_queue` above asserts something strictly stronger
    // — the queue is rebuilt from the draft on every press, so a file dropped from the
    // draft between two presses is dropped from the queue with it, which "keeps the files
    // already added" would have let through.

    // -----------------------------------------------------------------------
    // The draft
    // -----------------------------------------------------------------------

    fn draft_of(names: &[&str]) -> Draft {
        let mut draft = Draft::new();
        for n in names {
            draft.add(PathBuf::from(format!("/tmp/{n}")), n.to_string());
        }
        draft
    }

    fn a_recipe(method: Method) -> Recipe {
        Recipe {
            path: PathBuf::from("/tmp/backup.tar.gz"),
            method,
            level: method.default_level(),
            encrypt: false,
        }
    }

    fn dests(queue: &Queue) -> Vec<String> {
        queue.tasks()[1..]
            .iter()
            .map(|t| match t {
                Task::Add { dest, .. } => dest.clone(),
                other => panic!("not an add: {other:?}"),
            })
            .collect()
    }

    /// The projection: a draft plus a recipe *is* the queue, rebuilt from empty each press.
    ///
    /// This holds what P21's `set_creation` held — Create first, and only ever one — and one
    /// thing besides, which is the point of the round: a second press **refreshes** the adds
    /// from the draft rather than preserving whatever the first press left behind. A file
    /// dropped from the draft between the two presses is dropped from the queue with it, so
    /// a recipe can never be staged beside a stale list of what it is building.
    #[test]
    fn create_projects_the_draft_onto_the_queue() {
        let mut draft = draft_of(&["one", "two", "three"]);
        let mut queue = Queue::new();

        assert_eq!(draft.project_onto(&mut queue, a_recipe(Method::Gzip)), 0);
        assert_eq!(queue.len(), 4, "one Create and one Add per item");
        assert!(
            matches!(queue.tasks()[0], Task::Create { .. }),
            "Create is not first"
        );
        assert_eq!(
            queue
                .tasks()
                .iter()
                .filter(|t| matches!(t, Task::Create { .. }))
                .count(),
            1,
            "a second Create was pushed instead of the queue being rebuilt"
        );
        assert_eq!(
            dests(&queue),
            ["one", "two", "three"],
            "the dests are the draft's, in the draft's order"
        );

        // Second press: a different method, and one fewer file.
        draft.remove(2);
        assert_eq!(
            draft.project_onto(&mut queue, a_recipe(Method::Zstd)),
            0,
            "re-Create over its own projection displaces nothing"
        );
        assert_eq!(queue.creation().map(|r| r.method), Some(Method::Zstd));
        assert_eq!(
            dests(&queue),
            ["one", "two"],
            "the removed file is still staged — the queue was merged into, not rebuilt"
        );
    }

    /// A creation displaces mutations staged against the archive that is open, and reports
    /// how many. The maker's ruling for both doors that lose staged work is to say what went.
    #[test]
    fn create_over_staged_mutations_displaces_them_and_counts_them() {
        let mut queue = Queue::new();
        queue.push(Task::Rename {
            from: "a".into(),
            to: "b".into(),
        });
        queue.push(Task::Rename {
            from: "c".into(),
            to: "d".into(),
        });
        queue.push(Task::Remove { path: "e".into() });

        let draft = draft_of(&["one"]);
        assert_eq!(
            draft.project_onto(&mut queue, a_recipe(Method::Gzip)),
            3,
            "the changes against the open archive went unreported"
        );
        assert_eq!(queue.len(), 2, "a displaced task is still in the queue");
    }

    /// What *Bring from archive* pulls are **copies**, and this is the sentence that turns on.
    ///
    /// CORE §4 promises "the archive they came from can be closed without touching them", and
    /// F6 rules that the draft survives a Close untouched. Both are only true because a pull
    /// makes files: an entry inside an archive is not one, so a draft that held references
    /// into the archive would be a list of promises the engine could not keep the moment the
    /// archive was closed, renamed or rebuilt.
    ///
    /// Written against the real filesystem because that is the whole claim. The archive is
    /// deleted outright here — a harsher test than closing it — and what the draft projects
    /// is still readable afterwards.
    #[test]
    fn a_draft_survives_the_archive_it_was_drawn_from() {
        let dir =
            std::env::temp_dir().join(format!("indium-draft-survives-{}", std::process::id()));
        let pull = dir.join("1").join("docs");
        std::fs::create_dir_all(&pull).unwrap();

        let archive = dir.join("photos.zip");
        std::fs::write(&archive, b"not really an archive").unwrap();
        let pulled = pull.join("notes.txt");
        std::fs::write(&pulled, b"brought across").unwrap();

        // The dest a pull derives: the path the entry had *inside* the archive.
        let mut draft = Draft::new();
        draft.add(pulled.clone(), "docs/notes.txt".to_string());

        std::fs::remove_file(&archive).unwrap();

        let mut queue = Queue::new();
        draft.project_onto(&mut queue, a_recipe(Method::Zstd));
        let Task::Add { source, dest } = &queue.tasks()[1] else {
            panic!("the draft did not project an Add");
        };
        assert_eq!(dest, "docs/notes.txt", "a pull flattened the entry's path");
        assert_eq!(
            std::fs::read(source).unwrap(),
            b"brought across",
            "the draft's file went with the archive it came from"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Projecting does not consume the draft, and that is what makes *Discard* safe: it
    /// clears the queue, and the basket a person built is still there to build from.
    #[test]
    fn projecting_does_not_consume_the_draft() {
        let draft = draft_of(&["one", "two"]);
        let mut queue = Queue::new();
        draft.project_onto(&mut queue, a_recipe(Method::Gzip));

        queue.clear(); // what Discard does
        assert_eq!(draft.len(), 2, "the draft went with the queue");

        assert_eq!(draft.project_onto(&mut queue, a_recipe(Method::Gzip)), 0);
        assert_eq!(queue.len(), 3, "the draft could not be built from twice");
    }

    /// One item per name inside the archive, replaced in place, and the displaced one comes
    /// back so it can be said out loud rather than lost at Apply.
    #[test]
    fn a_second_item_with_the_same_name_replaces_the_first_in_place() {
        let mut draft = draft_of(&["one", "two"]);

        let displaced = draft.add(PathBuf::from("/elsewhere/one"), "one".to_string());
        assert_eq!(
            displaced.map(|d| d.source),
            Some(PathBuf::from("/tmp/one")),
            "the replaced item was not reported"
        );
        assert_eq!(draft.len(), 2, "the collision grew the draft instead");
        assert_eq!(draft.items()[0].source, PathBuf::from("/elsewhere/one"));
        assert_eq!(
            draft.items()[0].dest,
            "one",
            "the row moved instead of staying"
        );
        assert_eq!(draft.items()[1].dest, "two");
    }

    /// A draft names a file on disk and the name it will take, and the two are independent.
    /// That independence is what lets an item pulled out of an archive outlive it.
    ///
    /// What this reaches is the projection: both halves are copied through untouched, so no
    /// task a draft builds refers back to any archive. What it cannot reach is the pull —
    /// that entries are written to disk *before* they enter the draft is a fact about
    /// `Kind::Draft` and the worker, and only the window shows that.
    #[test]
    fn a_draft_names_a_file_and_the_name_it_will_take() {
        let mut draft = Draft::new();
        draft.add(
            PathBuf::from("/run/user/1000/indium/dr-42-1/photos/a.jpg"),
            "photos/a.jpg".to_string(),
        );
        let mut queue = Queue::new();
        draft.project_onto(&mut queue, a_recipe(Method::Gzip));

        let Task::Add { source, dest } = &queue.tasks()[1] else {
            panic!("the projection did not produce an add");
        };
        assert_eq!(
            source,
            Path::new("/run/user/1000/indium/dr-42-1/photos/a.jpg"),
            "the source stopped being where the file actually is"
        );
        assert_eq!(dest, "photos/a.jpg", "the name inside the archive drifted");
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

    /// The defect P11 recorded and P15 closed: folding every non-ASCII character to one `%`
    /// made two ordinary Turkish filenames share a lock, and the second window to open one
    /// was told the first was rebuilding it.
    #[test]
    fn two_names_alike_only_outside_ascii_take_different_locks() {
        let one = lock_name_for(Path::new("/tmp/açık.zip"));
        let two = lock_name_for(Path::new("/tmp/aşık.zip"));
        assert_ne!(one, two, "{one} and {two} must not be the same lock");
    }

    /// A lock name is one flat filename: nothing in it may still read as a path.
    #[test]
    fn a_lock_name_keeps_no_separator_and_stays_ascii() {
        let name = lock_name_for(Path::new("/tmp/a b/c.zip"));
        assert!(!name.contains('/'), "{name} still holds a separator");
        assert!(name.is_ascii(), "{name} is not pure ASCII");
        assert!(name.ends_with(".lock"), "{name} does not end in .lock");
    }

    /// The escape is unambiguous, which is what makes the mapping injective: a literal `%`
    /// in a name cannot be mistaken for one INDIUM wrote.
    #[test]
    fn a_literal_percent_is_escaped_like_any_other_byte() {
        let literal = lock_name_for(Path::new("/tmp/%41.zip"));
        let escaped = lock_name_for(Path::new("/tmp/A.zip"));
        assert!(
            literal.contains("%2541"),
            "{literal} lost its literal percent"
        );
        assert_ne!(literal, escaped);
    }

    /// Distinct directories keep distinct locks — the reason the whole path is folded in
    /// rather than just the filename.
    #[test]
    fn the_same_name_in_two_directories_takes_two_locks() {
        let here = lock_name_for(Path::new("/tmp/one/photos.7z"));
        let there = lock_name_for(Path::new("/tmp/two/photos.7z"));
        assert_ne!(here, there);
    }
}
