<script lang="ts">
  import { api, img, type AddonDescriptor } from "../lib/api";
  import { app, saveSettings } from "../lib/stores/app.svelte";
  import { toast, errorText } from "../lib/stores/toast.svelte";
  import Sheet from "../components/Sheet.svelte";
  import Icon from "../components/Icon.svelte";

  let { install: installUrl }: { install?: string } = $props();

  let list = $state<AddonDescriptor[]>([]);
  let health = $state<Record<string, { status: string; ms: number; detail?: string }>>({});
  let checking = $state(false);
  let url = $state("");
  let previewing = $state<AddonDescriptor | null>(null);
  let sheet = $state(false);
  let busy = $state(false);
  let dragFrom = $state(-1);
  let dragOver = $state(-1);

  async function load() {
    list = await api.addons();
  }

  $effect(() => {
    load();
  });

  $effect(() => {
    if (installUrl) preview(installUrl);
  });

  async function preview(u = url) {
    if (!u.trim()) return;
    busy = true;
    try {
      previewing = await api.addonPreview(u.trim());
      sheet = true;
    } catch (e) {
      toast(errorText(e), "error");
    }
    busy = false;
  }

  async function confirmInstall() {
    if (!previewing) return;
    busy = true;
    try {
      await api.install(previewing.transportUrl);
      toast(`Installed ${previewing.manifest.name}`, "ok");
      sheet = false;
      url = "";
      await load();
    } catch (e) {
      toast(errorText(e), "error");
    }
    busy = false;
  }

  async function remove(a: AddonDescriptor) {
    await api.uninstall(a.transportUrl);
    toast(`Removed ${a.manifest.name}`, "info", {
      label: "Undo",
      run: async () => {
        await api.install(a.transportUrl);
        load();
      },
    });
    load();
  }

  async function check() {
    checking = true;
    try {
      const r = await api.health();
      health = Object.fromEntries(r.map((h) => [h.transportUrl, h]));
      const bad = r.filter((h) => h.status === "dead");
      toast(bad.length ? `${bad.length} addon${bad.length > 1 ? "s" : ""} not answering` : "All addons answered", bad.length ? "warn" : "ok");
    } catch (e) {
      toast(errorText(e), "error");
    }
    checking = false;
  }

  async function refresh() {
    await api.refreshAddons();
    await load();
    toast("Addon manifests refreshed", "ok");
  }

  function configure(a: AddonDescriptor) {
    const base = a.transportUrl.replace(/\/manifest\.json$/, "");
    api.openExternal(`${base}/configure`);
  }

  // Drag to reorder: order decides which addon answers meta first and the
  // order of rows on Home.
  function onDrop(to: number) {
    const userList = list.filter((a) => !a.flags.protected);
    if (dragFrom < 0 || dragFrom === to) return;
    const moved = userList.splice(dragFrom, 1)[0];
    userList.splice(to, 0, moved);
    list = [...list.filter((a) => a.flags.protected), ...userList];
    api.reorder(userList.map((a) => a.transportUrl));
    dragFrom = dragOver = -1;
  }

  async function exportList() {
    const data = await api.exportAddons();
    await navigator.clipboard.writeText(JSON.stringify(data, null, 2));
    toast("Addon list copied to clipboard", "ok");
  }

  async function importList() {
    try {
      const txt = await navigator.clipboard.readText();
      const data = JSON.parse(txt);
      const urls: string[] = Array.isArray(data) ? data : data.addons;
      const n = await api.importAddons(urls);
      toast(`Imported ${n} addons`, "ok");
      load();
    } catch {
      toast("Copy an exported addon list first, then import", "warn");
    }
  }

  const resources = (a: AddonDescriptor) =>
    [...new Set(a.manifest.resources.map((r: any) => (typeof r === "string" ? r : r.name)))].map((r) => ({ catalog: "Catalogs", meta: "Details", stream: "Streams", subtitles: "Subtitles", addon_catalog: "Addon lists" })[r as string] ?? r);

  const userAddons = $derived(list.filter((a) => !a.flags.protected));
  const builtins = $derived(list.filter((a) => a.flags.protected));
