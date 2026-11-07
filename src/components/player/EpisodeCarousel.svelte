<script lang="ts">
  // Paused on a show: the season's episodes fanned up the right side, the
  // one you're on in the middle. Scroll (or arrow keys) to flip through,
  // click one to play it.
  import { img, type Video } from "../../lib/api";
  import Icon from "../Icon.svelte";

  let {
    videos,
    currentId,
    timeLeft = "",
    watched,
    progress,
    onplay,
  }: {
    videos: Video[];
    currentId: string;
    timeLeft?: string;
    watched: Set<string>;
    progress: (v: Video) => number;
    onplay: (v: Video) => void;
  } = $props();

  let focus = $state(0);
  let lastCurrent = "";
  $effect(() => {
    // follow the playing episode (and a season switch) back to centre
    const i = videos.findIndex((v) => v.id === currentId);
    const key = currentId + videos.length + (videos[0]?.id ?? "");
    if (key !== lastCurrent) {
      lastCurrent = key;
      focus = Math.max(0, i);
    }
  });

  let wheelAt = 0;
  function onWheel(e: WheelEvent) {
    e.preventDefault();
    const now = performance.now();
    if (now - wheelAt < 140) return;
    wheelAt = now;
    focus = Math.max(0, Math.min(videos.length - 1, focus + (e.deltaY > 0 ? 1 : -1)));
  }
  function onKey(e: KeyboardEvent) {
    if (e.key === "ArrowDown") focus = Math.min(videos.length - 1, focus + 1);
    else if (e.key === "ArrowUp") focus = Math.max(0, focus - 1);
    else if (e.key === "Enter" && videos[focus]) onplay(videos[focus]);
    else return;
    e.preventDefault();
    e.stopPropagation();
  }

  const label = (v: Video) => v.title || v.name || `Episode ${v.episode ?? v.number}`;
  const released = (v: Video) => !v.released || new Date(v.released).getTime() <= Date.now();
</script>

<div class="carousel" onwheel={onWheel} onkeydown={onKey} role="listbox" tabindex="0" aria-label="Episodes">
  {#each videos as v, i (v.id)}
    {@const d = i - focus}
    {#if Math.abs(d) <= 3}
      {@const cur = v.id === currentId}
      {@const p = progress(v)}
      <button
        class="ep"
        class:focus={d === 0}
        class:cur
        class:locked={!released(v)}
        style:--d={d}
        style:--ad={Math.abs(d)}
        style:z-index={10 - Math.abs(d)}
        onclick={() => (d === 0 ? released(v) && onplay(v) : (focus = i))}
        role="option"
        aria-selected={d === 0}
        aria-label="{v.episode ?? v.number}. {label(v)}"
      >
        <div class="art">
          {#if v.thumbnail}<img src={img(v.thumbnail)} alt="" loading="lazy" />{/if}
          <div class="shade"></div>
          <div class="txt">
            <span class="t">{v.episode ?? v.number}. {label(v)}</span>
            <span class="s">
              {#if cur}{timeLeft || "playing"}{:else if !released(v)}airs {new Date(v.released!).toLocaleDateString(undefined, { day: "numeric", month: "short" })}{:else if watched.has(v.id)}<Icon name="check" size={12} /> watched{:else if (v as any).runtime}{(v as any).runtime}{/if}
            </span>
          </div>
          {#if cur || p > 0}<div class="bar"><span style:width="{Math.max(4, (cur ? 1 : p) * 100)}%" class:now={cur}></span></div>{/if}
          {#if d === 0 && !cur && released(v)}<span class="go"><Icon name="play" size={18} /></span>{/if}
        </div>
      </button>
    {/if}
  {/each}
</div>

<style>
  .carousel {
    position: absolute;
    right: calc(var(--gutter) + 10px);
    top: 50%;
    width: min(330px, 28vw);
    height: 0;
    outline: none;
    animation: in 420ms var(--ease-out) both;
  }
  @keyframes in {
    from {
      opacity: 0;
      transform: translateX(30px);
    }
  }
  .ep {
    position: absolute;
    left: 0;
    right: 0;
    top: 0;
    /* the focused card sits in the middle; the rest fan out above and below,
       smaller and further back */
    transform: translateY(calc(-50% + var(--d) * 74px)) translateX(calc(var(--ad) * 22px)) scale(calc(1 - var(--ad) * 0.1));
    opacity: calc(1 - var(--ad) * 0.24);
    transition:
      transform 480ms var(--spring-soft),
      opacity 300ms ease;
    text-align: left;
  }
  .art {
    position: relative;
    aspect-ratio: 16 / 9;
    border-radius: var(--card-r);
    overflow: hidden;
    background: var(--surface-2);
    box-shadow: 0 18px 40px -16px rgb(var(--lo) / 0.9), inset 0 0 0 1px rgb(var(--hi) / 0.08);
    transition: box-shadow 300ms ease;
  }
  .focus .art {
    box-shadow:
      0 0 0 2px rgb(var(--hi) / 0.85),
      0 22px 50px -16px rgb(var(--lo) / 0.95),
      0 18px 46px -18px rgb(var(--neon-rgb) / 0.5);
  }
  .art img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .shade {
    position: absolute;
    inset: 0;
    background: linear-gradient(0deg, rgb(var(--lo) / 0.8), transparent 60%);
  }
  .txt {
    position: absolute;
    left: 12px;
    right: 12px;
    bottom: 12px;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .t {
    font-weight: 700;
    font-size: 15px;
    color: var(--text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .s {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 12px;
    color: rgb(var(--hi) / 0.7);
  }
  .bar {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: 3px;
    background: rgb(var(--hi) / 0.2);
  }
  .bar span {
    display: block;
    height: 100%;
    background: var(--progress);
  }
  .bar span.now {
    background: var(--neon);
  }
  .go {
    position: absolute;
    right: 12px;
    top: 12px;
    width: 36px;
    height: 36px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    color: var(--bg);
    background: var(--text);
  }
  .locked {
    filter: grayscale(0.6);
  }
</style>
