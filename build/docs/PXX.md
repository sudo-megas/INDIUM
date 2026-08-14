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
