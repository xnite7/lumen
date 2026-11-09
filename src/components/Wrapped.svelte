<script lang="ts">
  // couch Wrapped: your year as a story. Tap (or →) for the next card, ← or
  // the left edge to go back, hold to pause, Esc to leave. Every number comes
  // from your Simkl history (or what couch saw you watch).
  import { api, img, type WrapRow } from "../lib/api";
  import { wrapped } from "../lib/stores/wrapped.svelte";
  import { social, loadMe } from "../lib/stores/social.svelte";
  import { tasteMatch } from "../lib/taste";
  import Icon from "./Icon.svelte";
  import boil from "../assets/brand/couch-boil.webp";

  let rows = $state<WrapRow[] | null>(null);
  let twin = $state<{ name: string; avatar?: string; pct: number } | null>(null);
  let i = $state(0);
  let paused = $state(false);
  const SLIDE_MS = 6500;

  $effect(() => {
    if (!wrapped.open) return;
    i = 0;
    rows = null;
    twin = null;
    api.wrappedData(wrapped.year).then((r) => (rows = r ?? [])).catch(() => (rows = []));
    findTwin();
  });

  async function findTwin() {
    const me = await loadMe();
    if (!me) return;
    const mine = await api.profile(me.id).catch(() => null);
    const ppl = (await api.people().catch(() => ({ people: [] }))).people.filter((p) => p.id !== me.id).slice(0, 20);
    let best: typeof twin = null;
    for (const p of ppl) {
      const r = await api.profile(p.id).catch(() => null);
      const m = tasteMatch(mine?.profile?.taste, r?.profile?.taste);
      if (m && (!best || m.pct > best.pct)) best = { name: p.name, avatar: p.avatar, pct: m.pct };
    }
    twin = best;
  }

  // ------------------------------------------------------------- the year
  const S = $derived.by(() => {
    const r = rows ?? [];
    const films = r.filter((x) => x.type === "movie" && (x.status === "completed" || (x.watchedMinutes ?? 0) > 60));
    const shows = r.filter((x) => x.type === "series");
    const episodes = shows.reduce((n, x) => n + (x.eps ?? 0), 0);
    const minutes = r.reduce((n, x) => {
      if (x.watchedMinutes) return n + x.watchedMinutes;
      if (x.type === "movie") return n + (x.status === "completed" ? (x.minutes ?? 110) : 0);
      return n + (x.eps ?? 0) * (x.minutes ?? 42);
    }, 0);
    const g = new Map<string, number>();
    for (const x of r) for (const name of x.genres ?? []) g.set(name, (g.get(name) ?? 0) + 1);
    const genres = [...g.entries()].sort((a, b) => b[1] - a[1]).slice(0, 5);
    const months = Array(12).fill(0) as number[];
    const hours = Array(24).fill(0) as number[];
    for (const x of r) {
      const d = new Date(x.at);
      if (isNaN(+d)) continue;
      months[d.getMonth()]++;
      hours[d.getHours()]++;
    }
    const topMonth = months.indexOf(Math.max(...months));
    const slot = (a: number, b: number) => hours.slice(a, b).reduce((n, v) => n + v, 0);
    const parts = { night: slot(0, 6), morning: slot(6, 12), afternoon: slot(12, 18), evening: slot(18, 24) };
    const when = (Object.entries(parts) as [keyof typeof parts, number][]).sort((a, b) => b[1] - a[1])[0]?.[0] ?? "evening";
    const tens = r.filter((x) => (x.userRating ?? 0) >= 9).slice(0, 12);
    const binge = shows.slice().sort((a, b) => (b.eps ?? 0) - (a.eps ?? 0))[0];
    const byDate = r.slice().sort((a, b) => a.at.localeCompare(b.at));
    return { total: r.length, films: films.length, shows: shows.length, episodes, hours: Math.round(minutes / 60), genres, months, topMonth, parts, when, tens, binge, first: byDate[0], last: byDate[byDate.length - 1] };
  });

  const MONTHS = ["january", "february", "march", "april", "may", "june", "july", "august", "september", "october", "november", "december"];
  const WHEN = {
    night: { t: "a midnite creature", s: "most of your watching happened after midnight. the couch knows." },
    evening: { t: "prime-time regular", s: "evenings were yours. dinner, then the couch, every time." },
    afternoon: { t: "matinee person", s: "afternoons on the couch. respectfully, what's your job." },
    morning: { t: "early riser", s: "you watch in the morning. a rare and powerful creature." },
  };

  // which cards this year has (a quiet year skips the empty ones)
  const slides = $derived.by(() => {
    if (!rows) return [];
    if (!rows.length) return ["empty"];
    const l = ["intro", "hours", "count"];
    if (S.genres.length) l.push("genres");
    if (S.months.some((m) => m > 0)) l.push("months");
    l.push("when");
    if (S.binge && (S.binge.eps ?? 0) > 2) l.push("binge");
    if (S.tens.length) l.push("tens");
    if (twin) l.push("twin");
    l.push("outro");
    return l;
  });
  const slide = $derived(slides[i] ?? "intro");

  // auto-advance
  let t0 = $state(0);
  let held = 0;
  $effect(() => {
    void i;
    if (!wrapped.open || !slides.length) return;
    t0 = performance.now();
    held = 0;
  });
  let progress = $state(0);
  $effect(() => {
    if (!wrapped.open) return;
    let raf = 0;
    let last = performance.now();
    const tick = (now: number) => {
      if (paused) held += now - last;
      last = now;
      progress = Math.min(1, (now - t0 - held) / SLIDE_MS);
      if (progress >= 1 && slide !== "outro") next();
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  });

  function next() {
    if (i < slides.length - 1) i++;
  }
  function prev() {
    if (i > 0) i--;
    else t0 = performance.now();
  }
  function close() {
    wrapped.open = false;
  }
  function tap(e: MouseEvent) {
    if ((e.target as Element).closest("button")) return;
    const x = e.clientX / window.innerWidth;
    x < 0.3 ? prev() : next();
  }
  function key(e: KeyboardEvent) {
    if (!wrapped.open) return;
    if (e.key === "Escape") close();
    else if (e.key === "ArrowRight" || e.key === " ") next();
    else if (e.key === "ArrowLeft") prev();
  }

  // numbers that count up
  function count(node: HTMLElement, to: number) {
    let raf = 0;
    const run = (target: number) => {
      cancelAnimationFrame(raf);
      const start = performance.now();
      const dur = 1400;
      const step = (now: number) => {
        const k = Math.min(1, (now - start) / dur);
        const e = 1 - Math.pow(1 - k, 4);
        node.textContent = Math.round(target * e).toLocaleString();
        if (k < 1) raf = requestAnimationFrame(step);
      };
      raf = requestAnimationFrame(step);
    };
    run(to);
    return { update: run, destroy: () => cancelAnimationFrame(raf) };
  }
  const maxMonth = $derived(Math.max(1, ...S.months));
  const maxGenre = $derived(Math.max(1, ...S.genres.map((g) => g[1])));
  const bg = (r?: WrapRow) => (r ? img(r.background ?? r.poster, "w1280") : "");
</script>

<svelte:window onkeydown={key} />

{#if wrapped.open}
  <div
    class="wrapped s-{slide}"
    role="dialog"
    aria-label="Your {wrapped.year} on the couch"
    tabindex="-1"
    onclick={tap}
    onpointerdown={() => (paused = true)}
    onpointerup={() => (paused = false)}
    onpointerleave={() => (paused = false)}
  >
    <div class="grain" aria-hidden="true"></div>
    <div class="bars">
      {#each slides as _, n}<span><i style:width="{n < i ? 100 : n === i ? progress * 100 : 0}%"></i></span>{/each}
    </div>
    <button class="x press" onclick={close} aria-label="Close"><Icon name="close" size={20} /></button>

    {#if !rows}
      <div class="card"><img class="mascot" src={boil} alt="" /><p class="sub">rewinding your year…</p></div>
    {:else}
      {#key i}
        <div class="card">
          {#if slide === "empty"}
            <img class="mascot" src={boil} alt="" />
            <h1 class="big">quiet year</h1>
            <p class="sub">nothing logged for {wrapped.year} yet. watch something and come back.</p>
          {:else if slide === "intro"}
            <span class="kick">midnite™ couch</span>
            <h1 class="year">{wrapped.year}</h1>
            <p class="sub">your year on the couch, rewound.</p>
            <img class="mascot float" src={boil} alt="" />
          {:else if slide === "hours"}
            <span class="kick">you spent about</span>
            <h1 class="num"><span use:count={S.hours}>0</span></h1>
            <p class="unit">hours on the couch</p>
            <p class="sub">that's {Math.round((S.hours / 24) * 10) / 10} days. {S.hours > 300 ? "the cushions have your shape now." : S.hours > 100 ? "solid. very solid." : "a gentle year."}</p>
          {:else if slide === "count"}
            <span class="kick">this year you watched</span>
            <div class="trio">
              <div><b use:count={S.films}>0</b><span>films</span></div>
              <div><b use:count={S.shows}>0</b><span>shows</span></div>
              <div><b use:count={S.episodes}>0</b><span>episodes</span></div>
            </div>
            {#if S.first}<p class="sub">it started with <b>{S.first.name}</b>{S.last && S.last.id !== S.first.id ? ` and (so far) ends with ${S.last.name}` : ""}.</p>{/if}
          {:else if slide === "genres"}
            <span class="kick">your genres</span>
            <h1 class="big">{S.genres[0][0].toLowerCase()}</h1>
            <div class="gbars">
              {#each S.genres as [g, n], k}
                <div class="gb" style:--w="{(n / maxGenre) * 100}%" style:--d="{k * 120}ms"><span class="gl">{g}</span><i></i><span class="gn">{n}</span></div>
              {/each}
            </div>
          {:else if slide === "months"}
            <span class="kick">your busiest month</span>
            <h1 class="big">{MONTHS[S.topMonth]}</h1>
            <div class="months">
              {#each S.months as m, k}
                <div class="mo" class:top={k === S.topMonth} style:--h="{(m / maxMonth) * 100}%" style:--d="{k * 60}ms"><i></i><span>{MONTHS[k][0]}</span></div>
              {/each}
            </div>
          {:else if slide === "when"}
            <span class="kick">you are</span>
            <h1 class="big">{WHEN[S.when].t}</h1>
            <div class="clock">
              {#each Object.entries(S.parts) as [k, v]}
                <div class="part" class:on={k === S.when}><b>{Math.round((v / Math.max(1, S.total)) * 100)}%</b><span>{k}</span></div>
              {/each}
            </div>
            <p class="sub">{WHEN[S.when].s}</p>
          {:else if slide === "binge" && S.binge}
            <div class="art" style:background-image="url('{bg(S.binge)}')"></div>
            <span class="kick">most binged</span>
            <h1 class="big">{S.binge.name}</h1>
            <p class="unit"><b use:count={S.binge.eps ?? 0}>0</b> episodes</p>
          {:else if slide === "tens"}
            <span class="kick">the ones you scored highest</span>
            <div class="wall">
              {#each S.tens as t, k}
                <div class="pw" style:--d="{k * 90}ms" style:--r="{((k * 37) % 11) - 5}deg">{#if t.poster}<img src={img(t.poster, "w342")} alt={t.name} />{/if}<span>{t.userRating}/10</span></div>
              {/each}
            </div>
          {:else if slide === "twin" && twin}
            <span class="kick">your couch twin</span>
            <div class="twin">{#if twin.avatar}<img src={twin.avatar} alt="" />{:else}<b>{twin.name.charAt(0)}</b>{/if}</div>
            <h1 class="big">{twin.name}</h1>
            <p class="unit"><b use:count={twin.pct}>0</b>% taste match</p>
          {:else if slide === "outro"}
            <span class="kick">{wrapped.year} · wrapped</span>
            <div class="summary">
              <div><b>{S.hours.toLocaleString()}</b><span>hours</span></div>
              <div><b>{S.films}</b><span>films</span></div>
              <div><b>{S.shows}</b><span>shows</span></div>
              <div><b>{S.episodes.toLocaleString()}</b><span>episodes</span></div>
              {#if S.genres[0]}<div class="wide"><b>{S.genres[0][0]}</b><span>top genre</span></div>{/if}
              <div class="wide"><b>{WHEN[S.when].t}</b><span>you are</span></div>
              {#if twin}<div class="wide"><b>{twin.name} · {twin.pct}%</b><span>couch twin</span></div>{/if}
            </div>
            <button class="btn primary press again" onclick={() => (i = 0)}><Icon name="refresh" size={16} /> watch again</button>
          {/if}
        </div>
      {/key}
    {/if}
  </div>
{/if}

<style>
  .wrapped {
    position: fixed;
    inset: 0;
    z-index: 600;
    display: grid;
    place-items: center;
    overflow: hidden;
    color: #fff;
    cursor: pointer;
    user-select: none;
    background: #0b0610;
    transition: background 900ms ease;
    animation: w-in calc(500ms * var(--motion, 1)) cubic-bezier(0.16, 1, 0.3, 1) both;
  }
  /* each card its own light */
  .wrapped::before {
    content: "";
    position: absolute;
    inset: -20%;
    background:
      radial-gradient(40% 50% at var(--x1, 20%) var(--y1, 20%), var(--c1, rgb(224 48 154 / 0.75)), transparent 70%),
      radial-gradient(45% 55% at var(--x2, 85%) var(--y2, 80%), var(--c2, rgb(255 178 92 / 0.6)), transparent 70%);
    filter: blur(30px);
    animation: drift 14s ease-in-out infinite alternate;
    transition: background 900ms ease;
  }
  .s-hours {
    --c1: rgb(255 178 92 / 0.8);
    --c2: rgb(224 48 154 / 0.55);
    --x1: 80%;
  }
  .s-count {
    --c1: rgb(76 201 240 / 0.6);
    --c2: rgb(224 48 154 / 0.6);
  }
  .s-genres {
    --c1: rgb(126 224 129 / 0.55);
    --c2: rgb(76 201 240 / 0.5);
    --y1: 80%;
  }
  .s-months {
    --c1: rgb(155 107 255 / 0.65);
    --c2: rgb(255 110 196 / 0.5);
  }
  .s-when {
    --c1: rgb(40 30 120 / 0.9);
    --c2: rgb(224 48 154 / 0.45);
  }
  .s-tens {
    --c1: rgb(255 216 74 / 0.5);
    --c2: rgb(255 77 77 / 0.5);
  }
  .s-twin {
    --c1: rgb(45 212 191 / 0.55);
    --c2: rgb(255 178 92 / 0.55);
  }
  .grain {
    position: absolute;
    inset: 0;
    pointer-events: none;
    opacity: 0.22;
    mix-blend-mode: overlay;
    background-image: url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='180' height='180'><filter id='n'><feTurbulence type='fractalNoise' baseFrequency='.9' numOctaves='3' stitchTiles='stitch'/><feColorMatrix values='0 0 0 0 1  0 0 0 0 1  0 0 0 0 1  0 0 0 .9 0'/></filter><rect width='100%' height='100%' filter='url(%23n)'/></svg>");
  }
  .bars {
    position: absolute;
    top: calc(var(--safe-top, 0px) + 18px);
    left: 50%;
    translate: -50% 0;
    width: min(560px, calc(100% - 40px));
    display: flex;
    gap: 5px;
    z-index: 2;
  }
  .bars span {
    flex: 1;
    height: 3px;
    border-radius: 2px;
    background: rgb(255 255 255 / 0.25);
    overflow: hidden;
  }
  .bars i {
    display: block;
    height: 100%;
    background: #fff;
  }
  .x {
    position: absolute;
    top: calc(var(--safe-top, 0px) + 34px);
    right: 22px;
    z-index: 3;
    width: 40px;
    height: 40px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: rgb(0 0 0 / 0.3);
  }
  .card {
    position: relative;
    z-index: 1;
    width: min(560px, calc(100% - 48px));
    display: grid;
    justify-items: start;
    gap: 10px;
  }
  .card > * {
    animation: up calc(700ms * var(--motion, 1)) cubic-bezier(0.16, 1, 0.3, 1) both;
  }
  .card > *:nth-child(2) {
    animation-delay: 120ms;
  }
  .card > *:nth-child(3) {
    animation-delay: 260ms;
  }
  .card > *:nth-child(4) {
    animation-delay: 420ms;
  }
  .card > *:nth-child(5) {
    animation-delay: 560ms;
  }
  .kick {
    font-size: 15px;
    font-weight: 700;
    opacity: 0.8;
  }
  .year {
    margin: 0;
    font: 900 clamp(110px, 24vw, 220px) / 0.85 var(--font-display);
    letter-spacing: -0.07em;
  }
  .big {
    margin: 0;
    font: 900 clamp(46px, 9vw, 86px) / 0.95 var(--font-display);
    letter-spacing: -0.05em;
    overflow-wrap: anywhere;
  }
  .num {
    margin: 0;
    font: 900 clamp(110px, 26vw, 210px) / 0.85 var(--font-display);
    letter-spacing: -0.07em;
    font-variant-numeric: tabular-nums;
  }
  .unit {
    margin: 0;
    font: 800 24px var(--font-display);
    letter-spacing: -0.02em;
  }
  .sub {
    margin: 8px 0 0;
    font-size: 16px;
    line-height: 1.45;
    opacity: 0.85;
    max-width: 40ch;
  }
  .mascot {
    width: min(220px, 50vw);
    justify-self: center;
    margin-top: 10px;
  }
  .float {
    animation: up 700ms 400ms both, bob 3.6s 1.1s ease-in-out infinite;
  }

  .trio {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 14px;
    width: 100%;
    margin: 6px 0;
  }
  .trio div,
  .summary div {
    display: grid;
  }
  .trio b {
    font: 900 clamp(48px, 10vw, 78px) / 1 var(--font-display);
    letter-spacing: -0.05em;
  }
  .trio span,
  .summary span {
    font-size: 14px;
    opacity: 0.75;
  }

  .gbars {
    display: grid;
    gap: 10px;
    width: 100%;
    margin-top: 10px;
  }
  .gb {
    display: grid;
    grid-template-columns: 110px 1fr 36px;
    align-items: center;
    gap: 12px;
    font-weight: 700;
  }
  .gb i {
    height: 14px;
    border-radius: 7px;
    background: #fff;
    width: var(--w);
    animation: grow-w 900ms var(--d) cubic-bezier(0.16, 1, 0.3, 1) both;
  }
  .gn {
    text-align: right;
    opacity: 0.8;
  }

  .months {
    display: grid;
    grid-template-columns: repeat(12, 1fr);
    gap: 6px;
    width: 100%;
    height: 180px;
    align-items: end;
    margin-top: 14px;
  }
  .mo {
    display: grid;
    grid-template-rows: 1fr auto;
    height: 100%;
    gap: 6px;
    text-align: center;
    font-size: 12px;
    opacity: 0.85;
  }
  .mo i {
    align-self: end;
    height: max(4px, var(--h));
    border-radius: 6px;
    background: rgb(255 255 255 / 0.45);
    animation: grow-h 900ms var(--d) cubic-bezier(0.16, 1, 0.3, 1) both;
    transform-origin: bottom;
  }
  .mo.top i {
    background: #fff;
    box-shadow: 0 0 24px rgb(255 255 255 / 0.5);
  }

  .clock {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 10px;
    width: 100%;
    margin: 10px 0 4px;
  }
  .part {
    display: grid;
    gap: 2px;
    padding: 14px 12px;
    border-radius: 16px;
    background: rgb(255 255 255 / 0.08);
    opacity: 0.6;
  }
  .part.on {
    background: #fff;
    color: #0b0610;
    opacity: 1;
  }
  .part b {
    font: 900 26px var(--font-display);
  }
  .part span {
    font-size: 13px;
  }

  .art {
    position: fixed;
    inset: 0;
    z-index: -1;
    background-size: cover;
    background-position: center;
    opacity: 0.45;
    mask-image: linear-gradient(180deg, #000 30%, transparent 95%);
    animation: zoom 9s ease-out both !important;
  }

  .wall {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 12px;
    width: 100%;
    margin-top: 8px;
  }
  .pw {
    position: relative;
    aspect-ratio: 2 / 3;
    border-radius: 10px;
    overflow: hidden;
    background: rgb(255 255 255 / 0.08);
    rotate: var(--r);
    box-shadow: 0 16px 40px -14px #000;
    animation: drop 700ms var(--d) cubic-bezier(0.34, 1.56, 0.64, 1) both;
  }
  .pw img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .pw span {
    position: absolute;
    left: 6px;
    bottom: 6px;
    padding: 2px 7px;
    border-radius: 8px;
    background: #fff;
    color: #000;
    font-size: 12px;
    font-weight: 800;
  }

  .twin {
    width: 150px;
    height: 150px;
    border-radius: 50%;
    overflow: hidden;
    display: grid;
    place-items: center;
    background: rgb(255 255 255 / 0.12);
    box-shadow: 0 0 0 4px #fff, 0 0 60px rgb(255 255 255 / 0.35);
    font: 900 60px var(--font-display);
    margin: 6px 0;
  }
  .twin img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .summary {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 16px 20px;
    width: 100%;
    padding: 22px;
    border-radius: 22px;
    background: rgb(0 0 0 / 0.35);
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.12);
  }
  .summary b {
    font: 900 30px var(--font-display);
    letter-spacing: -0.03em;
  }
  .summary .wide {
    grid-column: span 2;
  }
  .again {
    margin-top: 8px;
    display: inline-flex;
    gap: 8px;
    align-items: center;
  }

  @keyframes w-in {
    from {
      opacity: 0;
      scale: 1.04;
    }
  }
  @keyframes up {
    from {
      opacity: 0;
      transform: translateY(26px);
      filter: blur(6px);
    }
  }
  @keyframes bob {
    50% {
      transform: translateY(-10px) rotate(-2deg);
    }
  }
  @keyframes drift {
    to {
      transform: translate(4%, -3%) scale(1.08);
    }
  }
  @keyframes grow-w {
    from {
      width: 0;
    }
  }
  @keyframes grow-h {
    from {
      transform: scaleY(0);
    }
  }
  @keyframes drop {
    from {
      opacity: 0;
      transform: translateY(-60px) scale(0.8);
    }
  }
  @keyframes zoom {
    from {
      transform: scale(1.15);
    }
  }
  @media (max-width: 600px) {
    .wall {
      grid-template-columns: repeat(3, 1fr);
    }
    .gb {
      grid-template-columns: 86px 1fr 30px;
    }
    .clock {
      grid-template-columns: repeat(2, 1fr);
    }
  }
</style>
