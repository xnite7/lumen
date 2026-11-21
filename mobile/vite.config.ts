// The iPhone / iPad app's screens. They share couch's engine (Rust) and its
// stores and helpers in ../src/lib with the rest of couch.
import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL(".", import.meta.url));

export default defineConfig({
  root,
  plugins: [svelte({ configFile: fileURLToPath(new URL("../svelte.config.js", import.meta.url)) })],
  clearScreen: false,
  resolve: { alias: { $lib: fileURLToPath(new URL("../src/lib", import.meta.url)), $old: fileURLToPath(new URL("../src", import.meta.url)) } },
  server: { port: 1430, strictPort: true, host: process.env.TAURI_DEV_HOST || false },
  build: {
    outDir: fileURLToPath(new URL("../dist-mobile", import.meta.url)),
    emptyOutDir: true,
    target: "es2022",
    cssMinify: true,
    sourcemap: false,
    chunkSizeWarningLimit: 900,
  },
});
