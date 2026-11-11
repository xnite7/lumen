<script lang="ts">
  // The release calendar: what's coming, day by day. Next episodes of the
  // shows you're watching, films on your list, new things from who you
  // follow, and the big releases everyone's waiting for (a switch turns
  // those off). A strip of days up top jumps to any of them.
  import { api, img, type CalItem } from "../lib/api";
  import { app, saveSettings } from "../lib/stores/app.svelte";
  import { go } from "../lib/router.svelte";
  import { live } from "../lib/stores/live.svelte";
  import { fallback, imdbArt } from "../lib/fallback";
  import Controls from "../components/Controls.svelte";
  import Icon from "../components/Icon.svelte";
  import boil from "../assets/brand/couch-boil.webp";

  const s = app.settings;
  let items = $state<CalItem[] | null>(null);
  let only = $state<"all" | "yours">("all");
  let seq = 0;

  $effect(() => {
    void live.tick;
    const hits = s.general.calendarHits !== false;
    const my = ++seq;
    api
      .calendar(hits)
      .then((r) => my === seq && (items = r ?? []))
      .catch(() => my === seq && (items = items ?? []));
  });

  const iso = (d: Date) => `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
  const today = iso(new Date());
  const shown = $derived((items ?? []).filter((i) => i.date >= today && (only === "all" || i.source !== "hit")));
  // just out: the last three days
  const recent = $derived((items ?? []).filter((i) => i.date < today && (only === "all" || i.source !== "hit")).reverse());

  const days = $derived.by(() => {
    const m = new Map<string, CalItem[]>();
    for (const i of shown) (m.get(i.date) ?? m.set(i.date, []).get(i.date)!).push(i);
    return [...m.entries()];
  });

  function dayName(d: string) {
    const date = new Date(d + "T12:00:00");
    const diff = Math.round((+new Date(d + "T00:00:00") - +new Date(today + "T00:00:00")) / 864e5);
    if (diff === 0) return "today";
    if (diff === 1) return "tomorrow";
    if (diff < 7) return date.toLocaleDateString(undefined, { weekday: "long" }).toLowerCase();
    return date.toLocaleDateString(undefined, { weekday: "short", day: "numeric", month: "short" }).toLowerCase();
  }
  const dayNum = (d: string) => new Date(d + "T12:00:00").getDate();
  const dayShort = (d: string) => new Date(d + "T12:00:00").toLocaleDateString(undefined, { weekday: "short" }).toLowerCase();

  // the strip: the next 21 days, a dot on the ones with something
  const strip = $derived.by(() => {
    const has = new Map(days.map(([d, l]) => [d, l]));
    return Array.from({ length: 21 }, (_, k) => {
      const d = new Date();
      d.setDate(d.getDate() + k);
      const key = iso(d);
      const l = has.get(key) ?? [];
      return { key, n: l.length, mine: l.some((i) => i.source !== "hit") };
    });
  });
  function jump(key: string) {
    document.getElementById(`cal-${key}`)?.scrollIntoView({ behavior: "smooth", block: "start" });
  }

  const open = (i: CalItem) => go({ name: "detail", type: i.type, id: i.imdb ?? i.id });
  const art = (i: CalItem) => i.still ?? img(i.background ?? i.poster, "w780");
  const TAG = { yours: "yours", follow: "following", hit: "hit" } as const;
</script>

<div class="cal">
  <header class="top">
    <h1 class="large-title">calendar</h1>
    <div class="tools">
      <Controls kind="segmented" bind:value={only} options={[{ value: "all", label: "Everything" }, { value: "yours", label: "Just mine" }]} label="Show" />
      <label class="hits">
        <span>big releases</span>
        <Controls kind="switch" bind:value={s.general.calendarHits} onchange={saveSettings} />
      </label>
    </div>
  </header>

  <div class="strip scroll-x">
    {#each strip as d (d.key)}
      <button class="d press" class:has={d.n > 0} class:today={d.key === today} disabled={!d.n} onclick={() => jump(d.key)}>
        <span class="wd">{dayShort(d.key)}</span>
        <b>{dayNum(d.key)}</b>
        <i class:mine={d.mine}></i>
      </button>
    {/each}
  </div>

  {#if !items}
    <div class="list">{#each Array(6) as _}<div class="ghost skeleton"></div>{/each}</div>
  {:else if !days.length && !recent.length}
    <div class="empty">
      <img src={boil} alt="" />
      <p>nothing on the calendar yet.</p>
      <p class="foot dim">add shows to your list or follow a studio and their releases land here.</p>
    </div>
  {:else}
    {#each days as [d, list] (d)}
      <section class="day" id="cal-{d}">
        <h2 class="dh" class:now={d === today}>{dayName(d)}<span class="dd">{new Date(d + "T12:00:00").toLocaleDateString(undefined, { day: "numeric", month: "long" })}</span></h2>
        <div class="list">
          {#each list as i (i.id + i.label)}
            <button class="card press" class:hit={i.source === "hit"} onclick={() => open(i)}>
              <div class="pic">
                {#if art(i)}<img src={art(i)} alt="" loading="lazy" use:fallback={[imdbArt(i.imdb, "background"), i.poster ? img(i.poster, "w342") : null]} />{/if}
                <span class="tag {i.source}">{#if i.source === "yours"}<Icon name="check" size={11} />{/if}{TAG[i.source]}</span>
              </div>
              <div class="txt">
                <span class="nm">{i.name}</span>
                <span class="lb">{i.label}{#if i.episode && !/^Episode \d+$/.test(i.episode)}<span class="ep"> · {i.episode}</span>{/if}</span>
              </div>
            </button>
          {/each}
        </div>
      </section>
    {/each}

    {#if recent.length}
      <section class="day">
        <h2 class="dh">just out</h2>
        <div class="list">
          {#each recent as i (i.id + i.label)}
            <button class="card press" class:hit={i.source === "hit"} onclick={() => open(i)}>
              <div class="pic">
                {#if art(i)}<img src={art(i)} alt="" loading="lazy" use:fallback={[imdbArt(i.imdb, "background"), i.poster ? img(i.poster, "w342") : null]} />{/if}
                <span class="tag {i.source}">{TAG[i.source]}</span>
              </div>
              <div class="txt">
                <span class="nm">{i.name}</span>
                <span class="lb">{i.label} · {dayName(i.date) === i.date ? i.date : new Date(i.date + "T12:00:00").toLocaleDateString(undefined, { weekday: "short", day: "numeric", month: "short" }).toLowerCase()}</span>
              </div>
            </button>
          {/each}
        </div>
      </section>
    {/if}
  {/if}
</div>

<style>
  .cal {
    padding: calc(var(--safe-top) + var(--bar-h) + 26px) 0 calc(var(--safe-bottom) + 120px);
  }
  .top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 12px 20px;
    padding: 0 var(--gutter);
    margin-bottom: 18px;
  }
  .large-title {
    margin: 0;
  }
  .tools {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 12px 18px;
  }
  .hits {
    display: inline-flex;
    align-items: center;
    gap: 10px;
    font-size: 14px;
    color: var(--label-2);
  }

  .strip {
    display: flex;
    gap: 8px;
    padding: 4px var(--gutter) 12px;
    margin-bottom: 8px;
  }
  .d {
    flex: none;
    width: 58px;
    padding: 9px 0 8px;
    border-radius: 16px;
    display: grid;
    justify-items: center;
    gap: 2px;
    background: rgb(255 255 255 / 0.04);
    color: var(--label-3);
  }
  .d.has {
    color: var(--label);
    background: rgb(255 255 255 / 0.08);
  }
  .d.today {
    box-shadow: inset 0 0 0 1.5px var(--lamp);
  }
  .d .wd {
    font-size: 11px;
    font-weight: 700;
    opacity: 0.75;
  }
  .d b {
    font: 800 20px var(--font-display);
    letter-spacing: -0.03em;
  }
  .d i {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: transparent;
  }
  .d.has i {
    background: rgb(255 255 255 / 0.45);
  }
  .d.has i.mine {
    background: var(--neon-soft);
  }

  .day {
    margin-top: 26px;
    scroll-margin-top: calc(var(--safe-top) + var(--bar-h) + 14px);
  }
  .dh {
    display: flex;
    align-items: baseline;
    gap: 12px;
    margin: 0 0 12px;
    padding: 0 var(--gutter);
    font: 800 24px var(--font-display);
    letter-spacing: -0.03em;
  }
  .dh.now {
    color: var(--lamp);
  }
  .dd {
    font: 500 13px var(--font-text);
    letter-spacing: 0;
    color: var(--label-3);
  }
  .list {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(250px, 1fr));
    gap: 16px 12px;
    padding: 0 var(--gutter);
  }
  .card {
    display: grid;
    gap: 8px;
    text-align: left;
  }
  .pic {
    position: relative;
    aspect-ratio: 16 / 9;
    border-radius: 14px;
    overflow: hidden;
    background: rgb(255 255 255 / 0.06);
    transition: transform 220ms var(--spring, ease);
  }
  .card:hover .pic {
    transform: scale(1.025);
  }
  .pic img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .tag {
    position: absolute;
    left: 8px;
    top: 8px;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 3px 9px;
    border-radius: 999px;
    font-size: 11px;
    font-weight: 800;
    background: rgb(0 0 0 / 0.6);
    backdrop-filter: blur(8px);
    color: #fff;
  }
  .tag.yours {
    background: var(--neon);
  }
  .tag.follow {
    background: #fff;
    color: #000;
  }
  .tag.hit {
    background: var(--lamp);
    color: #1c1000;
  }
  .txt {
    display: grid;
    gap: 1px;
    min-width: 0;
  }
  .nm {
    font-weight: 700;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .lb {
    font-size: 13px;
    color: var(--label-2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ep {
    color: var(--label-3);
  }
  .ghost {
    aspect-ratio: 16 / 9;
    border-radius: 14px;
  }
  .empty {
    display: grid;
    justify-items: center;
    gap: 6px;
    padding: 70px var(--gutter);
    text-align: center;
  }
  .empty img {
    width: min(200px, 50vw);
    opacity: 0.85;
    margin-bottom: 10px;
  }
  .empty p {
    margin: 0;
    max-width: 44ch;
  }
  @media (max-width: 759px) {
    .list {
      grid-template-columns: repeat(2, 1fr);
      gap: 14px 10px;
    }
    .dh {
      font-size: 21px;
    }
  }
</style>
