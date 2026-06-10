# Architecture

## Components

| Crate / dir | Role |
|-------------|------|
| `server/` | Actix HTTP server — GraphQL, game WS, static assets, Wasmtime |
| `lobby/` | Dioxus WASM SPA — discovery, lobbies, developer uploads |
| `game/` | Framework-agnostic game trait (`GameCore`, `Config`, …) |
| `game-wasm-host/` | WIT bridge exporting game traits as WASM components |
| `packages/game-sdk/` | Browser SDK — `/game` WebSocket + config iframe bridge |
| `tools/gamedev-cli/` | Scaffold, build, deploy toolchain |
| `sdk/rust/shared-types/` | Canonical API types (+ TS export) |
| `games/` | Reference implementations (tic-tac-toe, checkers) |

## Game lifecycle

1. **Registry** — server scans `GAMES_DIR/*/manifest.json`, loads `logic.wasm` into Wasmtime
2. **Lobby** — owner creates room, picks game type, configures via iframe `postMessage`
3. **Start** — `startLobby` mutation spawns WASM instance + DB record
4. **Play** — clients connect to `/game?id=…&player=…`, send actions, receive events
5. **Finish** — terminal state persisted; result iframe shown at `/game/:id`

## Real-time channels

- **Platform** — GraphQL subscriptions on `/graphql` (graphql-ws): `lobbiesUpdated`, `gameInstancesUpdated`, `lobbyUpdated`
- **In-game** — dedicated WebSocket at `/game` (JSON text frames, not GraphQL)

## Lobby frontend structure

```
lobby/src/
  main.rs           # Router, pages (being split — see refactor-plan.md)
  models.rs         # Shared types and helpers
  api/              # GraphQL client, subscriptions, config iframe bridge
  components/       # AppShell, UI primitives
```

## Auth model

- Guest / sign-up / password login via GraphQL mutations
- Session = user UUID stored in `localStorage`, sent as `Authorization: Bearer <uuid>`
- Developer uploads: publish tokens or `OPEN_DEVELOPER_UPLOADS=true`
