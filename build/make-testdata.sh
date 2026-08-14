#!/usr/bin/env bash
#
# make-testdata.sh — build the corpus that `build/docs/TESTPLAN.md` is written against:
# the GB-scale fixtures rounds 11-13 need, and the two small ones rounds 1, 3, 4 and 10 name.
#
# The corpus itself is deliberately NOT in the repository. It is gigabytes, it is
# regenerable, and P6 §9 was written after two release tarballs came within one `git add -A`
# of entering history forever. This script is the part that gets checked in, so the corpus
# is reproducible without any of it being tracked.
#
# Everything here is byte-for-byte deterministic: the pseudorandom filler is an AES-CTR
# keystream under a fixed pass phrase, not /dev/urandom, so two runs on two machines produce
# identical fixtures and a measurement taken here can be compared with one taken elsewhere.
#
# What it does NOT build:
#
#   bigsecret.7z   There is no 7z binary and no py7zr on the maker's machine, and INDIUM
#                  writes encrypted 7z itself (`sevenz.rs:294` encrypts the header too).
#                  So the archive is built at TESTPLAN 12.8, through the Create popup's
#                  Encrypted preset, from the `bigsecret-input.bin` this script does write.
#                  Building it is a test of the write path rather than setup for one.
#
#   the rest of the P11-round fixtures — large.tar, backup.7z, secret.7z, notrar.rar,
#                  notanarchive.zip, a.zip, b.zip, to-add/ — which are still in the corpus
#                  directory and are left alone. `secret.7z` in particular is byte-identical
#                  to `tests/fixtures/secret-headers.7z`, so the repository already pins it
#                  and its pass phrase; there is nothing here to reproduce.
#
# Two of that round's fixtures are built here, and the reason is worth writing down. This
# script used to say photos.zip and docs.tar.gz "already exist in the corpus directory and
# are left alone". By the end of PXX's certification walk they did not: photos.zip had been
# extracted into ~/indium-test/photos/ and the archive itself was gone. Nine steps name it —
# 1.7, 3.1, and seven of round 10's thirteen — and R10 re-walks round 10 in full, so a
# missing fixture is not a nuisance but a stop. A corpus the plan calls regenerable has to
# actually be regenerable, or the claim freezes into the repository as a false one.
#
# Usage:
#   build/make-testdata.sh                  # into ~/indium-test
#   build/make-testdata.sh --dir DIR        # somewhere else
#   build/make-testdata.sh --force          # rebuild fixtures that already exist
#   build/make-testdata.sh --list           # say what would be built, build nothing

set -euo pipefail

DIR="${HOME}/indium-test"
FORCE=0
LIST=0

while [ $# -gt 0 ]; do
	case "$1" in
	--dir)
		DIR="${2:?--dir needs a path}"
		shift 2
		;;
	--force)
		FORCE=1
		shift
		;;
	--list)
		LIST=1
		shift
		;;
	-h | --help)
		sed -n '2,41p' "$0" | sed 's/^# \{0,1\}//'
		exit 0
		;;
	*)
		echo "make-testdata.sh: unknown argument '$1'" >&2
		exit 2
		;;
	esac
done

# ---------------------------------------------------------------------------
# The sizes, and why each one is that size

# Under scratch.rs's hardcoded 1 GiB RAM_LIMIT, so a copy-out of the whole thing routes to
# $XDG_RUNTIME_DIR — a tmpfs systemd sizes at 10% of RAM, which is 712 MB on this machine.
# 900 MiB is inside the window and larger than the filesystem it will be sent to. This is
# the fixture that proves the defect rather than hinting at it, and swap cannot rescue it:
# a tmpfs is capped by its size= mount option, not by available memory.
UNDER_MIB=900

# Over the same limit, so the copy routes to the cache directory instead. The other side of
# the branch, which nothing has ever exercised at this scale.
OVER_MIB=1536

# ~3 GB after zstd, which needs the bulk of it to be incompressible. Apply/rebuild and
# Measure at a size where the 3450U's cores are the constraint.
MIXED_RAND_MIB=2800
MIXED_TEXT_MIB=600

