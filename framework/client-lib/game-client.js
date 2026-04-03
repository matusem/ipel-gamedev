/**
 * Game client library for connecting to the game server via WebSocket.
 *
 * Usage from a game iframe:
 *   const client = GameClient.fromUrlParams();
 *   client.onState(state => console.log("Initial state:", state));
 *   client.onEvent(event => console.log("Event:", event));
 *   client.sendAction({ row: 0, col: 1 });
 */
class GameClient {
  #ws;
  #stateCallback = null;
  #eventCallback = null;
  #errorCallback = null;
  #closeCallback = null;
  #receivedInitialState = false;
  /** @type {unknown[]} */
  #earlyMessages = [];

  constructor(wsUrl) {
    this.#ws = new WebSocket(wsUrl);

    this.#ws.onmessage = (event) => {
      let data;
      try {
        data = JSON.parse(event.data);
      } catch {
        data = event.data;
      }

      if (this.#stateCallback === null && !this.#receivedInitialState) {
        this.#earlyMessages.push(data);
        return;
      }
      this.#deliver(data);
    };

    this.#ws.onerror = (event) => {
      if (this.#errorCallback) this.#errorCallback(event);
    };

    this.#ws.onclose = (event) => {
      if (this.#closeCallback) this.#closeCallback(event);
    };
  }

  #deliver(data) {
    if (!this.#receivedInitialState) {
      this.#receivedInitialState = true;
      if (this.#stateCallback) this.#stateCallback(data);
    } else {
      if (this.#eventCallback) this.#eventCallback(data);
    }
  }

  static fromUrlParams() {
    const params = new URLSearchParams(window.location.search);
    const wsBase = params.get("ws");
    const gameId = params.get("id");
    const player = params.get("player");

    if (!wsBase || !gameId || !player) {
      throw new Error(
        "Missing required URL params: ws, id, player. " +
          `Got: ws=${wsBase}, id=${gameId}, player=${player}`
      );
    }

    const url = `${wsBase}?id=${encodeURIComponent(gameId)}&player=${encodeURIComponent(player)}`;
    return new GameClient(url);
  }

  onState(callback) {
    this.#stateCallback = callback;
    for (const data of this.#earlyMessages) {
      this.#deliver(data);
    }
    this.#earlyMessages.length = 0;
    return this;
  }

  onEvent(callback) {
    this.#eventCallback = callback;
    return this;
  }

  onError(callback) {
    this.#errorCallback = callback;
    return this;
  }

  onClose(callback) {
    this.#closeCallback = callback;
    return this;
  }

  sendAction(action) {
    const payload = typeof action === "string" ? action : JSON.stringify(action);
    if (this.#ws.readyState === WebSocket.OPEN) {
      this.#ws.send(payload);
    }
  }

  close() {
    this.#ws.close();
  }

  get ready() {
    return new Promise((resolve) => {
      if (this.#ws.readyState === WebSocket.OPEN) {
        resolve();
      } else {
        this.#ws.addEventListener("open", () => resolve(), { once: true });
      }
    });
  }
}
