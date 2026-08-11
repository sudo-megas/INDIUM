//! The terminal half — P17.
//!
//! CORE §7 named three headless subcommands, and `main.rs` has printed a promise about
//! them in its own `--help` since P6: *"Headless subcommands (extract, list, single-file
//! open) arrive in V1.3."* A promise printed by the program is a debt, which is the
//! reasoning P16 used to build the hex view. This pays the last one.
//!
//! **This module opens no window and touches nothing in `ui`.** That is not incidental:
//! `run` returns before `main` ever builds `NativeOptions`, so `indium list` on a machine
//! with no compositor is an ordinary program reading a file. Linking `pub mod ui` is not
//! initialising it, but a single `use crate::ui::SOMETHING` here would be the way that
//! stops being true by accident — most plausibly by somebody reaching for the Preview's
//! byte cap to size `cat`'s buffer. `cat` has no cap; see [`crate::arch::stream_entry`].
//!
//! **`run` returns a code and writes through `dyn Write`** rather than calling
//! `process::exit` and `println!`. That is what lets `tests/cli_path.rs` drive every
//! subcommand inside the test process and read back both streams and the code —
//! `src/lib.rs` already gives the reason: the library half exists so the tests can drive
//! the program without going through the window.
//!
//! Copyright © sudo-megas. GPL-3.0-only.

use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use crate::arch::{self, ArchiveError, Entry};
use crate::model;
use crate::secret::Secret;
use crate::util;

/// The words that put INDIUM in the terminal instead of the window.
///
/// Named once, so [`USAGE`] can be checked against them by a test rather than kept in
/// step by hand — the same rule CORE §4 puts on the Keys popup, which is "generated from
/// the bindings, never typed twice".
pub const SUBCOMMANDS: [&str; 3] = ["list", "extract", "cat"];

/// Exit codes. The window half already used 0, 1 and 2 this way; the terminal half does
/// not invent a fourth meaning.
const OK: i32 = 0;
const FAILED: i32 = 1;
const MISUSE: i32 = 2;

pub const USAGE: &str = "\
INDIUM — an archive manager for Linux on Wayland.

    indium [ARCHIVE]...            open a window on each archive named

    indium list    ARCHIVE [--long] [-0]
    indium extract ARCHIVE [--to DIR] [--] [MEMBER]...
    indium cat     ARCHIVE MEMBER

    -h, --help       this text
    -V, --version    the version

The terminal half is entered only when the first argument is exactly `list`,
`extract` or `cat`. An archive of that name is opened as `indium ./list`.

    list       one stored path per line, in archive order, undecorated, so the
               output feeds straight back into `cat` and `extract`
      --long   mode, size, packed, method, time and a total — for a person to
               read, not for a script to parse
      -0       separate with NUL instead of newline. A member name may contain
               a newline, and a line-oriented listing of one is silently wrong

    extract    everything, or only the MEMBERs named
      --to     where to put it; the working directory by default
      --       end of flags, for a member named like one

    cat        one member's bytes to stdout, whole, however large

An encrypted archive is asked for its password on the terminal, once, per use.
There is no --password and no environment variable: either would put the secret
in the process table and the shell's history, and CORE §9 says passwords are
never stored or remembered.
";

/// Is this a terminal invocation, or a path to open in a window?
///
/// Byte-exact, and deliberately not a prefix or substring test: `indium listing.zip` and
/// `indium ./list` are both archives, and only `indium list` is the subcommand. An archive
/// genuinely named `list` in the working directory is reachable as `./list`, which `USAGE`
/// says out loud because it is the one ambiguity in the surface.
///
/// Pure, so the rule is testable without a filesystem.
pub fn takes_the_terminal(args: &[OsString]) -> bool {
    match args.first() {
        Some(first) => SUBCOMMANDS
            .iter()
            .any(|s| first.as_os_str() == OsStr::new(s)),
        None => false,
    }
}

