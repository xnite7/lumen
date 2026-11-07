<script lang="ts">
  // The Shows / Movies tabs: a billboard of that kind, couch's picks for it,
  // then every catalog of that kind.
  import { listen } from "../lib/api";
  import Hero from "../components/Hero.svelte";
  import Row from "../components/Row.svelte";
  import { catalogRows, curated, type RowSpec } from "../lib/rows";

  let { kind, scrollY = 0 }: { kind: "series" | "movie"; scrollY?: number } = $props();
  let rows = $state<RowSpec[]>([]);

  async function load() {
    const [cats, picks] = await Promise.all([catalogRows(kind), curated(kind)]);
    const out: RowSpec[] = [];
    const a = cats.map((c) => ({ catalog: c })), b = [...picks];
    // a couple of catalogs, then a pick, and so on
    while (a.length || b.length) {
      out.push(...a.splice(0, 2));
      if (b.length) out.push(b.shift()!);
    }
    rows = out;
  }

  $effect(() => {
    void kind;
    load();
    let un: (() => void) | undefined;
    listen("addons-changed", load).then((u) => (un = u));
    return () => un?.();
  });
</script>

<div class="browse">
  <Hero {scrollY} {kind} />
  {#each rows as r (r.catalog.key)}
    <Row catalog={r.catalog} extra={r.extra} link={r.link} />
  {/each}
  <div class="end"></div>
</div>

<style>
  .end {
    height: 80px;
  }
</style>
