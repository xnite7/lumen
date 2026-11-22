<script lang="ts">
  import { img, type Meta } from "$lib/api";
  import { openTitle } from "../lib/nav.svelte";
  let { meta, w = 120 }: { meta: Meta; w?: number } = $props();
  let broken = $state(false);
</script>

<button class="poster press" style:width="{w}px" onclick={() => openTitle(meta)} aria-label={meta.name}>
  {#if meta.poster && !broken}
    <img src={img(meta.poster, "w342")} alt="" loading="lazy" onerror={() => (broken = true)} />
  {:else}
    <span class="ph">{meta.name}</span>
  {/if}
</button>

<style>
  .poster {
    flex: none;
    aspect-ratio: 2 / 3;
    border-radius: 12px;
    overflow: hidden;
    background: var(--surface2);
    box-shadow: 0 8px 24px -12px #000c;
    scroll-snap-align: start;
  }
  img { width: 100%; height: 100%; object-fit: cover; display: block; }
  .ph { display: grid; place-items: center; height: 100%; padding: 10px; font-weight: 700; font-size: 14px; text-align: center; color: var(--label2); }
</style>
