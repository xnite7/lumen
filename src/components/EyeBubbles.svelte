<script lang="ts" module>
  export type Bubble = { id: string; icon: string; label: string; run: () => void; repeat?: boolean };
</script>

<script lang="ts">
  // Eye controls (Tobii). Look at the bottom-left corner and liquid-glass
  // bubbles float up along the left edge in a "(" curve, big and well apart
  // so a resting gaze never lands on two. Rest your eyes on one: a ring fills,
  // it squashes like jelly and does its thing. Look away from the left side
  // and they sink back down.
  import { gaze } from "../lib/stores/gaze.svelte";
  import { app } from "../lib/stores/app.svelte";
  import Icon from "./Icon.svelte";

  let { items, onopen }: { items: Bubble[]; onopen?: () => void } = $props();

  const OPEN_MS = 380; // dwell in the corner to summon
  const CLOSE_MS = 1400; // away from the left side to dismiss
  const SIZE = 124;

  let open = $state(false);
  let target = $state<string | null>(null);
  let fill = $state(0); // 0..1 dwell progress on target
  let corner = $state(0); // 0..1 corner summon progress
  let popped = $state<string | null>(null);
  let vw = $state(innerWidth);
  let vh = $state(innerHeight);

  const enabled = $derived(app.settings.tobii.enabled && app.settings.tobii.gazeControls);
  const dwellMs = $derived(app.settings.tobii.dwellMs);

  // "(" layout: points on a circle whose centre sits far to the right, so
  // the middle bubble is nearest the edge and the ends curve inwards.
  const spots = $derived.by(() => {
    const n = items.length;
    const span = Math.min(vh * 0.74, (SIZE + 46) * (n - 1));
    const A = (34 * Math.PI) / 180;
    const R = span / 2 / Math.sin(A);
    const cy = vh * 0.5;
    const left = 40 + SIZE / 2;
    return items.map((_, i) => {
      const t = n === 1 ? 0 : -A + (2 * A * i) / (n - 1);
      return { x: left + R * (1 - Math.cos(t)), y: cy + R * Math.sin(t) };
    });
  });

  let raf = 0;
  let cornerSince = 0;
  let awaySince = 0;
  let targetSince = 0;
  let firedOn: string | null = null;

  function loop(now: number) {
    raf = requestAnimationFrame(loop);
    if (!enabled) return;
    const live = gaze.active && gaze.inside && gaze.present;
    const gx = gaze.x;
    const gy = gaze.y;

    if (!open) {
      const inCorner = live && gx < vw * 0.16 && gy > vh * 0.76;
      if (inCorner) {
        if (!cornerSince) cornerSince = now;
        corner = Math.min(1, (now - cornerSince) / OPEN_MS);
        if (corner >= 1) {
          open = true;
          awaySince = 0;
          corner = 0;
          cornerSince = 0;
          onopen?.();
        }
      } else {
        cornerSince = 0;
        corner = Math.max(0, corner - 0.08);
      }
      return;
    }

    // open: stay while the eyes are on the left side
    if (live && gx < vw * 0.42) awaySince = 0;
    else if (!awaySince) awaySince = now;
    else if (now - awaySince > CLOSE_MS) {
      close();
      return;
    }

    // which bubble? (hysteresis: keep the current one a bit longer)
    let hit: string | null = null;
    if (live) {
      let best = Infinity;
      items.forEach((b, i) => {
        const s = spots[i];
        const d = Math.hypot(gx - s.x, gy - s.y);
        const r = (SIZE / 2) * (b.id === target ? 1.45 : 1.15);
        if (d < r && d < best) {
          best = d;
          hit = b.id;
        }
      });
    }
    if (hit !== target) {
      target = hit;
      targetSince = now;
      fill = 0;
      if (hit !== firedOn) firedOn = null;
    }
    if (!target) {
      firedOn = null;
      return;
    }
    const b = items.find((x) => x.id === target);
    if (!b) return;
    if (firedOn === target && !b.repeat) {
      fill = 1;
      return;
    }
    fill = Math.min(1, (now - targetSince) / dwellMs);
    if (fill >= 1) {
      b.run();
      popped = b.id;
      setTimeout(() => popped === b.id && (popped = null), 520);
      firedOn = target;
      // repeatable ones (seek) go again after another full dwell
      targetSince = now;
      if (b.repeat) fill = 0;
    }
  }

  function close() {
    open = false;
    target = null;
    fill = 0;
    firedOn = null;
  }

  $effect(() => {
    if (!enabled) return;
    raf = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(raf);
  });

  const C = 2 * Math.PI * (SIZE / 2 - 5);
