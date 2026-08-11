//! Hand-written FFI over the system libarchive, and the safe wrapper around it.
//!
//! CORE §2: libarchive "reads and writes every supported container and filter
//! in-process". CORE §3: listing streams entries over a channel from a worker thread;
//! extraction runs with libarchive's secure flags so a hostile archive cannot write
//! outside its target. P1 §2 fixes the exact symbol list.
//!
//! No `-sys` crate and no bindgen: the declarations below are exactly what INDIUM
//! uses, checked against `/usr/include/archive.h` and `archive_entry.h`.

use std::collections::HashSet;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_long, c_void};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;

use crate::secret::Secret;
use crate::tasks::{Container, Meta, Method, Recipe, Sink};
use crate::util::{self, Crc32};

// ---------------------------------------------------------------------------
// Opaque C types
// ---------------------------------------------------------------------------

#[repr(C)]
pub struct Archive {
    _private: [u8; 0],
}

#[repr(C)]
pub struct ArchiveEntry {
    _private: [u8; 0],
}

// ---------------------------------------------------------------------------
// Constants, transcribed from the installed headers
// ---------------------------------------------------------------------------

const ARCHIVE_EOF: c_int = 1;
const ARCHIVE_OK: c_int = 0;
const ARCHIVE_WARN: c_int = -20;

const ARCHIVE_EXTRACT_PERM: c_int = 0x0002;
const ARCHIVE_EXTRACT_TIME: c_int = 0x0004;
const ARCHIVE_EXTRACT_SECURE_SYMLINKS: c_int = 0x0100;
const ARCHIVE_EXTRACT_SECURE_NODOTDOT: c_int = 0x0200;

/// P1 §2: "The secure flags are not optional — they are what stops a hostile archive
/// writing outside `dest`."
const EXTRACT_FLAGS: c_int = ARCHIVE_EXTRACT_TIME
    | ARCHIVE_EXTRACT_PERM
    | ARCHIVE_EXTRACT_SECURE_SYMLINKS
    | ARCHIVE_EXTRACT_SECURE_NODOTDOT;

const ARCHIVE_FORMAT_BASE_MASK: c_int = 0x00ff_0000;
const ARCHIVE_FORMAT_RAR: c_int = 0x000D_0000;
const ARCHIVE_FORMAT_RAR_V5: c_int = 0x0010_0000;

const AE_IFMT: u32 = 0o170000;
const AE_IFDIR: u32 = 0o040000;
const AE_IFREG: u32 = 0o100000;
const AE_IFLNK: u32 = 0o120000;

/// P4 §3: a refused option is `ARCHIVE_FAILED`, not a hard failure — which is exactly
/// why `set_options` must be checked. A mistyped level would otherwise be swallowed and
/// the archive built at the default, with nobody told.
const ARCHIVE_FAILED: c_int = -25;

/// The exact sentence CORE §5 requires. Nothing else may be shown for a RAR file.
pub const RAR_REFUSAL: &str = "RAR is not supported.";

// ---------------------------------------------------------------------------
// The FFI surface — exactly what INDIUM uses, nothing more
// ---------------------------------------------------------------------------

#[link(name = "archive")]
extern "C" {
    fn archive_read_new() -> *mut Archive;
    fn archive_read_support_filter_all(a: *mut Archive) -> c_int;
    fn archive_read_support_format_all(a: *mut Archive) -> c_int;
    fn archive_read_open_filename(a: *mut Archive, file: *const c_char, block: usize) -> c_int;
    fn archive_read_next_header(a: *mut Archive, e: *mut *mut ArchiveEntry) -> c_int;
    fn archive_read_data_block(
        a: *mut Archive,
        buff: *mut *const c_void,
        size: *mut usize,
        offset: *mut i64,
    ) -> c_int;
    fn archive_read_data_skip(a: *mut Archive) -> c_int;
    fn archive_read_extract(a: *mut Archive, e: *mut ArchiveEntry, flags: c_int) -> c_int;
    fn archive_read_close(a: *mut Archive) -> c_int;
    fn archive_read_free(a: *mut Archive) -> c_int;
    fn archive_read_add_passphrase(a: *mut Archive, pass: *const c_char) -> c_int;
    fn archive_read_has_encrypted_entries(a: *mut Archive) -> c_int;

    fn archive_format(a: *mut Archive) -> c_int;
    fn archive_format_name(a: *mut Archive) -> *const c_char;
    fn archive_filter_name(a: *mut Archive, n: c_int) -> *const c_char;
    fn archive_error_string(a: *mut Archive) -> *const c_char;

    fn archive_entry_pathname(e: *mut ArchiveEntry) -> *const c_char;
    fn archive_entry_set_pathname(e: *mut ArchiveEntry, p: *const c_char);
    // The `_utf8` half of the name accessors. See `entry_pathname` for why every name
    // INDIUM reads or writes goes through these and not through the plain ones.
    fn archive_entry_pathname_utf8(e: *mut ArchiveEntry) -> *const c_char;
    fn archive_entry_set_pathname_utf8(e: *mut ArchiveEntry, p: *const c_char);
    fn archive_entry_size(e: *mut ArchiveEntry) -> i64;
    fn archive_entry_mtime(e: *mut ArchiveEntry) -> i64;
    fn archive_entry_mtime_is_set(e: *mut ArchiveEntry) -> c_int;
    fn archive_entry_atime(e: *mut ArchiveEntry) -> i64;
    fn archive_entry_atime_is_set(e: *mut ArchiveEntry) -> c_int;
    fn archive_entry_ctime(e: *mut ArchiveEntry) -> i64;
    fn archive_entry_ctime_is_set(e: *mut ArchiveEntry) -> c_int;
    fn archive_entry_birthtime(e: *mut ArchiveEntry) -> i64;
    fn archive_entry_birthtime_is_set(e: *mut ArchiveEntry) -> c_int;
    fn archive_entry_uid(e: *mut ArchiveEntry) -> i64;
    fn archive_entry_gid(e: *mut ArchiveEntry) -> i64;
    fn archive_entry_uname(e: *mut ArchiveEntry) -> *const c_char;
    fn archive_entry_gname(e: *mut ArchiveEntry) -> *const c_char;
    fn archive_entry_mode(e: *mut ArchiveEntry) -> u32;
    fn archive_entry_filetype(e: *mut ArchiveEntry) -> u32;
    fn archive_entry_symlink(e: *mut ArchiveEntry) -> *const c_char;
    fn archive_entry_hardlink(e: *mut ArchiveEntry) -> *const c_char;
    fn archive_entry_symlink_utf8(e: *mut ArchiveEntry) -> *const c_char;
    fn archive_entry_hardlink_utf8(e: *mut ArchiveEntry) -> *const c_char;
    fn archive_entry_is_encrypted(e: *mut ArchiveEntry) -> c_int;
}

// Not libarchive's, but the reason libarchive can read a name at all. See
// `ensure_ctype_locale`.
unsafe extern "C" {
    fn setlocale(category: c_int, locale: *const c_char) -> *mut c_char;
    fn nl_langinfo(item: c_int) -> *mut c_char;
}
/// glibc's `__LC_CTYPE`, checked against `/usr/include/bits/locale.h` by hand, as every
/// other constant in this file was.
const LC_CTYPE: c_int = 0;
/// glibc's `CODESET` (`_NL_CTYPE_CODESET_NAME`), checked against `/usr/include/langinfo.h`
/// the same way — and against a two-line C program that printed it, because an enum's
/// position is not something to count by eye.
const CODESET: c_int = 14;

