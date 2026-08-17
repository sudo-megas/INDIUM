//! The live estimator — CORE §7's **V2.0**, built by P21.
//!
//! §7 asked for "the live estimator: sample the actual input, run the real candidates on
//! the real CPU, report measured time and ratio instead of folklore", and §5 marked the
//! eight method sentences "static in v1.x" until it existed. This module is what makes
//! that clause false.
//!
//! **It owns no format knowledge.** Every candidate is written through the same
//! [`Sink`] Apply writes through — `arch::Writer` for tar and zip, `sevenz::Writer` for
//! 7z — into a scratch file that is measured and deleted. Nothing here knows what a
//! compressor is, which is why a method added to `METHODS` is measured without this file
//! being touched.
//!
//! **What it is honest about.** Below [`BUDGET`] the whole input goes through the real
//! writer in plan order, so the figure is not an estimate at all: it is the size Apply
//! would produce, container overhead included. Above it the input is sampled and every
//! figure is marked, because sampling ratio is not sound and pretending otherwise would
//! be the folklore §7 sent this module to replace.
//!
//! **How unsound, measured.** Thirty-two 64 KiB chunks across this repository's own 6.3 MiB
//! tarball land gzip within **0.7** points and bzip2 within **2.1**, xz **7.0** high and
//! zstd **14.8** low. The error is not noise and it is not fixable by choosing a better
//! chunk size — it is the methods disagreeing about the one property a sample cannot
//! preserve, which is *how long the stream is*. LZMA's 8 MiB dictionary earns its ratio on
//! long-range matches, so chopping the stream destroys them and xz reads worse than it
//! will be. zstd's window at level 3 is 2 MiB, so it cannot span a 6.3 MiB input but spans
//! the whole of a 2 MiB sample — and it reads far better than it will be. Sweeping the
//! geometry confirms there is no setting that pleases both: 32 KiB chunks bring zstd to
//! +1.8 and push xz to +19.5; 256 KiB chunks bring xz to +1.2 and push zstd to −20.3.
//! Throughput samples honestly; ratio does not, in both directions. Hence the mark, and
//! hence the exactness below the budget being worth the wait.
//!
//! **What it never does.** It never encrypts: `sevenz::Writer` refuses `encrypt: true`
//! without a passphrase, and CORE §9 keeps passwords typed per use and never held, so
//! there is no passphrase at Measure time and asking for one to time a compressor would
//! be an outrageous trade. AES-256 is a rounding error beside LZMA2 in any case. And it
//! never writes beside the target — the scratch roots are `platform::scratch`'s, under
//! [`Kind::Estimate`](crate::platform::scratch::Kind::Estimate).
//!
//! Copyright © sudo-megas. GPL-3.0-only.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::Instant;

use crate::arch::Entry;
use crate::secret::Secret;
use crate::tasks::{Container, Meta, Method, Recipe, Sink, METHODS};

/// The measuring budget, and the line between an exact figure and an estimate.
///
/// Two mebibytes because the candidates run **in sequence** — CORE §3 fixes threading at
/// "the UI thread and one worker", so there is no pool to spread them over.
///
/// Measured rather than guessed, by `the_eight_candidates_run_in_sequence_over_a_real_input`
/// against this repository's own `src/` on a Ryzen 5 3450U: **1365 ms for 813 KiB**, of
/// which xz (571) and LZMA2 (566) are 83%. That scales to roughly **3.5 s** at this budget,
/// and doubling the budget doubles that wait for a figure nobody reads twice.
pub const BUDGET: u64 = 2 * 1024 * 1024;

/// One chunk of a stratified sample. Thirty-two of these fill [`BUDGET`].
const CHUNK: u64 = 64 * 1024;

/// How much of an open archive the walk will decompress before it stops.
///
/// Stratifying staged files is free — a file seeks. An archive does not: libarchive
/// decompresses sequentially and even `skip_data` on a solid stream does the work, so
/// spreading a sample across a 2 GiB `.tar.xz` means decompressing 2 GiB before the first
/// candidate runs. The walk goes as far as it can afford and stops here regardless, taking
/// whatever chunks it reached. Without this the feature is unusable on exactly the archives
/// that are worth re-compressing — and the sample it stops with is a head sample, which is
/// why the figures above the budget are marked whatever the walk managed.
pub const WALK_CAP: u64 = 64 * 1024 * 1024;

/// What the estimator says when the flag went up mid-member.
const CANCELLED: &str = "cancelled";

// ---------------------------------------------------------------------------
// What gets compressed
// ---------------------------------------------------------------------------

/// One member on its way into a candidate archive.
pub struct Member {
    pub meta: Meta,
    pub body: Body,
}

/// Where a member's bytes come from. Directories, symlinks and empty files have none.
pub enum Body {
    None,
    /// Read from disk when the candidate runs, so a staged add is never held in memory.
    File(PathBuf),
    /// Already in hand — an archive's entries, which cannot be re-read cheaply.
    Bytes(Vec<u8>),
}

/// The bytes a measurement rests on, and whether they are all of them.
pub enum Input {
    /// Every member, in the order Apply would write them. The figures are exact.
    Whole(Vec<Member>),
    /// One synthetic member holding chunks drawn from across the input. Figures from it
    /// are estimates and are marked as such everywhere they appear.
    ///
    /// A single member rather than truncated real ones on purpose: it holds container
    /// overhead constant across the eight candidates, so even where the absolute ratio
    /// drifts the *comparison* between methods stays fair, which is what the popup is
    /// actually asked to help with.
    Sampled(Vec<u8>),
}

impl Input {
    /// Is every figure drawn from this an estimate?
    pub fn sampled(&self) -> bool {
        matches!(self, Input::Sampled(_))
    }

