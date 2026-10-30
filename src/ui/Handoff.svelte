<script lang="ts">
  // A file Apple's player can't open (MKV and friends): hand it to an app
  // that can. VLC and Infuse both take a link through their URL schemes.
  import Sheet from "../components/Sheet.svelte";
  import Item from "./Item.svelte";
  import { pb } from "../lib/stores/playback.svelte";
  import { api } from "../lib/api";
  import { toast } from "../lib/stores/toast.svelte";

  let open = $state(false);
  $effect(() => {
    open = !!pb.handoff;
  });
  const local = $derived(/^http:\/\/(127\.0\.0\.1|localhost)/.test(pb.handoff));
  const enc = $derived(encodeURIComponent(pb.handoff));

  function go(url: string) {
    api.openExternal(url).catch(() => toast("That app isn't installed", "warn"));
    pb.handoff = "";
  }
</script>

<Sheet bind:open title="Open in another player" width={480} onclose={() => (pb.handoff = "")}>
  <p class="sub dim lead">
    This file is a format Apple's player can't open.
    {#if local}It's streaming from Lumen, so keep Lumen open in the background while it plays.{/if}
  </p>
  <div class="group">
    <Item label="Infuse" detail="Best picture and subtitles" icon="play" tint="#ff9f0a" chevron onclick={() => go(`infuse://x-callback-url/play?url=${enc}`)} />
    <Item label="VLC" detail="Free, plays everything" icon="play" tint="#ff6a00" chevron onclick={() => go(`vlc-x-callback://x-callback-url/stream?url=${enc}`)} />
    <Item label="Copy link" icon="link" tint="var(--gray)" onclick={() => { navigator.clipboard?.writeText(pb.handoff); toast("Link copied", "ok", undefined, 1400); pb.handoff = ""; }} />
  </div>
</Sheet>

<style>
  .lead {
    margin: 0 4px 16px;
    text-align: center;
  }
</style>
