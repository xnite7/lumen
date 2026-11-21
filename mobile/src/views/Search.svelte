<script lang="ts">
  // Search: type, and results stream in from every source as they land.
  import { api, listen, type Meta } from "$lib/api";
  import { nav } from "../lib/nav.svelte";
  import Poster from "../ui/Poster.svelte";

  let q = $state("");
  let results = $state<Meta[]>([]);
  let pending = $state(false);
  let recents = $state<string[]>([]);
  let fix = $state<string | null>(null);
  let reqId = "";
  let timer = 0;

  $effect(() => {
    api.recents().then((r) => (recents = r ?? [])).catch(() => {});
    let un: (() => void) | undefined;
    listen<any>("search", (c) => {
      if (c.reqId !== reqId) return;
      if (c.finished) {
        pending = false;
        if (!results.length && q.trim().length > 2) api.didYouMean(q.trim()).then((f) => (fix = f)).catch(() => {});
        return;
      }
      const have = new Set(results.map((m) => m.id));
      results = [...results, ...(c.metas ?? []).filter((m: Meta) => !have.has(m.id) && m.poster)];
    }).then((u) => (un = u));
    return () => un?.();
  });

  function run(text = q) {
    const t = text.trim();
    results = [];
    fix = null;
    if (t.length < 2) return void (pending = false);
    reqId = Math.random().toString(36).slice(2);
    pending = true;
    api.search(t, reqId).catch(() => (pending = false));
  }
  function input() {
    clearTimeout(timer);
    timer = window.setTimeout(run, 280);
  }
  const cols = $derived(nav.wide ? 160 : 104);
</script>

<div class="page">
  <div class="head">
    <h1>search</h1>
    <div class="field glass">
      <span>⌕</span>
      <input placeholder="shows, films, anime, people" bind:value={q} oninput={input} enterkeyhint="search" autocapitalize="off" autocorrect="off" spellcheck="false" />
      {#if q}<button onclick={() => ((q = ""), run(""))} aria-label="clear">✕</button>{/if}
    </div>
  </div>

  {#if !q && recents.length}
    <div class="recents">
      <h2 class="row-title">recent</h2>
      {#each recents.slice(0, 8) as r}
        <button class="rec press" onclick={() => ((q = r), run(r))}>{r}</button>
      {/each}
    </div>
  {/if}

  {#if fix}
    <button class="fix press" onclick={() => ((q = fix!), run(fix!))}>did you mean <b>{fix}</b>?</button>
  {/if}

  <div class="grid" style:grid-template-columns="repeat(auto-fill, minmax({cols}px, 1fr))">
    {#each results as m (m.id)}<Poster meta={m} w={undefined} />{/each}
  </div>
  {#if pending}<p class="faint center">looking…</p>{:else if q.trim().length > 1 && !results.length}<p class="faint center">nothing found</p>{/if}
</div>

<style>
  .head { position: sticky; top: 0; z-index: 5; padding: calc(var(--safe-t) + 14px) var(--g) 12px; background: linear-gradient(var(--bg) 70%, #0000); }
  h1 { margin: 0 0 12px; font-size: 32px; }
  .field { display: flex; align-items: center; gap: 10px; height: 48px; border-radius: 24px; padding: 0 16px; }
  .field input { flex: 1; background: none; border: 0; outline: 0; color: var(--text); font: inherit; font-size: 17px; }
  .field span { color: var(--label2); font-size: 20px; }
  .recents { padding: 0 var(--g); display: grid; justify-items: start; }
  .rec { padding: 10px 0; font-size: 16px; color: var(--label2); }
  .fix { margin: 6px var(--g); color: var(--label2); }
  .fix b { color: var(--neon-soft); }
  .grid { display: grid; gap: 12px; padding: 8px var(--g); }
  .grid :global(.poster) { width: 100% !important; }
  .center { text-align: center; padding: 30px; }
</style>
