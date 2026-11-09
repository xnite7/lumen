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
  <button class="arrow left" class:on={inside} class:near={near === "left"} onclick={() => page(-1)} aria-label="Scroll left"><span class="disc"><Icon name="back" size={22} /></span></button>
{/if}
{#if canRight}
  <button class="arrow right" class:on={inside} class:near={near === "right"} onclick={() => page(1)} aria-label="Scroll right"><span class="disc"><Icon name="forward" size={22} /></span></button>
{/if}

<style>
  /* a round dark button over the row's edge, chevron in the middle */
  .arrow {
    position: absolute;
    z-index: 5;
    top: var(--arrow-top, 10px);
    bottom: var(--arrow-bottom, 22px);
    width: 64px;
    display: grid;
    place-items: center;
    color: #fff;
    background: none;
    opacity: 0;
    pointer-events: none;
    transition: opacity 200ms ease;
  }
  .arrow.left {
    left: 6px;
  }
  .arrow.right {
    right: 6px;
  }
  .disc {
    width: 46px;
    height: 46px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: rgb(52 22 36 / 0.78);
    backdrop-filter: blur(12px) saturate(1.4);
    -webkit-backdrop-filter: blur(12px) saturate(1.4);
    box-shadow:
      inset 0 0 0 1px rgb(255 255 255 / 0.1),
      0 8px 22px -8px rgb(0 0 0 / 0.7);
    transition:
      transform 220ms var(--spring, ease),
      background 200ms ease;
  }
  .arrow.on {
    opacity: 0.75;
    pointer-events: auto;
  }
  .arrow.near,
  .arrow:hover {
    opacity: 1;
  }
  .arrow.near .disc,
  .arrow:hover .disc {
    transform: scale(1.1);
    background: rgb(70 28 48 / 0.9);
  }
  @media (max-width: 759px), (pointer: coarse) {
    .arrow {
      display: none;
    }
  }
</style>
