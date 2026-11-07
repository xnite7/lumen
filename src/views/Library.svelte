<script lang="ts">
  // My Lists: Plan to Watch, Watching, Completed (with your rating, sortable
  // by it), Favourites, and Simkl custom lists for PRO / VIP. With Simkl the
  // lists ARE your Simkl lists (backed up and synced there); without it they
  // come from what you've added and watched in couch.
  import boil from "../assets/brand/couch-boil.webp";
  import { lib, loadLibrary } from "../lib/stores/library.svelte";
  import { app } from "../lib/stores/app.svelte";
  import { api, type Meta } from "../lib/api";
  import { parseIsoLike } from "../lib/time";
  import Poster from "../components/Poster.svelte";
  import Controls from "../components/Controls.svelte";
  import Icon from "../components/Icon.svelte";
  import NavBar from "../ui/NavBar.svelte";
  import { openMenu } from "../lib/menu.svelte";
  import { go } from "../lib/router.svelte";
  import { social, loadFavs } from "../lib/stores/social.svelte";

  type Tab = "plan" | "watching" | "completed" | "favs" | "custom";
  type Row = Meta & { userRating?: number | null; when?: number };
  const tabs: { value: Tab; label: string }[] = [
    { value: "plan", label: "Plan to Watch" },
    { value: "watching", label: "Watching" },
    { value: "completed", label: "Completed" },
    { value: "favs", label: "Favourites" },
    { value: "custom", label: "Custom lists" },
  ];
  let tab = $state<Tab>("plan");
  let kind = $state("all");
  let sort = $state<"recent" | "rating">("recent");

  let simkl = $state<{ simkl: boolean; paid?: boolean; plan?: Row[]; watching?: Row[]; completed?: Row[] } | null>(null);
  let local = $state<Record<string, { rating: number; at: number }>>({});
  let custom = $state<{ paid: boolean; lists: any[] } | null>(null);
  let openList = $state<any>(null);
  let listItems = $state<Meta[]>([]);

  $effect(() => {
    loadLibrary();
    loadFavs();
    api.myLists().then((r) => (simkl = r)).catch(() => (simkl = { simkl: false }));
    api.localRatings().then((r) => (local = r)).catch(() => {});
  });
  $effect(() => {
    if (tab === "custom" && !custom && app.secrets.simkl_token) api.simklCustomLists().then((r) => (custom = r)).catch(() => (custom = { paid: false, lists: [] }));
  });

  // no Simkl: sort your couch library into the same three lists
  const fromLocal = $derived.by(() => {
    const out: Record<"plan" | "watching" | "completed", Row[]> = { plan: [], watching: [], completed: [] };
    for (const i of lib.items) {
      if (i.removed && !i.state.timesWatched && !i.state.flaggedWatched) continue;
      if (i.temp && !i.state.timeOffset && !i.state.timesWatched) continue;
      const r: Row = { id: i._id, type: i.type, name: i.name, poster: i.poster, background: i.background, logo: i.logo, userRating: local[i._id]?.rating ?? null, when: parseIsoLike(i.state.lastWatched) || parseIsoLike(i._mtime) };
      const done = i.state.flaggedWatched || i.state.timesWatched > 0;
      const going = !done && (i.state.timeOffset > 0 || (i.state.watchedIds?.length ?? 0) > 0);
      out[done ? "completed" : going ? "watching" : "plan"].push(r);
    }
    return out;
  });

  const rows = $derived.by((): Row[] => {
    let l: Row[];
    if (tab === "favs") l = social.favs.map((f) => ({ ...f, when: f.at }) as Row);
    else if (tab === "custom") l = listItems;
    else if (simkl?.simkl) l = ((simkl[tab] ?? []) as Row[]).map((m: any) => ({ ...m, when: parseIsoLike(m.ratedAt && sort === "rating" ? m.ratedAt : m.watchedAt ?? m.addedAt) }));
    else l = fromLocal[tab];
    if (kind !== "all") l = l.filter((i) => i.type === kind);
    if (tab === "completed" && sort === "rating") l = l.slice().sort((a, b) => (b.userRating ?? -1) - (a.userRating ?? -1) || (b.when ?? 0) - (a.when ?? 0));
    else if (tab !== "custom") l = l.slice().sort((a, b) => (b.when ?? 0) - (a.when ?? 0));
    return l;
  });

  function sortMenu(e: MouseEvent) {
    openMenu(
      e.currentTarget as HTMLElement,
      [
        { label: "Most recent", icon: sort === "recent" ? "check" : undefined, run: () => (sort = "recent") },
        { label: "My rating", icon: sort === "rating" ? "check" : undefined, run: () => (sort = "rating") },
      ],
      { title: "Sort completed by" },
    );
  }

  async function pick(l: any) {
    openList = l;
    listItems = [];
    listItems = await api.simklListItems(l.id).catch(() => []);
  }

  const paid = $derived(!!(custom?.paid ?? simkl?.paid));
  const empty: Record<Tab, string> = {
    plan: "nothing planned. press + on anything you want to get to.",
    watching: "nothing on the go right now.",
    completed: "nothing finished yet. get comfy.",
    favs: "no favourites yet. tap the heart on anything you love.",
    custom: "no custom lists yet. make them on simkl.com and they show up here.",
  };
