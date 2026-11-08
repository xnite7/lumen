<script lang="ts">
  // A ClipCafe scene playing as the billboard / preview picture: a plain
  // <video>, so no YouTube buttons or branding, ever. Fades in once moving.
  let {
    src,
    muted = true,
    paused = false,
    loop = false,
    onplaying,
    onended,
  }: { src: string; muted?: boolean; paused?: boolean; loop?: boolean; onplaying?: () => void; onended?: () => void } = $props();

  let v: HTMLVideoElement | undefined = $state();
  let on = $state(false);
  $effect(() => {
    if (!v) return;
    v.muted = muted;
    if (!muted) v.volume = 0.7;
  });
  $effect(() => {
    if (!v) return;
    if (paused) {
      on = false;
      setTimeout(() => v?.pause(), 400);
    } else v.play().catch(() => {});
  });
</script>

<div class="clip" class:on>
  <video
    bind:this={v}
    {src}
    autoplay
    playsinline
    {loop}
    preload="auto"
    muted
    onplaying={() => {
      if (paused) return;
      on = true;
      onplaying?.();
    }}
    onended={() => onended?.()}
    onerror={() => onended?.()}
  ></video>
</div>

<style>
  .clip {
    position: absolute;
    inset: 0;
    overflow: hidden;
    opacity: 0;
    transition: opacity 500ms ease;
    pointer-events: none;
  }
  .clip.on {
    opacity: 1;
  }
  video {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
</style>
