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
fn password_of(secret: Option<&Secret>) -> Result<Password, ArchiveError> {
    match secret {
        None => Ok(Password::empty()),
        Some(s) => match std::str::from_utf8(s.as_bytes()) {
            Ok(text) => Ok(Password::new(text)),
            // A password that is not UTF-8 cannot be put through the crate's UTF-16
            // conversion, and inventing an encoding for it would be worse than failing
            // to match.
            //
            // This used to return `Password::empty()`, which means *no password* — so a
            // password INDIUM could not encode came back as "Wrong password.", the one
            // sentence that is certainly untrue about it. Nothing was tried; the bytes
            // may be exactly right. P18 nominated the fix and left the sentence it should
            // say instead, which `ArchiveError::PasswordNotUtf8` now carries.
            Err(_) => Err(ArchiveError::PasswordNotUtf8),
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
    let password = password_of(passphrase)?;
    let archive = Archive::open_with_password(path, &password)
        .map_err(|e| classify(e, passphrase.is_some()))?;
    Ok(entries_of(&archive))
}

/// Whether the archive is solid, and how many compression blocks it holds.
///
/// CORE §4 promised this detail for P4. It belongs to the archive, not to an entry —
/// which is exactly why an entry's packed size so often cannot be given.
pub fn solid_info(path: &Path, passphrase: Option<&Secret>) -> Option<(bool, usize)> {
    // A password INDIUM cannot encode is the same answer as an archive that will not
    // open: this function reports detail or nothing, and has no way to carry a reason.
    let password = password_of(passphrase).ok()?;
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
fn classify(error: sevenz_rust2::Error, had_password: bool) -> ArchiveError {
    use sevenz_rust2::Error as E;
    match error {
        E::PasswordRequired => ArchiveError::NeedPassword,
        E::MaybeBadPassword(_) => ArchiveError::WrongPassword,
        E::UnsupportedCompressionMethod(name) => ArchiveError::Other(format!(
            "this 7z uses {name}, which INDIUM's 7z reader does not decode"
        )),

        // PXX 10.9. With **encrypted headers** a wrong key does not fail the decryption:
        // AES has nothing to check the key against, so it cheerfully hands the parser a
        // block of noise, and the parser then fails in whatever way that particular noise
        // happens to break it. The walk hit `Other("Broken or unsupported archive: no
        // Header")` and had no way to know it had simply mistyped the password.
        //
        // Every arm below is a *structural* failure to parse a header. Reached with a
        // password in hand, each one means the same thing, and it is the paragraph above
        // this function's decision applied to the case that actually occurs — not a new
        // policy. `BadSignature` is deliberately absent: the signature sits in plaintext
        // ahead of any encryption, so a bad one means the file is not a 7z, and no
        // password will ever help. `Io`, `FileOpen`, `FileNotFound` and `MaxMemLimited`
        // are absent for the same reason — none of them is about the key.
        E::Other(_)
        | E::NextHeaderCrcMismatch
        | E::ChecksumVerificationFailed
        | E::UnsupportedVersion { .. }
        | E::BadTerminatedStreamsInfo(_)
        | E::BadTerminatedUnpackInfo
        | E::BadTerminatedPackInfo(_)
        | E::BadTerminatedSubStreamsInfo
        | E::BadTerminatedHeader(_)
            if had_password =>
        {
            ArchiveError::WrongPassword
        }

        // `Display`, never `Debug`. `{other:?}` is what printed the crate's own enum
        // shape into a terminal — `indium: Other("…")` — naming an internal type at the
        // one moment the reader needs a sentence.
        other => ArchiveError::Other(other.to_string()),
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
        // PXX 9.5, and see [`crate::util::writable_parent`] for what it is for. Checked
        // before the open, because after it the only thing left to report is what the
        // failing library chose to say about a temp file the person never named.
        crate::util::writable_parent(path)?;

        let mut inner = ArchiveWriter::create(path)
            .map_err(|e| format!("could not open the 7z for writing: {e}"))?;

        let level = recipe.method.clamp_level(recipe.level);
        let methods: Vec<EncoderConfiguration> = if recipe.encrypt {
            let secret = passphrase.ok_or_else(|| {
                "an encrypted archive needs a password, and none was given".to_string()
            })?;
            vec![
                AesEncoderOptions::new(password_of(Some(secret)).map_err(|e| e.to_string())?)
                    .into(),
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
                .map_err(|e| format!("could not write {}: {e}", meta.out_path)),
            Some(reader) => writer
                .push_archive_entry(entry, Some(reader))
                .map(|_| ())
                .map_err(|e| format!("could not write {}: {e}", meta.out_path)),
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

/// Did a decode that returned `Ok` actually deliver what the member claimed?
///
/// Split out from `read_entry` so it can be gated on its own, and it needs to be: the end-to-end
/// tests reach this through a wrong AES key, and whether a given wrong key produces a *short*
/// decode or an outright decode *error* is a property of that key and that archive's random salt.
/// A gate built on it would pass or fail by luck. The comparison is the part that must be right,
/// so the comparison is what gets pinned.
///
/// `min` and not `stated`, because a capped read is legitimately short: asking for one byte of a
/// hundred thousand and getting one byte is the check working, not failing.
fn decode_reached_target(got: usize, cap: usize, stated: u64) -> bool {
    (got as u64) >= std::cmp::min(cap as u64, stated)
}

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
///
/// This block sat above `decode_reached_target` from `9175a28` until v2.5: the helper was
/// spliced in between the doc and the function, so rustdoc attached the whole of P4 §4's
/// paragraph — and a `cap`/`bool` contract naming neither of that helper's arguments — to a
/// three-line predicate, and left this function undocumented. Caught by a reader; no test can
/// see which item a doc comment lands on.
pub fn read_entry(
    path: &Path,
    entry_path: &str,
    cap: usize,
    passphrase: Option<&Secret>,
) -> Result<(Vec<u8>, bool), ArchiveError> {
    use std::io::Read as _;

    let password = password_of(passphrase)?;
    let mut source = std::fs::File::open(path)
        .map_err(|e| ArchiveError::Other(format!("could not open the archive: {e}")))?;
    let archive =
        Archive::read(&mut source, &password).map_err(|e| classify(e, passphrase.is_some()))?;

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
    // Read out beside the name, and for the same reason: both are needed after the decoder has
    // finished borrowing the archive. This one is what a short decode is measured against.
    let stated = archive.files[wanted].size;
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
        .map_err(|e| classify(e, passphrase.is_some()))?;

    if !found {
        return Err(ArchiveError::Other(format!("no such entry: {entry_path}")));
    }

    // A decode that stops short of the member's stated length is never legitimate, and it is the
    // only signal available here. `sevenz-rust2` compares a member's CRC only at end of stream, so
    // a stream that ends early is never checked against it; `read_to_end` over a reader that
    // returns `Ok(0)` is not an error; and `truncated` is `false` for `0 >= 1`. Three chances to
    // notice, and the value was zero bytes.
    //
    // A wrong AES key produces exactly that. There is nothing inside a CBC block to check a key
    // against, so decryption "succeeds" and hands LZMA2 noise — which usually, and **not always**,
    // fails part-way through. Tier 3 measured 14 of 1500 wrong passwords surviving a one-byte read
    // of an LZMA2 member, and `extract` then reported success after truncating a 100 000-byte
    // destination file to zero. The earlier reasoning here — "one byte is enough" — was true of the
    // single wrong password it was tested with, which is not the same as true.
    //
    // The number that settles it is the stated size, and it was in scope all along.
    //
    // One trade, taken deliberately: a genuinely truncated archive read with the **right** password
    // now reports `WrongPassword` rather than a truncation error. From here the two are
    // indistinguishable — a short stream is a short stream — and the alternative is reading one as
    // success, which is what destroyed the file.
    if !decode_reached_target(out.len(), cap, stated) {
        return Err(if passphrase.is_some() {
            ArchiveError::WrongPassword
        } else {
            ArchiveError::Other(format!("truncated stream: {entry_path}"))
        });
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

    /// A password INDIUM cannot encode is not a wrong password, and must not be reported
    /// as one.
    ///
    /// The path is reachable, which is why P18 nominated it rather than filing it as
    /// theory: the terminal prompt reads the password as **bytes** off `/dev/tty` and
    /// builds `Secret::new(line)` from them (`cli.rs`), so any byte sequence a keyboard
    /// and a locale can produce arrives here. Through v1.2.0-2 a non-UTF-8 one became
    /// `Password::empty()` — *no password* — and the operation came back "Wrong
    /// password.", which is the one thing known to be untrue about it: nothing had been
    /// tried against the archive at all.
    ///
    /// `0xFF` is the assertion's whole point. It is not valid UTF-8 in any position, and
    /// it is a byte a Latin-1 keyboard produces without anybody doing anything strange.
    #[test]
    fn a_password_that_is_not_text_is_refused_as_itself_and_not_as_a_wrong_password() {
        let not_text = Secret::new(vec![b'h', b'i', 0xFF]);
        let err = password_of(Some(&not_text)).expect_err("0xFF is not valid UTF-8");
        assert!(
            matches!(err, ArchiveError::PasswordNotUtf8),
            "a password that cannot be encoded came back as {err:?}, and the only wrong \
             answer here is WrongPassword — it says the archive rejected something it was \
             never shown"
        );
        assert!(
            err.to_string().contains("never tried"),
            "the message has to say the password was not tried, or it reads as a rejection: \
             {err}"
        );

        // The two neighbours, so the arm cannot start swallowing what it should pass.
        assert!(
            password_of(None).is_ok(),
            "no password at all is not an error; it is how an unencrypted 7z is opened"
        );
        assert!(
            password_of(Some(&Secret::from_text("hünde"))).is_ok(),
            "non-ASCII is not the test — non-UTF-8 is. A perfectly ordinary password with \
             an umlaut in it must still go through"
        );
    }
}

#[cfg(test)]
mod content_only_encryption {
    use super::*;

    /// Build a `7z a -p` archive: AES-256 on the content, headers in the clear.
    ///
    /// This is the **default** for the 7-Zip command line — `-mhe=on` is what adds header
    /// encryption — so it is the ordinary shape of an encrypted 7z rather than an exotic one.
    /// It is built here rather than committed because a fixture is not something this repo
    /// puts in its history, and because `Writer::create` cannot make one: it ties
    /// `set_encrypt_header` to the same flag that turns AES on, so every 7z INDIUM encrypts
    /// has ciphertext headers. That is exactly why this case had no coverage.
    fn write_content_encrypted(path: &Path, password: &str, members: &[(&str, &[u8])]) {
        let mut inner = ArchiveWriter::create(path).expect("could not open the 7z for writing");
        inner.set_content_methods(vec![
            AesEncoderOptions::new(password.into()).into(),
            Lzma2Options::from_level(6).into(),
        ]);
        inner.set_encrypt_header(false);
        for (name, body) in members {
            let meta = Meta {
                out_path: (*name).to_string(),
                size: body.len() as u64,
                is_dir: false,
                mode: 0o644,
                mtime: Some(1_704_164_645),
                atime: None,
                ctime: None,
                uid: 0,
                gid: 0,
                uname: None,
                gname: None,
                symlink: None,
                hardlink: None,
            };
            inner
                .push_archive_entry(seven_z_entry(&meta), Some(&mut &body[..]))
                .unwrap_or_else(|e| panic!("could not write {name}: {e}"));
        }
        inner.finish().expect("could not finish the archive");
    }

    fn scratch(tag: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("indium-7z-content-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        dir
    }

    /// **`PXX-2-002`.** A correct password was refused on the commonest encrypted 7z there is.
    ///
    /// The routing flag asked *"can libarchive read this archive's headers?"* and used the answer
    /// to decide who owns the **data**. For a 7z those are different questions: headers in the
    /// clear parse fine, so the flag came back false, extraction stayed with libarchive — and
    /// libarchive cannot decrypt 7z AES content at all. Measured: its passphrase check returns
    /// "wrong" for the right password and for a wrong one alike, so it was not verifying
    /// anything, it was failing. The reader that *can* decrypt this was sitting one branch away.
    #[test]
    fn a_content_encrypted_7z_extracts_with_the_right_password() {
        let dir = scratch("good");
        let path = dir.join("content-only.7z");
        write_content_encrypted(
            &path,
            "indium",
            &[
                ("alpha.txt", b"INDIUM fixture alpha\n"),
                ("sub/beta.txt", b"beta\n"),
            ],
        );

        let secret = Secret::from_text("indium");
        let listing = crate::arch::list_all(&path, Some(&secret)).expect("it must list");
        assert!(
            listing.iter().any(|e| e.encrypted),
            "the fixture is only the case if its members read as encrypted: {listing:?}"
        );

        let dest = dir.join("out");
        let wanted: std::collections::HashSet<String> =
            ["alpha.txt".to_string(), "sub/beta.txt".to_string()]
                .into_iter()
                .collect();
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let n = crate::arch::extract(&path, &wanted, &dest, Some(&secret), None, &cancel)
            .expect("the right password must extract, not be called wrong");

        assert_eq!(n, 2, "both members must come out");
        assert_eq!(
            std::fs::read(dest.join("alpha.txt")).expect("alpha must be on disk"),
            b"INDIUM fixture alpha\n"
        );
        assert_eq!(
            std::fs::read(dest.join("sub/beta.txt")).expect("beta must be on disk"),
            b"beta\n"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The other face of `PXX-2-002`, which a blind confirmer reached from the same six lines.
    ///
    /// Routing the data to the 7z reader is only half a fix. With the headers in the clear the
    /// listing succeeds whatever the password is, so nothing before the per-entry decode knows
    /// the key is wrong — and by then `create_dir_under` has already put directories into the
    /// destination, contradicting this function's own promise that a wrong password *"costs
    /// nothing and leaves no partial output behind."* So the verification moves to the reader
    /// that can actually perform it, and it runs while the filesystem is still untouched.
    #[test]
    fn a_wrong_password_on_a_content_encrypted_7z_leaves_the_destination_untouched() {
        let dir = scratch("bad");
        let path = dir.join("content-only.7z");
        write_content_encrypted(&path, "indium", &[("sub/beta.txt", b"beta\n")]);

        let dest = dir.join("out");
        let wanted: std::collections::HashSet<String> =
            ["sub/beta.txt".to_string()].into_iter().collect();
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let got = crate::arch::extract(
            &path,
            &wanted,
            &dest,
            Some(&Secret::from_text("not-the-password")),
            None,
            &cancel,
        );

        assert!(
            matches!(got, Err(ArchiveError::WrongPassword)),
            "a wrong password must be reported as one: {got:?}"
        );
        assert!(
            !dest.join("sub").exists(),
            "a refused extraction must leave no directory behind it"
        );
        assert!(!dest.join("sub/beta.txt").exists(), "and certainly no file");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **`PXX-T3-011`.** The comparison that turns a short decode from a success into a refusal.
    ///
    /// This is the defect that destroyed a file: a wrong AES key produced a zero-byte decode,
    /// `for_each_entries` returned `Ok`, `read_to_end` over a reader yielding `Ok(0)` is not an
    /// error, and `truncated = out.len() >= cap` is `false` for `0 >= 1`. Three chances to notice
    /// and the answer was success, so `extract` truncated a 100 000-byte destination file to zero
    /// and reported it had written one member.
    ///
    /// Gated here rather than end to end, and the reason is the honest one: whether a *particular*
    /// wrong password yields a short decode or a decode error depends on that archive's random
    /// salt, so an end-to-end assertion on it would pass by luck. Tier 3 measured the rate — 14 of
    /// 1500 — which is a rate and not a gate.
    #[test]
    fn a_decode_shorter_than_the_member_is_not_a_success() {
        assert!(
            !decode_reached_target(0, 1, 100_000),
            "zero bytes of a hundred thousand is the case that destroyed a file, whatever the \
             cap asked for"
        );
        assert!(
            !decode_reached_target(5, usize::MAX, 10),
            "half a member is not a member"
        );
        assert!(
            decode_reached_target(1, 1, 100_000),
            "a capped read is legitimately short — one byte of a hundred thousand, when one byte \
             was asked for, is the check working"
        );
        assert!(
            decode_reached_target(100_000, usize::MAX, 100_000),
            "a full read of a full member must pass, or extraction refuses everything"
        );
        assert!(
            decode_reached_target(0, 0, 0),
            "a member with no bytes asked for none and delivered none"
        );
        assert!(
            decode_reached_target(21, 4096, 21),
            "a member shorter than the cap is not truncated, it is finished"
        );
    }

    /// **`PXX-T3-012`. The gate the window actually presses, and the reason the first fix was
    /// invisible.**
    ///
    /// `da6c821` corrected the routing inside `extract` and left `verify_passphrase` alone. But
    /// `ui/password.rs` gates extraction on `verify_passphrase` and on nothing else, so the
    /// corrected code was unreachable from the window: a user holding the right password was
    /// refused three times and told the archive was cancelled, while `extract` — which never ran —
    /// would have accepted it. Two blind readers studied those six lines four times between them
    /// and neither asked what calls them.
    ///
    /// So this test is deliberately shaped as the *user's* question rather than the reader's. It is
    /// the boundary a fix has to move to count as a fix.
    #[test]
    fn the_window_accepts_a_right_password_on_a_content_encrypted_7z() {
        let dir = scratch("verify");
        let path = dir.join("content-only.7z");
        write_content_encrypted(&path, "indium", &[("alpha.txt", b"INDIUM fixture alpha\n")]);

        assert!(
            crate::arch::verify_passphrase(&path, &Secret::from_text("indium"))
                .expect("verification must answer, not error"),
            "the right password must verify — this is what the password popup asks, and it \
             answered no for every content-encrypted 7z until v2.5"
        );
        assert!(
            !crate::arch::verify_passphrase(&path, &Secret::from_text("not-the-password"))
                .expect("verification must answer, not error"),
            "and a wrong password must still be refused, or the fix has bought reachability \
             by giving up the check"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **`PXX-T3-012`, the class-9 half: the same defect at three more doors.**
    ///
    /// All three fallbacks were keyed on `EncryptedHeaders`, and libarchive does not report this
    /// archive class that way — it reports a wrong password, because it parses the headers, reaches
    /// the data, and cannot decrypt it. So every arm was dead on exactly the archives it existed
    /// for, and Preview, `indium cat` and CRC32 each refused a correct password while `extract`
    /// accepted it.
    ///
    /// Asserted through the three public entry points rather than through the arm, because the arm
    /// is what was already believed to be right.
    #[test]
    fn preview_cat_and_crc32_all_read_a_content_encrypted_7z() {
        let dir = scratch("siblings");
        let path = dir.join("content-only.7z");
        let body: &[u8] = b"INDIUM fixture alpha\n";
        write_content_encrypted(&path, "indium", &[("alpha.txt", body)]);
        let secret = Secret::from_text("indium");

        let (head, more) = crate::arch::head_of(&path, "alpha.txt", 4096, Some(&secret))
            .expect("Preview must read");
        assert_eq!(head, body, "Preview must see the plaintext");
        assert!(!more, "the member is shorter than the cap");

        let mut sunk: Vec<u8> = Vec::new();
        let n = crate::arch::stream_entry(&path, "alpha.txt", Some(&secret), &mut sunk)
            .expect("`indium cat` must read");
        assert_eq!(n, body.len() as u64, "the whole member must be streamed");
        assert_eq!(
            sunk, body,
            "and it must be the plaintext, not the ciphertext"
        );

        assert_eq!(
            crate::arch::crc32_of(&path, "alpha.txt", Some(&secret)).expect("CRC32 must read"),
            crate::util::crc32(body),
            "the CRC must be of the decrypted bytes"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **`PXX-T3-013` and `PXX-T3-018`. "Nothing to verify against" is not "verified."**
    ///
    /// The pre-flight looks for an encrypted member with a data stream to test the key on. Select
    /// only members that have none — a directory, or a zero-length file — and the search came back
    /// empty, the pre-flight was skipped entirely, and a wrong password extracted as a success.
    ///
    /// This gate is also the one the reviewer's third sabotage would have failed. It replaced the
    /// selection predicate with a plain `next()` — changing neither the routing nor the
    /// verification, only *which member gets tested* — and both of the gates above passed anyway.
    /// A pair that gates two halves of a fix does not thereby gate the line between them.
    #[test]
    fn an_encrypted_selection_with_no_bytes_in_it_still_refuses_a_wrong_password() {
        let dir = scratch("nobytes");
        let path = dir.join("content-only.7z");
        write_content_encrypted(
            &path,
            "indium",
            &[("empty.txt", b""), ("alpha.txt", b"INDIUM fixture alpha\n")],
        );

        // The premise, pinned rather than assumed: an empty member inside an AES block still reads
        // as encrypted, which is why it survives the `encrypted` filter and fails the `size > 0`
        // one.
        let listing =
            crate::arch::list_all(&path, Some(&Secret::from_text("indium"))).expect("it must list");
        let empty = listing
            .iter()
            .find(|e| e.path == "empty.txt")
            .expect("the empty member must be listed");
        assert!(
            empty.encrypted,
            "an empty member in an AES block is encrypted"
        );
        assert_eq!(empty.size, 0, "and it has no bytes to verify a key against");

        let dest = dir.join("out");
        let wanted: std::collections::HashSet<String> =
            ["empty.txt".to_string()].into_iter().collect();
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let got = crate::arch::extract(
            &path,
            &wanted,
            &dest,
            Some(&Secret::from_text("not-the-password")),
            None,
            &cancel,
        );

        assert!(
            matches!(got, Err(ArchiveError::WrongPassword)),
            "a wrong password must be refused even when the selection carries nothing to test \
             it on — the question goes to the archive instead: {got:?}"
        );
        assert!(
            !dest.join("empty.txt").exists(),
            "and nothing may be written on the way to finding out"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// The other half of the encrypted-7z surface: **ciphertext headers**, which is what INDIUM's own
/// writer produces for every archive it encrypts.
///
/// `content_only_encryption` above covers `7z a -p` — the 7-Zip CLI default, plaintext headers. This
/// module covers `-mhe=on`, and it exists because a tier-3 review of `9175a28` found that the
/// commit had repaired the first class and left the second one broken at the gate the window
/// presses. The two modules are deliberately separate: the classes fail differently, and one
/// module named for both would have hidden exactly this.
#[cfg(test)]
mod header_encryption {
    use super::*;

    /// A member: name, body, and whether a compressor sits behind the cipher.
    ///
    /// The `false` case — AES with **no** compressor, a COPY coder — is the one that matters, and
    /// it is not decoration. A wrong key's noise passes through COPY intact and at full length, so
    /// nothing but the member's own CRC can refuse it, and that CRC is compared at end of stream
    /// and nowhere else.
    type Member<'a> = (&'a str, &'a [u8], bool);

    fn write_7z(path: &Path, password: &str, encrypt_header: bool, members: &[Member]) {
        let mut inner = ArchiveWriter::create(path).expect("could not open the 7z for writing");
        inner.set_encrypt_header(encrypt_header);
        for (name, body, lzma2) in members {
            let mut methods: Vec<EncoderConfiguration> =
                vec![AesEncoderOptions::new(password.into()).into()];
            if *lzma2 {
                methods.push(Lzma2Options::from_level(6).into());
            }
            inner.set_content_methods(methods);
            let meta = Meta {
                out_path: (*name).to_string(),
                size: body.len() as u64,
                is_dir: false,
                mode: 0o644,
                mtime: Some(1_704_164_645),
                atime: None,
                ctime: None,
                uid: 0,
                gid: 0,
                uname: None,
                gname: None,
                symlink: None,
                hardlink: None,
            };
            inner
                .push_archive_entry(seven_z_entry(&meta), Some(&mut &body[..]))
                .unwrap_or_else(|e| panic!("could not write {name}: {e}"));
        }
        inner.finish().expect("could not finish the archive");
    }

    fn scratch(tag: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("indium-7z-header-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        dir
    }

    /// **`PXX-T3-026`. The program refused the correct password on its own archives.**
    ///
    /// `sevenz.rs`'s writer ties `set_encrypt_header` to the same flag that turns AES on, so
    /// *every* encrypted 7z INDIUM has ever written has ciphertext headers. `verify_passphrase`
    /// walked libarchive first, `next_entry()` answered `EncryptedHeaders`, and the function
    /// returned `Ok(false)` before its 7z branch was ever reached — so the password popup refused
    /// the right password three times and cancelled, on six of the seven pending actions, while
    /// `extract`, `head_of` and `crc32_of` all read the same archive with the same password.
    ///
    /// `PXX-2-002` fixed this for plaintext headers and `the_window_accepts_a_right_password_on_a_
    /// content_encrypted_7z` is its gate. This is the sibling site that fix did not reach: **class
    /// 9, and found by a reader rather than by the sweep that wrote the first fix.**
    #[test]
    fn the_window_accepts_a_right_password_on_a_header_encrypted_7z() {
        let dir = scratch("gate");
        let path = dir.join("headers.7z");
        write_7z(
            &path,
            "indium",
            true,
            &[("alpha.txt", b"INDIUM fixture alpha\n", true)],
        );

        assert!(
            crate::arch::verify_passphrase(&path, &Secret::from_text("indium"))
                .expect("verification must answer, not error"),
            "the right password must verify on an archive INDIUM itself wrote — this is what the \
             password popup asks, and it answered no for every encrypted archive the program \
             produced, from v2.1 to v2.5"
        );
        assert!(
            !crate::arch::verify_passphrase(&path, &Secret::from_text("not-the-password"))
                .expect("verification must answer, not error"),
            "and a wrong password must still be refused, or reachability has been bought by \
             giving up the check"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **`PXX-T3-027`. Two verification sites, one archive, opposite answers.**
    ///
    /// `verification_target` picks the *smallest* encrypted member and its doc argues the case at
    /// length. `verify_passphrase` then chose the **first** member with bytes instead — so on an
    /// archive whose first member is a large AES+COPY one, the popup said "unlocked" to a password
    /// `extract` refused a moment later, with the popup and its three-attempt counter already
    /// dismissed and no way to retype.
    ///
    /// The first member here is deliberately over the 1 MiB bound and deliberately has no
    /// compressor behind the cipher: that combination is the only one where a capped read cannot
    /// discriminate at all, because COPY returns noise at full length and the read stops before
    /// the CRC. The smallest member is small enough that its read runs to the end.
    ///
    /// Class 9 in its textbook form — **the same defect one door over, written by the hand that
    /// had just written the door.**
    #[test]
    fn the_password_gate_and_the_pre_flight_choose_the_same_member() {
        let dir = scratch("chooser");
        let path = dir.join("first-is-big.7z");
        let big = vec![0xA5u8; (1 << 20) + 4096];
        write_7z(
            &path,
            "indium",
            false,
            &[
                ("a-big.bin", &big, false),
                ("z-small.bin", &[7u8; 4096], false),
            ],
        );

        assert!(
            !crate::arch::verify_passphrase(&path, &Secret::from_text("not-the-password"))
                .expect("verification must answer, not error"),
            "a wrong password must be refused even when the first member with bytes is a large \
             AES+COPY one — taking the first rather than the smallest is what let this through"
        );
        assert!(
            crate::arch::verify_passphrase(&path, &Secret::from_text("indium"))
                .expect("verification must answer, not error"),
            "and the right password must still pass, or the chooser has been fixed by refusing \
             everything"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **`PXX-T3-028`. A correct password refused because the selection had nothing to test it on.**
    ///
    /// `extract`'s pre-flight falls back to `verify_passphrase` when the selection holds no
    /// encrypted member with bytes — a directory-only or empty-file-only extraction. That fallback
    /// asked a function that could not answer for a header-encrypted archive, so a correct
    /// password was refused on a selection that was perfectly extractable. Reachable from the CLI,
    /// which calls `arch::extract` directly and has no popup in front of it.
    ///
    /// The companion assertion is the one that matters more: a **wrong** password on the same
    /// selection must still be refused, because the archive is asked instead of the selection.
    #[test]
    fn an_empty_only_selection_from_a_header_encrypted_7z_still_answers_both_ways() {
        let dir = scratch("emptyonly");
        let path = dir.join("headers.7z");
        write_7z(
            &path,
            "indium",
            true,
            &[
                ("empty.txt", b"", true),
                ("alpha.bin", b"twenty-one bytes here", true),
            ],
        );

        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

        // The premise, pinned rather than assumed, exactly as the plaintext-header sibling pins
        // it: an empty member inside an AES block still lists as encrypted, so it passes the
        // `encrypted` filter and fails the `size > 0` one — which is what routes the pre-flight
        // into the fallback this gate is about.
        let listing =
            crate::arch::list_all(&path, Some(&Secret::from_text("indium"))).expect("it must list");
        let empty = listing
            .iter()
            .find(|e| e.path == "empty.txt")
            .expect("the empty member must be listed");
        assert!(
            empty.encrypted,
            "an empty member in an AES block is encrypted"
        );
        assert_eq!(empty.size, 0, "and it has no bytes to verify a key against");

        let wanted: std::collections::HashSet<String> =
            ["empty.txt".to_string()].into_iter().collect();

        let dest = dir.join("out-right");
        let got = crate::arch::extract(
            &path,
            &wanted,
            &dest,
            Some(&Secret::from_text("indium")),
            None,
            &cancel,
        );
        assert!(
            got.is_ok(),
            "the right password must extract an empty-only selection, not be refused by a check \
             that could not run: {got:?}"
        );

        let dest2 = dir.join("out-wrong");
        let got2 = crate::arch::extract(
            &path,
            &wanted,
            &dest2,
            Some(&Secret::from_text("not-the-password")),
            None,
            &cancel,
        );
        assert!(
            matches!(got2, Err(ArchiveError::WrongPassword)),
            "and a wrong password must still be refused even though the selection carries nothing \
             to test it on — the question goes to the archive instead: {got2:?}"
        );
        assert!(
            !dest2.join("empty.txt").exists(),
            "and nothing may be written on the way to finding out"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **The arm that had no gate, added because a sabotage survived.**
    ///
    /// `verify_passphrase`'s 7z branch answers `Ok(true)` when the archive holds no encrypted
    /// member with bytes, on the stated ground that there is no ciphertext in it to get wrong.
    /// Flipping that `Ok(true)` to `Ok(false)` was caught by **nothing** — 309 tests passed with a
    /// plain 7z's correct password being refused.
    ///
    /// This is the practice working on the round's own fix rather than on an inherited defect: the
    /// two arms either side of it were each caught by exactly one gate, and the survivor is the one
    /// whose behaviour nobody had thought to assert because it reads as obviously right. **A gate
    /// exists for the arms that look like they need one; this is the other kind.**
    #[test]
    fn an_archive_with_no_ciphertext_in_it_accepts_a_needless_password() {
        let dir = scratch("nociphertext");

        // A plain 7z, no encryption anywhere, handed a password it never asked for. `extract`'s
        // pre-flight reaches this whenever a password is supplied for an archive that does not
        // need one, and refusing it would refuse the archive.
        let plain = dir.join("plain.7z");
        {
            let mut inner = ArchiveWriter::create(&plain).expect("could not open the 7z");
            let meta = Meta {
                out_path: "alpha.txt".to_string(),
                size: 6,
                is_dir: false,
                mode: 0o644,
                mtime: Some(1_704_164_645),
                atime: None,
                ctime: None,
                uid: 0,
                gid: 0,
                uname: None,
                gname: None,
                symlink: None,
                hardlink: None,
            };
            inner
                .push_archive_entry(seven_z_entry(&meta), Some(&mut &b"hello\n"[..]))
                .expect("could not write alpha.txt");
            inner.finish().expect("could not finish the archive");
        }
        assert!(
            crate::arch::verify_passphrase(&plain, &Secret::from_text("needless"))
                .expect("verification must answer, not error"),
            "a plain 7z has nothing to disagree with, so a needless password must pass — \
             refusing it refuses the archive"
        );

        // And the same arm reached the other way: plaintext headers, every encrypted member
        // empty. The listing succeeds for any password because the headers are in the clear, and
        // there is no member with bytes to test the key against. Both answers must be yes, and
        // that is honest rather than lax — no ciphertext was read, so none was got wrong.
        let hollow = dir.join("hollow.7z");
        write_7z(
            &hollow,
            "indium",
            false,
            &[("empty-one.txt", b"", true), ("empty-two.txt", b"", true)],
        );
        assert!(
            crate::arch::verify_passphrase(&hollow, &Secret::from_text("indium"))
                .expect("verification must answer, not error"),
            "the right password must pass on an archive whose every encrypted member is empty"
        );
        assert!(
            crate::arch::verify_passphrase(&hollow, &Secret::from_text("not-the-password"))
                .expect("verification must answer, not error"),
            "and so must a wrong one, because nothing in the archive can tell them apart — if \
             this ever starts failing, something has begun claiming a verdict it cannot support"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
