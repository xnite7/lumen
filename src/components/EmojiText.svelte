<script lang="ts">
  // Text with its emoji drawn as Fluent 3D pictures (Apple's on iPhone / Mac).
  import { emojiParts } from "../lib/emoji";
  let { text, size = "1.25em" }: { text: string; size?: string } = $props();
  const parts = $derived(emojiParts(text ?? ""));
</script>

{#each parts as p}{#if p.src}<img class="emo" src={p.src} alt={p.t} draggable="false" style:width={size} style:height={size} />{:else}{p.t}{/if}{/each}

<style>
  .emo {
    display: inline-block;
    vertical-align: -0.28em;
    margin: 0 0.04em;
    object-fit: contain;
  }
</style>
