# Test fixtures

Small archives committed for `cargo test`. Required by P1 §4 and extended by P2 §6.

INDIUM itself never shells out. These *fixtures* may be born however convenient — they
were made on the build machine with `bsdtar` (libarchive) and `python3`. Every command
that produced a file in this directory is recorded below, verbatim and re-runnable.

**Password for every encrypted fixture: `indium`**

Build machine at time of creation:

```
bsdtar 3.8.9 - libarchive 3.8.9 zlib/1.3.2 liblzma/5.8.3 bz2lib/1.0.8 liblz4/1.10.0
              libzstd/1.5.7 openssl/3.6.3 libb2/bundled libacl/2.4.0
Python 3.14.6
pyzipper 0.4.0, py7zr 1.1.3   (installed into a throwaway venv OUTSIDE the repo)
```

---

## The shared payload

`basic.zip`, `basic.tar.gz`, `basic.tar.zst` and `basic.7z` all carry the **same three
files plus one subdirectory**, with these exact contents (note the trailing newline on
each):

| path | bytes | content | CRC32 (IEEE) |
| --- | --- | --- | --- |
| `alpha.txt` | 21 | `INDIUM fixture alpha\n` | `0xf28ec54d` |
| `beta.txt` | 20 | `INDIUM fixture beta\n` | `0xd5aced60` |
| `sub/` | 0 | *(directory)* | — |
| `sub/gamma.txt` | 21 | `INDIUM fixture gamma\n` | `0x78ffaf48` |

Created in a staging directory outside the repo:

```sh
mkdir -p /tmp/payload/sub
printf 'INDIUM fixture alpha\n' > /tmp/payload/alpha.txt
printf 'INDIUM fixture beta\n'  > /tmp/payload/beta.txt
printf 'INDIUM fixture gamma\n' > /tmp/payload/sub/gamma.txt

# one fixed mtime for all of it, so the archives are reproducible
cd /tmp/payload
TZ=UTC touch -d '2024-01-02 03:04:05 UTC' alpha.txt beta.txt sub/gamma.txt sub
```

`2024-01-02 03:04:05 UTC` is unix **1704164645**.

## `basic.zip`, `basic.tar.gz`, `basic.tar.zst`, `basic.7z`

```sh
cd /tmp/payload
COMMON="--uid 0 --gid 0 --uname root --gname root"

bsdtar -c --format zip            $COMMON -f basic.zip     alpha.txt beta.txt sub
bsdtar -c --format gnutar --gzip  $COMMON -f basic.tar.gz  alpha.txt beta.txt sub
bsdtar -c --format gnutar --zstd  $COMMON -f basic.tar.zst alpha.txt beta.txt sub
bsdtar -c --format 7zip           $COMMON -f basic.7z      alpha.txt beta.txt sub
```

Two things to know before you write assertions:

- Only `alpha.txt beta.txt sub` are named. bsdtar recurses into `sub` on its own — also
  naming `sub/gamma.txt` gets you the entry **twice**, and a 5-entry archive.
- `--uid 0 --gid 0 --uname root --gname root` normalises ownership so the fixtures do
  not encode whoever built them. 7z and zip do not carry uname/gname; tar does.

## `rooted.tar` — the same payload, stored the ordinary way

```sh
bsdtar -c --format gnutar --uid 0 --gid 0 --uname root --gname root \
       -f rooted.tar -C /tmp/payload .
```

Same four members as every `basic.*`, but named as `.` rather than one by one, which is
how `tar -cf x.tar -C dir .` — the commonest tar invocation there is — stores them. That
puts a **`./` entry first**:

```
./            ./beta.txt    ./alpha.txt    ./sub/    ./sub/gamma.txt
```

That leading `./` is the whole reason the fixture exists. `normalize_archive_path("./")`
is the empty string, and so is the normalised path of an entry whose name could not be
read at all — so until PXX, INDIUM grew a nameless row for it in the listing and
`extract` refused **the entire archive**, reporting that a name "could not be read on
this system". The name was `./`.

