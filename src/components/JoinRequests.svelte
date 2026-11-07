<script lang="ts">
  // "X wants to join your couch": let them in, or keep the couch.
  import { together, answer } from "../lib/stores/together.svelte";
  import Icon from "./Icon.svelte";
</script>

{#if together.inbox.length}
  <div class="asks" role="region" aria-label="Couch requests">
    {#each together.inbox as a (a.id)}
      <div class="ask glass strong" role="alertdialog" aria-label="{a.fromName} wants to join your couch">
        <span class="av">{#if a.fromAvatar}<img src={a.fromAvatar} alt="" />{:else}{a.fromName.charAt(0).toUpperCase()}{/if}</span>
        <div class="txt">
          <span class="headline">{a.fromName}</span>
          <span class="foot dim">wants to join your couch</span>
        </div>
        <button class="no press" onclick={() => answer(a, false)} aria-label="Decline"><Icon name="close" size={14} /></button>
        <button class="yes press" onclick={() => answer(a, true)}><Icon name="couch" size={16} /> Let them in</button>
      </div>
    {/each}
  </div>
{/if}

<style>
  .asks {
    position: fixed;
    z-index: 300; /* above the player, so "Let them in" works mid-film */
    top: calc(var(--safe-top) + 68px);
    right: var(--gutter);
    display: flex;
    flex-direction: column;
    gap: 10px;
    width: min(420px, calc(100vw - 32px));
  }
  .ask {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 12px 12px 14px;
    border-radius: 18px;
    background: rgb(10 8 9 / 0.92);
    box-shadow: inset 0 0 0 1px rgb(var(--hi) / 0.1), 0 24px 60px -20px rgb(0 0 0 / 0.95);
    animation: in 380ms var(--spring) both;
  }
  @keyframes in {
    from {
      opacity: 0;
      transform: translateY(-10px) scale(0.96);
    }
  }
  .av {
    flex: none;
    width: 40px;
    height: 40px;
    border-radius: 30%;
    overflow: hidden;
    display: grid;
    place-items: center;
    font-weight: 700;
    background: linear-gradient(140deg, var(--neon), #6b1d4c);
  }
  .av img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .txt {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .no {
    width: 34px;
    height: 34px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: rgb(var(--hi) / 0.1);
  }
  .yes {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    height: 36px;
    padding: 0 14px;
    border-radius: 12px;
    font-weight: 700;
    font-size: 14px;
    background: #fff4ea;
    color: #000;
  }
  @media (max-width: 759px) {
    .asks {
      top: calc(var(--safe-top) + 12px);
      right: 16px;
    }
  }
</style>
