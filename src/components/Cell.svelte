<script lang="ts">
  // One row of an inset grouped list, iOS Settings style.
  import type { Snippet } from "svelte";
  let { label, hint = "", children, stack = false }: { label: string; hint?: string; children?: Snippet; stack?: boolean } = $props();
</script>

<div class="cell" class:stack>
  <div class="lbl">
    <span>{label}</span>
    {#if hint}<small>{hint}</small>{/if}
  </div>
  {#if children}<div class="ctl">{@render children()}</div>{/if}
</div>

<style>
  .cell {
    display: flex;
    align-items: center;
    gap: 18px;
    min-height: 52px;
    padding: 10px 18px;
  }
  .cell + :global(.cell) {
    box-shadow: inset 0 1px 0 var(--line);
  }
  .cell.stack {
    flex-direction: column;
    align-items: stretch;
    gap: 10px;
  }
  .lbl {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .lbl span {
    font-size: 15px;
  }
  small {
    font-size: 12.5px;
    color: var(--text-dim);
    margin-top: 2px;
    line-height: 1.35;
  }
  .ctl {
    display: flex;
    align-items: center;
    gap: 10px;
    flex: none;
  }
  .stack .ctl {
    flex: 1;
  }
</style>