# The virtualized table, the listing walk, the filter and Select-all. Written by python's
# tarfile straight into the archive: 150k real files would cost 150k inodes and minutes of
# metadata churn to produce a fixture whose entries are never read.
ENTRIES=150000

# Deliberately larger than this machine's 7.0 GiB of RAM, so the member provably cannot be
# held in memory — which is the whole claim `arch.rs:1038` makes with its usize::MAX read.
# Sparse, so it costs no disk at all and reads as zeros at memory speed.
BIGSECRET_GIB=8

# Peak disk, counting the staging tree big-mixed is built from and deleted after.
NEED_MIB=$((UNDER_MIB + OVER_MIB + (MIXED_RAND_MIB + MIXED_TEXT_MIB) * 2 + 500))

# ---------------------------------------------------------------------------

say() { printf '  %s\n' "$*"; }
step() { printf '\n\033[1m%s\033[0m\n' "$*"; }

need() {
	command -v "$1" >/dev/null 2>&1 || {
		echo "make-testdata.sh: needs '$1' and cannot find it" >&2
		exit 1
	}
}
for t in bsdtar zstd openssl dd truncate python3; do need "$t"; done

mkdir -p "$DIR"
DIR="$(cd "$DIR" && pwd)"

# The corpus must live where a security fixture can prove something. /tmp, $XDG_RUNTIME_DIR
# and the DOTFILES overflow partition are all mounted nosuid,nodev on this machine; /home
# alone is not. An extraction test for setuid preservation or device-node creation that runs
# on such a mount passes because the mount forbade it, not because INDIUM did — which is a
# false pass, and the worst kind on a repository about to be frozen.
MNT_OPTS="$(findmnt -no OPTIONS --target "$DIR" 2>/dev/null || echo '')"
case "$MNT_OPTS" in
*nosuid* | *nodev*)
	echo "make-testdata.sh: WARNING — $DIR is on a mount with nosuid/nodev:" >&2
	echo "    $MNT_OPTS" >&2
	echo "  Bulk fixtures are fine here. Extraction-safety fixtures are not: they would" >&2
	echo "  pass because the mount forbade the thing, not because INDIUM did. Keep those" >&2
	echo "  on /home." >&2
	;;
esac

AVAIL_MIB="$(df -PBM "$DIR" | awk 'NR==2 {gsub(/M/,"",$4); print $4}')"

echo "INDIUM test corpus"
say "into        $DIR"
say "mount       ${MNT_OPTS:-unknown}"
say "free        ${AVAIL_MIB} MiB"
say "needs       ~${NEED_MIB} MiB at peak (plus ${BIGSECRET_GIB} GiB sparse, which costs none)"

if [ "$AVAIL_MIB" -lt "$NEED_MIB" ]; then
	echo >&2
	echo "make-testdata.sh: not enough room — needs ~${NEED_MIB} MiB, has ${AVAIL_MIB} MiB." >&2
	echo "  R9 offers /run/media/megas/DOTFILES/extra for bulk. Pass --dir to use it, and" >&2
	echo "  read the nosuid warning above before putting anything security-shaped there." >&2
	exit 1
fi

# have <name> — true when the fixture exists and --force was not given
have() {
	[ "$FORCE" -eq 0 ] && [ -e "$DIR/$1" ]
}

skip_or_build() {
	if have "$1"; then
		say "exists, leaving alone — pass --force to rebuild"
		return 1
	fi
	[ "$LIST" -eq 1 ] && {
		say "would build"
		return 1
	}
	return 0
}

# An AES-CTR keystream under a fixed pass phrase: incompressible, deterministic, and about
# four times faster than the disk it is written to. pipefail is off inside the subshell
# because dd closes the pipe at the byte count and openssl takes a SIGPIPE for it, which is
# the intended end of the stream rather than a failure.
blob() { # blob <path> <MiB> <seed>
	(
		set +o pipefail
		openssl enc -aes-256-ctr -pass "pass:indium-testdata-$3" -nosalt -pbkdf2 \
			</dev/zero 2>/dev/null |
			dd bs=1M count="$2" iflag=fullblock of="$1" status=none
	)
}

