<script lang="ts">
  // A chromeless YouTube player for trailers, controlled through the iframe
  // postMessage API (no script tag from YouTube is loaded into the app).
  let {
    id,
    muted = true,
    loop = false,
    start = 0,
    fill = false,
    paused = false,
    onended,
    onplaying,
  }: { id: string; muted?: boolean; loop?: boolean; start?: number; fill?: boolean; paused?: boolean; onended?: () => void; onplaying?: () => void } = $props();

  let frame: HTMLIFrameElement | undefined = $state();
  let ready = $state(false);

  const src = $derived(
    `https://www.youtube-nocookie.com/embed/${id}?autoplay=1&mute=1&controls=0&disablekb=1&fs=0&modestbranding=1&playsinline=1&rel=0&iv_load_policy=3&cc_load_policy=0&enablejsapi=1&start=${Math.floor(start)}` +
      (loop ? `&loop=1&playlist=${id}` : "") +
      `&origin=${encodeURIComponent(location.origin)}`,
  );

  function post(func: string, args: unknown[] = []) {
    frame?.contentWindow?.postMessage(JSON.stringify({ event: "command", func, args }), "*");
  }

  $effect(() => {
    if (!ready) return;
    if (muted) post("mute");
    else {
      post("unMute");
      post("setVolume", [70]);
    }
  });

  // Held: the picture fades out first (so YouTube's own pause screen is never
  // seen), then the video stops; let go, it plays and fades back once moving.
  let hold: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    if (!ready) return;
    clearTimeout(hold);
    if (paused) {
      playing = false;
      hold = setTimeout(() => post("pauseVideo"), 450);
    } else post("playVideo");
  });

  function onMessage(e: MessageEvent) {
    if (!frame || e.source !== frame.contentWindow) return;
    let d: any;
    try {
      d = typeof e.data === "string" ? JSON.parse(e.data) : e.data;
    } catch {
      return;
    }
    if (d?.event === "onReady" || d?.event === "initialDelivery") ready = true;
    const st = d?.info?.playerState ?? (d?.event === "onStateChange" ? d.info : undefined);
    if (st === 1) {
      ready = true;
      if (paused) return;
      playing = true;
      onplaying?.();
    }
    if (st === 0) onended?.();
  }

  let playing = $state(false);

  // YouTube picks quality from the player's own pixel size, so a small card
  // got 240p. Render the player at (at least) 1600 px wide and scale it down
  // to fit: it streams 1080p and looks sharp at any size.
  const BASE = 1600;
  let w = $state(0);
  let h = $state(0);
  const k = $derived(w > 0 ? Math.min(1, w / BASE) : 1);

  function onLoad() {
    // Ask the player to start talking to us.
    frame?.contentWindow?.postMessage(JSON.stringify({ event: "listening", id: 1, channel: "widget" }), "*");
    post("addEventListener", ["onStateChange"]);
  }
</script>

<svelte:window onmessage={onMessage} />

<div class="yt" class:fill class:playing bind:clientWidth={w} bind:clientHeight={h}>
  <div class="scaler" style:width="{w / k}px" style:height="{h / k}px" style:transform="scale({k})">
  <iframe bind:this={frame} {src} title="Trailer" allow="autoplay; encrypted-media; picture-in-picture" referrerpolicy="strict-origin-when-cross-origin" onload={onLoad} tabindex="-1"></iframe>
  </div>
</div>

<style>
  .yt {
    position: absolute;
    inset: 0;
    overflow: hidden;
    opacity: 0;
    transition: opacity 420ms var(--ease-out);
    pointer-events: none;
  }
  .yt.playing {
    opacity: 1;
  }
  .scaler {
    position: absolute;
    top: 0;
    left: 0;
    transform-origin: 0 0;
  }
  iframe {
    position: absolute;
    border: 0;
    /* oversize to push YouTube's own chrome off the edges */
    width: 100%;
    height: calc(100% + 140px);
    top: -70px;
    left: 0;
  }
  /* zoom scope (2.39:1) trailers so they fill instead of letterboxing;
     pulled up further so the black bar at the top (the worst one) is gone */
  .fill iframe {
    width: 135%;
    left: -17.5%;
    height: calc(135% + 140px);
    top: calc(-26% - 70px);
  }
</style>
