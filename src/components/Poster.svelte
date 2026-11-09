<script lang="ts">
  import { fallback, imdbArt, imdbEpisode } from "../lib/fallback";
  import { img, type Meta } from "../lib/api";
  import { app } from "../lib/stores/app.svelte";
  import { openPreview, closePreview, preview, hoverDelay } from "../lib/stores/preview.svelte";
  import { inLibrary } from "../lib/stores/library.svelte";
  import { openDetail } from "../lib/open";
  import Icon from "./Icon.svelte";
  import { pressMenu } from "../lib/menu.svelte";
  import { posterMenu } from "../lib/actions";
  import { tvFocus } from "../lib/tvfocus";

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

  // A short rest on the poster opens it; moving across from an open preview
  // swaps straight over (Netflix).
  function arm() {
    clearTimeout(timer);
    if (!app.settings.general.hoverPreviews || hidden) return;
    timer = setTimeout(() => el && openPreview(meta, el), hoverDelay(app.settings.general.hoverDelayMs));
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
  onmouseleave={disarm}
  onclick={click}
  use:pressMenu={() => (hidden ? null : posterMenu(meta))}
  aria-label={meta.name}
>
  {#if rank > 0}
    <span class="rank" aria-hidden="true">{rank}</span>
  {/if}
  <div class="art" class:loaded use:tvFocus={{ max: landscape ? 5 : 8 }}>
    {#if src && !failed}
      <img {src} alt="" loading="lazy" decoding="async" onload={() => (loaded = true)} use:fallback={{ alts: [imdbArt(meta.id, landscape ? "background" : "poster")], onfail: () => (failed = true) }} draggable="false" />
    {/if}
    {#if !loaded || failed}
      <div class="fallback" class:skeleton={!failed} class:img-ph={failed}>
        {#if failed}<span>{meta.name}</span>{/if}
      </div>
    {/if}
    {#if hidden}
      <div class="veil"><Icon name="eyeOff" size={22} /></div>
    {/if}
    {#if owned && !landscape}
      <span class="badge" title="In your library"><Icon name="check" size={15} /></span>
    {/if}
    {#if landscape && loaded}
      <div class="lshade"></div>
      {#if meta.logo}
        <img class="llogo" src={img(meta.logo)} alt="" loading="lazy" use:fallback={[imdbArt(meta.id, "logo")]} />
      {:else}
        <span class="lname">{meta.name}</span>
      {/if}
    {/if}
    {#if progress > 0}
      <div class="progress"><span style:width="{Math.max(4, progress * 100)}%"></span></div>
    {/if}
  </div>
  {#if showTitle}
    <div class="label">
      <span class="name">{meta.name}</span>
      {#if subtitle || meta.upNext}<span class="sub">{subtitle || meta.upNext}</span>{/if}
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
    width: var(--land-w);
  }
  .poster.ranked {
    margin-left: calc(var(--card-w) * 0.42);
  }
  .rank {
    position: absolute;
    right: 100%;
    bottom: -0.08em;
    margin-right: -0.12em;
    font-family: "Inter", "Segoe UI", system-ui, sans-serif;
    font-weight: 900;
    font-style: normal;
    font-size: calc(var(--card-w) * 0.95);
    line-height: 0.8;
    letter-spacing: -0.06em;
    color: #050405;
    -webkit-text-stroke: 3px rgb(244 234 240 / 0.42);
    paint-order: stroke fill;
    pointer-events: none;
  }
  .art {
    position: relative;
    aspect-ratio: 2 / 3;
    border-radius: var(--poster-r);
    overflow: hidden;
    background: var(--surface);
    box-shadow:
      0 10px 28px -14px rgb(var(--lo) / 0.8),
      inset 0 0 0 0.5px rgb(var(--hi) / 0.1);
  }
  .landscape .art {
    aspect-ratio: 16 / 9;
    border-radius: var(--card-r);
  }
  .lshade {
    position: absolute;
    inset: 40% 0 0;
    background: linear-gradient(0deg, rgb(var(--lo) / 0.7), transparent);
  }
  img.llogo {
    position: absolute;
    left: 12px;
    bottom: 12px;
    width: auto;
    height: auto;
    max-width: 52%;
    max-height: 34%;
    object-fit: contain;
    object-position: left bottom;
    opacity: 1;
    filter: drop-shadow(0 2px 10px rgb(var(--lo) / 0.6));
  }
  :global(.poster.pressing) .art {
    transform: scale(0.95);
    transition-duration: 480ms;
    transition-timing-function: ease-out;
  }
  .art > img:first-child {
    width: 100%;
    height: 100%;
    object-fit: cover;
    opacity: 0;
    transition: opacity 360ms ease;
  }
  .loaded > img:first-child {
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
  .hidden .art > img:first-child {
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
  /* same tick as the episodes' "watched" mark */
  .badge {
    position: absolute;
    top: 8px;
    right: 8px;
    width: 26px;
    height: 26px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: var(--accent);
    color: var(--accent-ink);
    box-shadow: 0 2px 8px rgb(var(--lo) / 0.35);
  }
  .lname {
    position: absolute;
    left: 12px;
    right: 12px;
    bottom: 10px;
    font-weight: 700;
    font-size: 15px;
    color: #fff;
    text-shadow: 0 1px 8px rgb(0 0 0 / 0.7);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  /* square, inset, dark track with a white fill */
  .progress {
    position: absolute;
    left: 10px;
    right: 10px;
    bottom: 8px;
    height: 3px;
    background: linear-gradient(90deg, rgb(255 255 255 / 0.28), rgb(255 255 255 / 0.12));
  }
  .progress span {
    display: block;
    height: 100%;
    background: #fff;
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
