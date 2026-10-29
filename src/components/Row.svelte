<script lang="ts">
  // A catalog row. Loads only when it scrolls near the viewport (so twenty
  // addons don't fire at once), shows shimmer while waiting, hides itself if
  // the addon answers with nothing, and pages more in as you reach the end.
  import { api, type CatalogRef, type Meta } from "../lib/api";
  import { app } from "../lib/stores/app.svelte";
  import { go } from "../lib/router.svelte";
  import Poster from "./Poster.svelte";
  import Icon from "./Icon.svelte";

  let { catalog, title = "", ranked = false }: { catalog: CatalogRef; title?: string; ranked?: boolean } = $props();

  let host: HTMLElement | undefined = $state();
  let rail: HTMLElement | undefined = $state();
  let items = $state<Meta[]>([]);
  let status = $state<"idle" | "loading" | "ready" | "empty" | "error">("idle");
  let more = $state(true);
  let fetchingMore = false;
  let canLeft = $state(false);
  let canRight = $state(false);

  const isTop10 = $derived(ranked || (app.settings.general.topTenRanks && /\btop\s*10\b/i.test(catalog.name)));
  const hideAdult = $derived(catalog.adult && app.settings.general.hideAdult && !app.adultRevealed);

  async function load() {
    if (status !== "idle") return;
    status = "loading";
    try {
      const list = await api.catalog(catalog, defaultExtra());
      items = isTop10 ? list.slice(0, 10) : list;
      status = items.length ? "ready" : "empty";
      more = !isTop10 && list.length >= 10;
      requestAnimationFrame(edges);
    } catch {
      status = "error";
    }
  }

  function defaultExtra(): [string, string][] {
    // Catalogs that require a choice (e.g. a genre with options) get the first.
    return catalog.extra.filter((e) => e.isRequired && e.options?.length).map((e) => [e.name, e.options![0]]);
  }

  async function loadMore() {
    if (fetchingMore || !more) return;
    fetchingMore = true;
    try {
      const list = await api.catalog(catalog, [...defaultExtra(), ["skip", String(items.length)]]);
      const seen = new Set(items.map((m) => m.id));
      const fresh = list.filter((m) => !seen.has(m.id));
      items = [...items, ...fresh];
      more = fresh.length > 0;
    } catch {
      more = false;
    }
    fetchingMore = false;
  }

  function edges() {
    if (!rail) return;
    canLeft = rail.scrollLeft > 8;
    canRight = rail.scrollLeft + rail.clientWidth < rail.scrollWidth - 8;
    if (rail.scrollLeft + rail.clientWidth > rail.scrollWidth - rail.clientWidth * 0.8) loadMore();
  }

  function page(dir: number) {
    rail?.scrollBy({ left: dir * rail.clientWidth * 0.85, behavior: "smooth" });
  }

  $effect(() => {
    if (!host) return;
    const io = new IntersectionObserver((es) => es.some((e) => e.isIntersecting) && load(), { rootMargin: "600px 0px" });
    io.observe(host);
    return () => io.disconnect();
  });

  // Vertical wheel over a rail scrolls the page, as expected; shift+wheel and
  // trackpads scroll the rail.
</script>

{#if status !== "empty" && status !== "error"}
  <section class="row" bind:this={host} class:top10={isTop10}>
    <header>
      <button class="title3 heading" onclick={() => go({ name: "discover", type: catalog.type, catalog: catalog.key })}>
        {title || catalog.name}
        <Icon name="forward" size={16} />
      </button>
      {#if catalog.adult && app.settings.general.hideAdult}
        <button class="chip press" onclick={() => (app.adultRevealed = !app.adultRevealed)}>
          <Icon name={hideAdult ? "eye" : "eyeOff"} size={13} />
          {hideAdult ? "Show" : "Hide"}
        </button>
      {/if}
      <span class="kind foot faint">{catalog.type === "series" ? "Shows" : catalog.type === "movie" ? "Films" : catalog.type}</span>
      <span class="addon foot faint">{catalog.addonName}</span>
    </header>
    <div class="wrap">
      <div class="rail scroll-x" bind:this={rail} onscroll={edges}>
        {#if status === "loading" || status === "idle"}
          {#each Array(8) as _}
            <div class="ghost skeleton"></div>
          {/each}
        {:else}
          {#each items as m, i (m.id)}
            <Poster meta={m} rank={isTop10 ? i + 1 : 0} hidden={hideAdult} showTitle={app.settings.general.showTitlesUnderPosters} />
          {/each}
        {/if}
      </div>
      {#if canLeft}
        <button class="arrow left glass press" onclick={() => page(-1)} aria-label="Scroll left"><Icon name="back" size={20} /></button>
      {/if}
      {#if canRight}
        <button class="arrow right glass press" onclick={() => page(1)} aria-label="Scroll right"><Icon name="forward" size={20} /></button>
      {/if}
    </div>
  </section>
{/if}

<style>
  .row {
    position: relative;
    margin-top: 30px;
    content-visibility: auto;
    contain-intrinsic-size: auto 320px;
  }
  header {
    display: flex;
    align-items: baseline;
    gap: 12px;
    padding: 0 var(--gutter);
    margin-bottom: 12px;
  }
  .heading {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .heading :global(svg) {
    opacity: 0;
    transform: translateX(-4px);
    transition:
      opacity var(--t-fast),
      transform var(--t-med) var(--spring);
  }
  .heading:hover :global(svg) {
    opacity: 0.7;
    transform: none;
  }
  .addon {
    margin-left: auto;
  }
  .wrap {
    position: relative;
  }
  .rail {
    display: flex;
    gap: 14px;
    padding: 6px var(--gutter) 18px;
    scroll-snap-type: x proximity;
    scroll-padding: 0 var(--gutter);
  }
  .top10 .rail {
    gap: 22px;
  }
  .ghost {
    flex: 0 0 auto;
    width: var(--card-w);
    aspect-ratio: 2 / 3;
    border-radius: var(--poster-r);
  }
  .arrow {
    position: absolute;
    top: calc(50% - 34px);
    width: 44px;
    height: 44px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    opacity: 0;
    transition: opacity var(--t-fast);
  }
  .wrap:hover .arrow {
    opacity: 1;
  }
  .arrow.left {
    left: calc(var(--gutter) - 22px);
  }
  .arrow.right {
    right: calc(var(--gutter) - 22px);
  }
</style>
