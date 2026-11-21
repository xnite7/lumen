<script lang="ts">
  // someone on the couch: what they're watching, have watched, want to.
  import { api, img, type Meta } from "$lib/api";
  import { back, nav } from "../lib/nav.svelte";
  import Name from "../ui/Name.svelte";
  import Row from "../ui/Row.svelte";
  import Poster from "../ui/Poster.svelte";
  let { id, title }: { id: string; title?: string } = $props();
  let p = $state<any>(null);
  $effect(() => {
    api.profile(id).then((r) => (p = r)).catch(() => {});
  });
  const prof = $derived(p?.profile);
  const w = $derived(nav.wide ? 140 : 104);
  const m = (i: any) => i as Meta;
</script>

<div class="page">
  <header class="top">
    {#if p?.style?.accent}<div class="banner" style:background="linear-gradient(160deg, {p.style.accent}88, #0000 80%)"></div>{/if}
    <div class="shade"></div>
    <button class="back glass press" onclick={back} aria-label="back">‹</button>
    <div class="me">
      {#if p?.avatar}<img class="av" src={p.avatar} alt="" />{/if}
      <Name {id} name={p?.name ?? title ?? ""} size={28} />
    </div>
  </header>
  {#if prof?.watching?.length}<Row title="watching">{#each prof.watching as i (i.id)}<Poster meta={m(i)} {w} />{/each}</Row>{/if}
  {#if prof?.recent?.length}<Row title="watched lately">{#each prof.recent as i (i.id)}<Poster meta={m(i)} {w} />{/each}</Row>{/if}
  {#if prof?.plan?.length}<Row title="wants to watch">{#each prof.plan as i (i.id)}<Poster meta={m(i)} {w} />{/each}</Row>{/if}
  {#if p && !prof}<p class="muted" style="padding: 24px var(--g)">nothing shared yet.</p>{/if}
</div>

<style>
  .top { position: relative; height: calc(var(--safe-t) + 220px); background: linear-gradient(160deg, #e0309a33, #0000); }
  .banner { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: cover; }
  .shade { position: absolute; inset: 0; background: linear-gradient(to top, var(--bg), #0000 70%); }
  .back { position: absolute; top: calc(var(--safe-t) + 10px); left: 14px; width: 42px; height: 42px; border-radius: 50%; font-size: 28px; display: grid; place-items: center; }
  .me { position: absolute; left: var(--g); right: var(--g); bottom: 6px; display: flex; gap: 14px; align-items: center; }
  .av { width: 72px; height: 72px; border-radius: 50%; box-shadow: 0 6px 20px #000a; }
</style>
