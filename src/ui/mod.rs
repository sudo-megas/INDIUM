//! The window: sidebar, table, Inspector, status bar, and every popup.
//!
//! CORE §4: "Five fixed zones and seven popups. Nothing else appears, ever." P2 §5 added
//! the password prompt by the maker's ordered CORE edit; P4 fills in the two the count
//! always allowed for — New Archive and Pending tasks — and puts rename in the table
//! rather than making it an eighth.

pub mod about;
pub mod extract;
pub mod filter;
pub mod inspector;
pub mod newarchive;
pub mod openwith;
pub mod password;
pub mod pending;
pub mod settings;
pub mod sidebar;
pub mod table;
pub mod tray;

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;

use eframe::egui;

use crate::arch::{self, ArchiveError, ArchiveInfo, Entry, ExtractMsg, ListMsg};
use crate::model::{self, Row};
use crate::platform::apps::{self, Candidate};
use crate::platform::clipboard;
use crate::platform::scratch::{self, Scratch};
use crate::platform::store::{self, ExtractDefault, Recents, Settings, Store};
use crate::secret::Secret;
use crate::tasks::{self, ApplyMsg, Queue, Task};
use crate::theme;

/// Which centre view the sidebar has selected. P2 §2: "the sidebar selects what the
/// centre shows".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Recents,
    Bookmarks,
    Archive,
}

/// CORE §4: two tabs, toggled with `Space`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectorTab {
    Details,
    Preview,
}

/// The popups. CORE §4 fixes the list at seven, and this is all of them: rename happens
/// in the table rather than in an eighth.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Popup {
    NewArchive,
    PendingTasks,
    Extract,
    About,
    Settings,
    OpenPath,
    Password,
    OpenWith,
}

/// What a password, once typed, is for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingAction {
    /// Re-list an archive whose headers are encrypted.
    List(PathBuf),
    /// Extract the current selection to a destination.
    Extract { dest: PathBuf },
    /// Compute a CRC for one entry.
    Crc { entry: String },
    /// Copy the current selection out to the clipboard.
    CopyOut,
    /// Extract one entry and offer it to an application.
    OpenWith { entry: String },
    /// Rebuild the archive once a password has been given.
    Apply,
}

pub struct Progress {
    pub done: usize,
    pub total: usize,
    pub label: String,
}

/// Everything the window is.
pub struct Indium {
    // --- the open archive -------------------------------------------------
    pub archive_path: Option<PathBuf>,
    pub archive_bytes: u64,
    pub archive_info: Option<ArchiveInfo>,
    pub entries: Vec<Entry>,
    pub listing: bool,

    // --- navigation -------------------------------------------------------
    pub section: Section,
    pub cwd: String,
    pub cursor: usize,
    /// Selected archive paths. Kept as paths, not row indices, so a selection
    /// survives descending, filtering and re-listing.
    pub selection: BTreeSet<String>,
    pub inspector_tab: InspectorTab,
    /// `Some` while the filter bar is open, even when empty.
    pub filter: Option<String>,
    pub filter_focus_requested: bool,

    // --- staging (P4) -----------------------------------------------------
    /// The queue CORE §3 calls the staging engine. Empty means the tray is hidden.
    pub tasks: Queue,
    /// The normalised paths the queue was staged against, so Apply can refuse if the
    /// archive changed on disk underneath it.
    pub staged_against: Vec<String>,
    /// `Some(path)` while a name is being edited in place. CORE §4 fixes the popup count
    /// at seven, so rename is not an eighth.
    pub rename_target: Option<String>,
    pub rename_input: String,

    // --- New Archive (P4 §5) ----------------------------------------------
    pub new_name: String,
    pub new_dir: String,
    pub new_preset: tasks::Preset,
    pub new_method: tasks::Method,
    pub new_level: u32,
    pub new_advanced: bool,
    pub new_encrypt: bool,
    apply_rx: Option<Receiver<ApplyMsg>>,
    /// Paths read off the clipboard by a worker. Reading blocks until the program that
    /// owns the selection finishes writing, so it never happens on the UI thread.
    paste_rx: Option<Receiver<Result<Vec<PathBuf>, String>>>,

    // --- popups -----------------------------------------------------------
    pub popup: Option<Popup>,
    pub extract_path: String,
    pub extract_to_subdir: bool,
    pub open_path: String,
    pub password_input: String,
    /// The second field, shown only when building a fresh encrypted archive: there is
    /// nothing to check a typo against, and a typo would build something nobody can open.
    pub password_confirm: String,
    pub password_attempts: u8,
    pub pending: Option<PendingAction>,
    pub bookmark_name: String,
    pub bookmark_path: String,

    // --- Open With (P3) ---------------------------------------------------
    pub openwith_candidates: Vec<Candidate>,
    pub openwith_filter: String,
    pub openwith_show_all: bool,
    pub openwith_path: Option<PathBuf>,
    pub openwith_name: String,
    pub openwith_mime: String,

    /// Where copies handed to the outside world live. Dropping it removes them.
    pub scratch: Scratch,

    // --- worker -----------------------------------------------------------
    pub cancel: Arc<AtomicBool>,
    list_rx: Option<Receiver<ListMsg>>,
    extract_rx: Option<Receiver<ExtractMsg>>,
    pub progress: Option<Progress>,

    // --- persistence ------------------------------------------------------
    pub store: Store,
    pub settings: Settings,
    pub recents: Recents,
    /// P2 §1: while a file failed to parse, INDIUM must not overwrite it.
    pub settings_broken: bool,
    pub recents_broken: bool,

    // --- chrome -----------------------------------------------------------
    pub status: String,
    /// The computed CRC of the focused entry, cleared whenever focus moves.
    pub crc_of: Option<(String, u32)>,
    /// Held only for the duration of one operation, then dropped and wiped.
    pub passphrase: Option<Secret>,
}

