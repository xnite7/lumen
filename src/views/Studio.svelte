<script lang="ts">
  // A studio's page: its logo up top, a follow button (their new releases get
  // a row on Home), then what they made: films and shows, most popular first.
  import { api, type Studio, type Meta } from "../lib/api";
  import { follows, loadFollows, isFollowing, toggleFollow } from "../lib/stores/follows.svelte";
  import Poster from "../components/Poster.svelte";
  import Controls from "../components/Controls.svelte";
  import Icon from "../components/Icon.svelte";
  import NavBar from "../ui/NavBar.svelte";

  let { id, label, logo }: { id: number; label?: string; logo?: string | null } = $props();
  let data = $state<Studio | null>(null);
  let failed = $state(false);
  let kind = $state<"movie" | "series">("movie");

  $effect(() => {
    void id;
    loadFollows();
    data = null;
    api
      .company(id)
      .then((r) => {
        data = r;
        // a studio that only makes TV opens on its shows
        if (!r.movies.length && r.shows.length) kind = "series";
      })
      .catch(() => (failed = true));
  });

  const name = $derived(data?.name ?? label ?? "studio");
  const mark = $derived(data?.logo ?? logo ?? null);
  const items = $derived<Meta[]>(kind === "movie" ? (data?.movies ?? []) : (data?.shows ?? []));
  const following = $derived(isFollowing("studio", id) && follows.loaded);
</script>

<NavBar title={name} />

<div class="studio">
  <header class="head">
    <div class="mark" class:none={!mark}>
      {#if mark}<img src={mark} alt={name} />{:else}<span>{name}</span>{/if}
    </div>
    <div class="who">
      <span class="kicker">studio</span>
      <h1 class="large-title">{name}</h1>
      {#if data?.hq || data?.country}<p class="foot dim">{[data.hq, data.country].filter(Boolean).join(" · ")}</p>{/if}
    </div>
    <button class="btn press follow" class:on={following} onclick={() => toggleFollow({ kind: "studio", id, name, img: mark })}>
      <Icon name={following ? "check" : "plus"} size={16} />{following ? "following" : "follow"}
    </button>
  </header>

  {#if data?.description}<p class="about dim">{data.description}</p>{/if}

  {#if data && data.movies.length && data.shows.length}
    <div class="bar">
      <Controls kind="segmented" bind:value={kind} options={[{ value: "movie", label: "Films" }, { value: "series", label: "Shows" }]} label="Kind" />
    </div>
  {/if}

  {#if !data && !failed}
    <div class="grid">{#each Array(12) as _}<div class="ghost skeleton"></div>{/each}</div>
  {:else if failed}
    <p class="empty dim">couldn't load this studio. try again in a bit.</p>
  {:else if items.length}
    <div class="grid">
      {#each items as m (m.id)}<Poster meta={m} />{/each}
    </div>
  {:else}
    <p class="empty dim">nothing from {name} here yet.</p>
  {/if}
</div>

<style>
  .studio {
    padding: calc(var(--safe-top) + var(--bar-h) + 22px) var(--gutter) calc(var(--safe-bottom) + 120px);
  }
  .head {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 18px 24px;
    margin-bottom: 18px;
  }
  /* logos are drawn for light backgrounds, so they sit on a light card */
  .mark {
    width: 180px;
    height: 110px;
    flex: none;
    display: grid;
    place-items: center;
    padding: 18px;
    border-radius: 20px;
    background: #f4f1f3;
    box-shadow: 0 14px 40px -16px #000;
  }
  .mark img {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
  }
  .mark.none {
    background: rgb(255 255 255 / 0.06);
    font: 800 18px var(--font-display);
    text-align: center;
  }
  .who {
    display: grid;
    gap: 2px;
    min-width: 0;
    flex: 1;
  }
  .kicker {
    font-size: 13px;
    font-weight: 700;
    color: var(--label-2);
  }
  .large-title {
    margin: 0;
  }
  .follow {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    background: var(--label);
    color: var(--black, #000);
  }
  .follow.on {
    background: rgb(255 255 255 / 0.1);
    color: var(--label);
  }
  .about {
    max-width: 70ch;
    margin: 0 0 20px;
    line-height: 1.5;
    font-size: 14px;
  }
  .bar {
    margin-bottom: 20px;
  }
  .grid {
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
  .empty {
    padding: 60px 0;
    text-align: center;
  }
  @media (max-width: 759px) {
    .grid {
      grid-template-columns: repeat(3, 1fr);
      gap: 16px 10px;
    }
    .mark {
      width: 140px;
      height: 86px;
    }
  }
</style>
