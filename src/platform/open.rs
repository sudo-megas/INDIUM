//! Hand a directory to the desktop's file manager — P13 §3.
//!
//! CORE §3 grants this module "handing a directory to the desktop's file manager, which is
//! the portal's job for the same reason the picker is", and CORE §4 says why the status bar
//! offers it: the directory on row 1 is the answer to *where is this archive*, and a person
//! who is asking that usually wants to be there rather than merely told.
//!
//! **The portal, not `xdg-open`.** `picker.rs` set that precedent and the argument is the
//! same one: the file manager should be the one the user has already chosen, the portal is
//! the only route that survives a sandbox, and shelling out to a helper that may not be
//! installed turns a missing package into a silent nothing. `OpenDirectory` also *selects*
//! the archive in the folder it opens where the backend supports it, which `xdg-open` on a
//! directory cannot do at all.
//!
//! It costs no dependency. `ashpd`'s `open_uri` feature is `open_uri = []` upstream — no
//! crates, no linkage, only D-Bus calls over the `zbus` P11 already brought in — which is
//! why `build/check-deps.sh` reads the same before and after.

use std::path::Path;

use ashpd::desktop::open_uri::OpenDirectoryRequest;

/// Ask the desktop to show `dir` in its file manager.
///
/// **This blocks on a D-Bus round trip**, and on a portal backend that decides to ask the
/// user something it blocks for as long as they think about it. It carries the same rule
/// `picker::open_files` states in its own doc comment and for the same reason: it must
/// never be called from the UI thread. `Indium` runs it on a worker.
///
/// A desktop with no portal is not a bug in INDIUM and does not get INDIUM's blame — it
/// gets a sentence saying what is missing, the same shape `picker::describe` writes.
pub fn open_directory(dir: &Path) -> Result<(), String> {
    // Opened before the async block so a path that does not exist — a bookmark to a
    // deleted folder, an archive on a stick that has been pulled — fails here with the
    // operating system's own words rather than somewhere inside the portal.
    let handle =
        std::fs::File::open(dir).map_err(|e| format!("Could not open {}: {e}", dir.display()))?;

    futures_lite::future::block_on(async move {
        use std::os::fd::AsFd;
        OpenDirectoryRequest::default()
            .send(&handle.as_fd())
            .await
            .map_err(describe)?;
        Ok(())
    })
}

/// The portal's failures in INDIUM's voice.
///
/// Deliberately parallel to `picker::describe`: the two are the only places this program
/// talks to `xdg-desktop-portal`, and a user who meets both should not be told about the
/// same missing service in two different vocabularies.
fn describe(e: ashpd::Error) -> String {
    match e {
        ashpd::Error::Zbus(_) | ashpd::Error::PortalNotFound(_) => {
            "No file manager is available — this desktop has no xdg-desktop-portal running."
                .to_string()
        }
        other => format!("Could not open the folder: {other}"),
    }
}
