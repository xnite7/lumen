<script lang="ts">
  // You: Discord, Simkl, and the handful of settings that matter on a phone.
  import { api } from "$lib/api";
  import { app, saveSettings, refreshSecrets } from "$lib/stores/app.svelte";
  import { toast } from "$lib/stores/toast.svelte";
  import SimklConnect from "$old/ui/SimklConnect.svelte";
  import DiscordSignIn from "$old/ui/DiscordSignIn.svelte";
  import Name from "../ui/Name.svelte";

  let me = $state<{ id: string; name: string; avatar?: string } | null>(null);
  $effect(() => {
    void app.secrets;
    api.discordMe().then((m) => (me = m)).catch(() => {});
  });
  const s = app.settings;
  const toggles: [string, () => boolean, (v: boolean) => void][] = [
    ["play the next episode by itself", () => s.playback.autoplayNext, (v) => (s.playback.autoplayNext = v)],
    ["skip intros", () => s.playback.autoSkipIntro, (v) => (s.playback.autoSkipIntro = v)],
    ["skip recaps", () => s.playback.autoSkipRecap, (v) => (s.playback.autoSkipRecap = v)],
    ["subtitles on by default", () => s.subtitles.enabledByDefault, (v) => (s.subtitles.enabledByDefault = v)],
    ["hide [sound cues] in subtitles", () => s.subtitles.hideSdh, (v) => (s.subtitles.hideSdh = v)],
    ["new episode alerts", () => s.general.episodeAlerts, (v) => (s.general.episodeAlerts = v)],
    ["prefer HDR", () => s.streams.preferHdr, (v) => (s.streams.preferHdr = v)],
  ];
  async function signOut() {
    await api.discordLogout().catch(() => {});
    await refreshSecrets();
    me = null;
    toast("signed out", "info");
  }
</script>

<div class="page">
  <header class="hd">
    {#if me}
      {#if me.avatar}<img class="av" src={me.avatar} alt="" />{/if}
      <Name id={me.id} name={me.name} size={26} />
    {:else}
      <h1>me</h1>
    {/if}
  </header>

  <section class="card">
    <h2>discord</h2>
    {#if me}
      <p class="muted">you're on the couch as {me.name}.</p>
      <button class="btn press" onclick={signOut}>sign out</button>
    {:else}
      <DiscordSignIn ondone={() => refreshSecrets()} />
    {/if}
  </section>

  <section class="card">
    <h2>simkl</h2>
    <SimklConnect />
  </section>

  <section class="card">
    <h2>watching</h2>
    {#each toggles as [label, get, set]}
      <label class="tg">
        <span>{label}</span>
        <input type="checkbox" checked={get()} onchange={(e) => (set(e.currentTarget.checked), saveSettings())} />
      </label>
    {/each}
  </section>

  <p class="faint ver">midnite™ couch {app.version}</p>
</div>

<style>
  .hd { display: flex; align-items: center; gap: 14px; padding: calc(var(--safe-t) + 18px) var(--g) 8px; }
  .hd h1 { margin: 0; font-size: 32px; }
  .av { width: 64px; height: 64px; border-radius: 50%; }
  .card { margin: 14px var(--g) 0; padding: 16px; border-radius: 20px; background: var(--surface); display: grid; gap: 10px; max-width: 720px; }
  .card h2 { margin: 0; font-size: 13px; text-transform: uppercase; letter-spacing: 0.08em; color: var(--label3); }
  .card p { margin: 0; }
  .tg { display: flex; justify-content: space-between; align-items: center; gap: 12px; padding: 6px 0; font-size: 15px; }
  .tg input { appearance: none; width: 50px; height: 30px; border-radius: 15px; background: #ffffff24; position: relative; transition: background 0.2s; flex: none; }
  .tg input::after { content: ""; position: absolute; top: 2px; left: 2px; width: 26px; height: 26px; border-radius: 50%; background: #fff; transition: transform 0.2s; box-shadow: 0 2px 6px #0006; }
  .tg input:checked { background: var(--neon); }
  .tg input:checked::after { transform: translateX(20px); }
  .ver { text-align: center; padding: 24px; font-size: 12px; }
</style>
