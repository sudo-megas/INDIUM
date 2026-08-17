# PXX — the round that ends the beta

*In progress. This document accumulates as the round runs; it is not finished until Phase 4.*

Every round before this one shipped a feature or closed a defect, and each is numbered for that
reason. This one ships no feature. It is the step where the program is examined and either earns
the right to stop calling itself a beta or does not, and it stands outside the numbered sequence
deliberately.

The condition it answers has stood in §7 since P18 and has never been met: *"the `1.0` line stays
one until the design work it is named for has been in real hands"* — the gate being **a testing
round against a released build carrying it**. The design work is P12's and P13's, shipped at
`v1.0.0-4`. `v2.1`, published 2026-08-13, is a released build carrying it. P19, P20 and P21 each
recorded the gate as deferred and left it as the maker's call. This is that call.

---

## Phase 1 — the suite, run against what shipped

Nothing in this phase is new code. It is the first complete run the project has ever had.

The build under test is **`v2.1`**, downloaded from its own release page rather than rebuilt:
`indium-2.1.0-1-x86_64.pkg.tar.zst` (10,319,410 B), `indium_2.1.0-1_amd64.deb` (7,282,304 B) and
`indium-2.1.0-1-x86_64.tar.gz` (9,774,204 B), each matching the size the release records. The
tarball's binary was unpacked into `target/release/` — `release.yml`'s own recipe — so the ELF
checks and the §2 toolkit gate read the binary that actually ships instead of skipping.

| What | Command | Result |
| --- | --- | --- |
| The default suite | `cargo test` | **334 passed, 0 failed**, 10 ignored |
| The packages, read by INDIUM itself | `INDIUM_PKG=… INDIUM_DEB=… cargo test --test package_path -- --ignored` | **8 passed** |
| The estimator, on the 3450U | `cargo test --release --lib the_eight_candidates -- --ignored --nocapture` | **1 passed** |
| The Wayland clipboard | `cargo test --lib -- --ignored offer_reaches_the_wayland_clipboard` | **1 passed** |
| Both packages | `verify.sh`, both env vars set | **40 passed, 0 failed, 0 skipped** |
| The §2 gate | `check-deps.sh` | toolkit-free · PIE, BIND_NOW |
| Formatting | `cargo fmt --check` | clean |
| Lints | `cargo clippy --all-targets -- -D warnings` | clean |

**344 of 344.** Every test the repository contained has now been run and passed at once, which had
never happened before. *(The table above is Phase 1's run and is left as it was taken. Phase 2e's
fixes bring their own regression tests with them, so the count is a running one and every figure
written here names the commit it was taken at: **350 passed, 10 ignored — 360** when the fixes
first landed, **355 passed, 0 failed, 10 ignored — 365 in total** at `ab96461`. Phase 4 takes it
once more and that is the number the freeze carries.)* The ten ignored tests divide three ways, and no single environment had
ever satisfied all three: **eight** (`tests/package_path.rs`) want release artefacts that do not
exist until a tag is pushed; **one** (`platform/clipboard.rs:174`) wants a live Wayland session
and a `wl-paste` no CI runner has; **one** (`estimate.rs:977`) wants a `--release` build and quiet
CPU time, and is a measurement of a particular machine rather than an assertion CI would learn
anything from. CI can reach the eight and can never reach the Wayland one. A person's machine can
reach the Wayland one and the estimator, and had never been pointed at the eight. This is the
first time all three were true at once.

`verify.sh` reporting **zero skipped** is the part worth naming. A skip there is not a pass — it
is a check that found nothing to look at — and the two ELF checks skip by default precisely when
nobody has put a shipped binary where they look.

*(Phase 2e/5 ran both again after the fixes and got the same two lines back — **40 passed, 0
failed, 0 skipped**, toolkit-free, PIE, BIND_NOW. It then pointed them a second time at the binary
the fixes produced, which had never been ELF-checked at all and is the one the re-walk runs: PIE,
BIND_NOW, no RUNPATH, no RPATH, no TEXTREL, non-executable stack, stripped, and no GTK, Qt, KF6,
X11 or portal. It was built from the tree at `71c7357` — twelve commits past v2.1, five of them
touching source — and none of them dragged in a forbidden dependency or lost a hardening default.
Two things were checked rather than assumed while doing it. `release.yml:342` says in a
comment that the tarball holds the same binary the `.deb` wraps — it does, byte for byte, and the
`.pkg`'s is a different build, which is the whole of why v2.1 shipped two distinct binaries.
And the swap has a trap in it worth writing down for anyone who repeats the recipe: both binary
checks read `target/release/indium`, `check-deps.sh` hardcodes that path with no argument, and
`~/.local/bin/indium` symlinks to it ahead of `/usr/bin` in `$PATH`. Unpacking a shipped binary
there and leaving it is how a walk ends up running the old build under the new build's name —
the same defect this round caught the first walk committing. It is put back afterwards, and the
restore is proved by hash rather than assumed.)*

*(Phase 2e/6 then reinstalled v2.1 and ran R10's re-walk. The point of R10 is that the first walk's
round 10 ran a local rebuild, and the argument that it was the same source — true, and provable
from `git diff` — is still an argument about an artefact rather than a measurement of one. This
time it is a measurement: `pacman -Q` reads `indium 2.1.0-1`, `indium` resolves to `/usr/bin/indium`
with the `~/.local` symlink parked, and that file's sha256 is `9d52300ada45993d…` — **byte-identical
to the binary inside the `.pkg`**. It is also the odd one out of the three v2.1 binaries, since the
tarball and the `.deb` share `f9bcea2d…` and the `.pkg` does not, so the re-walk certifies the one
build no other check in this round reaches. All thirteen steps of round 10 were run against it;
**twelve answered as written** and one carries a question, recorded below. 45 undecorated paths,
45 NULs and no newlines under `-0`, `--long -0` refused
with rc=2, `Extracted 3 entries.` and `Extracted 45 entries.` on the round trip with `beach day.jpg`
and `--weird-name` surviving the pipe, exactly 153 bytes from `cat` with `cmp` silent, and — through
a real pty, because a pipe cannot test a prompt that reads from the terminal by design — one
password prompt, no echo, and no trace of the typed word in the pty stream. `--password` is an
unknown option and `INDIUM_PASSWORD` is never consulted. `indium ./list` opened a window while
`indium list` entered the terminal half, which is the distinction §4 promises stated as a control
rather than asserted. `indium a.zip b.zip` forked one process per archive. The verdicts are the
maker's; what is recorded here is what the program did.)*

*(The question is 10.2's, and it is written down here rather than resolved because resolving it is
the maker's. The step asks for eight columns and a total; all eight are present and the total reads
`45 entries, 42 files, 3 directories, 6.1 KiB`. But the **packed column prints `-` for every member
of a zip**, while for `secret.7z` it prints a figure. Python's `zipfile` says the archive does carry
it — `compress_size=29` against `file_size=27` — so the data exists and INDIUM does not show it.
Two readings, and neither is asserted here. **Honest**: libarchive exposes no
`archive_entry_compressed_size()`, so `-` means *the reader did not tell me* rather than *zero*,
which is consistent with the total line printing a ratio for the 7z and none for the zip, and with
the GUI's Details panel showing the same shape. **Under-reported**: the figure is in the central
directory, `--help` promises "packed" without qualification, and a person reading `--long` on a zip
learns nothing about compression. It is **not one of the ledger's 23 and not a regression** — v2.1
shipped this way and so did every release before it. If 10.2 is approved it goes to Phase 3's agent
5 as a CLI observation; if it is denied it becomes work in this round under R3, like any other
denial.)*

### The re-walk came back, and four of its five denials were the instrument's

**139 approved, 14 denied, 5 unanswered.** The five unanswered are the appended steps — 3.14, 8.12,
10.14, 12.11, 14.7 — which cannot be answered until v2.2 exists, so they are correctly blank rather
than skipped. 10.2 was approved, which resolves the paragraph above: the packed column goes to
Phase 3's agent 5 as an observation and not as work in this round.

Of the 23 denials the first walk left, **nine flipped**. Fourteen rows were eligible, so five stayed
red, and the count is worth stating that way because it is arrived at by subtraction: the other nine
denials belong to the after-fix batch and were never walked here. Checking each of the five against
the corpus and the source — rather than against either side's word, which is the rule the pre-walk
corrections established — **four are this round's own instrument and one is the program's.**

**10.5** was denied with the note *"picked deny but didnt understand what this does"*, and the output
quoted back beside it was **correct**: `large.tar` holds exactly three members, so `Extracted 3
entries.` is the whole archive and not a truncation of it. A step that produces a right answer and a
denial is a step nobody could read. It was one sentence carrying three commands; it is now three
commands with their own expected output and a line saying what the step is for.

**10.8 and 10.9** name one member each and both were walked against a different archive — `large.tar`
in place of `photos.zip`, and a path in place of `f.txt`. Both members were verified present before a
word was changed: `README.txt` is in `photos.zip` and `f.txt` is `secret.7z`'s only member. The
likely cause is recoverable rather than mysterious, and it indicts the instrument twice: **`photos.zip`
did not exist during the first walk** — this round rebuilt it — so substituting `large.tar` was
reasonable then and habit the second time. What INDIUM printed both times (`no such entry`, and the
encrypted-header error for a wrong password on v2.1) is a correct program answering the question it
was actually asked.

**11.9** came back, for the second round running, as *"you check for me"*. The step never said which
directory the archives were in. That is the entire defect; it names `~/indium-test/realfile/` now and
gives one command that reads all nine. Run there: **eight of eight valid**, and the ninth,
`archiveencry.7z`, refuses with *"The archive header is encrypted"* — step 3.9's behaviour seen from
outside, which is a pass.

**8.11 is the real one**, and the maker's ruling on it is worth recording in his own words because it
is also the justification for treating it as a defect rather than a feature: *"i always thought
'preselect' was a button for persistent location. So we treat it as a bugfix than rather new
feature. Do not change its name, Preselect stands. Convert it into a clickable button. It must ask
user a persistent path."* The document's own account is that the UI misled the person who wrote the
rule it was obeying — a word set beside two pressable words, in the same row and at the same size, is
a third pressable word whatever it was meant to be. **That is what keeps "PXX ships no feature"
honest**: the affordance was already promised on screen and the round is delivering what was
promised, not adding something new. It landed in `8986ac6` with five tests, and CORE §4's popup 5 —
which had said *destination* while the setting stored only a mode — is true rather than merely old.

### Three ledger rows that were wrong about themselves

Checking 8.11's neighbours turned up something the freeze should not swallow: **the ledger's file
attributions are unreliable, while its fixes are real.** All three were found by looking for the
behaviour rather than for the named file.

**4.13 was a false denial and the ledger filed it as code.** It reads *"Code — `ui/keys.rs`"*, and
that file is only the help-screen key table; nothing in it handles a chord. The guard the step is
actually about — `ctrl_c && self.has_archive() && !typing && !selecting_text` — is at `ui/mod.rs:2967`
**in the shipped v2.1**, with a comment naming the four fields it protects. So no fix was needed and
none was made. The walker approved the step this round while describing the opposite in the note, and
asked directly whether he had tested the wrong thing: he had. The path he copied
(`/run/user/1000/indium/co-6203-22/…`) is what `Ctrl+C` puts on the clipboard **with the table
focused**, which is step 4.1 and correct. The approval stands; the reasoning under it did not.

**3.2 is fixed, in `ui/mod.rs::ascend`, not in `ui/table.rs` where the ledger put it** — the doc
comment there opens *"PXX 3.2:"* and the restore is `position(|r| r.path == left)`, absent from v2.1.
**6.2 is fixed too**, at `ui/mod.rs:2777`, with the sentence *"N folders left out: INDIUM adds files,
not folders."* and two tests. Both were briefly suspected of having been dropped, on the strength of
a search against the filenames the ledger named. **The lesson is the search, not the ledger**: on a
repository about to be frozen, "is this closed?" has to be asked of the program.

**14.4's denial note describes a crash that did not happen in this walk.** The journal's last two
abnormal INDIUM ends are 23:19 and 23:27 on the previous day — the `9/KILL` and the `6/ABRT` already
analysed in finding 2, both the maker's own hand. Nothing crashed during the re-walk. The note is a
stored answer from the first walk, which is what a sheet that preserves answers across a regeneration
is supposed to do, and reading it as new evidence would have sent a hunt after a defect with no event
behind it.

### What the estimator measured, and one thing it found

Eight candidates over this repository's own `src/`, **936.1 KiB weighed whole** (under the 2 MiB
budget, so not sampled):

