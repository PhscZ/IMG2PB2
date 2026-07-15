#!/usr/bin/env bash
# Build the web (WASM) version of IMG2PB2 and place the output in web/pkg.
#
# Requirements (one-time):
#   rustup target add wasm32-unknown-unknown
#   cargo install wasm-bindgen-cli
#
# After building, serve the `web/` directory with any static server, e.g.:
#   npx serve web
#   # or
#   python -m http.server --directory web 8080
# Then open http://localhost:8080

set -euo pipefail

cd "$(dirname "$0")"

echo ">> Building img2pb2-web for wasm32-unknown-unknown (release)..."
cargo build -p img2pb2-web --lib --release --target wasm32-unknown-unknown

# Match the wasm-bindgen CLI version to the wasm-bindgen library in Cargo.lock.
BINDGEN_VERSION=$(grep -m1 '^name = "wasm-bindgen"$' -A1 Cargo.lock | grep '^version' | sed 's/.*"\(.*\)"/\1/')
WASM=target/wasm32-unknown-unknown/release/img2pb2_web.wasm

echo ">> Running wasm-bindgen $BINDGEN_VERSION (--target web)..."
wasm-bindgen --version
wasm-bindgen --target web --no-typescript --out-dir web/pkg "$WASM"

echo ">> Done. Output is in web/pkg."
echo "   Serve the 'web/' folder and open it in a browser."
