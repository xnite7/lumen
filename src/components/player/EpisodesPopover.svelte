<script lang="ts">
  import { fallback, imdbEpisode } from "../../lib/fallback";
  // The episode list in the player's bar: season header with arrows, one
  // row per episode with a little progress line; the one under the pointer
  // (or the one playing) opens up with its still and summary.
  import { img, type Video } from "../../lib/api";
  import Icon from "../Icon.svelte";

  let {
    seasons,
    season = $bindable(),
    videos,
    currentId,
    watched,
    progress,
    onplay,
  }: {
    seasons: number[];
    season: number;
    videos: Video[];
    currentId: string;
    watched: Set<string>;
    progress: (v: Video) => number;
    onplay: (v: Video) => void;
  } = $props();

  let open = $state<string>("");
  $effect(() => {
    open = videos.find((v) => v.id === currentId)?.id ?? videos[0]?.id ?? "";
  });
  const label = (v: Video) => v.title || v.name || `Episode ${v.episode ?? v.number}`;
  const si = $derived(seasons.indexOf(season));
  const released = (v: Video) => !v.released || new Date(v.released).getTime() <= Date.now();
</script>

<div class="eps">
  <header>
    <button class="arrow" disabled={si <= 0} onclick={() => (season = seasons[si - 1])} aria-label="Previous season"><Icon name="back" size={18} /></button>
    <span class="headline">{season === 0 ? "Specials" : `Season ${season}`}</span>
    <button class="arrow" disabled={si >= seasons.length - 1} onclick={() => (season = seasons[si + 1])} aria-label="Next season"><Icon name="forward" size={18} /></button>
  </header>
  <div class="list">
    {#each videos as v (v.id)}
      {@const isOpen = open === v.id}
      {@const p = v.id === currentId ? 1 : watched.has(v.id) ? 1 : progress(v)}
      <button class="row" class:open={isOpen} class:cur={v.id === currentId} class:locked={!released(v)} onmouseenter={() => (open = v.id)} onfocus={() => (open = v.id)} onclick={() => released(v) && onplay(v)}>
        <div class="line">
          <span class="nm">{v.episode ?? v.number} · {label(v)}</span>
          <span class="mini"><span style:width="{p * 100}%" class:now={v.id === currentId}></span></span>
        </div>
        {#if isOpen}
          <div class="more">
            {#if v.thumbnail || imdbEpisode(v.id)}<img src={img(v.thumbnail ?? imdbEpisode(v.id) ?? undefined)} alt="" loading="lazy" use:fallback={[imdbEpisode(v.id)]} />{/if}
            <p class="foot dim">{v.overview ?? v.description ?? (released(v) ? "" : `Airs ${new Date(v.released!).toLocaleDateString()}`)}</p>
          </div>
        {/if}
      </button>
    {/each}
  </div>
</div>

<style>
  .eps {
    width: 380px;
    max-width: calc(100vw - 40px);
  }
  header {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 6px 10px;
  }
  header .headline {
    flex: 1;
    text-align: center;
  }
  .arrow {
    width: 32px;
    height: 32px;
    border-radius: 50%;
    display: grid;
    place-items: center;
  }
  .arrow:hover:not(:disabled) {
    background: var(--fill-3);
  }
  .arrow:disabled {
    opacity: 0.25;
  }
  .list {
    max-height: min(56vh, 520px);
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .row {
    text-align: left;
    padding: 10px 12px;
    border-radius: var(--r-sm);
    transition: background var(--t-fast);
  }
  .row.open {
    background: rgb(var(--hi) / 0.06);
  }
  .row.cur .nm {
    color: var(--neon-soft);
  }
  .row.locked {
    opacity: 0.45;
  }
  .line {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .nm {
    flex: 1;
    min-width: 0;
    font-weight: 600;
    font-size: 15px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .mini {
    flex: none;
    width: 54px;
    height: 4px;
    border-radius: 4px;
    overflow: hidden;
    background: rgb(var(--hi) / 0.18);
  }
  .mini span {
    display: block;
    height: 100%;
    background: var(--progress);
  }
  .mini span.now {
    background: var(--neon);
  }
  .more {
    display: flex;
    gap: 12px;
    margin-top: 10px;
    animation: open 260ms var(--ease-out) both;
  }
  @keyframes open {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }
  }
  .more img {
    width: 130px;
    flex: none;
    aspect-ratio: 16 / 9;
    object-fit: cover;
    border-radius: 8px;
  }
  .more p {
    margin: 0;
    display: -webkit-box;
    -webkit-line-clamp: 4;
    line-clamp: 4;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
</style>