</script>

<div class="addons">
  <header>
    <h1 class="large-title">Addons</h1>
    <div class="right">
      <button class="btn tinted small press" onclick={check} disabled={checking}><Icon name="gauge" size={15} /> {checking ? "Checking…" : "Check health"}</button>
      <button class="btn tinted small icon press" onclick={refresh} title="Refresh manifests" aria-label="Refresh manifests"><Icon name="refresh" size={15} /></button>
      <button class="btn tinted small icon press" onclick={exportList} title="Copy addon list" aria-label="Copy addon list"><Icon name="upload" size={15} /></button>
      <button class="btn tinted small icon press" onclick={importList} title="Import addon list from clipboard" aria-label="Import addon list"><Icon name="download" size={15} /></button>
    </div>
  </header>

  <form
    class="add glass"
    onsubmit={(e) => {
      e.preventDefault();
      preview();
    }}
  >
    <Icon name="link" size={18} />
    <input bind:value={url} placeholder="Paste an addon link (https://… or stremio://…)" spellcheck="false" />
    <button class="btn accent small press" disabled={!url.trim() || busy}>Add</button>
  </form>
  <p class="foot faint hint">
    Install buttons on addon websites open here too.
    <button class="link" onclick={() => api.openExternal("https://stremio-addons.net")}>Browse community addons</button>
  </p>

  <section class="group">
    {#each userAddons as a, i (a.transportUrl)}
      {@const h = health[a.transportUrl]}
      <div
        class="addon glass"
        class:over={dragOver === i}
        class:dragging={dragFrom === i}
        draggable="true"
        ondragstart={() => (dragFrom = i)}
        ondragover={(e) => {
          e.preventDefault();
          dragOver = i;
        }}
        ondragend={() => (dragFrom = dragOver = -1)}
        ondrop={() => onDrop(i)}
        role="listitem"
      >
        <span class="grip" aria-hidden="true"><Icon name="grip" size={18} /></span>
        <div class="logo">
          {#if a.manifest.logo}<img src={img(a.manifest.logo)} alt="" />{:else}<Icon name="addons" size={22} />{/if}
        </div>
        <div class="txt">
          <div class="nm">
            {a.manifest.name}
            <span class="faint foot">v{a.manifest.version}</span>
            {#if h}<span class="health {h.status}" title={h.detail ?? ""}>{h.status === "ok" ? `${h.ms} ms` : h.status === "slow" ? `slow · ${(h.ms / 1000).toFixed(1)} s` : h.status === "dead" ? "not answering" : "skipped"}</span>{/if}
          </div>
          {#if a.manifest.description}<p class="foot dim clamp-2">{a.manifest.description}</p>{/if}
          <div class="tags">
            {#each resources(a) as r}<span class="chip outline">{r}</span>{/each}
          </div>
        </div>
        <div class="acts">
          {#if a.manifest.behaviorHints?.configurable}
            <button class="btn tinted small press" onclick={() => configure(a)}>Configure</button>
          {/if}
          <button class="btn tinted small icon press danger" onclick={() => remove(a)} aria-label="Remove {a.manifest.name}" title="Remove"><Icon name="trash" size={16} /></button>
        </div>
      </div>
    {/each}
  </section>

  {#if builtins.length}
    <section class="group">
      <h2 class="title3">Built in</h2>
      {#each builtins as a (a.transportUrl)}
        <div class="addon glass">
          <div class="logo sunlogo"><span></span></div>
          <div class="txt">
            <div class="nm">{a.manifest.name}</div>
            <p class="foot dim">{a.manifest.description}</p>
          </div>
          <div class="acts">
            <button class="btn tinted small press" onclick={() => {
              app.settings.integrations.builtinCatalogs = !app.settings.integrations.builtinCatalogs;
              saveSettings();
              setTimeout(load, 500);
            }}>{app.settings.integrations.builtinCatalogs ? "Turn off" : "Turn on"}</button>
          </div>
        </div>
      {/each}
    </section>
  {/if}
</div>

<Sheet bind:open={sheet} width={560}>
  {#if previewing}
    <div class="pv">
      <div class="logo big">{#if previewing.manifest.logo}<img src={img(previewing.manifest.logo)} alt="" />{:else}<Icon name="addons" size={30} />{/if}</div>
      <h2 class="title2">{previewing.manifest.name}</h2>
      <p class="foot faint">v{previewing.manifest.version}</p>
      {#if previewing.manifest.description}<p class="dim">{previewing.manifest.description}</p>{/if}
      <div class="tags center">
        {#each resources(previewing) as r}<span class="chip">{r}</span>{/each}
        {#each previewing.manifest.types as t}<span class="chip outline">{t}</span>{/each}
      </div>
      {#if previewing.manifest.behaviorHints?.adult}<p class="foot warn">Contains adult content. It'll be hidden behind a toggle while “Hide adult” is on.</p>{/if}
      <div class="pv-acts">
        <button class="btn tinted press" onclick={() => (sheet = false)}>Cancel</button>
        <button class="btn accent press" onclick={confirmInstall} disabled={busy}>Install</button>
      </div>
    </div>
  {/if}
</Sheet>

<style>
  .addons {
    padding: calc(var(--bar-h) + 36px) var(--gutter) 60px;
    max-width: 1100px;
  }
  header {
    display: flex;
    align-items: center;
    margin-bottom: 22px;
  }
  .right {
    margin-left: auto;
    display: flex;
    gap: 8px;
  }
  .add {
    display: flex;
    align-items: center;
    gap: 12px;
    height: 54px;
    padding: 0 8px 0 18px;
    border-radius: var(--r-pill);
  }
  .add input {
    flex: 1;
    background: none;
    border: 0;
    outline: 0;
    font-size: 15.5px;
  }
  .hint {
    margin: 10px 18px 26px;
  }
  .link {
    color: var(--accent);
    font-weight: 600;
  }
  .group {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin-bottom: 30px;
  }
  .group h2 {
    margin: 8px 0 4px;
  }
  .addon {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 14px 16px;
    border-radius: var(--r-lg);
    transition: transform var(--t-med) var(--spring), opacity var(--t-fast);
  }
  .addon.dragging {
    opacity: 0.4;
  }
  .addon.over {
    transform: translateY(6px);
  }
  .grip {
    color: var(--text-faint);
    cursor: grab;
  }
  .logo {
    width: 52px;
    height: 52px;
    border-radius: 14px;
    overflow: hidden;
    display: grid;
    place-items: center;
    background: rgb(255 255 255 / 0.08);
    flex: none;
  }
  .logo img {
    width: 100%;
    height: 100%;
    object-fit: contain;
  }
  .sunlogo span {
    width: 26px;
    height: 26px;
    border-radius: 50%;
    background: radial-gradient(circle at 50% 40%, #fff4cc, #ffc22e 55%, #e4736a);
  }
  .txt {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .txt p {
    margin: 0;
  }
  .nm {
    display: flex;
    align-items: baseline;
    gap: 8px;
    font-weight: 650;
    font-size: 16px;
  }
  .health {
    font-size: 12px;
    font-weight: 600;
    padding: 1px 8px;
    border-radius: 6px;
  }
  .health.ok {
    color: #4ade80;
    background: rgb(74 222 128 / 0.12);
  }
  .health.slow {
    color: #fbbf24;
    background: rgb(251 191 36 / 0.12);
  }
  .health.dead {
    color: #f87171;
    background: rgb(248 113 113 / 0.14);
  }
  .tags {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
  }
  .tags .chip {
    height: 22px;
    font-size: 11px;
  }
  .acts {
    display: flex;
    gap: 8px;
  }
  .danger:hover {
    background: rgb(255 69 58 / 0.25);
  }
  .pv {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    gap: 8px;
    padding-top: 6px;
  }
  .pv p {
    margin: 0;
  }
  .logo.big {
    width: 76px;
    height: 76px;
    border-radius: 20px;
    margin-bottom: 6px;
  }
  .center {
    justify-content: center;
    margin: 6px 0;
  }
  .warn {
    color: #fbbf24;
  }
  .pv-acts {
    display: flex;
    gap: 10px;
    margin-top: 14px;
  }
</style>
