<script lang="ts">
  // Home: a billboard, then Up Next (new episodes first), then the rows.
  import { api, img, type Meta } from "$lib/api";
  import { live } from "$lib/stores/live.svelte";
  import { quickPlay } from "$lib/quickplay";
  import { upNext, rows as loadRows, type UpNext, type Row as R } from "../lib/feed";
  import { nav, openTitle } from "../lib/nav.svelte";
  import Row from "../ui/Row.svelte";
  import Poster from "../ui/Poster.svelte";
  import UpCard from "../ui/UpCard.svelte";

  let up = $state<UpNext[]>([]);
  let rows = $state<R[]>([]);
  let heroes = $state<Meta[]>([]);
  let at = $state(0);

  $effect(() => {
    void live.tick;
    upNext().then((u) => (up = u));
  });
  $effect(() => {
    api.hero().then((h) => (heroes = h.filter((m) => m.background).slice(0, 6))).catch(() => {});
    const got: R[] = [];
    loadRows((r) => {
      got.push(r);
      rows = [...got].sort((a, b) => parseInt(a.key) - parseInt(b.key));
    });
    const t = setInterval(() => heroes.length && (at = (at + 1) % heroes.length), 8000);
    return () => clearInterval(t);
  });
  const hero = $derived(heroes[at]);
  const cardW = $derived(nav.wide ? 380 : 290);
  const posterW = $derived(nav.wide ? 170 : 116);
</script>

<div class="page">
  {#if hero}
    <header class="hero" class:wide={nav.wide}>
      {#key hero.id}
        <img class="bg" src={img(hero.background, "w1280")} alt="" />
      {/key}
      <div class="shade"></div>
      <div class="info">
        {#if hero.logo}<img class="logo" src={img(hero.logo, "w500")} alt={hero.name} />{:else}<h1>{hero.name}</h1>{/if}
        {#if hero.description}<p class="clamp2 muted">{hero.description}</p>{/if}
        <div class="acts">
          <button class="btn primary press" onclick={() => quickPlay(hero)}>▶ play</button>
          <button class="btn press glass" onclick={() => openTitle(hero)}>details</button>
        </div>
        <div class="dots">{#each heroes as _, i}<i class:on={i === at}></i>{/each}</div>
      </div>
    </header>
  {:else}
    <div style="height: calc(var(--safe-t) + 20px)"></div>
  {/if}

  {#if up.length}
    <Row title="up next">
      {#each up as item (item.meta.id)}<UpCard {item} w={cardW} />{/each}
    </Row>
  {/if}

  {#each rows as r (r.key)}
    <Row title={r.title}>
      {#each r.items as m (m.id)}<Poster meta={m} w={posterW} />{/each}
    </Row>
  {/each}
</div>

<style>
  .hero { position: relative; height: min(72vh, 620px); min-height: 420px; }
  .hero.wide { height: min(78vh, 760px); }
  .bg { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: cover; animation: in 0.9s ease both; }
  @keyframes in { from { opacity: 0; transform: scale(1.04); } }
  .shade { position: absolute; inset: 0; background: linear-gradient(to top, var(--bg) 4%, #0009 40%, #0000 70%), linear-gradient(to right, #000a, #0000 60%); }
  .info { position: absolute; left: var(--g); right: var(--g); bottom: 14px; max-width: 560px; display: grid; gap: 12px; }
  .logo { max-width: 72%; max-height: 120px; object-fit: contain; object-position: left bottom; filter: drop-shadow(0 4px 14px #000b); }
  h1 { font-size: 34px; margin: 0; }
  p { margin: 0; font-size: 14px; }
  .acts { display: flex; gap: 10px; }
  .dots { display: flex; gap: 6px; }
  .dots i { width: 6px; height: 6px; border-radius: 3px; background: #fff4; transition: width 0.3s, background 0.3s; }
  .dots i.on { width: 18px; background: #fff; }
</style>
