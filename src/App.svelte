<script lang="ts">
  import { api, listen, type Meta, type Stream } from "./lib/api";
  import { app, boot, refreshSecrets } from "./lib/stores/app.svelte";
  import { applyCurrent } from "./lib/stores/theme.svelte";
  import { loadLibrary } from "./lib/stores/library.svelte";
  import { initGaze } from "./lib/stores/gaze.svelte";
  import { initPlayback, pb, play } from "./lib/stores/playback.svelte";
  import { toast } from "./lib/stores/toast.svelte";
  import { back, go, selectTab } from "./lib/router.svelte";
  import { ui } from "./lib/ui.svelte";
  import { isTauri } from "./lib/api";
  import TabBar from "./ui/TabBar.svelte";
  import ContextMenu from "./ui/ContextMenu.svelte";
  import Icon from "./components/Icon.svelte";
  import Onboarding from "./views/Onboarding.svelte";
  import Handoff from "./ui/Handoff.svelte";
  import Stack from "./ui/Stack.svelte";
  import PreviewCard from "./components/PreviewCard.svelte";
  import Toasts from "./components/Toasts.svelte";
  import JoinRequests from "./components/JoinRequests.svelte";
  import Gate from "./ui/Gate.svelte";
  import UpdateBanner from "./components/UpdateBanner.svelte";
  import { startUpdates } from "./lib/stores/updates.svelte";
  import { refreshCrew } from "./lib/stores/crew.svelte";
  import Player from "./views/Player.svelte";
  import Splash from "./ui/Splash.svelte";
  import { installTvNav } from "./lib/tvfocus";
  import { startCrew, joinCrew } from "./lib/stores/crew.svelte";

  let splashing = $state(!(import.meta.env.DEV && new URLSearchParams(location.search).has("nosplash")));

  const winControls = $derived(isTauri && (app.platform === "windows" || app.platform === "linux"));
  $effect(() => {
    document.documentElement.classList.toggle("winctl", winControls);
  });

  $effect(() => {
    (async () => {
      await boot();
      applyCurrent();
      initGaze();
      initPlayback();
      loadLibrary();
      startCrew();
      startUpdates();
      if (!app.settings.general.onboarded) ui.onboarding = true;
      else checkGate();
    })();
    listen<string>("deeplink", handleLink);
    listen("library-changed", () => loadLibrary());
    // kicked from the midnite server: lock the app right away
    listen<string>("discord-gate", (g) => g === "outsider" && (gate = "outsider"));
    setInterval(() => app.settings.general.onboarded && !ui.onboarding && checkGate(), 60_000);
    // Coming back to couch: ask Simkl what changed (one tiny request; the
    // real fetch only happens when something did).
    let lastSync = 0;
    document.addEventListener("visibilitychange", () => {
      if (document.visibilityState !== "visible" || Date.now() - lastSync < 120_000 || !app.secrets.simkl_token) return;
      lastSync = Date.now();
      api.simklSync().catch(() => {});
    });
    listen<{ kind: string; text: string }>("toast", (t) => toast(t.text, (t.kind as any) ?? "info"));
  });

  // TV-style arrow-key / controller focus on desktop
  $effect(() => {
    if (!app.ready || ui.phone || app.platform === "ios") return;
    return installTvNav({
      blocked: () => pb.open || ui.onboarding || splashing || !!document.querySelector(".sheet, .menu"),
      back: () => back(),
    });
  });

  $effect(() => {
    document.documentElement.classList.toggle("onboarding", ui.onboarding);
    document.documentElement.dataset.platform = app.platform;
  });

  // couch is for the midnite Discord server
  let gate = $state<"" | "outsider" | "signin">("");
  async function checkGate() {
    if (!isTauri) {
      // preview: ?gate=outsider / ?gate=signin
      const g = new URLSearchParams(location.search).get("gate");
      gate = g === "outsider" || g === "signin" ? g : "";
      return;
    }
    const m = await api.discordMembership().catch(() => "unknown" as const);
    gate = m === "outsider" ? "outsider" : m === "signin" ? "signin" : "";
    if (!gate) refreshCrew();
  }

  function handleLink(raw: string) {
    const u = raw.trim();
    if (!u) return;
    if (u.startsWith("lumen://auth/simkl")) {
      api
        .simklOAuthFinish(u)
        .then(async () => {
          await refreshSecrets();
          toast("Simkl connected", "ok");
        })
        .catch((e) => toast(String(e), "warn"));
      return;
    }
    if (u.startsWith("couch://auth/discord")) {
      api
        .discordFinish(u)
        .then(async () => {
          await refreshSecrets();
          toast("you're on the couch.", "ok");
        })
        .catch((e) => toast(String(e), "warn"));
      return;
    }
    if (u.startsWith("couch://crew")) {
      joinCrew(u)
        .then(() => {
          toast("you're in the crew.", "ok");
          selectTab("couch");
        })
        .catch((e) => toast(String(e), "warn"));
      return;
    }
    if ((u.startsWith("stremio://") || /^https?:\/\/.+\/manifest\.json(\?.*)?$/.test(u)) && u.includes("manifest.json")) {
      toast("couch comes with everything built in. no addons needed.", "info");
      return;
    }
    if (u.startsWith("magnet:")) {
      const hash = u.match(/xt=urn:btih:([a-zA-Z0-9]+)/)?.[1];
      if (!hash) return toast("That magnet link has no info hash", "warn");
      const name = decodeURIComponent(u.match(/[?&]dn=([^&]+)/)?.[1] ?? "Magnet").replace(/\+/g, " ");
      const trackers = [...u.matchAll(/[?&]tr=([^&]+)/g)].map((m) => "tracker:" + decodeURIComponent(m[1]));
      const meta: Meta = { id: `magnet:${hash.toLowerCase()}`, type: "movie", name };
      play(meta, { infoHash: hash.toLowerCase(), sources: trackers, title: name } as unknown as Stream, meta.id);
      return;
    }
    if (/^https?:\/\//.test(u)) {
      const name = decodeURIComponent(u.split("/").pop()?.split("?")[0] ?? "Video");
      const meta: Meta = { id: `url:${u}`, type: "movie", name };
      play(meta, { url: u, title: name } as unknown as Stream, meta.id);
    }
  }

  // Paste or drop a link anywhere (outside text fields) and it plays.
  function onPaste(e: ClipboardEvent) {
    const t = e.target as HTMLElement;
    if (t?.closest("input, textarea, [contenteditable]")) return;
    const text = e.clipboardData?.getData("text") ?? "";
    if (/^(magnet:|https?:\/\/|stremio:\/\/)/.test(text.trim())) {
      e.preventDefault();
      handleLink(text);
    }
  }
  function onDrop(e: DragEvent) {
    const text = e.dataTransfer?.getData("text/uri-list") || e.dataTransfer?.getData("text") || "";
    if (/^(magnet:|https?:\/\/|stremio:\/\/)/.test(text.trim())) {
      e.preventDefault();
      handleLink(text);
    }
  }

  function onKey(e: KeyboardEvent) {
    if (pb.open) return;
    const typing = (e.target as HTMLElement)?.closest("input, textarea");
    if (e.key === "Escape" && !typing) back();
    else if ((e.key === "/" || (e.ctrlKey && e.key === "k")) && !typing) {
      e.preventDefault();
      go({ name: "search" });
    } else if (e.altKey && e.key === "ArrowLeft") back();
  }
  function startDrag(e: MouseEvent) {
    if (e.button !== 0) return;
    if (e.detail === 2) api.window("maximize");
    else api.window("drag");
  }
  function onMouse(e: MouseEvent) {
    if (e.button === 3) {
      e.preventDefault();
      if (!pb.open) back();
    }
  }
