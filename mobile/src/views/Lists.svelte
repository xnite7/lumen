<script lang="ts">
  // Your lists: plan to watch (newest first), watching, done, and couch lists.
  import { api, type Meta } from "$lib/api";
  import { lists, loadLists } from "$lib/stores/lists.svelte";
  import { live } from "$lib/stores/live.svelte";
  import { nav } from "../lib/nav.svelte";
  import Poster from "../ui/Poster.svelte";

  type Tab = "plan" | "watching" | "completed" | string;
  let my = $state<{ simkl: boolean; plan?: Meta[]; watching?: Meta[]; completed?: Meta[] } | null>(null);
  let t = $state<Tab>("plan");
  $effect(() => {
    void live.tick;
    api.myLists().then((r) => (my = r)).catch(() => {});
    loadLists();
  });
  const items = $derived.by((): Meta[] => {
    if (t === "plan" || t === "watching" || t === "completed") return (my?.[t] ?? []) as Meta[];
    return (lists.all.find((l) => l.id === t)?.items ?? []) as Meta[];
  });
  const tabs = $derived([
    ["plan", "plan to watch"],
    ["watching", "watching"],
    ["completed", "done"],
    ...lists.all.map((l) => [l.id, l.name.toLowerCase()]),
  ] as [string, string][]);
</script>

<div class="page">
  <h1 class="ttl">lists</h1>
  <div class="tabs scroller">
    {#each tabs as [id, label] (id)}
      <button class="chip press" class:on={t === id} onclick={() => (t = id)}>{label}</button>
    {/each}
  </div>
  <div class="grid" style:grid-template-columns="repeat(auto-fill, minmax({nav.wide ? 160 : 104}px, 1fr))">
    {#each items as m (m.id)}<Poster meta={m} />{/each}
  </div>
  {#if my && !items.length}
    <p class="muted empty">{my.simkl ? "nothing here yet." : "connect Simkl on the me tab to bring your lists in."}</p>
  {/if}
</div>

<style>
  .ttl { margin: 0; padding: calc(var(--safe-t) + 14px) var(--g) 8px; font-size: 32px; }
  .tabs { gap: 8px; }
  .chip { flex: none; padding: 8px 14px; border-radius: 18px; background: var(--surface2); font-weight: 700; font-size: 13px; color: var(--label2); }
  .chip.on { background: var(--text); color: #000; }
  .grid { display: grid; gap: 12px; padding: 14px var(--g); }
  .grid :global(.poster) { width: 100% !important; }
  .empty { padding: 20px var(--g); }
</style>
