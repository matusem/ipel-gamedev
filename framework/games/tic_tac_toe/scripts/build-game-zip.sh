#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
GAME_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
ACTION="${1:-build}"
echo "[deprecated] This script now forwards to gamedev-cli."
if [[ "$ACTION" == "deploy" ]]; then
  cargo run -p gamedev-cli -- deploy --project-dir "$GAME_DIR" --auto-publish
else
  cargo run -p gamedev-cli -- build --project-dir "$GAME_DIR"
fi
