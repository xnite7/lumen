<script lang="ts">
  // "Name your list": a small sheet for making a couch list.
  import { namer, newList, renameList } from "../lib/stores/lists.svelte";
  import { toast } from "../lib/stores/toast.svelte";
  let name = $state("");
  let field: HTMLInputElement | undefined = $state();
  $effect(() => {
    if (namer.open) {
      name = namer.rename?.name ?? "";
      setTimeout(() => field?.focus(), 30);
    }
  });
  function make() {
    if (namer.rename) {
      renameList(namer.rename.id, name);
      namer.open = false;
      return;
    }
    const l = newList(name, namer.first ?? undefined);
    if (!l) return;
    toast(namer.first ? `made "${l.name}" with ${namer.first.name ?? "it"} in it` : `made "${l.name}"`, "ok");
    namer.open = false;
  }
</script>

{#if namer.open}
  <div class="scrim" role="presentation" onclick={() => (namer.open = false)}></div>
  <form class="sheet glass strong" onsubmit={(e) => (e.preventDefault(), make())}>
    <h2 class="title3">{namer.rename ? "rename list" : "new list"}</h2>
    <input bind:this={field} bind:value={name} maxlength="60" placeholder="comfort rewatches, scary sundays…" />
    <div class="row">
      <button type="button" class="btn tinted press" onclick={() => (namer.open = false)}>cancel</button>
      <button type="submit" class="btn primary press" disabled={!name.trim()}>{namer.rename ? "save" : "make it"}</button>
    </div>
  </form>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 450;
    background: rgb(0 0 0 / 0.45);
    backdrop-filter: blur(6px);
  }
  .sheet {
    position: fixed;
    z-index: 451;
    left: 50%;
    top: 50%;
    translate: -50% -50%;
    width: min(400px, calc(100vw - 32px));
    display: grid;
    gap: 14px;
    padding: 22px;
    border-radius: 24px;
  }
  h2 {
    margin: 0;
  }
  input {
    height: 46px;
    padding: 0 14px;
    border-radius: 14px;
    border: 0;
    outline: none;
    background: rgb(255 255 255 / 0.08);
    color: var(--label);
    font: inherit;
    font-size: 16px;
  }
  .row {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
  }
</style>
