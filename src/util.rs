//! Hand-written helpers.
//!
//! CORE §2: "CRC32 is a twenty-line table, byte formatting is ten lines." Nothing in
//! this file may grow a dependency; if a helper here ever needs a crate, it belongs
//! somewhere else or not at all.

// ---------------------------------------------------------------------------
// CRC32 (IEEE, the polynomial zip and gzip use)
// ---------------------------------------------------------------------------

const CRC32_POLY: u32 = 0xEDB8_8320;

const fn make_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut i = 0usize;
    while i < 256 {
        let mut c = i as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 {
                CRC32_POLY ^ (c >> 1)
            } else {
                c >> 1
            };
            k += 1;
        }
        table[i] = c;
        i += 1;
    }
    table
}

static CRC32_TABLE: [u32; 256] = make_table();

/// A streaming CRC32, so an entry can be checksummed block by block without ever
/// holding the whole thing in memory.
#[derive(Debug, Clone)]
pub struct Crc32 {
    state: u32,
}

impl Default for Crc32 {
    fn default() -> Self {
        Self::new()
    }
}

impl Crc32 {
    pub fn new() -> Self {
        Crc32 { state: 0xFFFF_FFFF }
    }

    pub fn update(&mut self, bytes: &[u8]) {
        let mut c = self.state;
        for &b in bytes {
            c = CRC32_TABLE[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8);
        }
        self.state = c;
    }

    pub fn finish(&self) -> u32 {
        self.state ^ 0xFFFF_FFFF
    }
}

/// One-shot CRC32 over a slice.
pub fn crc32(bytes: &[u8]) -> u32 {
    let mut c = Crc32::new();
    c.update(bytes);
    c.finish()
}

// ---------------------------------------------------------------------------
// Byte formatting
// ---------------------------------------------------------------------------

/// Human sizes in binary units. Exact for bytes, one decimal above that.
pub fn format_bytes(n: u64) -> String {
    const UNITS: [&str; 7] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB"];
    if n < 1024 {
        return format!("{n} B");
    }
    let mut value = n as f64;
    let mut unit = 0usize;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    format!("{:.1} {}", value, UNITS[unit])
}

/// The exact byte count with thousands separators, for the Inspector, which is
/// verbose on purpose and should never make you guess.
pub fn format_exact_bytes(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    let bytes = s.as_bytes();
    for (i, ch) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i).is_multiple_of(3) {
            out.push(' ');
        }
        out.push(*ch as char);
    }
    out
}

/// Compression ratio as a percentage of the original. Returns None when there is
/// nothing honest to say — a zero-byte original has no ratio.
pub fn ratio(real: u64, packed: u64) -> Option<f32> {
    if real == 0 {
        return None;
    }
    Some(packed as f32 / real as f32)
}

pub fn format_ratio(real: u64, packed: u64) -> String {
    match ratio(real, packed) {
        Some(r) => format!("{:.1}%", r * 100.0),
        None => "—".to_string(),
    }
}

// ---------------------------------------------------------------------------
// Hex
// ---------------------------------------------------------------------------

/// Bytes on a row of the hex view, and it does not depend on how wide the pane is.
///
/// **Fixed, deliberately.** The obvious thing is to fit the row to the Inspector's width, and
/// P13 spent an afternoon on exactly that idea in the sidebar's header before rolling it back:
/// a layout chosen by how much room a zone has means a first launch and a one-pixel drag can
/// show two different pictures of the same bytes, and here the picture is the data. Sixteen is
/// also what `xxd`, `hexdump` and every other tool prints, so an offset read in this pane names
/// the same byte as an offset read anywhere else. The pane scrolls sideways instead.
pub const HEX_COLUMNS: usize = 16;

/// The offset column: eight uppercase hex digits.
///
/// Uppercase because the Inspector already prints a CRC as `{v:08X}` and two conventions for
/// hex in one pane is one too many. Eight digits is two more than this can ever need — the
/// preview cap is 8 MiB, so the last row of the largest possible dump is `007FFFF0` — and it
/// is eight anyway, because that is the width the tools a reader is comparing against use.
pub fn hex_offset(offset: usize) -> String {
    format!("{offset:08X}")
}

