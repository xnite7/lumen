<script lang="ts">
  import { fallback, imdbArt, imdbEpisode } from "../lib/fallback";
  import { tvFocus } from "../lib/tvfocus";
  import { api, img, type Meta, type Video } from "../lib/api";
  import { app } from "../lib/stores/app.svelte";
  import { setAmbient } from "../lib/stores/ambient.svelte";
  import { pageCtx, isActive } from "../lib/page";
  import { parallax } from "../lib/stores/gaze.svelte";
  const page = pageCtx();
  import { inLibrary, toggleLibrary, setWatched, watchedIds, progressOf, itemOf, lib } from "../lib/stores/library.svelte";
  import { play, orderedVideos } from "../lib/stores/playback.svelte";
  import { toast, errorText } from "../lib/stores/toast.svelte";
  import { go } from "../lib/router.svelte";
  import { openDetail } from "../lib/open";
  import { year, runtime, episodeLabel } from "../lib/format";
  import StreamsSheet from "../components/StreamsSheet.svelte";
  import { quickPlay } from "../lib/quickplay";
  import Poster from "../components/Poster.svelte";
  import Icon from "../components/Icon.svelte";
  import NavBar from "../ui/NavBar.svelte";
  import { share, haptic } from "../lib/native";
  import { ui } from "../lib/ui.svelte";
  import { trimLogo } from "../lib/trimlogo";
  import { openGenre } from "../lib/genrelink.svelte";
  import { checkRelease, isUnreleased, releaseNote } from "../lib/release.svelte";
  import RateSheet from "../components/RateSheet.svelte";
  import { social, loadMe, reviewsOf, postActivity, isFav, toggleFav, ago, ref, type Activity } from "../lib/stores/social.svelte";

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
  // Share = a link that opens couch right on this page (for anyone with couch)
  async function doShare() {
    const mm = meta ?? preview;
    if (!mm) return;
    try {
      const url = await api.shareLink(mm.type, mm.id, mm.name);
      await navigator.clipboard.writeText(url);
      haptic("success" as any);
      toast("link copied. anyone with couch opens it right here.", "ok");
    } catch {
      share({ title: mm.name, url: mm.id?.startsWith("tt") ? `https://www.imdb.com/title/${mm.id.split(":")[0]}/` : undefined });
    }
  }
  // Simkl's episode runtimes + finale markers (season / series finale)
  let epInfo = $state<Record<string, { runtime?: number | null; finale?: number | null }>>({});
  $effect(() => {
    const t = type, i = id;
    epInfo = {};
    if (t === "series") api.episodeInfo(i).then((r) => (epInfo = r)).catch(() => {});
  });
  const finaleName = ["", "Mid-season finale", "Season finale", "Series finale"];
  let reviewOpen = $state(false);
  let logoFailed = $state(false);
  $effect(() => {
    void logo;
    logoFailed = false;
  });
  const spoilerFree = $derived(!!app.settings.playback.blurUnwatched);
  let reviews = $state<Activity[]>([]);
  const myReview = $derived(reviews.find((r) => r.who === social.me?.id));
  const others = $derived(reviews.filter((r) => r.who !== social.me?.id && (r.text || r.rating)));
  $effect(() => {
    const i = id;
    reviews = [];
    loadMe();
    reviewsOf(i).then((r) => (reviews = r));
  });
  function reviewClosed() {
    reviewsOf(id).then((r) => (reviews = r));
  }

  const m = $derived(meta ?? (preview as Meta | undefined) ?? null);
  // dimmed only when couch finds no streams for a too-new title (leaks count)
  $effect(() => checkRelease(m));
  const unreleased = $derived(isUnreleased(m?.id));
  const relNote = $derived(releaseNote(m?.id));

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
    api.simklStatus(t, i).then((s) => (rating = s?.rating ?? null)).catch(() => {});
  });

  const mainGenre = $derived(m ? String((extra?.genres ?? m.genres)?.[0] ?? "") : "");
  const factLine = $derived.by(() => {
    if (!m) return [];
    const n = seasons.filter((x) => x > 0).length;
    const len = m.type === "series" ? (n ? `${n} Season${n > 1 ? "s" : ""}` : "") : m.runtime || extra?.runtime ? runtime(m.runtime ?? extra?.runtime) : "";
    return [m.type === "series" ? "Show" : "Movie", (extra?.genres ?? m.genres)?.[0] ?? "", year(m), len, extra?.certification ?? ""].filter(Boolean).map(String);
  });
  // The art the page opened with (the billboard's / the poster's) stays put;
  // later loads only fill in what was missing, so nothing swaps after a second.
  const backdrop = $derived(preview?.background ?? m?.background ?? extra?.backdrop ?? m?.poster);
  const logo = $derived(preview?.logo ?? m?.logo ?? extra?.logo);
  $effect(() => {
    if (backdrop && isActive(page)) setAmbient(backdrop);
  });

  const videos = $derived(orderedVideos(meta));
  const seasons = $derived([...new Set(videos.map((v) => v.season ?? 0))].sort((a, b) => (a || 999) - (b || 999)));
  const watched = $derived(watchedIds(id));
  const item = $derived(itemOf(id));
  // rating (and reviewing) only once you've actually watched it
  const seenIt = $derived(rating != null || !!item?.state?.flaggedWatched || watched.size > 0 || ((item?.state as any)?.timesWatched ?? 0) > 0);
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
      if (v) return { label: "Resume", video: v, sub: episodeLabel(v.id).replace(/\s*·\s*/, " ") };
    }
    const next = videos.find((v) => (v.season ?? 0) > 0 && !watched.has(v.id) && (!v.released || new Date(v.released).getTime() < Date.now()));
    const v = next ?? videos[0];
    return { label: next && next !== videos.find((x) => (x.season ?? 0) > 0) ? "Continue" : "Play", video: v, sub: episodeLabel(v.id).replace(/\s*·\s*/, " ") };
  });

  function openStreams(v: Video | null) {
    // couch picks the stream (by your connection); the list only if you asked for it
    if (app.settings.streams.autoplayBest && m) {
      quickPlay(m, null, { videoId: v?.id });
      return;
    }
    sheetVideo = v;
    sheetOpen = true;
  }

  function trailerKey() {
    return extra?.videos?.find((v: any) => v.type === "Trailer")?.key ?? m?.trailers?.[0]?.source ?? extra?.videos?.[0]?.key;
  }

  function playTrailer(key?: string, name = "Trailer") {
    const k = key ?? trailerKey();
    if (!k || !m) return;
    play(m, { ytId: k, title: name, _couch: { kind: "youtube" } as any, _addonId: "", _addonName: "" } as any, m.id);
  }

  async function rate(n: number | null) {
    if (!m) return;
    try {
      await api.simklRate(m.type, m.id, n);
      rating = n;
      toast(n ? `Rated ${m.name} ${n}/10${app.secrets.simkl_token ? " on Simkl" : ""}` : "Rating removed", "ok");
      if (n) postActivity("rating", m, n);
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
  </NavBar>
  <article class="detail">
    <section class="hero">
      <!-- the picture AND its darkening travel together when a poster opens
           into this page, so the vignette never pops in at the end -->
      <div class="art-stack" style:view-transition-name="hero-art">
        {#if backdrop}
          <img class="bg" src={img(backdrop, "original")} alt="" use:parallax={1} use:fallback={[imdbArt(m.id, "background")]} />
        {/if}
        <div class="hero-shade"></div>
        <div class="hero-vignette"></div>
        <div class="lamp-glow" aria-hidden="true"></div>
      </div>
      <div class="info" use:parallax={-0.3} style:view-transition-name="hero-info">
        {#if logo && !logoFailed}
          <img class="logo" src={img(logo)} alt={m.name} use:trimLogo use:fallback={{ alts: [imdbArt(m.id, "logo")], onfail: () => (logoFailed = true) }} />
        {:else}
          <h1 class="large-title">{m.name}</h1>
        {/if}
        <!-- same line as the billboard: Show • Thriller • 2017 • 2 Seasons • 16+ -->
        <div class="facts">
          {#each factLine as f, n}{#if n}<span class="sep" aria-hidden="true"></span>{/if}{#if n === 1 && f === mainGenre}<button class="genre-link" onclick={() => openGenre(m.type, f)} title="More {f}">{f}</button>{:else}<span>{f}</span>{/if}{/each}
        </div>
        <div class="actions">
          {#if primary && unreleased}
            <button class="btn lg primary press cta unreleased" onclick={() => toast(`${m.name}: ${relNote}`, "info")} title={relNote}>
              <Icon name="play" size={18} />Unreleased<span class="psub">{relNote}</span>
            </button>
          {:else if primary}
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
              <span class="act-ic glass" class:lit={item?.state?.flaggedWatched}><Icon name={item?.state?.flaggedWatched ? "checkCircle" : "eye"} size={20} /></span><span class="act-l">{item?.state?.flaggedWatched ? "Watched" : "Mark watched"}</span>
            </button>
            <button class="act press" onclick={() => { haptic("light"); toast(toggleFav(m) ? "added to favourites" : "removed from favourites", "ok"); }} aria-label="Favourite">
              <span class="act-ic glass" class:fav={isFav(m.id)}><Icon name="heart" size={19} fill={isFav(m.id)} /></span><span class="act-l">Favourite</span>
            </button>
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
        {#if seenIt}
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
          <button class="review-btn press" onclick={() => (reviewOpen = true)}>
            <Icon name="pencil" size={15} />{myReview?.text ? "Edit your review" : "Write a review"}
          </button>
        {/if}
      </div>
    </section>

    {#if videos.length}
      <section class="block episodes">
        <header class="block-h">
          <h2 class="title2">episodes</h2>
          <div class="seasons scroll-x">
            {#each seasons as s}
              <button class="chip press" class:sel={s === season} use:tvFocus={{ tilt: false }} onclick={() => (season = s)}>{s === 0 ? "Specials" : `Season ${s}`}</button>
            {/each}
          </div>
        </header>
        <div class="eps scroll-x">
          {#each episodes as v (v.id)}
            {@const p = progressOf(id, v.id)}
            {@const seen = watched.has(v.id)}
            {@const future = v.released && new Date(v.released).getTime() > Date.now()}
            {@const info = epInfo[`${v.season}:${v.episode ?? v.number}`]}
            {@const hide = spoilerFree && !seen && p <= 0 && !future}
            <div class="ep" class:future class:hide>
              <div class="thumb-wrap">
                <button class="thumb press" use:tvFocus={{ max: 5 }} onclick={() => !future && openStreams(v)} aria-label="Play {v.title ?? v.name}">
                  <!-- the episode's still, IMDb's, then the show's own art; else couch's placeholder -->
                  <img src={img(v.thumbnail ?? imdbEpisode(m.id, v.season, v.episode ?? v.number) ?? undefined)} alt="" loading="lazy" use:fallback={[imdbEpisode(m.id, v.season, v.episode ?? v.number), backdrop ? img(backdrop, "w780") : null]} />
                  <span class="num">{v.episode ?? v.number}</span>
                  {#if !future}<span class="play glass clear"><Icon name="play" size={16} /></span>{/if}
                </button>
                <!-- watched tick: top right of the picture -->
                <button class="seen" class:on={seen} onclick={() => setWatched(m, v.id, !seen)} aria-label={seen ? "Mark unwatched" : "Mark watched"} title={seen ? "Watched" : "Mark watched"}>
                  <Icon name="check" size={15} />
                </button>
              </div>
              {#if p > 0}<div class="bar"><span style:width="{p * 100}%"></span></div>{/if}
              <div class="ep-txt">
                <div class="ep-h">
                  <span class="headline clamp-2">{v.title || v.name || `Episode ${v.episode}`}</span>
                </div>
                <span class="foot faint">{future ? "Airs " : ""}{v.released ? new Date(v.released).toLocaleDateString(undefined, { day: "numeric", month: "short", year: "numeric" }) : ""}{(v as any).runtime || info?.runtime ? ` · ${runtime((v as any).runtime ?? info?.runtime)}` : ""}</span>
                {#if info?.finale}<span class="finale f{info.finale}">{finaleName[info.finale]}</span>{/if}
                {#if v.overview || v.description}<p class="foot dim clamp-3">{v.overview ?? v.description}</p>{/if}
              </div>
            </div>
          {/each}
        </div>
      </section>
    {/if}

    {#if others.length || myReview?.text}
      <section class="block">
        <header class="block-h"><h2 class="title2">On the couch</h2></header>
        <div class="reviews">
          {#each [...(myReview?.text ? [myReview] : []), ...others] as r (r.id)}
            <article class="rv">
              <div class="rv-who">
                {#if r.avatar}<img src={r.avatar} alt="" />{:else}<span class="ph">{r.name.slice(0, 1)}</span>{/if}
                <span class="headline">{r.who === social.me?.id ? "you" : r.name}</span>
                {#if r.rating}<span class="rv-score"><Icon name="star" size={12} fill /> {r.rating}/10</span>{/if}
                <span class="foot faint">{ago(r.at)}</span>
              </div>
              {#if r.text}<p class="rv-text">{r.text}</p>{/if}
            </article>
          {/each}
        </div>
      </section>
    {/if}

    {#if extra?.companies?.length}
      <section class="block">
        <header class="block-h"><h2 class="title2">Studio</h2></header>
        <div class="studios scroll-x">
          {#each extra.companies as c (c.id)}
            <button class="studio press" onclick={() => go({ name: "studio", id: c.id, label: c.name, logo: c.logo })} title={c.name}>
              {#if c.logo}<img src={c.logo} alt={c.name} loading="lazy" />{:else}<span>{c.name}</span>{/if}
            </button>
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
              <div class="face" class:img-ph={!c.profile}>
                <span class="initials">{c.name?.split(" ").map((x: string) => x[0]).slice(0, 2).join("")}</span>
                {#if c.profile}<img src={img(c.profile)} alt="" loading="lazy" use:fallback={[]} />{/if}
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
                <img src={img(`https://i.ytimg.com/vi/${v.key}/hqdefault.jpg`)} alt="" loading="lazy" use:fallback={{ alts: [backdrop ? img(backdrop, "w780") : null], minWidth: 120 }} />
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
    {#if reviewOpen}
    <RateSheet bind:open={reviewOpen} meta={ref(m)} bind:rating text={myReview?.text ?? ""} onclose={reviewClosed} />
  {/if}
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
    filter: saturate(0.92) brightness(0.9);
    /* fade the art itself into the page so there's no seam where it ends */
    -webkit-mask-image: linear-gradient(180deg, var(--black) 50%, rgb(var(--lo) / 0.6) 72%, transparent 100%);
    mask-image: linear-gradient(180deg, var(--black) 50%, rgb(var(--lo) / 0.6) 72%, transparent 100%);
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    object-position: center 20%;
  }
  .art-stack {
    position: absolute;
    inset: 0;
  }
  /* Apple TV's dark rising from the bottom, in plum */
  .hero-shade {
    position: absolute;
    inset: 0;
    background:
      linear-gradient(0deg, var(--bg) 0%, color-mix(in srgb, var(--bg) 88%, transparent) 18%, color-mix(in srgb, var(--bg) 40%, transparent) 50%, transparent 74%),
      linear-gradient(90deg, color-mix(in srgb, var(--bg) 80%, transparent) 0%, color-mix(in srgb, var(--bg) 45%, transparent) 36%, transparent 64%),
      linear-gradient(180deg, color-mix(in srgb, var(--bg) 50%, transparent) 0%, transparent 20%);
  }
  .hero-vignette {
    position: absolute;
    inset: 0;
    pointer-events: none;
    background: radial-gradient(120% 95% at 60% 35%, transparent 50%, rgb(var(--lo) / 0.6) 100%);
  }
  /* a warm lamp somewhere behind the title */
  .lamp-glow {
    position: absolute;
    left: -10%;
    bottom: -10%;
    width: 60%;
    height: 70%;
    pointer-events: none;
    background: radial-gradient(closest-side, rgb(var(--lamp-rgb) / calc(0.09 + var(--night) * 0.06)), transparent);
    filter: blur(20px);
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
    filter: drop-shadow(0 4px 24px rgb(var(--lo) / 0.45));
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
    gap: 6px 10px;
    font-size: 17px;
    font-weight: 600;
    color: #fff;
  }
  .sep {
    width: 4px;
    height: 4px;
    border-radius: 50%;
    background: rgb(var(--hi) / 0.45);
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
    text-shadow: 0 1px 14px rgb(var(--lo) / 0.5);
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
    max-width: 56ch;
    color: #fff;
    font-size: 18px;
    font-weight: 500;
    line-height: 1.42;
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
  .finale {
    align-self: flex-start;
    padding: 2px 8px;
    border-radius: 6px;
    font: 700 10.5px system-ui, sans-serif;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    background: rgb(255 178 92 / 0.16);
    color: #ffb25c;
  }
  .finale.f3 {
    background: rgb(224 48 154 / 0.2);
    color: #ff6fc0;
  }
  .cta.unreleased {
    opacity: 0.42;
    filter: grayscale(1);
    cursor: default;
  }
  .genre-link {
    font: inherit;
    color: inherit;
    text-decoration: underline transparent 1.5px;
    text-underline-offset: 3px;
    transition: text-decoration-color var(--t-fast);
  }
  .genre-link:hover {
    text-decoration-color: currentColor;
  }
  .review-btn {
    align-self: flex-start;
    display: inline-flex;
    align-items: center;
    gap: 7px;
    margin-top: -6px;
    padding: 7px 13px;
    border-radius: 999px;
    font-size: 13px;
    font-weight: 600;
    color: var(--label);
    background: rgb(255 255 255 / 0.08);
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.12);
  }
  .review-btn:hover {
    background: rgb(255 255 255 / 0.14);
  }
  .act-ic.fav {
    color: #ff5a8a;
  }
  .reviews {
    padding: 0 var(--gutter);
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(min(100%, 340px), 1fr));
    gap: 14px;
  }
  .rv {
    padding: 14px 16px;
    border-radius: 16px;
    background: rgb(255 255 255 / 0.05);
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.08);
  }
  .rv-who {
    display: flex;
    align-items: center;
    gap: 9px;
  }
  .rv-who img,
  .rv-who .ph {
    width: 28px;
    height: 28px;
    border-radius: 9px;
    object-fit: cover;
    display: grid;
    place-items: center;
    background: var(--fill);
    font-weight: 700;
  }
  .rv-score {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    color: var(--rating);
    font: 700 12px var(--font-text);
  }
  .rv-who .foot {
    margin-left: auto;
  }
  .rv-text {
    margin: 10px 0 0;
    white-space: pre-wrap;
    line-height: 1.45;
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

  /* Apple TV episode shelf: landscape stills, text underneath */
  .eps {
    display: flex;
    gap: 20px;
    padding: 8px var(--gutter) 18px;
    scroll-snap-type: x proximity;
    scroll-padding: 0 var(--gutter);
  }
  .ep {
    flex: 0 0 auto;
    width: var(--land-w);
    display: flex;
    flex-direction: column;
    gap: 10px;
    scroll-snap-align: start;
  }
  .ep.future {
    opacity: 0.55;
  }
  .thumb {
    position: relative;
    width: 100%;
    flex: none;
    aspect-ratio: 16 / 9;
    border-radius: var(--card-r);
    overflow: hidden;
    background: var(--surface);
    box-shadow: 0 12px 30px -16px rgb(var(--lo) / 0.8), inset 0 0 0 1px rgb(var(--hi) / 0.06);
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
    color: var(--white);
    text-shadow: 0 2px 10px rgb(var(--lo) / 0.6);
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
    color: var(--white);
    opacity: 0;
    transition: opacity var(--t-fast);
  }
  .thumb:hover .play,
  .vid:hover .play {
    opacity: 1;
  }
  /* progress: under the picture, 60% of its width, centred */
  .bar {
    position: relative;
    width: 60%;
    margin: -4px auto 0;
    height: 3px;
    border-radius: 3px;
    overflow: hidden;
    background: rgb(var(--hi) / 0.18);
  }
  .thumb-wrap {
    position: relative;
  }
  .thumb-wrap .seen {
    position: absolute;
    top: 8px;
    right: 8px;
    color: var(--white);
    background: rgb(0 0 0 / 0.45);
    backdrop-filter: blur(8px);
    box-shadow: inset 0 0 0 1.5px rgb(255 255 255 / 0.7);
  }
  .thumb-wrap .seen.on {
    background: var(--accent);
    color: var(--accent-ink);
    box-shadow: none;
  }
  /* spoiler-free: episodes you haven't seen keep their picture and story blurred */
  .ep.hide .thumb img {
    filter: blur(14px) saturate(0.8);
    transform: scale(1.12);
  }
  .ep.hide .ep-txt p {
    filter: blur(5px);
    user-select: none;
    transition: filter var(--t-med);
  }
  .ep.hide:hover .ep-txt p {
    filter: blur(0);
  }
  .bar span {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: var(--progress);
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

  .studios {
    display: flex;
    gap: 12px;
    padding: 4px var(--gutter) 12px;
  }
  /* company logos are made for white backgrounds */
  .studio {
    flex: none;
    width: 168px;
    height: 84px;
    display: grid;
    place-items: center;
    padding: 14px 18px;
    border-radius: 16px;
    background: #f4f1f3;
    color: #111;
    font-weight: 800;
    text-align: center;
    transition: transform 0.2s var(--spring, ease);
  }
  .studio:hover {
    transform: translateY(-2px) scale(1.03);
  }
  .studio img {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
  }
  .cast,
  .vids,
  .rail {
    display: flex;
    gap: 16px;
    padding: 4px var(--gutter) 12px;
  }
  .person {
    width: 128px;
    flex: none;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    text-align: left;
    gap: 2px;
  }
  /* portrait cards, like a cast sheet */
  .face {
    width: 128px;
    aspect-ratio: 2 / 3;
    border-radius: 10px;
    overflow: hidden;
    background: rgb(var(--hi) / 0.08);
    display: grid;
    place-items: center;
    font-weight: 700;
    color: var(--text-dim);
    margin-bottom: 8px;
    box-shadow: inset 0 0 0 1px rgb(var(--hi) / 0.1);
    transition: transform var(--t-med) var(--spring);
  }
  .face {
    position: relative;
  }
  .face img {
    position: absolute;
    inset: 0;
    object-fit: cover;
  }
  .initials {
    font-size: 28px;
    letter-spacing: -0.02em;
    color: rgb(var(--hi) / 0.45);
  }
  /* no photo: a soft gradient + their initials (no mark behind) */
  .face:global(.img-ph) {
    background: radial-gradient(80% 70% at 50% 30%, rgb(224 48 154 / 0.22), transparent 70%), linear-gradient(160deg, #1d1220, #0b070a) !important;
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
      -webkit-mask-image: linear-gradient(180deg, var(--black) 60%, transparent 100%);
      mask-image: linear-gradient(180deg, var(--black) 60%, transparent 100%);
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
    .ep {
      width: 78vw;
    }
    .about dl {
      grid-template-columns: 1fr;
      gap: 2px;
    }
    dd {
      margin-bottom: 10px;
    }
  }
  /* the billboard's Play: warm-white slab, bold black label */
  .cta {
    height: 52px;
    padding: 0 30px 0 24px;
    gap: 10px;
    border-radius: 14px;
    background: #fff4ea;
    color: #000;
    font-size: 19px;
    font-weight: 700;
    box-shadow: 0 10px 30px -12px rgb(var(--lo) / 0.8);
  }
  .cta :global(svg) {
    width: 24px;
    height: 24px;
  }
</style>
