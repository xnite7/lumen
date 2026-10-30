<script lang="ts">
  // Streams, grouped by resolution and ranked the way you'd pick by hand:
  // instant debrid first, then quality, then the extras that make a 4K worth
  // it. Results appear as each addon answers; "Best" buttons are one tap.
  import { api, listen, uid, type Meta, type Stream, type Video } from "../lib/api";
  import { app } from "../lib/stores/app.svelte";
  import { play } from "../lib/stores/playback.svelte";
  import { bytes } from "../lib/format";
  import Sheet from "./Sheet.svelte";
  import Icon from "./Icon.svelte";
  import { haptic } from "../lib/native";

  let { open = $bindable(false), meta, video = null }: { open?: boolean; meta: Meta; video?: Video | null } = $props();

  let streams = $state<Stream[]>([]);
  let addons = $state<Record<string, { name: string; ms: number; error?: string; count: number }>>({});
  let finished = $state(false);
  let filter = $state<string>("all");
  let reqId = "";
  let collapsed = $state<Record<string, boolean>>({});

  const videoId = $derived(video?.id ?? meta.id);
  const tiers = ["4K", "1080p", "720p", "SD", "Unknown"];
  const tierLabel: Record<string, string> = { "4K": "4K", "1080p": "1080p", "720p": "720p", SD: "SD", Unknown: "Other" };

  $effect(() => {
    if (!open) return;
    const id = videoId;
    reqId = uid();
    streams = [];
    addons = {};
    finished = false;
    filter = "all";
    const my = reqId;
    let un: (() => void) | undefined;
    listen<any>("streams", (c) => {
      if (c.reqId !== my) return;
      if (c.finished) {
        finished = true;
        maybeAutoplay();
        return;
      }
      addons[c.addonId] = { name: c.addonName, ms: c.ms, error: c.error, count: c.streams.length };
      streams = [...streams, ...c.streams];
    }).then((u) => {
      un = u;
      api.streams(meta.type, id, my);
    });
    return () => un?.();
  });

  const s = $derived(app.settings.streams);

  const visible = $derived.by(() => {
    let list = streams.filter((x) => {
      const p = x._lumen;
      if (!p) return true;
      if (s.hideCam && p.cam) return false;
      if (s.hide3d && p.threeD) return false;
      if (s.maxSizeGb > 0 && p.sizeBytes && p.sizeBytes > s.maxSizeGb * 1e9) return false;
      if (filter !== "all" && x._addonId !== filter) return false;
      return true;
    });
    const by: Record<string, (a: Stream, b: Stream) => number> = {
      smart: (a, b) => (b._lumen?.score ?? 0) - (a._lumen?.score ?? 0),
      quality: (a, b) => (b._lumen?.resRank ?? 0) - (a._lumen?.resRank ?? 0) || (b._lumen?.sizeBytes ?? 0) - (a._lumen?.sizeBytes ?? 0),
      size: (a, b) => (b._lumen?.sizeBytes ?? 0) - (a._lumen?.sizeBytes ?? 0),
      seeders: (a, b) => (b._lumen?.seeders ?? 0) - (a._lumen?.seeders ?? 0),
      addon: () => 0,
    };
    return list.slice().sort(by[s.sort] ?? by.smart);
  });

  const groups = $derived.by(() => {
    if (!s.groupByQuality) return [{ tier: "", items: visible }];
    return tiers.map((t) => ({ tier: t, items: visible.filter((x) => (x._lumen?.res ?? "Unknown") === t) })).filter((g) => g.items.length);
  });

  const best = (res: string) => visible.filter((x) => x._lumen?.res === res).sort((a, b) => b._lumen.score - a._lumen.score)[0];

  function choose(st: Stream) {
    haptic("medium");
    open = false;
    const title = video ? video.title || video.name || `Episode ${video.episode ?? video.number}` : undefined;
    play(meta, st, videoId, { episodeTitle: title });
  }

  function maybeAutoplay() {
    if (!s.autoplayBest || !visible.length) return;
    choose(visible[0]);
  }

  function seedColor(n?: number) {
    if (n == null) return "";
    return n >= 200 ? "#4ade80" : n >= 50 ? "#facc15" : n >= 1 ? "#fb923c" : "#f87171";
  }
</script>

