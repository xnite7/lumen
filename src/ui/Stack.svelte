<script lang="ts">
  // The navigation stack as real layers. The top one is the screen; the one
  // under it waits, rendered, so the iOS back swipe can reveal it: the top
  // screen follows your finger 1:1, the one beneath slides in from a third
  // of the way with a parallax and its shade lifting, and letting go hands
  // your finger's speed to a spring that finishes the move -- or springs
  // it back if you didn't mean it.
  import { nav, back } from "../lib/router.svelte";
  import { ui } from "../lib/ui.svelte";
  import { app } from "../lib/stores/app.svelte";
  import { springGesture } from "../lib/motion";
  import { haptic } from "../lib/native";
  import PageView from "./PageView.svelte";
  import Ambient from "../components/Ambient.svelte";

  let { hidden = false }: { hidden?: boolean } = $props();

  let slots: (HTMLElement | undefined)[] = $state([]);
  let shade: HTMLElement | undefined = $state();
  const n = $derived(nav.stack.length);

  // gesture state (plain variables: per-frame work never touches Svelte)
  let tracking = false;
  let decided = false;
  let x0 = 0;
  let y0 = 0;
  let width = 1;
  let dx = 0;
  let samples: { x: number; t: number }[] = [];
  let cancelSpring: (() => void) | undefined;
  let raf = 0;
  let swiping = $state(false);

  const top = () => slots[n - 1];
  const under = () => slots[n - 2];

  function paint() {
    raf = 0;
    const t = top();
    const u = under();
    const p = Math.min(1, Math.max(0, dx / width));
    if (t) t.style.transform = `translate3d(${dx}px,0,0)`;
    if (u) u.style.transform = `translate3d(${-0.3 * width * (1 - p)}px,0,0)`;
    if (shade) shade.style.opacity = String(0.38 * (1 - p));
  }
  function schedule() {
    if (!raf) raf = requestAnimationFrame(paint);
  }

  function begin(e: PointerEvent) {
    if (n < 2 || e.pointerType === "mouse") return;
    cancelSpring?.();
    tracking = true;
    decided = false;
    x0 = e.clientX;
    y0 = e.clientY;
    width = window.innerWidth;
    dx = 0;
    samples = [{ x: e.clientX, t: e.timeStamp }];
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }

  function move(e: PointerEvent) {
    if (!tracking) return;
    const mx = e.clientX - x0;
    const my = e.clientY - y0;
    if (!decided) {
      // a few points of travel decide: sideways is ours, vertical lets go
      if (Math.hypot(mx, my) < 6) return;
      if (Math.abs(my) > Math.abs(mx) * 1.2) {
        tracking = false;
        return;
      }
      decided = true;
      swiping = true;
      x0 = e.clientX; // no jump: the page starts moving from here
    }
    dx = Math.max(0, e.clientX - x0);
    samples.push({ x: e.clientX, t: e.timeStamp });
    if (samples.length > 6) samples.shift();
    schedule();
  }

  function end() {
    if (!tracking) return;
    tracking = false;
    if (!decided) return;
    // release velocity from the last ~80 ms of movement
    const last = samples[samples.length - 1];
    const first = samples.find((s) => last.t - s.t < 80) ?? samples[0];
    const v = last && first && last.t > first.t ? (last.x - first.x) / (last.t - first.t) : 0;
    // projected landing point decides, like UIKit: distance plus momentum
    const projected = dx + v * 180;
    const commit = projected > width * 0.5 || v > 0.55;
    const from = dx;
    cancelSpring = springGesture({
      from,
      to: commit ? width : 0,
      velocity: v,
      stiffness: commit ? 380 : 520,
      onUpdate: (val) => {
        dx = val;
        paint();
      },
      onDone: () => {
        if (commit) {
          haptic("light");
          reset(true);
          back({ animate: false });
        } else reset(false);
        swiping = false;
      },
    });
  }

  function reset(popped: boolean) {
    const t = top();
    const u = under();
    if (u) u.style.transform = "";
    if (t && !popped) t.style.transform = "";
    if (shade) shade.style.opacity = "0";
    dx = 0;
  }

  const enabled = $derived(n > 1 && (ui.phone || app.platform === "ios") && !ui.onboarding);
</script>

<div class="stack" class:hidden class:swiping style:--win-controls={app.platform === "windows" || app.platform === "linux" ? "118px" : "0px"}>
  {#each nav.stack as r, i (r._k)}
    <div class="slot" class:top={i === n - 1} class:under={i === n - 2} class:deep={i < n - 2} bind:this={slots[i]}>
      {#if i >= n - 2}<Ambient />{/if}
      <PageView route={r} index={i} active={i === n - 1} />
      {#if i === n - 2}<div class="shade" bind:this={shade}></div>{/if}
    </div>
  {/each}
  {#if enabled}
    <!-- the left screen edge belongs to the back gesture -->
    <div class="edge" onpointerdown={begin} onpointermove={move} onpointerup={end} onpointercancel={end} role="presentation"></div>
  {/if}
</div>

<style>
  .stack {
    position: relative;
    height: 100%;
    overflow: hidden;
  }
  .stack.hidden {
    visibility: hidden;
  }
  .slot {
    position: absolute;
    inset: 0;
    isolation: isolate;
    overflow: hidden;
    background: var(--bg);
  }
  .slot.top,
  .slot.under {
    will-change: transform;
  }
  .slot :global(.layer) {
    z-index: 1;
  }
  /* the screen beneath is ready but unseen until a swipe reveals it */
  .slot.under {
    visibility: hidden;
  }
  .swiping .slot.under {
    visibility: visible;
  }
  /* deeper screens keep their state and scroll, skip all rendering */
  .slot.deep {
    visibility: hidden;
    content-visibility: hidden;
  }
  /* only the top screen takes part in push/pop View Transitions */
  .slot:not(.top) :global(*) {
    view-transition-name: none !important;
  }
  .swiping .slot.top {
    box-shadow: -18px 0 40px rgb(0 0 0 / 0.3);
  }
  .shade {
    z-index: 2;
    position: absolute;
    inset: 0;
    background: #000;
    opacity: 0;
    pointer-events: none;
  }
  .edge {
    position: absolute;
    z-index: 30;
    top: 0;
    bottom: 0;
    left: 0;
    width: 22px;
    touch-action: none;
  }
  .swiping {
    pointer-events: none;
  }
  .swiping .edge {
    pointer-events: auto;
  }
</style>