# Compressible filler, for the half of big-mixed that has to shrink.
text() { # text <path> <MiB>
	(
		set +o pipefail
		yes 'The quick brown fox jumps over the lazy dog. INDIUM test corpus filler.' |
			dd bs=1M count="$2" iflag=fullblock of="$1" status=none
	)
}

# ---------------------------------------------------------------------------

# Both of the small fixtures below are built with **explicit member names**, never with
# `-C dir .`, and the difference is not cosmetic. `-C dir .` stores a `./` root entry —
# which is exactly what under-limit.tar does two steps down, deliberately — and v2.1, the
# released build round 10 is walked against, refuses any archive that contains one. That is
# the defect PXX found at 12.6 and fixed. A reconstruction built the convenient way could
# not be opened by the very program it exists to certify.

step "photos.zip — the P11 album [TESTPLAN 1.7, 3.1-3.5, 4.1, 4.5, 4.7, 10.1-10.8]"
if skip_or_build photos.zip; then
	python3 - "$DIR/photos.zip" <<-'PY'
		import base64, sys, zipfile

		out = sys.argv[1]

		# A real 96x64 baseline JPEG, 517 bytes, minted once with ImageMagick and carried
		# here as base64 so this script needs no image tool of its own. Step 3.4 asks for
		# "image as image", and a file that merely ends in .jpg cannot answer that: the
		# `image` crate is what decodes it (Cargo.toml pins png/jpeg/gif/bmp), and it will
		# refuse nonsense. The fixture this replaces was 19 bytes.
		JPEG = base64.b64decode(
		    "/9j/4AAQSkZJRgABAQAAAQABAAD/2wBDAAYEBAUEBAYFBQUGBgYHCQ4JCQgICRINDQoOFRIWFhUS"
		    "FBQXGiEcFxgfGRQUHScdHyIjJSUlFhwpLCgkKyEkJST/2wBDAQYGBgkICREJCREkGBQYJCQkJCQk"
		    "JCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCT/wAARCABAAGADASIA"
		    "AhEBAxEB/8QAFgABAQEAAAAAAAAAAAAAAAAAAAUH/8QAFhAAAwAAAAAAAAAAAAAAAAAAABRh/8QA"
		    "FgEBAQEAAAAAAAAAAAAAAAAAAAUE/8QAFREBAQAAAAAAAAAAAAAAAAAAABP/2gAMAwEAAhEDEQA/"
		    "AMhVgVhWWgWhRuxTSVYFYVloFoLk0lWBWFZaBaC5NJVgVhWWgWguTSVYFYVloFoLk0lWBWFZaBaC"
		    "5NXWgWhWVgVhHu3zSVoFoVlYFYLk0laBaFZWBWC5NJWgWhWVgVguTSVoFoVlYFYLk0laBaFZWBWC"
		    "5NWVgVhWWgWhHuoTSVYFYVloFoLk0lWBWFZaBaC5NJVgVhWWgWguTSVYFYVloFoLk0lWBWFZaBaC"
		    "5NXWgWhWVgVhHu3zSVoFoVlYFYLk0laBaFZWBWC5NJWgWhWVgVguTSVoFoVlYFYLk0laBaFZWBWC"
		    "5N//2Q=="
		)

		# Non-text bytes for the hex view, deterministic by construction rather than by
		# seed. Step 3.4's "not text with holes in it" is what this is here to answer.
		BINARY = bytes((i * 37 + 11) & 0xFF for i in range(4096))

		# One fixed timestamp for every member, so two runs of this script on two machines
		# produce the same archive byte for byte — the same rule the AES-CTR filler follows.
		STAMP = (2026, 8, 10, 0, 0, 0)


		def add(z, name, body):
		    info = zipfile.ZipInfo(name, date_time=STAMP)
		    info.compress_type = zipfile.ZIP_DEFLATED
		    info.external_attr = 0o644 << 16
		    z.writestr(info, body)


		def adddir(z, name):
		    info = zipfile.ZipInfo(name.rstrip("/") + "/", date_time=STAMP)
		    info.external_attr = (0o755 << 16) | 0x10
		    z.writestr(info, b"")


		with zipfile.ZipFile(out, "w") as z:
		    # Thirty captions, so PgUp/PgDn/Home/End in step 3.1 have somewhere to travel
		    # and Ctrl+F in 3.5 has a fragment that narrows to a knowable number of rows:
		    # "caption-1" matches ten of them, "caption-01" exactly one.
		    for i in range(1, 31):
		        add(z, f"caption-{i:02d}.txt", f"Caption for photograph {i:02d}.\n".encode())

		    # Step 10.8 names this file and the byte count `wc -c` must print for it, so
		    # its contents are fixed here rather than described.
		    add(
		        z,
		        "README.txt",
		        b"INDIUM test album\n"
		        b"\n"
		        b"Built by build/make-testdata.sh for the TESTPLAN rounds that need a small\n"
		        b"archive with awkward names in it. Nothing here is precious.\n",
		    )
		    add(z, "index.md", b"# Album\n\nSee README.txt.\n")

		    # The names step 4.1 selects and pastes into a file manager. The emoji one is
		    # in the plan's record too: 4.1 was approved with a note that INDIUM draws it
		    # as `emoji-?-box.txt`, which is the one-face rule of CORE 6 showing through
		    # and not a defect. Keeping the file keeps that note reproducible.
		    add(z, "beach day.jpg", JPEG)
		    add(z, "köpek.txt", "Bir köpek fotoğrafı.\n".encode())
		    add(z, "ÇAĞDAŞ-ÖĞÜT-ŞİŞLİ.txt", b"Istanbul.\n")
		    add(z, "emoji-\U0001f4e6-box.txt", b"A box.\n")
		    add(z, "trailing.space .txt", b"The name ends in a space.\n")

		    # Step 10.7: `indium extract photos.zip -- --weird-name`. A member whose name
		    # is shaped like a flag is the only thing that can prove `--` ends the flags.
		    add(z, "--weird-name", b"Named like a flag, extracted like a file.\n")

		    add(z, "thumb.bin", BINARY)

		    # Somewhere to descend into and come back out of, for step 3.2's Enter and
		    # Backspace. Stored as real directory entries so `indium list` shows them too.
		    adddir(z, "2026")
		    adddir(z, "2026/summer")
		    adddir(z, "2026/winter")
		    add(z, "2026/summer/sunset.jpg", JPEG)
		    add(z, "2026/summer/notes.txt", b"Warm.\n")
		    add(z, "2026/winter/frost.txt", b"Cold.\n")
	PY
	say "$(du -h "$DIR/photos.zip" | cut -f1) — 40 rows at the top level, no ./ root"
