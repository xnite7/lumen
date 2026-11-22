<script lang="ts">
  // a person's name in their Discord display-name font
  import { nameCss } from "../lib/names.svelte";
  let { id, name, size = 15 }: { id?: string; name: string; size?: number } = $props();
</script>

<span class="nm" style="font-size:{size}px;{nameCss(id)}">{name}</span>

<style>
  .nm { font-weight: 700; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
</style>
