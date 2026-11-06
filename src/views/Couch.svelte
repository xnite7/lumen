<script lang="ts">
  // The Couch tab: the midnite crew. Who's watching right now, your own
  // sharing switch, and the invite link to pass around the server.
  import { api } from "../lib/api";
  import { app, saveSettings } from "../lib/stores/app.svelte";
  import { crew, refreshCrew, joinCrew } from "../lib/stores/crew.svelte";
  import { toast, errorText } from "../lib/stores/toast.svelte";
  import CouchRow from "../components/CouchRow.svelte";
  import Controls from "../components/Controls.svelte";
  import Cell from "../components/Cell.svelte";
  import Icon from "../components/Icon.svelte";

  const s = app.settings;
  let link = $state("");
  let joining = $state(false);
  let invite = $state<string | null>(null);

  $effect(() => {
    refreshCrew();
    api.crewInvite().then((v) => (invite = v)).catch(() => {});
  });

  const live = $derived(crew.members.filter((m) => m.watching?.title));
  const idle = $derived(crew.members.filter((m) => !m.watching?.title));

  async function join(e: Event) {
    e.preventDefault();
    joining = true;
    try {
      await joinCrew(link);
      link = "";
      invite = await api.crewInvite().catch(() => null);
      toast("you're in the crew.", "ok");
    } catch (err) {
      toast(errorText(err), "warn");
    }
    joining = false;
  }
  async function copyInvite() {
    if (!invite) return;
    try {
      await navigator.clipboard.writeText(invite);
      toast("invite copied. drop it in #midnite.", "ok");
    } catch {
      toast(invite, "info");
    }
  }
</script>

<div class="couch">
  <h1 class="large-title">couch</h1>
  <p class="lede dim">the midnite crew, live.</p>

  {#if !crew.loaded}
    <p class="bump-text quiet">finding the remote…</p>
  {:else if !crew.setup}
    <section class="setup">
      <p class="bump-text quiet">it's just you and the couch.</p>
      <form class="join glass" onsubmit={join}>
        <Icon name="link" size={18} />
        <input bind:value={link} placeholder="paste the crew link" spellcheck="false" autocapitalize="off" />
        <button class="btn small primary press" disabled={!link.trim() || joining}>{joining ? "…" : "join"}</button>
      </form>
      <p class="foot dim hint">Someone in the midnite server has the link. Tap it in Discord and couch opens it.</p>
    </section>
  {:else}
    {#if crew.error}<p class="foot err">the crew server got scared. {crew.error}</p>{/if}
    {#if live.length}
      <CouchRow big />
    {:else}
      <p class="bump-text quiet">nobody's on the couch. save them a seat.</p>
    {/if}
    {#if idle.length}
      <p class="foot dim around">around: {idle.map((m) => m.name).join(", ")}</p>
    {/if}

    <div class="groups">
      <div class="group">
        <Cell label="Share what I'm watching" hint="Only titles Simkl knows, with Simkl's own title and poster. Needs Simkl connected.">
          <Controls kind="switch" bind:value={s.integrations.crewShare} onchange={saveSettings} />
        </Cell>
        <Cell label="Name on the couch" hint="What your friends see.">
          <input class="field name" bind:value={s.integrations.crewName} onchange={saveSettings} placeholder={app.secrets.stremioUser?.email?.split("@")[0] ?? "you"} />
        </Cell>
      </div>
      {#if invite}
        <div class="group">
          <Cell label="Invite link" hint="Anyone with it can see who's on the couch. Share it in the server, not in public.">
            <button class="btn small tinted press" onclick={copyInvite}><Icon name="share" size={15} /> copy</button>
          </Cell>
        </div>
      {/if}
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
