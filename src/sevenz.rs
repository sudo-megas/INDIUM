//! 7z, in both directions — P4 §4.
//!
//! CORE §2's sentence for `sevenz-rust2`: "Writes 7z with AES-256, which libarchive
//! cannot do; also the source of 7z-specific detail (solid blocks) the generic reader
//! does not expose." Both halves live here.
//!
//! This module exists rather than living inside `arch` because `arch`'s own first line
//! declares it hand-written FFI over the system libarchive, and a crate-backed backend
//! does not belong inside that sentence. Recorded in P4's Deviations.
//!
//! Copyright © sudo-megas. GPL-3.0-only.

use std::collections::HashMap;
use std::path::Path;

use sevenz_rust2::encoder_options::{AesEncoderOptions, Lzma2Options};
use sevenz_rust2::{
    Archive, ArchiveEntry as SevenZEntry, ArchiveWriter, BlockDecoder, EncoderConfiguration,
    EncoderMethod, Password,
};

use crate::arch::{ArchiveError, ArchiveInfo, Entry};
use crate::secret::Secret;
use crate::tasks::{Meta, Recipe, Sink};
use crate::util::normalize_archive_path;

/// 100-nanosecond ticks between 0001-01-01 and the unix epoch.
///
/// The crate offers no unix conversion of its own — only `SystemTime` — and going through
/// `SystemTime` cannot represent a timestamp before 1970, which archives do contain.
const NT_UNIX_EPOCH: u64 = 116_444_736_000_000_000;

/// The p7zip convention for a unix mode in a 7z entry's Windows attribute word: the low
/// flag says "there is a unix mode here", and the mode sits in the high sixteen bits.
///
/// `sevenz-rust2` never interprets this — it reads and writes an opaque `u32` — so both
/// directions are INDIUM's own work, and both are here so they cannot drift apart.
const UNIX_EXTENSION: u32 = 0x8000;

fn nt_to_unix(ticks: u64) -> Option<i64> {
    if ticks == 0 {
        return None;
    }
    Some((ticks as i128 - NT_UNIX_EPOCH as i128).div_euclid(10_000_000) as i64)
}

fn unix_to_nt(seconds: i64) -> u64 {
    ((seconds as i128) * 10_000_000 + NT_UNIX_EPOCH as i128).max(0) as u64
}

fn mode_from_attributes(attributes: u32, has_attributes: bool) -> u32 {
    if has_attributes && attributes & UNIX_EXTENSION != 0 {
        attributes >> 16
    } else {
        0
    }
}

/// Turn INDIUM's `Secret` into the crate's `Password`.
///
/// The crate's `Password` neither wipes on drop nor hides itself from `Debug`, and its
/// key derivation caches the derived key in a process-global that is never cleared. P4 §4
/// records that; this function is the exact boundary where INDIUM's own guarantee ends.
fn password_of(secret: Option<&Secret>) -> Password {
    match secret {
        None => Password::empty(),
        Some(s) => match std::str::from_utf8(s.as_bytes()) {
            Ok(text) => Password::new(text),
            // A password that is not UTF-8 cannot be put through the crate's UTF-16
            // conversion, and inventing an encoding for it would be worse than failing
            // to match. Empty means "no password", which fails honestly.
            Err(_) => Password::empty(),
        },
    }
}

// ---------------------------------------------------------------------------
// Reading — P4 §4
// ---------------------------------------------------------------------------

/// List a 7z through `sevenz-rust2`.
///
/// Header parsing only: no member's data is decoded, so this is as cheap as libarchive's
/// listing and works on an archive whose *headers* are encrypted, which libarchive
/// refuses outright.
pub fn list_all(path: &Path, passphrase: Option<&Secret>) -> Result<Vec<Entry>, ArchiveError> {
    let password = password_of(passphrase);
    let archive = Archive::open_with_password(path, &password).map_err(classify)?;
    Ok(entries_of(&archive))
}

/// Whether the archive is solid, and how many compression blocks it holds.
///
/// CORE §4 promised this detail for P4. It belongs to the archive, not to an entry —
/// which is exactly why an entry's packed size so often cannot be given.
pub fn solid_info(path: &Path, passphrase: Option<&Secret>) -> Option<(bool, usize)> {
    let password = password_of(passphrase);
    let archive = Archive::open_with_password(path, &password).ok()?;
    Some((archive.is_solid, archive.blocks.len()))
}

