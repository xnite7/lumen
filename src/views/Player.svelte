<script lang="ts">
  // The player UI. On desktop the video itself is mpv, drawn underneath this
  // transparent page -- everything here is the glass floating over it.
  import { api, img } from "../lib/api";
  import { app } from "../lib/stores/app.svelte";
  import { gaze } from "../lib/stores/gaze.svelte";
  import { pb, close, cmd, nextVideo, playNext } from "../lib/stores/playback.svelte";
  import { time, lang, episodeLabel } from "../lib/format";
  import YouTube from "../components/YouTube.svelte";
  import Icon from "../components/Icon.svelte";

  let idle = $state(false);
  let idleTimer: ReturnType<typeof setTimeout> | undefined;
  let menu = $state<"" | "subs" | "audio" | "speed" | "picture">("");
  let scrub = $state<{ x: number; t: number } | null>(null);
  let dragging = $state(false);
  let bar: HTMLElement | undefined = $state();
  let video: HTMLVideoElement | undefined = $state();
  let upNextDismissed = $state(false);
  let countdown = $state(0);
  let skipped = new Set<string>();

  const st = $derived(pb.state);
  const native = $derived(pb.mode === "native");
  const sub = $derived(app.settings.subtitles);
  const pbs = $derived(app.settings.playback);

  // ------------------------------------------------------------ chrome idle
  function poke() {
    idle = false;
    clearTimeout(idleTimer);
    idleTimer = setTimeout(() => {
      if (!menu && !dragging && !st.paused) idle = true;
    }, 2600);
  }
  $effect(() => {
    poke();
    return () => clearTimeout(idleTimer);
  });
  // Eyes on the bottom of the screen bring the controls back (Tobii).
  $effect(() => {
    if (app.settings.tobii.enabled && app.settings.tobii.gazeControls && gaze.inside && gaze.y > innerHeight * 0.8) poke();
  });
  const chromeHidden = $derived(idle && !st.paused && !menu && !pb.loading && !pb.error);

  // --------------------------------------------------------------- web mode
  let reportTimer: ReturnType<typeof setInterval> | undefined;
  $effect(() => {
    if (pb.mode !== "web" || !video) return;
    const v = video;
    const sync = () => {
      pb.state = { ...pb.state, pos: v.currentTime, duration: v.duration || 0, paused: v.paused, buffering: v.readyState < 3, volume: v.volume * 100, muted: v.muted };
    };
    const evs = ["timeupdate", "play", "pause", "waiting", "playing", "loadedmetadata", "volumechange"];
    evs.forEach((e) => v.addEventListener(e, sync));
    v.addEventListener("playing", () => (pb.loading = false));
    reportTimer = setInterval(() => api.report(v.currentTime, v.duration || 0, v.paused), 5000);
    return () => {
      evs.forEach((e) => v.removeEventListener(e, sync));
      clearInterval(reportTimer);
    };
  });

  function toggle() {
    if (native) cmd("pause");
    else if (video) video.paused ? video.play() : video.pause();
    poke();
  }
  function seekTo(t: number) {
    t = Math.max(0, Math.min(st.duration || t, t));
    if (native) cmd("seek", { to: t });
    else if (video) video.currentTime = t;
  }
  function seekBy(d: number) {
    if (native) cmd("seek", { to: d, relative: true });
    else if (video) video.currentTime += d;
    poke();
  }
  function setVolume(v: number) {
    if (native) cmd("volume", { value: v });
    else if (video) video.volume = Math.min(1, v / 100);
  }
  function mute() {
    if (native) cmd("mute");
    else if (video) video.muted = !video.muted;
  }

  // -------------------------------------------------------------- scrubbing
  function posFromEvent(e: PointerEvent) {
    const r = bar!.getBoundingClientRect();
    const x = Math.max(0, Math.min(r.width, e.clientX - r.left));
    return { x, t: (x / r.width) * (st.duration || 0) };
  }
  function barDown(e: PointerEvent) {
    dragging = true;
    bar!.setPointerCapture(e.pointerId);
    scrub = posFromEvent(e);
  }
  function barMove(e: PointerEvent) {
    scrub = posFromEvent(e);
    poke();
  }
  function barUp(e: PointerEvent) {
    if (dragging && scrub) seekTo(scrub.t);
    dragging = false;
  }

  // --------------------------------------------------------------- segments
  const activeSegment = $derived.by(() => {
    for (const [kind, list] of Object.entries(pb.segments ?? {})) {
      for (const s of list ?? []) {
        const end = s.end ?? st.duration;
        if (st.pos >= s.start && st.pos < end - 1) return { kind, start: s.start, end };
      }
    }
    return null;
  });
  const segLabel: Record<string, string> = { intro: "Skip intro", recap: "Skip recap", credits: "Skip credits", preview: "Skip preview" };

  $effect(() => {
    const a = activeSegment;
    if (!a) return;
    const key = `${pb.videoId}:${a.kind}:${a.start}`;
    const auto = (a.kind === "intro" && pbs.autoSkipIntro) || (a.kind === "recap" && pbs.autoSkipRecap) || (a.kind === "credits" && pbs.autoSkipCredits && !nextVideo());
    if (auto && !skipped.has(key)) {
      skipped.add(key);
      seekTo(a.end);
    }
  });

  // ---------------------------------------------------------------- up next
  const next = $derived(nextVideo());
  const creditsStart = $derived(pb.segments?.credits?.[0]?.start);
  const showUpNext = $derived(
    !!next && !upNextDismissed && st.duration > 60 && (creditsStart ? st.pos >= creditsStart : st.duration - st.pos <= pbs.nextCardSecs),
  );
  $effect(() => {
    void pb.videoId;
    upNextDismissed = false;
  });
  let cdTimer: ReturnType<typeof setInterval> | undefined;
  $effect(() => {
    clearInterval(cdTimer);
    if (!showUpNext || !pbs.autoplayNext) {
      countdown = 0;
      return;
    }
    countdown = 10;
    cdTimer = setInterval(() => {
      countdown -= 1;
      if (countdown <= 0) {
        clearInterval(cdTimer);
        playNext();
      }
    }, 1000);
    return () => clearInterval(cdTimer);
  });
  $effect(() => {
    if (st.ended && next && pbs.autoplayNext && !upNextDismissed) playNext();
  });

  // ----------------------------------------------------------------- tracks
  const audio = $derived(pb.tracks.filter((t) => t.type === "audio"));
  const subs = $derived(pb.tracks.filter((t) => t.type === "sub"));
  const subsOn = $derived(st.sid && st.sid !== "no" && st.sid !== "false");

  function trackName(t: any) {
    const parts = [t.title, lang(t.lang)].filter((x, i, a) => x && a.indexOf(x) === i);
    if (t.channels) parts.push(t.channels >= 6 ? `${t.channels - 1}.1` : "Stereo");
    if (t.codec) parts.push(t.codec.toUpperCase());
    return parts.join(" · ");
  }

  // ------------------------------------------------------------- torrent
  let torrent = $state<any>(null);
  $effect(() => {
    if (!pb.stream?.infoHash) return;
    const t = setInterval(() => api.torrentStats().then((s) => (torrent = s)), 2000);
    return () => clearInterval(t);
  });

  // ------------------------------------------------------------ fullscreen
  async function fullscreen() {
    await api.window("fullscreen");
    app.fullscreen = !app.fullscreen;
  }
  async function pip() {
    app.pip = !app.pip;
    await api.pip(app.pip);
  }

  // -------------------------------------------------------------- keyboard
  function onKey(e: KeyboardEvent) {
    if (!pb.open || (e.target as HTMLElement)?.tagName === "INPUT") return;
    const k = e.key;
    if (k === " " || k === "k") toggle();
    else if (k === "ArrowRight") seekBy(e.shiftKey ? pbs.seekLarge : pbs.seekSmall);
    else if (k === "ArrowLeft") seekBy(-(e.shiftKey ? pbs.seekLarge : pbs.seekSmall));
    else if (k === "ArrowUp") setVolume(Math.min(pbs.volumeBoost ? 200 : 100, st.volume + 5));
    else if (k === "ArrowDown") setVolume(Math.max(0, st.volume - 5));
    else if (k === "f" || k === "F11") fullscreen();
    else if (k === "m") mute();
    else if (k === "p") pip();
    else if (k === "s") cmd("sid", { value: subsOn ? "no" : (subs[0]?.id ?? "auto") });
    else if (k === "n" && next) playNext();
    else if (k === "Escape") {
      if (menu) menu = "";
      else if (app.fullscreen) fullscreen();
      else if (app.pip) pip();
      else close();
    } else return;
    e.preventDefault();
    poke();
  }

  const quality = $derived.by(() => {
    const v = st.video;
    const h = v.height || 0;
    const tags = [];
    if (h >= 2000) tags.push("4K");
    else if (h >= 1000) tags.push("1080p");
    else if (h >= 700) tags.push("720p");
    if (v.hdr) tags.push(v.gamma === "hlg" ? "HLG" : "HDR");
    if (pb.stream?._lumen?.hdr?.includes("DV")) tags.push("Dolby Vision");
    return tags;
  });

  const bg = $derived(pb.meta?.background ?? pb.meta?.poster);
  const progress = $derived(st.duration ? (st.pos / st.duration) * 100 : 0);
  const buffered = $derived(st.duration ? Math.min(100, ((st.pos + st.cacheAhead) / st.duration) * 100) : 0);
