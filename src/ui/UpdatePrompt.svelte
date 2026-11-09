<script lang="ts">
  // A new couch is out: update now (restarts), or let it go in quietly when
  // you close the app. Never shows up over a movie.
  import { updates, installUpdate, updateOnExit } from "../lib/stores/updates.svelte";
  import { app } from "../lib/stores/app.svelte";
  import Icon from "../components/Icon.svelte";
</script>

{#if updates.ask && updates.available}
  <div class="scrim" role="presentation" onclick={() => (updates.ask = false)}></div>
  <div class="sheet glass strong" role="dialog" aria-label="Update couch">
    <span class="ic"><Icon name="download" size={24} /></span>
    <h2 class="title3">couch {updates.available} is here</h2>
    <p class="foot dim">you're on {app.version}. update now (couch restarts in a few seconds), or it goes in by itself next time you close the app.</p>
    <div class="row">
      <button class="btn primary press" onclick={installUpdate}>update now</button>
      <button class="btn tinted press" onclick={updateOnExit} disabled={updates.onExit}>{updates.onExit ? "set for when you close" : "when I close couch"}</button>
    </div>
    <button class="later foot dim" onclick={() => (updates.ask = false)}>not now</button>
  </div>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 400;
    background: rgb(0 0 0 / 0.45);
    backdrop-filter: blur(6px);
    animation: fade calc(220ms * var(--motion, 1)) ease both;
  }
  .sheet {
    position: fixed;
    z-index: 401;
    left: 50%;
    top: 50%;
    translate: -50% -50%;
    width: min(420px, calc(100vw - 32px));
    display: grid;
    justify-items: center;
    gap: 10px;
    text-align: center;
    padding: 28px 26px 18px;
    border-radius: 26px;
    animation: pop calc(380ms * var(--motion, 1)) cubic-bezier(0.16, 1, 0.3, 1) both;
  }
  .ic {
    width: 52px;
    height: 52px;
    border-radius: 16px;
    display: grid;
    place-items: center;
    background: rgb(var(--neon-rgb) / 0.18);
    color: var(--neon-soft);
  }
  h2,
  p {
    margin: 0;
  }
  p {
    max-width: 34ch;
    line-height: 1.45;
  }
  .row {
    display: flex;
    gap: 10px;
    margin-top: 8px;
    flex-wrap: wrap;
    justify-content: center;
  }
  .later {
    margin-top: 4px;
    padding: 6px 10px;
  }
  .later:hover {
    color: var(--label);
  }
  @keyframes fade {
    from {
      opacity: 0;
    }
  }
  @keyframes pop {
    from {
      opacity: 0;
      scale: 0.94;
    }
  }
</style>
