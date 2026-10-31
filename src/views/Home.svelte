<script lang="ts">
  import { api, listen, type CatalogRef } from "../lib/api";
  import { app } from "../lib/stores/app.svelte";
  import Hero from "../components/Hero.svelte";
  import ContinueRow from "../components/ContinueRow.svelte";
  import Row from "../components/Row.svelte";

  let { scrollY = 0 }: { scrollY?: number } = $props();
  let catalogs = $state<CatalogRef[]>([]);

  async function load() {
    const all = await api.catalogs();
    const browsable = all.filter((c) => !c.extra.some((e) => e.isRequired && (e.name === "search" || !e.options?.length)));
    const h = app.settings.home;
    const hidden = new Set(h.hiddenRows);
    const order = new Map(h.rowOrder.map((k, i) => [k, i]));
    catalogs = browsable
      .filter((c) => !hidden.has(c.key))
      .map((c, i) => ({ c, i }))
      .sort((a, b) => (order.get(a.c.key) ?? 1000 + a.i) - (order.get(b.c.key) ?? 1000 + b.i))
      .map((x) => x.c);
  }

  $effect(() => {
    load();
    let un: (() => void) | undefined;
    listen("addons-changed", load).then((u) => (un = u));
    return () => un?.();
  });

  // Top 10 rows read better near the top; "Because you watched" right under.
</script>

<div class="home">
  <Hero {scrollY} />
  <ContinueRow />
  {#each catalogs as c (c.key)}
    <Row catalog={c} />
  {/each}
  <div class="end"></div>
</div>

<style>
  .end {
    height: 80px;
  }
</style>
