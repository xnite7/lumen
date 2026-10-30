<script lang="ts">
  // Stremio sign-in, three ways:
  //  - iPhone: "Continue with Apple" opens Stremio's sign-in in a Safari
  //    sheet; approve with Face ID and the sheet closes itself.
  //  - Computer: a QR code. Scan it with the phone, approve, done.
  //  - Email and password, for anyone who has one.
  // The first two use Stremio's device-link codes, so they work with Apple,
  // Facebook and email accounts alike.
  import { api, img } from "../lib/api";
  import { refreshSecrets } from "../lib/stores/app.svelte";
  import { toast, errorText } from "../lib/stores/toast.svelte";
  import { openAuth, closeAuth, isIOS, haptic } from "../lib/native";
  import Icon from "../components/Icon.svelte";

  let { ondone, compact = false }: { ondone?: () => void; compact?: boolean } = $props();

  type Mode = "choose" | "code" | "email" | "waiting" | "done";
  let mode = $state<Mode>(isIOS() ? "choose" : "code");
  let link = $state<{ code: string; link: string; qrcode: string } | null>(null);
  let email = $state("");
  let password = $state("");
  let busy = $state(false);
  let timer: ReturnType<typeof setInterval> | undefined;
  let started = 0;

  async function startLink() {
    stop();
    try {
      link = await api.stremioLinkStart();
      started = Date.now();
      timer = setInterval(poll, 2500);
    } catch (e) {
      toast(errorText(e), "error");
    }
    return link;
  }
  async function poll() {
    if (!link) return;
    // codes expire; quietly fetch a fresh one every few minutes
    if (Date.now() - started > 4 * 60_000) {
      if (mode === "code") startLink();
      return;
    }
    try {
      if (await api.stremioLinkPoll(link.code)) finish();
    } catch {
      /* keep polling */
    }
  }
  function stop() {
    clearInterval(timer);
    timer = undefined;
  }
  async function finish() {
    stop();
    closeAuth();
    await refreshSecrets();
    haptic("success");
    mode = "done";
    setTimeout(() => ondone?.(), 900);
  }

  async function apple() {
    const l = await startLink();
    if (!l) return;
    mode = "waiting";
    if (!(await openAuth(l.link))) api.openExternal(l.link);
  }

  async function emailLogin(e: Event) {
    e.preventDefault();
    busy = true;
    try {
      await api.stremioLogin(email, password);
      password = "";
      finish();
    } catch (err) {
      toast(errorText(err), "error");
    }
    busy = false;
  }

  $effect(() => {
    if (mode === "code" && !link) startLink();
    return stop;
  });
</script>

