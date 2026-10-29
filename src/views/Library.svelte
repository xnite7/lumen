<script lang="ts">
  import { lib, loadLibrary } from "../lib/stores/library.svelte";
  import { app } from "../lib/stores/app.svelte";
  import { api, type Meta } from "../lib/api";
  import { toast, errorText } from "../lib/stores/toast.svelte";
  import { parseIsoLike } from "../lib/time";
  import Poster from "../components/Poster.svelte";
  import Controls from "../components/Controls.svelte";
  import Icon from "../components/Icon.svelte";

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

<div class="library">
  <header>
    <h1 class="large-title">Library</h1>
    <Controls kind="segmented" bind:value={kind} options={[{ value: "all", label: "All" }, { value: "movie", label: "Films" }, { value: "series", label: "Shows" }]} label="Filter" />
    <div class="right">
      <Controls kind="segmented" bind:value={sort} options={[{ value: "recent", label: "Added" }, { value: "watched", label: "Watched" }, { value: "az", label: "A–Z" }]} label="Sort" />
      {#if app.secrets.stremio_auth}
        <button class="btn tinted small press" onclick={sync} disabled={syncing}><Icon name="refresh" size={15} class={syncing ? "spin" : ""} /> Sync</button>
      {/if}
    </div>
  </header>
  {#if items.length}
    <div class="grid">
      {#each items as i (i._id)}
        <Poster meta={asMeta(i)} showTitle progress={i.state.duration ? i.state.timeOffset / i.state.duration : 0} />
      {/each}
    </div>
  {:else if lib.loaded}
    <div class="empty">
      <Icon name="library" size={38} />
      <p class="headline">Your library is empty</p>
      <p class="foot dim">Add titles with the plus button on any poster or detail page.{app.secrets.stremio_auth ? "" : " Sign in to Stremio in Settings to bring your existing library across."}</p>
    </div>
  {/if}
</div>

<style>
  .library {
    padding: calc(var(--bar-h) + 36px) var(--gutter) 60px;
  }
  header {
    display: flex;
    align-items: center;
    gap: 22px;
    margin-bottom: 28px;
  }
  .right {
    margin-left: auto;
    display: flex;
    gap: 12px;
    align-items: center;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(var(--card-w), 1fr));
    gap: 24px 16px;
  }
  .grid :global(.poster) {
    width: auto;
  }
  .empty {
    padding: 100px 0;
    display: grid;
    justify-items: center;
    gap: 6px;
    text-align: center;
    color: var(--text-dim);
  }
  .empty p {
    margin: 0;
    max-width: 50ch;
  }
  .empty .headline {
    color: var(--text);
  }
</style>
