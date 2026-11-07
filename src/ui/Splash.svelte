<script lang="ts">
  // Launch: the midnite sign sits still on the first frame for a beat, then
  // redraws itself into the couch (2x), holds, and you're home.
  // plays through every time; nothing skips it.
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


<div class="splash" out:fade={{ duration: 520 }}>
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
    /* drawn at 2x and shown near its real size so the lines stay crisp */
    width: min(40vmin, 240px);
    aspect-ratio: 259 / 233;
    object-fit: contain;
    pointer-events: none;
    filter: drop-shadow(0 0 14px rgb(var(--hi) / 0.14));
  }
</style>