impl Indium {
    pub fn new(cc: &eframe::CreationContext<'_>, open: Option<PathBuf>) -> Indium {
        theme::install(&cc.egui_ctx);

        let store = Store::new();
        let settings = store.load_settings();
        let recents = store.load_recents();

        let mut status = String::from("Ready.");
        if let Some(n) = settings.notice.clone().or_else(|| recents.notice.clone()) {
            status = n;
        }

        let mut app = Indium {
            archive_path: None,
            archive_bytes: 0,
            archive_info: None,
            entries: Vec::new(),
            listing: false,

            section: Section::Recents,
            cwd: String::new(),
            cursor: 0,
            selection: BTreeSet::new(),
            inspector_tab: InspectorTab::Details,
            filter: None,
            filter_focus_requested: false,

            popup: None,
            extract_path: String::new(),
            extract_to_subdir: settings.value.extract.default == ExtractDefault::Subdir,
            open_path: String::new(),
            tasks: Queue::new(),
            staged_against: Vec::new(),
            rename_target: None,
            rename_input: String::new(),
            new_name: String::new(),
            new_dir: String::new(),
            new_preset: tasks::Preset::Balanced,
            new_method: tasks::Method::Zstd,
            new_level: tasks::Method::Zstd.default_level(),
            new_advanced: false,
            new_encrypt: false,
            apply_rx: None,
            paste_rx: None,
            password_input: String::new(),
            password_confirm: String::new(),
            password_attempts: 0,
            pending: None,
            bookmark_name: String::new(),
            bookmark_path: String::new(),

            openwith_candidates: Vec::new(),
            openwith_filter: String::new(),
            openwith_show_all: false,
            openwith_path: None,
            openwith_name: String::new(),
            openwith_mime: String::new(),
            scratch: Scratch::new(),

            cancel: Arc::new(AtomicBool::new(false)),
            list_rx: None,
            extract_rx: None,
            progress: None,

            store,
            settings_broken: settings.was_broken,
            recents_broken: recents.was_broken,
            settings: settings.value,
            recents: recents.value,

            status,
            crc_of: None,
            passphrase: None,
        };

        // P3 §1: "Stale `scratch/` cache entries are swept at launch." The runtime
        // dir has the session's logout wipe as a backstop; the disk cache has nothing.
        app.scratch.sweep_stale();

        if let Some(path) = open {
            app.open_archive(&cc.egui_ctx, path, None);
        }
        app
    }

    // -----------------------------------------------------------------------
    // Opening and listing
    // -----------------------------------------------------------------------

    pub fn open_archive(&mut self, ctx: &egui::Context, path: PathBuf, passphrase: Option<Secret>) {
        self.cancel.store(true, Ordering::Relaxed);
        self.cancel = Arc::new(AtomicBool::new(false));

        self.entries.clear();
        self.selection.clear();
        self.cwd.clear();
        self.cursor = 0;
        self.crc_of = None;
        self.filter = None;
        self.archive_info = None;
        self.archive_bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        self.archive_path = Some(path.clone());
        self.section = Section::Archive;
        self.listing = true;
        self.status = format!("Reading {}…", path.display());

        let (tx, rx) = channel();
        self.list_rx = Some(rx);
        let cancel = Arc::clone(&self.cancel);
        let ctx = ctx.clone();
        let pass = passphrase;

        std::thread::spawn(move || {
            arch::list(&path, pass.as_ref(), &tx, &cancel);
            // Reactive mode: the worker is what wakes the UI. CORE §3 — "an idle
            // INDIUM repaints nothing and costs nothing".
            ctx.request_repaint();
        });
    }

    fn drain_worker(&mut self, ctx: &egui::Context) {
        // Messages are collected first and acted on afterwards: handling them inside
        // the `try_iter` loop would hold a borrow of `self.list_rx` across calls that
        // need `&mut self`.
        let list_msgs: Vec<ListMsg> = match &self.list_rx {
            Some(rx) => rx.try_iter().collect(),
            None => Vec::new(),
        };
        let extract_msgs: Vec<ExtractMsg> = match &self.extract_rx {
            Some(rx) => rx.try_iter().collect(),
            None => Vec::new(),
        };
        let apply_msgs: Vec<ApplyMsg> = match &self.apply_rx {
            Some(rx) => rx.try_iter().collect(),
            None => Vec::new(),
        };
        let paste_msgs: Vec<Result<Vec<PathBuf>, String>> = match &self.paste_rx {
            Some(rx) => rx.try_iter().collect(),
            None => Vec::new(),
        };
        let wake = !list_msgs.is_empty()
            || !extract_msgs.is_empty()
            || !apply_msgs.is_empty()
            || !paste_msgs.is_empty();

        for msg in list_msgs {
            match msg {
                ListMsg::Opened(info) => self.archive_info = Some(info),
                ListMsg::Entry(e) => self.entries.push(*e),
                ListMsg::Done { count } => {
                    self.listing = false;
                    self.list_rx = None;
                    let _ = count;
                    self.status = self
                        .archive_path
                        .as_ref()
                        .and_then(|p| p.file_name())
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "Ready.".to_string());
                    self.remember_current_archive();
                }
                ListMsg::Failed(e) => {
                    self.listing = false;
                    self.list_rx = None;
                    self.on_list_failure(e);
                }
            }
        }

        for msg in extract_msgs {
            match msg {
                ExtractMsg::Progress { done, total } => {
                    self.progress = Some(Progress {
                        done,
                        total,
                        label: "Extracting".to_string(),
                    });
                }
                ExtractMsg::Done { written } => {
                    self.progress = None;
                    self.extract_rx = None;
                    self.status = format!(
                        "Extracted {written} {}.",
                        if written == 1 { "entry" } else { "entries" }
                    );
                    // The password's job is over.
                    self.passphrase = None;
                }
                ExtractMsg::Failed(msg) => {
                    self.progress = None;
                    self.extract_rx = None;
                    self.status = msg;
                    self.passphrase = None;
                }
            }
        }

        for msg in paste_msgs {
            self.paste_rx = None;
            match msg {
                Ok(paths) if paths.is_empty() => {
                    self.status = "The clipboard holds no files.".to_string();
                }
                Ok(paths) => self.stage_adds(paths),
                Err(e) => self.status = e,
            }
        }

