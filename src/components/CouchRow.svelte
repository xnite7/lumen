<script lang="ts">
  // "On the couch": what the crew is watching right now, as Apple TV-style
  // landscape cards. Only shows up when someone's actually there.
  import { img, type CrewMember } from "../lib/api";
  import { crew } from "../lib/stores/crew.svelte";
  import { go } from "../lib/router.svelte";
  import { tvFocus } from "../lib/tvfocus";

  let { big = false }: { big?: boolean } = $props();

  const live = $derived(crew.members.filter((m) => m.watching?.title));

  function open(m: CrewMember) {
    const w = m.watching;
    if (w?.metaId && w.type) go({ name: "detail", type: w.type, id: w.metaId });
  }
  const pct = (m: CrewMember) => (m.duration ? Math.min(1, m.pos / m.duration) : 0);
</script>

{#if live.length}
  <section class="row" class:big>
    {#if !big}<header><h2 class="title3">on the couch</h2><span class="live"><span class="dot"></span>{live.length} watching</span></header>{/if}
    <div class="rail scroll-x">
      {#each live as m (m.id)}
        {@const w = m.watching!}
        <button class="card press" onclick={() => open(m)}>
          <div class="art" use:tvFocus={{ max: 5 }}>
            {#if w.background || w.poster}<img src={img(w.background ?? w.poster, "w780")} alt="" loading="lazy" class:tall={!w.background} />{/if}
            <div class="shade"></div>
            <span class="who"><span class="av">{#if m.avatar}<img src={m.avatar} alt="" />{:else}{m.name.charAt(0).toUpperCase()}{/if}</span>{m.name}</span>
            {#if m.paused}<span class="paused">paused</span>{/if}
            <div class="bar"><span style:width="{Math.max(4, pct(m) * 100)}%"></span></div>
          </div>
          <div class="meta">
            <span class="t">{w.title}</span>
            <span class="s">{w.episode ?? w.year ?? ""}</span>
          </div>
        </button>
      {/each}
    </div>
  </section>
{/if}

<style>
  .row {
    margin-top: var(--shelf-gap);
  }
  .row.big {
    margin-top: 8px;
  }
  header {
    display: flex;
    align-items: baseline;
    gap: 12px;
    padding: 0 var(--gutter);
    margin-bottom: 12px;
  }
  .live {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-foot);
    color: var(--text-dim);
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--neon);
    box-shadow: 0 0 8px var(--neon);
    animation: breathe 2.4s ease-in-out infinite;
  }
  @keyframes breathe {
    50% {
      opacity: 0.35;
    }
  }
  .rail {
    display: flex;
    gap: 18px;
    padding: 10px var(--gutter) 22px;
  }
  .big .rail {
    flex-wrap: wrap;
    overflow: visible;
  }
  .card {
    flex: 0 0 auto;
    width: var(--land-w);
    text-align: left;
  }
  .big .card {
    width: min(100%, 420px);
  }
  .art {
    position: relative;
    aspect-ratio: 16 / 9;
    border-radius: var(--card-r);
    overflow: hidden;
    background: var(--surface);
    box-shadow: 0 12px 30px -16px rgb(var(--lo) / 0.8), inset 0 0 0 1px rgb(var(--hi) / 0.06);
  }
  .art img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .art img.tall {
    object-position: center 20%;
  }
  .shade {
    position: absolute;
    inset: 0;
    background: linear-gradient(0deg, rgb(var(--lo) / 0.75), transparent 55%), linear-gradient(180deg, rgb(var(--lo) / 0.45), transparent 35%);
  }
  .who {
    position: absolute;
    left: 12px;
    top: 10px;
    display: inline-flex;
    align-items: center;
    gap: 7px;
    font-weight: 700;
    font-size: var(--fs-foot);
    color: var(--text);
  }
  .av img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .av {
    overflow: hidden;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    font-size: 12px;
    background: linear-gradient(140deg, var(--neon), #6b1d4c);
    box-shadow: 0 0 0 2px rgb(var(--lo) / 0.5);
  }
  .paused {
    position: absolute;
    right: 12px;
    top: 12px;
    font-family: var(--font-bump);
    font-size: 11px;
    color: var(--text-dim);
  }
  .bar {
    position: absolute;
    left: 12px;
    right: 12px;
    bottom: 10px;
    height: 4px;
    border-radius: 4px;
    overflow: hidden;
    background: rgb(var(--hi) / 0.24);
  }
  .bar span {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: var(--progress);
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
    .card {
      width: 72vw;
    }
  }
</style>