    /// How many bytes the candidates will be handed.
    pub fn len(&self) -> u64 {
        match self {
            Input::Whole(members) => members.iter().map(|m| m.meta.size).sum(),
            Input::Sampled(bytes) => bytes.len() as u64,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

// ---------------------------------------------------------------------------
// What comes back
// ---------------------------------------------------------------------------

/// One candidate, measured.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Measurement {
    pub method: Method,
    pub level: u32,
    /// Wall-clock for the whole build: opening the writer, every member, and the flush.
    pub millis: u64,
    /// What the finished candidate weighed.
    pub bytes: u64,
    /// What went in, so the ratio can be recomputed without trusting a stored float.
    pub input_bytes: u64,
}

impl Measurement {
    /// Compressed size as a percentage of the input. Smaller is better; `Store` is 100.
    pub fn ratio(&self) -> f32 {
        if self.input_bytes == 0 {
            return 0.0;
        }
        (self.bytes as f32 / self.input_bytes as f32) * 100.0
    }
}

/// What the worker sends back, in this order: one `Began`, then a `One` or `Failed` per
/// method as each lands, then one `Done`. `Fatal` replaces the lot when the input itself
/// could not be read.
pub enum Msg {
    /// The input resolved. Sent before the first candidate so the window can say what it
    /// is measuring while it measures it.
    Began {
        describe: String,
        sampled: bool,
        bytes: u64,
    },
    One(Measurement),
    /// One candidate failed. The rest still run — a method libarchive refuses on this
    /// build is a row that says so, not a dead window.
    Failed {
        method: Method,
        why: String,
    },
    Done,
    Fatal(String),
}

// ---------------------------------------------------------------------------
// Resolving the input
// ---------------------------------------------------------------------------

/// The members a staged queue contributes, in the order Apply would write them.
///
/// `expand_add` is the same walk Apply uses, so a staged directory contributes the same
/// tree here as it will there — the estimator does not get to see a different archive
/// from the one it is describing.
pub fn from_staged(adds: &[(PathBuf, String)]) -> Result<(Vec<Member>, u64), String> {
    let mut members = Vec::new();
    let mut total = 0u64;
    for (source, dest) in adds {
        let items = crate::tasks::expand_add(source, dest)
            .map_err(|e| format!("could not read {}: {e}", source.display()))?;
        for item in items {
            let meta = crate::tasks::meta_from_fs(&item.source, &item.out_path)?;
            total += meta.size;
            let body = if meta.is_dir || meta.symlink.is_some() || meta.size == 0 {
                Body::None
            } else {
                Body::File(item.source.clone())
            };
            members.push(Member { meta, body });
        }
    }
    Ok((members, total))
}

/// Turn a member list into what will actually be compressed.
///
/// Under budget every member goes through untouched. Over it, the members are replaced by
/// one synthetic member holding chunks lifted from across them — which is why this reads
/// files rather than merely listing them.
pub fn narrow(members: Vec<Member>, total: u64) -> Result<Input, String> {
    if total <= BUDGET {
        return Ok(Input::Whole(members));
    }

    // Where in the virtual concatenation each chunk is taken from. An even stride rather
    // than the first N bytes: the head of a tarball is source text and its tail is
    // already-compressed images, and a head sample of this repository's own tarball
    // predicted zstd at 28.5% where the truth was 54.0%. Sampling the head does not
    // measure the input, it measures the input's first chapter.
    let want = BUDGET / CHUNK;
    let stride = (total / want).max(1);

    let mut out: Vec<u8> = Vec::with_capacity(BUDGET as usize);
    let mut cursor = 0u64; // virtual offset of the current member's first byte
    let mut next = 0u64; // virtual offset of the next chunk to take
    let mut owed = 0u64; // bytes still due on a chunk that ran off the end of a member

    for m in &members {
        if out.len() as u64 >= BUDGET {
            break;
        }
        let size = m.meta.size;
        if size == 0 {
            continue;
        }
        let end = cursor + size;

        // A chunk is 64 KiB of the *concatenation*, not of whichever member it happened to
        // start in, so one that runs off the end of a member is finished at the start of
        // the next. Stopping at the boundary instead would hand back an 8 KiB scrap
        // wherever the members are small and a full 64 KiB wherever they are large — which
        // weights the sample by member count rather than by bytes, and quietly undoes the
        // even stride that the whole of this function exists to walk.
        if owed > 0 {
            let take = owed.min(size).min(BUDGET - out.len() as u64);
            let before = out.len();
            read_into(&m.body, 0, take, &mut out)?;
            owed -= (out.len() - before) as u64;
            if owed == 0 {
                next += stride;
            }
        }

        while owed == 0 && next < end && (out.len() as u64) < BUDGET {
            let within = next - cursor;
            let want = CHUNK.min(BUDGET - out.len() as u64);
            let before = out.len();
            read_into(&m.body, within, want.min(size - within), &mut out)?;
            let got = (out.len() - before) as u64;
            if got == 0 {
                break;
            }
            if got < want {
                owed = want - got;
            } else {
                next += stride;
            }
        }
        cursor = end;
    }

    Ok(Input::Sampled(out))
}

/// Lift `len` bytes at `offset` out of one member's body.
fn read_into(body: &Body, offset: u64, len: u64, out: &mut Vec<u8>) -> Result<(), String> {
    match body {
        Body::None => Ok(()),
        Body::Bytes(b) => {
            let start = (offset as usize).min(b.len());
            let end = (start + len as usize).min(b.len());
            out.extend_from_slice(&b[start..end]);
            Ok(())
        }
        Body::File(p) => {
            use std::io::Seek;
            let mut f = std::fs::File::open(p)
                .map_err(|e| format!("could not read {}: {e}", p.display()))?;
            f.seek(std::io::SeekFrom::Start(offset))
                .map_err(|e| format!("could not read {}: {e}", p.display()))?;
            let before = out.len();
            out.resize(before + len as usize, 0);
            let mut got = 0usize;
            while got < len as usize {
                match f.read(&mut out[before + got..]) {
                    Ok(0) => break,
                    Ok(n) => got += n,
                    Err(e) => return Err(format!("could not read {}: {e}", p.display())),
                }
            }
            out.truncate(before + got);
            Ok(())
        }
    }
}

/// The bytes an already-open archive contributes, ready to measure.
///
/// One sequential pass, because that is the only kind libarchive offers. `entries` is the
/// listing the window already holds, so this does not pay for a second walk to discover
/// what it is about to read.
///
/// Unlike the staged path this hands back a finished [`Input`] rather than members for
/// [`narrow`] to reduce, because the two cannot be separated here: a member's bytes exist
/// only while the walk is standing on it. Handing back members *and* the archive's total
/// invites the reduction to run twice — which is exactly what it did, striding 580 KiB of
/// held bytes with a 190 KiB stride computed from the 6 MiB the bytes were drawn from, and
/// throwing away seven eighths of what the walk had just paid to decompress.
pub fn from_archive(
    path: &Path,
    entries: &[Entry],
    passphrase: Option<&Secret>,
    cancel: &AtomicBool,
) -> Result<Input, String> {
    walk(path, entries, passphrase, cancel, WALK_CAP)
}

/// [`from_archive`] with the cap given rather than assumed.
///
/// The cap is a parameter for one reason: at 64 MiB the only input that exercises it is one
/// too big to build in a test, so a cap that never fired would test green forever. Handed a
/// small cap, the same walk over a small archive proves the same property.
fn walk(
    path: &Path,
    entries: &[Entry],
    passphrase: Option<&Secret>,
    cancel: &AtomicBool,
    cap: u64,
) -> Result<Input, String> {
    /// The same predicate `Meta::has_data` applies, against a listing rather than a
    /// member: a directory, a link and an empty file all carry no stream to compress.
    fn carries_data(e: &Entry) -> bool {
        !e.is_dir && e.symlink.is_none() && e.hardlink.is_none() && e.size > 0
    }

    let total: u64 = entries
        .iter()
        .filter(|e| carries_data(e))
        .map(|e| e.size)
        .sum();
    let mut reader = crate::arch::Reader::open(path, passphrase).map_err(|e| e.to_string())?;

    // Under the budget there is nothing to choose. Every member goes in whole, through the
    // real writer, and the figures are not estimates at all.
    if total <= BUDGET {
        let mut members = Vec::new();
        while let Some(entry) = reader.next_entry().map_err(|e| e.to_string())? {
            if cancel.load(Ordering::Relaxed) {
                return Err(CANCELLED.to_string());
            }
            if !carries_data(&entry) {
                reader.skip_data();
                continue;
            }
            let mut buf = Vec::with_capacity(entry.size as usize);
            crate::arch::EntryData::new(&mut reader)
                .take(entry.size)
                .read_to_end(&mut buf)
                .map_err(|e| e.to_string())?;
            let mut meta = Meta::from_entry(&entry, &entry.path, None);
            meta.size = buf.len() as u64;
            members.push(Member {
                meta,
                body: Body::Bytes(buf),
            });
        }
        return Ok(Input::Whole(members));
    }

    // Over the budget: 64 KiB chunks at even **byte** positions across the members'
    // concatenation — the same stride `narrow` walks for staged files, run against a stream
    // instead of against seekable files, so both paths tell one story.
    //
    // Byte positions rather than entry positions, and that distinction is the whole of it.
    // Striding the entry *list* weights the sample by how many members a class has instead
    // of by how many bytes it owns, and an archive of source is always hundreds of small
    // text files beside a handful of large binaries. This repository's own tarball is
    // eighty-odd text members and eight binary ones: an even sample of that list is nearly
    // all text, and the ratio it predicts belongs to an archive nobody has. Measured at the
    // window, it read zstd at 37.7% where the truth is 54.6% and bzip2 at 31.5% where the
    // truth is 49.4% — seventeen points optimistic, which is worse than the folklore
    // sentence it was built to replace, because it looks like a measurement. Chunks landing
    // at even byte offsets reproduce the archive's composition by construction: a member is
    // sampled in proportion to the bytes it actually contributes, and no member needs to be
    // chosen at all.
    let stride = (total / (BUDGET / CHUNK)).max(1);

    let mut out: Vec<u8> = Vec::with_capacity(BUDGET as usize);
    let mut cursor = 0u64; // virtual offset of the current member's first byte
    let mut next = 0u64; // virtual offset of the next chunk to take
    let mut owed = 0u64; // bytes still due on a chunk that ran off the end of a member
    let mut walked = 0u64;

    while let Some(entry) = reader.next_entry().map_err(|e| e.to_string())? {
        if cancel.load(Ordering::Relaxed) {
            return Err(CANCELLED.to_string());
        }
        if walked >= cap || out.len() as u64 >= BUDGET {
            break;
        }
        if !carries_data(&entry) {
            reader.skip_data();
            continue;
        }
        // Charged for every member the walk passes and not only for the ones it reads:
        // libarchive runs the filter sequentially, so `skip_data` pushes those bytes
        // through the decompressor exactly as a read would. Charging only what is kept
        // leaves the cap unreachable on precisely the archive it exists to bound — a 2 GiB
        // stream whose sampled chunks weigh two megabytes between them would sail past a
        // 64 MiB cap while decompressing all 2 GiB to reach the last of them.
        walked += entry.size;

        let end = cursor + entry.size;
        if owed == 0 && next >= end {
            reader.skip_data();
            cursor = end;
            continue;
        }

        {
            let mut data = crate::arch::EntryData::new(&mut reader);
            let mut pos = 0u64; // bytes of this member already consumed

            // Finish a chunk that began in an earlier member. A chunk is 64 KiB of the
            // concatenation, not of whichever member it started in — see `narrow`, which
            // carries the same debt across the same boundary for the same reason.
            if owed > 0 {
                let take = owed.min(entry.size).min(BUDGET - out.len() as u64);
                let before = out.len();
                (&mut data)
                    .take(take)
                    .read_to_end(&mut out)
                    .map_err(|e| e.to_string())?;
                let got = (out.len() - before) as u64;
                pos += got;
                owed -= got;
                if owed == 0 {
                    next += stride;
                }
            }

            while owed == 0 && next < end && (out.len() as u64) < BUDGET {
                // Forward to the chunk. A stream cannot seek, so reaching an offset means
                // reading to it and throwing the result away — the decompression is paid
                // for either way, which is what `cap` above is counting.
                let at = next - cursor;
                if at > pos {
                    pos += discard(&mut data, at - pos, cancel)?;
                    if pos < at {
                        break; // the member ended earlier than the listing claimed
                    }
                }
                let want = CHUNK.min(BUDGET - out.len() as u64);
                let before = out.len();
                (&mut data)
                    .take(want.min(end - next))
                    .read_to_end(&mut out)
                    .map_err(|e| e.to_string())?;
                let got = (out.len() - before) as u64;
                if got == 0 {
                    break;
                }
                pos += got;
                if got < want {
                    owed = want - got;
                } else {
                    next += stride;
                }
            }
        }
        cursor = end;
    }

    Ok(Input::Sampled(out))
}

/// Read `n` bytes out of a member and throw them away, returning how many there were.
///
/// Fewer than asked for means the member ended early. It goes a megabyte at a time so that
/// cancelling does not have to wait for a 400 MiB member to finish being ignored — the same
/// reason [`Cancellable`] exists on the writing side.
fn discard<R: Read>(data: &mut R, n: u64, cancel: &AtomicBool) -> Result<u64, String> {
    const STEP: u64 = 1024 * 1024;
    let mut done = 0u64;
    let mut sink = std::io::sink();
    while done < n {
        if cancel.load(Ordering::Relaxed) {
            return Err(CANCELLED.to_string());
        }
        let got = std::io::copy(&mut data.by_ref().take(STEP.min(n - done)), &mut sink)
            .map_err(|e| e.to_string())?;
        if got == 0 {
            break;
        }
        done += got;
    }
    Ok(done)
}

// ---------------------------------------------------------------------------
// Measuring
// ---------------------------------------------------------------------------

/// A reader that stops dead when the flag goes up.
///
/// Cancellation is otherwise only checked between members, and one 2 MiB member under
/// xz is over a second of uninterruptible work — long enough that closing the popup would
/// visibly fail to close it. Returning `Ok(0)` ends the stream early; the candidate is
/// abandoned and its file deleted, so a short read never becomes a reported figure.
struct Cancellable<'a, R> {
    inner: R,
    cancel: &'a AtomicBool,
}

impl<R: Read> Read for Cancellable<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.cancel.load(Ordering::Relaxed) {
            return Ok(0);
        }
        self.inner.read(buf)
    }
}

