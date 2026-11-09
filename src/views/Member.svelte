<script lang="ts">
  // Someone's page on the couch: what they're on right now, what they're in
  // the middle of, what they finished (with their score), their favourites and
  // what's next on their list. It comes from their Simkl (or their couch
  // library without it), published by their own couch. Each person picks an
  // accent colour, which tints the page and its grainy glow round the edges.
  import { api, img, type CrewProfile, type ProfileItem, type Meta } from "../lib/api";
  import { crew } from "../lib/stores/crew.svelte";
  import { social, loadMe, ago } from "../lib/stores/social.svelte";
  import { go } from "../lib/router.svelte";
  import { live } from "../lib/stores/live.svelte";
  import { logoBroken, logoOk } from "../lib/badlogo.svelte";
  import { imdbArt, fallback } from "../lib/fallback";
  import NavBar from "../ui/NavBar.svelte";
  import Poster from "../components/Poster.svelte";
  import Icon from "../components/Icon.svelte";
  import Controls from "../components/Controls.svelte";
  import boil from "../assets/brand/couch-boil.webp";

  let { id, who, avatar }: { id: string; who?: string; avatar?: string } = $props();

  let data = $state<CrewProfile | null>(null);
  let failed = $state(false);
  let editing = $state(false);
  let saving = $state(false);

  $effect(() => {
    void live.tick; // keeps itself fresh like the rest of couch
    loadMe();
    api
      .profile(id)
      .then(async (r) => {
        data = r;
        failed = false;
        // my own page, not published yet: send it now and show it
        if (!r.profile && social.me?.id === id) {
          await api.publishProfile().catch(() => {});
          data = await api.profile(id).catch(() => r);
        }
      })
      .catch(() => (failed = true));
  });

  const mine = $derived(social.me?.id === id);
  const name = $derived(data?.name ?? who ?? "someone");
  const pic = $derived(data?.avatar ?? avatar);
  const p = $derived(data?.profile ?? null);
  const accent = $derived(data?.style?.accent || "#e0309a");
  const glow = $derived(data?.style?.glow !== false);
  // live from the couch: what they've got on right now
  const now = $derived(crew.members.find((m) => m.id === id && m.watching?.title) ?? null);

  const meta = (i: { id: string; type: string; name: string; poster?: string; background?: string; logo?: string }): Meta =>
    ({ id: i.id, type: i.type, name: i.name, poster: i.poster, background: i.background, logo: i.logo }) as Meta;
  const shows = $derived((p?.watching ?? []).filter((i) => i.type === "series"));
  const films = $derived((p?.watching ?? []).filter((i) => i.type !== "series"));
  const pct = (i: ProfileItem) => (i.eps && i.eps[1] > 0 ? Math.min(100, Math.round((i.eps[0] / i.eps[1]) * 100)) : 0);
  const stats = $derived(
    p
      ? [
          { n: p.stats.movies ?? 0, label: "films" },
          { n: p.stats.shows ?? 0, label: "shows" },
          ...(p.stats.episodes ? [{ n: p.stats.episodes, label: "episodes" }] : []),
          { n: p.stats.plan ?? 0, label: "on the list" },
        ]
      : [],
  );

  // ---- customising your own page
  const SWATCHES = ["#e0309a", "#ff6ec4", "#ff4d4d", "#ffb25c", "#ffd84a", "#7ee081", "#2dd4bf", "#4cc9f0", "#5b7cff", "#9b6bff", "#f4eaf0"];
  async function save(style: { accent?: string; glow?: boolean }) {
    if (!data) return;
    const next = { accent, glow, ...style };
    data.style = next; // instant on screen
    saving = true;
    try {
      await api.setProfileStyle(next);
    } catch {
      /* stays as picked locally; next save retries */
    } finally {
      saving = false;
    }
  }
  let glowOn = $state(true);
  $effect(() => {
    glowOn = glow;
  });
</script>

<NavBar title={name} />

