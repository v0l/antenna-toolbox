#!/bin/sh
set -e
cd "$(dirname "$0")/../crates/app"
RUSTFLAGS="-C target-feature=+atomics,+bulk-memory,+mutable-globals -C link-arg=--shared-memory -C link-arg=--max-memory=4294967296 -C link-arg=--import-memory -C link-arg=--export=__wasm_init_tls -C link-arg=--export=__tls_size -C link-arg=--export=__tls_align -C link-arg=--export=__tls_base" \
    rustup run nightly wasm-pack build . --release --target web --out-dir ../../web/pkg --no-typescript --no-pack \
    -- -Z build-std=panic_abort,std
cd ../../web/pkg
sed -i "s|import('../../..')|import('../../../antenna_toolbox.js')|" snippets/wasm-bindgen-rayon-*/src/workerHelpers.js
