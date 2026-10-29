<script lang="ts">
  import { api, listen, type Meta, type Stream } from "./lib/api";
  import { app, boot, refreshSecrets } from "./lib/stores/app.svelte";
  import { loadThemes, applyCurrent } from "./lib/stores/theme.svelte";
  import { loadLibrary } from "./lib/stores/library.svelte";
  import { initGaze } from "./lib/stores/gaze.svelte";
  import { initPlayback, pb, play } from "./lib/stores/playback.svelte";
  import { toast } from "./lib/stores/toast.svelte";
  import { nav, current, back, go, savedScroll } from "./lib/router.svelte";
  import TitleBar from "./components/TitleBar.svelte";
  import Ambient from "./components/Ambient.svelte";
  import PreviewCard from "./components/PreviewCard.svelte";
  import Toasts from "./components/Toasts.svelte";
  import Home from "./views/Home.svelte";
  import Discover from "./views/Discover.svelte";
  import Library from "./views/Library.svelte";
  import Search from "./views/Search.svelte";
  import Detail from "./views/Detail.svelte";
  import Person from "./views/Person.svelte";
  import Addons from "./views/Addons.svelte";
  import Settings from "./views/Settings.svelte";
  import Player from "./views/Player.svelte";

  let scroller: HTMLElement | undefined = $state();
  let scrollY = $state(0);
  const route = $derived(current());

  $effect(() => {
    (async () => {
      await boot();
      await loadThemes();
      applyCurrent();
      initGaze();
      initPlayback();
      loadLibrary();
      firstRun();
    })();
    listen<string>("deeplink", handleLink);
    listen("library-changed", () => loadLibrary());
    listen<{ kind: string; text: string }>("toast", (t) => toast(t.text, (t.kind as any) ?? "info"));
  });

  // Restore scroll when going back; top of page for new pages.
  $effect(() => {
    void nav.stack.length;
    void route;
    requestAnimationFrame(() => scroller && (scroller.scrollTop = savedScroll()));
  });

  // First launch on the old app's machine: offer to bring its keys along.
  function firstRun() {
    let seen = false;
    try {
      seen = !!localStorage.getItem("lumen.welcomed");
      localStorage.setItem("lumen.welcomed", "1");
    } catch {}
    if (seen || app.platform !== "windows" || app.secrets.simkl_token) return;
    toast("Welcome to Lumen. Bring your keys over from Stremio Enhanced?", "info", {
      label: "Import",
      run: async () => {
        try {
          const r = await api.importEnhanced();
          await refreshSecrets();
          toast(`Imported ${Object.keys(r).length} settings`, "ok");
        } catch (e) {
          toast(String(e), "error");
        }
      },
    }, 12000);
  }

  function handleLink(raw: string) {
    const u = raw.trim();
    if (!u) return;
    if (u.startsWith("stremio://") && u.includes("manifest.json")) {
      go({ name: "addons", install: u });
      return;
    }
    if (/^https?:\/\/.+\/manifest\.json(\?.*)?$/.test(u)) {
      go({ name: "addons", install: u });
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
  <Ambient />
  <div class="grain" aria-hidden="true"></div>
  <TitleBar scrolled={scrollY > 40} />

  <main class="scroller" bind:this={scroller} data-scroller onscroll={() => (scrollY = scroller!.scrollTop)} class:behind={pb.open}>
    {#key route}
      <div class="page">
        {#if route.name === "home"}
          <Home {scrollY} />
        {:else if route.name === "discover"}
          <Discover type={route.type} catalog={route.catalog} />
        {:else if route.name === "library"}
          <Library />
        {:else if route.name === "search"}
          <Search q={route.q} />
        {:else if route.name === "detail"}
          <Detail type={route.type} id={route.id} hint={route.hint} preview={route.preview} />
        {:else if route.name === "person"}
          <Person name={route.person} tmdbId={route.tmdbId} adult={route.adult} />
        {:else if route.name === "addons"}
          <Addons install={route.install} />
        {:else if route.name === "settings"}
          <Settings section={route.section} />
        {/if}
      </div>
    {/key}
  </main>

  <PreviewCard />
  {#if pb.open}<Player />{/if}
  <Toasts />
{:else}
  <div class="splash"><span class="sun"></span></div>
{/if}

<style>
  .scroller {
    position: relative;
    height: 100%;
    overflow-y: auto;
    overflow-x: hidden;
    scroll-behavior: auto;
  }
  .scroller.behind {
    visibility: hidden;
  }
  .page {
    min-height: 100%;
  }
  .splash {
    height: 100%;
    display: grid;
    place-items: center;
  }
  .sun {
    width: 54px;
    height: 54px;
    border-radius: 50%;
    background: radial-gradient(circle at 50% 40%, #fff4cc, #ffc22e 55%, #e4736a);
    box-shadow: 0 0 60px 10px rgb(255 194 46 / 0.35);
    animation: rise 1.4s var(--ease-out) both;
  }
  @keyframes rise {
    from {
      transform: translateY(30px);
      opacity: 0;
    }
  }
</style>
