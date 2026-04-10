# WIT / wit-bindgen version pairing

The game server loads a **WebAssembly component** that implements the `game-core` world from [`../../test.wit`](../../test.wit) (Wasmtime component model).

## Rust guest (`game-wasm-host`)

- **Crate:** [`game-wasm-host`](../../game-wasm-host/Cargo.toml) uses **`wit-bindgen` 0.46.x** (and `wit-bindgen-rt` as specified in that `Cargo.toml`). This is newer than the TeaVM Java generator line; both target the same [`test.wit`](../../test.wit) world.
- **Build:** `cargo component build` for the per-game component crate.

## Java guest (TeaVM)

- **codegen:** The `teavm-java` binding generator ships only in **wit-bindgen-cli 0.40.x**. From 0.46 onward, the published CLI no longer includes the TeaVM Java backend, so Java projects should install a **dedicated** 0.40 binary for regeneration, e.g.  
  `cargo install --force --version 0.40.0 wit-bindgen-cli`  
  and run it as `wit-bindgen teavm-java …` (or install to a separate `--root` and invoke that copy so it does not replace a newer `wit-bindgen` used elsewhere).
- **Command:**  
  `wit-bindgen teavm-java <path-to-test.wit> --world game-core --out-dir <generated-src-dir>`  
  Omit `--generate-stub` after the first run so **`GameCore.java`** can be refreshed without overwriting your hand-written **`GameCoreImpl.java`**.
- **ABI:** Generated components embed a `component-type:GameCore` custom section produced by wit-bindgen 0.40’s toolchain (`wit-component` 0.227.x). This remains compatible with the server’s Wasmtime **37.x** component loader for the same WIT definitions.

## Practical rule

Keep **`test.wit`** as the single source of truth. After editing it, regenerate **Rust** bindings (via `wit-bindgen` in the Rust crate) and **Java** bindings (via wit-bindgen-cli **0.40** `teavm-java`), then rebuild both guests.
