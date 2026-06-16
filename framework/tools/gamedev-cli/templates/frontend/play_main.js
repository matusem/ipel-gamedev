/**
 * Networked play client — uses @upjs-gdd/game-sdk (wired by gamedev build).
 * Edit rendering only; connection protocol is handled by the SDK.
 */
import { GameClient } from "@upjs-gdd/game-sdk";

function decodePlayerParam(raw) {
  if (raw == null || raw === "") return null;
  const t = raw.trim();
  if ((t.startsWith('"') && t.endsWith('"')) || (t.startsWith("'") && t.endsWith("'"))) {
    try {
      return JSON.parse(t.startsWith("'") ? `"${t.slice(1, -1)}"` : t);
    } catch {
      return t.slice(1, -1);
    }
  }
  return t;
}

function normalizePlayer(p) {
  if (p == null) return null;
  if (typeof p === "string") return p === "X" || p === "O" ? p : p;
  if (typeof p === "object") {
    if ("Player1" in p) return "Player1";
    if ("Player2" in p) return "Player2";
    if ("X" in p) return "X";
    if ("O" in p) return "O";
  }
  return String(p);
}

function extractState(payload) {
  let v = payload;
  for (let i = 0; i < 3 && v && typeof v === "object" && "state" in v; i++) {
    v = v.state;
  }
  return v && typeof v === "object" ? v : payload;
}

function renderBoard(view, myPlayer, client) {
  const app = document.getElementById("app");
  if (!app) return;
  const board = view.board || [];
  const side = Math.round(Math.sqrt(board.length)) || 3;
  const cells = board.map((c, i) => {
    const mark = normalizePlayer(c) || "";
    const row = Math.floor(i / side);
    const col = i % side;
    return `<button data-r="${row}" data-c="${col}" style="width:48px;height:48px;font-size:20px;">${mark}</button>`;
  });
  app.innerHTML = `
    <h2>Play</h2>
    <p>You: ${myPlayer ?? "?"} · Turn: ${normalizePlayer(view.current_player) ?? "?"}</p>
    <div style="display:grid;grid-template-columns:repeat(${side},48px);gap:4px;">${cells.join("")}</div>`;
  for (const btn of app.querySelectorAll("button[data-r]")) {
    btn.addEventListener("click", () => {
      const row = Number(btn.dataset.r);
      const col = Number(btn.dataset.c);
      client.sendAction({ Place: { row, col } });
    });
  }
}

const params = new URLSearchParams(window.location.search);
const myPlayer = decodePlayerParam(params.get("player"));
const client = GameClient.fromUrlParams();

client
  .onState((data) => {
    const view = extractState(data);
    renderBoard(view, myPlayer, client);
  })
  .onEvent((ev) => {
    if (ev && typeof ev === "object" && "GameOver" in ev) {
      document.getElementById("app").innerHTML += `<p><strong>Game over</strong></p>`;
      return;
    }
    const view = extractState(ev);
    if (view && view.board) renderBoard(view, myPlayer, client);
  })
  .onError(() => {
    document.getElementById("app").textContent = "Connection error";
  });
