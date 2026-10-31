<script lang="ts">
  // Simkl, AUTH V2. One tap opens Simkl's consent page; approving sends you
  // straight back (to this computer, or into the app on iPhone). A short
  // code is the backup for when the browser is somewhere else.
  import { api, listen } from "../lib/api";
  import { app, refreshSecrets } from "../lib/stores/app.svelte";
  import { toast, errorText } from "../lib/stores/toast.svelte";
  import { haptic } from "../lib/native";
  import Item from "./Item.svelte";

  let waiting = $state(false);
  let code = $state<{ user_code: string; verification_url: string } | null>(null);
  let timer: ReturnType<typeof setInterval> | undefined;
  let deadline = 0;

  const connected = $derived(!!app.secrets.simkl_token);
  const legacy = $derived(connected && !app.secrets.simkl_v2);
  const who = $derived(app.secrets.simklUser?.user?.name as string | undefined);

  async function done() {
    stopCode();
    waiting = false;
    await refreshSecrets();
    haptic("success");
    toast("Simkl connected", "ok");
  }

  async function connect() {
    waiting = true;
    try {
      await api.simklOAuthStart();
    } catch (e) {
      waiting = false;
      toast(errorText(e), "error");
    }
  }

  async function useCode() {
    try {
      code = await api.simklDeviceStart();
      api.openExternal(code.verification_url);
      deadline = Date.now() + 10 * 60_000;
      clearInterval(timer);
      timer = setInterval(async () => {
        if (Date.now() > deadline) return stopCode();
        try {
          if (await api.simklDevicePoll()) done();
        } catch (e) {
          stopCode();
          toast(errorText(e), "warn");
        }
      }, 5500);
    } catch (e) {
      toast(errorText(e), "error");
    }
  }
  function stopCode() {
    clearInterval(timer);
    timer = undefined;
    code = null;
  }

  async function disconnect() {
    await api.simklLogout();
    await refreshSecrets();
  }

  $effect(() => {
    let un: (() => void) | undefined;
    listen<{ ok: boolean; error?: string }>("simkl-auth", (r) => {
      waiting = false;
      if (r.ok) done();
      else toast(r.error ?? "Simkl sign-in didn't finish", "warn");
    }).then((u) => (un = u));
    // iPhone: the lumen:// link landed and App finished the exchange
    const onFocus = () => {
      if (waiting) refreshSecrets().then(() => app.secrets.simkl_v2 && (waiting = false));
    };
    window.addEventListener("focus", onFocus);
    return () => {
      un?.();
      stopCode();
      window.removeEventListener("focus", onFocus);
    };
  });
</script>

{#if connected && !legacy}
  <Item label={who ? `Connected as ${who}` : "Simkl connected"} detail="History, ratings and watchlist stay in sync" icon="checkCircle" tint="var(--green)">
    <button class="btn small tinted press" onclick={disconnect}>Disconnect</button>
  </Item>
{:else if code}
  <Item label="Approve on Simkl" detail="Opened in your browser with the code filled in. Or go to simkl.com/pin and enter:">
    <span class="code">{code.user_code}</span>
  </Item>
  <Item label="Cancel" onclick={stopCode} />
{:else}
  <Item
    label={legacy ? "Reconnect Simkl" : "Connect Simkl"}
    detail={legacy ? "Your current sign-in uses Simkl's old system, which stops working in 2027. Reconnect once to move over." : waiting ? "Approve in your browser, then come back" : "Watch history, ratings and recommendations"}
    icon="sparkles"
    tint="#0b0b0f"
  >
    <button class="btn small accent press" disabled={waiting} onclick={connect}>{waiting ? "Waiting…" : "Connect"}</button>
  </Item>
  <Item label="Use a code instead" detail="For approving on another device" chevron onclick={useCode} />
{/if}

<style>
  .code {
    font-family: var(--font-round);
    font-weight: 700;
    font-size: 19px;
    letter-spacing: 0.08em;
  }
</style>