        for msg in apply_msgs {
            match msg {
                ApplyMsg::Progress { phase, done, total } => {
                    self.progress = Some(Progress {
                        done,
                        total,
                        label: phase.label().to_string(),
                    });
                }
                ApplyMsg::Done { entries } => {
                    self.progress = None;
                    self.apply_rx = None;
                    self.status = format!(
                        "Applied. The archive now holds {entries} entr{}.",
                        if entries == 1 { "y" } else { "ies" }
                    );
                    // The queue has been spent, and what is on screen is now a listing of
                    // the archive that was replaced. Re-open it rather than leave stale
                    // rows behind — the Inspector is the point of this program, and it
                    // must not describe a file that no longer exists.
                    self.tasks.clear();
                    self.staged_against.clear();
                    let path = self.archive_path.clone();
                    let pass = self.passphrase.take();
                    if let Some(path) = path {
                        self.open_archive(ctx, path, pass);
                    }
                }
                ApplyMsg::Cancelled => {
                    self.progress = None;
                    self.apply_rx = None;
                    self.passphrase = None;
                    // The queue survives a cancel: nothing was written, so the changes
                    // the user staged are still exactly what they asked for.
                    self.status =
                        "Cancelled. Nothing was written, and your changes are still staged."
                            .to_string();
                }
                ApplyMsg::Failed(msg) => {
                    self.progress = None;
                    self.apply_rx = None;
                    self.passphrase = None;
                    self.status = msg;
                }
            }
        }

