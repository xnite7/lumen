<script lang="ts">
  import { api } from "../lib/api";
  import { app, saveSettings, refreshSecrets } from "../lib/stores/app.svelte";
  import { themes, loadThemes, applyCurrent } from "../lib/stores/theme.svelte";
  import { themeTemplate, BUILTIN_THEMES } from "../lib/themes";
  import { toast, errorText } from "../lib/stores/toast.svelte";
  import { gaze } from "../lib/stores/gaze.svelte";
  import { bytes, lang } from "../lib/format";
  import Controls from "../components/Controls.svelte";
  import Cell from "../components/Cell.svelte";
  import SecretField from "../components/SecretField.svelte";
  import Icon from "../components/Icon.svelte";
  import NavBar from "../ui/NavBar.svelte";
  import SignIn from "../ui/SignIn.svelte";
  import { SECTIONS, sectionById } from "../lib/sections";

  let { section = "appearance" }: { section?: string } = $props();
  const meta = $derived(sectionById(section) ?? SECTIONS[0]);

  const s = app.settings;
  function changed() {
    saveSettings();
    applyCurrent();
  }

  $effect(() => {
    loadThemes();
  });

  // ------------------------------------------------------------ accounts
  let pin = $state<{ user_code: string; verification_url: string } | null>(null);
  let pinTimer: ReturnType<typeof setInterval> | undefined;
  async function simklConnect() {
    try {
      pin = await api.simklPinStart();
      if (pin?.verification_url) api.openExternal(pin.verification_url);
      clearInterval(pinTimer);
      pinTimer = setInterval(async () => {
        if (!pin) return clearInterval(pinTimer);
        try {
          if (await api.simklPinPoll(pin.user_code)) {
            clearInterval(pinTimer);
            pin = null;
            await refreshSecrets();
            toast("Simkl connected", "ok");
          }
        } catch {
          /* keep polling */
        }
      }, 5000);
    } catch (e) {
      toast(errorText(e), "error");
    }
  }

  async function importOld() {
    try {
      const r = await api.importEnhanced();
      await refreshSecrets();
      const n = Object.keys(r).length;
      toast(n ? `Imported ${n} setting${n > 1 ? "s" : ""} from Stremio Enhanced` : "Nothing to import found", n ? "ok" : "warn");
    } catch (e) {
      toast(errorText(e), "error");
    }
  }

  // ----------------------------------------------------------------- hue
  let bridges = $state<any[]>([]);
  let hueInfo = $state<any>(null);
  let hueBusy = $state(false);
  let manualIp = $state("");
  async function discover() {
    hueBusy = true;
    bridges = (await api.hueDiscover().catch(() => [])) ?? [];
    hueBusy = false;
    if (!bridges.length) toast("No bridge found automatically. Enter its IP address instead.", "warn");
  }
  async function pair(ip: string) {
    hueBusy = true;
    try {
      await api.huePair(ip);
      s.hue.bridgeIp = ip;
      s.hue.enabled = true;
      changed();
      await refreshSecrets();
      toast("Bridge paired", "ok");
      loadHue();
    } catch (e) {
      toast(errorText(e), "warn");
    }
    hueBusy = false;
  }
  async function loadHue() {
    try {
      hueInfo = await api.hueOverview();
    } catch {
      hueInfo = null;
    }
  }
  $effect(() => {
    if (section === "hue" && app.secrets.hue_username) loadHue();
  });
  let testing = $state(false);
  async function testSync() {
    testing = !testing;
    await api.hueTest(testing).catch((e) => toast(errorText(e), "error"));
  }

  // --------------------------------------------------------------- tobii
  let tobii = $state<{ status: string; dll?: string }>({ status: "off" });
  $effect(() => {
    if (section === "tobii") api.tobiiStatus().then((t) => (tobii = t));
  });

  // ------------------------------------------------------------- storage
  let cache = $state<{ images: number } | null>(null);
  $effect(() => {
    if (section === "storage") api.cacheInfo().then((c) => (cache = c));
  });

  // ------------------------------------------------------------ home rows
  let rows = $state<any[]>([]);
  $effect(() => {
    if (section === "home") api.catalogs().then((c) => (rows = c.filter((x) => !x.extra.some((e) => e.isRequired && e.name === "search"))));
  });
  function toggleRow(key: string) {
    const h = s.home.hiddenRows;
    s.home.hiddenRows = h.includes(key) ? h.filter((k) => k !== key) : [...h, key];
    saveSettings();
  }
  function moveRow(key: string, dir: number) {
    const keys = s.home.rowOrder.length ? [...s.home.rowOrder] : rows.map((r) => r.key);
    for (const r of rows) if (!keys.includes(r.key)) keys.push(r.key);
    const i = keys.indexOf(key);
    const j = i + dir;
    if (j < 0 || j >= keys.length) return;
    [keys[i], keys[j]] = [keys[j], keys[i]];
    s.home.rowOrder = keys;
    saveSettings();
  }
  const orderedRows = $derived.by(() => {
    const order = new Map(s.home.rowOrder.map((k, i) => [k, i]));
    return rows.map((r, i) => ({ r, i })).sort((a, b) => (order.get(a.r.key) ?? 1000 + a.i) - (order.get(b.r.key) ?? 1000 + b.i)).map((x) => x.r);
  });

  async function newTheme() {
    const base = themes.list.find((t) => t.id === s.general.theme) ?? BUILTIN_THEMES[0];
    const t = themeTemplate(base);
    t.id = `my-theme-${Date.now().toString(36)}`;
    await api.themeSave(t);
    await api.themesOpenDir();
    await loadThemes();
    toast("Theme file created in your themes folder. Edit it and press Reload.", "ok");
  }

  const langOptions = ["eng", "ara", "jpn", "spa", "fre", "ger", "ita", "por", "kor", "chi", "hin", "tur", "rus"];
  function toggleLang(list: string[], l: string) {
    return list.includes(l) ? list.filter((x) => x !== l) : [...list, l];
  }
