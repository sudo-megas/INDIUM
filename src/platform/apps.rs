//! `.desktop` scanning, the MIME table, and launching — P3 §3.
//!
//! CORE §3 gives `platform` "`.desktop` parsing for Open With". Everything here is
//! hand-written against the freedesktop Desktop Entry Specification; the tokenizer in
//! particular is the part that has to be right, so it is pure and heavily tested.
//!
//! Icons are deliberately not rendered in v1 (P3 §3): "icon-theme resolution is a
//! subsystem, the list is text, and the one-sentence rule of CORE §2 applies to
//! features too."

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// A parsed entry
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopApp {
    /// The desktop-file ID, e.g. `org.gnome.eog.desktop`. First found wins.
    pub id: String,
    pub name: String,
    pub exec: String,
    pub mime_types: Vec<String>,
    pub terminal: bool,
    /// Path of the file it was parsed from.
    pub path: PathBuf,
}

/// One line of the picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub app: DesktopApp,
    /// The user's registered default for this type, from `mimeapps.list`.
    pub is_default: bool,
    /// Its `MimeType=` names the type outright.
    pub exact: bool,
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Parse one `.desktop` file. `None` when it should not be offered at all.
///
/// P3 §3: "`NoDisplay` and `Hidden` are honoured; a `TryExec` that is not on `$PATH`
/// disqualifies; `Terminal=true` entries are listed".
pub fn parse_desktop(path: &Path, text: &str) -> Option<DesktopApp> {
    let mut in_entry = false;
    let mut fields: BTreeMap<&str, &str> = BTreeMap::new();

    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            // Only the main group counts; actions and other groups are ignored.
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            let k = k.trim();
            // Localised keys like `Name[de]` are ignored: English is the language of
            // v1 (CORE §7) and there is no locale detection (CORE §9).
            if !k.contains('[') {
                fields.entry(k).or_insert_with(|| v.trim());
            }
        }
    }

    if fields.get("Type").copied().unwrap_or("Application") != "Application" {
        return None;
    }
    if is_true(fields.get("NoDisplay").copied()) || is_true(fields.get("Hidden").copied()) {
        return None;
    }
    if let Some(try_exec) = fields.get("TryExec") {
        if !program_exists(try_exec) {
            return None;
        }
    }

    let exec = fields.get("Exec").copied()?.to_string();
    if exec.trim().is_empty() {
        return None;
    }
    let name = fields
        .get("Name")
        .copied()
        .unwrap_or("(unnamed)")
        .to_string();

    Some(DesktopApp {
        id: path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default(),
        name,
        exec,
        mime_types: fields
            .get("MimeType")
            .copied()
            .map(split_list)
            .unwrap_or_default(),
        terminal: is_true(fields.get("Terminal").copied()),
        path: path.to_path_buf(),
    })
}

fn is_true(v: Option<&str>) -> bool {
    matches!(v.map(str::trim), Some("true"))
}

fn split_list(v: &str) -> Vec<String> {
    v.split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

/// Is this program runnable? An absolute path is checked directly; a bare name is
/// looked up on `$PATH`.
fn program_exists(prog: &str) -> bool {
    let prog = prog.trim();
    if prog.is_empty() {
        return false;
    }
    if prog.contains('/') {
        return Path::new(prog).is_file();
    }
    std::env::var_os("PATH")
        .map(|paths| {
            std::env::split_paths(&paths).any(|dir| {
                let c = dir.join(prog);
                c.is_file()
            })
        })
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Scanning
// ---------------------------------------------------------------------------

/// Every applications directory, in precedence order.
///
/// P3 §3: "`$XDG_DATA_HOME/applications` and every `$XDG_DATA_DIRS/applications`
/// (defaults `~/.local/share`, then `/usr/local/share:/usr/share`)".
pub fn application_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    let data_home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| super::home().join(".local/share"));
    dirs.push(data_home.join("applications"));

    let data_dirs = std::env::var_os("XDG_DATA_DIRS")
        .map(|v| v.to_string_lossy().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".to_string());
    for d in data_dirs.split(':').filter(|s| !s.is_empty()) {
        dirs.push(PathBuf::from(d).join("applications"));
    }
    dirs
}

/// Scan the given directories, first-found-wins by desktop-file ID.
pub fn scan(dirs: &[PathBuf]) -> Vec<DesktopApp> {
    let mut seen: BTreeMap<String, DesktopApp> = BTreeMap::new();
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for e in entries.flatten() {
            let path = e.path();
            if path.extension().map(|x| x != "desktop").unwrap_or(true) {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            if let Some(app) = parse_desktop(&path, &text) {
                // First found wins: an earlier directory has higher precedence.
                seen.entry(app.id.clone()).or_insert(app);
            }
        }
    }
    seen.into_values().collect()
}

// ---------------------------------------------------------------------------
// mimeapps.list
// ---------------------------------------------------------------------------

/// The `[Default Applications]` mapping. P3 §3 ranks the entry for the type first and
/// tags it *default*.
pub fn parse_mimeapps(text: &str) -> BTreeMap<String, Vec<String>> {
    let mut out = BTreeMap::new();
    let mut in_defaults = false;
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            in_defaults = line == "[Default Applications]";
            continue;
        }
        if !in_defaults {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            out.insert(k.trim().to_string(), split_list(v));
        }
    }
    out
}

