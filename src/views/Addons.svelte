<script lang="ts">
  import { api, img, type AddonDescriptor } from "../lib/api";
  import { app, saveSettings } from "../lib/stores/app.svelte";
  import { toast, errorText } from "../lib/stores/toast.svelte";
  import Sheet from "../components/Sheet.svelte";
  import Icon from "../components/Icon.svelte";
  import NavBar from "../ui/NavBar.svelte";
  import { openMenu } from "../lib/menu.svelte";
  import { haptic } from "../lib/native";

  let selected = $state<AddonDescriptor | null>(null);
  let detailOpen = $state(false);
  function openAddon(a: AddonDescriptor) {
    selected = a;
    detailOpen = true;
  }
  function move(a: AddonDescriptor, dir: number) {
    const userList = list.filter((x) => !x.flags.protected);
    const i = userList.findIndex((x) => x.transportUrl === a.transportUrl);
    const j = i + dir;
    if (i < 0 || j < 0 || j >= userList.length) return;
    [userList[i], userList[j]] = [userList[j], userList[i]];
    list = [...list.filter((x) => x.flags.protected), ...userList];
    api.reorder(userList.map((x) => x.transportUrl));
    haptic("selection");
  }
  function moreMenu(e: MouseEvent) {
    openMenu(e.currentTarget as HTMLElement, [
      { label: checking ? "Checking…" : "Check health", icon: "gauge", disabled: checking, run: check },
      { label: "Refresh manifests", icon: "refresh", run: refresh },
      { label: "Copy addon list", icon: "upload", run: exportList },
      { label: "Import from clipboard", icon: "download", run: importList },
      { label: "Browse community addons", icon: "globe", run: () => api.openExternal("https://stremio-addons.net") },
    ]);
  }

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
    detailOpen = false;
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

