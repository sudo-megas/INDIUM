//! TOML persistence: settings, bookmarks, recent files.
//!
//! P2 §1 fixes the rules, and they matter more than the fields:
//! atomic writes only; malformed files are tolerated, never fatal; writes happen on
//! change, not on a timer; and passwords never appear in either file, under any name
//! (CORE §9).

use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// P2 §2: "the list caps at 15".
pub const RECENT_CAP: usize = 15;

// ---------------------------------------------------------------------------
// Shapes
// ---------------------------------------------------------------------------

/// Which action the Extract popover preselects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExtractDefault {
    /// Beside the archive.
    Here,
    /// Into a directory named after the archive.
    #[default]
    Subdir,
    /// Into one directory the user named once — the same one for every archive, until
    /// they pick another mode.
    ///
    /// PXX 8.11: the row's label read as a button, and the maker clicked it expecting
    /// exactly this. The word it was misread as is the word it keeps, because the
    /// defect was never the word: it was that the word promised something to press.
    ///
    /// The path lives beside this variant rather than inside it. A payload would cost
    /// `ExtractDefault` its `Copy`, which four call sites read by value, and the path is
    /// worth keeping while another mode is active so that coming back does not mean
    /// naming the directory twice.
    Preselect,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ExtractSettings {
    #[serde(default)]
    pub default: ExtractDefault,
    /// Where `Preselect` points, empty until one is chosen.
    ///
    /// Empty is load-bearing: a `settings.toml` hand-edited to `default = "preselect"`
    /// with no path names nowhere, and `extract_destination` falls back rather than
    /// offering the window an empty string as a directory.
    #[serde(default)]
    pub preselect: String,
}

