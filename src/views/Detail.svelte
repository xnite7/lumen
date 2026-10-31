<script lang="ts">
  import { api, img, type Meta, type Video } from "../lib/api";
  import { app } from "../lib/stores/app.svelte";
  import { setAmbient } from "../lib/stores/ambient.svelte";
  import { pageCtx, isActive } from "../lib/page";
  const page = pageCtx();
  import { inLibrary, toggleLibrary, setWatched, watchedIds, progressOf, itemOf, lib } from "../lib/stores/library.svelte";
  import { play, orderedVideos } from "../lib/stores/playback.svelte";
  import { toast, errorText } from "../lib/stores/toast.svelte";
  import { go } from "../lib/router.svelte";
  import { openDetail } from "../lib/open";
  import { year, runtime, episodeLabel } from "../lib/format";
  import StreamsSheet from "../components/StreamsSheet.svelte";
  import Poster from "../components/Poster.svelte";
  import Icon from "../components/Icon.svelte";
  import NavBar from "../ui/NavBar.svelte";
  import { share, haptic } from "../lib/native";
  import { ui } from "../lib/ui.svelte";

  let { type, id, hint, preview }: { type: string; id: string; hint?: string; preview?: any } = $props();

  let meta = $state<Meta | null>(null);
  let extra = $state<any>(null);
  let error = $state("");
  let season = $state<number | null>(null);
  let sheetOpen = $state(false);
  let sheetVideo = $state<Video | null>(null);
  let rating = $state<number | null>(null);
  let hoverStar = $state(0);
  let descOpen = $state(false);
  function doShare() {
    const mm = meta ?? preview;
    if (!mm) return;
    share({ title: mm.name, url: mm.id?.startsWith("tt") ? `https://www.imdb.com/title/${mm.id.split(":")[0]}/` : undefined });
  }

  const m = $derived(meta ?? (preview as Meta | undefined) ?? null);

  $effect(() => {
    const t = type, i = id;
    meta = null;
    extra = null;
    error = "";
    api.meta(t, i, hint)
      .then((r) => {
        meta = r;
        const auto = preview?.autoplay as string | undefined;
        if (auto) {
          const v = r.videos?.find((x) => x.id === auto) ?? null;
          openStreams(v);
        }
      })
      .catch((e) => (error = errorText(e)));
    if (!t || t === "other") return;
    api.enrich(t, i).then((e) => (extra = e)).catch(() => {});
    if (app.secrets.simkl_token) api.simklStatus(t, i).then((s) => (rating = s?.rating ?? null)).catch(() => {});
  });

  const backdrop = $derived(extra?.backdrop ?? m?.background ?? m?.poster);
  const logo = $derived(extra?.logo ?? m?.logo);
  $effect(() => {
    if (backdrop && isActive(page)) setAmbient(backdrop);
  });

  const videos = $derived(orderedVideos(meta));
  const seasons = $derived([...new Set(videos.map((v) => v.season ?? 0))].sort((a, b) => (a || 999) - (b || 999)));
  const watched = $derived(watchedIds(id));
  const item = $derived(itemOf(id));
  void lib;

  $effect(() => {
    if (season == null && seasons.length) {
      const resumeSeason = item?.state?.season;
      season = resumeSeason && seasons.includes(resumeSeason) ? resumeSeason : (seasons.find((s) => s > 0) ?? seasons[0]);
    }
  });

  const episodes = $derived(videos.filter((v) => (v.season ?? 0) === season));

  /** What the big button does: resume, next unwatched episode, or start. */
  const primary = $derived.by(() => {
    if (!meta) return null;
    if (meta.type !== "series" || !videos.length) {
      const p = progressOf(id);
      return { label: p > 0.02 ? "Resume" : "Play", video: null as Video | null, sub: "" };
    }
    const vid = item?.state?.video_id;
    if (vid && progressOf(id, vid) > 0.02) {
      const v = videos.find((x) => x.id === vid);
      if (v) return { label: "Resume", video: v, sub: episodeLabel(v.id) };
    }
    const next = videos.find((v) => (v.season ?? 0) > 0 && !watched.has(v.id) && (!v.released || new Date(v.released).getTime() < Date.now()));
    const v = next ?? videos[0];
    return { label: next && next !== videos.find((x) => (x.season ?? 0) > 0) ? "Continue" : "Play", video: v, sub: episodeLabel(v.id) };
  });

  function openStreams(v: Video | null) {
    sheetVideo = v;
    sheetOpen = true;
  }

  function trailerKey() {
    return extra?.videos?.find((v: any) => v.type === "Trailer")?.key ?? m?.trailers?.[0]?.source ?? extra?.videos?.[0]?.key;
  }

  function playTrailer(key?: string, name = "Trailer") {
    const k = key ?? trailerKey();
    if (!k || !m) return;
    play(m, { ytId: k, title: name, _lumen: { kind: "youtube" } as any, _addonId: "", _addonName: "" } as any, m.id);
  }

  async function rate(n: number | null) {
    if (!m) return;
    try {
      await api.simklRate(m.type, m.id, n);
      rating = n;
      toast(n ? `Rated ${m.name} ${n}/10 on Simkl` : "Rating removed", "ok");
    } catch (e) {
      toast(errorText(e), "error");
    }
  }

  async function openRec(r: any, el: HTMLElement) {
    if (String(r.id).startsWith("tmdb:")) {
      try {
        const res = await api.resolve(r.type, r.id);
        openDetail({ ...r, id: res.id, type: res.type }, el);
      } catch (e) {
        toast(errorText(e), "error");
      }
    } else openDetail(r, el);
  }

  const owned = $derived(m ? inLibrary(m.id) : false);
  const cast = $derived(extra?.cast?.length ? extra.cast : (m?.cast ?? []).map((n: string) => ({ name: n })));
