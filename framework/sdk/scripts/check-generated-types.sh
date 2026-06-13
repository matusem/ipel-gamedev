#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

cargo run -p upjs-gdd-shared-types --features typegen --bin export_ts
git diff --exit-code -- "framework/sdk/js/generated-types"
