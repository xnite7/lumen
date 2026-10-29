<script lang="ts">
  // Every search-capable addon is asked directly and in full, and everything
  // lands in one wrapping grid grouped by addon -- no ten-item preview rows,
  // no "See all", no addons left un-asked at the bottom of the list.
  import { api, listen, uid, img, type Meta, type CatalogRef } from "../lib/api";
  import { app } from "../lib/stores/app.svelte";
  import { openDetail } from "../lib/open";
  import Poster from "../components/Poster.svelte";
  import Icon from "../components/Icon.svelte";

  let { q: initial = "" }: { q?: string } = $props();
  let q = $state(initial);
  let groups = $state<{ catalog: CatalogRef; metas: Meta[] }[]>([]);
  let pending = $state(false);
  let suggestions = $state<any[]>([]);
  let recents = $state<string[]>([]);
  let input: HTMLInputElement | undefined = $state();
  let reqId = "";
  let typeTimer: ReturnType<typeof setTimeout> | undefined;
  let kind = $state<"all" | "movie" | "series">("all");

  $effect(() => {
    api.recents().then((r) => (recents = r ?? []));
    requestAnimationFrame(() => input?.focus());
    let un: (() => void) | undefined;
    listen<any>("search", (c) => {
      if (c.reqId !== reqId) return;
      if (c.finished) {
        pending = false;
        return;
      }
      if (c.metas?.length) groups = [...groups, { catalog: c.catalog, metas: c.metas }];
    }).then((u) => (un = u));
    if (initial) run(initial);
    return () => un?.();
  });

  function run(text: string) {
    const t = text.trim();
    if (t.length < 2) {
      groups = [];
      return;
    }
    reqId = uid();
    groups = [];
    pending = true;
    api.search(t, reqId);
  }

  function onInput() {
    clearTimeout(typeTimer);
    typeTimer = setTimeout(() => {
      run(q);
      if (q.trim()) api.suggest(q).then((s) => (suggestions = s ?? [])).catch(() => {});
      else suggestions = [];
    }, 250);
  }

  function pickRecent(r: string) {
    q = r;
    run(r);
  }

  const shown = $derived(
    groups
      .map((g) => ({ ...g, metas: g.metas.filter((m) => kind === "all" || m.type === kind) }))
      .filter((g) => g.metas.length)
      .filter((g) => !(g.catalog.adult && app.settings.general.hideAdult && !app.adultRevealed)),
  );
  const hiddenAdult = $derived(groups.some((g) => g.catalog.adult) && app.settings.general.hideAdult && !app.adultRevealed);
</script>