        // Reactive mode repaints on input, and a `ProgressBar` — unlike the listing
        // `Spinner` — asks for nothing on its own, while the worker holds no `Context` to
        // ask with. So the drain keeps the pump alive for exactly as long as an operation
        // is running, and stops the moment progress clears. This is what makes a progress
        // bar move rather than jump to done when the mouse happens to twitch.
        if wake || self.progress.is_some() {
            ctx.request_repaint();
        }
    }

    fn on_list_failure(&mut self, e: ArchiveError) {
        match e {
            // P2 §5, the encrypted-headers flow: prompt and reopen.
            //
            // All three variants land here because the answer is the same — ask. Which
            // one arrives depends on which reader refused: libarchive says
            // `EncryptedHeaders`, and the 7z reader, which can actually open such an
            // archive once it has the password, says `NeedPassword` or `WrongPassword`.
            // Before P5 only the first was handled, so routing 7z through the streaming
            // list would have failed to a status line with no prompt behind it.
            ArchiveError::EncryptedHeaders
            | ArchiveError::NeedPassword
            | ArchiveError::WrongPassword => {
                if let Some(p) = self.archive_path.clone() {
                    self.status = e.to_string();
                    self.pending = Some(PendingAction::List(p));
                    self.popup = Some(Popup::Password);
                    self.password_input.clear();
                }
            }
            other => {
                self.status = other.to_string();
                self.archive_info = None;
            }
        }
    }

    fn remember_current_archive(&mut self) {
        let Some(path) = &self.archive_path else {
            return;
        };
        let path = path.to_string_lossy().to_string();
        self.recents.bump(&path, store::now());
        // P2 §1: writes happen on change, and never over a file that failed to parse.
        if !self.recents_broken {
            if let Err(e) = self.store.save_recents(&self.recents) {
                self.status = format!("Could not save recent files: {e}");
            }
        }
    }

    pub fn save_settings(&mut self) {
        if self.settings_broken {
            self.status = "settings.toml could not be parsed earlier; it will not be overwritten."
                .to_string();
            return;
        }
        if let Err(e) = self.store.save_settings(&self.settings) {
            self.status = format!("Could not save settings: {e}");
        }
    }

    // -----------------------------------------------------------------------
    // Views
    // -----------------------------------------------------------------------

    /// The rows the table is currently showing.
    pub fn rows(&self) -> Vec<Row> {
        match &self.filter {
            Some(needle) => model::rows_for_filter(&self.entries, needle),
            None => model::rows_for(&self.entries, &self.cwd),
        }
    }

    pub fn entry(&self, path: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.path == path)
    }

    pub fn selected_entries(&self) -> Vec<&Entry> {
        self.selection
            .iter()
            .filter_map(|p| self.entry(p))
            .collect()
    }

    /// The row under the cursor, if any.
    pub fn focused_path(&self, rows: &[Row]) -> Option<String> {
        rows.get(self.cursor).map(|r| r.path.clone())
    }

    pub fn has_archive(&self) -> bool {
        self.archive_path.is_some()
    }

    // -----------------------------------------------------------------------
    // Staging — P4 §1
    // -----------------------------------------------------------------------

    /// Close whatever popup is open, and take the typed password with it.
    ///
    /// Every close path routes through here. CORE §9 says passwords are "typed per use,
    /// wiped after", and before P4 an `Esc` cleared the popup and the parked action but
    /// left the plaintext sitting on the window until the next prompt overwrote it.
    pub fn close_popup(&mut self) {
        self.popup = None;
        self.pending = None;
        self.password_input.clear();
        self.password_confirm.clear();
        self.password_attempts = 0;
    }

    /// Can anything be staged against what is currently open?
    ///
    /// P4 §1's two refusals, asked before the user invests in a queue rather than after.
    /// The engine checks both again at Apply — this is the courtesy, that is the guard.
    pub fn staging_refusal(&self) -> Option<String> {
        let path = self.archive_path.as_ref()?;
        if self.tasks.creation().is_some() {
            return None;
        }
        let info = self.archive_info.as_ref()?;
        let encrypted = self.entries.iter().any(|e| e.encrypted);
        match tasks::Recipe::from_info(info, path, encrypted) {
            None => Some(tasks::Conflict::FormatCannotBeWritten(info.format.clone()).to_string()),
            Some(recipe) if encrypted && !recipe.encrypt => {
                Some(tasks::Conflict::EncryptedSourceCannotBeRewritten.to_string())
            }
            Some(_) => None,
        }
    }

    /// Push a task, or say why it cannot be pushed.
    fn stage(&mut self, task: Task) {
        if let Some(refusal) = self.staging_refusal() {
            self.status = refusal;
            return;
        }
        if self.staged_against.is_empty() {
            self.staged_against = self.entries.iter().map(|e| e.path.clone()).collect();
        }
        self.status = task.summary();
        self.tasks.push(task);
    }

    /// `Del` — stage a remove for the selection, or for the row under the cursor.
    pub fn stage_remove(&mut self, rows: &[Row]) {
        if !self.has_archive() {
            return;
        }
        let subjects = self.subject_paths(rows);
        if subjects.is_empty() {
            return;
        }
        for path in subjects {
            self.stage(Task::Remove { path });
        }
    }

    /// `F2` — begin editing a name in the table. CORE §4 fixes the popup count at seven,
    /// so this is not an eighth popup; it is the Name cell becoming a text field.
    pub fn begin_rename(&mut self, rows: &[Row]) {
        if !self.has_archive() {
            return;
        }
        if let Some(refusal) = self.staging_refusal() {
            self.status = refusal;
            return;
        }
        if let Some(row) = rows.get(self.cursor) {
            self.rename_target = Some(row.path.clone());
            self.rename_input = crate::util::base_name(&row.path).to_string();
        }
    }

    /// Commit the in-place rename. The last component only — moving an entry between
    /// directories opens a family of questions v0.4 deliberately does not.
    pub fn commit_rename(&mut self) {
        let Some(from) = self.rename_target.take() else {
            return;
        };
        let name = self.rename_input.trim().to_string();
        self.rename_input.clear();
        if name.is_empty() || name.contains('/') {
            self.status = "A name cannot be empty or contain a slash.".to_string();
            return;
        }
        let parent = crate::util::parent_dir(&from);
        let to = if parent.is_empty() {
            name
        } else {
            format!("{parent}/{name}")
        };
        if to == from {
            return;
        }
        self.stage(Task::Rename { from, to });
    }

    /// `N` — seed and open the New Archive popup.
    pub fn open_new_archive(&mut self) {
        self.new_preset = tasks::Preset::Balanced;
        let (method, encrypt) = self.new_preset.recipe_parts();
        self.new_method = method;
        self.new_level = method.default_level();
        self.new_encrypt = encrypt;
        self.new_advanced = false;
        self.new_dir = self.default_extract_dir().to_string_lossy().to_string();
        self.new_name = "archive".to_string();
        self.popup = Some(Popup::NewArchive);
    }

    /// The container a rebuild would produce, for the metadata notes.
    pub fn staging_container(&self) -> Option<tasks::Container> {
        self.current_recipe().map(|r| r.container())
    }

    /// The recipe a rebuild would use: the one `Task::Create` staged, or the one the open
    /// archive implies.
    pub fn current_recipe(&self) -> Option<tasks::Recipe> {
        if let Some(recipe) = self.tasks.creation() {
            return Some(recipe.clone());
        }
        let path = self.archive_path.as_ref()?;
        let info = self.archive_info.as_ref()?;
        let encrypted = self.entries.iter().any(|e| e.encrypted);
        tasks::Recipe::from_info(info, path, encrypted)
    }

    /// Remove one row from the queue.
    ///
    /// A queue is validated as a sequence, so deleting the middle of it can orphan what
    /// came after — a rename of a name an earlier task produced, say. Re-folding after
    /// each removal is what catches that, and the orphans are dropped with a word rather
    /// than left to fail at Apply.
    pub fn remove_task(&mut self, index: usize) {
        self.tasks.remove(index);
        let before = self.tasks.len();
        self.tasks.retain_foldable(&self.entries);
        let dropped = before - self.tasks.len();
        if dropped > 0 {
            self.status = format!("Removed that change, and {dropped} that depended on it.",);
        }
        if self.tasks.is_empty() {
            self.staged_against.clear();
        }
    }

    /// **Apply.** Everything is settled here and then handed to a worker.
    pub fn request_apply(&mut self, ctx: &egui::Context) {
        if self.tasks.is_empty() {
            return;
        }
        let Some(recipe) = self.current_recipe() else {
            self.status = "This archive's format cannot be written.".to_string();
            return;
        };

        // A password is needed when the source is encrypted, or when the archive being
        // built is. CORE §9 keeps it out of the popup and asks at the moment of use.
        let needs_source = self.entries.iter().any(|e| e.encrypted);
        if (needs_source || recipe.encrypt) && self.passphrase.is_none() {
            self.pending = Some(PendingAction::Apply);
            self.popup = Some(Popup::Password);
            self.password_input.clear();
            self.password_confirm.clear();
            self.password_attempts = 0;
            return;
        }

        self.begin_apply(ctx, recipe);
    }

    /// Spawn the rebuild.
    pub fn begin_apply(&mut self, ctx: &egui::Context, recipe: tasks::Recipe) {
        self.cancel.store(true, Ordering::Relaxed);
        self.cancel = Arc::new(AtomicBool::new(false));

        let (tx, rx) = channel();
        self.apply_rx = Some(rx);
        self.progress = Some(Progress {
            done: 0,
            total: self.tasks.len(),
            label: tasks::Phase::Building.label().to_string(),
        });

        let encrypt = recipe.encrypt;
        let input = tasks::ApplyInput {
            target: recipe.path.clone(),
            recipe,
            tasks: self.tasks.tasks().to_vec(),
            adds: Vec::new(),
            staged_against: self.staged_against.clone(),
            source_password: self.passphrase.clone(),
            target_password: if encrypt {
                self.passphrase.clone()
            } else {
                None
            },
        };

        let cancel = Arc::clone(&self.cancel);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            if let Err(e) = tasks::apply(&input, &tx, &cancel) {
                let _ = tx.send(ApplyMsg::Failed(e));
            }
            // Reactive mode: the worker is what wakes the UI.
            ctx.request_repaint();
        });
    }

    /// Stage an add for every path given, landing each at the current directory.
    pub fn stage_adds(&mut self, paths: Vec<PathBuf>) {
        if paths.is_empty() {
            return;
        }
        for path in paths {
            let Some(name) = path.file_name().map(|n| n.to_string_lossy().to_string()) else {
                continue;
            };
            let dest = if self.cwd.is_empty() {
                name
            } else {
                format!("{}/{}", self.cwd, name)
            };
            self.stage(Task::Add { source: path, dest });
        }
    }

    /// `Ctrl+V` — read the clipboard on a worker and stage what comes back.
    ///
    /// The read hands a pipe to whichever program owns the selection and blocks until it
    /// finishes writing, with no timeout available. A slow or wedged source would freeze
    /// the window, so it does not happen here.
    pub fn request_paste(&mut self, ctx: &egui::Context) {
        if self.paste_rx.is_some() {
            return;
        }
        let (tx, rx) = channel();
        self.paste_rx = Some(rx);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(clipboard::paste_paths());
            ctx.request_repaint();
        });
    }

    /// Throw the queue away. The archive was never touched, so there is nothing to undo.
    pub fn discard_tasks(&mut self) {
        let n = self.tasks.len();
        self.tasks.clear();
        self.staged_against.clear();
        if n > 0 {
            self.status = format!(
                "Discarded {n} staged change{}.",
                if n == 1 { "" } else { "s" }
            );
        }
    }

    // -----------------------------------------------------------------------
    // Actions
    // -----------------------------------------------------------------------

    pub fn descend(&mut self, rows: &[Row]) {
        let Some(row) = rows.get(self.cursor) else {
            return;
        };
        if row.is_dir {
            self.cwd = row.path.clone();
            self.cursor = 0;
            self.filter = None;
        }
    }

    pub fn ascend(&mut self) {
        if let Some(parent) = model::parent_of(&self.cwd) {
            self.cwd = parent;
            self.cursor = 0;
        }
    }

    pub fn begin_extract(&mut self, ctx: &egui::Context, dest: PathBuf) {
        let Some(archive) = self.archive_path.clone() else {
            return;
        };

        // Nothing selected means the whole archive — the obvious reading of "Extract"
        // with no selection, and what every other archiver does.
        let wanted: std::collections::HashSet<String> = if self.selection.is_empty() {
            self.entries.iter().map(|e| e.path.clone()).collect()
        } else {
            self.selection.iter().cloned().collect()
        };

        if let Err(e) = std::fs::create_dir_all(&dest) {
            self.status = format!("Could not create {}: {e}", dest.display());
            return;
        }

        self.cancel = Arc::new(AtomicBool::new(false));
        let (tx, rx) = channel();
        self.extract_rx = Some(rx);
        self.progress = Some(Progress {
            done: 0,
            total: wanted.len(),
            label: "Extracting".to_string(),
        });

        let cancel = Arc::clone(&self.cancel);
        let ctx2 = ctx.clone();
        let pass = self.passphrase.clone();
        let dest2 = dest.clone();

        std::thread::spawn(move || {
            match arch::extract(&archive, &wanted, &dest2, pass.as_ref(), Some(&tx), &cancel) {
                Ok(written) => {
                    let _ = tx.send(ExtractMsg::Done { written });
                }
                Err(e) => {
                    let _ = tx.send(ExtractMsg::Failed(e.to_string()));
                }
            }
            ctx2.request_repaint();
        });

        self.status = format!("Extracting to {}…", dest.display());
    }

    /// Extraction needs a password when the selection contains encrypted entries and
    /// we do not already hold one. P2 §5: this is knowable **before starting**.
    pub fn extraction_needs_password(&self) -> bool {
        if self.passphrase.is_some() {
            return false;
        }
        let selected: Vec<&Entry> = if self.selection.is_empty() {
            self.entries.iter().collect()
        } else {
            self.selected_entries()
        };
        selected.iter().any(|e| e.encrypted)
    }

    pub fn request_extract(&mut self, ctx: &egui::Context, dest: PathBuf) {
        if self.extraction_needs_password() {
            self.pending = Some(PendingAction::Extract { dest });
            self.popup = Some(Popup::Password);
            self.password_input.clear();
            self.password_attempts = 0;
            return;
        }
        self.popup = None;
        self.begin_extract(ctx, dest);
    }

    pub fn compute_crc(&mut self, entry_path: &str) {
        let Some(archive) = self.archive_path.clone() else {
            return;
        };
        match arch::crc32_of(&archive, entry_path, self.passphrase.as_ref()) {
            Ok(v) => self.crc_of = Some((entry_path.to_string(), v)),
            Err(ArchiveError::WrongPassword) | Err(ArchiveError::NeedPassword) => {
                // The CRC action on an encrypted entry rides the same prompt (P2 §5).
                self.pending = Some(PendingAction::Crc {
                    entry: entry_path.to_string(),
                });
                self.popup = Some(Popup::Password);
                self.password_input.clear();
                self.password_attempts = 0;
            }
            Err(e) => self.status = e.to_string(),
        }
    }

    // -----------------------------------------------------------------------
    // P3: out into the system
    // -----------------------------------------------------------------------

    /// What `Ctrl+C` and Open With operate on: the selection, or the focused row when
    /// nothing is selected.
    fn subject_paths(&self, rows: &[Row]) -> Vec<String> {
        if !self.selection.is_empty() {
            return self.selection.iter().cloned().collect();
        }
        rows.get(self.cursor)
            .map(|r| r.path.clone())
            .into_iter()
            .collect()
    }

    /// Total uncompressed size of a set of entries, including anything beneath a
    /// selected directory — this is what decides RAM versus disk (P3 §1).
    fn uncompressed_total(&self, wanted: &std::collections::HashSet<String>) -> u64 {
        self.entries
            .iter()
            .filter(|e| arch::selection_matches(&e.path, wanted))
            .map(|e| e.size)
            .sum()
    }

    fn any_encrypted(&self, wanted: &std::collections::HashSet<String>) -> bool {
        self.entries
            .iter()
            .filter(|e| arch::selection_matches(&e.path, wanted))
            .any(|e| e.encrypted)
    }

    /// `Ctrl+C` — extract to scratch, then offer `file://` URIs on the clipboard.
    ///
    /// P3 §2: "it must feel better than drag, not apologetic about it."
    pub fn copy_out(&mut self, rows: &[Row]) {
        let paths = self.subject_paths(rows);
        if paths.is_empty() {
            self.status = "Nothing selected.".to_string();
            return;
        }
        let wanted: std::collections::HashSet<String> = paths.into_iter().collect();

        if self.passphrase.is_none() && self.any_encrypted(&wanted) {
            self.pending = Some(PendingAction::CopyOut);
            self.popup = Some(Popup::Password);
            self.password_input.clear();
            self.password_attempts = 0;
            return;
        }

        let Some(archive) = self.archive_path.clone() else {
            return;
        };
        let total = self.uncompressed_total(&wanted);

        let placement = match self.scratch.begin(scratch::Kind::CopyOut, total) {
            Ok(p) => p,
            Err(e) => {
                self.status = format!("Could not make a scratch directory: {e}");
                return;
            }
        };

        let cancel = Arc::new(AtomicBool::new(false));
        if let Err(e) = arch::extract(
            &archive,
            &wanted,
            &placement.dir,
            self.passphrase.as_ref(),
            None,
            &cancel,
        ) {
            self.status = e.to_string();
            return;
        }

        let files = collect_files(&placement.dir);
        if files.is_empty() {
            self.status = "Nothing to copy.".to_string();
            return;
        }

        match clipboard::offer(&files) {
            Ok(()) => {
                let n = files.len();
                let mut msg = format!(
                    "{n} {} ready to paste.",
                    if n == 1 { "file" } else { "files" }
                );
                if placement.on_disk {
                    // P3 §1's one-line notice.
                    msg.push_str(" Over 1 GiB — staged on disk rather than in RAM.");
                }
                self.status = msg;
            }
            Err(e) => self.status = e,
        }
    }

    /// `Enter` on a file — extract that one entry, then offer the picker.
    pub fn open_with(&mut self, entry_path: &str) {
        let Some(entry) = self.entry(entry_path).cloned() else {
            return;
        };
        if entry.is_dir {
            return;
        }

        if self.passphrase.is_none() && entry.encrypted {
            self.pending = Some(PendingAction::OpenWith {
                entry: entry_path.to_string(),
            });
            self.popup = Some(Popup::Password);
            self.password_input.clear();
            self.password_attempts = 0;
            return;
        }

        let Some(archive) = self.archive_path.clone() else {
            return;
        };
        let wanted: std::collections::HashSet<String> =
            std::iter::once(entry.path.clone()).collect();

        let placement = match self.scratch.begin(scratch::Kind::OpenWith, entry.size) {
            Ok(p) => p,
            Err(e) => {
                self.status = format!("Could not make a scratch directory: {e}");
                return;
            }
        };

        let cancel = Arc::new(AtomicBool::new(false));
        if let Err(e) = arch::extract(
            &archive,
            &wanted,
            &placement.dir,
            self.passphrase.as_ref(),
            None,
            &cancel,
        ) {
            self.status = e.to_string();
            return;
        }

        let extracted = placement.dir.join(&entry.raw_path);
        if !extracted.is_file() {
            self.status = "The entry did not extract to a file.".to_string();
            return;
        }

        let name = crate::util::base_name(&entry.path).to_string();
        let mime = apps::mime_for(&name).to_string();
        let installed = apps::scan(&apps::application_dirs());
        let defaults = std::fs::read_to_string(apps::mimeapps_path())
            .map(|t| apps::parse_mimeapps(&t))
            .unwrap_or_default();

        self.openwith_candidates = apps::rank(&installed, &mime, &defaults);
        self.openwith_path = Some(extracted);
        self.openwith_name = name;
        self.openwith_mime = mime;
        self.openwith_filter.clear();
        self.openwith_show_all = false;
        self.popup = Some(Popup::OpenWith);
    }

    pub fn default_extract_dir(&self) -> PathBuf {
        let Some(archive) = &self.archive_path else {
            return crate::platform::home();
        };
        let parent = archive
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .to_path_buf();
        if self.extract_to_subdir {
            parent.join(archive_stem(archive))
        } else {
            parent
        }
    }
}

