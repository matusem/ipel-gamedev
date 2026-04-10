# Java game logic (Fermyon TeaVM + `GameCore` WIT)

This directory contains:

- **`game/`** — portable library mirroring the Rust [`game`](../../game) crate (`GameRules`, `GameOrchestrator`, serde helpers). Compiles to **Java 17 bytecode** (via `--release 17`) while Gradle may run on **JDK 21+**.
- **`component-template/`** — **Tic-tac-toe** logic matching the Rust flat template (`rust_logic_flat_lib.rs`), wired through [`GameCoreImpl`](component-template/src/main/java/wit/worlds/GameCoreImpl.java) and the generated WIT bindings under `src/generated/java/`. The guest is built with **[fermyon/teavm-wasi](https://github.com/fermyon/teavm-wasi)** (`com.fermyon` on Maven Central), orchestrated by **Gradle** + **`pom.xml`** + the **Maven Wrapper** (`mvnw` / `mvnw.cmd`). JSON/MessagePack shapes follow the same serde conventions as the Rust game.

## Prerequisites

- **JDK 21+** for running Gradle (set `JAVA_HOME`; see [Gradle JVM requirements](https://docs.gradle.org/current/userguide/compatibility.html)).
- **JDK 17** for the **Maven / TeaVM** step: Gradle resolves it via the [Foojay toolchain resolver](https://docs.gradle.org/current/userguide/toolchains.html#sub:download-repositories) (see `settings.gradle.kts` `org.gradle.toolchains.foojay-resolver-convention` **1.0.0**). TeaVM 0.2.x cannot parse JDK 21 bootstrap classes when compiling to Wasm.
- **Gradle 8+** or a **Gradle wrapper** in the game project.
- **wit-bindgen-cli 0.40.x** for `teavm-java` codegen (see [WIT_VERSIONS.md](WIT_VERSIONS.md)).

## Build the `java-game` library

```bash
cd game
gradle test
```

## Build `logic.wasm` (Fermyon TeaVM)

From a game’s `backend/java/` (composite build with `component/`):

```bash
gradle :component:exportLogicWasm
```

From **`component-template/`** alone (no `component/` subproject; build file is at the template root):

```bash
gradle exportLogicWasm
```

Gradle runs **`mvnw package`** with `JAVA_HOME` set to **JDK 17**, using [`component-template/pom.xml`](component-template/pom.xml) (`com.fermyon:teavm-maven-plugin`, `targetType` **WEBASSEMBLY**). Staged output: **`component/build/out/logic.wasm`** (copied from Maven’s `target/generated/wasm/teavm-wasm/classes.wasm`).

Regenerate WIT Java glue after editing the WIT file (e.g. framework `test.wit`):

```bash
gradle generateWitBindings
```

### Version notes

- **Jackson** is pinned to **2.14.x** in the component `pom.xml`. Newer Jackson JARs ship Java 21 multi-release classes (bytecode major 65) that TeaVM 0.2.x cannot load.
- **Game library** and **Maven compiler** target **Java 17** bytecode so the TeaVM compiler accepts the classpath.

## `gamedev-cli`

Games with `backend = "java"` get `backend/java/` from `gamedev init --backend java`. `gamedev build` runs `:component:exportLogicWasm` (when `component/build.gradle.kts` exists) and packages `logic.wasm` into `dist/game.zip`.

### Deployment / server upload

The lobby upload path may expect a **WebAssembly Component** produced by **`cargo component build`** (same as Rust games in this repo).

**Fermyon TeaVM** here emits a **core** Wasm module with **WASI snapshot** imports and TeaVM GC/runtime imports — not the same artifact as `cargo component build`. Wasmtime’s component APIs and strict validators may reject it.

Use the Java backend for **local parity and tooling**; for **upload-ready** packs, prefer **`backend = "rust"`** and a `cargo-component` crate, unless you add a separate componentization step.
