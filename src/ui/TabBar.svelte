<script lang="ts">
  import { updates, installUpdate } from "../lib/stores/updates.svelte";
  import { untrack } from "svelte";
  // Tabs, Apple TV style. Phone: a floating plum-glass capsule at the bottom
  // with a draggable pill. Wide screens: tvOS's top row of quiet text tabs;
  // the selected one sits on a warm-white pill.
  import Icon from "../components/Icon.svelte";
  import { nav, selectTab, go, tabs, phoneTabs, type Tab } from "../lib/router.svelte";
  import { searchHint, searchBar } from "../lib/search.svelte";
  import logo from "../assets/brand/couch-mark.png";
  import { ui } from "../lib/ui.svelte";
  import { app } from "../lib/stores/app.svelte";
  import { haptic, nativeTabBar, onNative, type NativeTab } from "../lib/native";
  import { isTauri } from "../lib/api";

  const items: Record<Tab, { label: string; icon: string }> = {
    home: { label: "Home", icon: "home" },
    shows: { label: "Shows", icon: "tv" },
    movies: { label: "Movies", icon: "film" },
    search: { label: "Search", icon: "search" },
    library: { label: "My Lists", icon: "library" },
    couch: { label: "Couch", icon: "couch" },
    profile: { label: "Profile", icon: "personCircle" },
  };

  const active = $derived(phoneTabs.indexOf(nav.tab));
  let els: HTMLButtonElement[] = $state([]);
  let bar: HTMLElement | undefined = $state();
  let pill = $state({ x: 0, w: 0 });
  let drag = $state<null | { x: number; over: number }>(null);

  const user = $derived(app.secrets.discordUser);
  const initial = $derived(((user?.name || "") as string).trim().charAt(0).toUpperCase());

  // ---- iPhone app: Apple's own tab bar (Liquid Glass), not a web copy
  const symbols: Record<Tab, string> = { shows: "tv", movies: "film", home: "house.fill", search: "magnifyingglass", library: "books.vertical.fill", couch: "sofa.fill", profile: "person.crop.circle" };
  let native = $state(false);
  const nativeTabs = $derived<NativeTab[]>(
    phoneTabs.map((t) => ({ id: t, title: items[t].label, symbol: symbols[t], search: t === "search", avatar: t === "profile" ? (user?.avatar ?? undefined) : undefined })),
  );
  $effect(() => {
    if (!isTauri || app.platform !== "ios") return;
    let un: (() => void) | undefined;
    untrack(() => nativeTabBar({ tabs: $state.snapshot(nativeTabs), selected: nav.tab, visible: true }).then((ok) => (native = ok)));
    onNative<{ id: Tab }>("tab", ({ id }) => selectTab(id)).then((u) => (un = u));
    return () => un?.();
  });
  $effect(() => {
    if (native) nativeTabBar({ tabs: $state.snapshot(nativeTabs) });
  });
  $effect(() => {
    if (native) nativeTabBar({ selected: nav.tab });
  });
  $effect(() => {
    if (native) nativeTabBar({ visible: !app.playing && !ui.onboarding && ui.sheets === 0 });
  });

  // ---- wide: logo · [search] Home Library Couch · avatar
  const wideTabs: Tab[] = ["home", "shows", "movies", "library", "couch"];
  let field: HTMLInputElement | undefined = $state();
  let backTo: Tab = "home";
  let menuOpen = $state(false);

  function openSearch() {
    searchBar.open = true;
    requestAnimationFrame(() => field?.focus());
  }
  function typed() {
    // nothing happens until there's something to look for
    if (searchBar.q.trim() && nav.tab !== "search") {
      backTo = nav.tab;
      selectTab("search");
    } else if (!searchBar.q.trim() && nav.tab === "search") {
      selectTab(backTo);
    }
  }
  function closeSearch() {
    if (searchBar.q) {
      searchBar.q = "";
      typed();
    }
    searchBar.open = false;
    field?.blur();
  }
  function blurred() {
    setTimeout(() => {
      if (!searchBar.q.trim() && document.activeElement !== field) searchBar.open = false;
    }, 120);
  }
  function wideTab(t: Tab) {
    if (searchBar.q) closeSearch();
    selectTab(t);
  }
  function account() {
    menuOpen = false;
    selectTab("profile");
  }
  function settings() {
    menuOpen = false;
    selectTab("profile");
    go({ name: "settings" });
  }
  $effect(() => {
    if (!menuOpen) return;
    const off = (e: PointerEvent) => {
      if (!(e.target as Element)?.closest?.(".me")) menuOpen = false;
    };
    const esc = (e: KeyboardEvent) => e.key === "Escape" && (menuOpen = false);
    window.addEventListener("pointerdown", off);
    window.addEventListener("keydown", esc);
    return () => {
      window.removeEventListener("pointerdown", off);
      window.removeEventListener("keydown", esc);
    };
  });

  function measure(i: number) {
    const el = els[i];
    if (el) pill = { x: el.offsetLeft, w: el.offsetWidth };
  }
  $effect(() => {
    void ui.minimized;
    void ui.phone;
    measure(active);
  });

  // Drag the pill: it follows the finger and lands on the nearest tab.
  function down(e: PointerEvent) {
    if (e.pointerType === "mouse") return;
    const i = els.findIndex((el) => el?.contains(e.target as Node));
    if (i !== active) return;
    drag = { x: e.clientX, over: i };
    bar?.setPointerCapture(e.pointerId);
  }
  function move(e: PointerEvent) {
    if (!drag || !bar) return;
    const r = bar.getBoundingClientRect();
    const x = e.clientX - r.left;
    const w = els[0]?.offsetWidth ?? 80;
    pill = { x: Math.max(4, Math.min(r.width - w - 4, x - w / 2)), w };
    const over = els.findIndex((el) => el && x >= el.offsetLeft && x < el.offsetLeft + el.offsetWidth);
    if (over >= 0 && over !== drag.over) {
      drag.over = over;
      haptic("selection");
    }
  }
  function up() {
    if (!drag) return;
    const to = drag.over;
    drag = null;
    if (to !== active) selectTab(phoneTabs[to]);
    else measure(active);
  }
