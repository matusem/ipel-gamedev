/**
 * PostMessage `source` strings shared with the Dioxus lobby (`lobby/src/main.rs`).
 *
 * **Config iframe (e.g. `config.html`)**
 * - Lobby → iframe: `upjs-gdd-game-config-schema` (JSON Schema as a parsed object, plus `game`).
 * - Lobby → iframe: `upjs-gdd-game-config-state` (saved config object or null, plus `game`).
 * - iframe → Lobby: `upjs-gdd-game-config` (preview JSON string or object in `config`, plus `game`).
 * - Lobby → iframe: `upjs-gdd-game-config-result` (`ok`, `errors[]`) after each preview — **not** used iframe→Lobby.
 *
 * **Play iframe** uses the WebSocket protocol (`GameClient`), not these constants.
 *
 * **Result iframe** uses query param `payload` (URI-encoded JSON); see `readResultPayload`.
 */
export const LOBBY_GAME_ORIGIN = "*" as const;

/** Parent → iframe: JSON Schema as a JS object (`schema` field). */
export const CONFIG_SCHEMA_SOURCE = "upjs-gdd-game-config-schema" as const;

/** Parent → iframe: last saved config as object or null (`config` field). */
export const CONFIG_STATE_SOURCE = "upjs-gdd-game-config-state" as const;

/** iframe → parent: draft / preview config (`config` string or object). */
export const CONFIG_MSG_SOURCE = "upjs-gdd-game-config" as const;

/** Parent → iframe: validation outcome after a preview message. */
export const CONFIG_RESULT_SOURCE = "upjs-gdd-game-config-result" as const;
