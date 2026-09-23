#!/bin/sh
set -e
cd "$(dirname "$0")/../crates/app"
wasm-pack build --release --target web --out-dir ../../web/pkg --no-typescript --no-pack
