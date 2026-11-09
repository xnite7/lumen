<script lang="ts">
  // "streaming services": Netflix, Max, Disney+… as logo tiles. Each opens a
  // page of what's on that service.
  import { api } from "../lib/api";
  import { go } from "../lib/router.svelte";
  import { serviceLogo } from "../lib/badges";

  let list = $state<{ id: number; name: string; logo: string | null }[]>([]);
  $effect(() => {
    api.services().then((s) => (list = s)).catch(() => {});
  });
</script>

{#if list.length}
  <section class="row">
    <header><h2 class="title3">streaming services</h2></header>
    <div class="rail scroll-x">
      {#each list as s (s.id)}
        <button class="svc press" onclick={() => go({ name: "service", id: s.id, label: s.name, logo: s.logo })} aria-label={s.name}>
          {#if serviceLogo(s.id)}
            <img class="word" src={serviceLogo(s.id)} alt={s.name} loading="lazy" />
          {:else if s.logo}<img src={s.logo} alt="" loading="lazy" />{:else}<span>{s.name}</span>{/if}
        </button>
      {/each}
    </div>
  </section>
{/if}

<style>
  .row {
    margin-top: var(--shelf-gap);
  }
  header {
    padding: 0 var(--gutter);
    margin-bottom: 12px;
  }
  .rail {
    display: flex;
    gap: 16px;
    padding: 6px var(--gutter) 14px;
  }
  .svc {
    flex: none;
    width: 168px;
    aspect-ratio: 16 / 9;
    border-radius: 16px;
    display: grid;
    place-items: center;
    background: radial-gradient(120% 120% at 30% 0%, rgb(255 255 255 / 0.08), transparent 60%), #120b11;
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.08), 0 14px 30px -18px #000;
    transition: transform 220ms var(--spring), box-shadow 220ms ease;
  }
  .svc:hover {
    transform: translateY(-3px) scale(1.04);
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.18), 0 18px 36px -16px rgb(var(--neon-rgb) / 0.45);
  }
  .svc img {
    width: 64px;
    height: 64px;
    border-radius: 14px;
    object-fit: cover;
  }
  .svc img.word {
    width: 72%;
    height: 42%;
    border-radius: 0;
    object-fit: contain;
  }
  .svc span {
    font-weight: 800;
    letter-spacing: -0.02em;
  }
  @media (max-width: 759px) {
    .svc {
      width: 128px;
    }
    .svc img {
      width: 52px;
      height: 52px;
    }
  }
</style>
