#!/usr/bin/env bash
# Embed a TeaVM core Wasm module into a WebAssembly Component for game-core WIT.
# Requires wasm-tools on PATH (pin: see sdk/java/WIT_VERSIONS.md).
set -euo pipefail
CORE_WASM="${1:?core wasm path}"
WIT_FILE="${2:?wit path}"
OUT_WASM="${3:?output component path}"
[[ -f "$CORE_WASM" ]] || { echo "missing core wasm: $CORE_WASM" >&2; exit 1; }
[[ -f "$WIT_FILE" ]] || { echo "missing wit: $WIT_FILE" >&2; exit 1; }
OUT_DIR="$(dirname "$OUT_WASM")"
mkdir -p "$OUT_DIR"
EMBEDDED_WASM="${OUT_DIR}/logic-embedded.wasm"
ADAPTER="${OUT_DIR}/../wasm/wasi_snapshot_preview1.reactor.wasm"
if [[ ! -f "$ADAPTER" ]]; then
  ADAPTER="$(cd "$OUT_DIR/../.." && pwd)/wasm/wasi_snapshot_preview1.reactor.wasm"
fi
[[ -f "$ADAPTER" ]] || { echo "missing WASI adapter (see component-template/wasm/README.md): $ADAPTER" >&2; exit 1; }
wasm-tools component embed --world game-core "$WIT_FILE" "$CORE_WASM" -o "$EMBEDDED_WASM"
TEAVM_ADAPTER="$(dirname "$ADAPTER")/teavm_ascii.wasm"
[[ -f "$TEAVM_ADAPTER" ]] || { echo "missing TeaVM adapter: $TEAVM_ADAPTER" >&2; exit 1; }
wasm-tools component new "$EMBEDDED_WASM" \
  --adapt "wasi_snapshot_preview1=${ADAPTER}" \
  --adapt "teavm=${TEAVM_ADAPTER}" \
  -o "$OUT_WASM"
wasm-tools validate "$OUT_WASM"
echo "Wrote component: $OUT_WASM"