fi

step "docs.tar.gz — a plain gzipped tar that must simply open [TESTPLAN 1.8]"
if skip_or_build docs.tar.gz; then
	python3 - "$DIR/docs.tar.gz" <<-'PY'
		import gzip, io, sys, tarfile

		out = sys.argv[1]


		def add(tar, name, body):
		    info = tarfile.TarInfo(name)
		    info.size = len(body)
		    info.mtime = 1754784000
		    info.mode = 0o644
		    tar.addfile(info, io.BytesIO(body))


		# The gzip member carries its own timestamp, and the default is "now" — which would
		# make this the one fixture in the corpus whose bytes changed between two runs for
		# no reason anybody could see. mtime=0 and an empty stored filename settle it.
		with open(out, "wb") as raw:
		    with gzip.GzipFile(filename="", mtime=0, fileobj=raw, mode="wb") as gz:
		        with tarfile.open(fileobj=gz, mode="w") as tar:
		            add(tar, "docs/guide.md", b"# Guide\n\nHow to use the thing.\n")
		            add(tar, "docs/reference.md", b"# Reference\n\nEvery flag, once.\n")
		            add(tar, "docs/CHANGELOG.md", b"# Changelog\n\n- It exists.\n")
		            add(tar, "docs/notes/todo.txt", b"Nothing outstanding.\n")
	PY
	say "$(du -h "$DIR/docs.tar.gz" | cut -f1) — four members under docs/, no ./ root"
fi

