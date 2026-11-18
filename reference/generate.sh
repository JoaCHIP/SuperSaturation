#!/bin/sh
# Compiles the original C++ transform and regenerates tests/data/reference.bin from it.
# -ffp-contract=off: no fused multiply-adds, so the arithmetic matches the Rust port's.
set -eu
cd "$(dirname "$0")"
out=$(mktemp -d)
trap 'rm -rf "$out"' EXIT
g++ -std=c++17 -O2 -ffp-contract=off -o "$out/reference" reference.cpp
mkdir -p ../tests/data
"$out/reference" ../tests/data/reference.bin
echo "wrote tests/data/reference.bin ($(wc -c < ../tests/data/reference.bin) bytes)"
