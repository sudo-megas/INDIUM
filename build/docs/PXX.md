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

---

## Phase 3 — tier 2, the blind re-derivation

Twenty-one findings were filed `fix-in-v2.5`, and tier 2 is what they owe: *"independent
confirmation by a non-originating agent of equal or higher tier … a concurring static argument
written **without reading the original finding's reasoning** — only its `file` and `line_range`.
Blind re-derivation is the point: a confirmer who reads the argument confirms the argument, not the
bug."*

### How blindness was actually enforced, and where it leaked

Each confirmer received the file, the line range, and a one-word category. Nothing else. No claim,
no severity, no mechanism, not even a question. They were told to read the site cold and report what
they independently concluded was or was not wrong there — and told plainly that answering `NO` was a
wanted result, because a false positive here becomes a permanent unnecessary change to a repository
about to freeze.

They were barred from reading this document, barred from anything under the job's scratch directory,
and barred from running anything that writes to `target/` — the one-build-lane rule survives the
agent cap, because `target/` locking does not care how many agents there are. A confirmation needing
a run was to be filed `NEEDS-RUN:` and executed serially. None was needed.

**The blindness leaked once, and the confirmer disclosed it unprompted.** An unscoped
`grep -rn "unsafe_code" .` — run from the repository root rather than against `src/` — matched two
lines of this very document, one of them the `PXX-7-004` row it was assigned to confirm. It stopped,
said so, rescoped every subsequent search, and flagged its own verdict as degraded so the merge
could discount it. That is the disclosure rule working exactly as written: **a disclosed slip is
recoverable and a hidden one poisons the record.** The verdict was set aside anyway and the site
re-run blind by a different agent, because a confirmer that has seen the claim confirms the claim.

The instruction that would have prevented it — *never `grep -r` from the repository root; scope every
search to `src/` and `tests/`* — was **not** in the first briefs. It was added to the re-runs. The
brief was the defect, not the agent.

### Verdicts, and the three ways one can land

A blind pass has three outcomes and only the first is a pass. **CONFIRMED**: the confirmer said
`YES` and described the same mechanism — not the same words, the same failure. **REFUTED**: `NO`,
with a reason, which does not kill the finding but forces a reconcile, because one of the two passes
is then wrong. **DIVERGENT**: `YES`, but about a *different* mechanism at the same lines — the
outcome that must never be flattened into a pass, because it means the site is wrong in a way
neither pass has fully described.

| Finding | Site | Verdict | Confidence |
| --- | --- | --- | --- |
| `PXX-2-002` | `arch.rs:1030-1041` | **DIVERGENT** — see below | probable |
| `PXX-3-001` | `tasks.rs:1654-1660` | CONFIRMED | probable |
| `PXX-4-001` | `apps.rs:513-515` | CONFIRMED | certain |
| `PXX-4-003` | `platform/mod.rs` | CONFIRMED | certain |
| `PXX-4-004` | `platform/open.rs` | CONFIRMED | certain |
| `PXX-5-008` | `cli.rs`, `EchoOff`/`ask_for_password` | CONFIRMED | certain |
| `PXX-6-006` | `extract.rs:115-126` | CONFIRMED | certain |
| `PXX-6-007` | `table.rs:739-743` | CONFIRMED | certain |
| `PXX-8-003` | `pending.rs:108`, `:129-132` | CONFIRMED | certain |
| `PXX-10-001` | `theme.rs:153` | CONFIRMED, **and a second stale citation found** | certain |
| `PXX-10-002` | `README.md:9` | CONFIRMED | certain |
| `PXX-10-003` | `README.md:10` | CONFIRMED, **and sharper than filed** | certain |
| `PXX-10-005` | `README.md:65-76` | CONFIRMED, **and a sibling site found** | certain |

Thirteen settled. `PXX-7-004` was voided by the contamination above and re-run. The seven
`ui/mod.rs` findings — `PXX-1-001`, `-1-002`, `-1-003`, `-1-004`, `-1-005`, `-1-011` and
`PXX-4-002` — were assigned to a confirmer that **died on a session limit before reporting**, and
were re-run rather than assumed. A tier that reports on findings nobody re-derived is worth less
than no tier at all.

### The divergence — `PXX-2-002`, and why it must not be recorded as a pass

The original filed this as *a correct password refused*: the 7z fallback is keyed on
`EncryptedHeaders`, and the condition that actually matters is that libarchive cannot decrypt 7z
content at all, so a `7z a -p` archive lists fine and then refuses every read.

The blind confirmer, given only `arch.rs:1027-1046`, reached the opposite face of the same decision.
It observed that `extract` is the **only one of four read paths** that keys the fallback on *any*
error rather than on `EncryptedHeaders` — verified independently here:

- `arch.rs:1031` — `looks_like_7z(path) && Reader::open(path, passphrase)?.next_entry().is_err()`
- `arch.rs:1204`, `:1273`, `:1345` — all three read
  `Err(ArchiveError::EncryptedHeaders) if looks_like_7z(path)`

Four read paths, and one of them keyed differently from the other three. And the same flag does a
second job: `arch.rs:1041` reads `if !headers_need_sevenz && !verify_passphrase(path, secret)?`, so
the flag that routes extraction away from libarchive **also switches off password verification**.
The confirmer's failure path is therefore a 7z that `sevenz-rust2` can parse but whose first
libarchive header read fails for a reason that is not encryption — truncation, or a codec libarchive
lacks: it lists successfully, sets the flag, skips verification entirely, and reaches a decoder that
`arch.rs:877-880` itself says carries no bzip2, ppmd, deflate or zstd. The first thing to touch the
password is then the per-entry `?`, *after* `create_dir_all` has already put directories into the
destination — contradicting `extract`'s own doc at `:979-982`, which promises that a wrong password
"costs nothing and leaves no partial output behind."

Both passes are looking at `headers_need_sevenz`. One says it is *too narrow*; the other says it is
*too broad and load-bearing twice over*. **Both can be true, and the fact that two independent reads
of the same six lines produced two different defects is itself the finding.** Recorded as
`DIVERGENT`. It owes a third look before any fix is designed, and under this round's own rule the
fix is the riskiest artifact — a patch aimed at one face of this could deepen the other.

### Three findings the blind pass sharpened

- **`PXX-10-003` was filed too gently.** The badge reads `2026-08-12`; the filing compared it to
  v2.3.0's changelog stamp. The confirmer compared it to the version *the badge beside it claims* —
  and `2.1.0-1` is stamped `Thu, 13 Aug 2026`, while `2026-08-12` is `2.0.0-1`'s date
  (`Wed, 12 Aug 2026`). Verified here against the changelog trailers. **The two badges did not agree
  with each other even before either went stale.** One drifted claim, two independent errors.
- **`PXX-10-005` has a sibling one door over.** `README.md:87` carries
  `indium-2.1.0-1-x86_64.tar.gz`, outside the `:65-76` range the finding cited. Class 9 in the round
  that names class 9 — a sweep is not a habit.
- **`PXX-10-001` was one dangling citation; there are two.** See `PXX-T2-001` below.

### What the blind pass found that nobody had filed

Seven. They carry a `PXX-T2-` prefix because they came from the verification tier rather than from a
numbered agent, and saying so is cheaper than pretending the register was complete. Each was
re-verified here against the source before being written down.

| ID | Site | What it is | Severity |
| --- | --- | --- | --- |
| `PXX-T2-001` | `theme.rs:156` | Cites `CORE.md:368` for the sentence *"It is not an accent and never decorates."* Line 368 is the table separator `\| --- \| --- \|`; the sentence is at `CORE.md:374`. A **second** stale citation in the same doc comment as `PXX-10-001`, and of a class agent 10 did not sweep for — a line-number citation into CORE, not a `///`-cited identifier | document-only |
| `PXX-T2-002` | `README.md:87` | `indium-2.1.0-1-x86_64.tar.gz` — the same stale version as `PXX-10-005`, outside the range that finding cited | fix-in-v2.5 |
| `PXX-T2-003` | `extract.rs:109` | The pin stores `app.extract_path.trim()` with **no `expand_tilde`**, while the Extract button 67 lines below expands it at `:176`. A pinned `~/Downloads` is stored literally, and `table.rs:682`'s `Path::new(&b.path).is_dir()` then renders that chip permanently dimmed and "missing" — though clicking it works, because the *other* site expands on the way out. One string, two readings, in one popup | document-only |
| `PXX-T2-004` | `extract.rs:115-126` | When the typed path is already bookmarked, the whole block is skipped and **no status is set at all** — the `+` button silently does nothing, and the user cannot tell success from no-op | document-only |
| `PXX-T2-005` | `arch.rs:573` | `archive_read_add_passphrase(a, c.as_ptr());` discards its return. libarchive rejects an empty passphrase, so an empty password yields a reader with **no passphrase registered** and a downstream error that explains nothing. Every read path in the file routes through here | document-only |
| `PXX-T2-006` | `arch.rs:756-771` | The streaming `list()`'s 7z branch applies no `is_archive_root` filter, where `:811` and `:851` both do — and `:810`'s comment states the promise unconditionally: *"the table never grows a nameless row and Select-all never picks one up."* Latent rather than live, since 7z stores no `./` root, but the guarantee is format-specific where its comment is not | document-only |
| `PXX-T2-007` | `tasks.rs:1654` | The comment's stated rationale — *"so what is verified is what is on the disk"* — is not what `fsync` achieves: the verify at `:1678`/`:1680` reads back through the page cache whether or not the sync ran. The line's real value is crash durability, not verification. A correct line defended by a wrong reason | document-only |

`PXX-T2-002` is the only one of the seven that changes code, and it changes a filename in a README.

### What tier 2 does not settle, even at 21 of 21

1. **`PXX-2-001`'s replacement fix is still not in the tree.** It is designed and measured. When it
   lands it is a **new artifact** owing its own tier 3, reviewed by an agent that did not write it.
   Nothing in this pass touches it.
2. **`PXX-10-006` owes the maker's hand, not a re-derivation.** It is a CORE self-contradiction, and
   tier 0 was the right and only mechanical gate for it.
3. **Rule 8 still holds.** Nothing here touches the 100/125/150% window check. That verdict is the
   maker's eye and no agent may claim it.
4. **The seven `PXX-T2-` findings have had tier 0 only** — each quote re-verified at its cited line
   by this merge point. Six are `document-only` and clear at tier 1. `PXX-T2-002` is
   `fix-in-v2.5` and therefore owes a tier 2 of its own, from someone who did not file it.

Nothing in this tier was pushed. No file in `src/` was modified. The working tree is clean.

---

## Phase 3 — the CORE drafts, written out and not applied

**`CORE.md` is edited by the maker's hand only** (`CORE.md:3-5`, `:630`): *"Items enter and leave
only by his hand."* Everything below is a draft awaiting that hand. **None of it has been applied,
and no commit in this round touches `CORE.md`.**

They are written out here because of rule 3 — *"an ordered CORE edit is written out in full,
committed alone"*, since *"a rule being changed deserves its successor written down rather than
described"* — and because a draft that exists only in a scratch register is a draft that will have
to be reconstructed from a transcript, which this round has already paid for once.

Suggested commit form throughout, per the P7 convention:
`CORE: §<n> <what changed> (ordered by PXX)`. **Each is committed alone.**

### Draft 1 — §2, the typeface. Closes `PXX-10-006` (freeze-blocking)

Replaces `CORE.md:87-92`. The current text names Fira Mono Nerd Font Mono and rests its no-ligature
argument on it, while `CORE.md:392` and `assets/fonts/` both say CaskaydiaMono. **The paragraph
cannot be repaired by swapping the name**: a direct GSUB parse of both shipped faces returns `calt`
and `rlig` present, `liga` absent — so *"carries no ligatures at all"* would be false of the face
that actually ships.

> Bundled assets, not dependencies: CaskaydiaMono Nerd Font Mono, regular and bold, embedded in
> the binary, with the SIL Open Font Licence 1.1 alongside the GPL in `LICENSES/`. A filename
> holding `->` must render as the two characters the archive stores, and the **`Mono` cut** is what
> makes that true — Cascadia's programming ligatures live in the Code cut, not in this one. The
> guarantee is deliberately not stated as *"the face carries no ligatures"*: both shipped faces do
> carry `calt` and `rlig` in GSUB, and egui shapes through harfrust with `calt` on by default, so
> the claim that matters is the narrower one — this cut defines no substitution for the sequences a
> filename can hold, and `a_filename_is_the_characters_it_holds` holds it across twenty-nine of them
> in both weights rather than leaving it asserted in a comment. `Mono` is also the single-cell icon
> cut, so a glyph in a name never widens a table column.

**The adjacent trap: `CORE.md:494` also says "Fira Mono" and must NOT be touched.** It is the road
table's P12 row — a record of what P12 did, and correct as history. `README.md:243` is the third
site and is known-deferred to v2.5.

### Draft 2 — §3, threading. Closes `PXX-1-006`

Replaces `CORE.md:114-115`, which reads *"the UI thread and one worker."* Agent 1 returned verdict
(ii), *the document must be reworded*: five independent reachable concurrencies falsify "one
worker", two of them defended in code comments. The invariant the code genuinely enforces is **at
most one task**, and that one holds everywhere.

> Threading: the UI thread, and at most one **task** at a time. A task is an extraction or a rebuild
> — the thing that carries the progress row, the proportion bar and Cancel — and it runs on a worker
> thread that reports over a channel and honours a cancellation flag; nothing may start a second task
> while one runs. Around the task, short-lived readers come and go on threads of their own: the
> listing streams entries while the table fills, a Preview reads one member's head, the estimator
> measures — and the estimator alone is preempted rather than waited for when a task starts, because
> advisory work does not get to hold up real work. Every blocking wait on another program — the
> portal's picker, the clipboard's owner, the file manager being handed a folder, a child window
> being reaped — happens on a thread of its own, never the UI's.

Four in-code comments quote the old clause (`ui/mod.rs:1638-1641`, `:2107-2108`; `estimate.rs:55-56`,
`:662-663`). All four use it to justify *sequencing the eight estimator candidates*, which the reword
preserves — **none is falsified by this edit**, and all four may stand.

### Draft 3 — §3's `arch` row, and why it is *conditional*

`CORE.md:102` currently promises: *"extraction runs with libarchive's secure flags
(`SECURE_SYMLINKS`, `SECURE_NODOTDOT`) so a hostile archive cannot write outside its target."*
`PXX-2-001` falsifies it — the header-encrypted 7z branch does not call libarchive at all.

