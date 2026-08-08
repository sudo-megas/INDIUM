//! `Ctrl+C` — copy-out, P3 §2.
//!
//! "The mechanism that replaced drag-out (CORE §9), and it must feel better than drag,
//! not apologetic about it."
//!
//! URI building is hand-written and unit-tested (CORE §2: no crate for twenty lines).

use std::path::Path;

use wl_clipboard_rs::copy::{self, MimeSource, MimeType, Options, Source};

/// Percent-encode one path into a `file://` URI.
///
/// P3 §2 fixes the rule exactly: "percent-encode every byte outside RFC 3986 unreserved
/// (`A–Z a–z 0–9 - . _ ~`), keeping `/` as the separator; space is `%20`, never `+`;
/// paths are absolute".
pub fn path_to_uri(path: &Path) -> String {
    use std::os::unix::ffi::OsStrExt;

    let mut out = String::from("file://");
    for &b in path.as_os_str().as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                out.push(b as char);
            }
            // Uppercase hex: RFC 3986 says either case is valid but uppercase is
            // preferred, and consistency makes the tests readable.
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// The `text/uri-list` payload. P3 §2: "lines end `\r\n` per the spec."
pub fn build_uri_list(paths: &[std::path::PathBuf]) -> String {
    let mut out = String::new();
    for p in paths {
        out.push_str(&path_to_uri(p));
        out.push_str("\r\n");
    }
    out
}

/// The `text/plain` payload: plain newline-separated paths, "so a paste into a
/// terminal does something sensible too" (P3 §2).
pub fn build_plain_list(paths: &[std::path::PathBuf]) -> String {
    let mut out = String::new();
    for p in paths {
        out.push_str(&p.to_string_lossy());
        out.push('\n');
    }
    out
}