/// Run a terminal subcommand. Returns the process's exit code.
pub fn run(args: &[OsString], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let Some(word) = args.first().and_then(|a| a.to_str()) else {
        return usage_error(err, "no subcommand");
    };
    let rest = &args[1..];

    let code = match word {
        "list" => list(rest, out, err),
        "extract" => extract(rest, out, err),
        "cat" => cat(rest, out, err),
        // Unreachable through `main`, which asks `takes_the_terminal` first, but `run` is
        // public and a test may call it directly. Answering honestly beats a panic.
        other => usage_error(err, &format!("{other} is not a subcommand")),
    };

    // **The flush is checked, and it changes the code.** `BufWriter`'s `Drop` swallows the
    // error, and `process::exit` runs no destructors at all — so without this a `cat` onto
    // a full disk exits 0 having written a truncated file, which is the same class of lie
    // as a silent cap. `BrokenPipe` is judged first: see `broken_pipe`.
    match out.flush() {
        Ok(()) => code,
        Err(e) if broken_pipe(&e) => OK,
        Err(e) => {
            let _ = writeln!(err, "indium: {e}");
            FAILED
        }
    }
}

/// `indium cat big.zip x | head` is correct behaviour, not an error.
///
/// Rust sets `SIGPIPE` to `SIG_IGN` before `main`, so the write fails rather than the
/// process dying the way a C program's would. Reporting that would tell the user their
/// working pipeline had failed.
fn broken_pipe(e: &std::io::Error) -> bool {
    e.kind() == std::io::ErrorKind::BrokenPipe
}

fn usage_error(err: &mut dyn Write, why: &str) -> i32 {
    let _ = writeln!(err, "indium: {why}\n");
    let _ = write!(err, "{USAGE}");
    MISUSE
}

/// Report a failure in the program's own voice. One sentence, on stderr, and never on
/// stdout — stdout is `cat`'s data channel and `list`'s machine-readable output.
fn failure(err: &mut dyn Write, e: &ArchiveError) -> i32 {
    let _ = writeln!(err, "indium: {e}");
    FAILED
}

// ---------------------------------------------------------------------------
// Argument shapes
//
// CORE §2, as ruled in P17: no `clap`. Three subcommands, one string option and two
// flags is about forty lines of `match`, next to the thirty `main.rs` already hand-rolls,
// and §2's standard is that a dependency must do genuine work for the program.
// ---------------------------------------------------------------------------

/// A path argument, kept as bytes. `PathBuf::from(OsString)` never fails and never
/// lossily decodes, which is the whole reason `main` reads `args_os`.
fn as_path(arg: &OsString) -> PathBuf {
    PathBuf::from(arg)
}

// ---------------------------------------------------------------------------
// list
// ---------------------------------------------------------------------------

fn list(args: &[OsString], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let mut archive: Option<PathBuf> = None;
    let mut long = false;
    let mut nul = false;

    for arg in args {
        match arg.to_str() {
            Some("--long") => long = true,
            Some("-0") => nul = true,
            Some(text) if text.starts_with('-') => {
                return usage_error(err, &format!("list: unknown option {text}"));
            }
            _ => {
                if archive.is_some() {
                    return usage_error(err, "list: one archive at a time");
                }
                archive = Some(as_path(arg));
            }
        }
    }

    let Some(path) = archive else {
        return usage_error(err, "list: no archive named");
    };

    let entries = match read_listing(&path, err) {
        Ok(entries) => entries,
        Err(code) => return code,
    };

    if long {
        return list_long(&entries, out);
    }

    // Undecorated, in archive order, one stored path per record. No trailing slash on a
    // directory: this output is an input to `cat` and `extract`, and a decorated name is
    // not a name. `--long` is where a person's conveniences live.
    let sep = if nul { b'\0' } else { b'\n' };
    for entry in &entries {
        if out.write_all(entry.path.as_bytes()).is_err() || out.write_all(&[sep]).is_err() {
            // A closed pipe is judged by `run`'s flush; anything else is reported there
            // too. Stopping here simply avoids writing into a stream that is gone.
            break;
        }
    }
    OK
}

