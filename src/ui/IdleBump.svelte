<script lang="ts">
  // Leave Home alone for a few minutes and the TV does a bump: white text on
  // black, a deadpan line, then it fades back. Anything at all brings you back.
  import { fade } from "svelte/transition";
  import { app } from "../lib/stores/app.svelte";
  import { pb } from "../lib/stores/playback.svelte";
  import { nav } from "../lib/router.svelte";
  import { bumpLine } from "../lib/bumps";

  const IDLE_MS = 4 * 60_000;
  let line = $state<string | null>(null);
  let timer = 0;
  let cycle = 0;

  const eligible = () => app.settings.general.bumps && !pb.open && nav.stack.length === 1 && nav.stack[0].name === "home" && document.visibilityState === "visible";

  function arm() {
    clearTimeout(timer);
    timer = window.setTimeout(show, IDLE_MS);
  }
  function show() {
    if (!eligible()) return arm();
    line = bumpLine("idle");
    // a new line every 9 s while you're away
    clearInterval(cycle);
    cycle = window.setInterval(() => (line = bumpLine("idle")), 9000);
  }
  function wake() {
    if (line) {
      line = null;
      clearInterval(cycle);
    }
    arm();
  }

  $effect(() => {
    arm();
    return () => {
      clearTimeout(timer);
      clearInterval(cycle);
    };
  });
</script>

<svelte:window onpointermove={wake} onpointerdown={wake} onkeydown={wake} onwheel={wake} ontouchstart={wake} />

{#if line}
  <div class="idle" transition:fade={{ duration: 900 }} aria-live="polite">
    {#key line}
      <p class="bump-text" in:fade={{ duration: 700, delay: 300 }} out:fade={{ duration: 300 }}>{line}</p>
    {/key}
  </div>
{/if}

<style>
  .idle {
    position: fixed;
    inset: 0;
    z-index: 9400;
    display: grid;
    place-items: center;
    background: var(--bg);
  }
  .idle p {
    grid-area: 1 / 1;
    max-width: 32ch;
    padding: 0 24px;
  }
</style>