</script>

{#if native}
  <!-- the system tab bar is drawn natively over the page -->
{:else if ui.phone}
<nav
  class="tabbar glass"
  class:mini={ui.minimized && ui.phone}
  class:dragging={!!drag}
  bind:this={bar}
  onpointerdown={down}
  onpointermove={move}
  onpointerup={up}
  onpointercancel={up}
  aria-label="Tabs"
  style:view-transition-name="tabbar"
>
  <span class="pill" style:transform="translateX({pill.x}px)" style:width="{pill.w}px"></span>
  {#each phoneTabs as t, i (t)}
    <button
      bind:this={els[i]}
      class="tab"
      class:on={i === (drag?.over ?? active)}
      aria-current={i === active ? "page" : undefined}
      onclick={() => selectTab(t)}
    >
      {#if t === "profile" && initial}
        <span class="avatar">{#if user?.avatar}<img src={user.avatar} alt="" />{:else}{initial}{/if}</span>
      {:else if ui.phone || t === "profile"}
        <Icon name={items[t].icon} size={ui.phone ? 24 : 18} fill={i === active && (t === "home" || t === "library")} />
      {/if}
      {#if ui.phone || t !== "profile"}<span class="label">{items[t].label}</span>{/if}
    </button>
  {/each}
</nav>
{:else}
<div class="top" style:--fade={Math.min(1, ui.scrollY / 260)} style:view-transition-name="tabbar">
  <button class="brand" class:away={nav.stack.length > 1} onclick={() => wideTab("home")} aria-label="Home">
    <img src={logo} alt="couch" />
  </button>

  <nav class="links" class:searching={searchBar.open || !!searchBar.q} aria-label="Tabs">
    <form class="find" class:open={searchBar.open || !!searchBar.q} onsubmit={(e) => e.preventDefault()}>
      <button type="button" class="find-btn" onclick={() => (searchBar.open ? field?.focus() : openSearch())} aria-label="Search">
        <Icon name="search" size={18} />
      </button>
      <input
        bind:this={field}
        bind:value={searchBar.q}
        oninput={typed}
        onblur={blurred}
        onkeydown={(e) => e.key === "Escape" && closeSearch()}
        placeholder={searchHint.text}
        spellcheck="false"
        autocomplete="off"
        aria-label="Search"
        tabindex={searchBar.open ? 0 : -1}
      />
      {#if searchBar.q}<button type="button" class="find-x" onclick={closeSearch} aria-label="Clear"><Icon name="close" size={11} /></button>{/if}
    </form>
    {#each wideTabs as t (t)}
      <button class="link" class:on={nav.tab === t} aria-current={nav.tab === t ? "page" : undefined} onclick={() => wideTab(t)}>{items[t].label}</button>
    {/each}
  </nav>

  <div class="me">
    <!-- a new couch is ready: tap to update now -->
    {#if updates.available}
      <button class="upd press" onclick={installUpdate} title="couch {updates.available} is ready. click to update" aria-label="Update couch to {updates.available}">
        <Icon name="download" size={15} />
      </button>
    {/if}
    <button class="me-btn" class:on={nav.tab === "profile" || menuOpen} onclick={() => (menuOpen = !menuOpen)} aria-label="Account" aria-expanded={menuOpen}>
      <span class="avatar big">{#if user?.avatar}<img src={user.avatar} alt="" />{:else if initial}{initial}{:else}<Icon name="personCircle" size={20} />{/if}</span>
    </button>
    {#if menuOpen}
      <div class="menu glass strong" role="menu">
        {#if user?.name}<div class="who"><span class="headline">{user.name}</span>{#if user.username}<span class="foot dim">@{user.username}</span>{/if}</div>{/if}
        <button role="menuitem" onclick={account}><Icon name="personCircle" size={17} /> Account</button>
        <button role="menuitem" onclick={settings}><Icon name="settings" size={17} /> Settings</button>
      </div>
    {/if}
  </div>
</div>
{/if}

<style>
  .tabbar {
    position: fixed;
    z-index: 60;
    display: flex;
    padding: 4px;
    gap: 0;
    border-radius: var(--r-pill);
    touch-action: none;
    /* denser than default glass: labels must read over bright artwork */
    background: color-mix(in srgb, var(--glass-tint-strong) 78%, transparent);
    transition:
      transform var(--t-slow) var(--spring),
      padding var(--t-med) var(--spring),
      opacity var(--t-med) ease;
  }
  :global(html.playing) .tabbar,
  :global(html.onboarding) .tabbar {
    opacity: 0;
    pointer-events: none;
  }
  .pill {
    position: absolute;
    top: 4px;
    bottom: 4px;
    left: 0;
    border-radius: var(--r-pill);
    background: color-mix(in srgb, var(--label) 12%, transparent);
    box-shadow:
      inset 1px 1px 0 -0.5px rgb(var(--hi) / 0.35),
      inset 0 0 0 0.5px rgb(var(--hi) / 0.08);
    transition:
      transform var(--t-slow) var(--spring),
      width var(--t-slow) var(--spring);
  }
  .dragging .pill {
    transition: width var(--t-med) var(--spring);
    transform-origin: center;
    scale: 1.08;
    background: color-mix(in srgb, var(--label) 18%, transparent);
  }
  .tab {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 1px;
    color: var(--label);
    transition: color var(--t-fast) ease;
  }
  .tab.on {
    color: var(--lamp);
  }
  .label {
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.01em;
    transition: opacity var(--t-fast) ease, height var(--t-med) var(--spring);
  }
  .avatar img {
    width: 100%;
    height: 100%;
    border-radius: 50%;
    object-fit: cover;
  }
  .avatar {
    overflow: hidden;
    width: 26px;
    height: 26px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    font-size: 13px;
    font-weight: 700;
    color: var(--text);
    background: linear-gradient(140deg, var(--neon), #6b1d4c);
  }

  /* phone: bottom, label under icon */
  @media (max-width: 759px) {
    .tabbar {
      left: 50%;
      bottom: calc(var(--safe-bottom) + 14px);
      transform: translateX(-50%);
      width: min(360px, calc(100vw - 42px));
    }
    .tab {
      flex: 1;
      height: 54px;
      border-radius: var(--r-pill);
    }
    .tabbar.mini {
      transform: translateX(-50%) scale(0.86);
      transform-origin: center bottom;
    }
    .tabbar.mini .label {
      opacity: 0;
      height: 0;
    }
  }

  /* wide: no island, just the words floating over the page */
  .top {
    position: fixed;
    z-index: 60;
    top: 0;
    left: 0;
    right: 0;
    height: calc(var(--safe-top) + 58px);
    padding: calc(var(--safe-top) + 10px) var(--gutter) 0 var(--gutter); /* the avatar now hangs under the window buttons, so the tabs sit truly centred */
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    align-items: center;
    pointer-events: none;
    transition: opacity var(--t-med) ease;
  }
  .top::before {
    content: "";
    position: absolute;
    inset: 0 0 -30px;
    z-index: -1;
    pointer-events: none;
    /* clear over the billboard, fading to solid black as the page scrolls */
    background: linear-gradient(180deg, #000 0%, #000 calc(var(--fade) * 72%), rgb(0 0 0 / calc(var(--fade) * 0.6)) 86%, transparent);
    opacity: var(--fade);
    transition: opacity 380ms ease;
  }
  :global(html.playing) .top,
  :global(html.onboarding) .top {
    opacity: 0;
  }
  .top > * {
    pointer-events: auto;
  }
  .brand {
    justify-self: start;
    height: 38px;
    display: grid;
    place-items: center;
    transition: opacity var(--t-med) ease, transform var(--t-med) var(--spring);
  }
  .brand img {
    height: 30px;
    width: auto;
    /* the mark is black ink: flip it to light ink on the dark app */
    filter: invert(1) drop-shadow(0 2px 10px rgb(var(--lo) / 0.8));
  }
  /* a pushed page puts its back button here */
  .brand.away {
    opacity: 0;
    transform: scale(0.8);
    pointer-events: none;
  }
  .links {
    position: relative;
    padding-left: 38px;
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .link,
  .find-btn {
    height: 38px;
    padding: 0 14px;
    font-size: 15px;
    font-weight: 600;
    letter-spacing: -0.01em;
    color: rgb(var(--hi) / 0.62);
    text-shadow: 0 1px 12px rgb(var(--lo) / 0.9);
    transition: color var(--t-fast) ease;
  }
  .link:hover,
  .find-btn:hover {
    color: var(--text);
  }
  /* the tab you're on: a soft lit pill, no bar behind the rest */
  .link {
    border-radius: var(--r-pill);
    transition:
      color var(--t-fast) ease,
      background var(--t-med) ease,
      box-shadow var(--t-med) ease,
      opacity 220ms ease,
      transform var(--t-slow) var(--glide),
      filter 220ms ease;
  }
  .link:hover:not(.on) {
    background: rgb(var(--hi) / 0.06);
  }
  .link.on {
    color: var(--text);
    text-shadow: none;
    background: rgb(var(--hi) / 0.16);
    box-shadow: inset 0 0 0 0.5px rgb(var(--hi) / 0.2), 0 6px 20px -8px rgb(var(--lo) / 0.8);
    backdrop-filter: blur(14px) saturate(1.4);
    -webkit-backdrop-filter: blur(14px) saturate(1.4);
  }
  /* the search grows over the tabs, eating them as it opens */
  .find {
    position: absolute;
    left: 0;
    top: 0;
    z-index: 1;
    width: 38px;
    display: flex;
    align-items: center;
    height: 38px;
    border-radius: var(--r-pill);
    transition:
      width var(--t-slow) var(--glide),
      background var(--t-med) ease,
      box-shadow var(--t-med) ease;
  }
  .searching .find {
    width: max(100%, 360px);
  }
  .searching .link {
    opacity: 0;
    transform: translateX(18px) scale(0.92);
    filter: blur(4px);
    pointer-events: none;
  }
  .find-btn {
    display: grid;
    place-items: center;
    padding: 0 10px;
    filter: drop-shadow(0 1px 8px rgb(var(--lo) / 0.9));
  }
  .find input {
    flex: 1;
    min-width: 0;
    width: 0;
    padding: 0;
    border: 0;
    outline: none;
    background: none;
    color: var(--text);
    font: inherit;
    font-size: 15px;
    opacity: 0;
    transition: width var(--t-slow) var(--spring), opacity var(--t-med) ease, padding var(--t-med) ease;
  }
  .find.open {
    background: rgb(var(--lo) / 0.45);
    box-shadow: inset 0 0 0 1px rgb(var(--hi) / 0.14);
    backdrop-filter: blur(16px);
    -webkit-backdrop-filter: blur(16px);
  }
  .find.open .find-btn {
    color: var(--text);
    padding-right: 4px;
  }
  .find.open input {
    opacity: 1;
    padding-right: 12px;
  }
  .find input:focus-visible {
    outline: none;
    box-shadow: none;
  }
  .find input::placeholder {
    color: rgb(var(--hi) / 0.4);
  }
  .find-x {
    width: 22px;
    height: 22px;
    margin-right: 8px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: rgb(var(--hi) / 0.18);
    color: var(--text);
  }
  .me {
    justify-self: end;
    position: relative;
    display: flex;
    align-items: center;
    gap: 10px;
    pointer-events: auto;
  }
  /* PC: the avatar hangs right under the window's maximize / close buttons */
  :global(html.winctl) .me {
    position: fixed;
    top: 40px;
    right: 29px;
    z-index: 61;
  }
  .upd {
    width: 30px;
    height: 30px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    color: #fff;
    background: var(--neon);
    box-shadow: 0 0 0 3px rgb(var(--neon-rgb) / 0.25), 0 0 16px rgb(var(--neon-rgb) / 0.7);
    animation: upd-pulse 2.2s ease-in-out infinite;
  }
  .upd:hover {
    transform: scale(1.08);
  }
  @keyframes upd-pulse {
    50% {
      box-shadow: 0 0 0 6px rgb(var(--neon-rgb) / 0.12), 0 0 22px rgb(var(--neon-rgb) / 0.9);
    }
  }
  .me-btn {
    display: grid;
    place-items: center;
    border-radius: 50%;
    transition: box-shadow var(--t-fast) ease, transform var(--t-fast) var(--spring);
  }
  .me-btn:hover {
    transform: scale(1.06);
  }
  .me-btn.on {
    box-shadow: 0 0 0 2px var(--bg), 0 0 0 4px var(--lamp);
  }
  /* squircle, like an app icon */
  .avatar.big,
  .avatar.big img,
  .me-btn {
    border-radius: 30%;
    corner-shape: squircle;
  }
  .avatar.big {
    width: 34px;
    height: 34px;
    font-size: 15px;
    box-shadow: 0 4px 14px -4px rgb(var(--lo) / 0.9);
  }
  .menu {
    position: absolute;
    right: 0;
    top: calc(100% + 10px);
    min-width: 210px;
    padding: 6px;
    border-radius: var(--r-md);
    display: flex;
    flex-direction: column;
    background: color-mix(in srgb, var(--surface) 92%, transparent);
    box-shadow: 0 20px 50px -16px rgb(var(--lo) / 0.95), inset 0 0 0 1px rgb(var(--hi) / 0.08);
    transform-origin: top right;
    animation: drop 220ms var(--spring) both;
  }
  @keyframes drop {
    from {
      opacity: 0;
      transform: scale(0.92) translateY(-6px);
    }
  }
  .who {
    display: flex;
    flex-direction: column;
    padding: 8px 12px 10px;
    border-bottom: 1px solid rgb(var(--hi) / 0.08);
    margin-bottom: 4px;
  }
  .menu button {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 40px;
    padding: 0 12px;
    border-radius: var(--r-sm);
    font-size: 15px;
    font-weight: 500;
    color: var(--text);
    text-align: left;
  }
  .menu button:hover {
    background: rgb(var(--hi) / 0.08);
  }
</style>
