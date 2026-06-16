## Game skeleton

Workflow (from project root):

1. `gamedev doctor` — verify layout, client pages, and toolchain
2. `gamedev codegen` — generate typed client bindings from your game types
3. `gamedev test` — run logic / workspace tests
4. `gamedev build` — produce `dist/game.zip` for the framework (runs codegen)
5. `gamedev login --display-name <name> --password <pass>` (or `--publish-token` from lobby)
6. `gamedev deploy` (add `--publish` to go live; `--profile prod` for production)

You only edit **game logic** (Rust `GameCore` / Java `GameRules`) and **client UI** against generated types. WASM wiring and the WebSocket protocol are handled by the SDKs.

### Layout matrix

| Backend | Frontend | Layout | Deploy-ready |
|---------|----------|--------|--------------|
| Rust | JS / TS | nested (`backend/rust/*`, `frontend/web`) | Yes |
| Rust | Bevy | **flat** (`logic/`, `component/`, `bevy/`, `tests/`) | Yes |
| Rust | Dioxus | nested (`backend/rust/*`, `frontend/dioxus`) | Yes |
| Java | JS / TS | nested (`backend/java/`, `frontend/web`) | Yes (after component export) |

Unsupported in `init` today: C#, C++, Unity, Godot, Three.js.

### SDK surfaces

| Layer | Package | Your code |
|-------|---------|-----------|
| Server logic | `game` crate / `sk.upjs.gdd:game` | Rules, state, scoring |
| Client (JS/TS) | `@upjs-gdd/game-sdk` + generated types | UI only |
| Client (Bevy) | `upjs-gdd-bevy` | Rendering + typed events |
| Tooling | `gamedev-cli` | build, deploy, codegen |

### JS / TS frontend

- Sources in `frontend/web/` use `@upjs-gdd/game-sdk` for live play
- `gamedev codegen` writes `frontend/web/src/generated/` and `generated/schema/`
- `build` runs `npm run build` when possible and merges into `client/`

### Rust + Bevy

- Flat or nested workspace; `build` runs `wasm-bindgen` into `client/`
- Play client connects via lobby URL params (`ws`, `id`, `player`)

### Java + JS / TS

- `backend/java/` exports `logic.wasm` via Gradle `exportLogicComponent`
- Types codegen via Gradle `:game:exportJsonSchema` when configured
