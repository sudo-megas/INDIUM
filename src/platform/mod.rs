//! The Linux specifics.
//!
//! CORE §3 gives this module "clipboard, `.desktop` parsing for Open With,
//! default-app registration, XDG paths". P1 and P2 need only the paths and the TOML
//! store; the rest arrives with P3.

pub mod store;

use std::path::PathBuf;

/// `$XDG_CONFIG_HOME`, or `~/.config` when it is unset.
pub fn config_home() -> PathBuf {
    xdg_dir("XDG_CONFIG_HOME", ".config")
}

/// `$XDG_STATE_HOME`, or `~/.local/state` when it is unset.
pub fn state_home() -> PathBuf {
    xdg_dir("XDG_STATE_HOME", ".local/state")
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
