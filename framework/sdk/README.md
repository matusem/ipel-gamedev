# SDKs — developer surface only

Game developers interact with **SDKs and `gamedev-cli`**, not WASM wiring or the raw WebSocket protocol.

## Packages

| Package | Role |
|---------|------|
| [`game/`](../game/) + [`game-wasm-host/`](../game-wasm-host/) | Rust server logic (`GameCore`) — CLI builds `logic.wasm` |
| [`sdk/java/`](java/) | Java server logic (`GameRules`) — TeaVM → `logic.wasm` |
| [`packages/game-sdk/`](../packages/game-sdk/) | Browser play client (`GameClient`, `TypedGameClient`, lobby bridge) |
| [`sdk/js/`](js/) | GraphQL/deploy tooling (`@upjs-gdd/sdk-js`) |
| [`sdk/rust/shared-types`](rust/shared-types) | Canonical per-game types (Rust); export TS + JSON Schema |
| [`sdk/rust/bevy`](rust/bevy) | Bevy play plugin (`FrameworkGamePlayPlugin`) |
| [`sdk/rust/dioxus`](rust/dioxus) | Dioxus hooks (GraphQL realtime) |

## Workflow

1. **Define types** in `backend/rust/shared-types` (or Java `game` module).
2. **`gamedev codegen`** — emits `frontend/web/src/generated/` and `generated/schema/`.
3. **Implement logic** — Rust `GameCore` or Java `GameRules` (rules, scoring, results).
4. **Implement UI** — JS/TS with `@upjs-gdd/game-sdk` + generated types, or Bevy with `upjs-gdd-bevy`.
5. **`gamedev build`** / **`gamedev deploy`**.

## Type generation

**Rust backend:** `ts-rs` + `schemars` on shared-types (`typegen`, `schemars` features).

**Java backend:** Gradle `:game:exportJsonSchema` → JSON Schema IR → same client codegen path.

**Clients:** `@upjs-gdd/game-sdk` `TypedGameClient` / `readTypedResultPayload` wrap the wire protocol with your generated types.
