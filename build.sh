#!/bin/sh
# Builds the plug-in: supersaturation.wasm in this folder.
# Needs the wasm32-unknown-unknown target: `rustup target add wasm32-unknown-unknown`.
set -eu
cd "$(dirname "$0")"
cargo build --release --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/supersaturation.wasm supersaturation.wasm
echo "built supersaturation.wasm ($(wc -c < supersaturation.wasm) bytes)"
