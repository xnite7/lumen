<script lang="ts">
  // The couch mark. Plain: warm-white ink. Lit: the same lines glowing like
  // a neon sign (soft magenta bloom, hot white core). `flicker` stutters it
  // on like a real tube.
  import { COUCH } from "../lib/couchmark";

  let { size = 64, lit = false, flicker = false }: { size?: number; lit?: boolean; flicker?: boolean } = $props();
  const id = `c${Math.random().toString(36).slice(2, 8)}`;
  // crop of the 1024 artwork around the couch (with room for the glow)
  const VB = lit ? [110, 230, 800, 580] : [150, 280, 720, 480];
</script>

<svg
  class="logo"
  class:flicker-on={flicker}
  width={size}
  height={size * (VB[3] / VB[2])}
  viewBox={VB.join(" ")}
  role="img"
  aria-label="couch"
>
  <defs>
    <mask id="{id}m" maskUnits="userSpaceOnUse" x="0" y="0" width="1024" height="1024">
      <rect width="1024" height="1024" fill="black" />
      {#each COUCH as [d, ink]}<path {d} fill={ink ? "white" : "black"} />{/each}
    </mask>
    {#if lit}
      <filter id="{id}b" x="-20%" y="-20%" width="140%" height="140%"><feGaussianBlur stdDeviation="18" /></filter>
      <filter id="{id}s" x="-20%" y="-20%" width="140%" height="140%"><feGaussianBlur stdDeviation="5" /></filter>
    {/if}
  </defs>
  {#if lit}
    <g filter="url(#{id}b)" opacity="0.85"><rect width="1024" height="1024" fill="var(--neon)" mask="url(#{id}m)" /></g>
    <g filter="url(#{id}s)"><rect width="1024" height="1024" fill="var(--neon-soft)" mask="url(#{id}m)" /></g>
    <rect width="1024" height="1024" fill="var(--text)" mask="url(#{id}m)" opacity="0.92" />
  {:else}
    <rect width="1024" height="1024" fill="var(--text)" mask="url(#{id}m)" />
  {/if}
</svg>

<style>
  .logo {
    display: block;
    overflow: visible;
  }
</style>
