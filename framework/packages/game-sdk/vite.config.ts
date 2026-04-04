import { resolve } from "node:path";
import { defineConfig } from "vite";
import dts from "vite-plugin-dts";

export default defineConfig({
  plugins: [
    dts({
      include: ["src/**/*.ts"],
      exclude: ["src/**/*.test.ts"],
    }),
  ],
  build: {
    lib: {
      entry: resolve(__dirname, "src/index.ts"),
      name: "IpelGameSdk",
      formats: ["es"],
      fileName: "game-sdk",
    },
    sourcemap: true,
    emptyOutDir: true,
    outDir: "dist",
  },
});
