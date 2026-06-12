# WASI preview1 → component adapter

`wasi_snapshot_preview1.reactor.wasm` is the **reactor** adapter from [Wasmtime v37.0.2](https://github.com/bytecodealliance/wasmtime/releases/tag/v37.0.2) (same major line as `framework/server`’s `wasmtime` dependency). It is passed to `wasm-tools component new --adapt` so a core module that imports `wasi_snapshot_preview1::*` can be wrapped into a WebAssembly Component.

`teavm_ascii.wasm` (built from `teavm_ascii.wat`) satisfies TeaVM’s optional `teavm::towlower` / `teavm::towupper` imports (ASCII-only stubs) during the same `component new` step.

To refresh (e.g. after bumping Wasmtime):

```text
curl -L -o wasi_snapshot_preview1.reactor.wasm \
  https://github.com/bytecodealliance/wasmtime/releases/download/v37.0.2/wasi_snapshot_preview1.reactor.wasm
```
