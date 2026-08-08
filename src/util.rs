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
    /// Something else. CORE §4 reserves hex for V1.1, so the honest answer is a sentence.
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
}