// The write half, added by P4 §3. Same rule as above: every declaration is one INDIUM
// uses, checked by hand against the installed headers. Four of these are easy to get
// subtly wrong, so they are called out where they are declared.
#[link(name = "archive")]
extern "C" {
    fn archive_write_new() -> *mut Archive;
    fn archive_write_set_format_pax_restricted(a: *mut Archive) -> c_int;
    fn archive_write_set_format_zip(a: *mut Archive) -> c_int;
    fn archive_write_add_filter_none(a: *mut Archive) -> c_int;
    fn archive_write_add_filter_gzip(a: *mut Archive) -> c_int;
    fn archive_write_add_filter_bzip2(a: *mut Archive) -> c_int;
    fn archive_write_add_filter_xz(a: *mut Archive) -> c_int;
    fn archive_write_add_filter_zstd(a: *mut Archive) -> c_int;
    fn archive_write_add_filter_lz4(a: *mut Archive) -> c_int;
    fn archive_write_set_options(a: *mut Archive, opts: *const c_char) -> c_int;
    /// Two arguments, not three. `archive_read_open_filename` takes a block size and
    /// this does not; the EXAMPLES section of `archive_write_set_options(3)` shows a
    /// third argument and is simply wrong about it.
    fn archive_write_open_filename(a: *mut Archive, file: *const c_char) -> c_int;
    fn archive_write_header(a: *mut Archive, e: *mut ArchiveEntry) -> c_int;
    /// `*const c_void` here; the read side's equivalent is `*mut`. Returns the count
    /// written, or a negative error — check `< 0`, never `!= len`, because libarchive
    /// 3.x sometimes answers a successful write with zero.
    fn archive_write_data(a: *mut Archive, buf: *const c_void, size: usize) -> isize;
    fn archive_write_finish_entry(a: *mut Archive) -> c_int;
    /// Marks the handle fatal so a later free does not flush a partial archive. This is
    /// the cancel path: a cancelled Apply must not leave a half-built file behind.
    fn archive_write_fail(a: *mut Archive) -> c_int;
    fn archive_write_close(a: *mut Archive) -> c_int;
    fn archive_write_free(a: *mut Archive) -> c_int;

    fn archive_entry_new() -> *mut ArchiveEntry;
    fn archive_entry_free(e: *mut ArchiveEntry);
    /// Returns the entry, not `void`. The return is unused, but declaring it wrong
    /// would be a signature mismatch.
    fn archive_entry_clear(e: *mut ArchiveEntry) -> *mut ArchiveEntry;
    fn archive_entry_set_size(e: *mut ArchiveEntry, size: i64);
    fn archive_entry_set_filetype(e: *mut ArchiveEntry, t: u32);
    fn archive_entry_set_perm(e: *mut ArchiveEntry, m: u32);
    /// Seconds *and* nanoseconds — two value parameters, the second a C `long`.
    fn archive_entry_set_mtime(e: *mut ArchiveEntry, sec: i64, nsec: c_long);
    fn archive_entry_set_atime(e: *mut ArchiveEntry, sec: i64, nsec: c_long);
    fn archive_entry_set_ctime(e: *mut ArchiveEntry, sec: i64, nsec: c_long);
    fn archive_entry_set_uid(e: *mut ArchiveEntry, uid: i64);
    fn archive_entry_set_gid(e: *mut ArchiveEntry, gid: i64);
    fn archive_entry_set_uname(e: *mut ArchiveEntry, n: *const c_char);
    fn archive_entry_set_gname(e: *mut ArchiveEntry, n: *const c_char);
    fn archive_entry_set_symlink_utf8(e: *mut ArchiveEntry, p: *const c_char);
    fn archive_entry_set_hardlink_utf8(e: *mut ArchiveEntry, p: *const c_char);

    /// Preferred over `archive_read_data_block` for the rebuild's reader: it fills a
    /// caller buffer in one call, and it materialises sparse holes as zeros, which is
    /// exactly right when copying an entry into a new archive.
    fn archive_read_data(a: *mut Archive, buf: *mut c_void, size: usize) -> isize;
}

// ---------------------------------------------------------------------------
// Small FFI helpers
// ---------------------------------------------------------------------------

/// Borrow a C string as an owned Rust `String`, lossily. Archive member names are not
/// guaranteed to be UTF-8, and refusing to display a mis-encoded name would be worse
/// than showing it with replacement characters.
fn cstr_to_string(p: *const c_char) -> Option<String> {
    if p.is_null() {
        return None;
    }
    // SAFETY: caller guarantees `p` is a NUL-terminated string owned by libarchive
    // and valid until the next call that invalidates it.
    let bytes = unsafe { CStr::from_ptr(p) }.to_bytes();
    Some(String::from_utf8_lossy(bytes).into_owned())
}

/// Put this process into the user's own character set, once, before libarchive reads a name.
///
/// **This is the whole of P11's worst find.** libarchive converts an entry's stored name
/// into the *current locale's* encoding at the moment it reads the header, and a C program
/// enables that by calling `setlocale` on the way in — `bsdtar` does, which is why `bsdtar`
/// has always extracted `köpek.txt` correctly from the very archive INDIUM dropped it from.
/// **A Rust program never calls `setlocale` at all.** The process therefore sits in the `C`
/// locale for the whole of its life, the target charset is ASCII, and a name with any byte
/// outside it simply cannot be converted.
///
/// libarchive's answer to a conversion it cannot do is to store nothing, so
/// `archive_entry_pathname` hands back **NULL** rather than a mangled string.
/// `cstr_to_string(NULL).unwrap_or_default()` turned that into an empty name; an empty name
/// matches no selection; and `extract`'s `if !selection_matches(..) { skip_data(); continue; }`
/// then dropped the entry without a word. Every `köpek.txt`, `résumé.pdf` and `日本語.txt`
/// in every archive listed as a nameless row and was silently left behind by extraction,
/// copy-out, preview and CRC alike — from P1 until here, in every shipped binary.
///
/// `archive_entry_pathname_utf8` is *not* a fix for it, though it looks like one and P11
/// tried it first: the conversion has already failed by then, and re-encoding nothing gives
/// nothing. Proven, not assumed — the accessor was swapped in and `köpek.txt` was still
/// nameless. It is kept below anyway, as a second line worth the two lines it costs.
///
/// `LC_CTYPE` rather than `LC_ALL`, and that is deliberate. `LC_CTYPE` is the only category
/// libarchive's charset conversion consults, while `LC_ALL` would also adopt the user's
/// `LC_NUMERIC` — and on the machine this was found on that is `tr_TR.UTF-8`, where the
/// decimal separator is a comma. Every C library in the process, the GL stack included,
/// would start parsing and printing numbers differently as a side effect of naming a file.
/// One category, one purpose.
///
/// It lives here rather than in `main` because the library is driven directly by the tests
/// in `tests/`, and a fix a caller has to remember is a fix that goes missing.
///
/// **`setlocale(LC_CTYPE, "")` on its own is not enough**, and the release rehearsal proved
/// it: the `debian:bookworm` container the `.deb` is built in has no UTF-8 locale configured
/// at all, so the user's-locale call lands back on `C` and every name is empty again. The
/// three `utf8.zip` tests failed there while passing on Arch — which is precisely why they
/// exist. The same hole is open on any machine run under `LC_ALL=C`, which is to say most
/// of them once INDIUM is started by something other than a desktop session.
///
/// So the user's locale is tried first, because it is theirs, and kept only if it can
/// actually carry the names. Otherwise a UTF-8 locale is forced. That is not overriding a
/// preference: an `Entry::raw_path` is a Rust `String` and therefore UTF-8 whatever the
/// locale says, so a non-UTF-8 `LC_CTYPE` cannot round-trip a name through this program
/// even when it converts one — `cstr_to_string` would take latin1 bytes and lossily decode
/// them. UTF-8 is the only setting under which INDIUM is correct.
///
/// `C.UTF-8` is built into glibc from 2.35 and needs no locale generation, which is what
/// makes it usable in a container that has none.
fn ensure_ctype_locale() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        // SAFETY: every argument is a NUL-terminated literal, and `Once` guarantees this
        // block runs exactly once — `setlocale` is not safe to race against itself.
        unsafe {
            // The user's own first.
            setlocale(LC_CTYPE, c"".as_ptr());
            if ctype_is_utf8() {
                return;
            }
            // It cannot carry a filename. Take the first UTF-8 locale this system has.
            for name in [c"C.UTF-8", c"en_US.UTF-8", c"C.utf8"] {
                if !setlocale(LC_CTYPE, name.as_ptr()).is_null() && ctype_is_utf8() {
                    return;
                }
            }
            // Nothing UTF-8 anywhere. Names outside ASCII will arrive empty, and `extract`
            // refuses outright rather than losing them quietly — which is the whole reason
            // that guard is a refusal and not a `continue`.
        }
    });
}

/// Is the character set `LC_CTYPE` currently names one that can carry a filename?
///
/// # Safety
/// Must be called with the locale settled — i.e. from inside `ensure_ctype_locale`'s `Once`.
unsafe fn ctype_is_utf8() -> bool {
    // SAFETY: `nl_langinfo` returns a pointer into locale-owned static storage, valid until
    // the next `setlocale`; it is read and compared before anything else touches the locale.
    let codeset = unsafe { cstr_to_string(nl_langinfo(CODESET)) }.unwrap_or_default();
    // `UTF-8`, `utf8`, `UTF8` — glibc says `UTF-8`, musl says `UTF-8`, and the spelling is
    // not worth trusting.
    let flat: String = codeset
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect();
    flat.eq_ignore_ascii_case("utf8")
}

/// One name off a libarchive entry, asked for as UTF-8 first.
///
/// The locale accessor is correct once [`ensure_ctype_locale`] has run, and is what
/// libarchive itself documents. Asking for UTF-8 first costs nothing and is what INDIUM
/// wants regardless — `Entry::raw_path` is a `String` — so it is asked first and the
/// locale form is the fallback.
///
/// # Safety
/// `ep` must be a live libarchive entry, valid until the next call that invalidates it.
unsafe fn entry_name(
    ep: *mut ArchiveEntry,
    utf8: unsafe extern "C" fn(*mut ArchiveEntry) -> *const c_char,
    mbs: unsafe extern "C" fn(*mut ArchiveEntry) -> *const c_char,
) -> Option<String> {
    // SAFETY: the caller's guarantee, forwarded to two read-only getters.
    unsafe { cstr_to_string(utf8(ep)).or_else(|| cstr_to_string(mbs(ep))) }
}

fn last_error(a: *mut Archive) -> String {
    // SAFETY: `a` is a live archive handle.
    match cstr_to_string(unsafe { archive_error_string(a) }) {
        Some(s) if !s.is_empty() => s,
        _ => "libarchive reported an error but supplied no message".to_string(),
    }
}