/// A named directory. P2 §2: "in an archiver a bookmark is an *extract destination*."
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bookmark {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub extract: ExtractSettings,
    /// Serialised as `[[bookmark]]` tables, which is what a hand-editing user expects.
    #[serde(default, rename = "bookmark")]
    pub bookmarks: Vec<Bookmark>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Recent {
    pub path: String,
    /// Unix seconds. The list is ordered by this.
    pub opened: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Recents {
    #[serde(default, rename = "recent")]
    pub items: Vec<Recent>,
}

impl Recents {
    /// Move `path` to the top, stamping it with `now`, and enforce the cap.
    ///
    /// P2 §2: "Opening an archive bumps it to the top; the list caps at 15." There is
    /// deliberately no automatic pruning of missing files — "the list only loses
    /// entries by the user's hand or the cap."
    pub fn bump(&mut self, path: &str, now: i64) {
        self.items.retain(|r| r.path != path);
        self.items.insert(
            0,
            Recent {
                path: path.to_string(),
                opened: now,
            },
        );
        self.items.truncate(RECENT_CAP);
    }

    pub fn remove(&mut self, path: &str) {
        self.items.retain(|r| r.path != path);
    }

    /// Newest first, whatever order the file happened to be in.
    pub fn sorted(&self) -> Vec<&Recent> {
        let mut v: Vec<&Recent> = self.items.iter().collect();
        v.sort_by_key(|r| std::cmp::Reverse(r.opened));
        v
    }
}

// ---------------------------------------------------------------------------
// Loading
// ---------------------------------------------------------------------------

/// A loaded file, plus anything the user needs told about it.
#[derive(Debug, Clone)]
pub struct Loaded<T> {
    pub value: T,
    /// A sentence for the status bar, when the file could not be parsed.
    pub notice: Option<String>,
    /// True when the on-disk file failed to parse. While this is set, INDIUM must not
    /// overwrite the file — P2 §1: "Never silently overwrite a file that failed to
    /// parse until the user changes something."
    pub was_broken: bool,
}

impl<T> Loaded<T> {
    fn clean(value: T) -> Self {
        Loaded {
            value,
            notice: None,
            was_broken: false,
        }
    }
}

// ---------------------------------------------------------------------------
// The store
// ---------------------------------------------------------------------------

/// Where the two files live. Constructed from XDG in the app, and from explicit
/// directories in tests, so a test never touches the real `~/.config`.
#[derive(Debug, Clone)]
pub struct Store {
    pub config_dir: PathBuf,
    pub state_dir: PathBuf,
}

impl Store {
    pub fn new() -> Store {
        Store {
            config_dir: super::config_home().join("indium"),
            state_dir: super::state_home().join("indium"),
        }
    }

    pub fn at(config_dir: impl Into<PathBuf>, state_dir: impl Into<PathBuf>) -> Store {
        Store {
            config_dir: config_dir.into(),
            state_dir: state_dir.into(),
        }
    }

    pub fn settings_path(&self) -> PathBuf {
        self.config_dir.join("settings.toml")
    }

    pub fn recents_path(&self) -> PathBuf {
        self.state_dir.join("recents.toml")
    }

    pub fn load_settings(&self) -> Loaded<Settings> {
        load(&self.settings_path(), "settings.toml")
    }

    pub fn load_recents(&self) -> Loaded<Recents> {
        load(&self.recents_path(), "recents.toml")
    }

    pub fn save_settings(&self, settings: &Settings) -> std::io::Result<()> {
        let text = toml::to_string_pretty(settings)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        atomic_write(&self.settings_path(), text.as_bytes())
    }

    pub fn save_recents(&self, recents: &Recents) -> std::io::Result<()> {
        let text = toml::to_string_pretty(recents)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        atomic_write(&self.recents_path(), text.as_bytes())
    }

    /// Apply a change to the recents file **as it is now** — P8 §3.
    ///
    /// A window reads these files once, at startup, and CORE §1 puts as many windows on
    /// screen as the user has archives open. A window that saved its own copy back whole
    /// therefore undid everything every other window had done since it started: one
    /// window opening an archive was enough to drop a bookmark another had just added,
    /// and nothing anywhere said so, because from each window's side the write succeeded.
    ///
    /// What travels here is the change, not the caller's copy of the file. That is also
    /// what keeps a removal removed — merging two lists instead would resurrect in one
    /// window every entry the other had just deleted, which is a worse bug than the one
    /// being fixed.
    ///
    /// Returns what was written, so the caller can hold exactly what the file holds. The
    /// race is narrowed, not closed: two windows can still read, change and write across
    /// each other within the same few milliseconds. P4's lock is deliberately not taken
    /// for this — it guards a rebuild that runs for minutes, and a lock file per bookmark
    /// would cost more than the entry it saves.
    pub fn change_recents(&self, change: impl FnOnce(&mut Recents)) -> Result<Recents, String> {
        let fresh = self.load_recents();
        if fresh.was_broken {
            return Err(fresh
                .notice
                .unwrap_or_else(|| "recents.toml could not be read.".to_string()));
        }
        let mut recents = fresh.value;
        change(&mut recents);
        self.save_recents(&recents)
            .map_err(|e| format!("Could not save recent files: {e}"))?;
        Ok(recents)
    }

    /// Apply a change to the settings file as it is now. Shaped exactly like
    /// `change_recents`, and for every reason given there.
    ///
    /// One trap the callers carry: a change expressed as an **index** is a change to a
    /// list this window has not seen. Bookmarks are therefore removed by identity.
    pub fn change_settings(&self, change: impl FnOnce(&mut Settings)) -> Result<Settings, String> {
        let fresh = self.load_settings();
        if fresh.was_broken {
            return Err(fresh
                .notice
                .unwrap_or_else(|| "settings.toml could not be read.".to_string()));
        }
        let mut settings = fresh.value;
        change(&mut settings);
        self.save_settings(&settings)
            .map_err(|e| format!("Could not save settings: {e}"))?;
        Ok(settings)
    }
}

impl Default for Store {
    fn default() -> Self {
        Store::new()
    }
}

fn load<T: Default + for<'de> Deserialize<'de>>(path: &Path, label: &str) -> Loaded<T> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        // A file that has never existed is not an error; it is a first run.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Loaded::clean(T::default()),
        Err(e) => {
            return Loaded {
                value: T::default(),
                notice: Some(format!("Could not read {label}: {e}. Using defaults.")),
                was_broken: true,
            }
        }
    };

    match toml::from_str::<T>(&text) {
        Ok(value) => Loaded::clean(value),
        Err(e) => {
            let broken = set_extension_suffix(path, "broken");
            // "copy the file aside **once**" — an existing .broken is the first
            // breakage and is worth more than the latest one, so it is not replaced.
            let kept = if broken.exists() {
                true
            } else {
                std::fs::copy(path, &broken).is_ok()
            };
            let where_ = if kept {
                format!(" A copy is at {}.", broken.display())
            } else {
                String::new()
            };
            Loaded {
                value: T::default(),
                notice: Some(format!(
                    "{label} could not be parsed ({}). Running on defaults.{where_}",
                    first_line(&e.to_string())
                )),
                was_broken: true,
            }
        }
    }
}

fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or("").trim().to_string()
}

/// `settings.toml` -> `settings.toml.broken`, keeping the original name intact so it
/// is obvious what the file was.
fn set_extension_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".");
    name.push(suffix);
    path.with_file_name(name)
}

/// Write via a temporary file in the same directory, flushed and fsynced, then
/// renamed over the target.
///
/// P2 §1: "Atomic writes only ... The same discipline the archive rebuild will use in
/// P4." A half-written settings file must never be possible, whatever happens
/// mid-write.
///
/// **The temporary file carries the writer's process id — P8 §3.** P2 named it
/// `settings.toml.tmp`, which is one name for every INDIUM on the machine: two windows
/// saving at the same moment both truncate it, both write into it, and the two
/// serialisations interleave before either rename. The rename was always atomic; what
/// it committed was not necessarily either window's file. The pid makes the scratch
/// name a window's own, and the rename stays exactly as atomic as it was.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = set_extension_suffix(path, &format!("tmp.{}", std::process::id()));

    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.flush()?;
        f.sync_all()?;
    }

    match std::fs::rename(&tmp, path) {
        Ok(()) => {}
        Err(e) => {
            // Never leave the scratch file lying around on failure.
            let _ = std::fs::remove_file(&tmp);
            return Err(e);
        }
    }

    // Durability of the rename itself needs the directory synced too.
    if let Some(dir) = path.parent() {
        if let Ok(d) = std::fs::File::open(dir) {
            let _ = d.sync_all();
        }
    }
    Ok(())
}