**This draft must not be applied yet, and the reason is the point.** CORE describes what the program
*is*. The replacement fix is designed and measured but **not in the tree**, so applying a sentence
that describes the fixed program would be CORE describing behaviour the code lacks — class 5, the
exact class the ten doc-as-tests exist to catch, committed deliberately. So there are two artifacts
here and their order is fixed:

**Now — a Deviations entry, because the honest record of an unfixed hole is a deviation:**

> **Deviation.** §3's `arch` row states that extraction runs under libarchive's secure flags. That
> is true of every path libarchive reads. It is not true of the header-encrypted 7z branch, which
> libarchive cannot open and which therefore writes through `std::fs` — no flag of libarchive's is
> in force on it, and a symlink or hardlink already on disk at the destination redirects the write
> outside it. Recorded rather than quietly repaired: the sentence was believed when written, and the
> branch that falsifies it was added later without anyone re-reading it.

**When the fix lands — the row's replacement clause:**

> …extraction through libarchive runs under its secure flags (`SECURE_SYMLINKS`, `SECURE_NODOTDOT`)
> so a hostile archive cannot write outside its target; and the header-encrypted 7z branch, which
> libarchive cannot read and which therefore writes through `std`, earns the same guarantee in its
> own code rather than inheriting it — every directory component proven a real directory beneath the
> destination before it is created, and every member written to a name that has just been unlinked
> and is then created exclusively, so that neither a symlink nor a second name for an inode
> elsewhere can stand where the write is about to land.

**One error in the originating draft is recorded rather than silently fixed:** the tier-3 reviewer's
own version carried a comment reading *"CORE §6's sentence"* where line 102 is the **§3** module
table. It caught this itself and said so. The wording above says §3.

### Draft 4 — §3's `tasks` row. Covers `PXX-3-002` and `PXX-3-003`

Two sentences appended to `CORE.md:105`. Agent 3's, and both are corrections to claims the code
makes about itself rather than claims CORE makes:

> The lock a rebuild takes is named from the target's canonical path, and where that path cannot be
> canonicalised — which is every creation, since `apply` refuses a creation whose target already
> exists and `realpath` cannot resolve what is not there — the name falls back to the path as typed,
> so two spellings of one destination take two different locks and both proceed. The temp file
> beside the target is checked for and then created in two steps rather than one; the atomic
> `create_new` the code's own comment names is not there, and between the check and the create is a
> window a second writer can stand in.

Both sentences describe **what is true today**, not what should be, so they are applicable now.

### Draft 5 — §3's `cli` row, the exit codes. Agent 5's

`CORE.md:109` says the `cli` module owns *"their exit codes"* and never says what they are. All three
are behaviourally pinned by `tests/cli_path.rs` and named once in code at `cli.rs:45-47`
(`OK = 0`, `FAILED = 1`, `MISUSE = 2`) — so this is a number the tests can already check and the
document simply does not carry.

> …their output and their exit codes, which are three and mean three things: **0** the command did
> what it was asked, **1** it was asked correctly and could not — a missing archive, a wrong
> password, a full disk — and **2** it was asked wrongly, which is the only code that prints the
> usage text.

### Draft 6 — a Deviations entry for the transcribed C constants. Agent 5's

Not a §-row edit. `PXX-5-001` established that the gate guarding the hand-transcribed `termios` ABI
proves less than it appears to, and the only real fix — a build-time C probe, bindgen, or a `-sys`
crate — is already rejected by project convention, which makes this the maker's by rule 7.

> **Deviation.** `cli`'s terminal handling declares `struct termios` and three C constants by hand
> rather than binding them, because a `-sys` crate is not wanted in this tree. Six compile-time
> assertions guard the layout, and they are honest about less than they look: they catch a field
> reorder, which is what they were added for, but `NCCS` at 32, 33, 34 and 35 all produce the same
> sixty-byte layout and the assertions cannot tell those four apart; and `ECHO` and `TCSAFLUSH` are
> preprocessor values transcribed as ordinary integers, which nothing in the build ever reads a
> header to check. The transcription is correct against the installed glibc, verified by compiling a
> C program against those headers rather than by reading them. What holds it is the ship platform:
> `PKGBUILD` pins `x86_64`, both release containers are glibc, and the binary links
> `tcgetattr@GLIBC_2.2.5`. On a libc whose struct is larger than sixty bytes the failure would be
> silent and memory-unsafe, and nothing here would catch it.

### What is owed, and to whom

| Draft | Target | Applicable |
| --- | --- | --- |
| 1 — §2 typeface | `CORE.md:87-92` | **Now.** Closes the round's second freeze-blocker |
| 2 — §3 threading | `CORE.md:114-115` | **Now.** No doc-as-test reads §3's prose; zero code change |
| 3a — Deviations, the 7z write path | Deviations section | **Now**, and it is the honest record until 3b |
| 3b — §3 `arch` row | `CORE.md:102` | **Only after `PXX-2-001`'s fix is in the tree.** Applied earlier it makes CORE describe a program that does not exist |
| 4 — §3 `tasks` row | `CORE.md:105` | **Now.** Describes today's behaviour |
| 5 — §3 `cli` row | `CORE.md:109` | **Now.** The codes are already pinned by tests |
| 6 — Deviations, transcribed constants | Deviations section | **Now** |

---

## Phase 3 — `PXX-7-004` refuted at tier 2, and settled by the compiler

The re-run of the contaminated site came back **`defect: NO`**, and it is the most useful verdict in
the tier. It agrees with the original on every fact and disagrees about whether the fact is a defect.

### The arithmetic, confirmed to the file

The original claimed 9 insertions covering 30 of 34, leaving `arch.rs`, `cli.rs`, `secret.rs` and
`lib.rs`'s own scope. The blind re-derivation produced the same total **and decomposed it**, which
the original did not:

| Insertion | Covers |
| --- | --- |
| `#[forbid(unsafe_code)]` on `pub mod estimate;` | 1 |
| on `pub mod model;` | 1 |
| on `pub mod platform;` | 8 — `mod` + apps, clipboard, open, picker, scratch, store, window |
| on `pub mod sevenz;` | 1 |
| on `pub mod tasks;` | 1 |
| on `pub mod theme;` | 1 |
| on `pub mod ui;` | 15 — `mod` + about, extract, filter, inspector, keys, measure, newarchive, openwith, password, pending, settings, sidebar, table, tray |
| on `pub mod util;` | 1 |
| `#![forbid(unsafe_code)]` at the top of `main.rs` | 1 |

29 from `lib.rs`, plus `main.rs` = **30**. Uncovered: `arch.rs`, `cli.rs`, `secret.rs` — each holds
`unsafe` and so cannot sit inside any `forbid` scope — and **`lib.rs` itself, which holds no unsafe
at all** and is excluded for a structural reason: the only attribute that could cover a declaring
file is an inner `#![…]` covering the whole crate, and that crate lexically contains the other three.
**There is no lint scope meaning "this file's own tokens but not its child modules."** 30 + 4 = 34.

This is what the round asks for and what the original did not supply: a total that is the sum of a
list you can name. *A number nothing can check* is this project's unbeaten class, and a covered-file
count is exactly such a number.

### Settled by running it, not by reading it

The confirmer filed two `NEEDS-RUN` commands rather than asserting lint semantics from memory. Both
were run here — tier 2 route (b), reproduction on the maker's machine:

```
$ cargo rustc --lib -- -D unsafe_code
   FAILS. Lint hits, by file:  arch.rs 34   cli.rs 6   secret.rs 1
   By kind:  37 usage of an `unsafe` block
              2 usage of an `unsafe extern` block
              2 declaration of an `unsafe` function

$ cargo rustc --bin indium -- -D unsafe_code
   Finished `dev` profile ... in 4.12s        exit 0
```

Two results, and the second is the one that matters. **The lint fires in exactly three files** — no
fourth, no macro-expanded surprise in the other 29 — and **`main.rs` compiles clean under it**, a
direct compile witness that it is a separate crate root outside `lib.rs`'s reach and is itself
unsafe-free.

And the standing baseline held **exactly**: `usage of an unsafe block` = **37**, the number this
round has carried since the fleet was designed, now confirmed by the compiler rather than by a grep.
The other four hits are an `unsafe extern` block in each of `arch.rs` and `cli.rs` and two
`unsafe fn` declarations in `arch.rs` — a different counting basis, fully reconciled, not a
discrepancy. *Ask the program, not the record of it*, turned on the round's own record, and the
record was right.

### Why `NO`, and why that is not a technicality

Three reasons, in the confirmer's order:

1. **The block is correct and self-checking.** All eleven declarations resolve; 1 + 1 + 11 + 7 + 14 =
   34 with no orphan file; and `the_architecture_table_names_every_module_and_nothing_else` already
   closes the loop against disk *and* against CORE §3, in both directions.
2. **A crate-root `#![forbid(unsafe_code)]` is structurally impossible, not merely absent.** `forbid`
   is precisely the level a nested scope cannot escape — a nested `#![allow(unsafe_code)]` under it
   is a hard error, E0453. The attribute cannot be placed at `lib.rs` at all without failing the
   build. **A gate that cannot be correctly placed is not a missing gate.**
3. **The strongest thing this site could actually carry is `deny`, which is weaker than the framing
   implies.** `deny` is escapable by any future module adding an `allow`. It is a greppable review
   signal, not a non-bypassable compiler gate.

**And the honest ceiling is lower than 30/34 suggests.** `build.rs` is a **third** crate root, at the
repo root, outside the 34 — and its entire body links libarchive. Every archive byte this program
reads crosses the `arch.rs` FFI boundary into C that `unsafe_code` says nothing about, as it says
nothing about `eframe`/`glow`, `wl-clipboard-rs`, `image`, `sevenz-rust2` or `ashpd`, which is where
supply-chain unsafe actually lives. The measure's honest description is *"the 30 files that are not
the FFI boundary cannot grow one"* — **not** "INDIUM forbids unsafe." A badge saying the latter would
be a claim, not a record.

A one-insertion alternative exists and was not in the original: `[lints.rust] unsafe_code = "deny"`
in `Cargo.toml` reaches every package target — the lib, the bin, **and the five `tests/*.rs`
integration crates, which no `lib.rs` attribute can reach at all.** Still `deny`, never `forbid`,
and still needing the same three opt-outs.

### The disposition, and whose it is

The two passes agree on every fact and disagree on one judgement: whether the absence of an
inexpressible gate is a *defect*. `PXX-7-004` is the largest code change among the twenty-one — nine
insertions — and reclassifying it from `fix-in-v2.5` to `document-only` would be a policy call about
how much elective hardening a freezing repository takes on.

**That is rule 7: some decisions are the maker's by category.** It is recorded as
**REFUTED-ON-SEVERITY, facts confirmed**, and it is **not** reclassified here. Editing another
agent's severity to agree with a later argument is the class-12 shape this document exists to
prevent. The ledger's counts are left exactly as the fleet filed them.

### One more finding, and it is a trap laid for whoever implements this

| ID | Site | What it is | Severity |
| --- | --- | --- | --- |
| `PXX-T2-008` | `lib.rs:41-43` | `the_architecture_table_names_every_module_and_nothing_else` builds its `declared` set with `include_str!("lib.rs").lines().filter_map(\|l\| l.trim().strip_prefix("pub mod "))`. An outer attribute written **inline** — `#[forbid(unsafe_code)] pub mod estimate;` — no longer starts with `pub mod ` after trimming, so that module **silently drops out of `declared`** and the doc-as-test fails with a misleading message about what `lib.rs` declares versus what `src/` holds. Written on its own line, as rustfmt places item attributes, the parser never sees it and the test is unaffected | document-only |

Verified here by reading `lib.rs:39-48`. This is the shape the round keeps meeting: **the gate that
guards the change is broken by the change**, and it would have been discovered as a confusing red
test rather than as a known cost. It is filed `document-only` because nothing is wrong today — it is
a note owed to whoever applies the patch, if the maker decides it is applied at all.

---

## Phase 3 — the CORE drafts, continued

Agent 9 drafted nothing and was right to: it grepped `CORE.md` for every defective figure it found
— 1.37, 1.87, 1.88, 1.95, 2.24, 2.45, 318, 328, 27.4, 4.42 — and **all ten are absent**. Every wrong
number it found is `theme.rs`-comment-local: the file disagreeing with its own arithmetic or its own
git history, never with the maker's text. Agent 6 also drafted nothing, on the grounds that a draft
for `PXX-6-010` would mean guessing which side of the Password/Measure asymmetry is the anomaly —
which is the settling rule 8 forbids. Both are correct calls and are recorded as such.

---

## Phase 3 — tier 2 completed: the seven that were re-run, and the second refutation

The `ui/mod.rs` set was assigned to a confirmer that **died on a session limit before reporting a
single verdict**. Its only output was an observation that `HEAD` had moved beneath it. The seven were
re-run rather than assumed, because a tier that reports on findings nobody re-derived is worth less
than no tier at all — and the re-run returned **six confirmations and one refutation**, which is not
what a rubber stamp returns.

The brief carried `file` + `line_range` + a one-word category and nothing else, plus the scoping rule
the earlier leak taught: **never `grep -r` from the repository root.** It reported no slips, and its
`git status --porcelain` came back empty.

### The verdicts

