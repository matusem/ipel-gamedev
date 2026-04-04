/**
 * Tic-tac-toe **play surface** (`index.html`) — reference implementation for other WASM-backed games.
 *
 * ## Lifecycle (high level)
 * 1. The lobby opens this page inside an iframe with query params `ws`, `id`, `player` (see `GameClient.fromUrlParams`).
 * 2. The server pushes a **full state snapshot** as the first WebSocket JSON message; later messages are **events**
 *    (`{ Event: { player, action } }`) and finally `{ GameOver: ... }` when the match ends.
 * 3. UI maps server state to cells; clicks send moves as JSON the guest WASM expects — here `[row, col]` (see Rust `TicTacToe` action type).
 *
 * ## Extending to another game
 * - Replace `applyState` / `applyEvent` with your rules; keep the **separation**: snapshot vs incremental events.
 * - Keep `GameClient` usage: first message = snapshot, `sendAction` = opaque JSON for your component.
 * - Normalize `player` identifiers the same way the lobby encodes them in the URL (see `decodePlayerParam`).
 */

import { GameClient } from "@ipel/game-sdk";

type PlayerMark = "X" | "O" | null;

/** Flat board aligned with `row * side + col` indexing (matches the vanilla client and typical array layouts). */
type Board = PlayerMark[];

interface TicConfig {
  side_length?: number;
  win_length?: number;
}

/** Server snapshot; may be wrapped as `{ state: ... }` once or twice depending on serializer paths. */
interface TicStateInner {
  state?: TicStateInner;
  config?: TicConfig;
  current_player?: unknown;
  board?: unknown;
}

/**
 * `player` in the URL is sometimes a JSON string (quoted) or a serde-tagged enum in JSON — mirror the old client.
 */
