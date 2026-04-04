import { resolve } from "node:path";
import { defineConfig } from "vite";

/**
 * Multi-page build: three HTML entrypoints → `../client/` (what Actix serves as static files).
 * `base` must match the server mount path so chunk URLs resolve inside the iframe.
 */
export default defineConfig({
  base: "/games/tic_tac_toe/",
  root: __dirname,
  build: {
    outDir: resolve(__dirname, "../client"),
    emptyOutDir: true,
    rollupOptions: {
      input: {
        index: resolve(__dirname, "index.html"),
        config: resolve(__dirname, "config.html"),
        result: resolve(__dirname, "result.html"),
      },
    },
  },
});
