import {
  CONFIG_MSG_SOURCE,
  CONFIG_RESULT_SOURCE,
  CONFIG_SCHEMA_SOURCE,
  CONFIG_STATE_SOURCE,
} from "./protocol.js";
import type { JsonValue } from "./types.js";

export interface LobbyConfigBridgeOptions<TConfig extends JsonValue = JsonValue> {
  /** Same string as server/lobby game type (e.g. `tic_tac_toe`). */
  gameType: string;
  /** Lobby sent JSON Schema as a parsed object; validate drafts locally if needed. */
  onSchema: (schema: JsonValue) => void;
  /** Saved config from server, or null for default. */
  onState: (config: TConfig | null) => void;
  /** Lobby replied after `sendPreview` — update error UI. */
  onValidationReply: (ok: boolean, errors: string[]) => void;
}

/**
 * Wires a **config** iframe to the lobby `postMessage` flow (`lobby/src/main.rs`).
 *
 * - Origin must match `window.location.origin` (same as the lobby listener).
 * - Preview updates: call `sendPreview` with JSON text or an object; the lobby parses JSON and answers with
 *   `ipel-game-config-result` (`ok` / `errors`).
 * - Saving to the server is done in the lobby (**Apply configuration**); the iframe only pushes previews.
 */
export class LobbyConfigBridge<TConfig extends JsonValue = JsonValue> {
  private readonly gameType: string;
  private readonly onSchema: (schema: JsonValue) => void;
  private readonly onState: (config: TConfig | null) => void;
  private readonly onValidationReply: (ok: boolean, errors: string[]) => void;
  private boundMessage: ((ev: MessageEvent) => void) | null = null;

  constructor(opts: LobbyConfigBridgeOptions<TConfig>) {
    this.gameType = opts.gameType;
    this.onSchema = opts.onSchema;
    this.onState = opts.onState;
    this.onValidationReply = opts.onValidationReply;
  }

  attach(): void {
    if (this.boundMessage) return;
    const origin = window.location.origin;
    this.boundMessage = (ev: MessageEvent) => {
      if (ev.origin !== origin) return;
      const d = ev.data;
      if (typeof d !== "object" || d === null) return;
      const msg = d as Record<string, unknown>;
      if (msg.game !== this.gameType) return;

      if (msg.source === CONFIG_SCHEMA_SOURCE) {
        this.onSchema(msg.schema as JsonValue);
        return;
      }
      if (msg.source === CONFIG_STATE_SOURCE) {
        const c = msg.config;
        if (c === null || c === undefined) {
          this.onState(null);
        } else {
          this.onState(c as TConfig);
        }
        return;
      }
      if (msg.source === CONFIG_RESULT_SOURCE) {
        const ok = Boolean(msg.ok);
        const errorsRaw = msg.errors;
        const errors = Array.isArray(errorsRaw)
          ? errorsRaw.filter((x): x is string => typeof x === "string")
          : [];
        this.onValidationReply(ok, errors);
      }
    };
    window.addEventListener("message", this.boundMessage);
  }

  detach(): void {
    if (this.boundMessage) {
      window.removeEventListener("message", this.boundMessage);
      this.boundMessage = null;
    }
  }

  /**
   * Push current draft JSON to the lobby (stringified object or object — lobby accepts both).
   */
  sendPreview(config: TConfig | string): void {
    const configPayload =
      typeof config === "string" ? config : JSON.stringify(config);
    window.parent.postMessage(
      {
        source: CONFIG_MSG_SOURCE,
        game: this.gameType,
        config: configPayload,
      },
      window.location.origin,
    );
  }
}
