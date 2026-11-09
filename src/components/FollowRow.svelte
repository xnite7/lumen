<script lang="ts">
  // "new from who you follow": recent and upcoming releases from the actors,
  // directors and studios you follow, each saying who it's from.
  import { api, type Meta } from "../lib/api";
  import { follows, loadFollows } from "../lib/stores/follows.svelte";
  import { live } from "../lib/stores/live.svelte";
  import Poster from "./Poster.svelte";

  let items = $state<(Meta & { because?: string; date?: string })[]>([]);
  $effect(() => {
    loadFollows();
  });
  $effect(() => {
    void live.tick;
    const n = follows.list.length; // follow / unfollow refreshes it
    if (!n) {
      items = [];
      return;
    }
    api.followFeed().then((r) => (items = r ?? [])).catch(() => {});
  });
  const soon = (d?: string) => !!d && d > new Date().toISOString().slice(0, 10);
  const when = (d?: string) => (d ? new Date(d).toLocaleDateString(undefined, { day: "numeric", month: "short" }) : "");
</script>

{#if items.length}
  <section class="row">
    <header><h2 class="title3">new from who you follow</h2></header>
    <div class="rail scroll-x">
      {#each items as m (m.id)}
        <div class="cell">
          <Poster meta={m} />
          <span class="by foot">{m.because}</span>
          {#if m.date}<span class="dt foot" class:soon={soon(m.date)}>{soon(m.date) ? `out ${when(m.date)}` : when(m.date)}</span>{/if}
        </div>
      {/each}
    </div>
  </section>
{/if}

<style>
  .row {
    margin-top: var(--shelf-gap);
  }
  header {
    padding: 0 var(--gutter);
    margin-bottom: 12px;
  }
  .rail {
    display: flex;
    gap: 14px;
    padding: 4px var(--gutter) 8px;
  }
  .cell {
    flex: none;
    width: var(--card-w);
    display: grid;
    gap: 3px;
  }
  .cell :global(.poster) {
    width: auto;
    margin-bottom: 5px;
  }
  .by {
    font-weight: 700;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .dt {
    color: var(--label-3);
  }
  .dt.soon {
    color: var(--lamp);
    font-weight: 700;
  }
</style>
