<script lang="ts">
  // Launch: the room goes dark, the glass midnite sign sits still for a
  // beat, then melts into the couch (at 2x) and you're home. Any tap, click
  // or key skips straight through.
  import { fade } from "svelte/transition";
  import { app } from "../lib/stores/app.svelte";
  import { hum } from "../lib/sound";
  import Logo from "../components/Logo.svelte";
  import splash from "../assets/brand/splash.mp4";

  let { ondone }: { ondone: () => void } = $props();

  const HOLD_MS = 2000; // first frame, still
  const SPEED = 2;

  const reduced = matchMedia("(prefers-reduced-motion: reduce)").matches;
  let phase = $state<"video" | "logo">(reduced ? "logo" : "video");
  let finished = false;
  let video: HTMLVideoElement | undefined = $state();

  function done() {
    if (finished) return;
    // the home screen needs settings; if they're not in yet, wait a beat
    if (!app.ready) return void setTimeout(done, 120);
    finished = true;
    ondone();
  }

  $effect(() => {
    if (phase === "logo") {
      const t = setTimeout(done, 1500);
      return () => clearTimeout(t);
    }
  });

  $effect(() => {
    const v = video;
    if (!v) return;
    v.pause();
    v.currentTime = 0;
    const t = setTimeout(() => {
      v.playbackRate = SPEED;
      if (app.ready && app.settings.general.sound) hum();
      v.play().catch(() => (phase = "logo"));
    }, HOLD_MS);
    return () => clearTimeout(t);
  });
</script>

<svelte:window onkeydown={done} />

<div class="splash" role="presentation" onpointerdown={done} out:fade={{ duration: 520 }}>
  {#if phase === "video"}
    <video
      bind:this={video}
      class="glass"
      src={splash}
      muted
      playsinline
      preload="auto"
      onended={done}
      onerror={() => (phase = "logo")}
    ></video>
  {:else}
    <div class="mark"><Logo size={180} lit flicker /></div>
  {/if}
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
  /* the clip is glass on true black: screen-blend it so the black melts
     into the plum room instead of sitting there as a box */
  .glass {
    width: min(62vmin, 520px);
    aspect-ratio: 624 / 640;
    mix-blend-mode: screen;
    pointer-events: none;
    -webkit-mask-image: radial-gradient(closest-side, #000 72%, transparent);
    mask-image: radial-gradient(closest-side, #000 72%, transparent);
  }
  .mark {
    display: grid;
    place-items: center;
  }
</style>
