# Java game logic (Fermyon TeaVM + `GameCore` WIT)

This directory contains:

- **`game/`** — portable library mirroring the Rust [`game`](../../game) crate (`GameRules`, `GameOrchestrator`, serde helpers). Compiles to **Java 17 bytecode** (via `--release 17`) while Gradle may run on **JDK 21+**.
- **`component-template/`** — **Tic-tac-toe** logic matching the Rust flat template (`rust_logic_flat_lib.rs`), wired through [`GameCoreImpl`](component-template/src/main/java/wit/worlds/GameCoreImpl.java) and the generated WIT bindings under `src/generated/java/`. The guest is built with **[fermyon/teavm-wasi](https://github.com/fermyon/teavm-wasi)** (`com.fermyon` on Maven Central), orchestrated by **Gradle** + **`pom.xml`** + the **Maven Wrapper** (`mvnw` / `mvnw.cmd`). JSON/MessagePack shapes follow the same serde conventions as the Rust game.

## Prerequisites

- **JDK 21+** for running Gradle (set `JAVA_HOME`; see [Gradle JVM requirements](https://docs.gradle.org/current/userguide/compatibility.html)).
- **JDK 17** for the **Maven / TeaVM** step: Gradle resolves it via the [Foojay toolchain resolver](https://docs.gradle.org/current/userguide/toolchains.html#sub:download-repositories) (see `settings.gradle.kts` `org.gradle.toolchains.foojay-resolver-convention` **1.0.0**). TeaVM 0.2.x cannot parse JDK 21 bootstrap classes when compiling to Wasm.
- **Gradle 8+** or a **Gradle wrapper** in the game project.
- **wit-bindgen-cli 0.40.x** for `teavm-java` codegen (see [WIT_VERSIONS.md](WIT_VERSIONS.md)).
- **wasm-tools** (pinned in [WIT_VERSIONS.md](WIT_VERSIONS.md)) for the component embed post-step.

## Build the `java-game` library

```bash
cd game
gradle test
```

## Build upload-ready `logic.wasm` (TeaVM core → WebAssembly Component)

From a game’s `backend/java/` (composite build with `component/`):

```bash
gradle :component:exportLogicComponent
```

From **`component-template/`** alone:

```bash
gradle exportLogicComponent
```

Pipeline:

1. **Maven / TeaVM** → `logic-core.wasm` (core Wasm module)
2. **TeaVM `mainClass` + `classesToPreserve`** (same pattern as [spin-teavm-example](https://github.com/dicej/spin-teavm-example)): dummy **`TeaVmMain`** with `main()`, preserve generated **`wit.worlds.GameCore`** so `init` / `take-action` are not tree-shaken. Use a single `<classesToPreserve>wit.worlds.GameCore</classesToPreserve>` line (comma-separated for multiple classes; do **not** repeat nested `<classesToPreserve>` elements). Do **not** vendor stub `org.teavm.interop.*` sources — they shadow the real TeaVM backend and break codegen. The guest must use **`TeaVmGameSerde`** (not `GameSerdeFactory` / Jackson): calling Jackson into a preserved-export graph triggers a TeaVM `UnusedFunctionElimination` NPE.
3. **`wasm-tools component embed`** → `logic-embedded.wasm`
4. **`wasm-tools component new --adapt wasi_snapshot_preview1=…`** → upload-ready `logic.wasm` (see [`component-template/wasm/README.md`](component-template/wasm/README.md))

Helper scripts: [`../scripts/package-java-component.sh`](../scripts/package-java-component.sh) / `.ps1`.

Validate locally (same checks as upload):

```bash
gamedev validate --project-dir /path/to/game
# or
wasm-tools component wit component/build/out/logic.wasm
```

Regenerate WIT Java glue after editing the WIT file (e.g. framework `test.wit`):

```bash
gradle generateWitBindings
# or: wit-bindgen teavm-java ../../../test.wit --world game-core --out-dir src/generated/java
```

### Version notes

- **Jackson** is pinned to **2.14.x** in the component `pom.xml`. Newer Jackson JARs ship Java 21 multi-release classes (bytecode major 65) that TeaVM 0.2.x cannot load.
- **Game library** and **Maven compiler** target **Java 17** bytecode so the TeaVM compiler accepts the classpath.

## `gamedev-cli`

Games with `backend = "java"` get `backend/java/` from `gamedev init --backend java`.

- `gamedev build` runs `:component:exportLogicComponent` and packages upload-ready `logic.wasm` into `dist/game.zip`.
- `gamedev test` runs Gradle compile, `exportLogicComponent`, and component validation when `wasm-tools` is on PATH.
- `gamedev validate` dry-runs upload rules on `logic.wasm`.
