/**
 * Read-only WebSocket client for spectator mode (`/game?id=…&spectator=1`).
 *
 * Same framing as {@link GameClient}: first inbound text frame is initial spectator state JSON;
 * later frames are spectator events. Inbound actions are not sent (server ignores them).
 */
export class SpectatorClient {
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
   * Requires URL search params: `ws` (WebSocket base/path), `id` (game id).
   * Adds `spectator=1` automatically.
   */
  static fromUrlParams(): SpectatorClient {
    const params = new URLSearchParams(window.location.search);
    const wsBase = params.get("ws");
    const gameId = params.get("id");

    if (!wsBase || !gameId) {
      throw new Error(
        `Missing required URL params: ws, id. Got: ws=${wsBase}, id=${gameId}`,
      );
    }

    const url = `${wsBase}?id=${encodeURIComponent(gameId)}&spectator=1`;
    return new SpectatorClient(url);
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
