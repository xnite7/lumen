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
  import { searchBar, searchHint } from "../lib/search.svelte";

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
  // "brkng bed" -> did you mean Breaking Bad? (asked only when the search
  // looks lost: no title matches as you type, or nothing came back)
  let fix = $state<string | null>(null);
  let fixFor = "";
  function maybeFix(text: string) {
    const t = text.trim();
    if (t.length < 3 || fixFor === t) return;
    fixFor = t;
    api.didYouMean(t).then((f) => q.trim() === t && (fix = f)).catch(() => {});
  }

  $effect(() => {
    api.recents().then((r) => (recents = r ?? []));
    api.catalogs().then((c) => (catalogs = c.filter((x) => !x.extra.some((e) => e.isRequired && e.name === "search"))));
    let un: (() => void) | undefined;
    listen<any>("search", (c) => {
      if (c.reqId !== reqId) return;
      if (c.finished) {
        pending = false;
        if (!groups.some((g) => g.metas.length)) maybeFix(q);
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

  // wide: the field lives in the top bar
  $effect(() => {
    if (ui.phone) return;
    const t = searchBar.q;
    if (t === q) return;
    q = t;
    onInput();
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
      fix = null;
      if (q.trim())
        api
          .suggest(q)
          .then((s) => {
            suggestions = (s ?? []).slice(0, 5);
            if (!suggestions.length) maybeFix(q);
          })
          .catch(() => {});
      else suggestions = [];
    }, 250);
  }

  function pick(r: string) {
    fix = null;
    if (!ui.phone) searchBar.q = r;
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

</script>

<NavBar title="Search" />

<div class="search" class:wide={!ui.phone}>
  {#if ui.phone}
  <h1 class="large-title">Search</h1>

  <form class="box" class:focused onsubmit={(e) => { e.preventDefault(); run(q); input?.blur(); }}>
    <Icon name="search" size={18} />
    <input
      bind:this={input}
      bind:value={q}
      oninput={onInput}
      onfocus={() => (focused = true)}
      onblur={() => setTimeout(() => (focused = false), 150)}
      placeholder={searchHint.text}
      spellcheck="false"
      autocapitalize="off"
      enterkeyhint="search"
      aria-label="Search"
    />
    {#if q}<button type="button" class="clear" onclick={clear} aria-label="Clear"><Icon name="close" size={11} /></button>{/if}
  </form>
  {/if}

  {#if searching}
    {#if fix}
      <button class="dym press" onclick={() => pick(fix!)}>did you mean <b>{fix}</b>?</button>
    {/if}

    {#if groups.length}
      <div class="filter">
        <Controls kind="segmented" bind:value={kind} options={[{ value: "all", label: "All" }, { value: "movie", label: "Films" }, { value: "series", label: "Shows" }]} label="Kind" />
        {#if hiddenAdult}
          <button class="chip press" onclick={() => (app.adultRevealed = true)}><Icon name="eye" size={13} /> Show hidden</button>
        {/if}
        {#if pending}<span class="foot faint">still asking around…</span>{/if}
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
          <p class="bump-text">nothing called “{q}”. not even in the cushions.</p>
          <p class="sub dim">Check the spelling, or add an addon that searches this kind of thing.</p>
        </div>
      {/if}
    {/each}
  {:else}
    {#if recents.length}
      <section class="recents">
        <div class="sect-h">
          <h2 class="title3">recent</h2>
          <button class="btn plain small" onclick={() => { api.recentsClear(); recents = []; }}>Clear</button>
        </div>
        <div class="recent-list scroll-x">
          {#each recents as r}
            <button class="chip press" onclick={() => pick(r)}><Icon name="search" size={13} /> {r}</button>
          {/each}
        </div>
      </section>
    {/if}

    <p class="tip foot dim">Tip: paste a link or magnet anywhere in couch to play it.</p>
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
  .tip {
    text-align: center;
    margin: 10px 0 0;
  }
  @media (max-width: 759px) {
    .grid {
      grid-template-columns: repeat(3, 1fr);
      gap: 16px 10px;
    }
  }
  .dym {
    align-self: flex-start;
    padding: 10px 16px;
    border-radius: 14px;
    font-size: 15px;
    color: var(--label-2);
    background: rgb(255 255 255 / 0.06);
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.1);
  }
  .dym b {
    color: var(--label);
    text-decoration: underline 1.5px rgb(var(--neon-rgb) / 0.7);
    text-underline-offset: 3px;
  }
  .dym:hover {
    background: rgb(255 255 255 / 0.1);
  }
</style>
