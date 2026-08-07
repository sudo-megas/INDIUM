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
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ExtractSettings {
    #[serde(default)]
    pub default: ExtractDefault,
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
pub fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = set_extension_suffix(path, "tmp");

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

        let tmp_path = set_extension_suffix(&store.settings_path(), "tmp");
        assert!(!tmp_path.exists(), "the .tmp file was left behind");
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
        assert!(!set_extension_suffix(&path, "tmp").exists());
    }

    /// CORE §9: passwords are never written anywhere. Nothing in these shapes can
    /// carry one, and this test is here so a future field cannot quietly introduce it.
    #[test]
    fn no_settings_field_can_hold_a_password() {
        let settings = Settings {
            extract: ExtractSettings {
                default: ExtractDefault::Subdir,
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
}
