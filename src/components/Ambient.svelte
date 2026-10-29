<script lang="ts">
  // The light behind everything: the focused artwork, enormous and blurred,
  // cross-faded when it changes. On top of it the page is mostly glass, so
  // the whole interface takes the colour of what you're looking at.
  import { untrack } from "svelte";
  import { img } from "../lib/api";
  import { ambient } from "../lib/stores/ambient.svelte";
  import { app } from "../lib/stores/app.svelte";
  import { gaze } from "../lib/stores/gaze.svelte";

  let layers = $state<{ id: number; src: string }[]>([]);
  let n = 0;

  $effect(() => {
    const src = img(ambient.art, "w780");
    if (!src) return;
    const id = ++n;
    untrack(() => (layers = [...layers.slice(-1), { id, src }]));
  });

  // Head-tracked parallax (Tobii): the backdrop drifts opposite your head,
  // like looking through a window.
  const shift = $derived.by(() => {
    if (!app.settings.tobii.enabled || !app.settings.tobii.headParallax) return "";
    const k = app.settings.tobii.parallaxStrength;
    const x = Math.max(-1, Math.min(1, gaze.head.x / 220)) * 18 * k;
    const y = Math.max(-1, Math.min(1, gaze.head.y / 160)) * 12 * k;
    return `translate3d(${-x}px, ${y}px, 0) scale(1.08)`;
  });
</script>

<div class="ambient" aria-hidden="true" class:off={app.playing}>
  {#each layers as l (l.id)}
    <div class="ambient-layer" style:background-image="url({l.src})" style:transform={shift || undefined}></div>
  {/each}
  <div class="wash"></div>
  <div class="scanlines"></div>
</div>

<style>
  .ambient {
    position: fixed;
    inset: 0;
    z-index: 0;
    pointer-events: none;
    overflow: hidden;
    opacity: var(--ambient-strength, 0.75);
    transition: opacity var(--t-slow) ease;
  }
  .ambient.off {
    opacity: 0;
  }
  .ambient-layer {
    position: absolute;
    inset: -12%;
    background-size: cover;
    background-position: center 30%;
    filter: blur(80px) saturate(1.5) brightness(0.7);
    transform: scale(1.08);
    animation: in 1.2s var(--ease-out) both;
    transition: transform 400ms linear;
    will-change: opacity;
  }
  @keyframes in {
    from {
      opacity: 0;
    }
  }
  .wash {
    position: absolute;
    inset: 0;
    background:
      radial-gradient(90% 70% at 85% 0%, color-mix(in srgb, var(--ambient-1) 22%, transparent), transparent 60%),
      radial-gradient(80% 80% at 0% 100%, color-mix(in srgb, var(--ambient-2) 16%, transparent), transparent 60%),
      linear-gradient(180deg, transparent 0%, color-mix(in srgb, var(--ink) 55%, transparent) 55%, var(--ink) 100%);
  }
  /* 2000s-television texture, nearly invisible except in themes that ask for it */
  .scanlines {
    position: absolute;
    inset: 0;
    opacity: 0;
    background: repeating-linear-gradient(0deg, rgb(0 0 0 / 0.35) 0 1px, transparent 1px 3px);
    mix-blend-mode: multiply;
  }
</style>
