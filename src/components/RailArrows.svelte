<script lang="ts">
  // Netflix's row arrows: tall dark tabs at the rail's ends. They surface as
  // the pointer comes into the row and light up as it nears their side.
  import Icon from "./Icon.svelte";

  let { rail, onpage }: { rail: HTMLElement | undefined; onpage?: () => void } = $props();

  let canLeft = $state(false);
  let canRight = $state(false);
  let near = $state<"" | "left" | "right">("");
  let inside = $state(false);

  function edges() {
    if (!rail) return;
    canLeft = rail.scrollLeft > 8;
    canRight = rail.scrollLeft + rail.clientWidth < rail.scrollWidth - 8;
    onpage?.();
  }

  $effect(() => {
    const r = rail;
    if (!r) return;
    const wrap = r.parentElement!;
    const move = (e: PointerEvent) => {
      if (e.pointerType !== "mouse") return;
      inside = true;
      const b = r.getBoundingClientRect();
      const x = e.clientX - b.left;
      near = x < b.width * 0.18 ? "left" : x > b.width * 0.82 ? "right" : "";
    };
    const leave = () => {
      inside = false;
      near = "";
    };
    r.addEventListener("scroll", edges, { passive: true });
    wrap.addEventListener("pointermove", move);
    wrap.addEventListener("pointerleave", leave);
    const ro = new ResizeObserver(edges);
    ro.observe(r);
    edges();
    return () => {
      r.removeEventListener("scroll", edges);
      wrap.removeEventListener("pointermove", move);
      wrap.removeEventListener("pointerleave", leave);
      ro.disconnect();
    };
  });

  function page(dir: number) {
    rail?.scrollBy({ left: dir * rail.clientWidth * 0.82, behavior: "smooth" });
  }
</script>

{#if canLeft}
  <button class="arrow left" class:on={inside} class:near={near === "left"} onclick={() => page(-1)} aria-label="Scroll left"><Icon name="back" size={26} /></button>
{/if}
{#if canRight}
  <button class="arrow right" class:on={inside} class:near={near === "right"} onclick={() => page(1)} aria-label="Scroll right"><Icon name="forward" size={26} /></button>
{/if}

<style>
  .arrow {
    position: absolute;
    z-index: 5;
    top: var(--arrow-top, 10px);
    bottom: var(--arrow-bottom, 22px);
    width: calc(var(--gutter) - 6px);
    min-width: 34px;
    display: grid;
    place-items: center;
    color: #fff;
    background: rgb(0 0 0 / 0.55);
    backdrop-filter: blur(10px);
    -webkit-backdrop-filter: blur(10px);
    opacity: 0;
    pointer-events: none;
    transition:
      opacity 200ms ease,
      background 200ms ease;
  }
  .arrow.left {
    left: 0;
    border-radius: 0 10px 10px 0;
  }
  .arrow.right {
    right: 0;
    border-radius: 10px 0 0 10px;
  }
  .arrow.on {
    opacity: 0.55;
    pointer-events: auto;
  }
  .arrow.near,
  .arrow:hover {
    opacity: 1;
    background: rgb(0 0 0 / 0.72);
  }
  .arrow :global(svg) {
    transition: transform 200ms ease;
  }
  .arrow.near :global(svg),
  .arrow:hover :global(svg) {
    transform: scale(1.25);
  }
  @media (max-width: 759px), (pointer: coarse) {
    .arrow {
      display: none;
    }
  }
</style>