pub fn mimeapps_path() -> PathBuf {
    super::config_home().join("mimeapps.list")
}

// ---------------------------------------------------------------------------
// Ranking
// ---------------------------------------------------------------------------

/// Order the applications for one MIME type.
///
/// P3 §3: the registered default first and tagged, then exact `MimeType` matches, then
/// the rest — which the picker hides behind "Show all applications".
pub fn rank(
    apps: &[DesktopApp],
    mime: &str,
    defaults: &BTreeMap<String, Vec<String>>,
) -> Vec<Candidate> {
    let default_ids = defaults.get(mime).cloned().unwrap_or_default();

    let mut out: Vec<Candidate> = apps
        .iter()
        .map(|app| Candidate {
            is_default: default_ids.iter().any(|id| id == &app.id),
            exact: app.mime_types.iter().any(|m| m == mime),
            app: app.clone(),
        })
        .collect();

    out.sort_by(|a, b| {
        b.is_default
            .cmp(&a.is_default)
            .then_with(|| b.exact.cmp(&a.exact))
            .then_with(|| a.app.name.to_lowercase().cmp(&b.app.name.to_lowercase()))
    });
    out
}

// ---------------------------------------------------------------------------
// The MIME table
// ---------------------------------------------------------------------------

/// Guess a MIME type from a file name.
///
/// P3 §3: "a built-in extension→type table of about forty common types,
/// `application/octet-stream` as the honest fallback. Parsing the shared-mime-info
/// database is a later temptation, recorded here as deliberately skipped."
pub fn mime_for(name: &str) -> &'static str {
    let lower = name.to_ascii_lowercase();

    // Compound extensions first, so `.tar.gz` does not answer as `.gz`.
    for (suffix, mime) in [
        (".tar.gz", "application/x-compressed-tar"),
        (".tar.bz2", "application/x-bzip2-compressed-tar"),
        (".tar.xz", "application/x-xz-compressed-tar"),
        (".tar.zst", "application/x-zstd-compressed-tar"),
        (".tar.lz", "application/x-lzip-compressed-tar"),
    ] {
        if lower.ends_with(suffix) {
            return mime;
        }
    }

    let ext = match lower.rsplit_once('.') {
        Some((_, e)) => e,
        None => return "application/octet-stream",
    };

    match ext {
        // text
        "txt" | "text" | "log" | "nfo" => "text/plain",
        "md" | "markdown" => "text/markdown",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "csv" => "text/csv",
        "xml" => "application/xml",
        "json" => "application/json",
        "toml" => "application/toml",
        "yaml" | "yml" => "application/yaml",
        "rs" => "text/rust",
        "c" | "h" => "text/x-csrc",
        "cpp" | "cc" | "hpp" => "text/x-c++src",
        "py" => "text/x-python",
        "sh" | "bash" => "application/x-shellscript",
        "desktop" => "application/x-desktop",
        // images
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "ico" => "image/vnd.microsoft.icon",
        "tif" | "tiff" => "image/tiff",
        // documents
        "pdf" => "application/pdf",
        "epub" => "application/epub+zip",
        "odt" => "application/vnd.oasis.opendocument.text",
        "ods" => "application/vnd.oasis.opendocument.spreadsheet",
        // audio and video
        "mp3" => "audio/mpeg",
        "flac" => "audio/flac",
        "ogg" => "audio/ogg",
        "wav" => "audio/x-wav",
        "opus" => "audio/opus",
        "mp4" => "video/mp4",
        "mkv" => "video/x-matroska",
        "webm" => "video/webm",
        // archives
        "zip" => "application/zip",
        "7z" => "application/x-7z-compressed",
        "tar" => "application/x-tar",
        "gz" => "application/gzip",
        "xz" => "application/x-xz",
        "zst" => "application/zstd",
        "bz2" => "application/x-bzip2",
        "lz" => "application/x-lzip",
        "cpio" => "application/x-cpio",
        "iso" => "application/x-iso9660-image",
        // The honest fallback.
        _ => "application/octet-stream",
    }
}

