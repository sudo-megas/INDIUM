//! The window: sidebar, table, Inspector, status bar, and every popup.
//!
//! CORE §4: "Five fixed zones and nine popups. Nothing else appears, ever." P2 §5 added
//! the password prompt by the maker's ordered CORE edit; P4 fills in the two the count
//! always allowed for — New Archive and Pending tasks — and puts rename in the table
//! rather than making it another. P12 numbers the two §4 had been running without: Open,
//! which the keyboard table has carried since P1, and Keys.

pub mod about;
pub mod extract;
pub mod filter;
pub mod inspector;
pub mod keys;
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
use crate::platform::picker::{self, PickerFor};
use crate::platform::scratch::{self, Scratch};
use crate::platform::store::{self, ExtractDefault, Recents, Settings, Store};
use crate::platform::window::{self, Destination};
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

/// The popups — nine of them, and this is all of them: rename happens in the table
/// rather than in another popup.
///
/// CORE §4's numbered list once stopped at seven and did not carry `OpenPath`, while §4's
/// keyboard table had carried `Ctrl+O` since P1 and the window behind it had been a real
/// `egui::Window` for just as long: the document ordered the mechanism and forgot to
/// number the window it opens. This comment used to claim seven and count eight, which was
/// the wrong half to leave standing. P12 applied the edit `build/docs/P6.md` ordered, so
/// Open is numbered eighth, and added `Keys` as the ninth — the list is now the same length
/// in both places.
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
    Keys,
}

/// What INDIUM is currently saying, and whether it is bad news.
///
/// CORE §4: *"A failure is `#FFD800`. What INDIUM says is the only text in the window that
/// reports both triumph and disaster in the same place, and until now it reported them in
/// the same colour."* It did, for eleven milestones: `Removed bookmark photos.` and
/// `Could not create /home/megas/x: permission denied` were the same string in the same
/// grey, and a person watching the bar could not tell which had happened.
///
/// The severity rides **with** the sentence rather than beside it, in a separate flag,
/// because a separate flag is a thing you can forget to clear — set it on a failure, write
/// a success over the text a moment later, and the success is yellow. Carrying it in the
/// value makes that unrepresentable: every plain assignment goes through `From`, which sets
/// `bad: false`, so saying anything at all clears the last failure by construction.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Status {
    pub text: String,
    /// Drawn in [`theme::WARNING`] rather than the ordinary grey.
    pub bad: bool,
}

impl Status {
    /// Something went wrong. A refusal counts; a confirmation does not.
    pub fn bad(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            bad: true,
        }
    }
}

impl From<String> for Status {
    fn from(text: String) -> Self {
        Self { text, bad: false }
    }
}

impl From<&str> for Status {
    fn from(text: &str) -> Self {
        Self {
            text: text.to_string(),
            bad: false,
        }
    }
}

/// What the file picker came back with, and what it had been opened for.
///
/// The two travel together because the answer arrives on a channel a long time after the
/// button was clicked, and by then nothing else remembers which button it was.
type PickedFiles = (PickerFor, Result<Vec<PathBuf>, String>);

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
    /// Read one entry's head for the Preview tab.
    Preview { entry: String },
    /// Rebuild the archive once a password has been given.
    Apply,
}

/// What becomes of an extraction's output once the worker reports `Done`.
///
/// P3's two outward paths ran `arch::extract` inline on the UI thread until P6: the window
/// froze for as long as libarchive took, the progress bar never moved, and the status bar
/// drew a Cancel the UI thread was too busy inside the extraction to ever notice. They
/// spawn the same worker `Extract` does now, and everything that used to follow the call
/// follows the message instead.
enum PostExtract {
    /// A plain `E` extraction. The status line is the whole of it.
    None,
    /// `Ctrl+C` — offer what came out on the clipboard. `on_disk` is `Placement`'s answer,
    /// carried because P3 §1's one-line notice is owed at the end, not at the start.
    Clipboard { on_disk: bool },
    /// `Enter` on a file — find what came out and open the picker on it. Boxed for the
    /// same reason `ListMsg::Entry` is: an `Entry` dwarfs the other two variants.
    OpenWith { entry: Box<Entry> },
}

/// How much of an entry Preview will read.
///
/// Large enough that ordinary images and text files fit whole, small enough that a
/// gigabyte member cannot make the window disappear. An archive is untrusted input, and
/// a preview is a convenience — it does not get to spend unbounded memory.
pub const PREVIEW_CAP: usize = 8 * 1024 * 1024;

/// What the Preview tab is showing, and for which entry.
///
/// Keyed by path and checked before use, exactly as `crc_of` is: a value belonging to a
/// different entry is discarded rather than shown against the wrong name.
pub struct PreviewData {
    pub path: String,
    pub content: crate::util::Content,
    pub bytes: Vec<u8>,
    /// True when the entry was longer than `PREVIEW_CAP`. An image cannot be decoded from
    /// a truncated head, so this is what stops Preview handing a half a PNG to a decoder.
    pub truncated: bool,
    /// A stable key for egui's texture cache, unique per archive and member.
    pub uri: String,
}

/// One entry's head, or why it could not be read. Named because clippy is right that the
/// tuple was getting hard to read at the two places it appears.
type PreviewRead = (String, Result<(Vec<u8>, bool), String>);

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
    /// The row cursor **in the archive table**, and only there.
    ///
    /// Until P11 this one field was the cursor for all three sections at once, which was
    /// wrong twice over. It was clamped every frame against the archive's row count
    /// whatever section was on screen — so with a two-row archive open, a third bookmark
    /// could be clicked but never stayed lit, and `Enter` on the fourth recent opened
    /// whatever the cursor had been dragged back down to. And switching section threw the
    /// position away, because one number cannot remember three places.
    pub cursor: usize,
    /// The row cursor in *Recent files*. See [`Indium::cursor`].
    pub recents_cursor: usize,
    /// The row cursor in *Bookmarks*. See [`Indium::cursor`].
    pub bookmarks_cursor: usize,
    /// Set for one frame when the keyboard moved the archive cursor, so the table can
    /// scroll the cursor back into view.
    ///
    /// CORE §6: the cursor "is also **kept on screen** — a row scrolled out of view is a
    /// cursor nobody can see, by a different route." It is a one-shot flag rather than a
    /// standing "always show the cursor", because the latter would yank the view back every
    /// time somebody scrolled away from it with the wheel to read something else.
    pub scroll_to_cursor: bool,
    /// Selected archive paths. Kept as paths, not row indices, so a selection
    /// survives descending, filtering and re-listing.
    pub selection: BTreeSet<String>,
    pub inspector_tab: InspectorTab,
    /// `Some` while the filter bar is open, even when empty.
    pub filter: Option<String>,
    pub filter_focus_requested: bool,
    /// The popup whose first field has already been handed focus. See
    /// [`Indium::wants_initial_focus`].
    pub focus_given_to: Option<Popup>,

    // --- staging (P4) -----------------------------------------------------
    /// The queue CORE §3 calls the staging engine. Empty means the tray is hidden.
    pub tasks: Queue,
    /// The normalised paths the queue was staged against, so Apply can refuse if the
    /// archive changed on disk underneath it.
    pub staged_against: Vec<String>,
    /// `Some(path)` while a name is being edited in place. CORE §4 fixes the popup count
    /// at nine, so rename is not a tenth.
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
    /// The portal file picker's answer, and what it was asked for.
    picker_rx: Option<Receiver<PickedFiles>>,
    /// The answer from `platform::open::open_directory`, which only ever needs to be heard
    /// when it is bad — but it does need to be heard. A folder that silently fails to open
    /// is the same defect the second testing round found in extraction: work that reports
    /// success it did not have.
    reveal_rx: Option<Receiver<Result<(), String>>>,

    // --- Preview (P5) -----------------------------------------------------
    pub preview: Option<PreviewData>,
    /// Set while a head is being read, so the tab can say so rather than look empty.
    pub preview_loading: Option<String>,
    preview_rx: Option<Receiver<PreviewRead>>,

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
    /// Which application the keyboard is on. P11: the filter field held focus every
    /// frame, so the list could only ever be reached with the pointer.
    pub openwith_cursor: usize,
    pub openwith_path: Option<PathBuf>,
    pub openwith_name: String,
    pub openwith_mime: String,

    /// Where copies handed to the outside world live. Dropping it removes them.
    pub scratch: Scratch,

    // --- worker -----------------------------------------------------------
    pub cancel: Arc<AtomicBool>,
    list_rx: Option<Receiver<ListMsg>>,
    extract_rx: Option<Receiver<ExtractMsg>>,
    /// Set before the extraction worker is spawned, consumed when it reports.
    post_extract: PostExtract,
    pub progress: Option<Progress>,

    // --- persistence ------------------------------------------------------
    pub store: Store,
    pub settings: Settings,
    pub recents: Recents,
    /// P2 §1: while a file failed to parse, INDIUM must not overwrite it.
    pub settings_broken: bool,
    pub recents_broken: bool,

    // --- chrome -----------------------------------------------------------
    /// What INDIUM is saying, and whether it is a failure. See [`Status`].
    pub status: Status,
    /// The computed CRC of the focused entry, cleared whenever focus moves.
    pub crc_of: Option<(String, u32)>,
    /// Held only for the duration of one operation, then dropped and wiped.
    pub passphrase: Option<Secret>,
}

