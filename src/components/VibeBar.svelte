<script lang="ts">
  // Ask for a feeling, not a title. Answered from your own Simkl ratings.
  import { api, img } from "../lib/api";
  import { app } from "../lib/stores/app.svelte";
  import { openDetail } from "../lib/open";
  import { errorText } from "../lib/stores/toast.svelte";
  import Icon from "./Icon.svelte";

  const placeholders = [
    "what's the vibe today?",
    "cry all night tonight",
    "something that ruins me a little",
    "a cinematography masterpiece",
    "slow, gorgeous, nothing happens",
    "something mean and funny",
  ];
  let ph = $state(0);
  let q = $state("");
  let busy = $state(false);
  let result = $state<{ read?: string; picks: any[] } | null>(null);
  let error = $state("");
  let chips = $state<{ label: string; q: string }[]>([]);
  let focused = $state(false);

  const enabled = $derived(app.settings.integrations.aiEnabled);

  $effect(() => {
    const t = setInterval(() => !focused && !q && (ph = (ph + 1) % placeholders.length), 4200);
    return () => clearInterval(t);
  });

  $effect(() => {
    if (enabled && app.secrets.anthropic && app.secrets.simkl_token) {
      api.vibePrompts().then((r) => (chips = r?.prompts ?? [])).catch(() => {});
    }
  });

  async function ask(query = q) {
    if (!query.trim() || busy) return;
    q = query;
    busy = true;
    error = "";
    result = null;
    try {
      result = await api.vibe(query.trim());
    } catch (e) {
      error = errorText(e);
    }
    busy = false;
  }
</script>

{#if enabled}
  <section class="vibe">
    <form
      class="field-wrap glass"
      class:busy
      onsubmit={(e) => {
        e.preventDefault();
        ask();
      }}
    >
      <span class="blobs" aria-hidden="true"></span>
      <Icon name="sparkles" size={19} />
      <input
        bind:value={q}
        placeholder={placeholders[ph]}
        onfocus={() => (focused = true)}
        onblur={() => (focused = false)}
        aria-label="Describe a mood"
        spellcheck="false"
      />
      {#if q}
        <button type="submit" class="go press" disabled={busy} aria-label="Ask">
          <Icon name={busy ? "refresh" : "forward"} size={18} class={busy ? "spin" : ""} />
        </button>
      {/if}
    </form>
    {#if chips.length && !result && !busy}
      <div class="chips">
        {#each chips as c}
          <button class="chip press" onclick={() => ask(c.q)}>{c.label}</button>
        {/each}
      </div>
    {/if}

    {#if busy}
      <div class="picks">
        {#each Array(3) as _}<div class="pick ghost skeleton"></div>{/each}
      </div>
    {:else if error}
      <p class="foot dim err">{error}</p>
    {:else if result}
      {#if result.read}<p class="read">{result.read}</p>{/if}
      <div class="picks">
        {#each result.picks as p (p.id)}
          <button class="pick glass press" onclick={(e) => openDetail(p, e.currentTarget)}>
            <img src={img(p.poster, "w342")} alt="" />
            <div class="txt">
              <span class="headline">{p.name}</span>
              <span class="foot faint">{p.year} · {p.type === "series" ? "Series" : "Film"}</span>
              <span class="why foot dim">{p.why}</span>
            </div>
          </button>
        {/each}
      </div>
    {/if}
  </section>
{/if}

<style>
  .vibe {
    position: relative;
    z-index: 2;
    margin: -28px var(--gutter) 10px;
    max-width: 900px;
  }
  .field-wrap {
    position: relative;
    display: flex;
    align-items: center;
    gap: 12px;
    height: 56px;
    padding: 0 10px 0 20px;
    border-radius: var(--r-pill);
    overflow: hidden;
  }
  /* soft colour blobs from the hero artwork drifting behind the glass */
  .blobs {
    position: absolute;
    inset: -40%;
    z-index: -2;
    background:
      radial-gradient(30% 60% at 20% 50%, color-mix(in srgb, var(--ambient-1) 55%, transparent), transparent),
      radial-gradient(30% 60% at 70% 40%, color-mix(in srgb, var(--ambient-2) 50%, transparent), transparent),
      radial-gradient(25% 50% at 50% 70%, color-mix(in srgb, var(--ambient-3) 45%, transparent), transparent);
    filter: blur(24px);
    opacity: 0.55;
    animation: drift 14s ease-in-out infinite alternate;
  }
  .busy .blobs {
    animation-duration: 3s;
    opacity: 0.9;
  }
  @keyframes drift {
    to {
      transform: translate(12%, -6%) rotate(20deg);
    }
  }
  input {
    flex: 1;
    height: 100%;
    background: none;
    border: 0;
    outline: 0;
    font-size: 17px;
  }
  input::placeholder {
    color: var(--text-dim);
    transition: opacity var(--t-med);
  }
  .go {
    width: 40px;
    height: 40px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: var(--text);
    color: var(--ink);
  }
  :global(.spin) {
    animation: spin 1s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .chips {
    display: flex;
    gap: 8px;
    margin: 12px calc(-1 * var(--gutter)) 0;
    padding: 0 var(--gutter);
    overflow-x: auto;
    scrollbar-width: none;
  }
  .chips::-webkit-scrollbar {
    display: none;
  }
  .chips .chip {
    flex: none;
    height: 32px;
    font-size: 13px;
    font-weight: 500;
  }
  .read {
    margin: 18px 6px 10px;
    font-family: var(--font-display);
    font-size: 19px;
    font-weight: 500;
    letter-spacing: -0.01em;
  }
  .err {
    margin: 12px 8px 0;
  }
  .picks {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 14px;
    margin-top: 12px;
  }
  .pick {
    display: flex;
    gap: 14px;
    padding: 12px;
    border-radius: var(--r-md);
    text-align: left;
  }
  .pick.ghost {
    height: 170px;
  }
  .pick img {
    width: 92px;
    aspect-ratio: 2 / 3;
    object-fit: cover;
    border-radius: 10px;
    flex: none;
  }
  .txt {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }
  .why {
    margin-top: 6px;
    display: -webkit-box;
    -webkit-line-clamp: 5;
    line-clamp: 5;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  @media (max-width: 759px) {
    .vibe {
      margin: 22px var(--gutter) 4px;
    }
    .field-wrap {
      height: 50px;
      padding-left: 16px;
    }
    input {
      font-size: var(--fs-callout);
    }
    .picks {
      grid-template-columns: 1fr;
    }
  }
</style>