// ---------------------------------------------------------------------------
// The Exec tokenizer
// ---------------------------------------------------------------------------

/// Undo the general escape rules for a `string` value.
///
/// The spec is explicit that this runs **before** the quoting rule: "the general escape
/// rule for values of type string states that the backslash character can be escaped as
/// `\\\\` as well and that this escape rule is applied before the quoting rule. As
/// such, to unambiguously represent a literal backslash character in a quoted argument
/// in a desktop entry file requires the use of four successive backslash characters."
///
/// An unrecognised sequence keeps its backslash, so the quoting pass can still see
/// `\"` — which is exactly how a wine entry's `\\"` reaches the second stage as `\"`.
pub fn unescape_value(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('s') => out.push(' '),
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// Split an `Exec` value into words, honouring the spec's quoting and escaping.
///
/// Two passes, in the order the spec mandates: `unescape_value` first, then quoting.
/// A single pass gets four-backslash wine paths wrong — it yields two backslashes where
/// the file means one.
///
/// Inside a double-quoted argument only `"`, `` ` ``, `$` and `\` are escapable; any
/// other backslash is literal, which is what shells do and what GLib's
/// `g_shell_parse_argv` produces. Outside quotes a backslash escapes whatever follows.
/// `%%` is left alone for `expand_exec` to resolve, so a filename containing `%` cannot
/// be mistaken for a field code.
pub fn tokenize_exec(exec: &str) -> Vec<String> {
    let unescaped = unescape_value(exec);

    let mut out = Vec::new();
    let mut cur = String::new();
    let mut started = false;
    let mut in_quotes = false;
    let mut chars = unescaped.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '"' => {
                in_quotes = !in_quotes;
                started = true;
            }
            '\\' => {
                started = true;
                match chars.next() {
                    Some(n) if !in_quotes => cur.push(n),
                    Some(n @ ('"' | '`' | '$' | '\\')) => cur.push(n),
                    // Inside quotes, a backslash before anything else is literal.
                    Some(n) => {
                        cur.push('\\');
                        cur.push(n);
                    }
                    None => cur.push('\\'),
                }
            }
            c if c.is_whitespace() && !in_quotes => {
                if started {
                    out.push(std::mem::take(&mut cur));
                    started = false;
                }
            }
            c => {
                cur.push(c);
                started = true;
            }
        }
    }
    if started {
        out.push(cur);
    }
    out
}

