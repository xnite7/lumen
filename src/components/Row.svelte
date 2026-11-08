<script lang="ts">
  import { live } from "../lib/stores/live.svelte";
  // A catalog row. Loads only when it scrolls near the viewport (so twenty
  // addons don't fire at once), shows shimmer while waiting, hides itself if
  // the addon answers with nothing, and pages more in as you reach the end.
  import { api, type CatalogRef, type Meta } from "../lib/api";
  import { app } from "../lib/stores/app.svelte";
  import { go } from "../lib/router.svelte";
  import Poster from "./Poster.svelte";
  import Icon from "./Icon.svelte";
  import RailArrows from "./RailArrows.svelte";
  import { ui } from "../lib/ui.svelte";

  let {
    catalog,
    title = "",
    ranked = false,
    extra = undefined,
    link = true,
  }: { catalog: CatalogRef; title?: string; ranked?: boolean; extra?: [string, string][]; link?: boolean } = $props();

  let host: HTMLElement | undefined = $state();
  let rail: HTMLElement | undefined = $state();
  let items = $state<Meta[]>([]);
  let status = $state<"idle" | "loading" | "ready" | "empty" | "error">("idle");
  let more = $state(true);
  let fetchingMore = false;
  let canLeft = $state(false);
  let canRight = $state(false);

  const isTop10 = $derived(ranked || (app.settings.general.topTenRanks && /\btop\s*10\b/i.test(catalog.name)));
  // Apple TV shelves are landscape; fall back to posters when the addon
  // doesn't give backdrops (or for ranked Top 10s, which want the tall art).
  const wide = $derived(!isTop10 && items.length > 0 && items.filter((m) => m.background).length >= items.length * 0.7);
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

  // your own rows (Simkl lists, Top 10…) refresh by themselves
  const mine = $derived(catalog.builtin && /^(simkl-|motn-top|couch-)/.test(catalog.id));
  let lastTick = live.tick;
  $effect(() => {
    const t = live.tick;
    if (!mine || t === lastTick || status !== "ready") return;
    lastTick = t;
    api.catalog(catalog, defaultExtra()).then((list) => {
      if (!list.length) return;
      items = isTop10 ? list.slice(0, 10) : list;
    }).catch(() => {});
  });

  function defaultExtra(): [string, string][] {
    if (extra) return extra;
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
      {#if link}
        <button class="title3 heading" onclick={() => go({ name: "discover", type: catalog.type, catalog: catalog.key })}>
          {title || catalog.name}
          <Icon name="forward" size={16} />
        </button>
      {:else}
        <h2 class="title3 heading">{title || catalog.name}</h2>
      {/if}
      {#if catalog.adult && app.settings.general.hideAdult}
        <button class="chip press" onclick={() => (app.adultRevealed = !app.adultRevealed)}>
          <Icon name={hideAdult ? "eye" : "eyeOff"} size={13} />
          {hideAdult ? "Show" : "Hide"}
        </button>
      {/if}
    </header>
    <div class="wrap">
      <div class="rail scroll-x" bind:this={rail} data-own-arrows>
        {#if status === "loading" || status === "idle"}
          {#each Array(8) as _}
            <div class="ghost skeleton"></div>
          {/each}
        {:else}
          {#each items as m, i (m.id)}
            <Poster meta={m} rank={isTop10 ? i + 1 : 0} hidden={hideAdult} landscape={wide && !ui.phone} showTitle={false} />
          {/each}
        {/if}
      </div>
      <RailArrows {rail} onpage={edges} />
    </div>
  </section>
{/if}

<style>
  .row {
    position: relative;
    margin-top: var(--shelf-gap);
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
    gap: 18px;
    padding: 10px var(--gutter) 22px;
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
  @media (max-width: 759px) {
    .row {
      margin-top: 22px;
    }
    header {
      margin-bottom: 6px;
    }
    .heading {
      font-size: var(--fs-title3);
    }
    .heading :global(svg) {
      opacity: 0.5;
      transform: none;
    }
    .kind,
    .addon {
      display: none;
    }
    .rail {
      gap: 10px;
      padding-bottom: 12px;
      scroll-snap-type: x mandatory;
    }
    .top10 .rail {
      gap: 16px;
    }
    .arrow {
      display: none;
    }
  }
</style>
