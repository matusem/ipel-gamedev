# Server (Actix backend)

Native Rust HTTP server: GraphQL, game WebSocket, WASM logic, static `/games` assets.

**Do not run `dx serve` here.** That command builds a Dioxus **WASM** client and will fail on this crate (Tokio/mio do not target `wasm32`).

| Goal | Command |
|------|---------|
| Run backend (production / single port) | `cargo run -p server` → `:8080` |
| Run backend for lobby dev | `PORT=8081 cargo run -p server` or `..\scripts\dev-backend.ps1` |
| Run lobby with hot reload | `cd ../lobby && dx serve --platform web` or `..\scripts\dev-lobby.ps1` |

See [../README.md](../README.md) — **Local development**.