| key | ID | site | verdict | what the blind pass said |
| --- | --- | --- | --- | --- |
| A1 | `PXX-1-001` | `ui/mod.rs:756-759`, `:1115-1117` | **CONFIRMED** | *"The drain observes only messages, never channel state"* — `try_iter()` yields identically on `Empty` and `Disconnected`, so a worker that dies without sending its terminal message is indistinguishable from a slow one. Located the reason precisely: in both spawn closures the terminal send lives **inside** the match on the call's return value (`:2162-2182`, `:1896-1898`), so an unwinding thread sends nothing at all. `progress.is_some()` then holds line 1115 true and the UI busy-repaints at full frame rate for the life of the process |
| A2 | `PXX-1-002` | `ui/mod.rs:1432-1434` | **CONFIRMED** | *"`self.entries` is not the archive — during a listing it is a growing prefix of it."* Confirmed the refusal is permanent for that queue and that `staging_refusal()` (`:1395-1424`) checks creation, format and encryption and **says nothing about `self.listing`** |
| A3 | `PXX-1-003` | `ui/mod.rs:1861-1863` | **CONFIRMED, and extended** | Same mechanism, then further than the original: on `Failed` or `Cancelled` there is no re-open, so the truncated table **persists indefinitely with nothing on screen saying so** — and `E` with an empty selection builds `wanted` from `self.entries` (`:2200-2204`), extracting only the partial set and reporting *"Extracted N entries"* as if that were the archive |
| A4 | `PXX-1-004` | `ui/mod.rs:1038-1041` | **CONFIRMED, and the fix located** | Named the value the handler should have read: **`self.apply_target`**, recorded at `:1891` with a comment stating exactly why it exists — *"so `on_exit` cannot be told a target this Apply never had"* — and set to `None` at `:1022`, sixteen lines before `:1038` re-derives the same fact from mutable state |
| A5 | `PXX-1-005` | `ui/mod.rs:2241-2258` | **CONFIRMED** (`certain` on the buffering) | Found the doctrine the fallback contradicts, four lines above the neighbouring function: *"`io::copy` and not `read_to_end`: the whole point is that no member is ever held whole"* (`arch.rs:1312`). And the contrast that proves the shape was already known here — `request_preview` (`:1362-1375`) caps at `PREVIEW_CAP` **and** runs on a worker with a channel |
| A6 | `PXX-1-011` | `ui/mod.rs:3423-3429` | **CONFIRMED, and worsened** | Added the second-order effect: because `change_settings` leaves `self.settings` untouched on `Err`, **the bookmark is still in the sidebar list on the very next frame** — the window shows the row while the status bar claims it was removed |
| A7 | `PXX-4-002` | `ui/mod.rs:2478` | **REFUTED** | See below |

**Two negatives it recorded rather than dropped**, and they are worth as much as the confirmations,
because they mark where the next round need not look:

- The `wake` computation (`:786-793`) omits `picker_msgs` and `reveal_msgs` — and this is **harmless**:
  both workers `tx.send(...)` then `ctx.request_repaint()` in that order and deliver exactly one
  terminal message (`:2002-2005`, `:2021-2024`), so a single frame suffices. This is adjacent to A1
  and is *not* A1: A1 is about a thread that sends nothing, and that claim stands untouched.
- `work_running()` inside `open_archive` cannot silently skip A4's re-open, because `apply_rx` is
  cleared at `:1021` before the call and `extract_rx` is set only at `:2146`. The confirmer suspected
  this first, checked it, and reported the negative — the target's *provenance* is the defect, not a
  missed gate.

### `PXX-4-002` refuted — the asymmetry was the design, not the defect

The original filed `clipboard::offer()` as a UI-thread hazard on the grounds that its two siblings,
`open_directory()` and `paste_paths()`, are both `thread::spawn`-wrapped and it is not. Every fact
holds. The blind pass answered **`NO`** anyway, and the reason is that the asymmetry is load-bearing:

| | blocks on | timeout available? | threaded? |
| --- | --- | --- | --- |
| `request_paste` | *"whichever program owns the selection"* — an untrusted peer, writing into a pipe | **no** (stated at `:2027-2031`) | **yes** |
| `clipboard::offer` | the compositor: one connect-and-round-trip | bounded by the compositor | **no** |

And the fork hazard that would have made it serious is absent. **Verified here, in the vendored
crate:** `wl-clipboard-rs` 0.9.3's `copy_internal` takes the non-foreground branch, which is
`thread::spawn` at `copy.rs:975` — the only two appearances of the word *fork* in that file are doc
comments at `:706` and `:754` describing how `wl-copy` uses the API, not what this crate does. So the
multithreaded GUI process is never forked, and the module header's claim at
`platform/clipboard.rs:57-61` — *"serves requests from a thread inside this process"* — is accurate.
The serving thread is not a leak either: `Event::Cancelled => source.destroy()` (`copy.rs:346`) plus
the serve loop's `all_destroyed` exit (`:526-536`) retires a replaced offer's thread. Failure returns
`Err` and is handled: `Err(e) => self.status = Status::bad(e)` at `mod.rs:2492`.

**REFUTED. Recorded as the second refutation of this tier, and not reclassified** — for the same
reason `PXX-7-004` was not. The severity a fleet agent filed is that agent's; editing it to agree
with a later argument is the class-12 shape this document exists to prevent. The disposition is the
maker's under rule 7, and both refutations are put in front of him with their reasoning intact.

That two of twenty-one came back refuted, both with the facts conceded and the *inference* denied, is
the strongest single argument for the tier existing. Neither would have been caught by a confirmer
that read the original's reasoning first.

### Three more findings, and the first is a comment promising what the code omits

| ID | Site | What it is | Severity |
| --- | --- | --- | --- |
| `PXX-T2-009` | `ui/mod.rs:2141-2144` | `spawn_extract` installs a fresh cancellation flag **without** the `self.cancel.store(true, …)` that `reset_view:620` and `begin_apply:1862` both perform — and the two lines immediately above it read *"Same preemption Apply makes, for the same reason."* **The comment asserts the parity the code does not have.** A listing in flight when an extraction starts is left holding an `Arc` no code path can ever raise, so P7 §7's rug-pull guard cannot stop that walk; it self-heals only because the replaced `list_rx` makes the worker's sends fail. The exact inverse of `PXX-1-003`, at the third of the three `Arc::clone(&self.cancel)` sites | fix-in-v2.5 |
| `PXX-T2-010` | `ui/mod.rs:1023-1027` vs `:736` | `ApplyMsg::Done` sets *"Applied. The archive now holds N entries."*, then calls `open_archive` at `:1055`, which sets `self.status = "Reading …"` unconditionally at `:736` before a single frame is painted; `ListMsg::Done` then replaces that with the archive's name. **The only confirmation that a rebuild succeeded is visible for zero frames.** Unlike a discarding open, it has no `discarded_on_open`-style carrier (`:728`) to ride on | fix-in-v2.5 |
| `PXX-T2-011` | `wl-clipboard-rs-0.9.3/src/copy.rs:988` | `copy_internal` ends `if let Some(err) = rx.recv().unwrap()`. A panic inside the library's own prepare thread drops `tx`, `recv()` returns `Err`, and the `unwrap()` **panics INDIUM's main thread and kills the process.** Dependency-internal and requiring a prior panic — but explicitly *outside* the standing fact that `src/` carries no `unwrap` beyond `#[cfg(test)]`, which is why it is written down rather than assumed covered | document-only |

All three verified here against source: the three `Arc::clone(&self.cancel)` sites are `:740`, `:1893`
and `:2155`, and `self.cancel.store` appears at exactly `:620` and `:1862` and nowhere else;
`open_archive:736` sets the status with no guard; `copy.rs:988` is as quoted.

`PXX-T2-009` is the round's cleanest class-9 specimen — **the same defect one door over, in the
opposite direction**, with a comment above it claiming the parity is already there. `PXX-9-008`'s
absence and `P15:75`'s *"a sweep is not a habit"* both said to look for this; it was found by giving
one confirmer the neighbouring lines and no story about them.

### One prose figure checked, and it was right

The original `PXX-1-011` says the `Recents` arm sits *"eight lines above"*; the blind pass wrote
*"four lines above"*. Both are true of different anchors — the two **writes** are at `:3419` and
`:3427`, eight apart, and the comment stating the rule ends four lines before the `Bookmarks` arm
opens at `:3423`. **No correction is owed, and this is recorded because it looks like a drift and is
not.** In a round that hunts numbers nothing can check, a figure that survives checking is worth the
sentence.

### Tier 2, closed

| Outcome | Count | Findings |
| --- | --- | --- |
| **CONFIRMED** | **18** | The mechanism re-derived independently, from the lines alone |
| **DIVERGENT** | **1** | `PXX-2-002` — the same six lines, two different defects. Owes a third look before any patch |
| **REFUTED** | **2** | `PXX-7-004` (severity; facts conceded), `PXX-4-002` (mechanism; facts conceded) |
| **Total** | **21** | Every `fix-in-v2.5` finding has met tier 2 |

**Tier 2 is complete at 21 of 21.** Eleven findings were produced *by* the verification tier that the
eleven-agent fleet did not file — `PXX-T2-001` through `-011` — which is the measurement that matters
most about it: the tier is not a formality bolted onto the audit, it found roughly a tenth again as
much as the audit did, by the single expedient of not showing anyone the answer first.

### The register's total, stated once so nothing has to infer it

| Source | Count |
| --- | --- |
| Numbered fleet findings (`PXX-<agent>-<nnn>`) | **95** |
| Closed seed, never severity-tagged (`PXX-385`) | **1** |
| Filed by the verification tier (`PXX-T2-001` … `-011`) | **11** |
| **Total findings in this round** | **107** |

The fleet ledger above is **left exactly as the clerk wrote it at 96** and is not restated to 107.
Rule 4 makes this document append-only, and a count is corrected by a later count that says what it
includes, not by an edit that makes the earlier one appear never to have been wrong. `PXX-9-008` is
not among the 95: it is a numbering gap, and the only place that string appears is the clerk's own
sentence saying so.

### The verification tier verified: tier 0 over its own eleven

A tier that demands a quote check of every fleet finding and exempts its own is not a gate, it is a
privilege. So the eleven `PXX-T2-` findings were put through the same mechanical pass agent 11 ran on
the ninety-six: each cited file re-opened, each quote confirmed present at the cited range, each line
number checked for drift.

| ID | Cited | Quote present at range? | Note |
| --- | --- | --- | --- |
| `PXX-T2-001` | `theme.rs:156` → `CORE.md:368` | **yes, and worse than filed** | `CORE.md:368` is the palette table's `\| --- \| --- \|` separator row — it contains no prose at all. The quoted sentence *"It is not an accent and never decorates"* is the `Warning` row at **`:374`**, six rows down. The citation does not merely point at the wrong line; it points at a line that could never have held a sentence |
| `PXX-T2-002` | `README.md:87` | yes | Under blind tier 2 as this was written |
| `PXX-T2-003` | `extract.rs:109` | yes | `let path = app.extract_path.trim().to_string();` — and `expand_tilde` is **defined in this same file** at `:201` and applied on the Go path at `:176`. The pin site is the one place in the file that handles a typed path without it |
| `PXX-T2-004` | `extract.rs:115-126` | yes | `:115` opens `if !app.settings.bookmarks.iter().any(…)` and the status at `:125` sits **inside** it, so the already-pinned case falls out of the block having set nothing |
| `PXX-T2-005` | `arch.rs:573` | yes | `archive_read_add_passphrase(a, c.as_ptr());` with no comparison against `ARCHIVE_OK` — eleven lines above `archive_read_open_filename`, whose return **is** checked at `:584` |
| `PXX-T2-006` | `arch.rs:756-771` | **yes, and worse than filed** | The file *states the rule it breaks*: `arch.rs:850` reads *"The archive's own root is not one of its members. See `is_archive_root`."* Both filtering sites — `:811` and `:851` — are in the libarchive path. The 7z branch is the only listing route that admits the root |
| `PXX-T2-007` | `tasks.rs:1654` | yes | And it is `PXX-3-001`'s immediate neighbour, not a duplicate of it: the comment is at `:1654`, and the swallowed open (`if let Ok(handle)`) and swallowed sync (`let _ = handle.sync_all()`) that `PXX-3-001` files are at `:1658-1659`. One site, two findings — a wrong reason above two dropped errors |
| `PXX-T2-008` | `lib.rs:41-43` | yes | `include_str!("lib.rs")` / `.lines()` / `.filter_map(\|l\| l.trim().strip_prefix("pub mod "))`, exactly as cited |
| `PXX-T2-009` | `ui/mod.rs:2141-2144` | yes | `self.cancel.store` appears at exactly `:620` and `:1862` and nowhere else; the three `Arc::clone(&self.cancel)` sites are `:740`, `:1893`, `:2155` |
| `PXX-T2-010` | `ui/mod.rs:1023-1027` vs `:736` | yes | `open_archive` sets the status at `:736` with no guard of any kind |
| `PXX-T2-011` | `copy.rs:988` | yes | In the vendored crate, verbatim |

**Eleven of eleven clear tier 0.** Two came out of the check *stronger* than they were filed, which
is the argument for running it: `PXX-T2-001`'s citation turns out to name a table separator, and
`PXX-T2-006` turns out to break a rule its own file writes down eighty lines later. Neither
strengthening was available to the pass that filed them, because both were found by re-opening the
file rather than by re-reading the finding.

**Eight of the eleven are `document-only` and therefore complete** — tier 1, by the plan's own rule
that a finding changing no code is settled by tier 0. The three that are `fix-in-v2.5` —
`PXX-T2-002`, `-009`, `-010` — went out to blind confirmers of their own, because the tier assigns by
**severity, not by origin**, and a finding produced by the verification pass owes exactly what a
finding produced by the fleet owes.

**And the recursion stops there, stated rather than left to be discovered.** A confirmation may itself
produce a finding, which would owe a confirmation, without end. The rule this round adopts: **a
finding produced by a tier-2 confirmation is filed and tier-0'd, and enters tier 2 only if it is
`fix-in-v2.5` or above; a finding produced by *that* pass is filed, tier-0'd, and carried to v2.6 as
an open item rather than confirmed here.** The round is not permitted to chase its own tail into the
freeze. What it *is* required to do is say where it stopped, which is this sentence.

---

## Phase 3 — tier 2 on the tier's own three, and a finding withdrawn

The three `fix-in-v2.5` findings the verification tier produced went out to blind confirmers under
the same brief the fleet's findings got: `file` + `line_range` + one word, and nothing else. Two came
back confirmed. **The third refuted a finding this pass had filed itself, and it was right.**

| ID | Site | Verdict | Confidence |
| --- | --- | --- | --- |
| `PXX-T2-002` | `README.md:87` | **CONFIRMED** | `certain` |
| `PXX-T2-010` | `ui/mod.rs:1023-1027` | **CONFIRMED** | `certain` |
| `PXX-T2-009` | `ui/mod.rs:2141-2144` | **REFUTED** | `certain` |

### `PXX-T2-002` confirmed, and it is six sites rather than one

The README does not carry one stale version string. It carries **six**, and every one of them is
wrong: the version badge at `:9`, the two `pacman` lines at `:65` and `:68`, the two `apt` lines at
`:73` and `:76`, and the tarball name at `:87`. Counted here directly: `grep -c '2\.1\.0' README.md`
returns **6**.

Those six are not six findings. They are the three already filed — `PXX-10-002` (the badge),
`PXX-10-005` (the four install filenames) and `PXX-T2-002` (the tarball) — and the count closing
exactly is itself the check: the class-9 sweep found no seventh site.

