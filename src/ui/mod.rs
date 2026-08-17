//! The window: sidebar, table, Inspector, status bar, and every popup.
//!
//! CORE §4: "Five fixed zones and ten popups. Nothing else appears, ever." P2 §5 added
//! the password prompt by the maker's ordered CORE edit; P4 fills in the two the count
//! always allowed for — Create and Pending tasks — and puts rename in the table
//! rather than making it another. P12 numbers the two §4 had been running without: Open,
//! which the keyboard table has carried since P1, and Keys. P21b raises the count to ten,
//! by the maker's lifting of the cap, for Measure — the first popup to stand over another.

pub mod about;
pub mod extract;
pub mod filter;
pub mod inspector;
pub mod keys;
pub mod measure;
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
use crate::estimate;
use crate::model::{self, Row};
use crate::platform::apps::{self, Candidate};
use crate::platform::clipboard;
use crate::platform::picker::{self, PickerFor};
use crate::platform::scratch::{self, Scratch};
use crate::platform::store::{self, ExtractDefault, Recents, Settings, Store};
use crate::platform::window;
use crate::secret::Secret;
use crate::tasks::{self, ApplyMsg, Draft, Queue, Task};
use crate::theme;

/// What a measurement was drawn from — P21.
///
/// Held and shown rather than inferred, because "54%" with no statement of what was
/// weighed is the same species of claim as the folklore CORE §7 sent V2.0 to replace.
pub struct EstimateOf {
    /// The input, in words: `"12 staged items"`, or an archive's filename.
    pub describe: String,
    /// True when the input was over budget and only a sample was compressed. Every figure
    /// drawn from it is marked wherever it appears.
    pub sampled: bool,
    /// How many bytes the candidates were actually handed.
    pub bytes: u64,
}

/// Where a measurement's bytes come from. Resolved on the UI thread, read on the worker.
pub enum EstimateSource {
    /// Paths staged for adding, with the names they will take inside the archive.
    Staged(Vec<(PathBuf, String)>),
    /// The open archive. The listing rides along so the worker needs only one read pass.
    Archive { path: PathBuf, entries: Vec<Entry> },
}

/// Which centre view the sidebar has selected. P2 §2: "the sidebar selects what the
/// centre shows".
///
/// `Archive` became `File` in P22, when the row it names did: a section that is enterable
/// with nothing open is not "the archive", it is where the archive would be. `Draft` is the
/// new one, and it is a section rather than a zone — CORE §4 still says *five fixed zones*,
/// and the draft is drawn in the entry table's, as the two lists already are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    File,
    Draft,
    Recents,
    Bookmarks,
}

/// CORE §4: two tabs, toggled with `Space`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectorTab {
    Details,
    Preview,
}

/// The popups — ten of them, and this is all of them: rename happens in the table
/// rather than in another popup.
///
/// CORE §4's numbered list once stopped at seven and did not carry `OpenPath`, while §4's
/// keyboard table had carried `Ctrl+O` since P1 and the window behind it had been a real
/// `egui::Window` for just as long: the document ordered the mechanism and forgot to
/// number the window it opens. This comment used to claim seven and count eight, which was
/// the wrong half to leave standing. P12 applied the edit `build/docs/P6.md` ordered, so
/// Open is numbered eighth, and added `Keys` as the ninth — the list is now the same length
/// in both places. P21b adds the tenth, `Measure`, by the maker's lifting of the cap; it is
/// the only one that stands *over* another, in [`Indium::over`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Popup {
    Create,
    PendingTasks,
    Extract,
    About,
    Settings,
    OpenPath,
    Password,
    OpenWith,
    Keys,
    Measure,
}

impl Popup {
    /// Every variant, in CORE §4's numbered order.
    ///
    /// It exists to be counted. `std::mem::variant_count` is nightly-only, so the gate that
    /// holds this enum against CORE §4's numbered list — `the_popup_list_and_core_agree_about_
    /// how_many_there_are` — needs a list it can take the length of. Adding a variant without
    /// adding it here, or without numbering it in the document, fails that test either way,
    /// which is the drift the whole arrangement is for.
    pub const ALL: &'static [Popup] = &[
        Popup::Create,
        Popup::PendingTasks,
        Popup::Extract,
        Popup::OpenWith,
        Popup::Settings,
        Popup::About,
        Popup::Password,
        Popup::OpenPath,
        Popup::Keys,
        Popup::Measure,
    ];
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
    /// Pull the current selection across into the draft.
    ///
    /// Carries nothing, and could not usefully: the resume re-calls
    /// [`Indium::bring_from_archive`], which reads the selection off the window exactly as
    /// the first press did. That is the same shape as `CopyOut` and for the same reason —
    /// a password prompt does not change what is selected.
    Draft,
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
    /// *Bring from archive* — put what came out into the draft, under the names it had
    /// inside the archive.
    ///
    /// The only one of the four that carries its directory. The other outward path finds
    /// its files by asking `Scratch` for the current directory of its kind, which for a
    /// draft answers with the root shared by every pull — so this pull's own subdirectory
    /// has to travel, both to find the files and to strip the right prefix off them.
    Draft { dir: PathBuf },
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
    /// How far into the member being written, as a fraction of that one member.
    ///
    /// PXX. `done` and `total` are **members**, because row 3 prints them as a count and a
    /// count that said "0/1" while claiming 40% would be a worse lie than the one this
    /// fixes. So the within-member part is carried separately and spent only on the bar,
    /// which is a proportion and can hold it honestly. Zero for everything that reports
    /// per-member only — the listing, the extraction, the estimator — and that is the right
    /// answer for them rather than a missing one.
    pub within: f32,
}

/// Everything the window is.
pub struct Indium {
    // --- the open archive -------------------------------------------------
    pub archive_path: Option<PathBuf>,
    pub archive_bytes: u64,
    pub archive_info: Option<ArchiveInfo>,
    pub entries: Vec<Entry>,
    pub listing: bool,
    /// What the open that is in flight threw away — a count, and the archive it was staged
    /// against — waiting for a status line worth putting it on.
    ///
    /// P22 made opening a close-then-open, so `Ctrl+O` over an archive with four renames
    /// staged discards them, and F7 says the loss is recorded. It cannot be recorded where
    /// it happens: `open_archive` sets *"Reading …"* and `ListMsg::Done` overwrites that
    /// with the archive's name a few milliseconds later, so a sentence said at the close
    /// would be gone before it was read. So the two facts wait here and `Done` composes
    /// them, which is also the line that stays on screen.
    ///
    /// Set by every replacing open and cleared by the listing that lands. A same-archive
    /// re-open leaves it **standing**, because the password prompt's resume is not a second
    /// open — it is this one, continued, and the sentence still belongs to it.
    pub discarded_on_open: Option<(usize, String)>,

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

    // --- the draft (P22) --------------------------------------------------
    /// What the next archive will be made of. Deliberately not a second [`Queue`] — CORE §3
    /// says why — and **the source of truth until Apply succeeds**: the queue's creation
    /// lane is a projection of this, recomputed on every *Create* press. It outlives the
    /// tray's *Discard*, and it outlives the archive its items were pulled from.
    pub draft: Draft,
    /// The draft section's own cursor. Every section has kept its own since P11, so leaving
    /// one and coming back lands where you were.
    pub draft_cursor: usize,
    /// How many pulls this window has made, ever — the number each gets its own
    /// subdirectory under.
    ///
    /// Never reset, and that is the whole design: its only job is that no two pulls collide
    /// under one root, and a counter that only goes up gives that with no reset logic to get
    /// wrong. A fresh root after Apply carrying on at `7` costs nothing and is not a bug.
    draft_pulls: u32,
    /// `Some(path)` while a name is being edited in place. CORE §4 numbers the popups, and
    /// rename is not among them: it is the Name cell becoming a text field.
    pub rename_target: Option<String>,
    pub rename_input: String,

    // --- Create (P4 §5) ----------------------------------------------
    pub new_name: String,
    pub new_dir: String,
    pub new_preset: tasks::Preset,
    pub new_method: tasks::Method,
    pub new_level: u32,
    pub new_advanced: bool,
    pub new_encrypt: bool,
    apply_rx: Option<Receiver<ApplyMsg>>,
    /// What the Apply that is out right now is building, so that [`Indium::on_exit`] can
    /// find the temp beside it. `Some` exactly while `apply_rx` is.
    ///
    /// PXX's finding 4. `tasks::apply` cleans up after itself on every path it can reach —
    /// failure, cancellation, success — and `temp_path_for`'s determinism covers the crash:
    /// one leftover per archive, cleared by the next Apply. What neither covers is the
    /// window being closed while a write is in flight. The process then takes its **own**
    /// exit path, at a moment when the worker is between reads and will never run another
    /// line, and 159 MB is left on the user's disk under a dotted name they never chose.
    /// A killed process cannot clean up after itself; one that exits deliberately can, and
    /// until now did not.
    apply_target: Option<PathBuf>,
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
    /// How many columns the status bar's directory was cut to last frame; see
    /// `sb_what_is_open`. Held so a drag does not re-cut the path on every pixel.
    path_cells: usize,

    // --- Preview (P5) -----------------------------------------------------
    pub preview: Option<PreviewData>,
    /// Set while a head is being read, so the tab can say so rather than look empty.
    pub preview_loading: Option<String>,
    preview_rx: Option<Receiver<PreviewRead>>,

    // --- popups -----------------------------------------------------------
    pub popup: Option<Popup>,
    /// The popup drawn *over* [`Indium::popup`] — P21b, and CORE §4's *"Close the topmost
    /// popup"* becoming literally true for the first time.
    ///
    /// Only ever `Popup::Measure`, and only ever over `Popup::Create`. Typed as the enum
    /// rather than as a `bool` so the pair reads as one stack in every place that touches it,
    /// and so a second over-popup would need no new field.
    ///
    /// It deliberately does **not** take part in `focus_given_to`: that flag is re-armed
    /// against `popup` alone, one line down in `ui()`, so a `wants_initial_focus` keyed on an
    /// over-popup would be reset every frame and grab focus forever. The Measure popup has no
    /// text field, so there is nothing to hand focus to; teaching that line about `over` is
    /// the price of the first over-popup that does.
    pub over: Option<Popup>,
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

    // --- the estimator (P21) ----------------------------------------------
    /// Figures as they land, in the order `METHODS` lists them.
    pub estimates: Vec<estimate::Measurement>,
    /// Candidates that could not be built, and what libarchive said. One failing method
    /// is a row that says so, not a dead popup.
    pub estimate_failed: Vec<(tasks::Method, String)>,
    /// What the last measurement was drawn from. The popup states it, always, because a
    /// figure whose input is unstated is the folklore this round exists to replace.
    pub estimate_of: Option<EstimateOf>,
    estimate_rx: Option<Receiver<estimate::Msg>>,
    /// The estimator's **own** flag, never `self.cancel`. That one belongs to a rebuild
    /// or an extraction, and a measurement being abandoned must not be able to stop one.
    estimate_cancel: Arc<AtomicBool>,
    /// Set while a worker is out. The popup body runs every frame and would otherwise
    /// spawn a second one on every one of them — the same guard `preview_loading` is.
    pub estimate_running: bool,

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
            discarded_on_open: None,

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
            over: None,
            extract_path: String::new(),
            extract_to_subdir: settings.value.extract.default == ExtractDefault::Subdir,
            open_path: String::new(),
            tasks: Queue::new(),
            staged_against: Vec::new(),
            draft: Draft::new(),
            draft_cursor: 0,
            draft_pulls: 0,
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
            apply_target: None,
            paste_rx: None,
            picker_rx: None,
            reveal_rx: None,
            path_cells: 0,
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

            estimates: Vec::new(),
            estimate_failed: Vec::new(),
            estimate_of: None,
            estimate_rx: None,
            estimate_cancel: Arc::new(AtomicBool::new(false)),
            estimate_running: false,

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

