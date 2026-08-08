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
