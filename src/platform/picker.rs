//! The system file picker, through `xdg-desktop-portal` — P11 §4.
//!
//! CORE §4 gives INDIUM two ways to name a file it does not already hold: `Ctrl+O` and a
//! path field with tab completion, or a Browse… button that raises this picker. The path
//! field was not the only way anyone expected, and the testing round said so plainly — the
//! first note back was "we need an open file option, and it must use the xdg-portal file
//! picker". CORE §4 now names both.
//!
//! It matters more than a convenience. Until P11, adding a file to an archive had exactly
//! two routes: `Ctrl+V`, which had never worked (see `ui::clipboard_chords`), and a drop
//! onto the window, which **cannot** work here — `winit-0.30.13` emits `DroppedFile` only
//! from its X11 backend and has no Wayland data-device code at all. On a Wayland session
//! that left no route whatsoever. The portal is the third, and the first one that does not
//! depend on the toolkit having got something right.
//!
//! The portal, rather than a dialog of INDIUM's own, because the picker belongs to the
//! desktop: it is the one the user has already chosen, it honours their bookmarks and
//! their recent files, and it is the only kind that keeps working inside a sandbox. It is
//! also the only file dialog INDIUM can offer without drawing one, and CORE §6 has no
//! vocabulary for a file dialog.

use std::path::PathBuf;

use ashpd::desktop::file_chooser::OpenFileRequest;

/// What the picker was opened for. Every arm ends somewhere different, and the answer comes
/// back on a channel long after the button was clicked, so it has to carry its own reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerFor {
    /// Name an archive to open. Since P22 it takes *this* window, having closed what the
    /// window held — CORE §1, as the maker amended it.
    Open,
    /// Name files to stage as adds into the archive already open.
    Add,
    /// Name files to put in the draft. Distinct from `Add` because a draft has no current
    /// directory to land in: it is the whole archive-to-be, so a file takes its own name.
    Draft,
    /// Name the one directory every extraction preselects — PXX 8.11.
    ///
    /// The only arm that asks the portal for a **directory** rather than files. It is
    /// also the only one whose answer is written to `settings.toml` instead of to the
    /// window, which is why it cannot borrow `Add`'s route home.
    Preselect,
}

/// Ask the desktop's file picker for one or more files, or for a directory.
///
/// `directory` is the portal's own flag rather than a filter INDIUM applies afterwards:
/// `OpenFileRequest::directory(true)` makes the backend offer folders and refuse files,
/// so the answer needs no sifting on this side. That matters because the one thing this
/// program cannot do is pick a folder out of a file dialog's reply — see `split_out_folders`
/// and the sentence it exists to print.
///
/// **This blocks for as long as the dialog is on screen** — which is to say, for as long
/// as the user is thinking. It must never be called from the UI thread; `Indium` runs it
/// on a worker and takes the answer back on a channel, the same shape `request_paste`
/// uses for the clipboard.
///
/// A cancelled dialog is `Ok(vec![])` and not an error, for the reason `paste_paths` gives
/// about an empty clipboard: a user who changed their mind has made no mistake worth a
/// sentence.
pub fn open_files(title: &str, multiple: bool, directory: bool) -> Result<Vec<PathBuf>, String> {
    futures_lite::future::block_on(async move {
        let request = OpenFileRequest::default()
            .title(title)
            .multiple(multiple)
            .directory(directory)
            .send()
            .await
            .map_err(describe)?;

        let files = match request.response() {
            Ok(files) => files,
            // The user closed the dialog. Nothing to say.
            Err(ashpd::Error::Response(ashpd::desktop::ResponseError::Cancelled)) => {
                return Ok(Vec::new())
            }
            Err(e) => return Err(describe(e)),
        };

        // `ashpd::Uri` is a thin wrapper over the string the portal sent, so what arrives
        // is `file:///…` — exactly what `Ctrl+C` puts on the clipboard and exactly what
        // P4 already wrote a decoder for. Percent-decoding a portal's answer twice, in two
        // places, is the sort of thing CORE §2 exists to prevent.
        let mut out = Vec::new();
        for uri in files.uris() {
            out.extend(super::clipboard::parse_uri_list(uri.as_str().as_bytes()));
        }
        Ok(out)
    })
}

/// One sentence a person can act on, rather than a debug-printed enum.
///
/// The failure worth naming precisely is the one that will actually happen: a desktop with
/// no portal service installed, where every other program's file dialog works because it
/// draws its own.
fn describe(e: ashpd::Error) -> String {
    match e {
        ashpd::Error::Zbus(_) | ashpd::Error::PortalNotFound(_) => {
            "No file picker is available — this desktop has no xdg-desktop-portal running. \
             Type the path instead."
                .to_string()
        }
        other => format!("The file picker failed: {other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::super::clipboard::parse_uri_list;
    use std::path::PathBuf;

    /// The portal hands back one URI per entry, percent-encoded, with no line terminator
    /// — which is *not* the `text/uri-list` shape `parse_uri_list` was written for in P4.
    /// It reads both, and this is the test that says so, because reusing that decoder
    /// rather than writing a second one is the only reason `open_files` is short.
    #[test]
    fn a_portal_uri_decodes_without_the_crlf_a_uri_list_would_carry() {
        assert_eq!(
            parse_uri_list(b"file:///home/megas/plain.txt"),
            vec![PathBuf::from("/home/megas/plain.txt")]
        );
    }

    /// The names the picker is most likely to be pointed at are the ones INDIUM spent P11
    /// learning to read: spaces and bytes outside ASCII, arriving percent-encoded.
    #[test]
    fn a_portal_uri_decodes_the_names_p11_exists_for() {
        assert_eq!(
            parse_uri_list(b"file:///tmp/xx/k%C3%B6pek.txt"),
            vec![PathBuf::from("/tmp/xx/köpek.txt")]
        );
        assert_eq!(
            parse_uri_list(b"file:///tmp/beach%20day.jpg"),
            vec![PathBuf::from("/tmp/beach day.jpg")]
        );
    }
}
