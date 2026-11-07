<script lang="ts">
  import { listen } from "../lib/api";
  import Hero from "../components/Hero.svelte";
  import ContinueRow from "../components/ContinueRow.svelte";
  import Row from "../components/Row.svelte";
  import CouchRow from "../components/CouchRow.svelte";
  import BrowseRow from "../components/BrowseRow.svelte";
  import { catalogRows, curated, type RowSpec } from "../lib/rows";

  let { scrollY = 0 }: { scrollY?: number } = $props();
  let rows = $state<RowSpec[]>([]);

  // your own lists first, then couch's picks, woven together
  async function load() {
    const [mine, picks] = await Promise.all([catalogRows("home"), curated("all")]);
    const out: RowSpec[] = [];
    const a = mine.map((c) => ({ catalog: c })), b = [...picks];
    while (a.length || b.length) {
      if (a.length) out.push(a.shift()!);
      if (b.length) out.push(b.shift()!);
    }
    rows = out;
  }

  $effect(() => {
    load();
    let un: (() => void) | undefined;
    listen("addons-changed", load).then((u) => (un = u));
    return () => un?.();
  });
</script>

<div class="home">
  <Hero {scrollY} />
  <ContinueRow />
  <CouchRow />
  {#each rows as r (r.catalog.key)}
    <Row catalog={r.catalog} extra={r.extra} link={r.link} />
  {/each}
  <div class="pad"><BrowseRow /></div>
  <div class="end"></div>
</div>

<style>
  .pad {
    padding: 0 var(--gutter);
    margin-top: var(--shelf-gap);
  }
  .end {
    height: 80px;
  }
</style>