/// The level a candidate is measured at.
///
/// Each method at what the popup would actually build with it — its own default — except
/// the one currently selected, which is measured at the level the slider is on. Anything
/// else would report a figure for a build the user cannot ask for.
pub fn level_for(method: Method, selected: Option<(Method, u32)>) -> u32 {
    match selected {
        Some((m, level)) if m == method => method.clamp_level(level),
        _ => method.default_level(),
    }
}

/// Build one candidate and time it. The scratch file is removed before returning,
/// whatever happened.
fn measure(
    input: &Input,
    method: Method,
    level: u32,
    dir: &Path,
    cancel: &AtomicBool,
) -> Result<Measurement, String> {
    let path = dir.join("candidate");
    let _ = std::fs::remove_file(&path);

    let recipe = Recipe {
        path: path.clone(),
        method,
        level,
        // Never encrypted. See this module's own note: there is no password at Measure
        // time by CORE §9's design, and AES-256 is not what the figures are about.
        encrypt: false,
    };

    let started = Instant::now();
    let outcome = build(input, &recipe, &path, cancel);
    let millis = started.elapsed().as_millis() as u64;

    let result = outcome.and_then(|()| {
        std::fs::metadata(&path)
            .map(|md| md.len())
            .map_err(|e| format!("the candidate went missing: {e}"))
    });
    let _ = std::fs::remove_file(&path);

    Ok(Measurement {
        method,
        level,
        millis,
        bytes: result?,
        input_bytes: input.len(),
    })
}

