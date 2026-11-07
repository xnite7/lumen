<script lang="ts">
  import { api } from "../lib/api";
  import { app, saveSettings, refreshSecrets } from "../lib/stores/app.svelte";
  import { applyCurrent } from "../lib/stores/theme.svelte";
  import { toast, errorText } from "../lib/stores/toast.svelte";
  import { gaze } from "../lib/stores/gaze.svelte";
  import { bytes, lang } from "../lib/format";
  import Controls from "../components/Controls.svelte";
  import Cell from "../components/Cell.svelte";
  import Item from "../ui/Item.svelte";
  import { go } from "../lib/router.svelte";
  import SecretField from "../components/SecretField.svelte";
  import subPic from "../assets/sub-preview.webp";
  import { updates, checkForUpdates, installUpdate } from "../lib/stores/updates.svelte";
  import Icon from "../components/Icon.svelte";
  import NavBar from "../ui/NavBar.svelte";
  import DiscordSignIn from "../ui/DiscordSignIn.svelte";
  import SimklConnect from "../ui/SimklConnect.svelte";
  import { SECTIONS, sectionById } from "../lib/sections";

  let { section = "playback" }: { section?: string } = $props();
  const meta = $derived(sectionById(section) ?? SECTIONS[0]);

  const s = app.settings;
  function changed() {
    saveSettings();
    applyCurrent();
  }

  // ------------------------------------------------------------ accounts
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
  $effect(() => {
  });
  const langOptions = ["eng", "ara", "jpn", "spa", "fre", "ger", "ita", "por", "kor", "chi", "hin", "tur", "rus"];
  function toggleLang(list: string[], l: string) {
    return list.includes(l) ? list.filter((x) => x !== l) : [...list, l];
  }
  // what Discord said last time couch tried (open? refused?)
  let rpcStatus = $state("");
  $effect(() => {
    api.discordRpcStatus().then((r) => (rpcStatus = r)).catch(() => {});
  });
</script>

<NavBar title={meta.label} />

