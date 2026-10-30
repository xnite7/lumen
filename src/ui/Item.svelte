<script lang="ts">
  // One row of an inset grouped list: optional coloured symbol tile, label
  // with secondary text, a value and/or chevron, or a trailing control.
  import type { Snippet } from "svelte";
  import Icon from "../components/Icon.svelte";
  import { haptic } from "../lib/native";

  let {
    label,
    detail = "",
    value = "",
    icon = "",
    tint = "var(--gray)",
    chevron = false,
    destructive = false,
    accent = false,
    disabled = false,
    onclick,
    children,
    lead,
  }: {
    label: string;
    detail?: string;
    value?: string;
    icon?: string;
    tint?: string;
    chevron?: boolean;
    destructive?: boolean;
    accent?: boolean;
    disabled?: boolean;
    onclick?: () => void;
    children?: Snippet;
    lead?: Snippet;
  } = $props();

  function tap() {
    if (disabled) return;
    haptic("selection");
    onclick?.();
  }
</script>

{#if onclick}
  <button class="item" class:destructive class:accent {disabled} onclick={tap}>
    {@render body()}
  </button>
{:else}
  <div class="item">{@render body()}</div>
{/if}

{#snippet body()}
  {#if lead}
    {@render lead()}
  {:else if icon}
    <span class="tile" style:background={tint}><Icon name={icon} size={17} /></span>
  {/if}
  <span class="text">
    <span class="label">{label}</span>
    {#if detail}<span class="detail">{detail}</span>{/if}
  </span>
  {#if value}<span class="value">{value}</span>{/if}
  {#if children}<span class="ctl">{@render children()}</span>{/if}
  {#if chevron}<span class="chev"><Icon name="chevron" size={14} /></span>{/if}
{/snippet}

<style>
  .item {
    position: relative;
    display: flex;
    align-items: center;
    gap: 14px;
    width: 100%;
    min-height: 52px;
    padding: 10px 16px;
    text-align: left;
    font-size: var(--fs-body);
    transition: background-color var(--t-fast) ease;
  }
  button.item:active {
    background: var(--cell-press);
    transition-duration: 0ms;
  }
  @media (hover: hover) {
    button.item:hover {
      background: color-mix(in srgb, var(--label) 5%, transparent);
    }
  }
  button.item:disabled {
    opacity: 0.45;
  }
  /* inset separator from the label, like UITableView */
  .item + :global(.item)::before {
    content: "";
    position: absolute;
    top: 0;
    right: 0;
    left: var(--sep-inset, 16px);
    height: 0.5px;
    background: var(--separator);
  }
  :global(.group:has(.tile)) {
    --sep-inset: 58px;
  }
  .tile {
    width: 30px;
    height: 30px;
    border-radius: 8px;
    display: grid;
    place-items: center;
    color: #fff;
    flex: none;
    box-shadow: inset 0 0.5px 0 rgb(255 255 255 / 0.25);
  }
  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .label {
    line-height: 1.3;
  }
  .detail {
    font-size: var(--fs-foot);
    color: var(--label-2);
    line-height: 1.35;
  }
  .value {
    color: var(--label-2);
    white-space: nowrap;
    max-width: 45%;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .ctl {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: none;
  }
  .chev {
    color: var(--label-3);
    display: grid;
    margin-left: -4px;
  }
  .destructive .label {
    color: var(--red);
  }
  .accent .label {
    color: var(--accent);
  }
</style>
