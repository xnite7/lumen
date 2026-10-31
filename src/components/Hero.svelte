<script lang="ts">
  import { api, img, type Meta } from "../lib/api";
  import { app } from "../lib/stores/app.svelte";
  import { setAmbient } from "../lib/stores/ambient.svelte";
  import { pageCtx, isActive } from "../lib/page";
  const page = pageCtx();
  import { inLibrary, toggleLibrary } from "../lib/stores/library.svelte";
  import { openDetail } from "../lib/open";
  import { year, runtime } from "../lib/format";
  import YouTube from "./YouTube.svelte";
  import Icon from "./Icon.svelte";
  import { ui } from "../lib/ui.svelte";
  import { haptic } from "../lib/native";
  import { pressMenu } from "../lib/menu.svelte";
  import { posterMenu } from "../lib/actions";

  let { scrollY = 0 }: { scrollY?: number } = $props();

  let slides = $state<Meta[]>([]);
  let i = $state(0);
  let trailer = $state<string | null>(null);
  let trailerOn = $state(false);
  let muted = $state(false);
  let el: HTMLElement | undefined = $state();
  let paused = $state(false);
  let art = $state<Record<string, string>>({});
  let rotate: ReturnType<typeof setTimeout> | undefined;
  let dwell: ReturnType<typeof setTimeout> | undefined;

  const cur = $derived(slides[i]);

  $effect(() => {
    api.hero().then((s) => (slides = (s ?? []).filter((m) => m.background || m.poster).slice(0, 10)));
  });

  // Wide MOTN backdrops when available (nicer crops than metahub).
  $effect(() => {
    const m = cur;
    if (!m || art[m.id] !== undefined || !m.id.startsWith("tt")) return;
    art[m.id] = "";
    api.art(m.id).then((a) => {
      if (a?.backdrop) art[m.id] = a.backdrop;
    }).catch(() => {});
  });

  const bg = $derived(cur ? art[cur.id] || cur.background || cur.poster : undefined);

  $effect(() => {
    if (bg && scrollY < 300 && isActive(page)) setAmbient(bg);
  });

  function schedule() {
    clearTimeout(rotate);
    clearTimeout(dwell);
    trailer = null;
    trailerOn = false;
    if (!slides.length) return;
    rotate = setTimeout(() => !paused && !trailerOn && next(), 9000);
    // After sitting at the top for a moment, the slide's trailer takes over.
    if (app.settings.general.heroTrailers && !app.playing && !ui.phone) {
      dwell = setTimeout(async () => {
        const m = cur;
        if (!m || scrollY > 200) return;
        const k = m.trailer ?? m.trailers?.[0]?.source ?? (m.id.startsWith("tt") ? await api.trailer(m.type, m.id) : null);
        if (k && cur?.id === m.id) {
          trailer = k;
          muted = app.settings.general.heroMuted;
        }
      }, 3000);
    }
  }

  $effect(() => {
    void i;
    void slides.length;
    schedule();
    return () => {
      clearTimeout(rotate);
      clearTimeout(dwell);
    };
  });

  // Scrolled away: the trailer stops (no sound from a hero you can't see).
  $effect(() => {
    if (scrollY > 380 && trailer) {
      trailer = null;
      trailerOn = false;
    }
  });

  function next() {
    if (slides.length) i = (i + 1) % slides.length;
  }
  function pick(n: number) {
    i = n;
  }

  const owned = $derived(cur ? inLibrary(cur.id) : false);

  // phone: a swipeable track of poster cards
  let track: HTMLElement | undefined = $state();
  function onTrack() {
    if (!track) return;
    const w = track.clientWidth * 0.84;
    const n = Math.round(track.scrollLeft / Math.max(1, w));
    if (n !== i && n >= 0 && n < slides.length) {
      i = n;
      haptic("selection");
    }
  }
  const facts = (m: Meta) => [m.type === "series" ? "Series" : "Film", year(m), m.runtime ? runtime(m.runtime) : ""].filter(Boolean).join(" · ");
  const lift = $derived(Math.min(1, scrollY / 520));
</script>

