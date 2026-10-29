<script lang="ts">
  // The hover card: grows out of the poster it came from, plays the trailer
  // (muted unless you ask), and offers the three things people do from here.
  // Lookups start the moment it opens; the trailer fades in only once it's
  // actually playing, so there's never a black box.
  import { api, img, type Meta } from "../lib/api";
  import { preview, closePreview, keepPreview } from "../lib/stores/preview.svelte";
  import { app } from "../lib/stores/app.svelte";
  import { inLibrary, toggleLibrary, setWatched, loadLibrary } from "../lib/stores/library.svelte";
  import { openDetail } from "../lib/open";
  import { year, runtime } from "../lib/format";
  import YouTube from "./YouTube.svelte";
  import Icon from "./Icon.svelte";

  let trailer = $state<string | null>(null);
  let muted = $state(true);
  let card: HTMLElement | undefined = $state();
  let shown = $state(false);

  const meta = $derived(preview.meta);
  const W = 380;

  const pos = $derived.by(() => {
    const r = preview.rect;
    if (!r) return { left: 0, top: 0, ox: "50%", oy: "50%" };
    const vw = innerWidth, vh = innerHeight;
    let left = r.left + r.width / 2 - W / 2;
    left = Math.max(16, Math.min(vw - W - 16, left));
    const h = W * (9 / 16) + 210;
    let top = r.top + r.height / 2 - h / 2;
    top = Math.max(84, Math.min(vh - h - 16, top));
    // grow from the poster's centre
    const ox = `${r.left + r.width / 2 - left}px`;
    const oy = `${r.top + r.height / 2 - top}px`;
    return { left, top, ox, oy };
  });

  $effect(() => {
    const m = meta;
    const token = preview.token;
    trailer = null;
    shown = false;
    muted = app.settings.general.previewMuted;
    if (!m) return;
    requestAnimationFrame(() => (shown = true));
    const own = m.trailerStreams?.[0]?.ytId ?? m.trailers?.find((t) => t.type === "Trailer")?.source ?? m.trailers?.[0]?.source;
    if (own) {
      trailer = own;
      return;
    }
    if (m.id.startsWith("tt")) {
      api.trailer(m.type, m.id).then((k) => {
        if (preview.token === token) trailer = k;
      });
    }
  });

  const owned = $derived(meta ? inLibrary(meta.id) : false);

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape" && meta) closePreview();
  }
</script>

<svelte:window onkeydown={onKey} />

{#if meta}
  <div
    bind:this={card}
    class="card glass strong"
    class:shown
    style:left="{pos.left}px"
    style:top="{pos.top}px"
    style:width="{W}px"
    style:transform-origin="{pos.ox} {pos.oy}"
    onmouseenter={keepPreview}
    onmouseleave={() => closePreview(120)}
    role="dialog"
    aria-label={meta.name}
    tabindex="-1"
  >
    <button class="media" onclick={() => openDetail(meta, card)} aria-label="Open {meta.name}">
      {#if meta.background || meta.poster}
        <img src={img(meta.background ?? meta.poster, "w780")} alt="" />
      {/if}
      {#if trailer}
        <YouTube id={trailer} {muted} loop fill />
      {/if}
      <div class="shade"></div>
      {#if meta.logo}
        <img class="logo" src={img(meta.logo)} alt={meta.name} />
      {:else}
        <span class="title2 name">{meta.name}</span>
      {/if}
      {#if trailer}
        <span
          class="mute glass clear"
          role="button"
          tabindex="0"
          onclick={(e) => {
            e.stopPropagation();
            muted = !muted;
          }}
          onkeydown={(e) => e.key === "Enter" && (muted = !muted)}
          aria-label={muted ? "Unmute" : "Mute"}
        >
          <Icon name={muted ? "mute" : "volume"} size={16} />
        </span>
      {/if}
    </button>
    <div class="body">
      <div class="actions">
        <button class="btn primary small press" onclick={() => openDetail(meta, card)}><Icon name="play" size={15} /> Watch</button>
        <button class="btn tinted icon small press" onclick={() => toggleLibrary(meta)} aria-label={owned ? "Remove from library" : "Add to library"} title={owned ? "In your library" : "Add to library"}>
          <Icon name={owned ? "check" : "plus"} size={17} />
        </button>
        <button class="btn tinted icon small press" onclick={() => setWatched(meta, null, true)} aria-label="Mark as watched" title="Mark as watched">
          <Icon name="eye" size={17} />
        </button>
        {#if preview.cw}
          <button
            class="btn tinted icon small press"
            onclick={async () => {
              await api.dismiss(meta.id);
              closePreview();
              loadLibrary();
            }}
            aria-label="Remove from Continue Watching"
            title="Remove from Continue Watching"
          >
            <Icon name="close" size={16} />
          </button>
        {/if}
      </div>
      <div class="facts">
        {#if meta.imdbRating}<span class="chip"><Icon name="star" size={11} fill /> {meta.imdbRating}</span>{/if}
        {#if year(meta)}<span class="chip outline">{year(meta)}</span>{/if}
        {#if meta.runtime}<span class="chip outline">{runtime(meta.runtime)}</span>{/if}
        <span class="chip outline">{meta.type === "series" ? "Series" : "Film"}</span>
      </div>
      {#if meta.genres?.length}
        <p class="genres foot dim">{meta.genres.slice(0, 3).join(", ")}</p>
      {/if}
      {#if meta.description}
        <p class="desc foot dim clamp-3">{meta.description}</p>
      {/if}
    </div>
  </div>
{/if}

<style>
  .card {
    position: fixed;
    z-index: 70;
    border-radius: var(--r-lg);
    overflow: hidden;
    opacity: 0;
    transform: scale(0.62);
    transition:
      transform var(--t-slow) var(--spring),
      opacity var(--t-med) ease;
    box-shadow:
      inset 0 1px 0 var(--glass-spec),
      inset 0 0 0 1px var(--glass-rim),
      0 40px 80px -30px rgb(0 0 0 / 0.8);
  }
  .card.shown {
    opacity: 1;
    transform: scale(1);
  }
  .media {
    position: relative;
    display: block;
    width: 100%;
    aspect-ratio: 16 / 9;
    overflow: hidden;
    background: #000;
  }
  .media > img:first-child {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .shade {
    position: absolute;
    inset: 0;
    background: linear-gradient(0deg, rgb(0 0 0 / 0.65), transparent 55%);
  }
  .logo {
    position: absolute;
    left: 16px;
    bottom: 14px;
    max-width: 58%;
    max-height: 34%;
    object-fit: contain;
    object-position: left bottom;
    filter: drop-shadow(0 2px 10px rgb(0 0 0 / 0.5));
  }
  .name {
    position: absolute;
    left: 16px;
    bottom: 14px;
    right: 60px;
    text-align: left;
    color: #fff;
  }
  .mute {
    position: absolute;
    right: 12px;
    bottom: 12px;
    width: 34px;
    height: 34px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    color: #fff;
  }
  .body {
    padding: 14px 16px 16px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .actions {
    display: flex;
    gap: 8px;
  }
  .facts {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .genres,
  .desc {
    margin: 0;
  }
</style>
