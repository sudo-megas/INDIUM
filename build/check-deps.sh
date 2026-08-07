#!/bin/sh
# CORE §2: "No GTK. No Qt. No KF6. No portal." This script is the enforcement.
# It runs by hand until V1.4 wires it into CI, and it runs before every release.
set -e
out=$(ldd target/release/indium)
echo "$out"
for bad in gtk Qt KF6 X11 portal; do
  echo "$out" | grep -i "$bad" && { echo "FAIL: $bad linked"; exit 1; }
done
echo "OK: toolkit-free"
