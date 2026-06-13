# Agent instructions — UPJŠ GDD Platform

Multiplayer web game platform: Rust/Actix server, Dioxus WASM lobby, WASM game logic via Wasmtime. See [README.md](README.md) for architecture and setup.

## UI tasks — use Browser MCP

When the **cursor-ide-browser** MCP server is available, **always use it for UI tasks**. Do not rely on code inspection alone for layout, styling, interaction, or visual regressions.

UI tasks include:

- Lobby SPA (`lobby/`) — Dioxus + Tailwind
- Game client HTML/CSS/JS under `games/*/client/`
- Design tokens and mockups in `docs/ui-proposal/`
- Verifying pages served locally (`http://127.0.0.1:8080` lobby, `:8081` backend)

### Workflow

1. Start dev servers if needed (`.\scripts\dev-backend.ps1`, then `.\scripts\dev-lobby.ps1` or `dx serve` in `lobby/`).
2. Use `browser_navigate` to open the target URL.
3. Use `browser_snapshot` and `browser_take_screenshot` to inspect the page before and after changes.
4. Use browser interaction tools (`browser_click`, `browser_fill`, `browser_scroll`, etc.) to reproduce bugs and verify fixes.
5. Follow lock order: `browser_navigate` → `browser_lock` → interactions → `browser_lock` unlock.

If Browser MCP is unavailable, say so in the summary and fall back to static analysis — but prefer enabling it for any UI work.

## Local development

| Task | Command |
|------|---------|
| Backend (dev) | `.\scripts\dev-backend.ps1` — Actix on `:8081` |
| Lobby (dev) | `.\scripts\dev-lobby.ps1` — Dioxus on `:8080`, proxies API to `:8081` |
| Server only | `cargo run -p server` |
| Tests | `cargo test` |
| Build game WASM | `cargo component build -p <game>_component --release` |

**Important:** Run `dx serve` only from `lobby/`, not from the workspace root or `server/`.

## Code conventions

- Match existing patterns in the crate you are editing; keep diffs minimal.
- Lobby UI follows the "Calm & Credible" design system (`docs/ui-proposal/`, `lobby/tailwind.config.js`).
- Do not commit secrets (`.env`, OAuth credentials, deploy keys).
- Only create git commits when explicitly asked.

## Key paths

| Path | Purpose |
|------|---------|
| `server/` | Actix backend, GraphQL, game WebSocket, Wasmtime |
| `lobby/` | Dioxus lobby SPA |
| `games/` | Published and example game bundles |
| `sdk/` | Game SDK (Rust, Java, JS) |
| `tools/gamedev-cli/` | Developer CLI (`init`, `build`, `deploy`) |
| `docs/refactor-plan.md` | Living roadmap |
