/**
 * Networked play client — uses @upjs-gdd/game-sdk (wired by gamedev build).
 */
import { GameClient } from "@upjs-gdd/game-sdk";
import type { Action, PlayerState } from "./generated/PlayerState.js";

function decodePlayerParam(raw: string | null): string | null {
  if (raw == null || raw === "") return null;
  const t = raw.trim();
  if ((t.startsWith('"') && t.endsWith('"')) || (t.startsWith("'") && t.endsWith("'"))) {
    try {
      return JSON.parse(t.startsWith("'") ? `"${t.slice(1, -1)}"` : t) as string;
    } catch {
      return t.slice(1, -1);
    }
  }
  return t;
}

function extractState(payload: unknown): PlayerState | Record<string, unknown> {
  let v: unknown = payload;
  for (let i = 0; i < 3 && v && typeof v === "object" && v !== null && "state" in v; i++) {
    v = (v as { state: unknown }).state;
  }
  return (v && typeof v === "object" ? v : payload) as PlayerState;
}

function renderBoard(
  view: PlayerState | Record<string, unknown>,
  myPlayer: string | null,
  client: GameClient,
): void {
  const app = document.getElementById("app");
  if (!app) return;
  const board = (view as PlayerState).view?.board ?? [];
  const side = Math.round(Math.sqrt(board.length)) || 3;
  const cells = board.map((c, i) => {
    const mark = c != null ? String(c) : "";
    const row = Math.floor(i / side);
    const col = i % side;
    return `<button data-r="${row}" data-c="${col}" style="width:48px;height:48px;font-size:20px;">${mark}</button>`;
  });
  const current = (view as PlayerState).view?.current_player ?? "?";
  app.innerHTML = `
    <h2>Play</h2>
    <p>You: ${myPlayer ?? "?"} · Turn: ${String(current)}</p>
    <div style="display:grid;grid-template-columns:repeat(${side},48px);gap:4px;">${cells.join("")}</div>`;
  for (const btn of app.querySelectorAll("button[data-r]")) {
    btn.addEventListener("click", () => {
      const row = Number((btn as HTMLButtonElement).dataset.r);
      const col = Number((btn as HTMLButtonElement).dataset.c);
      const action: Action = { Place: { row, col } };
      client.sendAction(action);
    });
  }
}

const params = new URLSearchParams(window.location.search);
const myPlayer = decodePlayerParam(params.get("player"));
const client = GameClient.fromUrlParams();

client
  .onState((data) => {
    renderBoard(extractState(data), myPlayer, client);
  })
  .onEvent((ev) => {
    if (ev && typeof ev === "object" && ev !== null && "GameOver" in ev) {
      document.getElementById("app")!.innerHTML += `<p><strong>Game over</strong></p>`;
      return;
    }
    const view = extractState(ev);
    if (view && typeof view === "object" && "view" in view) {
      renderBoard(view, myPlayer, client);
    }
  })
  .onError(() => {
    document.getElementById("app")!.textContent = "Connection error";
  });
