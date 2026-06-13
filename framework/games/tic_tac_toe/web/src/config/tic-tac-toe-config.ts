/**
 * **Config iframe** — teaches how to cooperate with the lobby’s JSON Schema pipeline.
 *
 * The lobby (`LobbyConfigPanel` in Dioxus):
 * 1. Posts the manifest’s `config_schema` as a JS object (`upjs-gdd-game-config-schema`).
 * 2. Posts the stored server config or null (`upjs-gdd-game-config-state`).
 * 3. Listens for preview JSON from the iframe (`upjs-gdd-game-config`). It only checks that the body is **valid JSON**;
 *    richer validation is expected **inside the iframe** against the schema (this file implements a small Draft-07 subset).
 * 4. Replies with `upjs-gdd-game-config-result` (`ok`, `errors`) so the iframe can show whether the lobby accepted the preview string.
 *
 * The iframe does **not** call GraphQL; the host clicks **Apply configuration** to persist.
 */

import { LobbyConfigBridge } from "@upjs-gdd/game-sdk";
import type { JsonValue } from "@upjs-gdd/game-sdk";

const GAME_TYPE = "tic_tac_toe";

export interface TicTacToeConfig {
  side_length: number;
  win_length: number;
}

/**
 * Minimal JSON Schema validator (Draft-07 subset: `oneOf`, `type`, `required`, `properties`, integers, min/max).
 * Matches what `manifest.json` ships for this game — swap for Ajv in larger projects.
 */
function validateInstance(inst: unknown, sch: JsonValue, path: string): string | null {
  const p = path || "(root)";
  if (sch && typeof sch === "object" && !Array.isArray(sch) && Array.isArray((sch as { oneOf?: JsonValue[] }).oneOf)) {
    const branches = (sch as { oneOf: JsonValue[] }).oneOf;
    const branchErrs: string[] = [];
    for (const branch of branches) {
      const err = validateInstance(inst, branch, path);
      if (err === null) return null;
      branchErrs.push(err);
    }
    return `${p}: ${branchErrs.join(" | ")}`;
  }

  if (!sch || typeof sch !== "object" || Array.isArray(sch)) {
    return `${p}: invalid schema node`;
  }
  const S = sch as {
    type?: string;
    required?: string[];
    properties?: Record<string, JsonValue>;
    additionalProperties?: boolean;
    minimum?: number;
    maximum?: number;
  };

  const want = S.type;
  if (want === "null") {
    return inst === null ? null : `${p}: expected null`;
  }
  if (want === "object") {
    if (inst === null || typeof inst !== "object" || Array.isArray(inst)) {
      return `${p}: expected object`;
    }
    const obj = inst as Record<string, unknown>;
    const req = S.required ?? [];
    for (const key of req) {
      if (!Object.prototype.hasOwnProperty.call(obj, key)) {
        return `${p}: missing required "${key}"`;
      }
    }
    const props = S.properties ?? {};
    if (S.additionalProperties === false) {
      for (const k of Object.keys(obj)) {
        if (!Object.prototype.hasOwnProperty.call(props, k)) {
          return `${p}: additional property "${k}" not allowed`;
        }
      }
    }
    for (const key of Object.keys(props)) {
      if (!Object.prototype.hasOwnProperty.call(obj, key)) continue;
      const err = validateInstance(obj[key], props[key]!, `${p}/${key}`);
      if (err) return err;
    }
    return null;
  }
  if (want === "integer") {
    if (typeof inst !== "number" || !Number.isFinite(inst) || !Number.isInteger(inst)) {
      return `${p}: expected integer`;
    }
    if (S.minimum !== undefined && inst < S.minimum) return `${p}: below minimum`;
    if (S.maximum !== undefined && inst > S.maximum) return `${p}: above maximum`;
    return null;
  }
  return `${p}: unsupported schema fragment (type ${String(want)})`;
}

export function startTicTacToeConfig(): void {
  const sideEl = document.getElementById("side") as HTMLInputElement | null;
  const winEl = document.getElementById("win") as HTMLInputElement | null;
  const errEl = document.getElementById("err");
  const applyBtn = document.getElementById("apply");
  if (!sideEl || !winEl || !errEl || !applyBtn) return;

  let receivedSchema: JsonValue | null = null;

  const bridge = new LobbyConfigBridge<TicTacToeConfig>({
    gameType: GAME_TYPE,
    onSchema: (schema) => {
      receivedSchema = schema;
      errEl.textContent = "";
    },
    onState: (config) => {
      errEl.textContent = "";
      if (config === null || config === undefined) {
        sideEl.value = "3";
        winEl.value = "3";
      } else {
        const side = config.side_length;
        const win = config.win_length;
        if (typeof side === "number" && Number.isFinite(side)) {
          sideEl.value = String(Math.round(side));
        }
        if (typeof win === "number" && Number.isFinite(win)) {
          winEl.value = String(Math.round(win));
        }
      }
      clampWinMax();
    },
    onValidationReply: (ok, errors) => {
      if (ok) errEl.textContent = "";
      else errEl.textContent = errors.length ? errors.join("; ") : "Config rejected.";
    },
  });

  bridge.attach();

  function validateJsonAgainstReceivedSchema(jsonStr: string): boolean {
    if (!receivedSchema) return true;
    let data: unknown;
    try {
      data = JSON.parse(jsonStr);
    } catch (e) {
      errEl.textContent = `Invalid JSON: ${e instanceof Error ? e.message : String(e)}`;
      return false;
    }
    const msg = validateInstance(data, receivedSchema, "");
    if (msg) {
      errEl.textContent = msg;
      return false;
    }
    return true;
  }

  function clampWinMax(): void {
    const s = parseInt(sideEl.value, 10);
    if (!Number.isFinite(s)) return;
    winEl.max = String(Math.min(20, Math.max(2, s)));
    const w = parseInt(winEl.value, 10);
    if (Number.isFinite(w) && w > s) winEl.value = String(s);
  }

  sideEl.addEventListener("input", clampWinMax);
  sideEl.addEventListener("change", clampWinMax);

  function validateFields(): string | null {
    errEl.textContent = "";
    const side = parseInt(sideEl.value, 10);
    const win = parseInt(winEl.value, 10);
    if (!Number.isFinite(side) || side < 2 || side > 20) {
      return "Side length must be between 2 and 20.";
    }
    if (!Number.isFinite(win) || win < 2) {
      return "Win length must be at least 2.";
    }
    if (win > side) {
      return "Win length cannot be larger than the side.";
    }
    return null;
  }

  applyBtn.addEventListener("click", () => {
    const fieldErr = validateFields();
    if (fieldErr) {
      errEl.textContent = fieldErr;
      return;
    }
    const side = parseInt(sideEl.value, 10);
    const win = parseInt(winEl.value, 10);
    const json = JSON.stringify({ side_length: side, win_length: win });
    if (!validateJsonAgainstReceivedSchema(json)) return;
    bridge.sendPreview(json);
  });

  clampWinMax();
}