impl Indium {
    pub fn new(cc: &eframe::CreationContext<'_>, open: Option<PathBuf>) -> Indium {
        theme::install(&cc.egui_ctx);
        // Without this, `Image::from_bytes` has no loader and every preview fails. It is
        // idempotent by contract, and with only the `image` feature it registers exactly
        // one thing.
        egui_extras::install_image_loaders(&cc.egui_ctx);

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
            recents_cursor: 0,
            bookmarks_cursor: 0,
            scroll_to_cursor: false,
            selection: BTreeSet::new(),
            inspector_tab: InspectorTab::Details,
            filter: None,
            filter_focus_requested: false,
            focus_given_to: None,

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
            picker_rx: None,
            reveal_rx: None,
            preview: None,
            preview_loading: None,
            preview_rx: None,
            password_input: String::new(),
            password_confirm: String::new(),
            password_attempts: 0,
            pending: None,
            bookmark_name: String::new(),
            bookmark_path: String::new(),

            openwith_candidates: Vec::new(),
            openwith_filter: String::new(),
            openwith_show_all: false,
            openwith_cursor: 0,
            openwith_path: None,
            openwith_name: String::new(),
            openwith_mime: String::new(),
            scratch: Scratch::new(),

            cancel: Arc::new(AtomicBool::new(false)),
            list_rx: None,
            extract_rx: None,
            post_extract: PostExtract::None,
            progress: None,

            store,
            settings_broken: settings.was_broken,
            recents_broken: recents.was_broken,
            settings: settings.value,
            recents: recents.value,

            status: status.into(),
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

    /// Open an archive — here if this window is free to take it, and in a second window
    /// if it is not.
    ///
    /// This is reached from seven places — the command line, a drop, `Ctrl+O`, a click or
    /// `Enter` on a recent, the password prompt's List resume, and Apply's own re-open —
    /// and all seven ask the same question of `platform::window::destination` because
    /// CORE §1's rule does not have seven readings. P8 §2 is that rule; the call sites
    /// were not touched to add it.
    ///
    /// **The order of the two refusals below is the whole of P8 §2.** Until P8 this
    /// function began with `work_running`, because opening an archive replaced what was
    /// on screen: P7 §7 added the check after a copy-out was cut in half by a user who
    /// clicked a recent while it ran, and the two lines that follow are the rug-pull it
    /// stopped — they raise the old cancellation flag and hand the window a new one.
    ///
    /// A second window replaces nothing. So the destination is asked **first**, and a
    /// window busy extracting will now happily open a different archive next to itself
    /// rather than refuse — the refusal was never about the archive, it was about this
    /// window's own running work, and it still guards exactly that. Asking `work_running`
    /// first would have made the busiest window the one that could not do the one thing
    /// that costs it nothing.
    ///
    /// A *listing* is not work in this sense and is still cancelled without ceremony:
    /// nothing has been written, CORE §1 gives the window one archive, and opening the
    /// next one is the whole of what the user asked for.
    pub fn open_archive(&mut self, ctx: &egui::Context, path: PathBuf, passphrase: Option<Secret>) {
        if window::destination(self.archive_path.as_deref(), &path) == Destination::NewWindow {
            // The secret dies on this line rather than travelling to a command line.
            // No caller reaches here holding one — the two that hold one re-open the
            // archive already open — and `Secret`'s own `Drop` wipes it, so the day a
            // caller does, CORE §9 is kept by the code rather than by the argument.
            drop(passphrase);
            self.status = match window::open_new(&path) {
                Ok(()) => format!(
                    "Opening {} in a second window.",
                    path.file_name()
                        .unwrap_or(path.as_os_str())
                        .to_string_lossy()
                )
                .into(),
                Err(e) => Status::bad(e),
            };
            return;
        }
        if self.work_running() {
            return;
        }
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
        self.set_window_title(ctx);
        self.section = Section::Archive;
        self.listing = true;
        self.status = format!("Reading {}…", path.display()).into();

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
        let picker_msgs: Vec<PickedFiles> = match &self.picker_rx {
            Some(rx) => rx.try_iter().collect(),
            None => Vec::new(),
        };
        let reveal_msgs: Vec<Result<(), String>> = match &self.reveal_rx {
            Some(rx) => rx.try_iter().collect(),
            None => Vec::new(),
        };
        let preview_msgs: Vec<PreviewRead> = match &self.preview_rx {
            Some(rx) => rx.try_iter().collect(),
            None => Vec::new(),
        };
        let wake = !list_msgs.is_empty()
            || !extract_msgs.is_empty()
            || !apply_msgs.is_empty()
            || !paste_msgs.is_empty()
            || !preview_msgs.is_empty();

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
                        .unwrap_or_else(|| "Ready.".to_string())
                        .into();
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
                // `Done` now means done. The worker decides, so nothing here re-reads a
                // flag that may already belong to a later operation.
                ExtractMsg::Done { written } => {
                    self.progress = None;
                    self.extract_rx = None;
                    self.status = format!(
                        "Extracted {written} {}.",
                        if written == 1 { "entry" } else { "entries" }
                    )
                    .into();
                    // The password's job is over. Neither post-step below needs it: one
                    // reads the scratch directory, the other reads `.desktop` files.
                    self.passphrase = None;
                    match std::mem::replace(&mut self.post_extract, PostExtract::None) {
                        PostExtract::None => {}
                        PostExtract::Clipboard { on_disk } => self.finish_copy_out(on_disk),
                        PostExtract::OpenWith { entry } => self.finish_open_with(&entry),
                    }
                }
                ExtractMsg::Cancelled { written } => {
                    self.progress = None;
                    self.extract_rx = None;
                    self.passphrase = None;
                    // The post-step is dropped rather than run. Half a selection is not
                    // what `Ctrl+C` asked for, and half a file is not what Open With would
                    // hand an application — and neither of them can tell, because a
                    // partial extraction leaves whole files on disk beside the missing
                    // ones. What was asked for decides what the sentence can honestly say.
                    let outward = !matches!(self.post_extract, PostExtract::None);
                    self.post_extract = PostExtract::None;
                    let entries = if written == 1 { "entry" } else { "entries" };
                    self.status = if written == 0 {
                        "Cancelled. Nothing was extracted.".to_string()
                    } else if outward {
                        format!("Cancelled after {written} {entries}. Nothing was offered.")
                    } else {
                        format!(
                            "Cancelled after {written} {entries}; \
                             what came out is still in the destination."
                        )
                    }
                    .into();
                }
                ExtractMsg::Failed(msg) => {
                    self.progress = None;
                    self.extract_rx = None;
                    self.post_extract = PostExtract::None;
                    self.status = Status::bad(msg);
                    self.passphrase = None;
                }
            }
        }

        for result in reveal_msgs {
            self.reveal_rx = None;
            // Success says nothing. The window the user asked for is now in front of
            // them, and a status line announcing it would be talking about something they
            // can already see.
            if let Err(e) = result {
                self.status = Status::bad(e);
            }
        }

        for (path, result) in preview_msgs {
            self.preview_rx = None;
            self.preview_loading = None;
            match result {
                Ok((bytes, truncated)) => {
                    let content = crate::util::sniff(&bytes);
                    // The URI keys egui's texture cache. It must be unique per archive and
                    // member, or two different images would share one texture.
                    let uri = format!(
                        "bytes://{}#{}",
                        self.archive_path
                            .as_ref()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_default(),
                        path
                    );
                    self.preview = Some(PreviewData {
                        path,
                        content,
                        bytes,
                        truncated,
                        uri,
                    });
                }
                Err(e) => {
                    self.preview = None;
                    self.status = Status::bad(e);
                }
            }
        }

        for msg in paste_msgs {
            self.paste_rx = None;
            match msg {
                Ok(paths) if paths.is_empty() => {
                    self.status = Status::bad("The clipboard holds no files.");
                }
                Ok(paths) => self.stage_adds(paths),
                Err(e) => self.status = Status::bad(e),
            }
        }

