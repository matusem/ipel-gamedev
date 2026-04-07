#!/usr/bin/env bash
# Build a framework-compatible game zip (manifest.json, logic.wasm, client/*).
# Requires: Rust + wasm32-wasip1 + wasm32-unknown-unknown, cargo-component,
#           wasm-bindgen-cli on PATH (e.g. cargo install wasm-bindgen-cli --locked).
# Run from any cwd; paths are resolved from this script.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
GAME_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
OUT_ZIP="${1:-$GAME_DIR/dist/checkers.zip}"

command -v wasm-bindgen >/dev/null 2>&1 || {
  echo "error: wasm-bindgen not found. Install: cargo install wasm-bindgen-cli --locked" >&2
  exit 1
}

echo "==> Game dir:   $GAME_DIR"
echo "==> Output zip: $OUT_ZIP"

cd "$GAME_DIR"
rustup target add wasm32-wasip1 wasm32-unknown-unknown 2>/dev/null || true

cargo component build -p checkers_component --release
cargo build -p checkers_web --release --target wasm32-unknown-unknown

WASM_BIN="$GAME_DIR/target/wasm32-unknown-unknown/release/checkers_web.wasm"
LOGIC_SRC="$GAME_DIR/target/wasm32-wasip1/release/checkers_component.wasm"

if [[ ! -f "$LOGIC_SRC" ]]; then
  echo "error: missing $LOGIC_SRC" >&2
  exit 1
fi
if [[ ! -f "$WASM_BIN" ]]; then
  echo "error: missing $WASM_BIN" >&2
  exit 1
fi

wasm-bindgen "$WASM_BIN" --out-dir "$GAME_DIR/client" --target web --no-typescript

STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT

cp "$GAME_DIR/manifest.json" "$STAGE/"
cp "$LOGIC_SRC" "$STAGE/logic.wasm"
cp -r "$GAME_DIR/client" "$STAGE/client"

for f in index.html config.html result.html about.html; do
  if [[ ! -f "$STAGE/client/$f" ]]; then
    echo "error: missing client/$f" >&2
    exit 1
  fi
done

mkdir -p "$(dirname "$OUT_ZIP")"
rm -f "$OUT_ZIP"
(cd "$STAGE" && zip -r "$OUT_ZIP" manifest.json logic.wasm client)

echo "==> Wrote $OUT_ZIP"
