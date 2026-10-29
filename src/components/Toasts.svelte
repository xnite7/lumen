<script lang="ts">
  import { toasts } from "../lib/stores/toast.svelte";
  import Icon from "./Icon.svelte";
  const icon = { info: "info", ok: "check", warn: "info", error: "close" } as const;
</script>

<div class="stack" aria-live="polite">
  {#each toasts.list as t (t.id)}
    <div class="toast glass strong {t.kind}">
      <span class="ic"><Icon name={icon[t.kind]} size={15} /></span>
      <span class="txt">{t.text}</span>
      {#if t.action}
        <button class="btn small tinted press" onclick={t.action.run}>{t.action.label}</button>
      {/if}
    </div>
  {/each}
</div>

<style>
  .stack {
    position: fixed;
    left: 50%;
    bottom: 26px;
    transform: translateX(-50%);
    z-index: 200;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    pointer-events: none;
  }
  .toast {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 14px 10px 10px;
    border-radius: var(--r-pill);
    max-width: min(560px, 90vw);
    pointer-events: auto;
    animation: pop var(--t-slow) var(--spring) both;
  }
  @keyframes pop {
    from {
      opacity: 0;
      transform: translateY(16px) scale(0.92);
    }
  }
  .ic {
    width: 26px;
    height: 26px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: rgb(255 255 255 / 0.14);
    flex: none;
  }
  .ok .ic {
    background: #34c759;
    color: #fff;
  }
  .warn .ic {
    background: #ff9f0a;
    color: #1a1305;
  }
  .error .ic {
    background: #ff453a;
    color: #fff;
  }
  .txt {
    font-size: 14px;
    font-weight: 500;
  }
</style>
