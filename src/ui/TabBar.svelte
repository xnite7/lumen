<script lang="ts">
  // iOS 27 tab bar: a floating Liquid Glass capsule with a lighter glass
  // pill on the selected tab. The pill can be dragged across tabs. On phones
  // it sits at the bottom and shrinks while you scroll down; on wide screens
  // it floats at the top centre, like iPadOS.
  import Icon from "../components/Icon.svelte";
  import { nav, selectTab, tabs, type Tab } from "../lib/router.svelte";
  import { ui } from "../lib/ui.svelte";
  import { app } from "../lib/stores/app.svelte";
  import { haptic } from "../lib/native";

  const items: Record<Tab, { label: string; icon: string }> = {
    home: { label: "Home", icon: "home" },
    search: { label: "Search", icon: "search" },
    library: { label: "Library", icon: "library" },
    profile: { label: "Profile", icon: "personCircle" },
  };

  const active = $derived(tabs.indexOf(nav.tab));
  let els: HTMLButtonElement[] = $state([]);
  let bar: HTMLElement | undefined = $state();
  let pill = $state({ x: 0, w: 0 });
  let drag = $state<null | { x: number; over: number }>(null);

  const user = $derived(app.secrets.stremioUser);
  const initial = $derived(((user?.fullname || user?.email || "") as string).trim().charAt(0).toUpperCase());

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
    if (to !== active) selectTab(tabs[to]);
    else measure(active);
  }
</script>

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
  {#each tabs as t, i (t)}
    <button
      bind:this={els[i]}
      class="tab"
      class:on={i === (drag?.over ?? active)}
      aria-current={i === active ? "page" : undefined}
      onclick={() => selectTab(t)}
    >
      {#if t === "profile" && initial}
        <span class="avatar">{initial}</span>
      {:else}
        <Icon name={items[t].icon} size={ui.phone ? 24 : 18} fill={i === active && t !== "search"} />
      {/if}
      <span class="label">{items[t].label}</span>
    </button>
  {/each}
</nav>

<style>
  .tabbar {
    position: fixed;
    z-index: 60;
    display: flex;
    padding: 4px;
    gap: 0;
    border-radius: var(--r-pill);
    touch-action: none;
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
    background: color-mix(in srgb, var(--label) 13%, transparent);
    box-shadow:
      inset 1px 1px 0 -0.5px rgb(255 255 255 / 0.35),
      inset 0 0 0 0.5px rgb(255 255 255 / 0.08);
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
    color: var(--accent);
  }
  :global([data-appearance="light"]) .tab.on {
    color: color-mix(in srgb, var(--accent) 70%, black);
  }
  .label {
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.01em;
    transition: opacity var(--t-fast) ease, height var(--t-med) var(--spring);
  }
  .avatar {
    width: 26px;
    height: 26px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    font-size: 13px;
    font-weight: 700;
    color: var(--accent-ink);
    background: linear-gradient(140deg, var(--ambient-1), var(--ambient-3));
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

  /* wide: top centre, icon beside label */
  @media (min-width: 760px) {
    .tabbar {
      left: 50%;
      top: calc(var(--safe-top) + 10px);
      transform: translateX(-50%);
    }
    .tab {
      flex-direction: row;
      gap: 7px;
      height: 38px;
      padding: 0 16px;
      border-radius: var(--r-pill);
      font-size: 14px;
    }
    .label {
      font-size: 14px;
      font-weight: 600;
      letter-spacing: -0.01em;
    }
    .avatar {
      width: 20px;
      height: 20px;
      font-size: 11px;
    }
    .tab:not(.on):hover {
      color: var(--accent);
    }
  }
</style>