        for (what, msg) in picker_msgs {
            self.picker_rx = None;
            let paths = match msg {
                // A cancelled dialog. The user changed their mind, which is not news.
                Ok(paths) if paths.is_empty() => continue,
                Ok(paths) => paths,
                Err(e) => {
                    self.status = Status::bad(e);
                    continue;
                }
            };
            match what {
                // CORE §1: one archive per window, so a named archive opens a new one.
                PickerFor::Open => {
                    if let Some(first) = paths.into_iter().next() {
                        self.open_archive(ctx, first, None);
                    }
                }
                PickerFor::Add => self.stage_adds(paths),
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
                    )
                    .into();
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
                        "Cancelled. Nothing was written, and your changes are still staged.".into();
                }
                ApplyMsg::Failed(msg) => {
                    self.progress = None;
                    self.apply_rx = None;
                    self.passphrase = None;
                    self.status = Status::bad(msg);
                }
            }
        }

        // Reactive mode repaints on input, and a painted progress line — unlike the
        // listing `Spinner` — asks for nothing on its own, while the worker holds no
        // `Context` to ask with. So the drain keeps the pump alive for exactly as long as
        // an operation is running, and stops the moment progress clears. This is what makes
        // the line advance rather than jump to done when the mouse happens to twitch.
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
                    self.status = Status::bad(e.to_string());
                    self.pending = Some(PendingAction::List(p));
                    self.popup = Some(Popup::Password);
                    self.password_input.clear();
                }
            }
            other => {
                self.status = Status::bad(other.to_string());
                self.archive_info = None;
            }
        }
    }

    fn remember_current_archive(&mut self) {
        let Some(path) = &self.archive_path else {
            return;
        };
        let path = path.to_string_lossy().to_string();
        let now = store::now();
        self.change_recents(|r| r.bump(&path, now));
    }

    /// Change the recents file, and hold exactly what the file now holds.
    ///
    /// P2 §1: writes happen on change, and never over a file that failed to parse. A
    /// save that did not happen must say so; three call sites once swallowed both answers
    /// with `let _`, which made a full disk look exactly like a successful write.
    ///
    /// `store::Store::change_recents` owns the rest and says why. The window's part is
    /// the startup latch and the sentence — and its own copy is replaced only by what was
    /// actually written, because a list on screen that does not match the file is how a
    /// failed save gets mistaken for a successful one. A file that has *become*
    /// unparseable since startup needs no new latch: every change re-reads, so every
    /// change refuses again on its own.
    pub fn change_recents(&mut self, change: impl FnOnce(&mut Recents)) {
        if self.recents_broken {
            self.status = Status::bad(
                "recents.toml could not be parsed earlier; it will not be overwritten.",
            );
            return;
        }
        match self.store.change_recents(change) {
            Ok(recents) => self.recents = recents,
            Err(notice) => self.status = Status::bad(notice),
        }
    }

    /// Change the settings file. Shaped exactly like `change_recents`.
    pub fn change_settings(&mut self, change: impl FnOnce(&mut Settings)) {
        if self.settings_broken {
            self.status = Status::bad(
                "settings.toml could not be parsed earlier; it will not be overwritten.",
            );
            return;
        }
        match self.store.change_settings(change) {
            Ok(settings) => self.settings = settings,
            Err(notice) => self.status = Status::bad(notice),
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

    /// How many rows the section on screen is showing.
    ///
    /// `rows()` answers for the archive and for nothing else, which is the whole of the
    /// bug P11 fixed: every clamp in `handle_keys` measured the cursor against a list the
    /// user might not even be looking at.
    pub fn section_len(&self, rows: &[Row]) -> usize {
        match self.section {
            Section::Archive => rows.len(),
            Section::Recents => self.recents.sorted().len(),
            Section::Bookmarks => self.settings.bookmarks.len(),
        }
    }

    /// The cursor belonging to the section on screen.
    pub fn section_cursor(&self) -> usize {
        match self.section {
            Section::Archive => self.cursor,
            Section::Recents => self.recents_cursor,
            Section::Bookmarks => self.bookmarks_cursor,
        }
    }

    pub fn set_section_cursor(&mut self, i: usize) {
        match self.section {
            Section::Archive => self.cursor = i,
            Section::Recents => self.recents_cursor = i,
            Section::Bookmarks => self.bookmarks_cursor = i,
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
    /// True exactly once per opening of `which`: the frame its first field takes focus.
    ///
    /// Three popups used to call `request_focus()` unconditionally, every frame, for as
    /// long as they were open — which meant no *second* field in any of them could hold
    /// focus for longer than one frame. Clicking the confirm box in the Password popup
    /// bounced straight back to the box above it, so a new encrypted archive could not be
    /// given a password at all; the Open-With filter had the same grip on its own popup,
    /// so `↑`/`↓` could never reach the list of applications.
    ///
    /// Keyed on *which* popup was focused rather than on a `bool` per popup, so opening
    /// one arms it with no help from the twenty-odd places that assign `self.popup`. The
    /// pairing is with `close_popup` below, which forgets it again.
    pub fn wants_initial_focus(&mut self, which: &Popup) -> bool {
        if self.focus_given_to.as_ref() == Some(which) {
            return false;
        }
        self.focus_given_to = Some(which.clone());
        true
    }

    pub fn close_popup(&mut self) {
        self.popup = None;
        self.focus_given_to = None;
        self.pending = None;
        self.password_input.clear();
        self.password_confirm.clear();
        self.password_attempts = 0;
    }

    /// Name the open archive in the window title.
    ///
    /// CORE §1: "One archive per window. Opening a second archive opens a second window.
    /// There are no tabs." The title was set once at startup and never changed, so every
    /// window in that model was labelled `INDIUM` — identical in the compositor, the
    /// switcher and the taskbar, with no way to tell which held which archive short of
    /// focusing it. The information was already on `archive_path`.
    pub fn set_window_title(&self, ctx: &egui::Context) {
        let title = match self.archive_path.as_ref().and_then(|p| p.file_name()) {
            Some(name) => format!("{} — INDIUM", name.to_string_lossy()),
            None => "INDIUM".to_string(),
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
    }

    // -----------------------------------------------------------------------
    // Preview — P5 §B
    // -----------------------------------------------------------------------

    /// Read an entry's head on a worker so the Preview tab has something to show.
    ///
    /// Nothing here decodes an image: `egui_extras`' loader does that on its own
    /// background thread and repaints itself when it is done. What needs a worker is the
    /// archive read, because `arch::Reader` is not `Send` — only the path and the
    /// passphrase cross the boundary, and the worker opens its own reader.
    pub fn request_preview(&mut self, ctx: &egui::Context, entry_path: &str) {
        if self.preview.as_ref().map(|p| p.path.as_str()) == Some(entry_path) {
            return; // already showing this one
        }
        if self.preview_loading.as_deref() == Some(entry_path) {
            return; // already on its way
        }
        let Some(archive) = self.archive_path.clone() else {
            return;
        };
        let Some(entry) = self.entry(entry_path) else {
            return;
        };
        if entry.is_dir {
            self.forget_preview(ctx);
            return;
        }
        // An encrypted entry rides P2's park-and-resume path, exactly as a checksum does.
        if entry.encrypted && self.passphrase.is_none() {
            self.pending = Some(PendingAction::Preview {
                entry: entry_path.to_string(),
            });
            self.popup = Some(Popup::Password);
            self.password_input.clear();
            self.password_attempts = 0;
            return;
        }

        self.forget_preview(ctx);
        self.preview_loading = Some(entry_path.to_string());

        let (tx, rx) = channel();
        self.preview_rx = Some(rx);
        let want = entry_path.to_string();
        let pass = self.passphrase.clone();
        let ctx2 = ctx.clone();
        std::thread::spawn(move || {
            let got = arch::head_of(&archive, &want, PREVIEW_CAP, pass.as_ref())
                .map_err(|e| e.to_string());
            let _ = tx.send((want, got));
            ctx2.request_repaint();
        });
    }

    /// Drop whatever Preview was showing, and the texture behind it.
    ///
    /// Without the `forget_image` every previewed image would stay in egui's texture cache
    /// for the life of the process — a browse through a few hundred photographs would
    /// leak all of them.
    pub fn forget_preview(&mut self, ctx: &egui::Context) {
        if let Some(old) = self.preview.take() {
            ctx.forget_image(&old.uri);
        }
        self.preview_loading = None;
        self.preview_rx = None;
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
            self.status = Status::bad(refusal);
            return;
        }
        if self.staged_against.is_empty() {
            self.staged_against = self.entries.iter().map(|e| e.path.clone()).collect();
        }
        self.status = task.summary().into();
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

    /// `F2` — begin editing a name in the table. CORE §4 fixes the popup count at nine,
    /// so this is not a tenth popup; it is the Name cell becoming a text field.
    pub fn begin_rename(&mut self, rows: &[Row]) {
        if !self.has_archive() {
            return;
        }
        if let Some(refusal) = self.staging_refusal() {
            self.status = Status::bad(refusal);
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
            self.status = Status::bad("A name cannot be empty or contain a slash.");
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
            self.status =
                format!("Removed that change, and {dropped} that depended on it.",).into();
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
            self.status = Status::bad("This archive's format cannot be written.");
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
        // The window holds one `apply_rx` and one cancellation flag. A second Apply used to
        // replace both: the first worker went on rebuilding an archive nothing could report
        // on and nothing could stop, and the status bar's Cancel reached only the newer one.
        // The tray strip is a button and Apply is one keystroke, so twice is easy.
        if self.apply_rx.is_some() {
            self.status = Status::bad("A rebuild is already running. Cancel it, or let it finish.");
            return;
        }
        // And an extraction counts, for a sharper reason: the `store` below would cancel a
        // running copy-out without a word, and its `Done` would then arrive holding a fresh
        // flag that says nothing was cancelled — a partial selection offered to the
        // clipboard as if it were the whole of it. Both refusals come before that store.
        if self.extract_rx.is_some() {
            self.status =
                Status::bad("An extraction is already running. Cancel it, or let it finish.");
            return;
        }
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

    /// Raise the desktop's file picker on a worker, and act on what it names.
    ///
    /// Off the UI thread for a blunter reason than the clipboard's: the dialog is on
    /// screen for as long as the user is *choosing*, which may be a minute. A blocking
    /// call here would freeze the window that raised it, behind the dialog, for the whole
    /// of that.
    pub fn request_picker(&mut self, ctx: &egui::Context, what: PickerFor) {
        if self.picker_rx.is_some() {
            return;
        }
        let (tx, rx) = channel();
        self.picker_rx = Some(rx);
        let ctx = ctx.clone();
        let (title, multiple) = match what {
            PickerFor::Open => ("Open archive", false),
            PickerFor::Add => ("Add to archive", true),
        };
        std::thread::spawn(move || {
            let _ = tx.send((what, picker::open_files(title, multiple)));
            ctx.request_repaint();
        });
    }

    /// Show a directory in the desktop's file manager — CORE §4's clickable path.
    ///
    /// On a worker for the reason `request_picker` is: the portal call is a D-Bus round
    /// trip, and a backend that decides to prompt holds it open for as long as the user
    /// thinks. Nothing in the window waits on the answer, which is why the only thing that
    /// comes back is a sentence for when it failed.
    pub fn reveal_directory(&mut self, ctx: &egui::Context, dir: PathBuf) {
        if self.reveal_rx.is_some() {
            return;
        }
        let (tx, rx) = channel();
        self.reveal_rx = Some(rx);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(crate::platform::open::open_directory(&dir));
            ctx.request_repaint();
        });
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
            )
            .into();
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

    /// Is long work already running? Then say so, and start nothing.
    ///
    /// The window holds one `extract_rx` and one cancellation flag, so a second start
    /// replaces both and leaves the first worker sending into a dropped channel: no
    /// `Done`, progress that never clears, and a Cancel that reaches only the newer worker
    /// while the older keeps writing files with nothing left to stop it. Copy-out and Open
    /// With have a second reason — `Scratch::begin` removes the previous directory of its
    /// kind, which is the very directory the first worker is writing into.
    ///
    /// A rebuild counts too, and must: extraction and Apply share that one flag and one
    /// progress bar, so starting an extraction over a rebuild would leave the rebuild's
    /// Cancel pointing at a flag nothing reads. CORE §3 has one worker, and this is what
    /// keeps it to one.
    ///
    /// P7 added `open_archive` to the callers, which is the entry point that was doing
    /// the replacing rather than being refused it. Both sentences below are written to be
    /// read by someone who has just pressed a key and had nothing happen — and from P7
    /// they can be, because row 2 of the status bar no longer hides the status text while
    /// work is running.
    fn work_running(&mut self) -> bool {
        if self.extract_rx.is_some() {
            self.status =
                Status::bad("An extraction is already running. Cancel it, or let it finish.");
            return true;
        }
        if self.apply_rx.is_some() {
            self.status = Status::bad("A rebuild is already running. Cancel it, or let it finish.");
            return true;
        }
        false
    }

    /// Hand an extraction to a worker, and record what becomes of it when it lands.
    ///
    /// The one place the receiver, the cancellation flag and the progress total are set up,
    /// so the three callers cannot drift apart on any of them. CORE §3: "the UI thread and
    /// one worker … it reports progress over a channel and honours a cancellation flag."
    fn spawn_extract(
        &mut self,
        ctx: &egui::Context,
        archive: PathBuf,
        wanted: std::collections::HashSet<String>,
        dest: PathBuf,
        post: PostExtract,
    ) {
        self.cancel = Arc::new(AtomicBool::new(false));
        let (tx, rx) = channel();
        self.extract_rx = Some(rx);
        self.post_extract = post;
        self.progress = Some(Progress {
            done: 0,
            total: wanted.len(),
            label: "Extracting".to_string(),
        });

        let cancel = Arc::clone(&self.cancel);
        let ctx2 = ctx.clone();
        // Cloned here, on the UI thread, before the worker exists — the password prompt
        // drops `self.passphrase` the moment the caller returns, and the worker must not
        // depend on a field it cannot see.
        let pass = self.passphrase.clone();

        std::thread::spawn(move || {
            match arch::extract(&archive, &wanted, &dest, pass.as_ref(), Some(&tx), &cancel) {
                Ok(written) => {
                    // A cancelled extraction returns `Ok` with however much it managed to
                    // write, so the two answers have to be told apart here — and by *this*
                    // worker's own `Arc`, the one moved into the closure, never by
                    // `self.cancel`. The window replaces that field on every `spawn_extract`
                    // and every `open_archive`, so a UI-side read answers for whichever
                    // operation is newest rather than for the one that just landed. That is
                    // the shape `tasks::apply` already uses for `ApplyMsg::Cancelled`, and
                    // its doc comment named this very defect.
                    if cancel.load(Ordering::Relaxed) {
                        let _ = tx.send(ExtractMsg::Cancelled { written });
                    } else {
                        let _ = tx.send(ExtractMsg::Done { written });
                    }
                }
                Err(e) => {
                    let _ = tx.send(ExtractMsg::Failed(e.to_string()));
                }
            }
            // Reactive mode: the worker is what wakes the UI, and what makes the Open With
            // picker appear on the frame the extraction finished rather than the next time
            // the mouse happens to move.
            ctx2.request_repaint();
        });
    }

    pub fn begin_extract(&mut self, ctx: &egui::Context, dest: PathBuf) {
        let Some(archive) = self.archive_path.clone() else {
            return;
        };
        if self.work_running() {
            return;
        }

        // Nothing selected means the whole archive — the obvious reading of "Extract"
        // with no selection, and what every other archiver does.
        let wanted: std::collections::HashSet<String> = if self.selection.is_empty() {
            self.entries.iter().map(|e| e.path.clone()).collect()
        } else {
            self.selection.iter().cloned().collect()
        };

        if let Err(e) = std::fs::create_dir_all(&dest) {
            self.status = Status::bad(format!("Could not create {}: {e}", dest.display()));
            return;
        }

        self.status = format!("Extracting to {}…", dest.display()).into();
        self.spawn_extract(ctx, archive, wanted, dest, PostExtract::None);
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
            Err(e) => self.status = Status::bad(e.to_string()),
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

    /// `Ctrl+C` — extract to scratch on a worker, and offer the clipboard what lands.
    ///
    /// P3 §2: "it must feel better than drag, not apologetic about it." A copy-out that
    /// froze the window until libarchive was finished did not, so from P6 the extraction
    /// is the same worker `E` uses and the offer happens in `finish_copy_out`.
    pub fn copy_out(&mut self, ctx: &egui::Context, rows: &[Row]) {
        let paths = self.subject_paths(rows);
        if paths.is_empty() {
            self.status = Status::bad("Nothing selected.");
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
        // Asked before `Scratch::begin`, which removes the previous copy-out directory —
        // the one a running worker is writing into.
        if self.work_running() {
            return;
        }
        let total = self.uncompressed_total(&wanted);

        let placement = match self.scratch.begin(scratch::Kind::CopyOut, total) {
            Ok(p) => p,
            Err(e) => {
                self.status = Status::bad(format!("Could not make a scratch directory: {e}"));
                return;
            }
        };
        let on_disk = placement.on_disk;

        self.status = "Copying out…".to_string().into();
        self.spawn_extract(
            ctx,
            archive,
            wanted,
            placement.dir,
            PostExtract::Clipboard { on_disk },
        );
    }

    /// The other half of `copy_out`, run when the worker reports.
    ///
    /// The directory comes back off `self.scratch` rather than being carried through the
    /// channel: `Scratch` owns it either way, and only `on_disk` was worth stashing.
    fn finish_copy_out(&mut self, on_disk: bool) {
        let Some(dir) = self
            .scratch
            .current(scratch::Kind::CopyOut)
            .map(|p| p.to_path_buf())
        else {
            self.status = Status::bad("The scratch directory is gone; nothing was offered.");
            return;
        };

        let files = collect_files(&dir);
        if files.is_empty() {
            self.status = Status::bad("Nothing to copy.");
            return;
        }

        match clipboard::offer(&files) {
            Ok(()) => {
                let n = files.len();
                let mut msg = format!(
                    "{n} {} ready to paste.",
                    if n == 1 { "file" } else { "files" }
                );
                if on_disk {
                    // P3 §1's one-line notice.
                    msg.push_str(" Over 1 GiB — staged on disk rather than in RAM.");
                }
                self.status = msg.into();
            }
            Err(e) => self.status = Status::bad(e),
        }
    }

    /// `Enter` on a file — extract that one entry on a worker, and offer the picker when
    /// it lands.
    pub fn open_with(&mut self, ctx: &egui::Context, entry_path: &str) {
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
        if self.work_running() {
            return;
        }
        let wanted: std::collections::HashSet<String> =
            std::iter::once(entry.path.clone()).collect();

        let placement = match self.scratch.begin(scratch::Kind::OpenWith, entry.size) {
            Ok(p) => p,
            Err(e) => {
                self.status = Status::bad(format!("Could not make a scratch directory: {e}"));
                return;
            }
        };

        self.status = format!("Extracting {}…", crate::util::base_name(&entry.path)).into();
        self.spawn_extract(
            ctx,
            archive,
            wanted,
            placement.dir,
            PostExtract::OpenWith {
                entry: Box::new(entry),
            },
        );
    }

    /// The other half of `open_with`, run when the worker reports.
    fn finish_open_with(&mut self, entry: &Entry) {
        let Some(dir) = self
            .scratch
            .current(scratch::Kind::OpenWith)
            .map(|p| p.to_path_buf())
        else {
            self.status = Status::bad("The scratch directory is gone; there is nothing to open.");
            return;
        };

        let extracted = dir.join(&entry.raw_path);
        if !extracted.is_file() {
            self.status = Status::bad("The entry did not extract to a file.");
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
        self.openwith_cursor = 0;
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

/// Did this frame carry `Ctrl+C`, and did it carry `Ctrl+V`?
///
/// The clipboard chords, read off the event list on the **key's release**.
///
/// P10 found half of this and drew the wrong conclusion from it, so the whole of it is
/// written down here. `egui-winit-0.36.1/src/lib.rs:1019` matches the three clipboard
/// chords before it emits anything and returns early, so the `Key` event is never
/// produced and `key_pressed(Key::C)` is permanently false. P10 answered that by taking
/// `Event::Copy` and `Event::Paste` instead, and shipped `v1.0.0-2` claiming both chords
/// worked.
///
/// Only copy did. The paste arm is not the same shape as the other two:
///
/// ```text
/// } else if is_paste_command(self.modifiers, active_key) {
///     if let Some(contents) = self.clipboard.get() {
///         ...push Event::Paste(contents)
///     }
///     return;          // <- outside the `if let`
/// }
/// ```
///
/// `clipboard.get()` returns text or nothing. A file manager copying a *file* offers
/// `text/uri-list` and no plain text at all, so it returns `None`, **no event of any kind
/// is pushed**, and the `Key::V` is swallowed on the way past. There is nothing left in
/// the frame to notice. P10's speculative `Key` arm could never have fired, because the
/// early return is exactly what stops that key existing.
///
/// What survives is the **release**. egui-winit guards the whole interception with
/// `if pressed`, so `Key { key: V, pressed: false }` is emitted normally, for every
/// clipboard state there is. One signal, one per chord, no dependence on what the
/// clipboard happens to hold.
///
/// `Event::Copy` and `Event::Paste` are deliberately *not* also accepted. Answering both
/// a press-form and a release-form fires the action twice whenever the clipboard does
/// carry text, and `paste_rx.is_some()` is no guard against it — the read finishes long
/// before a human lifts the key.
///
/// The one case this cannot see is `Ctrl` released before the letter, which leaves a
/// release with no modifier on it and is indistinguishable from a bare keypress. That is
/// the opposite of how a chord is normally let go.
///
/// `Ctrl+X` is deliberately not answered: CORE §4's table has no cut, and an archive
/// manager that cuts is one that deletes on a paste that may never come.
/// Put the caret at the end of a `TextEdit` whose text was just replaced from outside it.
///
/// Assigning to the `String` behind a `TextEdit` changes what is drawn and nothing else:
/// egui keeps the caret in `TextEditState`, keyed by the widget's `Id`, and a caret that
/// was at column 9 stays at column 9 in a line that is now forty characters long. So `Tab`
/// completed the path and then left the user to click past the end of what they had just
/// been given — which is most of the point of tab completion gone.
///
/// The widget must therefore be given an explicit `Id`; an auto-generated one is not
/// knowable from out here.
fn caret_to_end(ctx: &egui::Context, id: egui::Id, text: &str) {
    let Some(mut state) = egui::TextEdit::load_state(ctx, id) else {
        return;
    };
    // Characters, not bytes: `CCursor` counts the former, and a completed path is exactly
    // where a name outside ASCII turns up.
    let end = egui::text::CCursor::new(text.chars().count());
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::one(end)));
    state.store(ctx, id);
}

fn clipboard_chords(events: &[egui::Event]) -> (bool, bool) {
    let mut copy = false;
    let mut paste = false;
    for ev in events {
        let egui::Event::Key {
            key,
            pressed: false,
            modifiers,
            ..
        } = ev
        else {
            continue;
        };
        if !(modifiers.ctrl || modifiers.command) {
            continue;
        }
        match key {
            egui::Key::C => copy = true,
            egui::Key::V => paste = true,
            _ => {}
        }
    }
    (copy, paste)
}

// ---------------------------------------------------------------------------
// The eframe App
// ---------------------------------------------------------------------------

impl eframe::App for Indium {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        // `VOID`, because this is the only thing that shows *between* the zones: every
        // zone is a `theme::zone()` frame with half a gutter of outer margin (P7's
        // spacing note on `GUTTER`), and what fills that gutter is whatever eframe
        // cleared the framebuffer to. `WINDOW` here made the gutter the same colour as
        // the entry table's well, so the cards had nothing to float above.
        let c = theme::VOID;
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

        // A popup that is no longer the open one has no claim on focus. This one line is
        // what re-arms `wants_initial_focus` for the *next* opening, and it lives here
        // rather than at every site that assigns `self.popup` — several of which set it to
        // `None` directly without going through `close_popup`.
        if self.focus_given_to != self.popup {
            self.focus_given_to = None;
        }

        let rows = self.rows();
        self.handle_keys(&ctx, &rows);

        // Panels are shown into the root `Ui`; the order fixes the layout, with the
        // table taking whatever the four edges leave behind.
        //
        // The status bar goes **first**, before the sidebar, and that ordering is the
        // whole of it: egui gives an earlier panel priority over the space, so the bar
        // now spans the full window and the sidebar stops on top of it rather than
        // running past it to the bottom edge. Row 3 of the new bar is the *program's*
        // progress — a rebuild is not the table's work, and neither is a copy-out — so
        // the zone that carries it has no business being inset under one of the four it
        // reports on. A floor that reaches both walls is also what makes the sidebar,
        // the tray, the Inspector and the table read as cards standing on something.
        status_bar(self, ui);
        sidebar::show(self, ui);
        tray::show(self, ui);
        inspector::show(self, ui, &rows);
        table::show(self, ui, &rows);

        // Popups take the context, and come last so they draw over every zone.
        extract::show(self, &ctx);
        settings::show(self, &ctx);
        about::show(self, &ctx);
        keys::show(self, &ctx);
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

        // `focused()` is true of a `TextEdit` and of nothing else. A **label** with a live
        // text selection never takes focus, so `typing` is false while the user is dragging
        // across the status bar — and P10's `Ctrl+C` then started an extraction underneath
        // someone who had asked for four words of a path. egui is already copying that
        // selection itself; all this has to do is not also copy the archive out.
        //
        // It guards `Ctrl+C` alone rather than joining `typing`. Widening `typing` would
        // switch off every bare-letter shortcut in CORE §4's table for as long as a
        // selection existed anywhere in the window, which is a second bug wearing the
        // first one's clothes.
        let selecting_text = ctx
            .plugin::<egui::text_selection::LabelSelectionState>()
            .lock()
            .has_selection();

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

        // `Ctrl+F` and `Ctrl+O` work even while typing — a `TextEdit` claims neither, so
        // opening the filter bar or the path field from inside another field is no
        // ambiguity at all.
        //
        // The last two do **not** come from `key_pressed`, and P10 §1 is the whole
        // explanation: by the time a clipboard chord reaches us it is no longer a key.
        let (ctrl_f, ctrl_a, ctrl_o, ctrl_c, ctrl_v) = ctx.input(|i| {
            let (copy, paste) = clipboard_chords(&i.events);
            (
                i.modifiers.ctrl && i.key_pressed(egui::Key::F),
                i.modifiers.ctrl && i.key_pressed(egui::Key::A),
                i.modifiers.ctrl && i.key_pressed(egui::Key::O),
                copy,
                paste,
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
        // The three chords a `TextEdit` already owns, all guarded on `typing`: select-all,
        // copy and paste belong to the focused field. Without the guard, `Ctrl+C` in the
        // filter bar, the rename box, the Extract path field or the `Ctrl+O` field copied
        // the archive selection out instead of the text the user had just highlighted.
        if ctrl_a && self.has_archive() && !typing {
            self.selection = rows.iter().map(|r| r.path.clone()).collect();
        }
        if ctrl_c && self.has_archive() && !typing && !selecting_text {
            self.copy_out(ctx, rows);
        }
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
        // Same reason: `request_picker` takes `&mut self` and a `&Context`, and the closure
        // is already holding the input lock the context would have to hand back.
        let mut open_picker = false;
        let mut add_picker = false;

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
                    // CORE §4's order, and it moved in P12: the archive is `1` because
                    // it is what a person is looking at. The numbers are literals in the
                    // sidebar too (`sidebar.rs`), so these two lists have to be read
                    // together — a key that disagrees with the label beside it is worse
                    // than no key.
                    egui::Key::Num1 => self.section = Section::Archive,
                    egui::Key::Num2 => self.section = Section::Bookmarks,
                    egui::Key::Num3 => self.section = Section::Recents,
                    // `O` opens the desktop's picker; `Ctrl+O` still opens the path field,
                    // so the two readings of "open" sit on one letter with and without the
                    // modifier. `I` adds into the directory the breadcrumb names — the same
                    // call the *Add files…* button makes. `A` is About and could not move,
                    // and `+` is `Shift+4` on the maker's own layout, so it is not a bare
                    // key on the machine this is built on.
                    egui::Key::O => open_picker = true,
                    egui::Key::I => add_picker = true,
                    egui::Key::F1 => self.popup = Some(Popup::Keys),
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
        if open_picker {
            self.request_picker(ctx, PickerFor::Open);
        }
        if add_picker && self.has_archive() {
            self.request_picker(ctx, PickerFor::Add);
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

        // `section_len`, not `rows.len()`. Measuring against the archive while Bookmarks
        // was on screen is what stopped the third bookmark ever staying lit and what made
        // `Enter` on a recent open the wrong row or none at all — one line, three symptoms.
        let len = self.section_len(rows);
        let moved = up || down || pgup || pgdn || home || end;
        if len > 0 {
            let step = 12usize;
            let mut at = self.section_cursor();
            if up {
                at = at.saturating_sub(1);
            }
            if down {
                at = (at + 1).min(len - 1);
            }
            if pgup {
                at = at.saturating_sub(step);
            }
            if pgdn {
                at = (at + step).min(len - 1);
            }
            if home {
                at = 0;
            }
            if end {
                at = len - 1;
            }
            self.set_section_cursor(at.min(len - 1));

            // Moving the cursor selects what it lands on, which is what makes
            // "Inspector updates on arrow-key movement" (P1's manual checklist) true.
            // A checksum belongs to the entry it was computed for, so it is dropped.
            if moved && self.section == Section::Archive {
                self.crc_of = None;
                self.forget_preview(ctx);
                self.scroll_to_cursor = true;
                self.selection.clear();
                if let Some(row) = rows.get(self.cursor) {
                    self.selection.insert(row.path.clone());
                }
            } else if moved {
                self.crc_of = None;
                self.forget_preview(ctx);
            }
        } else {
            self.set_section_cursor(0);
        }

        match self.section {
            Section::Archive => {
                if enter {
                    match rows.get(self.cursor) {
                        Some(row) if row.is_dir => self.descend(rows),
                        Some(row) => {
                            let path = row.path.clone();
                            self.open_with(ctx, &path);
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
                    if let Some(r) = self.recents.sorted().get(self.recents_cursor).cloned() {
                        let path = PathBuf::from(&r.path);
                        if path.exists() {
                            self.open_archive(ctx, path, None);
                        } else {
                            self.status = Status::bad(format!("{} is no longer there.", r.path));
                        }
                    }
                }
                if del {
                    if let Some(r) = self.recents.sorted().get(self.recents_cursor).cloned() {
                        let path = r.path.clone();
                        // The success line first, the write last: a save that failed has
                        // something to say, and it must not be overwritten by a sentence
                        // announcing a change that never reached the disk.
                        self.status = format!("Removed {path} from recent files.").into();
                        self.change_recents(|r| r.remove(&path));
                    }
                }
            }
            Section::Bookmarks => {
                if del && self.bookmarks_cursor < self.settings.bookmarks.len() {
                    let gone = self.settings.bookmarks[self.bookmarks_cursor].clone();
                    let name = gone.name.clone();
                    self.change_settings(move |s| s.bookmarks.retain(|b| *b != gone));
                    self.status = format!("Removed bookmark {name}.").into();
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Status bar — CORE §4's fifth zone
// ---------------------------------------------------------------------------

/// The whole panel, gutter included.
///
/// 3 × SB_ROW(32) + 2 × SB_GAP(4) = 104 of content, + 10 + 10 of inner margin, + the 2 + 2
/// the edge costs, + the 4 + 4 of `zone()`'s outer gutter = 136. `exact_size` counts every
/// one of those, so this number is the panel and its half-gutter rim together. It was 100
/// until P13 raised `SB_ROW` to carry an icon at `ICON_SCALE`.
///
/// **The edge is in that sum, and has to be.** `Frame::total_margin` is `inner_margin +
/// stroke.width + outer_margin`, and the box egui actually paints is `content +
/// inner_margin + stroke.width` — so a 2px border costs 2px of layout on every side,
/// whatever `StrokeKind::Inside` suggests about where the line lands. At 96 the panel
/// overflowed by exactly those 4 and paid for it out of its own outer margin: measured on
/// screen, the gutter above the bar came out half the width of every other gutter in the
/// window, and the bottom rim vanished entirely. `the_status_bar_is_as_tall_as_it_says`
/// pins the arithmetic, because a panel that overflows `exact_size` is reported at the
/// size it asked for — the clamp is on the number, not on the paint.
/// The smallest window INDIUM will run in, and the one number `main.rs` hands the
/// compositor as `with_min_inner_size`.
///
/// **880** is the three zones' own floors added up: the sidebar is fixed at 202, the
/// Inspector will not go under 272, and the entry table cannot show Name, Size, Packed and
/// Method in less than 360 plus its scrollbar, over 20 of central chrome.
///
/// **680** is the height at which the sidebar shows all seven rows beneath its header.
///
/// Both are only a **request**, and this compositor declines them: measured on KWin, INDIUM
/// asks for 1180×720 and is handed 960×540 whatever floor it names. Nothing here tries to
/// force the point — a program that resizes the window out from under the hand dragging it is
/// worse than a short window. Shorter than this and the sidebar scrolls, which is what any
/// program does when its contents outgrow it.
pub const MIN_W: f32 = 880.0;
/// See [`MIN_W`].
pub const MIN_H: f32 = 680.0;

const SB_HEIGHT: f32 = 136.0;

/// The status bar's frame, named so the height above can be checked against it.
fn sb_frame() -> egui::Frame {
    theme::zone(theme::STATUS_BAR).inner_margin(egui::Margin::symmetric(12, 10))
}

/// Three rows, each exactly [`theme::SB_ROW`] tall, in a panel that never changes size.
///
/// The old bar was one row, content-driven, and it **moved**: a `Cancel` button and a
/// progress bar are taller than a label, so the floor of the window rose by about a pixel
/// the instant work started and dropped back when it finished. `exact_size` is what stops
/// that — the panel is now the same height whether INDIUM is idle, listing or rebuilding,
/// and nothing above it can be nudged by anything below it.
///
/// The rows answer three different questions and never each other's: *what is open*,
/// *what is in it and what is INDIUM saying*, and *is anything running*.
fn status_bar(app: &mut Indium, ui: &mut egui::Ui) {
    egui::Panel::bottom("status")
        .resizable(false)
        .exact_size(SB_HEIGHT)
        // `theme::zone`'s own requirement: egui draws a hairline between panels, and it
        // would stack with the 2px edge the frame already paints.
        .show_separator_line(false)
        .frame(sb_frame())
        .show(ui, |ui| {
            // The 4 in the arithmetic above is this line. egui's default is 5.0, which
            // makes the three rows 70 tall in a lane that is 68 — and the third row is
            // the one that gets clipped.
            ui.spacing_mut().item_spacing.y = theme::SB_GAP;
            // And this is what keeps Cancel to its 20. `interact_size.y` is `CONTROL_H`,
            // which P13 split away from `SB_ROW` precisely so that a taller row does not
            // drag every button in the program up with it — but the theme's 3px of
            // vertical padding would still push Cancel to 23, and a control that grows
            // inside a row is one that decides the row's height instead of sitting in it.
            ui.spacing_mut().button_padding.y = 1.0;

            // CORE §4: "A rule separates the rows. They are three statements, not one
            // paragraph in three pieces."
            //
            // **Painted into the gap, never allocated into it.** `SB_HEIGHT` is asserted by
            // `the_status_bar_is_as_tall_as_it_says`, and the lane is exactly
            // `3 * SB_ROW + 2 * SB_GAP` — so a `ui.separator()` between the rows, which
            // allocates 6pt of its own, would push row 3 out of a panel that is
            // `exact_size` and cannot grow to absorb it. A line drawn down the middle of a
            // gap that already exists costs nothing.
            let lane = ui.available_rect_before_wrap();
            sb_what_is_open(app, ui);
            let after_1 = ui.cursor().top();
            sb_the_numbers(app, ui);
            let after_2 = ui.cursor().top();
            sb_progress(app, ui);
            for y in [after_1, after_2] {
                ui.painter().hline(
                    lane.x_range(),
                    (y - theme::SB_GAP / 2.0).round(),
                    theme::hairline(),
                );
            }

            // CORE §4: "The proportion done is drawn as a 2px line along the bar's own top
            // edge." Painted for the same reason the two hairlines above are painted and
            // never allocated — `exact_size` means this panel cannot grow to absorb
            // anything new, and a widget here would push row 3 out of the window.
            //
            // 2px is the edge weight §6 already owns, and Orange is already Apply/progress,
            // so this measurement costs the document no new vocabulary at all.
            if let Some(p) = &app.progress {
                let frac = if p.total == 0 {
                    0.0
                } else {
                    (p.done as f32 / p.total as f32).clamp(0.0, 1.0)
                };
                // Out to the frame's own edge rather than the content lane's: the inner
                // margin is where the padding starts, so backing out by it lands on the
                // line the 2px stroke draws.
                let pad = sb_frame().inner_margin;
                let edge = lane.expand2(egui::vec2(pad.left as f32, pad.top as f32));
                let run = egui::Rect::from_min_size(
                    edge.left_top(),
                    egui::vec2(edge.width() * frac, 2.0),
                );
                ui.painter()
                    .with_clip_rect(edge)
                    .rect_filled(run, 0.0, theme::ORANGE);
            }
        });
}

/// Open one status-bar row: a lane of exactly one [`theme::SB_ROW`], whatever goes in it.
///
/// `allocate_ui_with_layout` hands the closure a `Ui` of that size but advances the cursor
/// by the *content's* extent, so a row holding nothing — the idle progress lane — would
/// collapse to nothing and the arithmetic in `status_bar` would stop being true.
/// `set_min_height` pins it from inside.
fn sb_row(ui: &mut egui::Ui, layout: egui::Layout, add: impl FnOnce(&mut egui::Ui)) {
    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), theme::SB_ROW),
        layout,
        |ui| {
            ui.set_min_height(theme::SB_ROW);
            add(ui);
        },
    );
}

/// Row 1 — what is open.
///
/// The name on the left because it is the answer to "which window is this"; the directory
/// on the right because it is the longest thing in the bar and the least urgent, so it is
/// what gives way when the window narrows.
fn sb_what_is_open(app: &mut Indium, ui: &mut egui::Ui) {
    sb_row(ui, egui::Layout::left_to_right(egui::Align::Center), |ui| {
        match app.archive_path.as_ref().and_then(|p| p.file_name()) {
            // The row's subject, so it is bold — CORE §4. It is also the answer to
            // "which window is this", which on a desktop full of them is the question.
            //
            // **A glyph takes the ink of the text it names**, here and everywhere else in
            // the bar: the drawer is as bright as the name, the folder below is as muted
            // as the path, and the triangle on row 2 is the same yellow as the failure. A
            // glyph in a third colour would be a fourth thing to read rather than a
            // shorter way of reading the first.
            Some(name) => {
                ui.label(
                    egui::RichText::new(theme::icon::ARCHIVE)
                        .family(theme::MONO)
                        .size(13.0 * theme::ICON_SCALE)
                        .color(theme::TEXT),
                );
                ui.label(
                    egui::RichText::new(name.to_string_lossy())
                        .family(theme::bold())
                        .size(13.0)
                        .color(theme::TEXT),
                )
            }
            None => ui.label(
                egui::RichText::new("No archive open.")
                    .family(theme::MONO)
                    .size(13.0)
                    .color(theme::TEXT_MUTED),
            ),
        };

        if let Some(info) = &app.archive_info {
            ui.label(egui::RichText::new("·").color(theme::TEXT_MUTED));
            ui.label(
                egui::RichText::new(&info.format)
                    .family(theme::MONO)
                    .size(13.0)
                    .color(theme::TEXT_SECONDARY),
            );
            if !info.filter.is_empty() && info.filter != "none" {
                ui.label(
                    egui::RichText::new(&info.filter)
                        .family(theme::MONO)
                        .size(13.0)
                        .color(theme::TEXT_MUTED),
                );
            }
        }

        let dir = app
            .archive_path
            .as_ref()
            .and_then(|p| p.parent())
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        if !dir.is_empty() {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // CORE §4: "elided in the middle, never at the end, because the end is the
                // folder the archive is actually in and the start is the tree it belongs
                // to; a path that keeps only one of those has kept the wrong half."
                //
                // egui's own `.truncate()` cuts the tail, which is what this replaces. The
                // budget has to be counted rather than guessed, so it is measured: one
                // glyph advance out of the font at the size this row actually uses, and
                // the width divided by it. **That arithmetic is only true because the face
                // is monospace** (CORE §6) — a double-width script would elide short. The
                // corpus this program is tested against is Turkish, which is single-width.
                let font = egui::FontId::new(12.0, theme::MONO);
                // `fonts_mut`, not `fonts`: measuring a glyph can populate the atlas, so
                // the accessor that admits it is the correct one.
                let cell = ui.ctx().fonts_mut(|f| f.glyph_width(&font, '0')).max(1.0);
                let gap = ui.spacing().item_spacing.x;
                // Less the folder glyph and the space before it, which are added after
                // this label because the lane runs right to left. The glyph is drawn at
                // `ICON_SCALE`, so it costs that many cells and not one.
                let glyph = cell * theme::ICON_SCALE + gap;
                let budget = ((ui.available_width() - glyph) / cell).floor().max(0.0) as usize;
                let shown = crate::util::elide_middle(&dir, budget);

                let hit = ui
                    .add(
                        egui::Label::new(
                            egui::RichText::new(shown)
                                .family(theme::MONO)
                                .size(12.0)
                                .color(theme::TEXT_MUTED),
                        )
                        // CORE §4: "clicking it hands that folder to the desktop's file
                        // manager." The tray strip is the precedent — §4 says of it "the
                        // strip itself is a button" — so a bar element that acts is a
                        // shape this window already has.
                        .sense(egui::Sense::click()),
                    )
                    .on_hover_text(&dir)
                    .on_hover_cursor(egui::CursorIcon::PointingHand);

                if hit.clicked() {
                    let ctx = ui.ctx().clone();
                    app.reveal_directory(&ctx, PathBuf::from(&dir));
                }

                ui.label(
                    egui::RichText::new(theme::icon::FOLDER)
                        .family(theme::MONO)
                        .size(12.0 * theme::ICON_SCALE)
                        .color(theme::TEXT_MUTED),
                );
            });
        }
    });
}

/// Row 2 — the numbers, and the voice.
///
/// **`app.status` is drawn unconditionally, and that is the point of the row.** Until P7
/// the status text lived in the `else` of a branch that `progress.is_some()` pre-empted,
/// so every sentence INDIUM says *while something is running* was written to a field
/// nothing drew. The four "already running" refusals in `begin_apply` and `work_running`
/// are set at exactly the moments that branch was hiding — a user who pressed `E` twice
/// got silence, which reads as the program ignoring the key. Nothing in this row may
/// consult `progress`; that is row 3's job and only row 3's.
fn sb_the_numbers(app: &Indium, ui: &mut egui::Ui) {
    sb_row(ui, egui::Layout::left_to_right(egui::Align::Center), |ui| {
        if app.has_archive() {
            let agg = model::aggregate(app.entries.iter());
            // CORE §4: "One thing per row is the subject, and it is bold." On this row
            // that is the count — everything else here qualifies it.
            ui.label(
                egui::RichText::new(format!("{} entries", agg.count))
                    .family(theme::bold())
                    .size(13.0)
                    .color(theme::TEXT),
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
                .size(13.0)
                .color(theme::TEXT_SECONDARY),
            );

            if !app.selection.is_empty() {
                ui.label(egui::RichText::new("·").color(theme::TEXT_MUTED));
                ui.label(
                    egui::RichText::new(format!("{} selected", app.selection.len()))
                        .family(theme::MONO)
                        .size(13.0)
                        .color(theme::TEXT_SECONDARY),
                );
            }
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add(
                egui::Label::new(
                    egui::RichText::new(&app.status.text)
                        .family(theme::MONO)
                        .size(13.0)
                        // CORE §4: "A failure is `#FFD800`... A refusal is a failure; a
                        // confirmation is not." For eleven milestones every sentence in
                        // this bar was the same grey, so `Could not create …` and
                        // `Removed bookmark photos.` were indistinguishable at a glance.
                        .color(if app.status.bad {
                            theme::WARNING
                        } else {
                            theme::TEXT_SECONDARY
                        }),
                )
                .truncate(),
            );
            // CORE §6's one sanctioned redundancy: "colour alone carrying meaning fails
            // anyone who cannot separate `#FFD800` from grey, so there the shape and the
            // colour say the same thing on purpose." Added *after* the sentence because
            // this layout is right-to-left — the first widget in is the rightmost, so the
            // triangle lands to the left of the words, which is where it is read.
            if app.status.bad {
                ui.label(
                    egui::RichText::new(theme::icon::WARNING)
                        .family(theme::MONO)
                        .size(13.0 * theme::ICON_SCALE)
                        .color(theme::WARNING),
                );
            }
        });
    });
}

/// Row 3 — progress, and nothing else.
///
/// The lane is laid out right-to-left so Cancel and the count take their space from the
/// right edge first and the phase fills whatever is left.
///
/// **There is no track in this row any more.** P13 moved the proportion to a 2px line along
/// the panel's top edge (CORE §4), which is drawn by `status_bar` because only `status_bar`
/// knows where that edge is. What stays here is what a track could never carry: the phase,
/// the count, and the Cancel that is the only user-reachable writer to `app.cancel` in the
/// program — which is also why this row could not simply be deleted.
fn sb_progress(app: &Indium, ui: &mut egui::Ui) {
    // Copied out before the row opens: the closure needs `app.cancel` while it holds a
    // reading of `app.progress`, and a label is three words.
    let running = app
        .progress
        .as_ref()
        .map(|p| (p.done, p.total, p.label.clone()));

    sb_row(ui, egui::Layout::right_to_left(egui::Align::Center), |ui| {
        // The track is `extreme_bg_color`, which `install_visuals` sets to `WINDOW` —
        // the colour of the entry table's well, and *lighter* than the floor this bar
        // paints. A rail lighter than the surface it lies on reads as raised, which is
        // exactly backwards for a groove that something fills up. `VOID` is the only
        // ground darker than `STATUS_BAR`, so the track recedes and the orange sits
        // down in it. Scoped to this row: `visuals_mut` is clone-on-write per `Ui`, and
        // every text field in the program wants the lighter well (theme §Base).
        ui.visuals_mut().extreme_bg_color = theme::VOID;

        if let Some((done, total, label)) = running {
            // Orange with a Cancel beside it is CORE §6's third permitted meaning —
            // Apply/progress — and this is the only place in the window that draws it.
            if theme::button(ui, "Cancel".into(), true).clicked() {
                app.cancel.store(true, Ordering::Relaxed);
            }
            ui.label(
                egui::RichText::new(format!("{done}/{total}"))
                    .family(theme::MONO)
                    .size(13.0)
                    .color(theme::TEXT),
            );
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                // Row 3's subject: what is happening. Bold, per CORE §4.
                ui.label(
                    egui::RichText::new(label)
                        .family(theme::bold())
                        .size(13.0)
                        .color(theme::TEXT),
                );
                // **The track is not here any more.** CORE §4: "The proportion done is
                // drawn as a 2px line along the bar's own top edge, not as a track inside
                // the row: it is the one measurement in the window that wants the whole
                // width, and the edge is already there." `status_bar` paints it, because
                // only `status_bar` knows where the panel's edge is.
                //
                // What stays is what a track could never carry anyway: the phase, the
                // count, and the Cancel that is the only user-reachable writer to
                // `app.cancel` in the program.
            });
        } else if app.listing {
            // `progress` is consulted first and `listing` second, as the old bar did:
            // `E` works while a listing is still streaming in, and when both are true
            // the Cancel has to reach the worker that is writing files.
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                // Sized explicitly: a `Spinner` left to itself takes
                // `interact_size.y`, which is a whole SB_ROW, and a lane-height
                // spinner is the tallest thing in a bar that is otherwise all text.
                ui.add(egui::Spinner::new().color(theme::ORANGE).size(14.0));
                let n = app.entries.len();
                ui.label(
                    egui::RichText::new(format!(
                        "Reading… {n} {}",
                        if n == 1 { "entry" } else { "entries" }
                    ))
                    .family(theme::MONO)
                    .size(13.0)
                    .color(theme::TEXT_SECONDARY),
                );
            });
        } else {
            // Idle. A hairline down the middle of an empty lane, because a row that is
            // simply blank reads as a layout mistake, and the emptiness is the answer:
            // nothing is running. CORE §6 — 1px hairlines, inside a zone.
            let lane = ui.available_rect_before_wrap();
            ui.painter()
                .hline(lane.x_range(), lane.center().y, theme::hairline());
        }
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
            // Named, because `caret_to_end` below has to find this field's state, and
            // `lock_focus` so `Tab` completes the path instead of leaving the field.
            let field = egui::Id::new("open-path-field");
            let resp = ui.add(
                egui::TextEdit::singleline(&mut app.open_path)
                    .id(field)
                    .lock_focus(true)
                    .font(egui::TextStyle::Monospace)
                    .desired_width(f32::INFINITY),
            );
            if app.wants_initial_focus(&Popup::OpenPath) {
                resp.request_focus();
            }

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
                    caret_to_end(ctx, field, &app.open_path);
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
                        app.status = Status::bad(format!("{} is not a file.", path.display()));
                    }
                }

                // The desktop's own picker, beside the field rather than instead of it.
                // `Ctrl+O` still opens what CORE §4's keyboard table says it opens — a
                // path field with tab completion — and rebinding the chord to raise a
                // dialog would have changed documented behaviour to add a button's worth
                // of convenience. Two ways in, one of them typed, neither hidden.
                if ui.button("Browse…").clicked() {
                    app.popup = None;
                    app.request_picker(ctx, PickerFor::Open);
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

    /// The one thing `exact_size` cannot tell you itself.
    ///
    /// A `Panel` clamps the *reported* rect to `outer_size_range.max` and then paints
    /// whatever its content needed, so a status bar four pixels too tall reports 96 and
    /// silently eats its own gutter. The height is therefore checked against the frame it
    /// is a height *of* — which fails if the edge gets thicker, if `PAD` or `GUTTER`
    /// move, if a fourth row is added, or if a future egui changes what `total_margin`
    /// counts.
    #[test]
    fn the_status_bar_is_as_tall_as_it_says() {
        let content = 3.0 * theme::SB_ROW + 2.0 * theme::SB_GAP;
        assert_eq!(content + sb_frame().total_margin().sum().y, SB_HEIGHT);
    }

    #[test]
    fn a_dotfile_archive_keeps_its_name() {
        // ".hidden.zip" must not stem to the empty string.
        assert_eq!(
            archive_stem(std::path::Path::new("/x/.hidden.zip")),
            ".hidden.zip"
        );
    }

    fn key(key: egui::Key, pressed: bool, modifiers: egui::Modifiers) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers,
        }
    }

    /// Held down: what egui-winit emits on the way *down*, which is nothing we may use.
    fn down(k: egui::Key) -> egui::Event {
        key(k, true, egui::Modifiers::CTRL)
    }

    /// Let go: the only signal that survives every clipboard state.
    fn up(k: egui::Key) -> egui::Event {
        key(k, false, egui::Modifiers::CTRL)
    }

    /// The bug `v1.0.0-2` did **not** fix, as a test.
    ///
    /// This is the exact event list egui-winit produces for `Ctrl+V` when a file manager
    /// has copied a *file*: the clipboard offers `text/uri-list` and no plain text, so
    /// `clipboard.get()` returns `None`, the `Paste` is never pushed, and the early
    /// return eats the `Key` as well. The whole press is empty. P10 watched the press and
    /// therefore shipped paste-to-stage still dead.
    #[test]
    fn the_paste_chord_is_read_on_release_because_the_press_is_empty() {
        assert_eq!(clipboard_chords(&[]), (false, false), "the press: nothing");
        assert_eq!(clipboard_chords(&[up(egui::Key::V)]), (false, true));
    }

    /// Copy is read the same way, though its press does carry `Event::Copy`. One rule for
    /// both chords is worth more than a rule per chord that happens to work today.
    #[test]
    fn the_copy_chord_is_read_on_the_same_signal() {
        assert_eq!(clipboard_chords(&[up(egui::Key::C)]), (true, false));
    }

    /// The press forms must be ignored, and this is the test that says why.
    ///
    /// When the clipboard *does* hold text, egui-winit pushes `Event::Paste` on the way
    /// down **and** the release still arrives on the way up. Answering both would stage
    /// the same paste twice, and `paste_rx.is_some()` cannot prevent it — the clipboard
    /// read is finished long before a key comes back up.
    #[test]
    fn a_press_never_acts_so_a_text_clipboard_cannot_act_twice() {
        assert_eq!(clipboard_chords(&[egui::Event::Copy]), (false, false));
        assert_eq!(
            clipboard_chords(&[egui::Event::Paste("/tmp/whatever".to_string())]),
            (false, false)
        );
        assert_eq!(clipboard_chords(&[down(egui::Key::C)]), (false, false));
        assert_eq!(clipboard_chords(&[down(egui::Key::V)]), (false, false));
    }

    /// One whole chord, in the order a window actually receives it, acts exactly once.
    #[test]
    fn a_complete_chord_acts_exactly_once() {
        let frame = [
            egui::Event::Paste("/tmp/whatever".to_string()),
            down(egui::Key::V),
            up(egui::Key::V),
            key(egui::Key::V, false, egui::Modifiers::NONE),
        ];
        assert_eq!(clipboard_chords(&frame), (false, true));
    }

    /// `Ctrl+X` is not in CORE §4's table, and a bare `C` is a shortcut for nothing.
    /// Neither may reach the clipboard path by accident.
    #[test]
    fn nothing_else_is_mistaken_for_a_clipboard_chord() {
        assert_eq!(clipboard_chords(&[egui::Event::Cut]), (false, false));
        assert_eq!(clipboard_chords(&[up(egui::Key::X)]), (false, false));
        assert_eq!(
            clipboard_chords(&[key(egui::Key::C, false, egui::Modifiers::NONE)]),
            (false, false)
        );
        assert_eq!(clipboard_chords(&[]), (false, false));
    }
}
