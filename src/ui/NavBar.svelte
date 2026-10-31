<script lang="ts">
  // Per-page navigation bar. Floating glass back button on the left, the
  // page's actions on the right, and the title fading into the middle once
  // the page's large title has scrolled under the bar (iOS large titles).
  import { getContext, type Snippet } from "svelte";
  import Icon from "../components/Icon.svelte";
  import { back, nav } from "../lib/router.svelte";
  import { ui } from "../lib/ui.svelte";

  let {
    title = "",
    showAfter = 44,
    transparent = false,
    actions,
  }: { title?: string; showAfter?: number; transparent?: boolean; actions?: Snippet } = $props();

  const page = getContext<{ index: number; y: number } | undefined>("page");
  const canBack = $derived(page ? page.index > 0 : nav.stack.length > 1);
  const shown = $derived((page ? page.y : ui.scrollY) > showAfter);
</script>

<header class="navbar" class:shown class:transparent style:view-transition-name="navbar">
  <div class="lead">
    {#if canBack}
      <button class="orb glass press" onclick={() => back()} aria-label="Back"><Icon name="back" size={21} /></button>
    {/if}
  </div>
  <h2 class="mid headline" style:visibility={shown ? "visible" : "hidden"}>{title}</h2>
  <div class="trail">
    {#if actions}{@render actions()}{/if}
  </div>
</header>
{#if !transparent}<div class="edge-top" class:on={shown}></div>{/if}

<style>
  .navbar {
    position: fixed;
    z-index: 55;
    top: 0;
    left: 0;
    right: 0;
    height: calc(var(--safe-top) + var(--bar-h) + 12px);
    padding: calc(var(--safe-top) + 10px) var(--gutter) 0;
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    align-items: center;
    pointer-events: none;
  }
  :global(html.playing) .navbar {
    opacity: 0;
  }
  .lead,
  .trail {
    display: flex;
    align-items: center;
    gap: 10px;
    pointer-events: auto;
  }
  .trail {
    grid-column: 3;
    justify-content: flex-end;
  }
  .mid {
    margin: 0;
    opacity: 0;
    transform: translateY(6px);
    transition:
      opacity var(--t-fast) ease,
      transform var(--t-med) var(--spring);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 52vw;
  }
  .mid {
    transition-property: opacity, transform, visibility;
  }
  .shown .mid {
    opacity: 1;
    transform: none;
  }
  .edge-top {
    opacity: 0;
    transition: opacity var(--t-med) ease;
  }
  .edge-top.on {
    opacity: 1;
  }
  /* wide: the tab bar owns the centre, so the title stays out of it */
  @media (min-width: 760px) {
    .mid {
      display: none;
    }
    .trail {
      padding-right: var(--win-controls, 0px);
    }
  }
</style>