Not one committed fixture was rooted this way, which is how that reached a release. Two
tests hold it now: `a_dot_slash_rooted_tar_lists_and_extracts_like_any_other` in
`tests/read_path.rs`, and `arch::tests::the_archive_root_is_told_apart_from_a_name_that_could_not_be_read`,
which pins the other direction so the fix cannot decay into ignoring both.

**Do not add `sub/gamma.txt` to the command.** `.` already recurses; naming it too stores
it twice, exactly as the `basic.*` note above warns.

## `meta.tar`

A symlink, a hardlink, explicit non-zero uid/gid with names, and old mtimes. GNU tar is
not installed on the build machine; python3's `tarfile` gives full control over every
`TarInfo` field, so it was used instead of `bsdtar`.

```python
#!/usr/bin/env python3
"""Build tests/fixtures/meta.tar: symlink, hardlink, explicit uid/gid, old mtimes."""
import io, tarfile, calendar

OUT = "/home/megas/INDIUM/tests/fixtures/meta.tar"

UID, GID = 1234, 5678
UNAME, GNAME = "indiumuser", "indiumgroup"
MTIME_2001 = calendar.timegm((2001, 2, 3, 0, 0, 0))   # 2001-02-03T00:00:00Z
MTIME_1999 = calendar.timegm((1999, 12, 31, 0, 0, 0)) # 1999-12-31T00:00:00Z

REGULAR = b"INDIUM meta fixture regular\n"
ANCIENT = b"INDIUM meta fixture ancient file\n"

def base(name, ttype, mtime, mode):
    ti = tarfile.TarInfo(name)
    ti.type = ttype
    ti.mode = mode
    ti.uid, ti.gid = UID, GID
    ti.uname, ti.gname = UNAME, GNAME
    ti.mtime = mtime
    return ti

with tarfile.open(OUT, "w", format=tarfile.GNU_FORMAT) as tf:
    # 1. regular file
    ti = base("regular.txt", tarfile.REGTYPE, MTIME_2001, 0o644)
    ti.size = len(REGULAR)
    tf.addfile(ti, io.BytesIO(REGULAR))

    # 2. symlink -> regular.txt
    ti = base("symlink.txt", tarfile.SYMTYPE, MTIME_2001, 0o777)
    ti.linkname = "regular.txt"
    ti.size = 0
    tf.addfile(ti)

    # 3. hardlink -> regular.txt
    ti = base("hardlink.txt", tarfile.LNKTYPE, MTIME_2001, 0o644)
    ti.linkname = "regular.txt"
    ti.size = 0
    tf.addfile(ti)

    # 4. directory with an older mtime
    ti = base("oldstuff/", tarfile.DIRTYPE, MTIME_1999, 0o755)
    ti.size = 0
    tf.addfile(ti)

    # 5. regular file inside it, same old mtime
    ti = base("oldstuff/ancient.txt", tarfile.REGTYPE, MTIME_1999, 0o644)
    ti.size = len(ANCIENT)
    tf.addfile(ti, io.BytesIO(ANCIENT))
```

`MTIME_2001` is unix **981158400**, `MTIME_1999` is unix **946598400**.

## `evil.zip`

Exactly one entry, whose stored name is literally `../escape.txt`. Extraction must fail
and nothing may appear outside the destination.

```python
#!/usr/bin/env python3
"""Build tests/fixtures/evil.zip: exactly one entry stored as '../escape.txt'."""
import zipfile
OUT = "/home/megas/INDIUM/tests/fixtures/evil.zip"
PAYLOAD = b"INDIUM path traversal canary\n"
with zipfile.ZipFile(OUT, "w", zipfile.ZIP_DEFLATED) as z:
    zi = zipfile.ZipInfo("../escape.txt", date_time=(2024, 1, 2, 3, 4, 4))
    zi.external_attr = 0o644 << 16
    z.writestr(zi, PAYLOAD)
```