**What the blind pass added, and it is the part that matters.** It closed the one live counter-argument
nobody had addressed: that a README might legitimately describe the last *published* release rather
than the tree. `git show v2.3:README.md | grep -c '2\.1\.0'` returns **6** as well. The drift is
baked into the v2.3 release tree itself, so this is not a working tree running ahead of its front
page — it is a front page that was already two tagged releases stale when those releases were cut.
Last edit to `README.md` is `e34e844`, *"P22: … a front page read against the tree"* — a v2.1-era
commit, in a round whose own title claims the page was read against the tree.

### `PXX-T2-010` confirmed, and the file already had the cure

The confirmation traced the whole chain and reached `certain`: `self.status` is a single slot
(`:150-155`) with no queue anywhere in the program; `open_archive` overwrites it at `:736`; the paint
happens at `:3051` → `:3863`, after `drain_worker` ran at the top of `ui()`. So the sentence is
destroyed **inside the same synchronous call stack** that wrote it, and the careful singular/plural at
`:1025` is unreachable output.

Then it found the thing that turns this from an oversight into an omission: **`discarded_on_open`
(`:289-302`) exists for precisely this hazard**, and its own comment says so — that `open_archive`
sets *"Reading …"* and `ListMsg::Done` overwrites it *"so a sentence said at the close would be gone
before it was read."* The sentence is parked in a field at `:728` and composed into the surviving line
at `:812-815`. Three siblings use the pattern or avoid the hazard: `ExtractMsg::Done` (`:838-848`)
survives because nothing re-opens after it; `close_archive` (`:684-692`) sets its sentence after the
transition; `PickerFor::Preselect` (`:969-987`) reasons explicitly about answers that arrive frames
later. `ApplyMsg::Done` is the one site that neither uses the carrier nor avoids the need for it.

**And one path does escape**, which the original had not found: `path == None` at `:1054` needs both no
creation recipe in the queue and no `archive_path` — reachable by pressing *Discard* during a creation
rebuild, since the tray stays clickable and `discard_tasks` has no `work_running()` gate. There the
sentence survives, **and the archive that was just written is never opened at all, the draft is never
cleared, and the window sits empty.** That is `PXX-8-003`, independently re-derived from a different
site by an agent that had never seen it — and extended, because `PXX-8-003` named the skipped
`draft.clear()` and did not name the archive that goes unopened.

### `PXX-T2-009` refuted, and it was this pass's own finding

Filed here, verified here against source, and **wrong**. The facts all held: `spawn_extract:2144`
installs a fresh flag with no `store(true)`, where `reset_view:620` and `begin_apply:1862` both raise
first. The inference was inverted, and the blind pass showed why in three legs — each of which is
verified here:

1. **Raising the flag there would break a listing the user still wants.** `arch::list` answers a
   raised flag with `ListMsg::Done { count }` (`arch.rs:797-799`, `:762-764`), which the handler at
   `:799-816` treats as a completed listing — table truncated, status clobbered. And the file states
   the intended behaviour in as many words at `:3947-3949`: *"`E` works while a listing is still
   streaming in, and when both are true the Cancel has to reach the worker that is writing files."*
   Replace-without-raise **is** that sentence's implementation.
2. **Only a listing worker can hold the outgoing flag at `:2144`.** All four callers gate on
   `work_running()`, which refuses on `extract_rx`/`apply_rx`, and both are cleared only in terminal
   message arms.
3. **Orphaning the flag is observably equivalent to raising it.** `arch::list` returns on send failure
   at the same loop iteration the flag check would have caught, so a stale walker dies within one
   entry either way; and the Cancel button is drawn only when `progress.is_some()` (`:3915`), so it
   always names the newest worker.

**The comment, which was the finding's strongest point, does not say what this pass read it as saying.**
`:2141-2142` reads *"Same preemption Apply makes, for the same reason: an extraction is work the user
is waiting on, and a measurement is not"* — and the line it introduces is `self.cancel_estimate()`.
`begin_apply:1859-1860` says the same thing about the same call: *"The estimate is advisory and the
rebuild is not, so the advisory one goes."* **Both comments are about the estimator.** The
`store(true)` at `:1862` is a separate and entirely uncommented act. So the comment claims parity on
`cancel_estimate()`, which is byte-identical between the two sites, and claims nothing about the flag.
Read correctly, there is a **missing comment** where this pass read a **false one**.

**Withdrawn.** And the resolution is clean rather than merely negative: the *inverse* is the defect.
`begin_apply:1862` raising the flag over a live listing is `PXX-1-003`, which tier 2 confirmed
independently — and this confirmer reached it too, adding that the behaviour is *inconsistent between
runs*, because an intervening extraction orphans the flag and there is then nothing to raise.

That the tier refuted a finding the tier itself filed, on the strongest evidence that finding had, is
the most useful single result of the whole verification pass. A gate that never fires on its own
author's work is not a gate.

### Four more findings, and the stopping rule applied to them

These come from the confirmation *of a confirmation* — the second-order pass. **Under the stopping
rule recorded above they are filed, tier-0'd here, and carried unconfirmed rather than chased**, and
the severity column is the confirmer's judgement, not a disposition dressed up as one.

| ID | Site | What it is | Severity | Disposition |
| --- | --- | --- | --- | --- |
| `PXX-T2-012` | `ui/mod.rs:1825` | `let needs_source = self.entries.iter().any(\|e\| e.encrypted);` reads a **partial listing**, so an Apply started while rows are still streaming can decide no password is needed, skip the upfront prompt, and fail in the worker instead. A class-9 sibling of `PXX-1-002`, which is the same partial-`entries` read one decision over | correctness | carried to v2.6, unconfirmed |
| `PXX-T2-013` | `ui/mod.rs:2131` | `spawn_extract`'s doc says *"so the three callers cannot drift apart"*. There are **four** — `:2212`, `:2334`, `:2401`, `:2533` — since `bring_from_archive` joined at P22. A number nothing can check, in a sentence about drift | doc-drift | carried to v2.6, unconfirmed |
| `PXX-T2-014` | `ui/mod.rs:2150` vs `arch.rs:1079` | The UI seeds `total: wanted.len()` from selection strings; the worker reports `total: selected.len()` after directory expansion. A single-directory selection shows `0/1` and then jumps to `1/57` | resource | carried to v2.6, unconfirmed |
| — | `ui/mod.rs:2200-2201` | **An extension to `PXX-1-003`, not a new ID.** `E` with an empty selection snapshots `self.entries` under the comment *"Nothing selected means the whole archive"* — and needs **no** precondition at all: no truncated listing, no prior Apply. Press `E` while 500 of 200,000 rows have arrived and exactly 500 are written, reported as *"Extracted 500 entries."* `work_running()` does not count a listing, so the press is not refused. `PXX-1-003` reached this consequence only through its own truncation; it is reachable directly | correctness | folded into `PXX-1-003` |

All four confirmed present at their cited lines here, by reading them.

### What "blind" was actually able to mean, disclosed

One confirmer reported a slip it could not have avoided: **the harness injected the maker's own
`MEMORY.md` into its context before its first action** — a path the brief marks off limits. It
disclosed unprompted, characterised the content, and showed it bears on nothing at this site.

The verdict stands, and the *method* is what gets recorded: **blindness in this round means blind to
the finding and to `build/docs/`, not blind to everything outside `src/`.** An agent's context is not
wholly under the briefer's control, and a brief that says "off limits" cannot make it so. Nothing here
was contaminated — the injected file concerns commit conventions and machine layout and names no site
under audit — but the *next* round cannot assume the instruction is sufficient, and this is the
sentence that tells it so. The other confirmer's slip was smaller and of the same shape: a `find` over
`build/` printed filenames under `build/docs/` without opening one.

### One thing settled before stage 3 trips over it

Three dates disagree, and only one of them is authority. The changelog's newest stanza is stamped
`Fri, 14 Aug 2026` (`build/package/deb/changelog.Debian`); `RELEASE_DATE` in `about.rs:28` reads
`2026-08-14` and agrees with it, pinned by
`the_date_about_prints_is_the_one_the_changelog_stamped`; the `v2.3` **tag** is dated `2026-08-15`;
and the README badge reads `2026-08-12`, which is v2.0-era.

**The badge's authority is the changelog stanza, not the tag.** The tag-to-changelog gap is one
evening — the stanza is stamped 23:30 +0300 and the tag commit landed the next day — and it is not a
defect, because nothing claims they match. Setting the badge from `git log` at v2.5 would introduce a
*fresh* disagreement with a constant a test already pins, which is why it is written down here before
the round that edits that badge begins.

Also flagged and deliberately **not** guessed at: the two package-size badges at `README.md:15-16`
(9.80 MB / 6.92 MB) are v2.1-era on the same reasoning as the version badge, and no local artefact is
a release authority. They need a measurement, not an estimate, and they are stage 3's to take.

---

## Phase 3 — `PXX-2-001` closed, and draft 3's condition met

The one freeze-blocking *code* defect is fixed and in the tree at **`401c5d5`**. It had been designed,
tier-3 reviewed with a REPLACE verdict against the original patch, and measured — and it had not
landed, which the tier-2 section recorded as the largest thing this round still owed.

### The evidence, in the order it was produced

The reproduction was written **before** the fix and run against the unfixed tree, where it failed
with its own message:

```
the payload was written through a symlink to
/tmp/indium-test-pxx2001-sym-106255-0/outside/pwned.txt, outside the destination
```

Then the fix, then the same test, passing. **Fail-before, pass-after** — tier-2 route (a), and the
strongest evidence the tiers recognise. The order matters and is recorded because a test written after
a fix demonstrates that the fix does what its author expected, which is a weaker claim.

### What landed, and what did not

**Agent 2's `O_NOFOLLOW` patch is not what shipped**, and the tier-3 verdict that refused it was
right for a reason worth keeping in one place: `O_NOFOLLOW` does not refuse a hardlink, because a
hardlink is not a link the kernel resolves — it is a second name for one inode. So:

| part | what it does |
| --- | --- |
| `create_dir_under(root, rel)` | Descends one component at a time, asking `symlink_metadata` before each step **precisely because it does not follow the last component**: a symlink *to* a directory answers `is_dir() == false` there, which is exactly the case that must be refused. `root` — the destination the *user* named — is still created with `create_dir_all`, because refusing a destination somebody reached through their own symlink would be the guard overreaching into their business |
| the write | Unlink, then `create_new`. **The unlink is the part that severs a hardlink**, because it removes the *name*. `create_new` is `O_CREAT\|O_EXCL`, so it then refuses anything that has reappeared there — including a dangling symlink, which `std::fs::write` followed and created through |

And a distinction the code states in a comment because it is easy to get backwards: **severing is not
refusing.** A destination holding a stale link is not a hostile archive, and the user asked for their
files, so the entry is replaced and the extraction still succeeds. A patch that returned an error here
would have passed a security test and broken every ordinary re-extraction.

### Two tests, and neither one is the gate

`a_link_planted_in_the_destination_cannot_redirect_an_encrypted_header_write`
(`tests/read_path.rs`) covers three variants — symlink, hardlink, and a linked *directory* component —
and pins the case that must keep working: a destination the user named through their own link.
`create_dir_under_refuses_a_link_and_permits_everything_else` (`src/arch.rs`) asserts six legitimate
cases and three refusals, and the legitimate ones are asserted at greater length than the hostile
ones on purpose.

**Sabotaged in both directions before either was trusted**, and the result is the argument for having
two:

| sabotage | security test | permissiveness test |
| --- | --- | --- |
| guard made permissive (the pre-fix code) | **FAILS** | passes |
| guard made refuse-everything | **passes** | **FAILS** on an existing directory |

A fix that refused all input would have shipped behind the security test alone. That is class 4 — a
test weaker than its name — caught by construction rather than by luck.

**Suite: 374 → 380**, zero failures, `fmt` and `clippy --all-targets -D warnings` clean.
`src/lib.rs` 286 → 291 and `tests/read_path.rs` 34 → 35, which is the tier-3 verdict's own predicted
286→287 and 34→35 plus the four road-table gates committed separately at `a3aa129`.

### Recorded and not closed

An intermediate-component race remains, between the `symlink_metadata` and the `create_dir`. Closing
it needs `openat2(RESOLVE_BENEATH)` and descriptor-relative writes throughout, which is a larger
change than this branch. It requires a hostile process **already running as the user**; what the fix
closes is an archive doing it alone. The code says so at the helper, in the same words.

### Draft 3's condition is met, which changes which half applies

Draft 3 was deliberately split, and its own words were: *"When the fix lands — the row's replacement
clause."* **The fix has landed, so the replacement clause is now the applicable draft and the
Deviations entry written beside it is superseded** — that entry described an unfixed hole, and the
hole is fixed. Applying it now would make CORE describe a defect the program no longer has, which is
the same class-5 error as the one that kept it from being applied before, pointing the other way.

**But the replacement clause should not go in alone**, because it ends *"so that neither a symlink nor
a second name for an inode elsewhere can stand where the write is about to land"* — and the residual
race means that is true of an archive acting alone, not of a concurrent hostile local process. The
sentence it sits beside has the same character: libarchive's own `SECURE_SYMLINKS` carries a
comparable limitation. So the draft below is at parity with its neighbour, and this round's rule is to
state a limit rather than let it be inferred:

**Draft 3b — §3's `arch` row, now applicable.** As written in draft 3 above, unchanged.

**Draft 3c — a Deviations entry, replacing draft 3a and narrower than it:**

> **Deviation.** §3's `arch` row says the header-encrypted 7z branch earns the secure-flag guarantee
> in its own code. It earns it against the archive: no name the archive supplies, and no link already
> sitting at the destination, can redirect a write outside it. It does not earn it against a hostile
> process running concurrently as the same user, which could swap a directory component for a link in
> the window between the check and the creation. Closing that needs `openat2(RESOLVE_BENEATH)` and
> descriptor-relative writes through the whole branch. Recorded because the guarantee is real and
> bounded, and a bounded guarantee stated as an unbounded one is the class this round hunts.

### The register, counted again

| Source | Count |
| --- | --- |
| Numbered fleet findings (`PXX-<agent>-<nnn>`) | **95** |
| Closed seed, never severity-tagged (`PXX-385`) | **1** |
| Filed by the verification tier (`PXX-T2-001` … `-014`) | **14** |
| **Total findings in this round** | **110** |

The earlier counts of 96 and 107 are left standing. Rule 4 makes this document append-only, and a
count is corrected by a later count that says what it includes — not by an edit that makes an earlier
one appear never to have been written.