/// The long listing, in the window's own words.
///
/// Every column comes from a function the Inspector or the entry table already uses —
/// `util::format_mode`, `Entry::packed`, `Entry::method`, `util::format_timestamp` — so
/// the two halves of the program cannot come to describe the same archive differently.
///
/// **Sizes are exact integers, and that is not a second vocabulary.** `util::format_bytes`
/// exists because the window's columns are narrow; a terminal's are not, and
/// `format_exact_bytes`'s space separators would be actively hostile to `awk`. The
/// declaration in `USAGE` that `--long` is for a person is what makes the aligned columns
/// honest rather than a trap.
fn list_long(entries: &[Entry], out: &mut dyn Write) -> i32 {
    for e in entries {
        let packed = match e.packed {
            Some(n) => n.to_string(),
            // The Inspector's own honesty: libarchive exposes no per-entry compressed
            // size, and a dash is not a guess.
            None => "-".to_string(),
        };
        let size = if e.is_dir {
            "-".to_string()
        } else {
            e.size.to_string()
        };
        let time = match e.mtime {
            Some(t) => util::format_timestamp(t),
            None => "-".to_string(),
        };
        // `enc` gets a lane of its own rather than a sigil beside the mode, because the
        // Inspector states encryption as a fact and a sigil would need a legend.
        let enc = if e.encrypted { "enc" } else { "   " };
        if writeln!(
            out,
            "{:<11} {:>12} {:>12}  {:<8} {enc} {:<23} {}",
            util::format_mode(e.mode, e.filetype),
            size,
            packed,
            e.method,
            time,
            e.path
        )
        .is_err()
        {
            break;
        }
    }

    // The footer is the status bar's second row, in a terminal. Same function, same
    // numbers, so the two cannot drift.
    let agg = model::aggregate(entries);
    let packed = match agg.total_packed {
        Some(p) => format!(
            "{} → {} ({})",
            util::format_bytes(agg.total_real),
            util::format_bytes(p),
            util::format_ratio(agg.total_real, p)
        ),
        None => util::format_bytes(agg.total_real),
    };
    let _ = writeln!(
        out,
        "\n{} {}, {} {}, {} {}, {packed}",
        agg.count,
        plural(agg.count, "entry", "entries"),
        agg.files,
        plural(agg.files, "file", "files"),
        agg.dirs,
        plural(agg.dirs, "directory", "directories")
    );
    OK
}

// ---------------------------------------------------------------------------
// extract
// ---------------------------------------------------------------------------

fn extract(args: &[OsString], _out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let mut archive: Option<PathBuf> = None;
    let mut dest: Option<PathBuf> = None;
    let mut members: Vec<String> = Vec::new();
    let mut flags_done = false;
    let mut expecting_dest = false;

    for arg in args {
        if expecting_dest {
            dest = Some(as_path(arg));
            expecting_dest = false;
            continue;
        }
        let text = arg.to_str();
        if !flags_done {
            match text {
                Some("--") => {
                    flags_done = true;
                    continue;
                }
                Some("--to") => {
                    expecting_dest = true;
                    continue;
                }
                Some(t) if t.starts_with("--to=") => {
                    dest = Some(PathBuf::from(&t["--to=".len()..]));
                    continue;
                }
                Some(t) if t.starts_with('-') => {
                    return usage_error(err, &format!("extract: unknown option {t}"));
                }
                _ => {}
            }
        }
        if archive.is_none() {
            archive = Some(as_path(arg));
        } else {
            // A member name is an archive-internal path and therefore a Rust `String` —
            // `Entry::raw_path` already is one. A name that is not UTF-8 cannot match
            // anything in a listing, so refusing it here is honest rather than limiting.
            match text {
                Some(t) => members.push(util::normalize_archive_path(t)),
                None => return usage_error(err, "extract: a member name must be text"),
            }
        }
    }

    if expecting_dest {
        return usage_error(err, "extract: --to needs a directory");
    }
    let Some(path) = archive else {
        return usage_error(err, "extract: no archive named");
    };
    let dest = dest.unwrap_or_else(|| PathBuf::from("."));

    if let Err(e) = std::fs::create_dir_all(&dest) {
        let _ = writeln!(err, "indium: {}: {e}", dest.display());
        return FAILED;
    }

    let entries = match read_listing(&path, err) {
        Ok(entries) => entries,
        Err(code) => return code,
    };

    // **The trap this is written around.** `arch::selection_matches` returns false for an
    // empty set — verified, not assumed — so handing `extract` an empty `wanted` writes
    // nothing at all and returns `Ok(0)`, which reads as a clean success. "No member named"
    // means "everything", so everything has to be named. `arch::extract`'s semantics are
    // not touched: the window depends on them.
    let wanted: HashSet<String> = if members.is_empty() {
        entries.iter().map(|e| e.path.clone()).collect()
    } else {
        members.into_iter().collect()
    };

    let cancel: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
    match with_password(&path, err, |secret| {
        arch::extract(&path, &wanted, &dest, secret, None, &cancel)
    }) {
        Ok(n) => {
            // On stderr, so `extract`'s stdout stays empty and the subcommand composes.
            let _ = writeln!(err, "Extracted {n} {}.", plural(n, "entry", "entries"));
            OK
        }
        Err(e) => failure(err, &e),
    }
}