fn path_to_cstring(p: &Path) -> Result<CString, String> {
    CString::new(p.as_os_str().as_bytes())
        .map_err(|_| format!("path contains an interior NUL: {}", p.display()))
}

// ---------------------------------------------------------------------------
// The RAR gate
// ---------------------------------------------------------------------------

/// True if the file begins with a RAR signature.
///
/// CORE §5 says INDIUM "checks the detected format after open and refuses". We check
/// the magic bytes *as well*, and first, because libarchive can be built without RAR
/// support — in which case it reports "unrecognised format" and the user would get a
/// vague error instead of the exact sentence CORE requires. Recorded in Deviations.
pub fn looks_like_rar(path: &Path) -> bool {
    use std::io::Read;
    let mut head = [0u8; 8];
    let Ok(mut f) = std::fs::File::open(path) else {
        return false;
    };
    let Ok(n) = f.read(&mut head) else {
        return false;
    };
    if n < 7 {
        return false;
    }
    // "Rar!\x1a\x07" then 0x00 (RAR 1.5–4.x) or 0x01 0x00 (RAR 5.0+).
    &head[..6] == b"Rar!\x1a\x07" && (head[6] == 0x00 || (n >= 8 && head[6] == 0x01))
}

fn format_is_rar(a: *mut Archive) -> bool {
    // SAFETY: `a` is a live archive handle.
    let raw = unsafe { archive_format(a) };
    let base = raw & ARCHIVE_FORMAT_BASE_MASK;
    if base == ARCHIVE_FORMAT_RAR || base == ARCHIVE_FORMAT_RAR_V5 {
        return true;
    }
    // SAFETY: as above.
    cstr_to_string(unsafe { archive_format_name(a) })
        .map(|n| n.to_ascii_uppercase().contains("RAR"))
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Entry
// ---------------------------------------------------------------------------

/// Everything the reader can know about one member of an archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The path exactly as stored. Extraction matches on this; display does not.
    pub raw_path: String,
    /// The normalised path, for display and for building the directory tree.
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    /// Per-entry compressed size.
    ///
    /// Always `None` from the generic reader: libarchive exposes no per-entry
    /// compressed-size getter. CORE §4 already carries the matching honesty note for
    /// the stored CRC; the Inspector renders this as "—" and says why. 7z detail
    /// arrives with `sevenz-rust2` in P4.
    pub packed: Option<u64>,
    pub method: String,
    pub mtime: Option<i64>,
    pub atime: Option<i64>,
    pub ctime: Option<i64>,
    pub birthtime: Option<i64>,
    pub uid: i64,
    pub gid: i64,
    pub uname: Option<String>,
    pub gname: Option<String>,
    pub mode: u32,
    pub filetype: u32,
    pub symlink: Option<String>,
    pub hardlink: Option<String>,
    pub encrypted: bool,
}

/// A short, honest label for how an entry is stored.
///
/// Pure so it can be tested without an archive. libarchive reports the compression at
/// two levels: the *filter* (gzip, xz, zstd…) wraps the whole stream, and the *format
/// name* carries per-entry detail for zip, which libarchive updates as it reads.
pub fn method_label(format_name: &str, filter_name: &str) -> String {
    let filter = filter_name.trim();
    if !filter.is_empty() && !filter.eq_ignore_ascii_case("none") {
        return filter.to_string();
    }
    // "ZIP 2.0 (deflation)" -> "deflate"
    if let Some(start) = format_name.find('(') {
        if let Some(end) = format_name[start + 1..].find(')') {
            let inner = format_name[start + 1..start + 1 + end].trim();
            let lower = inner.to_ascii_lowercase();
            let mapped = match lower.as_str() {
                "deflation" => "deflate",
                "uncompressed" | "stored" => "store",
                other => other,
            };
            if !mapped.is_empty() {
                return mapped.to_string();
            }
        }
    }
    let upper = format_name.to_ascii_uppercase();
    if upper.contains("7-ZIP") || upper.contains("7ZIP") {
        // Per-entry 7z method (LZMA2, solid-block detail) needs sevenz-rust2 — P4.
        return "7z".to_string();
    }
    if upper.contains("TAR") || upper.contains("CPIO") || upper.contains("AR ") {
        return "store".to_string();
    }
    if format_name.trim().is_empty() {
        return "—".to_string();
    }
    format_name.to_string()
}

// ---------------------------------------------------------------------------
// Reader
// ---------------------------------------------------------------------------

/// An open archive. Freed on drop; never sent between threads.
pub struct Reader {
    raw: *mut Archive,
    first_header_seen: bool,
    /// libarchive's current entry, valid only until the next `next_entry` call.
    /// Extraction needs it to rewrite the pathname in place.
    current: *mut ArchiveEntry,
}

/// What an open archive reports about itself.
#[derive(Debug, Clone, Default)]
pub struct ArchiveInfo {
    pub format: String,
    pub filter: String,
    /// Whether the archive shares compression blocks between members.
    ///
    /// CORE §4 promised this in P4 and it was written and never shown. It belongs to the
    /// archive rather than to an entry — which is exactly *why* an entry's packed size so
    /// often cannot be given. `None` for every format but 7z, because no other reader
    /// exposes it.
    pub solid: Option<bool>,
    /// How many compression blocks the archive holds. `None` outside 7z.
    pub blocks: Option<usize>,
}

/// Why an open or a listing failed, when the reason is actionable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArchiveError {
    /// CORE §5's refusal. Carries no detail because the sentence is the whole message.
    Rar,
    /// The archive's headers are encrypted; nothing can be listed without a password.
    EncryptedHeaders,
    /// A password was supplied and rejected.
    WrongPassword,
    /// A password was supplied whose bytes are not valid UTF-8, and 7z stores a password
    /// as text. **Distinct from [`WrongPassword`](ArchiveError::WrongPassword) on purpose:**
    /// nothing was tried against the archive, so the password may well be right and INDIUM
    /// simply cannot encode it. Saying "wrong password" here would be the program knowing
    /// more than it says, which P18 nominated and P19 fixed.
    PasswordNotUtf8,
    /// The selection contains encrypted entries and no password was given.
    NeedPassword,
    /// An entry's stored path would write outside the destination. Carries the path
    /// so the user can see exactly what the archive tried.
    UnsafePath(String),
    /// Anything else, with libarchive's own words.
    Other(String),
}

impl std::fmt::Display for ArchiveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArchiveError::Rar => write!(f, "{RAR_REFUSAL}"),
            ArchiveError::EncryptedHeaders => {
                write!(
                    f,
                    "This archive's file names are encrypted. A password is needed to list it."
                )
            }
            ArchiveError::WrongPassword => write!(f, "Wrong password."),
            ArchiveError::PasswordNotUtf8 => write!(
                f,
                "INDIUM could not use this password: 7z stores one as text, and these \
                 bytes are not valid UTF-8. It was never tried against the archive."
            ),
            ArchiveError::NeedPassword => {
                write!(f, "This selection is encrypted. A password is needed.")
            }
            ArchiveError::UnsafePath(p) => write!(
                f,
                "Refused: an entry would be written outside the destination ({p})."
            ),
            ArchiveError::Other(s) => write!(f, "{s}"),
        }
    }
}

/// Does this message look like libarchive complaining about a missing or wrong
/// passphrase? libarchive has no error code for this, only prose, so we match prose.
fn mentions_passphrase(msg: &str) -> bool {
    let m = msg.to_ascii_lowercase();
    m.contains("passphrase")
        || m.contains("password")
        || m.contains("encrypted")
        || m.contains("incorrect")
}

impl Reader {
    pub fn open(path: &Path, passphrase: Option<&Secret>) -> Result<Reader, ArchiveError> {
        // Before anything else, and before every read path in the program, because a name
        // libarchive has already failed to convert cannot be recovered afterwards.
        ensure_ctype_locale();

        // The gate comes before anything is handed to libarchive.
        if looks_like_rar(path) {
            return Err(ArchiveError::Rar);
        }

        let cpath = path_to_cstring(path).map_err(ArchiveError::Other)?;

        // SAFETY: the sequence below is libarchive's documented read lifecycle. Every
        // pointer is checked before use and the handle is freed on every error path.
        unsafe {
            let a = archive_read_new();
            if a.is_null() {
                return Err(ArchiveError::Other(
                    "libarchive could not allocate a reader".to_string(),
                ));
            }
            archive_read_support_filter_all(a);
            archive_read_support_format_all(a);

            if let Some(secret) = passphrase {
                match secret.to_c_string() {
                    Some(c) => {
                        archive_read_add_passphrase(a, c.as_ptr());
                    }
                    None => {
                        archive_read_free(a);
                        return Err(ArchiveError::Other(
                            "a password cannot contain a NUL byte".to_string(),
                        ));
                    }
                }
            }

            if archive_read_open_filename(a, cpath.as_ptr(), 65536) != ARCHIVE_OK {
                let msg = last_error(a);
                archive_read_free(a);
                return Err(ArchiveError::Other(msg));
            }

            Ok(Reader {
                raw: a,
                first_header_seen: false,
                current: std::ptr::null_mut(),
            })
        }
    }

