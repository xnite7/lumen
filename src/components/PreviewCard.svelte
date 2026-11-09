<script lang="ts">
  import { trimLogo } from "../lib/trimlogo";
  import { fallback, imdbArt } from "../lib/fallback";
  import { logoBroken, logoOk } from "../lib/badlogo.svelte";
  import Clip from "./Clip.svelte";
  import { checkRelease, isUnreleased, releaseNote } from "../lib/release.svelte";
  // The hover card: grows out of the poster it came from, plays the trailer
  // (muted unless you ask), and offers the three things people do from here.
  // Lookups start the moment it opens; the trailer fades in only once it's
  // actually playing, so there's never a black box.
  import { api, img, type Meta } from "../lib/api";
  import { preview, closePreview, keepPreview } from "../lib/stores/preview.svelte";
  import { app } from "../lib/stores/app.svelte";
  import { inLibrary, toggleLibrary, setWatched, loadLibrary } from "../lib/stores/library.svelte";
  import { openDetail } from "../lib/open";
  import { quickPlay } from "../lib/quickplay";
  import { year, runtime } from "../lib/format";
  import YouTube from "./YouTube.svelte";
  import Icon from "./Icon.svelte";

  let trailer = $state<string | null>(null);
  let clip = $state<string | null>(null);
  let trailerOn = $state(false);
  let muted = $state(true);
  let card: HTMLElement | undefined = $state();
  let grown = $state(false);
  let bodyH = $state(190);

  // Up Next (and other bare) cards only carry a name and pictures: fetch the
  // rest (year, rating, genres, story) so every preview looks the same
  let full = $state<Meta | null>(null);
  $effect(() => {
    const m = preview.meta;
    full = null;
    if (!m || (m.description && (m.releaseInfo || (m as any).year))) return;
    api.meta(m.type, m.id.split(":")[0], (m as any)._addon).then((r) => preview.meta?.id === m.id && (full = r)).catch(() => {});
  });
  // one mute for every preview, remembered: sound on until you mute one
  const MUTE_KEY = "couch.previewMuted";
  function previewMuted() {
    try {
      return localStorage.getItem(MUTE_KEY) === "1";
    } catch {
      return false;
    }
  }
  function setPreviewMuted(on: boolean) {
    muted = on;
    try {
      localStorage.setItem(MUTE_KEY, on ? "1" : "0");
    } catch {}
  }
  const meta = $derived.by(() => {
    const m = preview.meta;
    if (!m || !full) return m;
    const own = Object.fromEntries(Object.entries(m).filter(([, v]) => v != null && v !== ""));
    return { ...full, ...own } as Meta;
  });
  $effect(() => checkRelease(meta));

  // Where it ends up: half again as wide as the poster (within reason), its
  // picture centred on the poster, kept inside the window and under the bar.
  const target = $derived.by(() => {
    const r = preview.rect;
    if (!r) return { left: 0, top: 0, w: 380, mh: 214 };
    const vw = innerWidth, vh = innerHeight;
    const w = Math.round(Math.min(480, Math.max(360, r.width * 1.8, r.width + 40)));
    const mh = Math.round((w * 9) / 16);
    const left = Math.max(16, Math.min(vw - w - 16, r.left + r.width / 2 - w / 2));
    // a tall poster: the card starts at its top so none of it peeks out above
    const want = r.height > mh ? r.top - 14 : r.top + r.height / 2 - mh / 2;
    const top = Math.max(76, Math.min(vh - mh - bodyH - 14, want));
    return { left, top, w, mh };
  });

  // Starts exactly as the poster, grows into the card, shrinks back on close.
  const box = $derived.by(() => {
    const r = preview.rect;
    const big = grown && !preview.closing;
    if (!r || big) return { ...target, body: true };
    return { left: r.left, top: r.top, w: r.width, mh: r.height, body: false };
  });

  $effect(() => {
    const m = preview.meta;
    const token = preview.token;
    trailer = null;
    clip = null;
    trailerOn = false;
    played = false;
    grown = false;
    muted = previewMuted();
    if (!m) return;
    // two frames: lay out at the poster's size first, then grow
    requestAnimationFrame(() => requestAnimationFrame(() => preview.token === token && (grown = true)));
    // a short scene (ClipCafe, 20 s or so) beats a full trailer; trailer if none
    if (m.id.startsWith("tt")) {
      api.clipUrl(m.id, m.name, String((m as any).year ?? (m as any).releaseInfo ?? "").slice(0, 4) || undefined).then((c) => {
        if (preview.token === token && c) clip = c;
      }).catch(() => {});
    }
    // teaser first (api.trailer prefers them); the title's own trailer if none
    const own = m.trailerStreams?.[0]?.ytId ?? m.trailers?.find((t) => t.type === "Teaser")?.source ?? m.trailers?.[0]?.source ?? null;
    if (m.id.startsWith("tt")) {
      api.trailer(m.type, m.id).then((k) => {
        if (preview.token === token) trailer = k;
      }).catch(() => {});
    } else trailer = own;
  });

  const owned = $derived(meta ? inLibrary(meta.id) : false);

  // the wheel over the card scrolls what's under it (the page, or a row
  // sideways) and the card gets out of the way
  // previews play once, then rest on the backdrop and title
  let played = $state(false);
  function finished() {
    trailerOn = false;
    setTimeout(() => (played = true), 450); // let it fade out first
  }

  function passWheel(e: WheelEvent) {
    if (!card) return;
    e.preventDefault();
    card.style.pointerEvents = "none";
    let el = document.elementFromPoint(e.clientX, e.clientY) as HTMLElement | null;
    card.style.pointerEvents = "";
    const sideways = Math.abs(e.deltaX) > Math.abs(e.deltaY) || e.shiftKey;
    while (el && el !== document.body) {
      const cs = getComputedStyle(el);
      if (sideways ? /(auto|scroll)/.test(cs.overflowX) && el.scrollWidth > el.clientWidth : /(auto|scroll)/.test(cs.overflowY) && el.scrollHeight > el.clientHeight) break;
      el = el.parentElement;
    }
    const target = el && el !== document.body ? el : (document.querySelector("[data-scroller]") as HTMLElement | null);
    const dy = e.deltaMode === 1 ? e.deltaY * 32 : e.deltaY;
    const dx = e.deltaMode === 1 ? e.deltaX * 32 : e.deltaX;
    target?.scrollBy({ top: sideways ? 0 : dy, left: sideways ? dx || dy : 0 });
    closePreview(0);
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape" && meta) closePreview();
  }