export function decodePlayerParam(raw: string | null): string | null {
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

function normalizePlayer(p: unknown): PlayerMark {
  if (p == null) return null;
  if (typeof p === "string") return p === "X" || p === "O" ? p : null;
  if (typeof p === "object" && p !== null) {
    const o = p as Record<string, unknown>;
    if ("X" in o && o.X === null) return "X";
    if ("O" in o && o.O === null) return "O";
  }
  const s = String(p);
  return s === "X" || s === "O" ? s : null;
}

function normalizeCell(c: unknown): PlayerMark {
  return normalizePlayer(c);
}

/** Smaller cells for large boards so the grid still fits the viewport. */
function cellRem(side: number): number {
  if (side <= 4) return 5.5;
  if (side <= 7) return 4;
  if (side <= 10) return 3;
  return Math.max(2.25, 36 / side);
}

function inferSideFromBoard(b: unknown): number | null {
  if (b == null) return null;
  let cells: unknown = b;
  if (Array.isArray(b) && b.length === 1 && Array.isArray(b[0])) cells = b[0];
  if (!Array.isArray(cells) || cells.length === 0) return null;
  const n = cells.length;
  const s = Math.round(Math.sqrt(n));
  return s * s === n ? s : null;
}

function isTagged(v: unknown, tag: string): boolean {
  return v != null && typeof v === "object" && Object.prototype.hasOwnProperty.call(v, tag);
}

/**
 * Mounts the interactive board into `#board` and drives status lines. Call once on DOM ready.
 */
export function startTicTacToePlay(): void {
  const boardEl = document.getElementById("board");
  const statusEl = document.getElementById("status");
  const playerInfoEl = document.getElementById("player-info");
  if (!boardEl || !statusEl || !playerInfoEl) return;

  const params = new URLSearchParams(window.location.search);
  const myPlayer = decodePlayerParam(params.get("player"));

  let board: Board = [];
  let sideLength = 3;
  let winLength = 3;
  let currentPlayer: PlayerMark = null;
  let gameOver = false;

  playerInfoEl.textContent = `You are: ${myPlayer ?? "?"}`;

  function rebuildBoardDom(side: number): void {
    const cell = `${cellRem(side)}rem`;
    boardEl.style.setProperty("--side", String(side));
    boardEl.style.setProperty("--cell", cell);
    boardEl.replaceChildren();
    board = Array(side * side).fill(null);
    for (let i = 0; i < side * side; i++) {
      const btn = document.createElement("button");
      btn.type = "button";
      btn.dataset.index = String(i);
      btn.className = "ttt-empty";
      btn.addEventListener("click", () => onCellClick(Number(btn.dataset.index)));
      boardEl.appendChild(btn);
    }
    sideLength = side;
  }

  function ensureBoardFromConfig(side: number, win: number): void {
    const s = Number(side);
    const w = Number(win);
    if (!Number.isFinite(s) || s < 2) return;
    winLength = Number.isFinite(w) && w >= 2 ? w : s;
    if (sideLength !== s || boardEl.querySelectorAll("button[data-index]").length !== s * s) {
      rebuildBoardDom(s);
    }
  }

  function renderBoard(): void {
    const cells = boardEl.querySelectorAll("button[data-index]");
    const n = sideLength * sideLength;
    for (let i = 0; i < n; i++) {
      const val = board[i];
      const el = cells[i];
      if (!el) continue;
      el.textContent = val ?? "";
      el.className = val ? (val === "X" ? "ttt-x" : "ttt-o") : "ttt-empty";
    }
  }

  function updatePlayerInfo(): void {
    playerInfoEl.textContent = `You are: ${myPlayer ?? "?"} · ${sideLength}×${sideLength}, ${winLength} in a row`;
  }

  function updateStatus(): void {
    if (gameOver) return;
    const cp = currentPlayer ?? "?";
    if (currentPlayer != null && myPlayer != null && currentPlayer === myPlayer) {
      statusEl.textContent = "Your turn!";
      statusEl.className = "text-lg mb-4 text-green-400";
    } else {
      statusEl.textContent = `Waiting for ${String(cp)}...`;
      statusEl.className = "text-lg mb-4 text-yellow-400";
    }
  }

  function onCellClick(index: number): void {
    if (gameOver || board[index] || currentPlayer !== myPlayer) return;
    const row = Math.floor(index / sideLength);
    const col = index % sideLength;
    client.sendAction([row, col]);
  }

  /**
   * The server may wrap state as `{ state: { board, ... } }` or send the inner object directly — accept both
   * so this page stays tolerant if the envelope changes.
   */
  function applyState(state: unknown): void {
    if (!state || typeof state !== "object") return;

    let inner: TicStateInner = state as TicStateInner;
    if (inner.state != null && typeof inner.state === "object") {
      inner = inner.state as TicStateInner;
    }

    let side = 3;
    let win = 3;
    if (inner.config != null && typeof inner.config === "object") {
      side = Number(inner.config.side_length);
      win = Number(inner.config.win_length);
    } else {
      const inferred = inferSideFromBoard(inner.board);
      if (inferred != null) {
        side = inferred;
        win = inferred;
      }
    }
    if (!Number.isFinite(side) || side < 2) side = 3;
    if (!Number.isFinite(win) || win < 2) win = side;
    ensureBoardFromConfig(side, win);
    updatePlayerInfo();

    if (inner.current_player !== undefined) {
      currentPlayer = normalizePlayer(inner.current_player);
    }

    const b = inner.board;
    if (b != null) {
      let cells: unknown = b;
      if (Array.isArray(b) && b.length === 1 && Array.isArray(b[0])) {
        cells = b[0];
      }
      const total = sideLength * sideLength;
      if (Array.isArray(cells)) {
        for (let i = 0; i < total; i++) board[i] = normalizeCell(cells[i]);
      } else if (typeof b === "object" && (b as Record<string, unknown>)["0"] !== undefined) {
        const o = b as Record<string, unknown>;
        for (let i = 0; i < total; i++) board[i] = normalizeCell(o[String(i)]);
      }
    }

    renderBoard();
    updateStatus();
  }

  /**
   * `GameOver` carries **per-player outcomes** from the Rust logic (`PlayerOutcome`: Win / Loss / Draw) but we
   * still accept legacy shapes (bare winner mark) so old recordings or tools do not break the UI.
   */
  function applyEvent(event: unknown): void {
    if (!event || typeof event !== "object") return;
    const e = event as Record<string, unknown>;

    if ("Event" in e && e.Event && typeof e.Event === "object") {
      const ev = e.Event as { player?: unknown; action?: unknown };
      const p = normalizePlayer(ev.player);
      const action = ev.action;
      if (Array.isArray(action) && action.length >= 2) {
        const row = Number(action[0]);
        const col = Number(action[1]);
        const index = row * sideLength + col;
        board[index] = p;
        currentPlayer = currentPlayer === "X" ? "O" : "X";
        renderBoard();
        updateStatus();
      }
      return;
    }

    if ("GameOver" in e) {
      gameOver = true;
      const go = e.GameOver;

      if (go === "Draw" || isTagged(go, "Draw")) {
        statusEl.textContent = "Draw!";
        statusEl.className = "text-lg mb-4 text-amber-400 font-bold";
      } else if (go === "Win" || isTagged(go, "Win")) {
        statusEl.textContent = "You win!";
        statusEl.className = "text-lg mb-4 text-green-400 font-bold";
      } else if (go === "Loss" || isTagged(go, "Loss")) {
        statusEl.textContent = "You lost!";
        statusEl.className = "text-lg mb-4 text-red-400 font-bold";
      } else {
        const winner = normalizePlayer(go);
        if (winner === myPlayer) {
          statusEl.textContent = "You win!";
          statusEl.className = "text-lg mb-4 text-green-400 font-bold";
        } else if (winner != null) {
          statusEl.textContent = `${winner} wins!`;
          statusEl.className = "text-lg mb-4 text-red-400 font-bold";
        } else {
          statusEl.textContent = "Game over";
          statusEl.className = "text-lg mb-4 text-gray-400 font-bold";
        }
      }
    }
  }

  let client: GameClient;
  try {
    client = GameClient.fromUrlParams();
    client
      .onState((state) => {
        statusEl.textContent = "Connected!";
        statusEl.className = "text-lg mb-4 text-gray-300";
        applyState(state);
      })
      .onEvent((event) => {
        applyEvent(event);
      })
      .onError(() => {
        statusEl.textContent = "Connection error";
        statusEl.className = "text-lg mb-4 text-red-400";
      })
      .onClose(() => {
        if (!gameOver) {
          statusEl.textContent = "Disconnected";
          statusEl.className = "text-lg mb-4 text-gray-500";
        }
      });
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    statusEl.textContent = msg;
    statusEl.className = "text-lg mb-4 text-red-400";
  }
}
