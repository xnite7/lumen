<script lang="ts">
  // Sign in with Discord: one button, the browser does the rest. Your
  // Discord name and avatar become you on the couch.
  import { api, listen } from "../lib/api";
  import { app, refreshSecrets } from "../lib/stores/app.svelte";
  import { toast, errorText } from "../lib/stores/toast.svelte";

  let { ondone }: { ondone?: () => void } = $props();
  let busy = $state(false);

  $effect(() => {
    let un: (() => void) | undefined;
    listen<{ ok: boolean; error?: string }>("discord-auth", async (r) => {
      busy = false;
      if (r.ok) {
        await refreshSecrets();
        ondone?.();
      } else toast(r.error ?? "the tv got scared. try again?", "warn");
    }).then((u) => (un = u));
    return () => un?.();
  });

  // iPhone: App hands the couch://auth/discord link over and refreshes
  $effect(() => {
    if (busy && app.secrets.discordUser) {
      busy = false;
      ondone?.();
    }
  });

  async function go() {
    busy = true;
    try {
      await api.discordStart();
    } catch (e) {
      busy = false;
      toast(errorText(e), "warn");
    }
  }
</script>

<button class="discord press" onclick={go} disabled={busy}>
  <svg width="22" height="22" viewBox="0 0 24 24" aria-hidden="true"><path fill="currentColor" d="M19.6 5.3A17.6 17.6 0 0 0 15.3 4l-.5 1.1a16.4 16.4 0 0 0-5 0L9.3 4a17.6 17.6 0 0 0-4.3 1.3C2.3 9.3 1.6 13.2 1.9 17a17.7 17.7 0 0 0 5.4 2.7l1.1-1.8a11.4 11.4 0 0 1-1.8-.9l.4-.3a12.6 12.6 0 0 0 10.8 0l.4.3c-.6.3-1.2.6-1.8.9l1.1 1.8a17.6 17.6 0 0 0 5.4-2.7c.4-4.4-.7-8.3-3.3-11.7ZM8.7 14.7c-1 0-1.9-1-1.9-2.1s.8-2.1 1.9-2.1 1.9 1 1.9 2.1-.8 2.1-1.9 2.1Zm6.6 0c-1 0-1.9-1-1.9-2.1s.8-2.1 1.9-2.1 1.9 1 1.9 2.1-.8 2.1-1.9 2.1Z" /></svg>
  {busy ? "waiting for Discord…" : "sign in with Discord"}
</button>

<style>
  .discord {
    width: 100%;
    max-width: 400px;
    height: 52px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 10px;
    border-radius: var(--r-pill);
    font-weight: 700;
    font-size: var(--fs-body);
    color: #fff;
    background: #5865f2;
    box-shadow: 0 10px 30px -10px rgb(88 101 242 / 0.7);
  }
  .discord:disabled {
    opacity: 0.7;
  }
</style>
