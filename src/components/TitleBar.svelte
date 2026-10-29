<script lang="ts">
  // One floating glass bar: tabs in the middle (a single glass thumb slides
  // between them), utilities and window controls at the edges. The empty
  // space is a drag handle for the borderless window.
  import Icon from "./Icon.svelte";
  import { go, nav, back, current } from "../lib/router.svelte";
  import { api, isTauri } from "../lib/api";
  import { app } from "../lib/stores/app.svelte";

  const tabs = [
    { name: "home", label: "Home", icon: "home" },
    { name: "discover", label: "Discover", icon: "discover" },
    { name: "library", label: "Library", icon: "library" },
    { name: "search", label: "Search", icon: "search" },
  ] as const;

  let { scrolled = false }: { scrolled?: boolean } = $props();

  const route = $derived(current());
  const activeIdx = $derived(tabs.findIndex((t) => nav.stack[0]?.name === t.name));
  let tabEls: HTMLButtonElement[] = $state([]);
  let thumb = $state({ x: 0, w: 0 });

  $effect(() => {
    const el = tabEls[activeIdx];
    if (el) thumb = { x: el.offsetLeft, w: el.offsetWidth };
  });

  function startDrag(e: MouseEvent) {
    if (e.button !== 0 || (e.target as HTMLElement).closest("button,input,a")) return;
    if (e.detail === 2) api.window("maximize");
    else api.window("drag");
  }
</script>

<header class="bar" class:scrolled class:hidden={app.playing} onmousedown={startDrag} role="presentation" style:view-transition-name="titlebar">
  <div class="side left">
    {#if nav.stack.length > 1}
      <button class="round glass press" onclick={() => back()} aria-label="Back"><Icon name="back" size={20} /></button>
    {:else}
      <div class="brand" aria-hidden="true">
        <span class="sun"></span>
      </div>
    {/if}
  </div>

  <nav class="tabs glass" aria-label="Main">
    {#if activeIdx >= 0}
      <span class="thumb" style:transform="translateX({thumb.x}px)" style:width="{thumb.w}px"></span>
    {/if}
    {#each tabs as t, i}
      <button
        bind:this={tabEls[i]}
        class="tab press"
        class:active={i === activeIdx}
        aria-current={i === activeIdx ? "page" : undefined}
        onclick={() => go({ name: t.name } as any)}
      >
        <Icon name={t.icon} size={18} />
        <span>{t.label}</span>
      </button>
    {/each}
  </nav>

  <div class="side right">
    <button class="round glass press" class:on={route.name === "addons"} onclick={() => go({ name: "addons" })} aria-label="Addons" title="Addons">
      <Icon name="addons" size={20} />
    </button>
    <button class="round glass press" class:on={route.name === "settings"} onclick={() => go({ name: "settings" })} aria-label="Settings" title="Settings">
      <Icon name="settings" size={20} />
    </button>
    {#if isTauri && app.platform !== "ios"}
      <div class="win">
        <button onclick={() => api.window("minimize")} aria-label="Minimize"><Icon name="minimize" size={14} /></button>
        <button onclick={() => api.window("maximize")} aria-label="Maximize"><Icon name="maximize" size={13} /></button>
        <button class="x" onclick={() => api.window("close")} aria-label="Close"><Icon name="close" size={14} /></button>
      </div>
    {/if}
  </div>
</header>

<style>
  .bar {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    z-index: 50;
    height: calc(var(--bar-h) + 12px);
    padding: 12px max(var(--gutter), 16px) 0;
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    align-items: center;
    transition:
      transform var(--t-med) var(--glide),
      opacity var(--t-med) ease;
  }
  /* iOS scroll-edge effect: content blurs and fades as it slides under the bar */
  .bar::before {
    content: "";
    position: absolute;
    inset: 0 0 -28px 0;
    z-index: -1;
    pointer-events: none;
    backdrop-filter: blur(14px) saturate(150%);
    -webkit-backdrop-filter: blur(14px) saturate(150%);
    background: linear-gradient(180deg, color-mix(in srgb, var(--ink) 70%, transparent), transparent);
    -webkit-mask-image: linear-gradient(180deg, #000 45%, transparent);
    mask-image: linear-gradient(180deg, #000 45%, transparent);
    opacity: 0;
    transition: opacity var(--t-med) ease;
  }
  .bar.scrolled::before {
    opacity: 1;
  }
  .bar.hidden {
    transform: translateY(-120%);
    opacity: 0;
    pointer-events: none;
  }
  .side {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .right {
    justify-content: flex-end;
  }
  .brand {
    width: 40px;
    height: 40px;
    display: grid;
    place-items: center;
  }
  .sun {
    width: 22px;
    height: 22px;
    border-radius: 50%;
    background: radial-gradient(circle at 50% 40%, #fff4cc, #ffc22e 55%, #e4736a);
    box-shadow: 0 0 22px 2px color-mix(in srgb, var(--accent) 55%, transparent);
  }
  .round {
    width: 44px;
    height: 44px;
    border-radius: 50%;
    display: grid;
    place-items: center;
  }
  .round.on {
    background: color-mix(in srgb, var(--text) 22%, var(--glass-tint));
  }
  .tabs {
    position: relative;
    background: color-mix(in srgb, var(--glass-tint-strong) 70%, transparent);
    display: flex;
    gap: 2px;
    padding: 5px;
    border-radius: var(--r-pill);
  }
  .thumb {
    position: absolute;
    top: 5px;
    bottom: 5px;
    left: 0;
    border-radius: var(--r-pill);
    background: color-mix(in srgb, var(--text) 16%, transparent);
    box-shadow:
      inset 0 1px 0 rgb(255 255 255 / 0.25),
      inset 0 0 0 1px rgb(255 255 255 / 0.08);
    transition:
      transform var(--t-slow) var(--spring),
      width var(--t-slow) var(--spring);
  }
  .tab {
    position: relative;
    display: flex;
    align-items: center;
    gap: 7px;
    height: 38px;
    padding: 0 16px;
    border-radius: var(--r-pill);
    font-weight: 600;
    font-size: 14px;
    color: var(--text-dim);
  }
  .tab.active {
    color: var(--text);
  }
  .tab:hover:not(.active) {
    color: var(--text);
  }
  .win {
    display: flex;
    margin-left: 6px;
    gap: 2px;
  }
  .win button {
    width: 34px;
    height: 34px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    color: var(--text-dim);
    transition: background var(--t-fast);
  }
  .win button:hover {
    background: rgb(255 255 255 / 0.12);
    color: var(--text);
  }
  .win .x:hover {
    background: #e5484d;
    color: white;
  }
  @media (max-width: 1100px) {
    .tab span {
      display: none;
    }
  }
  /* iOS: tabs float at the bottom, like a real tab bar */
  @media (max-width: 760px) {
    .bar {
      padding-top: calc(env(safe-area-inset-top) + 8px);
      grid-template-columns: auto 1fr auto;
    }
    .tabs {
      position: fixed;
      left: 50%;
      transform: translateX(-50%);
      bottom: calc(env(safe-area-inset-bottom) + 12px);
    }
    .tab {
      flex-direction: column;
      gap: 2px;
      height: 50px;
      padding: 0 18px;
      font-size: 10.5px;
    }
    .tab span {
      display: block;
    }
  }
</style>
