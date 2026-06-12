#!/usr/bin/env bash
# Dioxus lobby dev server on :8080 (proxies GraphQL/games to backend on :8081).
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root/lobby"

if [[ ! -d node_modules ]]; then
  echo "Installing lobby CSS tooling (npm ci)..."
  npm ci
fi
if [[ ! -f assets/tailwind.css ]]; then
  echo "Building Tailwind CSS..."
  npm run build:css
fi

echo ""
echo "Starting lobby at http://127.0.0.1:8080"
echo "Backend must run separately on :8081 — from framework/: PORT=8081 cargo run -p server"
echo ""

dx serve --platform web
