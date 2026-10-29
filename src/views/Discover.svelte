<script lang="ts">
  import { api, type CatalogRef, type Meta } from "../lib/api";
  import { app } from "../lib/stores/app.svelte";
  import Poster from "../components/Poster.svelte";
  import Controls from "../components/Controls.svelte";

  let { type: initialType, catalog: initialCatalog }: { type?: string; catalog?: string } = $props();

  let all = $state<CatalogRef[]>([]);
  let type = $state(initialType ?? "movie");
  let key = $state(initialCatalog ?? "");
  let genre = $state<string>("");
  let items = $state<Meta[]>([]);
  let loading = $state(false);
  let more = $state(true);
  let sentinel: HTMLElement | undefined = $state();
  let seq = 0;

  $effect(() => {
    api.catalogs().then((c) => {
      all = c.filter((x) => !x.extra.some((e) => e.isRequired && e.name === "search"));
      if (!key) key = all.find((x) => x.type === type)?.key ?? "";
    });
  });

  const types = $derived([...new Set(all.map((c) => c.type))].filter((t) => !(t === "xxx" && app.settings.general.hideAdult && !app.adultRevealed)));
  const forType = $derived(all.filter((c) => c.type === type));
  const cat = $derived(all.find((c) => c.key === key));
  const genres = $derived(cat?.extra.find((e) => e.name === "genre")?.options ?? []);

  function typeLabel(t: string) {
    return t === "movie" ? "Films" : t === "series" ? "Shows" : t === "tv" ? "Channels" : t.charAt(0).toUpperCase() + t.slice(1);
  }

  function extra(skip = 0): [string, string][] {
    const e: [string, string][] = [];
    const g = genre || (cat?.extra.find((x) => x.name === "genre" && x.isRequired)?.options?.[0] ?? "");
    if (g) e.push(["genre", g]);
    if (skip) e.push(["skip", String(skip)]);
    return e;
  }

  async function reload() {
    if (!cat) return;
    const my = ++seq;
    loading = true;
    items = [];
    more = true;
    try {
      const r = await api.catalog($state.snapshot(cat) as CatalogRef, extra());
      if (my !== seq) return;
      items = r;
      more = r.length > 0;
    } catch {
      more = false;
    }
    loading = false;
  }

  async function loadMore() {
    if (loading || !more || !cat) return;
    loading = true;
    const my = seq;
    try {
      const r = await api.catalog($state.snapshot(cat) as CatalogRef, extra(items.length));
      if (my !== seq) return;
      const seen = new Set(items.map((m) => m.id));
      const fresh = r.filter((m) => !seen.has(m.id));
      items = [...items, ...fresh];
      more = fresh.length > 0;
    } catch {
      more = false;
    }
    loading = false;
  }

  $effect(() => {
    void key;
    void genre;
    reload();
  });

  $effect(() => {
    if (!sentinel) return;
    const io = new IntersectionObserver((e) => e[0].isIntersecting && loadMore(), { rootMargin: "800px" });
    io.observe(sentinel);
    return () => io.disconnect();
  });

  function setType(t: string) {
    type = t;
    genre = "";
    key = all.find((x) => x.type === t)?.key ?? "";
  }
</script>

<div class="discover">
  <header>
    <h1 class="large-title">Discover</h1>
    <Controls kind="segmented" value={type} options={types.map((t) => ({ value: t, label: typeLabel(t) }))} onchange={setType} label="Type" />
  </header>
  <div class="cats scroll-x">
    {#each forType as c (c.key)}
      <button class="chip press" class:sel={c.key === key} onclick={() => { key = c.key; genre = ""; }}>{c.name}<span class="faint">{c.addonName}</span></button>
    {/each}
  </div>
  {#if genres.length}
    <div class="genres scroll-x">
      <button class="g press" class:sel={!genre} onclick={() => (genre = "")}>All genres</button>
      {#each genres as g}
        <button class="g press" class:sel={g === genre} onclick={() => (genre = g)}>{g}</button>
      {/each}
    </div>
  {/if}
  <div class="grid">
    {#each items as m (m.id)}
      <Poster meta={m} showTitle hidden={!!m.adult && app.settings.general.hideAdult && !app.adultRevealed} />
    {/each}
    {#if loading}
      {#each Array(12) as _}<div class="ghost skeleton"></div>{/each}
    {/if}
  </div>
  <div bind:this={sentinel} class="sentinel"></div>
</div>

<style>
  .discover {
    padding: calc(var(--bar-h) + 36px) var(--gutter) 60px;
  }
  header {
    display: flex;
    align-items: center;
    gap: 22px;
    margin-bottom: 20px;
  }
  .cats,
  .genres {
    display: flex;
    gap: 8px;
    margin-bottom: 12px;
  }
  .cats .chip {
    height: 34px;
    padding: 0 14px;
    font-size: 13.5px;
  }
  .cats .chip span {
    margin-left: 6px;
    font-weight: 500;
  }
  .chip.sel {
    background: var(--text);
    color: var(--ink);
  }
  .chip.sel span {
    color: color-mix(in srgb, var(--ink) 60%, transparent);
  }
  .g {
    padding: 6px 2px;
    margin-right: 14px;
    color: var(--text-dim);
    font-weight: 600;
    white-space: nowrap;
    border-bottom: 2px solid transparent;
    transition: color var(--t-fast), border-color var(--t-fast);
  }
  .g.sel {
    color: var(--text);
    border-color: var(--accent);
  }
  .grid {
    margin-top: 18px;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(var(--card-w), 1fr));
    gap: 24px 16px;
  }
  .grid :global(.poster) {
    width: auto;
  }
  .ghost {
    aspect-ratio: 2 / 3;
    border-radius: var(--poster-r);
  }
  .sentinel {
    height: 1px;
  }
</style>