/// Drive one `Sink` to completion over the input.
fn build(input: &Input, recipe: &Recipe, path: &Path, cancel: &AtomicBool) -> Result<(), String> {
    let mut sink: Box<dyn Sink> = match recipe.container() {
        Container::SevenZ => Box::new(crate::sevenz::Writer::create(path, recipe, None)?),
        _ => Box::new(crate::arch::Writer::create(path, recipe)?),
    };

    let fed = match input {
        Input::Whole(members) => feed(sink.as_mut(), members, cancel),
        Input::Sampled(bytes) => {
            let meta = Meta {
                out_path: "sample".to_string(),
                size: bytes.len() as u64,
                is_dir: false,
                mode: 0o644,
                mtime: None,
                atime: None,
                ctime: None,
                uid: 0,
                gid: 0,
                uname: None,
                gname: None,
                symlink: None,
                hardlink: None,
            };
            let mut body = Cancellable {
                inner: std::io::Cursor::new(&bytes[..]),
                cancel,
            };
            sink.put(&meta, Some(&mut body))
        }
    };

    if let Err(e) = fed {
        sink.abandon();
        return Err(e);
    }
    if cancel.load(Ordering::Relaxed) {
        sink.abandon();
        return Err(CANCELLED.to_string());
    }
    sink.finish()
}