### What this fix now owes

It is a new artifact, so it owes its own tier-3 review **by an agent that did not write it** — the
rule that produced the REPLACE verdict on the previous patch applies to the thing that verdict
produced. That review is commissioned and its outcome is not recorded here, because it has not
returned. A class-9 sweep runs beside it over every other filesystem write in the tree, on the
principle this project wrote down after fixing nineteen of something and finding the twentieth a
milestone later: **a sweep is not a habit.**

---

## Phase 3 — two more CORE drafts, and one decision the round may not make

### Draft 7 — §7's beta clause. Annotated, and the verdict withheld

`CORE.md:509-522` carries the beta and its condition. Two of its sentences are now in play, and they
are not in play the same way.

**The first is simply false and can be annotated.** *"the gate is a testing round against a released
build carrying it, and no such round has been run."* Such a round **has** now been run: the 158-step
walk against the released `v2.1`, which returned 139 approvals and is recorded in this document. The
annotation:

> …the gate is a testing round against a released build carrying it. *(Run in PXX, against the
> released `v2.1`: 158 steps walked, 139 approvals, every denial closed. The sentence above said "no
> such round has been run" from P18 until then, and is annotated rather than rewritten so the road
> reads as what happened.)*

**The second is the maker's and this round will not touch it.** The same paragraph says, in its own
words: *"**What "real hands" means is deliberately left undefined**: it is a decision the maker has
not made, and recording the sentence is not the place to make it for him."*

So the beta has a two-part condition, and **only one part is mechanical.** A testing round against a
released build carrying P12's and P13's work — done, evidenced, checkable. Whether that round
constitutes the design work having *been in real hands* — undefined by deliberate choice, reserved by
name, and **rule 7's exact shape: a decision that is the maker's by category.**

**This corrects the plan that commissioned this round.** That plan states *"§7 beta gate is met by the
walk already done"* and *"v2.5 lifting the beta is therefore honest"*, and lists as a v2.5 commitment
that *"the release notes drop the beta sentence and say in one line why."* The mechanical half
supports it. The reserved half is not the plan's to answer, and CORE outranks a plan: `CORE.md:3-5`
gives the document to one hand, and this paragraph puts this specific question in it explicitly.

**So the round's output here is evidence and a question, not a lifted beta:**

> The mechanical half of §7's beta condition is met and the evidence is in `PXX.md`. The half §7
> reserves — whether that walk is the design work having been in real hands — is yours. If it is met,
> the release notes drop the beta sentence with the walk named as the reason. If it is not, they keep
> it, and the round records that the gate was reached and not passed. Either answer is a complete
> outcome; the round taking the decision itself is not.

A beta lifted by an agent reading a sentence that says the decision is not the agent's would be worse
than a beta left standing.

### Draft 8 — the road table's three new rows, and the shape the new gate permits

The table stops at P22 / `v2.1`. Three tags exist beyond it — `v2.2` (`e4718e4`), `v2.3` (`d70cb0f`)
and the unreached `v2.5` — and the road is where they get written down.

**Before drafting the rows, the constraint, because the new test discovered it rather than the
drafter.** `every_tag_core_seven_names_is_one_the_release_workflow_would_accept` reads the **last cell
of each row** and parses it as one tag. A Tag cell holding two — `**`v2.2`** and **`v2.5`**` — does
not parse as either, and the gate refuses the row. So **a row carries exactly one tag, or a held-tag
em dash, and nothing else.** That is not a rule anybody wrote; it is what the table has always done
and what is now enforced. A round that had written PXX's two tags into one cell would have found out
at `cargo test` instead of at the push, which is the entire point of the gate existing.

That forces the shape below — PXX takes two rows, because PXX carries two tags:

> | PXX | The round that ends the beta: the suite run against what shipped, the instrument checked in, and 158 steps walked against the released `v2.1` | **`v2.2`** |
> | P23 | The redesign the fifth project's study asked for: zones with corners, a cast measured rather than eyeballed, Caskaydia, the type scale named in one place, popups that move | **`v2.3`** |
> | PXX Phase 3 | The hardening: eleven agents over thirty-four files, a tiered verification gate, and the write outside the destination that libarchive's flags were never in force on | **`v2.5`** |

**Checked against both new gates by their own rules, before the maker applies anything.** All three
are the two-numeral form with no revision, which is what `release.yml` writes at `pkgrel` 1 — so each
parses, and none is the `v2.5.0-1` shape that no push can be accepted under. And the sequence
`v2.1 → v2.2 → v2.3 → v2.5` is non-decreasing, so the ordering gate passes too. This is reasoned from
the rule rather than executed, because executing it would mean writing into `CORE.md`; the mechanical
confirmation arrives the instant the rows land, which is the correct division of labour and not a
gap.

**One thing deliberately not drafted.** Whether the third row is titled *"PXX Phase 3"* or given a
name of its own is the maker's — he named PXX himself, and the plan that commissioned this round
reserved the naming for him in as many words when it declined to invent one for the redesign. The row
is drafted with a placeholder that says what it carries, and the title is his to set.

## Phase 3 — the class-9 sweep: the twentieth door, and the one this tree already got right

`PXX-2-001` closed a write that escaped the destination. Closing it is not the same as closing the
*class*, and this project has the receipt: `P15.md:75` records a sweep that fixed nineteen sites and
was followed one milestone later, **by the same hand**, by the twentieth. **"A sweep is not a
habit."** So the fix was followed by a sweep whose only question was: *where else does this tree write
to a path it did not fully choose, through a handle that follows a link?*

The sweep was read-only and barred from the build lane, which was held by the tier-3 review of the fix
itself. Its enumeration is closed and stated as such: **26 mutating call sites in production, 17 in
test code**, reached API-first (`create_dir_all`, `create_dir`, `write`, `File::create`,
`OpenOptions`, `copy`, `rename`, `hard_link`, `symlink`, `remove_file`, `remove_dir_all`,
`set_permissions`) and then swept a second time for `write_all`, `io::copy`, `set_len`,
`create(true)` and `truncate(true)`, plus the libc, libarchive and `sevenz-rust2` layers underneath.

### The answer is yes, and it is one door over

**`tasks::apply`'s temp file is the sibling.** The fixed site is *unlink-then-`create_new`*. The Apply
temp is *unlink-then-follow*, with the unlink conditionalised and the create left following. Same two
steps, same file-writing purpose, one function over — the class-9 shape exactly, and not a
generalisation of it.

The gate, `src/tasks.rs:1518-1526`:

```rust
    // 4. Build into a temp beside the target. A leftover from an interrupted Apply is
    //    removed first — that is the whole of the orphan policy, and it only ever
    //    touches a file whose name is provably ours.
    let temp = temp_path_for(&input.target);
    if let Some(name) = temp.file_name().and_then(|n| n.to_str()) {
        if is_our_temp(name) && temp.exists() {
            let _ = std::fs::remove_file(&temp);
        }
    }
```

`Path::exists()` **traverses**. For a dangling symlink it answers `false`, the `&&` short-circuits the
removal away, and the next thing to touch that name is a link-following `O_CREAT|O_TRUNC`. The one
input the removal exists to handle is the one input that skips it.

### The measurement, because the record of a library is not the library

The 7z branch was certain on inspection: `sevenz.rs:311` calls `ArchiveWriter::create`, which is
`File::create` in `sevenz-rust2-0.21.4/src/writer.rs:93`. The tar/zip branch rested on libarchive's
flags, which is a *modelled* claim — class 7, and this round's counter-rule for it is **"ask the
program, not the record of it."** So it was asked. A C probe forward-declaring the four symbols (no
`archive.h` needed), against a dangling symlink, built with `cc` so the cargo lane stayed free:

```
rc=0 err=(none)
-rw-r--r-- 1 megas megas 0 Aug 17 12:05 victim
probe-dir/link -> .../cls9/victim
```

`archive_write_open_filename` **succeeded and created the file at the link's target.** So
`arch.rs:1582` follows too, and the finding is `certain` on both branches rather than `certain` on one
and `probable` on the other. One `cc` invocation moved a confidence label that a paragraph of
reasoning could not have.

### The guard's only reachable outcome is the fail-open one

This is the second mechanism at the same lines, and it is worse than the first.

`is_our_temp` takes `&str`:

```rust
pub fn is_our_temp(name: &str) -> bool {
    name.starts_with('.') && name.ends_with(".indium-new") && name.len() > ".indium-new".len() + 1
}
```

That signature is why the call site reaches for `.and_then(|n| n.to_str())`, and `to_str()` returns
`None` the moment the archive's filename holds a non-UTF-8 byte — `temp_path_for` builds its name by
`OsString::push`, so those bytes survive into it. `if let Some(name)` then skips **the entire block**,
and the hole widens from dangling-links-only to *every* link: a symlink to an existing file is
followed and truncated, and a hardlink is written straight into the victim inode — the precise case
`arch.rs:1134-1137`'s comment exists to explain, and the case the unconditional unlink there is the
whole answer to.

And this input class is supported on purpose. `cli.rs:392-397` records the round that adopted
`args_os` **specifically** so non-UTF-8 path bytes would stop being lost.

Now the other half, which the sweep did not state and tier 0 surfaced: on a name that *does* decode,
`is_our_temp` is a **tautology**. `temp_path_for` always yields `.<name>.indium-new` with a non-empty
`<name>` (it falls back to `"archive"`), so all three conjuncts hold by construction, and `apply`
derives the value two lines above the check. **The check cannot return `false` for any input the call
site can produce.** Its only reachable effect is the `None` arm that skips it. A guard whose passing
branch is unreachable-by-tautology and whose failing branch is fail-open is class 4 — a gate that
cannot fail — wearing a security guard's clothes, and the comment above it claims a guarantee
(*"provably ours"*) that the code delivers by accident rather than by check.

### The one this tree already got right, twenty lines away

`src/estimate.rs:561-562`:

```rust
    let path = dir.join("candidate");
    let _ = std::fs::remove_file(&path);
```

**Unconditional.** No `exists()`, no decode, no name test — so it severs a dangling symlink and a
hardlink alike, and only then does `build` reach `Writer::create`. That is the correct form of the
code in `PXX-C9-001`, in the same crate, written by the same hand, and it is not a security guard at
all: its doc-comment says *"The scratch file is removed before returning, whatever happened."* It gets
the shape right for an unrelated reason.

Which is the class-9 lesson at its sharpest. The sweep did not have to invent a fix, or read an
advisory, or reason about `openat2`. **The pattern was already in the tree, correct, and it was not
copied twenty lines away.** What made the difference at the `apply` site was adding two conditions
that each looked like caution.

### The findings — ten filed, all tier-0 clear

Every quote below was re-opened and confirmed verbatim at its cited range before filing. IDs take the
`PXX-C9-` prefix for the same reason the verification tier took `PXX-T2-`: the origin stays legible,
and the tier a finding owes is set by its severity, never by where it came from.

| id | file:lines | mechanism | cat | severity | owner | conf |
|---|---|---|---|---|---|---|
| `PXX-C9-001` | `tasks.rs:1518-1526`, `:1574-1580` | `exists()` traverses, so a dangling symlink skips the removal and both sink branches follow it | security | **freeze-blocking** | 3 | certain |
| `PXX-C9-002` | `tasks.rs:1522` | `is_our_temp(&str)` forces `to_str()`; a non-UTF-8 archive name skips the whole block, widening 001 to hardlinks and live symlinks. The passing branch is a tautology | security | **freeze-blocking** | 3 | certain |
| `PXX-C9-003` | `store.rs:271-278` | the `.broken` copy-aside is `exists()`-gated the same way; `fs::copy` follows a dangling link, writes fully attacker-chosen bytes, then stamps the source's mode on the victim | security | fix-in-v2.5 | 4 | certain |
| `PXX-C9-004` | `store.rs:329` | `atomic_write`'s pid-named tmp is `File::create` — follows; the rename then leaves `settings.toml` as the link | security | fix-in-v2.5 | 4 | probable |
| `PXX-C9-005` | `tasks.rs:1340-1346` | `Lock::take`'s `create(true)` follows a dangling link and creates a zero-byte file at an arbitrary path. **The obvious fix is wrong — see below** | security | fix-in-v2.5 | 3 | certain |
| `PXX-C9-006` | `tests/write_path.rs:944-957` | the orphan test plants an **ordinary file**; no link variant exists for the Apply temp, though extraction has exactly that at `read_path.rs:889` | test-gap | fix-in-v2.5 | 3 | certain |
| `PXX-C9-007` | `ui/mod.rs:4064-4066` | a **fixed** name written straight into the shared temp dir — `indium-6-2-not-a-folder.txt`, no pid, no pre-removal; `fs::write` follows and truncates | security | fix-in-v2.5 | 1 | certain |
| `PXX-C9-008` | `tasks.rs:2323-2327` | same shape: `indium-pxx-orphan`, fixed, no pid; plant the directory as a link and `File::create` puts `b"half an archive"` at the victim | security | fix-in-v2.5 | 3 | certain |
| `PXX-C9-009` | `estimate.rs:716`, `tasks.rs:2575`, `window.rs:222` | three pid-qualified test dirs with no pre-removal, where **nine** sibling helpers in this tree do it | security | fix-in-v2.5 | 1/3/4 | probable |
| `PXX-C9-010` | `tasks.rs:1545` | the commit rename replaces a symlink the user deliberately keeps at the archive name, silently — not this class, and recorded so it is not lost | correctness | document-only | 3 | probable |

**The register stands at 120.**

### `PXX-C9-005` is the one where the mechanical fix reintroduces a fixed defect

Nine of these ten want the same one-line answer: unlink first, unconditionally, the way
`estimate.rs:562` does. **The lock file must not get it.**

`Lock::take` opens with `truncate(false)` and never writes a byte; the file *is* the lock, and
`flock` is held on the **inode**. Unlink-then-create hands two racing processes two different inodes,
each holding a lock on its own, and both proceed. That is not a hypothetical — *"`flock` on an
inode"* is on this round's own twelve-class list as class 3, a premise that did not survive contact,
and it is already paid for once in this repository's history. A sweep that applied its own pattern
uniformly would have re-opened it.

So the fix here is the opposite one: **`O_NOFOLLOW`**, via
`OpenOptionsExt::custom_flags(libc::O_NOFOLLOW)`, refusing the link without touching the name.

