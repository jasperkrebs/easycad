#!/usr/bin/env bash
# Builds the Rust engine for the browser: wasm32 build → wasm-bindgen JS glue → web/pkg/.
# Fails if the engine grows past the size budget (docs/architecture.md §13).
set -euo pipefail
cd "$(dirname "$0")/.."

BUDGET_GZIP_BYTES=$((3 * 1024 * 1024))
WASM=target/wasm32-unknown-unknown/wasm/easycad_wasm.wasm

cargo build -p easycad-wasm --profile wasm --target wasm32-unknown-unknown

# The CLI must match the wasm-bindgen crate version exactly.
need=$(cargo metadata --format-version 1 --locked |
  python3 -c "import sys, json; print(next(p['version'] for p in json.load(sys.stdin)['packages'] if p['name'] == 'wasm-bindgen'))")
have=$(wasm-bindgen --version 2>/dev/null | awk '{print $2}' || true)
if [ "$need" != "$have" ]; then
  echo "wasm-bindgen-cli ${have:-missing}, but the crate is $need." >&2
  echo "Install it with: cargo install wasm-bindgen-cli --version $need --locked" >&2
  exit 1
fi

rm -rf web/pkg
wasm-bindgen --target web --no-typescript --out-dir web/pkg "$WASM"

raw=$(wc -c < web/pkg/easycad_wasm_bg.wasm)
gz=$(gzip -9 -c web/pkg/easycad_wasm_bg.wasm | wc -c)
echo "engine: $raw bytes, $gz bytes gzipped (budget $BUDGET_GZIP_BYTES)"
if [ "$gz" -gt "$BUDGET_GZIP_BYTES" ]; then
  echo "The engine is over its size budget." >&2
  exit 1
fi
