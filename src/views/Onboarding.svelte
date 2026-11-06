<script lang="ts">
  // First launch: sign in, set the mood of the room, bring your addons, light
  // the room, and you're on the couch. One decision per screen over a slow,
  // dim wall of posters. Every step can be skipped and rerun from Profile.
  import { api, img, type AddonDescriptor, type Meta } from "../lib/api";
  import { app, saveSettings, refreshSecrets } from "../lib/stores/app.svelte";
  import { applyCurrent } from "../lib/stores/theme.svelte";
  import Logo from "../components/Logo.svelte";
  import boil from "../assets/brand/couch-boil.webp";
  import { toast, errorText } from "../lib/stores/toast.svelte";
  import { haptic, isIOS } from "../lib/native";
  import { ui } from "../lib/ui.svelte";
  import Icon from "../components/Icon.svelte";
  import Item from "../ui/Item.svelte";
  import { untrack } from "svelte";
  import SignIn from "../ui/SignIn.svelte";
  import SimklConnect from "../ui/SimklConnect.svelte";
  import SecretField from "../components/SecretField.svelte";

  let { start = 0, ondone }: { start?: number; ondone?: () => void } = $props();

  const desktop = app.platform === "windows" || app.platform === "browser";
  type Step = "welcome" | "signin" | "mood" | "addons" | "services" | "room" | "done";
  const steps: Step[] = ["welcome", "signin", "mood", "addons", "services", "room", "done"];

  let i = $state(untrack(() => start));
  let dir = $state(1);
  const step = $derived(steps[i]);

  function next() {
    haptic("light");
    dir = 1;
    if (i < steps.length - 1) i++;
  }
  function prev() {
    haptic("light");
    dir = -1;
    if (i > 0) i--;
  }
  function finish() {
    haptic("success");
    app.settings.general.onboarded = true;
    saveSettings();
    leaving = true;
    setTimeout(() => ondone?.(), 650);
  }
  let leaving = $state(false);

  // ---------------------------------------------------------- poster wall
  let posters = $state<string[]>([]);
  $effect(() => {
    (async () => {
      const hero: Meta[] = await api.hero().catch(() => []);
      const cats = await api.catalogs().catch(() => []);
      const first = cats.filter((c) => !c.extra.some((e) => e.isRequired)).slice(0, 3);
      const rows = await Promise.all(first.map((c) => api.catalog(c).catch(() => [] as Meta[])));
      const all = [...hero, ...rows.flat()].map((m) => m.poster).filter(Boolean) as string[];
      posters = [...new Set(all)].slice(0, 36);
    })();
  });
  const cols = $derived.by(() => {
    const n = ui.phone ? 4 : 7;
    const out: string[][] = Array.from({ length: n }, () => []);
    const src = posters.length ? posters : [];
    for (let k = 0; k < Math.max(src.length, n * 5); k++) out[k % n].push(src[k % Math.max(1, src.length)] ?? "");
    return out;
  });

  // ----------------------------------------------------------------- mood
  const NEON = [
    { v: "soft", label: "soft", hint: "a lamp in the corner" },
    { v: "lit", label: "lit", hint: "the sign is on" },
    { v: "loud", label: "loud", hint: "full neon" },
  ] as const;
  function setLook(fn: () => void) {
    haptic("selection");
    fn();
    applyCurrent();
    saveSettings();
  }

  // --------------------------------------------------------------- addons
  let addons = $state<AddonDescriptor[]>([]);
  let addonUrl = $state("");
  let adding = $state(false);
  async function loadAddons() {
    addons = (await api.addons().catch(() => [])).filter((a) => !a.flags.protected);
  }
  $effect(() => {
    if (step === "addons") loadAddons();
  });
  async function addAddon(e: Event) {
    e.preventDefault();
    if (!addonUrl.trim()) return;
    adding = true;
    try {
      const d = await api.install(addonUrl.trim());
      toast(`Added ${d.manifest.name}`, "ok");
      addonUrl = "";
      haptic("success");
      loadAddons();
    } catch (err) {
      toast(errorText(err), "error");
    }
    adding = false;
  }

  // ------------------------------------------------------------- services
  let imported = $state<Record<string, boolean> | null>(null);
  async function importOld() {
    try {
      imported = await api.importEnhanced();
      await refreshSecrets();
      const n = Object.keys(imported).length;
      toast(n ? `Brought over ${n} setting${n > 1 ? "s" : ""}` : "Nothing found to bring over", n ? "ok" : "warn");
      if (n) haptic("success");
    } catch (err) {
      toast(errorText(err), "error");
    }
  }
  let moreKeys = $state(false);

  // --------------------------------------------------------------- extras
  let bridges = $state<any[] | null>(null);
  let hueBusy = $state(false);
  async function findHue() {
    hueBusy = true;
    bridges = (await api.hueDiscover().catch(() => [])) ?? [];
    hueBusy = false;
  }
  async function pairHue(ip: string) {
    hueBusy = true;
    try {
      await api.huePair(ip);
      app.settings.hue.bridgeIp = ip;
      app.settings.hue.enabled = true;
      saveSettings();
      await refreshSecrets();
      haptic("success");
    } catch (err) {
      toast(errorText(err), "warn");
    }
    hueBusy = false;
  }
  let tobii = $state<{ status: string; dll?: string } | null>(null);
  $effect(() => {
    if (step === "room" && desktop) api.tobiiStatus().then((t) => (tobii = t)).catch(() => {});
  });

  const signedIn = $derived(!!app.secrets.stremio_auth);
