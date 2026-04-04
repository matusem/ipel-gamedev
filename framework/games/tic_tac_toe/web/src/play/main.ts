/**
 * Play entry (`index.html`). Imports global styles then boots the example game controller.
 *
 * Other games should copy this pattern: one thin `main.ts` + a focused module (`*-play.ts`) that owns DOM and protocol details.
 */
import "../styles/main.css";
import { startTicTacToePlay } from "./tic-tac-toe-play.js";

startTicTacToePlay();
