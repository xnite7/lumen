<script lang="ts">
  // A tiny phone mock painted in a theme's colours.
  import type { Theme } from "../lib/themes";
  import Icon from "../components/Icon.svelte";
  let { theme: t, on = false, onclick }: { theme: Theme; on?: boolean; onclick?: () => void } = $props();
  const bg = $derived(t.preview?.[0] ?? t.tokens.bg ?? t.tokens.ink ?? "#07080c");
</script>

<button class="theme press" class:on {onclick} aria-pressed={on}>
  <span class="mock" style:background={bg}>
    <span class="m-hero" style:background="linear-gradient(135deg, {t.preview?.[2] ?? '#ffc22e'}, {t.preview?.[1] ?? '#3a4157'})"></span>
    <span class="m-row">{#each [0, 1, 2] as _}<i style:background={t.preview?.[1] ?? "#3a4157"}></i>{/each}</span>
    <span class="m-bar" style:background={t.appearance === "light" ? "rgb(255 255 255 / .7)" : "rgb(255 255 255 / .14)"}></span>
  </span>
  <span class="tn">{t.name}</span>
  {#if t.author}<span class="ta">{t.user ? "Your theme" : t.author}</span>{/if}
  <span class="check" class:show={on}><Icon name="check" size={14} /></span>
</button>

<style>
  .theme {
    position: relative;
    flex: none;
    width: 140px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    scroll-snap-align: center;
  }
  .mock {
    position: relative;
    width: 140px;
    height: 214px;
    border-radius: 24px;
    overflow: hidden;
    margin-bottom: 8px;
    box-shadow:
      inset 0 0 0 0.5px rgb(255 255 255 / 0.18),
      0 18px 40px -16px rgb(0 0 0 / 0.6);
    transition: box-shadow var(--t-med) ease;
  }
  .on .mock {
    box-shadow:
      0 0 0 3px var(--bg),
      0 0 0 5.5px var(--label),
      0 18px 40px -16px rgb(0 0 0 / 0.6);
  }
  .m-hero {
    position: absolute;
    inset: 0 0 46% 0;
    -webkit-mask-image: linear-gradient(#000 55%, transparent);
    mask-image: linear-gradient(#000 55%, transparent);
  }
  .m-row {
    position: absolute;
    left: 10px;
    right: -20px;
    bottom: 44px;
    display: flex;
    gap: 6px;
  }
  .m-row i {
    width: 38px;
    height: 56px;
    border-radius: 6px;
    opacity: 0.85;
  }
  .m-bar {
    position: absolute;
    left: 20px;
    right: 20px;
    bottom: 10px;
    height: 22px;
    border-radius: 11px;
  }
  .tn {
    font-size: var(--fs-sub);
    font-weight: 600;
    text-align: center;
  }
  .ta {
    font-size: var(--fs-caption);
    color: var(--label-2);
  }
  .check {
    position: absolute;
    top: 10px;
    right: 10px;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: var(--label);
    color: var(--bg);
    transform: scale(0);
    transition: transform var(--t-med) var(--spring);
  }
  .check.show {
    transform: scale(1);
  }
</style>