And note *why* that is available here and was refused for `PXX-2-001`: `O_NOFOLLOW` does not refuse a
hardlink, which is what disqualified it there — the payload would have landed in the victim inode. At
the lock there is no payload. Zero bytes are ever written, so the hardlink case is harmless and the
only thing needing refusal is the symlink. **The same flag is wrong at one site and right at the
other, for a reason that is about the write and not about the flag** — which is exactly the
distinction a uniform sweep flattens.

### Two the sweep filed that tier 0 sent back

Recorded, because a verification pass that only ever confirms is not a verification pass.

**Refuted — `tests/write_path.rs:1154`.** The sweep reported that a panic between the `0o500`
`set_permissions` and its restore *"leaves a directory `TempDir::drop`'s `remove_dir_all` cannot
clear."* Re-reading the range shows the restore at `:1173` sits **before** the `read_dir` at `:1175`
and the `assert_eq!` after it, with a second restore on the early-return path at `:1162` — and the
comment above it states the reason in the sweep's own words: *"Put the mode back before asserting, or
a failure here leaves a directory the harness cannot clear."* The hazard named is the hazard the code
was written to avoid, and says so. **Not filed.**

**Reframed — `store.rs:274`.** The sweep called `broken.exists()` *"the whole guard."* It is not a
guard at all. The comment two lines above reads *"copy the file aside **once**"* — it is an
idempotence policy, and there was never a link check at that site to be gated. The reachable outcome
is unchanged, so `PXX-C9-003` stands as filed; the *characterisation* does not, and a fix framed as
"repair the guard" would have looked for something that was never there.

### Why 001 and 002 are freeze-blocking, with the asymmetry named rather than buried

They are not as reachable as `PXX-2-001` was, and pretending otherwise would be its own defect.
`PXX-2-001` took untrusted input from **the archive** — the thing INDIUM exists to open, untrusted by
charter, with `CORE.md:102` naming the secure flags that were not in force. `PXX-C9-001` needs a local
actor able to create a name in the directory the user is writing into. In `/tmp` at mode `1777` that
is any account on the box; in the user's own `~/Archives` it is nobody. **That is a weaker precondition
and it is stated here so the severity is not read as equivalent.**

They are filed freeze-blocking anyway, on two grounds:

1. **Freeze-blocking is a label about treatment, not about a score.** It is what routes a fix through
   tier 3 — reviewed by an agent that did not write it, its diff carrying its own blast radius. That
   is the treatment this fix needs, because it edits the commit path of the archive rebuild, and
   `PXX-2-001`'s fix has just demonstrated that the two-way sabotage is where the real risk sits.
2. **The record would otherwise be wrong.** `PXX-2-001` is written up in this document as closed. Ship
   v2.5 with the identical write one function over still following links, and the round's own record
   describes a class as handled when half of it is. That is class 5 — CORE and the record describing
   behaviour the code lacks — committed knowingly, and it is the specific error that kept draft 3a
   from being applied.

`PXX-C9-003` through `010` are `fix-in-v2.5` and owe tier 2: independent confirmation by a
non-originating agent, blind to the reasoning above and given only `file` + `line_range`.

### What is not written yet, and why

**No code changed in this section.** The build lane is held by the tier-3 review of `401c5d5`, and
`src/tasks.rs` is not to be edited underneath a reviewer that may be running `cargo test` against the
tree — a spurious failure attributed to the fix under review is worse than a delay. The fix for 001
and 002 is designed and recorded here so the design is on the record before the diff exists:

- Remove the temp **unconditionally**, matching `estimate.rs:562`, and report a hard error on anything
  other than `NotFound` — which also turns a directory at that name into a sentence instead of a
  confusing failure two calls later.
- Keep the *"provably ours"* guarantee the comment claims, but at `OsStr` level: an
  `is_our_temp_os(&OsStr)` that `is_our_temp(&str)` delegates to. Dropping the check instead would
  falsify the comment above it, which is class 1 — and correcting the code by breaking the prose is
  not a trade this round is allowed to make.
- The same `to_str()` skip exists at `ui/mod.rs:2995-3003` in `on_exit`, where it only leaves litter.
  One root, two call sites; the second is noted here so it is not found third.

Tier 3 applies to that diff when it exists, exactly as it did to `401c5d5`.

## Phase 3 — tier 3 returned REPLACE, and the round's most expensive lesson is now paid for

The plan that commissioned this round wrote tier 3 for one stated reason: **"The fix is the riskiest
artifact in this round, not the finding — a correct diagnosis with a wrong patch is the failure mode a
frozen repo cannot survive."** That was written before any agent ran. It has now been earned.

`401c5d5` closed `PXX-2-001`. The tier-3 review of it returned **REPLACE**, and it was right.

### The verdict, and what it turned on

The diagnosis was right and the guard's core function works — the reviewer proved that independently,
end to end, on a case no committed test covers: a header-encrypted 7z naming `d/inner.txt` with
`dest/d` planted as a symlink is refused, and the same archive into a clean destination gives `Ok(1)`.

But the patch shipped **a freeze-blocking functional regression on the exact branch it touched**, and
this is the part that matters:

> `create_dir_under` refuses `Component::CurDir`, so a header-encrypted 7z whose stored names carry a
> leading `./` cannot be extracted at all, and the user is told the archive tried to escape the
> destination. **Both new tests pass with the defect present.**

Every link of the proof chain was measured, not argued:

- `path_escapes` **permits** `"./"` — it neither starts with `/` nor splits to any `".."`.
- Rust keeps a *leading* `.` as a component and folds an interior one away:
  `"./alpha.txt"` → `[CurDir, Normal("alpha.txt")]`, `parent = Some(".")`; `"a/./b"` →
  `[Normal("a"), Normal("b")]`.
- `tasks::out_path_for` is explicit that *"An unrenamed member keeps its **stored** name byte for
  byte"*.

So the live route is INDIUM breaking an archive **it wrote itself**: encrypt a `./`-named 7z, and the
headers are now encrypted, and the name is kept byte for byte, and extraction of the result gives
`Err(UnsafePath("."))` — rendered to the reader as *"Refused: an entry would be written outside the
destination (.)."* A false accusation about an archive that escapes nothing.

**And the round had already paid for `./` once.** `tests/read_path.rs:139` is
`a_dot_slash_rooted_tar_lists_and_extracts_like_any_other`, whose own doc records that *"until this
round INDIUM could neither list nor extract any archive shaped that way"*, and `rooted.tar` exists
precisely because no fixture had been rooted that way for twenty-two rounds. **That test passes with
this defect present**, because `rooted.tar` is a tar and goes through libarchive; the new guard is in
the `sevenz` branch, which no `./` test could reach. A class already found, already fixed, already
regression-tested — re-broken one branch over, behind a green test that names the exact behaviour.
That is class 9 for the third time in this round, and the first time it was the round's own fix that
committed it.

### The sabotage, run both ways, before the replacement was trusted

| what was tried | `a_dot_component_…` | `a_dot_slash_name_…` | `re_extracting_over_a_tightened_…` | the two gates that shipped with `401c5d5` | the existing `rooted.tar` gate |
|---|---|---|---|---|---|
| `CurDir` refused (the rejected draft) | **FAILED** | **FAILED** `Err(UnsafePath("."))` | pass | **both pass** | **pass** |
| mode carry-over removed | pass | pass | **FAILED** — `got 644` | both pass | pass |
| the replacement | pass | pass | pass | both pass | pass |

The middle row is the reproduction of the reviewer's own measurement — `0600` before, `0644` after —
and the third column of the first row is what makes the point: **no single one of these tests is the
gate.** Each new gate fails only for its own defect, and the two that shipped with the original fix
fail for neither.

### The ten findings tier 3 produced

**Every `arch.rs` line number in this table is as-of `401c5d5`, the commit under review, and several
have moved in `5b2c19f`.** Stating it beats a tier-0 pass that fails for the right reason on the wrong
tree — a finding against a remembered line number is the class this round hunts, and a finding against
a *superseded* one is the same error wearing a date.

| id | site | what | severity | state |
|---|---|---|---|---|
| `PXX-T3-001` | `arch.rs:1010-1015` | `Component::CurDir` refused; a `./`-named entry unextractable and reported as an escape | **freeze-blocking** | **fixed, `5b2c19f`** |
| `PXX-T3-002` | `arch.rs:1001-1004` | the comment claimed the libarchive path *"creates it the same way"*; measured, libarchive **refuses** a destination reached through the user's own symlink while this branch permits it | fix-in-v2.5 | comment corrected; the behavioural divergence is the maker's |
| `PXX-T3-003` | `arch.rs:1141-1154` | unlink-then-`create_new` discarded an existing file's mode — `0600` widened to `0644` on re-extract | fix-in-v2.5 | **fixed, `5b2c19f`** |
| `PXX-T3-004` | `arch.rs:1141-1154` | after a successful unlink, a failing `create_new` leaves neither the old file nor a new one, where the old `std::fs::write` failed at the open and the file survived | document-only | recorded |
| `PXX-T3-005` | `arch.rs:1005` | `create_dir_all(root)` runs per entry, so an N-entry archive walks the destination prefix N times | no-action | recorded, deliberately not fixed |
| `PXX-T3-006` | `arch.rs:1020-1023` | a genuine on-disk-link refusal returns `Other` (a generic error) while a benign name got `UnsafePath` (the security sentence) — the two were exactly inverted | document-only | half closed by `PXX-T3-001`; the variant question deferred |
| `PXX-T3-007` | `arch.rs:1037-1040` | *"traversal is refused outright … and leaves no partial output behind"* — an on-disk-link refusal is raised **mid-loop**, so earlier entries are on the disk when it returns | document-only | recorded; a disk condition genuinely cannot be pre-flighted here |
| `PXX-T3-008` | `ui/mod.rs:880-886` | the `Failed` arm says nothing about the destination, where `Cancelled` deliberately says *"what came out is still in the destination"* — so a mid-loop refusal shows "Refused" with files sitting unmentioned | fix-in-v2.5 | owner 1; §4 status text is the maker's |
| `PXX-T3-009` | `arch.rs` libarchive branch | for a symlinked **intermediate** component libarchive silently *deletes the user's symlink* and puts a real directory there, where the sevenz branch refuses | fix-in-v2.5 | second face of `PXX-T3-002` |
| `PXX-T3-010` | `tasks.rs` `verify_against` | **found in passing, pre-existing, and not this commit's:** `apply` on a `./`-rooted tar fails with `"alpha.txt was written at 20 bytes instead of 21"` — `rooted.tar` stores beta (20) first and alpha (21) second, so sizes are attributed to the wrong members | fix-in-v2.5 | untouched by any commit this round |

`PXX-T3-010` deserves its own sentence, because it is the only one here that is nobody's regression: a
`./`-rooted tar **cannot be rebuilt** today, and the failure names a byte count for the wrong member.
It was found only because the reviewer went looking for a reachability route and hit a wall that was
already there. It owes tier 2 like any other finding.

### What this section corrects about an earlier one, by addendum

Rule 4: a P-document is append-only, and corrections go in addenda rather than rewrites. So the
earlier heading *"`PXX-2-001` closed, and draft 3's condition met"* is not edited. It is corrected
here:

**`PXX-2-001`'s fix was `401c5d5`, and `401c5d5` did not survive tier 3.** The defect it closed stayed
closed throughout — the reviewer verified that independently — but the artifact that closed it carried
two new defects, one of them freeze-blocking. The finding is closed by `5b2c19f`, not by `401c5d5`.

**And the process criticism is recorded rather than argued with.** The reviewer noted that
`60d9f16`, titled *"PXX-2-001 recorded as closed"*, landed **before** the tier-3 review returned. That
is a fair hit. The section it committed did say, verbatim, *"That review is commissioned and its
outcome is not recorded here, because it has not returned"* — so the text was honest — but the commit
**subject** announced a closure that had not cleared its own gate, and a subject line is what anyone
reading `git log` sees. Tier 3 is not advisory; a finding whose fix is under review is not closed. The
correct title would have said the fix had landed and was under review.

### Draft 3c is unaffected, and one CORE draft is now owed a word

Draft 3c bounded the guarantee as *real against the archive, not against a concurrent local process*.
Tier 3 confirms that framing independently and narrows it usefully: *"Within one call on this branch
the archive genuinely cannot exploit it — 7z carries no symlinks and this loop writes only regular
files and directories."* The residual window is also **wider** than the code's own comment admits — it
named the `symlink_metadata`→`create_dir` gap, but it extends to the open of `target`, which
re-resolves the whole path. Draft 3c's wording survives; the code comment's does not, and correcting
it is a `document-only` item for the same hand that writes the fix.

---

## Phase 3 — `PXX-C9-001` and `PXX-3-009`: the same six lines, and why that is DIVERGENT and not a duplicate

The class-9 sweep filed `PXX-C9-001` at `tasks.rs:1518-1526`. **Agent 3 had already filed
`PXX-3-009` at `tasks.rs:1521-1526`** — the same gate, in the same round, by a file owner who owned
it. This is the exact collision the deduplication pass exists to catch, and flattening it into a
duplicate would have been the wrong call. Both are recorded, and the relation is the finding.

**What agent 3 filed**, verbatim from the register:

> Between the temp unlink and the writer's open, a planted symlink redirects the build. Needs a second
> principal writable in the archive's directory

**That is a race**, and it was dispositioned **`document-only`** — one of the sixty-two, an accepted
limitation, no diff owed. For a TOCTOU window needing an attacker to win a timing gap, that
disposition is defensible and was defensibly reached.

**What the sweep found at the same lines** is that for the case that matters there is **no race to
win**. `Path::exists()` traverses, so a dangling symlink makes it answer `false`, the `&&`
short-circuits, and **the unlink never runs at all.** Nothing has to be timed. And on a non-UTF-8
archive name the entire block is skipped, because `is_our_temp` takes `&str`.

So the two findings are not the same claim at different confidence. They are **two mechanisms at one
site**, and the round's own rule for that is explicit: DIVERGENT is *"the outcome that must never be
flattened into a pass."* Agent 3 was not wrong — the race is real, and it survives the fix as the
residual, exactly as it does at `arch.rs`. It is simply not the whole of what is there, and the part
it missed needs no luck.

**The consequence is a disposition, not an argument.** A `document-only` verdict was reached on a
characterisation that understated the site. Severity in this round is set by mechanism; when the
mechanism is under-derived, the disposition inherits the error silently, and there is no gate that
catches it — the quote checks out, the reasoning is sound, and the finding is simply about less than
what is present. That is class 12 in a form the round had not yet seen: **not the record contradicting
itself, but the record being correct and incomplete at once.**