/// One row's bytes, then the same bytes as printable characters.
///
/// `chunk` is up to [`HEX_COLUMNS`] bytes; a short one is the last row of the dump. **The hex
/// columns are padded to the full width when it is short**, or the gutter slides left on the
/// final row and stops being a column — which is the whole failure this function has to avoid,
/// and the case a naive implementation gets wrong.
///
/// A byte outside `0x20..=0x7E` is drawn as `.`, which means a literal full stop and an
/// unprintable byte look the same. That is what every hex dump has always done, and the hex
/// columns are the truth beside it: the gutter is for finding your place, not for reading.
pub fn hex_body(chunk: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(HEX_COLUMNS * 4 + 8);
    for i in 0..HEX_COLUMNS {
        match chunk.get(i) {
            Some(b) => {
                out.push(HEX[(b >> 4) as usize] as char);
                out.push(HEX[(b & 0x0F) as usize] as char);
            }
            None => out.push_str("  "),
        }
        out.push(' ');
        // The wider gap down the middle, so an eye can count to eight without counting.
        if i == HEX_COLUMNS / 2 - 1 {
            out.push(' ');
        }
    }
    out.push_str(" |");
    for b in chunk {
        out.push(if (0x20..=0x7E).contains(b) {
            *b as char
        } else {
            '.'
        });
    }
    out.push('|');
    out
}

/// How many rows a buffer of this length becomes, the short final one included.
///
/// One line, and it lives here rather than at the call site for the sake of the test that
/// pins it: written inline in the view, the only thing a test could reach was
/// `usize::div_ceil`, and asserting the standard library against a table of literals would
/// have stayed green through a plain division that dropped the last row of every file.
pub fn hex_rows(len: usize) -> usize {
    len.div_ceil(HEX_COLUMNS)
}

// ---------------------------------------------------------------------------
// Mode formatting
// ---------------------------------------------------------------------------

/// `-rw-r--r-- 644`, as CORE §4 specifies for the Inspector.
pub fn format_mode(mode: u32, filetype: u32) -> String {
    let kind = match filetype & 0o170000 {
        0o040000 => 'd',
        0o120000 => 'l',
        0o100000 => '-',
        0o060000 => 'b',
        0o020000 => 'c',
        0o010000 => 'p',
        0o140000 => 's',
        _ => '?',
    };
    let perm = mode & 0o7777;
    let mut s = String::with_capacity(14);
    s.push(kind);
    for shift in [6, 3, 0] {
        let bits = (perm >> shift) & 0o7;
        s.push(if bits & 0o4 != 0 { 'r' } else { '-' });
        s.push(if bits & 0o2 != 0 { 'w' } else { '-' });
        s.push(if bits & 0o1 != 0 { 'x' } else { '-' });
    }
    // setuid / setgid / sticky, applied over the execute column
    if perm & 0o4000 != 0 {
        let c = if perm & 0o100 != 0 { 's' } else { 'S' };
        s.replace_range(3..4, &c.to_string());
    }
    if perm & 0o2000 != 0 {
        let c = if perm & 0o010 != 0 { 's' } else { 'S' };
        s.replace_range(6..7, &c.to_string());
    }
    if perm & 0o1000 != 0 {
        let c = if perm & 0o001 != 0 { 't' } else { 'T' };
        s.replace_range(9..10, &c.to_string());
    }
    format!("{s} {:03o}", perm & 0o777)
}

// ---------------------------------------------------------------------------
// Timestamps
// ---------------------------------------------------------------------------

/// Civil date from a unix timestamp, by Howard Hinnant's `civil_from_days`.
/// Hand-written because a date crate cannot fill in its CORE §2 sentence.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// `2001-02-03 04:05:06` in UTC. INDIUM shows UTC and says so, rather than
/// pretending to a timezone database it does not carry.
pub fn format_timestamp(unix: i64) -> String {
    let days = unix.div_euclid(86_400);
    let secs = unix.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    let (hh, mm, ss) = (secs / 3600, (secs % 3600) / 60, secs % 60);
    format!("{y:04}-{m:02}-{d:02} {hh:02}:{mm:02}:{ss:02} UTC")
}