</script>

{#if error && !m}
  <div class="err">
    <Icon name="info" size={34} />
    <p class="headline">Couldn't open this title</p>
    <p class="foot dim">{error}</p>
  </div>
{:else if m}
  <NavBar title={m.name} showAfter={ui.phone ? 360 : 420} transparent={ui.scrollY < 300}>
    {#snippet actions()}
      <button class="orb glass press" onclick={doShare} aria-label="Share"><Icon name="share" size={19} /></button>
    {/snippet}
  </NavBar>
  <article class="detail">
    <section class="hero">
      {#if backdrop}
        <img class="bg" src={img(backdrop, "original")} alt="" style:view-transition-name="hero-art" />
      {/if}
      <div class="hero-shade"></div>
      <div class="info">
        {#if logo}
          <img class="logo" src={img(logo)} alt={m.name} />
        {:else}
          <h1 class="large-title">{m.name}</h1>
        {/if}
        <div class="facts">
          {#if m.imdbRating}<span class="rating"><Icon name="star" size={12} fill /> {m.imdbRating}</span>{/if}
          <span class="sub">{[m.type === "series" ? "Series" : "Film", year(m), m.runtime || extra?.runtime ? runtime(m.runtime ?? extra?.runtime) : "", seasons.filter((s) => s > 0).length ? `${seasons.filter((s) => s > 0).length} seasons` : ""].filter(Boolean).join(" · ")}</span>
          {#if extra?.certification}<span class="chip small outline">{extra.certification}</span>{/if}
          {#if (extra?.genres ?? m.genres)?.length}<span class="sub dim">{(extra?.genres ?? m.genres).slice(0, 3).join(" · ")}</span>{/if}
        </div>
        <div class="actions">
          {#if primary}
            <button class="btn lg primary press cta" onclick={() => { haptic("medium"); openStreams(primary.video); }}>
              <Icon name="play" size={18} />
              {primary.label}{#if primary.sub}<span class="psub">{primary.sub}</span>{/if}
            </button>
          {/if}
          <div class="secondary">
            <button class="act press" onclick={() => { haptic("light"); toggleLibrary(m); }} aria-label={owned ? "Remove from My List" : "Add to My List"}>
              <span class="act-ic glass"><Icon name={owned ? "check" : "plus"} size={20} /></span><span class="act-l">My List</span>
            </button>
            <button class="act press" onclick={() => { haptic("light"); setWatched(m, null, !(item?.state?.flaggedWatched)); }} aria-label="Mark watched">
              <span class="act-ic glass" class:lit={item?.state?.flaggedWatched}><Icon name="checkCircle" size={20} /></span><span class="act-l">{item?.state?.flaggedWatched ? "Watched" : "Mark watched"}</span>
            </button>
            {#if trailerKey()}
              <button class="act press" onclick={() => playTrailer()}>
                <span class="act-ic glass"><Icon name="film" size={19} /></span><span class="act-l">Trailer</span>
              </button>
            {/if}
            <button class="act press" onclick={doShare} aria-label="Share">
              <span class="act-ic glass"><Icon name="share" size={19} /></span><span class="act-l">Share</span>
            </button>
          </div>
        </div>
        {#if extra?.tagline}<p class="tagline">{extra.tagline}</p>{/if}
        {#if m.description || extra?.overview}
          <button class="desc-wrap" onclick={() => (descOpen = !descOpen)} aria-expanded={descOpen}>
            <p class="desc" class:clamp-3={!descOpen && ui.phone}>{m.description ?? extra?.overview}</p>
          </button>
        {/if}
        {#if app.secrets.simkl_token}
          <div class="stars" role="radiogroup" tabindex="-1" aria-label="Your rating" onmouseleave={() => (hoverStar = 0)}>
            <span class="foot dim">Your rating</span>
            {#each Array(10) as _, n}
              <button
                class="star"
                class:on={(hoverStar || rating || 0) > n}
                onmouseenter={() => (hoverStar = n + 1)}
                onclick={() => { haptic("selection"); rate(rating === n + 1 ? null : n + 1); }}
                aria-label="{n + 1} out of 10"
              ><Icon name="star" size={16} fill={(hoverStar || rating || 0) > n} /></button>
            {/each}
          </div>
        {/if}
      </div>
    </section>

    {#if videos.length}
      <section class="block episodes">
        <header class="block-h">
          <h2 class="title2">Episodes</h2>
          <div class="seasons scroll-x">
            {#each seasons as s}
              <button class="chip press" class:sel={s === season} onclick={() => (season = s)}>{s === 0 ? "Specials" : `Season ${s}`}</button>
            {/each}
          </div>
        </header>
        <div class="eps">
          {#each episodes as v (v.id)}
            {@const p = progressOf(id, v.id)}
            {@const seen = watched.has(v.id)}
            {@const future = v.released && new Date(v.released).getTime() > Date.now()}
            <div class="ep" class:future>
              <button class="thumb press" onclick={() => !future && openStreams(v)} aria-label="Play {v.title ?? v.name}">
                {#if v.thumbnail}<img src={img(v.thumbnail)} alt="" loading="lazy" />{/if}
                <span class="num">{v.episode ?? v.number}</span>
                {#if !future}<span class="play glass clear"><Icon name="play" size={16} /></span>{/if}
                {#if p > 0}<div class="bar"><span style:width="{p * 100}%"></span></div>{/if}
              </button>
              <div class="ep-txt">
                <div class="ep-h">
                  <span class="headline clamp-2">{v.title || v.name || `Episode ${v.episode}`}</span>
                  <button class="seen" class:on={seen} onclick={() => setWatched(m, v.id, !seen)} aria-label={seen ? "Mark unwatched" : "Mark watched"} title={seen ? "Watched" : "Mark watched"}>
                    <Icon name="check" size={15} />
                  </button>
                </div>
                <span class="foot faint">{future ? "Airs " : ""}{v.released ? new Date(v.released).toLocaleDateString(undefined, { day: "numeric", month: "short", year: "numeric" }) : ""}</span>
                {#if v.overview || v.description}<p class="foot dim clamp-3">{v.overview ?? v.description}</p>{/if}
              </div>
            </div>
          {/each}
        </div>
      </section>
    {/if}

    {#if cast.length}
      <section class="block">
        <header class="block-h"><h2 class="title2">Cast</h2></header>
        <div class="cast scroll-x">
          {#each cast as c}
            <button class="person press" onclick={() => go({ name: "person", person: c.name, tmdbId: c.id, adult: !!m.adult || !m.id.startsWith("tt") })}>
              <div class="face">
                {#if c.profile}<img src={img(c.profile)} alt="" loading="lazy" />{:else}<span>{c.name?.split(" ").map((x: string) => x[0]).slice(0, 2).join("")}</span>{/if}
              </div>
              <span class="nm">{c.name}</span>
              {#if c.character}<span class="ch">{c.character}</span>{/if}
            </button>
          {/each}
        </div>
      </section>
    {/if}

    {#if extra?.videos?.length > 1}
      <section class="block">
        <header class="block-h"><h2 class="title2">Trailers & extras</h2></header>
        <div class="vids scroll-x">
          {#each extra.videos as v (v.key)}
            <button class="vid press" onclick={() => playTrailer(v.key, v.name)}>
              <div class="vthumb">
                <img src={img(`https://i.ytimg.com/vi/${v.key}/hqdefault.jpg`)} alt="" loading="lazy" />
                <span class="play glass clear"><Icon name="play" size={16} /></span>
              </div>
              <span class="nm clamp-2">{v.name}</span>
              <span class="ch">{v.type}</span>
            </button>
          {/each}
        </div>
      </section>
    {/if}

    {#if extra?.collection?.parts?.length > 1}
      <section class="block">
        <header class="block-h"><h2 class="title2">{extra.collection.name}</h2></header>
        <div class="rail scroll-x">
          {#each extra.collection.parts as r (r.id)}
            <Poster meta={r} showTitle onclick={openRec} />
          {/each}
        </div>
      </section>
    {/if}

    {#if extra?.recommendations?.length}
      <section class="block">
        <header class="block-h"><h2 class="title2">More like this</h2></header>
        <div class="rail scroll-x">
          {#each extra.recommendations as r (r.id)}
            <Poster meta={r} onclick={openRec} />
          {/each}
        </div>
      </section>
    {/if}

    {#if extra}
      <section class="block about">
        <header class="block-h"><h2 class="title2">About</h2></header>
        <dl>
          {#if extra.directors?.length}<dt>Directed by</dt><dd>{extra.directors.join(", ")}</dd>{/if}
          {#if extra.creators?.length}<dt>Created by</dt><dd>{extra.creators.join(", ")}</dd>{/if}
          {#if extra.writers?.length}<dt>Written by</dt><dd>{extra.writers.join(", ")}</dd>{/if}
          {#if extra.studios?.length}<dt>Studio</dt><dd>{extra.studios.join(", ")}</dd>{/if}
          {#if extra.networks?.length}<dt>Network</dt><dd>{extra.networks.map((n: any) => n.name).join(", ")}</dd>{/if}
          {#if extra.spokenLanguages?.length}<dt>Languages</dt><dd>{extra.spokenLanguages.slice(0, 4).join(", ")}</dd>{/if}
          {#if extra.status}<dt>Status</dt><dd>{extra.status}</dd>{/if}
          {#if extra.budget}<dt>Budget</dt><dd>${(extra.budget / 1e6).toFixed(0)}M</dd>{/if}
          {#if extra.revenue}<dt>Box office</dt><dd>${(extra.revenue / 1e6).toFixed(0)}M</dd>{/if}
          {#if extra.keywords?.length}<dt>Mood</dt><dd class="tags">{#each extra.keywords as k}<span class="chip outline">{k}</span>{/each}</dd>{/if}
        </dl>
      </section>
    {/if}
    <div style="height: 80px"></div>
  </article>

  <StreamsSheet bind:open={sheetOpen} meta={m} video={sheetVideo} />
{/if}

<style>
  .hero {
    position: relative;
    min-height: min(86vh, 60vw);
    display: flex;
    align-items: flex-end;
  }
  .bg {
    /* fade the art itself into the page so there's no seam where it ends */
    -webkit-mask-image: linear-gradient(180deg, #000 50%, rgb(0 0 0 / 0.6) 72%, transparent 100%);
    mask-image: linear-gradient(180deg, #000 50%, rgb(0 0 0 / 0.6) 72%, transparent 100%);
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    object-position: center 20%;
  }
  .hero-shade {
    position: absolute;
    inset: 0;
    -webkit-mask-image: linear-gradient(180deg, #000 55%, transparent 100%);
    mask-image: linear-gradient(180deg, #000 55%, transparent 100%);
    background:
      linear-gradient(0deg, color-mix(in srgb, var(--bg) 70%, transparent) 0%, transparent 50%),
      linear-gradient(90deg, color-mix(in srgb, var(--bg) 92%, transparent) 0%, color-mix(in srgb, var(--bg) 60%, transparent) 38%, transparent 66%);
  }
  .info {
    position: relative;
    padding: 120px var(--gutter) 34px;
    width: min(700px, 56vw);
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .logo {
    max-width: min(480px, 90%);
    max-height: 170px;
    object-fit: contain;
    object-position: left bottom;
    filter: drop-shadow(0 4px 24px rgb(0 0 0 / 0.45));
    animation: rise 600ms var(--ease-out) both 120ms;
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(14px);
    }
  }
  .facts {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 12px;
  }
  .rating {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-weight: 700;
    font-size: var(--fs-sub);
    color: var(--rating);
  }
  :global(:root:not([data-appearance="light"])) .info {
    text-shadow: 0 1px 14px rgb(0 0 0 / 0.5);
  }
  .tagline {
    margin: 0;
    font-family: var(--font-display);
    font-size: 19px;
    font-style: italic;
    color: color-mix(in srgb, var(--text) 85%, transparent);
  }
  .desc {
    margin: 0;
    max-width: 66ch;
    color: color-mix(in srgb, var(--text) 82%, transparent);
    user-select: text;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
    margin-top: 4px;
  }
  .psub {
    font-weight: 500;
    opacity: 0.6;
    margin-left: 4px;
  }
  .secondary {
    display: flex;
    gap: 10px;
  }
  .act {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .act-ic {
    width: 50px;
    height: 50px;
    border-radius: 50%;
    display: grid;
    place-items: center;
  }
  .act-ic.lit {
    color: var(--green);
  }
  .act-l {
    display: none;
  }
  .desc-wrap {
    text-align: left;
    cursor: default;
  }
  .stars {
    display: flex;
    align-items: center;
    gap: 1px;
  }
  .stars .foot {
    margin-right: 8px;
  }
  .star {
    padding: 3px;
    color: var(--label-3);
    transition: transform var(--t-fast) var(--spring), color var(--t-fast);
  }
  .star.on {
    color: var(--rating);
  }
  .star:hover {
    transform: scale(1.2);
  }

  .block {
    margin-top: 34px;
  }
  .block-h {
    display: flex;
    align-items: center;
    gap: 18px;
    padding: 0 var(--gutter);
    margin-bottom: 14px;
  }
  .seasons {
    display: flex;
    gap: 6px;
  }
  .seasons .chip {
    height: 32px;
    padding: 0 14px;
    font-size: 13px;
  }

  .eps {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(420px, 1fr));
    gap: 14px 22px;
    padding: 0 var(--gutter);
  }
  .ep {
    display: flex;
    gap: 14px;
    align-items: flex-start;
  }
  .ep.future {
    opacity: 0.55;
  }
  .thumb {
    position: relative;
    width: 200px;
    flex: none;
    aspect-ratio: 16 / 9;
    border-radius: var(--r-sm);
    overflow: hidden;
    background: rgb(255 255 255 / 0.06);
  }
  .thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .num {
    position: absolute;
    left: 8px;
    bottom: 6px;
    font-family: var(--font-display);
    font-weight: 800;
    font-size: 26px;
    line-height: 1;
    color: #fff;
    text-shadow: 0 2px 10px rgb(0 0 0 / 0.6);
  }
  .play {
    position: absolute;
    right: 8px;
    bottom: 8px;
    width: 34px;
    height: 34px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    color: #fff;
    opacity: 0;
    transition: opacity var(--t-fast);
  }
  .thumb:hover .play,
  .vid:hover .play {
    opacity: 1;
  }
  .bar {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: 3px;
    background: rgb(255 255 255 / 0.25);
  }
  .bar span {
    display: block;
    height: 100%;
    background: var(--accent);
  }
  .ep-txt {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .ep-txt p {
    margin: 4px 0 0;
  }
  .ep-h {
    display: flex;
    align-items: flex-start;
    gap: 8px;
  }
  .ep-h .headline {
    flex: 1;
    font-size: 15px;
  }
  .seen {
    width: 26px;
    height: 26px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    flex: none;
    color: var(--text-faint);
    box-shadow: inset 0 0 0 1.5px currentColor;
    transition: all var(--t-fast);
  }
  .seen.on {
    background: var(--accent);
    color: var(--accent-ink);
    box-shadow: none;
  }

  .cast,
  .vids,
  .rail {
    display: flex;
    gap: 16px;
    padding: 4px var(--gutter) 12px;
  }
  .person {
    width: 112px;
    flex: none;
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    gap: 2px;
  }
  .face {
    width: 96px;
    height: 96px;
    border-radius: 50%;
    overflow: hidden;
    background: rgb(255 255 255 / 0.08);
    display: grid;
    place-items: center;
    font-weight: 700;
    color: var(--text-dim);
    margin-bottom: 8px;
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.1);
    transition: transform var(--t-med) var(--spring);
  }
  .person:hover .face {
    transform: scale(1.05);
  }
  .face img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .nm {
    font-weight: 600;
    font-size: 13.5px;
  }
  .ch {
    font-size: 12px;
    color: var(--text-dim);
  }
  .vid {
    width: 260px;
    flex: none;
    text-align: left;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .vthumb {
    position: relative;
    aspect-ratio: 16 / 9;
    border-radius: var(--r-sm);
    overflow: hidden;
    margin-bottom: 4px;
  }
  .vthumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .about dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 10px 28px;
    margin: 0;
    padding: 0 var(--gutter);
    max-width: 900px;
  }
  dt {
    color: var(--text-dim);
  }
  dd {
    margin: 0;
  }
  .tags {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .err {
    height: 70vh;
    display: grid;
    place-content: center;
    justify-items: center;
    gap: 6px;
    text-align: center;
    color: var(--text-dim);
  }
  .err p {
    margin: 0;
  }
  @media (max-width: 759px) {
    .hero {
      min-height: 0;
    }
    .bg {
      height: 72vw;
      max-height: 480px;
      inset: 0 0 auto 0;
      -webkit-mask-image: linear-gradient(180deg, #000 60%, transparent 100%);
      mask-image: linear-gradient(180deg, #000 60%, transparent 100%);
    }
    .hero-shade {
      display: none;
    }
    .info {
      width: auto;
      padding: calc(min(72vw, 480px) - 70px) var(--gutter) 6px;
      align-items: center;
      text-align: center;
      gap: 14px;
    }
    .logo {
      object-position: center bottom;
      max-height: 110px;
      max-width: 80%;
    }
    .facts {
      justify-content: center;
    }
    .actions {
      width: 100%;
      flex-direction: column;
      gap: 18px;
    }
    .cta {
      width: 100%;
    }
    .secondary {
      width: 100%;
      justify-content: space-around;
    }
    .act {
      flex-direction: column;
      gap: 6px;
      flex: 1;
    }
    .act-ic {
      width: 46px;
      height: 46px;
    }
    .act-l {
      display: block;
      font-size: var(--fs-caption);
      color: var(--label-2);
      font-weight: 500;
    }
    .desc {
      text-align: left;
      font-size: var(--fs-sub);
    }
    .tagline {
      font-size: var(--fs-callout);
    }
    .stars {
      justify-content: center;
      flex-wrap: wrap;
    }
    .stars .foot {
      width: 100%;
      margin: 0 0 4px;
    }
    .block {
      margin-top: 28px;
    }
    .block-h {
      flex-wrap: wrap;
      gap: 10px;
    }
    .block-h .title2 {
      font-size: var(--fs-title3);
    }
    .seasons {
      width: calc(100% + var(--gutter));
      margin-right: calc(-1 * var(--gutter));
    }
    .seasons .chip {
      flex: none;
    }
    .eps {
      grid-template-columns: 1fr;
    }
    .thumb {
      width: 140px;
    }
    .about dl {
      grid-template-columns: 1fr;
      gap: 2px;
    }
    dd {
      margin-bottom: 10px;
    }
  }
</style>