<div class="signin" class:compact>
  {#if mode === "done"}
    <div class="done rise">
      <span class="tick"><Icon name="check" size={34} /></span>
      <p class="headline">Signed in</p>
      <p class="foot dim">Your addons and library are on their way.</p>
    </div>
  {:else if mode === "choose"}
    <div class="stack">
      <button class="btn lg block apple press" onclick={apple}><span class="glyph"></span> Continue with Apple</button>
      <button class="btn lg block tinted press" onclick={() => (mode = "code")}><Icon name="qr" size={19} /> Sign in with a code</button>
      <button class="btn lg block tinted press" onclick={() => (mode = "email")}><Icon name="mail" size={19} /> Use email</button>
    </div>
  {:else if mode === "waiting"}
    <div class="waiting">
      <span class="spinner"></span>
      <p class="headline">Approve in the Stremio page</p>
      <p class="foot dim">Sign in with Apple there. Lumen picks it up the moment you're in.</p>
      <button class="btn plain" onclick={() => link && openAuth(link.link)}>Open it again</button>
      <button class="btn plain dim" onclick={() => { stop(); mode = "choose"; }}>Back</button>
    </div>
  {:else if mode === "code"}
    <div class="code">
      <div class="qr">
        {#if link}
          <img src={img(link.qrcode)} alt="QR code for {link.link}" />
        {:else}
          <span class="skeleton"></span>
        {/if}
      </div>
      <div class="steps">
        <p class="headline">Scan with your iPhone</p>
        <ol class="foot dim">
          <li>Point the camera at the code</li>
          <li>Sign in with the account you use in Stremio (Apple works)</li>
          <li>Lumen signs in by itself</li>
        </ol>
        {#if link}
          <p class="manual foot">
            or go to <button class="linkish" onclick={() => api.openExternal(link!.link)}>{link.link.replace("https://", "")}</button>
          </p>
        {/if}
      </div>
    </div>
    <div class="alt">
      {#if isIOS()}<button class="btn plain" onclick={() => (mode = "choose")}>Other options</button>{/if}
      <button class="btn plain" onclick={() => (mode = "email")}>Use email instead</button>
    </div>
  {:else}
    <form class="email" onsubmit={emailLogin}>
      <div class="group fields">
        <input type="email" bind:value={email} placeholder="Email" autocomplete="username" />
        <input type="password" bind:value={password} placeholder="Password" autocomplete="current-password" />
      </div>
      <button class="btn lg block accent press" disabled={!email || !password || busy}>{busy ? "Signing in…" : "Sign in"}</button>
      <button type="button" class="btn plain" onclick={() => (mode = isIOS() ? "choose" : "code")}>{isIOS() ? "Other options" : "Use a code instead"}</button>
    </form>
  {/if}
</div>

<style>
  .signin {
    width: 100%;
    max-width: 440px;
    margin: 0 auto;
  }
  .stack,
  .email {
    display: flex;
    flex-direction: column;
    gap: 10px;
    align-items: center;
  }
  .apple {
    background: var(--label);
    color: var(--bg);
    font-weight: 600;
  }
  .glyph {
    font-family: -apple-system, system-ui;
    font-size: 20px;
    margin-top: -3px;
  }
  .code {
    display: flex;
    align-items: center;
    gap: 22px;
    padding: 18px;
    border-radius: var(--r-lg);
    background: var(--cell);
  }
  .qr {
    width: 148px;
    height: 148px;
    flex: none;
    border-radius: 20px;
    background: #fff;
    padding: 10px;
    display: grid;
  }
  .qr img,
  .qr .skeleton {
    width: 100%;
    height: 100%;
    border-radius: 8px;
    image-rendering: pixelated;
  }
  .steps p {
    margin: 0 0 6px;
  }
  ol {
    margin: 0;
    padding-left: 18px;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .manual {
    margin-top: 10px !important;
    color: var(--label-3);
  }
  .linkish {
    color: var(--accent);
    font-weight: 600;
  }
  .alt {
    display: flex;
    justify-content: center;
    gap: 6px;
    margin-top: 10px;
  }
  .fields {
    width: 100%;
  }
  .fields input {
    width: 100%;
    height: 50px;
    padding: 0 16px;
    border: 0;
    background: transparent;
    outline: none;
    font-size: var(--fs-body);
  }
  .fields input + input {
    border-top: 0.5px solid var(--separator);
  }
  .waiting,
  .done {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    gap: 6px;
    padding: 10px 0;
  }
  .waiting p,
  .done p {
    margin: 0;
  }
  .spinner {
    width: 30px;
    height: 30px;
    border-radius: 50%;
    border: 3px solid var(--fill-2);
    border-top-color: var(--label);
    animation: spin 0.9s linear infinite;
    margin-bottom: 8px;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .tick {
    width: 64px;
    height: 64px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: var(--green);
    color: #fff;
    margin-bottom: 8px;
    animation: pop var(--t-slow) var(--spring) both;
  }
  @keyframes pop {
    from {
      transform: scale(0.3);
      opacity: 0;
    }
  }
  @media (max-width: 480px) {
    .code {
      flex-direction: column;
      text-align: center;
    }
    ol {
      text-align: left;
    }
  }
</style>