    pub fn info(&self) -> ArchiveInfo {
        // SAFETY: `self.raw` is live for the lifetime of `self`.
        unsafe {
            ArchiveInfo {
                format: cstr_to_string(archive_format_name(self.raw)).unwrap_or_default(),
                filter: cstr_to_string(archive_filter_name(self.raw, 0)).unwrap_or_default(),
                // libarchive knows nothing of blocks; only the 7z reader fills these.
                solid: None,
                blocks: None,
            }
        }
    }

    /// Advance to the next header. `Ok(None)` is end of archive.
    ///
    /// The returned `Entry` is a snapshot: the underlying `archive_entry` belongs to
    /// libarchive and is invalidated by the next call.
    pub fn next_entry(&mut self) -> Result<Option<Entry>, ArchiveError> {
        let mut ep: *mut ArchiveEntry = std::ptr::null_mut();
        self.current = std::ptr::null_mut();
        // SAFETY: `self.raw` is live; `ep` is written by libarchive on success.
        let rc = unsafe { archive_read_next_header(self.raw, &mut ep) };

        if rc == ARCHIVE_EOF {
            // A RAR reaches here: libarchive identifies the format but returns EOF
            // rather than a header, so "check after the first *successful* header"
            // (P1 §2) would never fire. The format name is set by now either way.
            if !self.first_header_seen && format_is_rar(self.raw) {
                return Err(ArchiveError::Rar);
            }
            return Ok(None);
        }

        if rc != ARCHIVE_OK && rc != ARCHIVE_WARN {
            let msg = last_error(self.raw);
            // A RAR that libarchive was built to recognise but not to read reaches
            // here; the exact sentence still wins over libarchive's wording.
            if format_is_rar(self.raw) {
                return Err(ArchiveError::Rar);
            }
            if !self.first_header_seen && mentions_passphrase(&msg) {
                // SAFETY: `self.raw` is live.
                let enc = unsafe { archive_read_has_encrypted_entries(self.raw) };
                // > 0 means libarchive knows there are encrypted entries; -1 means it
                // cannot tell, which for a first-header failure is the same situation.
                if enc != 0 {
                    return Err(ArchiveError::EncryptedHeaders);
                }
            }
            return Err(ArchiveError::Other(msg));
        }

        if ep.is_null() {
            return Err(ArchiveError::Other(
                "libarchive returned success but no entry".to_string(),
            ));
        }

        if !self.first_header_seen {
            self.first_header_seen = true;
            if format_is_rar(self.raw) {
                return Err(ArchiveError::Rar);
            }
        }

        self.current = ep;
        let info = self.info();
        // SAFETY: `ep` is a live entry owned by libarchive, valid until the next
        // `archive_read_next_header` call. Every getter below is read-only.
        let entry = unsafe {
            let raw_path = entry_name(ep, archive_entry_pathname_utf8, archive_entry_pathname)
                .unwrap_or_default();
            let filetype = archive_entry_filetype(ep);
            let normalized = util::normalize_archive_path(&raw_path);
            let size = archive_entry_size(ep).max(0) as u64;
            let is_dir = (filetype & AE_IFMT) == AE_IFDIR || raw_path.ends_with('/');

            Entry {
                raw_path,
                path: normalized,
                is_dir,
                size,
                packed: None,
                method: method_label(&info.format, &info.filter),
                mtime: (archive_entry_mtime_is_set(ep) != 0).then(|| archive_entry_mtime(ep)),
                atime: (archive_entry_atime_is_set(ep) != 0).then(|| archive_entry_atime(ep)),
                ctime: (archive_entry_ctime_is_set(ep) != 0).then(|| archive_entry_ctime(ep)),
                birthtime: (archive_entry_birthtime_is_set(ep) != 0)
                    .then(|| archive_entry_birthtime(ep)),
                uid: archive_entry_uid(ep),
                gid: archive_entry_gid(ep),
                uname: cstr_to_string(archive_entry_uname(ep)).filter(|s| !s.is_empty()),
                gname: cstr_to_string(archive_entry_gname(ep)).filter(|s| !s.is_empty()),
                mode: archive_entry_mode(ep),
                filetype,
                symlink: entry_name(ep, archive_entry_symlink_utf8, archive_entry_symlink)
                    .filter(|s| !s.is_empty()),
                hardlink: entry_name(ep, archive_entry_hardlink_utf8, archive_entry_hardlink)
                    .filter(|s| !s.is_empty()),
                encrypted: archive_entry_is_encrypted(ep) != 0,
            }
        };

        Ok(Some(entry))
    }

    pub fn skip_data(&mut self) {
        // SAFETY: `self.raw` is live.
        unsafe { archive_read_data_skip(self.raw) };
    }

    /// libarchive's current entry pointer, or null if we are not positioned on one.
    /// Valid only until the next `next_entry` call.
    fn current_entry_ptr(&self) -> *mut ArchiveEntry {
        self.current
    }
}

impl Drop for Reader {
    fn drop(&mut self) {
        // SAFETY: `self.raw` came from `archive_read_new` and is freed exactly once.
        // Freeing the reader is also what releases libarchive's internal copy of any
        // passphrase — see the note in `Secret::to_c_string`.
        unsafe {
            archive_read_close(self.raw);
            archive_read_free(self.raw);
        }
    }
}

// ---------------------------------------------------------------------------
// Listing
// ---------------------------------------------------------------------------

/// What the worker sends back while listing.
#[derive(Debug)]
pub enum ListMsg {
    Opened(ArchiveInfo),
    Entry(Box<Entry>),
    Done { count: usize },
    Failed(ArchiveError),
}

/// Read every header, streaming entries as they arrive.
///
/// CORE §3: "Listing streams entries over a channel from a worker thread." The table
/// fills while a huge archive is still being read.
pub fn list(
    path: &Path,
    passphrase: Option<&Secret>,
    tx: &Sender<ListMsg>,
    cancel: &Arc<AtomicBool>,
) {
    // A 7z takes the same route here that `list_all` takes, or everything
    // `sevenz-rust2` knows — packed sizes, the real coder, an encrypted-header archive
    // that libarchive will not open at all — would reach the tests and never the window.
    // 7z headers do not parse incrementally, so the entries arrive in one go and are
    // then sent one at a time; the channel shape the UI drains is unchanged.
    if looks_like_7z(path) {
        match crate::sevenz::list_all(path, passphrase) {
            Ok(entries) => {
                let _ = tx.send(ListMsg::Opened(crate::sevenz::info_of(path, passphrase)));
                let count = entries.len();
                for entry in entries {
                    if cancel.load(Ordering::Relaxed) {
                        let _ = tx.send(ListMsg::Done { count });
                        return;
                    }
                    if tx.send(ListMsg::Entry(Box::new(entry))).is_err() {
                        return; // the UI went away
                    }
                }
                let _ = tx.send(ListMsg::Done { count });
                return;
            }
            // A password problem belongs to the caller — the window turns it into a
            // prompt. Falling through to libarchive would replace it with a vaguer
            // error, or with an empty listing, which is worse.
            Err(e @ (ArchiveError::NeedPassword | ArchiveError::WrongPassword)) => {
                let _ = tx.send(ListMsg::Failed(e));
                return;
            }
            // Anything else and libarchive gets its ordinary turn; nothing is lost.
            Err(_) => {}
        }
    }

    let mut reader = match Reader::open(path, passphrase) {
        Ok(r) => r,
        Err(e) => {
            let _ = tx.send(ListMsg::Failed(e));
            return;
        }
    };

    let mut count = 0usize;
    let mut announced = false;

    loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = tx.send(ListMsg::Done { count });
            return;
        }
        match reader.next_entry() {
            Ok(Some(entry)) => {
                if !announced {
                    // The format is only fully known once a header has been read.
                    let _ = tx.send(ListMsg::Opened(reader.info()));
                    announced = true;
                }
                count += 1;
                if tx.send(ListMsg::Entry(Box::new(entry))).is_err() {
                    return; // the UI went away
                }
                reader.skip_data();
            }
            Ok(None) => break,
            Err(e) => {
                let _ = tx.send(ListMsg::Failed(e));
                return;
            }
        }
    }

    if !announced {
        let _ = tx.send(ListMsg::Opened(reader.info()));
    }
    let _ = tx.send(ListMsg::Done { count });
}

/// List into a `Vec`, for tests and for any caller that does not want a channel.
pub fn list_all(path: &Path, passphrase: Option<&Secret>) -> Result<Vec<Entry>, ArchiveError> {
    if let Some(entries) = list_7z(path, passphrase) {
        return entries;
    }
    list_via_libarchive(path, passphrase)
}

fn list_via_libarchive(
    path: &Path,
    passphrase: Option<&Secret>,
) -> Result<Vec<Entry>, ArchiveError> {
    let mut reader = Reader::open(path, passphrase)?;
    let mut out = Vec::new();
    while let Some(e) = reader.next_entry()? {
        out.push(e);
        reader.skip_data();
    }
    Ok(out)
}

