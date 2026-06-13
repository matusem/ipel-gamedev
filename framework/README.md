# UPJŠ GDD Platform

Multiplayer web game platform for university deployment. Hosts WASM game logic, serves a Dioxus lobby SPA, and exposes GraphQL + WebSocket APIs for real-time play.

**Refactor plan:** see [docs/refactor-plan.md](docs/refactor-plan.md) for the living roadmap and progress tracker.

## Architecture

```
Browser
  ├── Lobby SPA (Dioxus WASM)     → /           GraphQL + subscriptions
  ├── Game client (per game)      → /games/…    static assets + iframes
  └── Game SDK                    → /game       per-player WebSocket

Server (Rust / Actix)
  ├── GraphQL API                 auth, lobbies, uploads, game instances
  ├── Game WebSocket              action/event protocol
  └── Wasmtime                    logic.wasm per published game
```

Each published game ships as:

```
manifest.json
logic.wasm
client/
  index.html      # play UI
  config.html     # lobby config iframe (optional)
  result.html     # post-game screen (optional)
  about.html      # game info (optional)
```

## Dependencies

```bash
cargo install --locked wasm-tools
cargo install --locked cargo-component
curl https://wasmtime.dev/install.sh -sSf | bash   # Linux/macOS
cargo install dioxus-cli --locked                   # lobby builds
```

## Local development

### 1. Build a reference game (tic-tac-toe)

```bash
cargo component build -p tic_tac_toe_component --release
```

Copy the output WASM and client assets into your `GAMES_DIR` (see server env vars below), or use the upload pipeline.

### 2. Run the server

```bash
cargo run -p server
```

Defaults: `http://0.0.0.0:8080`, SQLite at `./data/app.db`.

### 3. Lobby dev server (hot reload)

`dx serve` uses port **8080**. The Actix backend must use a **different** port so API calls can be proxied.

**Terminal 1 — backend** (must stay running):

```powershell
# Windows (from framework/)
.\scripts\dev-backend.ps1

# Or manually:
$env:PORT=8081; cargo run -p server
```

```bash
# Linux/macOS
PORT=8081 cargo run -p server
```

**Terminal 2 — lobby:**

```bash
cd lobby
npm ci && npm run build:css
dx serve --platform web
```

Open `http://127.0.0.1:8080`. GraphQL, game WebSocket, and `/games` assets are proxied to `:8081` via `lobby/Dioxus.toml`.

Shortcut scripts (from `framework/`):

```powershell
.\scripts\dev-backend.ps1   # terminal 1 — Actix on :8081
.\scripts\dev-lobby.ps1     # terminal 2 — Dioxus lobby on :8080
```

**Production / single-process:** build the lobby (`dx build --platform web --release`) and point `LOBBY_DIR` at the output; then only `cargo run -p server` on `:8080` is needed.

### Troubleshooting `dx serve`

| Symptom | Cause | Fix |
|---------|--------|-----|
| `Failed to find binary package` from `framework/` | Root workspace has no Dioxus app | `cd lobby` or run `.\scripts\dev-lobby.ps1` |
| `mio` / `wasm32` errors while in `server/` | `server` is **not** a web UI crate | Use `cargo run -p server`; use `dx serve` only in **`lobby/`** |
| `dx and dioxus versions are incompatible` in `server/` | `dx` picked the wrong crate; workspace SDK still uses Dioxus 0.6 | Ignore when in `server/`; run `dx` from **`lobby/`** (Dioxus 0.7.3) |
| API calls fail on `:8080` | Backend not on `:8081` | Start `dev-backend.ps1` before `dev-lobby.ps1` |

Install CLI to match the lobby crate: `cargo install dioxus-cli --locked --version 0.7.3` (or latest 0.7.x).

## Environment variables

| Variable | Default | Description |
|----------|---------|-------------|
| `HOST` | `0.0.0.0` | Bind address |
| `PORT` | `8080` | HTTP port |
| `DATABASE_URL` | `sqlite:./data/app.db` | SQLite connection |
| `GAMES_DIR` | `./games` | Published game bundles |
| `DRAFTS_DIR` | `./drafts` | Developer upload staging |
| `LOBBY_DIR` | `./lobby` | Built lobby static files (Docker copies `dx build` output here) |
| `LIB_DIR` | `./client-lib` | Legacy game-sdk IIFE (`packages/game-sdk` build output) |
| `OPEN_DEVELOPER_UPLOADS` | `true` | Allow developer uploads without role |
| `DRAFT_RETENTION_SECS` | `604800` | Draft cleanup TTL |
| `RUST_LOG` | `info,server=info` | Log filter (`debug` for game actions: `RUST_LOG=server::game_db=debug`) |

## Docker

```bash
docker build -t upjs-gdd .
docker run -p 8080:8080 \
  -v upjs-gdd-data:/app/data \
  -v upjs-gdd-games:/app/games \
  upjs-gdd
```

## Developer CLI

```bash
cargo run -p gamedev-cli -- init my_game
cargo run -p gamedev-cli -- build
cargo run -p gamedev-cli -- deploy --server http://localhost:8080
```

## Production requirements

See [docs/requirements.md](docs/requirements.md) — single instance, 4 CPU / 8 GB recommended, daily backup of SQLite + games volume.

## UI design

Design system and mockups: `docs/ui-proposal/` ("Calm & Credible"). Tokens are integrated into `lobby/tailwind.config.js`.
