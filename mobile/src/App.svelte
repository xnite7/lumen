<script lang="ts">
  // midnite™ couch for iPhone and iPad. The same engine as the PC app, new screens.
  import { api, listen, isTauri } from "$lib/api";
  import { app, boot, refreshSecrets } from "$lib/stores/app.svelte";
  import { loadLibrary } from "$lib/stores/library.svelte";
  import { initPlayback, pb } from "$lib/stores/playback.svelte";
  import { startCrew, refreshCrew, joinCrew } from "$lib/stores/crew.svelte";
  import { startLive, live } from "$lib/stores/live.svelte";
  import { toast } from "$lib/stores/toast.svelte";
  import Toasts from "$old/components/Toasts.svelte";
  import Gate from "$old/ui/Gate.svelte";
  import Player from "$old/views/Player.svelte";
  import { nav, back, tab, go, type Page } from "./lib/nav.svelte";
  import Nav from "./ui/Nav.svelte";
  import DeviceBar from "./ui/DeviceBar.svelte";
  import { startDevices } from "./lib/devices.svelte";
  import Home from "./views/Home.svelte";
  import Search from "./views/Search.svelte";
  import Couch from "./views/Couch.svelte";
  import Lists from "./views/Lists.svelte";
  import Me from "./views/Me.svelte";
  import Detail from "./views/Detail.svelte";
  import Member from "./views/Member.svelte";

  let gate = $state<"" | "outsider" | "signin">("");
  let ready = $state(false);

  // iPad (or a wide iPhone in landscape): the sidebar layout
  $effect(() => {
    const mq = matchMedia("(min-width: 820px)");
    const f = () => (nav.wide = mq.matches);
    f();
    mq.addEventListener("change", f);
    return () => mq.removeEventListener("change", f);
  });

  async function checkGate() {
    if (!isTauri) return;
    const m = await api.discordMembership().catch(() => "unknown" as const);
    gate = m === "outsider" ? "outsider" : m === "signin" ? "signin" : "";
    if (!gate) refreshCrew();
  }

  $effect(() => {
    (async () => {
      await boot().catch(() => {});
      ready = true;
      initPlayback();
      loadLibrary();
      startCrew();
      startLive();
      startDevices();
      checkGate();
    })();
    listen<string>("deeplink", link);
    listen("library-changed", () => loadLibrary());
    listen<string>("discord-gate", (g) => (g === "outsider" || g === "signin") && (gate = g));
    listen<{ name: string; body: string; id: string; type: string }>("episode-alert", (a) =>
      toast(`${a.name}: ${a.body}`, "info", { label: "open", run: () => go({ name: "detail", meta: { id: a.id, type: a.type, name: a.name } as any }) }, 9000),
    );
    listen<{ kind: string; text: string }>("toast", (t) => toast(t.text, (t.kind as any) ?? "info"));
    let last = 0;
    document.addEventListener("visibilitychange", () => {
      if (document.visibilityState !== "visible" || Date.now() - last < 120_000 || !app.secrets.simkl_token) return;
      last = Date.now();
      api.simklSync().catch(() => {});
    });
  });

  $effect(() => {
    if (live.tick) loadLibrary().catch(() => {});
  });

  function link(raw: string) {
    const u = raw.trim();
    if (u.startsWith("lumen://auth/simkl")) {
      api.simklOAuthFinish(u).then(async () => (await refreshSecrets(), toast("Simkl connected", "ok"))).catch((e) => toast(String(e), "warn"));
    } else if (u.startsWith("couch://auth/discord")) {
      api.discordFinish(u).then(async () => (await refreshSecrets(), checkGate(), toast("you're on the couch.", "ok"))).catch((e) => toast(String(e), "warn"));
    } else if (u.startsWith("couch://crew")) {
      joinCrew(u).then(() => (toast("you're in the crew.", "ok"), tab("couch"))).catch((e) => toast(String(e), "warn"));
    } else {
      const t = u.match(/^couch:\/\/title\/(movie|series)\/([^/?#]+)/);
      if (t) go({ name: "detail", meta: { id: decodeURIComponent(t[2]), type: t[1], name: "" } as any });
    }
  }

  // swipe from the left edge to go back
  let sx = 0, sy = 0, edge = false;
  const ts = (e: TouchEvent) => ((sx = e.touches[0].clientX), (sy = e.touches[0].clientY), (edge = sx < 24 && nav.stack.length > 0));
  const te = (e: TouchEvent) => {
    if (!edge) return;
    const t = e.changedTouches[0];
    if (t.clientX - sx > 80 && Math.abs(t.clientY - sy) < 60) back();
    edge = false;
  };
  const top = $derived<Page | undefined>(nav.stack[nav.stack.length - 1]);
</script>

<main class:wide={nav.wide} ontouchstart={ts} ontouchend={te}>
  {#if ready}
    <div class="tab" class:under={!!top}>
      {#if nav.tab === "home"}<Home />
      {:else if nav.tab === "search"}<Search />
      {:else if nav.tab === "couch"}<Couch />
      {:else if nav.tab === "lists"}<Lists />
      {:else}<Me />{/if}
    </div>
    {#each nav.stack as p, i (i)}
      <div class="sheet" class:under={i < nav.stack.length - 1}>
        {#if p.name === "detail"}<Detail meta={p.meta} />
        {:else if p.name === "member"}<Member id={p.id} title={p.title} />{/if}
      </div>
    {/each}
    <Nav />
    <DeviceBar />
  {:else}
    <div class="boot"><b>midnite<span>™</span> couch</b></div>
  {/if}
  {#if pb.open}<Player />{/if}
  <Toasts />
  {#if gate}<Gate state={gate} recheck={checkGate} />{/if}
</main>

<style>
  main { position: fixed; inset: 0; overflow: hidden; }
  .tab, .sheet { position: absolute; inset: 0; }
  main.wide .tab, main.wide .sheet { left: 252px; }
  main.wide .tab :global(.page), main.wide .sheet :global(.page) { padding-bottom: 40px; }
  .sheet { background: var(--bg); animation: slide 0.38s cubic-bezier(0.2, 0.8, 0.2, 1) both; box-shadow: -20px 0 40px #0008; }
  @keyframes slide { from { transform: translateX(100%); } }
  .under { visibility: hidden; transition: visibility 0s 0.4s; }
  .boot { position: fixed; inset: 0; display: grid; place-items: center; font-size: 24px; font-weight: 900; }
  .boot span { color: var(--neon-soft); font-size: 13px; vertical-align: super; }
</style>
