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
use std::os::raw::{c_char, c_int, c_void};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;

use crate::secret::Secret;
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
    fn archive_entry_is_encrypted(e: *mut ArchiveEntry) -> c_int;
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
            let raw_path = cstr_to_string(archive_entry_pathname(ep)).unwrap_or_default();
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
                symlink: cstr_to_string(archive_entry_symlink(ep)).filter(|s| !s.is_empty()),
                hardlink: cstr_to_string(archive_entry_hardlink(ep)).filter(|s| !s.is_empty()),
                encrypted: archive_entry_is_encrypted(ep) != 0,
            }
        };

        Ok(Some(entry))
    }

    fn skip_data(&mut self) {
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
    let mut reader = Reader::open(path, passphrase)?;
    let mut out = Vec::new();
    while let Some(e) = reader.next_entry()? {
        out.push(e);
        reader.skip_data();
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Extraction
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum ExtractMsg {
    Progress { done: usize, total: usize },
    Done { written: usize },
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

    for entry in &selected {
        if path_escapes(&entry.raw_path) {
            return Err(ArchiveError::UnsafePath(entry.raw_path.clone()));
        }
    }

    if selected.iter().any(|e| e.encrypted) {
        match passphrase {
            None => return Err(ArchiveError::NeedPassword),
            Some(secret) => {
                if !verify_passphrase(path, secret)? {
                    return Err(ArchiveError::WrongPassword);
                }
            }
        }
    }
    // ---- Pre-flight over. ----

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

/// Stream one entry through the hand-written CRC32.
///
/// CORE §4: libarchive does not expose an entry's *stored* CRC, so INDIUM computes it
/// on demand and the Inspector labels the value *computed*.
pub fn crc32_of(
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
