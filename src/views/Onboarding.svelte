<script lang="ts">
  // First launch. One decision per screen, over a slowly drifting wall of
  // posters. Every step can be skipped and rerun later from Profile.
  import { api, img, type AddonDescriptor, type Meta } from "../lib/api";
  import { app, saveSettings, refreshSecrets } from "../lib/stores/app.svelte";
  import { themes, applyCurrent, loadThemes } from "../lib/stores/theme.svelte";
  import { toast, errorText } from "../lib/stores/toast.svelte";
  import { haptic, isIOS } from "../lib/native";
  import { ui } from "../lib/ui.svelte";
  import Icon from "../components/Icon.svelte";
  import Item from "../ui/Item.svelte";
  import { untrack } from "svelte";
  import SignIn from "../ui/SignIn.svelte";
  import ThemeCard from "../ui/ThemeCard.svelte";
  import SimklConnect from "../ui/SimklConnect.svelte";
  import SecretField from "../components/SecretField.svelte";

  let { start = 0, ondone }: { start?: number; ondone?: () => void } = $props();

  const desktop = app.platform === "windows" || app.platform === "browser";
  type Step = "welcome" | "signin" | "theme" | "addons" | "services" | "extras" | "done";
  const steps: Step[] = ["welcome", "signin", "theme", "addons", "services", ...(desktop ? (["extras"] as Step[]) : []), "done"];

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

  // ---------------------------------------------------------------- theme
  $effect(() => {
    loadThemes();
  });
  function pickTheme(id: string) {
    haptic("selection");
    app.settings.general.theme = id;
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
    if (step === "extras") api.tobiiStatus().then((t) => (tobii = t)).catch(() => {});
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
      <button class="btn plain skip" onclick={next}>Skip</button>
    {:else}<span></span>{/if}
  </header>

  {#key step}
    <section class="step" style:--dir={dir}>
      {#if step === "welcome"}
        <div class="welcome">
          <span class="sun"></span>
          <h1 class="xl-title">Lumen</h1>
          <p class="title3 dim tagline">Everything you watch,<br />beautifully lit.</p>
        </div>
        <div class="foot-actions">
          <button class="btn lg block primary press" onclick={next}>Get started</button>
        </div>
      {:else if step === "signin"}
        <div class="head">
          <span class="badge" style:background="linear-gradient(140deg, #8a5cf6, #4f46e5)"><Icon name="personCircle" size={30} /></span>
          <h1 class="large-title">Sign in to Stremio</h1>
          <p class="callout dim">Your addons, library and Continue Watching come with you, and stay in sync with your other devices.</p>
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
          {#if signedIn}<button class="btn lg block primary press" onclick={next}>Continue</button>{/if}
        </div>
      {:else if step === "theme"}
        <div class="head">
          <span class="badge" style:background="linear-gradient(140deg, #ff9f0a, #ff375f)"><Icon name="palette" size={28} /></span>
          <h1 class="large-title">Pick a look</h1>
          <p class="callout dim">Everything changes as you tap. Add your own themes later from Profile.</p>
        </div>
        <div class="themes scroll-x">
          {#each themes.list as t (t.id)}
            <ThemeCard theme={t} on={app.settings.general.theme === t.id} onclick={() => pickTheme(t.id)} />
          {/each}
        </div>
        <div class="body narrow">
          <div class="group">
            <Item label="Colour from artwork" detail="The accent follows whatever you're looking at">
              <button class="mini-switch" class:on={app.settings.general.accentFromArtwork} aria-label="Colour from artwork" onclick={() => { app.settings.general.accentFromArtwork = !app.settings.general.accentFromArtwork; applyCurrent(); saveSettings(); haptic("light"); }}><span></span></button>
            </Item>
          </div>
        </div>
        <div class="foot-actions"><button class="btn lg block primary press" onclick={next}>Continue</button></div>
      {:else if step === "addons"}
        <div class="head">
          <span class="badge" style:background="linear-gradient(140deg, #30d158, #0a84ff)"><Icon name="addons" size={28} /></span>
          <h1 class="large-title">Your addons</h1>
          <p class="callout dim">
            {#if addons.length}Here's what came over{signedIn ? " from your account" : ""}. Every Stremio addon works.{:else}Paste an addon link, or install one from any addon website: the install button opens Lumen.{/if}
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
        <div class="foot-actions"><button class="btn lg block primary press" onclick={next}>Continue</button></div>
      {:else if step === "services"}
        <div class="head">
          <span class="badge" style:background="linear-gradient(140deg, #bf5af2, #5e5ce6)"><Icon name="sparkles" size={28} /></span>
          <h1 class="large-title">Connect services</h1>
          <p class="callout dim">All optional. Each one makes Lumen a little smarter.</p>
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
        <div class="foot-actions"><button class="btn lg block primary press" onclick={next}>Continue</button></div>
      {:else if step === "extras"}
        <div class="head">
          <span class="badge" style:background="linear-gradient(140deg, #ffd60a, #ff9f0a)"><Icon name="bulb" size={28} /></span>
          <h1 class="large-title">Your room</h1>
          <p class="callout dim">Lights that follow the picture, and a player that knows when you look away.</p>
        </div>
        <div class="body narrow">
          <div class="group">
            {#if app.secrets.hue_username}
              <Item label="Philips Hue" detail="Bridge paired. Pick a room in Profile → Philips Hue." icon="bulb" tint="var(--yellow)"><Icon name="checkCircle" size={22} class="ok" /></Item>
            {:else}
              <Item label="Philips Hue" detail={bridges === null ? "Press the button on your bridge, then search" : bridges.length ? "Choose your bridge" : "No bridge found. Add it later by IP address."} icon="bulb" tint="var(--yellow)">
                <button class="btn small tinted press" disabled={hueBusy} onclick={findHue}>{hueBusy ? "Searching…" : "Search"}</button>
              </Item>
              {#each bridges ?? [] as b}
                <Item label={b.name ?? "Hue Bridge"} detail={b.ip}><button class="btn small accent press" disabled={hueBusy} onclick={() => pairHue(b.ip)}>Pair</button></Item>
              {/each}
            {/if}
            <Item
              label="Tobii eye tracker"
              detail={tobii?.dll ? "Found. Turn it on in Profile → Eye tracking." : "Not found. Skip this if you don't have one."}
              icon="eye"
              tint="var(--green)"
            />
          </div>
        </div>
        <div class="foot-actions"><button class="btn lg block primary press" onclick={next}>Continue</button></div>
      {:else}
        <div class="welcome">
          <span class="tickbig"><Icon name="check" size={40} /></span>
          <h1 class="large-title">You're all set</h1>
          <p class="callout dim tagline">Long-press any poster for quick actions.<br />Change anything later in Profile.</p>
        </div>
        <div class="foot-actions"><button class="btn lg block primary press" onclick={finish}>Start watching</button></div>
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
    opacity: 0.55;
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
  .welcome .xl-title {
    font-size: clamp(52px, 9vw, 84px);
    animation: rise 900ms var(--glide) 200ms both;
  }
  .tagline {
    margin: 0;
    font-weight: 500;
    animation: rise 900ms var(--glide) 380ms both;
  }
  .sun {
    width: 84px;
    height: 84px;
    border-radius: 50%;
    background: radial-gradient(circle at 50% 38%, #fff4cc, #ffc22e 55%, #e4736a);
    box-shadow:
      0 0 80px 14px rgb(255 194 46 / 0.4),
      0 0 200px 40px rgb(228 115 106 / 0.18);
    margin-bottom: 18px;
    animation: sunrise 1400ms var(--spring) both;
  }
  @keyframes sunrise {
    from {
      transform: translateY(60px) scale(0.6);
      opacity: 0;
    }
  }
  .tickbig {
    width: 88px;
    height: 88px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: var(--green);
    color: #fff;
    margin-bottom: 14px;
    box-shadow: 0 0 60px rgb(48 209 88 / 0.35);
    animation: sunrise 900ms var(--spring) both;
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
    color: #fff;
    margin-bottom: 8px;
    box-shadow: inset 0 1px 0 rgb(255 255 255 / 0.3), 0 12px 30px -10px rgb(0 0 0 / 0.5);
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
  .themes {
    display: flex;
    gap: 14px;
    width: calc(100% + 2 * var(--gutter));
    margin: 0 calc(-1 * var(--gutter)) 18px;
    padding: 6px var(--gutter) 12px;
    scroll-snap-type: x mandatory;
    justify-content: safe center;
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
    background: #fff;
    box-shadow: 0 3px 8px rgb(0 0 0 / 0.15);
    transition: transform var(--t-med) var(--spring);
  }
  .mini-switch.on {
    background: var(--green);
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