/// The magic of a 7z, sniffed before either reader is asked to open the file.
///
/// Mirrors `looks_like_rar`, and for the same reason: the decision of which reader to
/// use should not depend on first getting an error out of the wrong one.
pub fn looks_like_7z(path: &Path) -> bool {
    use std::io::Read as _;
    let mut head = [0u8; 6];
    match std::fs::File::open(path).and_then(|mut f| f.read_exact(&mut head)) {
        Ok(()) => head == [0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C],
        Err(_) => false,
    }
}

/// P4 §4's routing: a 7z is **listed** through `sevenz-rust2`, because it alone can give
/// per-entry packed sizes, solid-block detail, and a listing of an archive whose headers
/// are encrypted. `None` means "not a 7z, or that reader could not parse it" — in which
/// case libarchive gets its ordinary turn, and nothing is lost.
///
/// Data — extraction, CRC32, passphrase checks — deliberately does **not** route here.
/// With this crate's default features off it carries no bzip2, ppmd, deflate or zstd
/// decoder, so making it the sole 7z reader would be a read regression against CORE §5's
/// promise to read everything libarchive reads.
fn list_7z(path: &Path, passphrase: Option<&Secret>) -> Option<Result<Vec<Entry>, ArchiveError>> {
    if !looks_like_7z(path) {
        return None;
    }
    match crate::sevenz::list_all(path, passphrase) {
        Ok(entries) => Some(Ok(entries)),
        // A password problem is the caller's to hear about: falling through to
        // libarchive would turn "wrong password" into a vaguer error, or into an empty
        // listing, which is worse.
        Err(e @ (ArchiveError::NeedPassword | ArchiveError::WrongPassword)) => Some(Err(e)),
        Err(_) => None,
    }
}

// ---------------------------------------------------------------------------
// Extraction
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum ExtractMsg {
    Progress {
        done: usize,
        total: usize,
    },
    Done {
        written: usize,
    },
    /// The cancellation flag was set, and `extract` stopped with `written` entries on
    /// disk out of however many were asked for.
    ///
    /// `ApplyMsg::Cancelled` in `tasks.rs` is the precedent, and its doc comment named
    /// this defect while it still stood: "unlike `ExtractMsg`, where its absence means a
    /// cancelled extraction is indistinguishable from a finished one". A cancelled
    /// extraction returns `Ok(written)` like any other, so without this variant the only
    /// way to tell the two apart was for the UI to re-read a flag the window may already
    /// have replaced — which is how half a selection reached the clipboard as if it were
    /// the whole of it. The worker decides, with its own clone, and says so in band.
    Cancelled {
        written: usize,
    },
    Failed(String),
}

/// Should this entry come out, given the selection?
///
/// An entry matches if it was selected outright, or if it lives beneath a selected
/// directory. Pure, so the rule is testable without an archive.
pub fn selection_matches(entry_path: &str, wanted: &HashSet<String>) -> bool {
    if wanted.contains(entry_path) {
        return true;
    }
    wanted.iter().any(|w| {
        !w.is_empty()
            && entry_path.starts_with(w)
            && entry_path.as_bytes().get(w.len()) == Some(&b'/')
    })
}

/// Would this stored path write outside the destination?
///
/// An archive member may name anything at all; an absolute path or any `..`
/// component is a traversal attempt. Pure, so the rule is testable on its own.
///
/// This exists because libarchive's `SECURE_NODOTDOT` alone is **not** sufficient the
/// way P1 §2 assumes. P1 has extraction prefix an absolute `dest` onto the stored name
/// via `archive_entry_set_pathname`; with an absolute path in hand libarchive does not
/// refuse the `..`, and `evil.zip` extracts. Proven by
/// `a_traversal_entry_is_refused_and_writes_nothing`. INDIUM therefore judges the path
/// itself and keeps the secure flags as a second line. Recorded in Deviations.
pub fn path_escapes(raw: &str) -> bool {
    let p = raw.replace('\\', "/");
    if p.starts_with('/') {
        return true;
    }
    p.split('/').any(|c| c == "..")
}

/// Extract the selected entries into `dest`.
///
/// Everything that can be known before a byte is written is settled first: traversal
/// is refused outright, and encryption is resolved from the entry flags (P2 §5 —
/// "known **before starting**"), so a wrong password costs nothing and leaves no
/// partial output behind.
pub fn extract(
    path: &Path,
    wanted: &HashSet<String>,
    dest: &Path,
    passphrase: Option<&Secret>,
    tx: Option<&Sender<ExtractMsg>>,
    cancel: &Arc<AtomicBool>,
) -> Result<usize, ArchiveError> {
    // ---- Pre-flight. Nothing below this block touches the filesystem. ----
    let listing = list_all(path, passphrase)?;
    let selected: Vec<&Entry> = listing
        .iter()
        .filter(|e| selection_matches(&e.path, wanted))
        .collect();

    // A nameless entry is the shape P11's locale defect took, and this is the line that
    // makes it impossible for it — or anything like it — to be silent a second time.
    //
    // An entry whose name did not survive the read matches no selection, so the loop
    // below would `skip_data(); continue;` past it exactly as it would past a file the
    // user did not ask for. The two are indistinguishable from inside the loop and mean
    // opposite things. Judged here instead, over the whole listing rather than the
    // selection, and before a single byte is written: an archive INDIUM cannot name every
    // member of is one it refuses to extract at all, rather than one it extracts almost
    // all of. `ensure_ctype_locale` should mean this is never reached.
    if listing.iter().any(|e| e.path.is_empty()) {
        return Err(ArchiveError::Other(
            "this archive holds an entry whose name could not be read on this system; \
             nothing was extracted"
                .to_string(),
        ));
    }

    for entry in &selected {
        if path_escapes(&entry.raw_path) {
            return Err(ArchiveError::UnsafePath(entry.raw_path.clone()));
        }
    }

    // Does libarchive refuse this archive's headers outright? If so the 7z reader owns
    // both the verification and the data, and asking libarchive to check the password
    // would fail for a reason that has nothing to do with the password being wrong.
    let headers_need_sevenz =
        looks_like_7z(path) && Reader::open(path, passphrase)?.next_entry().is_err();

    if selected.iter().any(|e| e.encrypted) {
        match passphrase {
            None => return Err(ArchiveError::NeedPassword),
            Some(secret) => {
                // The listing above already succeeded, and an encrypted-header archive
                // cannot be parsed at all without the right password — so reaching this
                // line *is* the verification, and a second one through a reader that
                // cannot open the file would only ever report a false failure.
                if !headers_need_sevenz && !verify_passphrase(path, secret)? {
                    return Err(ArchiveError::WrongPassword);
                }
            }
        }
    }
    // ---- Pre-flight over. ----

    // P5 §A1b, the third read path. An encrypted-header 7z lists through `sevenz` and
    // libarchive cannot read a byte of it, so extraction routes there too rather than
    // leaving an archive that opens and then refuses to give anything up. The guards
    // above are untouched — the decoder changes where the bytes come from, never what is
    // allowed to be written.
    if headers_need_sevenz {
        let mut written = 0usize;
        for entry in &selected {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            let target = dest.join(&entry.raw_path);
            if entry.is_dir {
                std::fs::create_dir_all(&target).map_err(|e| {
                    ArchiveError::Other(format!("could not create {target:?}: {e}"))
                })?;
                continue;
            }
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    ArchiveError::Other(format!("could not create {parent:?}: {e}"))
                })?;
            }
            let (bytes, _) = crate::sevenz::read_entry(path, &entry.path, usize::MAX, passphrase)?;
            std::fs::write(&target, &bytes)
                .map_err(|e| ArchiveError::Other(format!("could not write {target:?}: {e}")))?;
            written += 1;
            if let Some(tx) = tx {
                let _ = tx.send(ExtractMsg::Progress {
                    done: written,
                    total: selected.len(),
                });
            }
        }
        return Ok(written);
    }

    let mut reader = Reader::open(path, passphrase)?;
    let total = selected.len();
    let mut written = 0usize;

    loop {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let entry = match reader.next_entry()? {
            Some(e) => e,
            None => break,
        };

        if !selection_matches(&entry.path, wanted) {
            reader.skip_data();
            continue;
        }

        // Re-checked here as well as in pre-flight: the archive is read twice, and
        // the guard that keeps files inside `dest` should not depend on the two
        // passes agreeing.
        if path_escapes(&entry.raw_path) {
            return Err(ArchiveError::UnsafePath(entry.raw_path.clone()));
        }

        let target: PathBuf = dest.join(&entry.raw_path);
        let ctarget = path_to_cstring(&target).map_err(ArchiveError::Other)?;

        // The entry `next_entry` just read is still libarchive's current one, and the
        // Reader kept its pointer. Rewriting the pathname through it is what makes
        // extraction chdir-free.
        let ep = reader.current_entry_ptr();
        if ep.is_null() {
            return Err(ArchiveError::Other(
                "lost libarchive's current entry before extraction".to_string(),
            ));
        }
        // SAFETY: `ep` is libarchive's live current entry — we have not advanced the
        // reader since `next_entry` returned — and `ctarget` outlives the call.
        let rc = unsafe {
            // Deliberately the **locale** setter and not `_utf8`, which is the opposite of
            // what the writer does two hundred lines down and is not an oversight. What
            // goes in here is a *filesystem* path INDIUM already holds as bytes, not a
            // name to be stored in an archive: libarchive must lay these bytes down
            // exactly as given. Naming them UTF-8 would invite a conversion on the way to
            // the syscall, and a path is bytes on Linux, not text.
            archive_entry_set_pathname(ep, ctarget.as_ptr());
            archive_read_extract(reader.raw, ep, EXTRACT_FLAGS)
        };

        if rc != ARCHIVE_OK && rc != ARCHIVE_WARN {
            let msg = last_error(reader.raw);
            if mentions_passphrase(&msg) {
                return Err(ArchiveError::WrongPassword);
            }
            return Err(ArchiveError::Other(msg));
        }

        // ARCHIVE_WARN carries two opposite meanings at this call. It is what libarchive
        // returns for a file it wrote but could not finish stamping — `EXTRACT_PERM`
        // against vfat is the everyday case, and failing that extraction would be wrong.
        // It is *also* what it returns for a file it could not create at all: a
        // destination the user cannot write answers -20 with `Can't create '<path>'` and
        // leaves nothing behind. Treating both as written is what let `/boot` report
        // "Extracted 1 entry." with an empty directory underneath.
        //
        // The wording is libarchive's and not worth parsing, so the filesystem is asked
        // instead. `symlink_metadata`, because a symlink entry that points nowhere is a
        // legitimate extraction and `exists` would follow it and say no.
        //
        // One case survives on purpose: a target left by an earlier extraction cannot be
        // told from one written a moment ago, so a failed overwrite still counts. Closing
        // it means comparing timestamps, which costs more than the case is worth.
        if rc == ARCHIVE_WARN && std::fs::symlink_metadata(&target).is_err() {
            let msg = last_error(reader.raw);
            return Err(ArchiveError::Other(if msg.is_empty() {
                format!("could not write {}", target.display())
            } else {
                msg
            }));
        }

        written += 1;
        if let Some(tx) = tx {
            let _ = tx.send(ExtractMsg::Progress {
                done: written,
                total,
            });
        }
    }

    Ok(written)
}

