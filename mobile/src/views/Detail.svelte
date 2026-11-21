<script lang="ts">
  // A title: backdrop, logo, play / resume, the seasons and episodes, more like it.
  import { api, img, type Meta, type Video } from "$lib/api";
  import { quickPlay } from "$lib/quickplay";
  import { orderedVideos } from "$lib/stores/playback.svelte";
  import { inLibrary, itemOf, toggleLibrary, watchedIds } from "$lib/stores/library.svelte";
  import { toast, errorText } from "$lib/stores/toast.svelte";
  import { back, nav } from "../lib/nav.svelte";
  import Row from "../ui/Row.svelte";
  import Poster from "../ui/Poster.svelte";

  let { meta: base }: { meta: Meta } = $props();
  let meta = $state<Meta>(base);
  let season = $state(1);
  let busy = $state("");
  let similar = $state<Meta[]>([]);

  $effect(() => {
    api.meta(base.type, base.id).then((m) => {
      meta = { ...base, ...m };
      const vid = (itemOf(base.id)?.state as any)?.video_id as string | undefined;
      const s = vid?.split(":")[1];
      const seasons = [...new Set(orderedVideos(meta).map((v) => v.season ?? 0))].filter((s) => s > 0);
      season = s ? +s : seasons[0] ?? 1;
    }).catch((e) => toast(errorText(e), "warn"));
    api.enrich(base.type, base.id).then((e: any) => (similar = e?.recommendations ?? [])).catch(() => {});
  });

  const eps = $derived(orderedVideos(meta).filter((v) => (v.season ?? 0) > 0));
  const seasons = $derived([...new Set(eps.map((v) => v.season!))]);
  const shown = $derived(eps.filter((v) => v.season === season));
  const seen = $derived(watchedIds(base.id));
  const st = $derived((itemOf(base.id)?.state ?? {}) as any);
  const resume = $derived(st.timeOffset > 0 && st.duration > 0 && st.timeOffset < st.duration * 0.92);
  const playLabel = $derived.by(() => {
    if (meta.type !== "series") return resume ? "▶ resume" : "▶ play";
    const p = (st.video_id as string | undefined)?.split(":");
    return p?.length === 3 ? `▶ ${resume ? "resume" : "play"} S${p[1]} E${p[2]}` : "▶ play S1 E1";
  });

  async function go(v?: Video) {
    if (busy) return;
    busy = v?.id ?? "main";
    try {
      await quickPlay(meta, null, { videoId: v?.id ?? (meta.type === "series" ? st.video_id : undefined) });
    } catch (e) {
      toast(errorText(e), "warn");
    } finally {
      busy = "";
    }
  }
  const today = new Date().toISOString().slice(0, 10);
</script>