**What is *not* done here.** `PXX-3-009` is not reclassified, not rewritten, and not annotated in its
own row. Editing another agent's severity is the shape this document exists to prevent, and under rule
7 the disposition is the maker's. The two findings sit side by side with this section naming the
relation, and the maker decides whether `PXX-3-009` keeps its `document-only` row.

**Three sites, one pattern, and it is now three-for-three.** `arch.rs`'s 7z branch, `tasks.rs`'s Apply
temp, and `store.rs`'s `.broken` copy-aside all reach a write through a name they did not fully
choose. Two of the three gate their protection on `exists()`. The one that gets it right —
`estimate.rs:562` — is unconditional, and got there for a reason that has nothing to do with security.

**The register stands at 130.**

## Phase 3 — tier 2 on the class-9 sweep, and the mode-discard class found three times

Eight of the sweep's ten findings owed tier 2. Two blind confirmers ran them, each given only
`file` + `line_range` + a one-word category. **Nine of nine site-verdicts came back real** — but two
came back as something other than what was filed, which is the third time this round that a blind pass
has returned a *different mechanism at the same lines*.

### The verdicts

| filed | site | verdict | what changed |
|---|---|---|---|
| `PXX-C9-003` | `store.rs:271-278` | **CONFIRMED**, mechanism · **boundary REFUTED** | the mechanism is exactly as filed; the *actor* is not. See below |
| `PXX-C9-004` | `store.rs:329` | **CONFIRMED**, and far worse than filed | the symlink-follow is real but secondary. The primary defect **needs no actor at all** |
| `PXX-C9-005` | `tasks.rs:1340-1346` | **DIVERGENT** | the mechanism holds and is nearly inert; the real defects there are availability and privacy |
| `PXX-C9-006` | `write_path.rs:944-972` | **CONFIRMED**, by a different argument | not the missing link-variant I filed — the test cannot fail for the reason its name gives |
| `PXX-C9-007` | `ui/mod.rs:4064-4066` | **CONFIRMED** | *"the purest instance of the shape in the tree"* — a fixed name, zero entropy, in the shared temp dir |
| `PXX-C9-008` | `tasks.rs:2323-2327` | **CONFIRMED** | *"the most complete chain of the five"*: the directory name and the derived file name are both computable from public source |
| `PXX-C9-009` | `estimate.rs:716`, `tasks.rs:2575`, `window.rs:222` | **CONFIRMED** | and the sibling count corrected — see below |
| `PXX-C9-010` | `tasks.rs:1545` | **superseded** | filed as a silent symlink replacement; the confirmer found the same line discards the archive's **mode**, which is worse and measurable |

### `PXX-C9-003`: the mechanism confirmed, my severity refuted

The confirmer reproduced the `.broken` write-through with a standalone `rustc` probe: `broken.exists()`
is `false` with a dangling symlink present, `kept` comes back `true`, and the link's target is created
holding the settings text **at the source file's mode**. Precisely as filed.

Then it went and looked at who could do it, and **refuted the precondition I had written.** I said *"an
actor with write access to the config directory."* Measured: `/home/megas` is `drwx------`, so no other
local account can traverse in at all — and in the ordinary single-uid case, an actor who can plant the
symlink can already write the target file directly. **The primitive grants no capability and crosses no
boundary.** That half of my filing is wrong and is withdrawn here rather than quietly left standing.

The one boundary it found is real and narrower: **INDIUM run as root over an unprivileged user's config
directory**, which `CORE.md:657` explicitly permits — *"Allowed, for the record: … running as root"* —
and which on Wayland in practice means carrying the invoking user's environment. Then uid 1000 chooses
both the content and the destination and root performs the write. The confirmer declined to claim it
was live here, because `/etc/sudoers` was unreadable to it and `env_reset` would defeat it.

**And it withdrew an overclaim of its own, unprompted.** It expected the copy to carry setuid bits;
measured, `4755` came back `755`, because `fs::copy` chmods *before* writing and the kernel strips
`S_ISUID` on write without `CAP_FSETID`. A confirmer that reports its own failed hypothesis is doing
the job the escape valve was written for.

### `PXX-C9-004`: the same lines, a defect needing no attacker, and the class found for the third time

I filed the symlink-follow at `store.rs:329`. The confirmer found it, agreed, called it *"strictly
weaker than the mode finding"* — and then named the mode finding:

> `File::create` mints a **new inode** at `0o666 & ~umask`, and the `rename` makes that new inode *be*
> the file. Every save therefore discards the permission bits the file had. A `settings.toml` the user
> had narrowed to `0600` comes back **world-readable `0644`**, silently, with no actor involved and
> nothing reported.

Measured, and then measured against the live machine: `~/.config/indium/settings.toml` and
`~/.local/state/indium/recents.toml` are both `-rw-r--r--` on this box right now.

**And it quoted this round's own new code back at it.** `PXX-T3-003` — the mode-widening the tier-3
review found *inside* the `PXX-2-001` fix, fixed an hour earlier in `5b2c19f` — is the identical
mechanism. `store::atomic_write` had it unfixed, in a different file, and the comment that now explains
it in `arch.rs` is what the confirmer cited to prove it. **A fix that lands a lesson in one file does
not thereby land it anywhere else.** That is class 9 stated as plainly as this round has managed it.

Then it went one further, outside its three assigned sites, and named a third instance: `tasks.rs:1545`,
where **Apply's commit** renames a fresh inode over the user's archive with no prior mode captured
anywhere. It filed that `probable` and said in as many words that it had not run it.

**Measured here, it is `certain`:** `an_apply_does_not_widen_the_mode_of_the_archive_it_rebuilds`
returned `got 644` on a `0600` archive. For an AES-256 7z that puts the ciphertext where any account on
the machine can read it, after any rebuild, with nothing said. **The archive matters more than the
settings file, and it was the third place the same mechanism was sitting.**

All three are closed in `25be01d` and `5b2c19f`, by one shape: read the prior mode through
`symlink_metadata`, and set it on the **temp, before the rename**, so the file is never present at its
own name under permissions the user did not choose. Apply fails loudly if it cannot, because on that
path the original is untouched and `let _ =` there would be the silent-failure class this round audits.

### `PXX-C9-005`: DIVERGENT, and the fix I proposed does not compile

I filed the lock file's `create(true)` symlink-follow and proposed `O_NOFOLLOW`, reasoning that it is
right here and wrong at `PXX-2-001` because no bytes are written. The confirmer agreed the mechanism
exists, called it *"nearly inert"* for exactly that reason — and found two defects at those lines that
are not about symlinks at all:

1. **`.write(true)` buys nothing and can refuse Apply for good.** Measured: `try_lock()` is granted on
   a *read-only* fd, and a second fd in the same process is still refused — so write access is not what
   makes the guard work. But this option set fails `Permission denied` on a `0444` lock file, and then
   `Lock::take` refuses Apply **for that archive, permanently, with a message that never names the
   file.** Reachable with no attacker at all: one root-run Apply leaves a root-owned lock in the user's
   own lock directory, and the user's Applies on that archive are dead. `CORE.md` permits running as
   root and the doc comment builds for it, which is what makes this ordinary rather than exotic.
2. **The locks are never removed, and the name is an injective encoding of the archive path.**
   Measured live: **238** zero-byte files in `/run/user/1000/indium/locks`. In the runtime directory
   that is session-scoped as the comment claims — but the comment's claim that *"the session's own
   logout wipe clears whatever a crash leaves behind"* is **true of only one of the two branches**, and
   the `~/.cache` fallback it invokes for the root case is swept by nothing. The residue is a
   permanent, decodable list of every archive ever rebuilt.

It also refuted two things, including one of its own: the 180-byte tail truncation that makes two
archives share a lock is **deliberate and documented** at `tasks.rs:1395-1400`, so not a finding; and
its own hypothesis that the scratch sweeper might unlink a held lock is refuted by `cache_root()`
pointing at `indium/scratch` and by `owner()`'s strict parse.

**And it told me my proposed fix would not build.** `std` rejects `read(true).create(true)` outright —
*"creating or truncating a file requires write or append access"* — so dropping `.write(true)` is not a
one-line change. It also warned off the obvious alternative: unlinking the lock on drop *"would break
something the code is deliberately doing"*, reintroducing the unlinked-inode race the design exists to
avoid. **Both of those are the escape valve working**: a confirmer that refuses a fix is more valuable
than one that writes it.

### `PXX-C9-006`: confirmed by an argument I had not made, and a count corrected

I filed the orphan test's gap as *"no link variant exists for the Apply temp."* The confirmer derived
something sharper without seeing that: the test's assertions check only the **absence of named files**,
never that the resulting archive is valid — and because every successful Apply renames the temp onto
the target, `!temp.exists()` is true whatever happened to the orphan's bytes. Its conclusion:

> even if the explicit removal block were deleted entirely — a regression exactly matching the test's
> own name — this test would very likely still pass, because libarchive's open call truncates the
> orphan independently of the Rust-level guard.

It also **independently derived `is_our_temp`'s tautology**, at a different site, with no knowledge that
`PXX-C9-002` had claimed it: *"tautologically true for anything `temp_path_for` produces … it's not a
live branch."* That is blind corroboration in its intended form.

And it corrected my arithmetic. I wrote *"nine helpers pre-remove."* It enumerated **16** `temp_dir()`
call sites and found **11** that pre-clean — 8 with tag + pid + counter + pre-clean, 3 more with
pre-clean but no counter. The corrected figure is **11 of 16**, and `PXX-C9-008` is the weakest of the
sixteen on both axes at once. My nine stands corrected.

### The six findings these confirmations produced

Per the stopping rule this round wrote, a finding produced by a tier-2 confirmation is filed and
tier-0'd, and enters tier 2 only if it is `fix-in-v2.5` or above.

| id | site | what | severity | state |
|---|---|---|---|---|
| `PXX-C9-011` | `tasks.rs:1545` | Apply's commit discards the archive's mode; an encrypted 7z's ciphertext becomes world-readable on any rebuild | **freeze-blocking** | **fixed, `25be01d`** — the fix owes tier 3 |
| `PXX-C9-012` | `store.rs:322-342` | `atomic_write` discards the file's mode on **every** save, no actor required | fix-in-v2.5 | **fixed, `25be01d`** |
| `PXX-C9-013` | `store.rs:335` | the rename replaces a symlinked `settings.toml` with a regular file, so a dotfiles-managed config silently stops receiving writes | document-only | recorded |
| `PXX-C9-014` | `tasks.rs:1340-1346` | `.write(true)` is unnecessary for the lock and turns an unwritable lock file into a permanent, unexplained refusal of Apply | fix-in-v2.5 | not fixed — the fix is not a one-liner |
| `PXX-C9-015` | `tasks.rs` lock dir | locks accumulate forever under the cache fallback (238 measured live), and the comment's logout-wipe claim is true of only one of its two branches | document-only | recorded; the comment correction is class 1 |
| `PXX-C9-016` | `write_path.rs:944-972` | the orphan test cannot distinguish INDIUM's orphan policy from libarchive's incidental truncate-on-open | test-gap | recorded, with the confirmer's `NEEDS-RUN` still open |

**The register stands at 136.**

### `PXX-10-001` closed, and the class closed with a gate rather than an edit

`theme.rs` cited the cast's band test under a name missing its *six*. Fixed — and a second defect was
sitting in the same doc comment: it cited **`CORE.md:368`** for *"It is not an accent and never
decorates"*, which lives at **`:374`**. `:368` is the palette table's `| --- | --- |` separator and
could never have held a sentence. That is almost certainly where `PXX-T2-001`'s identical
mis-citation came from — **the register inherited a wrong line number from the code it was auditing.**
The citation now names the row as well as the line, because the number moves and the row does not.

The class is closed by a test rather than by the edit:
`every_test_name_a_doc_comment_cites_resolves_to_a_real_one` scans `src/` **and** `tests/` for
backticked snake_case names of at least 25 characters and 4 underscores and asserts each resolves to a
real `fn`. **40 citations checked, with a floor of 30** so a scan that finds nothing cannot read as a
scan that found nothing wrong.

Two things it taught on its first run, both kept: it flagged **its own doc comment**, because that
paragraph quoted the wrong name as the example — so the wrong name is now written unquoted, and the
comment says why. And it flagged `ui/mod.rs:119`, which wraps a citation **mid-identifier** across two
lines; rustdoc renders that as one code span, and a line-at-a-time scan called it dangling. The lint
now follows a backtick run across line breaks and strips whitespace before the lookup. **A false
positive is how a lint like this gets switched off**, so it was fixed in the tool rather than by
editing prose to suit it.

### Two process notes, both against me

**The memory note's prediction held twice.** Both confirmers disclosed, unprompted and before acting,
that the harness had injected this project's `MEMORY.md` into their context before their first tool
call. Neither had requested it; both assessed relevance and both found none bearing on their sites; one
noted that the leak is itself recorded as structural. Blindness still cannot be briefed — but
*disclosure* can be, and it worked exactly as designed on two agents independently.

**I broke a confirmer's compliance check.** Its brief required it to finish with an empty
`git status --porcelain`. It reported ` M src/theme.rs`, identified the file, the mtime, and that the
change was not its own, and declined to revert it. It was right: I edited `theme.rs` while it was
running. The instruction was mine and I made it impossible to satisfy, and the correct brief would have
scoped the check to *files the agent itself touched*. Recorded because the next round will run
confirmers alongside work again, and this will recur otherwise.

**Both confirmers also reported the advisor tool returning "unavailable" when called.** Recorded
because it matches what this session observed directly, and because a round that leans on review
should say when a review channel was not available rather than let its absence be inferred.

## Phase 3 — the freeze-blocking pair, and the guard that was reading the wrong question

`PXX-C9-001` and `PXX-C9-002` sat in six lines of `tasks::apply`, and between them they
described the whole shape of class 9: a write reached through a name INDIUM did not fully
choose, guarded by two tests that each opened at exactly the input the guard existed for.

```rust
    let temp = temp_path_for(&input.target);
    if let Some(name) = temp.file_name().and_then(|n| n.to_str()) {
        if is_our_temp(name) && temp.exists() {
            let _ = std::fs::remove_file(&temp);
        }
    }
```

Three clauses, and two of them were wrong in the same direction — **both fail open, and both
fail open on the input that matters.**

- **`temp.exists()` traverses.** For a dangling symlink at the temp's name it answers `false`,
  so the removal is skipped for the one file shape that can turn a build into a write somewhere
  else. A leftover *regular* file — the case the guard was written for, and the only case its
  test covers — is the case where skipping it costs nothing, because libarchive truncates
  whatever it opens.
