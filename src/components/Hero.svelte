<script lang="ts">
  import { api, img, type Meta } from "../lib/api";
  import { app } from "../lib/stores/app.svelte";
  import { setAmbient } from "../lib/stores/ambient.svelte";
  import { inLibrary, toggleLibrary } from "../lib/stores/library.svelte";
  import { openDetail } from "../lib/open";
  import { year, runtime } from "../lib/format";
  import YouTube from "./YouTube.svelte";
  import Icon from "./Icon.svelte";

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
    if (bg && scrollY < 300) setAmbient(bg);
  });

  function schedule() {
    clearTimeout(rotate);
    clearTimeout(dwell);
    trailer = null;
    trailerOn = false;
    if (!slides.length) return;
    rotate = setTimeout(() => !paused && !trailerOn && next(), 9000);
    // After sitting at the top for a moment, the slide's trailer takes over.
    if (app.settings.general.heroTrailers && !app.playing) {
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
  const lift = $derived(Math.min(1, scrollY / 520));
</script>

{#if cur}
  <section
    class="hero"
    bind:this={el}
    style:--lift={lift}
    onmouseenter={() => (paused = true)}
    onmouseleave={() => (paused = false)}
    aria-roledescription="carousel"
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
            <h1 class="large-title">{cur.name}</h1>
          {/if}
          <div class="facts">
            <span class="chip">{cur.type === "series" ? "Series" : "Film"}</span>
            {#if year(cur)}<span class="chip outline">{year(cur)}</span>{/if}
            {#if cur.imdbRating}<span class="chip outline"><Icon name="star" size={11} fill /> {cur.imdbRating}</span>{/if}
            {#if cur.runtime}<span class="chip outline">{runtime(cur.runtime)}</span>{/if}
            {#if cur.certification}<span class="chip outline">{cur.certification}</span>{/if}
            {#if cur.genres?.length}<span class="genres foot">{cur.genres.slice(0, 3).join(", ")}</span>{/if}
          </div>
          {#if cur.description}<p class="desc clamp-3">{cur.description}</p>{/if}
        </div>
      {/key}
      <div class="actions">
        <button class="btn primary press" onclick={() => openDetail(cur, el)}><Icon name="play" size={17} /> Watch now</button>
        <button class="btn tinted glass press" onclick={() => toggleLibrary(cur)}>
          <Icon name={owned ? "check" : "plus"} size={18} />
          {owned ? "In library" : "Add to library"}
        </button>
        {#if trailer}
          <button class="btn tinted icon glass press" onclick={() => (muted = !muted)} aria-label={muted ? "Unmute" : "Mute"}>
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
  <section class="hero empty">
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
      linear-gradient(0deg, color-mix(in srgb, var(--ink) 55%, transparent) 0%, transparent 45%),
      linear-gradient(90deg, color-mix(in srgb, var(--ink) 78%, transparent) 0%, transparent 52%);
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
    gap: 6px;
    transition: opacity var(--t-slow), transform var(--t-slow);
  }
  .genres {
    color: var(--text-dim);
    margin-left: 4px;
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
  @media (max-width: 760px) {
    .hero {
      height: 72vh;
      min-height: 420px;
    }
    .info {
      width: auto;
      right: var(--gutter);
      bottom: 40px;
    }
    .desc,
    .dots {
      display: none;
    }
  }
</style>