    /// Drop everything on screen that belonged to the archive that was open.
    ///
    /// Shared by opening, which is about to draw another archive here, and by closing,
    /// which is not. It is the codebase's own enumeration of what an archive puts on
    /// screen, and it is a list that has grown by discovery rather than by design — so it
    /// lives in one place, where the next thing to join it can only be added once.
    fn reset_view(&mut self, ctx: &egui::Context) {
        // P7 §7's rug-pull: raise the old listing's cancellation flag and hand the window
        // a new one, so a worker that is still walking an archive cannot deliver rows into
        // a table that has moved on.
        self.cancel.store(true, Ordering::Relaxed);
        self.cancel = Arc::new(AtomicBool::new(false));

        self.entries.clear();
        self.selection.clear();
        self.cwd.clear();
        self.cursor = 0;
        self.crc_of = None;
        // The Preview is the other half of what `crc_of` clears, and it was the half this
        // reset missed. `ApplyMsg::Done` re-opens the archive in this window precisely so
        // the Inspector cannot describe a file that no longer exists — and an entry that kept
        // its name through the rebuild kept its `PreviewData` too, so `request_preview`'s
        // path check called it current and went on showing the bytes Apply had just replaced.
        // A computed checksum and a previewed body go stale together; they are dropped
        // together. The texture goes with it, which is `forget_preview`'s own reason to exist.
        self.forget_preview(ctx);
        self.filter = None;
        self.archive_info = None;
    }

    /// Leave the archive this window holds, and report what leaving it cost.
    ///
    /// `None` when there was nothing open. Otherwise the count [`discarded_by_closing`]
    /// gives and the name of what was left, for whichever sentence the caller is writing.
    ///
    /// Beyond [`reset_view`](Self::reset_view) it does three things that only *leaving*
    /// does. It forgets the passphrase, which CORE §9 requires and which the old open
    /// never did because opening is not leaving. It drops the listing in flight —
    /// otherwise a cancelled walk's late `Done` overwrites the sentence this returns with
    /// the name of an archive the window no longer holds. And it empties the tray, because
    /// a tray must not describe changes against a closed archive.
    fn leave_archive(&mut self, ctx: &egui::Context) -> Option<(usize, String)> {
        let name = archive_name(self.archive_path.as_deref()?);

        self.reset_view(ctx);
        self.list_rx = None;
        self.listing = false;
        self.archive_bytes = 0;
        self.archive_path = None;
        self.set_window_title(ctx);
        self.passphrase = None;

        let discarded = discarded_by_closing(self.tasks.creation().is_some(), self.tasks.len());
        if discarded > 0 {
            self.tasks.clear();
        }
        self.staged_against.clear();
        Some((discarded, name))
    }

    /// Close the archive, per CORE §1 as P22 amended it — and say what that cost.
    ///
    /// The control is on the breadcrumb row (`table.rs`), and until this round there was
    /// no control at all: the answer to *"how do I get out of this archive"* was to close
    /// the window, which took every other window's answer with it.
    ///
    /// It refuses while work is running for the same reason opening does — an extraction
    /// narrating an archive the window no longer holds is a worse outcome than a button
    /// that waits — and `work_running` says which of the two is in the way.
    ///
    /// It lands on Recents rather than on the empty File view, because *"nothing open ⇒
    /// Recents"* is already this program's own convention: it is where a launch with no
    /// archive named puts you. Close leaves the window in the state it launches in, and
    /// what you most likely want next is the list of what you had open before.
    pub fn close_archive(&mut self, ctx: &egui::Context) {
        if self.work_running() {
            return;
        }
        let Some((discarded, name)) = self.leave_archive(ctx) else {
            return;
        };
        self.section = Section::Recents;
        self.status = discarded_line(&format!("Closed {name}"), discarded, None).into();
    }

