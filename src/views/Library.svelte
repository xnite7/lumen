<script lang="ts">
  import { lib, loadLibrary } from "../lib/stores/library.svelte";
  import { app } from "../lib/stores/app.svelte";
  import { api, type Meta } from "../lib/api";
  import { toast, errorText } from "../lib/stores/toast.svelte";
  import { parseIsoLike } from "../lib/time";
  import Poster from "../components/Poster.svelte";
  import Controls from "../components/Controls.svelte";
  import Icon from "../components/Icon.svelte";
  import NavBar from "../ui/NavBar.svelte";
  import { openMenu } from "../lib/menu.svelte";
  import { go } from "../lib/router.svelte";

  const sorts = [
    { value: "recent", label: "Recently added" },
    { value: "watched", label: "Recently watched" },
    { value: "az", label: "Title" },
  ];
  function sortMenu(e: MouseEvent) {
    openMenu(
      e.currentTarget as HTMLElement,
      sorts.map((o) => ({ label: o.label, icon: sort === o.value ? "check" : undefined, run: () => (sort = o.value) })),
      { title: "Sort by" },
    );
  }

  let kind = $state("all");
  let sort = $state("recent");
  let syncing = $state(false);

  $effect(() => {
    loadLibrary();
  });

  const items = $derived.by(() => {
    let l = lib.items.filter((i) => !i.removed && !i.temp);
    if (kind !== "all") l = l.filter((i) => i.type === kind);
    if (sort === "az") l = l.slice().sort((a, b) => a.name.localeCompare(b.name));
    else if (sort === "watched") l = l.slice().sort((a, b) => parseIsoLike(b.state.lastWatched) - parseIsoLike(a.state.lastWatched));
    return l;
  });

  async function sync() {
    syncing = true;
    try {
      const r = await api.stremioSync(false);
      await loadLibrary();
      toast(`Synced with Stremio · ${r.pulled} updated here, ${r.pushed} sent`, "ok");
    } catch (e) {
      toast(errorText(e), "error");
    }
    syncing = false;
  }

  function asMeta(i: (typeof lib.items)[number]): Meta {
    return { id: i._id, type: i.type, name: i.name, poster: i.poster, background: i.background, logo: i.logo };
  }
</script>

<NavBar title="Library">
  {#snippet actions()}
    {#if app.secrets.stremio_auth}
      <button class="orb glass press" onclick={sync} disabled={syncing} aria-label="Sync with Stremio"><span class:spin={syncing}><Icon name="refresh" size={19} /></span></button>
    {/if}
    <button class="orb glass press" onclick={sortMenu} aria-label="Sort"><Icon name="sliders" size={19} /></button>
  {/snippet}
</NavBar>

<div class="library">
  <h1 class="large-title">Library</h1>
  <div class="bar">
    <Controls kind="segmented" bind:value={kind} options={[{ value: "all", label: "All" }, { value: "movie", label: "Films" }, { value: "series", label: "Shows" }]} label="Filter" />
    <span class="foot dim count">{items.length} {items.length === 1 ? "title" : "titles"}</span>
  </div>
  {#if items.length}
    <div class="grid">
      {#each items as i (i._id)}
        <Poster meta={asMeta(i)} showTitle progress={i.state.duration ? i.state.timeOffset / i.state.duration : 0} />
      {/each}
    </div>
  {:else if lib.loaded}
    <div class="empty rise">
      <Icon name="bookmark" size={46} />
      <p class="title3">Nothing here yet</p>
      <p class="sub dim">Long-press any poster and choose Add to My List.</p>
      {#if !app.secrets.stremio_auth}
        <button class="btn accent press" onclick={() => go({ name: "profile" })}>Sign in to bring your library</button>
      {/if}
    </div>
  {/if}
</div>

<style>
  .library {
    padding: calc(var(--safe-top) + var(--bar-h) + 18px) var(--gutter) calc(var(--safe-bottom) + 120px);
  }
  .large-title {
    margin-bottom: 14px;
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
  .spin {
    display: grid;
    animation: spin 0.9s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
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
</style>
