import type { JsonValue } from "./types.js";

/**
 * Read `?payload=<URI-encoded JSON>` as set by the lobby for the result iframe.
 * Returns `null` if missing or invalid JSON.
 */
export function readResultPayload<T extends JsonValue = JsonValue>(): T | null {
  const params = new URLSearchParams(window.location.search);
  const raw = params.get("payload");
  if (raw === null || raw === "") return null;
  try {
    return JSON.parse(decodeURIComponent(raw)) as T;
  } catch {
    return null;
  }
}
