# INDIUM — the test plan

The instrument. One person, one window, one released build, working down a list that does not
change between rounds so that two runs can be compared.

**Why it is in the tree.** P11 and P12 ran eight rounds and forty-one steps against `v1.0.0-2`
and `v1.0.0-3` on the night of 9–10 August, and every step passed. That list was never checked
in. It survives only as quoted fragments inside `P11.md` and in the maker's own notes, which is
how a program comes to be certified by an instrument nobody can produce afterwards.

**Twenty-six steps below carry the mark of that round.** Fourteen of the originals survive as
verbatim quotations inside the maker's own record, which reproduced each step's wording before
answering it; the rest are recovered from the findings in that same record, and from the
coordinates `P11.md` gives — round 7 step 2, round 5 steps 2 and 3, round 4 step 2, and round 2
steps 4 and 6. There are more marked rows than recovered originals because several of the
originals are split here: a step that bundled four expectations into one line hid which of the
four had failed. The rest of this document is written for the program as it now is, twelve rounds
of features later. If this repository is to be frozen, the thing that certified it is part of what
gets frozen.

**How to use it.** Every step is `what you do` → `what must happen`, with the clause of `CORE.md`
it holds. A step passes only if what must happen happens *as written*. A step that cannot be run
is not a pass; mark it and say why. Prose in the *must happen* column is deliberate — a step
whose expectation is "works" tests nothing.

**The build under test.** A released build, installed from its own package, never the working
tree. §7's gate says *"a testing round against a released build"* and means it: a tree can carry
an uncommitted fix that no user will ever have.

**The fixtures.** Rounds 11–13 need archives measured in gigabytes, and none of them are in this
repository — they are regenerable, and P6 §9 was written after two release tarballs came within
one `git add -A` of entering history forever. `build/make-testdata.sh` builds them into
`~/indium-test`: `under-limit.tar`, `over-limit.tar`, `big-mixed.tar.zst`, `many-entries.tar`,
`deep.tar`, and the 8 GiB sparse `bigsecret-input.bin` that step 12.8 turns into an archive.
The filler is an AES-CTR keystream under a fixed pass phrase rather than `/dev/urandom`, so two
runs produce identical bytes and a figure measured here can be compared with one measured
elsewhere. Run it on `/home` and not on `/tmp`, `$XDG_RUNTIME_DIR` or the overflow partition:
those three are `nosuid,nodev`, and an extraction test that passes there passed because the mount
forbade the thing rather than because INDIUM did.

The same script also builds the two small ones the early rounds name — **`photos.zip`** and
**`docs.tar.gz`**. It did not always: they came from the P11 round, the script recorded them as
already present and left them alone, and by the end of the first certification walk they were
gone — `photos.zip` had been extracted into `~/indium-test/photos/` and the archive itself
deleted. Nine steps name it. A corpus this document calls regenerable has to actually be
regenerable, so they are built here now, and **`photos.zip` is a reconstruction rather than the
original**: same shape, same awkward names, contents chosen to answer the steps that name it. The
remaining P11 fixtures — `large.tar`, `backup.7z`, `secret.7z`, `notrar.rar`, `notanarchive.zip`,
`a.zip`, `b.zip`, `to-add/` — are still in the corpus and are left alone.

**Notation.** `[R1.4]` is round 1, step 4. **†** marks a step recovered from the P11/P12 round —
either quoted verbatim or located by the coordinate `P11.md` gives it; where that round reported
a defect, the step says what went wrong then, because a step that once failed is the step most
worth running again. **‡** marks a step P22 left unticked and this round inherits.

---

## Round 1 — the package, and getting in