// ---------------------------------------------------------------------------
// Paths inside an archive
// ---------------------------------------------------------------------------

/// Normalise a stored archive path for display and tree-building: strip a leading
/// `./`, collapse repeated slashes, drop a trailing slash. The result is never
/// trusted as a filesystem path — extraction hands the original to libarchive and
/// lets its secure flags do the judging.
pub fn normalize_archive_path(raw: &str) -> String {
    let mut s = raw.replace('\\', "/");
    while let Some(rest) = s.strip_prefix("./") {
        s = rest.to_string();
    }
    while s.contains("//") {
        s = s.replace("//", "/");
    }
    let trimmed = s.trim_end_matches('/');
    trimmed.to_string()
}

/// The last component of an archive path.
pub fn base_name(path: &str) -> &str {
    match path.rsplit_once('/') {
        Some((_, name)) => name,
        None => path,
    }
}

/// The directory portion of an archive path, `""` for a top-level entry.
pub fn parent_dir(path: &str) -> &str {
    match path.rsplit_once('/') {
        Some((dir, _)) => dir,
        None => "",
    }
}

/// The one thing worth checking before a writer opens its file: that the folder it is being
/// asked to write into is there.
///
/// **PXX 9.5.** Naming a destination folder that does not exist reported
/// `could not open the 7z for writing: Io(Os { code: 2, kind: NotFound, … },
/// "…/large/.archivesadfad.7z.indium-new")` — three faults in one line. It printed a Rust
/// struct at a person; it named `.indium-new`, an internal temp file nobody asked for and
/// nobody can act on; and it never mentioned the one fact that would have fixed it, which is
/// that the folder is not there.
///
/// It lives here rather than in either writer because **both writers have the fault and only
/// one of them was reported.** The walk built a `.7z`, so that is the branch the maker saw; a
/// `.tar.gz` into the same missing folder went to libarchive instead and came back
/// `could not open the archive for writing: Failed to open '…/.archive.tar.gz.indium-new'`
/// — no Rust struct, and no better an answer, still naming a file the person never chose and
/// still not saying the folder is missing. A round that freezes the repository forever does
/// not get to fix the format that was demonstrated and leave the other six.
///
/// What it deliberately does not do is check whether the folder is *writable*. That is a
/// question only the open can answer honestly — permissions, mounts and ACLs all get a vote —
/// and a pre-flight that guessed at it would either refuse a write that would have worked or
/// pass one that then failed with a second, worse sentence.
pub fn writable_parent(path: &std::path::Path) -> Result<(), String> {
    match path.parent() {
        // A bare filename has no parent to check; `Path::parent` also yields `""` for one,
        // which is the current directory and always exists.
        Some(dir) if !dir.as_os_str().is_empty() && !dir.is_dir() => Err(format!(
            "{} does not exist, so there is nowhere to write the archive",
            dir.display()
        )),
        _ => Ok(()),
    }
}

/// Shorten `s` to at most `cells` columns by taking the middle out, not the end.
///
/// CORE §4: *"The directory is elided in the middle, never at the end, because the end is
/// the folder the archive is actually in and the start is the tree it belongs to; a path
/// that keeps only one of those has kept the wrong half."* egui's own `.truncate()` cuts
/// the tail, which on `/home/megas/Downloads/2026/archives/holiday` throws away the only
/// word that identifies it.
///
/// **Counts `chars`, never bytes.** The corpus this program is tested against is Turkish,
/// where `ş` and `ğ` are two bytes each, and a byte-slicing version of this function would
/// panic on the first one.
///
/// The tail gets the odd column when the budget is odd: between the tree and the leaf, the
/// leaf is what a person is looking for.
pub fn elide_middle(s: &str, cells: usize) -> String {
    let n = s.chars().count();
    if n <= cells {
        return s.to_string();
    }
    match cells {
        0 => String::new(),
        1 => "…".to_string(),
        _ => {
            let keep = cells - 1;
            // **The tail always takes the odd column.** `keep / 2` alternates which half
            // absorbs it as the budget changes, so widening a window by one column used to
            // re-partition *both* ends and the ellipsis visibly swung back and forth. Fixing
            // the share means one more character appears at one end and nothing else moves.
            let head = keep / 2;
            let tail = keep - head;
            let mut out = String::with_capacity(s.len());
            out.extend(s.chars().take(head));
            out.push('…');
            out.extend(s.chars().skip(n - tail));
            out
        }
    }
}