step "under-limit.tar — ${UNDER_MIB} MiB, under the 1 GiB RAM_LIMIT [TESTPLAN 12.6]"
if skip_or_build under-limit.tar; then
	stage="$DIR/.stage-under"
	rm -rf "$stage" && mkdir -p "$stage"
	blob "$stage/part-a.bin" $((UNDER_MIB / 3)) under-a
	blob "$stage/part-b.bin" $((UNDER_MIB / 3)) under-b
	blob "$stage/part-c.bin" $((UNDER_MIB - 2 * (UNDER_MIB / 3))) under-c
	bsdtar -cf "$DIR/under-limit.tar" -C "$stage" .
	rm -rf "$stage"
	say "$(du -h "$DIR/under-limit.tar" | cut -f1) — routes to \$XDG_RUNTIME_DIR on copy-out"
fi

step "over-limit.tar — ${OVER_MIB} MiB, over the same limit [TESTPLAN 12.7]"
if skip_or_build over-limit.tar; then
	stage="$DIR/.stage-over"
	rm -rf "$stage" && mkdir -p "$stage"
	blob "$stage/part-a.bin" $((OVER_MIB / 2)) over-a
	blob "$stage/part-b.bin" $((OVER_MIB - OVER_MIB / 2)) over-b
	bsdtar -cf "$DIR/over-limit.tar" -C "$stage" .
	rm -rf "$stage"
	say "$(du -h "$DIR/over-limit.tar" | cut -f1) — routes to the cache directory instead"
fi

step "big-mixed.tar.zst — ~3 GB, mixed content [TESTPLAN 12.3, 12.4, 12.5]"
if skip_or_build big-mixed.tar.zst; then
	stage="$DIR/.stage-mixed"
	rm -rf "$stage" && mkdir -p "$stage/media" "$stage/text" "$stage/small"
	say "incompressible half (${MIXED_RAND_MIB} MiB) …"
	blob "$stage/media/video.bin" $((MIXED_RAND_MIB / 2)) mixed-a
	blob "$stage/media/audio.bin" $((MIXED_RAND_MIB / 4)) mixed-b
	blob "$stage/media/raw.bin" $((MIXED_RAND_MIB - MIXED_RAND_MIB / 2 - MIXED_RAND_MIB / 4)) mixed-c
	say "compressible half (${MIXED_TEXT_MIB} MiB) …"
	text "$stage/text/corpus.txt" $((MIXED_TEXT_MIB / 2))
	text "$stage/text/log.txt" $((MIXED_TEXT_MIB - MIXED_TEXT_MIB / 2))
	say "a spread of small files, so Measure sees variety rather than two blobs …"
	for i in $(seq 1 200); do
		blob "$stage/small/f${i}.bin" 1 "small-$i"
	done
	say "tar | zstd, streaming — the tar is never staged on disk …"
	bsdtar -cf - -C "$stage" . | zstd -3 -T0 -q -o "$DIR/big-mixed.tar.zst" -f
	rm -rf "$stage"
	say "$(du -h "$DIR/big-mixed.tar.zst" | cut -f1)"
fi

step "many-entries.tar — ${ENTRIES} entries [TESTPLAN 12.1, 12.2]"
if skip_or_build many-entries.tar; then
	python3 - "$DIR/many-entries.tar" "$ENTRIES" <<-'PY'
		import io, sys, tarfile

		out, count = sys.argv[1], int(sys.argv[2])

		# Written straight into the archive. 150k real files would cost 150k inodes and
		# minutes of metadata churn to produce entries the test never opens; what round 12
		# exercises is the listing walk, the filter and the virtualized table, all of which
		# only ever see the headers.
		with tarfile.open(out, "w") as tar:
		    for i in range(count):
		        # Fanned out 500 to a directory: a flat 150k-entry directory is a shape no
		        # real archive has, and the breadcrumb has nothing to say about it.
		        name = f"tree/{i // 500:04d}/entry-{i:06d}.txt"
		        payload = f"entry {i}\n".encode()
		        info = tarfile.TarInfo(name)
		        info.size = len(payload)
		        info.mtime = 1754784000  # fixed, so two runs give identical bytes
		        info.mode = 0o644
		        tar.addfile(info, io.BytesIO(payload))
	PY
	say "$(du -h "$DIR/many-entries.tar" | cut -f1) — $ENTRIES entries, 500 to a directory"