</script>

<svelte:window onpaste={onPaste} ondrop={onDrop} ondragover={(e) => e.preventDefault()} onkeydown={onKey} onmouseup={onMouse} />

<!-- refraction filter used by html.refraction .glass -->
<svg width="0" height="0" style="position:absolute" aria-hidden="true">
  <filter id="lg-refract" x="-10%" y="-10%" width="120%" height="120%">
    <feTurbulence type="fractalNoise" baseFrequency="0.008 0.012" numOctaves="2" seed="7" result="n" />
    <feGaussianBlur in="n" stdDeviation="2" result="nb" />
    <feDisplacementMap in="SourceGraphic" in2="nb" scale="38" xChannelSelector="R" yChannelSelector="G" />
  </filter>
</svg>

{#if app.ready}
  <div class="night" aria-hidden="true"></div>
  <div class="grain" aria-hidden="true"></div>

  <Stack hidden={pb.open} />

  {#if ui.phone}<div class="edge-bottom" aria-hidden="true"></div>{/if}
  <TabBar />

  {#if winControls}
    <!-- borderless window: the strip along the top drags it -->
    <div class="drag" onmousedown={startDrag} role="presentation"></div>
    <div class="win">
      <button onclick={() => api.window("minimize")} aria-label="Minimize">
        <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><path d="M0 5.5h10" stroke="currentColor" stroke-width="1" /></svg>
      </button>
      <button onclick={() => api.window("maximize")} aria-label="Maximize">
        <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><rect x="0.5" y="0.5" width="9" height="9" fill="none" stroke="currentColor" stroke-width="1" /></svg>
      </button>
      <button class="x" onclick={() => api.window("close")} aria-label="Close">
        <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><path d="M0.5 0.5l9 9M9.5 0.5l-9 9" stroke="currentColor" stroke-width="1" /></svg>
      </button>
    </div>
  {/if}

  <PreviewCard />
  {#if pb.open}<Player />{/if}
  <ContextMenu />
  <Handoff />
  {#if ui.onboarding}
    <Onboarding ondone={() => { ui.onboarding = false; checkGate(); }} />
  {/if}
  {#if gate && !ui.onboarding}<Gate state={gate} recheck={checkGate} />{/if}
  <Toasts />
  <JoinRequests />
  <UpdateBanner />
{/if}
{#if splashing}<Splash ondone={() => (splashing = false)} />{/if}

<style>
  .drag {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    height: calc(var(--bar-h) + 10px);
    z-index: 50;
  }
  /* like every other Windows app: flush in the corner, full-height squares */
  .win {
    position: fixed;
    z-index: 200;
    top: 0;
    right: 0;
    display: flex;
    transition: opacity var(--t-med) ease;
  }
  /* watching: they fade with the player's controls */
  :global(html.playing.chrome-off) .win {
    opacity: 0;
    pointer-events: none;
  }
  .win button {
    width: 46px;
    height: 32px;
    border-radius: 0;
    display: grid;
    place-items: center;
    color: rgb(255 255 255 / 0.82);
    transition: background 120ms linear, color 120ms linear;
  }
  .win button:hover {
    background: rgb(255 255 255 / 0.1);
    color: #fff;
  }
  .win .x:hover {
    background: #c42b1c;
    color: #fff;
  }
  .win .x:active {
    background: #b0271a;
  }
</style>
