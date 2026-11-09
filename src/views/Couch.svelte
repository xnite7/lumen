<script lang="ts">
  // The Couch tab: the midnite crew. Who's watching right now, and your
  // Gatekeeper switch. Everyone in the midnite Discord server is in; there's
  // nothing to join.
  import { app, saveSettings } from "../lib/stores/app.svelte";
  import { crew, refreshCrew } from "../lib/stores/crew.svelte";
  import CouchRow from "../components/CouchRow.svelte";
  import Controls from "../components/Controls.svelte";
  import Cell from "../components/Cell.svelte";
  import Icon from "../components/Icon.svelte";
  import boil from "../assets/brand/couch-boil.webp";
  import wobble from "../assets/brand/couch-wobble.webp";
  import { api, type CrewPerson } from "../lib/api";
  import { go } from "../lib/router.svelte";
  import { live as tick } from "../lib/stores/live.svelte";

  const s = app.settings;

  $effect(() => {
    refreshCrew();
  });

  // everyone with a page, the ones on the couch right now first
  let people = $state<CrewPerson[]>([]);
  $effect(() => {
    void tick.tick;
    api.people().then((r) => (people = r.people ?? [])).catch(() => {});
  });
  const onNow = (id: string) => crew.members.some((m) => m.id === id);
  const crewList = $derived(people.slice().sort((a, b) => Number(onNow(b.id)) - Number(onNow(a.id)) || b.at - a.at));
  const openPerson = (p: { id: string; name: string; avatar?: string }) => go({ name: "member", id: p.id, who: p.name, avatar: p.avatar });

  const live = $derived(crew.members.filter((m) => m.watching?.title));
  const idle = $derived(crew.members.filter((m) => !m.watching?.title));
</script>

<div class="couch">
  <h1 class="large-title">couch</h1>
  <p class="lede dim">See what your friends from the server are watching right now. Hover someone's card to ask to join and watch in sync.</p>

  {#if !crew.loaded}
    <div class="state"><img src={wobble} alt="" /><p class="bump-text">finding the remote…</p></div>
  {:else if !crew.setup}
    <div class="state"><img src={wobble} alt="" /><p class="bump-text">getting you on the couch…</p></div>
  {:else}
    {#if crew.error}<p class="foot err">the crew server got scared. {crew.error}</p>{/if}
    {#if live.length}
      <CouchRow big />
    {:else}
      <div class="state"><img src={boil} alt="" /><p class="bump-text">nobody's on the couch.</p></div>
    {/if}
    {#if crewList.length}
      <section class="people">
        <h2 class="title3">the crew</h2>
        <div class="ppl scroll-x">
          {#each crewList as p (p.id)}
            <button class="pp press" onclick={() => openPerson(p)} style:--acc={p.accent || "var(--neon)"}>
              <span class="av">{#if p.avatar}<img src={p.avatar} alt="" />{:else}{p.name.charAt(0).toUpperCase()}{/if}{#if onNow(p.id)}<i></i>{/if}</span>
              <span class="pn">{p.name}</span>
            </button>
          {/each}
        </div>
      </section>
    {:else if idle.length}
      <p class="foot dim around">around: {idle.map((m) => m.name).join(", ")}</p>
    {/if}

    <div class="groups">
      <div class="group">
        <Cell label="Gatekeeper mode" hint="Friends see you're on the couch, not what you're watching. Nobody can ask to join.">
          <Controls kind="switch" bind:value={s.integrations.crewGatekeeper} onchange={saveSettings} />
        </Cell>
      </div>
    </div>
  {/if}
</div>

<style>
  .couch {
    padding: calc(var(--safe-top) + var(--bar-h) + 30px) 0 120px;
    max-width: 1400px;
  }
  .large-title,
  .lede {
    padding: 0 var(--gutter);
  }
  .lede {
    margin: 6px 0 22px;
    max-width: 62ch;
    font-size: 15px;
    line-height: 1.45;
  }
  .quiet {
    padding: 48px var(--gutter);
    color: var(--text-dim);
  }
  .state {
    display: grid;
    justify-items: center;
    gap: 6px;
    padding: 28px var(--gutter) 20px;
    color: var(--text-dim);
  }
  .state img {
    width: min(240px, 56vw);
    height: auto;
    opacity: 0.85;
    margin-bottom: 18px;
  }
  .state p {
    margin: 0;
  }
  .setup {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    padding: 0 var(--gutter);
  }
  .join {
    width: min(520px, 100%);
    display: flex;
    align-items: center;
    gap: 10px;
    height: 52px;
    padding: 0 8px 0 16px;
    border-radius: var(--r-pill);
  }
  .join input {
    flex: 1;
    min-width: 0;
    border: 0;
    background: none;
    outline: none;
  }
  .hint {
    max-width: 420px;
    text-align: center;
  }
  .people {
    margin-top: 28px;
  }
  .people h2 {
    padding: 0 var(--gutter);
    margin: 0 0 12px;
  }
  .ppl {
    display: flex;
    gap: 18px;
    padding: 12px var(--gutter) 10px;
  }
  .pp {
    flex: none;
    display: grid;
    justify-items: center;
    gap: 8px;
    width: 84px;
  }
  .pp .av {
    position: relative;
    width: 72px;
    height: 72px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    font: 800 26px system-ui, sans-serif;
    background: rgb(255 255 255 / 0.07);
    box-shadow:
      0 0 0 2px var(--acc),
      0 0 22px -6px var(--acc);
    transition: transform 0.2s;
  }
  .pp:hover .av {
    transform: scale(1.06);
  }
  .pp .av img {
    width: 100%;
    height: 100%;
    border-radius: 50%;
    object-fit: cover;
  }
  .pp .av i {
    position: absolute;
    right: 2px;
    bottom: 2px;
    width: 15px;
    height: 15px;
    border-radius: 50%;
    background: #3ddc84;
    box-shadow: 0 0 0 3px var(--black, #000);
  }
  .pn {
    font-size: 13px;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .around {
    padding: 0 var(--gutter);
  }
  .err {
    padding: 0 var(--gutter);
    color: var(--danger);
  }
  .groups {
    display: flex;
    flex-direction: column;
    gap: 20px;
    padding: 30px var(--gutter) 0;
    max-width: 720px;
  }
  .name {
    width: 180px;
    height: 36px;
  }
</style>