fi

step "deep.tar — pathological nesting and names [TESTPLAN 3.12]"
if skip_or_build deep.tar; then
	python3 - "$DIR/deep.tar" "$HOME" <<-'PY'
		import io, sys, tarfile

		out, home = sys.argv[1], sys.argv[2].rstrip("/")

		def add(tar, name, body=b"x\n", mode=0o644):
		    info = tarfile.TarInfo(name)
		    info.size = len(body)
		    info.mtime = 1754784000
		    info.mode = mode
		    tar.addfile(info, io.BytesIO(body))

		with tarfile.open(out, "w") as tar:
		    # 60 levels. The breadcrumb has to elide in the middle rather than grow off the
		    # edge of the window, and the elision is what round 3 step 12 looks at.
		    deep = "/".join(f"level-{i:02d}" for i in range(60))
		    add(tar, f"{deep}/bottom.txt", b"the bottom\n")

		    # A single component at the long end of what most filesystems take.
		    add(tar, "long/" + ("n" * 250) + ".txt")

		    # Names that have historically broken table rendering, one way or another.
		    add(tar, "awkward/a file with spaces.txt")
		    add(tar, "awkward/ÇAĞDAŞ-ÖĞÜT-ŞİŞLİ.txt")
		    add(tar, "awkward/emoji-\U0001f4e6-box.txt")
		    add(tar, "awkward/tab\there.txt")
		    add(tar, "awkward/newline\nhere.txt")
		    add(tar, "awkward/-leading-dash.txt")
		    add(tar, "awkward/.hidden")
		    add(tar, "awkward/trailing.space .txt")

		    # The traversal members. path_escapes (arch.rs:940-946) is supposed to refuse
		    # every one of these, and TESTPLAN 3.13 is where that gets checked. They are in
		    # the fixture precisely because a check nobody ever fed a hostile path to is a
		    # check nobody has tested. Extract this ONLY into a throwaway directory.
		    add(tar, "../escaped-one-up.txt", b"should never be written\n")
		    add(tar, "../../escaped-two-up.txt", b"should never be written\n")
		    add(tar, "safe/../../escaped-via-middle.txt", b"should never be written\n")
		    # The absolute member points into $HOME rather than at /absolute-escape.txt.
		    # A non-root INDIUM cannot write to / whatever path_escapes decides, so EACCES
		    # would mask the hole and the step would pass for the wrong reason — the same
		    # trap as running the setuid fixture on a nosuid mount. Aimed somewhere the
		    # process really can write, only a refusal stops it.
		    add(tar, f"{home}/escaped-absolute.txt", b"should never be written\n")
	PY
	say "$(du -h "$DIR/deep.tar" | cut -f1) — 60 levels, hostile names, and four traversal members"
	say "WARNING: extract deep.tar into a throwaway directory and nowhere else."
fi

step "bigsecret-input.bin — ${BIGSECRET_GIB} GiB, sparse [feeds TESTPLAN 12.8]"
if skip_or_build bigsecret-input.bin; then
	truncate -s "${BIGSECRET_GIB}G" "$DIR/bigsecret-input.bin"
	say "apparent $(du -h --apparent-size "$DIR/bigsecret-input.bin" | cut -f1), on disk $(du -h "$DIR/bigsecret-input.bin" | cut -f1) — sparse zeros"
	say "TESTPLAN 12.8 builds bigsecret.7z from this, through Create → Encrypted."
	say "It is not built here: there is no 7z binary on this machine, and INDIUM"
	say "writing it is itself the test."
fi

step "a throwaway directory for extractions"
[ "$LIST" -eq 1 ] || mkdir -p "$DIR/sandbox"
say "$DIR/sandbox — where deep.tar gets extracted, and nowhere else"

step "done"
if [ "$LIST" -eq 1 ]; then
	say "nothing was built — --list only"
else
	ls -lh "$DIR" | awk 'NR>1 {printf "  %-28s %s\n", $9, $5}'
fi