<div class="page detail">
  <header class="top" class:wide={nav.wide}>
    {#if meta.background}<img class="bg" src={img(meta.background, "w1280")} alt="" />{/if}
    <div class="shade"></div>
    <button class="back glass press" onclick={back} aria-label="back">‹</button>
    <div class="info">
      {#if meta.logo}<img class="logo" src={img(meta.logo, "w500")} alt={meta.name} />{:else}<h1>{meta.name}</h1>{/if}
      <div class="facts muted">
        {#if meta.releaseInfo}<span>{meta.releaseInfo}</span>{/if}
        {#if meta.runtime}<span>{meta.runtime}</span>{/if}
        {#if meta.imdbRating}<span>★ {meta.imdbRating}</span>{/if}
        {#if meta.genres?.length}<span>{meta.genres.slice(0, 3).join(" · ").toLowerCase()}</span>{/if}
      </div>
      <div class="acts">
        <button class="btn primary press" onclick={() => go()}>{busy === "main" ? "finding a stream…" : playLabel}</button>
        <button class="btn round glass press" onclick={() => toggleLibrary(meta)} aria-label="my list">{inLibrary(meta.id) ? "✓" : "+"}</button>
      </div>
      {#if meta.description}<p class="clamp3">{meta.description}</p>{/if}
    </div>
  </header>

  {#if seasons.length}
    <div class="seasons scroller">
      {#each seasons as s}
        <button class="chip press" class:on={s === season} onclick={() => (season = s)}>season {s}</button>
      {/each}
    </div>
    <div class="eps" class:grid={nav.wide}>
      {#each shown as v (v.id)}
        {@const out = !v.released || v.released.slice(0, 10) <= today}
        <button class="ep press" class:dim={!out} disabled={!out} onclick={() => go(v)}>
          <div class="thumb">
            {#if v.thumbnail}<img src={img(v.thumbnail, "w500")} alt="" loading="lazy" />{/if}
            {#if seen.has(v.id)}<span class="seen">✓</span>{/if}
            {#if busy === v.id}<span class="spin">…</span>{/if}
          </div>
          <div class="txt">
            <b>{v.episode ?? v.number}. {v.name ?? v.title ?? "episode"}</b>
            <span class="faint">{out ? (v.released ?? "").slice(0, 10) : `out ${(v.released ?? "").slice(0, 10)}`}</span>
            {#if v.overview ?? v.description}<p class="clamp2 muted">{v.overview ?? v.description}</p>{/if}
          </div>
        </button>
      {/each}
    </div>
  {/if}

  {#if meta.cast?.length}
    <p class="cast muted">with {meta.cast.slice(0, 6).join(", ")}</p>
  {/if}

  {#if similar.length}
    <Row title="more like this">
      {#each similar as m (m.id)}<Poster meta={m} w={nav.wide ? 150 : 110} />{/each}
    </Row>
  {/if}
</div>

<style>
  .top { position: relative; min-height: 76vh; display: flex; align-items: flex-end; }
  .bg { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: cover; }
  .shade { position: absolute; inset: 0; background: linear-gradient(to top, var(--bg) 6%, #000b 45%, #0003 80%); }
  .back { position: absolute; top: calc(var(--safe-t) + 10px); left: 14px; width: 42px; height: 42px; border-radius: 50%; font-size: 28px; line-height: 1; display: grid; place-items: center; z-index: 2; }
  .info { position: relative; padding: 0 var(--g) 10px; display: grid; gap: 12px; max-width: 640px; }
  .logo { max-width: 78%; max-height: 130px; object-fit: contain; object-position: left bottom; filter: drop-shadow(0 4px 14px #000b); }
  h1 { margin: 0; font-size: 32px; }
  .facts { display: flex; flex-wrap: wrap; gap: 4px 12px; font-size: 13px; font-weight: 600; }
  .acts { display: flex; gap: 10px; }
  .acts .primary { flex: 1; max-width: 340px; }
  p { margin: 0; font-size: 14px; line-height: 1.45; }
  .seasons { margin-top: 18px; gap: 8px; }
  .chip { flex: none; padding: 8px 14px; border-radius: 18px; background: var(--surface2); font-weight: 700; font-size: 13px; color: var(--label2); }
  .chip.on { background: var(--text); color: #000; }
  .eps { display: grid; gap: 14px; padding: 16px var(--g) 0; }
  .eps.grid { grid-template-columns: repeat(auto-fill, minmax(320px, 1fr)); }
  .ep { display: grid; grid-template-columns: 150px 1fr; gap: 12px; text-align: left; align-items: start; }
  .ep.dim { opacity: 0.45; }
  .thumb { position: relative; aspect-ratio: 16 / 9; border-radius: 10px; overflow: hidden; background: var(--surface2); }
  .thumb img { width: 100%; height: 100%; object-fit: cover; }
  .seen { position: absolute; right: 6px; top: 6px; width: 20px; height: 20px; border-radius: 50%; background: var(--neon); font-size: 11px; display: grid; place-items: center; }
  .spin { position: absolute; inset: 0; display: grid; place-items: center; background: #0008; font-weight: 900; }
  .txt { display: grid; gap: 3px; font-size: 14px; }
  .txt span { font-size: 12px; }
  .txt p { font-size: 13px; }
  .cast { padding: 18px var(--g) 0; font-size: 13px; }
</style>