</script>

<div class="onboarding" class:leaving>
  <!-- drifting poster wall -->
  <div class="wall" aria-hidden="true">
    {#each cols as col, c}
      <div class="col" style:--d="{38 + (c % 3) * 9}s" style:--o="{c % 2 ? -50 : 0}%">
        {#each [...col, ...col] as p}
          <div class="tile">{#if p}<img src={img(p, "w342")} alt="" loading="lazy" />{/if}</div>
        {/each}
      </div>
    {/each}
  </div>
  <div class="veil" class:deep={step !== "welcome"}></div>

  <header class="top">
    {#if i > 0 && step !== "done"}
      <button class="orb glass press" onclick={prev} aria-label="Back"><Icon name="back" size={21} /></button>
    {:else}<span></span>{/if}
    <div class="dots" aria-label="Step {i + 1} of {steps.length}">
      {#each steps as _, k}<span class:on={k === i} class:past={k < i}></span>{/each}
    </div>
    {#if step !== "welcome" && step !== "done"}
      <button class="btn plain skip" onclick={next}>skip</button>
    {:else}<span></span>{/if}
  </header>

  {#key step}
    <section class="step" style:--dir={dir}>
      {#if step === "welcome"}
        <div class="welcome">
          <img class="sign" src={boil} alt="" />
          <h1 class="wordmark">midnite<sup>™</sup> couch</h1>
          <p class="bump-text tagline">welcome to the couch.</p>
        </div>
        <div class="foot-actions">
          <button class="btn lg block primary press" onclick={next}>sit down</button>
        </div>
      {:else if step === "signin"}
        <div class="head">
          <span class="badge"><Icon name="personCircle" size={30} /></span>
          <h1 class="large-title">who's on the couch?</h1>
          <p class="callout dim">Sign in with your Stremio account. Your addons, library and Up Next come with you and stay in sync.</p>
        </div>
        <div class="body">
          {#if signedIn}
            <div class="group">
              <Item label={app.secrets.stremioUser?.email ?? "Signed in"} detail="Stremio account" icon="checkCircle" tint="var(--green)" />
            </div>
          {:else}
            <SignIn ondone={next} />
          {/if}
        </div>
        <div class="foot-actions">
          {#if signedIn}<button class="btn lg block primary press" onclick={next}>continue</button>{/if}
        </div>
      {:else if step === "mood"}
        <div class="head">
          <span class="badge"><Icon name="sparkles" size={28} /></span>
          <h1 class="large-title">set the mood</h1>
          <p class="callout dim">How bright is the neon in your room? Change it any time in Settings.</p>
        </div>
        <div class="body narrow">
          <div class="moods">
            {#each NEON as n (n.v)}
              <button class="mood press" class:on={app.settings.general.neon === n.v} data-neon={n.v} onclick={() => setLook(() => (app.settings.general.neon = n.v))}>
                <span class="mood-sign"><Logo size={54} lit /></span>
                <span class="headline">{n.label}</span>
                <span class="caption dim">{n.hint}</span>
              </button>
            {/each}
          </div>
          <div class="group">
            <Item label="Film grain" detail="A whisper of old-TV texture">
              <button class="mini-switch" class:on={app.settings.general.grain} aria-label="Film grain" onclick={() => setLook(() => (app.settings.general.grain = !app.settings.general.grain))}><span></span></button>
            </Item>
            <Item label="Later = cozier" detail="After midnight the room gets warmer and dimmer">
              <button class="mini-switch" class:on={app.settings.general.timeAware} aria-label="Later is cozier" onclick={() => setLook(() => (app.settings.general.timeAware = !app.settings.general.timeAware))}><span></span></button>
            </Item>
          </div>
        </div>
        <div class="foot-actions"><button class="btn lg block primary press" onclick={next}>continue</button></div>
      {:else if step === "addons"}
        <div class="head">
          <span class="badge"><Icon name="addons" size={28} /></span>
          <h1 class="large-title">bring your addons</h1>
          <p class="callout dim">
            {#if addons.length}Here's what came over{signedIn ? " from your account" : ""}. Every Stremio addon works.{:else}Paste an addon link, or install one from any addon website: the install button opens couch.{/if}
          </p>
        </div>
        <div class="body narrow">
          {#if addons.length}
            <div class="group list">
              {#each addons.slice(0, 6) as a (a.transportUrl)}
                <Item label={a.manifest.name} detail={a.manifest.description ?? ""}>
                  {#snippet lead()}
                    <span class="alogo">{#if a.manifest.logo}<img src={img(a.manifest.logo)} alt="" />{:else}<Icon name="addons" size={18} />{/if}</span>
                  {/snippet}
                </Item>
              {/each}
              {#if addons.length > 6}<Item label="and {addons.length - 6} more" />{/if}
            </div>
          {/if}
          <form class="add glass" onsubmit={addAddon}>
            <Icon name="link" size={18} />
            <input bind:value={addonUrl} placeholder="https://…/manifest.json" spellcheck="false" autocapitalize="off" />
            <button class="btn small accent press" disabled={!addonUrl.trim() || adding}>{adding ? "…" : "Add"}</button>
          </form>
          <button class="btn plain" onclick={() => api.openExternal("https://stremio-addons.net")}>Browse community addons</button>
        </div>
        <div class="foot-actions"><button class="btn lg block primary press" onclick={next}>continue</button></div>
      {:else if step === "services"}
        <div class="head">
          <span class="badge"><Icon name="key" size={28} /></span>
          <h1 class="large-title">plug things in</h1>
          <p class="callout dim">All optional. Each one makes the couch a little smarter.</p>
        </div>
        <div class="body narrow">
          {#if app.platform === "windows"}
            <div class="group">
              <Item label="Bring over Stremio Enhanced" detail={imported ? `${Object.keys(imported).length} settings brought over` : "Simkl sign-in, TMDB and TheIntroDB keys, worker URL"} icon="download" tint="var(--indigo)">
                {#if imported}<Icon name="checkCircle" size={22} class="ok" />{:else}<button class="btn small tinted press" onclick={importOld}>Import</button>{/if}
              </Item>
            </div>
          {/if}
          <div class="group">
            <SimklConnect />
            <Item label="More keys" detail="TMDB, vibe search, skip intros" icon="key" tint="var(--gray)" chevron onclick={() => (moreKeys = !moreKeys)} />
            {#if moreKeys}
              <div class="keyrow"><span class="foot dim">TMDB (a shared key works until you add yours)</span><SecretField name="tmdb" placeholder="TMDB v3 key" /></div>
              <div class="keyrow"><span class="foot dim">Anthropic, for mood search</span><SecretField name="anthropic" placeholder="sk-ant-…" /></div>
              <div class="keyrow"><span class="foot dim">TheIntroDB, for skip buttons</span><SecretField name="tidb" placeholder="theintrodb:…" /></div>
            {/if}
          </div>
        </div>
        <div class="foot-actions"><button class="btn lg block primary press" onclick={next}>continue</button></div>
      {:else if step === "room"}
        <div class="head">
          <span class="badge lamp"><Icon name="bulb" size={28} /></span>
          <h1 class="large-title">dim the lights</h1>
          <p class="callout dim">Couch mode fades your Hue lights to a warm lamp when you press play, and brings them back after.</p>
        </div>
        <div class="body narrow">
          <div class="group">
            {#if app.secrets.hue_username}
              <Item label="Philips Hue" detail="Bridge paired. Pick a room in Profile → Philips Hue." icon="bulb" tint="var(--lamp)"><Icon name="checkCircle" size={22} class="ok" /></Item>
            {:else}
              <Item label="Philips Hue" detail={bridges === null ? "Press the button on your bridge, then search" : bridges.length ? "Choose your bridge" : "No bridge found. Add it later by IP address."} icon="bulb" tint="var(--lamp)">
                <button class="btn small tinted press" disabled={hueBusy} onclick={findHue}>{hueBusy ? "Searching…" : "Search"}</button>
              </Item>
              {#each bridges ?? [] as b}
                <Item label={b.name ?? "Hue Bridge"} detail={b.ip}><button class="btn small accent press" disabled={hueBusy} onclick={() => pairHue(b.ip)}>Pair</button></Item>
              {/each}
            {/if}
            {#if desktop}
              <Item
                label="Tobii eye tracker"
                detail={tobii?.dll ? "Found. Turn it on in Settings → Eye tracking." : "Not found. Skip this if you don't have one."}
                icon="eye"
                tint="var(--neon-soft)"
              />
            {/if}
          </div>
        </div>
        <div class="foot-actions"><button class="btn lg block primary press" onclick={next}>continue</button></div>
      {:else}
        <div class="welcome">
          <img class="sign" src={boil} alt="" />
          <p class="bump-text done-line">you're on the couch.</p>
        </div>
        <div class="foot-actions"><button class="btn lg block primary press" onclick={finish}>press play</button></div>
      {/if}
    </section>
  {/key}
</div>

<style>
  .onboarding {
    position: fixed;
    inset: 0;
    z-index: 150;
    background: var(--bg);
    overflow: hidden;
    display: flex;
    flex-direction: column;
    transition:
      opacity 600ms var(--ease-out),
      transform 600ms var(--ease-out);
  }
  .onboarding.leaving {
    opacity: 0;
    transform: scale(1.06);
    pointer-events: none;
  }
  .wall {
    position: absolute;
    inset: -10% -6%;
    display: flex;
    gap: 14px;
    transform: rotate(-8deg) scale(1.15);
    opacity: 0.3;
    filter: saturate(0.7) sepia(0.15);
  }
  .col {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 14px;
    animation: drift var(--d) linear infinite;
    translate: 0 var(--o);
  }
  @keyframes drift {
    to {
      transform: translateY(-50%);
    }
  }
  .tile {
    aspect-ratio: 2 / 3;
    border-radius: 14px;
    overflow: hidden;
    background: var(--fill-4);
    flex: none;
  }
  .tile img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .veil {
    position: absolute;
    inset: 0;
    background:
      radial-gradient(90% 60% at 50% 40%, transparent, color-mix(in srgb, var(--bg) 70%, transparent) 70%),
      linear-gradient(180deg, color-mix(in srgb, var(--bg) 30%, transparent) 0%, color-mix(in srgb, var(--bg) 55%, transparent) 50%, var(--bg) 88%);
    transition: background var(--t-slow) ease, backdrop-filter var(--t-slow) ease;
  }
  .veil.deep {
    background: color-mix(in srgb, var(--bg) 82%, transparent);
    backdrop-filter: blur(30px) saturate(140%);
    -webkit-backdrop-filter: blur(30px) saturate(140%);
  }
  .top {
    position: relative;
    z-index: 2;
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    align-items: center;
    padding: calc(var(--safe-top) + 12px) var(--gutter) 0;
    min-height: calc(var(--safe-top) + 60px);
  }
  .skip {
    justify-self: end;
    color: var(--label-2);
  }
  .dots {
    display: flex;
    gap: 7px;
  }
  .dots span {
    width: 7px;
    height: 7px;
    border-radius: 4px;
    background: var(--fill-1);
    transition:
      width var(--t-med) var(--spring),
      background var(--t-med) ease;
  }
  .dots span.past {
    background: var(--label-3);
  }
  .dots span.on {
    width: 22px;
    background: var(--label);
  }
  .step {
    position: relative;
    z-index: 2;
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 0 var(--gutter) calc(var(--safe-bottom) + 24px);
    overflow-y: auto;
    animation: step-in var(--t-slow) var(--glide) both;
  }
  @keyframes step-in {
    from {
      opacity: 0;
      transform: translateX(calc(var(--dir) * 60px));
    }
  }
  .welcome {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    gap: 10px;
  }
  .sign {
    width: min(280px, 60vw);
    height: auto;
    margin-bottom: 8px;
    animation: rise 900ms var(--glide) both;
  }
  .wordmark {
    margin: 0;
    font-family: var(--font-display);
    font-weight: 800;
    font-size: clamp(40px, 7vw, 64px);
    letter-spacing: -0.04em;
    animation: rise 900ms var(--glide) 900ms both;
  }
  .wordmark sup {
    font-size: 0.32em;
    font-weight: 600;
    vertical-align: 1.6em;
    margin-left: 1px;
    color: var(--text-dim);
  }
  .tagline {
    margin: 4px 0 0;
    color: var(--text-dim);
    animation: rise 900ms var(--glide) 1200ms both;
  }
  .done-line {
    margin: 0;
    font-size: clamp(22px, 3vw, 32px);
    animation: rise 1200ms var(--glide) 200ms both;
  }
  .moods {
    width: 100%;
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 12px;
  }
  .mood {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    padding: 18px 8px 14px;
    border-radius: var(--r-lg);
    background: var(--surface);
    box-shadow: inset 0 0 0 1px var(--separator);
  }
  .mood.on {
    background: var(--surface-2);
    box-shadow: var(--glow-neon);
  }
  .mood-sign {
    height: 70px;
    display: grid;
    place-items: center;
    margin-bottom: 4px;
  }
  .mood[data-neon="soft"] .mood-sign {
    filter: saturate(0.6) brightness(0.75);
  }
  .mood[data-neon="loud"] .mood-sign {
    filter: saturate(1.25) brightness(1.2) drop-shadow(0 0 12px rgb(var(--neon-rgb) / 0.6));
  }
  .head {
    width: 100%;
    max-width: 560px;
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    gap: 10px;
    padding: clamp(12px, 5vh, 56px) 0 26px;
  }
  .head p {
    margin: 0;
    max-width: 440px;
  }
  .badge {
    width: 64px;
    height: 64px;
    border-radius: 18px;
    display: grid;
    place-items: center;
    color: var(--text);
    background: var(--surface-2);
    margin-bottom: 8px;
    box-shadow: inset 0 1px 0 rgb(var(--hi) / 0.12), 0 0 0 1px var(--separator), 0 12px 30px -10px rgb(var(--lo) / 0.8);
  }
  .badge.lamp {
    color: var(--lamp);
    box-shadow: inset 0 1px 0 rgb(var(--hi) / 0.12), 0 0 0 1px var(--separator), var(--glow-lamp);
  }
  .body {
    width: 100%;
    max-width: 560px;
    display: flex;
    flex-direction: column;
    gap: 16px;
    align-items: center;
  }
  .body.narrow {
    max-width: 520px;
  }
  .body > :global(.group) {
    width: 100%;
  }
  .foot-actions {
    width: 100%;
    max-width: 400px;
    margin-top: auto;
    padding-top: 24px;
  }
  .mini-switch {
    width: 51px;
    height: 31px;
    border-radius: 31px;
    background: var(--fill-1);
    position: relative;
    transition: background var(--t-med) ease;
  }
  .mini-switch span {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 27px;
    height: 27px;
    border-radius: 50%;
    background: var(--white);
    box-shadow: 0 3px 8px rgb(var(--lo) / 0.15);
    transition: transform var(--t-med) var(--spring);
  }
  .mini-switch.on {
    background: var(--neon);
  }
  .mini-switch.on span {
    transform: translateX(20px);
  }
  .alogo {
    width: 34px;
    height: 34px;
    border-radius: 9px;
    overflow: hidden;
    display: grid;
    place-items: center;
    background: var(--fill-3);
    flex: none;
  }
  .alogo img {
    width: 100%;
    height: 100%;
    object-fit: contain;
  }
  .list :global(.detail) {
    display: -webkit-box;
    -webkit-line-clamp: 1;
    line-clamp: 1;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .add {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    height: 52px;
    padding: 0 8px 0 16px;
    border-radius: var(--r-pill);
  }
  .add input {
    flex: 1;
    min-width: 0;
    border: 0;
    background: none;
    outline: none;
    font-size: var(--fs-callout);
  }
  .keyrow {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 4px 16px 12px 58px;
  }
  .code {
    font-family: var(--font-round);
    font-weight: 700;
    font-size: 18px;
    letter-spacing: 0.1em;
  }
  :global(.ok) {
    color: var(--green);
  }
</style>
