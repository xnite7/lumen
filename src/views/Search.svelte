<script lang="ts">
  // Search tab. Empty: recent searches, mood search, and browse tiles for
  // every catalog and genre. Typing: top hits as you type, then every
  // search-capable addon's results in one grid, grouped by addon.
  import { api, listen, uid, img, type Meta, type CatalogRef } from "../lib/api";
  import { app } from "../lib/stores/app.svelte";
  import { go } from "../lib/router.svelte";
  import { ui } from "../lib/ui.svelte";
  import { openDetail } from "../lib/open";
  import NavBar from "../ui/NavBar.svelte";
  import Poster from "../components/Poster.svelte";
  import Icon from "../components/Icon.svelte";
  import Controls from "../components/Controls.svelte";

  let { q: initial = "" }: { q?: string } = $props();
  let q = $state("");
  let groups = $state<{ catalog: CatalogRef; metas: Meta[] }[]>([]);
  let pending = $state(false);
  let suggestions = $state<any[]>([]);
  let recents = $state<string[]>([]);
  let catalogs = $state<CatalogRef[]>([]);
  let input: HTMLInputElement | undefined = $state();
  let focused = $state(false);
  let reqId = "";
  let typeTimer: ReturnType<typeof setTimeout> | undefined;
  let kind = $state<"all" | "movie" | "series">("all");

  $effect(() => {
    api.recents().then((r) => (recents = r ?? []));
    api.catalogs().then((c) => (catalogs = c.filter((x) => !x.extra.some((e) => e.isRequired && e.name === "search"))));
    if (!ui.phone) requestAnimationFrame(() => input?.focus());
    let un: (() => void) | undefined;
    listen<any>("search", (c) => {
      if (c.reqId !== reqId) return;
      if (c.finished) {
        pending = false;
        return;
      }
      if (c.metas?.length) groups = [...groups, { catalog: c.catalog, metas: c.metas }];
    }).then((u) => (un = u));
    if (initial) {
      q = initial;
      run(initial);
    }
    return () => un?.();
  });

  function run(text: string) {
    const t = text.trim();
    if (t.length < 2) {
      groups = [];
      pending = false;
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
      if (q.trim()) api.suggest(q).then((s) => (suggestions = (s ?? []).slice(0, 5))).catch(() => {});
      else suggestions = [];
    }, 250);
  }

  function pick(r: string) {
    q = r;
    run(r);
    input?.blur();
  }
  function clear() {
    q = "";
    groups = [];
    suggestions = [];
    pending = false;
    input?.focus();
  }

  const searching = $derived(q.trim().length >= 2);
  const shown = $derived(
    groups
      .map((g) => ({ ...g, metas: g.metas.filter((m) => kind === "all" || m.type === kind) }))
      .filter((g) => g.metas.length)
      .filter((g) => !(g.catalog.adult && app.settings.general.hideAdult && !app.adultRevealed)),
  );
  const hiddenAdult = $derived(groups.some((g) => g.catalog.adult) && app.settings.general.hideAdult && !app.adultRevealed);

  // ------------------------------------------------------------- browse
  const tints = ["#6155f5", "#ff375f", "#0a84ff", "#30d158", "#ff9f0a", "#bf5af2", "#40cbe0", "#ac8e68", "#ff453a", "#5e5ce6"];
  const typeLabel = (t: string) => (t === "movie" ? "Films" : t === "series" ? "Shows" : t === "tv" ? "Channels" : t.charAt(0).toUpperCase() + t.slice(1));
  const browse = $derived(
    catalogs
      .filter((c) => !(c.adult && app.settings.general.hideAdult && !app.adultRevealed))
      .filter((c) => !c.extra.some((e) => e.isRequired && !e.options?.length))
      .slice(0, 12),
  );
  const genreSource = $derived(catalogs.find((c) => c.type === "movie" && c.extra.some((e) => e.name === "genre" && e.options?.length)));
  const genres = $derived(genreSource?.extra.find((e) => e.name === "genre")?.options?.slice(0, 16) ?? []);
</script>

<NavBar title="Search" />