</script>

<NavBar title="My Lists">
  {#snippet actions()}
    {#if tab === "completed"}
      <button class="orb glass press" onclick={sortMenu} aria-label="Sort"><Icon name="sliders" size={19} /></button>
    {/if}
  {/snippet}
</NavBar>

<div class="lists">
  <h1 class="large-title">My Lists</h1>
  <div class="tabs scroll-x" role="tablist">
    {#each tabs as t}
      <button class="chip press" class:sel={tab === t.value} role="tab" aria-selected={tab === t.value} onclick={() => { tab = t.value; openList = null; }}>
        {#if t.value === "favs"}<Icon name="heart" size={14} fill={tab === t.value} />{/if}{t.label}
      </button>
    {/each}
  </div>

  {#if tab === "custom" && !paid}
    <div class="promo rise">
      <img class="simkl" src="https://www.google.com/s2/favicons?domain=simkl.com&sz=128" alt="Simkl" />
      <div class="tags"><span class="tag vip">VIP</span><span class="tag pro">PRO</span></div>
      <p class="title3">Custom lists live on Simkl</p>
      <p class="sub dim">Make as many lists as you like on Simkl and they show up here, backed up and synced everywhere. It's a Simkl PRO / VIP thing.</p>
      {#if app.secrets.simkl_token}
        <button class="btn primary press" onclick={() => api.openExternal("https://simkl.com/vip/")}>Get Simkl PRO or VIP</button>
      {:else}
        <button class="btn primary press" onclick={() => go({ name: "settings", section: "accounts" })}>Connect Simkl</button>
        <button class="btn ghost press" onclick={() => api.openExternal("https://simkl.com/vip/")}>About PRO / VIP</button>
      {/if}
    </div>
  {:else if tab === "custom" && !openList}
    {#if custom?.lists.length}
      <div class="cl-grid">
        {#each custom.lists as l (l.id)}
          <button class="cl press" onclick={() => pick(l)}>
            <span class="headline">{l.name}</span>
            <span class="foot dim">{l.counts?.items ?? 0} titles · {l.media_type ?? "mixed"}</span>
          </button>
        {/each}
      </div>
    {:else if custom}
      <div class="empty rise"><img class="couch-empty" src={boil} alt="" /><p>{empty.custom}</p></div>
    {/if}
  {:else}
    {#if tab === "custom" && openList}
      <button class="back press" onclick={() => (openList = null)}><Icon name="back" size={16} /> {openList.name}</button>
    {/if}
    <div class="bar">
      <Controls kind="segmented" bind:value={kind} options={[{ value: "all", label: "All" }, { value: "movie", label: "Films" }, { value: "series", label: "Shows" }]} label="Filter" />
      <span class="foot dim count">{rows.length} {rows.length === 1 ? "title" : "titles"}{tab === "completed" ? ` · ${sort === "rating" ? "by my rating" : "most recent"}` : ""}</span>
    </div>
    {#if rows.length}
      <div class="grid">
        {#each rows as i (i.id)}
          <div class="cell">
            <Poster meta={i} />
            {#if tab === "completed"}
              <span class="mine" class:none={!i.userRating}>
                {#if i.userRating}<Icon name="star" size={12} fill /> {i.userRating}/10{:else}not rated{/if}
              </span>
            {/if}
          </div>
        {/each}
      </div>
    {:else if lib.loaded}
      <div class="empty rise">
        <img class="couch-empty" src={boil} alt="" />
        <p class="bump-text">{empty[tab]}</p>
        {#if !app.secrets.simkl_token && tab !== "favs"}
          <p class="sub dim">Connect Simkl to back these up and sync them everywhere.</p>
          <button class="btn tinted press" onclick={() => go({ name: "settings", section: "accounts" })}>connect Simkl</button>
        {/if}
      </div>
    {/if}
  {/if}
</div>

<style>
  .lists {
    padding: calc(var(--safe-top) + var(--bar-h) + 18px) var(--gutter) calc(var(--safe-bottom) + 120px);
  }
  .large-title {
    margin-bottom: 14px;
  }
  .tabs {
    display: flex;
    gap: 8px;
    margin: 0 calc(-1 * var(--gutter)) 18px;
    padding: 0 var(--gutter);
  }
  .tabs .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    flex: none;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 14px;
    margin-bottom: 22px;
  }
  .count {
    margin-left: auto;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(var(--card-w), 1fr));
    gap: 22px 16px;
  }
  .grid :global(.poster) {
    width: auto;
  }
  .cell {
    display: grid;
    gap: 7px;
  }
  .mine {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font: 600 12px ui-monospace, monospace;
    color: var(--rating);
  }
  .mine.none {
    color: var(--label-3);
    font-weight: 400;
  }
  .back {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 14px;
    font-weight: 700;
    font-size: 17px;
  }
  .cl-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
    gap: 12px;
  }
  .cl {
    display: grid;
    gap: 4px;
    text-align: left;
    padding: 18px;
    border-radius: 16px;
    background: rgb(255 255 255 / 0.06);
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.09);
  }
  .cl:hover {
    background: rgb(255 255 255 / 0.1);
  }
  .promo {
    display: grid;
    justify-items: center;
    gap: 10px;
    text-align: center;
    padding: 70px 20px;
  }
  .promo .simkl {
    width: 64px;
    height: 64px;
    border-radius: 16px;
  }
  .promo .sub {
    max-width: 46ch;
    margin: 0 0 8px;
  }
  .promo .title3 {
    margin: 0;
  }
  .tags {
    display: flex;
    gap: 6px;
  }
  .tag {
    padding: 3px 9px;
    border-radius: 6px;
    font: 800 11px system-ui, sans-serif;
    letter-spacing: 0.08em;
  }
  .tag.vip {
    background: linear-gradient(135deg, #ffd36b, #ff9f43);
    color: #2a1600;
  }
  .tag.pro {
    background: linear-gradient(135deg, #8a7dff, #e0309a);
    color: #fff;
  }
  .empty {
    padding: 90px 20px;
    display: grid;
    justify-items: center;
    gap: 8px;
    text-align: center;
    color: var(--label-3);
  }
  .empty p {
    margin: 0;
    max-width: 44ch;
    color: var(--label);
  }
  .empty .dim {
    color: var(--label-2);
    margin-bottom: 10px;
  }
  @media (max-width: 759px) {
    .grid {
      grid-template-columns: repeat(3, 1fr);
      gap: 16px 10px;
    }
  }
  .couch-empty {
    width: min(220px, 50vw);
    height: auto;
    opacity: 0.85;
  }
</style>
