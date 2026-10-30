<script lang="ts">
  // Continue Watching, 16:9. Prefers Movie of the Night's horizontal poster
  // (a real wide crop with the title baked in); otherwise the backdrop with
  // the logo laid over it, so the layout never changes either way.
  import { api, img, type LibraryItem, type Meta } from "../lib/api";
  import { lib } from "../lib/stores/library.svelte";
  import { openPreview, closePreview, preview } from "../lib/stores/preview.svelte";
  import { app } from "../lib/stores/app.svelte";
  import { go } from "../lib/router.svelte";
  import { episodeLabel, remaining } from "../lib/format";
  import Icon from "./Icon.svelte";
  import { pressMenu } from "../lib/menu.svelte";
  import { posterMenu } from "../lib/actions";
  import { loadLibrary } from "../lib/stores/library.svelte";

  function cwMenu(it: LibraryItem) {
    return posterMenu({ ...asMeta(it), background: it.background ?? it.poster, posterShape: "landscape" }, [
      { label: "Remove from Continue Watching", icon: "close", destructive: true, run: async () => { await api.dismiss(it._id); loadLibrary(); } },
    ]);
  }

  const ART_KEY = "lumen.cwArt";
  let art = $state<Record<string, string | null>>(load());
  function load() {
    try {
      return JSON.parse(localStorage.getItem(ART_KEY) || "{}");
    } catch {
      return {};
    }
  }

  $effect(() => {
    for (const it of lib.cw) {
      const id = it._id;
      if (!id.startsWith("tt") || id in art) continue;
      art[id] = null;
      api.art(id).then((a) => {
        art[id] = a?.poster ?? null;
        try {
          localStorage.setItem(ART_KEY, JSON.stringify(art));
        } catch {}
      }).catch(() => {});
    }
  });

  function asMeta(it: LibraryItem): Meta {
    return { id: it._id, type: it.type, name: it.name, poster: it.poster, background: it.background, logo: it.logo };
  }

  function resume(it: LibraryItem) {
    closePreview();
    go({ name: "detail", type: it.type, id: it._id, preview: { ...asMeta(it), autoplay: it.state.video_id } });
  }

  let timers = new Map<string, ReturnType<typeof setTimeout>>();
  function arm(it: LibraryItem, el: HTMLElement) {
    clearTimeout(timers.get(it._id));
    if (!app.settings.general.hoverPreviews) return;
    timers.set(it._id, setTimeout(() => openPreview(asMeta(it), el, { cw: it }), app.settings.general.hoverDelayMs + 300));
  }
  function disarm(it: LibraryItem) {
    clearTimeout(timers.get(it._id));
    if (preview.meta?.id === it._id) closePreview(160);
  }
</script>

{#if lib.cw.length}
  <section class="row">
    <header><h2 class="title3">Continue watching</h2></header>
    <div class="rail scroll-x">
      {#each lib.cw as it (it._id)}
        {@const wide = art[it._id]}
        {@const p = it.state.duration ? it.state.timeOffset / it.state.duration : 0}
        <button
          class="card press"
          onclick={() => resume(it)}
          onmouseenter={(e) => arm(it, e.currentTarget)}
          onmousemove={(e) => arm(it, e.currentTarget)}
          onmouseleave={() => disarm(it)}
          use:pressMenu={() => cwMenu(it)}
        >
          <div class="art">
            {#if wide}
              <img src={img(wide)} alt="" loading="lazy" />
            {:else}
              <img src={img(it.background ?? it.poster, "w780")} alt="" loading="lazy" />
              <div class="shade"></div>
              {#if it.logo}<img class="logo" src={img(it.logo)} alt={it.name} loading="lazy" />{:else}<span class="nm">{it.name}</span>{/if}
            {/if}
            <span class="play glass clear"><Icon name="play" size={18} /></span>
            <div class="bar"><span style:width="{Math.max(3, p * 100)}%"></span></div>
          </div>
          <div class="meta">
            <span class="t">{it.name}</span>
            <span class="s">{[episodeLabel(it.state.video_id), remaining(it.state.timeOffset / 1000, it.state.duration / 1000)].filter(Boolean).join("  ·  ")}</span>
          </div>
        </button>
      {/each}
    </div>
  </section>
{/if}

<style>
  .row {
    margin-top: 6px;
  }
  header {
    padding: 0 var(--gutter);
    margin-bottom: 12px;
  }
  .rail {
    display: flex;
    gap: 16px;
    padding: 6px var(--gutter) 16px;
  }
  .card {
    flex: 0 0 auto;
    width: clamp(260px, 19vw, 340px);
    text-align: left;
  }
  .art {
    position: relative;
    aspect-ratio: 16 / 9;
    border-radius: var(--r-md);
    overflow: hidden;
    background: rgb(255 255 255 / 0.05);
    box-shadow: 0 12px 30px -16px rgb(0 0 0 / 0.7), inset 0 0 0 1px rgb(255 255 255 / 0.08);
    transition: transform var(--t-med) var(--spring);
  }
  @media (hover: hover) {
    .card:hover .art {
      transform: translateY(-4px) scale(1.015);
    }
  }
  :global(.card.pressing) .art {
    transform: scale(0.96);
    transition-duration: 480ms;
  }
  .art > img:first-child {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .shade {
    position: absolute;
    inset: 0;
    background: linear-gradient(0deg, rgb(0 0 0 / 0.6), transparent 60%);
  }
  .logo {
    position: absolute;
    left: 14px;
    bottom: 20px;
    max-width: 55%;
    max-height: 38%;
    object-fit: contain;
    object-position: left bottom;
  }
  .nm {
    position: absolute;
    left: 14px;
    bottom: 18px;
    font-weight: 700;
    font-size: 18px;
    color: #fff;
  }
  .play {
    position: absolute;
    right: 12px;
    bottom: 18px;
    width: 40px;
    height: 40px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    color: #fff;
    opacity: 0;
    transform: scale(0.8);
    transition: opacity var(--t-fast), transform var(--t-med) var(--spring);
  }
  .card:hover .play {
    opacity: 1;
    transform: none;
  }
  .bar {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: 4px;
    background: rgb(255 255 255 / 0.22);
  }
  .bar span {
    display: block;
    height: 100%;
    background: var(--accent);
  }
  .meta {
    display: flex;
    flex-direction: column;
    padding: 9px 2px 0;
  }
  .t {
    font-weight: 600;
    font-size: var(--fs-foot);
  }
  .s {
    font-size: var(--fs-caption);
    color: var(--label-2);
  }
  @media (max-width: 759px) {
    .row {
      margin-top: 24px;
    }
    header {
      margin-bottom: 6px;
    }
    .rail {
      gap: 10px;
      scroll-snap-type: x mandatory;
      scroll-padding: 0 var(--gutter);
    }
    .card {
      width: 74vw;
      scroll-snap-align: start;
    }
    .play {
      opacity: 1;
      transform: none;
      width: 36px;
      height: 36px;
    }
  }
</style>