## `secret.zip` — password `indium`

One AES-256 encrypted entry. Python's stdlib `zipfile` cannot *write* AES, so a
throwaway venv outside the repo supplied `pyzipper`:

```sh
python3 -m venv /tmp/venv            # anywhere OUTSIDE the repo — never commit a venv
/tmp/venv/bin/python -m ensurepip --upgrade
/tmp/venv/bin/pip install pyzipper py7zr
```

```python
#!/usr/bin/env python3
"""Build tests/fixtures/secret.zip: one AES-256 encrypted entry, password 'indium'."""
import pyzipper
from pyzipper.zipfile_aes import AESZipInfo

OUT = "/home/megas/INDIUM/tests/fixtures/secret.zip"
PAYLOAD = b"INDIUM secret payload\n"

with pyzipper.AESZipFile(OUT, "w",
                         compression=pyzipper.ZIP_DEFLATED,
                         encryption=pyzipper.WZ_AES) as z:
    z.setpassword(b"indium")
    z.setencryption(pyzipper.WZ_AES, nbits=256)
    zi = AESZipInfo("secret.txt", date_time=(2024, 1, 2, 3, 4, 4))
    zi.external_attr = 0o644 << 16
    zi.compress_type = pyzipper.ZIP_DEFLATED
    z.writestr(zi, PAYLOAD)
```

`pyzipper.ZipInfo` is the *stdlib* class and is rejected by `AESZipFile.writestr` —
`pyzipper.zipfile_aes.AESZipInfo` is the one that works.

Zip AES never encrypts filenames, so **listing works with no password** (P1) and only
the data needs the prompt (P2).

## `secret-headers.7z` — password `indium`

7z with **encrypted headers** (`-mhe=on` equivalent): the filenames themselves are
encrypted. The `7z` binary is not installed on the build machine and libarchive cannot
*write* encrypted 7z, so `py7zr` from the same venv was used:

```python
#!/usr/bin/env python3
"""Build tests/fixtures/secret-headers.7z: encrypted filenames, password 'indium'."""
import io, py7zr
OUT = "/home/megas/INDIUM/tests/fixtures/secret-headers.7z"
PAYLOAD = b"INDIUM header-encrypted payload\n"
with py7zr.SevenZipFile(OUT, "w", password="indium", header_encryption=True) as z:
    z.writef(io.BytesIO(PAYLOAD), "f.txt")
```

> **libarchive cannot read this fixture, with or without the passphrase.**
> libarchive 3.8.9 answers every `archive_read_next_header` with `ARCHIVE_FATAL` and
> the message *"The archive header is encrypted, but currently not supported"*, even
> after `archive_read_add_passphrase`. `archive_read_has_encrypted_entries` does
> return `1`, so the *detection* half of P2 §6 is reachable; the "with the passphrase,
> lists and extracts" half is not, for as long as `arch` is pure libarchive. The
> fixture itself is sound — `py7zr` lists and extracts it correctly with `indium`.

## `notrar.rar`

Not a real RAR archive — just the 8-byte **RAR5 signature**, enough for libarchive's
format detector to report `RAR5` so INDIUM's RAR gate (CORE §5) can refuse it.

```sh
python3 -c "open('/home/megas/INDIUM/tests/fixtures/notrar.rar','wb').write(
    bytes([0x52,0x61,0x72,0x21,0x1A,0x07,0x01,0x00]))"
```

- RAR5 signature: `52 61 72 21 1A 07 01 00` — used here.
- RAR4 signature: `52 61 72 21 1A 07 00` (7 bytes) — for reference; libarchive reports
  it as format name `RAR`, and `archive_read_next_header` returns `ARCHIVE_FATAL`
  ("Failed to read next header").

