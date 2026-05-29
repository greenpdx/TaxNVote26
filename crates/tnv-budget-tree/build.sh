#!/bin/bash
# Build TNV Budget Tree WASM module
# Prerequisites: rustup, wasm-pack
# Place your budauth.csv in data/budauth.csv before building with --features wasm

set -e

echo "=== Building native (tests) ==="
cargo test

echo "=== Building WASM ==="
wasm-pack build --target web --out-dir pkg --features wasm

echo "=== Done ==="
echo "Output in pkg/"
ls -la pkg/