/// Every regular file beneath `dir`, sorted, so the clipboard offer is stable.
fn collect_files(dir: &std::path::Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            match e.file_type() {
                Ok(t) if t.is_dir() => stack.push(p),
                Ok(t) if t.is_file() || t.is_symlink() => out.push(p),
                _ => {}
            }
        }
    }
    out.sort();
    out
}

/// `photos-2026.tar.gz` -> `photos-2026`. Strips every extension, because a
/// double-extension archive should not extract into `photos-2026.tar`.
pub fn archive_stem(path: &std::path::Path) -> String {
    let name = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "archive".to_string());
    match name.split_once('.') {
        Some((stem, _)) if !stem.is_empty() => stem.to_string(),
        _ => name,
    }
}

// ---------------------------------------------------------------------------
// The eframe App
// ---------------------------------------------------------------------------

impl eframe::App for Indium {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        let c = theme::WINDOW;
        [
            c.r() as f32 / 255.0,
            c.g() as f32 / 255.0,
            c.b() as f32 / 255.0,
            1.0,
        ]
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        self.drain_worker(&ctx);
        self.take_dropped_files(&ctx);

        let rows = self.rows();
        self.handle_keys(&ctx, &rows);

        // Panels are shown into the root `Ui`; the order fixes the layout, with the
        // table taking whatever the four edges leave behind.
        sidebar::show(self, ui);
        status_bar(self, ui);
        tray::show(self, ui);
        inspector::show(self, ui, &rows);
        table::show(self, ui, &rows);

