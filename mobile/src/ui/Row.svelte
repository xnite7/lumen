<script lang="ts">
  import type { Snippet } from "svelte";
  let { title, sub, children }: { title: string; sub?: string; children: Snippet } = $props();
</script>

<section class="row">
  <h2 class="row-title">{title}{#if sub}<span class="faint"> · {sub}</span>{/if}</h2>
  <div class="scroller">{@render children()}</div>
</section>

<style>
  .row { margin-top: 26px; }
</style>