<div class="search">
  <aside class="rail">
    <form
      class="box glass"
      onsubmit={(e) => {
        e.preventDefault();
        run(q);
      }}
    >
      <Icon name="search" size={19} />
      <input bind:this={input} bind:value={q} oninput={onInput} placeholder="Films, shows, people…" spellcheck="false" aria-label="Search" />
      {#if q}<button type="button" class="clear" onclick={() => { q = ""; groups = []; suggestions = []; input?.focus(); }} aria-label="Clear"><Icon name="close" size={14} /></button>{/if}
    </form>

    {#if suggestions.length}
      <div class="sect">
        <h3 class="foot faint">Suggestions</h3>
        {#each suggestions as s (s.id)}
          <button class="sugg press" onclick={(e) => openDetail({ id: s.id, type: s.type, name: s.name, poster: s.poster }, e.currentTarget)}>
            {#if s.poster}<img src={img(s.poster)} alt="" />{:else}<span class="ph"></span>{/if}
            <span class="st"><span class="nm">{s.name}</span><span class="foot faint">{s.year ?? ""} {s.cast ? `· ${s.cast}` : ""}</span></span>
          </button>
        {/each}
      </div>
    {:else if recents.length && !q}
      <div class="sect">
        <div class="sect-h">
          <h3 class="foot faint">Recent</h3>
          <button class="foot faint" onclick={() => { api.recentsClear(); recents = []; }}>Clear</button>
        </div>
        {#each recents as r}
          <button class="recent press" onclick={() => pickRecent(r)}><Icon name="refresh" size={14} /> {r}</button>
        {/each}
      </div>
    {/if}
    <p class="tip foot faint">Paste a link or magnet anywhere in Lumen to play it.</p>
  </aside>

  <main class="results">
    {#if groups.length}
      <div class="filter">
        {#each [["all", "All"], ["movie", "Films"], ["series", "Shows"]] as [k, l]}
          <button class="chip press" class:sel={kind === k} onclick={() => (kind = k as any)}>{l}</button>
        {/each}
        {#if hiddenAdult}
          <button class="chip press" onclick={() => (app.adultRevealed = true)}><Icon name="eye" size={13} /> Show hidden results</button>
        {/if}
        {#if pending}<span class="foot faint">Still asking addons…</span>{/if}
      </div>
    {/if}
    {#each shown as g (g.catalog.key)}
      <section class="group">
        <h2 class="title3">{g.catalog.name} <span class="foot faint">{g.catalog.addonName}</span></h2>
        <div class="grid">
          {#each g.metas as m (m.id + g.catalog.key)}
            <Poster meta={m} showTitle />
          {/each}
        </div>
      </section>
    {:else}
      {#if pending}
        <div class="grid pad">{#each Array(12) as _}<div class="ghost skeleton"></div>{/each}</div>
      {:else if q.trim().length >= 2}
        <div class="empty"><p class="headline">Nothing found for “{q}”</p><p class="foot dim">Check the spelling, or install an addon that searches the kind of thing you're after.</p></div>
      {:else}
        <div class="empty"><p class="title2">Search every addon at once</p><p class="foot dim">Results from each addon appear as soon as it answers.</p></div>
      {/if}
    {/each}
  </main>
</div>

<style>
  .search {
    display: grid;
    grid-template-columns: 320px 1fr;
    gap: 28px;
    padding: calc(var(--bar-h) + 32px) var(--gutter) 60px;
    min-height: 100%;
  }
  .rail {
    position: sticky;
    top: calc(var(--bar-h) + 32px);
    align-self: start;
    display: flex;
    flex-direction: column;
    gap: 18px;
  }
  .box {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 52px;
    padding: 0 14px 0 18px;
    border-radius: var(--r-pill);
  }
  .box input {
    flex: 1;
    background: none;
    border: 0;
    outline: 0;
    font-size: 17px;
    height: 100%;
  }
  .clear {
    width: 24px;
    height: 24px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: rgb(255 255 255 / 0.18);
  }
  .sect {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .sect-h {
    display: flex;
    justify-content: space-between;
  }
  h3 {
    margin: 0 0 4px 6px;
    font-weight: 600;
  }
  .sugg,
  .recent {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 7px 8px;
    border-radius: var(--r-sm);
    text-align: left;
  }
  .sugg:hover,
  .recent:hover {
    background: rgb(255 255 255 / 0.07);
  }
  .sugg img,
  .ph {
    width: 34px;
    height: 50px;
    object-fit: cover;
    border-radius: 6px;
    flex: none;
    background: rgb(255 255 255 / 0.08);
  }
  .st {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .nm {
    font-weight: 600;
    font-size: 14px;
  }
  .recent {
    color: var(--text-dim);
    font-size: 14px;
  }
  .tip {
    margin: 0 6px;
  }
  .filter {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 18px;
  }
  .chip.sel {
    background: var(--text);
    color: var(--ink);
  }
  .group {
    margin-bottom: 34px;
  }
  .group h2 {
    margin-bottom: 14px;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(var(--card-w), 1fr));
    gap: 22px 16px;
  }
  .grid :global(.poster) {
    width: auto;
  }
  .ghost {
    aspect-ratio: 2 / 3;
    border-radius: var(--poster-r);
  }
  .empty {
    padding: 80px 0;
    text-align: center;
  }
  .empty p {
    margin: 0 0 6px;
  }
  @media (max-width: 760px) {
    .search {
      grid-template-columns: 1fr;
    }
    .rail {
      position: static;
    }
  }
</style>
