<script lang="ts">
  // midnite™ glyph. Plain: a clean neon mark for small places. Lit: the
  // sign switched on -- tube outline, bloom, star glints -- for splash and
  // onboarding. `flicker` makes it stutter on like a real neon sign.
  import { GLYPH, GLYPH_W, GLYPH_H, GLINTS } from "../lib/brand";

  let { size = 64, lit = false, flicker = false }: { size?: number; lit?: boolean; flicker?: boolean } = $props();
  const id = `g${Math.random().toString(36).slice(2, 8)}`;
  const pad = lit ? 22 : 6;
</script>

<svg
  class="logo"
  class:flicker-on={flicker}
  class:lit
  width={size}
  height={size * ((GLYPH_H + pad * 2) / (GLYPH_W + pad * 2))}
  viewBox="{-pad} {-pad} {GLYPH_W + pad * 2} {GLYPH_H + pad * 2}"
  role="img"
  aria-label="midnite"
>
  {#if lit}
    <defs>
      <filter id="{id}b" x="-40%" y="-40%" width="180%" height="180%">
        <feGaussianBlur stdDeviation="6" />
      </filter>
      <filter id="{id}s" x="-40%" y="-40%" width="180%" height="180%">
        <feGaussianBlur stdDeviation="1.6" />
      </filter>
      <radialGradient id="{id}f" cx="45%" cy="40%" r="70%">
        <stop offset="0" stop-color="var(--neon-soft)" stop-opacity="0.75" />
        <stop offset="1" stop-color="var(--neon)" stop-opacity="0.55" />
      </radialGradient>
      <g id="{id}star">
        <path d="M0,-6 L0.9,-0.9 L6,0 L0.9,0.9 L0,6 L-0.9,0.9 L-6,0 L-0.9,-0.9Z" fill="var(--text)" />
      </g>
    </defs>
    <!-- bloom -->
    <path d={GLYPH} fill="var(--neon)" opacity="0.55" filter="url(#{id}b)" />
    <!-- glass fill -->
    <path d={GLYPH} fill="url(#{id}f)" />
    <!-- the tube: a soft pink halo and a hot core -->
    <path d={GLYPH} fill="none" stroke="var(--neon-soft)" stroke-width="3.4" stroke-linejoin="round" filter="url(#{id}s)" />
    <path d={GLYPH} fill="none" stroke="var(--text)" stroke-width="1.1" stroke-linejoin="round" opacity="0.92" />
    {#each GLINTS as [x, y, s], i}
      <use href="#{id}star" class="glint" style:--i={i} transform="translate({x} {y}) scale({s})" />
    {/each}
  {:else}
    <path d={GLYPH} fill="var(--neon)" />
    <path d={GLYPH} fill="none" stroke="var(--neon-soft)" stroke-width="2" stroke-linejoin="round" />
  {/if}
</svg>

<style>
  .logo {
    display: block;
    overflow: visible;
  }
  .glint {
    transform-box: fill-box;
    transform-origin: center;
    animation: twinkle 3.4s ease-in-out infinite;
    animation-delay: calc(var(--i) * -0.9s);
    filter: drop-shadow(0 0 3px var(--neon-soft));
  }
  @keyframes twinkle {
    0%,
    100% {
      opacity: 0.95;
    }
    45% {
      opacity: 0.35;
    }
    55% {
      opacity: 1;
    }
  }
</style>
