<script lang="ts">
  // "couch 0.3.12 is here": shown when auto-update is off (or when it's
  // waiting for you to finish watching).
  import { updates, installUpdate } from "../lib/stores/updates.svelte";
  import { app } from "../lib/stores/app.svelte";
  import { pb } from "../lib/stores/playback.svelte";
  import Icon from "./Icon.svelte";

  let dismissed = $state("");
  const show = $derived(!!updates.available && dismissed !== updates.available && !pb.open && (!app.settings.general.autoUpdate || updates.installing));
</script>

{#if show}
  <div class="upd glass strong" role="status">
    <Icon name="download" size={18} />
    <div class="txt">
      <span class="headline">couch {updates.available}</span>
      <span class="foot dim">{updates.installing ? `installing… ${Math.round(updates.progress * 100)}%` : "a new version is ready"}</span>
    </div>
    {#if !updates.installing}
      <button class="no press" onclick={() => (dismissed = updates.available ?? "")} aria-label="Later"><Icon name="close" size={13} /></button>
      <button class="yes press" onclick={installUpdate}>Update</button>
    {:else}
      <span class="bar"><span style:width="{updates.progress * 100}%"></span></span>
    {/if}
  </div>
{/if}

<style>
  .upd {
    position: fixed;
    z-index: 95;
    right: var(--gutter);
    bottom: 24px;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 12px 12px 16px;
    border-radius: 16px;
    background: rgb(10 8 9 / 0.92);
    box-shadow: inset 0 0 0 1px rgb(var(--hi) / 0.1), 0 24px 60px -20px rgb(0 0 0 / 0.95);
    animation: in 380ms var(--spring) both;
  }
  @keyframes in {
    from {
      opacity: 0;
      transform: translateY(10px);
    }
  }
  .txt {
    display: flex;
    flex-direction: column;
    min-width: 140px;
  }
  .no {
    width: 32px;
    height: 32px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: rgb(var(--hi) / 0.1);
  }
  .yes {
    height: 34px;
    padding: 0 16px;
    border-radius: 10px;
    font-weight: 700;
    background: #fff4ea;
    color: #000;
  }
  .bar {
    width: 90px;
    height: 4px;
    background: rgb(var(--hi) / 0.15);
  }
  .bar span {
    display: block;
    height: 100%;
    background: #fff;
  }
</style>
