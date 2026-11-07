// Builds the UI as ONE self-contained HTML file (mock data, no backend):
// the live preview for polishing the design. `npm run demo` -> dist-demo/couch-preview.html
import { build } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import fs from "node:fs";
import path from "node:path";

const out = "dist-demo";
process.env.VITE_DEMO = "1";
await build({
  configFile: false,
  plugins: [svelte()],
  define: { "import.meta.env.VITE_DEMO": JSON.stringify("1") },
  build: {
    outDir: out,
    emptyOutDir: true,
    target: "es2022",
    assetsInlineLimit: 100_000_000,
    cssCodeSplit: false,
    modulePreload: false,
    rollupOptions: { output: { inlineDynamicImports: true } },
  },
  logLevel: "warn",
});

let html = fs.readFileSync(path.join(out, "index.html"), "utf8");
html = html.replace(/<script type="module" crossorigin src="([^"]+)"><\/script>/g, (_, src) => {
  const js = fs.readFileSync(path.join(out, src), "utf8").replace(/<\/script/g, "<\\/script");
  return `<script type="module">${js}</script>`;
});
html = html.replace(/<link rel="stylesheet" crossorigin href="([^"]+)">/g, (_, href) => `<style>${fs.readFileSync(path.join(out, href), "utf8")}</style>`);
// The artifact host wraps the page in its own document: hand it just the
// title, styles, mount point and script.
const title = html.match(/<title>.*?<\/title>/)?.[0] ?? "<title>couch preview</title>";
const styles = [...html.matchAll(/<style>[\s\S]*?<\/style>/g)].map((m) => m[0]).join("\n");
const scripts = [...html.matchAll(/<script type="module">[\s\S]*?<\/script>/g)].map((m) => m[0]).join("\n");
html = `${title}\n<meta name="color-scheme" content="dark">\n<style>:root{color-scheme:dark}html,body{height:100%;background:#000}</style>\n${styles}\n<div id="app"></div>\n${scripts}\n`;
const file = path.join(out, "couch-preview.html");
fs.writeFileSync(file, html);
console.log(file, (fs.statSync(file).size / 1e6).toFixed(2), "MB");
