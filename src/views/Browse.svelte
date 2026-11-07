<script lang="ts">
  import { wantGenre } from "../lib/genrelink.svelte";
  // The Shows / Movies tabs (Netflix): a title with a Genres picker, a
  // billboard card, then rows. Pick a genre and the whole page becomes it.
  import { api, listen, type CatalogRef, type Meta } from "../lib/api";
  import Hero from "../components/Hero.svelte";
  import Row from "../components/Row.svelte";
  import Icon from "../components/Icon.svelte";
  import { catalogRows, curated, type RowSpec } from "../lib/rows";
  import { haptic } from "../lib/native";

  let { kind, scrollY = 0 }: { kind: "series" | "movie"; scrollY?: number } = $props();
  let rows = $state<RowSpec[]>([]);
  let all = $state<CatalogRef[]>([]);
  let genre = $state("");
  let open = $state(false);
  let heroItems = $state<Meta[] | undefined>(undefined);

  const heading = $derived(kind === "series" ? "TV Shows" : "Movies");
  // couch's own browse rows know every genre
  const source = $derived(all.find((c) => c.builtin && c.type === kind && c.id === "tmdb-popular"));
  const genres = $derived(source?.extra.find((e) => e.name === "genre")?.options ?? []);

  async function load() {
    all = await api.catalogs();
    if (!genre) {
      const [cats, picks] = await Promise.all([catalogRows(kind), curated(kind)]);
      const out: RowSpec[] = [];
      const a = cats.map((c) => ({ catalog: c })), b = [...picks];
      while (a.length || b.length) {
        out.push(...a.splice(0, 2));
        if (b.length) out.push(b.shift()!);
      }
      rows = out;
      heroItems = undefined;
      return;
    }
    const g = genre;
    heroItems = [];
    const label: Record<string, string> = {
      "tmdb-trending": `Trending ${g}`,
      "tmdb-popular": `Popular ${g}`,
      "tmdb-top": `Critically Acclaimed ${g}`,
      "tmdb-new": `New ${g}`,
    };
    const mine = all.filter((c) => c.type === kind && c.builtin && label[c.id]);
    // other catalogs that can narrow to this genre too
    const theirs = all.filter((c) => c.type === kind && !c.builtin && c.extra.some((e) => e.name === "genre" && e.options?.includes(g)));
    rows = [
      ...mine.map((c) => ({ catalog: { ...c, key: `${c.key}#${g}` }, title: label[c.id], extra: [["genre", g]] as [string, string][], link: false })),
      ...theirs.map((c) => ({ catalog: { ...c, key: `${c.key}#${g}` }, title: `${c.name} · ${g}`, extra: [["genre", g]] as [string, string][], link: false })),
    ];
    const pop = mine.find((c) => c.id === "tmdb-popular") ?? mine[0];
    // Simkl's own trending for the genre first, TMDB's popular as a fallback
    const sim = await api.genreTrending(kind, g).catch(() => [] as Meta[]);
    heroItems = sim.length ? sim : pop ? await api.catalog(pop, [["genre", g]]).catch(() => []) : [];
  }

  // a genre opened from a title page
  $effect(() => {
    const g = wantGenre[kind];
    if (!g) return;
    wantGenre[kind] = "";
    const match = genres.find((x) => x.toLowerCase() === g.toLowerCase()) ?? g;
    choose(match);
  });

  function choose(g: string) {
    haptic("selection");
    open = false;
    genre = g;
    document.querySelector("[data-scroller]")?.scrollTo({ top: 0, behavior: "smooth" });
  }

  $effect(() => {
    void kind;
    void genre;
    load();
    let un: (() => void) | undefined;
    listen("addons-changed", load).then((u) => (un = u));
    return () => un?.();
  });

  $effect(() => {
    if (!open) return;
    const off = (e: PointerEvent) => !(e.target as Element)?.closest?.(".gp") && (open = false);
    const esc = (e: KeyboardEvent) => e.key === "Escape" && (open = false);
    window.addEventListener("pointerdown", off);
    window.addEventListener("keydown", esc);
    return () => {
      window.removeEventListener("pointerdown", off);
      window.removeEventListener("keydown", esc);
    };
  });
</script>

<div class="browse">
  <header class="head">
    {#if genre}
      <button class="crumb" onclick={() => choose("")}>{heading}</button>
      <Icon name="chevron" size={14} />
      <h1 class="title"><span class="accent"></span>{genre}</h1>
    {:else}
      <h1 class="title">{heading}</h1>
    {/if}
    {#if genres.length}
      <div class="gp">
        <button class="gbtn" class:on={open} onclick={() => (open = !open)} aria-expanded={open}>
          Genres <Icon name="chevronDown" size={14} />
        </button>
        {#if open}
          <div class="gmenu" role="menu">
            {#each genres as g}
              <button role="menuitem" class:sel={g === genre} onclick={() => choose(g)}>{g}</button>
            {/each}
          </div>
        {/if}
      </div>
    {/if}
  </header>

  {#key genre}
    <Hero {scrollY} {kind} items={heroItems} inset />
  {/key}
  {#each rows as r (r.catalog.key)}
    <Row catalog={r.catalog} title={r.title} extra={r.extra} link={r.link} />
  {/each}
  <div class="end"></div>
</div>

<style>
  .head {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: calc(var(--safe-top) + 74px) var(--gutter) 18px;
    color: var(--text-dim);
  }
  .title {
    margin: 0;
    display: flex;
    align-items: center;
    gap: 12px;
    font-size: 30px;
    font-weight: 700;
    letter-spacing: -0.02em;
    color: var(--text);
  }
  .accent {
    width: 4px;
    height: 28px;
    background: var(--lamp);
    box-shadow: 0 0 12px rgb(var(--lamp-rgb) / 0.7);
  }
  .crumb {
    font-size: 20px;
    font-weight: 600;
    color: var(--text-dim);
  }
  .crumb:hover {
    color: var(--text);
  }
  .gp {
    position: relative;
    margin-left: 10px;
  }
  .gbtn {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 38px;
    padding: 0 16px;
    border-radius: var(--r-pill);
    font-size: 15px;
    font-weight: 600;
    color: var(--text);
    background: rgb(0 0 0 / 0.4);
    box-shadow: inset 0 0 0 1px rgb(var(--hi) / 0.35);
    transition: background var(--t-fast) ease;
  }
  .gbtn:hover,
  .gbtn.on {
    background: rgb(var(--hi) / 0.1);
  }
  .gmenu {
    position: absolute;
    z-index: 30;
    left: 0;
    top: calc(100% + 8px);
    display: grid;
    grid-template-columns: repeat(3, minmax(150px, auto));
    gap: 2px 8px;
    padding: 12px;
    border-radius: 14px;
    background: rgb(10 8 9 / 0.94);
    backdrop-filter: blur(20px);
    -webkit-backdrop-filter: blur(20px);
    box-shadow: inset 0 0 0 1px rgb(var(--hi) / 0.1), 0 24px 60px -20px rgb(0 0 0 / 0.95);
    transform-origin: top left;
    animation: drop 200ms var(--ease-out) both;
  }
  @keyframes drop {
    from {
      opacity: 0;
      transform: scale(0.96) translateY(-4px);
    }
  }
  .gmenu button {
    text-align: left;
    padding: 8px 12px;
    border-radius: 8px;
    font-size: 15px;
    color: var(--text-dim);
  }
  .gmenu button:hover {
    color: var(--text);
    background: rgb(var(--hi) / 0.07);
  }
  .gmenu button.sel {
    color: var(--lamp);
    font-weight: 600;
  }
  .end {
    height: 80px;
  }
</style>