// ---------------------------------------------------------------------------
// CRC32 on demand
// ---------------------------------------------------------------------------

/// The first `cap` bytes of one entry, and whether more was left unread.
///
/// P5 §B1. There was no way to get an entry's bytes into memory: `crc32_of` consumed them
/// as it went and `extract` only ever wrote to disk. This is shaped like `crc32_of` and
/// built on P4's `EntryData`, so `archive_read_data`'s short-read and sparse-hole
/// behaviour is shared with the rebuild path rather than derived a second time.
///
/// The cap is not a nicety. An archive is untrusted input and a member may be gigabytes;
/// a preview that read all of it would be a way to make the window disappear. The `bool`
/// is true when the entry was longer than the cap, which is what tells Preview it may
/// sniff but must not try to decode an image.
pub fn head_of(
    path: &Path,
    entry_path: &str,
    cap: usize,
    passphrase: Option<&Secret>,
) -> Result<(Vec<u8>, bool), ArchiveError> {
    // Same routing as every other read path: libarchive first, and the 7z reader only
    // where libarchive refuses. See §A1b.
    match head_via_libarchive(path, entry_path, cap, passphrase) {
        Err(ArchiveError::EncryptedHeaders) if looks_like_7z(path) => {
            crate::sevenz::read_entry(path, entry_path, cap, passphrase)
        }
        other => other,
    }
}

fn head_via_libarchive(
    path: &Path,
    entry_path: &str,
    cap: usize,
    passphrase: Option<&Secret>,
) -> Result<(Vec<u8>, bool), ArchiveError> {
    use std::io::Read as _;

    let mut reader = Reader::open(path, passphrase)?;
    while let Some(entry) = reader.next_entry()? {
        if entry.path != entry_path {
            reader.skip_data();
            continue;
        }
        let mut out = Vec::new();
        // One byte past the cap, so "there is more" is known without a second pass.
        let mut data = EntryData::new(&mut reader).take(cap as u64 + 1);
        data.read_to_end(&mut out).map_err(|e| {
            let msg = e.to_string();
            if mentions_passphrase(&msg) {
                ArchiveError::WrongPassword
            } else {
                ArchiveError::Other(msg)
            }
        })?;
        let truncated = out.len() > cap;
        out.truncate(cap);
        return Ok((out, truncated));
    }
    Err(ArchiveError::Other(format!("no such entry: {entry_path}")))
}

/// Write one entry's bytes to `out`, streamed, with no cap at all.
///
/// P17 §2. This is what `indium cat` is built on, and it is deliberately **not**
/// [`head_of`]. `head_of` caps because the Preview must not be able to make the window
/// disappear, and 8 MiB is the number it caps at; a `cat` that silently stopped there
/// would be the same class of lie as a hex view that reflowed — the reader would have no
/// way to know the file continued. Calling `head_of` with `usize::MAX` is not the fix
/// either: it does `take(cap as u64 + 1)`, which overflows, and it buffers the member in
/// a `Vec` besides.
///
/// The routing is `head_of`'s, line for line — libarchive first, and the 7z reader only
/// where libarchive refuses an encrypted-header archive — so the two cannot drift about
/// which reader owns which archive.
///
/// Returns the number of bytes written, which the caller may compare against the entry's
/// stated size.
///
/// **The 7z branch does not stream.** `sevenz::read_entry` reads the member into a `Vec`
/// before a byte reaches `out`, so `cat` of a four-gigabyte 7z member wants four gigabytes
/// of memory where the libarchive branch wants none. That is not a new compromise: it is
/// what [`extract`] and [`crc32_of`] already do for 7z, because `sevenz-rust2` decodes a
/// solid block at a time and there is no smaller unit to hand out. It is written here
/// rather than left for whoever first cats a large 7z.
pub fn stream_entry(
    path: &Path,
    entry_path: &str,
    passphrase: Option<&Secret>,
    out: &mut dyn std::io::Write,
) -> Result<u64, ArchiveError> {
    match stream_via_libarchive(path, entry_path, passphrase, out) {
        Err(ArchiveError::EncryptedHeaders) if looks_like_7z(path) => {
            // `usize::MAX` is safe here and was checked rather than assumed: `read_entry`
            // uses `take(cap as u64)` with no `+ 1`, so nothing overflows, and its
            // `out.len() >= cap` can only be true for a member no machine could hold.
            let (bytes, _truncated) =
                crate::sevenz::read_entry(path, entry_path, usize::MAX, passphrase)?;
            out.write_all(&bytes)
                .map_err(|e| ArchiveError::Other(e.to_string()))?;
            Ok(bytes.len() as u64)
        }
        other => other,
    }
}

fn stream_via_libarchive(
    path: &Path,
    entry_path: &str,
    passphrase: Option<&Secret>,
    out: &mut dyn std::io::Write,
) -> Result<u64, ArchiveError> {
    let mut reader = Reader::open(path, passphrase)?;
    while let Some(entry) = reader.next_entry()? {
        if entry.path != entry_path {
            reader.skip_data();
            continue;
        }
        // A directory has no bytes, and asking for them is a caller's mistake rather than
        // an empty success — `cat` on a directory is an error everywhere else too.
        //
        // **This branch only.** The 7z fallback goes through `sevenz::read_entry`, whose
        // no-data-stream path returns `Ok((vec![], false))` for a directory rather than an
        // error — so a directory inside an *encrypted-header* 7z, the one case that reaches
        // that fallback, is a silent empty success instead. Recorded rather than fixed:
        // levelling it means changing `read_entry`, which the Preview and the rebuild both
        // call, to serve a case reachable only through `cat`.
        if entry.is_dir {
            return Err(ArchiveError::Other(format!("{entry_path} is a directory")));
        }
        let mut data = EntryData::new(&mut reader);
        // `io::copy` and not `read_to_end`: the whole point is that no member is ever
        // held whole. A wrong password surfaces here, as it does in `head_of`, because
        // libarchive reports it when the data is asked for and not when the header was.
        return std::io::copy(&mut data, out).map_err(|e| {
            let msg = e.to_string();
            if mentions_passphrase(&msg) {
                ArchiveError::WrongPassword
            } else {
                ArchiveError::Other(msg)
            }
        });
    }
    Err(ArchiveError::Other(format!("no such entry: {entry_path}")))
}

/// Stream one entry through the hand-written CRC32.
///
/// CORE §4: libarchive does not expose an entry's *stored* CRC, so INDIUM computes it
/// on demand and the Inspector labels the value *computed*.
pub fn crc32_of(
    path: &Path,
    entry_path: &str,
    passphrase: Option<&Secret>,
) -> Result<u32, ArchiveError> {
    // P5 §A1b: libarchive first, and the 7z reader only where libarchive refuses. That
    // is what P4 §4 promised and did not build, and it became reachable the moment an
    // encrypted-header archive could be listed — such an archive would otherwise open,
    // list, and then refuse every read.
    //
    // The refusal does not come from `Reader::open`, which succeeds: opening the *file*
    // is fine, and it is the first header that libarchive cannot decrypt. So the
    // fallback is keyed on the error the read actually produces.
    match crc32_via_libarchive(path, entry_path, passphrase) {
        Err(ArchiveError::EncryptedHeaders) if looks_like_7z(path) => {
            let (bytes, _) = crate::sevenz::read_entry(path, entry_path, usize::MAX, passphrase)?;
            Ok(util::crc32(&bytes))
        }
        other => other,
    }
}