/// Substitute the field codes and drop the ones INDIUM does not supply.
///
/// P3 §3: "`%f`/`%F` become the scratch path, `%u`/`%U` its `file://` URI,
/// `%i`/`%c`/`%k` are stripped". The deprecated codes (`%d %D %n %N %v %m`) are
/// stripped too, as the spec directs.
pub fn expand_exec(tokens: &[String], path: &Path, uri: &str) -> Vec<String> {
    let path_str = path.to_string_lossy().to_string();
    let mut out = Vec::with_capacity(tokens.len());

    for token in tokens {
        // A token that is exactly a dropped code disappears entirely, rather than
        // becoming an empty argument the program would have to cope with.
        if matches!(
            token.as_str(),
            "%i" | "%c" | "%k" | "%d" | "%D" | "%n" | "%N" | "%v" | "%m"
        ) {
            continue;
        }

        let mut result = String::with_capacity(token.len());
        let mut chars = token.chars().peekable();
        while let Some(c) = chars.next() {
            if c != '%' {
                result.push(c);
                continue;
            }
            match chars.next() {
                Some('%') => result.push('%'),
                Some('f') | Some('F') => result.push_str(&path_str),
                Some('u') | Some('U') => result.push_str(uri),
                // Embedded dropped codes contribute nothing.
                Some('i') | Some('c') | Some('k') | Some('d') | Some('D') | Some('n')
                | Some('N') | Some('v') | Some('m') => {}
                Some(other) => {
                    result.push('%');
                    result.push(other);
                }
                None => result.push('%'),
            }
        }
        out.push(result);
    }
    out
}

// ---------------------------------------------------------------------------
// Launching
// ---------------------------------------------------------------------------

