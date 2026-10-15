#!/bin/sh
# Builds the browser version into dist/web (and dist/the_garden-web.zip for itch.io).
# Needs: rustup target wasm32-unknown-unknown, wasm-bindgen-cli (same version as in Cargo.lock),
# optionally wasm-opt (binaryen) for a smaller download.
set -eu
cd "$(dirname "$0")/.."
cargo build --profile wasm-release --target wasm32-unknown-unknown
rm -rf dist/web
mkdir -p dist/web
wasm-bindgen --target web --no-typescript --out-name the_garden --out-dir dist/web \
  target/wasm32-unknown-unknown/wasm-release/the_garden.wasm
python3 tools/fix_wasm_externrefs.py dist/web/the_garden_bg.wasm
if command -v wasm-opt >/dev/null 2>&1; then
  wasm-opt -Os --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext \
    dist/web/the_garden_bg.wasm -o dist/web/the_garden_bg.wasm.opt && \
    mv dist/web/the_garden_bg.wasm.opt dist/web/the_garden_bg.wasm || \
    echo "wasm-opt failed, keeping the unoptimized module"
fi
cp web/index.html dist/web/index.html
cp assets/sprites/logo.png dist/web/logo.png
cp assets/fonts/OFL-Nunito.txt dist/web/LICENSE-font.txt
# itch.io wants a zip with index.html at the root
rm -f dist/the_garden-web.zip
(cd dist/web && zip -9 -qr ../the_garden-web.zip .)
ls -la dist/web dist/the_garden-web.zip