| Method | Level | Time | Ratio |
| --- | --- | --- | --- |
| Store | 0 | 18 ms | 103.0% |
| lz4 | 1 | 11 ms | 45.8% |
| gzip | 6 | 47 ms | 28.7% |
| zstd | 3 | 8 ms | 28.3% |
| bzip2 | 9 | 86 ms | 20.1% |
| xz | 6 | 496 ms | 22.5% |
| LZMA2 | 6 | 423 ms | 28.7% |
| Deflate | 6 | 36 ms | 31.4% |

**1125 ms in total**, of which xz and LZMA2 are 81.7%.

`estimate.rs:58-62` records the previous measurement and reasons from it:

> Measured rather than guessed … **1365 ms for 813 KiB**, of which xz (571) and LZMA2 (566) are
> 83%. That scales to roughly **3.5 s** at this budget, and doubling the budget doubles that wait
> for a figure nobody reads twice.

Both halves have drifted. The input is no longer 813 KiB — `src/` has grown to 936.1 KiB — and
the time is no longer 1365 ms but 1125. Carried through the same arithmetic the docstring
performs, 2 MiB now costs **roughly 2.5 s, not 3.5**. The paragraph is honest about being a
record of one measurement, so it is not false; it is stale, and it is stale in the one place that
matters, because the figure exists to justify `BUDGET`'s size and a reader deciding whether to
double it would be reasoning from a 40% overestimate.

This is the shape P22 already met once, when `LZMA2:19` was found copied into two docstrings and
into §4.1 itself. A figure written down by hand beside the code that produces it drifts silently;
the fix P22 chose was to make the test read the document rather than let a person copy from it.
Recorded here for the audit to rule on rather than closed now.

---

## Phase 2 — the instrument, the corpus, and the sheet

### The instrument

`build/docs/TESTPLAN.md` — **153 steps across 14 rounds** as Phase 2 wrote it, and **158** after
Phase 2e appended five without renumbering any of the first 153. P11 and P12 ran eight rounds and
forty-one steps against `v1.0.0-2` and `v1.0.0-3` on the night of 9–10 August and every step
passed, and that list was never checked in. It survives only as quoted fragments inside `P11.md`
and in `~/indium-test/OBSERVATIONS.md`, which is how a program comes to be certified by an
instrument nobody can produce afterwards. Twenty-six steps here carry that round's mark: fourteen
of the originals survive as verbatim quotations in the maker's own record, and the rest are
recovered from its findings and from the six coordinates `P11.md` names them at. There are more
marked rows than recovered originals because several originals are split — a step that bundled
four expectations into one line hid which of the four had failed.

### Three steps that were wrong before anyone walked them

Worth recording, because each was found by checking the plan against the program rather than by
running it, and a plan that tells the walker to expect the wrong thing produces a false denial.

- **3.9** said the password popup for `secret.7z` appears *at the moment of use, not at open*.
  It is the other way round. That archive has **encrypted headers** — `bsdtar -tf` refuses it too,
  with *"The archive header is encrypted"* — so the listing is itself the moment of use and the
  prompt comes at open. Not a quirk of the fixture either: `sevenz.rs:294` calls
  `set_encrypt_header(recipe.encrypt)`, so **every archive INDIUM encrypts behaves this way**.
  A walker following the old wording would have denied a correct program.
- **12.6** said to *"watch for `ENOSPC`"*, which is an observation and not something anyone can
  approve or deny. Running out of room on a 712 MB tmpfs is the expected outcome; what is being
  tested is how INDIUM ends. The step now says: approve if it reports the failure in a sentence,
  leaves nothing half-written and keeps the window usable; deny if it hangs, dies silently,
  claims success, or leaves a partial file it does not mention.
- **12.8** asked for `VmPeak` without giving the command to read it. It now carries one, and says
  why `VmPeak` rather than `VmHWM`: under 99 GiB of swap, resident high-water caps near physical
  RAM and measures the box instead of the program.

A fourth gap was structural rather than factual. `bigsecret.7z` was to be *"sized by agent 2"* —
but agent 2 belongs to Phase 3, and the maker opens the archive during Phase 2's walk. Resolved
by making the build a step of its own: the generator writes an 8 GiB input file, **12.8 builds the
archive through Create → Encrypted**, and 12.9 reads it back. There is no `7z` binary and no
`py7zr` on this machine, so INDIUM writing it is not setup for the test — it *is* the test, on the
write path.

### The corpus

`build/make-testdata.sh`, checked in; the fixtures themselves are not, and are gigabytes. Sizes
are chosen against a boundary rather than for bulk:

| Fixture | Size | What it is there to find |
| --- | --- | --- |
| `under-limit.tar` | 943,718,400 B of content | Under `scratch.rs`'s hardcoded 1 GiB `RAM_LIMIT`, so a copy-out routes to `$XDG_RUNTIME_DIR` — a **712 MB tmpfs**, smaller than the payload |
| `over-limit.tar` | 1,610,612,736 B | Over the same limit, routing to cache instead: the other side of a branch nothing has exercised at scale |
| `big-mixed.tar.zst` | 3.0 GB, 209 members | Half incompressible by design, so zstd cannot shrink it and Measure has real bytes to weigh |
| `many-entries.tar` | 150,000 entries | The virtualized table, the filter, Select-all |
| `deep.tar` | 20 KB | 60 levels, a 250-character component, names with spaces, tabs, a newline, Turkish and emoji — **and four traversal members** |
| `bigsecret-input.bin` | 8 GiB, sparse | Larger than this machine's 7.0 GiB of RAM, so the member provably cannot be held in memory — the claim `arch.rs:1038` makes with its `usize::MAX` read. Costs no disk at all |

Two small fixtures joined the table in Phase 2e, and the reason is a finding rather than an
addition. `photos.zip` and `docs.tar.gz` came from the P11 round; this script recorded them as
already present in the corpus and left them alone, and by the end of the walk they were gone —
`photos.zip` extracted into `~/indium-test/photos/` and the archive deleted. Nine steps name it,
seven of them inside the round R10 re-walks in full, so the corpus this document calls
*regenerable* could not in fact be regenerated. Both are built by the script now. **`photos.zip`
is a reconstruction, not the original**, and it is built with explicit member names rather than
`-C dir .`: that idiom stores a `./` root, and v2.1 — the build round 10 is walked against —
refuses any archive carrying one. A convenient rebuild could not have been opened by the very
program it exists to certify.

The filler is an **AES-CTR keystream under a fixed pass phrase**, not `/dev/urandom`, so two runs
on two machines produce identical bytes and a figure measured here can be compared with one
measured elsewhere. That matters for a defect whose report has to say what happens on a machine
with less swap than this one. Total real cost: **5.7 GB**, leaving 71 GB free on `/home`. R9's
overflow partition was offered and is not needed at these sizes.

`deep.tar`'s four traversal members — two `../`, one via a middle component, one absolute — are
there because `path_escapes` (`arch.rs:940-946`) has never been fed a hostile path by anything but
its own unit test. **New step 3.13** extracts them, names its throwaway target twice, and denies on
any file landing outside it. It is the only step in the plan that could write outside its target.

All four aim at paths this user can really write, and the absolute one at `$HOME` rather than at
`/`. That is the same trap the `nosuid` rule above avoids from the other direction: a member
aimed at `/absolute-escape.txt` is stopped by `EACCES` on any non-root run whether or not
`path_escapes` refuses it, so the step would pass without ever testing the check. A security
fixture that cannot fail for the right reason has proved nothing.

### The sheet

`build/make-checksheet.py` reads `TESTPLAN.md` and emits `build/docs/checksheet.html` — one row
per step, a two-button verdict, a note, live tallies per round and overall, filters for unanswered
and denied, and a block at the foot that builds the text to paste back with denials first. State
is in `localStorage`, so a closed tab loses nothing.

**It is generated, not transcribed**, and that is the whole point: this round already found
`estimate.rs:58` stale for exactly the reason P22 found `LZMA2:19` stale, and a 153-row checklist
copied by hand is that hazard with more rows. Closing a denial is one edit to the plan and one
command. Output is byte-identical across runs, so a rebuilt sheet diffs cleanly against the one
before it.

Published as a private artifact for the walk:
<https://claude.ai/code/artifact/52fad8f7-72f5-4318-9e55-9224c3995de3>

**The gate (R3).** Every denial becomes work in this round — fixed, re-verified and re-ticked.
Phase 3 does not begin until the sheet is clean.

Two rules for closing one, both learned from what generating the sheet costs rather than from
running it. **Step numbers are the storage keys**, so a denial is closed by editing what a row
says and never by renumbering — inserting `3.14` is safe, promoting one to `3.10` silently
reattaches every later answer to the wrong step. If a renumber ever becomes unavoidable the
storage key `indium-pxx-checksheet-v1` is bumped with it and the walk restarts. And **the sheet is
regenerated and republished with every plan edit**, because tracking a generated file next to its
source creates exactly the drift this round was built to find; Phase 4 verifies
`checksheet.html` is byte-identical to a fresh regeneration, in the spirit of the nine
doc-as-tests.

---

## Phase 2e — the question the maker raised before Phase 3

Mid-round and unprompted, the maker said the window's text *"feels off"* and that *"the text
readability all low"* — and was candid that he could not name the factor: not size, face,
antialiasing, weight or colour, *"İ SIMPLY DONT KNOW"*. He offered a guess, that the Ubuntu
Aubergine ground is too dark, and asked whether a lighter tint of the same hue would fix it.

It is the only complaint in the round that is about the program as a whole rather than about a
step, so it has no row on the sheet and could not be closed by one. Three findings, each measured
rather than argued, and the third is the answer to his guess.

### 1. Binning was off, and the reason written down for turning it off is false

`theme.rs` carried `v.text_options.subpixel_binning = false`, set by P21, justified in a comment
that read: *"§6 puts this entire window in monospace on a fixed advance: INDIUM cannot collect
that benefit and was paying the blur for it at every size."*

**The advance is fixed in em, not in pixels**, and the font file settles it rather than the
argument. Read from `FiraMonoNerdFontMono-Regular.otf`'s own tables — `head.unitsPerEm` 1000,
`hmtx.advanceWidth[0]` 600 — the advance is exactly 0.600 em. At the three sizes this window sets:

| size | advance | rounded glyph-origin gaps over six glyphs | |
| --- | --- | --- | --- |
| 12.0 px | 7.200 px | 7, 7, 8, 7, 7, 7 | uneven |
| 13.0 px | 7.800 px | 8, 8, 7, 8, 8, 8 | uneven |
| 18.0 px | 10.800 px | 11, 11, 10, 11, 11, 11 | uneven |

None of 7.2, 7.8 or 10.8 is an integer, so with binning off every glyph origin rounds and the ink
gap swings a full pixel roughly every fifth character — at 13 px, on every line in the window. The
even kerning the comment says INDIUM could not collect is precisely what it gave up. This is the
only one of the three findings that is *systemic*, which is the shape the complaint had.

### The A/B P21 promised and never ran

`P21.md:551` set the terms itself: *"The verdict is the maker's, by eye, at 100% / 125% / 150% — a
test cannot tell anyone that text got sharper. If the A/B refuses it, the line comes out, CORE §6
is not touched."* `P21.md:673` — *"The binning A/B has been looked at, and kept or refused"* — was
never ticked. The line shipped on an argument nobody had checked against the font it described.