/// What a 7z reports about itself, in the shape the rest of the program consumes.
///
/// libarchive names the container and its filter; only this reader can say whether the
/// archive is solid and how many blocks it holds, so those two arrive here and nowhere
/// else. The format string is written to read as libarchive's does, because the status
/// bar and the archive card show it beside formats libarchive named.
pub fn info_of(path: &Path, passphrase: Option<&Secret>) -> ArchiveInfo {
    let (solid, blocks) = match solid_info(path, passphrase) {
        Some((s, b)) => (Some(s), Some(b)),
        None => (None, None),
    };
    ArchiveInfo {
        format: "7-Zip".to_string(),
        // A 7z carries its compression inside the container rather than as an outer
        // filter, so there is no filter to name — the Method column carries the coder.
        filter: String::new(),
        solid,
        blocks,
    }
}

/// Map a parsed 7z onto INDIUM's own `Entry`, so the rest of the program never learns
/// there are two readers.
fn entries_of(archive: &Archive) -> Vec<Entry> {
    // How many entries name each block. This is the whole of the packed-size rule, and
    // it is counted rather than inferred: the crate stamps a block's packed size onto
    // whichever entry happens to be first in it, so `compressed_size != 0` would report
    // a shared block's total against one of its occupants and nothing against the rest.
    let mut occupancy: HashMap<usize, usize> = HashMap::new();
    for block in archive.stream_map.file_block_index.iter().flatten() {
        *occupancy.entry(*block).or_insert(0) += 1;
    }

    archive
        .files
        .iter()
        .enumerate()
        .map(|(i, file)| {
            let block = archive
                .stream_map
                .file_block_index
                .get(i)
                .copied()
                .flatten();

            // Reported only where the entry owns its block outright. Apportioning a
            // shared block between its members would be a guess, and CORE would rather
            // show nothing than guess.
            let packed = match block {
                Some(b) if occupancy.get(&b).copied().unwrap_or(0) == 1 => {
                    Some(file.compressed_size)
                }
                _ => None,
            };

            let methods = block
                .and_then(|b| archive.blocks.get(b))
                .map(|b| {
                    b.coders
                        .iter()
                        .filter_map(|c| EncoderMethod::by_id(c.encoder_method_id()))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            let encrypted = methods.contains(&EncoderMethod::AES256_SHA256);
            let mode = mode_from_attributes(file.windows_attributes, file.has_windows_attributes);

            Entry {
                raw_path: file.name.clone(),
                path: normalize_archive_path(&file.name),
                is_dir: file.is_directory,
                size: file.size,
                packed,
                method: method_label(&methods),
                mtime: file
                    .has_last_modified_date
                    .then(|| nt_to_unix(u64::from(file.last_modified_date)))
                    .flatten(),
                atime: file
                    .has_access_date
                    .then(|| nt_to_unix(u64::from(file.access_date)))
                    .flatten(),
                ctime: file
                    .has_creation_date
                    .then(|| nt_to_unix(u64::from(file.creation_date)))
                    .flatten(),
                birthtime: None,
                // 7z carries no ownership at all. Reporting a zero would look like root.
                uid: 0,
                gid: 0,
                uname: None,
                gname: None,
                mode,
                filetype: 0,
                symlink: None,
                hardlink: None,
                encrypted,
            }
        })
        .collect()
}

/// The Method column for a 7z entry, from the coders its block actually uses.
pub fn method_label(methods: &[EncoderMethod]) -> String {
    let names: Vec<&str> = methods
        .iter()
        .filter(|m| **m != EncoderMethod::AES256_SHA256)
        .map(|m| m.name())
        .collect();

    let base = if names.is_empty() {
        "—".to_string()
    } else {
        names.join("+")
    };

    if methods.contains(&EncoderMethod::AES256_SHA256) {
        format!("{base}+AES-256")
    } else {
        base
    }
}

/// Map the crate's errors onto INDIUM's, keeping the exact user-facing sentences.
///
/// Wrong-password and corrupt-file are not reliably distinguishable — the crate's own
/// variant is named "maybe", and it never inspects *why* a decode failed. So an
/// encrypted archive that will not open reports the password, which is the far more
/// likely cause and the one the user can do something about.
fn classify(error: sevenz_rust2::Error) -> ArchiveError {
    use sevenz_rust2::Error as E;
    match error {
        E::PasswordRequired => ArchiveError::NeedPassword,
        E::MaybeBadPassword(_) => ArchiveError::WrongPassword,
        E::UnsupportedCompressionMethod(name) => ArchiveError::Other(format!(
            "this 7z uses {name}, which INDIUM's 7z reader does not decode"
        )),
        other => ArchiveError::Other(format!("{other:?}")),
    }
}

// ---------------------------------------------------------------------------
// Writing — P4 §3
// ---------------------------------------------------------------------------

/// A 7z being written, with LZMA2 and optionally AES-256.
pub struct Writer {
    inner: Option<ArchiveWriter<std::fs::File>>,
}

impl Writer {
    /// Open `path` for writing under `recipe`.
    ///
    /// The order of the content methods is load-bearing and easy to get backwards: index
    /// zero is the coder closest to the file, so AES goes **first** in the vector and
    /// LZMA2 second. That yields plaintext → LZMA2 → AES → disk, which is compress-then-
    /// encrypt. Written the other way round it still round-trips through this crate, and
    /// produces a non-standard archive that compresses nothing.
    pub fn create(
        path: &Path,
        recipe: &Recipe,
        passphrase: Option<&Secret>,
    ) -> Result<Writer, String> {
        let mut inner = ArchiveWriter::create(path)
            .map_err(|e| format!("could not open the 7z for writing: {e:?}"))?;

        let level = recipe.method.clamp_level(recipe.level);
        let methods: Vec<EncoderConfiguration> = if recipe.encrypt {
            let secret = passphrase.ok_or_else(|| {
                "an encrypted archive needs a password, and none was given".to_string()
            })?;
            vec![
                AesEncoderOptions::new(password_of(Some(secret))).into(),
                Lzma2Options::from_level(level).into(),
            ]
        } else {
            vec![Lzma2Options::from_level(level).into()]
        };

        inner.set_content_methods(methods);
        // Only meaningful when AES is present; a no-op otherwise. With it on, the member
        // names are ciphertext too — see P4 §4 for the tradeoff this accepts.
        inner.set_encrypt_header(recipe.encrypt);

        Ok(Writer { inner: Some(inner) })
    }

    fn writer(&mut self) -> Result<&mut ArchiveWriter<std::fs::File>, String> {
        self.inner
            .as_mut()
            .ok_or_else(|| "the 7z writer has already been closed".to_string())
    }
}

/// Build the crate's entry from ours.
fn seven_z_entry(meta: &Meta) -> SevenZEntry {
    let mut entry = if meta.is_dir {
        SevenZEntry::new_directory(&meta.out_path)
    } else {
        SevenZEntry::new_file(&meta.out_path)
    };

    if let Some(t) = meta.mtime {
        entry.last_modified_date = unix_to_nt(t).into();
        entry.has_last_modified_date = true;
    }
    if let Some(t) = meta.atime {
        entry.access_date = unix_to_nt(t).into();
        entry.has_access_date = true;
    }
    if let Some(t) = meta.ctime {
        entry.creation_date = unix_to_nt(t).into();
        entry.has_creation_date = true;
    }

    let mode = meta.mode & 0o7777;
    if mode != 0 {
        entry.windows_attributes = UNIX_EXTENSION | (mode << 16);
        entry.has_windows_attributes = true;
    }
    entry
}

impl Sink for Writer {
    fn put(&mut self, meta: &Meta, data: Option<&mut dyn std::io::Read>) -> Result<(), String> {
        // 7z carries neither symlinks nor hardlinks, and P4 §3's table says so. Writing
        // the link's own (empty) body would silently turn a link into an empty file, so
        // it is skipped and the loss is what the popup already warned about.
        if meta.symlink.is_some() || meta.hardlink.is_some() {
            return Ok(());
        }

        let entry = seven_z_entry(meta);
        let writer = self.writer()?;

        match data {
            // The turbofish is required: with `None` there is nothing for the reader
            // type to be inferred from.
            None => writer
                .push_archive_entry::<&[u8]>(entry, None)
                .map(|_| ())
                .map_err(|e| format!("could not write {}: {e:?}", meta.out_path)),
            Some(reader) => writer
                .push_archive_entry(entry, Some(reader))
                .map(|_| ())
                .map_err(|e| format!("could not write {}: {e:?}", meta.out_path)),
        }
    }

    fn finish(&mut self) -> Result<(), String> {
        match self.inner.take() {
            None => Ok(()),
            Some(writer) => writer
                .finish()
                .map(|_| ())
                .map_err(|e| format!("could not finish the 7z: {e}")),
        }
    }

    fn abandon(&mut self) {
        // Dropped without `finish`, so no end-of-archive header is written and the temp
        // file cannot be mistaken for a complete archive. Apply removes it either way.
        self.inner = None;
    }
}

// ---------------------------------------------------------------------------
// Reading data — P5 §A1b
// ---------------------------------------------------------------------------

/// Read one entry's bytes out of a 7z.
///
/// P4 §4 promised this — *"Data … goes to libarchive first, and to `sevenz` only where
/// libarchive refuses"* — and did not build it. Nothing noticed, because until P5 the
/// window could not open an encrypted-header archive at all. Now that it can, an archive
/// that lists and then refuses every read would be a worse state than not opening.
///
/// `cap` bounds the read: an archive is untrusted input, and a caller that wants a
/// preview must not be handed four gigabytes. The `bool` is true when the entry was
/// longer than the cap and the read stopped early.
pub fn read_entry(
    path: &Path,
    entry_path: &str,
    cap: usize,
    passphrase: Option<&Secret>,
) -> Result<(Vec<u8>, bool), ArchiveError> {
    use std::io::Read as _;

    let password = password_of(passphrase);
    let mut source = std::fs::File::open(path)
        .map_err(|e| ArchiveError::Other(format!("could not open the archive: {e}")))?;
    let archive = Archive::read(&mut source, &password).map_err(classify)?;

    // Which block holds it, so the other blocks are never decoded at all.
    let wanted = archive
        .files
        .iter()
        .position(|f| normalize_archive_path(&f.name) == entry_path)
        .ok_or_else(|| ArchiveError::Other(format!("no such entry: {entry_path}")))?;
    let Some(block) = archive
        .stream_map
        .file_block_index
        .get(wanted)
        .copied()
        .flatten()
    else {
        // A directory or an empty file has no data stream, and that is not an error.
        return Ok((Vec::new(), false));
    };

    let name = archive.files[wanted].name.clone();
    let mut out = Vec::new();
    let mut truncated = false;
    let mut found = false;

    let decoder = BlockDecoder::new(1, block, &archive, &password, &mut source);
    decoder
        .for_each_entries(&mut |entry, reader| {
            if entry.name != name {
                // A solid block is one sequential stream, so a member before the one we
                // want has to be read through rather than skipped over.
                std::io::copy(reader, &mut std::io::sink())?;
                return Ok(true);
            }
            found = true;
            let mut limited = reader.take(cap as u64);
            limited.read_to_end(&mut out)?;
            truncated = out.len() >= cap;
            Ok(false) // stop; nothing after this one needs decoding
        })
        .map_err(classify)?;

    if !found {
        return Err(ArchiveError::Other(format!("no such entry: {entry_path}")));
    }
    Ok((out, truncated))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The crate offers no unix conversion, so this one is INDIUM's and is worth pinning
    /// at the epoch and either side of it — archives do hold dates before 1970.
    #[test]
    fn nt_and_unix_timestamps_round_trip_in_both_directions() {
        for unix in [0_i64, 1_704_164_645, 981_158_400, -86_400] {
            let nt = unix_to_nt(unix);
            assert_eq!(
                nt_to_unix(nt),
                Some(unix),
                "{unix} did not survive the conversion"
            );
        }
        assert_eq!(nt_to_unix(0), None, "an unset time is not the year 1601");
    }

    /// The p7zip convention, both ways, in one place so the two cannot drift apart.
    #[test]
    fn a_unix_mode_survives_the_windows_attribute_word() {
        for mode in [0o644, 0o600, 0o755, 0o777] {
            let attributes = UNIX_EXTENSION | (mode << 16);
            assert_eq!(mode_from_attributes(attributes, true), mode);
        }
        assert_eq!(
            mode_from_attributes(0x20, true),
            0,
            "an entry without the unix flag has no mode to report"
        );
        assert_eq!(
            mode_from_attributes(UNIX_EXTENSION | (0o644 << 16), false),
            0,
            "an entry with no attributes at all has no mode either"
        );
    }

    /// The Method column. AES is a property of the block, and reads as part of the
    /// method rather than as a separate column.
    #[test]
    fn the_method_label_names_every_coder_and_flags_encryption() {
        assert_eq!(method_label(&[EncoderMethod::LZMA2]), "LZMA2");
        assert_eq!(
            method_label(&[EncoderMethod::AES256_SHA256, EncoderMethod::LZMA2]),
            "LZMA2+AES-256"
        );
        assert_eq!(
            method_label(&[]),
            "—",
            "a directory has no coders, and an empty column is a dash"
        );
    }
}
