<script lang="ts">
  import NavBar from "../ui/NavBar.svelte";
  // A person's own page: portrait, the facts that matter, gallery, and
  // everything of theirs you can open. TMDB for actors; for adult
  // performers, any adult addon that exposes a performer route.
  import { api, img } from "../lib/api";
  import { setAmbient } from "../lib/stores/ambient.svelte";
  import { openDetail } from "../lib/open";
  import { toast, errorText } from "../lib/stores/toast.svelte";
  import Poster from "../components/Poster.svelte";
  import Icon from "../components/Icon.svelte";

  let { name, tmdbId, adult = false }: { name: string; tmdbId?: number; adult?: boolean } = $props();
  let p = $state<any>(null);
  let error = $state("");
  let bioOpen = $state(false);
  let sort = $state<"popular" | "newest">("popular");

  $effect(() => {
    p = null;
    error = "";
    api.person(name, tmdbId, adult).then((r) => {
      p = r;
      if (r?.profile) setAmbient(r.profile);
    }).catch((e) => (error = errorText(e)));
  });

  const credits = $derived.by(() => {
    const c = (p?.credits ?? []).slice();
    if (sort === "newest") c.sort((a: any, b: any) => String(b.releaseInfo ?? "").localeCompare(String(a.releaseInfo ?? "")));
    return c;
  });

  function age(b?: string, d?: string) {
    if (!b) return "";
    const end = d ? new Date(d) : new Date();
    const a = Math.floor((end.getTime() - new Date(b).getTime()) / 31557600000);
    return isFinite(a) ? String(a) : "";
  }

  async function open(m: any, el: HTMLElement) {
    if (String(m.id).startsWith("tmdb:")) {
      try {
        const r = await api.resolve(m.type, m.id);
        openDetail({ ...m, id: r.id, type: r.type }, el);
      } catch (e) {
        toast(errorText(e), "error");
      }
    } else openDetail(m, el);
  }
</script>

<NavBar title={name} showAfter={260} />

<div class="person">
  {#if error}
    <div class="empty"><p class="headline">Couldn't find {name}</p><p class="foot dim">{error}</p></div>
  {:else}
    <section class="head">
      <div class="portrait">
        {#if p?.profile}<img src={img(p.profile)} alt={name} />{:else}<div class="skeleton fill"></div>{/if}
      </div>
      <div class="facts">
        <h1 class="large-title">{p?.name ?? name}</h1>
        <div class="chips">
          {#if p?.knownFor}<span class="chip">{p.knownFor}</span>{/if}
          {#if p?.birthday}<span class="chip outline">Born {new Date(p.birthday).toLocaleDateString(undefined, { day: "numeric", month: "long", year: "numeric" })}{age(p.birthday, p.deathday) ? ` (${age(p.birthday, p.deathday)})` : ""}</span>{/if}
          {#if p?.deathday}<span class="chip outline">Died {new Date(p.deathday).toLocaleDateString()}</span>{/if}
          {#if p?.birthplace}<span class="chip outline">{p.birthplace}</span>{/if}
          {#each Object.entries(p?.facts ?? {}).slice(0, 6) as [k, v]}<span class="chip outline">{k}: {v}</span>{/each}
        </div>
        {#if p?.bio}
          <p class="bio" class:open={bioOpen}>{p.bio}</p>
          {#if p.bio.length > 420}<button class="more foot" onclick={() => (bioOpen = !bioOpen)}>{bioOpen ? "Less" : "More"}</button>{/if}
        {/if}
        {#if p?.aliases?.length}<p class="foot faint">Also known as {p.aliases.slice(0, 4).join(", ")}</p>{/if}
      </div>
    </section>

    {#if p?.gallery?.length > 1}
      <section class="block">
        <h2 class="title2">Photos</h2>
        <div class="gallery scroll-x">
          {#each p.gallery as g}<img src={img(g)} alt="" loading="lazy" />{/each}
        </div>
      </section>
    {/if}

    {#if credits.length}
      <section class="block">
        <div class="block-h">
          <h2 class="title2">Known for</h2>
          <div class="sort">
            <button class="chip press" class:sel={sort === "popular"} onclick={() => (sort = "popular")}>Popular</button>
            <button class="chip press" class:sel={sort === "newest"} onclick={() => (sort = "newest")}>Newest</button>
          </div>
        </div>
        <div class="grid">
          {#each credits as c (c.id)}
            <Poster meta={c} showTitle subtitle={c.character || c.releaseInfo || ""} onclick={open} />
          {/each}
        </div>
      </section>
    {/if}
  {/if}
</div>

<style>
  .person {
    padding: calc(var(--safe-top) + var(--bar-h) + 24px) var(--gutter) calc(var(--safe-bottom) + 120px);
  }
  .head {
    display: grid;
    grid-template-columns: 260px 1fr;
    gap: 40px;
    align-items: start;
  }
  .portrait {
    aspect-ratio: 2 / 3;
    border-radius: var(--r-lg);
    overflow: hidden;
    box-shadow: 0 30px 60px -30px rgb(0 0 0 / 0.8), inset 0 0 0 1px rgb(255 255 255 / 0.1);
  }
  .portrait img,
  .fill {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .facts {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding-top: 10px;
    max-width: 760px;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .bio {
    margin: 0;
    color: color-mix(in srgb, var(--text) 84%, transparent);
    display: -webkit-box;
    -webkit-line-clamp: 7;
    line-clamp: 7;
    -webkit-box-orient: vertical;
    overflow: hidden;
    white-space: pre-line;
    user-select: text;
  }
  .bio.open {
    -webkit-line-clamp: unset;
  }
  .more {
    align-self: flex-start;
    color: var(--accent-text);
    font-weight: 600;
  }
  .block {
    margin-top: 40px;
  }
  .block h2 {
    margin-bottom: 14px;
  }
  .block-h {
    display: flex;
    align-items: center;
    gap: 16px;
    margin-bottom: 14px;
  }
  .block-h h2 {
    margin: 0;
  }
  .sort {
    display: flex;
    gap: 6px;
  }
  .chip.sel {
    background: var(--text);
    color: var(--ink);
  }
  .gallery {
    display: flex;
    gap: 12px;
  }
  .gallery img {
    height: 240px;
    border-radius: var(--r-md);
    flex: none;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(var(--card-w), 1fr));
    gap: 24px 16px;
  }
  .grid :global(.poster) {
    width: auto;
  }
  .empty {
    padding: 100px 0;
    text-align: center;
  }
  @media (max-width: 760px) {
    .head {
      grid-template-columns: 1fr;
    }
    .portrait {
      width: 60%;
    }
  }
</style>