<div class="settings">
  <h1 class="large-title">{meta.label}</h1>
  {#key section}
      <div class="inner">
        {#if section === "playback"}
          <div class="group">
            <Cell label="Play next episode automatically"><Controls kind="switch" bind:value={s.playback.autoplayNext} onchange={changed} /></Cell>
            <Cell label="Minimal player" hint="The progress bar sits inside the controls bar, no title in the middle."><Controls kind="switch" bind:value={s.playback.minimalPlayer} onchange={changed} /></Cell>
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
          </div>
          {#if app.platform !== "ios"}
          <h3 class="group-head">Video engine</h3>
          <div class="group">
            <Cell label="Video inside couch" hint="Turn off if video doesn't appear: it will open in its own window instead. Restart couch after changing."><Controls kind="switch" bind:value={s.playback.embedVideo} onchange={changed} /></Cell>
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
          {/if}
        {:else if section === "subtitles"}
          <div class="preview-sub" style:--pic="url({subPic})">
            <span class="cue" style:font-size="{28 * s.subtitles.size}px" style:font-weight={s.subtitles.weight} style:color={s.subtitles.color} data-bg={s.subtitles.background} style:--bg-o={s.subtitles.bgOpacity} style:--blur="{s.subtitles.blur}px">
              {#if s.subtitles.styleSdh && !s.subtitles.hideSdh}<em>(soft static)</em>&nbsp;{/if}The porch light's been on all summer.
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
            <Cell label="Hide sound descriptions"><Controls kind="switch" bind:value={s.subtitles.hideSdh} onchange={changed} /></Cell>
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
            <Cell label="Pick my own stream" hint="Off: couch plays the best stream your connection can keep up with, switches up when your Wi-Fi gets better, and moves on if one won't start. On: you choose from the list every time."><Controls kind="switch" value={!s.streams.autoplayBest} onchange={() => { s.streams.autoplayBest = !s.streams.autoplayBest; changed(); }} /></Cell>
          </div>
        {:else if section === "accounts"}
          {#if app.platform === "windows"}
            <div class="group">
              <Cell label="Bring keys over from Stremio Enhanced" hint="Copies your Simkl sign-in, TMDB and TheIntroDB keys and worker URL from the old app.">
                <button class="btn tinted small press" onclick={importOld}>Import</button>
              </Cell>
            </div>
          {/if}
          <h3 class="group-head">Discord</h3>
          <div class="group">
            {#if app.secrets.discordUser}
              <Cell label={app.secrets.discordUser.name} hint="Your name and avatar on the couch.">
                <button class="btn tinted small press" onclick={async () => { await api.discordLogout(); await refreshSecrets(); }}>Sign out</button>
              </Cell>
            {:else}
              <div class="signin"><DiscordSignIn /></div>
            {/if}
          </div>
          <h3 class="group-head">Simkl</h3>
          <div class="group">
            <SimklConnect />
            <Cell label="Scrobble what you watch"><Controls kind="switch" bind:value={s.integrations.simklScrobble} onchange={changed} /></Cell>
            <Cell label="Library is your Plan to Watch"><Controls kind="switch" bind:value={s.integrations.simklSyncWatchlist} onchange={changed} /></Cell>
          </div>
        {:else if section === "integrations"}
          <div class="group">
            <Cell label="Discord RPC" hint={"Show what you're watching on your Discord profile." + (rpcStatus ? ` · ${rpcStatus}` : "")}><Controls kind="switch" bind:value={s.integrations.discordPresence} onchange={changed} /></Cell>
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
                <Controls kind="segmented" bind:value={s.hue.playbackMode} options={[{ value: "off", label: "Nothing" }, { value: "dim", label: "Couch mode" }, { value: "ambilight", label: "Match the picture" }]} onchange={changed} />
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
            <Cell label="Eye controls" hint="Look at the bottom-left corner while watching: bubbles appear, rest your eyes on one to use it."><Controls kind="switch" bind:value={s.tobii.gazeControls} onchange={changed} /></Cell>
            <Cell label="Bubble dwell time" hint="{(s.tobii.dwellMs / 1000).toFixed(1)} s of looking"><div style="width:200px"><Controls kind="slider" bind:value={s.tobii.dwellMs} min={1300} max={3000} step={50} onchange={changed} label="Dwell" /></div></Cell>
            <Cell label="Window effect" hint="Artwork shifts with your head, like looking through glass."><Controls kind="switch" bind:value={s.tobii.headParallax} onchange={changed} /></Cell>
            {#if s.tobii.headParallax}
              <Cell label="Window depth" hint="{Math.round(s.tobii.parallaxStrength * 100)}%"><div style="width:200px"><Controls kind="slider" bind:value={s.tobii.parallaxStrength} min={0.3} max={2} step={0.1} onchange={changed} label="Depth" /></div></Cell>
            {/if}
            <Cell label="3D films: look around" hint="On 3D releases (SBS / OU) the picture follows your head, like a window into the scene. Depth uses the window depth slider."><Controls kind="switch" bind:value={s.tobii.depth3d} onchange={changed} /></Cell>
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
        {:else if section === "experimental"}
          <div class="exp-warn">
            <Icon name="flask" size={22} />
            <div>
              <p class="headline">here be dragons.</p>
              <p class="foot dim">These are half-finished toys: they can misbehave, drain your laptop, flicker your lights or stop working after an update. Nothing here is needed to watch anything.</p>
            </div>
          </div>
          <div class="group">
            <Cell label="Experimental features" hint="Turn on to see and use the features below."><Controls kind="switch" bind:value={s.general.experimental} onchange={changed} /></Cell>
          </div>
          {#if s.general.experimental}
            <div class="group">
              {#if app.platform !== "ios"}
                <Item label="Eye tracking" detail="Tobii: eye controls, the window effect, head-tracked 3D films" icon="eye" tint="var(--green)" chevron onclick={() => go({ name: "settings", section: "tobii" })} />
              {/if}
              <Item label="Philips Hue" detail="Couch mode lamp and lights that follow the picture" icon="bulb" tint="var(--lamp)" chevron onclick={() => go({ name: "settings", section: "hue" })} />
            </div>
          {/if}
        {:else if section === "about"}
          <p class="dim lead">Version {app.version}. Built for one person.</p>
          {#if app.platform === "windows" || app.platform === "linux"}
            <div class="group">
              <Cell label="Auto-update" hint="New versions install by themselves, never in the middle of something you're watching."><Controls kind="switch" bind:value={s.general.autoUpdate} onchange={changed} /></Cell>
              <Cell label="Updates" hint={updates.installing ? `installing ${updates.available}…` : updates.available ? `couch ${updates.available} is ready` : "you're on the newest couch"}>
                {#if updates.available && !updates.installing}
                  <button class="btn small primary press" onclick={installUpdate}>Update now</button>
                {:else}
                  <button class="btn small tinted press" onclick={() => checkForUpdates(true)} disabled={updates.checking || updates.installing}>{updates.checking ? "checking…" : "Check"}</button>
                {/if}
              </Cell>
            </div>
          {/if}
          <div class="group">
            <Cell label="Playback engine" hint={app.native ? "mpv, rendered natively under the interface" : "System video player"} />
            <Cell label="Keyboard" hint="Space play/pause · ←/→ seek · ↑/↓ volume · F fullscreen · M mute · S subtitles · P picture in picture · Esc back" />
          </div>
        {/if}
      </div>
  {/key}
</div>

<style>
  .exp-warn {
    display: flex;
    gap: 14px;
    align-items: flex-start;
    padding: 16px 18px;
    margin-bottom: 18px;
    border-radius: var(--r-lg);
    color: var(--lamp);
    background: rgb(var(--lamp-rgb) / 0.08);
    box-shadow: inset 0 0 0 1px rgb(var(--lamp-rgb) / 0.3);
  }
  .exp-warn p {
    margin: 0;
  }
  .exp-warn .headline {
    color: var(--text);
    margin-bottom: 4px;
  }
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
    box-shadow: inset 0 0 0 1px rgb(var(--lo) / 0.2);
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
    height: 200px;
    border-radius: var(--r-lg);
    margin-bottom: 18px;
    display: flex;
    align-items: flex-end;
    justify-content: center;
    padding-bottom: 22px;
    background: #1a0e06 var(--pic) center 28% / cover no-repeat;
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
    background: rgb(var(--lo) / var(--bg-o));
    backdrop-filter: blur(var(--blur));
  }
  .cue[data-bg="box"] {
    background: rgb(var(--lo) / var(--bg-o));
  }
  .cue[data-bg="shadow"] {
    text-shadow: 0 2px 6px rgb(var(--lo) / 0.9), 0 0 2px rgb(var(--lo) / 0.9);
  }
  .cue[data-bg="outline"] {
    -webkit-text-stroke: 3px rgb(var(--lo) / 0.85);
    paint-order: stroke fill;
  }
  @media (max-width: 759px) {
    .field.small,
    .field.wide {
      width: 100%;
    }
  }
</style>