/// Now, in unix seconds.
pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Tmp(PathBuf);

    impl Tmp {
        fn new(tag: &str) -> Tmp {
            use std::sync::atomic::{AtomicU32, Ordering};
            static N: AtomicU32 = AtomicU32::new(0);
            let p = std::env::temp_dir().join(format!(
                "indium-store-{}-{}-{}",
                tag,
                std::process::id(),
                N.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = std::fs::remove_dir_all(&p);
            std::fs::create_dir_all(&p).unwrap();
            Tmp(p)
        }
        fn store(&self) -> Store {
            Store::at(self.0.join("config"), self.0.join("state"))
        }
    }

    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// P2 §6: "settings round-trip: write, read back, equal; the `.tmp` never remains."
    #[test]
    fn settings_round_trip_and_leave_no_tmp() {
        let tmp = Tmp::new("roundtrip");
        let store = tmp.store();

        let settings = Settings {
            extract: ExtractSettings {
                default: ExtractDefault::Here,
                // Set even though `Here` is the mode: PXX 8.11 keeps the path across a
                // change of mode, so a round trip that only ever saw it empty would not
                // be testing the thing that has to survive.
                preselect: "/home/megas/somewhere else".into(),
            },
            bookmarks: vec![Bookmark {
                name: "Downloads".into(),
                path: "/home/megas/Downloads".into(),
            }],
        };

        store.save_settings(&settings).unwrap();
        let back = store.load_settings();
        assert_eq!(back.value, settings);
        assert!(!back.was_broken);
        assert!(back.notice.is_none());

        assert!(
            no_scratch_beside(&store.settings_path()),
            "the .tmp file was left behind"
        );
    }

    /// Is there nothing beside `path` that looks like a half-finished write?
    ///
    /// Asked by prefix rather than by one exact name. The two callers used to name
    /// `settings.toml.tmp` outright, and P8 §3 put the writer's process id on the end of
    /// it — which would have left both of them asserting the absence of a file the
    /// program had stopped writing, passing for ever and testing nothing.
    fn no_scratch_beside(path: &Path) -> bool {
        let dir = path.parent().unwrap();
        let prefix = format!("{}.tmp", path.file_name().unwrap().to_string_lossy());
        !std::fs::read_dir(dir)
            .unwrap()
            .flatten()
            .any(|e| e.file_name().to_string_lossy().starts_with(prefix.as_str()))
    }

    #[test]
    fn a_missing_file_is_a_first_run_not_an_error() {
        let tmp = Tmp::new("missing");
        let loaded = tmp.store().load_settings();
        assert_eq!(loaded.value, Settings::default());
        assert!(!loaded.was_broken, "absence is not breakage");
        assert!(loaded.notice.is_none());
    }

    /// P2 §6: "malformed `settings.toml` → defaults load, `.broken` copy exists,
    /// original untouched."
    #[test]
    fn a_malformed_file_falls_back_and_is_kept_aside() {
        let tmp = Tmp::new("broken");
        let store = tmp.store();
        let path = store.settings_path();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let garbage = "this is not [valid toml at all";
        std::fs::write(&path, garbage).unwrap();

        let loaded = store.load_settings();
        assert_eq!(loaded.value, Settings::default(), "defaults must load");
        assert!(loaded.was_broken);
        assert!(loaded.notice.is_some(), "the user must be told");

        let broken = set_extension_suffix(&path, "broken");
        assert!(broken.exists(), "no .broken copy was made");
        assert_eq!(std::fs::read_to_string(&broken).unwrap(), garbage);
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            garbage,
            "the original must be left untouched"
        );
    }

    #[test]
    fn the_first_broken_copy_is_not_overwritten_by_a_later_one() {
        let tmp = Tmp::new("broken-twice");
        let store = tmp.store();
        let path = store.settings_path();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();

        std::fs::write(&path, "first breakage [").unwrap();
        let _ = store.load_settings();
        std::fs::write(&path, "second breakage [").unwrap();
        let _ = store.load_settings();

        let broken = set_extension_suffix(&path, "broken");
        assert_eq!(
            std::fs::read_to_string(&broken).unwrap(),
            "first breakage [",
            "the original breakage is the one worth keeping"
        );
    }

    /// A user editing the file by hand must be obeyed. P2's manual checklist:
    /// "hand-edit `settings.toml` in a text editor while INDIUM is closed — the change
    /// is respected on launch".
    #[test]
    fn a_hand_written_file_is_respected() {
        let tmp = Tmp::new("handwritten");
        let store = tmp.store();
        let path = store.settings_path();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            "[extract]\ndefault = \"here\"\n\n[[bookmark]]\nname = \"Code\"\npath = \"/srv/code\"\n",
        )
        .unwrap();

        let loaded = store.load_settings();
        assert!(!loaded.was_broken);
        assert_eq!(loaded.value.extract.default, ExtractDefault::Here);
        assert_eq!(loaded.value.bookmarks.len(), 1);
        assert_eq!(loaded.value.bookmarks[0].name, "Code");
    }

    /// P2 §6: "recents: 16th insert evicts the oldest; reopening bumps order."
    #[test]
    fn recents_cap_at_fifteen() {
        let mut r = Recents::default();
        for i in 0..16 {
            r.bump(&format!("/archive-{i}.zip"), 1000 + i as i64);
        }
        assert_eq!(r.items.len(), RECENT_CAP);
        assert!(
            !r.items.iter().any(|x| x.path == "/archive-0.zip"),
            "the oldest must have been evicted"
        );
        assert_eq!(r.items[0].path, "/archive-15.zip");
    }

    #[test]
    fn reopening_bumps_to_the_top_without_duplicating() {
        let mut r = Recents::default();
        r.bump("/a.zip", 100);
        r.bump("/b.zip", 200);
        r.bump("/a.zip", 300);

        assert_eq!(r.items.len(), 2, "reopening must not duplicate");
        assert_eq!(r.items[0].path, "/a.zip");
        assert_eq!(r.items[0].opened, 300);
    }

    #[test]
    fn recents_sort_newest_first_even_if_the_file_was_shuffled() {
        let r = Recents {
            items: vec![
                Recent {
                    path: "/old.zip".into(),
                    opened: 10,
                },
                Recent {
                    path: "/new.zip".into(),
                    opened: 99,
                },
            ],
        };
        assert_eq!(r.sorted()[0].path, "/new.zip");
    }

    #[test]
    fn recents_are_removed_only_by_hand() {
        let mut r = Recents::default();
        r.bump("/a.zip", 1);
        r.bump("/b.zip", 2);
        r.remove("/a.zip");
        assert_eq!(r.items.len(), 1);
        assert_eq!(r.items[0].path, "/b.zip");
    }

    #[test]
    fn recents_round_trip_through_toml() {
        let tmp = Tmp::new("recents");
        let store = tmp.store();
        let mut r = Recents::default();
        r.bump("/data/backup-2026.7z", 1_786_142_074);
        store.save_recents(&r).unwrap();
        assert_eq!(store.load_recents().value, r);
    }

    #[test]
    fn atomic_write_replaces_content_completely() {
        let tmp = Tmp::new("atomic");
        let path = tmp.0.join("x/y/file.toml");
        atomic_write(&path, b"first, and longer than the second").unwrap();
        atomic_write(&path, b"second").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "second");
        assert!(no_scratch_beside(&path));
    }

    /// P8 §3. CORE §1 puts two windows on screen the moment a user opens two archives,
    /// and each of them read this file once, at startup. A window that wrote its own copy
    /// back whole undid everything the other had done since — silently, because from
    /// each side the write succeeded.
    #[test]
    fn a_second_windows_save_does_not_undo_the_first() {
        let tmp = Tmp::new("two-windows");
        let store = tmp.store();

        // Two windows, each changing the file without having seen the other's change.
        store
            .change_recents(|r| r.bump("/archives/photos.7z", 100))
            .unwrap();
        store
            .change_recents(|r| r.bump("/archives/docs.zip", 200))
            .unwrap();

        let back = store.load_recents().value;
        let paths: Vec<&str> = back.items.iter().map(|r| r.path.as_str()).collect();
        assert!(
            paths.contains(&"/archives/photos.7z"),
            "the first window's archive was dropped by the second window's save: {paths:?}"
        );
        assert!(paths.contains(&"/archives/docs.zip"));
    }

    /// The reason the change travels rather than the copy. Merging the two lists instead
    /// would have made every removal temporary: the window that had not seen it would put
    /// the entry back the next time it saved anything at all.
    #[test]
    fn a_removal_in_one_window_is_not_resurrected_by_the_other() {
        let tmp = Tmp::new("resurrect");
        let store = tmp.store();

        store
            .change_recents(|r| r.bump("/archives/photos.7z", 100))
            .unwrap();
        store
            .change_recents(|r| r.bump("/archives/docs.zip", 200))
            .unwrap();

        // One window forgets an archive.
        store
            .change_recents(|r| r.remove("/archives/photos.7z"))
            .unwrap();
        // The other window, which still has it listed, opens something else.
        store
            .change_recents(|r| r.bump("/archives/music.tar.zst", 300))
            .unwrap();

        let paths: Vec<String> = store
            .load_recents()
            .value
            .items
            .iter()
            .map(|r| r.path.clone())
            .collect();
        assert!(
            !paths.contains(&"/archives/photos.7z".to_string()),
            "a forgotten archive came back: {paths:?}"
        );
        assert_eq!(paths.len(), 2);
    }

    /// P2 §1's rule survives the new path: a file that cannot be parsed is never written
    /// over, and the caller is given the sentence rather than a silent success.
    #[test]
    fn a_change_never_overwrites_a_file_that_will_not_parse() {
        let tmp = Tmp::new("change-broken");
        let store = tmp.store();

        std::fs::create_dir_all(store.recents_path().parent().unwrap()).unwrap();
        std::fs::write(store.recents_path(), "this is not [ toml").unwrap();

        let refused = store.change_recents(|r| r.bump("/archives/photos.7z", 100));
        assert!(refused.is_err(), "a broken file must not be written over");
        assert_eq!(
            std::fs::read_to_string(store.recents_path()).unwrap(),
            "this is not [ toml",
            "the user's file was changed anyway"
        );
    }

    /// P8 §3's other half. The scratch file P2 wrote was `settings.toml.tmp` — one name
    /// for every INDIUM on the machine, so two windows saving together truncated and
    /// wrote into the same file and the rename committed a splice of both. A window's
    /// scratch name is now its own, and it leaves everyone else's alone.
    #[test]
    fn a_write_does_not_touch_another_windows_half_finished_one() {
        let tmp = Tmp::new("foreign-tmp");
        let path = tmp.0.join("settings.toml");
        std::fs::create_dir_all(&tmp.0).unwrap();

        // A different window, mid-write.
        let theirs = tmp.0.join("settings.toml.tmp.999999");
        std::fs::write(&theirs, b"half of another window's settings").unwrap();

        atomic_write(&path, b"ours").unwrap();

        assert_eq!(std::fs::read_to_string(&path).unwrap(), "ours");
        assert_eq!(
            std::fs::read_to_string(&theirs).unwrap(),
            "half of another window's settings",
            "one window's save wrote through another window's scratch file"
        );
    }

    /// CORE §9: passwords are never written anywhere. Nothing in these shapes can
    /// carry one, and this test is here so a future field cannot quietly introduce it.
    #[test]
    fn no_settings_field_can_hold_a_password() {
        let settings = Settings {
            extract: ExtractSettings {
                default: ExtractDefault::Subdir,
                // PXX 8.11 added this field, which is exactly the event the test was
                // written to catch. It holds a directory: a place, never a credential.
                preselect: "/p/preselected".into(),
            },
            bookmarks: vec![Bookmark {
                name: "n".into(),
                path: "/p".into(),
            }],
        };
        let text = toml::to_string_pretty(&settings).unwrap().to_lowercase();
        for forbidden in ["password", "passphrase", "secret", "token", "key"] {
            assert!(!text.contains(forbidden), "settings mention {forbidden}");
        }
    }

    /// PXX 8.11 added a field to a file that already exists in every install that has ever
    /// run. A `settings.toml` written before it — which is all of them — has to keep
    /// parsing, or an upgrade quietly costs the user their bookmarks and their mode.
    ///
    /// `#[serde(default)]` is what makes that true, and this is the test that says so
    /// rather than the attribute saying it about itself.
    #[test]
    fn a_settings_file_written_before_preselect_existed_still_parses() {
        let tmp = Tmp::new("preselect-upgrade");
        let store = tmp.store();
        std::fs::create_dir_all(store.settings_path().parent().unwrap()).unwrap();
        std::fs::write(
            store.settings_path(),
            "[extract]\ndefault = \"here\"\n\n\
             [[bookmark]]\nname = \"hagda\"\npath = \"/tmp/one\"\n",
        )
        .unwrap();

        let loaded = store.load_settings();
        assert!(!loaded.was_broken, "a pre-PXX settings.toml read as broken");
        assert_eq!(loaded.value.extract.default, ExtractDefault::Here);
        assert_eq!(
            loaded.value.extract.preselect, "",
            "an absent preselect is an empty one, not a parse failure"
        );
        assert_eq!(loaded.value.bookmarks.len(), 1, "the bookmarks survived");
    }

    /// The new mode has to survive the file as its own word: `rename_all = "lowercase"` is
    /// what decides what gets written, and a variant that serialises to something the next
    /// load cannot read is a setting that silently resets.
    #[test]
    fn preselect_round_trips_as_its_own_word() {
        let settings = Settings {
            extract: ExtractSettings {
                default: ExtractDefault::Preselect,
                preselect: "/home/megas/extracted".into(),
            },
            bookmarks: Vec::new(),
        };
        let text = toml::to_string_pretty(&settings).unwrap();
        assert!(text.contains("default = \"preselect\""), "{text}");
        assert_eq!(toml::from_str::<Settings>(&text).unwrap(), settings);
    }
}
