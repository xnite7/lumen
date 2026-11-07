<script lang="ts">
  // "From the couch": what the crew's been saying. A review is a speech
  // bubble coming out of the person (picture + name), score on top; a rating
  // on its own is just the score; finishing something with nothing to say is
  // "finished <title>". Only shows up when there's something to show.
  import { img } from "../lib/api";
  import { go } from "../lib/router.svelte";
  import { social, loadFeed, loadMe, ago, type Activity } from "../lib/stores/social.svelte";
  import Icon from "./Icon.svelte";

  $effect(() => {
    loadMe();
    loadFeed();
    const t = setInterval(() => loadFeed(), 120_000);
    return () => clearInterval(t);
  });
  const feed = $derived(social.feed.slice(0, 24));
  const open = (a: Activity) => go({ name: "detail", type: a.title.type, id: a.title.id });
  const who = (a: Activity) => (a.who === social.me?.id ? "you" : a.name);
</script>

{#if feed.length}
  <section class="row">
    <header><h2 class="title3">from the couch</h2></header>
    <div class="rail scroll-x">
      {#each feed as a (a.id)}
        <button class="item press" class:review={a.kind === "review"} onclick={() => open(a)} aria-label="{who(a)} on {a.title.name}">
          <div class="art">
            {#if a.title.background || a.title.poster}<img src={img(a.title.background ?? a.title.poster, "w780")} alt="" loading="lazy" />{/if}
            <div class="shade"></div>
            {#if a.title.logo}<img class="logo" src={img(a.title.logo)} alt={a.title.name} loading="lazy" />{:else}<span class="tname">{a.title.name}</span>{/if}
          </div>
          <div class="said">
            <span class="av">{#if a.avatar}<img src={a.avatar} alt="" />{:else}{a.name.charAt(0).toUpperCase()}{/if}</span>
            <div class="bubble">
              <div class="top">
                <span class="nm">{who(a)}</span>
                {#if a.rating}<span class="score"><Icon name="star" size={11} fill />{a.rating}/10</span>{/if}
                <span class="when">{ago(a.at)}</span>
              </div>
              {#if a.text}
                <p class="txt">{a.text}</p>
              {:else if a.kind === "finished"}
                <p class="txt dim">finished <b>{a.title.name}</b></p>
              {:else}
                <p class="txt dim">rated <b>{a.title.name}</b></p>
              {/if}
            </div>
          </div>
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
    padding: 6px var(--gutter) 18px;
    align-items: flex-start;
  }
  .item {
    flex: none;
    width: 300px;
    display: grid;
    gap: 10px;
    text-align: left;
  }
  .art {
    position: relative;
    aspect-ratio: 16 / 9;
    border-radius: 12px;
    overflow: hidden;
    background: var(--fill);
  }
  .art > img:first-child {
    width: 100%;
    height: 100%;
    object-fit: cover;
    transition: transform 500ms var(--ease-out);
  }
  .item:hover .art > img:first-child {
    transform: scale(1.04);
  }
  .shade {
    position: absolute;
    inset: 0;
    background: linear-gradient(transparent 35%, rgb(0 0 0 / 0.75));
  }
  .logo {
    position: absolute;
    left: 12px;
    bottom: 10px;
    max-width: 55%;
    max-height: 42%;
    object-fit: contain;
    object-position: left bottom;
  }
  .tname {
    position: absolute;
    left: 12px;
    bottom: 10px;
    right: 12px;
    font-weight: 800;
    letter-spacing: -0.02em;
  }
  .said {
    display: flex;
    gap: 9px;
    align-items: flex-start;
  }
  .av {
    flex: none;
    width: 30px;
    height: 30px;
    border-radius: 10px;
    overflow: hidden;
    display: grid;
    place-items: center;
    background: linear-gradient(135deg, #e0309a, #8a2be2);
    font-weight: 700;
    font-size: 13px;
  }
  .av img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .bubble {
    position: relative;
    flex: 1;
    min-width: 0;
    padding: 8px 12px 10px;
    border-radius: 4px 14px 14px 14px;
    background: rgb(255 255 255 / 0.07);
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.08);
  }
  /* the little tail toward the face */
  .bubble::before {
    content: "";
    position: absolute;
    left: -6px;
    top: 0;
    width: 7px;
    height: 10px;
    background: rgb(255 255 255 / 0.07);
    clip-path: polygon(100% 0, 0 0, 100% 100%);
  }
  .top {
    display: flex;
    align-items: center;
    gap: 7px;
  }
  .nm {
    font-weight: 700;
    font-size: 13.5px;
  }
  .score {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    color: var(--rating);
    font: 700 11.5px ui-monospace, monospace;
  }
  .when {
    margin-left: auto;
    font-size: 11.5px;
    color: var(--label-3);
  }
  .txt {
    margin: 4px 0 0;
    font-size: 13.5px;
    line-height: 1.4;
    display: -webkit-box;
    -webkit-line-clamp: 4;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .txt.dim {
    color: var(--label-2);
  }
  .txt b {
    color: var(--label);
    font-weight: 600;
  }
  @media (max-width: 759px) {
    .item {
      width: 260px;
    }
  }
</style>
