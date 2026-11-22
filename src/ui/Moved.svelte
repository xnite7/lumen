<script lang="ts">
  // This app is retired. Everyone moves to the new couch: one button
  // installs it next to this one and opens it; the page is the fallback.
  import { api } from "../lib/api";
  import { app } from "../lib/stores/app.svelte";
  import mark from "../assets/brand/couch-mark.png";

  const PAGE = "https://midnite-couch.xnite7.workers.dev/download";
  let busy = $state(false);
  let err = $state("");

  async function move() {
    if (app.platform !== "windows") return api.openExternal(PAGE).catch(() => {});
    busy = true;
    err = "";
    try {
      await api.betaInstall();
      // the new one is open; this one is done
      setTimeout(() => api.window("close").catch(() => {}), 1500);
    } catch (e) {
      err = String(e);
      busy = false;
    }
  }
</script>

<div class="moved" role="dialog" aria-modal="true" aria-label="couch has moved">
  <div class="card">
    <img src={mark} alt="" width="72" height="72" />
    <h1>couch has moved</h1>
    <p>this version is retired. the new couch is faster, smoother, and it's where everyone's watching now.</p>
    <button class="go" disabled={busy} onclick={move}>{busy ? "getting it…" : "get the new couch"}</button>
    {#if err}<p class="err">that didn't work ({err}). <button class="link" onclick={() => api.openExternal(PAGE)}>download it here</button></p>{/if}
    <button class="link" onclick={() => api.openExternal(PAGE)}>or download it from the site</button>
  </div>
</div>

<style>
  .moved {
    position: fixed;
    inset: 0;
    z-index: 100000;
    display: grid;
    place-items: center;
    background: rgba(5, 5, 9, 0.92);
    backdrop-filter: blur(24px);
  }
  .card {
    max-width: 420px;
    text-align: center;
    display: grid;
    gap: 14px;
    justify-items: center;
    padding: 32px;
  }
  h1 { font-size: 26px; margin: 4px 0 0; font-weight: 600; }
  p { opacity: 0.7; line-height: 1.5; margin: 0; }
  .go {
    margin-top: 8px;
    padding: 12px 28px;
    border-radius: 999px;
    border: 0;
    background: #fff;
    color: #000;
    font-weight: 600;
    cursor: pointer;
  }
  .go:disabled { opacity: 0.6; cursor: default; }
  .link { background: none; border: 0; color: inherit; opacity: 0.55; text-decoration: underline; cursor: pointer; font-size: 13px; }
  .err { color: #ff8a8a; font-size: 13px; opacity: 1; }
</style>
