<script lang="ts">
  // One Up Next card: the still, the logo, where you are and a play button
  // that goes straight to the episode.
  import { img } from "$lib/api";
  import type { UpNext } from "../lib/feed";
  import { openTitle } from "../lib/nav.svelte";
  import { quickPlay } from "$lib/quickplay";
  let { item, w = 300 }: { item: UpNext; w?: number } = $props();
  let busy = $state(false);
  async function play(e: MouseEvent) {
    e.stopPropagation();
    if (busy) return;
    busy = true;
    try {
      await quickPlay(item.meta, null, { videoId: item.videoId });
    } finally {
      busy = false;
    }
  }
</script>

<div class="card press" style:width="{w}px" role="button" tabindex="0" onclick={() => openTitle(item.meta)} onkeydown={(e) => e.key === "Enter" && openTitle(item.meta)}>
  <div class="art">
    {#if item.meta.background || item.meta.poster}<img src={img(item.meta.background ?? item.meta.poster, "w780")} alt="" loading="lazy" />{/if}
    <div class="fade"></div>
    {#if item.meta.logo}<img class="logo" src={img(item.meta.logo, "w500")} alt={item.meta.name} />{:else}<b class="name">{item.meta.name}</b>{/if}
    {#if item.fresh}<span class="new">NEW</span>{/if}
    <button class="go glass" onclick={play} aria-label="play">{busy ? "…" : "▶"}</button>
    {#if item.progress}<div class="bar"><i style:width="{item.progress * 100}%"></i></div>{/if}
  </div>
  <div class="meta">
    {#if item.label}<b>{item.label}</b>{/if}
    {#if item.sub}<span class="muted">{item.sub}</span>{/if}
  </div>
</div>

<style>
  .card { flex: none; scroll-snap-align: start; }
  .art { position: relative; aspect-ratio: 16 / 9; border-radius: 14px; overflow: hidden; background: var(--surface2); box-shadow: 0 10px 30px -14px #000; }
  .art > img:first-child { width: 100%; height: 100%; object-fit: cover; }
  .fade { position: absolute; inset: 0; background: linear-gradient(to top, #000c, #0000 60%); }
  .logo { position: absolute; left: 12px; bottom: 14px; max-width: 46%; max-height: 38%; object-fit: contain; object-position: left bottom; filter: drop-shadow(0 2px 8px #000a); }
  .name { position: absolute; left: 12px; bottom: 12px; right: 60px; font-size: 17px; }
  .new { position: absolute; top: 10px; left: 10px; font-size: 10px; font-weight: 900; letter-spacing: 0.08em; background: var(--neon); padding: 3px 7px; border-radius: 6px; }
  .go { position: absolute; right: 10px; bottom: 10px; width: 40px; height: 40px; border-radius: 50%; font-size: 15px; display: grid; place-items: center; }
  .bar { position: absolute; left: 0; right: 0; bottom: 0; height: 3px; background: #fff3; }
  .bar i { display: block; height: 100%; background: var(--neon-soft); }
  .meta { display: flex; gap: 8px; align-items: baseline; padding: 8px 2px 0; font-size: 13px; }
</style>