</script>

<NavBar title={meta.label} />

<div class="settings">
  <h1 class="large-title">{meta.label}</h1>
  {#key section}
      <div class="inner">
        {#if section === "appearance"}
          <div class="themes">
            {#each themes.list as t (t.id)}
              <button class="theme press" class:on={s.general.theme === t.id} onclick={() => { s.general.theme = t.id; changed(); }}>
                <span class="sw">
                  {#each t.preview ?? [t.tokens.ink ?? "#0e1220", t.tokens.text ?? "#f4f6fb", t.tokens.accent ?? "#ffc22e"] as c}<i style:background={c}></i>{/each}
                </span>
                <span class="tn">{t.name}</span>
                <span class="foot faint">{t.user ? "Your theme" : t.author}</span>
              </button>
            {/each}
          </div>
          {#if app.platform !== "ios"}<div class="group">
            <Cell label="Create a theme" hint="Copies the current theme into a file you can edit. Themes are plain JSON plus optional CSS.">
              <button class="btn tinted small press" onclick={newTheme}>New theme</button>
              <button class="btn tinted small press" onclick={() => api.themesOpenDir()}>Open folder</button>
              <button class="btn tinted small icon press" onclick={async () => { await loadThemes(); applyCurrent(); }} aria-label="Reload themes"><Icon name="refresh" size={15} /></button>
            </Cell>
          </div>{/if}
          <div class="group">
            <Cell label="Colour from artwork" hint="The accent and the glow take their colour from what you're looking at."><Controls kind="switch" bind:value={s.general.accentFromArtwork} onchange={changed} /></Cell>
            <Cell label="Ambient glow"><Controls kind="switch" bind:value={s.general.ambientGlow} onchange={changed} /></Cell>
            <Cell label="Film grain" hint="Faint TV grain. Also removes banding in dark gradients."><Controls kind="switch" bind:value={s.general.grain} onchange={changed} /></Cell>
            <Cell label="Glass refraction" hint="Glass edges bend what's behind them. Costs a little GPU."><Controls kind="switch" bind:value={s.general.refraction} onchange={changed} /></Cell>
            <Cell label="Motion" hint={s.general.motionScale === 0 ? "Off" : s.general.motionScale < 0.8 ? "Quick" : s.general.motionScale > 1.15 ? "Floaty" : "Normal"}>
              <div style="width:220px"><Controls kind="slider" bind:value={s.general.motionScale} min={0} max={1.5} step={0.05} onchange={changed} label="Motion" /></div>
            </Cell>
          </div>
          <div class="group">
            <Cell label="Hover previews" hint="Rest the pointer on a poster to see its trailer."><Controls kind="switch" bind:value={s.general.hoverPreviews} onchange={changed} /></Cell>
            <Cell label="Hover delay" hint="{(s.general.hoverDelayMs / 1000).toFixed(1)} s of stillness">
              <div style="width:220px"><Controls kind="slider" bind:value={s.general.hoverDelayMs} min={300} max={3000} step={100} onchange={changed} label="Hover delay" /></div>
            </Cell>
            <Cell label="Previews start muted"><Controls kind="switch" bind:value={s.general.previewMuted} onchange={changed} /></Cell>
            <Cell label="Hero trailers" hint="The banner plays its trailer after a few seconds."><Controls kind="switch" bind:value={s.general.heroTrailers} onchange={changed} /></Cell>
            <Cell label="Hero starts muted"><Controls kind="switch" bind:value={s.general.heroMuted} onchange={changed} /></Cell>
            <Cell label="Titles under posters"><Controls kind="switch" bind:value={s.general.showTitlesUnderPosters} onchange={changed} /></Cell>
            <Cell label="Top 10 numbers"><Controls kind="switch" bind:value={s.general.topTenRanks} onchange={changed} /></Cell>
            <Cell label="Hide adult content" hint="Adult rows and results stay blurred until you choose Show."><Controls kind="switch" bind:value={s.general.hideAdult} onchange={changed} /></Cell>
          </div>
        {:else if section === "playback"}
          <div class="group">
            <Cell label="Play next episode automatically"><Controls kind="switch" bind:value={s.playback.autoplayNext} onchange={changed} /></Cell>
            <Cell label="Keep the same source" hint="The next episode plays from the same addon, quality and release group, without the stream list."><Controls kind="switch" bind:value={s.playback.bingeSameSource} onchange={changed} /></Cell>
            <Cell label="Up next appears" hint="{s.playback.nextCardSecs} s before the end, or when the credits start">
              <div style="width:200px"><Controls kind="slider" bind:value={s.playback.nextCardSecs} min={10} max={120} step={5} onchange={changed} label="Up next timing" /></div>
            </Cell>
          </div>
          <div class="group">
            <Cell label="Skip buttons" hint="From TheIntroDB, when segments are known for the episode."><Controls kind="switch" bind:value={s.playback.showSkipButtons} onchange={changed} /></Cell>
            <Cell label="Skip intros automatically"><Controls kind="switch" bind:value={s.playback.autoSkipIntro} onchange={changed} /></Cell>
            <Cell label="Skip recaps automatically"><Controls kind="switch" bind:value={s.playback.autoSkipRecap} onchange={changed} /></Cell>
            <Cell label="Skip credits automatically"><Controls kind="switch" bind:value={s.playback.autoSkipCredits} onchange={changed} /></Cell>
          </div>
          <div class="group">
            <Cell label="Audio language" stack>
              <div class="langs">{#each langOptions as l}<button class="chip press" class:sel={s.playback.audioLanguages.includes(l)} onclick={() => { s.playback.audioLanguages = toggleLang(s.playback.audioLanguages, l); changed(); }}>{lang(l)}</button>{/each}</div>
            </Cell>
            <Cell label="Default picture" hint="How video fills your screen. Fill crops 16:9 to cover a 21:9 display.">
              <Controls kind="segmented" bind:value={s.playback.aspect} options={[{ value: "fit", label: "Fit" }, { value: "fill", label: "Fill" }, { value: "stretch", label: "Stretch" }]} onchange={changed} />
            </Cell>
            <Cell label="Seek step" hint="Arrow keys: {s.playback.seekSmall} s · Shift+arrow: {s.playback.seekLarge} s">
              <Controls kind="segmented" bind:value={s.playback.seekSmall} options={[{ value: 5, label: "5 s" }, { value: 10, label: "10 s" }, { value: 15, label: "15 s" }, { value: 30, label: "30 s" }]} onchange={changed} />
            </Cell>
            <Cell label="Volume boost" hint="Allows up to 200% for quiet mixes."><Controls kind="switch" bind:value={s.playback.volumeBoost} onchange={changed} /></Cell>
            <Cell label="Count as watched at" hint="{s.playback.watchedThreshold}% (Simkl needs at least 80%)">
              <div style="width:200px"><Controls kind="slider" bind:value={s.playback.watchedThreshold} min={80} max={98} step={1} onchange={changed} label="Watched threshold" /></div>
            </Cell>
          </div>
          <h3 class="group-head">Video engine</h3>
          <div class="group">
            <Cell label="Video inside Lumen" hint="Turn off if video doesn't appear: it will open in its own window instead. Restart Lumen after changing."><Controls kind="switch" bind:value={s.playback.embedVideo} onchange={changed} /></Cell>
            <Cell label="Hardware decoding"><Controls kind="segmented" bind:value={s.playback.hwdec} options={[{ value: "auto-safe", label: "Auto" }, { value: "nvdec", label: "NVDEC" }, { value: "d3d11va", label: "D3D11" }, { value: "no", label: "Off" }]} onchange={changed} /></Cell>
            <Cell label="Renderer"><Controls kind="segmented" bind:value={s.playback.vo} options={[{ value: "gpu-next", label: "gpu-next" }, { value: "gpu", label: "gpu" }]} onchange={changed} /></Cell>
            <Cell label="HDR passthrough" hint="Sends HDR to an HDR display instead of tone-mapping it."><Controls kind="switch" bind:value={s.playback.hdrPassthrough} onchange={changed} /></Cell>
            <Cell label="Debanding" hint="Smooths banding in skies and dark scenes."><Controls kind="switch" bind:value={s.playback.deband} onchange={changed} /></Cell>
            <Cell label="Smooth motion" hint="Frame interpolation synced to your refresh rate."><Controls kind="switch" bind:value={s.playback.interpolation} onchange={changed} /></Cell>
            <Cell label="Buffer" hint="{s.playback.cacheMb} MB ahead, up to {s.playback.readaheadSecs} s">
              <div style="width:200px"><Controls kind="slider" bind:value={s.playback.cacheMb} min={100} max={2000} step={50} onchange={changed} label="Buffer size" /></div>
            </Cell>
          </div>
          <p class="foot faint note">Engine changes apply to the next video you open.</p>
        {:else if section === "subtitles"}
          <div class="preview-sub">
            <span class="cue" style:font-size="{28 * s.subtitles.size}px" style:font-weight={s.subtitles.weight} style:color={s.subtitles.color} data-bg={s.subtitles.background} style:--bg-o={s.subtitles.bgOpacity} style:--blur="{s.subtitles.blur}px">
              {#if s.subtitles.styleSdh && !s.subtitles.hideSdh}<em>(soft static)</em> {/if}The porch light's been on all summer.
            </span>
          </div>
          <div class="group">
            <Cell label="Show subtitles by default"><Controls kind="switch" bind:value={s.subtitles.enabledByDefault} onchange={changed} /></Cell>
            <Cell label="Languages" hint="In order of preference" stack>
              <div class="langs">{#each langOptions as l}<button class="chip press" class:sel={s.subtitles.languages.includes(l)} onclick={() => { s.subtitles.languages = toggleLang(s.subtitles.languages, l); changed(); }}>{lang(l)}</button>{/each}</div>
            </Cell>
          </div>
          <div class="group">
            <Cell label="Size"><div style="width:220px"><Controls kind="slider" bind:value={s.subtitles.size} min={0.6} max={1.8} step={0.05} onchange={changed} label="Size" /></div></Cell>
            <Cell label="Weight"><Controls kind="segmented" bind:value={s.subtitles.weight} options={[{ value: 400, label: "Regular" }, { value: 600, label: "Semibold" }, { value: 750, label: "Bold" }]} onchange={changed} /></Cell>
            <Cell label="Colour">
              <div class="swatches">{#each ["#ffffff", "#fff3b0", "#ffe9a8", "#bde0ff", "#c8f7c5"] as c}<button class="swatch" class:on={s.subtitles.color === c} style:background={c} onclick={() => { s.subtitles.color = c; changed(); }} aria-label={c}></button>{/each}</div>
            </Cell>
            <Cell label="Background"><Controls kind="segmented" bind:value={s.subtitles.background} options={[{ value: "glass", label: "Glass" }, { value: "box", label: "Box" }, { value: "shadow", label: "Shadow" }, { value: "outline", label: "Outline" }]} onchange={changed} /></Cell>
            {#if s.subtitles.background === "glass" || s.subtitles.background === "box"}
              <Cell label="Background opacity"><div style="width:200px"><Controls kind="slider" bind:value={s.subtitles.bgOpacity} min={0} max={0.9} step={0.05} onchange={changed} label="Opacity" /></div></Cell>
            {/if}
            {#if s.subtitles.background === "glass"}
              <Cell label="Blur"><div style="width:200px"><Controls kind="slider" bind:value={s.subtitles.blur} min={0} max={30} step={1} onchange={changed} label="Blur" /></div></Cell>
            {/if}
            <Cell label="Height from bottom"><div style="width:200px"><Controls kind="slider" bind:value={s.subtitles.bottomOffset} min={2} max={25} step={0.5} onchange={changed} label="Offset" /></div></Cell>
            <Cell label="Animate lines"><Controls kind="switch" bind:value={s.subtitles.animateCues} onchange={changed} /></Cell>
          </div>
          <div class="group">
            <Cell label="Style sound descriptions" hint="(door slams), [music] and speaker labels are set apart in italics."><Controls kind="switch" bind:value={s.subtitles.styleSdh} onchange={changed} /></Cell>
            <Cell label="Hide sound descriptions"><Controls kind="switch" bind:value={s.subtitles.hideSdh} onchange={changed} /></Cell>
            <Cell label="Fix right-to-left text" hint="Correct punctuation and direction for Arabic."><Controls kind="switch" bind:value={s.subtitles.rtlFix} onchange={changed} /></Cell>
            <Cell label="Native rendering for styled anime subtitles" hint="Typeset ASS tracks are drawn by the video engine exactly as authored; everything else uses the styles above."><Controls kind="switch" bind:value={s.playback.nativeAss} onchange={changed} /></Cell>
          </div>
        {:else if section === "streams"}
          <div class="group">
            <Cell label="Sort by"><Controls kind="segmented" bind:value={s.streams.sort} options={[{ value: "smart", label: "Best" }, { value: "quality", label: "Quality" }, { value: "size", label: "Size" }, { value: "seeders", label: "Seeders" }, { value: "addon", label: "Addon" }]} onchange={changed} /></Cell>
            <Cell label="Group by resolution"><Controls kind="switch" bind:value={s.streams.groupByQuality} onchange={changed} /></Cell>
            <Cell label="Hide cam recordings"><Controls kind="switch" bind:value={s.streams.hideCam} onchange={changed} /></Cell>
            <Cell label="Hide 3D releases"><Controls kind="switch" bind:value={s.streams.hide3d} onchange={changed} /></Cell>
            <Cell label="Largest file" hint={s.streams.maxSizeGb ? `${s.streams.maxSizeGb} GB` : "No limit"}>
              <div style="width:200px"><Controls kind="slider" bind:value={s.streams.maxSizeGb} min={0} max={100} step={1} onchange={changed} label="Max size" /></div>
            </Cell>
            <Cell label="Play the best stream immediately" hint="Skips the list. You can still pick manually from the player."><Controls kind="switch" bind:value={s.streams.autoplayBest} onchange={changed} /></Cell>
          </div>
        {:else if section === "accounts"}
          {#if app.platform === "windows"}
            <div class="group">
              <Cell label="Bring keys over from Stremio Enhanced" hint="Copies your Simkl sign-in, TMDB and TheIntroDB keys and worker URL from the old app.">
                <button class="btn tinted small press" onclick={importOld}>Import</button>
              </Cell>
            </div>
          {/if}
          <h3 class="group-head">Stremio</h3>
          <div class="group">
            {#if app.secrets.stremio_auth}
              <Cell label={app.secrets.stremioUser?.email ?? "Signed in"} hint="Addons and Continue Watching stay in sync with Stremio on your phone.">
                <button class="btn tinted small press" onclick={async () => { await api.stremioSync(true); toast("Synced", "ok"); }}>Sync now</button>
                <button class="btn tinted small press" onclick={async () => { await api.stremioLogout(); await refreshSecrets(); }}>Sign out</button>
              </Cell>
              <Cell label="Sync automatically"><Controls kind="switch" bind:value={s.integrations.stremioSync} onchange={changed} /></Cell>
            {:else}
              <div class="signin"><SignIn /></div>
            {/if}
          </div>
          <h3 class="group-head">Simkl</h3>
          <div class="group">
            <Cell label="Client ID" hint="From your app at simkl.com/settings/developer."><SecretField name="simkl_client_id" placeholder="Simkl client id" /></Cell>
            {#if app.secrets.simkl_token}
              <Cell label={app.secrets.simklUser?.user?.name ? `Connected as ${app.secrets.simklUser.user.name}` : "Connected"}>
                <button class="btn tinted small press" onclick={async () => { await api.simklLogout(); await refreshSecrets(); }}>Disconnect</button>
              </Cell>
            {:else if pin}
              <Cell label="Enter this code at simkl.com/pin" hint="Lumen connects as soon as you approve it.">
                <span class="code">{pin.user_code}</span>
              </Cell>
            {:else}
              <Cell label="Connect" hint="Or paste an existing access token below.">
                <button class="btn accent small press" disabled={!app.secrets.simkl_client_id} onclick={simklConnect}>Connect Simkl</button>
              </Cell>
              <Cell label="Access token"><SecretField name="simkl_token" placeholder="Existing token" /></Cell>
            {/if}
            <Cell label="Scrobble what you watch"><Controls kind="switch" bind:value={s.integrations.simklScrobble} onchange={changed} /></Cell>
            <Cell label="Library is your Plan to Watch"><Controls kind="switch" bind:value={s.integrations.simklSyncWatchlist} onchange={changed} /></Cell>
          </div>
        {:else if section === "integrations"}
          <div class="group">
            <Cell label="Built-in rows" hint="Simkl lists, TMDB browse rows, Top 10 and richer artwork, without the worker."><Controls kind="switch" bind:value={s.integrations.builtinCatalogs} onchange={changed} /></Cell>
            <Cell label="TMDB key" hint="Cast, collections, recommendations and trailers. A shared key is used until you add your own."><SecretField name="tmdb" placeholder="TMDB v3 key" /></Cell>
            <Cell label="Official trailers from KinoCheck"><Controls kind="switch" bind:value={s.integrations.kinocheckTrailers} onchange={changed} /></Cell>
          </div>
          <h3 class="group-head">Vibe search</h3>
          <div class="group">
            <Cell label="Mood search on Home" hint="Ask for a feeling; picks come from your own Simkl ratings."><Controls kind="switch" bind:value={s.integrations.aiEnabled} onchange={changed} /></Cell>
            <Cell label="Anthropic API key" hint="Sent only to Anthropic, straight from this computer."><SecretField name="anthropic" placeholder="sk-ant-…" /></Cell>
            <Cell label="Model"><input class="field small" bind:value={s.integrations.aiModel} onchange={changed} /></Cell>
            <Cell label="Fast mode when available"><Controls kind="switch" bind:value={s.integrations.aiFast} onchange={changed} /></Cell>
          </div>
          <h3 class="group-head">Skip intros</h3>
          <div class="group">
            <Cell label="TheIntroDB"><Controls kind="switch" bind:value={s.integrations.tidbEnabled} onchange={changed} /></Cell>
            <Cell label="API key"><SecretField name="tidb" placeholder="theintrodb:…" /></Cell>
          </div>
          <h3 class="group-head">Movie of the Night</h3>
          <div class="group">
            <Cell label="API key" hint="Wide Continue Watching art and Top 10 rows. 500 requests a month, so everything is cached."><SecretField name="motn" placeholder="MOTN key" /></Cell>
            <Cell label="Or your worker" hint="Your Simkl worker's URL, which already holds the key."><SecretField name="art_worker" placeholder="https://…workers.dev" /></Cell>
            <Cell label="Country"><input class="field small" bind:value={s.integrations.motnCountry} onchange={changed} maxlength="2" /></Cell>
            <Cell label="Top 10 service"><input class="field small" bind:value={s.integrations.motnService} onchange={changed} /></Cell>
          </div>
        {:else if section === "hue"}
          <div class="group">
            <Cell label="Use Hue" hint={app.secrets.hue_username ? `Bridge at ${s.hue.bridgeIp}` : "Pair your bridge to begin."}><Controls kind="switch" bind:value={s.hue.enabled} onchange={changed} /></Cell>
            {#if !app.secrets.hue_username}
              <Cell label="Find bridge" hint="Press the round button on the bridge, then choose it here within 30 seconds.">
                <button class="btn tinted small press" onclick={discover} disabled={hueBusy}>{hueBusy ? "Searching…" : "Search"}</button>
              </Cell>
              {#each bridges as b}
                <Cell label={b.name ?? "Hue Bridge"} hint={b.ip}><button class="btn accent small press" onclick={() => pair(b.ip)} disabled={hueBusy}>Pair</button></Cell>
              {/each}
              <Cell label="Or by IP address">
                <input class="field small" bind:value={manualIp} placeholder="192.168.1.20" />
                <button class="btn tinted small press" disabled={!manualIp || hueBusy} onclick={() => pair(manualIp)}>Pair</button>
              </Cell>
            {/if}
          </div>
          {#if app.secrets.hue_username}
            <div class="group">
              <Cell label="During playback">
                <Controls kind="segmented" bind:value={s.hue.playbackMode} options={[{ value: "off", label: "Nothing" }, { value: "dim", label: "Dim" }, { value: "ambilight", label: "Match the picture" }]} onchange={changed} />
              </Cell>
              <Cell label="Room" hint="Dimmed, restored and tinted by artwork.">
                <select class="field small" bind:value={s.hue.roomId} onchange={changed}>
                  <option value="">Choose…</option>
                  {#each hueInfo?.groups ?? [] as g}<option value={g.id}>{g.name}</option>{/each}
                </select>
              </Cell>
              {#if s.hue.playbackMode === "ambilight"}
                <Cell label="Entertainment area" hint="Set one up in the Hue app; light positions decide which part of the picture each light shows.">
                  <select class="field small" bind:value={s.hue.areaId} onchange={changed}>
                    <option value="">Choose…</option>
                    {#each hueInfo?.areas ?? [] as a}<option value={a.id}>{a.name} ({a.channels} lights)</option>{/each}
                  </select>
                </Cell>
                <Cell label="Brightness"><div style="width:200px"><Controls kind="slider" bind:value={s.hue.syncBrightness} min={10} max={100} step={1} onchange={changed} label="Brightness" /></div></Cell>
                <Cell label="Colour intensity"><div style="width:200px"><Controls kind="slider" bind:value={s.hue.syncSaturation} min={0.6} max={2} step={0.05} onchange={changed} label="Saturation" /></div></Cell>
                <Cell label="Smoothness" hint="Higher is calmer; lower follows cuts instantly."><div style="width:200px"><Controls kind="slider" bind:value={s.hue.syncSmoothing} min={0} max={0.95} step={0.05} onchange={changed} label="Smoothing" /></div></Cell>
                <Cell label="Test">
                  <button class="btn small press" class:accent={testing} class:tinted={!testing} onclick={testSync}>{testing ? "Stop" : "Start sync"}</button>
                </Cell>
              {:else if s.hue.playbackMode === "dim"}
                <Cell label="Dim to"><div style="width:200px"><Controls kind="slider" bind:value={s.hue.dimBrightness} min={0} max={60} step={1} onchange={changed} label="Dim level" /></div></Cell>
              {/if}
              <Cell label="Restore lights afterwards"><Controls kind="switch" bind:value={s.hue.restoreAfter} onchange={changed} /></Cell>
            </div>
            <div class="group">
              <Cell label="Room follows the artwork" hint="While browsing, the room slowly takes the colours of the title in focus."><Controls kind="switch" bind:value={s.hue.artworkMood} onchange={changed} /></Cell>
              <Cell label="Mood brightness"><div style="width:200px"><Controls kind="slider" bind:value={s.hue.moodBrightness} min={5} max={100} step={1} onchange={changed} label="Mood brightness" /></div></Cell>
              <Cell label="Identify lights"><button class="btn tinted small press" onclick={() => api.hueIdentify()}>Blink</button></Cell>
            </div>
          {/if}
        {:else if section === "tobii"}
          <div class="group">
            <Cell label="Use Tobii eye tracker" hint={tobii.status === "off" ? "Uses Tobii's Stream Engine from your Tobii software." : tobii.status}>
              <Controls kind="switch" bind:value={s.tobii.enabled} onchange={() => { changed(); setTimeout(() => api.tobiiStatus().then((t) => (tobii = t)), 1500); }} />
            </Cell>
            <Cell label="Stream Engine" hint={tobii.dll ? `Found: ${tobii.dll}` : "Not found automatically. Point to tobii_stream_engine.dll."}>
              <input class="field small wide" bind:value={s.tobii.dllPath} placeholder="C:\…\tobii_stream_engine.dll" onchange={changed} />
            </Cell>
            {#if gaze.active}
              <Cell label="Live" hint="Gaze and head position are coming through."><span class="live"></span></Cell>
            {/if}
          </div>
          <div class="group">
            <Cell label="Pause when I look away" hint="Resumes when you look back."><Controls kind="switch" bind:value={s.tobii.presencePause} onchange={changed} /></Cell>
            <Cell label="Grace period" hint="{(s.tobii.presenceGraceMs / 1000).toFixed(1)} s"><div style="width:200px"><Controls kind="slider" bind:value={s.tobii.presenceGraceMs} min={800} max={8000} step={100} onchange={changed} label="Grace" /></div></Cell>
            <Cell label="Show controls when I look down" hint="Player controls appear when your eyes rest on the bottom of the screen."><Controls kind="switch" bind:value={s.tobii.gazeControls} onchange={changed} /></Cell>
            <Cell label="Look to preview" hint="Rest your eyes on a poster to open its preview."><Controls kind="switch" bind:value={s.tobii.lookToPreview} onchange={changed} /></Cell>
            <Cell label="Dwell time" hint="{s.tobii.dwellMs} ms"><div style="width:200px"><Controls kind="slider" bind:value={s.tobii.dwellMs} min={300} max={2000} step={50} onchange={changed} label="Dwell" /></div></Cell>
            <Cell label="Window effect" hint="Artwork shifts with your head, like looking through glass."><Controls kind="switch" bind:value={s.tobii.headParallax} onchange={changed} /></Cell>
            <Cell label="Effect strength"><div style="width:200px"><Controls kind="slider" bind:value={s.tobii.parallaxStrength} min={0.2} max={2.5} step={0.1} onchange={changed} label="Strength" /></div></Cell>
          </div>
        {:else if section === "home"}
          <p class="dim lead">Turn rows off or move them. New addons add their rows at the end.</p>
          <div class="group">
            {#each orderedRows as r, i (r.key)}
              <Cell label={r.name} hint="{r.addonName} · {r.type === 'series' ? 'Shows' : r.type === 'movie' ? 'Films' : r.type}">
                <button class="btn tinted small icon press" onclick={() => moveRow(r.key, -1)} disabled={i === 0} aria-label="Move up"><Icon name="chevronUp" size={16} /></button>
                <button class="btn tinted small icon press" onclick={() => moveRow(r.key, 1)} disabled={i === orderedRows.length - 1} aria-label="Move down"><Icon name="chevronDown" size={16} /></button>
                <Controls kind="switch" value={!s.home.hiddenRows.includes(r.key)} onchange={() => toggleRow(r.key)} />
              </Cell>
            {/each}
          </div>
        {:else if section === "network"}
          <div class="group">
            <Cell label="Encrypted DNS" hint="Resolves names over HTTPS, which gets past most DNS-level blocks on images and addons."><Controls kind="switch" bind:value={s.network.doh} onchange={changed} /></Cell>
            <Cell label="DNS provider"><Controls kind="segmented" bind:value={s.network.dohProvider} options={[{ value: "cloudflare", label: "Cloudflare" }, { value: "google", label: "Google" }, { value: "quad9", label: "Quad9" }]} onchange={changed} /></Cell>
            <Cell label="Retry blocked images through a proxy"><Controls kind="switch" bind:value={s.network.proxyFailedImages} onchange={changed} /></Cell>
            <Cell label="Image proxy" hint="The URL is followed by the image address."><input class="field small wide" bind:value={s.network.imageProxy} onchange={changed} /></Cell>
            <Cell label="Addon timeout" hint="{(s.network.addonTimeoutMs / 1000).toFixed(0)} s"><div style="width:200px"><Controls kind="slider" bind:value={s.network.addonTimeoutMs} min={3000} max={30000} step={500} onchange={changed} label="Timeout" /></div></Cell>
          </div>
        {:else if section === "storage"}
          <div class="group">
            <Cell label="Image cache" hint={cache ? bytes(cache.images) || "Empty" : "…"}>
              <button class="btn tinted small press" onclick={async () => { await api.cacheClear(); cache = await api.cacheInfo(); toast("Cache cleared", "ok"); }}>Clear</button>
            </Cell>
            <Cell label="Torrent cache" hint="Deleted automatically when playback stops." />
          </div>
        {:else if section === "about"}
          <p class="dim lead">Version {app.version}. Built for one person.</p>
          <div class="group">
            <Cell label="Playback engine" hint={app.native ? "mpv, rendered natively under the interface" : "System video player"} />
            <Cell label="Keyboard" hint="Space play/pause · ←/→ seek · ↑/↓ volume · F fullscreen · M mute · S subtitles · P picture in picture · Esc back" />
          </div>
        {/if}
      </div>
  {/key}
</div>

<style>
  .settings {
    max-width: 760px;
    margin: 0 auto;
    padding: calc(var(--safe-top) + var(--bar-h) + 18px) var(--gutter) calc(var(--safe-bottom) + 120px);
  }
  .large-title {
    margin-bottom: 20px;
  }
  .inner {
    animation: in var(--t-med) var(--ease-out) both;
  }
  @keyframes in {
    from {
      opacity: 0;
      transform: translateY(8px);
    }
  }
  .group-head {
    margin-top: 28px;
  }
  .lead {
    margin: -8px 4px 18px;
  }
  .note {
    margin: -14px 16px 22px;
  }
  .group {
    margin-bottom: 24px;
  }
  .signin {
    padding: 16px;
  }
  .themes {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
    gap: 12px;
    margin-bottom: 18px;
  }
  .theme {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 10px;
    border-radius: var(--r-md);
    text-align: left;
    background: rgb(255 255 255 / 0.05);
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.08);
  }
  .theme.on {
    box-shadow: inset 0 0 0 2px var(--accent);
  }
  .sw {
    display: flex;
    height: 54px;
    border-radius: 10px;
    overflow: hidden;
    margin-bottom: 6px;
  }
  .sw i {
    flex: 1;
  }
  .tn {
    font-weight: 650;
  }
  .langs {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .chip.sel {
    background: var(--text);
    color: var(--ink);
  }
  .swatches {
    display: flex;
    gap: 8px;
  }
  .swatch {
    width: 26px;
    height: 26px;
    border-radius: 50%;
    box-shadow: inset 0 0 0 1px rgb(0 0 0 / 0.2);
  }
  .swatch.on {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .field.small {
    height: 36px;
    width: 180px;
  }
  .field.wide {
    width: min(360px, 40vw);
  }
  select.field {
    appearance: none;
    padding-right: 28px;
  }
  .code {
    font-family: var(--font-display);
    font-size: 26px;
    font-weight: 700;
    letter-spacing: 0.12em;
  }
  .live {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: #32d74b;
    box-shadow: 0 0 10px #32d74b;
  }
  .preview-sub {
    height: 150px;
    border-radius: var(--r-lg);
    margin-bottom: 18px;
    display: flex;
    align-items: flex-end;
    justify-content: center;
    padding-bottom: 22px;
    background: linear-gradient(135deg, var(--ambient-1), var(--ambient-2) 60%, var(--ambient-3));
    overflow: hidden;
  }
  .cue {
    padding: 4px 14px;
    border-radius: 10px;
    text-align: center;
  }
  .cue em {
    opacity: 0.7;
    font-style: italic;
  }
  .cue[data-bg="glass"] {
    background: rgb(0 0 0 / var(--bg-o));
    backdrop-filter: blur(var(--blur));
  }
  .cue[data-bg="box"] {
    background: rgb(0 0 0 / var(--bg-o));
  }
  .cue[data-bg="shadow"] {
    text-shadow: 0 2px 6px rgb(0 0 0 / 0.9), 0 0 2px rgb(0 0 0 / 0.9);
  }
  .cue[data-bg="outline"] {
    -webkit-text-stroke: 3px rgb(0 0 0 / 0.85);
    paint-order: stroke fill;
  }
  @media (max-width: 759px) {
    .field.small,
    .field.wide {
      width: 100%;
    }
  }
</style>