fn feed(sink: &mut dyn Sink, members: &[Member], cancel: &AtomicBool) -> Result<(), String> {
    for m in members {
        if cancel.load(Ordering::Relaxed) {
            return Err(CANCELLED.to_string());
        }
        match &m.body {
            Body::None => sink.put(&m.meta, None)?,
            Body::Bytes(b) => {
                let mut body = Cancellable {
                    inner: std::io::Cursor::new(&b[..]),
                    cancel,
                };
                sink.put(&m.meta, Some(&mut body))?;
            }
            Body::File(p) => {
                let f = std::fs::File::open(p)
                    .map_err(|e| format!("could not read {}: {e}", p.display()))?;
                let mut body = Cancellable { inner: f, cancel };
                sink.put(&m.meta, Some(&mut body))?;
            }
        }
    }
    Ok(())
}

/// The worker. One thread, eight candidates, **in sequence** — CORE §3 has one worker and
/// this is what keeps it to one — sending each figure the moment it lands so the rows fill
/// as they are earned rather than all at once at the end.
/// `wake` is called after every send. A worker holds no `egui::Context` and an idle
/// INDIUM repaints nothing (CORE §3), so without it the rows would appear only when the
/// mouse happened to move. It is a closure rather than a `Context` so this module goes on
/// knowing nothing about the window.
pub fn run(
    input: Input,
    describe: String,
    dir: PathBuf,
    selected: Option<(Method, u32)>,
    tx: &Sender<Msg>,
    cancel: &Arc<AtomicBool>,
    wake: &dyn Fn(),
) {
    let _ = tx.send(Msg::Began {
        describe,
        sampled: input.sampled(),
        bytes: input.len(),
    });
    wake();

    for method in METHODS {
        if cancel.load(Ordering::Relaxed) {
            return;
        }
        let level = level_for(method, selected);
        match measure(&input, method, level, &dir, cancel) {
            Ok(m) => {
                let _ = tx.send(Msg::One(m));
            }
            Err(why) if why == CANCELLED => return,
            Err(why) => {
                let _ = tx.send(Msg::Failed { method, why });
            }
        }
        wake();
    }
    let _ = tx.send(Msg::Done);
    wake();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A directory of this test's own.
    ///
    /// Not `temp_dir()` itself: `measure` writes and deletes a file called `candidate`,
    /// and two tests sharing that name delete each other's — which is the very collision
    /// P8 fixed in `platform::scratch` by putting the process id in the name, arrived at
    /// here the same way, by watching one test remove the other's file.
    fn scratch_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("indium-p21-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a temp directory can be made");
        dir
    }

    /// A member list whose bodies are held, for the tests that do not touch the disk.
    /// One member of `n` bytes, every one of them `fill`, so a sample can be asked which
    /// *class* of member it was drawn from rather than merely which member.
    fn filled(name: &str, fill: u8, n: usize) -> Member {
        let (mut members, _) = held(&[n]);
        let mut member = members.remove(0);
        member.meta.out_path = name.to_string();
        member.body = Body::Bytes(vec![fill; n]);
        member
    }

    fn held(sizes: &[usize]) -> (Vec<Member>, u64) {
        let mut members = Vec::new();
        let mut total = 0u64;
        for (i, &n) in sizes.iter().enumerate() {
            // Each member is filled with its own index, so a sample can be read back and
            // asked *which* members it came from.
            let bytes = vec![i as u8; n];
            total += n as u64;
            members.push(Member {
                meta: Meta {
                    out_path: format!("m{i}"),
                    size: n as u64,
                    is_dir: false,
                    mode: 0o644,
                    mtime: None,
                    atime: None,
                    ctime: None,
                    uid: 0,
                    gid: 0,
                    uname: None,
                    gname: None,
                    symlink: None,
                    hardlink: None,
                },
                body: Body::Bytes(bytes),
            });
        }
        (members, total)
    }

    #[test]
    fn an_input_within_the_budget_is_measured_whole_and_is_not_an_estimate() {
        let (members, total) = held(&[1024, 2048]);
        let input = narrow(members, total).expect("narrowing a small input cannot fail");
        assert!(!input.sampled(), "under budget, nothing is an estimate");
        assert_eq!(input.len(), total);
    }

    #[test]
    fn an_input_over_the_budget_is_sampled_and_says_so() {
        let (members, total) = held(&[BUDGET as usize + 1]);
        let input = narrow(members, total).expect("narrowing cannot fail");
        assert!(input.sampled(), "over budget, every figure is an estimate");
    }

    #[test]
    fn the_sample_never_exceeds_the_budget() {
        for over in [BUDGET + 1, BUDGET * 4, BUDGET * 64] {
            let (members, total) = held(&[over as usize]);
            let input = narrow(members, total).expect("narrowing cannot fail");
            assert!(
                input.len() <= BUDGET,
                "a {over}-byte input sampled {} bytes, over the {BUDGET} budget",
                input.len()
            );
        }
    }

    /// The finding P21 was designed around, pinned so it cannot quietly regress.
    ///
    /// Each member is filled with its own index, so the sample's *contents* say which
    /// members it was drawn from. A sampler that took the head would return bytes from
    /// the first member only and this fails; the stride reaches the last one.
    #[test]
    fn the_sample_is_drawn_from_across_the_input_and_not_from_its_head() {
        let count = 40usize;
        let (members, total) = held(&vec![BUDGET as usize / 8; count]);
        let input = narrow(members, total).expect("narrowing cannot fail");
        let Input::Sampled(bytes) = input else {
            panic!("an input this size must be sampled");
        };

        let mut seen: Vec<u8> = bytes.clone();
        seen.sort_unstable();
        seen.dedup();
        assert!(
            seen.len() > 1,
            "the sample came from one member — that is a head sample, not a stratified one"
        );
        assert!(
            seen.iter().any(|&m| m as usize >= count / 2),
            "nothing was drawn from the second half of the input: saw members {seen:?}"
        );
    }

    #[test]
    fn every_method_the_popup_offers_is_measured_at_a_level_that_method_allows() {
        for method in METHODS {
            let level = level_for(method, None);
            assert_eq!(
                level,
                method.clamp_level(level),
                "{} would be measured at a level it does not accept",
                method.label()
            );
        }
    }

    /// The selected method is measured at the slider's value; the other seven are not.
    #[test]
    fn the_selected_method_is_measured_at_the_level_the_window_is_showing() {
        let selected = Some((Method::Zstd, 19));
        assert_eq!(level_for(Method::Zstd, selected), 19);
        assert_eq!(
            level_for(Method::Gzip, selected),
            Method::Gzip.default_level(),
            "an unselected method is measured at what the popup would build with it"
        );
    }

    /// CORE §9 keeps passwords typed per use and never held, so there is none to measure
    /// with. A candidate that asked for one could not run at all.
    #[test]
    fn a_candidate_is_never_encrypted() {
        let dir = scratch_dir("never-encrypted");
        let (members, total) = held(&[64]);
        let input = narrow(members, total).expect("narrowing cannot fail");
        let cancel = AtomicBool::new(false);
        // Lzma2 is the one method that *can* encrypt, and the one whose writer refuses to
        // open at all when asked to encrypt with no password. That it measures proves the
        // recipe this module builds carries `encrypt: false`.
        let m = measure(
            &input,
            Method::Lzma2,
            Method::Lzma2.default_level(),
            &dir,
            &cancel,
        )
        .expect("a 7z candidate builds without a password");
        assert!(m.bytes > 0, "the candidate weighed nothing");
        assert!(
            !dir.join("candidate").exists(),
            "the scratch file outlived the measurement"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_cancelled_measurement_reports_nothing_and_leaves_no_scratch_file() {
        let dir = scratch_dir("cancelled");
        let (members, total) = held(&[4096]);
        let input = narrow(members, total).expect("narrowing cannot fail");
        let cancel = AtomicBool::new(true);
        // A figure from a candidate that was abandoned part-built would be a smaller
        // archive than the method actually produces — the most flattering possible lie
        // about it. So cancellation refuses to report at all rather than reporting early.
        let outcome = measure(&input, Method::Gzip, 6, &dir, &cancel);
        assert!(
            outcome.is_err(),
            "a cancelled measurement reported a figure: {outcome:?}"
        );
        assert!(
            !dir.join("candidate").exists(),
            "the scratch file outlived a cancellation"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The round's central claim, checked against the program rather than against a shell
    /// pipe: **under budget, the figure is not an estimate.**
    ///
    /// It is supposed to be the size Apply would produce, container overhead and all. So
    /// the same members are written twice — once by `measure`, once straight through the
    /// writer — and the two sizes must agree to the byte. What this catches is everything
    /// `measure` wraps around the writer: a candidate weighed before its flush, a stale
    /// file left from the previous method and measured again, a timing wrapper that
    /// truncates. What it does not claim to catch is a fault in the writer itself, which
    /// is the write path's own tests' business.
    ///
    /// It also drives the directory branch of `from_staged`, which is the one that has to
    /// walk a tree rather than stat a file.
    #[test]
    fn an_exact_measurement_is_the_size_the_writer_actually_produces() {
        let dir = scratch_dir("exact");
        let src = dir.join("tree");
        std::fs::create_dir_all(src.join("nested")).expect("a tree can be made");
        for i in 0..8usize {
            // Compressible, and different per file, so the result is not a fixed-size
            // header that would match by accident.
            let body = "the quick brown fox jumps over the lazy dog ".repeat(200 + i * 30);
            let at = if i % 2 == 0 {
                src.join(format!("f{i}.txt"))
            } else {
                src.join("nested").join(format!("f{i}.txt"))
            };
            std::fs::write(at, body).expect("a fixture file can be written");
        }

        let (members, total) =
            from_staged(&[(src.clone(), "tree".to_string())]).expect("the tree is readable");
        assert!(
            total > 0 && total <= BUDGET,
            "the fixture must have bytes and fit the budget to be exact; it is {total}"
        );
        let input = narrow(members, total).expect("narrowing cannot fail");
        assert!(
            !input.sampled(),
            "a fixture this size must be measured whole"
        );

        let cancel = AtomicBool::new(false);
        let measured =
            measure(&input, Method::Gzip, 6, &dir, &cancel).expect("gzip measures a real tree");

        // The same members again, by hand, through the writer Apply uses.
        let out = dir.join("by-hand.tar.gz");
        let recipe = Recipe {
            path: out.clone(),
            method: Method::Gzip,
            level: 6,
            encrypt: false,
        };
        {
            let Input::Whole(ref ms) = input else {
                unreachable!("asserted above")
            };
            let mut sink = crate::arch::Writer::create(&out, &recipe).expect("the writer opens");
            feed(&mut sink, ms, &cancel).expect("the members are written");
            sink.finish().expect("the archive flushes");
        }
        let by_hand = std::fs::metadata(&out).expect("the archive exists").len();

        assert_eq!(
            measured.bytes, by_hand,
            "the measured size is not the size the writer produces"
        );
        assert!(
            measured.ratio() > 0.0 && measured.ratio() < 100.0,
            "text this repetitive must compress: got {}%",
            measured.ratio()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The whole worker, end to end, over a real input — and the harness the figures in
    /// P21.md were taken with.
    ///
    /// Ignored by default because it is the one test that costs what the feature costs:
    /// eight candidates in sequence, which is the point, and several seconds even in
    /// release. Run it deliberately:
    ///
    /// ```text
    /// cargo test --release --lib the_eight_candidates -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "measures eight real candidates; run with --release --ignored --nocapture"]
    fn the_eight_candidates_run_in_sequence_over_a_real_input() {
        let dir = scratch_dir("eight");
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));

        // This repository's own source, which is what an estimator is for: real text, real
        // repetition, and a size that has to be sampled rather than weighed whole.
        let (members, total) = from_staged(&[(root.join("src"), "src".to_string())])
            .expect("this repository's own src/ is readable");
        let sampled = total > BUDGET;
        let input = narrow(members, total).expect("narrowing cannot fail");

        let (tx, rx) = std::sync::mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        run(
            input,
            "src/".to_string(),
            dir.clone(),
            None,
            &tx,
            &cancel,
            &|| {},
        );
        drop(tx);

        println!(
            "\n  input {} ({}), {}\n",
            crate::util::format_bytes(total),
            if sampled { "sampled" } else { "whole" },
            root.display()
        );
        let mut seen = 0usize;
        let mut done = false;
        for msg in rx {
            match msg {
                Msg::One(m) => {
                    seen += 1;
                    println!("  {:<8} {}", m.method.label(), figure_line(&m, sampled));
                }
                Msg::Failed { method, why } => {
                    println!("  {:<8} unavailable: {why}", method.label())
                }
                Msg::Done => done = true,
                Msg::Fatal(why) => panic!("the estimator gave up: {why}"),
                Msg::Began { .. } => {}
            }
        }
        println!();

        assert!(done, "the worker never reported Done");
        assert_eq!(seen, METHODS.len(), "not every method reported a figure");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The same shape the popup's lane shows, so the harness prints what the window prints.
    fn figure_line(m: &Measurement, sampled: bool) -> String {
        let mark = if sampled { '~' } else { ' ' };
        format!(
            "level {:>2} · {:>6} ms · {mark}{:>5.1}%",
            m.level,
            m.millis,
            m.ratio()
        )
    }

    /// The archive source, over budget: read across the entry list and never past it.
    ///
    /// The fixture is **built here rather than committed** — every fixture in the tree is
    /// kilobytes, and a walk that wrongly took the first entries would pass against all of
    /// them. Writing this test is what found exactly that: a stride of `len / want` rounds
    /// down to 1 at forty members, which selects the first thirty-two and calls it a
    /// spread. The index mapping in `from_archive` is the fix, and this is what holds it.
    #[test]
    fn an_archive_over_the_budget_is_read_across_its_members_and_not_from_its_first_few() {
        let dir = scratch_dir("archive-walk");
        let path = dir.join("big.tar.gz");

        // Forty members at an eighth of the budget each: five mebibytes, comfortably over,
        // and each filled with its own index so what comes back says where it came from.
        let count = 40usize;
        let (members, _) = held(&vec![(BUDGET / 16) as usize; count]);
        let recipe = Recipe {
            path: path.clone(),
            method: Method::Gzip,
            level: 1,
            encrypt: false,
        };
        {
            let cancel = AtomicBool::new(false);
            let mut sink = crate::arch::Writer::create(&path, &recipe).expect("the writer opens");
            feed(&mut sink, &members, &cancel).expect("the members are written");
            sink.finish().expect("the archive flushes");
        }

        let entries = crate::arch::list_all(&path, None).expect("the archive lists");
        let cancel = AtomicBool::new(false);
        let input = from_archive(&path, &entries, None, &cancel).expect("the archive is readable");

        assert!(
            input.sampled(),
            "an archive far over the budget cannot be measured exactly"
        );
        assert!(
            input.len() <= BUDGET,
            "the walk held {} bytes, over the {BUDGET} budget",
            input.len()
        );

        // And it **spanned** the stream rather than filling from the front. Reaching "the
        // second half" is not the test: a stride that lands in the first thirty-two members
        // of forty satisfies that while never seeing the last eight. What separates a
        // spread from a head sample is how far the *last* chunk sits.
        let Input::Sampled(bytes) = &input else {
            panic!("an over-budget archive must come back sampled");
        };
        let seen: std::collections::BTreeSet<usize> = bytes.iter().map(|&b| b as usize).collect();
        let furthest = *seen.iter().next_back().expect("something was read");
        assert!(
            furthest >= count - count / 8,
            "the walk stopped at member {furthest} of {count} — that is a head sample of \
             the stream, not a spread across it: {seen:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The sample reproduces the archive's **composition**, not its member count.
    ///
    /// This is the property that was missing, and its absence is why the spread test and the
    /// budget test both stood green over a sampler that was seventeen points wrong at the
    /// window. Spreading across the entry *list* weights a class of member by how many files
    /// it has rather than by how many bytes it owns — and every source archive is hundreds of
    /// small text files beside a handful of large binaries, so a list-even sample is nearly
    /// all text and predicts a ratio for an archive nobody has.
    ///
    /// The fixture is that shape on purpose: a hundred small members against four large ones,
    /// so the two weightings disagree by a mile. By bytes the large class owns 83.7% of the
    /// archive; by member count it owns 3.8%. Striding the list sampled it at 21%. Striding
    /// the bytes puts it within a couple of points of the truth, which is the only version of
    /// this the window can print without lying.
    #[test]
    fn the_sample_carries_the_archives_proportions_and_not_its_member_count() {
        let dir = scratch_dir("archive-mix");
        let path = dir.join("mixed.tar.gz");

        // Two classes, told apart by the byte they are filled with rather than by name.
        const SMALL: u8 = 1;
        const LARGE: u8 = 2;
        let small_each = 8 * 1024u64;
        let large_each = 1024 * 1024u64;
        let smalls = 100u64;
        let larges = 4u64;

        let mut members = Vec::new();
        for i in 0..smalls {
            members.push(filled(&format!("s{i}"), SMALL, small_each as usize));
        }
        for i in 0..larges {
            members.push(filled(&format!("l{i}"), LARGE, large_each as usize));
        }
        let recipe = Recipe {
            path: path.clone(),
            method: Method::Gzip,
            level: 1,
            encrypt: false,
        };
        {
            let cancel = AtomicBool::new(false);
            let mut sink = crate::arch::Writer::create(&path, &recipe).expect("the writer opens");
            feed(&mut sink, &members, &cancel).expect("the members are written");
            sink.finish().expect("the archive flushes");
        }

        let entries = crate::arch::list_all(&path, None).expect("the archive lists");
        let cancel = AtomicBool::new(false);
        let input = from_archive(&path, &entries, None, &cancel).expect("the archive is readable");
        let Input::Sampled(bytes) = &input else {
            panic!("an over-budget archive must come back sampled");
        };

        let total = smalls * small_each + larges * large_each;
        let truth = (larges * large_each) as f64 / total as f64 * 100.0;
        let drawn =
            bytes.iter().filter(|&&b| b == LARGE).count() as f64 / bytes.len() as f64 * 100.0;
        assert!(
            (drawn - truth).abs() <= 8.0,
            "the large members are {truth:.1}% of the archive's bytes but {drawn:.1}% of the \
             sample — the sample is weighted by how many members each class has, not by how \
             many bytes it owns, and every ratio drawn from it belongs to a different archive"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The walk stops for the members it *skipped*, not only for the ones it kept.
    ///
    /// This is the property [`WALK_CAP`] exists for and the one it did not have. Skipping a
    /// member is not free — libarchive decompresses sequentially, so `skip_data` pushes the
    /// bytes through the filter exactly as a read does — and a cap charged only for what it
    /// keeps never fires on the archive it was written for: thirty-two sampled members of a
    /// 2 GiB stream weigh a couple of megabytes between them, so the walk would decompress
    /// all 2 GiB and report a cap that was never approached.
    ///
    /// Two hundred members make the two accountings say different things loudly. The chunks
    /// land six members apart, so five are passed for every one read: charged properly the
    /// cap falls once sixteen members have gone by and the furthest chunk came from about
    /// the twelfth; charged only for reads it takes thirty-two chunks to spend, by which
    /// time the walk has passed all two hundred.
    ///
    /// Two hundred rather than more because `held` fills each member with its own index and
    /// a `u8` stops counting at 256: a fixture wide enough to wrap would report member 400
    /// as member 144 and the assertion would be reading noise.
    #[test]
    fn the_walk_pays_for_the_members_it_skips_and_stops_when_it_has_spent_the_cap() {
        let dir = scratch_dir("archive-cap");
        let path = dir.join("many.tar.gz");

        let count = 200usize;
        let each = 16 * 1024usize;
        let cap = 256 * 1024u64; // sixteen members' worth of walking
        let (members, _) = held(&vec![each; count]);
        let recipe = Recipe {
            path: path.clone(),
            method: Method::Gzip,
            level: 1,
            encrypt: false,
        };
        {
            let cancel = AtomicBool::new(false);
            let mut sink = crate::arch::Writer::create(&path, &recipe).expect("the writer opens");
            feed(&mut sink, &members, &cancel).expect("the members are written");
            sink.finish().expect("the archive flushes");
        }

        let entries = crate::arch::list_all(&path, None).expect("the archive lists");
        let cancel = AtomicBool::new(false);
        let input = walk(&path, &entries, None, &cancel, cap).expect("the archive is readable");
        let Input::Sampled(bytes) = &input else {
            panic!("an over-budget archive must come back sampled");
        };

        let seen: std::collections::BTreeSet<usize> = bytes.iter().map(|&b| b as usize).collect();
        let furthest = *seen.iter().next_back().expect("something was read");
        assert!(
            furthest < count / 4,
            "the walk reached member {furthest} of {count} on a cap worth sixteen of \
             them — it is charging itself only for the members it read and decompressing \
             the rest for free, which is the one thing the cap exists to stop: {seen:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// An archive that fits the budget is read whole, and is not an estimate.
    #[test]
    fn an_archive_within_the_budget_is_read_whole() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/basic.tar.gz");
        let entries = crate::arch::list_all(&root, None).expect("the fixture lists");
        let cancel = AtomicBool::new(false);
        let input = from_archive(&root, &entries, None, &cancel).expect("the fixture is readable");

        let listed: u64 = entries
            .iter()
            .filter(|e| !e.is_dir && e.symlink.is_none() && e.hardlink.is_none() && e.size > 0)
            .map(|e| e.size)
            .sum();
        assert!(!input.sampled(), "a fixture this small is not an estimate");
        assert_eq!(
            input.len(),
            listed,
            "a fixture this small must be read whole"
        );
    }

    /// CORE §9 refuses in-place archive writes, and a measurement is not an exception.
    /// Everything a candidate touches lives inside the directory it was handed.
    #[test]
    fn measuring_writes_nothing_outside_the_directory_it_was_given() {
        let dir = scratch_dir("contained");
        let beside = dir.join("beside");
        std::fs::create_dir_all(&beside).expect("a sibling directory can be made");
        let target = beside.join("photos.tar.gz");
        std::fs::write(&target, b"an archive that must not be touched")
            .expect("the decoy can be written");

        let work = dir.join("work");
        std::fs::create_dir_all(&work).expect("a work directory can be made");
        let (members, total) = held(&[8192]);
        let input = narrow(members, total).expect("narrowing cannot fail");
        let cancel = AtomicBool::new(false);
        for method in METHODS {
            let _ = measure(&input, method, level_for(method, None), &work, &cancel);
        }

        assert_eq!(
            std::fs::read(&target).expect("the decoy still exists"),
            b"an archive that must not be touched",
            "a measurement wrote over a file beside it"
        );
        assert_eq!(
            std::fs::read_dir(&beside).expect("readable").count(),
            1,
            "a measurement left something in a directory that was not its own"
        );
        // And its own directory is left empty: every candidate cleans up after itself.
        assert_eq!(
            std::fs::read_dir(&work).expect("readable").count(),
            0,
            "a candidate file outlived the measurement that made it"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_ratio_is_a_percentage_of_what_went_in() {
        let m = Measurement {
            method: Method::Zstd,
            level: 3,
            millis: 17,
            bytes: 540,
            input_bytes: 1000,
        };
        assert!((m.ratio() - 54.0).abs() < 0.001);
    }
}
