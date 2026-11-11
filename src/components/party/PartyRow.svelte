<script lang="ts">
  // Couch tab: upcoming watch parties as little tickets, with who's coming,
  // a countdown, and the right button for you (answer, start, join).
  import { img, type Party } from "../../lib/api";
  import { parties, rsvp, callOff, liveNow, loadParties } from "../../lib/stores/parties.svelte";
  import { crew } from "../../lib/stores/crew.svelte";
  import { askToJoin, together } from "../../lib/stores/together.svelte";
  import { quickPlay } from "../../lib/quickplay";
  import { live } from "../../lib/stores/live.svelte";
  import type { Meta } from "../../lib/api";

  $effect(() => {
    void live.tick;
    loadParties();
  });
  let now = $state(Date.now());
  $effect(() => {
    const t = setInterval(() => (now = Date.now()), 30_000);
    return () => clearInterval(t);
  });

  const list = $derived(parties.list.filter((p) => p.at > now - 3 * 3600_000));
  const mine = (p: Party) => p.host === parties.me;
  const going = (p: Party) => Object.values(p.rsvp).filter((r) => r.yes);
  const answered = (p: Party) => p.rsvp[parties.me];
  function countdown(p: Party) {
    const m = Math.round((p.at - now) / 60_000);
    if (m <= 0) return "on now";
    if (m < 60) return `in ${m} min`;
    const h = Math.round(m / 60);
    if (h < 24) return `in ${h} h`;
    return new Date(p.at).toLocaleDateString(undefined, { weekday: "long" }).toLowerCase();
  }
  function start(p: Party) {
    quickPlay({ id: p.title.id, type: p.title.type, name: p.title.name, poster: p.title.poster, background: p.title.background, logo: p.title.logo } as Meta, null, p.videoId ? { videoId: p.videoId } : {});
  }
  const hostOnline = (p: Party) => crew.members.find((m) => m.id === p.host && m.watching?.title);
</script>

{#if list.length}
  <section class="parties">
    <h2 class="title3">watch parties</h2>
    <div class="rail scroll-x">
      {#each list as p (p.id)}
        {@const a = answered(p)}
        <div class="tk" class:live={liveNow(p)}>
          <div class="tk-art">{#if p.title.background || p.title.poster}<img src={img(p.title.background ?? p.title.poster, "w780")} alt="" loading="lazy" />{/if}<span class="when">{countdown(p)}</span></div>
          <div class="tk-body">
            <b class="tk-t">{p.title.name}</b>
            <span class="tk-m">{new Date(p.at).toLocaleString(undefined, { weekday: "short", day: "numeric", month: "short", hour: "numeric", minute: "2-digit" }).toLowerCase()} · {mine(p) ? "your party" : `${p.hostName}'s`}</span>
            <div class="tk-going">
              {#each going(p).slice(0, 6) as g}<span class="g" title={g.name}>{#if g.avatar}<img src={g.avatar} alt="" />{:else}{g.name.charAt(0)}{/if}</span>{/each}
              <span class="foot dim">{going(p).length} going</span>
            </div>
            <div class="tk-act">
              {#if mine(p)}
                {#if liveNow(p)}<button class="btn primary small press" onclick={() => start(p)}>start it</button>{/if}
                <button class="btn tinted small press" onclick={() => callOff(p)}>call off</button>
              {:else if !a}
                <button class="btn primary small press" onclick={() => rsvp(p, true)}>I'm in</button>
                <button class="btn tinted small press" onclick={() => rsvp(p, false)}>can't</button>
              {:else if a.yes && liveNow(p) && hostOnline(p) && !together.following}
                <button class="btn primary small press" onclick={() => askToJoin(hostOnline(p)!)}>join now</button>
              {:else}
                <span class="foot dim">{a.yes ? "you're going" : "you passed"} · <button class="link" onclick={() => rsvp(p, !a.yes)}>change</button></span>
              {/if}
            </div>
          </div>
        </div>
      {/each}
    </div>
  </section>
{/if}

<style>
  .parties {
    margin-top: 28px;
  }
  h2 {
    padding: 0 var(--gutter);
    margin: 0 0 12px;
  }
  .rail {
    display: flex;
    gap: 14px;
    padding: 4px var(--gutter) 10px;
  }
  .tk {
    flex: none;
    width: 290px;
    border-radius: 16px;
    overflow: hidden;
    background: rgb(255 255 255 / 0.05);
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.08);
  }
  .tk.live {
    box-shadow:
      inset 0 0 0 1.5px var(--neon),
      0 0 30px -10px var(--neon);
  }
  .tk-art {
    position: relative;
    aspect-ratio: 16 / 7;
    background: #111;
    /* the tear line between the picture and the stub */
    -webkit-mask: radial-gradient(circle at 0 100%, transparent 9px, #000 9.5px) left / 51% 100% no-repeat, radial-gradient(circle at 100% 100%, transparent 9px, #000 9.5px) right / 51% 100% no-repeat;
    mask: radial-gradient(circle at 0 100%, transparent 9px, #000 9.5px) left / 51% 100% no-repeat, radial-gradient(circle at 100% 100%, transparent 9px, #000 9.5px) right / 51% 100% no-repeat;
  }
  .tk-art img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .when {
    position: absolute;
    left: 10px;
    top: 10px;
    padding: 3px 10px;
    border-radius: 999px;
    background: rgb(0 0 0 / 0.6);
    font-size: 12px;
    font-weight: 800;
  }
  .live .when {
    background: var(--neon);
  }
  .tk-body {
    display: grid;
    gap: 6px;
    padding: 12px 14px 14px;
    border-top: 2px dashed rgb(255 255 255 / 0.12);
  }
  .tk-t {
    font: 800 16px var(--font-display);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tk-m {
    font-size: 12px;
    color: var(--label-2);
  }
  .tk-going {
    display: flex;
    align-items: center;
  }
  .g {
    width: 24px;
    height: 24px;
    margin-right: -6px;
    border-radius: 50%;
    overflow: hidden;
    display: grid;
    place-items: center;
    background: #333;
    font-size: 11px;
    font-weight: 800;
    box-shadow: 0 0 0 2px #0b070a;
  }
  .g img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .tk-going .foot {
    margin-left: 14px;
  }
  .tk-act {
    display: flex;
    gap: 8px;
    align-items: center;
    margin-top: 2px;
  }
  .link {
    text-decoration: underline;
    color: var(--label-2);
  }
</style>
