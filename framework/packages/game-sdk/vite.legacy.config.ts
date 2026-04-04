import { resolve } from "node:path";
import { defineConfig } from "vite";

/** Standalone IIFE → `framework/client-lib/game-client.js` (global `GameClient`). */
export default defineConfig({
  build: {
    outDir: resolve(__dirname, "../../client-lib"),
    emptyOutDir: false,
    rollupOptions: {
      input: resolve(__dirname, "src/legacy-global.ts"),
      output: {
        entryFileNames: "game-client.js",
        format: "iife",
      },
    },
  },
});