fn crc32_via_libarchive(
    path: &Path,
    entry_path: &str,
    passphrase: Option<&Secret>,
) -> Result<u32, ArchiveError> {
    let mut reader = Reader::open(path, passphrase)?;
    while let Some(entry) = reader.next_entry()? {
        if entry.path != entry_path {
            reader.skip_data();
            continue;
        }
        let mut crc = Crc32::new();
        loop {
            let mut buf: *const c_void = std::ptr::null();
            let mut size: usize = 0;
            let mut offset: i64 = 0;
            // SAFETY: `reader.raw` is live and we are positioned on a header.
            let rc =
                unsafe { archive_read_data_block(reader.raw, &mut buf, &mut size, &mut offset) };
            if rc == ARCHIVE_EOF {
                break;
            }
            if rc != ARCHIVE_OK && rc != ARCHIVE_WARN {
                let msg = last_error(reader.raw);
                if mentions_passphrase(&msg) {
                    return Err(ArchiveError::WrongPassword);
                }
                return Err(ArchiveError::Other(msg));
            }
            if size > 0 && !buf.is_null() {
                // SAFETY: libarchive guarantees `buf` is valid for `size` bytes.
                let slice = unsafe { std::slice::from_raw_parts(buf as *const u8, size) };
                crc.update(slice);
            }
        }
        return Ok(crc.finish());
    }
    Err(ArchiveError::Other(format!("no such entry: {entry_path}")))
}

// ---------------------------------------------------------------------------
// Password verification (P2 §5)
// ---------------------------------------------------------------------------

/// Try a password against the first encrypted entry, writing nothing.
///
/// P2 §5: "verify by test-reading the first data block of the first encrypted entry
/// with a throwaway reader" — so three wrong attempts cost the user nothing and leave
/// no partial output to clean up.
pub fn verify_passphrase(path: &Path, passphrase: &Secret) -> Result<bool, ArchiveError> {
    let mut reader = match Reader::open(path, Some(passphrase)) {
        Ok(r) => r,
        Err(ArchiveError::EncryptedHeaders) => return Ok(false),
        Err(e) => return Err(e),
    };

    loop {
        let entry = match reader.next_entry() {
            Ok(Some(e)) => e,
            Ok(None) => return Ok(true), // nothing encrypted to disagree with
            Err(ArchiveError::EncryptedHeaders) | Err(ArchiveError::WrongPassword) => {
                return Ok(false)
            }
            Err(ArchiveError::Other(msg)) if mentions_passphrase(&msg) => return Ok(false),
            Err(e) => return Err(e),
        };

        if !entry.encrypted || entry.is_dir || entry.size == 0 {
            reader.skip_data();
            continue;
        }

        let mut buf: *const c_void = std::ptr::null();
        let mut size: usize = 0;
        let mut offset: i64 = 0;
        // SAFETY: `reader.raw` is live and positioned on an encrypted entry's header.
        let rc = unsafe { archive_read_data_block(reader.raw, &mut buf, &mut size, &mut offset) };
        if rc == ARCHIVE_OK || rc == ARCHIVE_WARN || rc == ARCHIVE_EOF {
            return Ok(true);
        }
        return Ok(false);
    }
}

/// Does this archive contain encrypted entries? `None` when libarchive cannot tell.
pub fn has_encrypted_entries(path: &Path) -> Option<bool> {
    let mut reader = Reader::open(path, None).ok()?;
    // The answer is only reliable once a header has been read.
    let _ = reader.next_entry();
    // SAFETY: `reader.raw` is live.
    let rc = unsafe { archive_read_has_encrypted_entries(reader.raw) };
    match rc {
        -2 => None, // ENCRYPTION_UNSUPPORTED: the format cannot encrypt
        -1 => None, // ENCRYPTION_DONT_KNOW
        0 => Some(false),
        _ => Some(true),
    }
}

// ---------------------------------------------------------------------------
// The write half — P4 §3
// ---------------------------------------------------------------------------

/// An open archive being written. Freed on drop; never sent between threads.
///
/// Mirrors `Reader`: the same hand-written FFI, the same RAII, the same refusal to
/// interpret a non-`ARCHIVE_OK` return as success. It reuses one `archive_entry` across
/// every member, cleared each time, because allocating one per member would be a wasted
/// call in an archive with a hundred thousand of them.
pub struct Writer {
    raw: *mut Archive,
    entry: *mut ArchiveEntry,
    finished: bool,
}

impl Writer {
    /// Open `path` for writing under `recipe`.
    ///
    /// Order matters and is fixed by the header's own comment: format and filters and
    /// options are all set before the file is opened, because opening "freezes the
    /// settings".
    pub fn create(path: &Path, recipe: &Recipe) -> Result<Writer, String> {
        // The write half needs it for the same reason the read half does: a name is
        // converted on its way *into* an archive as well as out of one.
        ensure_ctype_locale();

        // SAFETY: a fresh handle; every early return frees it before leaving.
        let raw = unsafe { archive_write_new() };
        if raw.is_null() {
            return Err("libarchive would not allocate a writer".to_string());
        }

        let mut writer = Writer {
            raw,
            entry: std::ptr::null_mut(),
            finished: false,
        };

        writer.configure(recipe)?;

        let cpath = path_to_cstring(path)?;
        // SAFETY: `raw` is live and `cpath` outlives the call.
        let rc = unsafe { archive_write_open_filename(writer.raw, cpath.as_ptr()) };
        writer.check(rc, "open the archive for writing")?;

        // SAFETY: allocation only; checked immediately.
        writer.entry = unsafe { archive_entry_new() };
        if writer.entry.is_null() {
            return Err("libarchive would not allocate an entry".to_string());
        }
        Ok(writer)
    }

    /// Select the container, the filter and the compression level.
    fn configure(&mut self, recipe: &Recipe) -> Result<(), String> {
        // SAFETY: `self.raw` is live for every call in this function.
        unsafe {
            let rc = match recipe.container() {
                Container::Tar => archive_write_set_format_pax_restricted(self.raw),
                Container::Zip => archive_write_set_format_zip(self.raw),
                Container::SevenZ => {
                    return Err(
                        "7z is written by sevenz-rust2, not by libarchive — CORE §2".to_string()
                    )
                }
            };
            self.check(rc, "set the archive format")?;

            let rc = match recipe.method {
                Method::Store | Method::Deflate => archive_write_add_filter_none(self.raw),
                Method::Gzip => archive_write_add_filter_gzip(self.raw),
                Method::Bzip2 => archive_write_add_filter_bzip2(self.raw),
                Method::Xz => archive_write_add_filter_xz(self.raw),
                Method::Zstd => archive_write_add_filter_zstd(self.raw),
                Method::Lz4 => archive_write_add_filter_lz4(self.raw),
                Method::Lzma2 => unreachable!("LZMA2 is 7z, refused above"),
            };
            self.check(rc, "set the compression filter")?;
        }

        // A refused option is `ARCHIVE_FAILED`, which is not fatal to libarchive but is
        // fatal to honesty: silently dropping the level the user chose would build a
        // different archive from the one the popup promised.
        if let Some(opts) = write_options(recipe) {
            let copts = CString::new(opts.clone())
                .map_err(|_| "compression options contained a NUL".to_string())?;
            // SAFETY: `self.raw` is live and `copts` outlives the call.
            let rc = unsafe { archive_write_set_options(self.raw, copts.as_ptr()) };
            if rc == ARCHIVE_FAILED {
                return Err(format!(
                    "libarchive refused the compression options ({opts})"
                ));
            }
            self.check(rc, "set the compression options")?;
        }
        Ok(())
    }

    fn check(&self, rc: c_int, what: &str) -> Result<(), String> {
        if rc == ARCHIVE_OK || rc == ARCHIVE_WARN {
            return Ok(());
        }
        let msg = last_error(self.raw);
        if msg.is_empty() {
            Err(format!("could not {what}"))
        } else {
            Err(format!("could not {what}: {msg}"))
        }
    }
}

/// The `archive_write_set_options` string for a recipe, or `None` where a level would
/// mean nothing.
///
/// Ranges are `archive_write_set_options(3)`'s, and the level is clamped into them
/// before it gets here. zip's level `0` is deliberately not special-cased away: the
/// manual says it switches the method to "store", which is what the user asked for.
fn write_options(recipe: &Recipe) -> Option<String> {
    match recipe.method {
        Method::Store => None,
        Method::Deflate => Some(format!(
            "zip:compression=deflate,zip:compression-level={}",
            recipe.method.clamp_level(recipe.level)
        )),
        Method::Gzip | Method::Bzip2 | Method::Xz | Method::Zstd | Method::Lz4 => {
            let filter = match recipe.method {
                Method::Gzip => "gzip",
                Method::Bzip2 => "bzip2",
                Method::Xz => "xz",
                Method::Zstd => "zstd",
                Method::Lz4 => "lz4",
                _ => unreachable!(),
            };
            Some(format!(
                "{filter}:compression-level={}",
                recipe.method.clamp_level(recipe.level)
            ))
        }
        Method::Lzma2 => None,
    }
}

