<script lang="ts">
  // The couch: who's watching right now, what the crew's been up to, and everyone.
  import { api, img, type Meta } from "$lib/api";
  import { crew, frames, refreshCrew } from "$lib/stores/crew.svelte";
  import { live } from "$lib/stores/live.svelte";
  import { quickPlay } from "$lib/quickplay";
  import { go, nav, openTitle } from "../lib/nav.svelte";
  import Name from "../ui/Name.svelte";

  let feed = $state<any[]>([]);
  let people = $state<any[]>([]);
  $effect(() => {
    void live.tick;
    refreshCrew();
    api.social<any>("GET", "/v1/activity").then((r) => (feed = (r?.feed ?? []).slice(0, 30))).catch(() => {});
    api.people().then((r) => (people = r.people ?? [])).catch(() => {});
  });
  const watching = $derived(crew.members.filter((m) => m.watching?.title));
  const ago = (t: number) => {
    const s = (Date.now() - (t > 1e12 ? t : t * 1000)) / 1000;
    return s < 3600 ? `${Math.max(1, Math.round(s / 60))}m` : s < 86400 ? `${Math.round(s / 3600)}h` : `${Math.round(s / 86400)}d`;
  };
  const metaOf = (t: any): Meta => ({ id: t.id, type: t.type, name: t.name, poster: t.poster, background: t.background, logo: t.logo }) as Meta;
  const verb = (a: any) => (a.kind === "review" ? "reviewed" : a.kind === "rating" ? "rated" : a.kind === "finished" ? "finished" : "watched");
</script>

<div class="page">
  <h1 class="ttl">the couch</h1>

  {#if watching.length}
    <h2 class="row-title">on the couch now</h2>
    <div class="scroller">
      {#each watching as m (m.id)}
        {@const w = m.watching!}
        <button class="live press" style:width="{nav.wide ? 360 : 280}px" onclick={() => w.metaId && openTitle({ id: w.metaId, type: w.type ?? "movie", name: w.title ?? "" } as Meta)}>
          <div class="art">
            <img src={frames[m.id]?.img ?? img(w.background ?? w.poster, "w780")} alt="" />
            <div class="fade"></div>
            <span class="dot" class:paused={m.paused}>{m.paused ? "paused" : "live"}</span>
            <div class="who">
              {#if m.avatar}<img class="av" src={m.avatar} alt="" />{/if}
              <Name id={m.id} name={m.name} />
            </div>
            {#if m.duration}<div class="bar"><i style:width="{(m.pos / m.duration) * 100}%"></i></div>{/if}
          </div>
          <div class="cap"><b>{w.title}</b>{#if w.episode}<span class="muted"> · {w.episode}</span>{/if}</div>
        </button>
      {/each}
    </div>
  {/if}

  {#if feed.length}
    <h2 class="row-title" style="margin-top:26px">lately</h2>
    <div class="feed" class:two={nav.wide}>
      {#each feed as a, i (i)}
        <div class="act">
          <button class="pst press" onclick={() => a.title?.id && openTitle(metaOf(a.title))}>
            {#if a.title?.poster}<img src={img(a.title.poster, "w342")} alt="" loading="lazy" />{/if}
          </button>
          <div class="body">
            <div class="line"><Name id={a.who} name={a.name ?? "someone"} size={14} /><span class="muted"> {verb(a)}</span></div>
            <b class="t">{a.title?.name}</b>
            {#if a.rating}<span class="score">{a.rating}/10</span>{/if}
            {#if a.text}<p class="clamp3 muted">“{a.text}”</p>{/if}
            <span class="faint">{ago(a.at)}</span>
          </div>
          {#if a.title?.id}<button class="mini glass press" onclick={() => quickPlay(metaOf(a.title))} aria-label="play">▶</button>{/if}
        </div>
      {/each}
    </div>
  {/if}

  {#if people.length}
    <h2 class="row-title" style="margin-top:26px">the crew</h2>
    <div class="people">
      {#each people as p (p.id)}
        <button class="person press" onclick={() => go({ name: "member", id: p.id, title: p.name })}>
          {#if p.avatar}<img src={p.avatar} alt="" />{:else}<span class="ph">{p.name?.[0] ?? "?"}</span>{/if}
          <Name id={p.id} name={p.name} size={13} />
        </button>
      {/each}
    </div>
  {/if}

  {#if !crew.setup && !people.length}
    <p class="muted empty">sign in with Discord on the me tab to join the couch.</p>
  {/if}
</div>

<style>
  .ttl { margin: 0; padding: calc(var(--safe-t) + 14px) var(--g) 4px; font-size: 32px; }
  .live { flex: none; text-align: left; scroll-snap-align: start; }
  .art { position: relative; aspect-ratio: 16 / 9; border-radius: 14px; overflow: hidden; background: var(--surface2); }
  .art > img:first-child { width: 100%; height: 100%; object-fit: cover; }
  .fade { position: absolute; inset: 0; background: linear-gradient(to top, #000c, #0000 55%); }
  .dot { position: absolute; top: 10px; left: 10px; font-size: 10px; font-weight: 900; text-transform: uppercase; letter-spacing: 0.08em; background: var(--neon); padding: 3px 7px; border-radius: 6px; }
  .dot.paused { background: #fff3; }
  .who { position: absolute; left: 10px; bottom: 12px; right: 10px; display: flex; gap: 8px; align-items: center; }
  .av { width: 26px; height: 26px; border-radius: 50%; }
  .bar { position: absolute; left: 0; right: 0; bottom: 0; height: 3px; background: #fff3; }
  .bar i { display: block; height: 100%; background: var(--neon-soft); }
  .cap { padding: 8px 2px 0; font-size: 13px; }
  .feed { display: grid; gap: 14px; padding: 0 var(--g); }
  .feed.two { grid-template-columns: 1fr 1fr; }
  .act { display: grid; grid-template-columns: 64px 1fr auto; gap: 12px; align-items: start; background: var(--surface); border-radius: 16px; padding: 10px; }
  .pst { aspect-ratio: 2 / 3; border-radius: 8px; overflow: hidden; background: var(--surface2); }
  .pst img { width: 100%; height: 100%; object-fit: cover; }
  .body { display: grid; gap: 3px; font-size: 13px; min-width: 0; }
  .line { display: flex; align-items: baseline; min-width: 0; }
  .t { font-size: 15px; }
  .score { color: var(--lamp); font-weight: 900; }
  .body p { margin: 0; }
  .mini { width: 36px; height: 36px; border-radius: 50%; align-self: center; }
  .people { display: grid; grid-template-columns: repeat(auto-fill, minmax(84px, 1fr)); gap: 16px; padding: 0 var(--g); }
  .person { display: grid; justify-items: center; gap: 6px; min-width: 0; }
  .person img, .person .ph { width: 64px; height: 64px; border-radius: 50%; object-fit: cover; background: var(--surface2); display: grid; place-items: center; font-weight: 900; font-size: 22px; }
  .person :global(.nm) { max-width: 100%; }
  .empty { padding: 30px var(--g); }
</style>
