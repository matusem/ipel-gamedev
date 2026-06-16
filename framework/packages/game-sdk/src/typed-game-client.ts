import type { GameClient } from "./game-client.js";
import type { JsonValue } from "./types.js";

export interface TypedGameClientHandlers<TState, TEvent, TAction> {
  onState: (state: TState) => void;
  onEvent: (event: TEvent) => void;
  onError?: (ev: Event) => void;
  onClose?: (ev: CloseEvent) => void;
  parseState?: (raw: JsonValue) => TState;
  parseEvent?: (raw: JsonValue) => TEvent;
  serializeAction?: (action: TAction) => unknown;
}

/**
 * Typed wrapper over {@link GameClient} — connection protocol stays in the SDK;
 * your game supplies parsers for generated types.
 */
export class TypedGameClient<TState, TEvent, TAction> {
  private readonly inner: GameClient;
  private serializeAction?: (action: TAction) => unknown;

  constructor(inner: GameClient) {
    this.inner = inner;
  }

  static fromUrlParams<TState, TEvent, TAction>(): TypedGameClient<TState, TEvent, TAction> {
    return new TypedGameClient(GameClient.fromUrlParams());
  }

  wire(handlers: TypedGameClientHandlers<TState, TEvent, TAction>): this {
    const parseState =
      handlers.parseState ??
      ((raw: JsonValue) => raw as TState);
    const parseEvent =
      handlers.parseEvent ??
      ((raw: JsonValue) => raw as TEvent);

    this.inner
      .onState((raw) => handlers.onState(parseState(raw as JsonValue)))
      .onEvent((raw) => handlers.onEvent(parseEvent(raw as JsonValue)));

    if (handlers.onError) {
      this.inner.onError(handlers.onError);
    }
    if (handlers.onClose) {
      this.inner.onClose(handlers.onClose);
    }
    return this;
  }

  sendAction(action: TAction): void {
    const payload = this.serializeAction ? this.serializeAction(action) : action;
    this.inner.sendAction(payload);
  }

  withSerializer(fn: (action: TAction) => unknown): this {
    this.serializeAction = fn;
    return this;
  }

  get ready(): Promise<void> {
    return this.inner.ready;
  }

  close(): void {
    this.inner.close();
  }
}
