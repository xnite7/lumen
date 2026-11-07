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

  const s = app.settings;

  $effect(() => {
    refreshCrew();
  });

  const live = $derived(crew.members.filter((m) => m.watching?.title));
  const idle = $derived(crew.members.filter((m) => !m.watching?.title));
</script>

<div class="couch">
  <h1 class="large-title">couch</h1>
  <p class="lede dim">the midnite crew, live.</p>

  {#if !crew.loaded}
    <div class="state"><img src={wobble} alt="" /><p class="bump-text">finding the remote…</p></div>
  {:else if !crew.setup}
    <div class="state"><img src={wobble} alt="" /><p class="bump-text">getting you on the couch…</p></div>
  {:else}
    {#if crew.error}<p class="foot err">the crew server got scared. {crew.error}</p>{/if}
    {#if live.length}
      <CouchRow big />
    {:else}
      <div class="state"><img src={boil} alt="" /><p class="bump-text">nobody's on the couch. save them a seat.</p></div>
    {/if}
    {#if idle.length}
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
    margin: 4px 0 20px;
    font-family: var(--font-bump);
    font-size: 14px;
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
