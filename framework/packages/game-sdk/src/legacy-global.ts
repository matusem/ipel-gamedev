/**
 * Side-effect entry for the IIFE build: exposes `GameClient` on `window` for script-tag users.
 */
import { GameClient } from "./game-client.js";

declare global {
  interface Window {
    GameClient: typeof GameClient;
  }
}

window.GameClient = GameClient;
