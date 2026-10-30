<script lang="ts">
  // The lifted preview plus menu that `pressMenu` opens.
  import Icon from "../components/Icon.svelte";
  import { menu, closeMenu } from "../lib/menu.svelte";
  import { haptic } from "../lib/native";

  const vw = $derived(typeof window !== "undefined" ? window.innerWidth : 400);
  const vh = $derived(typeof window !== "undefined" ? window.innerHeight : 800);

  // Preview scales up around the pressed element, then the whole stack is
  // nudged to fit on screen with the menu under it.
  const layout = $derived.by(() => {
    const r = menu.rect;
    const menuH = menu.items.length * 46 + (menu.title ? 40 : 0) + 8;
    const menuW = Math.min(260, vw - 32);
    if (!r) return { px: vw / 2 - 90, py: 120, pw: 0, ph: 0, mx: vw / 2 - menuW / 2, my: vh / 2 - menuH / 2, menuW };
    const scale = menu.image ? Math.min(1.25, (vh * 0.42) / r.height, (vw - 48) / r.width) : 1;
    const pw = menu.image ? r.width * scale : 0;
    const ph = menu.image ? r.height * scale : 0;
    let px = r.left + r.width / 2 - pw / 2;
    let py = r.top + r.height / 2 - ph / 2;
    const total = ph + 12 + menuH;
    py = Math.max(60, Math.min(py, vh - total - 30));
    px = Math.max(16, Math.min(px, vw - pw - 16));
    let mx = Math.max(16, Math.min(r.left + r.width / 2 - menuW / 2, vw - menuW - 16));
    if (menu.image) mx = Math.max(16, Math.min(px, vw - menuW - 16));
    const my = menu.image ? py + ph + 12 : Math.max(60, Math.min(r.bottom + 8, vh - menuH - 20));
    return { px, py, pw, ph, mx, my, menuW };
  });

  function run(i: number) {
    const it = menu.items[i];
    if (it.disabled) return;
    haptic("light");
    closeMenu();
    setTimeout(it.run, 60);
  }
</script>

<svelte:window onkeydown={(e) => menu.open && e.key === "Escape" && closeMenu()} onresize={() => menu.open && closeMenu()} />

{#if menu.open}
  <div class="backdrop" onclick={closeMenu} oncontextmenu={(e) => { e.preventDefault(); closeMenu(); }} role="presentation"></div>
  {#if menu.image && menu.rect}
    <div
      class="preview"
      style:left="{layout.px}px"
      style:top="{layout.py}px"
      style:width="{layout.pw}px"
      style:height="{layout.ph}px"
      style:--from-x="{menu.rect.left - layout.px}px"
      style:--from-y="{menu.rect.top - layout.py}px"
      style:--from-s={menu.rect.width / layout.pw}
    >
      <img src={menu.image} alt="" />
    </div>
  {/if}
  <div class="menu glass strong" role="menu" style:left="{layout.mx}px" style:top="{layout.my}px" style:width="{layout.menuW}px">
    {#if menu.title}<div class="mtitle foot dim clamp-1">{menu.title}</div>{/if}
    {#each menu.items as it, i}
      <button class="row" class:destructive={it.destructive} disabled={it.disabled} role="menuitem" onclick={() => run(i)}>
        <span>{it.label}</span>
        {#if it.icon}<Icon name={it.icon} size={19} />{/if}
      </button>
    {/each}
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 200;
    background: rgb(0 0 0 / 0.28);
    backdrop-filter: blur(14px) saturate(120%);
    -webkit-backdrop-filter: blur(14px) saturate(120%);
    animation: fade var(--t-med) ease both;
  }
  @keyframes fade {
    from {
      opacity: 0;
    }
  }
  .preview {
    position: fixed;
    z-index: 201;
    border-radius: calc(var(--poster-r) * 1.4);
    overflow: hidden;
    box-shadow: 0 30px 80px -20px rgb(0 0 0 / 0.7);
    transform-origin: top left;
    animation: lift var(--t-slow) var(--spring) both;
  }
  .preview img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  @keyframes lift {
    from {
      transform: translate(var(--from-x), var(--from-y)) scale(var(--from-s));
    }
  }
  .menu {
    position: fixed;
    z-index: 202;
    border-radius: 22px;
    padding: 4px 0;
    overflow: hidden;
    transform-origin: top center;
    animation: pop var(--t-slow) var(--spring) both;
  }
  @keyframes pop {
    from {
      opacity: 0;
      transform: scale(0.6) translateY(-10px);
    }
  }
  .mtitle {
    padding: 9px 16px 8px;
    border-bottom: 0.5px solid var(--separator);
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    width: 100%;
    height: 44px;
    padding: 0 16px;
    font-size: var(--fs-body);
    text-align: left;
  }
  .row + .row {
    box-shadow: inset 0 0.5px 0 var(--separator);
  }
  .row:active,
  .row:hover {
    background: var(--fill-3);
  }
  .row:disabled {
    opacity: 0.4;
  }
  .destructive {
    color: var(--red);
  }
</style>
