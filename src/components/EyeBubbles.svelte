<script lang="ts" module>
  export type Bubble = {
    id: string;
    icon: string;
    label: string;
    run: () => void;
    repeat?: boolean;
    /** small text inside the bubble, e.g. "10" on the seek arrows */
    badge?: string;
  };
</script>

<script lang="ts">
  // Eye controls (Tobii). Look at the bottom-left corner: an arc draws itself
  // around it while little droplets climb the curve, then liquid-glass
  // bubbles float up along the left edge in a "(" over a soft blur.
  //
  // Rest your eyes on one: a ring fills (and drains back just as smoothly
  // if you look away), then it squashes like jelly and does its thing.
  // Bubbles dim while your eyes are elsewhere and sink away after 3 s of not
  // looking near them. A fresh panel ignores your gaze for a moment, so
  // reading the labels never presses anything.
  import { gaze } from "../lib/stores/gaze.svelte";
  import { app } from "../lib/stores/app.svelte";
  import Icon from "./Icon.svelte";

  let { items, onopen, timeLeft = "", endsAt = "" }: { items: Bubble[]; onopen?: () => void; timeLeft?: string; endsAt?: string } = $props();

  const OPEN_MS = 650; // look at the corner this long to summon
  const HIDE_MS = 3000; // not looking near any bubble this long: they go
  const GRACE_MS = 900; // a new panel: read first, nothing fires
  const MIN_DWELL = 1300; // never quicker than this, whatever the setting
  const SIZE = 124;

  let open = $state(false);
  let target = $state<string | null>(null);
  let fills = $state<Record<string, number>>({}); // 0..1 per bubble, eased both ways
  let corner = $state(0); // 0..1 summon progress
  let near = $state(true); // eyes close to the bubbles
  let popped = $state<string | null>(null);
  let vw = $state(innerWidth);
  let vh = $state(innerHeight);

  const enabled = $derived(app.settings.tobii.enabled && app.settings.tobii.gazeControls);
  const dwellMs = $derived(Math.max(MIN_DWELL, app.settings.tobii.dwellMs));

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
  let last = 0;
  let openedAt = 0;
  let nearAt = 0;
  let firedOn: string | null = null;
  let cooldownUntil = 0;

  const ease = (t: number) => t * t * (3 - 2 * t); // smoothstep, same curve in and out

  function loop(now: number) {
    raf = requestAnimationFrame(loop);
    const dt = last ? Math.min(64, now - last) : 16;
    last = now;
    if (!enabled) return;
    const live = gaze.active && gaze.inside && gaze.present;
    const gx = gaze.x;
    const gy = gaze.y;

    if (!open) {
      const inCorner = live && gx < vw * 0.16 && gy > vh * 0.76;
      corner = inCorner ? Math.min(1, corner + dt / OPEN_MS) : Math.max(0, corner - dt / (OPEN_MS * 0.8));
      if (corner >= 1) {
        open = true;
        openedAt = nearAt = now;
        corner = 0;
        fills = {};
        firedOn = null;
        onopen?.();
      }
      return;
    }

    // how close are the eyes to the nearest bubble?
    let best = Infinity;
    let hit: string | null = null;
    let hitD = Infinity;
    if (live) {
      items.forEach((b, i) => {
        const s = spots[i];
        const d = Math.hypot(gx - s.x, gy - s.y);
        const r = (SIZE / 2) * (b.id === target ? 1.4 : 1.1);
        if (d < best) best = d;
        if (d < r && d < hitD) {
          hitD = d;
          hit = b.id;
        }
      });
    }
    near = best < SIZE * 1.9;
    if (near) nearAt = now;
    else if (now - nearAt > HIDE_MS) return close();

    if (now - openedAt < GRACE_MS || now < cooldownUntil) hit = null;
    if (hit !== target) {
      target = hit;
      if (hit !== firedOn) firedOn = null;
    }
    if (!target) firedOn = null;

    // fill the one you're looking at, drain the rest at the same pace
    const step = dt / dwellMs;
    const next: Record<string, number> = {};
    for (const b of items) {
      const f = fills[b.id] ?? 0;
      const held = b.id === target && !(firedOn === b.id && !b.repeat);
      next[b.id] = held ? Math.min(1, f + step) : Math.max(0, f - step * 1.4);
    }
    fills = next;

    const b = target ? items.find((x) => x.id === target) : null;
    if (b && (fills[b.id] ?? 0) >= 1) {
      b.run();
      popped = b.id;
      setTimeout(() => popped === b.id && (popped = null), 520);
      firedOn = b.id;
      fills = { ...fills, [b.id]: 0 };
      cooldownUntil = now + 450;
      nearAt = now;
    }
  }

  function close() {
    open = false;
    target = null;
    fills = {};
    firedOn = null;
  }

  $effect(() => {
    if (!enabled) return;
    last = 0;
    raf = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(raf);
  });

  const C = 2 * Math.PI * (SIZE / 2 - 5);
  // corner arc: a quarter circle hugging the bottom-left corner
  const ARC_R = 150;
  const ARC_L = (Math.PI / 2) * ARC_R;
  const drops = [0.22, 0.45, 0.68, 0.9];