// ---------------------------------------------------------------------------
// What a preview is looking at — P5 §B2
// ---------------------------------------------------------------------------

/// What INDIUM can make of an entry's first bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Content {
    /// An image in a format the Preview tab decodes.
    Image(&'static str),
    /// Text: no NUL byte, and valid UTF-8.
    Text,
    /// Something else, which the Preview tab draws as hex. This variant read "CORE §4
    /// reserves hex for V1.1, so the honest answer is a sentence" until P16 built it, and
    /// was the twin of a comment in `inspector.rs` that the same commit deleted — a pair
    /// split across two files, one half updated. That is the class of defect P13 left, P14
    /// reintroduced and P16 found twice more, so it is worth naming where it happened.
    Binary,
    /// No bytes at all.
    Empty,
}

/// Judge an entry's head.
///
/// The order matters. An image is recognised **by its bytes, never its extension**: a PNG
/// named `notes.txt` is a PNG, and reporting what is actually there rather than what a
/// name claims is the same principle that makes the Inspector worth having.
pub fn sniff(head: &[u8]) -> Content {
    if head.is_empty() {
        return Content::Empty;
    }
    if let Some(kind) = image_format(head) {
        return Content::Image(kind);
    }
    // A NUL byte is the classic tell, and it is what `grep` and `file` have used
    // forever. UTF-8 validity is checked rather than assumed, because a preview that
    // lossily decoded arbitrary bytes into replacement characters would be inventing
    // content — the one thing this program does not do.
    if !head.contains(&0) && std::str::from_utf8(head).is_ok() {
        return Content::Text;
    }
    Content::Binary
}

