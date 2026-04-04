/**
 * Loose JSON values as exchanged on the WebSocket and in postMessage payloads.
 * Games deserialize into stricter shapes in their own modules.
 */
export type JsonValue =
  | null
  | boolean
  | number
  | string
  | JsonValue[]
  | { [key: string]: JsonValue };