/// Launch an application on an extracted copy.
///
/// P3 §3: "Spawn detached in its own process group (`CommandExt::process_group(0)`):
/// closing INDIUM must never take the viewer down with it."
pub fn launch(app: &DesktopApp, path: &Path) -> Result<(), String> {
    use std::os::unix::process::CommandExt;

    let uri = super::clipboard::path_to_uri(path);
    let tokens = expand_exec(&tokenize_exec(&app.exec), path, &uri);

    let (program, args) = tokens
        .split_first()
        .ok_or_else(|| format!("{} has an empty Exec line", app.name))?;

    let mut cmd = std::process::Command::new(program);
    cmd.args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    cmd.process_group(0);

    cmd.spawn()
        .map(|_| ())
        .map_err(|e| format!("Could not launch {}: {e}", app.name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/desktop")
            .join(name)
    }

    fn parse_fixture(name: &str) -> Option<DesktopApp> {
        let path = fixture(name);
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("missing fixture {}: {e}", path.display()));
        parse_desktop(&path, &text)
    }

    // --- tokenizer ---------------------------------------------------------

    #[test]
    fn plain_words_split_on_whitespace() {
        assert_eq!(tokenize_exec("eog %f"), vec!["eog", "%f"]);
        assert_eq!(tokenize_exec("  spaced   out  "), vec!["spaced", "out"]);
    }

    #[test]
    fn quoted_arguments_keep_their_spaces() {
        assert_eq!(
            tokenize_exec(r#"app "one two" three"#),
            vec!["app", "one two", "three"]
        );
    }

    #[test]
    fn an_escaped_quote_survives_inside_a_quoted_argument() {
        assert_eq!(tokenize_exec(r#"app "a \" b""#), vec!["app", r#"a " b"#]);
    }

    #[test]
    fn an_empty_quoted_argument_is_preserved() {
        assert_eq!(tokenize_exec(r#"app "" x"#), vec!["app", "", "x"]);
    }

    /// The spec's own worked example: four backslashes in the file are **one** literal
    /// backslash after both the value-level and the quoting-level rule.
    #[test]
    fn four_backslashes_are_one() {
        assert_eq!(tokenize_exec(r#"app "a\\\\b""#), vec!["app", r"a\b"]);
    }

    #[test]
    fn a_backslash_before_an_ordinary_character_stays_literal_inside_quotes() {
        // Stage 1 turns `\\` into `\`; inside quotes `\b` is not an escapable pair, so
        // the backslash survives — same as a shell, same as g_shell_parse_argv.
        assert_eq!(tokenize_exec(r#"app "a\\b""#), vec!["app", r"a\b"]);
    }

    #[test]
    fn value_unescaping_runs_first() {
        assert_eq!(unescape_value(r"a\\b"), r"a\b");
        assert_eq!(unescape_value(r"a\sb"), "a b");
        assert_eq!(unescape_value(r"a\nb"), "a\nb");
        // An unrecognised sequence keeps its backslash for the quoting pass.
        assert_eq!(unescape_value(r#"a\"b"#), r#"a\"b"#);
    }

    /// End-to-end against the committed wine-style fixture, which P3 §3 asks for by
    /// name: "a pathological real Exec line from a wine `.desktop`". The expected argv
    /// was cross-checked against GLib's own `g_shell_parse_argv`.
    #[test]
    fn the_wine_fixture_tokenizes_and_expands_exactly() {
        let app = parse_fixture("quoting.desktop").expect("quoting.desktop must be kept");
        let tokens = tokenize_exec(&app.exec);
        let argv = expand_exec(
            &tokens,
            Path::new("/tmp/scratch/file.png"),
            "file:///tmp/scratch/file.png",
        );
        assert_eq!(
            argv,
            vec![
                "env",
                "WINEPREFIX=/home/megas/.wine",
                "wine",
                r#"C:\Program Files\Acme "Deluxe" Viewer\view.exe"#,
                "/tmp/scratch/file.png",
            ],
            "wine Exec line did not tokenize as the spec requires"
        );
    }

    // --- field codes -------------------------------------------------------

    #[test]
    fn f_becomes_the_path_and_u_becomes_the_uri() {
        let toks = tokenize_exec("viewer %f");
        let out = expand_exec(
            &toks,
            Path::new("/tmp/ow-1/a.png"),
            "file:///tmp/ow-1/a.png",
        );
        assert_eq!(out, vec!["viewer", "/tmp/ow-1/a.png"]);

        let toks = tokenize_exec("browser %u");
        let out = expand_exec(
            &toks,
            Path::new("/tmp/ow-1/a.png"),
            "file:///tmp/ow-1/a.png",
        );
        assert_eq!(out, vec!["browser", "file:///tmp/ow-1/a.png"]);
    }

    #[test]
    fn plural_codes_take_the_single_file_we_have() {
        let toks = tokenize_exec("viewer %F");
        let out = expand_exec(&toks, Path::new("/tmp/a.png"), "file:///tmp/a.png");
        assert_eq!(out, vec!["viewer", "/tmp/a.png"]);
    }

    #[test]
    fn stripped_codes_leave_no_empty_arguments() {
        let toks = tokenize_exec("app %i %c %k %f");
        let out = expand_exec(&toks, Path::new("/tmp/a.png"), "file:///tmp/a.png");
        assert_eq!(out, vec!["app", "/tmp/a.png"], "no empty argv entries");
    }

    #[test]
    fn deprecated_codes_are_stripped_too() {
        let toks = tokenize_exec("app %d %D %n %N %v %m %f");
        let out = expand_exec(&toks, Path::new("/tmp/a.png"), "file:///tmp/a.png");
        assert_eq!(out, vec!["app", "/tmp/a.png"]);
    }

    #[test]
    fn a_double_percent_is_a_literal_percent() {
        let toks = tokenize_exec("app 100%% %f");
        let out = expand_exec(&toks, Path::new("/tmp/a.png"), "file:///tmp/a.png");
        assert_eq!(out, vec!["app", "100%", "/tmp/a.png"]);
    }

    /// A path containing `%` must not be re-read as a field code.
    #[test]
    fn a_percent_in_the_filename_is_not_a_field_code() {
        let toks = tokenize_exec("app %f");
        let out = expand_exec(&toks, Path::new("/tmp/50%f.png"), "file:///tmp/50%25f.png");
        assert_eq!(out, vec!["app", "/tmp/50%f.png"]);
    }

    // --- MIME table --------------------------------------------------------

    #[test]
    fn compound_archive_extensions_win_over_the_last_one() {
        assert_eq!(mime_for("x.tar.zst"), "application/x-zstd-compressed-tar");
        assert_eq!(mime_for("x.tar.gz"), "application/x-compressed-tar");
        assert_eq!(mime_for("x.zst"), "application/zstd");
        assert_eq!(mime_for("x.tar"), "application/x-tar");
    }

    #[test]
    fn common_types_resolve() {
        assert_eq!(mime_for("a.png"), "image/png");
        assert_eq!(mime_for("a.JPG"), "image/jpeg");
        assert_eq!(mime_for("a.txt"), "text/plain");
        assert_eq!(mime_for("a.pdf"), "application/pdf");
    }

    #[test]
    fn an_unknown_extension_is_octet_stream() {
        assert_eq!(mime_for("a.qqq"), "application/octet-stream");
        assert_eq!(mime_for("noextension"), "application/octet-stream");
        assert_eq!(mime_for(""), "application/octet-stream");
    }

    // --- .desktop parsing, against committed fixtures ----------------------

    #[test]
    fn a_normal_entry_is_kept() {
        let app = parse_fixture("normal.desktop").expect("normal.desktop must be kept");
        assert!(!app.name.is_empty());
        assert!(!app.exec.is_empty());
        assert!(app.mime_types.iter().any(|m| m == "image/png"));
    }

    #[test]
    fn nodisplay_and_hidden_are_skipped() {
        assert!(
            parse_fixture("nodisplay.desktop").is_none(),
            "NoDisplay=true"
        );
        assert!(parse_fixture("hidden.desktop").is_none(), "Hidden=true");
    }

    #[test]
    fn a_missing_tryexec_disqualifies() {
        assert!(parse_fixture("tryexec-missing.desktop").is_none());
    }

    #[test]
    fn a_present_tryexec_is_kept() {
        assert!(parse_fixture("tryexec-present.desktop").is_some());
    }

    /// P3 §3: "`Terminal=true` entries are listed".
    #[test]
    fn terminal_entries_are_listed() {
        let app = parse_fixture("terminal.desktop").expect("Terminal=true is still listed");
        assert!(app.terminal);
    }

    #[test]
    fn program_lookup_finds_path_entries_and_rejects_nonsense() {
        assert!(program_exists("sh"), "sh must be on PATH");
        assert!(!program_exists("/nonexistent/binary/definitely-not-here"));
        assert!(!program_exists(""));
    }

    // --- ranking -----------------------------------------------------------

    #[test]
    fn the_registered_default_comes_first_and_is_tagged() {
        let text = std::fs::read_to_string(fixture("mimeapps.list")).expect("fixture");
        let defaults = parse_mimeapps(&text);

        let apps = vec![
            DesktopApp {
                id: "zzz-other.desktop".into(),
                name: "Zzz Other".into(),
                exec: "other %f".into(),
                mime_types: vec!["image/png".into()],
                terminal: false,
                path: PathBuf::from("/x/zzz-other.desktop"),
            },
            DesktopApp {
                id: "normal.desktop".into(),
                name: "Normal".into(),
                exec: "normal %f".into(),
                mime_types: vec!["image/png".into()],
                terminal: false,
                path: fixture("normal.desktop"),
            },
        ];

        let ranked = rank(&apps, "image/png", &defaults);
        assert_eq!(ranked[0].app.id, "normal.desktop");
        assert!(ranked[0].is_default, "the default must be tagged");
        assert!(!ranked[1].is_default);
    }

    #[test]
    fn exact_matches_outrank_everything_else() {
        let defaults = BTreeMap::new();
        let apps = vec![
            DesktopApp {
                id: "a.desktop".into(),
                name: "Aaa Generic".into(),
                exec: "a %f".into(),
                mime_types: vec![],
                terminal: false,
                path: PathBuf::from("/x/a.desktop"),
            },
            DesktopApp {
                id: "b.desktop".into(),
                name: "Bbb Exact".into(),
                exec: "b %f".into(),
                mime_types: vec!["image/png".into()],
                terminal: false,
                path: PathBuf::from("/x/b.desktop"),
            },
        ];
        let ranked = rank(&apps, "image/png", &defaults);
        assert_eq!(ranked[0].app.id, "b.desktop", "exact match must come first");
        assert!(ranked[0].exact);
        assert!(!ranked[1].exact);
    }

    #[test]
    fn mimeapps_parsing_ignores_other_sections() {
        let text = "[Added Associations]\nimage/png=wrong.desktop;\n\n\
                    [Default Applications]\nimage/png=right.desktop;\n";
        let m = parse_mimeapps(text);
        assert_eq!(
            m.get("image/png").unwrap(),
            &vec!["right.desktop".to_string()]
        );
    }

    #[test]
    fn application_dirs_are_in_precedence_order() {
        let dirs = application_dirs();
        assert!(!dirs.is_empty());
        assert!(
            dirs[0].ends_with("applications"),
            "the user's own directory comes first"
        );
    }
}
