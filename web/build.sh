#!/bin/sh
set -e
cd "$(dirname "$0")/.."
version=$(grep -A1 '^name = "wasm-bindgen"$' Cargo.lock | sed -n 's/^version = "\(.*\)"/\1/p')
bindgen=target/tools/bin/wasm-bindgen
if ! "$bindgen" --version 2>/dev/null | grep -q " $version$"; then
    cargo install --locked --root target/tools wasm-bindgen-cli --version "$version"
fi
cargo build --release --target wasm32-unknown-unknown -p thermview-web
out=target/web
rm -rf "$out"
"$bindgen" --target web --no-typescript --out-dir "$out" target/wasm32-unknown-unknown/release/thermview_web.wasm
if command -v wasm-opt >/dev/null; then
    wasm-opt -O2 --enable-bulk-memory --enable-nontrapping-float-to-int -o "$out/thermview_web_bg.wasm" "$out/thermview_web_bg.wasm"
fi
build=$(sha256sum "$out/thermview_web_bg.wasm" | cut -c1-12)
sed "s/\"BUILD\"/\"$build\"/" web/index.html > "$out/index.html"
cp -r web/public/. "$out/"
echo "built $out"
