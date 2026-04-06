#!/usr/bin/env bash
# Build a framework-compatible game zip (manifest.json, logic.wasm, client/*).
# Requires: Rust + wasm32-wasip1, cargo-component, Node/npm (for web client).
# Run from any cwd; paths are resolved from this script.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
GAME_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
FRAMEWORK_ROOT="$(cd "$GAME_DIR/../.." && pwd)"
OUT_ZIP="${1:-$GAME_DIR/dist/tic_tac_toe.zip}"

echo "==> Framework root: $FRAMEWORK_ROOT"
echo "==> Game dir:       $GAME_DIR"
echo "==> Output zip:     $OUT_ZIP"

cd "$FRAMEWORK_ROOT"
cargo component build -p tic_tac_toe_component --release

cd "$GAME_DIR/web"
if [[ -d node_modules ]]; then
  npm run build
else
  npm ci
  npm run build
fi

LOGIC_SRC="$FRAMEWORK_ROOT/target/wasm32-wasip1/release/tic_tac_toe_component.wasm"
if [[ ! -f "$LOGIC_SRC" ]]; then
  echo "error: missing $LOGIC_SRC" >&2
  exit 1
fi

STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT

cp "$GAME_DIR/manifest.json" "$STAGE/"
cp "$LOGIC_SRC" "$STAGE/logic.wasm"
cp -r "$GAME_DIR/client" "$STAGE/client"

for f in index.html config.html result.html; do
  if [[ ! -f "$STAGE/client/$f" ]]; then
    echo "error: missing client/$f (run vite build)" >&2
    exit 1
  fi
done

mkdir -p "$(dirname "$OUT_ZIP")"
rm -f "$OUT_ZIP"
(cd "$STAGE" && zip -r "$OUT_ZIP" manifest.json logic.wasm client)

echo "==> Wrote $OUT_ZIP"
