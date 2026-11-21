<script lang="ts">
  // iPhone: a floating glass tab bar at the bottom.
  // iPad: a glass sidebar on the left, like the TV app.
  import Icon from "$old/components/Icon.svelte";
  import { nav, tab, type Tab } from "../lib/nav.svelte";
  const TABS: [Tab, string, string][] = [
    ["home", "home", "home"],
    ["search", "search", "search"],
    ["couch", "couch", "couch"],
    ["lists", "library", "lists"],
    ["me", "personCircle", "me"],
  ];
</script>

{#if nav.wide}
  <nav class="side glass">
    <div class="brand">midnite<span>™</span> couch</div>
    {#each TABS as [t, icon, label]}
      <button class="it press" class:on={nav.tab === t} onclick={() => tab(t)}>
        <Icon name={icon} size={22} fill={nav.tab === t} />{label}
      </button>
    {/each}
  </nav>
{:else}
  <nav class="bar glass">
    {#each TABS as [t, icon, label]}
      <button class="tb press" class:on={nav.tab === t} onclick={() => tab(t)} aria-label={label}>
        <Icon name={icon} size={24} fill={nav.tab === t} />
        <small>{label}</small>
      </button>
    {/each}
  </nav>
{/if}

<style>
  .bar {
    position: fixed; z-index: 40;
    left: 14px; right: 14px; bottom: calc(var(--safe-b) + 8px);
    height: 62px; border-radius: 31px;
    display: flex; justify-content: space-around; align-items: center;
  }
  .tb { display: grid; justify-items: center; gap: 2px; color: var(--label3); padding: 6px 10px; transition: color 0.2s; }
  .tb.on { color: var(--text); }
  .tb small { font-size: 10px; font-weight: 700; }
  .side {
    position: fixed; z-index: 40;
    top: calc(var(--safe-t) + 16px); bottom: 16px; left: 16px; width: 220px;
    border-radius: 26px; padding: 22px 12px; display: flex; flex-direction: column; gap: 4px;
  }
  .brand { font-weight: 900; font-size: 18px; padding: 0 12px 18px; }
  .brand span { color: var(--neon-soft); font-size: 11px; vertical-align: super; }
  .it { display: flex; gap: 12px; align-items: center; padding: 12px; border-radius: 14px; font-weight: 700; font-size: 16px; color: var(--label2); transition: background 0.2s, color 0.2s; }
  .it.on { background: #ffffff1f; color: var(--text); }
</style>