</script>

<svelte:window onkeydown={onKey} />

{#if meta}
  <div
    bind:this={card}
    class="pv card"
    class:grown={box.body}
    style:left="{box.left}px"
    style:top="{box.top}px"
    style:width="{box.w}px"
    style:--mh="{box.mh}px"
    onmouseenter={keepPreview}
    onmouseleave={() => closePreview(80)}
    onwheel={passWheel}
    role="dialog"
    aria-label={meta.name}
    tabindex="-1"
  >
    <button class="media" onclick={() => openDetail(meta, card)} aria-label="Open {meta.name}">
      <!-- always the wide backdrop (IMDb's if the title has none); the
           poster only as a last resort -->
      {#if meta.background || meta.poster || meta.id.startsWith("tt")}
        <img class="pv-art" src={img(meta.background ?? imdbArt(meta.id, "background") ?? meta.poster, "w780")} alt="" use:fallback={[meta.background ? imdbArt(meta.id, "background") : null, meta.poster ? img(meta.poster, "w780") : null]} />
      {/if}
      {#if played}
        <!-- played once: the backdrop and the title, that's it -->
      {:else if clip}
        <div class="tr" class:on={trailerOn}><Clip src={clip} {muted} onplaying={() => (trailerOn = true)} onended={finished} /></div>
      {:else if trailer}
        <div class="tr" class:on={trailerOn}><YouTube id={trailer} {muted} fill onplaying={() => (trailerOn = true)} onended={finished} /></div>
      {/if}
      <!-- the poster it came from, fading out as the card grows -->
      {#if preview.art}<img class="from" src={preview.art} alt="" />{/if}
      <div class="shade"></div>
      {#if meta.logo && !logoBroken(meta.logo)}
        <img class="logo" src={img(meta.logo)} alt={meta.name} use:logoOk={{ src: meta.logo, id: meta.id }} use:trimLogo />
      {:else}
        <span class="title2 name">{meta.name}</span>
      {/if}
      {#if (trailer || clip) && !played}
        <span
          class="mute glass clear"
          role="button"
          tabindex="0"
          onclick={(e) => {
            e.stopPropagation();
            setPreviewMuted(!muted);
          }}
          onkeydown={(e) => e.key === "Enter" && setPreviewMuted(!muted)}
          aria-label={muted ? "Unmute" : "Mute"}
        >
          <Icon name={muted ? "mute" : "volume"} size={16} />
        </span>
      {/if}
    </button>
    <div class="body-wrap">
      <div class="clip">
      <div class="body" bind:offsetHeight={bodyH}>
        <div class="actions">
          {#if isUnreleased(meta.id)}
            <button class="btn primary small press unreleased" title={releaseNote(meta.id)} onclick={() => { const m = meta; closePreview(0, false); openDetail(m, card); }}><Icon name="play" size={15} /> Unreleased</button>
          {:else}
            <button class="btn primary small press" onclick={() => { const m = meta; closePreview(0, false); app.settings.streams.autoplayBest ? quickPlay(m, card) : openDetail(m, card); }}><Icon name="play" size={15} /> Watch</button>
          {/if}
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
          <button class="btn tinted icon small press more" onclick={() => openDetail(meta, card)} aria-label="More info" title="More info">
            <Icon name="chevronDown" size={17} />
          </button>
        </div>
        <div class="facts">
          {#if meta.imdbRating}<span class="chip"><Icon name="star" size={11} fill /> {meta.imdbRating}</span>{/if}
          {#if year(meta)}<span class="chip outline">{year(meta)}</span>{/if}
          {#if meta.runtime}<span class="chip outline">{runtime(meta.runtime)}</span>{/if}
          <span class="chip outline">{meta.type === "series" ? "Series" : "Film"}</span>
        </div>
        {#if meta.genres?.length}
          <p class="genres foot dim">{meta.genres.slice(0, 3).join(" · ")}</p>
        {/if}
        {#if meta.description}
          <p class="desc foot dim clamp-3">{meta.description}</p>
        {/if}
      </div>
      </div>
    </div>
  </div>
{/if}

<style>
  /* one easing for every part, so it moves as one piece */
  .card {
    --pv-ease: cubic-bezier(0.22, 0.9, 0.24, 1);
    --pv-t: 300ms;
    position: fixed;
    z-index: 70;
    border-radius: var(--card-r, 10px);
    overflow: hidden;
    background: var(--surface);
    box-shadow: 0 0 0 1px rgb(var(--hi) / 0);
    transition:
      left var(--pv-t) var(--pv-ease),
      top var(--pv-t) var(--pv-ease),
      width var(--pv-t) var(--pv-ease),
      border-radius var(--pv-t) var(--pv-ease),
      box-shadow var(--pv-t) ease;
    will-change: left, top, width;
  }
  .card.grown {
    border-radius: var(--r-md);
    box-shadow:
      inset 0 0 0 1px rgb(var(--hi) / 0.08),
      0 30px 70px -20px rgb(var(--lo) / 0.95);
  }
  .media {
    position: relative;
    display: block;
    width: 100%;
    height: var(--mh);
    overflow: hidden;
    background: var(--black);
    transition: height var(--pv-t) var(--pv-ease);
  }
  .media > img.pv-art,
  .from {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .from {
    transition: opacity 220ms ease;
  }
  .grown .from {
    opacity: 0;
  }
  .tr {
    position: absolute;
    inset: 0;
    opacity: 0;
    transition: opacity 500ms ease;
  }
  .tr.on {
    opacity: 1;
  }
  .shade {
    position: absolute;
    inset: 0;
    background: linear-gradient(0deg, rgb(var(--lo) / 0.65), transparent 55%);
    opacity: 0;
    transition: opacity var(--pv-t) ease;
  }
  .grown .shade,
  .grown .logo,
  .grown .name,
  .grown .mute {
    opacity: 1;
  }
  .logo,
  .name,
  .mute {
    opacity: 0;
    transition: opacity 200ms ease 80ms;
  }
  /* the details unfold under the picture */
  .body-wrap {
    display: grid;
    grid-template-rows: 0fr;
    transition: grid-template-rows var(--pv-t) var(--pv-ease);
  }
  .grown .body-wrap {
    grid-template-rows: 1fr;
  }
  .clip {
    min-height: 0;
    overflow: hidden;
  }
  .body {
    padding: 14px 16px 16px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    opacity: 0;
    transition: opacity 180ms ease;
  }
  .grown .body {
    opacity: 1;
    transition-delay: 90ms;
  }
  .actions .more {
    margin-left: auto;
  }
  .logo {
    position: absolute;
    left: 16px;
    bottom: 14px;
    max-width: 58%;
    max-height: 34%;
    object-fit: contain;
    object-position: left bottom;
    filter: drop-shadow(0 2px 10px rgb(var(--lo) / 0.5));
  }
  .name {
    position: absolute;
    left: 16px;
    bottom: 14px;
    right: 60px;
    text-align: left;
    color: var(--white);
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
    color: var(--white);
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
  .unreleased {
    opacity: 0.42;
    filter: grayscale(1);
  }
</style>