<div class="search">
  <h1 class="large-title">Search</h1>

  <form class="box" class:focused onsubmit={(e) => { e.preventDefault(); run(q); input?.blur(); }}>
    <Icon name="search" size={18} />
    <input
      bind:this={input}
      bind:value={q}
      oninput={onInput}
      onfocus={() => (focused = true)}
      onblur={() => setTimeout(() => (focused = false), 150)}
      placeholder="Films, shows, people"
      spellcheck="false"
      autocapitalize="off"
      enterkeyhint="search"
      aria-label="Search"
    />
    {#if q}<button type="button" class="clear" onclick={clear} aria-label="Clear"><Icon name="close" size={11} /></button>{/if}
  </form>

  {#if searching}
    <!-- top hits -->
    {#if suggestions.length}
      <div class="hits group">
        {#each suggestions as s (s.id)}
          <button class="hit" onclick={(e) => openDetail({ id: s.id, type: s.type, name: s.name, poster: s.poster }, e.currentTarget)}>
            {#if s.poster}<img src={img(s.poster)} alt="" />{:else}<span class="ph"></span>{/if}
            <span class="st">
              <span class="nm">{s.name}</span>
              <span class="foot dim">{[s.type === "series" ? "Show" : s.type === "movie" ? "Film" : "", s.year, s.cast].filter(Boolean).join(" · ")}</span>
            </span>
            <Icon name="chevron" size={14} class="chev" />
          </button>
        {/each}
      </div>
    {/if}

    {#if groups.length}
      <div class="filter">
        <Controls kind="segmented" bind:value={kind} options={[{ value: "all", label: "All" }, { value: "movie", label: "Films" }, { value: "series", label: "Shows" }]} label="Kind" />
        {#if hiddenAdult}
          <button class="chip press" onclick={() => (app.adultRevealed = true)}><Icon name="eye" size={13} /> Show hidden</button>
        {/if}
        {#if pending}<span class="foot faint">Still asking addons…</span>{/if}
      </div>
    {/if}

    {#each shown as g (g.catalog.key)}
      <section class="results rise">
        <h2 class="title3">{g.catalog.name} <span class="foot faint">{g.catalog.addonName}</span></h2>
        <div class="grid">
          {#each g.metas as m (m.id + g.catalog.key)}
            <Poster meta={m} showTitle />
          {/each}
        </div>
      </section>
    {:else}
      {#if pending}
        <div class="grid">{#each Array(ui.phone ? 6 : 12) as _}<div class="ghost skeleton"></div>{/each}</div>
      {:else}
        <div class="empty">
          <Icon name="search" size={44} />
          <p class="title3">No results for “{q}”</p>
          <p class="sub dim">Check the spelling, or install an addon that searches this kind of thing.</p>
        </div>
      {/if}
    {/each}
  {:else}
    {#if recents.length}
      <section class="recents">
        <div class="sect-h">
          <h2 class="title3">Recent</h2>
          <button class="btn plain small" onclick={() => { api.recentsClear(); recents = []; }}>Clear</button>
        </div>
        <div class="recent-list scroll-x">
          {#each recents as r}
            <button class="chip press" onclick={() => pick(r)}><Icon name="search" size={13} /> {r}</button>
          {/each}
        </div>
      </section>
    {/if}

    {#if browse.length}
      <section>
        <h2 class="title3 sect-title">Browse</h2>
        <div class="tiles">
          {#each browse as c, n (c.key)}
            <button class="tile press" style:--t={tints[n % tints.length]} onclick={() => go({ name: "discover", type: c.type, catalog: c.key })}>
              <span class="tile-name">{c.name}</span>
              <span class="tile-sub">{typeLabel(c.type)} · {c.addonName}</span>
            </button>
          {/each}
        </div>
        {#if app.settings.integrations.aiEnabled}
          <button class="vibe-card press" onclick={() => go({ name: "vibe" })}>
            <span class="blobs" aria-hidden="true"></span>
            <span class="vc-icon"><Icon name="sparkles" size={22} /></span>
            <span class="vc-txt">
              <span class="headline">Search by vibe</span>
              <span class="foot">Describe a feeling. Picks come from your own taste.</span>
            </span>
            <Icon name="forward" size={17} />
          </button>
        {/if}
      </section>
    {/if}

    {#if genres.length && genreSource}
      <section>
        <h2 class="title3 sect-title">Genres</h2>
        <div class="genres">
          {#each genres as g, n}
            <button class="genre press" style:--t={tints[(n + 3) % tints.length]} onclick={() => go({ name: "discover", type: genreSource!.type, catalog: genreSource!.key, genre: g })}>{g}</button>
          {/each}
        </div>
      </section>
    {/if}

    <p class="tip foot dim">Tip: paste a link or magnet anywhere in Lumen to play it.</p>
  {/if}
</div>

<style>
  .search {
    max-width: 1280px;
    margin: 0 auto;
    padding: calc(var(--safe-top) + var(--bar-h) + 18px) var(--gutter) calc(var(--safe-bottom) + 120px);
    display: flex;
    flex-direction: column;
    gap: 20px;
  }
  .large-title {
    margin-bottom: -8px;
  }
  .box {
    position: sticky;
    top: calc(var(--safe-top) + 12px);
    z-index: 30;
    display: flex;
    align-items: center;
    gap: 8px;
    height: 44px;
    padding: 0 12px 0 14px;
    border-radius: var(--r-pill);
    background: var(--fill-3);
    backdrop-filter: blur(20px) saturate(160%);
    -webkit-backdrop-filter: blur(20px) saturate(160%);
    color: var(--label-2);
    max-width: 680px;
    transition: box-shadow var(--t-fast);
  }
  .box.focused {
    box-shadow: inset 0 0 0 1.5px color-mix(in srgb, var(--accent) 60%, transparent);
  }
  @media (min-width: 760px) {
    .box {
      top: calc(var(--safe-top) + 64px);
    }
  }
  .box input {
    flex: 1;
    min-width: 0;
    background: none;
    border: 0;
    outline: 0;
    color: var(--label);
    font-size: var(--fs-body);
    height: 100%;
  }
  .box input::placeholder {
    color: var(--label-2);
  }
  .clear {
    width: 20px;
    height: 20px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: var(--label-3);
    color: var(--bg);
  }
  .hits {
    max-width: 680px;
  }
  .hit {
    position: relative;
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    padding: 8px 16px 8px 10px;
    text-align: left;
  }
  .hit:active {
    background: var(--cell-press);
  }
  .hit + .hit::before {
    content: "";
    position: absolute;
    top: 0;
    left: 64px;
    right: 0;
    height: 0.5px;
    background: var(--separator);
  }
  .hit img,
  .ph {
    width: 40px;
    height: 60px;
    object-fit: cover;
    border-radius: 7px;
    flex: none;
    background: var(--fill-3);
  }
  .st {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .nm {
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .hit :global(.chev) {
    color: var(--label-3);
  }
  .filter {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 10px;
  }
  .results h2 {
    margin-bottom: 12px;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(var(--card-w), 1fr));
    gap: 20px 14px;
  }
  .grid :global(.poster) {
    width: auto;
  }
  .ghost {
    aspect-ratio: 2 / 3;
    border-radius: var(--poster-r);
  }
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    gap: 6px;
    padding: 70px 20px;
    color: var(--label-3);
  }
  .empty p {
    margin: 0;
    color: var(--label);
  }
  .empty .dim {
    color: var(--label-2);
    max-width: 360px;
  }
  .sect-h {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 10px;
  }
  .recent-list {
    display: flex;
    gap: 8px;
    margin: 0 calc(-1 * var(--gutter));
    padding: 0 var(--gutter);
  }
  .recent-list .chip {
    flex: none;
    font-weight: 500;
  }
  .sect-title {
    margin-bottom: 12px;
  }
  .tiles {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
    gap: 12px;
  }
  .tile {
    position: relative;
    height: 104px;
    border-radius: var(--r-md);
    padding: 14px;
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
    text-align: left;
    overflow: hidden;
    color: #fff;
    background:
      radial-gradient(120% 120% at 100% 0%, color-mix(in srgb, var(--t) 50%, white 0%), transparent 60%),
      linear-gradient(150deg, color-mix(in srgb, var(--t) 82%, black 18%), color-mix(in srgb, var(--t) 45%, black 55%));
    text-shadow: 0 1px 8px rgb(0 0 0 / 0.35);
    box-shadow: inset 0 0.5px 0 rgb(255 255 255 / 0.3);
  }
  .tile-name {
    font-size: var(--fs-head);
    font-weight: 700;
    letter-spacing: -0.02em;
  }
  .tile-sub {
    font-size: var(--fs-caption);
    font-weight: 500;
    opacity: 0.92;
  }
  .vibe-card {
    position: relative;
    isolation: isolate;
    overflow: hidden;
    margin-top: 12px;
    width: 100%;
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 16px 18px 16px 16px;
    border-radius: var(--r-md);
    text-align: left;
    color: #fff;
    background: #17121f;
  }
  .vibe-card .blobs {
    position: absolute;
    inset: -60%;
    z-index: -1;
    background:
      radial-gradient(30% 50% at 25% 50%, color-mix(in srgb, var(--ambient-1, #a86bff) 75%, #7b4dff), transparent),
      radial-gradient(30% 55% at 70% 40%, color-mix(in srgb, var(--ambient-2, #ff6bb5) 70%, #ff4f9a), transparent),
      radial-gradient(25% 50% at 50% 75%, color-mix(in srgb, var(--ambient-3, #4fc3ff) 65%, #3aa0ff), transparent);
    filter: blur(28px) saturate(1.3);
    opacity: 0.85;
    animation: drift 14s ease-in-out infinite alternate;
  }
  .vibe-card::after {
    content: "";
    position: absolute;
    inset: 0;
    z-index: -1;
    background: linear-gradient(90deg, rgb(0 0 0 / 0.45), rgb(0 0 0 / 0.1));
  }
  @keyframes drift {
    to {
      transform: translate(10%, -6%) rotate(18deg);
    }
  }
  .vc-icon {
    flex: none;
    width: 44px;
    height: 44px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: rgb(255 255 255 / 0.18);
    backdrop-filter: blur(10px);
    -webkit-backdrop-filter: blur(10px);
  }
  .vc-txt {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .vc-txt .foot {
    opacity: 0.85;
  }
  .genres {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .genre {
    height: 38px;
    padding: 0 16px;
    border-radius: var(--r-pill);
    font-weight: 600;
    font-size: var(--fs-sub);
    background: color-mix(in srgb, var(--t) 22%, transparent);
    color: color-mix(in oklab, var(--t) 35%, var(--label));
    box-shadow: inset 0 0 0 0.5px color-mix(in srgb, var(--t) 40%, transparent);
  }
  .tip {
    text-align: center;
    margin: 10px 0 0;
  }
  @media (max-width: 759px) {
    .tiles {
      grid-template-columns: 1fr 1fr;
      gap: 10px;
    }
    .tile {
      height: 92px;
      padding: 12px;
    }
    .grid {
      grid-template-columns: repeat(3, 1fr);
      gap: 16px 10px;
    }
  }
</style>