</script>

<svelte:window bind:innerWidth={vw} bind:innerHeight={vh} />

{#if enabled}
  <!-- summon: an arc draws itself round the corner, droplets climb it -->
  {#if !open && corner > 0.01}
    <svg class="summon" width="240" height="240" viewBox="0 0 240 240" aria-hidden="true" style:--c={corner}>
      <defs>
        <filter id="eb-glow" x="-50%" y="-50%" width="200%" height="200%"><feGaussianBlur stdDeviation="4" /></filter>
      </defs>
      <path class="arc-glow" d="M0 {240 - ARC_R} A{ARC_R} {ARC_R} 0 0 1 {ARC_R} 240" style:stroke-dasharray={ARC_L} style:stroke-dashoffset={ARC_L * (1 - ease(corner))} filter="url(#eb-glow)" />
      <path class="arc" d="M0 {240 - ARC_R} A{ARC_R} {ARC_R} 0 0 1 {ARC_R} 240" style:stroke-dasharray={ARC_L} style:stroke-dashoffset={ARC_L * (1 - ease(corner))} />
      {#each drops as at, i}
        {@const th = (Math.PI / 2) * (1 - at)}
        {@const x = ARC_R * 0.6 * Math.sin(th) + 8}
        {@const y = 240 - ARC_R * 0.6 * Math.cos(th) - 8}
        {@const k = ease(Math.max(0, Math.min(1, (corner - at * 0.7) / 0.3)))}
        <circle class="drop" cx={x} cy={y} r={5 + i * 2.5} style:--k={k} style:transform-origin="{x}px {y}px" />
      {/each}
    </svg>
  {/if}

  {#if open}
    <div class="veil" class:far={!near} aria-hidden="true"></div>
    {#if timeLeft}
      <div class="left-time" class:far={!near} aria-live="polite">
        <span class="lt-big">{timeLeft}</span>
        {#if endsAt}<span class="lt-sub">ends at {endsAt}</span>{/if}
      </div>
    {/if}
    <div class="bubbles" class:far={!near} role="toolbar" aria-label="Eye controls">
      {#each items as b, i (b.id)}
        {@const s = spots[i]}
        {@const f = fills[b.id] ?? 0}
        <div class="spot" style:left="{s.x}px" style:top="{s.y}px" style:--i={i}>
          <div class="bubble" class:on={target === b.id} class:pop={popped === b.id} style:--size="{SIZE}px">
            <span class="sheen" aria-hidden="true"></span>
            <svg class="ring" viewBox="0 0 {SIZE} {SIZE}" aria-hidden="true">
              <circle class="track" cx={SIZE / 2} cy={SIZE / 2} r={SIZE / 2 - 5} />
              <circle class="fill" cx={SIZE / 2} cy={SIZE / 2} r={SIZE / 2 - 5} style:stroke-dasharray={C} style:stroke-dashoffset={C * (1 - ease(f))} style:opacity={f > 0.001 ? 1 : 0} />
            </svg>
            <span class="ic"><Icon name={b.icon} size={b.badge ? 46 : 40} /></span>
            {#if b.badge}<span class="badge">{b.badge}</span>{/if}
          </div>
          <span class="label" class:on={target === b.id}>{b.label}</span>
        </div>
      {/each}
    </div>
  {/if}
{/if}

<style>
  /* ----------------------------------------------------------- summon */
  .summon {
    position: fixed;
    z-index: 120;
    left: 0;
    bottom: 0;
    pointer-events: none;
    overflow: visible;
    opacity: calc(0.3 + var(--c) * 0.7);
  }
  .arc,
  .arc-glow {
    fill: none;
    stroke-linecap: round;
  }
  .arc {
    stroke: var(--text);
    stroke-width: 3;
  }
  .arc-glow {
    stroke: var(--neon-soft);
    stroke-width: 9;
    opacity: 0.75;
  }
  .drop {
    fill: rgb(var(--hi) / 0.16);
    stroke: rgb(var(--hi) / 0.7);
    stroke-width: 1.5;
    transform: scale(var(--k));
    opacity: var(--k);
    filter: drop-shadow(0 0 6px rgb(var(--neon-rgb) / 0.6));
  }

  /* -------------------------------------------- blur behind the panel */
  .veil {
    position: fixed;
    z-index: 119;
    inset: 0 auto 0 0;
    width: min(560px, 46vw);
    pointer-events: none;
    background: linear-gradient(90deg, rgb(var(--lo) / 0.55), rgb(var(--lo) / 0.25) 60%, transparent);
    backdrop-filter: blur(22px) saturate(120%);
    -webkit-backdrop-filter: blur(22px) saturate(120%);
    -webkit-mask-image: linear-gradient(90deg, #000 55%, transparent);
    mask-image: linear-gradient(90deg, #000 55%, transparent);
    animation: veil-in 420ms var(--ease-out) both;
    transition: opacity 500ms ease;
  }
  @keyframes veil-in {
    from {
      opacity: 0;
    }
  }
  .veil.far {
    opacity: 0.45;
  }

  /* ------------------------------------------------------ time left */
  .left-time {
    position: fixed;
    z-index: 120;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    display: grid;
    justify-items: center;
    gap: 4px;
    padding: 18px 30px;
    border-radius: var(--r-xl);
    pointer-events: none;
    background: rgb(var(--lo) / 0.45);
    backdrop-filter: blur(20px);
    -webkit-backdrop-filter: blur(20px);
    box-shadow: inset 0 0 0 1px rgb(var(--hi) / 0.08);
    animation: veil-in 500ms var(--ease-out) both;
    transition: opacity 500ms ease;
  }
  .left-time.far {
    opacity: 0.4;
  }
  .lt-big {
    font-family: var(--font-display);
    font-weight: 800;
    font-size: clamp(34px, 4vw, 56px);
    letter-spacing: -0.03em;
    line-height: 1;
    color: var(--text);
  }
  .lt-sub {
    font-family: var(--font-bump);
    font-size: 14px;
    color: var(--lamp);
  }

  /* -------------------------------------------------------- bubbles */
  .bubbles {
    position: fixed;
    inset: 0;
    z-index: 120;
    pointer-events: none;
    transition: opacity 500ms ease;
  }
  .bubbles.far {
    opacity: 0.38;
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
    color: var(--text);
    border-radius: 50%;
    /* liquid glass: clear body, tinted depth, bright rim, soft caustic */
    background:
      radial-gradient(circle at 32% 26%, rgb(var(--hi) / 0.45), rgb(var(--hi) / 0.08) 26%, transparent 44%),
      radial-gradient(circle at 70% 82%, rgb(var(--neon-rgb) / 0.22), transparent 46%),
      radial-gradient(circle at 50% 50%, rgb(var(--lo) / 0.25), rgb(var(--hi) / 0.12) 78%, rgb(var(--hi) / 0.3) 100%);
    backdrop-filter: blur(16px) saturate(170%) brightness(0.9);
    -webkit-backdrop-filter: blur(16px) saturate(170%) brightness(0.9);
    box-shadow:
      inset 0 0 0 1.5px rgb(var(--hi) / 0.38),
      inset 10px 12px 26px rgb(var(--hi) / 0.18),
      inset -12px -16px 30px rgb(var(--lo) / 0.35),
      0 22px 44px -10px rgb(var(--lo) / 0.6);
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
      inset 10px 12px 26px rgb(var(--hi) / 0.26),
      inset -12px -16px 30px rgb(var(--lo) / 0.3),
      0 0 0 10px rgb(var(--hi) / 0.05),
      0 0 40px -6px rgb(var(--neon-rgb) / 0.45),
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
    background: linear-gradient(180deg, rgb(var(--hi) / 0.7), rgb(var(--hi) / 0));
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
    stroke-width: 5;
  }
  .ring .track {
    stroke: rgb(var(--hi) / 0.1);
  }
  .ring .fill {
    stroke: var(--lamp);
    stroke-linecap: round;
    filter: drop-shadow(0 0 6px rgb(var(--lamp-rgb) / 0.8));
    transition: opacity 200ms ease;
  }
  .ic {
    position: relative;
    display: grid;
    filter: drop-shadow(0 2px 6px rgb(var(--lo) / 0.45));
  }
  .badge {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -38%);
    font-family: var(--font-display);
    font-weight: 800;
    font-size: 14px;
    letter-spacing: -0.02em;
    pointer-events: none;
  }
  .label {
    padding: 7px 14px;
    border-radius: 999px;
    background: rgb(var(--lo) / 0.6);
    backdrop-filter: blur(10px);
    color: rgb(var(--hi) / 0.85);
    font-size: 15px;
    font-weight: 600;
    white-space: nowrap;
    opacity: 0.6;
    transform: translateX(-6px);
    transition:
      opacity 200ms ease,
      transform 300ms var(--spring);
  }
  .label.on {
    opacity: 1;
    transform: none;
    color: var(--text);
  }
</style>
