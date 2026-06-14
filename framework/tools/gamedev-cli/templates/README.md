## Game skeleton

Workflow (from project root):

1. `gamedev doctor` — verify layout, client pages, and toolchain
2. `gamedev test` — run logic / workspace tests
3. `gamedev build` — produce `dist/game.zip` for the framework
4. `gamedev login --user-id <uuid>`
5. `gamedev deploy --draft-only` (or `--auto-publish`)

### Layout matrix

| Backend | Frontend | Layout | Deploy-ready |
|---------|----------|--------|--------------|
| Rust | JS / TS | nested (`backend/rust/*`, `frontend/web`) | Yes |
| Rust | Bevy | **flat** (`logic/`, `component/`, `bevy/`, `tests/`) | Yes |
| Rust | Dioxus | nested (`backend/rust/*`, `frontend/dioxus`) | Yes |
| Java | JS / TS | nested (`backend/java/`, `frontend/web`) | Yes (after component export) |

Unsupported in `init` today: C#, C++, Unity, Godot, Three.js.

### Rust + Bevy (flat layout)

- Workspace: `logic`, `component`, `bevy`, `tests`, `client/`
- `build` compiles the Wasm component and runs `wasm-bindgen` for the Bevy play client into `client/`
- Install once if `doctor` warns: `cargo install wasm-bindgen-cli`, `rustup target add wasm32-unknown-unknown`

### Rust + Bevy (nested layout)

- Workspace: `backend/rust/*`, `frontend/bevy`, `client/`
- `build` compiles the Wasm component and runs `wasm-bindgen` for the Bevy web client into `client/`
- When created under the framework tree, `frontend/bevy` links `upjs-gdd-bevy` automatically

### Rust + Dioxus (nested layout)

- Workspace: `backend/rust/*`, `frontend/dioxus`, `client/`
- `build` compiles the Wasm component and runs `wasm-bindgen` for the Dioxus web client into `client/`
- When created under the framework tree, `frontend/dioxus` links `upjs-gdd-dioxus` automatically

### Java + JS / TS

- Workspace: `backend/java/` (TeaVM guest), `frontend/web`, `client/`
- `build` runs Gradle `exportLogicComponent` and packages upload-ready `logic.wasm`
- See `framework/sdk/java/README.md` for JDK / wit-bindgen toolchain pins

### JS / TS frontend

- Sources live in `frontend/web/`
- `build` runs `npm run build` when possible and merges output into `client/` for packaging
- When created under the framework tree, `package.json` links `@upjs-gdd/sdk-js` via a local `file:` dependency
