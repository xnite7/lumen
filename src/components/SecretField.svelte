<script lang="ts">
  // Keys are write-only from the UI: you can replace or clear one, but its
  // value is never sent back to the page.
  import { api } from "../lib/api";
  import { app, refreshSecrets } from "../lib/stores/app.svelte";
  import { toast, errorText } from "../lib/stores/toast.svelte";

  let { name, placeholder = "Paste key" }: { name: string; placeholder?: string } = $props();
  let value = $state("");
  const set = $derived(!!app.secrets[name]);

  async function save() {
    try {
      await api.setSecret(name, value || null);
      value = "";
      await refreshSecrets();
      toast("Saved", "ok", undefined, 1600);
    } catch (e) {
      toast(errorText(e), "error");
    }
  }
  async function clear() {
    await api.setSecret(name, null);
    await refreshSecrets();
  }
</script>

<div class="sf">
  <input class="field" type="password" bind:value placeholder={set ? "•••••••• saved" : placeholder} autocomplete="off" spellcheck="false" onkeydown={(e) => e.key === "Enter" && value && save()} />
  {#if value}
    <button class="btn small accent press" onclick={save}>Save</button>
  {:else if set}
    <button class="btn small tinted press" onclick={clear}>Remove</button>
  {/if}
</div>

<style>
  .sf {
    display: flex;
    gap: 8px;
    width: min(360px, 40vw);
  }
  .field {
    height: 36px;
  }
</style>