impl Sink for Writer {
    fn put(&mut self, meta: &Meta, data: Option<&mut dyn std::io::Read>) -> Result<(), String> {
        let cpath = CString::new(meta.out_path.as_bytes())
            .map_err(|_| format!("{} contains a NUL byte", meta.out_path))?;

        // Held until after `archive_write_header`, because libarchive copies strings out
        // of them during that call and not before.
        let cuname = meta.uname.as_deref().and_then(|s| CString::new(s).ok());
        let cgname = meta.gname.as_deref().and_then(|s| CString::new(s).ok());
        let csymlink = meta.symlink.as_deref().and_then(|s| CString::new(s).ok());
        let chardlink = meta.hardlink.as_deref().and_then(|s| CString::new(s).ok());

        let filetype = if meta.is_dir {
            AE_IFDIR
        } else if meta.symlink.is_some() {
            AE_IFLNK
        } else {
            AE_IFREG
        };

        // A hardlink and a symlink carry no data of their own, and a directory has none;
        // giving libarchive a size for them would make it expect bytes that never come.
        let size = if meta.is_dir || meta.symlink.is_some() || meta.hardlink.is_some() {
            0
        } else {
            meta.size as i64
        };

        // SAFETY: `self.entry` is live for the writer's lifetime, and every `CString`
        // above outlives the header call below.
        unsafe {
            archive_entry_clear(self.entry);
            // `_utf8`, and for the mirror of `entry_name`'s reason. A Rust `String` is
            // UTF-8; handing those bytes to the locale setter in a process that never
            // called `setlocale` tells libarchive they are ASCII, so a zip gets the bytes
            // with its UTF-8 flag left clear and every other tool reads them as CP437.
            // Naming the encoding writes the flag and makes the name mean the same thing
            // to a reader that is not INDIUM.
            archive_entry_set_pathname_utf8(self.entry, cpath.as_ptr());
            archive_entry_set_filetype(self.entry, filetype);
            archive_entry_set_perm(self.entry, meta.mode & 0o7777);
            archive_entry_set_size(self.entry, size);
            archive_entry_set_uid(self.entry, meta.uid);
            archive_entry_set_gid(self.entry, meta.gid);
            if let Some(n) = cuname.as_ref() {
                archive_entry_set_uname(self.entry, n.as_ptr());
            }
            if let Some(n) = cgname.as_ref() {
                archive_entry_set_gname(self.entry, n.as_ptr());
            }
            if let Some(t) = csymlink.as_ref() {
                archive_entry_set_symlink_utf8(self.entry, t.as_ptr());
            }
            if let Some(t) = chardlink.as_ref() {
                archive_entry_set_hardlink_utf8(self.entry, t.as_ptr());
            }
            // Only set a timestamp that was actually read. An entry whose mtime the
            // source never recorded must not acquire one here.
            if let Some(t) = meta.mtime {
                archive_entry_set_mtime(self.entry, t, 0);
            }
            if let Some(t) = meta.atime {
                archive_entry_set_atime(self.entry, t, 0);
            }
            if let Some(t) = meta.ctime {
                archive_entry_set_ctime(self.entry, t, 0);
            }

            let rc = archive_write_header(self.raw, self.entry);
            self.check(rc, &format!("write the header for {}", meta.out_path))?;
        }

        if let Some(reader) = data {
            let mut buf = vec![0u8; 65536];
            loop {
                let n = reader
                    .read(&mut buf)
                    .map_err(|e| format!("could not read {}: {e}", meta.out_path))?;
                if n == 0 {
                    break;
                }
                // SAFETY: `buf[..n]` is initialised and `self.raw` is live.
                let written =
                    unsafe { archive_write_data(self.raw, buf.as_ptr() as *const c_void, n) };
                // Negative means error. Zero does **not**: libarchive 3.x sometimes
                // answers a successful write with zero, so `!= n` is the wrong test.
                if written < 0 {
                    let msg = last_error(self.raw);
                    return Err(format!("could not write {}: {msg}", meta.out_path));
                }
            }
        }

        // SAFETY: `self.raw` is live.
        let rc = unsafe { archive_write_finish_entry(self.raw) };
        self.check(rc, &format!("finish {}", meta.out_path))
    }

    fn finish(&mut self) -> Result<(), String> {
        if self.finished {
            return Ok(());
        }
        self.finished = true;
        // SAFETY: `self.raw` is live and has not been closed.
        let rc = unsafe { archive_write_close(self.raw) };
        self.check(rc, "close the archive")
    }

    fn abandon(&mut self) {
        if self.finished {
            return;
        }
        self.finished = true;
        // Marking the handle fatal is what stops the drop below from flushing a partial
        // archive out to the temp file on the way past.
        // SAFETY: `self.raw` is live.
        unsafe {
            archive_write_fail(self.raw);
        }
    }
}

impl Drop for Writer {
    fn drop(&mut self) {
        // SAFETY: both pointers were allocated by libarchive and are freed once. `free`
        // implicitly closes an open handle, which is why `finish` is called explicitly
        // elsewhere — an error that surfaces only here would have nowhere to go.
        unsafe {
            if !self.entry.is_null() {
                archive_entry_free(self.entry);
            }
            if !self.finished {
                archive_write_fail(self.raw);
            }
            archive_write_free(self.raw);
        }
    }
}

/// The current entry's data, as a `Read`.
///
/// Streams straight out of libarchive so a rebuild never holds a member in memory.
/// `archive_read_data` is used rather than `archive_read_data_block` because it fills a
/// caller buffer in one call and materialises sparse holes as zeros — which is what a
/// copy into a new archive wants.
pub struct EntryData<'r> {
    reader: &'r mut Reader,
}

impl<'r> EntryData<'r> {
    pub fn new(reader: &'r mut Reader) -> EntryData<'r> {
        EntryData { reader }
    }
}

impl std::io::Read for EntryData<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        // SAFETY: the reader is live and positioned on an entry whose header has been
        // read; `buf` is a valid writable slice of `buf.len()` bytes.
        let n = unsafe {
            archive_read_data(self.reader.raw, buf.as_mut_ptr() as *mut c_void, buf.len())
        };
        if n < 0 {
            let msg = last_error(self.reader.raw);
            return Err(std::io::Error::other(msg));
        }
        Ok(n as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_labels_prefer_the_filter() {
        assert_eq!(method_label("GNU tar format", "gzip"), "gzip");
        assert_eq!(method_label("POSIX ustar format", "zstd"), "zstd");
        assert_eq!(method_label("GNU tar format", "xz"), "xz");
    }

    #[test]
    fn method_labels_read_zips_parenthetical() {
        assert_eq!(method_label("ZIP 2.0 (deflation)", "none"), "deflate");
        assert_eq!(method_label("ZIP 1.0 (uncompressed)", "none"), "store");
    }

    #[test]
    fn method_labels_name_7z_without_guessing_its_detail() {
        // Per-entry 7z method is P4's job; until then the generic reader says "7z".
        assert_eq!(method_label("7-Zip", "none"), "7z");
    }

    #[test]
    fn method_labels_call_plain_containers_store() {
        assert_eq!(method_label("POSIX ustar format", "none"), "store");
        assert_eq!(method_label("POSIX cpio", "none"), "store");
        assert_eq!(method_label("", ""), "—");
    }

    #[test]
    fn selection_takes_directory_children() {
        let mut wanted = HashSet::new();
        wanted.insert("sub".to_string());
        assert!(selection_matches("sub", &wanted));
        assert!(selection_matches("sub/gamma.txt", &wanted));
        assert!(selection_matches("sub/deep/x", &wanted));
        assert!(
            !selection_matches("subtle.txt", &wanted),
            "prefix must stop at a slash"
        );
        assert!(!selection_matches("other.txt", &wanted));
    }

    #[test]
    fn selection_of_a_plain_file_takes_only_that_file() {
        let mut wanted = HashSet::new();
        wanted.insert("alpha.txt".to_string());
        assert!(selection_matches("alpha.txt", &wanted));
        assert!(!selection_matches("alpha.txt.bak", &wanted));
    }

    #[test]
    fn traversal_and_absolute_paths_are_rejected() {
        assert!(path_escapes("../escape.txt"));
        assert!(path_escapes("a/../../escape.txt"));
        assert!(path_escapes("a/b/.."));
        assert!(path_escapes("/etc/passwd"));
        assert!(
            path_escapes("..\\escape.txt"),
            "backslashes must not smuggle a .."
        );
    }

    #[test]
    fn ordinary_paths_are_allowed() {
        assert!(!path_escapes("alpha.txt"));
        assert!(!path_escapes("sub/gamma.txt"));
        assert!(!path_escapes("a/b/c.txt"));
        // A filename that merely starts with dots is not a traversal.
        assert!(!path_escapes("..hidden.txt"));
        assert!(!path_escapes("sub/...weird"));
    }

    #[test]
    fn passphrase_prose_is_recognised() {
        assert!(mentions_passphrase("Passphrase required for this entry"));
        assert!(mentions_passphrase("Incorrect passphrase"));
        assert!(!mentions_passphrase("Truncated tar archive"));
    }
}
