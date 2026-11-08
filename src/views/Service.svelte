<script lang="ts">
  // One streaming service's page: its logo, then what's on it (shows or
  // films), most popular first, loading more as you scroll.
  import { api, type Meta } from "../lib/api";
  import { serviceCatalog } from "../lib/services";
  import Poster from "../components/Poster.svelte";
  import Controls from "../components/Controls.svelte";
  import NavBar from "../ui/NavBar.svelte";

  let { id, label, logo }: { id: number; label: string; logo?: string | null } = $props();
  let kind = $state<"series" | "movie">("series");
  let items = $state<Meta[]>([]);
  let loading = $state(false);
  let more = $state(true);
  let sentinel: HTMLElement | undefined = $state();
  let seq = 0;

  async function page(reset: boolean) {
    if (loading || (!more && !reset)) return;
    const my = reset ? ++seq : seq;
    if (reset) {
      items = [];
      more = true;
    }
    loading = true;
    try {
      const cat = serviceCatalog(id, kind, label);
      const r = await api.catalog(cat, items.length ? [["skip", String(items.length)]] : []);
      if (my !== seq) return;
      const seen = new Set(items.map((m) => m.id));
      const fresh = r.filter((m) => !seen.has(m.id));
      items = [...items, ...fresh];
      more = fresh.length > 0;
    } catch {
      more = false;
    }
    loading = false;
  }

  $effect(() => {
    void kind;
    void id;
    page(true);
  });
  $effect(() => {
    if (!sentinel) return;
    const io = new IntersectionObserver((e) => e[0].isIntersecting && page(false), { rootMargin: "800px" });
    io.observe(sentinel);
    return () => io.disconnect();
  });
</script>

<NavBar title={label} />

<div class="svc-page">
  <header>
    {#if logo}<img class="logo" src={logo} alt="" />{/if}
    <h1 class="large-title">{label}</h1>
    <Controls kind="segmented" bind:value={kind} options={[{ value: "series", label: "Shows" }, { value: "movie", label: "Films" }]} label="Kind" />
  </header>
  <div class="grid">
    {#each items as m (m.id)}
      <Poster meta={m} />
    {/each}
    {#if loading}
      {#each Array(12) as _}<div class="ghost skeleton"></div>{/each}
    {/if}
  </div>
  {#if !loading && !items.length}<p class="dim empty">nothing here right now.</p>{/if}
  <div bind:this={sentinel} class="sentinel"></div>
</div>

<style>
  .svc-page {
    padding: calc(var(--safe-top) + var(--bar-h) + 18px) var(--gutter) calc(var(--safe-bottom) + 120px);
  }
  header {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 14px 18px;
    margin-bottom: 22px;
  }
  .logo {
    width: 56px;
    height: 56px;
    border-radius: 14px;
    box-shadow: 0 10px 30px -10px #000;
  }
  .large-title {
    margin: 0;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(var(--card-w), 1fr));
    gap: 24px 16px;
  }
  .grid :global(.poster) {
    width: auto;
  }
  .ghost {
    aspect-ratio: 2 / 3;
    border-radius: var(--poster-r);
  }
  .empty {
    padding: 60px 0;
    text-align: center;
  }
  .sentinel {
    height: 1px;
  }
  @media (max-width: 759px) {
    .grid {
      grid-template-columns: repeat(3, 1fr);
      gap: 16px 10px;
    }
  }
</style>
