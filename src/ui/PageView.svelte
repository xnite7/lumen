<script lang="ts">
  // One screen on the navigation stack: its own scroll container, so going
  // back reveals the previous screen exactly as you left it.
  import { onMount, onDestroy, setContext } from "svelte";
  import type { Entry } from "../lib/router.svelte";
  import { scrolls } from "../lib/router.svelte";
  import { onScroll } from "../lib/ui.svelte";
  import Home from "../views/Home.svelte";
  import Discover from "../views/Discover.svelte";
  import Library from "../views/Library.svelte";
  import Search from "../views/Search.svelte";
  import Profile from "../views/Profile.svelte";
  import Detail from "../views/Detail.svelte";
  import Person from "../views/Person.svelte";
  import Addons from "../views/Addons.svelte";
  import Settings from "../views/Settings.svelte";
  import Vibe from "../views/Vibe.svelte";

  let { route, active, index = 0, layer = $bindable() }: { route: Entry; active: boolean; index?: number; layer?: HTMLElement } = $props();
  let y = $state(0);

  // each screen's own depth and scroll, so its nav bar is right even while
  // it's the one revealed under a back swipe
  setContext("page", {
    get index() {
      return index;
    },
    get y() {
      return y;
    },
    get active() {
      return active;
    },
  });

  onMount(() => {
    const saved = route._k ? scrolls.get(route._k) : 0;
    if (saved && layer) {
      layer.scrollTop = saved;
      y = layer.scrollTop;
    }
  });
  onDestroy(() => {
    if (route._k && layer) scrolls.set(route._k, layer.scrollTop);
  });

  // the active layer drives the shared bar state (large titles, tab bar)
  $effect(() => {
    if (active && layer) onScroll(layer.scrollTop);
  });

  function scrolled() {
    if (!layer) return;
    y = layer.scrollTop;
    if (active) onScroll(y);
  }
</script>

<main class="layer" bind:this={layer} data-scroller={active ? "" : undefined} onscroll={scrolled}>
  <div class="page">
    {#if route.name === "home"}
      <Home scrollY={y} />
    {:else if route.name === "discover"}
      <Discover type={route.type} catalog={route.catalog} genre={route.genre} />
    {:else if route.name === "library"}
      <Library />
    {:else if route.name === "search"}
      <Search q={route.q} />
    {:else if route.name === "profile"}
      <Profile />
    {:else if route.name === "detail"}
      <Detail type={route.type} id={route.id} hint={route.hint} preview={route.preview} />
    {:else if route.name === "person"}
      <Person name={route.person} tmdbId={route.tmdbId} adult={route.adult} />
    {:else if route.name === "addons"}
      <Addons install={route.install} />
    {:else if route.name === "settings"}
      <Settings section={route.section} />
    {:else if route.name === "vibe"}
      <Vibe q={route.q} />
    {/if}
  </div>
</main>

<style>
  .layer {
    position: absolute;
    inset: 0;
    overflow-y: auto;
    overflow-x: hidden;
    overscroll-behavior-y: contain;
    -webkit-overflow-scrolling: touch;
    background: transparent;
  }
  .page {
    min-height: 100%;
  }
</style>