Run here at last: two builds differing in that line alone, the same archive at the same scroll, no
keypress in either so neither view could drift. The windows landed pixel-identically — a title-bar
strip compared with `magick compare -metric AE` returned `0 (0)` — so every difference below is
the line and nothing else.

- `indium-binning-off` — `6b1a3f92…`, what v2.2 would have shipped
- `indium-binning-on` — `905da733…`

The uneven spacing is visible magnified. **The finding that needs no eye at all is the wrap.** The
same string, in the same panel, at the same width:

| | the Details panel's `Contents` value |
| --- | --- |
| binning **off** | `23.4 MiB (24 576 000` ⏎ `bytes)` — **two lines** |
| binning **on** | `23.4 MiB (24 576 000 bytes)` — one |

Rounding every origin up widens a string enough to wrap it. So the line was not only costing
sharpness, which is a matter of eyes; it was costing **layout**, silently, in the narrowest panel
in the window — and a panel that wraps a value nobody asked it to wrap reads as cramped, which is
one of the things *"feels off"* describes.

**The verdict was the maker's and he gave it away**: *"i dont even comprehend the difference with
human eye. i trust your verdict."* Given the arithmetic and the wrap, the line came out. Deleting
it rather than writing `= true` is deliberate — egui's default is already `true`
(`epaint-0.36.1/src/text/mod.rs:62`), so `P21.md:551`'s *"the line comes out"* is literally what
happened, and the test now pins the toolkit's default instead of pinning an override of it.
`the_window_does_not_pay_for_kerning_it_cannot_collect` is replaced by
`a_fixed_em_advance_is_not_a_fixed_pixel_advance`, whose failure message carries the true
arithmetic. CORE §6 is untouched, exactly as P21 said it would be.

**`P21.md:673` is left unticked on purpose.** No round in this project's history has edited an
earlier round's document — every commit touching `P21.md` is titled `P21:`, and the same holds for
P20 and P19. The closure is recorded here instead, where it belongs, and the empty checkbox stands
as evidence of what PXX inherited. `P21.md:545` still names the deleted test for the same reason.

### 2. One ink failed AA, and it was the filename

Measured off the screen rather than modelled: with `docs.tar.gz` open and the cursor moved with
`Down` so the status bar reads `1 selected`, the wash sampled from a glyph-free part of the row is
`#712322` — one unit down in two channels from the `#712422` the code computes.

| ink on the selection wash | ratio | AA 4.5:1 |
| --- | --- | --- |
| `ORANGE` — what the Name column painted when focused | **2.91:1** | fails |
| `TEXT_SECONDARY` — every other column on a selected row | 5.64:1 | passes |
| `TEXT_MUTED` — what those columns would have been | 3.72:1 | fails |
| `TEXT` — what the Name column paints now | 9.14:1 | passes |

The row above it is the point: `table.rs` steps every *other* column up from `TEXT_MUTED` to
`TEXT_SECONDARY` **for the express purpose of clearing AA on this exact ground**, and says so in a
comment quoting these same 3.72 and 5.64. The one column carrying the filename was the one going
the other way. And because moving the cursor also sets the selection, focused-and-selected is the
ordinary state after any arrow key, not an edge case.

CORE §6 forbade it twice over before contrast entered into it. The accent is *"reserved for
exactly three meanings — the current selection, staged changes, and Apply/progress"*
(`CORE.md:366`), of which the keyboard's position is none; and the Cursor row says that position
is *"a line, not a colour"* (`CORE.md:370`). The ring has done that job since P12 — `table.rs`
simply went on painting `ORANGE` in the Name column for three rounds afterwards, which is why the
passage above the ring stood in the past tense while the code did not. **This is a fix that brings
the code back to CORE, not a design change**, and the maker approved it as such.

**One number was wrong in two places.** Both `table.rs` and `CORE.md:370` quoted **2.06:1** for
that pair. 2.06 is what a *linear* blend gives — but `linear_multiply` composites in sRGB byte
space, landing on `#712422`, the ground the same comment names one line later and derives its own
two ratios from. So the file disagreed with itself, and CORE carried the disagreement. Corrected
to **2.91:1** in both. Nothing either passage concludes changes; it failed AA at 2.91 as surely as
at 2.06. Worth the edit only because a frozen repository should not carry a figure its own
neighbouring line refutes — and because this is exactly the class of defect Phase 3's agent 10
exists to hunt, found here by accident first.

### 3. It is not the aubergine, and lightening it would have made every number worse

His guess, tested rather than waved off. Ordinary text on the window ground:

| ink on `WINDOW #300A24` | ratio |
| --- | --- |
| `TEXT #EEEEEC` | **15.13:1** |
| `TEXT_SECONDARY #BDBDBB` | 9.34:1 |
| `TEXT_MUTED #999997` | 6.16:1 |
| `ORANGE #E95420` | 4.82:1 |

15.13:1 is more than three times the AA floor. Every ink in the window clears AA against the
ground, and the ground is dark **because** that is what buys the headroom — a lighter tint of the
same hue lowers every one of these numbers. He was right that something was wrong, and wrong about
the cause in the one direction that would have cost the most to act on. The palette is not
touched, and no CORE §6 amendment is needed.

### The deviation this creates, stated rather than absorbed

**Every approval on the sheet was given against binning-off rendering**, because v2.1 is what the
maker walked and v2.1 shipped P21's line. v2.2 places every glyph in the window differently. No
step asserts sharpness, and `TESTPLAN.md` does not contain the word *orange* at all, so no stored
answer is falsified by either change — but **round 13 is a layout check at three scale factors**
(*"Nothing clips, nothing overlaps, every control reachable"* at 100% / 125% / 150%), and string
width is precisely what moved. **13.1 through 13.4 therefore join the v2.2 re-walk**, which also
happens to be the check `P21.md:551` wanted from the beginning and never got.

### Three corrections to this document's own record

An independent pass over the twenty ledger rows, checking each against the tree rather than
against the ledger's own file names, resolved all twenty — ten closed in code, three false
denials, five instrument-only, two cascades, none outstanding — and found two more rows that were
wrong about themselves, in addition to the three already recorded above.

- **3.1 is filed as `Code — ui/table.rs, virtualized row range`, and no code changed for it
  anywhere.** `git log v2.1..HEAD -- src/ui/table.rs` shows two commits and both belong to 6.3 by
  their own messages. It closed the way `71c7357` states: *"3.1 and 5.2 are closed the same way
  12.6 was: the documented behaviour becomes part of the approve condition."* This is a worse kind
  of mis-attribution than a wrong filename — a wrong file with the right disposition still tells
  the next reader that code changed; a wrong disposition tells them the wrong kind of thing
  happened at all.
- **1.12 and 1.13 are filed as pure `Instrument`, and both needed code.** `install-desktop.sh`
  invoked its payload by a path relative to the *caller's* working directory, so the corrected
  wording — which sends the walker to `$HOME` deliberately — would still have failed against the
  unfixed script. `5d903bb` makes both scripts resolve their own directory via `$0`. This is the
  same miss as 8.11, in the same direction.
- **The work plan's owning-file list is independently wrong for three rows.** It names
  `ui/newarchive.rs` for 9.5 and 12.8 and `cli.rs` for 10.9; neither file was touched this round.
  9.5 landed in `util.rs` (new `writable_parent`) called from `arch.rs` and `sevenz.rs`, 12.8's
  orphan-temp half in `ui/mod.rs::on_exit`, and 10.9 in `sevenz.rs` alone.

Which is the same lesson three times, and it is why the ledger is not the thing being frozen:
**ask the program, not the record of it.** That applies to the checking pass too — its four commit
citations were spot-checked against `git show` before being written down here.

### The roster — 25 rows, and where it had been scattered

Everything above adds rows to the maker's next walk, and until now it added them **in three
different documents**: the plan carried the 17-row after-fix batch, the section above added round
13, and `3fecc0c` corrected a step that no list mentioned at all. A roster split across three
places is not a roster, and the walk is the one part of this round that is not mine to run — so
it is written once, here, and nowhere else.

It is derived mechanically rather than remembered. The sheet the maker walked is
`git show 71c7357:build/docs/TESTPLAN.md`; comparing every step row in it against HEAD with the
generator's own `STEP_RE` gives **158 steps both sides, none added, none removed, and eight whose
text changed**: 1.8, 8.8, 8.11, 10.5, 10.8, 10.9, 11.9, 12.7. That check is worth more than the
recollection it replaced, which had 3.8 on the list — 3.8 *was* rewritten, in `71c7357`, which is
**before** the re-walk, so its answer already stands against the words it now carries.

**The count, by subtraction.** 158 steps, **139 approved** — so **19 are not green**, and the
sheet shows him every one of them without being told. What the sheet *cannot* show him is a row
that is green and should not be, and there are **six**:

| Row | Green against | What moved under it |
| --- | --- | --- |
| 1.8 | v2.1 | It named *"a `.deb` from `/var/cache/pacman/pkg/`"*. Arch's cache holds 133 `.pkg.tar.zst` and **not one `.deb`** — the step could not be performed as written, and was approved anyway. It now names the `.deb` this repository builds |
| 8.8 | v2.1 | The step is unchanged in what it requires and changed in how it is reached: *Preselect* was the row's label when it was written, and 8.11 made it a **third control**. The two mode buttons are clicked directly now |
| 13.1–13.4 | v2.1 | Sub-pixel binning is on in v2.2, so **every glyph in the window sits somewhere else** and string widths moved with them. Round 13 is the layout check at 100% / 125% / 150% — clipping and overlap are exactly what a width change disturbs |

**19 + 6 = 25.** The six are the whole reason this section exists: a regeneration preserves stored
answers by design, which is what keeps 130 approvals from having to be re-earned every time a
step is reworded — and the price of that design is that a stale green is silent. Naming them is
the only thing that makes them visible.

**All 25 run against the v2.2 build, in one pass.** The five instrument-only rows — 1.8, 10.5,
10.8, 10.9, 11.9 — could in principle be re-ticked against v2.1 without waiting, and they are not,
for a reason found by looking rather than assumed: **v2.1 is no longer installed.** `pacman -Q
indium` returns nothing and `~/.local/bin/indium` points at `target/release/indium` again. That is
not damage; it is step 14.6 doing its job. 14.6 is `pacman -R indium` and the batch was ordered to
put it last precisely so it would. Re-installing a superseded package to re-tick five rows whose
program behaviour v2.2 does not change — none of the round's fixes touch `cat`'s member lookup —
would buy a certification that Phase 4's install of the published v2.2 packages provides anyway.

**One order constraint survives, unchanged and for the same reason.** 14.6 and 14.7 are the
uninstall pair, so they run **last**; every row above them needs an installed binary.

---

## Phase 3 — the hardening, and the fleet that runs it

### This section exists because the fleet did not

Four sentences above route work to Phase 3 by name. `:103` sends a step there if it is approved;
`:112` files an observation with **agent 5**; `:254` sizes a corpus archive with **agent 2** and
then withdraws the assignment because agent 2 does not exist yet; `:441` says the 2.06→2.91
correction is *"exactly the class of defect Phase 3's agent 10 exists to hunt."* Every one of those
is a promissory note drawn on a roster.

