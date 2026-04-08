import type { RealtimeEnvelope } from "../generated-types/index";

export type RealtimeState = "disconnected" | "connecting" | "connected" | "reconnecting";

export type RealtimeCallbacks = {
  onEvent?: (event: RealtimeEnvelope) => void;
  onStateChange?: (state: RealtimeState) => void;
};

export class RealtimeClient {
  private ws: WebSocket | null = null;
  private retry = 0;
  private state: RealtimeState = "disconnected";

  constructor(
    private readonly wsUrl: string,
    private readonly bearerToken: string,
    private readonly callbacks: RealtimeCallbacks = {}
  ) {}

  connect() {
    this.setState(this.retry === 0 ? "connecting" : "reconnecting");
    const url = `${this.wsUrl}${this.wsUrl.includes("?") ? "&" : "?"}token=${encodeURIComponent(this.bearerToken)}`;
    this.ws = new WebSocket(url);
    this.ws.onopen = () => {
      this.retry = 0;
      this.setState("connected");
    };
    this.ws.onmessage = (msg) => {
      try {
        this.callbacks.onEvent?.(JSON.parse(String(msg.data)) as RealtimeEnvelope);
      } catch {
        // ignore malformed payloads
      }
    };
    this.ws.onclose = () => {
      this.setState("reconnecting");
      const waitMs = Math.min(1000 * (2 ** this.retry), 10_000);
      this.retry += 1;
      setTimeout(() => this.connect(), waitMs);
    };
  }

  disconnect() {
    this.retry = 0;
    this.ws?.close();
    this.ws = null;
    this.setState("disconnected");
  }

  private setState(next: RealtimeState) {
    this.state = next;
    this.callbacks.onStateChange?.(next);
  }
}
