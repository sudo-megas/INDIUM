//! The Linux specifics.
//!
//! CORE §3 gives this module "clipboard, `.desktop` parsing for Open With,
//! default-app registration, XDG paths, the second window ... and handing a directory to
//! the desktop's file manager". P1 and P2 need only the paths and the TOML store; the rest
//! arrives with P3. The window arrives with P8: on this platform a window is a process,
//! which is what makes it a Linux specific like the others. The file manager arrives with
//! P13, and goes through the portal for the reason `picker` already does.

pub mod apps;
pub mod clipboard;
pub mod open;
pub mod picker;
pub mod scratch;
pub mod store;
pub mod window;

use std::path::PathBuf;

/// `$XDG_CONFIG_HOME`, or `~/.config` when it is unset.
pub fn config_home() -> PathBuf {
    xdg_dir("XDG_CONFIG_HOME", ".config")
}

/// `$XDG_STATE_HOME`, or `~/.local/state` when it is unset.
pub fn state_home() -> PathBuf {
    xdg_dir("XDG_STATE_HOME", ".local/state")
}

/// `$XDG_RUNTIME_DIR`, or the cache directory when it is unset.
///
/// The runtime directory is the right home for state that should not outlive the
/// session — P4's Apply locks live here, so a crash leaves nothing a logout will not
/// clear. `$XDG_RUNTIME_DIR` can be legitimately absent (CORE §9 permits running as
/// root, and root often has none), and the fallback is silent, exactly as
/// `platform::scratch` already treats the same absence.
pub fn runtime_or_cache_dir() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| xdg_dir("XDG_CACHE_HOME", ".cache"))
}

fn xdg_dir(var: &str, fallback: &str) -> PathBuf {
    match std::env::var_os(var) {
        // The spec says a relative value is invalid and must be ignored.
        Some(v) if !v.is_empty() && PathBuf::from(&v).is_absolute() => PathBuf::from(v),
        _ => home().join(fallback),
    }
}

/// The user's home directory. Falls back to the current directory rather than
/// panicking — CORE §9 permits running as root, and a broken environment should
/// degrade, not crash.
pub fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| PathBuf::from("."))
}