/// The image format a head's magic bytes announce, if it is one INDIUM decodes.
///
/// Only the four formats CORE §2's row now covers. A format that is recognised here but
/// not compiled in would produce a decoder error instead of an honest sentence, so this
/// list and `Cargo.toml`'s feature list are the same list.
pub fn image_format(head: &[u8]) -> Option<&'static str> {
    const PNG: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    if head.starts_with(PNG) {
        return Some("PNG");
    }
    if head.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some("JPEG");
    }
    if head.starts_with(b"GIF87a") || head.starts_with(b"GIF89a") {
        return Some("GIF");
    }
    // BMP's magic is only two bytes, so the declared file size is checked too — "BM"
    // alone matches far too much to trust on its own.
    if head.starts_with(b"BM") && head.len() >= 6 {
        return Some("BMP");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_matches_known_vectors() {
        // The canonical check value for the IEEE polynomial.
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b""), 0x0000_0000);
        assert_eq!(crc32(b"a"), 0xE8B7_BE43);
    }

    #[test]
    fn crc32_streams_the_same_as_one_shot() {
        let data: Vec<u8> = (0u8..=255).cycle().take(5000).collect();
        let mut streamed = Crc32::new();
        for chunk in data.chunks(97) {
            streamed.update(chunk);
        }
        assert_eq!(streamed.finish(), crc32(&data));
    }

    #[test]
    fn bytes_format_readably() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(1023), "1023 B");
        assert_eq!(format_bytes(1024), "1.0 KiB");
        assert_eq!(format_bytes(1536), "1.5 KiB");
        assert_eq!(format_bytes(1024 * 1024), "1.0 MiB");
    }

    #[test]
    fn exact_bytes_group_by_threes() {
        assert_eq!(format_exact_bytes(0), "0");
        assert_eq!(format_exact_bytes(999), "999");
        assert_eq!(format_exact_bytes(1000), "1 000");
        assert_eq!(format_exact_bytes(1234567), "1 234 567");
    }

    #[test]
    fn modes_render_like_ls() {
        assert_eq!(format_mode(0o644, 0o100000), "-rw-r--r-- 644");
        assert_eq!(format_mode(0o755, 0o040000), "drwxr-xr-x 755");
        assert_eq!(format_mode(0o777, 0o120000), "lrwxrwxrwx 777");
        assert_eq!(format_mode(0o4755, 0o100000), "-rwsr-xr-x 755");
        assert_eq!(format_mode(0o1777, 0o040000), "drwxrwxrwt 777");
    }

    #[test]
    fn timestamps_render_in_utc() {
        assert_eq!(format_timestamp(0), "1970-01-01 00:00:00 UTC");
        assert_eq!(format_timestamp(981_173_106), "2001-02-03 04:05:06 UTC");
        // A pre-epoch stamp must not panic or wrap.
        assert_eq!(format_timestamp(-1), "1969-12-31 23:59:59 UTC");
    }

    #[test]
    fn archive_paths_normalize() {
        assert_eq!(normalize_archive_path("./a/b.txt"), "a/b.txt");
        assert_eq!(normalize_archive_path("a//b/"), "a/b");
        assert_eq!(normalize_archive_path("sub/"), "sub");
        // A traversal name is preserved verbatim: display must not launder it.
        assert_eq!(normalize_archive_path("../escape.txt"), "../escape.txt");
    }

    #[test]
    fn path_components_split() {
        assert_eq!(base_name("a/b/c.txt"), "c.txt");
        assert_eq!(base_name("c.txt"), "c.txt");
        assert_eq!(parent_dir("a/b/c.txt"), "a/b");
        assert_eq!(parent_dir("c.txt"), "");
    }

    #[test]
    fn ratios_refuse_to_divide_by_zero() {
        assert_eq!(format_ratio(0, 0), "—");
        assert_eq!(format_ratio(100, 50), "50.0%");
    }

    // --- Preview sniffing — P5 §B2 -------------------------------------------

    /// The whole point: bytes decide, not names. A PNG called `notes.txt` is a PNG.
    #[test]
    fn an_image_is_recognised_by_its_bytes_not_its_extension() {
        let png = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13];
        assert_eq!(sniff(&png), Content::Image("PNG"));
        assert_eq!(image_format(&png), Some("PNG"));

        assert_eq!(
            sniff(&[0xFF, 0xD8, 0xFF, 0xE0, 0, 16]),
            Content::Image("JPEG")
        );
        assert_eq!(sniff(b"GIF89a\x01\x00"), Content::Image("GIF"));
        assert_eq!(sniff(b"BM\x8a\x00\x00\x00"), Content::Image("BMP"));
    }

    /// A NUL byte is the tell `file` and `grep` have used forever.
    #[test]
    fn a_file_with_a_nul_byte_is_not_offered_as_text() {
        assert_eq!(sniff(b"hello\0world"), Content::Binary);
        assert_eq!(sniff(b"hello world"), Content::Text);
    }

    /// Lossily decoding arbitrary bytes into replacement characters would be inventing
    /// content, which is the one thing this program refuses to do.
    #[test]
    fn non_utf8_bytes_are_reported_rather_than_lossily_decoded() {
        // A lone continuation byte: valid Latin-1, invalid UTF-8.
        assert_eq!(sniff(&[b'a', 0xC3, 0x28, b'b']), Content::Binary);
        // Valid multi-byte UTF-8 is text.
        assert_eq!(sniff("ödev".as_bytes()), Content::Text);
    }

    #[test]
    fn an_empty_entry_is_neither_text_nor_binary() {
        assert_eq!(sniff(b""), Content::Empty);
    }

    /// "BM" alone matches far too much to trust, so a BMP needs its size field too.
    #[test]
    fn two_bytes_are_not_enough_to_call_something_a_bitmap() {
        assert_eq!(image_format(b"BM"), None);
        assert_eq!(
            sniff(b"BM"),
            Content::Text,
            "it is printable, so it reads as text"
        );
    }

    // -----------------------------------------------------------------------
    // elide_middle — CORE §4's "never at the end"
    // -----------------------------------------------------------------------

    const LONG: &str = "/home/megas/Downloads/2026/archives/holiday-crete";

    #[test]
    fn a_path_that_fits_is_not_touched() {
        assert_eq!(elide_middle(LONG, LONG.chars().count()), LONG);
        assert_eq!(elide_middle(LONG, 500), LONG);
        assert_eq!(elide_middle("", 0), "");
    }

    #[test]
    fn an_elided_path_never_exceeds_its_budget() {
        for cells in 0..=60 {
            let out = elide_middle(LONG, cells);
            assert!(
                out.chars().count() <= cells,
                "budget {cells} produced {} columns: {out}",
                out.chars().count()
            );
        }
    }

    #[test]
    fn both_ends_survive_the_elision() {
        let out = elide_middle(LONG, 24);
        assert!(out.starts_with('/'), "the root went missing: {out}");
        assert!(
            out.ends_with("crete"),
            "the leaf went missing, which is the half worth keeping: {out}"
        );
        assert!(out.contains('…'));
    }

    /// The reason this counts `chars` and not bytes. Every one of these is two bytes, so a
    /// byte-slicing implementation panics somewhere in this string rather than returning
    /// something merely ugly.
    #[test]
    fn a_turkish_path_elides_without_panicking() {
        let turkish = "/home/megas/belgeler/açık-şeyler/AŞÇALIKĞA-yedek";
        for cells in 0..=turkish.chars().count() + 5 {
            let out = elide_middle(turkish, cells);
            assert!(out.chars().count() <= cells);
        }
        assert!(elide_middle(turkish, 20).ends_with("yedek"));
    }

    /// A lane too narrow for anything still has to return something drawable.
    #[test]
    fn an_impossible_budget_degrades_rather_than_panics() {
        assert_eq!(elide_middle(LONG, 0), "");
        assert_eq!(elide_middle(LONG, 1), "…");
        assert_eq!(elide_middle(LONG, 2).chars().count(), 2);
    }

    // --- the hex view ---------------------------------------------------

    /// The whole row, written out, so a change to the shape has to be deliberate.
    #[test]
    fn a_full_row_reads_exactly_as_a_hex_dump_should() {
        let bytes: Vec<u8> = (0u8..16).collect();
        assert_eq!(
            hex_body(&bytes),
            "00 01 02 03 04 05 06 07  08 09 0A 0B 0C 0D 0E 0F  |................|"
        );
    }

    /// **The case a naive implementation gets wrong.** A short last row must pad its hex
    /// columns, or the gutter walks left and stops being a column.
    #[test]
    fn a_short_last_row_keeps_the_gutter_in_its_lane() {
        let full = hex_body(&(0u8..16).collect::<Vec<u8>>());
        let lane = full.find('|').expect("a full row has a gutter");
        for n in 0..=HEX_COLUMNS {
            let short = hex_body(&vec![b'A'; n]);
            assert_eq!(
                short.find('|'),
                Some(lane),
                "a row of {n} bytes puts its gutter in a different column"
            );
        }
    }

    /// The gutter is printable ASCII and nothing else — a stray control character in it
    /// would move the cursor and take the column with it.
    #[test]
    fn the_gutter_prints_only_what_can_be_printed() {
        let all: Vec<u8> = (0u8..=255).collect();
        for chunk in all.chunks(HEX_COLUMNS) {
            let row = hex_body(chunk);
            let gutter = &row[row.find('|').unwrap() + 1..row.len() - 1];
            for (i, ch) in gutter.chars().enumerate() {
                let raw = chunk[i];
                if (0x20..=0x7E).contains(&raw) {
                    assert_eq!(ch, raw as char);
                } else {
                    assert_eq!(ch, '.', "byte {raw:#04X} was not substituted");
                }
            }
        }
    }

    /// **Both nibbles, for every byte there is.** The exact-row test above feeds `0x00..0x0F`,
    /// whose high nibble is nought — so on its own it stays green if the high nibble is
    /// shifted by five instead of four, and every byte over `0x0F` renders wrong. Checked
    /// against `{:02X}`, which is a different implementation and not this one restated.
    #[test]
    fn both_nibbles_of_every_byte_are_written() {
        for b in 0u8..=255 {
            let row = hex_body(&[b]);
            assert_eq!(
                &row[..2],
                format!("{b:02X}"),
                "byte {b:#04X} came out wrong"
            );
        }
        // And in a full row, where the columns have to stay in line as well.
        let high: Vec<u8> = (0xF0u8..=0xFF).collect();
        assert_eq!(
            hex_body(&high),
            "F0 F1 F2 F3 F4 F5 F6 F7  F8 F9 FA FB FC FD FE FF  |................|"
        );
    }

    /// The invariant this test had the wrong name for until P16: rows are **not** all the
    /// same width — the gutter is only as long as the row has bytes — but everything to the
    /// left of it is, which is the half that has to line up.
    #[test]
    fn the_hex_columns_hold_their_width_and_the_gutter_does_not() {
        let full = hex_body(&(0u8..16).collect::<Vec<u8>>()).chars().count();
        for n in 0..=HEX_COLUMNS {
            let row = hex_body(&vec![0xFFu8; n]);
            assert_eq!(row.chars().count(), full - (HEX_COLUMNS - n));
        }
    }

    #[test]
    fn an_offset_is_eight_uppercase_digits() {
        assert_eq!(hex_offset(0), "00000000");
        assert_eq!(hex_offset(16), "00000010");
        assert_eq!(hex_offset(0xABCDEF), "00ABCDEF");
        // The last row of the largest dream the preview cap allows.
        assert_eq!(hex_offset(8 * 1024 * 1024 - HEX_COLUMNS), "007FFFF0");
        assert!(hex_offset(0xABCDEF).chars().all(|c| !c.is_lowercase()));
    }

    /// The row count the view hands `show_rows`, including the partial one at the end.
    #[test]
    fn the_row_count_covers_every_byte() {
        let cases: [(usize, usize); 7] =
            [(0, 0), (1, 1), (15, 1), (16, 1), (17, 2), (32, 2), (33, 3)];
        for (len, want) in cases {
            assert_eq!(hex_rows(len), want, "for {len} bytes");
        }
        // The property the table is only a sample of: every byte is on a row, and no row is
        // conjured for bytes that are not there.
        for len in 0..200usize {
            assert!(hex_rows(len) * HEX_COLUMNS >= len, "{len} bytes lost a row");
            assert!(
                len == 0 || (hex_rows(len) - 1) * HEX_COLUMNS < len,
                "{len} bytes grew an empty row"
            );
        }
    }

    /// PXX 9.5. The three shapes a destination path comes in, and what each is owed.
    ///
    /// The sentence itself is asserted end-to-end by `write_path.rs`'s
    /// `a_missing_destination_folder_is_named_plainly`, once per writable container. What is
    /// checked here is the part that is easy to get subtly wrong: which paths have a parent
    /// worth checking at all. `Path::parent` answers `Some("")` for a bare filename, and an
    /// empty path is not a directory, so a naive check refuses `archive.7z` — a perfectly
    /// ordinary thing to type — on the grounds that the current directory does not exist.
    #[test]
    fn only_a_parent_that_is_named_and_missing_is_refused() {
        use std::path::Path;

        // A real folder, checked against the one directory every run is guaranteed to have.
        assert!(writable_parent(Path::new("/tmp/archive.7z")).is_ok());
        // No parent to speak of, and `Some("")` for a parent, are both the current
        // directory. Neither may be refused.
        assert!(writable_parent(Path::new("archive.7z")).is_ok());
        assert_eq!(Path::new("archive.7z").parent(), Some(Path::new("")));

        let err = writable_parent(Path::new("/tmp/indium-no-such-folder-9c1f/archive.7z"))
            .expect_err("a folder that is not there must be refused");
        assert!(
            err.contains("/tmp/indium-no-such-folder-9c1f"),
            "the folder is the whole point of the sentence: {err:?}"
        );
        assert!(
            !err.contains("archive.7z"),
            "and the file is not what is missing: {err:?}"
        );
    }
}
