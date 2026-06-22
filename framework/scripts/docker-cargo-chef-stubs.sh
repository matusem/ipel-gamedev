#!/usr/bin/env bash
# Minimal Rust sources for cargo-chef cook (dependency layer only).
set -euo pipefail

ROOT="${1:-.}"
cd "$ROOT"

WORKSPACE_MEMBERS=(
  game
  bot
  bot-wasm-host
  game-wasm-host
  server
  games/tic_tac_toe/rust/logic
  games/tic_tac_toe/rust/component
  games/tic_tac_toe_bot/rust/logic
  games/tic_tac_toe_bot/rust/component
  tools/gamedev-cli
  sdk/rust/shared-types
  sdk/rust/shared
  sdk/rust/bevy
  sdk/rust/dioxus
)

stub_lib() {
  local dir="$1"
  mkdir -p "$dir/src"
  printf '%s\n' 'pub fn __docker_chef_stub() {}' > "$dir/src/lib.rs"
}

stub_bin() {
  local dir="$1"
  mkdir -p "$dir/src"
  printf '%s\n' 'fn main() {}' > "$dir/src/main.rs"
}

for member in "${WORKSPACE_MEMBERS[@]}"; do
  manifest="$member/Cargo.toml"
  if [[ ! -f "$manifest" ]]; then
    echo "missing $manifest" >&2
    exit 1
  fi

  case "$member" in
    tools/gamedev-cli)
      stub_bin "$member"
      ;;
    server)
      stub_lib "$member"
      stub_bin "$member"
      ;;
    *)
      stub_lib "$member"
      ;;
  esac
done
