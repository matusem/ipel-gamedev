import type { JsonValue } from "./types.js";
import { readResultPayload } from "./result-payload.js";
import { formatGameOverLine, parseGameOverTag } from "./wire.js";

export interface TypedFinishedGame<TPlayerResult = JsonValue> {
  raw: JsonValue;
  gameOverLine: string | null;
  gameOverTag: string | null;
  playerResults: TPlayerResult[];
}

export interface TypedResultHandlers<TPlayerResult> {
  parsePlayerResult?: (raw: JsonValue) => TPlayerResult;
}

/**
 * Read and parse the finished-game payload for `result.html` using generated result types.
 */
export function readTypedResultPayload<TPlayerResult = JsonValue>(
  handlers: TypedResultHandlers<TPlayerResult> = {},
): TypedFinishedGame<TPlayerResult> | null {
  const raw = readResultPayload();
  if (raw == null) {
    return null;
  }
  const parse =
    handlers.parsePlayerResult ?? ((v: JsonValue) => v as TPlayerResult);
  const playerResults: TPlayerResult[] = [];
  if (typeof raw === "object" && raw !== null && "player_scores" in raw) {
    const scores = (raw as { player_scores?: JsonValue }).player_scores;
    if (Array.isArray(scores)) {
      for (const entry of scores) {
        playerResults.push(parse(entry as JsonValue));
      }
    }
  }
  const gameOverLine = formatGameOverLine(raw);
  const gameOverTag = parseGameOverTag(raw);
  return {
    raw: raw as JsonValue,
    gameOverLine,
    gameOverTag,
    playerResults,
  };
}