<NavBar title="Addons">
  {#snippet actions()}
    <button class="orb glass press" onclick={moreMenu} aria-label="More"><Icon name="more" size={22} /></button>
  {/snippet}
</NavBar>

<div class="addons">
  <h1 class="large-title">Addons</h1>

  <form class="add" onsubmit={(e) => { e.preventDefault(); preview(); }}>
    <Icon name="link" size={18} />
    <input bind:value={url} placeholder="Paste an addon link" spellcheck="false" autocapitalize="off" />
    <button class="btn small accent press" disabled={!url.trim() || busy}>Add</button>
  </form>
  <p class="group-foot">Install buttons on addon websites open Lumen too. Drag to reorder: the order decides which addon answers first.</p>

  <h3 class="group-head">Installed</h3>
  <section class="group list">
    {#each userAddons as a, i (a.transportUrl)}
      {@const h = health[a.transportUrl]}
      <div
        class="addon"
        class:over={dragOver === i}
        class:dragging={dragFrom === i}
        draggable="true"
        ondragstart={() => (dragFrom = i)}
        ondragover={(e) => { e.preventDefault(); dragOver = i; }}
        ondragend={() => (dragFrom = dragOver = -1)}
        ondrop={() => onDrop(i)}
        role="listitem"
      >
        <button class="row" onclick={() => openAddon(a)}>
          <span class="logo">{#if a.manifest.logo}<img src={img(a.manifest.logo)} alt="" />{:else}<Icon name="addons" size={20} />{/if}</span>
          <span class="txt">
            <span class="nm">
              {a.manifest.name}
              {#if h}<span class="health {h.status}" title={h.detail ?? ""}>{h.status === "ok" ? `${h.ms} ms` : h.status === "slow" ? `slow` : h.status === "dead" ? "down" : "skipped"}</span>{/if}
            </span>
            <span class="foot dim clamp-1">{resources(a).join(" · ")}</span>
          </span>
          <span class="grip" aria-hidden="true"><Icon name="grip" size={18} /></span>
          <Icon name="chevron" size={14} class="chev" />
        </button>
      </div>
    {:else}
      <div class="none foot dim">No addons yet. Paste a link above.</div>
    {/each}
  </section>

  {#if builtins.length}
    <h3 class="group-head">Built in</h3>
    <section class="group list">
      {#each builtins as a (a.transportUrl)}
        <div class="addon">
          <div class="row static">
            <span class="logo sunlogo"><span></span></span>
            <span class="txt">
              <span class="nm">{a.manifest.name}</span>
              <span class="foot dim clamp-2">{a.manifest.description}</span>
            </span>
            <button class="btn small tinted press" onclick={() => {
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

<Sheet bind:open={detailOpen} width={520} title={selected?.manifest.name ?? ""}>
  {#if selected}
    {@const a = selected}
    {@const idx = userAddons.findIndex((x) => x.transportUrl === a.transportUrl)}
    <div class="pv">
      <div class="logo big">{#if a.manifest.logo}<img src={img(a.manifest.logo)} alt="" />{:else}<Icon name="addons" size={30} />{/if}</div>
      <p class="foot faint">Version {a.manifest.version}</p>
      {#if a.manifest.description}<p class="sub dim">{a.manifest.description}</p>{/if}
      <div class="tags center">
        {#each resources(a) as r}<span class="chip small">{r}</span>{/each}
        {#each a.manifest.types as t}<span class="chip small outline">{t}</span>{/each}
      </div>
    </div>
    <div class="group">
      {#if a.manifest.behaviorHints?.configurable}
        <button class="srow" onclick={() => configure(a)}><span>Configure</span><Icon name="forward" size={14} /></button>
      {/if}
      <button class="srow" disabled={idx <= 0} onclick={() => move(a, -1)}><span>Move up</span><Icon name="chevronUp" size={16} /></button>
      <button class="srow" disabled={idx < 0 || idx >= userAddons.length - 1} onclick={() => move(a, 1)}><span>Move down</span><Icon name="chevronDown" size={16} /></button>
      <button class="srow" onclick={() => { navigator.clipboard?.writeText(a.transportUrl); toast("Link copied", "ok", undefined, 1400); }}><span>Copy link</span><Icon name="link" size={16} /></button>
    </div>
    <div class="group danger-group">
      <button class="srow destructive" onclick={() => remove(a)}><span>Remove addon</span><Icon name="trash" size={16} /></button>
    </div>
  {/if}
</Sheet>

<Sheet bind:open={sheet} width={520} title="Install addon">
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
    max-width: 760px;
    margin: 0 auto;
    padding: calc(var(--safe-top) + var(--bar-h) + 18px) var(--gutter) calc(var(--safe-bottom) + 120px);
  }
  .large-title {
    margin-bottom: 18px;
  }
  .add {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 50px;
    padding: 0 8px 0 16px;
    border-radius: var(--r-pill);
    background: var(--fill-3);
    color: var(--label-2);
  }
  .add input {
    flex: 1;
    min-width: 0;
    background: none;
    border: 0;
    outline: 0;
    color: var(--label);
    font-size: var(--fs-callout);
  }
  .add input::placeholder {
    color: var(--label-2);
  }
  .group-foot {
    margin-bottom: 26px;
  }
  .group {
    margin-bottom: 26px;
  }
  .addon {
    position: relative;
    transition: transform var(--t-med) var(--spring), opacity var(--t-fast);
  }
  .addon + .addon::before {
    content: "";
    position: absolute;
    top: 0;
    left: 68px;
    right: 0;
    height: 0.5px;
    background: var(--separator);
  }
  .addon.dragging {
    opacity: 0.4;
  }
  .addon.over {
    transform: translateY(6px);
  }
  .row {
    display: flex;
    align-items: center;
    gap: 14px;
    width: 100%;
    padding: 10px 16px;
    text-align: left;
  }
  button.row:active {
    background: var(--cell-press);
  }
  .row :global(.chev) {
    color: var(--label-3);
  }
  .grip {
    color: var(--label-3);
    cursor: grab;
  }
  @media (hover: none) {
    .grip {
      display: none;
    }
  }
  .logo {
    width: 40px;
    height: 40px;
    border-radius: 10px;
    overflow: hidden;
    display: grid;
    place-items: center;
    background: var(--fill-3);
    flex: none;
  }
  .logo img {
    width: 100%;
    height: 100%;
    object-fit: contain;
  }
  .sunlogo span {
    width: 22px;
    height: 22px;
    border-radius: 50%;
    background: radial-gradient(circle at 50% 40%, #fff4cc, #ffc22e 55%, #e4736a);
  }
  .txt {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .nm {
    display: flex;
    align-items: center;
    gap: 8px;
    font-weight: 600;
  }
  .health {
    font-size: 11px;
    font-weight: 700;
    padding: 1px 7px;
    border-radius: 6px;
  }
  .health.ok {
    color: var(--green);
    background: color-mix(in srgb, var(--green) 14%, transparent);
  }
  .health.slow {
    color: var(--orange);
    background: color-mix(in srgb, var(--orange) 14%, transparent);
  }
  .health.dead {
    color: var(--red);
    background: color-mix(in srgb, var(--red) 14%, transparent);
  }
  .none {
    padding: 18px 16px;
  }
  .pv {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    gap: 8px;
    padding: 4px 0 18px;
  }
  .pv p {
    margin: 0;
  }
  .logo.big {
    width: 72px;
    height: 72px;
    border-radius: 18px;
    margin-bottom: 4px;
  }
  .tags {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
  }
  .center {
    justify-content: center;
    margin-top: 4px;
  }
  .warn {
    color: var(--orange);
  }
  .pv-acts {
    display: flex;
    gap: 10px;
    margin-top: 14px;
    width: 100%;
  }
  .pv-acts .btn {
    flex: 1;
  }
  .srow {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    height: 50px;
    padding: 0 16px;
    font-size: var(--fs-body);
    color: var(--accent);
  }
  .srow + .srow::before {
    content: "";
    position: absolute;
    top: 0;
    left: 16px;
    right: 0;
    height: 0.5px;
    background: var(--separator);
  }
  .srow:active {
    background: var(--cell-press);
  }
  .srow:disabled {
    opacity: 0.35;
  }
  .srow.destructive {
    color: var(--red);
  }
  .danger-group {
    margin-bottom: 0;
  }
</style>