| | Do | Must happen | Holds |
| --- | --- | --- | --- |
| 1.1 | `sudo pacman -U indium-2.1.0-1-x86_64.pkg.tar.zst` | Installs with no dependency INDIUM did not declare, and no prompt about replacing a file another package owns. | §2, §8 |
| 1.2 | `pacman -Ql indium \| wc -l`, then look for the icons | **42.** The package carries 45 tar entries and three of them — `.PKGINFO`, `.BUILDINFO`, `.MTREE` — are metadata `pacman -Ql` does not count. Every one of the ten hicolor sizes is on the disk, plus the desktop entry and both licence files. | §8 |
| 1.3 | Find INDIUM in the desktop's application menu | It is there, under its own name and its own icon, without logging out. | §8 |
| 1.4 | `indium --version`, then `indium --help` | `indium 2.1.0`. The help names `list`, `extract`, `cat` and nothing that does not exist. | §4 |
| 1.5 | Launch INDIUM with no argument | A window opens with no archive in it. The table says so and offers the way in rather than being blank. | §4 |
| 1.6 | Press `1` with nothing open | *File* is enterable. A window with no archive still has somewhere to be. | §4 |
| 1.7 | `indium ~/indium-test/photos.zip` | The window opens on that archive, entries listed, breadcrumb showing its name. | §4 |
| 1.8 | Open `large.tar`, `docs.tar.gz`, `backup.7z`, and a `.deb` from `/var/cache/pacman/pkg/` | All four open and list. The `.deb` is read as a container, not refused. | §5 |
| 1.9 | Open `notrar.rar` | *"RAR is not supported."* — a plain sentence, no popup, no crash. Not read, not offered. | §5 |
| 1.10 | Open `notanarchive.zip` (53 bytes of nonsense) | Refused with a sentence naming the file. The window stays usable. | §5 |
| 1.11 | Open a file that does not exist | Refused with a sentence. No empty window, no silent nothing. | §4 |
| 1.12 † | **`cd ~` first**, so you are standing somewhere that is not the repository, then run it by its full path: `~/INDIUM/build/install-desktop.sh` | It works from anywhere — it finds its own payload by resolving its own path, not by trusting the working directory. *(P11 round 4 step 2 — it failed then with `./build/install-payload.sh: No such file or directory`. The step is written this way because the first walk read it as a relative path and denied the program for the plan's omission.)* | §8 |
| 1.13 | Still in `~`, run `~/INDIUM/build/install-desktop.sh --set-default`, then double-click a `.zip` in the file manager | INDIUM opens it. Undo afterwards if you would rather keep your own association. | §8 |

## Round 2 — the window's furniture

| | Do | Must happen | Holds |
| --- | --- | --- | --- |
| 2.1 | Press `1`, `2`, `3`, `4` in turn | *File*, *Draft*, *Recent files*, *Bookmarks*. The digits follow the order the rows are drawn. | §4 |
| 2.2 | Read the sidebar top to bottom | *File* `1`, *Draft* `2`, *Create* `N`, then *Open file* `O`, *Recent files* `3`, *Bookmarks* `4`. | §4 |
| 2.3 | Press `F1` | The keys table is drawn in the window, and every row in it is a key that works. | §4.9 |
| 2.4 † | Press `O`, then *Add files…* | Both raise the desktop's own picker through `xdg-desktop-portal`, not a picker INDIUM drew. *(P11 round 2 step 4)* | §4.8 |
| 2.5 † | Drag the window's bottom edge up as far as it goes | The sidebar does not overlap itself. There is a floor. *(P11 round 2 step 6)* | §6 |
| 2.6 | Run the pointer down the table and over the sidebar † | Rows wash lightly under the pointer without disturbing the row cursor. Double-click descends into a folder. *(P11)* | §6 |
| 2.7 | Press `A` | About: the mark, the maker, version and date, the source address, the licence in full. | §4.6 |
| 2.8 | In About, try to click the source address | It selects as text. No browser opens. INDIUM follows no link. | §4.6, §9 |
| 2.9 | Press `,` | Settings, with **exactly three** groups: *Extract* default destination, *Bookmarks*, *Recent files*. | §4.5 |
| 2.10 | Count the popups you can reach by key | `N` `W` `E` `A` `,` `F1` `Ctrl+O`, plus Open With, Password and Measure, which have no key of their own. Ten. | §4 |
| 2.11 | With any popup open, press `Esc` | The topmost popup closes, and only that one. | §4 |
| 2.12 | Resize the window narrow, then wide | Nothing clips, nothing overlaps, the breadcrumb elides rather than pushing the layout. | §6 |

## Round 3 — reading an archive

| | Do | Must happen | Holds |
| --- | --- | --- | --- |
| 3.1 | Open `photos.zip` (40 rows at the top level), walk the table with arrows, `PgUp`/`PgDn`, `Home`/`End` | The cursor moves as named. `Home` reaches the first row, `End` the last. **One documented behaviour is part of the pass, not against it:** under a *held* key the bottom row's text steps between two brightnesses at the key-repeat rate and settles back the moment you let go. It was measured rather than guessed — one row, identical geometry, identical background, a uniform intensity multiplier that reaches even the Size cell, whose colour no branch in INDIUM's row code can change. That exonerates the row code and puts it in the toolkit's paint-and-present layer, where this repository has no line to fix. Approve if the cursor lands where it is named; the stepping is expected. | §4 |
| 3.2 | Press `Enter` on a directory, then `Backspace` | Descends, then goes back up. The breadcrumb follows both ways. | §4 |
| 3.3 | Press `Space` on a file | Details ⇄ Preview. Press again to go back. | §4 |
| 3.4 | Preview a text file, then a `.jpg`, then a binary | Text as text, image as image, binary as hex — not text with holes in it. | §4 |
| 3.5 | Press `Ctrl+F`, type a fragment | The filter bar takes it and the table narrows. `Esc` closes it and the table returns. | §4 |
| 3.6 | With the filter closed, press a bare letter | It is a shortcut, not a search. There is deliberately no type-to-jump. | §4 |
| 3.7 | `Ctrl+A` | Every row in the current view is selected — and only the current view if a filter is on. | §4 |
| 3.8 | Open `many-entries.tar`, press `End` | It lists and scrolls without stalling; the table virtualizes rather than drawing all of it. | §4 |
| 3.9 | Open `secret.7z`. **Its password is `indium`** — the fixture's, written down here because it is a fixture's, and `tests/fixtures/README.md` has said so since P1. It holds one member, `f.txt` | The password popup appears **at open**, before any entry is listed — its headers are encrypted, so the listing is itself the moment of use. (`bsdtar -tf secret.7z` refuses it too: *"The archive header is encrypted"*.) | §4.7 |
| 3.10 | Give it the wrong password | Refused with a sentence, and asked again. Nothing partial is listed. | §4.7 |
| 3.11 | Give it the right one — `indium` — then close and reopen the archive | It lists `f.txt`, and on reopening it asks **again**. The password nowhere survives its use. | §4.7, §9 |
| 3.12 | Open `deep.tar` (60 levels, long names, spaces, tabs, a newline, Turkish, emoji) | The breadcrumb elides in the middle rather than growing off the edge; every name is listed as one row, and the one containing a newline does not become two. | §4, §6 |
| 3.13 | Extract **all** of `deep.tar` into `~/indium-test/sandbox` — **that directory and nowhere else** — then `ls ~/indium-test` and `ls ~` (the two places the four members aim at: one up and via-middle land in the first, two-up and the absolute one in the second) | It carries four traversal members (`../`, `../../`, one via a middle component, and one absolute — `/home/megas/escaped-absolute.txt`). `path_escapes` (`arch.rs:940-946`) must refuse every one, saying so rather than silently dropping them. **Deny** if any `escaped-*.txt` exists afterwards outside the sandbox. All four aim at paths this user really can write, `$HOME` included — a member aimed at `/` would be stopped by `EACCES` whether or not INDIUM refused it, and would pass for the wrong reason. This is the only step in the plan that could write outside its target, which is why it names its target twice. | §3, §4.3 |
| 3.14 | Open `under-limit.tar` — an archive made the commonest way there is, `tar -cf x.tar -C dir .`, so its first stored member is the archive root `./`. Compare against `bsdtar -tf under-limit.tar`, then extract **one** member, `part-a.bin`, anywhere convenient | It lists exactly three rows — `part-a.bin`, `part-b.bin`, `part-c.bin` — with **no empty first row**, and the one member extracts. `bsdtar` shows four entries because it shows the `./` root; INDIUM drops the root and keeps the names, which is the same archive read correctly. **This step exists because v2.1 could not do it.** The `./` root normalised to an empty path, a guard written for a locale defect could not tell that apart from a name that genuinely had not survived the read, and INDIUM refused the whole archive with *"this archive holds an entry whose name could not be read on this system"* — blaming the reader's machine for its own normalisation, on the commonest tar shape in existence. | §4, §5 |

## Round 4 — out of an archive

| | Do | Must happen | Holds |
| --- | --- | --- | --- |
| 4.1 † | Select two files, `Ctrl+C`, then paste into a folder in Dolphin | Two real files land there, contents and names intact — including the one with a space and the Turkish one. *(P11)* | §4 |
| 4.2 | Repeat 4.1 using Dolphin's *right click → Paste* | Same result. (P11 found `Ctrl+V` in Dolphin failed where right-click worked — check both.) | §4 |
| 4.3 | Press `E` | The Extract popover: *Extract here*, *Extract to `<name>/`*, a path field with tab completion, bookmarks beneath. | §4.3 |
| 4.4 | Type a partial path in the field and press `Tab` | It completes, and the caret lands **at the end of the line**. *(P11 round 5 step 3)* † | §4.3 |
| 4.5 | *Extract here*, then *Extract to `<name>/`* | Flat into the current directory; then under a directory of the archive's name. Both exactly as labelled. | §4.3 |
| 4.6 | Extract with a bookmark chosen from beneath the field | It goes to the bookmarked directory. | §4.3, §4.5 |
| 4.7 | Extract a selection, not the whole archive | Only what was selected is written. | §4.3 |
| 4.8 † | Open `large.tar`, `Ctrl+A`, `Ctrl+C`, then press Cancel while it runs | The bar moves, Cancel actually stops it, the status line says how many entries were written, and nothing partial reaches the clipboard. *(P11 — the original run was too fast to catch; use a GB fixture.)* | §4 |
| 4.9 † | Press `Enter` on a text file inside an archive | Applications appear, best match first. Pick one and it opens. | §4.4 |
| 4.10 † | In Open With, use `↑`/`↓` then `Enter` | The arrows move the selection; the filter bar does not swallow them. *(P11 round 5 step 2)* | §4.4 |
| 4.11 † | With the file open in the other program, close INDIUM | The other program keeps running. | §4.4 |
| 4.12 | Type in Open With's filter | The list narrows by name as you type. | §4.4 |
| 4.13 † | Put the caret in a **named text field** and copy from there. The shortest route: press `Ctrl+F` for the filter bar, type a few letters, select them (`Ctrl+A` inside a focused field selects the field, not the table), then `Ctrl+C`. The Extract popover's path field, the `F2` rename box and `Ctrl+O`'s field all behave the same way. Then a second leg with no field at all: drag across the status line to select some of its words, and press `Ctrl+C` | The letters land on the clipboard, and **no extraction starts** — no progress bar, no status line about entries. *(P11 found the progress bar flashing here.)* **First read the other half of this, or the program will be denied for working:** with focus in the **table** `Ctrl+C` is *supposed* to start an extraction — that is step 4.1, and copying entries out as real files is what the chord means there. The thing under test is only that a focused text field, or a live selection in an ordinary label, takes the chord for itself instead. | §4 |

## Round 5 — into an archive

| | Do | Must happen | Holds |
| --- | --- | --- | --- |
| 5.1 † | Copy `~/indium-test/to-add/pasted.txt` in Dolphin, press `Ctrl+V` in INDIUM | A strip appears above the status bar reading *1 change — add pasted.txt*, with *Discard* and *Apply*. *(P11: NOT WORKING then.)* | §4 |
| 5.2 † | Drag `dropped.txt` from Dolphin onto the window. **On this desktop, which is Wayland-only, that is the whole step** — try the drag, and watch what the window does | **On Wayland: nothing is added, and nothing else happens either — no error, no phantom row, no strip that says *1 change* and cannot name it. That is the pass, and it is ticked green like any other step.** INDIUM is a Wayland program (§1) and the drop protocol it would need is X11's; the honest outcome of a drag it never receives is silence. **On X11**, if there is ever a machine to try it on: the strip says *2 changes*, and dragging three at once gives three, not one. This step has no *not applicable* button on purpose — a verdict of two buttons that quietly grows a third is a verdict nobody can count. | §4 |
| 5.3 | Press `I` with the File section showing | Adds into the directory the breadcrumb names. | §4 |
| 5.4 | Select a file, press `Del` | A remove is staged, not performed. The strip counts it. | §4 |
| 5.5 | Select a file, press `F2`, give a new name | A rename is staged. The name shown is the new one. | §4 |
| 5.6 | Rename to a name containing a `/` | Refused with a sentence. | §4 |
| 5.7 | Press `W` | Pending tasks: one row per staged operation, each with its own ✕, then *Discard all* and *Apply*. | §4.2 |
| 5.8 | Remove one row with its ✕ | Only that operation goes. The others stand. | §4.2 |
| 5.9 | *Discard all* | The strip empties. The archive on disk is untouched. | §4.2 |
| 5.10 | Stage several changes and press *Apply* | The archive is rewritten. Names with spaces and Turkish characters survive exactly. | §4.2 |
| 5.11 † | Start an Apply on a GB archive and cancel it partway | The original opens unchanged and there is **no leftover `.indium-new`** beside it. *(P11 — never actually caught; the GB corpus is what makes this runnable.)* | §4.2 |
| 5.12 † | Open the same archive in two windows, Apply in both | The second refuses with *"Another INDIUM window is rebuilding this archive"* and writes nothing. | §3 |
| 5.13 | Apply, then check the directory beside the archive | No temp file, no orphan, no `.indium-new`. | §3 |

## Round 6 — the draft, and Create last

*P22's round. The draft is the source of truth and the queue's creation is a projection of it.*

| | Do | Must happen | Holds |
| --- | --- | --- | --- |
| 6.1 ‡ | Press `2` with nothing open | The Draft section, empty, saying so and offering *Add files…*. | §4 |
| 6.2 ‡ | *Add files…*, choose several | They are listed, each row carrying its own ✕. | §4 |
| 6.3 | Remove one with its ✕ | Only that one goes. | §4 |
| 6.4 | Open an archive, **select at least one entry** — the button has nothing to bring without one — then press `2` and click *Bring from archive* | They are copied out and added to the draft **as files**. **The button being dead before you select something is the design, not the defect:** it goes live only when there is both an open archive and a selection, and INDIUM writes a sentence beside it naming whichever of the two is missing. On v2.1 that sentence shares a row with the buttons and a narrow window can push it out of sight, which is worth knowing before reading the greyed-out button as broken. Approve on the copy landing in the draft once a selection exists. | §4 |
| 6.5 | Close the archive they came from — delete it on disk | The draft entries stay. They are files from that moment on. | §4 |
| 6.6 | With the draft empty, press `N` | The popup opens and **its own button is dead, with a sentence**. | §4.1 |
| 6.7 | With the draft full, press `N` | The popup opens and **Measure is live from the first frame**. | §4.1 |
| 6.8 | Read the popup top to bottom | Instruction line; four preset chips — *Fastest*, *Balanced* (default), *Smallest*, *Encrypted*; every method carrying its one-sentence verdict and nothing else. | §4.1, §5 |
| 6.9 | Click each preset chip | Each highlights its row in the method list below. | §4.1 |
| 6.10 | Open *Advanced* | The level slider, and only there. | §4.1 |
| 6.11 | Read the sentence at the foot | It states exactly what will be built — name, format, codec and level, and encryption if on. Change a control and it changes. | §4.1 |
| 6.12 | Press *Measure* | The Measure popup opens **over** Create — the only popup drawn over another. | §4.10 |
| 6.13 | Watch it fill | All eight rows stand from the first frame; cells fill as candidates land. A table that grows is a table that moves. | §4.10 |
| 6.14 | Read the figures | Level, time, size, ratio. Told in text, never in colour. The popup states what it weighed. | §4.10 |
| 6.15 | Measure something too large to weigh whole | A `~` marks the ratios the sample could not promise. | §4.10, §5 |
| 6.16 | Click a row | That method is chosen. The measuring was to decide, so the answer is the control. | §4.10 |
| 6.17 | Close Measure, reopen it | The figures are still there. *Measure again* is how they are spent. | §4.10 |
| 6.18 | Close Create entirely, reopen | The figures are gone. A figure that outlives its input is folklore. | §4.10 |
| 6.19 | Press Create, then change the draft | INDIUM says to restage, and **names the key that fixes it**. | §4 |
| 6.20 ‡ | Press Create, then discard the queue | The draft still stands. A recipe can be thrown away without throwing away the files. | §4 |
| 6.21 ‡ | With a creation staged, try `F2` or `Del` on an open archive | Refused with a sentence, not folded into the queue. | §4 |
| 6.22 | Press Create and look at the title bar and status line | Nothing has adopted a filename that does not exist yet. Draft, then queue, then the archive on disk. | §4 |

## Round 7 — Close, and one archive at a time

| | Do | Must happen | Holds |
| --- | --- | --- | --- |
| 7.1 ‡ | Open an archive, press *Close* on the breadcrumb row | The archive closes. The window stays. | §1, §4 |
| 7.2 ‡ | Stage changes, then Close | The status line reports it — *"Closed photos.zip · 4 staged changes discarded."* — with no popup. | §4 |
| 7.3 ‡ | With an archive open, press `O` and name another | The first closes, the second takes the same window, in the same words. | §1, §4.8 |
| 7.4 | From a file manager, open a second archive while one is open | A **second window**. That rule is what the file-manager route was always for. | §1 |
| 7.5 | Close the last archive, then look at the sidebar | *File* is still enterable and the table offers the way back in. | §4 |

## Round 8 — recents, bookmarks, settings

| | Do | Must happen | Holds |
| --- | --- | --- | --- |
| 8.1 † | Open several archives, press `3` | They are listed, most recent first. | §4 |
| 8.2 † | Click a recent **once** | It opens. One click, every time, with no volatility. *(P11 round 7 step 2 — this was the worst finding of that round.)* | §4 |
| 8.3 † | Press `Enter` on a recent | It opens. | §4 |
| 8.4 † | Delete one of them on disk, look again | That row is dimmed. `Del` takes it off the list. | §4 |
| 8.5 † | With one dimmed, click the others | They still open on one click. *(P11: after a row dimmed, the rest stopped responding.)* | §4 |
| 8.6 | Bookmarks are **made** in `,` → *Bookmarks*: type a name, type a directory in the `/path/to/directory` field, press *Add*, three times. Section `4` shows them and has never been where they are added. Then press `4` and click each of the three in turn | **All three** light and stay lit — including the third. *(P11: the third never highlighted.)* † | §4 |
| 8.7 | `,` → *Recent files* → *Clear list* | The history empties. It is the only destructive control in the panel, and the count is written beside it. | §4.5 |
| 8.8 | `,` → *Extract* → click **into a subdirectory**. Open an archive and press `E`; then come back, click **here**, and press `E` again. *(Wording corrected in PXX: this used to say "set Preselect to…", from when *Preselect* was the row's label. It is a third control now — 8.11 is where it gets exercised, and this step is only about the other two.)* | The popover's path field opens on the archive's own folder plus the archive's name the first time, and on the folder alone the second. The setting chooses which of the two the field is **prefilled** with; both buttons are always there. | §4.5, §4.3 |
| 8.9 | Restart INDIUM | Recents, bookmarks and the extract default are all still there. | §4.5 |
| 8.10 † | Put deliberate garbage in `settings.toml` and launch | It starts on defaults, **says so**, and leaves a `.broken` copy of what it could not read. *(P11: it silently accepted a nonexistent path.)* | §4.5 |
| 8.11 | Open `,` and work all **three** controls in the *Extract* group. Click *here*, click *into a subdirectory*, then click **Preselect** and choose a directory in the picker it raises. Press `E` over an open archive after each to see what the path field was prefilled with. Then reopen `,` once more. | Three controls, each taking and holding the click. *here* and *into a subdirectory* prefill 8.8's field as they always did. **Preselect raises the desktop's folder picker**, and the directory you choose is shown under the row and used by `E` for **every** archive, not just the one open — that is what makes it persistent. Cancelling the picker changes nothing, including which mode is lit. Clicking *here* or *into a subdirectory* afterwards deselects Preselect but **keeps the path on screen**, so returning to it does not mean naming the directory twice. The path survives closing and reopening INDIUM. **This step changed in PXX and the reason is recorded rather than assumed:** *Preselect* was the row's **label**, and the maker read it as a button — a word set beside two pressable words, in the same row and at the same size, is a third pressable word whatever it was meant to be. It is a button now. | §4.5 |
| 8.12 | Press `E`, and type a destination that does not exist — two levels of it, so nothing could have made it by accident: `~/indium-test/nowhere/deeper`. Extract | **The folders are created and the files land inside them.** That is the intended answer rather than an oversight: an extractor that refuses to make its own destination sends you away to make it by hand, and this is the same `mkdir -p` behaviour `indium extract --to` has always had on the terminal. What must not happen is silence or a lie — if it truly cannot create the directory it says *"Could not create …"* and names the one it could not make. | §4.3 |

## Round 9 — encryption

| | Do | Must happen | Holds |
| --- | --- | --- | --- |
| 9.1 | Fill the draft, `N`, pick *Encrypted* | The method list moves to 7z/LZMA2 — the only road to AES-256. | §5 |
| 9.2 | Read the foot sentence | It names AES-256. | §4.1 |
| 9.3 † | Give a password and a different confirmation | Refused **before anything is built**. | §4.7 |
| 9.4 † | Try to type in the confirm box | It takes the typing. *(P11: focus jumped back to the first box.)* | §4.7 |
| 9.5 † | Build it, then close INDIUM and reopen the new archive | It asks for the password **at open**, before listing anything — `sevenz.rs:294` encrypts the header too, so the names are ciphertext — and then the files are all there. *(P11 could not run this: adding files was broken, so the archive was never built.)* | §4.7, §5 |
| 9.6 | Open it a second time in the same session | It asks **again**. Per use, always. | §4.7, §9 |
| 9.7 | `ps aux` while the password popup is up, and check shell history | The password appears in neither. | §9 |
| 9.8 | Try to encrypt with a non-7z method | Not offered. Encryption is 7z AES-256 and nothing else. | §5 |

## Round 10 — the terminal half

| | Do | Must happen | Holds |
| --- | --- | --- | --- |
| 10.1 | `indium list photos.zip` | One stored path per line, archive order, undecorated. | §4 |
| 10.2 | `indium list photos.zip --long` | Mode, size, packed, method, encryption, time, path, and a total. | §4 |
| 10.3 | `indium list photos.zip -0` | NUL-separated. Pipe it to `xargs -0` and the names survive. | §4 |
| 10.4 | `indium list photos.zip --long -0` | **Refused.** One is for a person, one for a script; a flag silently ignored is worse than one refused. | §4 |
| 10.5 | **Three commands, in this order. Copy each one exactly** — the `$(…)` and the `\|` are part of what is being tested, and a command typed differently is testing something else. **(a)** `indium list large.tar` · **(b)** `indium extract large.tar --to /tmp/rt $(indium list large.tar)` · **(c)** `indium list photos.zip -0 \| xargs -0 indium extract photos.zip --to /tmp/rt2 --` | **What this step is for, in one sentence:** `list`'s output must feed straight back into `extract` without being edited by hand — that is the whole claim, and each command checks one part of it. **(a)** exactly **three lines** and nothing else. Three is the whole of `large.tar`, not a truncation of it. **(b)** **`Extracted 3 entries.`** — the three names from (a), handed back unedited. **(c)** **`Extracted 45 entries.`**, with every awkward name surviving the pipe: `beach day.jpg` with its space, `köpek.txt`, and `--weird-name`, which is why the trailing `--` is there. **Approve if all three print what is written here.** | §4 |
| 10.6 | `indium extract photos.zip --to /tmp/x` | Everything, under that directory. | §4 |
| 10.7 | `indium extract photos.zip -- --weird-name` | `--` ends the flags; a member named like a flag is extracted. | §4 |
| 10.8 | `indium cat photos.zip README.txt \| wc -c` — **type both names exactly as written.** The archive is `photos.zip` and the member is `README.txt`, which really is inside it; an earlier round of this plan left the member as a placeholder and the walker had to guess one, which is the mistake this wording exists to prevent. Another archive, or a path that is not a member, tests nothing here. | **`153`.** The member's bytes, whole. Then compare against the copy 10.5 already extracted: `indium cat photos.zip README.txt \| cmp - /tmp/rt2/README.txt` says **nothing at all**, which is `cmp` agreeing. **If instead you see `indium: no such entry: …`, the command was typed against the wrong archive or the wrong member — that message is INDIUM being right, so fix the command rather than denying the step.** | §4 |
| 10.9 | `indium cat secret.7z f.txt` — **type it exactly.** `f.txt` is the archive's **one and only** member, and the password is **`indium`**, typed at the prompt when it asks. | Asks for the password **on the terminal**, once, per use — the prompt reads `Password for secret.7z:` and does not echo what you type. Then the member's bytes. **Two ways to get a wrong answer from a right program, both worth knowing before you tick:** naming a member that is not `f.txt` gives an error about the archive rather than about the member, and giving a password that is not `indium` gives one too. Neither is this step. **This step is the happy path**; the wrong password is step 10.14's job, deliberately. | §4, §9 |
| 10.10 | `indium --password=x …` and `INDIUM_PASSWORD=x indium …` | Neither exists. There is no flag and no environment variable. | §9 |
| 10.11 | `indium ./list` where a file named `list` exists | Opened as an archive. The terminal half is entered only when the first argument is exactly `list`, `extract` or `cat`. | §4 |
| 10.12 | `indium list notrar.rar` | *"RAR is not supported."* on the terminal too. | §5 |
| 10.13 | `indium a.zip b.zip` | One window on each. | §1 |
| 10.14 | `indium list secret.7z`, and give it a password that is **deliberately wrong** — anything that is not `indium` | **`indium: Wrong password.`** and nothing listed. One sentence, in words, naming the thing you can actually fix. **This step exists because 10.9's fix would otherwise ship unwalked:** with the password finally written down, 10.9 now passes on the first try and never reaches the failure path again. The first walk got `Broken or unsupported archive: no Header` wrapped in `Other("…")` — a crate's own enum printed at a person — because with encrypted headers a wrong key does not fail cleanly: AES has nothing to check it against, hands the parser noise, and the parser breaks in whatever way that noise happens to break it. | §4, §9 |

## Round 11 — the eight methods

*One archive built with each, from the same draft, then reopened and read back.*

| | Do | Must happen | Holds |
| --- | --- | --- | --- |
| 11.1 | Build with **Store** | *"No compression — instant, and as large as the input."* Result is not smaller than the input. | §5 |
| 11.2 | **lz4** | *"The fastest real compression there is, and the largest result."* | §5 |
| 11.3 | **gzip** | *"Fast, everywhere, and beaten in both speed and size by zstd."* | §5 |
| 11.4 | **zstd** | *"Very fast with a small archive — the sane default."* | §5 |
| 11.5 | **bzip2** | *"Slower than gzip for a somewhat smaller file; kept for compatibility."* | §5 |
| 11.6 | **xz** | *"Among the smallest archives, built slowly; extraction is quick enough."* | §5 |
| 11.7 | **7z / LZMA2** | *"Smallest for mixed content, slow to build — and the only road to AES-256."* | §5 |
| 11.8 | **zip / Deflate** | *"Not the smallest or fastest, but opens absolutely anywhere."* | §5 |
| 11.9 | **The archives you built in round 11 live in `~/indium-test/realfile/`** — the step never said so before, which is why it kept coming back as *"you check for me"*. One command reads all of them: `cd ~/indium-test/realfile && for f in *; do printf '%-28s ' "$f"; bsdtar -tf "$f" >/dev/null 2>&1 && echo VALID \|\| echo REFUSED; done` | Every one is a valid archive of its declared format — checked, eight of eight. **If you also built an encrypted one, it is a ninth and it is expected to refuse:** `bsdtar` answers *"The archive header is encrypted"*, which is step 3.9's documented behaviour seen from the outside. The names are ciphertext; a reader without the password has nothing it could list. That refusal is a pass, not a failure. | §5 |
| 11.10 | Compare each verdict against the popup | The sentence in the window is the sentence in §5, word for word. | §5 |

## Round 12 — scale, on a 3450U

*The GB corpus. R6: "If it can handle, modern processors can drink their cocktails."*

| | Do | Must happen | Holds |
| --- | --- | --- | --- |
| 12.1 | Open `many-entries.tar` (150k+ entries) | Lists without stalling. Scrolling stays smooth; `Ctrl+A` completes. | §4 |
| 12.2 | Filter it | The filter narrows 150k rows without freezing the window. | §4 |
| 12.3 | Open `big-mixed.tar.zst` (~3 GB) and Measure | The eight candidates run over real bytes and finish. Figures above the budget are marked. | §4.10 |
| 12.4 | Apply a rename on `big-mixed.tar.zst` | The rebuild completes; the original is untouched until the rename commits. | §4.2 |
| 12.5 | Cancel that Apply partway | Original unchanged, no orphan left beside it. | §4.2 |
| 12.6 | Open `under-limit.tar`, press **`Ctrl+A`** to select all three members — the whole ~900 MB, not one of them — then `Ctrl+C` | **The known suspect** — under the 1 GiB `RAM_LIMIT` it routes to `$XDG_RUNTIME_DIR`, a 712 MB tmpfs, so the copy is expected to run out of room. **Running out of room is not the failure; how it ends is.** Approve if INDIUM says in a sentence that it could not write, leaves nothing half-written behind, and the window stays usable. **Deny** if it hangs, dies without a word, reports success, or leaves a partial file it does not mention. Note that swap cannot save this one: a tmpfs is capped by its `size=` option, not by memory. | §3 |
| 12.7 | Open **`over-limit.tar`** — check the name, because `under-limit.tar` sits beside it in the same directory and is a different test. `ls -lh ~/indium-test/*-limit.tar` tells them apart: **`over-limit.tar` is 1.6 G**, `under-limit.tar` is 901 M, and only the 1.6 G one crosses the limit this step is named for. Then press **`Ctrl+A`** for both members and `Ctrl+C`. | Over the limit it routes to the cache directory instead of the runtime tmpfs, and completes. | §3 |
| 12.8 | **Build `bigsecret.7z` here**, through `N` → *Encrypted*, from the generator's `bigsecret-input.bin` (**8 GiB** of low-entropy filler — deliberately larger than this machine's 7.0 GiB of RAM, so the member provably cannot be held in memory). There is no `7z` binary on this machine, so INDIUM writing it *is* the test. | It completes without the window going unresponsive, and the finished `.7z` is a tiny fraction of 8 GiB — LZMA2 over low-entropy filler. **That size gap is the point**, not an accident: it is what makes the next step a decompression bomb rather than a big file. | §5, §4.7 |
| 12.9 | Open `bigsecret.7z` (it prompts at open — encrypted headers) and extract its large member, running `grep -E 'VmPeak\|VmHWM' /proc/$(pgrep -x indium)/status` in another terminal while it works | `arch.rs:1038` buffers a whole 7z member with an uncapped `usize::MAX` read, so this is a memory measurement, not a survival test. **Write both numbers down.** `VmPeak` is the one that means something: under 99 GiB of swap `VmHWM` caps near physical RAM and measures this box rather than the program. Approve if it completes and the figures are recorded; **Deny** if the session becomes unusable or the OOM killer takes it. | §3 |
| 12.10 | Run the window through 12.1–12.9 watching the status line | It stays readable and says what is happening throughout. | §6 |
| 12.11 | Start the 12.8 build over — `N` → *Encrypted*, `bigsecret-input.bin` — and this time press **Cancel** partway | The progress line **moves and keeps naming where it is** while it runs, Cancel actually stops it, and afterwards `ls -a ~/indium-test` shows **nothing left beside the target**: no `.<name>.7z.indium-new`, no half-written archive. **This is the step the first walk had no way to run.** The encrypted-create path reported nothing at all and its Cancel was never observed, so the only way out of it was to kill the window — which is exactly how 159 MB of `.archiveadfadsf.7z.indium-new` came to be sitting in the corpus, left behind by a process that had taken its own clean exit path mid-write. | §4.2, §3 |

## Round 13 — the window, at three scales

| | Do | Must happen | Holds |
| --- | --- | --- | --- |
| 13.1 ‡ | Set the desktop to **100%** and walk rounds 2 and 6 | Nothing clips, nothing overlaps, every control reachable. | §6 |
| 13.2 ‡ | **125%** | The same. | §6 |
| 13.3 ‡ | **150%** | The same. | §6 |
| 13.4 | At 150%, open every one of the ten popups | Each fits on a 1080p 15" screen without being cut off. | §6 |

## Round 14 — leaving no trace

| | Do | Must happen | Holds |
| --- | --- | --- | --- |
| 14.1 † | Close everything, then `ps aux \| grep indium` | Nothing left over — no `<defunct>`. | §3 |
| 14.2 | After a copy-out, look in `$XDG_RUNTIME_DIR` | Scratch directories are cleaned up, not accumulating. | §3 |
| 14.3 † | Copy files out in one window, then open and close a second window | The copied files are still on disk and still complete. *(P11: this step was not understood — it is here reworded.)* | §3 |
| 14.4 | Kill INDIUM mid-Apply with `kill -9` | The original archive is intact. Whatever is left over is identifiable and does not masquerade as the archive. | §3 |
| 14.5 | Relaunch after that kill | It starts clean and says nothing false about the archive. | §3 |
| 14.6 | **Write down what the package owns first** — `pacman -Ql indium > /tmp/owned.txt` — then `sudo pacman -R indium`, then check that every path in that file is gone | All 42 of them are gone, and nothing of yours is. **What must not be counted against it:** `~/.local/share/applications/org.indium.desktop` and the ten icons under `~/.local/share/icons/hicolor` are still on the disk afterwards, and that is correct — **step 1.12 put them there**, the package never owned them, and a `pacman -R` that deleted files belonging to no package would be the defect. Step 14.7 is what takes those back. **Run this step last:** it uninstalls, and everything after it would have nothing left to run against. | §8 |
| 14.7 | `~/INDIUM/build/install-desktop.sh --uninstall`, then look again in `~/.local/share/applications` and `~/.local/share/icons/hicolor` | The desktop entry and all ten user-scope icons go, and both caches are refreshed, so the menu stops offering an entry whose files are not there. **One thing it deliberately leaves alone:** if you ran 1.13's `--set-default`, that association stays. `xdg-mime` has no inverse, so taking it back would mean this script editing your `mimeapps.list` — your file, holding every other association you have ever made — and a default naming an entry that no longer exists is inert anyway; the desktop falls through to the next handler. | §8 |

---

## The verdict

The round is a pass when every step above is ticked with none outstanding. §7's beta condition
asks for *"a testing round against a released build"* and this is the round it meant. What the
round finds is fixed and re-verified before anything else in `PXX` proceeds; what it cannot
resolve is written down rather than quietly dropped.

**What "real hands" means is deliberately left undefined in §7**, and this document does not
decide it in general. It records what was done, once, by the person whose program it is.
