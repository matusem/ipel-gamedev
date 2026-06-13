/**
 * @upjs-gdd/game-sdk — shared browser utilities for UPJŠ GDD game iframes:
 * - WebSocket `GameClient` (play)
 * - Lobby `postMessage` bridge (config)
 * - Wire helpers and protocol constants
 *
 * Reference integration: `games/tic_tac_toe/web` (Vite + TypeScript).
 */
export {
  CONFIG_MSG_SOURCE,
  CONFIG_RESULT_SOURCE,
  CONFIG_SCHEMA_SOURCE,
  CONFIG_STATE_SOURCE,
  LOBBY_GAME_ORIGIN,
} from "./protocol.js";
export { GameClient } from "./game-client.js";
export { SpectatorClient } from "./spectator-client.js";
export { LobbyConfigBridge } from "./lobby-config-bridge.js";
export { readResultPayload } from "./result-payload.js";
export {
  formatGameOverLine,
  parseGameOverTag,
  playerFromParams,
} from "./wire.js";
export type { JsonValue } from "./types.js";
export type { LobbyConfigBridgeOptions } from "./lobby-config-bridge.js";