// ---------------------------------------------------------------------------
// cat
// ---------------------------------------------------------------------------

fn cat(args: &[OsString], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let mut positional: Vec<&OsString> = Vec::new();
    for arg in args {
        match arg.to_str() {
            Some(t) if t.starts_with('-') && t != "-" => {
                return usage_error(err, &format!("cat: unknown option {t}"));
            }
            _ => positional.push(arg),
        }
    }
    if positional.len() != 2 {
        return usage_error(err, "cat: an archive and one member");
    }
    let path = as_path(positional[0]);
    let Some(member) = positional[1].to_str() else {
        return usage_error(err, "cat: a member name must be text");
    };
    let member = util::normalize_archive_path(member);

    // No `isatty` guard. `cat(1)` writes binary to a terminal and this is called `cat`;
    // the name is the promise. Recorded as a deviation rather than fixed, because the
    // terminal check exists in this module for the password prompt and somebody would
    // otherwise propose reusing it here as a discovery.
    match with_password(&path, err, |secret| {
        arch::stream_entry(&path, &member, secret, out)
    }) {
        Ok(_) => OK,
        Err(e) => failure(err, &e),
    }
}

// ---------------------------------------------------------------------------
// Shared
// ---------------------------------------------------------------------------

/// List an archive, reporting a failure in the program's voice, with the password prompt
/// wired in for an archive whose headers are encrypted.
fn read_listing(path: &Path, err: &mut dyn Write) -> Result<Vec<Entry>, i32> {
    match with_password(path, err, |secret| arch::list_all(path, secret)) {
        Ok(entries) => Ok(entries),
        Err(e) => Err(failure(err, &e)),
    }
}

/// Run an operation, and if it turns out to need a password, ask for one and run it once
/// more.
///
/// **Asked at the moment of use, and only then** — CORE §3's words. The archive is not
/// sniffed for encryption in advance: the reader already reports `NeedPassword` and
/// `EncryptedHeaders` when it reaches something it cannot open, and that is exactly the
/// moment the password is wanted. One retry, because a second wrong password is a wrong
/// password and looping is how a script hangs.
fn with_password<T>(
    path: &Path,
    err: &mut dyn Write,
    mut op: impl FnMut(Option<&Secret>) -> Result<T, ArchiveError>,
) -> Result<T, ArchiveError> {
    match op(None) {
        // **All three, and `WrongPassword` is not a mistake in that list.** Which one comes
        // back depends on where the refusal happened, not on what the user did: `extract`
        // decides in its pre-flight that a selected entry is encrypted and says
        // `NeedPassword`, while `stream_entry` only finds out when libarchive is asked for
        // the bytes, and that is reported as a passphrase failure — so a `cat` of an
        // encrypted member with no password at all comes back `WrongPassword`. The window
        // has always treated the pair together; the terminal half does the same rather than
        // inventing a second rule.
        Err(ArchiveError::NeedPassword)
        | Err(ArchiveError::WrongPassword)
        | Err(ArchiveError::EncryptedHeaders) => {
            let secret = ask_for_password(path, err)?;
            op(Some(&secret))
        }
        other => other,
    }
}

/// `1 entry`, `2 entries` — the window's own words, at `ui/mod.rs`'s
/// `if written == 1 { "entry" } else { "entries" }`.
///
/// Small, and it is here because the alternative was the terminal half saying
/// "Extracted 1 entries." while the window said "Extracted 1 entry." about the same
/// operation. Two vocabularies for one program is the thing this module is written to
/// avoid, and it starts exactly this way.
fn plural(n: usize, one: &'static str, many: &'static str) -> &'static str {
    if n == 1 {
        one
    } else {
        many
    }
}

/// Ask the terminal for a password. Replaced with a real prompt in the commit that
/// carries the termios FFI; until then an encrypted archive is refused rather than
/// silently mis-read.
fn ask_for_password(_path: &Path, _err: &mut dyn Write) -> Result<Secret, ArchiveError> {
    Err(ArchiveError::Other(
        "this archive is encrypted, and the terminal half cannot yet ask for a password"
            .to_string(),
    ))
}
