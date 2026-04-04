/**
 * WebSocket client for the framework game protocol (see also `client-lib/game-client.js` history).
 *
 * 1. Connect to the URL built from query params (`ws`, `id`, `player`) or pass a full URL.
 * 2. The first inbound text frame is delivered to `onState` as parsed JSON (typically `{ state: ... }`).
 * 3. Later frames go to `onEvent`.
 * 4. Outbound: the wire body is **only** `JSON.stringify(action)` — the server already knows the player
 *    from the socket registration; do **not** wrap actions in `{ player, action }`.
 *
 * Callbacks can be registered after construction; early messages are buffered until `onState` is set.
 */
export class GameClient {
  private readonly ws: WebSocket;
  private stateCallback: ((data: unknown) => void) | null = null;
  private eventCallback: ((data: unknown) => void) | null = null;
  private errorCallback: ((ev: Event) => void) | null = null;
  private closeCallback: ((ev: CloseEvent) => void) | null = null;
  private receivedInitialState = false;
  private readonly earlyMessages: unknown[] = [];

  constructor(wsUrl: string) {
    this.ws = new WebSocket(wsUrl);

    this.ws.onmessage = (event) => {
      let data: unknown;
      try {
        data = JSON.parse(String(event.data));
      } catch {
        data = event.data;
      }

      if (this.stateCallback === null && !this.receivedInitialState) {
        this.earlyMessages.push(data);
        return;
      }
      this.deliver(data);
    };

    this.ws.onerror = (event) => {
      this.errorCallback?.(event);
    };

    this.ws.onclose = (event) => {
      this.closeCallback?.(event);
    };
  }

  private deliver(data: unknown): void {
    if (!this.receivedInitialState) {
      this.receivedInitialState = true;
      this.stateCallback?.(data);
    } else {
      this.eventCallback?.(data);
    }
  }

  /**
   * Requires URL search params: `ws` (WebSocket base/path), `id` (game id), `player` (seat identity).
   */
  static fromUrlParams(): GameClient {
    const params = new URLSearchParams(window.location.search);
    const wsBase = params.get("ws");
    const gameId = params.get("id");
    const player = params.get("player");

    if (!wsBase || !gameId || !player) {
      throw new Error(
        `Missing required URL params: ws, id, player. Got: ws=${wsBase}, id=${gameId}, player=${player}`,
      );
    }

    const url = `${wsBase}?id=${encodeURIComponent(gameId)}&player=${encodeURIComponent(player)}`;
    return new GameClient(url);
  }

  onState(callback: (data: unknown) => void): this {
    this.stateCallback = callback;
    for (const data of this.earlyMessages) {
      this.deliver(data);
    }
    this.earlyMessages.length = 0;
    return this;
  }

  onEvent(callback: (data: unknown) => void): this {
    this.eventCallback = callback;
    return this;
  }

  onError(callback: (ev: Event) => void): this {
    this.errorCallback = callback;
    return this;
  }

  onClose(callback: (ev: CloseEvent) => void): this {
    this.closeCallback = callback;
    return this;
  }

  sendAction(action: unknown): void {
    const payload = typeof action === "string" ? action : JSON.stringify(action);
    if (this.ws.readyState === WebSocket.OPEN) {
      this.ws.send(payload);
    }
  }

  close(): void {
    this.ws.close();
  }

  get ready(): Promise<void> {
    return new Promise((resolve) => {
      if (this.ws.readyState === WebSocket.OPEN) {
        resolve();
      } else {
        this.ws.addEventListener("open", () => resolve(), { once: true });
      }
    });
  }
}
