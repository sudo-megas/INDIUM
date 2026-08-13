#!/usr/bin/env bash
#
# make-testdata.sh — build the GB-scale corpus that `build/docs/TESTPLAN.md` rounds 11-13
# are written against.
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
#   the small fixtures from the P11 round — photos.zip, large.tar, docs.tar.gz, backup.7z,
#                  secret.7z, notrar.rar, to-add/ — which already exist in the corpus
#                  directory and are left alone. This script only adds what rounds 11-13 need.
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
		sed -n '2,34p' "$0" | sed 's/^# \{0,1\}//'
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