> **The gate must not wait for a successful `next_header`.** With this fixture the
> first `archive_read_next_header` returns `ARCHIVE_EOF`, never `ARCHIVE_OK`, and
> `archive_format`/`archive_format_name` are only populated *after* that call.
> P1 §2's wording — *"after the first successful `next_header`, check
> `archive_format`"* — would therefore never fire here and the archive would look
> merely empty. Check the format name after the first `next_header` call **regardless
> of its return code** (`ARCHIVE_OK`, `ARCHIVE_EOF`, or `ARCHIVE_FATAL`); the name
> contains `RAR` in all three cases.

## `utf8.zip`

Names outside ASCII, in three scripts, plus one directory whose own name is outside ASCII
so that a selection has to match a non-ASCII prefix. Every entry carries zip's UTF-8 flag
(bit 11), which `zipfile` sets on its own for any name it cannot encode as CP437.

```python
#!/usr/bin/env python3
"""Build tests/fixtures/utf8.zip: names outside ASCII, in three scripts."""
import zipfile
OUT = "/home/megas/INDIUM/tests/fixtures/utf8.zip"
FILES = [
    ("köpek.txt",        b"INDIUM utf8 kopek\n"),
    ("日本語.txt",        b"INDIUM utf8 nihongo\n"),
    ("Ünlü/naïve.txt",   b"INDIUM utf8 naive\n"),
]
with zipfile.ZipFile(OUT, "w", zipfile.ZIP_DEFLATED) as z:
    zi = zipfile.ZipInfo("Ünlü/", date_time=(2024, 1, 2, 3, 4, 4))
    zi.external_attr = (0o755 << 16) | 0x10
    z.writestr(zi, b"")
    for name, payload in FILES:
        zi = zipfile.ZipInfo(name, date_time=(2024, 1, 2, 3, 4, 4))
        zi.external_attr = 0o644 << 16
        zi.compress_type = zipfile.ZIP_DEFLATED
        z.writestr(zi, payload)
```

`list_all` reports the four entries in this order: `Ünlü`, `köpek.txt`, `日本語.txt`,
`Ünlü/naïve.txt`.

> **This fixture exists because every other one is pure ASCII, and that is what let P11's
> worst defect survive seven milestones of green tests.** libarchive converts a stored name
> into the *current locale's* charset while reading the header; a Rust program never calls
> `setlocale`, so INDIUM ran in the `C` locale, the conversion failed for every byte outside
> ASCII, and `archive_entry_pathname` returned NULL. The entry listed with an empty name,
> matched no selection, and extraction skipped it in the same breath it skips a file nobody
> asked for. `arch::ensure_ctype_locale` is the fix; these are the tests that hold it down.
> Disable that one call and all three fail — which is how they were checked.

---

## Notes for whoever writes the assertions

- **Do not assert exact mtimes on `evil.zip` or `secret.zip`.** Python's `zipfile` and
  `pyzipper` write only DOS local time, so libarchive reconstructs the unix mtime using
  the *reader's* timezone: the same file reads back as `1704164644` under `TZ=UTC`,
  `1704182644` under `America/New_York`, and `1704132244` under `Asia/Tokyo`.
  `basic.zip` is safe — bsdtar adds an extended-timestamp extra field, so it reads
  `1704164645` in every timezone. All tar fixtures store unix time directly and are
  stable everywhere.
- **`basic.7z` lists `sub/` last.** libarchive's 7z reader emits directories after
  files, so the order is `alpha.txt`, `beta.txt`, `sub/gamma.txt`, `sub/`. The other
  three `basic.*` list `sub/` third. Assert on sets, or on per-format order.
- **The hardlink entry has filetype `0`.** For `hardlink.txt` in `meta.tar`,
  `archive_entry_filetype` returns `0`, not `AE_IFREG`. Detect hardlinks with
  `archive_entry_hardlink() != NULL`, and make sure `is_dir` logic does not mistake a
  filetype of `0` for anything.
