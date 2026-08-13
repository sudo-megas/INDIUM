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

**344 of 344.** Every test the repository contains has now been run and passed at once, which had
never happened before. The ten ignored tests divide three ways, and no single environment had
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
