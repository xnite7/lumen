<script lang="ts">
  // A settings row: label with explanation on the left, control on the right.
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
    position: relative;
    display: flex;
    align-items: center;
    gap: 16px;
    min-height: 52px;
    padding: 11px 16px;
  }
  .cell + :global(.cell)::before {
    content: "";
    position: absolute;
    top: 0;
    left: 16px;
    right: 0;
    height: 0.5px;
    background: var(--separator);
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
    gap: 2px;
    min-width: 0;
  }
  .lbl span {
    font-size: var(--fs-body);
    line-height: 1.3;
  }
  small {
    font-size: var(--fs-foot);
    color: var(--label-2);
    line-height: 1.35;
  }
  .ctl {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: none;
    max-width: 62%;
  }
  .stack .ctl {
    flex: 1;
    max-width: none;
  }
  @media (max-width: 759px) {
    .cell:has(:global(.slider)),
    .cell:has(:global(.seg)),
    .cell:has(.ctl > :global(:nth-child(2))),
    .cell:has(:global(.sf)) {
      flex-direction: column;
      align-items: stretch;
      gap: 10px;
    }
    .cell:has(:global(.slider)) .ctl,
    .cell:has(:global(.seg)) .ctl,
    .cell:has(.ctl > :global(:nth-child(2))) .ctl,
    .cell:has(:global(.sf)) .ctl {
      max-width: none;
    }
    .cell :global(.ctl > div[style*="width"]) {
      width: 100% !important;
    }
  }
</style>
