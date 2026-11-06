<script lang="ts">
  // Launch: the midnite sign sits still on the first frame for a beat, then
  // redraws itself into the couch (2x), holds, and you're home. Any tap,
  // click or key skips straight through.
  import { fade } from "svelte/transition";
  import { app } from "../lib/stores/app.svelte";
  import { hum } from "../lib/sound";
  import first from "../assets/brand/intro-first.png";
  import anim from "../assets/brand/intro.webp";
  import last from "../assets/brand/intro-last.png";

  let { ondone }: { ondone: () => void } = $props();

  const HOLD_MS = 3000; // first frame, still
  const PLAY_MS = 1220; // 61 frames x 20 ms (the drawing at 2x)
  const END_MS = 650; // the finished couch, before home fades in

  const reduced = matchMedia("(prefers-reduced-motion: reduce)").matches;
  let phase = $state<"hold" | "play" | "end">(reduced ? "end" : "hold");
  let src = $derived(phase === "hold" ? first : phase === "play" ? `${anim}?t=${Date.now()}` : last);
  let finished = false;

  function done() {
    if (finished) return;
    // the home screen needs settings; if they're not in yet, wait a beat
    if (!app.ready) return void setTimeout(done, 120);
    finished = true;
    ondone();
  }

  $effect(() => {
    // warm the animation so it starts on time
    const pre = new Image();
    pre.src = anim;
    const timers = [
      setTimeout(() => {
        phase = "play";
        if (app.ready && app.settings.general.sound) hum();
      }, reduced ? 0 : HOLD_MS),
      setTimeout(() => (phase = "end"), reduced ? 0 : HOLD_MS + PLAY_MS),
      setTimeout(done, (reduced ? 0 : HOLD_MS + PLAY_MS) + END_MS),
    ];
    return () => timers.forEach(clearTimeout);
  });
</script>

<svelte:window onkeydown={done} />

<div class="splash" role="presentation" onpointerdown={done} out:fade={{ duration: 520 }}>
  <img class="intro" {src} alt="" draggable="false" />
</div>

<style>
  .splash {
    position: fixed;
    inset: 0;
    z-index: 9500;
    display: grid;
    place-items: center;
    background: var(--bg);
    cursor: default;
  }
  /* white line drawing on a transparent background */
  .intro {
    width: min(56vmin, 420px);
    aspect-ratio: 259 / 233;
    object-fit: contain;
    pointer-events: none;
    filter: drop-shadow(0 0 18px rgb(var(--hi) / 0.18));
  }
</style>
