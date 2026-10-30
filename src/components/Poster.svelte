<script lang="ts">
  import { img, type Meta } from "../lib/api";
  import { app } from "../lib/stores/app.svelte";
  import { openPreview, closePreview, preview } from "../lib/stores/preview.svelte";
  import { inLibrary } from "../lib/stores/library.svelte";
  import { dwell } from "../lib/stores/gaze.svelte";
  import { openDetail } from "../lib/open";
  import Icon from "./Icon.svelte";
  import { pressMenu } from "../lib/menu.svelte";
  import { posterMenu } from "../lib/actions";

  let {
    meta,
    rank = 0,
    landscape = false,
    progress = 0,
    subtitle = "",
    showTitle = false,
    hidden = false,
    onclick,
  }: {
    meta: Meta;
    rank?: number;
    landscape?: boolean;
    progress?: number;
    subtitle?: string;
    showTitle?: boolean;
    hidden?: boolean;
    onclick?: (m: Meta, el: HTMLElement) => void;
  } = $props();

  let el: HTMLElement | undefined = $state();
  let loaded = $state(false);
  let failed = $state(false);
  let timer: ReturnType<typeof setTimeout> | undefined;

  const src = $derived(img(landscape ? (meta.background ?? meta.poster) : meta.poster, landscape ? "w780" : "medium"));
  const owned = $derived(inLibrary(meta.id));

  // Intent = the pointer sitting STILL, not merely passing over.
  function arm() {
    clearTimeout(timer);
    if (!app.settings.general.hoverPreviews || hidden) return;
    timer = setTimeout(() => el && openPreview(meta, el), app.settings.general.hoverDelayMs);
  }
  function disarm() {
    clearTimeout(timer);
    if (preview.meta?.id === meta.id) closePreview(160);
  }
  function click() {
    clearTimeout(timer);
    if (onclick && el) onclick(meta, el);
    else openDetail(meta, el);
  }
</script>

<button
  bind:this={el}
  class="poster press"
  class:landscape
  class:hidden
  class:ranked={rank > 0}
  onmouseenter={arm}
  onmousemove={arm}
  onmouseleave={disarm}
  onclick={click}
  use:dwell={() => el && !hidden && openPreview(meta, el)}
  use:pressMenu={() => (hidden ? null : posterMenu(meta))}
  aria-label={meta.name}
>
  {#if rank > 0}
    <span class="rank" aria-hidden="true">{rank}</span>
  {/if}
  <div class="art" class:loaded>
    {#if src && !failed}
      <img {src} alt="" loading="lazy" decoding="async" onload={() => (loaded = true)} onerror={() => (failed = true)} draggable="false" />
    {/if}
    {#if !loaded || failed}
      <div class="fallback" class:skeleton={!failed}>
        {#if failed}<span>{meta.name}</span>{/if}
      </div>
    {/if}
    {#if hidden}
      <div class="veil"><Icon name="eyeOff" size={22} /></div>
    {/if}
    {#if owned && !landscape}
      <span class="badge" title="In your library"><Icon name="check" size={13} /></span>
    {/if}
    {#if progress > 0}
      <div class="progress"><span style:width="{Math.max(4, progress * 100)}%"></span></div>
    {/if}
  </div>
  {#if showTitle || landscape}
    <div class="label">
      <span class="name">{meta.name}</span>
      {#if subtitle}<span class="sub">{subtitle}</span>{/if}
    </div>
  {/if}
</button>

<style>
  .poster {
    position: relative;
    flex: 0 0 auto;
    width: var(--card-w);
    text-align: left;
    border-radius: var(--poster-r);
    scroll-snap-align: start;
  }
  .poster.landscape {
    width: calc(var(--card-w) * 1.9);
  }
  .poster.ranked {
    margin-left: calc(var(--card-w) * 0.42);
  }
  .rank {
    position: absolute;
    right: 100%;
    bottom: -0.08em;
    margin-right: -0.12em;
    font-family: var(--font-display);
    font-weight: 800;
    font-size: calc(var(--card-w) * 0.98);
    line-height: 1;
    letter-spacing: -0.08em;
    color: transparent;
    -webkit-text-stroke: 2px color-mix(in srgb, var(--text) 55%, transparent);
    pointer-events: none;
  }
  .art {
    position: relative;
    aspect-ratio: 2 / 3;
    border-radius: var(--poster-r);
    overflow: hidden;
    background: var(--fill-4);
    box-shadow:
      0 10px 28px -14px rgb(0 0 0 / 0.65),
      inset 0 0 0 0.5px rgb(255 255 255 / 0.12);
    transition:
      transform var(--t-med) var(--spring),
      box-shadow var(--t-med) ease;
  }
  .landscape .art {
    aspect-ratio: 16 / 9;
  }
  @media (hover: hover) {
    .poster:hover .art {
      transform: translateY(-4px) scale(1.02);
      box-shadow:
        0 22px 40px -18px rgb(0 0 0 / 0.8),
        inset 0 0 0 1px rgb(255 255 255 / 0.18);
    }
  }
  :global(.poster.pressing) .art {
    transform: scale(0.95);
    transition-duration: 480ms;
    transition-timing-function: ease-out;
  }
  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    opacity: 0;
    transition: opacity 360ms ease;
  }
  .loaded img {
    opacity: 1;
  }
  .fallback {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    padding: 12px;
    text-align: center;
    font-weight: 600;
    color: var(--text-dim);
  }
  .hidden img {
    filter: blur(22px) saturate(0.6);
    transform: scale(1.15);
  }
  .veil {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    color: var(--text-dim);
  }
  .badge {
    position: absolute;
    top: 8px;
    right: 8px;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: color-mix(in srgb, var(--accent) 88%, transparent);
    color: var(--accent-ink);
    box-shadow: 0 2px 8px rgb(0 0 0 / 0.35);
  }
  .progress {
    position: absolute;
    left: 10px;
    right: 10px;
    bottom: 10px;
    height: 4px;
    border-radius: 4px;
    background: rgb(255 255 255 / 0.28);
    overflow: hidden;
    backdrop-filter: blur(6px);
  }
  .progress span {
    display: block;
    height: 100%;
    background: var(--accent);
    border-radius: inherit;
  }
  .label {
    display: flex;
    flex-direction: column;
    padding: 9px 2px 0;
    min-width: 0;
  }
  .name {
    font-weight: 600;
    font-size: var(--fs-foot);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .sub {
    font-size: var(--fs-caption);
    color: var(--label-2);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
