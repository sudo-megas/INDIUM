#!/bin/sh
# CORE §2: "No GTK. No Qt. No KF6. No portal." This script is the enforcement.
#
# It runs by hand before every release, inside the Arch package's own check() so no package
# can be built from a binary that grew a toolkit, and inside the release workflow. What
# V1.4 adds is the rest of it: CI on every push, with this script as a gate on a merge.
set -e
out=$(ldd target/release/indium)
echo "$out"
for bad in gtk Qt KF6 X11 portal; do
  echo "$out" | grep -i "$bad" && { echo "FAIL: $bad linked"; exit 1; }
done
echo "OK: toolkit-free"

# P6, hardening. Rust gives both of these by default on x86_64-unknown-linux-gnu — that is
# exactly why they are ASSERTED here rather than configured anywhere. Nothing in this
# repository asks for them, so a toolchain that quietly stopped giving them would ship a
# binary with a fixed load address and a writable GOT, and nobody would be told.
readelf -hW target/release/indium | grep -q "Type:.*DYN" || { echo "FAIL: not PIE"; exit 1; }
readelf -dW target/release/indium | grep -q "BIND_NOW"   || { echo "FAIL: no BIND_NOW"; exit 1; }
echo "OK: PIE, BIND_NOW"
