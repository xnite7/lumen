<script lang="ts">
  // Rate, favourite and review one title. Opens by itself after you finish
  // something (`finished`), or from a title page's "Write a review".
  // Ratings go to Simkl (and stay on this device); reviews go to the crew,
  // since Simkl's API has no reviews.
  import Sheet from "./Sheet.svelte";
  import Icon from "./Icon.svelte";
  import { api, img } from "../lib/api";
  import { app } from "../lib/stores/app.svelte";
  import { toast, errorText } from "../lib/stores/toast.svelte";
  import { haptic } from "../lib/native";
  import { isFav, toggleFav, postActivity, type TitleRef } from "../lib/stores/social.svelte";

  let {
    open = $bindable(false),
    meta,
    finished = false,
    rating = $bindable<number | null>(null),
    text = "",
    onclose,
  }: { open?: boolean; meta: TitleRef; finished?: boolean; rating?: number | null; text?: string; onclose?: () => void } = $props();

  let hover = $state(0);
  let words = $state("");
  let busy = $state(false);
  let rated = $state(false);
  $effect(() => {
    if (open) {
      words = text;
      rated = false;
      if (rating == null) api.simklStatus(meta.type, meta.id).then((s) => (rating = s?.rating ?? null)).catch(() => {});
    }
  });
  const fav = $derived(isFav(meta.id));

  async function rate(n: number | null) {
    haptic("selection");
    const old = rating;
    rating = n;
    rated = true;
    try {
      await api.simklRate(meta.type, meta.id, n);
    } catch (e) {
      rating = old;
      toast(errorText(e), "error");
    }
  }

  async function done() {
    busy = true;
    const t = words.trim();
    if (t || (rated && rating)) await postActivity(t ? "review" : "rating", meta, rating, t);
    if (t) toast("posted to the couch", "ok");
    else if (rated && rating) toast(`rated ${rating}/10${app.secrets.simkl_token ? " · on Simkl too" : ""}`, "ok");
    busy = false;
    open = false;
    onclose?.();
  }
</script>

<Sheet bind:open width={520} {onclose}>
  <div class="rs">
    {#if meta.background || meta.poster}
      <div class="art" style:background-image="url({img(meta.background ?? meta.poster, 'w780')})"></div>
    {/if}
    <p class="eyebrow">{finished ? "that's a wrap" : "your take"}</p>
    {#if meta.logo}
      <img class="logo" src={img(meta.logo)} alt={meta.name} />
    {:else}
      <h2 class="nm">{meta.name}</h2>
    {/if}
    <p class="q">{finished ? "you finished it. how was it?" : "how was it?"}</p>

    <div class="stars" role="radiogroup" tabindex="-1" aria-label="Your rating" onmouseleave={() => (hover = 0)}>
      {#each Array(10) as _, n}
        <button class="star" class:on={(hover || rating || 0) > n} onmouseenter={() => (hover = n + 1)} onclick={() => rate(rating === n + 1 ? null : n + 1)} aria-label="{n + 1} out of 10">
          <Icon name="star" size={24} fill={(hover || rating || 0) > n} />
        </button>
      {/each}
    </div>
    <p class="score">{hover || rating ? `${hover || rating}/10` : "tap a star"}</p>

    <textarea bind:value={words} rows="4" maxlength="1200" placeholder="say something about it (the crew sees this)"></textarea>

    <div class="row">
      <button class="btn ghost press fav" class:on={fav} onclick={() => { haptic("light"); toast(toggleFav(meta) ? "added to favourites" : "removed from favourites", "ok"); }}>
        <Icon name="heart" size={18} fill={fav} /> {fav ? "Favourite" : "Favourite"}
      </button>
      <button class="btn primary press" disabled={busy} onclick={done}>
        {words.trim() ? "Post review" : "Done"}
      </button>
    </div>
  </div>
</Sheet>

<style>
  .rs {
    position: relative;
    display: grid;
    justify-items: center;
    gap: 10px;
    padding: 6px 4px 8px;
    text-align: center;
    isolation: isolate;
  }
  .art {
    position: absolute;
    inset: -60px -40px auto;
    height: 260px;
    z-index: -1;
    background: center / cover no-repeat;
    opacity: 0.35;
    mask-image: linear-gradient(#000 20%, transparent);
    -webkit-mask-image: linear-gradient(#000 20%, transparent);
    filter: blur(2px);
  }
  .eyebrow {
    margin: 18px 0 0;
    font: 12px var(--mono, ui-monospace, monospace);
    letter-spacing: 0.06em;
    color: var(--label-2);
    text-transform: lowercase;
  }
  .logo {
    max-width: min(320px, 80%);
    max-height: 90px;
    object-fit: contain;
    filter: drop-shadow(0 4px 20px rgb(0 0 0 / 0.5));
  }
  .nm {
    margin: 0;
    font-size: 28px;
    font-weight: 800;
    letter-spacing: -0.03em;
  }
  .q {
    margin: 4px 0 0;
    color: var(--label-2);
  }
  .stars {
    display: flex;
    gap: 2px;
    margin-top: 6px;
  }
  .star {
    padding: 4px;
    color: var(--label-3);
    transition: transform var(--t-fast) var(--spring), color var(--t-fast);
  }
  .star.on {
    color: var(--rating);
  }
  .star:hover {
    transform: scale(1.18);
  }
  .score {
    margin: 0;
    font: 600 13px ui-monospace, monospace;
    color: var(--label-2);
    min-height: 18px;
  }
  textarea {
    width: 100%;
    margin-top: 6px;
    padding: 12px 14px;
    border-radius: 14px;
    border: 0;
    resize: vertical;
    background: rgb(255 255 255 / 0.07);
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.1);
    color: var(--label);
    font: inherit;
  }
  textarea:focus {
    outline: none;
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.28);
  }
  .row {
    display: flex;
    gap: 10px;
    width: 100%;
    justify-content: space-between;
    margin-top: 6px;
  }
  .fav.on {
    color: #ff5a8a;
  }
  @media (max-width: 420px) {
    .star {
      padding: 2px;
    }
  }
</style>