- **`secret.zip`'s stored CRC is `0`.** WinZip AE-2 omits the CRC from the header. The
  CRC32 of the decrypted content is `0x29c22f32`.
- **`archive_read_has_encrypted_entries` returns `-2`** (`ARCHIVE_READ_FORMAT_ENCRYPTION_UNSUPPORTED`)
  for the tar fixtures — it is not a boolean, so compare against `1` explicitly.
- `meta.tar` is 10240 bytes because tar pads to its 20-block default. Only 5 headers
  are real; the rest is the zero padding.

---

## P3 desktop fixtures

`desktop/` holds the committed `.desktop` inputs P3 §5 asks for — *".desktop parsing
against committed fixture files in `tests/fixtures/desktop/`"* — plus a `mimeapps.list`
for the ranking test. All seven `.desktop` files pass `desktop-file-validate`, and each
carries `[Desktop Entry]`, `Type=Application`, `Name=` and `Exec=`.

Every fixture's `Exec` names a binary that really is on `$PATH` (`cat`, `sh`, `env`), so
the only thing differing between them is the field under test. That is deliberate: GLib's
own loader *also* rejects an entry whose `Exec` binary is missing — behaviour P3 §3 does
not ask for — and pinning `Exec` to a real binary keeps that from masking the actual
discriminator.

| File | Expected | Why |
| --- | --- | --- |
| `normal.desktop` | **kept** | Ordinary entry; `Terminal=false`, `MimeType` includes `image/png`. The baseline every other row is a delta from. |
| `nodisplay.desktop` | **skipped** | `NoDisplay=true`, which P3 §3 honours. |
| `hidden.desktop` | **skipped** | `Hidden=true` — the spec calls this "strictly equivalent to the .desktop file not existing at all". |
| `tryexec-missing.desktop` | **skipped** | `TryExec=/nonexistent/binary/definitely-not-here` is not on `$PATH`, which P3 §3 says disqualifies. Its `Exec` is valid, so `TryExec` is the sole cause. |
| `tryexec-present.desktop` | **kept** | `TryExec=sh` resolves on `$PATH`, so the `TryExec` check must not disqualify it. |
| `terminal.desktop` | **kept** | `Terminal=true`. P3 §3: "Terminal=true entries are listed" — kept, with no special launch handling. |
| `quoting.desktop` | **kept** | Pathological wine-style `Exec`: partial quoting, a quoted argument with spaces, escaped quotes and literal backslashes inside quotes, and `%i`/`%c`/`%k` to strip. Feeds the tokenizer test, not the keep/skip test. |
| `mimeapps.list` | — | `[Default Applications]` maps `image/png` to `normal.desktop` first, so the ranking test can assert the default is first and tagged. |

`quoting.desktop`'s `Exec` unescapes in two passes — the general string-value rules
(`\\` → `\`) first, then the `Exec` quoting rules — and with `%f` = `/tmp/scratch/file.png`
and `%i`/`%c`/`%k` stripped it must tokenize to exactly these five arguments:

```
env
WINEPREFIX=/home/megas/.wine
wine
C:\Program Files\Acme "Deluxe" Viewer\view.exe
/tmp/scratch/file.png
```

Verified against GLib (`g_key_file_get_string` then `g_shell_parse_argv`). One caveat for
whoever writes the tokenizer: `WINEPREFIX="/home/megas/.wine"` opens its quote *mid*
argument, and the spec only says arguments "may be quoted in whole" — it does not define
partial quoting. Every real implementation reads it shell-style and drops the quotes, as
above; a tokenizer recognising a quote only at argument start would instead yield
`WINEPREFIX="/home/megas/.wine"`, quotes intact. The fixture is written so that this is
the single token on which the two readings differ.

`mimeapps.list` also carries `[Added Associations]` and `[Removed Associations]` for
realism. P3 §3 consumes only `[Default Applications]`; nothing here requires the other
two to be read.