</script>

<svelte:window bind:innerWidth={vw} bind:innerHeight={vh} />

{#if enabled}
  <!-- the corner breathes as your eyes settle there -->
  <div class="corner" style:--c={corner} class:hide={open} aria-hidden="true"></div>

  {#if open}
    <div class="bubbles" role="toolbar" aria-label="Eye controls">
      {#each items as b, i (b.id)}
        {@const s = spots[i]}
        <div class="spot" style:left="{s.x}px" style:top="{s.y}px" style:--i={i}>
          <div class="bubble" class:on={target === b.id} class:pop={popped === b.id} style:--size="{SIZE}px">
            <span class="sheen" aria-hidden="true"></span>
            <svg class="ring" viewBox="0 0 {SIZE} {SIZE}" aria-hidden="true">
              <circle cx={SIZE / 2} cy={SIZE / 2} r={SIZE / 2 - 5} style:stroke-dasharray={C} style:stroke-dashoffset={target === b.id ? C * (1 - fill) : C} />
            </svg>
            <span class="ic"><Icon name={b.icon} size={40} /></span>
          </div>
          <span class="label" class:on={target === b.id}>{b.label}</span>
        </div>
      {/each}
    </div>
  {/if}
{/if}

<style>
  .corner {
    position: fixed;
    z-index: 120;
    left: -160px;
    bottom: -160px;
    width: 320px;
    height: 320px;
    border-radius: 50%;
    pointer-events: none;
    background: radial-gradient(circle, rgb(var(--hi) / calc(var(--c) * 0.32)), transparent 62%);
    transform: scale(calc(0.6 + var(--c) * 0.6));
    opacity: calc(var(--c) * 1.4);
    transition: opacity 200ms ease;
  }
  .corner.hide {
    opacity: 0;
  }
  .bubbles {
    position: fixed;
    inset: 0;
    z-index: 120;
    pointer-events: none;
  }
  .spot {
    position: absolute;
    display: flex;
    align-items: center;
    gap: 18px;
    transform: translate(-62px, -50%);
    animation: rise 760ms both;
    animation-delay: calc(var(--i) * 55ms);
  }
  /* float up from the corner with a springy overshoot */
  @keyframes rise {
    0% {
      opacity: 0;
      transform: translate(-140px, calc(-50% + 220px)) scale(0.3);
    }
    55% {
      opacity: 1;
      transform: translate(-58px, calc(-50% - 10px)) scale(1.1);
    }
    75% {
      transform: translate(-63px, calc(-50% + 4px)) scale(0.96);
    }
    100% {
      transform: translate(-62px, -50%) scale(1);
    }
  }

  .bubble {
    position: relative;
    flex: none;
    width: var(--size);
    height: var(--size);
    display: grid;
    place-items: center;
    color: var(--white);
    border-radius: 50%;
    /* liquid glass: clear body, tinted depth, bright rim, soft caustic */
    background:
      radial-gradient(circle at 32% 26%, rgb(var(--hi) / 0.5), rgb(var(--hi) / 0.1) 26%, transparent 44%),
      radial-gradient(circle at 70% 82%, rgb(160 210 255 / 0.28), transparent 46%),
      radial-gradient(circle at 50% 50%, rgb(var(--hi) / 0.06), rgb(var(--hi) / 0.16) 78%, rgb(var(--hi) / 0.32) 100%);
    backdrop-filter: blur(14px) saturate(190%) brightness(1.08);
    -webkit-backdrop-filter: blur(14px) saturate(190%) brightness(1.08);
    box-shadow:
      inset 0 0 0 1.5px rgb(var(--hi) / 0.38),
      inset 10px 12px 26px rgb(var(--hi) / 0.22),
      inset -12px -16px 30px rgb(var(--lo) / 0.28),
      0 22px 44px -10px rgb(var(--lo) / 0.55);
    animation: wobble 5.5s ease-in-out infinite;
    animation-delay: calc(var(--i) * -1.1s);
    transition:
      scale 420ms linear(0, 0.35, 0.86, 1.1, 1.13, 1.08, 1.02, 0.99, 1),
      box-shadow 300ms ease;
  }
  /* the surface never quite holds still */
  @keyframes wobble {
    0%,
    100% {
      border-radius: 50% 50% 50% 50% / 50% 50% 50% 50%;
      translate: 0 0;
    }
    25% {
      border-radius: 53% 47% 51% 49% / 48% 53% 47% 52%;
      translate: 0 -4px;
    }
    50% {
      border-radius: 48% 52% 47% 53% / 52% 47% 53% 48%;
      translate: 0 2px;
    }
    75% {
      border-radius: 51% 49% 54% 46% / 47% 51% 49% 53%;
      translate: 0 -2px;
    }
  }
  .bubble.on {
    scale: 1.14;
    box-shadow:
      inset 0 0 0 2px rgb(var(--hi) / 0.6),
      inset 10px 12px 26px rgb(var(--hi) / 0.3),
      inset -12px -16px 30px rgb(var(--lo) / 0.24),
      0 0 0 10px rgb(var(--hi) / 0.06),
      0 26px 50px -10px rgb(var(--lo) / 0.6);
  }
  /* boing: squash, stretch, settle */
  .bubble.pop {
    animation: boing 520ms ease-out;
  }
  @keyframes boing {
    0% {
      transform: scale(1, 1);
    }
    18% {
      transform: scale(1.28, 0.76);
    }
    38% {
      transform: scale(0.86, 1.18);
    }
    58% {
      transform: scale(1.08, 0.94);
    }
    78% {
      transform: scale(0.97, 1.03);
    }
    100% {
      transform: scale(1, 1);
    }
  }
  .bubble.pop::after {
    content: "";
    position: absolute;
    inset: 0;
    border-radius: 50%;
    border: 2px solid rgb(var(--hi) / 0.7);
    animation: ripple 520ms ease-out forwards;
  }
  @keyframes ripple {
    to {
      transform: scale(1.7);
      opacity: 0;
    }
  }
  .sheen {
    position: absolute;
    top: 9%;
    left: 20%;
    width: 42%;
    height: 24%;
    border-radius: 50%;
    background: linear-gradient(180deg, rgb(var(--hi) / 0.75), rgb(var(--hi) / 0));
    filter: blur(1.5px);
    transform: rotate(-24deg);
  }
  .ring {
    position: absolute;
    inset: 0;
    transform: rotate(-90deg);
  }
  .ring circle {
    fill: none;
    stroke: var(--white);
    stroke-width: 5;
    stroke-linecap: round;
    filter: drop-shadow(0 0 6px rgb(var(--hi) / 0.8));
  }
  .ic {
    position: relative;
    display: grid;
    filter: drop-shadow(0 2px 6px rgb(var(--lo) / 0.45));
  }
  .label {
    padding: 7px 14px;
    border-radius: 999px;
    background: rgb(var(--lo) / 0.55);
    color: rgb(var(--hi) / 0.85);
    font-size: 15px;
    font-weight: 600;
    white-space: nowrap;
    opacity: 0.55;
    transform: translateX(-6px);
    transition:
      opacity 200ms ease,
      transform 300ms var(--spring);
  }
  .label.on {
    opacity: 1;
    transform: none;
    color: var(--white);
  }
</style>
