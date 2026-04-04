/**
 * Result entry — decodes `payload` via the SDK helper, then renders a static summary.
 * If your game needs charts or links, keep parsing here thin and push logic into `result-view.ts`.
 */
import "../styles/main.css";
import { readResultPayload } from "@ipel/game-sdk";
import { escapeHtml, renderMatchResult, type ResultPayload } from "./result-view.js";

function main(): void {
  const root = document.getElementById("root");
  if (!root) return;

  const data = readResultPayload<ResultPayload>();
  if (data === null) {
    const params = new URLSearchParams(window.location.search);
    if (!params.get("payload")) {
      root.innerHTML = `<p class="text-amber-200">Missing <code class="text-white">payload</code> query parameter (URI-encoded JSON).</p>`;
    } else {
      root.innerHTML = `<p class="text-red-300">Invalid JSON in <code class="text-white">payload</code>.</p>`;
    }
    return;
  }

  try {
    renderMatchResult(root, data);
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    root.innerHTML = `<p class="text-red-300">Render error: ${escapeHtml(msg)}</p>`;
  }
}

main();
