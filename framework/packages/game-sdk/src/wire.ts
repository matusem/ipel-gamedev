import type { JsonValue } from "./types.js";

/**
 * Normalise `player` from URL search params.
 * The server and `GameClient` expect a non-empty string; default matches historical JS.
 */
export function playerFromParams(params: URLSearchParams, fallback = "1"): string {
  return params.get("player")?.trim() || fallback;
}

/**
 * Parse `GameOver` payloads that mirror Rust enums tagged with struct names, e.g.:
 * `{ "Winner": "1" }`, `{ "Draw": null }`, `{ "Forfeit": { "winner": "2", "loser": "1" } }`.
 */
export function parseGameOverTag(
  raw: JsonValue | undefined,
): { tag: string; inner: JsonValue | undefined } | null {
  if (raw === null || raw === undefined || typeof raw !== "object" || Array.isArray(raw)) {
    return null;
  }
  const entries = Object.entries(raw as Record<string, JsonValue>);
  if (entries.length !== 1) return null;
  const [tag, inner] = entries[0]!;
  if (typeof tag !== "string" || !tag) return null;
  return { tag, inner };
}

/**
 * Human-readable line for lobby/result screens from a parsed `GameOver` tag.
 */
export function formatGameOverLine(
  parsed: { tag: string; inner: JsonValue | undefined },
  you: string,
): string {
  switch (parsed.tag) {
    case "Winner":
      return String(parsed.inner) === you ? "You won." : "You lost.";
    case "Draw":
      return "Draw.";
    case "Forfeit": {
      const inner = parsed.inner;
      if (inner && typeof inner === "object" && !Array.isArray(inner)) {
        const w = (inner as Record<string, JsonValue>).winner;
        return String(w) === you ? "You won (opponent forfeited)." : "You forfeited.";
      }
      return "Game ended (forfeit).";
    }
    default:
      return `Game over (${parsed.tag}).`;
  }
}
