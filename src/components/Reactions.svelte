<script lang="ts">
  // Teleparty-style reactions while watching together: tap an emoji and it
  // floats up the side of everyone's screen with your name on it.
  import { api } from "../lib/api";
  import { pb } from "../lib/stores/playback.svelte";
  import { together } from "../lib/stores/together.svelte";
  import { social, loadMe } from "../lib/stores/social.svelte";
  import { haptic } from "../lib/native";

  const EMOJI = ["😂", "😭", "😱", "🔥", "❤️", "👀", "💀", "🍿", "👏", "🤯"];
  const room = $derived(pb.open ? (together.following?.host ?? together.hosting) : null);

  type Fly = { k: number; emoji: string; name: string; avatar?: string; x: number; mine: boolean };
  let flying = $state<Fly[]>([]);
  let open = $state(false);
  let k = 0;
  let lastSeen = 0;
  let since = 0;
  const shown = new Set<number>();

  function launch(emoji: string, name: string, avatar: string | undefined, mine: boolean) {
    const f = { k: ++k, emoji, name, avatar, x: Math.random() * 60, mine };
    flying = [...flying.slice(-24), f];
    setTimeout(() => (flying = flying.filter((x) => x.k !== f.k)), 3600);
  }

  $effect(() => {
    const r = room;
    if (!r) return;
    loadMe();
    since = 0;
    let alive = true;
    const tick = async () => {
      try {
        const res = await api.social<{ reactions: any[]; last?: number }>("GET", `/v1/react/${r}?since=${since}`);
        const fresh = (res.reactions ?? []).filter((x) => !shown.has(x.n) && x.who !== social.me?.id);
        for (const x of res.reactions ?? []) since = Math.max(since, x.n);
        // replay a batch with the gaps it was sent with, not all at once
        const first = fresh.length ? Math.min(...fresh.map((x) => x.at)) : 0;
        if (fresh.length) lastSeen = Date.now();
        for (const x of fresh) {
          shown.add(x.n);
          setTimeout(() => launch(x.emoji, x.name, x.avatar, false), Math.min(1500, x.at - first));
        }
      } catch {}
      // quick while emojis are flying, gentler when it's quiet (server costs)
      if (alive) setTimeout(tick, Date.now() - lastSeen < 15000 ? 350 : 900);
    };
    tick();
    return () => (alive = false);
  });

  // the open tray folds itself away after a few quiet seconds
  let fold: ReturnType<typeof setTimeout> | undefined;
  function nudge() {
    clearTimeout(fold);
    fold = setTimeout(() => (open = false), 5000);
  }
  $effect(() => {
    if (open) nudge();
    return () => clearTimeout(fold);
  });

  // sends go out one by one as you tap (queued, never dropped)
  let queue: string[] = [];
  let sending = false;
  async function flush() {
    if (sending || !room) return;
    sending = true;
    while (queue.length) {
      const e = queue.shift()!;
      await api.social("POST", `/v1/react/${room}`, { emoji: e }).catch(() => {});
    }
    sending = false;
  }

  function send(e: string) {
    if (!room) return;
    nudge();
    haptic("light");
    launch(e, "you", social.me?.avatar, true);
    lastSeen = Date.now();
    queue.push(e);
    flush();
  }
</script>

{#if room}
  <div class="stage" aria-hidden="true">
    {#each flying as f (f.k)}
      <div class="fly" style:right="{24 + f.x}px">
        <span class="e">{f.emoji}</span>
        <span class="who">{#if f.avatar}<img src={f.avatar} alt="" />{/if}{f.name}</span>
      </div>
    {/each}
  </div>
  <div class="tray" class:open>
    {#if open}
      {#each EMOJI as e}
        <button class="em press" onclick={() => send(e)} aria-label="React {e}">{e}</button>
      {/each}
    {/if}
    <button class="toggle press" onclick={() => (open = !open)} aria-label="Reactions" aria-expanded={open}>{open ? "✕" : "😊"}</button>
  </div>
{/if}

<style>
  .stage {
    position: fixed;
    right: 0;
    bottom: 110px;
    width: 160px;
    height: 70vh;
    pointer-events: none;
    z-index: 9000;
  }
  .fly {
    position: absolute;
    bottom: 0;
    display: grid;
    justify-items: center;
    gap: 2px;
    animation: up 3.6s cubic-bezier(0.22, 1, 0.36, 1) forwards;
  }
  .e {
    font-size: 40px;
    filter: drop-shadow(0 4px 12px rgb(0 0 0 / 0.5));
    animation: wob 1.2s ease-in-out infinite alternate;
  }
  .who {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 2px 8px 2px 3px;
    border-radius: 999px;
    background: rgb(0 0 0 / 0.55);
    backdrop-filter: blur(8px);
    font: 600 11px system-ui, sans-serif;
    color: #fff;
    white-space: nowrap;
  }
  .who img {
    width: 16px;
    height: 16px;
    border-radius: 5px;
  }
  @keyframes up {
    0% { transform: translateY(0) scale(0.4); opacity: 0; }
    10% { transform: translateY(-30px) scale(1.1); opacity: 1; }
    75% { opacity: 1; }
    100% { transform: translateY(-62vh) scale(0.9); opacity: 0; }
  }
  @keyframes wob {
    to { transform: rotate(10deg) translateX(6px); }
  }
  .tray {
    position: fixed;
    right: 22px;
    bottom: 120px;
    z-index: 9001;
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 5px;
    border-radius: 999px;
    background: rgb(0 0 0 / 0.45);
    backdrop-filter: blur(14px);
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.12);
    opacity: 0.55;
    transition: opacity 200ms;
  }
  .tray:hover,
  .tray.open {
    opacity: 1;
  }
  /* the player's bars went away: so does this */
  :global(html.chrome-hidden) .tray {
    opacity: 0;
    pointer-events: none;
    transform: translateY(8px);
  }
  .tray {
    transition: opacity 250ms ease, transform 250ms ease;
  }
  .em,
  .toggle {
    width: 38px;
    height: 38px;
    border-radius: 50%;
    font-size: 21px;
    display: grid;
    place-items: center;
    transition: transform 150ms var(--spring);
  }
  .em:hover {
    transform: scale(1.25) translateY(-3px);
  }
  .toggle {
    color: #fff;
    font-size: 18px;
  }
</style>
