#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
GAME_DIR="$ROOT/games/tic_tac_toe"
BOT_DIR="$ROOT/games/tic_tac_toe_bot"
FIXTURE_DIR="$ROOT/server/tests/fixtures/games/tic_tac_toe"
BOT_FIXTURE_DIR="$ROOT/server/tests/fixtures/bots/tic_tac_toe_bot"

cd "$ROOT"
cargo run -p tic_tac_toe --features schemars --bin export_schema

cd "$GAME_DIR/rust/component"
cargo component build --release
WASM="$ROOT/target/wasm32-wasip1/release/tic_tac_toe_component.wasm"
test -f "$WASM"

mkdir -p "$FIXTURE_DIR/client"
cp -f "$WASM" "$FIXTURE_DIR/logic.wasm"
cp -f "$GAME_DIR/manifest.json" "$FIXTURE_DIR/manifest.json"
cp -f "$GAME_DIR/contract.json" "$FIXTURE_DIR/contract.json"
for html in index.html config.html result.html about.html; do
  if [[ -f "$GAME_DIR/client/$html" ]]; then
    cp -f "$GAME_DIR/client/$html" "$FIXTURE_DIR/client/$html"
  fi
done
echo "Staged game fixture at $FIXTURE_DIR"

cd "$BOT_DIR/rust/component"
cargo component build --release
BOT_WASM="$ROOT/target/wasm32-wasip1/release/tic_tac_toe_bot_component.wasm"
test -f "$BOT_WASM"

mkdir -p "$BOT_FIXTURE_DIR"
cp -f "$BOT_WASM" "$BOT_FIXTURE_DIR/bot.wasm"
cp -f "$BOT_DIR/manifest.json" "$BOT_FIXTURE_DIR/manifest.json"
cp -f "$BOT_DIR/contract/contract.json" "$BOT_FIXTURE_DIR/contract.json"
echo "Staged bot fixture at $BOT_FIXTURE_DIR"