<Sheet bind:open width={820}>
  {#snippet header()}
    <header class="head">
      <div>
        <h2 class="title2">{video ? (video.title || video.name || `Episode ${video.episode}`) : meta.name}</h2>
        <p class="foot dim">
          {#if !finished}Asking {Object.keys(addons).length ? "more addons" : "your addons"}…{:else}{visible.length} streams from {Object.keys(addons).length} addons{/if}
        </p>
      </div>
      <div class="best">
        {#each ["4K", "1080p"] as r}
          {@const b = best(r)}
          <button class="btn small press" class:accent={!!b} class:tinted={!b} disabled={!b} onclick={() => b && choose(b)}>
            <Icon name="bolt" size={14} /> Best {r}
          </button>
        {/each}
      </div>
    </header>
    {#if Object.keys(addons).length > 1}
      <div class="filters scroll-x">
        <button class="chip press" class:sel={filter === "all"} onclick={() => (filter = "all")}>All</button>
        {#each Object.entries(addons) as [id, a]}
          <button class="chip press" class:sel={filter === id} class:bad={!!a.error} onclick={() => (filter = id)} title={a.error ?? `${a.ms} ms`}>
            {a.name} <span class="faint">{a.error ? "failed" : a.count}</span>
          </button>
        {/each}
      </div>
    {/if}
  {/snippet}

  {#if !streams.length && !finished}
    {#each Array(5) as _}<div class="ghost skeleton"></div>{/each}
  {:else if !visible.length}
    <div class="empty">
      <Icon name="film" size={34} />
      <p class="headline">No streams found</p>
      <p class="foot dim">None of your addons returned a playable stream for this. Add a stream addon in Addons, or loosen the filters in Settings → Streams.</p>
    </div>
  {:else}
    {#each groups as g (g.tier)}
      {#if g.tier}
        <button class="group-h" onclick={() => (collapsed[g.tier] = !collapsed[g.tier])}>
          <span class="tier t-{g.tier}">{tierLabel[g.tier]}</span>
          <span class="foot faint">{g.items.length}</span>
          <Icon name={collapsed[g.tier] ? "chevronDown" : "chevronUp"} size={16} />
        </button>
      {/if}
      {#if !collapsed[g.tier]}
        <div class="list">
          {#each g.items as st, i (i + (st.url ?? st.infoHash ?? "") + st._addonId)}
            {@const p = st._lumen}
            <button class="stream press" onclick={() => choose(st)}>
              <div class="l1">
                <span class="label">{p?.label ?? st.title ?? st.name}</span>
                {#if p?.seeders != null}
                  <span class="seed" style:color={seedColor(p.seeders)}><Icon name="person" size={12} /> {p.seeders}</span>
                {/if}
              </div>
              <div class="l2">
                {#if p?.cached}<span class="pill instant"><Icon name="bolt" size={11} /> Instant</span>{/if}
                {#if p?.cached === false}<span class="pill">Not cached</span>{/if}
                {#each p?.hdr ?? [] as h}<span class="pill hdr">{h}</span>{/each}
                {#if p?.source}<span class="pill">{p.source}</span>{/if}
                {#if p?.codec}<span class="pill">{p.codec}</span>{/if}
                {#each (p?.audio ?? []).slice(0, 2) as a}<span class="pill">{a}</span>{/each}
                {#if p?.sizeBytes}<span class="pill plain">{bytes(p.sizeBytes)}</span>{/if}
                {#if p?.kind === "torrent"}<span class="pill plain"><Icon name="magnet" size={11} /> P2P</span>{/if}
                {#each p?.languages ?? [] as f}<span class="flag">{f}</span>{/each}
                <span class="src faint">{st._addonName}{p?.debrid ? ` · ${p.debrid}` : ""}</span>
              </div>
            </button>
          {/each}
        </div>
      {/if}
    {/each}
  {/if}
</Sheet>

<style>
  .head {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 16px;
    padding: 2px 24px 12px;
  }
  .head p {
    margin: 4px 0 0;
  }
  .best {
    display: flex;
    gap: 8px;
  }
  .filters {
    display: flex;
    gap: 6px;
    padding: 0 24px 12px;
  }
  .filters .chip {
    flex: none;
  }
  .chip.bad {
    opacity: 0.6;
  }
  .ghost {
    height: 74px;
    border-radius: var(--r-md);
    margin-bottom: 8px;
  }
  .group-h {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 14px 4px 8px;
    color: var(--text-dim);
  }
  .group-h :global(svg) {
    margin-left: auto;
  }
  .tier {
    font-family: var(--font-display);
    font-weight: 700;
    font-size: 15px;
    color: var(--text);
  }
  .t-4K {
    color: #8ab8ff;
  }
  .t-1080p {
    color: #6fe3a0;
  }
  .t-720p {
    color: #f6d55c;
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .stream {
    text-align: left;
    padding: 12px 14px;
    border-radius: var(--r-md);
    background: var(--cell);
    box-shadow: inset 0 0 0 0.5px rgb(255 255 255 / 0.06);
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  @media (hover: hover) {
    .stream:hover {
      background: var(--cell-press);
    }
  }
  .stream:active {
    background: var(--cell-press);
  }
  .l1 {
    display: flex;
    gap: 12px;
    align-items: baseline;
  }
  .label {
    flex: 1;
    font-weight: 600;
    font-size: 14px;
    word-break: break-word;
  }
  .seed {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    font-size: 12.5px;
    font-weight: 600;
    flex: none;
  }
  .l2 {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 5px;
  }
  .pill {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    height: 21px;
    padding: 0 8px;
    border-radius: 6px;
    font-size: 11.5px;
    font-weight: 650;
    background: var(--fill-3);
  }
  .pill.instant {
    background: color-mix(in srgb, #34c759 26%, transparent);
    color: #8ff0ab;
  }
  .pill.hdr {
    background: color-mix(in srgb, #f0abfc 18%, transparent);
    color: #f5c6ff;
  }
  .pill.plain {
    background: transparent;
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.14);
  }
  .flag {
    font-size: 14px;
  }
  .src {
    margin-left: auto;
    font-size: 12px;
  }
  .empty {
    display: grid;
    justify-items: center;
    gap: 6px;
    padding: 40px 20px 30px;
    text-align: center;
    color: var(--text-dim);
  }
  .empty p {
    margin: 0;
    max-width: 46ch;
  }
  .empty .headline {
    color: var(--text);
  }
  @media (max-width: 759px) {
    .head {
      flex-direction: column;
      align-items: stretch;
      gap: 12px;
      padding: 2px 16px 12px;
    }
    .head .title2 {
      font-size: var(--fs-title3);
      text-align: center;
    }
    .head p {
      text-align: center;
    }
    .best .btn {
      flex: 1;
      height: 44px;
    }
    .filters {
      padding: 0 16px 10px;
    }
    .label {
      font-size: var(--fs-foot);
    }
  }
</style>
