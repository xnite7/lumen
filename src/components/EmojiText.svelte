<script lang="ts">
  import { emojiParts } from "../lib/emoji";
  let { text, size = "1.25em" }: { text: string; size?: string } = $props();
  const parts = $derived(emojiParts(text ?? ""));
</script>

{#each parts as p}{#if p.emoji}<span class="emoji" style:font-size={size}>{p.t}</span>{:else}{p.t}{/if}{/each}

<style>
  .emoji {
    display: inline-block;
    vertical-align: -0.28em;
    margin: 0 0.04em;
    font-family: var(--font-emoji);
  }
</style>