- **`to_str()` answers `None` for a name that is not UTF-8.** A Linux filename is a byte string
  that promises no encoding. The code then did nothing at all, silently, about a file whose name
  **it had constructed itself** two lines earlier.

### What actually happens, measured

Not modelled. Both tests were written first and run against the unfixed code:

```
thread 'a_dangling_link_at_the_apply_temp_is_unlinked_not_written_through' panicked:
  the rebuild landed at /tmp/indium-write-apply-temp-link-141042-0/victim,
  so the link was followed instead of unlinked

thread 'a_target_whose_name_is_not_utf8_still_has_its_temp_cleared' panicked:
  the rebuild landed at /tmp/indium-write-apply-temp-raw-141042-1/victim-raw,
  so an undecodable name skipped the removal
```

And the consequence runs one step further than "a file was written." The temp is renamed onto
the target at commit — and **`rename` moves the link, not what the link points at.** So the
sequence is: the rebuild lands in the attacker's chosen file, and then the user's `.tar.gz`
*becomes a symlink* to it. The archive is not corrupted; it is replaced by a pointer to
somebody else's file, which is a worse outcome and a quieter one.

### The open NEEDS-RUN, closed by running it

A blind tier-2 confirmer predicted that `an_orphaned_temp_from_a_crashed_apply_is_overwritten_not_multiplied`
would still pass with the removal deleted outright, because libarchive truncates independently
of anything the Rust does. It filed that as a prediction and said it had not run it.

**Run:** the entire removal block was replaced with a comment, and the test passed.

That closes `PXX-C9-016` at `certain`. The orphan test cannot fail for the reason its name
gives — it asserts the absence of named files, and every successful Apply renames the temp away
regardless of what happened to the orphan's bytes. It has been green for its whole life without
ever having been able to say anything.

### The fix, and the one line of it that is a judgement call

The removal is now unconditional, matching the form `estimate`'s scratch candidate has always
used twenty lines from the code that got it wrong. `remove_file` unlinks a name and does not
care whether it resolves, so testing first bought nothing and cost the case that mattered. The
check moved to bytes as `is_our_temp_os`, with `is_our_temp` delegating to it, so the *"provably
ours"* guarantee the comment claims now holds for every name a Linux filesystem can hold rather
than for the subset Rust can decode.

The judgement call is the third change: **a failed removal now refuses the Apply** where it used
to be `let _ =` and proceed. Proceeding past a failed removal is the write-through with an extra
step. Refusing costs the user nothing at that point — not one byte has been written and the
original is untouched — and the message names the file, because a refusal nobody can act on is
the exact fault this round filed against the lock file at `PXX-C9-014`.

**And that is where a fix for one class nearly commits another.** `PXX-C9-014` is the finding
that an unwritable lock file turns into a permanent, unexplained refusal of Apply. Making a
failed unlink fatal *looks* like the same trap. It is not, and the reason is not about severity:

> **Unlink permission belongs to the directory. Open-for-write permission belongs to the file.**

Measured rather than asserted, and it comes out as a **double dissociation** — each permission
succeeds exactly where the other fails, which is the strongest form this claim could take:

| | `open` for write | `unlink` |
|---|---|---|
| file `0444` in a `0700` directory | **refused** — `Permission denied` | **succeeds** |
| file `0666` in a `0500` directory | **succeeds** | **refused** — `Permission denied` |

So in the user's own directory the unlink succeeds even against a leftover they cannot open —
the right to remove a name comes from the directory holding it — and this therefore **cannot**
become the permanent refusal the lock finding describes, where the failing operation needs
permission on the file itself. The sticky bit does not disturb that for the ordinary case:
measured at euid 1000, the owner of a file in a `1777` directory still unlinks it.

In a sticky *shared* directory the unlink **can** fail — and there the leftover is a file this
account may not remove, which is precisely the circumstance in which proceeding **is** the
write-through. The asymmetry is what makes fatal right here and wrong at the lock, and it is
written into the comment rather than left to be re-derived by whoever reads it next.

### What this does not close, said in the code rather than assumed

The temp is opened by libarchive or by the 7z writer, neither of which will take an `O_EXCL`.
So a name re-planted **between** this unlink and that open is still followed.

That is `PXX-3-009`'s mechanism — the one agent 3 filed as a TOCTOU race and which was
dispositioned `document-only`. It keeps its row. What is closed here is the mechanism that needs
no timing at all, which is the DIVERGENT half recorded two sections ago.

**A comment claiming the symlink question was closed would have been worse than no comment**:
it would be this round's class 5, committed inside the fix for class 9, and it would have
flattened the DIVERGENT distinction the previous section was written to preserve.

### Four gates, each broken alone

| sabotage | who fails | who stays green |
|---|---|---|
| put `&& temp.exists()` back | both symlink gates | the refusal gate |
| put `to_str()`/`is_our_temp` back | **only** the non-UTF-8 gate | the UTF-8 gate |
| make the failed removal non-fatal | **only** the refusal gate | both symlink gates |
| make the byte check fail open on undecodable names | **only** the unit case | — |

The second row is the one worth reading twice: the two symlink tests share a body and differ in
exactly one variable — whether the target's name happens to decode — so restoring the old
`to_str()` guard fails one and leaves the other passing. That is what makes them a pair rather
than a duplicate.

The refusal gate carries a **loud skip**: it closes a directory to build its precondition, and if
the process can still write into that directory it is root (`CORE.md:657` permits running as
root), the precondition cannot exist, and the test prints why it did nothing rather than
reporting a pass. A test that can skip silently is a gate that cannot fail, which is the class
this round is auditing.

Suite **387 → 391**, clippy clean, `cargo fmt --check` clean.

### Tier 3 returned AMEND, and the most useful thing it found was a hundred lines away

The verdict was **AMEND**, not ACCEPT — the fix is right in substance, and one specific thing had
to change.

**What it could not break, which is worth as much as what it could.** The reviewer went at the
fatal arm hardest, because that is where a fix for one class most plausibly commits another, and
**refuted its own headline hypothesis in five steps**: the arm can only fire on a non-`ENOENT`
`unlink` error at `temp`; `temp` and the target share a parent; the commit ends in a `rename` in
that parent; `rename` needs the **same** directory-modify permission `unlink` does; therefore
every permission-shaped failure of the unlink implies the Apply would have failed at the commit
anyway. It then walked sticky `/tmp`, append-only directories, `chattr +i`, SELinux
`remove_name`, read-only mounts, `EBUSY` at a mountpoint, a readable-but-unwritable directory and
root, and found no Apply that used to succeed and now fails. **No availability was lost.**

It also measured the thing that makes the arm safe in ordinary use, which I had not: on ext4, an
absent name in a `0500` directory returns **`ENOENT`, not `EACCES`** — Linux checks the negative
dentry *before* `may_delete`'s permission check. So the everyday "nothing to clean up, directory
not writable" Apply falls through the `NotFound` arm untouched.

And it settled the byte-equivalence question by exhaustion rather than by argument: a standalone
probe holding exact copies of both implementations, run over 22 hand-picked edge names, 7,644
exhaustive short decodable names and 200,000 random ones — **207,644 names, zero disagreements.**
The doc comment's claim that the two forms agree wherever both can speak is true as written.

#### The AMEND: a refusal that described a file that was not there

`unlink` reports **path-resolution and mount** failures before it ever looks for the name.
`EROFS`, `ENOTDIR`, `ENAMETOOLONG`, `EACCES` for a missing search bit — all come back for a name
that does not exist and never did. So an ordinary Apply onto a read-only mount, with nothing
planted anywhere, reached the fatal arm and was told:

> A leftover from an interrupted rebuild is in the way at `…/.out.tar.gz.indium-new` … Remove it
> and try again.

with `temp present = false` measured immediately before and after. **That is `PXX-C9-014`'s
unactionable refusal wearing a friendlier face**, committed inside the fix whose own comment cites
`PXX-C9-014` as the thing it is avoiding. Measured end to end on a read-only bind mount inside a
user namespace.

Fixed with one `stat` on an already-failing path, choosing between two sentences. The path is
named in both, so the contract the commit set itself is kept either way.

**And the gate for it needs no privileges at all.** The reviewer needed `unshare -Urm`; the same
false premise reproduces through `ENAMETOOLONG`, because the temp name is the target's plus twelve
bytes — so a 250-byte target yields a 262-byte temp name that cannot exist on ext4, with nothing
planted anywhere. That is now `an_unclearable_workspace_does_not_claim_a_leftover_that_is_not_there`,
and sabotaging the two-sentence split back to one fails it while leaving the true-premise gate
green, which is the pair proving the sentences are actually separated.

#### The sibling, in the same function, a hundred lines up

The round's best finding, and it came from reviewing the fix rather than from the sweep that was
looking for exactly this:

```rust
    // A new archive must never silently replace an existing file. `create_new` failing
    // with `AlreadyExists` is the check, and it costs nothing.
    if creating && input.target.exists() {
```

**`PXX-C9-001`'s mechanism, verbatim, in the same function as the fix for it.** A dangling symlink
at the destination is not "an existing file" to `exists()`, so Create proceeds — and the commit
`rename` replaces the link with a regular file, which is the silent replacement this guard exists
to prevent, performed by the code refusing to perform it. Reproduced independently before fixing:
`Ok(1)` where a refusal was required, and the link gone afterwards.

Nothing of the user's is destroyed, because the link pointed at nothing. **What is destroyed is
the guarantee**, and a guarantee that holds for every input except the one shaped to defeat it is
not one.

The comment above it was wrong in its own right: it credits `create_new` failing with
`AlreadyExists` — an atomic, non-traversing check — for a guarantee delivered entirely by the
line below it. Measured: **neither writer takes an `O_EXCL`.** `arch::Writer::create` and
`sevenz::Writer::create` both followed a planted dangling link and created the file at the far
end. The comment described a mechanism the code has never had.

> **Class 9 has now been found four times in this round, and this is the second time the round's
> own fix was one of them.** The sweep found three siblings. Tier 3 found a fourth, a hundred
> lines from a fix for the same mechanism, written by a hand that had just spent a section on
> `P15.md:75` — *"A sweep is not a habit."* The sweep looked at twenty-six write sites and did not
> look up.

#### Three corrections against the record, taken rather than argued with

- **The precedent was half-cited.** The comment appealed to `estimate`'s scratch candidate to
  justify unconditional-**and**-fatal. That one is unconditional and `let _ =`. It now says it is
  borrowed for the unconditionality alone, because a half-cited precedent is worse than none.
- **"Skips loudly" was false.** libtest captures the output of *passing* tests, so the skip
  message printed nothing under `cargo test`. Measured with a `rustc --test` probe. The comment
  now says the reason is visible under `--show-output`, and states the residual plainly: under
  root the gate reports `ok` without having tested anything. Failing instead was considered and
  rejected — `CORE.md:657` permits running as root, and turning a permitted configuration red is
  not a gate, it is a complaint.
- **A wrong implementation that all four gates pass exists.** Delete the `NotFound` arm — making
  *every* ordinary Apply refuse, the most destructive one-line change available there — and all
  four new gates stay green, because every one of them plants something at the temp path. The
  suite does catch it, at `an_apply_with_no_tasks_reproduces_the_archive`. Recorded, not patched:
  the honest statement is that these gates cover the planted cases and the identity test covers
  the empty one.

#### The eight findings

| id | site | what | severity | state |
|---|---|---|---|---|
| `PXX-T3B-001` | `tasks.rs:1573-1600` | the refusal asserted a leftover that on `EROFS`/`ENAMETOOLONG`/`ENOTDIR`/no-search-`EACCES` does not exist, and told the user to remove it | fix-in-v2.5 | **fixed, `c9878b2`** |
| `PXX-T3B-002` | `tasks.rs:1467` | the Create guard traverses, so a dangling symlink at the destination defeats "must never silently replace" | fix-in-v2.5 | **fixed, `c9878b2`** |
| `PXX-T3B-003` | `tasks.rs:1465-1466` | that guard's comment credits `create_new`/`AlreadyExists`; neither writer takes an `O_EXCL` and the check is never performed | document-only | **fixed with 002** |
| `PXX-T3B-004` | `write_path.rs:1086-1089` | "skips loudly" is false — libtest captures passing tests' output, so under root the gate vacates in silence | document-only | **fixed, `c9878b2`** |
| `PXX-T3B-005` | `tasks.rs:1577` | deleting the `NotFound` arm makes every Apply refuse and all four new gates still pass | no-action | recorded |
| `PXX-T3B-006` | `write_path.rs:1125` | the refusal gate's discriminating power rests entirely on one `contains`, since a fail-open still errors later at the rename | no-action | recorded |
| `PXX-T3B-007` | `tasks.rs:1564` | `estimate`'s precedent is cited for unconditional-and-fatal; it is `let _ =` | document-only | **fixed, `c9878b2`** |
| `PXX-T3B-008` | `tasks.rs:1574`, `ui/mod.rs:2996` | the guard cannot reject: all **1,885** names `temp_path_for` can produce are accepted, so "provably ours" is delivered by construction, not by the check | no-action | confirmed surviving |

**The register stands at 144.** Suite **391 → 393**.

#### What tier 3 has now cost and returned, twice

`401c5d5` went in with two passing tests and a freeze-blocking regression behind them; tier 3
returned REPLACE. `ba26617` went in with four sabotaged gates and a measured double dissociation
behind it; tier 3 returned AMEND, and found a fourth class-9 sibling the sweep had walked past.

**Neither defect was findable by the hand that wrote the fix**, and both were in the fix rather
than in the diagnosis. The charter's line — *"the fix is the riskiest artifact in this round, not
the finding"* — has now been paid for twice at full price, and the second time the fix was written
by someone who had just finished writing that sentence down.

One loose end recorded rather than dropped: under the reviewer's most destructive sabotage, a full
`--test write_path` run hung past 400 seconds where the clean baseline finishes in about two. It
is a sabotage artifact with no bearing on the shipped code — the load-bearing test ran alone and
answered — but it is filed `unverified-hypothesis` rather than discarded, because a suite that can
hang is worth knowing about even when only a deliberately broken build reaches it.

Both the reviewer and both earlier confirmers disclosed, unprompted, that `MEMORY.md` had been
injected into their context before their first action. **Three for three.** The reviewer added the
detail the earlier two did not: it stated which entries it had been given and why none bore on the
files it was judging.