/// Offer the paths on the Wayland clipboard.
///
/// Two MIME sources, as P3 §2 specifies. The crate serves requests from a thread inside
/// this process, which is why P3 states the honest behaviour plainly: "the offer lives
/// as long as INDIUM does. Close INDIUM before pasting and the clipboard is empty —
/// that is Wayland, and a clipboard manager is the user's own business, not a
/// dependency INDIUM grows."
pub fn offer(paths: &[std::path::PathBuf]) -> Result<(), String> {
    if paths.is_empty() {
        return Err("nothing to copy".to_string());
    }

    let uri_list = build_uri_list(paths);
    let plain = build_plain_list(paths);

    let sources = vec![
        MimeSource {
            source: Source::Bytes(uri_list.into_bytes().into_boxed_slice()),
            mime_type: MimeType::Specific("text/uri-list".to_string()),
        },
        MimeSource {
            source: Source::Bytes(plain.into_bytes().into_boxed_slice()),
            mime_type: MimeType::Specific("text/plain;charset=utf-8".to_string()),
        },
    ];

    // Default options: the serving thread stays in this process, and requests are
    // served indefinitely for as long as INDIUM lives.
    copy::copy_multi(Options::new(), sources).map_err(|e| format!("Clipboard: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// P3 §5: "URI encoding: spaces, UTF-8 (`ödev.txt`), `#`, `%`, `?` — exact expected
    /// strings."
    #[test]
    fn spaces_become_percent_twenty_never_plus() {
        let uri = path_to_uri(Path::new("/tmp/a b.txt"));
        assert_eq!(uri, "file:///tmp/a%20b.txt");
        assert!(!uri.contains('+'), "space must never encode as +");
    }

    #[test]
    fn utf8_is_encoded_byte_by_byte() {
        // 'ö' is U+00F6, which is 0xC3 0xB6 in UTF-8.
        assert_eq!(
            path_to_uri(Path::new("/tmp/ödev.txt")),
            "file:///tmp/%C3%B6dev.txt"
        );
    }

    #[test]
    fn reserved_characters_are_encoded() {
        assert_eq!(
            path_to_uri(Path::new("/tmp/a#b.txt")),
            "file:///tmp/a%23b.txt"
        );
        assert_eq!(
            path_to_uri(Path::new("/tmp/a%b.txt")),
            "file:///tmp/a%25b.txt"
        );
        assert_eq!(
            path_to_uri(Path::new("/tmp/a?b.txt")),
            "file:///tmp/a%3Fb.txt"
        );
        assert_eq!(
            path_to_uri(Path::new("/tmp/a&b.txt")),
            "file:///tmp/a%26b.txt"
        );
    }

    #[test]
    fn unreserved_characters_are_left_alone() {
        assert_eq!(
            path_to_uri(Path::new("/tmp/Az09-._~/x.txt")),
            "file:///tmp/Az09-._~/x.txt"
        );
    }

    #[test]
    fn separators_stay_separators() {
        assert_eq!(
            path_to_uri(Path::new("/a/b/c/d.txt")),
            "file:///a/b/c/d.txt",
            "slashes must not be encoded"
        );
    }

    #[test]
    fn the_uri_list_ends_every_line_with_crlf() {
        let paths = vec![PathBuf::from("/tmp/a.txt"), PathBuf::from("/tmp/b b.txt")];
        assert_eq!(
            build_uri_list(&paths),
            "file:///tmp/a.txt\r\nfile:///tmp/b%20b.txt\r\n"
        );
    }

    #[test]
    fn the_plain_list_is_raw_paths_one_per_line() {
        let paths = vec![PathBuf::from("/tmp/a.txt"), PathBuf::from("/tmp/b b.txt")];
        assert_eq!(build_plain_list(&paths), "/tmp/a.txt\n/tmp/b b.txt\n");
    }

    #[test]
    fn an_empty_selection_is_refused_rather_than_offered() {
        assert!(offer(&[]).is_err());
    }

    /// P3 §5's manual check, automated but opt-in: "`wl-paste --list-types` shows
    /// `text/uri-list` and `text/plain;charset=utf-8`; `wl-paste -t text/uri-list`
    /// prints correct URIs".
    ///
    /// Ignored by default because it needs a live Wayland session and `wl-paste`, and
    /// it puts something on the real clipboard. Run it deliberately:
    /// `cargo test --lib -- --ignored offer_reaches_the_wayland_clipboard`
    #[test]
    #[ignore = "needs a live Wayland session and wl-paste"]
    fn offer_reaches_the_wayland_clipboard() {
        let paths = vec![
            PathBuf::from("/tmp/indium check.txt"),
            PathBuf::from("/tmp/ödev.txt"),
        ];
        offer(&paths).expect("the offer should be accepted");

        let types = std::process::Command::new("wl-paste")
            .arg("--list-types")
            .output()
            .expect("wl-paste --list-types");
        let types = String::from_utf8_lossy(&types.stdout);
        assert!(types.contains("text/uri-list"), "types were: {types}");
        assert!(types.contains("text/plain"), "types were: {types}");

        let uris = std::process::Command::new("wl-paste")
            .args(["-t", "text/uri-list"])
            .output()
            .expect("wl-paste -t text/uri-list");
        let uris = String::from_utf8_lossy(&uris.stdout);
        assert!(
            uris.contains("file:///tmp/indium%20check.txt"),
            "got: {uris}"
        );
        assert!(uris.contains("file:///tmp/%C3%B6dev.txt"), "got: {uris}");
    }
}

// ---------------------------------------------------------------------------
// The read half — `Ctrl+V`, P4 §5
// ---------------------------------------------------------------------------

/// Read file paths off the Wayland clipboard.
///
/// P3 built only the offer; staging an add needs the other direction. `text/uri-list`
/// first, then plain text, which is the same pair `offer` puts out — so a copy from
/// INDIUM pastes back into INDIUM, and so does a copy from any file manager.
///
/// An empty clipboard is `Ok(vec![])`, not an error: the crate's own documentation groups
/// `NoMimeType`, `ClipboardEmpty` and `NoSeats` as "nothing to worry about", and a user
/// who presses `Ctrl+V` with nothing copied has made no mistake worth a message.
///
/// **This blocks.** `get_contents` hands a pipe to the selection's owner and the read
/// runs until that program finishes writing; a slow or wedged source would freeze
/// whatever thread calls this. It must never be called from the UI thread.
pub fn paste_paths() -> Result<Vec<std::path::PathBuf>, String> {
    use wl_clipboard_rs::paste::{
        get_contents, ClipboardType, Error as PasteError, MimeType as PasteMime, Seat,
    };

    fn read(mime: PasteMime<'_>) -> Result<Vec<u8>, Option<String>> {
        use std::io::Read as _;
        match get_contents(ClipboardType::Regular, Seat::Unspecified, mime) {
            Ok((mut pipe, _actual)) => {
                let mut buf = Vec::new();
                pipe.read_to_end(&mut buf)
                    .map_err(|e| Some(format!("Clipboard: {e}")))?;
                Ok(buf)
            }
            Err(PasteError::NoMimeType)
            | Err(PasteError::ClipboardEmpty)
            | Err(PasteError::NoSeats) => Err(None),
            Err(e) => Err(Some(format!("Clipboard: {e}"))),
        }
    }

    match read(PasteMime::Specific("text/uri-list")) {
        Ok(bytes) => return Ok(parse_uri_list(&bytes)),
        Err(None) => {}
        Err(Some(e)) => return Err(e),
    }
    match read(PasteMime::Specific("text/plain;charset=utf-8")) {
        Ok(bytes) => Ok(parse_plain_list(&bytes)),
        Err(None) => Ok(Vec::new()),
        Err(Some(e)) => Err(e),
    }
}

/// The exact inverse of `path_to_uri`, hand-written for the same reason it was.
///
/// RFC 2483: lines end `\r\n` and a line beginning `#` is a comment. Percent-decoding is
/// byte-wise rather than through a string, because an archive member's name is not
/// guaranteed to be UTF-8 and neither is a filename.
pub fn parse_uri_list(bytes: &[u8]) -> Vec<std::path::PathBuf> {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let mut out = Vec::new();
    for line in bytes.split(|&b| b == b'\n') {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if line.is_empty() || line[0] == b'#' {
            continue;
        }
        let Some(rest) = line.strip_prefix(b"file://") else {
            continue;
        };
        // `file://host/path` — an authority we do not recognise is not ours to open.
        let rest = match rest.first() {
            Some(b'/') => rest,
            _ => match rest.iter().position(|&b| b == b'/') {
                Some(i) => &rest[i..],
                None => continue,
            },
        };
        let mut buf = Vec::with_capacity(rest.len());
        let mut i = 0;
        while i < rest.len() {
            if rest[i] == b'%' && i + 2 < rest.len() {
                let hex = std::str::from_utf8(&rest[i + 1..i + 3])
                    .ok()
                    .and_then(|h| u8::from_str_radix(h, 16).ok());
                if let Some(b) = hex {
                    buf.push(b);
                    i += 3;
                    continue;
                }
            }
            buf.push(rest[i]);
            i += 1;
        }
        out.push(std::path::PathBuf::from(OsString::from_vec(buf)));
    }
    out
}

/// The inverse of `build_plain_list`. Absolute paths only — a relative one from another
/// program means nothing here, since INDIUM has no idea what it was relative to.
pub fn parse_plain_list(bytes: &[u8]) -> Vec<std::path::PathBuf> {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    bytes
        .split(|&b| b == b'\n')
        .map(|l| l.strip_suffix(b"\r").unwrap_or(l))
        .filter(|l| !l.is_empty() && l[0] == b'/')
        .map(|l| std::path::PathBuf::from(OsStr::from_bytes(l)))
        .collect()
}

#[cfg(test)]
mod paste_tests {
    use super::*;
    use std::path::PathBuf;

    /// P3 §2's encoder and P4's decoder must be exact inverses, including on the names
    /// P3 chose precisely because they are awkward.
    #[test]
    fn a_file_uri_round_trips_through_encode_and_decode() {
        let cases = [
            "/home/megas/plain.txt",
            "/home/megas/with space.txt",
            "/home/megas/ödev.txt",
            "/home/megas/hash#and%percent?.txt",
            "/home/megas/sub dir/inner file.tar.gz",
        ];
        for case in cases {
            let uri = path_to_uri(Path::new(case));
            let back = parse_uri_list(uri.as_bytes());
            assert_eq!(back, vec![PathBuf::from(case)], "{case}");
        }
    }

    #[test]
    fn a_uri_list_ignores_comments_and_foreign_schemes() {
        let list = b"#comment\r\nfile:///a.txt\r\nhttps://example.invalid/b.txt\r\n";
        assert_eq!(parse_uri_list(list), vec![PathBuf::from("/a.txt")]);
    }

    #[test]
    fn a_localhost_authority_is_accepted_and_stripped() {
        assert_eq!(
            parse_uri_list(b"file://localhost/a.txt\r\n"),
            vec![PathBuf::from("/a.txt")]
        );
    }

    #[test]
    fn plain_text_paths_must_be_absolute_to_be_taken() {
        let list = b"/absolute.txt\nrelative.txt\n\n/second.txt\n";
        assert_eq!(
            parse_plain_list(list),
            vec![PathBuf::from("/absolute.txt"), PathBuf::from("/second.txt")]
        );
    }
}