    /// Open an archive **here**, having closed whatever this window held.
    ///
    /// This is reached from seven places — the command line, a drop, `Ctrl+O`, a click or
    /// `Enter` on a recent, the password prompt's List resume, and Apply's own re-open —
    /// and all seven ask [`window::already_open`] the same question, because the rule does
    /// not have seven readings. The call sites were not touched to add it.
    ///
    /// **P22 changed what the rule says.** Until this round a window that already held an
    /// archive answered a second one by spawning a process, so the destination was asked
    /// *before* `work_running`: a second window replaces nothing, so a window busy
    /// extracting could still open a different archive beside itself rather than refuse.
    /// That order is now inverted, and it must be. An in-program open replaces what is
    /// here, so it is exactly the rug-pull P7 §7 added `work_running` to stop — a window
    /// mid-extraction that opened a different archive would leave the extraction narrating
    /// one archive while the table showed another. `open_new` still exists and still means
    /// what it always meant, but it is now the *launcher's* door: a file manager or a
    /// command line naming a second archive gets a second window, and nothing in here does.
    ///
    /// A *listing* is not work in this sense and is still cancelled without ceremony:
    /// nothing has been written, and opening the next archive is the whole of what the
    /// user asked for.
    pub fn open_archive(&mut self, ctx: &egui::Context, path: PathBuf, passphrase: Option<Secret>) {
        if self.work_running() {
            return;
        }

        if window::already_open(self.archive_path.as_deref(), &path) {
            // The archive already here, handed back: Apply's re-open and the password
            // prompt's resume. Nothing is being left, so the tray and the passphrase stay
            // — and so does anything a *previous* open left waiting to be said, because
            // the resume is that open continued rather than a new one.
            self.reset_view(ctx);
        } else {
            self.discarded_on_open = self.leave_archive(ctx).filter(|(n, _)| *n > 0);
        }

        self.archive_bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        self.archive_path = Some(path.clone());
        self.set_window_title(ctx);
        self.section = Section::File;
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
        let estimate_msgs: Vec<estimate::Msg> = match &self.estimate_rx {
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
            || !estimate_msgs.is_empty()
            || !preview_msgs.is_empty();

        for msg in list_msgs {
            match msg {
                ListMsg::Opened(info) => self.archive_info = Some(info),
                ListMsg::Entry(e) => self.entries.push(*e),
                ListMsg::Done { count } => {
                    self.listing = false;
                    self.list_rx = None;
                    let _ = count;
                    let name = self
                        .archive_path
                        .as_deref()
                        .map(archive_name)
                        .unwrap_or_else(|| "Ready.".to_string());
                    // The line that stays on screen, so it is the line F7's sentence gets
                    // to ride on. Composed only when the open discarded something —
                    // otherwise this is the archive's name and nothing else, as it has
                    // been since P5, full stop and all absent.
                    self.status = match self.discarded_on_open.take() {
                        Some((n, against)) => discarded_line(&name, n, Some(&against)).into(),
                        None => name.into(),
                    };
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
                        within: 0.0,
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
                    // The password's job is over. No post-step below needs it: two read a
                    // scratch directory, the third reads `.desktop` files.
                    self.passphrase = None;
                    match std::mem::replace(&mut self.post_extract, PostExtract::None) {
                        PostExtract::None => {}
                        PostExtract::Clipboard { on_disk } => self.finish_copy_out(on_disk),
                        PostExtract::OpenWith { entry } => self.finish_open_with(&entry),
                        PostExtract::Draft { dir } => self.finish_draft_pull(&dir),
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
                // Where the paste lands is the section on screen, as `I` and a drop are.
                Ok(paths) => match self.section {
                    Section::Draft => self.add_to_draft(paths),
                    _ => self.stage_adds(paths),
                },
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
                // CORE §1 as P22 amends it: a window holds one archive at a time, and the
                // one it is given takes this window rather than opening another. The comment
                // here used to say the opposite while the code did this, which is the drift
                // the amendment was written to end.
                PickerFor::Open => {
                    if let Some(first) = paths.into_iter().next() {
                        self.open_archive(ctx, first, None);
                    }
                }
                PickerFor::Add => self.stage_adds(paths),
                PickerFor::Draft => self.add_to_draft(paths),
                // PXX 8.11. The mode is only taken up once a directory is actually named:
                // a cancelled dialog is `Ok(vec![])` — `open_files` says so — and a user
                // who changed their mind has not asked for their default to change.
                //
                // `extract_to_subdir` is cleared here rather than left to `settings.rs`,
                // which syncs it right after a click. This answer does not arrive from a
                // click; it arrives on a channel, frames later, with that row long since
                // drawn. Without this line the popover would preselect the subdirectory
                // it was told to stop preselecting.
                PickerFor::Preselect => {
                    if let Some(dir) = paths.into_iter().next() {
                        let dir = dir.to_string_lossy().to_string();
                        self.change_settings(move |s| {
                            s.extract.preselect = dir;
                            s.extract.default = ExtractDefault::Preselect;
                        });
                        self.extract_to_subdir = false;
                    }
                }
            }
        }

        for msg in apply_msgs {
            match msg {
                ApplyMsg::Progress { phase, done, total } => {
                    // A new member has started, so whatever fraction of the last one was
                    // on the bar is spent. Resetting here rather than in the `Within` arm
                    // is what keeps the two in step without either knowing about the
                    // other: `Progress` is the only thing that moves `done`, and `within`
                    // measures the member after it.
                    self.progress = Some(Progress {
                        done,
                        total,
                        within: 0.0,
                        label: phase.label().to_string(),
                    });
                }
                ApplyMsg::Within { done, total } => {
                    if let Some(p) = self.progress.as_mut() {
                        // Guarded both ways. A zero-length member never gets here — it has
                        // no data to read — but a `total` of zero would divide by it if one
                        // ever did, and a reader handing back more than the size the
                        // metadata promised must not push the bar into the member after it.
                        p.within = if total == 0 {
                            0.0
                        } else {
                            (done as f32 / total as f32).clamp(0.0, 1.0)
                        };
                    }
                }
                ApplyMsg::Done { entries } => {
                    self.progress = None;
                    self.apply_rx = None;
                    self.apply_target = None;
                    self.status = format!(
                        "Applied. The archive now holds {entries} entr{}.",
                        if entries == 1 { "y" } else { "ies" }
                    )
                    .into();
                    // The queue has been spent, and what is on screen is now a listing of
                    // the archive that was replaced. Re-open it rather than leave stale
                    // rows behind — the Inspector is the point of this program, and it
                    // must not describe a file that no longer exists.
                    //
                    // **Which archive to open is the recipe's business when this was a
                    // creation.** Since P22 nothing adopts a path before there is a file at
                    // it, so `archive_path` still names whatever was open while the new one
                    // was being built — or nothing at all. The target has to come from the
                    // queue, and it has to be read before the queue is cleared.
                    let created = self.tasks.creation().map(|r| r.path.clone());
                    let path = created.clone().or_else(|| self.archive_path.clone());
                    self.tasks.clear();
                    self.staged_against.clear();
                    if created.is_some() {
                        // The draft has been spent: it is a file now. This is the one place
                        // it is emptied, and the reason *Discard* can leave it standing.
                        self.draft.clear();
                        self.draft_cursor = 0;
                        // And with it the copies *Bring from archive* made, which have just
                        // been written into the archive and are of no further use to anyone.
                        // The one `discard` of this kind there is: everything else about a
                        // draft's scratch is `ensure`, which removes nothing.
                        self.scratch.discard(scratch::Kind::Draft);
                    }
                    let pass = self.passphrase.take();
                    if let Some(path) = path {
                        self.open_archive(ctx, path, pass);
                    }
                }
                ApplyMsg::Cancelled => {
                    self.progress = None;
                    self.apply_rx = None;
                    self.apply_target = None;
                    self.passphrase = None;
                    // The queue survives a cancel: nothing was written, so the changes
                    // the user staged are still exactly what they asked for.
                    self.status =
                        "Cancelled. Nothing was written, and your changes are still staged.".into();
                }
                ApplyMsg::Failed(msg) => {
                    self.progress = None;
                    self.apply_rx = None;
                    self.apply_target = None;
                    self.passphrase = None;
                    self.status = Status::bad(msg);
                }
            }
        }

        // The estimator — P21. Figures are appended as they land, so the eight rows fill
        // one at a time over about 2.7 s rather than appearing together at the end. That
        // filling is the only progress this popup shows: CORE §6 refuses motion beyond
        // what it already permits, and a row acquiring a number is not an animation.
        for msg in estimate_msgs {
            match msg {
                estimate::Msg::Began {
                    describe,
                    sampled,
                    bytes,
                } => {
                    self.estimate_of = Some(EstimateOf {
                        describe,
                        sampled,
                        bytes,
                    });
                }
                estimate::Msg::One(m) => self.estimates.push(m),
                estimate::Msg::Failed { method, why } => self.estimate_failed.push((method, why)),
                estimate::Msg::Done => {
                    self.estimate_running = false;
                    self.estimate_rx = None;
                }
                estimate::Msg::Fatal(why) => {
                    self.estimate_running = false;
                    self.estimate_rx = None;
                    self.estimate_of = None;
                    self.status = Status::bad(why);
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
            Section::File => rows.len(),
            Section::Draft => self.draft.len(),
            Section::Recents => self.recents.sorted().len(),
            Section::Bookmarks => self.settings.bookmarks.len(),
        }
    }

    /// The cursor belonging to the section on screen.
    pub fn section_cursor(&self) -> usize {
        match self.section {
            Section::File => self.cursor,
            Section::Draft => self.draft_cursor,
            Section::Recents => self.recents_cursor,
            Section::Bookmarks => self.bookmarks_cursor,
        }
    }

    pub fn set_section_cursor(&mut self, i: usize) {
        match self.section {
            Section::File => self.cursor = i,
            Section::Draft => self.draft_cursor = i,
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
        // P21: a measurement belongs to the popup that asked for it. Left running it would
        // spend three seconds of CPU on figures with nowhere to appear, and the Create
        // popup has a second close path of its own — the `X` at `newarchive.rs` — which
        // calls this for exactly that reason rather than clearing `popup` by hand.
        self.cancel_estimate();
        self.popup = None;
        // P21b: the over-popup belongs to the popup underneath it and cannot outlive it.
        self.over = None;
        self.focus_given_to = None;
        self.pending = None;
        self.password_input.clear();
        self.password_confirm.clear();
        self.password_attempts = 0;
    }

    /// Name the open archive in the window title.
    ///
    /// CORE §1 gives a window one archive, so P5 put that archive's name here: the title was
    /// set once at startup and never changed, and every window was labelled `INDIUM` —
    /// identical in the compositor, the switcher and the taskbar, with no way to tell which
    /// held which archive short of focusing it. The information was already on `archive_path`.
    ///
    /// P22 turned that from a convenience into the thing the title is for. A window holds one
    /// archive *at a time* now, and Close leaves it holding none, so the title is no longer a
    /// label fixed at launch — it is the one place outside this window that says what this
    /// window currently is. Which is why `leave_archive` calls this on its way out.
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
        // **P22: a creation closes the door on the archive that is open.** They are two
        // targets and `plan` folds toward one, so a `Rename` pushed into a queue whose
        // target is `backup.7z` fails at Apply — a late failure of exactly the species D5
        // exists to prevent. This used to return `None` here and let it through, which was
        // harmless only because a staged creation cleared `entries` and left no rows for
        // `Del` and `F2` to act on. P22 leaves the archive open and listed, so the guard
        // that was a consequence of an empty table has to be said out loud.
        //
        // The asymmetry with Create is deliberate: Create displaces mutations and says what
        // went, because a person choosing to build a new archive has chosen. A rename cannot
        // displace a creation in the same way — nothing about `F2` says *and throw away the
        // archive I was building* — so this direction refuses.
        if self.tasks.creation().is_some() {
            return Some(
                "A creation is staged — Apply or Discard it before changing this archive."
                    .to_string(),
            );
        }
        let path = self.archive_path.as_ref()?;
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

    /// `F2` — begin editing a name in the table. CORE §4 numbers the popups and rename is
    /// not among them; it is the Name cell becoming a text field.
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

    /// `N` — seed and open the Create popup.
    ///
    /// **Re-openable since P21.** Resetting to Balanced unconditionally was harmless while
    /// `stage_creation` cleared the queue behind it: pressing `N` a second time could only
    /// ever be starting again. Now that a staged creation keeps the files added to it —
    /// which is what lets the estimator measure them — re-opening has to show the recipe
    /// that is *staged*, or the popup would sit there offering to build something other
    /// than the thing actually pending.
    pub fn open_create(&mut self) {
        self.new_advanced = false;

        if let Some(recipe) = self.tasks.creation().cloned() {
            self.new_method = recipe.method;
            self.new_level = recipe.level;
            self.new_encrypt = recipe.encrypt;
            // The chip that would have produced this recipe, if one would have. A method
            // chosen by hand matches none of the four, and then the chips are left as they
            // were rather than one being lit that does not describe what is staged.
            if let Some(preset) = [
                tasks::Preset::Fastest,
                tasks::Preset::Balanced,
                tasks::Preset::Smallest,
                tasks::Preset::Encrypted,
            ]
            .into_iter()
            .find(|p| p.recipe_parts() == (recipe.method, recipe.encrypt))
            {
                self.new_preset = preset;
            }

            self.new_name = newarchive::stem_of(&recipe.path, recipe.method, recipe.encrypt);
            self.new_dir = recipe
                .path
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
        } else {
            self.new_preset = tasks::Preset::Balanced;
            let (method, encrypt) = self.new_preset.recipe_parts();
            self.new_method = method;
            self.new_level = method.default_level();
            self.new_encrypt = encrypt;
            self.new_dir = self.default_extract_dir().to_string_lossy().to_string();
            self.new_name = "archive".to_string();
        }

        self.popup = Some(Popup::Create);
    }

    // -----------------------------------------------------------------------
    // The estimator — P21, CORE §7's V2.0
    // -----------------------------------------------------------------------

    /// Why Measure cannot run, or `None` if it can. The button is dead **and says this** —
    /// an inert control with no explanation is the one thing §4 will not have.
    ///
    /// **Split from `estimate_source` because the popup body runs every frame.** The two
    /// were one function, and the button asked it for its enabled state — which meant
    /// cloning every staged path and, on the archive branch, the *entire entry list*, on
    /// every frame the popup was open. On a hundred-thousand-entry archive that is hundreds
    /// of megabytes a second of allocation spent deciding whether a button is grey. This
    /// half reads counts and flags; the expensive half runs once, on the click.
    ///
    /// The order is D5's. Files staged for adding come first because packaging them is what
    /// the popup is open for; the archive already open comes second because re-compressing
    /// one is the other reason to be here.
    pub fn estimate_refusal(&self) -> Option<&'static str> {
        estimate_refusal_for(
            // **The draft counts, and it is why P22 exists.** Before *Create* is pressed
            // there are no tasks at all — the files live in the draft — and that is exactly
            // the moment the popup now opens in. Folded into this one boolean rather than
            // given a fifth parameter, because the existing order already produces the right
            // sentence: a full draft short-circuits before `unread`, and an empty one with
            // nothing open falls to "Add files, or open an archive", which is the correct
            // words already. After a Create the queue's adds are the draft's projection, so
            // the two halves of this `||` say the same thing.
            !self.draft.is_empty()
                || self
                    .tasks
                    .tasks()
                    .iter()
                    .any(|t| matches!(t, Task::Add { .. })),
            self.archive_path.is_some(),
            // **One fact, not two.** `archive_info` is what a successful listing produces, so
            // its absence is exactly "nothing at this path has ever been read" — the phantom
            // `stage_creation` adopts, the phantom that outlives `discard_tasks`, a listing
            // still in flight, and a listing that failed. The first version of this line read
            // `creation().is_some() && archive_info.is_none()`, which described only the
            // first of those: `discard_tasks` clears the queue and leaves `archive_path`
            // standing, so `N` → Create → Discard put the button back over a file that had
            // never been written. The walk is built from `entries`, and in none of these four
            // is `entries` something to walk: the two phantoms and a listing that has not
            // reached `Opened` have it empty, and `on_list_failure` leaves behind whatever had
            // streamed in before the failure — a partial list of an archive we have just been
            // told we cannot read. Refusing is right in all four, for the same reason twice.
            //
            // Note what this deliberately does *not* gate: `Opened` arrives before the
            // entries do, so once the header is read Measure goes live over a listing still
            // streaming. That is unchanged from P21 and is not a phantom — the file is there
            // and readable, and `walk` caps itself anyway.
            self.archive_info.is_none(),
            self.entries.iter().any(|e| e.encrypted) && self.passphrase.is_none(),
        )
    }

    /// The bytes themselves, resolved once — on the click, never on the frame.
    ///
    /// See [`estimate_refusal_for`] for why the decision above it is a free function.
    pub fn estimate_source(&self) -> Result<EstimateSource, &'static str> {
        if let Some(why) = self.estimate_refusal() {
            return Err(why);
        }
        // The draft first, because it is the source of truth. Before *Create* it is the only
        // thing holding anything, which is what makes Measure live on the popup's first
        // frame; after *Create* the queue's adds are its projection and the two say the same
        // thing, so which one is read cannot matter.
        let adds: Vec<(PathBuf, String)> = if !self.draft.is_empty() {
            self.draft
                .items()
                .iter()
                .map(|i| (i.source.clone(), i.dest.clone()))
                .collect()
        } else {
            self.tasks
                .tasks()
                .iter()
                .filter_map(|t| match t {
                    Task::Add { source, dest } => Some((source.clone(), dest.clone())),
                    _ => None,
                })
                .collect()
        };
        if !adds.is_empty() {
            return Ok(EstimateSource::Staged(adds));
        }
        let Some(path) = self.archive_path.clone() else {
            return Err("Add files, or open an archive: there is nothing to measure yet.");
        };
        Ok(EstimateSource::Archive {
            path,
            entries: self.entries.clone(),
        })
    }

    /// Hand the eight candidates to a worker.
    ///
    /// One worker and one candidate at a time — CORE §3 fixes threading at "the UI thread
    /// and one worker", so the eight cost around three and a half seconds at the budget
    /// rather than under a second spread over the cores. That is the price of the contract,
    /// paid deliberately, and it is why nothing here starts on
    /// its own: §4.1's Measure is a button, and a popup that spent three seconds of CPU
    /// every time it opened would be a worse program than one that asserts.
    pub fn request_estimate(&mut self, ctx: &egui::Context) {
        // The popup body runs every frame, so without this the button's `clicked()` would
        // be the only thing between here and eight workers.
        if self.estimate_running {
            return;
        }
        // A rebuild or an extraction is real work; measuring is advisory, and advisory
        // work does not get to compete for the disk with the thing the user is waiting on.
        if self.work_running() {
            return;
        }

        let source = match self.estimate_source() {
            Ok(source) => source,
            Err(why) => {
                self.status = Status::bad(why);
                return;
            }
        };

        // `begin` removes the previous measurement's directory, which is the whole reason
        // to route through `Scratch` rather than inventing a temp path: the routing rule,
        // the process id in the name, the launch sweep and the drop guard all come with it.
        let placement = match self
            .scratch
            .begin(scratch::Kind::Estimate, estimate::BUDGET)
        {
            Ok(placement) => placement,
            Err(e) => {
                self.status = Status::bad(format!("Could not make a scratch directory: {e}"));
                return;
            }
        };

        self.estimates.clear();
        self.estimate_failed.clear();
        self.estimate_of = None;
        self.estimate_cancel = Arc::new(AtomicBool::new(false));
        self.estimate_running = true;

        let (tx, rx) = channel();
        self.estimate_rx = Some(rx);

        let cancel = Arc::clone(&self.estimate_cancel);
        let dir = placement.dir;
        let selected = Some((self.new_method, self.new_method.clamp_level(self.new_level)));
        let pass = self.passphrase.clone();
        let ctx2 = ctx.clone();

        std::thread::spawn(move || {
            let wake = || ctx2.request_repaint();
            let (describe, resolved) = match source {
                EstimateSource::Staged(adds) => {
                    let describe = match adds.len() {
                        1 => "1 staged item".to_string(),
                        n => format!("{n} staged items"),
                    };
                    (
                        describe,
                        estimate::from_staged(&adds)
                            .and_then(|(members, total)| estimate::narrow(members, total)),
                    )
                }
                EstimateSource::Archive { path, entries } => {
                    let describe = path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "the open archive".to_string());
                    (
                        describe,
                        estimate::from_archive(&path, &entries, pass.as_ref(), &cancel),
                    )
                }
            };

            // Each branch above already produced a finished `Input`: staged files are
            // narrowed after the walk because they can be re-read at will, an archive during
            // it because its bytes exist only while the walk is standing on them. Narrowing
            // here instead would have run the archive's reduction a second time.
            match resolved {
                Ok(input) if input.is_empty() => {
                    let _ = tx.send(estimate::Msg::Fatal(
                        "There are no bytes to measure — everything here is empty.".to_string(),
                    ));
                    wake();
                }
                Ok(input) => estimate::run(input, describe, dir, selected, &tx, &cancel, &wake),
                Err(why) => {
                    let _ = tx.send(estimate::Msg::Fatal(why));
                    wake();
                }
            }
        });
    }

    /// Stop a measurement and forget it — including the figures it already reported.
    ///
    /// The figures go because a measurement that outlives the data it measured is folklore
    /// again, which is the thing this whole round exists to remove. The popup closes, the
    /// staged files change, Apply rebuilds the queue — and the numbers still on screen would
    /// describe an input that is no longer there while looking exactly like ones that do.
    /// Nothing here is cached between openings: measuring is a button, and pressing it again
    /// costs the three seconds it honestly costs.
    ///
    /// The scratch directory is deliberately **not** removed here. The worker is still
    /// inside it, and pulling the directory out from under it would turn an orderly
    /// abandonment into a fistful of I/O errors; `measure` deletes its own candidate file
    /// on every path including this one, and the empty directory goes on the next `begin`
    /// or on `Scratch`'s drop, both of which already do exactly that.
    /// True while anything from a measurement is still held — a worker on its way back,
    /// figures, refusals, or the statement of what was weighed.
    ///
    /// One predicate rather than four inlined tests, because the sweeper in `ui()` asks this
    /// on every frame and the answer has to mean *exactly* what `cancel_estimate` clears. A
    /// field added to one and forgotten in the other is how a stale figure survives a close.
    pub fn holds_estimate(&self) -> bool {
        self.estimate_running
            || !self.estimates.is_empty()
            || !self.estimate_failed.is_empty()
            || self.estimate_of.is_some()
    }

    pub fn cancel_estimate(&mut self) {
        self.estimate_cancel.store(true, Ordering::Relaxed);
        self.estimate_rx = None;
        self.estimate_running = false;
        self.estimates.clear();
        self.estimate_failed.clear();
        self.estimate_of = None;
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
        // P21: real work preempts a measurement. Create is an `egui::Window` rather
        // than a `Modal`, so the tray strip stays clickable underneath it and Apply can
        // genuinely start while the eight candidates are running. The estimate is advisory
        // and the rebuild is not, so the advisory one goes.
        self.cancel_estimate();
        self.cancel.store(true, Ordering::Relaxed);
        self.cancel = Arc::new(AtomicBool::new(false));

        let (tx, rx) = channel();
        self.apply_rx = Some(rx);
        self.progress = Some(Progress {
            done: 0,
            total: self.tasks.len(),
            within: 0.0,
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

        // Recorded before the worker exists, from the same value the worker will build
        // into, so `on_exit` cannot be told a target this Apply never had.
        self.apply_target = Some(input.target.clone());

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

    /// Put every path given into the draft, under the name it will take inside the archive.
    ///
    /// A draft has no current directory to add *into* — it is the whole archive-to-be — so a
    /// file takes its own name, where [`Indium::stage_adds`] prefixes the breadcrumb's.
    ///
    /// One item per name, so a second file called `notes.txt` replaces the first rather than
    /// joining it: the fold would resolve that collision at Apply by keeping one of them
    /// silently, and a loss a person can see is worth more than one they cannot.
    pub fn add_to_draft(&mut self, paths: Vec<PathBuf>) {
        if paths.is_empty() {
            return;
        }
        let (paths, folders) = split_out_folders(paths);
        let left_out = folders_note(folders);
        let mut added = 0usize;
        let mut replaced: Vec<String> = Vec::new();
        for path in paths {
            let Some(name) = path.file_name().map(|n| n.to_string_lossy().to_string()) else {
                continue;
            };
            if self.draft.add(path, name.clone()).is_some() {
                replaced.push(name);
            } else {
                added += 1;
            }
        }
        let s = |n: usize| if n == 1 { "" } else { "s" };
        let note = restage_note(self.tasks.creation().is_some());
        self.status = match (added, replaced.len()) {
            // Nothing landed. Silent when nothing was offered either — but when folders were,
            // that is exactly the case 6.2 was denied for, and it is the one that must speak.
            (0, 0) if folders == 0 => return,
            (0, 0) => format!("Draft: nothing added.{left_out}").into(),
            (n, 0) => format!("Draft: {n} file{} added.{left_out}{note}", s(n)).into(),
            (0, 1) => format!("Draft: {} replaced.{left_out}{note}", replaced[0]).into(),
            (0, r) => format!("Draft: {r} files replaced.{left_out}{note}").into(),
            (n, r) => format!(
                "Draft: {n} file{} added, {r} replaced.{left_out}{note}",
                s(n)
            )
            .into(),
        };
    }

    /// Stage an add for every path given, landing each at the current directory.
    pub fn stage_adds(&mut self, paths: Vec<PathBuf>) {
        if paths.is_empty() {
            return;
        }
        let (paths, folders) = split_out_folders(paths);
        let mut staged = 0usize;
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
            staged += 1;
        }
        // `stage` has already said what it staged, one task at a time; this only speaks when
        // there is something it could not know about. Overwriting rather than appending is
        // deliberate — the last task's summary is the least interesting of the batch, and
        // what the walker needs to read is why the folder they named is not in the list.
        // The refusal check is what keeps this from talking over a worse problem: when
        // staging is refused nothing was pushed, `stage` has already said why, and a folder
        // that was left out is the smaller half of that news.
        if folders > 0 && self.staging_refusal().is_none() {
            let s = |n: usize| if n == 1 { "" } else { "s" };
            self.status = match staged {
                0 => format!("Nothing staged.{}", folders_note(folders)).into(),
                n => format!("Staged {n} add{}.{}", s(n), folders_note(folders)).into(),
            };
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
        let (title, multiple, directory) = match what {
            PickerFor::Open => ("Open archive", false, false),
            PickerFor::Add => ("Add to archive", true, false),
            PickerFor::Draft => ("Add to draft", true, false),
            PickerFor::Preselect => ("Choose the preselect directory", false, true),
        };
        std::thread::spawn(move || {
            let _ = tx.send((what, picker::open_files(title, multiple, directory)));
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

    /// Walk out of the current directory, landing on the one just left.
    ///
    /// PXX 3.2: this used to drop the cursor at row 0, so backing out of `a/b/c` put you
    /// at the top of `a/b` with `c` somewhere below — and climbing two levels meant
    /// finding your place twice. The row to land on is the directory being left, which
    /// `rows()` can be asked for directly: it is a pure function of `entries` and `cwd`,
    /// so reading it here, after the change, gives the new listing rather than the old.
    ///
    /// `unwrap_or(0)` keeps the old behaviour for the case it was right for — a filter is
    /// showing, or the parent does not list the child — rather than leaving the cursor
    /// past the end.
    pub fn ascend(&mut self) {
        let Some(parent) = model::parent_of(&self.cwd) else {
            return;
        };
        let left = std::mem::replace(&mut self.cwd, parent);
        self.cursor = self.rows().iter().position(|r| r.path == left).unwrap_or(0);
        // A deep directory can put that row well off screen, and a cursor nobody can see
        // is the defect this was reported as in the first place.
        self.scroll_to_cursor = true;
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
        // Same preemption Apply makes, for the same reason: an extraction is work the user
        // is waiting on, and a measurement is not.
        self.cancel_estimate();
        self.cancel = Arc::new(AtomicBool::new(false));
        let (tx, rx) = channel();
        self.extract_rx = Some(rx);
        self.post_extract = post;
        self.progress = Some(Progress {
            done: 0,
            total: wanted.len(),
            within: 0.0,
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

    /// *Bring from archive* — extract the selected entries to scratch, and put what lands
    /// into the draft.
    ///
    /// The one new capability of P22, and the answer to a question the draft otherwise could
    /// not answer: **an entry inside an archive is not a file.** A draft holds files, so
    /// something has to make one first, and this is the route `copy_out` already walks —
    /// only what comes out is handed to the draft rather than to the clipboard.
    ///
    /// What it makes are **copies**, deliberately, and that is what makes F6's *"the draft
    /// survives a Close"* true rather than a promise the engine could not keep: the files
    /// exist on their own from the moment they land, and closing the archive they came from
    /// does not touch them.
    pub fn bring_from_archive(&mut self, ctx: &egui::Context) {
        if let Some(refusal) = pull_refusal_for(!self.has_archive(), self.selection.is_empty()) {
            self.status = Status::bad(refusal);
            return;
        }
        // The selection, and nothing else — no cursor fallback the way `subject_paths` has
        // one. The button lives on the Draft view, where the cursor is over draft rows and
        // has nothing to say about the archive.
        let wanted: std::collections::HashSet<String> = self.selection.iter().cloned().collect();

        if self.passphrase.is_none() && self.any_encrypted(&wanted) {
            self.pending = Some(PendingAction::Draft);
            self.popup = Some(Popup::Password);
            self.password_input.clear();
            self.password_attempts = 0;
            return;
        }

        let Some(archive) = self.archive_path.clone() else {
            return;
        };
        // Asked before the scratch directory is touched, as `copy_out` does — though here
        // `ensure` would not remove a running worker's directory, because it removes
        // nothing. What it guards is the one worker CORE §3 allows.
        if self.work_running() {
            return;
        }
        let total = self.uncompressed_total(&wanted);

        let root = match self.scratch.ensure(scratch::Kind::Draft, total) {
            Ok(dir) => dir,
            Err(e) => {
                self.status = Status::bad(format!("Could not make a scratch directory: {e}"));
                return;
            }
        };
        self.draft_pulls += 1;
        let dir = root.join(self.draft_pulls.to_string());
        // Made here rather than left to the worker so that a root that cannot be written is
        // a sentence now, in the same press, instead of an extraction error later.
        if let Err(e) = std::fs::create_dir_all(&dir) {
            self.status = Status::bad(format!("Could not make a scratch directory: {e}"));
            return;
        }

        self.status = "Bringing files across…".to_string().into();
        self.spawn_extract(
            ctx,
            archive,
            wanted,
            dir.clone(),
            PostExtract::Draft { dir },
        );
    }

    /// The other half of `bring_from_archive`, run when the worker reports.
    ///
    /// Each file's name in the draft is the path it had **inside the archive**, recovered by
    /// stripping this pull's directory off the front. `add_to_draft` takes a disk file's
    /// `file_name` for the same reason in reverse: a file on disk has an absolute path that
    /// means nothing inside an archive, and an entry has one that means everything. Each
    /// keeps the name it actually has. Pulling a *directory* needs no special case — the
    /// extraction expanded it, and stripping the prefix recovers `docs/notes.txt` for a file
    /// that was never named in the selection.
    ///
    /// **A cancelled or failed pull needs no cleanup.** Nothing was added to the draft, so
    /// nothing dangles, and the orphaned subdirectory goes with the root — at the Drop guard
    /// on a clean exit, at the launch sweep otherwise.
    fn finish_draft_pull(&mut self, dir: &std::path::Path) {
        let files = collect_files(dir);
        if files.is_empty() {
            self.status = Status::bad("Nothing came across.");
            return;
        }

        let mut added = 0usize;
        let mut replaced = 0usize;
        for file in files {
            let Ok(rel) = file.strip_prefix(dir) else {
                continue;
            };
            let dest = rel.to_string_lossy().to_string();
            if self.draft.add(file.clone(), dest).is_some() {
                replaced += 1;
            } else {
                added += 1;
            }
        }

        let s = |n: usize| if n == 1 { "" } else { "s" };
        let note = restage_note(self.tasks.creation().is_some());
        self.status = match (added, replaced) {
            (0, 0) => Status::bad("Nothing came across."),
            (n, 0) => format!("Draft: {n} file{} brought across.{note}", s(n)).into(),
            (0, r) => format!("Draft: {r} file{} replaced.{note}", s(r)).into(),
            (n, r) => format!(
                "Draft: {n} file{} brought across, {r} replaced.{note}",
                s(n)
            )
            .into(),
        };
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

    /// Where the Extract popover opens pointing — which is not always beside the archive.
    ///
    /// PXX 8.11: `Preselect` names one directory for every archive, so it answers before
    /// the archive's own parent is consulted. An empty path is not a directory, and a
    /// hand-edited `settings.toml` can say `default = "preselect"` while naming nowhere,
    /// so that case falls back to the walk the other two modes take rather than offering
    /// the window an empty string.
    ///
    /// Deliberately **not** folded into `default_extract_dir`, which the Create popup
    /// also calls for where a new archive is *written*. A rule about where things come
    /// out is not a rule about where things are made, and one function answering both
    /// questions is how it would quietly become one.
    pub fn extract_destination(&self) -> PathBuf {
        resolve_extract_destination(
            self.settings.extract.default,
            &self.settings.extract.preselect,
            || self.default_extract_dir(),
        )
    }
}

/// The rule `Indium::extract_destination` applies, with nothing of the window in it.
///
/// **Free rather than a method, and that is the point** — the same reason `clipboard_chords`
/// is free. `Indium::new` wants an `eframe::CreationContext`, which no test can conjure, so
/// a rule that lives only on the struct is a rule no test can reach. PXX 8.11 made this one
/// worth reaching: it decides between the directory the user named once and the one derived
/// from the archive, and getting it wrong sends extractions somewhere nobody asked for.
///
/// `derived` is a closure rather than a value so that the archive walk is not paid for when
/// `Preselect` answers first.
fn resolve_extract_destination(
    default: ExtractDefault,
    preselect: &str,
    derived: impl FnOnce() -> PathBuf,
) -> PathBuf {
    if default == ExtractDefault::Preselect {
        let chosen = preselect.trim();
        if !chosen.is_empty() {
            return PathBuf::from(chosen);
        }
    }
    derived()
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

/// What an archive is called on screen: its file name, whole, extensions and all.
///
/// Every sentence that names an archive wants this and not [`archive_stem`] — a person who
/// opened `photos.zip` is not told about `photos`. The fallback matters for the same reason
/// [`archive_stem`]'s does: a path can end in `..` or `/` and have no file name at all, and
/// a status line that then said nothing would be worse than one that said the path.
pub fn archive_name(path: &std::path::Path) -> String {
    path.file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy()
        .to_string()
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

/// The status line for a door that loses staged work.
///
/// CORE §4, and the maker's ruling on it: both doors that can lose a queue act immediately —
/// nothing to dismiss, nothing to decide — and say what went. **One function because it is
/// one ruling.** A program that guards your staged renames when you press Create and drops
/// them without a word when you press Close is a rule nobody could hold in their head, and
/// the two sentences were about to be written separately.
///
/// `against` names the archive the lost changes described, when the sentence has room for it.
/// Create is displacing work aimed somewhere else and can say where; Close is taking that
/// archive with it and has already named it in the headline.
///
/// Free rather than a method, for [`estimate_refusal_for`]'s reason: `Indium` cannot be built
/// in a unit test, and a sentence nobody can reach is a sentence nobody checks.
/// What a change to the draft owes the person when a creation is already staged.
///
/// The projection is recomputed when *Create* is pressed and at no other time, which is what
/// lets the draft be edited without the queue moving underneath it — but it means a file
/// added after a Create is in the draft and *not* in the creation the tray is offering to
/// build. Silently is the one way that must not happen: Apply would succeed, and the archive
/// would simply not contain the file the person watched themselves add.
///
/// A sentence rather than an automatic restage, because CORE §4 says the projection is
/// recomputed on a press and a queue that rebuilt itself under a staged recipe would make
/// that false. Empty when nothing is staged, so it costs the ordinary case nothing.
pub fn restage_note(creation_staged: bool) -> &'static str {
    if creation_staged {
        " Press N again to restage the creation."
    } else {
        ""
    }
}

/// Split what arrived into the files INDIUM will take, and a count of the folders it will not.
///
/// PXX's certification walk denied 6.2 over a silence: *Add files…* raises the portal, the
/// portal is asked for files and hands back only files, so a walker who tried to name a
/// folder watched nothing at all happen and had no way to know why. The other two routes are
/// worse than silent — a drop on X11 and the path field *can* both produce a directory, and
/// taking one used to stage an `Add` whose source is not a file at all.
///
/// One split, at the one place every route converges, so the answer is the same wherever the
/// path came from. **This is not a recursive add**: that is a feature, and PXX ships none. The
/// round's job here is to end the silence, not to grow the program.
///
/// A path that does not exist is *not* a folder and is passed through deliberately — the
/// staged task then fails by name, which says more than being quietly dropped here would.
pub fn split_out_folders(paths: Vec<PathBuf>) -> (Vec<PathBuf>, usize) {
    let mut folders = 0;
    let files = paths
        .into_iter()
        .filter(|p| {
            let dir = p.is_dir();
            folders += usize::from(dir);
            !dir
        })
        .collect();
    (files, folders)
}

/// What [`split_out_folders`] left behind, as a clause to append — empty when it left none.
pub fn folders_note(n: usize) -> String {
    match n {
        0 => String::new(),
        1 => " 1 folder left out: INDIUM adds files, not folders.".to_string(),
        n => format!(" {n} folders left out: INDIUM adds files, not folders."),
    }
}

/// Why *Bring from archive* cannot run, or `None` if it can.
///
/// Free, and drawn beside the dead button rather than pushed to the status bar by a click,
/// which is the discipline Measure has followed since P21b and Create since P22: a disabled
/// button reports no click, so a sentence that waits for one is a sentence nobody reads.
///
/// The archive comes first because it is the larger absence — with nothing open there is
/// nothing to select from, so *"select some entries"* would be advice about a window that
/// is not there.
pub fn pull_refusal_for(no_archive: bool, nothing_selected: bool) -> Option<&'static str> {
    if no_archive {
        return Some("Open an archive first — this brings entries across from one.");
    }
    if nothing_selected {
        return Some("Select entries in the archive first, on the File view.");
    }
    None
}

/// How much closing an archive throws away — the whole of what leaving one costs.
///
/// **A staged creation is not a change against the archive being closed, and does not go
/// with it.** It names an archive that does not exist yet; photos.zip is not its subject
/// and never was. F6 ruled that the draft survives a Close untouched, and the queue's
/// creation lane is that draft projected — so discarding the projection while keeping the
/// draft would cost a person one `N` press to reach a state they were already in, for no
/// reason anyone could give them.
///
/// The two cases cannot overlap: `staging_refusal` refuses a rename or a remove while a
/// creation is staged, so the queue holds a creation *or* mutations and never both. That is
/// what lets this be a count rather than a filter, and it is what stops the sentence from
/// having to say *"4 changes"* about a number that included the `Task::Create` itself.
///
/// Free rather than a method, for [`estimate_refusal_for`]'s reason: `Indium` cannot be
/// built in a unit test, so a decision left on the type is held by eye.
pub fn discarded_by_closing(creation_staged: bool, staged: usize) -> usize {
    if creation_staged {
        0
    } else {
        staged
    }
}

pub fn discarded_line(headline: &str, discarded: usize, against: Option<&str>) -> String {
    if discarded == 0 {
        return format!("{headline}.");
    }
    let s = if discarded == 1 { "" } else { "s" };
    match against {
        Some(name) => format!("{headline} · {discarded} change{s} against {name} discarded."),
        None => format!("{headline} · {discarded} staged change{s} discarded."),
    }
}

/// Why Measure cannot run, or `None` if it can — as a function of the four facts it turns on.
///
/// **Free rather than a method, and that is the point.** `Indium::new` wants an
/// `eframe::CreationContext`, so the application cannot be built in a unit test; every gate
/// written as a method on it is held by construction and by eye, and this one was wrong for a
/// whole round without anything noticing. Four booleans and a sentence each is not something
/// that has to be unreachable.
///
/// The order is D5's. Files staged for adding come first, because packaging them is what the
/// popup is open for; the archive already open comes second, because re-compressing one is
/// the other reason to be here.
///
/// **`unread` is the case P21b found and P21 shipped without.** `stage_creation` used to adopt
/// the recipe's path before anything existed at it, so `archive_path` was `Some` for a file
/// libarchive cannot open. The button was therefore live over a freshly staged creation, the
/// worker failed in `Reader::open`, and the raw error went to the status bar while the Measure
/// popup sat there with eight blank rows and no reason on it. A refusal that says so is both
/// the honest answer and the one that reaches the person who needs it, because a disabled
/// button never reports a click.
///
/// **P22 removed the phantom that made that case, and this stays anyway.** Nothing adopts a
/// path before there is a file at it any more, so the creation half of `unread` can no longer
/// arise — but the other three it covers can: a listing still in flight, a listing that
/// failed, and `on_list_failure` leaving a partial `entries` behind. One fact, four causes,
/// and the round that removed one of them is not a reason to start asking two questions.
fn estimate_refusal_for(
    has_adds: bool,
    has_path: bool,
    unread: bool,
    locked: bool,
) -> Option<&'static str> {
    if has_adds {
        return None;
    }
    if !has_path {
        return Some("Add files, or open an archive: there is nothing to measure yet.");
    }
    if unread {
        return Some("This archive has not been read: there is nothing to measure yet.");
    }
    // The case easiest to forget: reading an encrypted member needs the password, and CORE §9
    // does not keep one lying about.
    if locked {
        return Some("These members are encrypted, and measuring them needs the password.");
    }
    None
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

    /// The one chance a deliberate exit gets to take back what it left lying about.
    ///
    /// PXX's finding 4, and the evidence for it is a journal record rather than a theory:
    /// unit `ad3e76c9…` went from `Started` to `Consumed` with no signal and no non-zero
    /// code — an ordinary `exit(0)` — one second after `.archiveadfadsf.7z.indium-new`
    /// last grew. 159 MB, under a name nobody chose, from a window that had simply been
    /// closed mid-write. From the user's chair a window that vanishes is a crash; from the
    /// process table it is this.
    ///
    /// **Unlink, do not wait.** Joining the worker would hold the window on screen after
    /// the user asked it to go, which is the one thing a close must never do. Unlinking a
    /// file another thread still holds open is exactly what POSIX makes safe: the name goes
    /// now, the worker writes on into an inode nobody can reach, and the blocks are freed
    /// when the process ends a moment later. The original archive is never touched — Apply
    /// builds beside it and renames at the end, so what is removed here is only ever the
    /// half-built temp.
    ///
    /// `is_our_temp_os` guards the removal for the same reason `tasks::apply` guards its own:
    /// nothing is deleted on a loose match, ever. It reads the name as bytes, because the
    /// `to_str()` this used to go through answers `None` for a perfectly ordinary Linux name
    /// and left our own litter behind for it. Litter is all that is at stake on this path — the
    /// Apply is already over — so a failed removal is not worth refusing an exit over.
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        // The worker may be between reads rather than mid-write; telling it to stop costs
        // nothing and occasionally saves it a few megabytes of pointless work.
        self.cancel
            .store(true, std::sync::atomic::Ordering::Relaxed);
        let Some(target) = self.apply_target.take() else {
            return;
        };
        let temp = tasks::temp_path_for(&target);
        if temp.file_name().is_some_and(tasks::is_our_temp_os) {
            let _ = std::fs::remove_file(&temp);
        }
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

        // And the same treatment for the second slot, for the same reason and one worse.
        //
        // `over` is only ever `Measure`, and Measure only ever stands over Create. Sixteen
        // sites assign `self.popup` by hand without going through `close_popup` — the sidebar
        // and the keyboard table among them, and both are reachable while Create is open,
        // because §4.1's popup is an `egui::Window` and not a modal. Left unswept, pressing `,`
        // over an open Measure would park the over-popup in the state, and pressing `N` a minute
        // later would raise it again over figures measured from an input that had since changed.
        //
        // Sweeping the figures with it is what makes E1's *"discarded when Create closes"*
        // true on **every** close path rather than on the four that call `close_popup`. That
        // half was already leaking before this round; it was invisible while the figures were
        // drawn only inside the popup that was going away.
        if self.popup != Some(Popup::Create) && (self.over.is_some() || self.holds_estimate()) {
            self.over = None;
            self.cancel_estimate();
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
        // Immediately after the popup it stands on, and before the rest — a `Modal` paints
        // above every `Window` whatever the call order, but reading the two together is what
        // says they are one stack.
        measure::show(self, &ctx);
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
        // Before P4 this took `.next()` and silently discarded every other path. CORE §4
        // says a drop stages an add — of all of them — and since P22 it lands "where `I`
        // does, in the section that is showing". With nowhere for files to land, the first
        // path is read as an archive to open instead.
        match self.section {
            Section::Draft => self.add_to_draft(dropped),
            Section::File if self.has_archive() => self.stage_adds(dropped),
            _ => {
                if let Some(path) = dropped.into_iter().next() {
                    self.open_archive(ctx, path, None);
                }
            }
        }
    }

    /// Is there somewhere in the section on screen for a file to land?
    ///
    /// CORE §4: `I`, `Ctrl+V` and a drop all put files in the section that is showing — the
    /// draft, or the directory the breadcrumb names. The two lists are not places files go,
    /// and the File view with nothing open has no directory to add into.
    fn can_receive_files(&self) -> bool {
        match self.section {
            Section::Draft => true,
            Section::File => self.has_archive(),
            Section::Recents | Section::Bookmarks => false,
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
            // P21b: the topmost popup, which for the first time in the program's life may be
            // one standing on another. It closes alone — the measurement underneath it is not
            // cancelled and its figures are not discarded, because E1 keeps them for as long
            // as Create lives and a run three seconds in still has somewhere to land.
            if self.over.is_some() {
                self.over = None;
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
        if ctrl_v && self.can_receive_files() && !typing {
            self.request_paste(ctx);
        }

        // CORE §4's bare letters are shortcuts, so they must not fire into a popup that
        // happens to hold no focused text field. About and Settings never focus anything,
        // and P4's two new popups are full of chips, rows and a slider that hold nothing
        // either — without this guard, pressing `E` inside Create would silently
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
                    // CORE §4's order, and it moved in P12: the file is `1` because it is
                    // what a person is looking at. **Which digit means which section is
                    // `sidebar::ROWS`' business and not this match's** — it used to be a
                    // second hand-kept list with a comment saying the two "have to be read
                    // together", which is the arrangement this project keeps finding
                    // drifted. P22 made this one ask.
                    egui::Key::Num1 | egui::Key::Num2 | egui::Key::Num3 | egui::Key::Num4 => {
                        let digit = match key {
                            egui::Key::Num1 => "1",
                            egui::Key::Num2 => "2",
                            egui::Key::Num3 => "3",
                            _ => "4",
                        };
                        if let Some(section) = sidebar::section_for_key(digit) {
                            self.section = section;
                        }
                    }
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
                                self.extract_destination().to_string_lossy().to_string();
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
            self.open_create();
        }
        if open_picker {
            self.request_picker(ctx, PickerFor::Open);
        }
        if add_picker {
            // CORE §4: "`I` adds to whichever section is showing: the draft, or the
            // directory the breadcrumb names." Decided rather than discovered — there are
            // two *Add files…* controls after P22 and one key, and the section on screen is
            // the one place the person is looking.
            match self.section {
                Section::Draft => self.request_picker(ctx, PickerFor::Draft),
                Section::File if self.has_archive() => self.request_picker(ctx, PickerFor::Add),
                _ => {}
            }
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
            if moved && self.section == Section::File {
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
            Section::File => {
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
            Section::Draft => {
                // `Del` only. Nothing in a draft is descendable and nothing in it is a
                // staged mutation, so there is no `Enter`, no `Backspace` and no rename —
                // a row here is a file that will go in, and the only thing to say about it
                // is that it will not.
                if del {
                    let note = restage_note(self.tasks.creation().is_some());
                    if let Some(gone) = self.draft.remove(self.draft_cursor) {
                        self.status = format!("Removed {} from the draft.{note}", gone.dest).into();
                    }
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
/// **A floor must sit below what the compositor actually hands out, or it becomes a trap.**
/// It read 840×600 and this machine is given a window **540 points tall**. KWin does not
/// apply the minimum when it restores a remembered geometry — but it applies it the instant
/// a person grabs an edge, so the first pixel of a drag snapped the frame up sixty points.
/// That is the "it jumped again" this cost an evening: not layout, not a fractional scale,
/// just a declared minimum the window was already under.
///
/// At a scale of 1.0 the same physical window is 675 points and clears 600 easily, which is
/// exactly why the jump looked like a fractional-scale problem and was not.
///
/// So these are deliberately far below anything in use. They are not a comfortable size —
/// comfort is not a floor's job — they are the size under which nothing can be done at all.
/// Every zone gives way on its own long before here: the sidebar's rows scroll under a bar
/// that floats over them instead of displacing them, the table clips its columns, the
/// Inspector has its own minimum — **and, since PXX, a ceiling that makes it reach that
/// minimum**. The last clause is not a flourish. A `Panel::right` keeps whatever width it
/// holds and the `CentralPanel` after it absorbs the whole difference, so a `min_size` alone
/// buys nothing: the Inspector sailed through 560 at its full 330 and left the table sixteen
/// points to draw four columns into, which is what step 6.3 of the walk denied. This sentence
/// was true of the other two zones and merely plausible of the third for six rounds.
/// `inspector::CENTRE_MIN` is what makes it true of all three.
///
/// (The bar floats rather than standing in a permanent track,
/// and `sidebar.rs` says why at the line that chooses it — this comment claimed the opposite
/// from P14 until P15, which is the fault `b0baa52` was written to stop making.)
pub const MIN_W: f32 = 560.0;
/// See [`MIN_W`].
pub const MIN_H: f32 = 400.0;

/// The whole panel, gutter included.
///
/// 3 × SB_ROW(24) + 2 × SB_GAP(4) = 80 of content, + 10 + 10 of inner margin, + the 2 + 2
/// the edge costs, + the 4 + 4 of `zone()`'s outer gutter = 112. `exact_size` counts every
/// one of those, so this number is the panel and its half-gutter rim together. It was 100
/// until P13 raised `SB_ROW` to carry an icon at `ICON_SCALE`.
const SB_HEIGHT: f32 = 112.0;

/// How thick the proportion-done bar is, and the reason [`SB_HEIGHT`] is not affected by it.
///
/// It was 2 — the edge weight §6 already owned — until the certification walk asked for it
/// three times thicker. **It grows upward**, from the underside the 2px line already had, so
/// all four of the new pixels are spent on the panel's own 2px stroke and on the half-gutter
/// above it: space this panel is already allocated and nothing else paints into. Which is
/// what keeps the change free. Nothing in the three rows below moves, the arithmetic above
/// still adds to 112, and `the_status_bar_is_as_tall_as_it_says` still holds. Growing it
/// *downward* would have cost 4px of a lane that has none to give.
const PROGRESS_H: f32 = 6.0;

/// The status bar's frame, named so the height above can be checked against it.
fn sb_frame() -> egui::Frame {
    theme::zone(theme::STATUS_BAR).inner_margin(egui::Margin::symmetric(12, 10))
}

/// Where the proportion bar is painted, given the status bar's content lane: its run, and the
/// clip it is drawn under.
///
/// Lifted out of [`status_bar`] by P23 §2a for the reason the round keeps finding: the
/// arithmetic sat inside a closure that needs a window, so when §2a gave this bar an arc to
/// clear, nothing in the suite could ask whether it did.
/// [`tests::the_proportion_bar_clears_the_corner_it_runs_into`] is what can ask now.
///
/// **Out to the frame's own edge rather than the content lane's**: the inner margin is where
/// the padding starts, so backing out by it lands on the line the 2px stroke draws.
///
/// **Then both ends pulled in by [`theme::R_ZONE`]**, which is the price of that line. This
/// bar is the only thing in the window that paints against a zone's own rect instead of its
/// content lane, so the inner margin that clears the arc for every other zone does not clear
/// it here — by construction, since backing out of that margin is the whole point. At
/// `R_ZONE = 6` the top edge is only straight between `left + 6` and `right - 6`; a run
/// starting at the frame's own left would begin *inside* the top-left arc, where the fill has
/// been cut away and there is no edge to lie along, so every small fraction would draw a
/// six-pixel orange stub floating in the notch against `VOID`. Both ends and not just the
/// left: the bar is a proportion, and a proportion whose ends are measured from different
/// insets has stopped being one.
///
/// **Upward**, and the anchor is the underside rather than the top: the bar still ends exactly
/// where the 2px line ended, and every one of the extra pixels is spent above it. The clip has
/// to be let up by the same amount or it would crop back the growth it was widened for.
fn sb_progress_geometry(lane: egui::Rect, frac: f32) -> (egui::Rect, egui::Rect) {
    let pad = sb_frame().inner_margin;
    let edge = lane
        .expand2(egui::vec2(pad.left as f32, pad.top as f32))
        .shrink2(egui::vec2(f32::from(theme::R_ZONE), 0.0));
    let up = PROGRESS_H - 2.0;
    let run = egui::Rect::from_min_size(
        edge.left_top() - egui::vec2(0.0, up),
        egui::vec2(edge.width() * frac, PROGRESS_H),
    );
    let mut clip = edge;
    clip.min.y -= up;
    (run, clip)
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
            // The 4 in the arithmetic above is this line. The theme's own `item_spacing.y`
            // is 5.0, which would make the three rows 106 tall in a lane that is 104 — and
            // the third row is the one that gets clipped.
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
                // **To the pixel, not to the point.** `.round()` snapped this to a whole
                // point, and a point is only a whole pixel at a scale of 1. At 1.25 it is
                // 1.25 pixels, so a 1px rule straddled two rows of them and its share
                // shifted as the window moved — a shimmer that only exists on a fractional
                // display. `round_to_pixels` is `emath`'s answer to exactly this and is the
                // identity at 1.0.
                use egui::emath::GuiRounding as _;
                ui.painter().hline(
                    lane.x_range(),
                    (y - theme::SB_GAP / 2.0).round_to_pixels(ui.pixels_per_point()),
                    theme::hairline(),
                );
            }

            // CORE §4: "The proportion done is drawn as a [`PROGRESS_H`]px bar along the
            // bar's own top edge, growing upward out of it." Painted for the same reason the
            // two hairlines above are painted and never allocated — `exact_size` means this
            // panel cannot grow to absorb anything new, and a widget here would push row 3
            // out of the window.
            if let Some(p) = &app.progress {
                // `done + within`, not `done`: the count beside it is members and stays
                // members, but the bar is a proportion and can say what the count cannot.
                // Before PXX an archive of one large member drew a bar that sat at zero
                // until it jumped to full, which is a bar that has told you nothing at the
                // only moment you wanted it to.
                let frac = if p.total == 0 {
                    0.0
                } else {
                    ((p.done as f32 + p.within) / p.total as f32).clamp(0.0, 1.0)
                };
                let (run, clip) = sb_progress_geometry(lane, frac);
                ui.painter()
                    .with_clip_rect(clip)
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
                        .size(theme::BODY * theme::ICON_SCALE)
                        .color(theme::TEXT),
                );
                ui.label(
                    egui::RichText::new(name.to_string_lossy())
                        .family(theme::bold())
                        .size(theme::BODY)
                        .color(theme::TEXT),
                )
            }
            None => ui.label(
                egui::RichText::new("No archive open.")
                    .family(theme::MONO)
                    .size(theme::BODY)
                    .color(theme::TEXT_MUTED),
            ),
        };

        if let Some(info) = &app.archive_info {
            ui.label(egui::RichText::new("·").color(theme::TEXT_MUTED));
            ui.label(
                egui::RichText::new(&info.format)
                    .family(theme::MONO)
                    .size(theme::BODY)
                    .color(theme::TEXT_SECONDARY),
            );
            if !info.filter.is_empty() && info.filter != "none" {
                ui.label(
                    egui::RichText::new(&info.filter)
                        .family(theme::MONO)
                        .size(theme::BODY)
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
                let font = egui::FontId::new(theme::SMALL, theme::MONO);
                // `fonts_mut`, not `fonts`: measuring a glyph can populate the atlas, so
                // the accessor that admits it is the correct one.
                let cell = ui.ctx().fonts_mut(|f| f.glyph_width(&font, '0')).max(1.0);
                let gap = ui.spacing().item_spacing.x;
                // Less the folder glyph and the space before it, which are added after
                // this label because the lane runs right to left. The glyph is drawn at
                // `ICON_SCALE`, so it costs that many cells and not one.
                let glyph = cell * theme::ICON_SCALE + gap;
                let want = ((ui.available_width() - glyph) / cell).floor().max(0.0) as usize;

                // **Hysteresis, so a drag does not re-cut the path on every pixel.** The
                // budget steps a whole column every ~9 physical pixels of resize, and a
                // path that re-elides that often is a line of text visibly chewing itself
                // while the window moves. Holding the previous width until it is wrong by
                // two columns halves the number of changes and costs a `usize`; the label
                // is `.truncate()`d anyway, so a budget one column stale can never overrun
                // the lane.
                if want.abs_diff(app.path_cells) >= 2 || want == 0 {
                    app.path_cells = want;
                }
                let shown = crate::util::elide_middle(&dir, app.path_cells.min(want.max(1)));

                let hit = ui
                    .add(
                        egui::Label::new(
                            egui::RichText::new(shown)
                                .family(theme::MONO)
                                .size(theme::SMALL)
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
                        .size(theme::SMALL * theme::ICON_SCALE)
                        .color(theme::TEXT_MUTED),
                );
            });
        }
    });
}

// The widest each of row 2's fields can be, so a lane never has to grow. `format_bytes` is
// widest at `1023.9 KiB` — ten — and not at the top of its ladder as one would guess: a
// value only ever reaches four digits before the unit changes beneath it, so the rungs above
// KiB are no wider, and `u64::MAX` itself is merely `16.0 EiB`. A ratio reads `100.0%` at six
// and is given seven so a format that grew what it packed still lands inside its lane. The
// two counts are given six, a million entries, ten times the hundred thousand CORE §1 claims.
const LANE_COUNT: usize = 6;
const LANE_SIZE: usize = 10;
const LANE_RATIO: usize = 7;

/// Row 2's fields, padded into the lanes CORE §4 asks for.
///
/// Pure and separate from the drawing so the rule can be tested without a window: what the
/// rule actually says is that these strings are the same length whatever the numbers are.
fn sb_lane_entries(count: usize) -> String {
    format!("{count:>LANE_COUNT$} entries")
}

fn sb_lane_sizes(real: u64, packed: u64) -> String {
    format!(
        "{:>LANE_SIZE$} -> {:>LANE_SIZE$} ({:>LANE_RATIO$})",
        crate::util::format_bytes(real),
        crate::util::format_bytes(packed),
        crate::util::format_ratio(real, packed),
    )
}

fn sb_lane_selected(n: usize) -> String {
    format!("{n:>LANE_COUNT$} selected")
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
///
/// **The numbers hold their columns, and from P12 to P15 they did not.** CORE §4 asks that
/// sizes, counts and ratios be "right-aligned to fixed positions and do not move as their
/// digits change", and this row laid every field out with a plain left-to-right `label`, so
/// a listing dragged the whole row sideways: `aggregate` is recomputed each frame from the
/// entries that have arrived, so the count and the real size both grow as they stream, and
/// the ratio slid a cell to the right on every tenth entry. P12 recorded it as a deviation,
/// P13 claimed to have written it down here and did not, and P15 both wrote it and fixed it.
/// The lanes below are the fix; the fields are padded rather than measured because §6's face
/// is the `Mono` cut, where a character is a column.
fn sb_the_numbers(app: &Indium, ui: &mut egui::Ui) {
    sb_row(ui, egui::Layout::left_to_right(egui::Align::Center), |ui| {
        if app.has_archive() {
            let agg = model::aggregate(app.entries.iter());
            // CORE §4: "One thing per row is the subject, and it is bold." On this row
            // that is the count — everything else here qualifies it.
            ui.label(
                egui::RichText::new(sb_lane_entries(agg.count))
                    .family(theme::bold())
                    .size(theme::BODY)
                    .color(theme::TEXT),
            );
            ui.label(egui::RichText::new("·").color(theme::TEXT_MUTED));

            // Archive-level real -> packed, which is honest: the packed side
            // is the archive's own size on disk.
            ui.label(
                egui::RichText::new(sb_lane_sizes(agg.total_real, app.archive_bytes))
                    .family(theme::MONO)
                    .size(theme::BODY)
                    .color(theme::TEXT_SECONDARY),
            );

            if !app.selection.is_empty() {
                ui.label(egui::RichText::new("·").color(theme::TEXT_MUTED));
                ui.label(
                    egui::RichText::new(sb_lane_selected(app.selection.len()))
                        .family(theme::MONO)
                        .size(theme::BODY)
                        .color(theme::TEXT_SECONDARY),
                );
            }
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add(
                egui::Label::new(
                    egui::RichText::new(&app.status.text)
                        .family(theme::MONO)
                        .size(theme::BODY)
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
                        .size(theme::BODY * theme::ICON_SCALE)
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
/// **There is no track in this row any more.** P13 moved the proportion to a line along the
/// panel's top edge, and PXX made that line a [`PROGRESS_H`]px bar growing upward out of it
/// (CORE §4). Either way it is drawn by `status_bar`, because only `status_bar` knows where
/// that edge is. What stays here is what a track could never carry: the phase,
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
        if let Some((done, total, label)) = running {
            // Orange with a Cancel beside it is CORE §6's third permitted meaning —
            // Apply/progress — and this is the only place in the window that draws it.
            if theme::button(ui, "Cancel".into(), true).clicked() {
                app.cancel.store(true, Ordering::Relaxed);
            }
            ui.label(
                egui::RichText::new(format!("{done}/{total}"))
                    .family(theme::MONO)
                    .size(theme::BODY)
                    .color(theme::TEXT),
            );
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                // Row 3's subject: what is happening. Bold, per CORE §4.
                ui.label(
                    egui::RichText::new(label)
                        .family(theme::bold())
                        .size(theme::BODY)
                        .color(theme::TEXT),
                );
                // **The track is not here any more.** CORE §4: "The proportion done is
                // drawn as a 6px bar along the bar's own top edge, growing upward out of
                // it, not as a track inside the row: it is the one measurement in the
                // window that wants the whole width, and the edge is already there."
                // `status_bar` paints it, because only `status_bar` knows where the panel's
                // edge is.
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
                // Sized explicitly, and from a named length: `SPINNER_D` carries the reason.
                // `Spinner::size` is a diameter that happens to share a method name with the
                // type scale, so the number does not get to live inline here.
                ui.add(
                    egui::Spinner::new()
                        .color(theme::ORANGE)
                        .size(theme::SPINNER_D),
                );
                let n = app.entries.len();
                ui.label(
                    egui::RichText::new(format!(
                        "Reading… {n} {}",
                        if n == 1 { "entry" } else { "entries" }
                    ))
                    .family(theme::MONO)
                    .size(theme::BODY)
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
    theme::floating(ctx, "Open archive")
        .collapsible(false)
        .resizable(false)
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

    /// PXX 6.2: a folder handed to *Add files…* used to vanish without a word. It is still
    /// not added — that would be a feature — but it is now counted and said out loud.
    #[test]
    fn a_folder_is_left_out_and_said_out_loud() {
        let dir = std::env::temp_dir();
        let file = dir.join("indium-6-2-not-a-folder.txt");
        std::fs::write(&file, b"x").expect("temp file");

        let (files, folders) = split_out_folders(vec![dir.clone(), file.clone(), dir.clone()]);
        assert_eq!(
            files,
            vec![file.clone()],
            "only the file survives the split"
        );
        assert_eq!(folders, 2, "both folders are counted, not silently dropped");

        // A path that does not exist is not a folder: it goes through, so the task that
        // fails on it can name it. Being quietly swallowed here would say less.
        let ghost = dir.join("indium-6-2-no-such-thing");
        let (files, folders) = split_out_folders(vec![ghost.clone()]);
        assert_eq!((files, folders), (vec![ghost], 0));

        assert_eq!(folders_note(0), "", "the ordinary case costs nothing");
        for (n, want) in [(1, "1 folder left out"), (3, "3 folders left out")] {
            let note = folders_note(n);
            assert!(note.contains(want), "{n} folders reads as {note:?}");
            assert!(
                note.contains("INDIUM adds files, not folders"),
                "the note has to say why, not just that: {note:?}"
            );
            assert!(
                note.starts_with(' '),
                "it is appended to a sentence: {note:?}"
            );
        }

        std::fs::remove_file(&file).ok();
    }

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

    /// The proportion bar begins and ends on the *straight* part of the status bar's top edge.
    ///
    /// P23 §2a moved [`theme::R_ZONE`] off zero, and this bar is the one thing in the window
    /// that paints against a zone's own rect rather than its content lane: every other zone
    /// clears its arc through an inner margin, and this one deliberately backs out of that
    /// margin to sit on the line the stroke draws. So the arc is a hazard here and nowhere
    /// else, and the failure is not a crash — at a small fraction the run would be a six-pixel
    /// orange stub standing in the notch the arc cuts, outside the fill, against `VOID`.
    ///
    /// Measured against the frame the running window actually paints: at scale 1 the status
    /// bar's rect is x 69..1240, its arc occupies x 69..75, and the unpulled run started at
    /// **71** — four pixels inside the curve. Those are read pixels, not modelled ones.
    ///
    /// The lane is derived from `sb_frame`'s own margins rather than restated, so the two
    /// cannot drift; and the last assertion is the leg, because a test whose premise has
    /// quietly become true measures nothing.
    #[test]
    fn the_proportion_bar_clears_the_corner_it_runs_into() {
        let r = f32::from(theme::R_ZONE);
        let pad = sb_frame().inner_margin;
        // The frame as the window paints it, and the content lane inside its stroke and
        // padding — which is the rect `status_bar` hands the bar.
        let frame = egui::Rect::from_min_max(egui::pos2(69.0, 693.0), egui::pos2(1240.0, 796.0));
        let lane = egui::Rect::from_min_max(
            frame.min + egui::vec2(2.0 + f32::from(pad.left), 2.0 + f32::from(pad.top)),
            frame.max - egui::vec2(2.0 + f32::from(pad.right), 2.0 + f32::from(pad.bottom)),
        );

        for frac in [0.0, 0.001, 0.01, 0.5, 0.999, 1.0] {
            let (run, clip) = sb_progress_geometry(lane, frac);
            assert!(
                run.left() >= frame.left() + r,
                "at {frac} the run starts at {}, and the fill is cut away left of {} — the bar \
                 would stand in the top-left notch",
                run.left(),
                frame.left() + r
            );
            assert!(
                run.right() <= frame.right() - r,
                "at {frac} the run reaches {}, past where the top edge stops being straight \
                 at {}",
                run.right(),
                frame.right() - r
            );
            assert!(
                clip.left() >= frame.left() + r && clip.right() <= frame.right() - r,
                "the clip readmits what the run was pulled in from, so the pull-in is decorative"
            );
        }

        let unpulled = lane.expand2(egui::vec2(f32::from(pad.left), f32::from(pad.top)));
        assert!(
            unpulled.left() < frame.left() + r,
            "the frame's own edge at {} already clears the arc at {}, so the pull-in this test \
             exists to check is doing nothing and the test has stopped measuring it",
            unpulled.left(),
            frame.left() + r
        );
    }

    /// CORE §4's numbered list of popups, and the enum that holds them, are the same length.
    ///
    /// **Nothing enforced this until P21b, and the count had already drifted twice.** §4 said
    /// seven while the code had eight; a doc comment in this file said seven and counted eight
    /// in the same breath; P12 straightened both by hand. The number is written in three
    /// places — §4's opening sentence in words, §4's numbered list, and [`Popup`] itself — and
    /// two of the three were prose that no test read. This reads all three.
    ///
    /// It is a count and not a comparison of names because the two lists are not written in the
    /// same idiom: §4 numbers *Open* where the enum says `OpenPath`, and *Pending tasks* where
    /// the enum says `PendingTasks`. A count is what the drift actually looked like both times.
    #[test]
    fn the_popup_list_and_core_agree_about_how_many_there_are() {
        let core = include_str!("../../CORE.md");
        let after = core
            .split_once("### The popups")
            .expect("CORE §4 has a 'The popups' heading")
            .1;

        // Every item opens at column zero with its own number; an item's continuation lines are
        // indented, and the list ends at the next heading.
        let numbered: Vec<u32> = after
            .lines()
            .take_while(|l| !l.starts_with('#'))
            .filter_map(|l| l.split_once(". ").and_then(|(n, _)| n.parse().ok()))
            .collect();

        assert_eq!(
            numbered.len(),
            Popup::ALL.len(),
            "CORE §4 numbers {} popups and the program has {}",
            numbered.len(),
            Popup::ALL.len()
        );
        // And they are numbered 1..=n, in order: a list that goes 8, 9, 9 is the same defect
        // wearing the right length.
        for (i, n) in numbered.iter().enumerate() {
            assert_eq!(
                *n as usize,
                i + 1,
                "CORE §4's list is numbered {numbered:?}"
            );
        }
        // A variant added to the enum but pasted twice into `ALL` would keep the length and
        // lose the popup, which is the one way this gate could be satisfied while wrong.
        for (i, p) in Popup::ALL.iter().enumerate() {
            assert!(!Popup::ALL[..i].contains(p), "{p:?} is listed twice in ALL");
        }

        // The third place the count is written: §4's opening sentence, in words.
        const WORDS: &[&str] = &[
            "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
            "eleven", "twelve",
        ];
        let sentence = format!(
            "Five fixed zones and {} popups. Nothing else appears, ever.",
            WORDS[Popup::ALL.len()]
        );
        assert!(
            core.contains(&sentence),
            "CORE §4 does not open with {sentence:?}"
        );
    }

    /// Measure is dead in every state where it has nothing honest to measure, and says which.
    ///
    /// The `unread` row is the one this test exists for. P21 shipped a button that was live
    /// over a creation staged but never written — `archive_path` is `Some` the moment Create
    /// is pressed — so the worker opened a file that was not there and the failure landed in
    /// the status bar, one zone away from the popup that had asked for it. Nothing caught it
    /// because the decision lived on `Indium`, which no test can build.
    ///
    /// **What this test still cannot reach**, and the reason the first fix was wrong: the
    /// mapping from `Indium`'s fields onto these four booleans is itself a decision, it lives
    /// on `Indium`, and moving the branch off the type did not move the mapping with it. The
    /// first version passed `creation().is_some() && archive_info.is_none()` here and this
    /// test passed, because the row below was already `true` — while `Discard` cleared the
    /// queue, left `archive_path` standing, and put the live button straight back over the
    /// file that had never been written. Every row here can be green with the call site
    /// wrong. Only the window shows that.
    #[test]
    fn measure_refuses_wherever_it_has_nothing_to_weigh_and_says_which() {
        // has_adds, has_path, unread, locked
        //
        // **This first row is the whole of P22 in one assertion.** `has_adds` carries the
        // draft as well as the queue's adds since this round, so "a full draft with nothing
        // open" is live — which is what makes Measure available on the Create popup's first
        // frame, and what moving Create last was for. What the row cannot reach is *which*
        // of the two filled the boolean: the mapping folds them, deliberately, because after
        // a Create the queue's adds are the draft's projection and the two say the same
        // thing. Only the window shows that the draft alone reaches it.
        assert_eq!(estimate_refusal_for(true, false, false, false), None);
        // Staged adds outrank everything: they are bytes on disk, whatever the window holds.
        assert_eq!(estimate_refusal_for(true, true, true, true), None);
        // A real archive, listed and unencrypted.
        assert_eq!(estimate_refusal_for(false, true, false, false), None);

        for (adds, path, unread, locked, want) in [
            (false, false, false, false, "Add files, or open an archive"),
            (false, true, true, false, "has not been read"),
            // An unread archive is unread before it is locked: nothing has told us there are
            // encrypted members, so the password sentence would be a guess at the reason.
            (false, true, true, true, "has not been read"),
            (false, true, false, true, "encrypted"),
        ] {
            let got = estimate_refusal_for(adds, path, unread, locked)
                .unwrap_or_else(|| panic!("{adds}/{path}/{unread}/{locked} should refuse"));
            assert!(
                got.contains(want),
                "{adds}/{path}/{unread}/{locked}: {got:?} does not mention {want:?}"
            );
        }
    }

    /// F7 at the Create door: it displaces the queue and says what went.
    ///
    /// The sentence rather than the plumbing. [`discarded_line`] is where the maker's ruling
    /// actually lives, and it is a free function so that the ruling can be *read* by a test
    /// rather than looked at in a window — which is the only way the other door's sentence
    /// can be held to the same words.
    #[test]
    fn create_says_what_it_displaced() {
        assert_eq!(
            discarded_line("Staged: create backup.7z", 4, Some("photos.zip")),
            "Staged: create backup.7z · 4 changes against photos.zip discarded."
        );
        assert_eq!(
            discarded_line("Staged: create backup.7z", 1, Some("photos.zip")),
            "Staged: create backup.7z · 1 change against photos.zip discarded.",
            "one change is not one changes"
        );
        assert_eq!(
            discarded_line("Staged: create backup.7z", 0, Some("photos.zip")),
            "Staged: create backup.7z.",
            "nothing went, so nothing is said about it"
        );
    }

    /// F7 at the other door, and in the other form: closing names no second archive, so the
    /// clause is *"4 staged changes discarded"* and not *"4 changes against photos.zip"* —
    /// which would have said photos.zip twice in one line to no purpose.
    #[test]
    fn closing_says_what_it_discarded() {
        assert_eq!(
            discarded_line("Closed photos.zip", 4, None),
            "Closed photos.zip · 4 staged changes discarded."
        );
        assert_eq!(
            discarded_line("Closed photos.zip", 1, None),
            "Closed photos.zip · 1 staged change discarded.",
            "one change is not one changes"
        );
        assert_eq!(
            discarded_line("Closed photos.zip", 0, None),
            "Closed photos.zip.",
            "nothing went, so nothing is said about it"
        );
    }

    /// The other half of F7's Close: **what** goes, before there is a sentence about it.
    ///
    /// A staged creation stands. It names an archive that does not exist yet, photos.zip was
    /// never its subject, and F6 ruled the draft it projects survives untouched — so
    /// discarding the projection while keeping the draft would cost a person one `N` press
    /// to get back somewhere they already were. The queue's `len()` here is 5 (a `Create` and
    /// four `Add`s) and none of it goes, which is also why the count could never have been
    /// reported as *"5 changes"*: one of the five was the creation itself.
    #[test]
    fn closing_leaves_the_draft_alone() {
        assert_eq!(
            discarded_by_closing(true, 5),
            0,
            "a creation is not left behind"
        );
        assert_eq!(
            discarded_line("Closed photos.zip", discarded_by_closing(true, 5), None),
            "Closed photos.zip.",
            "nothing was discarded, so the line must not claim anything was"
        );

        // Mutations are against the archive being closed, and go with it.
        assert_eq!(discarded_by_closing(false, 4), 4);
        assert_eq!(discarded_by_closing(false, 0), 0);
    }

    /// The carried sentence, composed where it is finally read.
    ///
    /// `Ctrl+O` over an open archive is a close and an open, so F7 owes the same sentence —
    /// but the status line at the moment of the close lives for as long as the listing takes,
    /// which is milliseconds. `ListMsg::Done` composes it onto the line that stays, and this
    /// is that composition: the *new* archive is the headline, so here the against-form is
    /// the right one and the closed archive is what it names.
    #[test]
    fn opening_over_an_archive_says_what_that_cost() {
        assert_eq!(
            discarded_line("other.zip", 4, Some("photos.zip")),
            "other.zip · 4 changes against photos.zip discarded."
        );
        // And with nothing staged the carry is never set, so `Done` writes the bare name it
        // has written since P5 — no full stop, because a file name is not a sentence.
        assert_eq!(
            archive_name(std::path::Path::new("/x/other.zip")),
            "other.zip"
        );
    }

    /// *Bring from archive* is dead in two states, and says which — the archive before the
    /// selection, because with nothing open *"select some entries"* is advice about a window
    /// that is not there.
    #[test]
    fn the_pull_says_which_of_the_two_things_it_is_missing() {
        assert_eq!(pull_refusal_for(false, false), None, "it must be live here");

        let no_archive = pull_refusal_for(true, true).expect("nothing open must refuse");
        assert!(
            no_archive.contains("Open an archive"),
            "{no_archive:?} does not name the larger absence"
        );
        assert_eq!(
            pull_refusal_for(true, false),
            Some(no_archive),
            "a selection cannot outrank the archive it would be a selection in"
        );

        let nothing_picked = pull_refusal_for(false, true).expect("no selection must refuse");
        assert!(
            nothing_picked.contains("Select entries"),
            "{nothing_picked:?} does not say what to do"
        );
    }

    /// A file added to the draft after *Create* is in the draft and not in the creation the
    /// tray is offering to build, because the projection is recomputed on a press and at no
    /// other time. Apply would succeed and the archive would simply not hold it — the one
    /// loss in this round that no sentence would otherwise report.
    #[test]
    fn changing_the_draft_under_a_staged_creation_says_so() {
        assert_eq!(restage_note(false), "", "the ordinary case pays nothing");

        let staged = restage_note(true);
        assert!(
            staged.starts_with(' '),
            "the note is appended to a finished sentence and must carry its own space"
        );
        assert!(
            staged.contains('N'),
            "{staged:?} does not say which key restages it"
        );
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

    /// CORE §4: "Numbers hold their columns. Sizes, counts and ratios are right-aligned to
    /// fixed positions and do not move as their digits change." In the `Mono` cut a column
    /// is a character, so the rule is exactly this: one width, whatever the number.
    #[test]
    fn row_twos_numbers_hold_their_columns() {
        let counts: Vec<usize> = vec![0, 7, 42, 999, 100_000, 999_999];
        for window in counts.windows(2) {
            assert_eq!(
                sb_lane_entries(window[0]).chars().count(),
                sb_lane_entries(window[1]).chars().count(),
                "{} and {} entries are not the same width",
                window[0],
                window[1],
            );
            assert_eq!(
                sb_lane_selected(window[0]).chars().count(),
                sb_lane_selected(window[1]).chars().count(),
            );
        }

        // Every rung of `format_bytes`, plus the zero-real case whose ratio is an em dash
        // rather than a percentage — the one arm that is not a number at all.
        let sizes: Vec<(u64, u64)> = vec![
            (0, 0),
            (1, 1),
            (1023, 500),
            (1024, 1024),
            (1_048_576, 700_000),
            (1_099_511_627_776, 900_000_000_000),
            (u64::MAX, u64::MAX / 3),
        ];
        let widths: Vec<usize> = sizes
            .iter()
            .map(|(r, p)| sb_lane_sizes(*r, *p).chars().count())
            .collect();
        assert!(
            widths.windows(2).all(|w| w[0] == w[1]),
            "the size lanes moved: {widths:?} for {sizes:?}",
        );
    }

    /// A lane that is too narrow silently stops being a lane, so the widths are asserted
    /// against the widest thing each field can actually hold rather than trusted.
    #[test]
    fn no_number_overflows_the_lane_reserved_for_it() {
        // `1023.9 KiB` is the widest `format_bytes` ever gets, and it is worth pinning
        // because it is not where one would look for it — the ladder's top rung is
        // narrower, since `u64::MAX` is only `16.0 EiB`.
        assert_eq!(
            crate::util::format_bytes(1_048_474).chars().count(),
            LANE_SIZE
        );
        assert!(crate::util::format_bytes(u64::MAX).chars().count() <= LANE_SIZE);

        // Every rung, and both sides of each boundary, rather than trusting the argument.
        let mut n: u64 = 1;
        loop {
            for probe in [n.saturating_sub(1), n, n.saturating_add(1)] {
                let w = crate::util::format_bytes(probe).chars().count();
                assert!(w <= LANE_SIZE, "format_bytes({probe}) is {w} wide");
            }
            match n.checked_mul(2) {
                Some(next) => n = next,
                None => break,
            }
        }

        assert!(crate::util::format_ratio(1000, 1000).chars().count() <= LANE_RATIO);
        assert!(crate::util::format_ratio(0, 0).chars().count() <= LANE_RATIO);
        assert!(999_999usize.to_string().chars().count() <= LANE_COUNT);
    }

    /// PXX 8.11: the whole of what *Preselect* means — one directory, for every archive,
    /// ahead of the one the archive's own parent would give.
    #[test]
    fn preselect_answers_before_the_archives_own_parent() {
        let derived = || PathBuf::from("/beside/the/archive");
        assert_eq!(
            resolve_extract_destination(ExtractDefault::Preselect, "/named/once", derived),
            PathBuf::from("/named/once")
        );
    }

    /// `settings.toml` is a file a person can open, and `default = "preselect"` can be typed
    /// into it without a path ever being chosen. Nowhere is not a destination: the window is
    /// handed the derived directory rather than an empty string, which `Extract` would show
    /// as a blank field and write to the process's cwd.
    #[test]
    fn a_preselect_naming_nowhere_falls_back_instead_of_naming_nowhere() {
        let derived = || PathBuf::from("/beside/the/archive");
        for nothing in ["", "   ", "\t"] {
            assert_eq!(
                resolve_extract_destination(ExtractDefault::Preselect, nothing, derived),
                PathBuf::from("/beside/the/archive"),
                "{nothing:?} is not a directory"
            );
        }
    }

    /// The path outlives the mode on purpose — picking *here* keeps it, so that pressing
    /// *Preselect* again does not mean naming the directory a second time. This is the test
    /// that says the stored path stays *inert* while another mode is lit, which is the other
    /// half of that decision and the half that would be a defect if it were wrong.
    #[test]
    fn a_stored_preselect_is_inert_while_another_mode_is_lit() {
        let derived = || PathBuf::from("/beside/the/archive");
        for mode in [ExtractDefault::Here, ExtractDefault::Subdir] {
            assert_eq!(
                resolve_extract_destination(mode, "/named/once", derived),
                PathBuf::from("/beside/the/archive"),
                "{mode:?} read the preselect path"
            );
        }
    }
}
