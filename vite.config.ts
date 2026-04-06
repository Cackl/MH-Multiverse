import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const host = process.env.TAURI_DEV_HOST;

const tauriConf = JSON.parse(
  readFileSync(resolve(__dirname, "src-tauri", "tauri.conf.json"), "utf-8")
);

const appVersion =
  tauriConf?.version ??
  tauriConf?.package?.version ??
  "0.0.0";

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  define: {
    __APP_VERSION__: JSON.stringify(appVersion)
  },
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: { ignored: ["**/src-tauri/**"] },
  },
});