        // Popups take the context, and come last so they draw over every zone.
        extract::show(self, &ctx);
        settings::show(self, &ctx);
        about::show(self, &ctx);
        open_path_popup(self, &ctx);
        newarchive::show(self, &ctx);
        pending::show(self, &ctx);
        password::show(self, &ctx);
        openwith::show(self, &ctx);
    }
}

impl Indium {
    fn take_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .collect()
        });
        if dropped.is_empty() {
            return;
        }
        // Before P4 this took `.next()` and silently discarded every other path. With an
        // archive open, CORE §4 says a drop stages an add — of all of them.
        if self.has_archive() && self.section == Section::Archive {
            self.stage_adds(dropped);
        } else if let Some(path) = dropped.into_iter().next() {
            self.open_archive(ctx, path, None);
        }
    }

    /// CORE §4's keyboard table. Bare letters are shortcuts, so every one of them is
    /// suppressed while a text field has focus — which is also why there is
    /// deliberately no type-to-jump in the table.
    fn handle_keys(&mut self, ctx: &egui::Context, rows: &[Row]) {
        let typing = ctx.memory(|m| m.focused().is_some());

        // `Esc` has a fixed priority, set by P2 §4 "for good": a focused filter bar
        // clears and closes first; then the topmost popup; then nothing.
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.filter.is_some() {
                self.filter = None;
                self.filter_focus_requested = false;
                ctx.memory_mut(|m| m.request_focus(egui::Id::NULL));
                return;
            }
            if self.rename_target.is_some() {
                self.rename_target = None;
                self.rename_input.clear();
                ctx.memory_mut(|m| m.request_focus(egui::Id::NULL));
                return;
            }
            if self.popup.is_some() {
                self.close_popup();
                return;
            }
        }

        // Ctrl chords work even while typing.
        let (ctrl_f, ctrl_a, ctrl_o, ctrl_c, ctrl_v) = ctx.input(|i| {
            (
                i.modifiers.ctrl && i.key_pressed(egui::Key::F),
                i.modifiers.ctrl && i.key_pressed(egui::Key::A),
                i.modifiers.ctrl && i.key_pressed(egui::Key::O),
                i.modifiers.ctrl && i.key_pressed(egui::Key::C),
                i.modifiers.ctrl && i.key_pressed(egui::Key::V),
            )
        });

        if ctrl_f && self.has_archive() {
            self.filter = Some(String::new());
            self.filter_focus_requested = true;
            self.cursor = 0;
        }
        if ctrl_o {
            self.popup = Some(Popup::OpenPath);
        }
        if ctrl_a && self.has_archive() && !typing {
            self.selection = rows.iter().map(|r| r.path.clone()).collect();
        }
        if ctrl_c && self.has_archive() {
            self.copy_out(rows);
        }
        // Guarded on `typing`, unlike the others: a paste into the Extract path field or
        // the rename box must reach the field, not stage an add.
        if ctrl_v && self.has_archive() && !typing {
            self.request_paste(ctx);
        }

        // CORE §4's bare letters are shortcuts, so they must not fire into a popup that
        // happens to hold no focused text field. About and Settings never focus anything,
        // and P4's two new popups are full of chips, rows and a slider that hold nothing
        // either — without this guard, pressing `E` inside New Archive would silently
        // swap it for the Extract popover.
        if typing || self.popup.is_some() || self.rename_target.is_some() {
            return;
        }

        // Set inside the input closure and acted on after it, because seeding the New
        // Archive popup needs `&mut self` methods the closure cannot hold.
        let mut new_archive = false;

        ctx.input(|i| {
            for ev in &i.events {
                let egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } = ev
                else {
                    continue;
                };
                if modifiers.ctrl || modifiers.alt || modifiers.command {
                    continue;
                }
                match key {
                    egui::Key::Num1 => self.section = Section::Recents,
                    egui::Key::Num2 => self.section = Section::Bookmarks,
                    egui::Key::Num3 => self.section = Section::Archive,
                    egui::Key::A => self.popup = Some(Popup::About),
                    egui::Key::Comma => self.popup = Some(Popup::Settings),
                    egui::Key::E => {
                        if self.has_archive() {
                            self.extract_path =
                                self.default_extract_dir().to_string_lossy().to_string();
                            self.popup = Some(Popup::Extract);
                        }
                    }
                    egui::Key::N => new_archive = true,
                    egui::Key::W => self.popup = Some(Popup::PendingTasks),
                    egui::Key::Space => {
                        self.inspector_tab = match self.inspector_tab {
                            InspectorTab::Details => InspectorTab::Preview,
                            InspectorTab::Preview => InspectorTab::Details,
                        };
                    }
                    egui::Key::Enter => {
                        // Descend is handled outside this closure so it can mutate.
                    }
                    _ => {}
                }
            }
        });

        if new_archive {
            self.open_new_archive();
        }

        // Movement and descent, which need `rows`.
        let (up, down, pgup, pgdn, home, end, enter, back, del, f2) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::ArrowUp),
                i.key_pressed(egui::Key::ArrowDown),
                i.key_pressed(egui::Key::PageUp),
                i.key_pressed(egui::Key::PageDown),
                i.key_pressed(egui::Key::Home),
                i.key_pressed(egui::Key::End),
                i.key_pressed(egui::Key::Enter),
                i.key_pressed(egui::Key::Backspace),
                i.key_pressed(egui::Key::Delete),
                i.key_pressed(egui::Key::F2),
            )
        });

        let len = rows.len();
        let moved = up || down || pgup || pgdn || home || end;
        if len > 0 {
            let step = 12usize;
            if up {
                self.cursor = self.cursor.saturating_sub(1);
            }
            if down {
                self.cursor = (self.cursor + 1).min(len - 1);
            }
            if pgup {
                self.cursor = self.cursor.saturating_sub(step);
            }
            if pgdn {
                self.cursor = (self.cursor + step).min(len - 1);
            }
            if home {
                self.cursor = 0;
            }
            if end {
                self.cursor = len - 1;
            }
            self.cursor = self.cursor.min(len - 1);

            // Moving the cursor selects what it lands on, which is what makes
            // "Inspector updates on arrow-key movement" (P1's manual checklist) true.
            // A checksum belongs to the entry it was computed for, so it is dropped.
            if moved && self.section == Section::Archive {
                self.crc_of = None;
                self.selection.clear();
                if let Some(row) = rows.get(self.cursor) {
                    self.selection.insert(row.path.clone());
                }
            } else if moved {
                self.crc_of = None;
            }
        } else {
            self.cursor = 0;
        }

        match self.section {
            Section::Archive => {
                if enter {
                    match rows.get(self.cursor) {
                        Some(row) if row.is_dir => self.descend(rows),
                        Some(row) => {
                            let path = row.path.clone();
                            self.open_with(&path);
                        }
                        None => {}
                    }
                }
                if back {
                    self.ascend();
                }
                if del {
                    self.stage_remove(rows);
                }
                if f2 {
                    self.begin_rename(rows);
                }
            }
            Section::Recents => {
                if enter {
                    if let Some(r) = self.recents.sorted().get(self.cursor).cloned() {
                        let path = PathBuf::from(&r.path);
                        if path.exists() {
                            self.open_archive(ctx, path, None);
                        } else {
                            self.status = format!("{} is no longer there.", r.path);
                        }
                    }
                }
                if del {
                    if let Some(r) = self.recents.sorted().get(self.cursor).cloned() {
                        let path = r.path.clone();
                        self.recents.remove(&path);
                        if !self.recents_broken {
                            let _ = self.store.save_recents(&self.recents);
                        }
                        self.status = format!("Removed {path} from recent files.");
                    }
                }
            }
            Section::Bookmarks => {
                if del && self.cursor < self.settings.bookmarks.len() {
                    let b = self.settings.bookmarks.remove(self.cursor);
                    self.save_settings();
                    self.status = format!("Removed bookmark {}.", b.name);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Status bar — CORE §4's fifth zone
// ---------------------------------------------------------------------------

fn status_bar(app: &mut Indium, ui: &mut egui::Ui) {
    egui::Panel::bottom("status")
        .frame(
            egui::Frame::NONE
                .fill(theme::STATUS_BAR)
                .inner_margin(egui::Margin::symmetric(10, 5)),
        )
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                if app.has_archive() {
                    let agg = model::aggregate(app.entries.iter());
                    ui.label(
                        egui::RichText::new(format!("{} entries", agg.count))
                            .family(theme::MONO)
                            .color(theme::TEXT_SECONDARY),
                    );
                    ui.label(egui::RichText::new("·").color(theme::TEXT_MUTED));

                    // Archive-level real -> packed, which is honest: the packed side
                    // is the archive's own size on disk.
                    ui.label(
                        egui::RichText::new(format!(
                            "{} -> {} ({})",
                            crate::util::format_bytes(agg.total_real),
                            crate::util::format_bytes(app.archive_bytes),
                            crate::util::format_ratio(agg.total_real, app.archive_bytes),
                        ))
                        .family(theme::MONO)
                        .color(theme::TEXT_SECONDARY),
                    );

                    if let Some(info) = &app.archive_info {
                        ui.label(egui::RichText::new("·").color(theme::TEXT_MUTED));
                        ui.label(
                            egui::RichText::new(&info.format)
                                .family(theme::MONO)
                                .color(theme::TEXT),
                        );
                        if !info.filter.is_empty() && info.filter != "none" {
                            ui.label(
                                egui::RichText::new(&info.filter)
                                    .family(theme::MONO)
                                    .color(theme::TEXT_MUTED),
                            );
                        }
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(p) = &app.progress {
                        if ui.button("Cancel").clicked() {
                            app.cancel.store(true, Ordering::Relaxed);
                        }
                        let frac = if p.total == 0 {
                            0.0
                        } else {
                            p.done as f32 / p.total as f32
                        };
                        ui.add(
                            egui::ProgressBar::new(frac)
                                .desired_width(160.0)
                                .fill(theme::ORANGE)
                                .text(format!("{} {}/{}", p.label, p.done, p.total)),
                        );
                    } else if app.listing {
                        ui.add(egui::Spinner::new().color(theme::ORANGE));
                        ui.label(egui::RichText::new("Reading…").color(theme::TEXT_SECONDARY));
                    } else {
                        ui.label(egui::RichText::new(&app.status).color(theme::TEXT_MUTED));
                    }
                });
            });
        });
}

