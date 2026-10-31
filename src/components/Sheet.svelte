<script lang="ts">
  // iOS 27 sheet. On phones it floats inset from the screen edges with the
  // device's corner radius and follows your finger -- a quick flick or a
  // long pull dismisses it; otherwise it springs back. On wide screens it's
  // a centred card (or a side panel with `side`).
  import type { Snippet } from "svelte";
  import Icon from "./Icon.svelte";
  import { haptic } from "../lib/native";
  import { portal } from "../lib/portal";

  let {
    open = $bindable(false),
    title = "",
    width = 640,
    side = false,
    closeButton = true,
    children,
    header,
    onclose,
  }: {
    open?: boolean;
    title?: string;
    width?: number;
    side?: boolean;
    closeButton?: boolean;
    children: Snippet;
    header?: Snippet;
    onclose?: () => void;
  } = $props();

  let panel: HTMLElement | undefined = $state();
  let content: HTMLElement | undefined = $state();
  let dy = $state(0);
  let dragging = $state(false);
  let closing = $state(false);
  let startY = 0;
  let lastY = 0;
  let lastT = 0;
  let vel = 0;

  function close() {
    if (closing) return;
    closing = true;
    setTimeout(() => {
      open = false;
      closing = false;
      dy = 0;
      onclose?.();
    }, 260);
  }

  function down(e: PointerEvent) {
    if (side || e.button !== 0) return;
    const t = e.target as HTMLElement;
    // only start a drag from the header, or from content scrolled to the top
    const inContent = content?.contains(t);
    if (inContent && (content!.scrollTop > 0 || t.closest("input, textarea, button, a, [role=slider]"))) return;
    if (!inContent && t.closest("button, input")) return;
    dragging = true;
    startY = lastY = e.clientY;
    lastT = performance.now();
    vel = 0;
    panel?.setPointerCapture(e.pointerId);
  }
  function move(e: PointerEvent) {
    if (!dragging) return;
    const now = performance.now();
    vel = (e.clientY - lastY) / Math.max(1, now - lastT);
    lastY = e.clientY;
    lastT = now;
    const raw = e.clientY - startY;
    // rubber-band upward
    dy = raw > 0 ? raw : -Math.sqrt(-raw) * 3;
  }
  function up() {
    if (!dragging) return;
    dragging = false;
    if (dy > 140 || (vel > 0.6 && dy > 24)) {
      haptic("light");
      close();
    } else dy = 0;
  }

  $effect(() => {
    if (open) {
      closing = false;
      requestAnimationFrame(() => panel?.focus());
    }
  });
</script>

<svelte:window onkeydown={(e) => open && e.key === "Escape" && close()} />

{#if open}
  <div use:portal class="scrim" class:closing onclick={close} role="presentation" style:opacity={dy > 0 ? Math.max(0.2, 1 - dy / 500) : undefined}></div>
  <div
    use:portal
    class="sheet glass strong"
    class:side
    class:closing
    class:dragging
    bind:this={panel}
    style:--w="{width}px"
    style:--dy="{dy}px"
    onpointerdown={down}
    onpointermove={move}
    onpointerup={up}
    onpointercancel={up}
    role="dialog"
    aria-modal="true"
    aria-label={title}
    tabindex="-1"
  >
    {#if !side}<div class="grab" aria-hidden="true"><span></span></div>{/if}
    {#if header}
      {@render header()}
    {:else if title || closeButton}
      <header>
        <h2 class="headline">{title}</h2>
        {#if closeButton}
          <button class="x press" onclick={close} aria-label="Close"><Icon name="close" size={15} /></button>
        {/if}
      </header>
    {/if}
    <div class="content" bind:this={content}>
      {@render children()}
    </div>
  </div>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 80;
    background: var(--scrim);
    animation: fade var(--t-med) ease both;
    transition: opacity var(--t-med) ease;
  }
  .scrim.closing {
    opacity: 0 !important;
  }
  @keyframes fade {
    from {
      opacity: 0;
    }
  }
  .sheet {
    position: fixed;
    z-index: 81;
    left: 50%;
    bottom: max(12px, var(--safe-bottom));
    width: min(var(--w), calc(100vw - 24px));
    max-height: min(88vh, 920px);
    border-radius: var(--r-sheet);
    display: flex;
    flex-direction: column;
    outline: none;
    transform: translate(-50%, var(--dy));
    animation: rise var(--t-slow) var(--spring) both;
    transition: transform var(--t-slow) var(--spring);
    touch-action: pan-x;
  }
  .sheet.dragging {
    transition: none;
  }
  .sheet.closing {
    transform: translate(-50%, 110%);
    transition: transform 260ms var(--ease-out);
  }
  @keyframes rise {
    from {
      transform: translate(-50%, 105%);
    }
  }
  .sheet.side {
    left: auto;
    right: 12px;
    top: 12px;
    bottom: 12px;
    max-height: none;
    transform: none;
    animation-name: slide;
  }
  .sheet.side.closing {
    transform: translateX(110%);
  }
  @keyframes slide {
    from {
      transform: translateX(105%);
    }
  }
  .grab {
    display: grid;
    place-items: center;
    height: 16px;
    flex: none;
    cursor: grab;
  }
  .grab span {
    width: 36px;
    height: 5px;
    border-radius: 3px;
    background: var(--fill-1);
  }
  header {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    min-height: 44px;
    padding: 0 60px;
    flex: none;
  }
  header h2 {
    text-align: center;
  }
  .x {
    position: absolute;
    right: 14px;
    top: 50%;
    margin-top: -15px;
    width: 30px;
    height: 30px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: var(--fill-2);
    color: var(--label-2);
  }
  .content {
    overflow-y: auto;
    padding: 6px 20px 22px;
    overscroll-behavior: contain;
    touch-action: pan-y;
  }
  @media (max-width: 759px) {
    .sheet {
      width: calc(100vw - 16px);
      bottom: max(8px, calc(var(--safe-bottom) - 26px));
    }
    .content {
      padding: 6px 16px calc(var(--safe-bottom) + 8px);
    }
  }
</style>
