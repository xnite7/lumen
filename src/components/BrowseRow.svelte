<script lang="ts">
  // Home: the smart bar up front, then browse tiles for every catalog and
  // a row of genres. (Used to live on the Search page.)
  import { api, type CatalogRef } from "../lib/api";
  import { app } from "../lib/stores/app.svelte";
  import { go } from "../lib/router.svelte";
  import Icon from "./Icon.svelte";

  let catalogs = $state<CatalogRef[]>([]);
  $effect(() => {
    api.catalogs().then((c) => (catalogs = c.filter((x) => !x.extra.some((e) => e.isRequired && e.name === "search")))).catch(() => {});
  });

  // dusky late-night tones from the midnite palette, no rainbow
  const tints = ["#3a1830", "#2a1a24", "#3b2416", "#241626", "#33152a", "#2d1d1a", "#1f1824", "#3a2014", "#281422", "#1d1418"];
  const typeLabel = (t: string) => (t === "movie" ? "Films" : t === "series" ? "Shows" : t === "tv" ? "Channels" : t.charAt(0).toUpperCase() + t.slice(1));
  const browse = $derived(
    catalogs
      .filter((c) => !(c.adult && app.settings.general.hideAdult && !app.adultRevealed))
      .filter((c) => !c.extra.some((e) => e.isRequired && !e.options?.length))
      .slice(0, 12),
  );
  const genreSource = $derived(catalogs.find((c) => c.type === "movie" && c.extra.some((e) => e.name === "genre" && e.options?.length)));
  const genres = $derived(genreSource?.extra.find((e) => e.name === "genre")?.options?.slice(0, 18) ?? []);
</script>

{#if browse.length || app.settings.integrations.aiEnabled}
  <section class="browse">
    <h2 class="title3 head">browse</h2>
    <div class="strip scroll-x">
      {#if app.settings.integrations.aiEnabled}
        <button class="vibe press" onclick={() => go({ name: "vibe" })}>
          <span class="blobs" aria-hidden="true"></span>
          <span class="vc-icon"><Icon name="sparkles" size={20} /></span>
          <span class="tile-name">Smart bar</span>
          <span class="tile-sub">describe a feeling</span>
        </button>
      {/if}
      {#each browse as c, n (c.key)}
        <button class="tile press" style:--t={tints[n % tints.length]} onclick={() => go({ name: "discover", type: c.type, catalog: c.key })}>
          <span class="tile-name">{c.name}</span>
          <span class="tile-sub">{typeLabel(c.type)}</span>
        </button>
      {/each}
    </div>
    {#if genres.length && genreSource}
      <div class="genres scroll-x">
        {#each genres as g, n}
          <button class="genre press" style:--t={tints[(n + 3) % tints.length]} onclick={() => go({ name: "discover", type: genreSource!.type, catalog: genreSource!.key, genre: g })}>{g}</button>
        {/each}
      </div>
    {/if}
  </section>
{/if}

<style>
  .browse {
    margin: 8px 0 28px;
  }
  .head {
    margin: 0 0 12px;
  }
  .strip,
  .genres {
    display: flex;
    gap: 12px;
    margin: 0 calc(-1 * var(--gutter));
    padding: 0 var(--gutter) 4px;
  }
  .genres {
    gap: 8px;
    margin-top: 12px;
  }
  .tile,
  .vibe {
    position: relative;
    flex: none;
    width: 210px;
    height: 100px;
    border-radius: var(--r-md);
    padding: 14px;
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
    text-align: left;
    overflow: hidden;
    isolation: isolate;
    color: var(--text);
    text-shadow: 0 1px 8px rgb(var(--lo) / 0.4);
    box-shadow: inset 0 0 0 1px rgb(var(--hi) / 0.07);
  }
  .tile {
    background:
      radial-gradient(120% 120% at 100% 0%, color-mix(in srgb, var(--t) 70%, white 6%), transparent 60%),
      linear-gradient(150deg, color-mix(in srgb, var(--t) 82%, black 18%), color-mix(in srgb, var(--t) 40%, black 60%));
  }
  .tile-name {
    font-size: var(--fs-head);
    font-weight: 700;
    letter-spacing: -0.02em;
  }
  .tile-sub {
    font-size: var(--fs-caption);
    font-weight: 500;
    opacity: 0.75;
  }
  .vibe {
    width: 240px;
    background: var(--surface);
  }
  .vibe .blobs {
    position: absolute;
    inset: -60%;
    z-index: -1;
    background:
      radial-gradient(30% 50% at 25% 50%, rgb(var(--neon-rgb) / 0.6), transparent),
      radial-gradient(30% 55% at 70% 40%, rgb(var(--lamp-rgb) / 0.4), transparent),
      radial-gradient(25% 50% at 50% 75%, #5a2348, transparent);
    filter: blur(26px) saturate(1.3);
    animation: drift 14s ease-in-out infinite alternate;
  }
  @keyframes drift {
    to {
      transform: translate(10%, -6%) rotate(18deg);
    }
  }
  .vc-icon {
    position: absolute;
    top: 12px;
    right: 12px;
    width: 36px;
    height: 36px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: rgb(var(--hi) / 0.16);
    backdrop-filter: blur(10px);
    -webkit-backdrop-filter: blur(10px);
  }
  .genre {
    flex: none;
    height: 36px;
    padding: 0 16px;
    border-radius: var(--r-pill);
    font-weight: 600;
    font-size: var(--fs-sub);
    background: color-mix(in srgb, var(--t) 40%, transparent);
    color: var(--text);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--t) 70%, transparent);
  }
  @media (max-width: 759px) {
    .tile {
      width: 160px;
      height: 88px;
      padding: 12px;
    }
    .vibe {
      width: 190px;
      height: 88px;
      padding: 12px;
    }
  }
</style>