{#if cur && ui.phone}
  <section class="hero-phone" aria-roledescription="carousel">
    <div class="glow" style:background-image="url({img(cur.poster ?? bg, 'w342')})"></div>
    <div class="track scroll-x" bind:this={track} onscroll={onTrack}>
      {#each slides as m, n (m.id)}
        <article class="card" class:on={n === i} use:pressMenu={() => posterMenu(m)}>
          <button class="art" onclick={() => openDetail(m)} aria-label={m.name}>
            <img src={img(m.poster ?? m.background, "w780")} alt="" />
          </button>
          <div class="card-shade"></div>
          <div class="card-info">
            {#if m.logo}
              <img class="card-logo" src={img(m.logo)} alt={m.name} />
            {:else}
              <h2 class="title1">{m.name}</h2>
            {/if}
            <p class="foot facts-line">{[facts(m), m.genres?.slice(0, 2).join(", ")].filter(Boolean).join(" · ")}</p>
            <div class="card-actions">
              <button class="btn primary press grow" onclick={() => openDetail(m)}><Icon name="play" size={16} /> Play</button>
              <button class="btn glass press grow" onclick={() => { haptic("light"); toggleLibrary(m); }}>
                <Icon name={inLibrary(m.id) ? "check" : "plus"} size={18} /> My List
              </button>
            </div>
          </div>
        </article>
      {/each}
    </div>
    <div class="pdots" aria-hidden="true">
      {#each slides as _, n}<span class:on={n === i}></span>{/each}
    </div>
  </section>
{:else if cur}
  <section
    class="hero"
    bind:this={el}
    style:--lift={lift}
    onmouseenter={() => (paused = true)}
    onmouseleave={() => (paused = false)}
    aria-roledescription="carousel"
    role="group"
  >
    <div class="stage">
      {#key cur.id}
        <img class="bg" src={img(bg, "original")} alt="" />
      {/key}
      {#if trailer}
        <YouTube id={trailer} {muted} fill onplaying={() => (trailerOn = true)} onended={() => setTimeout(next, 2500)} />
      {/if}
      <div class="hero-shade"></div>
    </div>

    <div class="info" class:quiet={trailerOn}>
      {#key cur.id}
        <div class="copy">
          {#if cur.logo}
            <img class="logo" src={img(cur.logo)} alt={cur.name} />
          {:else}
            <h1 class="xl-title">{cur.name}</h1>
          {/if}
          <div class="facts">
            {#if cur.imdbRating}<span class="rating"><Icon name="star" size={12} fill /> {cur.imdbRating}</span>{/if}
            <span class="sub">{facts(cur)}</span>
            {#if cur.certification}<span class="chip small outline">{cur.certification}</span>{/if}
            {#if cur.genres?.length}<span class="sub dim">{cur.genres.slice(0, 3).join(" · ")}</span>{/if}
          </div>
          {#if cur.description}<p class="desc clamp-3">{cur.description}</p>{/if}
        </div>
      {/key}
      <div class="actions">
        <button class="btn lg primary press" onclick={() => openDetail(cur, el)}><Icon name="play" size={18} /> Play</button>
        <button class="btn lg glass press" onclick={() => toggleLibrary(cur)}>
          <Icon name={owned ? "check" : "plus"} size={19} />
          {owned ? "In My List" : "My List"}
        </button>
        {#if trailer}
          <button class="btn lg icon glass press" onclick={() => (muted = !muted)} aria-label={muted ? "Unmute" : "Mute"}>
            <Icon name={muted ? "mute" : "volume"} size={19} />
          </button>
        {/if}
      </div>
    </div>

    <div class="dots glass clear" role="tablist">
      {#each slides as s, n (s.id)}
        <button class="dot" class:on={n === i} onclick={() => pick(n)} role="tab" aria-selected={n === i} aria-label={s.name}>
          {#if n === i}<span class="fill" class:run={!paused && !trailerOn}></span>{/if}
        </button>
      {/each}
    </div>
  </section>
{:else}
  <section class="hero empty" class:phone={ui.phone}>
    <div class="stage skeleton"></div>
  </section>
{/if}

<style>
  .hero {
    position: relative;
    height: min(78vh, 52vw);
    min-height: 460px;
    overflow: hidden;
  }
  .stage {
    position: absolute;
    inset: 0;
    transform: translateY(calc(var(--lift, 0) * 18%)) scale(calc(1 + var(--lift, 0) * 0.06));
    opacity: calc(1 - var(--lift, 0) * 0.6);
    will-change: transform;
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
    object-position: center 22%;
    animation: bg-in 1.1s var(--ease-out) both;
  }
  @keyframes bg-in {
    from {
      opacity: 0;
      transform: scale(1.04);
    }
  }
  .hero-shade {
    position: absolute;
    inset: 0;
    -webkit-mask-image: linear-gradient(180deg, #000 55%, transparent 100%);
    mask-image: linear-gradient(180deg, #000 55%, transparent 100%);
    background:
      linear-gradient(0deg, color-mix(in srgb, var(--bg) 70%, transparent) 0%, transparent 50%),
      linear-gradient(90deg, color-mix(in srgb, var(--bg) 90%, transparent) 0%, color-mix(in srgb, var(--bg) 55%, transparent) 34%, transparent 62%);
  }
  .info {
    position: absolute;
    left: var(--gutter);
    bottom: 72px;
    width: min(620px, 46vw);
    display: flex;
    flex-direction: column;
    gap: 18px;
    transition: opacity var(--t-slow) ease, transform var(--t-slow) var(--glide);
  }
  .info.quiet .copy .desc,
  .info.quiet .facts {
    opacity: 0;
    transform: translateY(6px);
  }
  .copy {
    display: flex;
    flex-direction: column;
    gap: 14px;
    animation: copy-in 700ms var(--ease-out) both;
  }
  @keyframes copy-in {
    from {
      opacity: 0;
      transform: translateY(12px);
    }
  }
  .logo {
    max-width: min(460px, 80%);
    max-height: 150px;
    object-fit: contain;
    object-position: left bottom;
    filter: drop-shadow(0 4px 24px rgb(0 0 0 / 0.45));
  }
  .facts {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px 12px;
    transition: opacity var(--t-slow), transform var(--t-slow);
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
    text-shadow: 0 1px 14px rgb(0 0 0 / 0.55);
  }
  .desc {
    margin: 0;
    color: color-mix(in srgb, var(--text) 82%, transparent);
    font-size: 15.5px;
    max-width: 62ch;
    transition: opacity var(--t-slow), transform var(--t-slow);
  }
  .actions {
    display: flex;
    gap: 10px;
  }
  .dots {
    position: absolute;
    right: var(--gutter);
    bottom: 80px;
    display: flex;
    gap: 6px;
    padding: 8px 10px;
    border-radius: var(--r-pill);
  }
  .dot {
    position: relative;
    width: 7px;
    height: 7px;
    border-radius: 7px;
    background: rgb(255 255 255 / 0.4);
    overflow: hidden;
    transition: width var(--t-slow) var(--spring), background var(--t-fast);
  }
  .dot.on {
    width: 30px;
    background: rgb(255 255 255 / 0.3);
  }
  .fill {
    position: absolute;
    inset: 0;
    background: #fff;
    transform-origin: left;
    transform: scaleX(1);
  }
  .fill.run {
    animation: fill 9s linear both;
  }
  @keyframes fill {
    from {
      transform: scaleX(0);
    }
  }
  .empty .stage {
    opacity: 0.4;
  }
  .empty.phone {
    height: calc(var(--safe-top) + 600px);
  }

  /* ------------------------------------------------------------ phone */
  .hero-phone {
    position: relative;
    padding-top: calc(var(--safe-top) + 58px);
  }
  .glow {
    position: absolute;
    inset: -40px -20px 10%;
    background-size: cover;
    background-position: center;
    filter: blur(60px) saturate(1.6);
    opacity: calc(var(--ambient-strength) * 0.7);
    transition: background-image 0.8s ease;
    -webkit-mask-image: linear-gradient(180deg, #000 30%, transparent);
    mask-image: linear-gradient(180deg, #000 30%, transparent);
    pointer-events: none;
  }
  .track {
    position: relative;
    display: flex;
    gap: 12px;
    padding: 0 8%;
    scroll-snap-type: x mandatory;
  }
  .card {
    position: relative;
    flex: none;
    width: 84%;
    aspect-ratio: 2 / 2.9;
    max-height: 64vh;
    border-radius: 26px;
    overflow: hidden;
    scroll-snap-align: center;
    box-shadow:
      inset 0 0 0 0.5px rgb(255 255 255 / 0.16),
      0 24px 60px -24px rgb(0 0 0 / 0.75);
    transform: scale(0.94);
    opacity: 0.7;
    transition:
      transform var(--t-slow) var(--spring),
      opacity var(--t-med) ease;
  }
  .card.on {
    transform: none;
    opacity: 1;
  }
  .art {
    position: absolute;
    inset: 0;
  }
  .art img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .card-shade {
    position: absolute;
    inset: 35% 0 0;
    background: linear-gradient(180deg, transparent, rgb(0 0 0 / 0.62) 42%, rgb(0 0 0 / 0.9));
    pointer-events: none;
  }
  .card-info {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    padding: 16px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    text-align: center;
    color: #fff;
  }
  .card-logo {
    max-width: 78%;
    max-height: 84px;
    object-fit: contain;
    filter: drop-shadow(0 2px 12px rgb(0 0 0 / 0.5));
  }
  .facts-line {
    margin: 0;
    color: rgb(255 255 255 / 0.88);
  }
  .card-actions {
    display: flex;
    gap: 10px;
    width: 100%;
    margin-top: 4px;
  }
  .grow {
    flex: 1;
  }
  .card-actions .glass {
    color: #fff;
    background: rgb(255 255 255 / 0.16);
  }
  .card-actions .primary {
    background: #fff;
    color: #000;
  }
  .pdots {
    display: flex;
    justify-content: center;
    gap: 6px;
    margin-top: 14px;
  }
  .pdots span {
    width: 6px;
    height: 6px;
    border-radius: 3px;
    background: var(--fill-1);
    transition: width var(--t-med) var(--spring), background var(--t-med);
  }
  .pdots span.on {
    width: 18px;
    background: var(--label);
  }
</style>
