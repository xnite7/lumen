<script lang="ts">
  // another of your devices is playing: "playing on …", play/pause, play here
  import { img } from "$lib/api";
  import { pb } from "$lib/stores/playback.svelte";
  import { devices, send, playHere } from "../lib/devices.svelte";
  import Icon from "$old/components/Icon.svelte";

  const d = $derived(pb.ios || pb.open ? undefined : devices.others.find((x) => x.playing));
  const paused = $derived.by(() => {
    const p = devices.pending;
    if (d && p && p.id === d.id && Date.now() - p.at < 6000) return p.paused;
    return !!d?.playing?.paused;
  });
  const frac = $derived(d?.playing?.duration > 0 ? Math.min(1, d.playing.pos / d.playing.duration) : 0);
  function toggle() {
    if (!d) return;
    send(d.id, paused ? "play" : "pause");
    devices.pending = { id: d.id, paused: !paused, at: Date.now() };
  }
</script>

{#if d}
  {@const p = d.playing}
  <div class="dev glass">
    {#if p.background || p.poster}<img src={img(p.background ?? p.poster, "w300")} alt="" />{/if}
    <div class="txt">
      <small>playing on {d.name}</small>
      <b>{p.title}{#if p.episode} · {p.episode}{/if}</b>
    </div>
    <button class="tg press" onclick={toggle} aria-label={paused ? "play" : "pause"}><Icon name={paused ? "play" : "pause"} size={22} fill /></button>
    <button class="here press" onclick={() => playHere(d)}>play here</button>
    <i style:width="{frac * 100}%"></i>
  </div>
{/if}

<style>
  .dev {
    position: fixed; z-index: 39; left: 12px; right: 12px;
    bottom: calc(var(--safe-b) + 80px);
    height: 62px; border-radius: 18px; overflow: hidden;
    display: flex; align-items: center; gap: 10px; padding: 0 10px 0 8px;
  }
  :global(main.wide) .dev { left: 268px; right: 24px; bottom: 20px; max-width: 620px; }
  img { width: 76px; height: 44px; border-radius: 10px; object-fit: cover; flex: none; }
  .txt { flex: 1; min-width: 0; display: grid; }
  small { font-size: 11px; font-weight: 800; color: #7ee08a; }
  b { font-size: 14px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .tg { width: 38px; height: 38px; border-radius: 50%; font-size: 13px; display: grid; place-items: center; }
  .here { height: 36px; padding: 0 14px; border-radius: 18px; background: var(--text); color: #000; font-weight: 800; font-size: 13px; flex: none; }
  i { position: absolute; left: 0; bottom: 0; height: 2px; background: #7ee08a; }
</style>