<div class="member" style:--acc={accent}>
  {#if glow}<div class="edge" aria-hidden="true"></div>{/if}

  <header class="head rise">
    <div class="pic">
      {#if pic}<img src={pic} alt="" />{:else}<span>{name.charAt(0).toUpperCase()}</span>{/if}
      {#if now}<i class="live-dot" title="on the couch now"></i>{/if}
    </div>
    <div class="who">
      <h1 class="large-title">{name}</h1>
      {#if now?.watching}
        {@const w = now.watching}
        <button class="now press" onclick={() => w.metaId && go({ name: "detail", type: w.type ?? "movie", id: w.metaId })}>
          <i class="pulse"></i>
          {now.paused ? "paused on" : "watching"} <b>{w.title}</b>{#if w.episode}<span class="dim"> · {w.episode}</span>{/if}
        </button>
      {:else if p}
        <p class="foot dim">last seen on the couch {ago(p.at)}</p>
      {/if}
      {#if stats.length}
        <div class="stats">
          {#each stats as s}
            <div class="stat"><b>{s.n.toLocaleString()}</b><span>{s.label}</span></div>
          {/each}
        </div>
      {/if}
    </div>
    {#if mine}
      <button class="btn tinted press edit" class:on={editing} onclick={() => (editing = !editing)}>
        <Icon name="palette" size={16} /> {editing ? "done" : "customise"}
      </button>
    {/if}
  </header>

  {#if mine && editing}
    <section class="custom glass rise">
      <div class="crow">
        <span class="headline">accent</span>
        <div class="swatches">
          {#each SWATCHES as c}
            <button class="sw press" class:sel={accent === c} style:--c={c} onclick={() => save({ accent: c })} aria-label="Accent {c}"></button>
          {/each}
          <label class="sw custom-sw press" title="Pick any colour" style:--c={accent}>
            <input type="color" value={accent} onchange={(e) => save({ accent: (e.currentTarget as HTMLInputElement).value })} />
            <Icon name="plus" size={14} />
          </label>
        </div>
      </div>
      <div class="crow">
        <span class="headline">grainy glow round the edges</span>
        <Controls kind="switch" bind:value={glowOn} onchange={() => save({ glow: glowOn })} />
      </div>
      <p class="foot dim">{saving ? "saving…" : "everyone on the couch sees your page like this."}</p>
    </section>
  {/if}

  {#if failed && !data}
    <div class="empty"><img src={boil} alt="" /><p>couldn't reach the couch. it'll try again in a bit.</p></div>
  {:else if data && !p}
    <div class="empty">
      <img src={boil} alt="" />
      <p>{mine ? "your page fills in from your next sync. give it a minute." : `${name} hasn't opened the new couch yet.`}</p>
    </div>
  {/if}

  {#if p}
    {#if shows.length}
      <section class="sec">
        <h2 class="title3">in the middle of</h2>
        <div class="rail scroll-x">
          {#each shows as i (i.id)}
            <button class="wide press" onclick={() => go({ name: "detail", type: i.type, id: i.id })} aria-label={i.name}>
              <img class="bg" src={img(i.background ?? imdbArt(i.id, "background") ?? i.poster, "w780")} alt="" loading="lazy" use:fallback={[imdbArt(i.id, "background"), i.poster ? img(i.poster, "w780") : null]} />
              <span class="shade"></span>
              {#if i.logo && !logoBroken(i.logo)}<img class="logo" src={img(i.logo)} alt={i.name} loading="lazy" use:logoOk={{ src: i.logo, id: i.id }} />{:else}<span class="nm">{i.name}</span>{/if}
              {#if i.eps?.[1]}
                <span class="eps foot">{i.eps[0]} / {i.eps[1]} episodes</span>
                <span class="bar"><span style:width="{pct(i)}%"></span></span>
              {/if}
            </button>
          {/each}
        </div>
      </section>
    {/if}

    {#if films.length}
      <section class="sec">
        <h2 class="title3">films on the go</h2>
        <div class="rail scroll-x">
          {#each films as i (i.id)}<div class="cell"><Poster meta={meta(i)} /></div>{/each}
        </div>
      </section>
    {/if}

    {#if p.recent.length}
      <section class="sec">
        <h2 class="title3">recently watched</h2>
        <div class="rail scroll-x">
          {#each p.recent as i (i.id)}
            <div class="cell">
              <Poster meta={meta(i)} />
              <span class="score" class:none={!i.rating}>
                {#if i.rating}<Icon name="star" size={11} fill /> {i.rating}/10{:else}{i.at ? ago(i.at) : "watched"}{/if}
              </span>
            </div>
          {/each}
        </div>
      </section>
    {/if}
  {/if}

  {#if data?.favs.length}
    <section class="sec">
      <h2 class="title3"><Icon name="heart" size={16} fill /> favourites</h2>
      <div class="rail scroll-x">
        {#each data.favs as i (i.id)}<div class="cell"><Poster meta={meta(i)} /></div>{/each}
      </div>
    </section>
  {/if}

  {#if data?.activity.filter((a) => a.text).length}
    <section class="sec">
      <h2 class="title3">said on the couch</h2>
      <div class="said">
        {#each data.activity.filter((a) => a.text).slice(0, 6) as a (a.id)}
          <button class="quote press" onclick={() => go({ name: "detail", type: a.title.type, id: a.title.id })}>
            <span class="q-top"><b>{a.title.name}</b>{#if a.rating}<span class="score"><Icon name="star" size={11} fill /> {a.rating}/10</span>{/if}<span class="dim foot">{ago(a.at)}</span></span>
            <span class="q-txt">{a.text}</span>
          </button>
        {/each}
      </div>
    </section>
  {/if}

  {#if p?.plan.length}
    <section class="sec">
      <h2 class="title3">on their list</h2>
      <div class="rail scroll-x">
        {#each p.plan as i (i.id)}<div class="cell"><Poster meta={meta(i)} /></div>{/each}
      </div>
    </section>
  {/if}

  {#if p}
    <p class="src foot dim">{p.source === "simkl" ? "from their Simkl" : "from their couch library"}</p>
  {/if}
</div>

<style>
  .member {
    --acc: #e0309a;
    position: relative;
    padding: calc(var(--safe-top) + var(--bar-h) + 26px) 0 calc(var(--safe-bottom) + 120px);
    min-height: 100%;
  }
  /* the grainy glow: their colour bleeding in from the edges, with film grain */
  .edge {
    position: fixed;
    inset: 0;
    pointer-events: none;
    z-index: 0;
    background:
      radial-gradient(120% 70% at 50% -12%, color-mix(in srgb, var(--acc) 42%, transparent), transparent 62%),
      radial-gradient(60% 90% at -8% 60%, color-mix(in srgb, var(--acc) 26%, transparent), transparent 60%),
      radial-gradient(60% 90% at 108% 40%, color-mix(in srgb, var(--acc) 26%, transparent), transparent 60%),
      radial-gradient(120% 60% at 50% 115%, color-mix(in srgb, var(--acc) 22%, transparent), transparent 60%);
    animation: edge-in calc(900ms * var(--motion, 1)) ease both;
  }
  .edge::after {
    content: "";
    position: absolute;
    inset: 0;
    opacity: 0.32;
    mix-blend-mode: overlay;
    background-image: url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='180' height='180'><filter id='n'><feTurbulence type='fractalNoise' baseFrequency='.9' numOctaves='3' stitchTiles='stitch'/><feColorMatrix values='0 0 0 0 1  0 0 0 0 1  0 0 0 0 1  0 0 0 .9 0'/></filter><rect width='100%' height='100%' filter='url(%23n)'/></svg>");
    -webkit-mask-image: radial-gradient(ellipse at center, transparent 38%, #000 100%);
    mask-image: radial-gradient(ellipse at center, transparent 38%, #000 100%);
  }
  @keyframes edge-in {
    from {
      opacity: 0;
    }
  }
  .member > :not(.edge) {
    position: relative;
    z-index: 1;
  }

  .head {
    display: flex;
    align-items: center;
    gap: 26px;
    padding: 0 var(--gutter);
    flex-wrap: wrap;
  }
  .pic {
    position: relative;
    width: 124px;
    height: 124px;
    flex: none;
    border-radius: 50%;
    box-shadow:
      0 0 0 3px var(--acc),
      0 0 46px -4px var(--acc);
    background: rgb(255 255 255 / 0.06);
    display: grid;
    place-items: center;
    font: 800 46px system-ui, sans-serif;
  }
  .pic img {
    width: 100%;
    height: 100%;
    border-radius: 50%;
    object-fit: cover;
  }
  .live-dot {
    position: absolute;
    right: 6px;
    bottom: 6px;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    background: #3ddc84;
    box-shadow: 0 0 0 4px var(--black, #000);
  }
  .who {
    display: grid;
    gap: 8px;
    min-width: 0;
    flex: 1;
  }
  .who h1 {
    margin: 0;
    overflow-wrap: anywhere;
  }
  .now {
    justify-self: start;
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 7px 14px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--acc) 20%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--acc) 45%, transparent);
    font-size: 14px;
    max-width: 100%;
  }
  .now b {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .pulse {
    width: 8px;
    height: 8px;
    flex: none;
    border-radius: 50%;
    background: var(--acc);
    animation: pulse 1.6s ease-in-out infinite;
  }
  @keyframes pulse {
    50% {
      box-shadow: 0 0 0 6px color-mix(in srgb, var(--acc) 0%, transparent);
      opacity: 0.6;
    }
  }
  .stats {
    display: flex;
    gap: 26px;
    margin-top: 6px;
    flex-wrap: wrap;
  }
  .stat {
    display: grid;
  }
  .stat b {
    font: 800 26px ui-monospace, monospace;
    letter-spacing: -0.03em;
    color: var(--acc);
  }
  .stat span {
    font-size: 12px;
    color: var(--label-2);
    text-transform: lowercase;
  }
  .edit {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    align-self: flex-start;
  }

  .custom {
    margin: 22px var(--gutter) 0;
    padding: 18px 20px;
    border-radius: 20px;
    display: grid;
    gap: 14px;
    max-width: 640px;
  }
  .crow {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    flex-wrap: wrap;
  }
  .swatches {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  .sw {
    width: 30px;
    height: 30px;
    border-radius: 50%;
    background: var(--c);
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.18);
    transition: transform 0.15s;
  }
  .sw.sel {
    box-shadow:
      0 0 0 2px var(--black, #000),
      0 0 0 4px var(--c);
  }
  .custom-sw {
    position: relative;
    display: grid;
    place-items: center;
    background: conic-gradient(#ff4d4d, #ffd84a, #7ee081, #4cc9f0, #9b6bff, #ff6ec4, #ff4d4d);
    color: #000;
    cursor: pointer;
  }
  .custom-sw input {
    position: absolute;
    inset: 0;
    opacity: 0;
    cursor: pointer;
  }
  .custom p {
    margin: 0;
  }

  .sec {
    margin-top: 34px;
  }
  .sec h2 {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 var(--gutter);
    margin: 0 0 12px;
  }
  .sec h2 :global(svg) {
    color: var(--acc);
  }
  .rail {
    display: flex;
    gap: 14px;
    padding: 0 var(--gutter) 6px;
  }
  .cell {
    flex: none;
    width: var(--card-w);
    display: grid;
    gap: 7px;
  }
  .cell :global(.poster) {
    width: auto;
  }
  .score {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font: 600 12px ui-monospace, monospace;
    color: var(--acc);
  }
  .score.none {
    color: var(--label-3);
    font-weight: 400;
  }

  .wide {
    position: relative;
    flex: none;
    width: 300px;
    aspect-ratio: 16 / 9;
    border-radius: 14px;
    overflow: hidden;
    background: rgb(255 255 255 / 0.05);
    text-align: left;
  }
  .wide .bg {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .wide .shade {
    position: absolute;
    inset: 0;
    background: linear-gradient(0deg, rgb(0 0 0 / 0.85), transparent 65%);
  }
  .wide .logo {
    position: absolute;
    left: 14px;
    bottom: 30px;
    max-width: 60%;
    max-height: 38%;
    object-fit: contain;
    object-position: left bottom;
  }
  .wide .nm {
    position: absolute;
    left: 14px;
    right: 14px;
    bottom: 30px;
    font: 800 18px system-ui, sans-serif;
    color: #fff;
  }
  .wide .eps {
    position: absolute;
    left: 14px;
    bottom: 11px;
    color: rgb(255 255 255 / 0.75);
    font-size: 11px;
  }
  .wide .bar {
    position: absolute;
    right: 14px;
    bottom: 15px;
    width: 40%;
    height: 3px;
    background: rgb(255 255 255 / 0.25);
  }
  .wide .bar span {
    display: block;
    height: 100%;
    background: var(--acc);
  }

  .said {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: 12px;
    padding: 0 var(--gutter);
  }
  .quote {
    display: grid;
    gap: 6px;
    text-align: left;
    padding: 14px 16px;
    border-radius: 16px 16px 16px 4px;
    background: rgb(255 255 255 / 0.06);
    box-shadow: inset 3px 0 0 var(--acc);
  }
  .q-top {
    display: flex;
    align-items: baseline;
    gap: 10px;
    flex-wrap: wrap;
  }
  .q-txt {
    font-size: 14px;
    line-height: 1.45;
    display: -webkit-box;
    -webkit-line-clamp: 4;
    line-clamp: 4;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .empty {
    display: grid;
    justify-items: center;
    gap: 10px;
    padding: 60px var(--gutter);
    text-align: center;
    color: var(--label-2);
  }
  .empty img {
    width: min(200px, 50vw);
    opacity: 0.85;
  }
  .empty p {
    margin: 0;
    max-width: 44ch;
  }
  .src {
    padding: 40px var(--gutter) 0;
    text-align: center;
  }

  @media (max-width: 759px) {
    .head {
      gap: 18px;
    }
    .pic {
      width: 88px;
      height: 88px;
      font-size: 34px;
    }
    .stats {
      gap: 18px;
    }
    .stat b {
      font-size: 21px;
    }
    .wide {
      width: 250px;
    }
  }
</style>
