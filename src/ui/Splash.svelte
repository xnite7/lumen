<script lang="ts">
  // Launch: the room goes dark, the glass midnite sign melts into the couch,
  // a bump says something deadpan, and you're home. Any tap, click or key
  // skips straight through.
  import { fade } from "svelte/transition";
  import { app } from "../lib/stores/app.svelte";
  import { bumpLine } from "../lib/bumps";
  import { hum } from "../lib/sound";
  import Logo from "../components/Logo.svelte";
  import splash from "../assets/brand/splash.mp4";

  let { ondone }: { ondone: () => void } = $props();

  const reduced = matchMedia("(prefers-reduced-motion: reduce)").matches;
  let phase = $state<"video" | "logo" | "bump">(reduced ? "logo" : "video");
  let line = $state("");
  let finished = false;
  let video: HTMLVideoElement | undefined = $state();

  function toBump() {
    if (finished) return;
    // the bump needs settings; if they're not in yet, the couch waits a beat
    if (!app.ready) return void setTimeout(toBump, 120);
    if (!app.settings.general.bumps) return done();
    line = bumpLine("launch");
    phase = "bump";
    if (app.settings.general.sound) hum();
    setTimeout(done, 1900);
  }

  function done() {
    if (finished) return;
    if (!app.ready) return void setTimeout(done, 120);
    finished = true;
    ondone();
  }

  $effect(() => {
    if (phase === "logo") {
      const t = setTimeout(toBump, 1500);
      return () => clearTimeout(t);
    }
  });

  $effect(() => {
    if (!video) return;
    video.playbackRate = 1.3;
    video.play().catch(() => (phase = "logo"));
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
      autoplay
      preload="auto"
      onended={toBump}
      onerror={() => (phase = "logo")}
      out:fade={{ duration: 380 }}
    ></video>
  {:else if phase === "logo"}
    <div class="mark" out:fade={{ duration: 300 }}><Logo size={150} lit flicker /></div>
  {:else}
    <p class="bump-text line" in:fade={{ duration: 420, delay: 160 }}>{line}</p>
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
  .line {
    max-width: 30ch;
    padding: 0 24px;
  }
</style>