**There was no roster.** It was designed once, in conversation, and written to no file — not here,
not in `CORE.md`, not anywhere in `build/docs/`. Recovering it cost a line-by-line scan of a 116 MB
session transcript, and a sweep of all twenty-six documents in this directory confirms the negative
independently: **five passing mentions in this file, across those four routing decisions, naming
three numbered agents — 5, 2 and 10 — and no document anywhere that defines any of them.** The only
other occurrence of the word in the corpus is `P12.md:50`, describing the refute-the-finding
practice rather than this fleet. *(The sweep reported eight; counting them gives five. Recorded
because writing an inherited number into the section that names inherited numbers as the class this
project has never beaten would have been the round's first and stupidest defect.)*

So it is written down here, in full, **before the first agent launches** — the order P23 used when
it opened its document before the first line of code. It is written **once**: this is the only place
in the repository the fleet appears, and no later document restates it. A roster in two places is
the same failure as a roster in none, one milestone later.

The line counts below are current as of this section, not as of the design. That distinction is
load-bearing and gets its own paragraph under the fleet.

### The deviation, stated rather than absorbed

`:320` above reads, in this document's own words: *"Phase 3 does not begin until the sheet is
clean."* **The sheet has not been walked.** Not the 25-row roster against v2.2, and not the wider
re-walk v2.3 requires — v2.3 moved the typeface, the corner radius and the type scale, so it places
every glyph in the window somewhere new and round 13 comes back into scope a second time.

The maker's instruction was *"move on"*, given twice. **His gate, and his to waive.** Recorded here
rather than stepped over, because a precondition that is quietly skipped reads afterwards as a
precondition that was met.

What the waiver costs, stated precisely rather than waved at: the audit runs against a build whose
manual verification is outstanding, so a walk finding can land on a file an agent has already
reported on. That is handled the same way a late route is handled — **the finding enters through the
same thirteen-field contract and the same tiers, and every file the walk touches is flagged for
re-check by its owning agent** rather than being merged on the assumption that the audit already
covered it. What the waiver does *not* cost is coverage: the walk checks what the window looks like
and the fleet checks what the code does, and those two sets barely intersect.

### What this round hunts — twelve classes, from this project's own history

A sweep read all twenty-six documents in `build/docs/` and `CORE.md`, and re-checked every test name
they cite against HEAD. What came back is not a generic audit checklist. It is **this repository's
measured recurrence record**, and it is the difference between telling eleven agents to look for
bugs and telling them where this project has actually failed.

| Class | Shape | What guards it today |
| --- | --- | --- |
| 1. **Stale prose** (~20) | A sentence true when written, falsified by a later change that never touched it. `P6.md:302` names it *"the failure this project names as unforgivable."* | **Eleven** doc-as-tests read `CORE.md`: ten by `include_str!` in `src/` — compile-time, so a vanished CORE fails the *build* — plus `tests/package_path.rs:779` by `fs::read_to_string`, which fails only that test. But **all eleven anchor on structural lists**: tables, counts, sets. **None reads §6's prose.** Code moving under a stationary sentence has no gate at all |
| 2. **A number nothing can check** (~13) | The one class this project has named and declared unbeaten. `P17.md:123`: *"Every number in this project a test can reach has been right for sixteen milestones. This is the class that has not."* | `the_date_about_prints_is_the_one_the_changelog_stamped`, `the_copyright_header_names_the_font_that_ships`, `every_drawn_size_comes_from_the_type_scale` — every one added *after* its escape |
| 3. **A premise that did not survive contact** (~13) | Correct on paper, wrong the first time it ran: the RAR gate that never fires, `flock` living on the inode so the guard held once and failed silently after, `sh` without `pipefail` shipping a structurally-valid empty `.deb` | Found by measuring or by a test, never by review. `tests/read_path.rs`, `tests/write_path.rs` |
| 4. **A test weaker than its name** (~11) | A gate that cannot fail. P16: a deliberately broken `>> 5` **passed all six** covering tests. `P6.md:811`: the synthetic package mirrored `package()` exactly, passed everything, and *could not have caught* the real fault | The sabotage practice itself — every new gate is deliberately broken before it is trusted. There is no meta-test |
| 5. **CORE describes behaviour the code lacks** (~9) | About had no date and §4 said it did; the mark named from P1 and never drawn; §5's verdict table naming a method its own write list forbids | The eleven doc-as-tests. This is the class they were built for, and where they demonstrably work |
| 6. **Silent failure** (~8) | `let _ = save_recents(…)` at three sites; `ARCHIVE_WARN` counted as success; `Ok(written)` meaning both *finished* and *cancelled*; non-ASCII names dropped because a Rust program never calls `setlocale` | `tests/cancel_path.rs` (three tests) and six locale tests including `a_filename_is_the_characters_it_holds` |
| 7. **Modelled vs measured** (~8) | The seeded case: **2.06 modelled, 2.91 actual**, carried in `table.rs` *and* `CORE.md:370` for three rounds. P23 re-ran the class and found four more | `composite()` now models the renderer instead of the maths. The counter-rule is this document's: **ask the program, not the record of it** |
| 8. **Attribution contradicting the shipped asset** (~6) | The `.deb` naming JetBrains for a Fira face across three releases; `OFL-1.1.txt` CRLF in all 93 lines so `sed 's/^$/./'` matched nothing — *"it parsed either way, which is why nobody saw it"* | `the_copyright_header_names_the_font_that_ships`. **Directly in v2.3's path, since the face moved again** |
| 9. **The same defect one door over** (~7) | Not recurrence over time — a *sibling site* missed in the same sweep. `P15.md:75`: the sweep that fixed nineteen comments was followed one milestone later, **by the same hand**, by the twentieth. *"A sweep is not a habit."* | The design answer, not a test: `no_two_grounds_are_the_same_colour` is **pairwise over all six**, not a check of the two that happened to break |
| 10. **A constant fitted, pinned by nothing** (~7) | `SB_ROW` fitted in P7, re-fitted in P13, still unpinned when P23 found it — three rounds | Now pinned in *both* directions (`line_box <= SB_ROW` and `SB_ROW - line_box <= 6.0`), so it cannot drift upward either |
| 11. **One meaning painted two ways** (~6) | Orange and Aubergine both meaning *chosen*, in one popup, three rows apart. A `⇄` neither embedded face carries | `orange_has_not_spread_into_the_widget_states`, asserted in both directions, and six tofu tests |
| 12. **The record correcting its own record** (~7) | P-documents found wrong about themselves, corrected **by addendum only** — P12 wrote addenda into both P6 and P7 | None, and there cannot be one. The instrument is the round: `P16.md`'s *"a milestone cannot audit itself from the inside"* |

**Where the effort goes.** Six classes escaped detection in more than one round, and they are ranked
by how many separate rounds failed to catch them: **2** (three separate three-release escapes, and
DEP-5 separators for ten) → **1** (escaped continuously, and re-entered *inside* the round that was
fixing it) → **4** (six rounds, six escapes) → **9** (a second escape by construction — that is what
the class is) → **7** (three rounds carrying one wrong figure in two files) → **10** (`SB_ROW`,
three rounds).

Two observations shape the briefs more than the ranking does.

- **Reach is the blind spot, not depth.** Every long-lived escape sat somewhere the suite
  structurally could not go: a `.deb` control file, a README badge, a comment, a screenshot, an
  unmeasured pixel. 385 named tests is a strong suite with a boundary, and **the boundary is where
  to look**.
- **Fixing a defect creates the opportunity for a quieter one.** `P6.md:763` records it as a shape
  rather than an incident, and Deviation 14 in the same document describes guards that *"created two
  new reachable hazards rather than closing one."* This is the empirical basis for tier 3 sending
  the *fix* back through the pipeline. The practice was derived from this history, not invented for
  this round.

### What an auditor may not change — eight rules already recorded

None of these is new policy. Each is quoted from the corpus, and six of the eight are things an
audit agent breaches by default unless told not to.

1. **CORE.md is the maker's** (`CORE.md:3-5`, `:630`): *"Items enter and leave only by his hand."*
2. **When reality contradicts CORE, stop** (`P2.md:24-27`, inherited verbatim by P3 through P7). A
   deviation goes in the ledger; *"the deviation log is part of the deliverable."*
3. **An ordered CORE edit is written out in full** and committed alone, in the form
   `CORE: §4 gains the password popup (ordered by P2)` — *"a rule being changed deserves its
   successor written down rather than described."*
4. **P-documents are append-only** (`P5.md:271`): *"A P-document is a record of what was believed at
   the time."* Corrections go in addenda or inline `(ticked in P<n>: …)` annotations. **This is why
   agent 10 excludes `build/docs/P*.md`** — and why a test name a P-document still carries after the
   test was renamed is convention, not drift. `:404` above records that policy explicitly, and at
   least four such names exist.
5. **A box is closed by the thing it asks for** (`P3.md:189`) — never by something adjacent that
   passed.
6. **A raised gate is never softened to make it pass** (`P6.md:86-88`). The `verify.sh` glibc-floor
   FAIL on this machine — 2.43 local against a 2.36 bookworm target — is **the proof the gate works,
   not a broken step.** A finding proposing to relax it is auto-filed `no-action` on receipt, citing
   that passage; it is not "rejected at tier 0", which is a mechanical quote check and by its own
   definition renders no judgement on a claim.
7. **Some decisions are the maker's by category** — mechanism questions under CORE §3, status-bar
   layout under §4. Record them; do not settle them.
8. **Verdicts a test cannot render belong to the maker's eye** (`P21.md:550`): *"a test cannot tell
   anyone that text got sharper."* The 100% / 125% / 150% check is his, and no agent may claim it.

### The fleet — eleven agents, thirty-four files, each owned exactly once

Two ownership axes, deliberately separate. **Files owned**: exactly one agent each, no file
unclaimed and none claimed twice. **Concern owned**: cross-cutting, and the concern owner may *read*
another agent's files but files the finding under **that agent's** ID with its own number in
`cross_ref`. The contract routes by file, and the concern co-signs — never the reverse.

| # | Model | Scope and concern | Files owned — lines (production) | What it must return |
| --- | --- | --- | --- | --- |
| 1 | Fable 5 | **The threading contract.** Sole owner of the `CORE:114` *"UI thread and one worker"* question | `ui/mod.rs` 4638, `estimate.rs` 1298 — **5,936** (4,759) | A thread-lifecycle census of all eight spawn sites: what each spawns, what joins it, what happens if it panics. A verdict on `work_running()` naming which of the eight it fails to gate. A concrete interleaving proving or disproving the estimator-preemption overlap. One of three verdicts on `CORE:114` — code can match, doc must reword, or both. **Ranked fix options with blast radius, not a diff.** Also owns `sb_progress_geometry`, the other half of the `R_ZONE` change, co-signed `cross_ref 6` |
| 2 | Opus 5 (A) | **Extraction safety and all FFI/crypto.** Sole build and stress lane | `arch.rs` 1906, `sevenz.rs` 582, `secret.rs` 144 — **2,632** (2,344); plus `tests/read_path.rs` | An exploitability verdict on each of: symlink and hardlink **target** strings, which nothing inspects anywhere; the absent `AE_IFCHR/IFBLK/IFIFO/IFSOCK` constants; the three `usize::MAX` sites; `EXTRACT_PERM` with setuid; missing `SECURE_NOABSOLUTEPATHS`; no size cap. Reachable in the shipped UI or CLI path, or blocked upstream by `path_escapes` and libarchive's own flags? A working proof archive per confirmed hole, extending the `evil.zip` pattern — **fixtures are not committed**. Audit all **32** `unsafe {}` blocks in his own files — 31 in `arch.rs`, 1 in `secret.rs` — for *correctness*, verifying **pairing**, not the presence of a comment. `Secret`'s un-wiped `CString` at the FFI boundary and its `Clone` copy-count: bound it, or state that it cannot be bounded |
| 3 | Opus 5 (B) | **Filesystem state machine and crash consistency** | `tasks.rs` 2795, `platform/scratch.rs` 594 — **3,389** (2,100); plus `tests/write_path.rs`, `tests/cancel_path.rs` | The rename-commit in `apply`: the exact TOCTOU window, what survives a SIGKILL at each instant, whether the temp orphan is ever user-visible. The canonicalize-then-fallback lock name: construct the collision — two paths, one fallback name — or prove it impossible. `sweep_stale` reads `cache_root` only; **judge the documented rationale at `scratch.rs:78`** rather than rediscovering it as an oversight, since the code already argues `runtime_root` is tmpfs and the leak is bounded by reboot |
| 4 | Sonnet 5 | **Platform layer and untrusted-input parsing** | `apps.rs` 813, `store.rs` 763, `clipboard.rs` 359, `window.rs` 246, `picker.rs` 139, `open.rs` 62, `platform/mod.rs` 60 — **2,442** (1,435) | `apps.rs` parses attacker-influenceable `.desktop` files: field-code (`%f %u %F`) and quoting handling against the two `Command::new` argv sites. `store.rs` TOML round-trip on hostile and truncated input, and the hard rule that **no password reaches either file**. `window.rs`'s unbounded reaper-thread-per-child-window — measure the bound. `clipboard.rs`'s serving thread is spawned **inside `wl-clipboard-rs`**, not by INDIUM, which bounds what may be proposed. Co-signs `cross_ref 1` on both thread items. Also flags its two no-test-module files, `platform/mod.rs` and `open.rs` |
| 5 | Sonnet 5 | **CLI, pure helpers, and the ABI risk** | `cli.rs` 863, `util.rs` 768 — **1,631** (1,304); plus `tests/cli_path.rs` | Verify each hand-transcribed Termios `const_assert` against the real glibc x86_64 headers on disk, **quoting the header line per assert**. State what happens on musl, on aarch64, on non-glibc: compile error, which is safe, or silent wrong layout, which is not. `cli.rs` is **863 lines of production code with no in-file tests** and 5 `unsafe` blocks — propose the minimum test set, noting that its header claims external coverage by `tests/cli_path.rs`. `util.rs`: the CRC32 table and path normalisation against reference vectors |
| 6 | Sonnet 5 | **UI mid-tier: rendering and interaction** | `table.rs` 1190, `inspector.rs` 850, `newarchive.rs` 723, `sidebar.rs` 559, `measure.rs` 525, `extract.rs` 367 — **4,214** (3,322) | A panic-reachability sweep over all indexing, slicing and arithmetic. No `unwrap` exists, but `v[i]`, `a - b` on unsigned and `usize` casts still panic. **Brief rewritten below.** Every user-facing string that makes a promise is routed to agent 10 |
| 7 | Sonnet 5 | **Supply chain, CI and packaging.** All non-`src/` surfaces | `Cargo.toml`, `Cargo.lock`, `build.rs`, `ci.yml`, `release.yml`, `check-deps.sh`, `install-*.sh`, `make-deb.sh`, `PKGBUILD`, `verify.sh`; plus `tests/package_path.rs` | **Actually run** `cargo audit` and `cargo deny check` and paste the raw output; resolve `memoffset` to an advisory ID and a verdict. A focused risk note on `sevenz-rust2`, which owns AES-256 and encrypted-header reads: last release, maintainer count, open advisories. The exact placement and fallout of `#![forbid(unsafe_code)]` — it **cannot** go crate-wide against 37 blocks, so propose per-module `deny`. A CI diff costed in runner-minutes against `ci.yml`'s stated no-cache policy |
| 8 | Haiku 4.5 | **Small-file sweep** | `model.rs` 352, `settings.rs` 303, `password.rs` 270, `about.rs` 268, `openwith.rs` 204, `keys.rs` 181, `main.rs` 170, `pending.rs` 140, `lib.rs` 115, `tray.rs` 95, `filter.rs` 74 — **2,172** (1,672) | A mechanical per-file pass: panic-reachable expressions, integer over- and underflow, dead code, unhandled `Result` and `let _ =`. Flag the **six no-test-module files** in this set — `password`, `openwith`, `main`, `pending`, `tray`, `filter`. `password.rs` carries one extra question: does the plain `String` in the text field get cleared on *every* exit path, window close included? |
| 9 | Sonnet 5 | **`theme.rs` — the round's numbers audit** | `theme.rs` 3501 — **3,501** (1,374) | **Brief rewritten below.** |
| 10 | Haiku 4.5 | **Doc-versus-code drift** | `CORE.md`, `README.md`, `LICENSES/`, `assets/org.indium.desktop`. **Excluded: `build/docs/P*.md`** — historical record, immutable by rule 4 | Extract **every mechanically checkable claim** from CORE §1–§9 and the README as a numbered list with its source line. Verify each one that is a grep-able fact — dependency lists, §9 toolkit bans, format tables — and **route each judgement-requiring claim to its owning agent by ID**, returning the routed table even where unresolved. `CORE:114` is assigned to agent 1 and recorded here as routed, not verified. Additionally: **every `///`-cited identifier in `src/` must resolve to a real `fn`.** One dangling reference is already seeded below |
| 11 | Haiku 4.5 | **Verification clerk and merge integrity.** No files owned | — | Runs after 1–10. For every finding: re-open the cited file, confirm the verbatim `quote` exists at the cited range, confirm the line numbers have not drifted. Rejects any finding whose quote does not match — **no judgement on the claim**, only on whether the cited text is real. Then run the coverage checksum and produce the deduplicated merged register |

**The checksum, taken with `wc` against the tree as it stands, not remembered.**
5,936 + 2,632 + 3,389 + 2,442 + 1,631 + 4,214 + 2,172 + 3,501 = **25,917 lines across 34 files**.
By production lines, which is the truer denominator because `theme.rs` is 61% test:
4,759 + 2,344 + 2,100 + 1,435 + 1,304 + 3,322 + 1,672 + 1,374 = **18,310** — which is **2,289 per
file-owning agent**, not the 1,665 it comes to spread across all eleven. Agents 7, 10 and 11 own no
`src/` lines by design, so the two figures differ by a third, and only the first describes anyone's
actual reading load. `tests/` — 4,299 lines across five files — is assigned rather than left loose:
read to 2, write and cancel to 3, cli to 5, package to 7.

**The thread census confirms the ownership boundary rather than merely respecting it.** There are
**nine** `thread::spawn` sites in `src/`: **eight in `ui/mod.rs`**, which is agent 1's file and the
whole of its `CORE:114` brief, and **one in `platform/window.rs`**, which is agent 4's and is exactly
the unbounded reaper thread agent 4 is told to measure. The 8/1 split falls on the ownership line
without being arranged to, which is why agent 4 co-signs `cross_ref 1` rather than owning the
concern: one agent holds the contract, the other holds the outlier.

**The untested set decomposes exactly, which is how it was checked.** Nine files carry no test
module: agent 8's six (953 lines), agent 4's two (122), and agent 5's `cli.rs` (863). 953 + 122 +
863 = **1,938**, with no remainder and no file counted twice. The design-era figure was eleven files
and 3,503 lines, and an inventory pass during this reconstruction returned 1,868 — **wrong by 70,
and wrong in exactly the way class 2 describes.** The number was computed three times and was right
once.

**Every line number in the original briefs is stale, and that is a standing instruction rather than
a footnote.** The tree has moved **+3,436 lines** since the fleet was designed. Briefs keep their
verbatim quotes; agents re-locate every anchor by grep before working. Filing a finding against a
remembered line number is itself the class this round exists to hunt, and tier 0 rejects it.

### The two briefs the drift rewrote

**Agent 9 — `theme.rs` inverted.** The design called it *"the lowest-criticality file in the tree —
no security surface, no I/O, no threads"* and gave it a mechanical sweep. That was true at 1,683
lines. It is now **3,501**, the second-largest file in `src/`, and it carries **every measured figure
of the v2.3 redesign**. Its 2,127-line test module is a repo-wide lint that reads `CORE.md` and scans
`src/`, so whoever owns the file implicitly owns a cross-cutting invariant. The brief:

- **Audit the numbers, not the lines.** Every number-bearing comment — contrast ratios, ΔE figures,
  em advances, pt and px measurements, hue degrees — checked against the test that pins it, or
  flagged as pinned by nothing. Roughly ninety such comments exist.
- **The pt/px unit ambiguity.** `18.2pt` means egui logical units (`BODY * ICON_SCALE`); `9.75pt`
  means typographic points derived from the same 13px; one test writes the same quantity as
  `18.2px`. The arithmetic is internally consistent and the unit label is not. Verdict wanted:
  correct the labels, or state the convention once and stop.
- **Confirm what has already been checked and do not re-derive it**: `13 × 1.4 = 18.2`,
  `1200/2048 = 0.586`, `R(1 − 1/√2) = 1.757` at R=6, `24 − 21.125 = 2.875`. All re-measured in round.
- The model moves up a tier by the fleet's own rule — this is a *present* thing checked against a
  spec, but the spec is a sampled measurement rather than a fixed checklist.
- **The nine-agent degradation path recorded in the design is now invalid.** Folding agent 9 into
  agent 8 cannot work: 3,501 lines do not fold into a 2,172-line mechanical sweep. If degradation is
  ever needed, fold **8 into 6** and leave 9 standing.

**Agent 6 — its central ask was already satisfied.** The design named `inspector.rs` (717 lines) and
`table.rs` (862) as *"the two biggest untested files"* and asked for the minimum test set for each.
**Both now have test modules.** The brief:

- **Verify the new test modules cover the rounding work**, rather than proposing tests that exist.
  `table.rs` has three tests, `inspector.rs` two. Are those the minimum set, or the easy set?
- **The `R_ZONE` 0→6 change and its two leaning sites.** The clipping problem was solved by proving
  it does not arise: a rounded rect of radius R admits a point `(c,c)` in from its corner once
  `c ≥ R(1 − 1/√2)`, which is 1.76px at R=6, and the table's content inset of `4 + 2 = 6` clears
  that threefold. So there are **no corner covers, no inset first row, and no rounded clipping** —
  epaint 0.36 offers none. What actually had to change was the **cursor ring**, which
  `expand2(0.5 * item_spacing)` pushes *outward* past the inset: solving
  `(R − dx)² + (R − dy)² = R²` gives an upper root of **8.46**, so R=6 leaves one pixel. It had been
  passing `theme::R_ZONE` as its own radius — harmless while that constant was `0`, wrong the
  instant it became `6`. Audit the second leaning site, **the status-bar proportion bar in
  `ui/mod.rs`**, and file it under **`owner_agent: 1` with `cross_ref: 6`**, since the contract
  routes by file owner and that file is agent 1's.
- **Route every promise-making user-facing string to agent 10**, unchanged from the original brief.

### The output contract — thirteen fields, all mandatory

A finding missing any field is rejected by agent 11 unread.

`id` in the form `PXX-<agent#>-<nnn>` · `owner_agent` · `cross_ref`, the concern owner's number where
the finding falls in another agent's concern and empty otherwise · `file` and `line_range` ·
**`quote`**, verbatim source, copy-pasted and unedited — the anti-hallucination anchor, and the
reason a clerking tier can be mechanical · `claim`, one sentence · `category`, one of `security`,
`correctness`, `concurrency`, `doc-drift`, `supply-chain`, `test-gap`, `resource` · `severity`, one
of `freeze-blocking`, `fix-in-v2.5`, `document-only`, `no-action` · `evidence_type`, one of
`static-argument`, `test-run`, `tool-output`, `poc-artifact` · `repro`, an exact command with
expected against actual, or the static argument in five steps or fewer — **not** *"seems likely"* ·
`proposed_fix`, a minimal diff sketch **or the literal string `none — document only`**, never a
speculative refactor · `blast_radius`, required even when the fix is `none` · `confidence`, one of
`certain`, `probable`, `unverified-hypothesis` — the last is **allowed and encouraged**, because it
routes to tier 2 instead of being dropped · `verified_by`, filled by agent 11 and **never
self-filled**.

### Verification — asymmetric, because the costs are

A false positive causes a permanent unnecessary change. A false negative ships a permanent defect.
Those are not the same cost, so the gate is tiered by what a finding costs **if acted on**.

- **Tier 0 — the quote check, every finding, agent 11.** Re-open the file, confirm the `quote` exists
  verbatim at `line_range`. Rejects fabricated findings and line-drifted ones. Mechanical, and by
  design renders no judgement on the claim itself.
- **Tier 1 — `document-only` and `no-action`.** Tier 0 suffices. These change no code, and the worst
  case is a wrong sentence in a Deviations section: cheap, and honest.
- **Tier 2 — `fix-in-v2.5`.** Requires independent confirmation by a **non-originating** agent of
  equal or higher tier, by one of three routes: a test that fails before and passes after; a
  reproduction on the maker's machine; or a concurring static argument written **without reading the
  original finding's reasoning** — only its `file` and `line_range`. Blind re-derivation is the whole
  point. **A confirmer who reads the argument confirms the argument, not the bug.**
- **Tier 3 — `freeze-blocking`.** Everything in tier 2, **plus** the `proposed_fix` diff goes back
  through the full pipeline as a new finding with its own `blast_radius`, reviewed by an agent that
  did not write it. **The fix is the riskiest artifact in this round, not the finding** — a correct
  diagnosis with a wrong patch is the failure a frozen repository cannot survive, and class 3's
  history is the evidence.

**The escape valve, stated explicitly because its absence has a predictable failure mode.** Any agent
may file a real, confirmed defect with `severity: document-only` and `proposed_fix: none`. For a
repository about to freeze, *"this is a known limitation, recorded in Deviations"* is frequently the
**correct** engineering answer. Six of the twelve classes above were closed by adding a gate rather
than by changing behaviour, and class 12 has no gate and can have none. Without this said out loud,
every agent optimises for producing a patch — and patches are what break v2.5.

### Standing instructions, in all eleven briefs

1. **`assume clean, do not re-verify`**, verbatim. Without it several agents spend their budget
   re-grepping for `unwrap`, which is the single most reflexive thing an audit agent does. The list:
   no `unwrap` in `src/`; the 37 `unsafe` blocks all carry SAFETY comments, and agent 2 checks their
   *pairing*, not their presence; attribution across the history is clean; the suite is green at 374.
2. **Every line number in these briefs is stale.** Re-locate every anchor by grep. See above.
3. **Known-deferred, recorded, do not file.** `README.md:243` still names Fira and moves at v2.5 with
   the badge and the install filenames; `SECTION_ABOVE = 14.0` is deliberately untouched, being
   spacing rather than type; the glibc-floor `verify.sh` FAIL is structural; the `.deb` says
   `OFL-1.1` where the PKGBUILD says `OFL-1.1-RFN` and **the two are correct to differ** — do not
   reconcile them. And a test name that a P-document still carries after a rename is convention, not
   drift, by rule 4. A `///` citation *in `src/`* resolving to no `fn` is the opposite.
4. **CORE.md is the maker's to edit.** Any CORE change is drafted for approval, never applied.
5. **No AI attribution on commits, tag annotations or release bodies** — CORE §8 and §9.
6. **The twelve classes and the eight rules are pasted into every brief**, not summarised. The classes
   say where this repository has actually failed; the rules say what must not be touched on finding
   it. Both are quotations, so an agent that disagrees with one is disagreeing with a recorded
   decision and files a finding rather than acting on it.
7. **Prefer `document-only` where the class says so.** See the escape valve.

### The order they run in

Three concurrent at most, so four waves, with two hard constraints and one that turned out to matter
more than it looked.

| Wave | Agents | Why |
| --- | --- | --- |
| 1 | **10**, 7, 4 | **Agent 10 runs first** — its routed-claims table feeds the file owners, and run late it routes work to agents that have already finished. Its two wave-mates are chosen as the ones that *cannot* receive routes: agent 7 owns no `src/` and runs tools, and agent 4's files are the least CORE-described in the tree |
| 2 | **1**, 2, 3 | The three heaviest reasoning scopes. **Agent 2 is the sole build and stress lane** |
| 3 | 6, 9, **8** | Agent 8 moved here out of wave 1, because it holds **two of the eleven CORE-mirroring test sites** — `keys.rs:133` and `settings.rs:248`, both verified at those exact lines — which makes it the file owner most likely to receive agent 10's routes, and it must not run beside it |
| 4 | 5, then **11** | **Agent 11 is strictly last.** It clerks everyone |

**The residue is handled rather than assumed away.** Agents 7 and 4 still finish alongside 10, so a
route can land on an agent that has already reported. The rule: **a late route is not dropped and not
re-launched — it becomes a re-check item appended to agent 11's queue**, which runs last and can
re-open the file for a tier-0 quote check itself. Where the route needs judgement rather than a quote
check, agent 11 files it as `confidence: unverified-hypothesis` naming the owner, and it goes to tier
2 like anything else. Silently losing a route is the same defect as silently losing a finding.

The design's thrash warning was written for nine to eleven concurrent agents and is largely moot at a
cap of three — but **the one-build-lane rule survives it**, because `target/` locking does not care
about the cap. Stress-reproduction requests from any agent queue to agent 2, which computes the OOM
threshold before running anything: `arch.rs` buffers a whole member into the heap, so a single ~2 GiB
7z member can take the machine down.

### Seed findings, in hand before the first agent launched

Found while reconstructing the fleet. They enter the register as ordinary findings and clear the same
tiers — but they are evidence that the round has something to find, and two of them are defects in
the plan that designed it.

| Finding | Class | Status |
| --- | --- | --- |
| `theme.rs:153` cites `the_cast_lands_in_the_band_core_gives_it`; the real test is `…_core_six_gives_it` at `:1872`. The only dangling `///`-cited identifier in the file, in code written this round | doc-drift, class 1 — agent 10 with agent 9 | **Confirmed.** The cited name exists at exactly one site and resolves to no `fn` |
| `table.rs:235-237` justifies naming the family with *"sat 18% larger than the three mono columns"* — a two-face-era measurement. `MONO` and `SANS` now resolve to the same face, and `FontDefinitions::empty()` leaves no default behind them, so the stated condition can no longer arise | class 1, stale rationale | **Hedged.** The *action* is still defended on other grounds; it is the measured justification that died |
| The v2.5 plan's §2f called `luminance`, `contrast` and `composite` production helpers at `theme.rs:972-1021`. They are **inside `mod tests`**, at `:1382-1459`, and **nothing in the shipping binary does colour maths at all** | premise error, class 3 | **Confirmed.** Lines 1–1374 contain no call to any of them |
| The same plan's §2b argued the no-ligature guarantee comes from egui rather than the face, on the premise that *"epaint resolves glyphs through a `HashMap` … no HarfBuzz or rustybuzz anywhere"*. **`harfrust` 0.12.0 is a direct dependency of epaint** and shapes with `calt` on by default. The guarantee comes from the **Mono cut**, which is why keeping it is load-bearing | premise error, class 3 | **Confirmed** at `Cargo.lock:1258`; retracted in round and pinned by `a_filename_is_the_characters_it_holds` |
| The untested-file total is **9 files / 1,938 lines** — not the design-era 11 / 3,503, and not the 1,868 an inventory pass returned | class 2, arithmetic | **Confirmed** by direct sum, and it decomposes 6 + 2 + 1 with no remainder |
| **385 uniquely-named `#[test]` fns exist; `cargo test` accounts for 384** — 374 passed and 10 ignored. One named test appears never to run, and no `#[cfg]` gate on a `#[test]` was found to explain it | class 4 — a gate that cannot fire | **Unverified hypothesis**, routed to agent 11 with the coverage checksum. **Rule out the measurement artifact first**: `package_path.rs` writes `#[ignore]` inside doc-comment prose, which already fooled one grep during this reconstruction, so the count may be counting a name that exists only in a comment |
| `P7.md:719-727` specifies **four** tests for the cancellation fix; `tests/cancel_path.rs` holds **three**. The two absent *names* are convention under rule 4 — but *pre-cancellation* as a behaviour may have gone with them | class 4, test-gap — agent 3 | **Probable.** The count of three is confirmed; whether the fourth behaviour is covered elsewhere is agent 3's to settle. P7's own brief for it is still the right one: *"the test that is the bug"* |

The fifth row is the point of the whole round. That figure was computed three times and was wrong
twice, in a project whose entire discipline is built around exactly that failure. The two rows after
it are the point of the *contract*: both are admitted at less than certainty, and both would have
been dropped by a fleet that only files what it can already prove.

---

## Phase 3 — what the fleet returned

Eleven agents ran, in the four waves the order above sets out. All eleven reported. **Ninety-six
findings** entered the register; agent 11 re-opened every cited file and checked every quoted line
against the tree.

### The tier-0 pass, and what it did not find

**Zero rejections.** Ninety-six citations, and not one quoted a line that does not exist. That is
the result this section is most obliged to distrust, so it was spot-checked independently at the
merge point rather than accepted: the file count, the line total, the test count, the working tree,
and both of the clerk's two non-PASS verdicts were re-derived by hand. All six confirmed.

| Checked | Claimed | Found |
| --- | --- | --- |
| `.rs` files in `src/`, each owned exactly once | 34 | **34** |
| Total lines across those files | 25,917 | **25,917**, exact |
| `#[test]` attributes against what `cargo test` accounts for | 384 = 384 | **384**. Confirmed |
| `git status --porcelain` after eleven agents | empty | **empty** |

Two citations were not clean, and neither is a rejection. `PXX-5-006` cites four lines for the exit
codes it says are behaviourally pinned; three are exact, and the fourth points at
`tests/cli_path.rs:169` — `let path = fixture(name);` — where the assertion it means is two lines
down at `:171`. Recorded as **drift**, which is a bookkeeping correction, not a false finding.
`PXX-2-002` quotes a string that is real but lives at `password.rs:199` rather than where it was
cited: **paraphrase, not fabrication.** Both were re-read here before being recorded as such.

### The ledger, and it balances

| Severity | Count |
| --- | --- |
| freeze-blocking | **2** |
| fix-in-v2.5 | **21** |
| document-only | **62** |
| no-action | **10** |
| closed seed, never severity-tagged (`PXX-385`) | **1** |
| **Total** | **96** |

Under standing rule 7, `document-only` and `no-action` need only tier 0 — which this pass supplied.
**Seventy-two findings are therefore fully cleared and owe nothing further.** Twenty-one
`fix-in-v2.5` findings still owe **tier 2**: independent blind re-derivation by a non-originating
agent, working from `file` and `line_range` alone and never from the original's reasoning. That work
is not done, and this section does not pretend otherwise. **Phase 3 has completed its audit. It has
not completed its verification.**

The three severities the clerk reported as unassigned — `PXX-5-003`, `PXX-5-007`, `PXX-5-011` — were
**dropped in transcription into the working register, not omitted by the agent that filed them.**
Agent 5's own report assigns all three `document-only` explicitly, `PXX-5-003` adding that the call
belongs to `main.rs`'s owner. They are restored here with that provenance stated, and the clerk's
verdict table is left as it was written. Editing another agent's table to agree with a later
correction is precisely the class-12 failure this document exists to record.

### The two freeze-blocking findings

**`PXX-2-001` — `arch.rs:1054-1084`. An arbitrary write outside the destination, and the only
finding to have completed all three tiers.**

The encrypted-header 7z branch **never calls libarchive.** It writes with `std::fs::create_dir_all`
and `std::fs::write`, and both follow symlinks. The `path_escapes` pre-flight at `:1020-1024` runs
before `headers_need_sevenz` is computed at `:1030`, so no entry reaching this loop is lexically
outside `dest` — the vector is not a hostile path, it is **a link already on the disk**. INDIUM
plants it itself: an ordinary tar carrying a symlink extracts with exit 0 and the message
"Extracted 1 entry.", after which any header-encrypted 7z extracted into that directory writes
straight through it. Reproduced end to end, twice, the second time using the **committed** fixture
`tests/fixtures/secret-headers.7z`: `verify_dest/` kept only the symlink, and `outside/pwned.txt`
received the payload.

This falsifies `CORE.md:102`, which states that extraction runs under libarchive's `SECURE_SYMLINKS`
and `SECURE_NODOTDOT` so a hostile archive cannot write outside its target. On this branch there are
no flags to run under.

**Tier 3 reviewed the fix and returned REPLACE.** Agent 2's `O_NOFOLLOW` patch closes two variants
of three. **`O_NOFOLLOW` does not refuse a hardlink** — a hardlink is not a link the kernel resolves,
it is a second name for one inode — and the reviewer verified it against the kernel rather than
arguing it:

```
symlink  + O_CREAT|O_TRUNC|O_NOFOLLOW  ->  refused, errno=40 (ELOOP)
hardlink + O_CREAT|O_TRUNC|O_NOFOLLOW  ->  OPENED AND WROTE
```

INDIUM plants the hardlink as readily as the symlink; the reviewer applied agent 2's patch and wrote
through it anyway. **Agent 2's original patch must not ship.** The replacement descends one path
component at a time with `symlink_metadata` before each `mkdir` (`create_dir_under(root, dir, raw)`),
and replaces `std::fs::write` with **unlink-then-`create_new`** — the unlink removes the *name* and
so severs the hardlink, and `create_new` (`O_CREAT|O_EXCL`) refuses anything that reappears,
including a dangling symlink, which is why no hand-transcribed `O_NOFOLLOW` is needed at all. Four
variants closed, eight legitimate cases intact, suite 286→287 and 34→35 with zero failures.

Two things are recorded rather than closed. An intermediate-component race remains, needing a
concurrent local writer, and it is closable only with `openat2(RESOLVE_BENEATH)`. And **the
replacement is not in the tree.** When it lands it is a new artifact and owes its own tier-3 review
by an agent that did not write it — the rule that produced this verdict applies to the thing the
verdict produced.

**`PXX-10-006` — `CORE.md:87-92`. CORE contradicts itself on the shipped typeface**: §2 names Fira
Mono where §6:392 and `assets/fonts/` carry CaskaydiaMono. It has tier 0 and nothing else, correctly:
it is a wording contradiction, not an exploit, so there is no reproduction to re-derive and no patch
to review. **What it owes is the maker's hand.** A draft replacement is written and not applied.

### The twenty-one that owe tier 2

Recorded here in full, because a list that lives only in a scratch register is a list this project
has already lost once.

| ID | Site | What it is |
| --- | --- | --- |
| `PXX-1-001` | `ui/mod.rs` | A worker's death is unobservable — nothing joins it, nothing reports it |
| `PXX-1-002` | `ui/mod.rs` | `staged_against` refuses on a false positive |
| `PXX-1-003` | `ui/mod.rs` | A rug-pulled listing drains as though it completed |
| `PXX-1-004` | `ui/mod.rs:1038-1040` | `ApplyMsg::Done` reads the live queue, un-gated during a running Apply |
| `PXX-1-005` | `ui/mod.rs` | Synchronous CRC decompress freezes the window |
| `PXX-1-011` | `ui/mod.rs:3427` | Bookmark removal writes the status before the write |
| `PXX-2-002` | `arch.rs` | A correct password refused on encrypted-content, plaintext-header 7z |
| `PXX-3-001` | `tasks.rs:1654-1660` | Apply's sole durability barrier discards both the open failure and the sync failure |
| `PXX-4-001` | `platform/window.rs` | `Child` never reaped — zombie viewers accumulate |
| `PXX-4-002` | `clipboard.rs` | `clipboard::offer` runs synchronous I/O on the UI thread |
| `PXX-4-003`, `PXX-4-004` | `platform/mod.rs`, `open.rs` | Zero test coverage, both extractable offline |
| `PXX-5-008` | `cli.rs:700-863` | The five termios `unsafe` blocks are entirely untested by CI |
| `PXX-6-006`, `PXX-6-007` | `extract.rs:117`, `table.rs:741` | Status written before the write it announces |
| `PXX-7-004` | crate-wide | A nine-line `#[forbid(unsafe_code)]` patch covering 30 of 34 files |
| `PXX-8-003` | `pending.rs:108,129-132` | "Discard all" reachable via the always-live `W` keybind with no guard |
| `PXX-10-001` | `theme.rs:153` | The dangling `///` citation |
| `PXX-10-002`, `PXX-10-003`, `PXX-10-005` | `README.md` | Version and date drift |

**The status-order class was swept to completion rather than sampled.** Eleven production call sites
of `change_settings`/`change_recents`: three correct and each carrying the rule as a comment, three
inverted, five making no claim. **No test anywhere gates the ordering.** `ui/mod.rs:3413-3428` holds
the rule and its violation in adjacent arms of one `match` — the `Recents` arm writes the status
first and carries the comment explaining why, and the `Bookmarks` arm eight lines below does the
opposite. That is class 9 in its purest recorded form: not a defect recurring over time, but a
sibling site missed in the same sweep, in the same `match`.

### The seventy-two that clear at tier 1

The ledger above counts these. Counting them is not recording them.

**For a `document-only` finding the disposition *is* the deliverable.** There is no diff to point at
later, no test that goes green; the whole artifact is the sentence saying what was found and that it
was decided not to act. So a finding filed `document-only` whose text lives only in a working
register is indistinguishable, a week later, from a finding nobody made. That is class 12 — the
record failing to record itself — and this round has no gate against it except writing the thing
down. Sixty-two document-only and ten no-action findings follow, one line each, with the site that
carries them.

Nine of these were reached by an agent refuting its own hypothesis rather than filing it, and they
are here at the same weight as the hits. A round that only records what it confirmed teaches the
next round to stop looking.

**Document-only — sixty-two.**

| ID | Site | What it is |
| --- | --- | --- |
| `PXX-1-006` | `CORE.md:114-115` | Verdict (ii), the document must reword: five reachable concurrencies falsify *"one worker"*; the invariant the code holds is *at most one **task*** |
| `PXX-1-007` | `ui/mod.rs:1857-1861` | Estimator preemption is flag, not join — estimator and task provably overlap for a bounded window; shared state benign by construction |
| `PXX-1-009` | `ui/mod.rs:3440-3442` | `MIN_W`'s doc comment opens by explaining `SB_HEIGHT`'s arithmetic; *"that sum"* has no antecedent in its own text |
| `PXX-1-010` | `ui/mod.rs:1844-1848` | `begin_apply` re-implements `work_running()`'s predicate pair privately, so any widening of "work" must be made twice |
| `PXX-2-003` | `arch.rs:969-975` | `path_escapes` judges the stored *name* only; nothing anywhere inspects `Entry::symlink` or `Entry::hardlink`. The enabler for `PXX-2-001` |
| `PXX-2-004` | `arch.rs:1425-1434` | `verify_passphrase` accepts `ARCHIVE_EOF` as proof the password is right — EOF on the first block means zero bytes were produced |
| `PXX-2-005` | `arch.rs:1365-1387` | The CRC loop folds an `ARCHIVE_WARN` block in indistinguishably and labels the result *computed*; its only exit is EOF |
| `PXX-2-006` | `arch.rs:584-588` | The only comparison in the file treating `ARCHIVE_WARN` as fatal; five siblings pair it with OK. The *warning* text is shown as the error |
| `PXX-2-007` | `arch.rs:1272-1282` | Measured, not modelled: 30,795 bytes → 288.2 MiB peak RSS, 9,810:1. The OOM threshold was computed and deliberately **not** run |
| `PXX-2-008` | `arch.rs:62-65` | No `AE_IFCHR`/`IFBLK`/`IFIFO`/`IFSOCK`, so a special file cannot be named: a FIFO lands inside dest in silence, a chardev aborts mid-loop |
| `PXX-2-010` | `read_path.rs:409-410` | `every_traversal_shape_is_refused_end_to_end_and_writes_nothing` covers four *pathname* shapes and no link shape at all — why `PXX-2-001` was reachable |
| `PXX-2-011` | `secret.rs:47-49` | Live plaintext copies bound at five instantaneously, unbounded cumulatively. §9's *"wiped after"* is true of INDIUM's memory, false of the process's |
| `PXX-3-002` | `tasks.rs:1450-1452` | The comment names an atomic `create_new`/`O_EXCL` check that exists nowhere in `src/` — one repo-wide hit, and it is this comment |
| `PXX-3-003` | `tasks.rs:1370-1375` | The canonicalize fallback is not an edge case but 100% of creations; `/home/x/a.7z` and `/home/x//a.7z` take two different locks, both proceed |
| `PXX-3-004` | `scratch.rs:190-192` | `locks/` under the cache fallback sits outside the sweep by nobody's decision — one zero-byte file per archive ever applied to, permanently |
| `PXX-3-005` | `tasks.rs:1535-1540` | `apply` returns `Ok(0)` for both a cancellation and a legitimate zero-entry rebuild — the class-6 shape `build_and_verify` was given `Option` to avoid |
| `PXX-3-006` | `tasks.rs:2385-2390` | The assertion states a propagation property *in its own failure message* that it never tests; nothing cancels mid-member through a real sink |
| `PXX-3-007` | `cancel_path.rs:208-211` | Seed settled, not a defect: P7's pre-cancellation behaviour travelled into the merged test and survives both sabotages |
| `PXX-3-008` | `tasks.rs:1528-1534` | Answers `PXX-1-001` from the disk side — a worker panicking mid-Apply leaves no unrecoverable state. Agent 1's wedge is a UI defect, not a data one |
| `PXX-3-009` | `tasks.rs:1521-1526` | Between the temp unlink and the writer's open, a planted symlink redirects the build. Needs a second principal writable in the archive's directory |
| `PXX-3-010` | `write_path.rs:1160-1165` | Under root — which CORE §9 permits — the test `return`s as a **pass** having asserted nothing, and the skip is invisible in the tally |
| `PXX-4-005` | `window.rs:105-130` | The ninth spawn site: one thread per argv entry, one call path, all before `run_native`, self-terminating. No accumulation path exists |
| `PXX-4-006` | `store.rs:328-341` | Cleanup guards only the *rename* failing; an `ENOSPC` at write leaves `settings.toml.tmp.<pid>` permanently, and no sweep covers `config_home()` |
| `PXX-4-007` | `ui/mod.rs:504-506` | If both config files break at one startup, `.or_else` drops `recents.notice` from the status line. The `was_broken` latches are still correct |
| `PXX-5-001` | `cli.rs:671-683` | The six const-asserts cannot separate `NCCS` 32–35 — four values, one layout — and cannot check `ECHO` or `TCSAFLUSH` at all. False confidence, not a live hazard |
| `PXX-5-002` | `cli.rs:564` | `must_exist` tests `is_file()` and prints *"does not exist"*. The directory plainly exists; it is the wrong kind of path |
| `PXX-5-003` | `main.rs:116` vs `cli.rs:564` | One program, two entry paths, two different answers to "does this archive path exist". Class 9. Filed `owner_agent: unidentified` rather than guessed; routed to agent 8 |
| `PXX-5-005` | `arch.rs:631` vs `:584` | `next_entry` exempts `ARCHIVE_WARN`; `Reader::open` does not — so `failure()` prints the raw libarchive warning verbatim, against its own doc comment |
| `PXX-5-006` | `cli_path.rs:148`,`:169`,`:443`,`:683` | All three exit codes are behaviourally pinned. What is missing is documentation, not coverage — the finding was narrowed to match |
| `PXX-5-007` | `cli.rs:508` vs `:403` | `cat` exempts a literal `-` from flag rejection; `extract` does not. `cli.rs:497` records this gap biting the file before, *in the opposite direction* |
| `PXX-5-009` | `cli.rs:118-129` | Hypothesis tested and refuted: `run()`'s centralized `out.flush()` catches the deferred `/dev/full` error in all five cases. Reframed as a test-gap |
| `PXX-5-010` | `util.rs:270-284` | The doc comment enumerates three behaviours; the function's **first line** is an undocumented fourth |
| `PXX-5-011` | `ui/mod.rs:843`, `:867` | The identical singular/plural ternary duplicated 24 lines apart, where `cli.rs` factors it into one helper and says why. Routed to agent 1 |
| `PXX-6-001` | `table.rs:356-367`, `:1147-1189` | The cursor-ring test validates the ring's *arithmetic* and never reads the shipped `CornerRadius::ZERO`; reverting line 363 to the P23 bug leaves it green |
| `PXX-6-002` | `inspector.rs:71-72`, `:827-830` | Production computes the pane's chrome dynamically at **36**; the test hardcodes **24** and the doc comment repeats it. Production is the correct half |
| `PXX-6-003` | `table.rs:1050-1051`, `:1078-1082` | Two doc comments claim 238 from an Inspector floor of 260. With the real floor (272) it is **226**; the assertions' literals are unaffected |
| `PXX-6-004` | `table.rs:16`, `:147` | `ROW_HEIGHT` and the header's `22.0` are literal, undocumented and untested — nothing pins that they still clear `BODY` in the shipped face |
| `PXX-6-005` | `table.rs:234-241` | Seed settled — doc-drift, code correct. The *"18% larger"* justification is two-face-era and the condition can no longer arise; the call itself stands |
| `PXX-6-008` | `extract.rs:242-270`, `newarchive.rs:85` | `complete_path` runs a synchronous `read_dir` on the UI thread, every frame, from two popups, with no cache |
| `PXX-6-009` | `table.rs:551`, `:682` | One filesystem stat per visible row per frame in a **non-virtualized** `ScrollArea`. Recents cap at 15; no bookmarks cap was found |
| `PXX-6-010` | `measure.rs:53` | Password and Measure are the only two `egui::Modal` popups, but CORE tags only Password `(modal)`. Recorded, **not settled** — rule 8 |
| `PXX-7-002` | `Cargo.toml:23` | `sevenz-rust2` one patch behind; both changelogs read, and the gap is non-security |
| `PXX-7-003` | `Cargo.lock:2513` | Sibling-crate advisory context for `PXX-1-005` |
| `PXX-7-005` | `ci.yml:63-68` | Adding `cargo audit` as a push gate is a genuine policy tension — an advisory-db update can redden an unchanged tree, the exact red the no-cache policy exists to prevent. Both sides presented, not decided |
| `PXX-7-010` | `PXX.md:855` | The 385-vs-384 seed does not reproduce: 384 anchored attributes, no `#[cfg]`-gated test, no `harness = false` |
| `PXX-7-011` | `make-deb.sh:158-164` | Names a test that does not exist; the real one is `core_and_the_deb_name_the_same_dlopened_libraries`. A second dangling citation, in `build/` rather than `src/` |
| `PXX-8-001` | `model.rs:33-40` | `Row::entry_index()` has zero references repo-wide. Flagged as *possibly deliberate API completeness* rather than recommended for deletion |
| `PXX-8-004` | `password.rs:167-171`, `:263-270` | The header's *"until submit or cancel clears them"* assumes those are the only exits. A compositor-driven close reaches neither, so the fields are not even length-zeroed |
| `PXX-8-005` | `main.rs:42`, `:151-156` | No `catch_unwind`, no `panic::set_hook`, no `Drop for Indium`. One process, N windows — a panic in the update loop takes them all, at exit 101 |
| `PXX-8-006` | `keys.rs:25-49`, `:132-172` | `the_popup_and_core_agree_about_the_keys` cannot fail for the five chords it lists: dispatch `continue`s on `ctrl` before `match key`. All five could break and stay green |
| `PXX-8-007` | `settings.rs:247-302` | The panel test extracts only the string after each `theme::section(ui, "` and never inspects what is drawn *within* a section |
| `PXX-8-008` | `about.rs:81`, `:197-267` | Neither test exercises `show()`; replacing `field(ui, "Date", RELEASE_DATE)` with a literal leaves both green |
| `PXX-9-001` | `theme.rs:17` | **WRONG.** The five-rung ladder measures 1.518–1.953, not *"1.37 and 1.87"* — P18 changed "Six" to "Five" in this sentence and left the range untouched |
| `PXX-9-002` | `theme.rs:207-210` | **WRONG.** `EDGE` measures 1.886–2.031, not 1.88–1.95. Hue, not luminance: `composite()` mixes in gamma bytes, and green carries 0.7152 of WCAG luminance |
| `PXX-9-003` | `theme.rs:220-224` | **WRONG.** Ten pairing interpretations tried, none lands in 2.24–2.45; the one reproducible cleanly gives **1.397** — the very figure the comment disowns |
| `PXX-9-004` | `theme.rs:17-18`, `:164-165` | **WRONG.** The 318°/328° narrative mixes CIELAB and HSV; no single convention makes both halves true, and the real gap is 20–48°, not ~10° |
| `PXX-9-005` | `theme.rs:1456` | **WRONG, and backwards.** True linear blending gives 2.637 — *lower* than the gamma-byte 3.72 the code uses. The code is right; only the justifying sentence is not |
| `PXX-9-006` | `theme.rs:183` | ΔE 27.4 modelled; actual 27.78–27.98, and an alpha sweep found no byte value producing 27.4. The rhetorical point survives either way |
| `PXX-9-007` | `theme.rs:2366-2385` | `only_three_corner_radii_exist` scans seven `Visuals` fields, not the program — a name promising whole-program coverage over a struct-field census |
| `PXX-9-009` | `theme.rs:1102` | *"The two must stay the same number"* is asserted by nothing; it holds by shared identifier. Named as a third category — **self-pinned** — between pinned and unpinned |
| `PXX-9-010` | `theme.rs:3476`, `:3486` | The repo-wide lints are the sabotage-resistant runtime-read kind but key on **lexical** matches; a `use egui::Window;` alias would not match. A family characteristic, not a defect |
| `PXX-10-004` | `README.md:243` | Names Fira; Caskaydia ships. Known-deferred — it moves at v2.5 with the badge and the install filenames |

**No-action — ten.** Filed, verified, and deliberately not acted on. Four of them exist so that a
later hand does not "fix" something that is already correct.

| ID | Site | What it is |
| --- | --- | --- |
| `PXX-1-008` | `ui/mod.rs:3530-3534` | The second `R_ZONE` leaning site **does not lean**: `sb_progress_geometry` reads the constant parametrically, degrades to old behaviour at 0, and has no cursor-ring-style ceiling |
| `PXX-2-009` | `arch.rs:53-56` | **Recorded so it is never "fixed":** `EXTRACT_PERM` drops s-bits by design, and `SECURE_NOABSOLUTEPATHS` *cannot* be added — `set_pathname` writes the absolute join before extract, so the flag would refuse every extraction |
| `PXX-5-004` | `cli.rs:594-609` | Agent 5 rejected its own candidate fix as worse than the status quo — adding `Other` to the retry allowlist would re-prompt on every unrelated I/O error. The correct fix is upstream. Rule 7 and the escape valve, working |
| `PXX-7-001` | `Cargo.lock:1562` | `memoffset` clears both advisories **and** never enters the Linux build: `cargo tree --target x86_64-unknown-linux-gnu -i memoffset` prints nothing |
| `PXX-7-006` | `release.yml:385-429` | The draft gate behaves exactly as the ritual assumes: no draft → `gh release view` fails → `exit 1` |
| `PXX-7-007` | `verify.sh:649-698` | All three font/copyright checks pass against the Caskaydia files — traced by hand against real bytes rather than by running the script |
| `PXX-7-008` | `verify.sh:256-318` | The glibc floor computes 2.43 against target 2.36 and FAILs. That is the proof the gate works. **No relaxation proposed** — rule 6 |
| `PXX-7-009` | `package_path.rs` | A naive grep returns 14 `#[ignore]`; eight are real attributes and six are backtick prose. The repo-wide real total is **10**, matching cargo exactly |
| `PXX-8-002` | `settings.rs:123`,`:163`,`:188`,`:204-205` | The four routed status-order sites, closed on the record as clean — three set no status at all, and the fourth is status-first-save-last with its own correct comment |
| `PXX-8-009` | `tray.rs:89-91` | Corroborates `PXX-1-004` without re-filing it; `PXX-8-003`'s patch covers both sites |

### Three convergences — the same line, reached from two directions

Kept as separate IDs and merged as one fix unit. Neither half was deleted; each was reached blind.

- **`table.rs:363`** — agent 6 found that the cursor-ring test validates the ring's *arithmetic* and
  never reads the shipped `CornerRadius::ZERO` argument, so reverting that line to the original P23
  bug leaves every assertion green. Agent 9, from the other end, found that
  `only_three_corner_radii_exist` guards the radius *vocabulary* by scanning seven `Visuals` fields
  and cannot reach the call site at all. **Two tests whose names promise coverage, one line neither
  touches.**
- **`arch.rs:584`** — agent 2 found it is the only comparison in the file that does not special-case
  `ARCHIVE_WARN`, where `next_entry` at `:631` does. Agent 5 arrived from the caller's side and
  confirmed what it costs: the raw libarchive warning string printed verbatim to the user, breaking
  the "program's own voice, one sentence" contract stated in the doc comment of the very function
  that prints it.
- **The discard-races-Apply cluster** — `PXX-1-004`, `PXX-8-003`, `PXX-8-009`. Agent 8 filed the
  second as *"strictly worse than PXX-1-004"* and the third at `no-action` as *"corroborates
  PXX-1-004 without re-filing"*, which is the contract's cross-reference discipline used correctly
  rather than three agents claiming one defect three times.

Two candidates were examined and **deliberately not merged**: `PXX-4-002` and `PXX-6-008` share a
class — synchronous filesystem I/O on the UI thread — but enter through different doors, and the
three status-order inversions are three distinct sites. Same class is not same defect.

### The seed findings, closed out

The seven rows above at `:848-856` are answered here rather than edited, per rule 4.

The dangling `///` citation at `theme.rs:153` stands, and is `PXX-10-001`. The two premise errors in
the v2.5 plan stand as confirmed. The untested-file arithmetic stands. The stale `table.rs:235-237`
justification stands as hedged.

**The 385 row is closed, twice.** It was first closed at the merge point by diagnosis: the count came
from a `grep -A2` that scraped a nested helper, `fn rs_files` at `theme.rs:2905`, as a 385th test
name. The clerk then closed it again by direct count, deliberately running both a clean method and
the known-flawed one, and got **384 = 384**. There is no dark test. The finding was a defect in the
instrument that measured it — which is the class the round was hunting, found in the round's own
tooling, and it is recorded here at full length rather than quietly dropped because the answer turned
out to be "nothing".

**The `P7.md:719-727` row resolved against the agent that owned it.** The two absent test *names* are
convention under rule 4 and are not drift. Agent 3 corrected the brief that sent it: P7 applies *"the
test that is the bug"* to `ok_zero_alone_cannot_tell_a_cancelled_extraction_from_an_empty_selection`,
not to the pre-cancellation test. The genuinely absent name is
`a_precancelled_extraction_writes_nothing_and_sends_no_progress`.

### The register's own integrity, including this document's

Four items, recorded because a round that audits a repository and not its own bookkeeping has
audited half of what it touched.

1. **Three severities were dropped in transcription into the working register** — `PXX-5-003`,
   `PXX-5-007`, `PXX-5-011`. The agent assigned all three; the register lost them. Restored above,
   with the provenance stated.
2. **A numbering gap at `PXX-9-008`**, unexplained. No finding is lost — agent 9's three
   test-integrity findings are all present under other IDs — but the gap is recorded rather than
   renumbered away.
3. **A cross-reference mismatch on `PXX-7-003`**, whose xref target does not line up with the citing
   agent's own numbering.
4. **The clerk found a last-write-wins bug in its own extraction script and disclosed it unprompted.**
   `PXX-1-005` and `PXX-1-008` each appear twice in the working register — once as the real finding
   and once as a severity-less index row sharing the ID — and a naive linear scan let the empty row
   overwrite the real one. Caught before it reached the deliverable. It is the clerk's role to catch
   silent corruption, and the corruption it caught was its own.

### What is not done

Phase 3's audit is complete and its verification is not. **Twenty-one findings owe tier 2.**
`PXX-2-001`'s replacement fix is designed, measured, and **not in the tree**; when it lands it owes
its own tier-3 review. `PXX-10-006` and every CORE draft below await the maker's hand.

**CORE drafts written and not applied**, in one place so none is lost: §2 (the typeface
contradiction, `PXX-10-006`); §3 (one task, not one worker — agent 1); `CORE.md:102` (the module-table
replacement falsified by `PXX-2-001` — and the tier-3 reviewer flagged an error in its own draft, a
comment reading "CORE §6's sentence" where line 102 is **§3**); `CORE.md:105`'s `tasks` row (agent 3,
two sentences); and agent 5's two drafts, one recording what the exit codes mean and one recording
the constant-transcription ritual and the precise reach of the gate that guards it.

Nothing in this phase was pushed. No fixture was committed. The working tree is clean.