// ---------------------------------------------------------------------------
// Ctrl+O — the path field
// ---------------------------------------------------------------------------

fn open_path_popup(app: &mut Indium, ctx: &egui::Context) {
    if app.popup != Some(Popup::OpenPath) {
        return;
    }
    let mut open = true;
    egui::Window::new("Open archive")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .open(&mut open)
        .show(ctx, |ui| {
            ui.set_min_width(460.0);
            ui.label(egui::RichText::new("Path to an archive").color(theme::TEXT_SECONDARY));
            let resp = ui.add(
                egui::TextEdit::singleline(&mut app.open_path)
                    .font(egui::TextStyle::Monospace)
                    .desired_width(f32::INFINITY),
            );
            resp.request_focus();

            if let Some(completed) = extract::complete_path(&app.open_path) {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Tab ->").color(theme::TEXT_MUTED));
                    ui.label(
                        egui::RichText::new(&completed)
                            .family(theme::MONO)
                            .color(theme::TEXT_MUTED),
                    );
                });
                if ui.input(|i| i.key_pressed(egui::Key::Tab)) {
                    app.open_path = completed;
                }
            }

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let go =
                    ui.button("Open").clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter));
                if go && !app.open_path.trim().is_empty() {
                    let path = PathBuf::from(app.open_path.trim());
                    if path.is_file() {
                        app.popup = None;
                        app.open_archive(ctx, path, None);
                    } else {
                        app.status = format!("{} is not a file.", path.display());
                    }
                }
            });
        });
    if !open {
        app.popup = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_stem_strips_every_extension() {
        assert_eq!(
            archive_stem(std::path::Path::new("/x/photos-2026.tar.gz")),
            "photos-2026"
        );
        assert_eq!(archive_stem(std::path::Path::new("/x/backup.7z")), "backup");
        assert_eq!(archive_stem(std::path::Path::new("/x/noext")), "noext");
    }

    #[test]
    fn a_dotfile_archive_keeps_its_name() {
        // ".hidden.zip" must not stem to the empty string.
        assert_eq!(
            archive_stem(std::path::Path::new("/x/.hidden.zip")),
            ".hidden.zip"
        );
    }
}