</script>

<svelte:window onkeydown={onKey} onmousemove={poke} />

<div class="player" class:hide-cursor={chromeHidden} class:native ondblclick={fullscreen} role="presentation">
  {#if pb.mode === "web" && pb.url}
    <!-- svelte-ignore a11y_media_has_caption -->
    <video bind:this={video} src={pb.url} autoplay playsinline onclick={toggle}></video>
  {:else if pb.mode === "youtube" && pb.ytId}
    <div class="yt"><YouTube id={pb.ytId} muted={false} onplaying={() => (pb.loading = false)} onended={close} /></div>
  {/if}

  <!-- click-catcher over the video for play/pause -->
  {#if native}<div class="catch" onclick={toggle} role="presentation"></div>{/if}

  {#if pb.loading || (st.buffering && st.pos < 1)}
    <div class="loading" class:over-art={st.pos < 1}>
      {#if st.pos < 1 && bg}<img class="art" src={img(bg, "original")} alt="" />{/if}
      <div class="load-center">
        {#if pb.meta?.logo}<img class="load-logo" src={img(pb.meta.logo)} alt="" />{/if}
        <div class="ring"></div>
        {#if torrent?.peers != null}
          <p class="foot dim">{torrent.peers} peers · {(torrent.downloadMbps ?? 0).toFixed(1)} MB/s</p>
        {:else if st.bufferingPct > 0}
          <p class="foot dim">{st.bufferingPct}%</p>
        {/if}
      </div>
    </div>
  {:else if st.buffering}
    <div class="mini-ring"></div>
  {/if}

  {#if pb.error}
    <div class="error glass strong">
      <p class="headline">Playback stopped</p>
      <p class="foot dim">{pb.error}</p>
      <div class="row">
        <button class="btn tinted press" onclick={close}>Choose another stream</button>
      </div>
    </div>
  {/if}

  <!-- subtitles -->
  {#if pb.cue && st.subOverlay && subsOn}
    <div class="subs" class:top={pb.cue.top} class:lifted={!chromeHidden} style:--sub-bottom="{sub.bottomOffset}vh">
      {#key pb.cue.key}
        <div class="cue" class:anim={sub.animateCues} data-bg={sub.background} style:font-size="calc({sub.size} * clamp(20px, 2.4vw, 46px))" style:font-weight={sub.weight} style:color={sub.color} style:--bg-o={sub.bgOpacity} style:--blur="{sub.blur}px" style:font-family={sub.font === "system-ui" ? undefined : sub.font}>
          {#each pb.cue.lines as line}
            <span class="line" dir={line.rtl && sub.rtlFix ? "rtl" : undefined}>
              {#each line.segments as seg}<span class:sdh={seg.sdh && sub.styleSdh} class:speaker={seg.speaker && sub.styleSdh} class:it={seg.italic}>{seg.text}</span>{/each}
            </span>
          {/each}
        </div>
      {/key}
    </div>
  {/if}

  <!-- skip -->
  {#if activeSegment && pbs.showSkipButtons && !showUpNext}
    <button class="skip glass press" onclick={() => seekTo(activeSegment.end)}>{segLabel[activeSegment.kind] ?? "Skip"} <Icon name="next" size={14} /></button>
  {/if}

  <!-- up next -->
  {#if showUpNext && next}
    <div class="upnext glass strong">
      {#if next.thumbnail}<img src={img(next.thumbnail)} alt="" />{/if}
      <div class="un-txt">
        <span class="foot faint">Up next · {episodeLabel(next.id)}</span>
        <span class="headline clamp-2">{next.title || next.name || `Episode ${next.episode}`}</span>
        <div class="row">
          <button class="btn primary small press" onclick={playNext}><Icon name="play" size={14} /> {countdown > 0 ? `Playing in ${countdown}` : "Play now"}</button>
          <button class="btn tinted small press" onclick={() => (upNextDismissed = true)}>Watch credits</button>
        </div>
      </div>
    </div>
  {/if}

  <!-- top chrome -->
  <header class="top" class:gone={chromeHidden}>
    <button class="round glass press" onclick={close} aria-label="Close player"><Icon name="back" size={20} /></button>
    <div class="ttl">
      {#if pb.meta?.logo}<img src={img(pb.meta.logo)} alt={pb.meta.name} />{:else}<span class="title3">{pb.meta?.name}</span>{/if}
      {#if pb.episodeTitle}<span class="foot dim">{episodeLabel(pb.videoId)} · {pb.episodeTitle}</span>{/if}
    </div>
    <div class="badges">
      {#each quality as q}<span class="chip">{q}</span>{/each}
      {#if pb.mode === "web" && pb.url}
        <!-- the system player can't do MKV/HEVC: hand off to a real player -->
        <button class="chip press" onclick={() => api.openExternal(`vlc-x-callback://x-callback-url/stream?url=${encodeURIComponent(pb.url)}`)}>Open in VLC</button>
        <button class="chip press" onclick={() => api.openExternal(`infuse://x-callback-url/play?url=${encodeURIComponent(pb.url)}`)}>Open in Infuse</button>
      {/if}
    </div>
  </header>

  <!-- bottom chrome -->
  <footer class="bottom glass" class:gone={chromeHidden}>
    <div class="scrub" bind:this={bar} onpointerdown={barDown} onpointermove={barMove} onpointerup={barUp} onpointerleave={() => !dragging && (scrub = null)} role="slider" aria-label="Seek" aria-valuemin={0} aria-valuemax={st.duration} aria-valuenow={st.pos} tabindex="0">
      <div class="track">
        <div class="buf" style:width="{buffered}%"></div>
        {#each Object.entries(pb.segments ?? {}) as [kind, list]}
          {#each list ?? [] as s}
            <div class="seg seg-{kind}" style:left="{(s.start / (st.duration || 1)) * 100}%" style:width="{(((s.end ?? st.duration) - s.start) / (st.duration || 1)) * 100}%"></div>
          {/each}
        {/each}
        <div class="done" style:width="{dragging && scrub ? (scrub.t / (st.duration || 1)) * 100 : progress}%"></div>
      </div>
      {#if scrub}
        <div class="tip glass strong" style:left="{scrub.x}px">{time(scrub.t)}</div>
      {/if}
    </div>
    <div class="ctl">
      <div class="grp">
        <button class="ic press" onclick={toggle} aria-label={st.paused ? "Play" : "Pause"}><Icon name={st.paused ? "play" : "pause"} size={26} /></button>
        <button class="ic press" onclick={() => seekBy(-pbs.seekSmall)} aria-label="Back {pbs.seekSmall} seconds"><Icon name="skipB" size={22} /></button>
        <button class="ic press" onclick={() => seekBy(pbs.seekSmall)} aria-label="Forward {pbs.seekSmall} seconds"><Icon name="skipF" size={22} /></button>
        {#if next}<button class="ic press" onclick={playNext} aria-label="Next episode" title="Next episode"><Icon name="next" size={20} /></button>{/if}
        <div class="vol">
          <button class="ic press" onclick={mute} aria-label={st.muted ? "Unmute" : "Mute"}><Icon name={st.muted || st.volume === 0 ? "mute" : "volume"} size={22} /></button>
          <input type="range" min="0" max={pbs.volumeBoost ? 200 : 100} value={st.volume} oninput={(e) => setVolume(+(e.target as HTMLInputElement).value)} aria-label="Volume" style:--p="{(st.volume / (pbs.volumeBoost ? 200 : 100)) * 100}%" />
        </div>
        <span class="time foot">{time(st.pos)} <span class="dim">/ {time(st.duration)}</span></span>
      </div>
      <div class="grp">
        {#if native}
          <button class="ic press" class:on={menu === "subs"} onclick={() => (menu = menu === "subs" ? "" : "subs")} aria-label="Subtitles"><Icon name="subtitles" size={22} /></button>
          <button class="ic press" class:on={menu === "audio"} onclick={() => (menu = menu === "audio" ? "" : "audio")} aria-label="Audio"><Icon name="audio" size={21} /></button>
          <button class="ic press" class:on={menu === "picture"} onclick={() => (menu = menu === "picture" ? "" : "picture")} aria-label="Picture"><Icon name="tv" size={21} /></button>
        {/if}
        <button class="ic press" class:on={menu === "speed"} onclick={() => (menu = menu === "speed" ? "" : "speed")} aria-label="Speed"><span class="spd">{st.speed}×</span></button>
        {#if app.native}<button class="ic press" onclick={pip} aria-label="Picture in picture"><Icon name="pip" size={21} /></button>{/if}
        <button class="ic press" onclick={fullscreen} aria-label="Fullscreen"><Icon name={app.fullscreen ? "exitFullscreen" : "fullscreen"} size={21} /></button>
      </div>
    </div>

    {#if menu}
      <div class="menu glass strong">
        {#if menu === "subs"}
          <h4 class="foot faint">Subtitles</h4>
          <button class="opt" class:sel={!subsOn} onclick={() => cmd("sid", { value: "no" })}>Off</button>
          {#each subs as t (t.id)}
            <button class="opt" class:sel={t.selected} onclick={() => cmd("sid", { value: t.id })}>{trackName(t)}{#if t.external}<span class="faint"> · addon</span>{/if}</button>
          {/each}
          <div class="delay">
            <span class="foot dim">Delay {st.subDelay.toFixed(1)} s</span>
            <button class="btn tinted small press" onclick={() => cmd("sub-delay", { value: st.subDelay - 0.1 })}>−0.1</button>
            <button class="btn tinted small press" onclick={() => cmd("sub-delay", { value: st.subDelay + 0.1 })}>+0.1</button>
          </div>
        {:else if menu === "audio"}
          <h4 class="foot faint">Audio</h4>
          {#each audio as t (t.id)}
            <button class="opt" class:sel={t.selected} onclick={() => cmd("aid", { value: t.id })}>{trackName(t)}</button>
          {/each}
        {:else if menu === "speed"}
          <h4 class="foot faint">Speed</h4>
          {#each [0.5, 0.75, 1, 1.25, 1.5, 2] as s}
            <button class="opt" class:sel={st.speed === s} onclick={() => (native ? cmd("speed", { value: s }) : video && (video.playbackRate = s))}>{s}×</button>
          {/each}
        {:else if menu === "picture"}
          <h4 class="foot faint">Picture</h4>
          {#each [["fit", "Fit"], ["fill", "Fill screen"], ["stretch", "Stretch"]] as [v, l]}
            <button class="opt" onclick={() => cmd("aspect", { value: v })}>{l}</button>
          {/each}
          <p class="foot faint info">{st.video.width}×{st.video.height} · {st.video.codec} · {st.video.hwdec && st.video.hwdec !== "no" ? `hardware (${st.video.hwdec})` : "software"} · {st.video.fps?.toFixed(2)} fps</p>
        {/if}
      </div>
    {/if}
  </footer>
</div>

<style>
  /* Video is always dark, whatever the theme: the player's chrome uses its
     own dark material so it stays readable over any picture. */
  .player {
    position: fixed;
    inset: 0;
    z-index: 100;
    background: #000;
    color: #fff;
    color-scheme: dark;
    --label: #fff;
    --label-2: rgb(255 255 255 / 0.72);
    --label-3: rgb(255 255 255 / 0.45);
    --text: #fff;
    --text-dim: rgb(255 255 255 / 0.72);
    --text-faint: rgb(255 255 255 / 0.45);
    --fill-1: rgb(255 255 255 / 0.28);
    --fill-2: rgb(255 255 255 / 0.2);
    --fill-3: rgb(255 255 255 / 0.14);
    --fill-4: rgb(255 255 255 / 0.08);
    --separator: rgb(255 255 255 / 0.14);
    --glass-tint: rgb(16 16 20 / 0.52);
    --glass-tint-strong: rgb(14 14 18 / 0.88);
    --glass-spec: rgb(255 255 255 / 0.32);
    --glass-rim: rgb(255 255 255 / 0.12);
    --glass-edge: rgb(0 0 0 / 0.3);
    --glass-shadow: 0 12px 36px -12px rgb(0 0 0 / 0.6);
  }
  .player.native {
    background: transparent;
  }
  .hide-cursor,
  .hide-cursor * {
    cursor: none !important;
  }
  video,
  .yt {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    background: #000;
  }
  .catch {
    position: absolute;
    inset: 0;
  }
  .loading {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
  }
  .loading.over-art {
    background: #000;
  }
  .art {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    filter: blur(28px) brightness(0.45) saturate(1.3);
    transform: scale(1.08);
    animation: breathe 6s ease-in-out infinite alternate;
  }
  @keyframes breathe {
    to {
      transform: scale(1.14);
    }
  }
  .load-center {
    position: relative;
    display: grid;
    justify-items: center;
    gap: 22px;
  }
  .load-logo {
    max-width: 360px;
    max-height: 120px;
    object-fit: contain;
    filter: drop-shadow(0 4px 20px rgb(0 0 0 / 0.5));
  }
  .ring,
  .mini-ring {
    width: 38px;
    height: 38px;
    border-radius: 50%;
    border: 3px solid rgb(255 255 255 / 0.18);
    border-top-color: #fff;
    animation: spin 0.9s linear infinite;
  }
  .mini-ring {
    position: absolute;
    left: calc(50% - 19px);
    top: calc(50% - 19px);
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .error {
    position: absolute;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    padding: 22px 26px;
    border-radius: var(--r-lg);
    max-width: 460px;
    text-align: center;
  }
  .error p {
    margin: 0 0 6px;
  }
  .row {
    display: flex;
    gap: 8px;
    justify-content: center;
    margin-top: 12px;
  }

  .subs {
    position: absolute;
    left: 6%;
    right: 6%;
    bottom: var(--sub-bottom);
    display: flex;
    justify-content: center;
    pointer-events: none;
    transition: bottom var(--t-med) var(--glide);
  }
  .subs.lifted {
    bottom: calc(var(--sub-bottom) + 92px);
  }
  .subs.top {
    top: 8vh;
    bottom: auto;
  }
  .cue {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.08em;
    line-height: 1.22;
    text-align: center;
  }
  .cue.anim {
    animation: cue-in 180ms var(--ease-out) both;
  }
  @keyframes cue-in {
    from {
      opacity: 0;
      transform: translateY(4px);
    }
  }
  .line {
    padding: 0.06em 0.5em;
    border-radius: 0.32em;
  }
  .cue[data-bg="glass"] .line {
    background: rgb(0 0 0 / var(--bg-o));
    backdrop-filter: blur(var(--blur)) saturate(140%);
    -webkit-box-decoration-break: clone;
  }
  .cue[data-bg="box"] .line {
    background: rgb(0 0 0 / var(--bg-o));
  }
  .cue[data-bg="shadow"] {
    text-shadow: 0 2px 6px rgb(0 0 0 / 0.95), 0 0 2px rgb(0 0 0 / 0.95);
  }
  .cue[data-bg="outline"] {
    -webkit-text-stroke: 0.12em rgb(0 0 0 / 0.85);
    paint-order: stroke fill;
  }
  .sdh {
    font-style: italic;
    opacity: 0.72;
    font-size: 0.86em;
  }
  .speaker {
    opacity: 0.66;
    font-size: 0.82em;
    letter-spacing: 0.02em;
  }
  .it {
    font-style: italic;
  }

  .skip {
    position: absolute;
    right: var(--gutter);
    bottom: 128px;
    display: flex;
    align-items: center;
    gap: 8px;
    height: 46px;
    padding: 0 20px;
    border-radius: var(--r-pill);
    font-weight: 650;
    animation: pop var(--t-slow) var(--spring) both;
  }
  @keyframes pop {
    from {
      opacity: 0;
      transform: translateX(20px) scale(0.9);
    }
  }
  .upnext {
    position: absolute;
    right: var(--gutter);
    bottom: 128px;
    width: 420px;
    display: flex;
    gap: 14px;
    padding: 12px;
    border-radius: var(--r-lg);
    animation: pop var(--t-slow) var(--spring) both;
  }
  .upnext img {
    width: 150px;
    aspect-ratio: 16 / 9;
    object-fit: cover;
    border-radius: var(--r-sm);
  }
  .un-txt {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }
  .un-txt .row {
    justify-content: flex-start;
    margin-top: auto;
  }

  .top {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 20px var(--gutter) 40px;
    background: linear-gradient(180deg, rgb(0 0 0 / 0.55), transparent);
    transition: opacity var(--t-med) ease, transform var(--t-med) var(--glide);
  }
  .round {
    width: 44px;
    height: 44px;
    border-radius: 50%;
    display: grid;
    place-items: center;
  }
  .ttl {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .ttl img {
    max-height: 42px;
    max-width: 260px;
    object-fit: contain;
    object-position: left center;
  }
  .badges {
    margin-left: auto;
    display: flex;
    gap: 6px;
  }
  .badges .chip {
    background: rgb(0 0 0 / 0.42);
    color: #fff;
    backdrop-filter: blur(12px);
    box-shadow: inset 0 0 0 0.5px rgb(255 255 255 / 0.18);
  }
  .bottom {
    position: absolute;
    left: var(--gutter);
    right: var(--gutter);
    bottom: 22px;
    padding: 10px 18px 10px;
    border-radius: var(--r-xl);
    transition: opacity var(--t-med) ease, transform var(--t-med) var(--glide);
  }
  .gone {
    opacity: 0;
    pointer-events: none;
  }
  .bottom.gone {
    transform: translateY(16px);
  }
  .top.gone {
    transform: translateY(-16px);
  }
  .scrub {
    position: relative;
    height: 22px;
    display: flex;
    align-items: center;
    cursor: pointer;
    touch-action: none;
  }
  .track {
    position: relative;
    width: 100%;
    height: 5px;
    border-radius: 5px;
    background: rgb(255 255 255 / 0.2);
    overflow: hidden;
    transition: height var(--t-fast) var(--spring);
  }
  .scrub:hover .track {
    height: 9px;
  }
  .buf {
    position: absolute;
    inset: 0 auto 0 0;
    background: rgb(255 255 255 / 0.25);
  }
  .seg {
    position: absolute;
    top: 0;
    bottom: 0;
    background: rgb(255 214 10 / 0.45);
  }
  .seg-credits {
    background: rgb(100 210 255 / 0.4);
  }
  .seg-recap {
    background: rgb(191 90 242 / 0.4);
  }
  .done {
    position: absolute;
    inset: 0 auto 0 0;
    background: #fff;
    border-radius: inherit;
  }
  .tip {
    position: absolute;
    bottom: 26px;
    transform: translateX(-50%);
    padding: 4px 10px;
    border-radius: 8px;
    font-size: 13px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }
  .ctl {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-top: 4px;
  }
  .grp {
    display: flex;
    align-items: center;
    gap: 2px;
  }
  .ic {
    width: 44px;
    height: 44px;
    border-radius: 50%;
    display: grid;
    place-items: center;
  }
  .ic:hover,
  .ic.on {
    background: rgb(255 255 255 / 0.12);
  }
  .spd {
    font-weight: 700;
    font-size: 14px;
  }
  .vol {
    display: flex;
    align-items: center;
  }
  .vol input {
    -webkit-appearance: none;
    appearance: none;
    width: 0;
    opacity: 0;
    height: 24px;
    background: transparent;
    transition: width var(--t-med) var(--spring), opacity var(--t-fast);
  }
  .vol:hover input {
    width: 110px;
    opacity: 1;
    margin-right: 8px;
  }
  .vol input::-webkit-slider-runnable-track {
    height: 4px;
    border-radius: 4px;
    background: linear-gradient(90deg, #fff var(--p), rgb(255 255 255 / 0.25) var(--p));
  }
  .vol input::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 14px;
    height: 14px;
    margin-top: -5px;
    border-radius: 50%;
    background: #fff;
  }
  .time {
    margin-left: 10px;
    font-variant-numeric: tabular-nums;
    font-weight: 600;
  }
  .menu {
    position: absolute;
    right: 12px;
    bottom: calc(100% + 10px);
    min-width: 280px;
    max-height: 50vh;
    overflow-y: auto;
    padding: 10px;
    border-radius: var(--r-lg);
    animation: menu-in var(--t-med) var(--spring) both;
    transform-origin: bottom right;
  }
  @keyframes menu-in {
    from {
      opacity: 0;
      transform: scale(0.9);
    }
  }
  h4 {
    margin: 4px 10px 6px;
    font-weight: 600;
  }
  .opt {
    display: block;
    width: 100%;
    text-align: left;
    padding: 9px 12px;
    border-radius: 10px;
    font-size: 14px;
  }
  .opt:hover {
    background: rgb(255 255 255 / 0.08);
  }
  .opt.sel {
    background: rgb(255 255 255 / 0.16);
    font-weight: 650;
  }
  .delay {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px 10px 2px;
  }
  .delay span {
    margin-right: auto;
  }
  .info {
    margin: 8px 12px 4px;
  }
</style>
