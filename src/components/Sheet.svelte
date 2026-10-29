<script lang="ts">
  // A sheet that rises from the bottom edge (iOS) and sits in a glass panel.
  // Escape, the scrim, or dragging the grabber down closes it.
  import type { Snippet } from "svelte";
  let {
    open = $bindable(false),
    title = "",
    width = 720,
    side = false,
    children,
    header,
    onclose,
  }: { open?: boolean; title?: string; width?: number; side?: boolean; children: Snippet; header?: Snippet; onclose?: () => void } = $props();

  let panel: HTMLElement | undefined = $state();
  let dragY = $state(0);
  let dragging = false;
  let startY = 0;

  function close() {
    open = false;
    onclose?.();
  }
  function down(e: PointerEvent) {
    dragging = true;
    startY = e.clientY;
    (e.target as HTMLElement).setPointerCapture(e.pointerId);
  }
  function move(e: PointerEvent) {
    if (dragging) dragY = Math.max(0, e.clientY - startY);
  }
  function up() {
    if (!dragging) return;
    dragging = false;
    if (dragY > 120) close();
    dragY = 0;
  }
  $effect(() => {
    if (open) requestAnimationFrame(() => panel?.focus());
  });
</script>

<svelte:window onkeydown={(e) => open && e.key === "Escape" && close()} />

{#if open}
  <div class="scrim" onclick={close} role="presentation"></div>
  <div
    class="sheet glass strong"
    class:side
    bind:this={panel}
    style:--w="{width}px"
    style:transform={dragY ? `translateY(${dragY}px)` : undefined}
    style:transition={dragY ? "none" : undefined}
    role="dialog"
    aria-modal="true"
    aria-label={title}
    tabindex="-1"
  >
    {#if !side}
      <div class="grab" onpointerdown={down} onpointermove={move} onpointerup={up} role="presentation"><span></span></div>
    {/if}
    {#if header}
      {@render header()}
    {:else if title}
      <header><h2 class="title2">{title}</h2></header>
    {/if}
    <div class="content">
      {@render children()}
    </div>
  </div>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 80;
    background: rgb(0 0 0 / 0.38);
    backdrop-filter: blur(2px);
    animation: fade var(--t-med) ease both;
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
    bottom: 12px;
    width: min(var(--w), calc(100vw - 24px));
    max-height: min(86vh, 900px);
    margin-left: calc(min(var(--w), calc(100vw - 24px)) / -2);
    border-radius: var(--r-xl);
    display: flex;
    flex-direction: column;
    outline: none;
    animation: rise var(--t-slow) var(--spring) both;
    transition: transform var(--t-med) var(--spring);
  }
  .sheet.side {
    left: auto;
    right: 12px;
    top: 12px;
    bottom: 12px;
    margin-left: 0;
    max-height: none;
    animation-name: slide;
  }
  @keyframes rise {
    from {
      transform: translateY(105%);
    }
  }
  @keyframes slide {
    from {
      transform: translateX(105%);
    }
  }
  .grab {
    display: grid;
    place-items: center;
    height: 22px;
    cursor: grab;
    flex: none;
  }
  .grab span {
    width: 40px;
    height: 5px;
    border-radius: 5px;
    background: rgb(255 255 255 / 0.28);
  }
  header {
    padding: 4px 24px 12px;
    flex: none;
  }
  .content {
    overflow-y: auto;
    padding: 0 24px 24px;
    overscroll-behavior: contain;
  }
</style>
