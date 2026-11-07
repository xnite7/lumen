<script lang="ts">
  // The player UI. On desktop the video itself is mpv, drawn underneath this
  // transparent page -- everything here is the glass floating over it.
  import { api, img } from "../lib/api";
  import { app } from "../lib/stores/app.svelte";
  import { pb, close, cmd, nextVideo, playNext, playVideo, orderedVideos } from "../lib/stores/playback.svelte";
  import { progressOf, watchedIds } from "../lib/stores/library.svelte";
  import EpisodeCarousel from "../components/player/EpisodeCarousel.svelte";
  import EpisodesPopover from "../components/player/EpisodesPopover.svelte";
  import { time, lang, episodeLabel } from "../lib/format";
  import YouTube from "../components/YouTube.svelte";
  import Icon from "../components/Icon.svelte";
  import EyeBubbles, { type Bubble } from "../components/EyeBubbles.svelte";
  import { saveSettings, tobiiOn, hueOn } from "../lib/stores/app.svelte";
  import wobble from "../assets/brand/couch-wobble.webp";
  import { gaze } from "../lib/stores/gaze.svelte";

  let idle = $state(false);
  let idleTimer: ReturnType<typeof setTimeout> | undefined;
  let menu = $state<"" | "tracks" | "episodes" | "volume" | "more">("");
  let menuX = $state(0);
  function openMenu(m: typeof menu, e: MouseEvent) {
    if (menu === m) return void (menu = "");
    const b = (e.currentTarget as HTMLElement).getBoundingClientRect();
    menuX = b.left + b.width / 2;
    menu = m;
  }
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
  // paused and the mouse left alone: the "you're watching" screen
  let restful = $state(false);
  let restTimer: ReturnType<typeof setTimeout> | undefined;
  function poke() {
    restful = false;
    clearTimeout(restTimer);
    restTimer = setTimeout(() => {
      if (st.paused && !menu && !dragging) restful = true;
    }, 6000);
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

  // ------------------------------------------------------------ eye controls
  const eyeItems = $derived.by((): Bubble[] => {
    const n = pbs.seekSmall;
    const list: Bubble[] = [];
    if (next) list.push({ id: "next", icon: "next", label: "Next episode", run: playNext });
    list.push({ id: "fwd", icon: "skipF", badge: String(n), label: `Forward ${n} seconds`, run: () => seekBy(n), repeat: true });
    list.push({ id: "toggle", icon: st.paused ? "play" : "pause", label: st.paused ? "Play" : "Pause", run: toggle });
    list.push({ id: "back", icon: "skipB", badge: String(n), label: `Back ${n} seconds`, run: () => seekBy(-n), repeat: true });
    if (activeSegment) list.push({ id: "skip", icon: "skipAhead", label: segLabel[activeSegment.kind] ?? "Skip", run: () => seekTo(activeSegment!.end) });
    // the icon shows what looking at it will do
    else list.push({ id: "mute", icon: st.muted ? "volume" : "mute", label: st.muted ? "Turn sound on" : "Mute", run: mute });
    return list;
  });
  // ------------------------------------------------- head-tracked 3D
  // 3D releases (SBS / OU) go through a depth shader in mpv; your head
  // position picks the camera, so the screen behaves like a window.
  let mode3d = $state<"auto" | "off" | "sbs" | "ou">("auto");
  const layout3d = $derived<"" | "sbs" | "ou">(
    !native ? "" : mode3d === "off" ? "" : mode3d === "auto" ? (pb.stream?._couch?.threeDLayout ?? "") : mode3d,
  );
  let applied3d = "";
  $effect(() => {
    const w = st.video.width;
    const h = st.video.height;
    const want = layout3d && w && h ? layout3d : "";
    const key = want ? `${want}:${w}x${h}` : "";
    if (key === applied3d) return;
    applied3d = key;
    if (!want) {
      cmd("3d-off").catch(() => {});
      return;
    }
    // full-width SBS / full-height OU: each eye is already the right shape
    const r = w / h;
    const aspect = want === "sbs" && r > 2.5 ? r / 2 : want === "ou" && r < 1.2 ? r * 2 : 0;
    cmd("3d-on", { layout: want, aspect }).catch(() => {});
  });
  // follow the head: self-centring, smoothed, sent ~30 times a second
  $effect(() => {
    if (!layout3d || !tobiiOn() || !app.settings.tobii.depth3d) return;
    let raf = 0;
    let centre: number | null = null;
    let cur = 0;
    let sent = 99;
    let lastSend = 0;
    const tick = (now: number) => {
      raf = requestAnimationFrame(tick);
      const hx = gaze.head.x;
      if (centre === null) centre = hx;
      centre += (hx - centre) * 0.002;
      const t = Math.max(-1, Math.min(1, (hx - centre) / 160));
      cur += (t - cur) * 0.2;
      if (now - lastSend > 33 && Math.abs(cur - sent) > 0.003) {
        sent = cur;
        lastSend = now;
        cmd("3d-view", { x: cur, strength: app.settings.tobii.parallaxStrength }).catch(() => {});
      }
    };
    raf = requestAnimationFrame(tick);
    return () => {
      cancelAnimationFrame(raf);
      cmd("3d-view", { x: 0, strength: 1 }).catch(() => {});
    };
  });

  // shown in the middle while the eye controls are up
  const leftSecs = $derived(Math.max(0, (st.duration || 0) - (st.pos || 0)));
  const timeLeft = $derived.by(() => {
    if (!st.duration) return "";
    const m = Math.round(leftSecs / 60);
    if (m < 1) return "under a minute left";
    const h = Math.floor(m / 60);
    return h ? `${h}h ${m % 60}m left` : `${m} min left`;
  });
  const endsAt = $derived(st.duration ? new Date(Date.now() + leftSecs * 1000 / (st.speed || 1)).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" }).toLowerCase() : "");
  const HINT_KEY = "couch.eyeHint.v1";
  let eyeHint = $state(false);
  $effect(() => {
    if (!tobiiOn() || !app.settings.tobii.gazeControls || pb.meta?.type !== "series") return;
    let seen = false;
    try {
      seen = !!localStorage.getItem(HINT_KEY);
    } catch {}
    if (!seen) eyeHint = true;
  });
  function hintDone() {
    eyeHint = false;
    try {
      localStorage.setItem(HINT_KEY, "1");
    } catch {}
  }
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

  // Chameleon progress bar: it takes the picture's overall colour, a little
  // brighter and more vivid. Windows reads the window (like the Hue
  // ambilight); the in-app video is sampled straight off its frames.
  let scene = $state<string | undefined>(undefined);
  function vivid(hex: string) {
    const n = parseInt(hex.slice(1), 16);
    let r = ((n >> 16) & 255) / 255, g = ((n >> 8) & 255) / 255, b = (n & 255) / 255;
    const max = Math.max(r, g, b), min = Math.min(r, g, b), d = max - min;
    let h = 0;
    if (d) h = max === r ? ((g - b) / d) % 6 : max === g ? (b - r) / d + 2 : (r - g) / d + 4;
    h = (h * 60 + 360) % 360;
    const l0 = (max + min) / 2;
    const s0 = d ? d / (1 - Math.abs(2 * l0 - 1)) : 0;
    // a grey scene stays a soft warm white instead of turning muddy
    if (s0 < 0.08) return "hsl(30 30% 88%)";
    const s = Math.min(1, s0 * 1.35 + 0.15);
    const l = Math.min(0.74, Math.max(0.55, l0 * 1.25 + 0.12));
    return `hsl(${Math.round(h)} ${Math.round(s * 100)}% ${Math.round(l * 100)}%)`;
  }
  let sampleCanvas: HTMLCanvasElement | null = null;
  function sampleVideo(): string | null {
    if (!video || video.readyState < 2) return null;
    try {
      sampleCanvas ??= document.createElement("canvas");
      sampleCanvas.width = 32;
      sampleCanvas.height = 18;
      const g = sampleCanvas.getContext("2d", { willReadFrequently: true })!;
      g.drawImage(video, 0, 0, 32, 18);
      const d = g.getImageData(0, 0, 32, 18).data;
      let r = 0, gg = 0, b = 0, w = 0;
      for (let i = 0; i < d.length; i += 4) {
        const mx = Math.max(d[i], d[i + 1], d[i + 2]), mn = Math.min(d[i], d[i + 1], d[i + 2]);
        const k = 0.05 + ((mx - mn) / 255) * 3 + (mx / 255) * 0.2;
        r += d[i] * k; gg += d[i + 1] * k; b += d[i + 2] * k; w += k;
      }
      const hx = (v: number) => Math.round(v / w).toString(16).padStart(2, "0");
      return `#${hx(r)}${hx(gg)}${hx(b)}`;
    } catch {
      return null; // cross-origin frames can't be read
    }
  }
  $effect(() => {
    if (!pb.open) return;
    let alive = true;
    const tick = async () => {
      if (!alive) return;
      const hex = native ? await api.sceneColor().catch(() => null) : sampleVideo();
      if (hex) scene = vivid(hex);
      setTimeout(tick, 700);
    };
    tick();
    return () => (alive = false);
  });

  // Material's replay/forward glyphs carry the number for 5, 10 and 30
  const numbered = $derived([5, 10, 30].includes(pbs.seekSmall));
  const quality = $derived.by(() => {
    const v = st.video;
    const h = v.height || 0;
    const tags = [];
    if (h >= 2000) tags.push("4K");
    else if (h >= 1000) tags.push("1080p");
    else if (h >= 700) tags.push("720p");
    const c: any = pb.stream?._couch ?? {};
    if (c.hdr?.includes("DV")) tags.push("Dolby Vision");
    else if (v.hdr || c.hdr?.length) tags.push(v.gamma === "hlg" ? "HLG" : c.hdr?.includes("HDR10+") ? "HDR10+" : "HDR10");
    if (c.audio?.includes("Atmos")) tags.push("Dolby Atmos");
    return tags;
  });

  const bg = $derived(pb.meta?.background ?? pb.meta?.poster);

  // couch mode: one switch for the Hue lamp (dim + warm while playing)
  const hueReady = $derived(hueOn() && !!app.secrets.hue_username);
  const couchMode = $derived(app.settings.hue.playbackMode === "dim");
  async function toggleCouch() {
    const on = !couchMode;
    app.settings.hue.playbackMode = on ? "dim" : "off";
    await saveSettings();
    // re-run the playback hook so the lamp follows right away
    await api.hueTest(false).catch(() => {});
    if (on) await api.hueTest(true).catch(() => {});
  }
  // a paused VHS: dimmed frame, vignette, grain, the title in big lowercase
  const pausedScreen = $derived(st.paused && !pb.loading && !pb.error && st.pos > 1);
  const idleScreen = $derived(pausedScreen && restful && !menu);
  $effect(() => {
    document.documentElement.classList.toggle("chrome-off", chromeHidden || idleScreen);
    return () => document.documentElement.classList.remove("chrome-off");
  });
  function dragWindow(e: MouseEvent) {
    if (e.button !== 0 || (e.target as Element).closest("button, select, input, a")) return;
    if (e.detail === 2) api.window("maximize");
    else api.window("drag");
  }

  // ------------------------------------------------------------ episodes
  const isShow = $derived(pb.meta?.type === "series" && (pb.meta?.videos?.length ?? 0) > 1);
  const allVideos = $derived(orderedVideos(pb.meta));
  const nowVideo = $derived(allVideos.find((v) => v.id === pb.videoId));
  const seasons = $derived([...new Set(allVideos.map((v) => v.season ?? 0))].sort((a, b) => (a || 999) - (b || 999)));
  let season = $state(0);
  $effect(() => {
    season = nowVideo?.season ?? seasons[0] ?? 0;
  });
  const seasonVideos = $derived(allVideos.filter((v) => (v.season ?? 0) === season));
  const watched = $derived(pb.meta ? watchedIds(pb.meta.id) : new Set<string>());
  const epProgress = (v: { id: string }) => (pb.meta ? progressOf(pb.meta.id, v.id) : 0);
  function pickEpisode(v: any) {
    menu = "";
    playVideo(v);
  }
  const facts = $derived(
    [pb.meta?.type === "series" ? (nowVideo?.season ? `Season ${nowVideo.season}` : "") : "", pb.meta?.releaseInfo ?? "", pb.meta?.genres?.[0] ?? ""].filter(Boolean).join(" · "),
  );
  const blurb = $derived(nowVideo?.overview ?? nowVideo?.description ?? pb.meta?.description ?? "");
  const epShort = $derived(nowVideo ? `S${nowVideo.season}: Ep. ${nowVideo.episode ?? nowVideo.number}` : "");
  const progress = $derived(st.duration ? (st.pos / st.duration) * 100 : 0);
  const buffered = $derived(st.duration ? Math.min(100, ((st.pos + st.cacheAhead) / st.duration) * 100) : 0);
</script>

<svelte:window onkeydown={onKey} onmousemove={poke} />

<div class="player" class:hide-cursor={chromeHidden} class:native ondblclick={fullscreen} role="presentation" style:--scene={scene}>
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
        <img class="couch-load" src={wobble} alt="" />
        {#if torrent?.peers != null}
          <p class="foot dim">{torrent.peers} peers · {(torrent.downloadMbps ?? 0).toFixed(1)} MB/s</p>
        {:else if st.bufferingPct > 0}
          <p class="foot dim">{st.bufferingPct}%</p>
        {:else}
          <p class="bump-text load-line">finding the remote…</p>
        {/if}
      </div>
    </div>
  {:else if st.buffering}
    <div class="mini-ring"></div>
  {/if}

  {#if pb.error}
    <div class="error glass strong">
      <p class="headline">the tv got scared. try again?</p>
      <p class="foot dim">{pb.error}</p>
      <div class="row">
        <button class="btn tinted press" onclick={close}>pick another stream</button>
      </div>
    </div>
  {/if}

  <!-- subtitles -->
  {#if pb.cue && st.subOverlay && subsOn && !pausedScreen}
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
  {#if activeSegment && pbs.showSkipButtons && !showUpNext && !pausedScreen}
    <button class="skip press skip-{activeSegment.kind}" onclick={() => seekTo(activeSegment.end)}><Icon name="skipAhead" size={17} /> {segLabel[activeSegment.kind] ?? "Skip"}</button>
  {/if}

  {#if pausedScreen && !idleScreen}
    <!-- paused: the episode up close, the season fanned on the right -->
    <div class="paused" role="presentation">
      <div class="p-shade"></div>
      <div class="p-center">
        <button class="p-seek press" onclick={() => seekBy(-pbs.seekSmall)} aria-label="Back {pbs.seekSmall} seconds">{#if numbered}<Icon name={"replay" + pbs.seekSmall} size={36} />{:else}<Icon name="skipB" size={34} /><span>{pbs.seekSmall}</span>{/if}</button>
        <button class="p-play press" onclick={toggle} aria-label="Play"><Icon name="play" size={40} /></button>
        <button class="p-seek press" onclick={() => seekBy(pbs.seekSmall)} aria-label="Forward {pbs.seekSmall} seconds">{#if numbered}<Icon name={"forward" + pbs.seekSmall} size={36} />{:else}<Icon name="skipF" size={34} /><span>{pbs.seekSmall}</span>{/if}</button>
      </div>
      <div class="p-info">
        {#if isShow}<span class="p-show">{pb.meta?.name ?? ""}</span>{/if}
        {#if facts}<span class="p-facts">{facts}</span>{/if}
        <h2 class="p-title">{pb.episodeTitle || pb.meta?.name || ""}</h2>
        {#if blurb}<p class="p-desc">{blurb}</p>{/if}
      </div>
      {#if isShow}
        <EpisodeCarousel videos={seasonVideos} currentId={pb.videoId} {timeLeft} {watched} progress={epProgress} onplay={pickEpisode} />
      {/if}
    </div>
  {/if}

  {#if idleScreen}
    <!-- paused and left alone: who's watching what, like a screensaver -->
    <div class="resting" role="presentation" onclick={() => (restful = false)}>
      <div class="r-copy">
        <span class="r-eyebrow">you're watching</span>
        <h2 class="r-title">{pb.meta?.name ?? ""}</h2>
        {#if epShort}<span class="r-ep">{epShort}</span>{/if}
        {#if pb.episodeTitle}<span class="r-name">{pb.episodeTitle}</span>{/if}
        {#if blurb}<p class="r-desc">{blurb}</p>{/if}
      </div>
    </div>
  {/if}

  <!-- up next -->
  {#if showUpNext && next}
    <div class="upnext glass strong">
      {#if next.thumbnail}<img src={img(next.thumbnail)} alt="" />{/if}
      <div class="un-txt">
        <span class="foot faint">up next · {episodeLabel(next.id)}</span>
        <span class="headline clamp-2">{next.title || next.name || `Episode ${next.episode}`}</span>
        <div class="row">
          <button class="btn primary small press un-play" onclick={playNext}>
            {#if countdown > 0}
              <svg class="lamp-ring" viewBox="0 0 20 20" aria-hidden="true"><circle cx="10" cy="10" r="8" /><circle class="fg" cx="10" cy="10" r="8" style:stroke-dashoffset={50.3 * (countdown / 10)} /></svg>
              playing in {countdown}
            {:else}<Icon name="play" size={14} /> play now{/if}
          </button>
          <button class="btn tinted small press" onclick={() => (upNextDismissed = true)}>watch credits</button>
        </div>
      </div>
    </div>
  {/if}

  <EyeBubbles items={eyeItems} {timeLeft} {endsAt} onopen={() => eyeHint && hintDone()} />
  {#if eyeHint}
    <div class="eye-hint">
      <div class="demo" aria-hidden="true">
        <span class="screen"></span>
        <span class="dot"></span>
        {#each Array(4) as _, i}<span class="mini" style:--i={i}></span>{/each}
      </div>
      <div class="hint-txt">
        <span class="headline">Eye controls</span>
        <span class="foot">Look at the <b>bottom-left corner</b>. Bubbles float up on the left. Rest your eyes on one to pause, skip back, or jump to the next episode.</span>
        <button class="btn primary small press" onclick={hintDone}>Got it</button>
      </div>
    </div>
  {/if}

  <!-- top chrome -->
  <!-- the empty part of the top bar moves the window, like a title bar -->
  <header class="top" class:gone={chromeHidden || idleScreen} onmousedown={dragWindow} role="presentation">
    <button class="round glass press" onclick={close} aria-label="Close player"><Icon name="back" size={20} /></button>
    <div class="ttl">
      {#if !pausedScreen}
        {#if pb.meta?.logo}<img src={img(pb.meta.logo)} alt={pb.meta.name} />{:else}<span class="title3">{pb.meta?.name}</span>{/if}
      {/if}
    </div>
    <div class="badges">
      {#if pausedScreen && isShow && seasons.length > 1}
        <label class="season-pick">
          <select bind:value={season} aria-label="Season">
            {#each seasons as n}<option value={n}>{n === 0 ? "Specials" : `Season ${n}`}</option>{/each}
          </select>
          <Icon name="chevronDown" size={14} />
        </label>
      {/if}
      {#each quality as q}
        {@const dolby = q.startsWith("Dolby ")}
        <span class="spec" class:dolby>{#if dolby}<b>DOLBY</b><i>{q.slice(6).toUpperCase()}</i>{:else}{q}{/if}</span>
      {/each}
      {#if layout3d}<span class="chip">3D{tobiiOn() && app.settings.tobii.depth3d ? " · look around" : ""}</span>{/if}
    </div>
  </header>

  <!-- bottom chrome -->
  <footer class="bottom" class:gone={chromeHidden || idleScreen}>
    <div class="scrub-row">
      <div class="scrub" bind:this={bar} onpointerdown={barDown} onpointermove={barMove} onpointerup={barUp} onpointerleave={() => !dragging && (scrub = null)} role="slider" aria-label="Seek" aria-valuemin={0} aria-valuemax={st.duration} aria-valuenow={st.pos} tabindex="0">
        <div class="track">
          <div class="buf" style:width="{buffered}%"></div>
          <div class="done" style:width="{dragging && scrub ? (scrub.t / (st.duration || 1)) * 100 : progress}%"></div>
          {#each Object.entries(pb.segments ?? {}) as [kind, list]}
            {#each list ?? [] as s}
              <div class="seg seg-{kind}" style:left="{(s.start / (st.duration || 1)) * 100}%" style:width="{(((s.end ?? st.duration) - s.start) / (st.duration || 1)) * 100}%"></div>
            {/each}
          {/each}
        </div>
        {#if scrub}
          <div class="tip glass strong" style:left="{scrub.x}px">{time(scrub.t)}</div>
        {/if}
      </div>
      <span class="remain tabular">{time(Math.max(0, (st.duration || 0) - (st.pos || 0)))}</span>
    </div>

    <!-- paused: the big middle buttons take over, so the button bar goes -->
    <div class="bar glass strong" class:gone={pausedScreen}>
      <button class="ic press" onclick={toggle} aria-label={st.paused ? "Play" : "Pause"}><Icon name={st.paused ? "play" : "pause"} size={22} /></button>
      <button class="ic press seekic" onclick={() => seekBy(-pbs.seekSmall)} aria-label="Back {pbs.seekSmall} seconds">{#if numbered}<Icon name={"replay" + pbs.seekSmall} size={24} />{:else}<Icon name="skipB" size={22} /><span>{pbs.seekSmall}</span>{/if}</button>
      <button class="ic press seekic" onclick={() => seekBy(pbs.seekSmall)} aria-label="Forward {pbs.seekSmall} seconds">{#if numbered}<Icon name={"forward" + pbs.seekSmall} size={24} />{:else}<Icon name="skipF" size={22} /><span>{pbs.seekSmall}</span>{/if}</button>
      <div class="now">
        <span class="now-show">{pb.meta?.name ?? ""}</span>
        {#if epShort}<span class="now-ep">{epShort} {pb.episodeTitle}</span>{/if}
      </div>
      {#if next}<button class="ic press" onclick={playNext} aria-label="Next episode" title="Next episode"><Icon name="next" size={20} /></button>{/if}
      <button class="ic press" class:on={menu === "volume"} onclick={(e) => openMenu("volume", e)} aria-label="Volume"><Icon name={st.muted || st.volume === 0 ? "mute" : "volume"} size={21} /></button>
      {#if native}<button class="ic press" class:on={menu === "tracks"} onclick={(e) => openMenu("tracks", e)} aria-label="Audio and subtitles"><Icon name="subtitles" size={21} /></button>{/if}
      {#if isShow}<button class="ic press" class:on={menu === "episodes"} onclick={(e) => openMenu("episodes", e)} aria-label="Episodes"><Icon name="stack" size={21} /></button>{/if}
      <button class="ic press" class:on={menu === "more"} onclick={(e) => openMenu("more", e)} aria-label="Speed and picture"><Icon name="sliders" size={20} /></button>
      {#if hueReady}<button class="ic press couchmode" class:on={couchMode} onclick={toggleCouch} aria-label="Couch mode" title={couchMode ? "couch mode: on" : "couch mode: off"}><Icon name="couch" size={21} /></button>{/if}
      {#if app.native}<button class="ic press" onclick={pip} aria-label="Picture in picture"><Icon name="pip" size={20} /></button>{/if}
      <button class="ic press" onclick={fullscreen} aria-label="Fullscreen"><Icon name={app.fullscreen ? "exitFullscreen" : "fullscreen"} size={20} /></button>
    </div>

    {#if menu}
      <div class="pop glass strong" class:wide={menu === "tracks" || menu === "episodes"} style:--x="{menuX}px">
        {#if menu === "volume"}
          <div class="vol-pop">
            <input type="range" min="0" max={pbs.volumeBoost ? 200 : 100} value={st.volume} oninput={(e) => setVolume(+(e.target as HTMLInputElement).value)} aria-label="Volume" style:--p="{(st.volume / (pbs.volumeBoost ? 200 : 100)) * 100}%" />
            <button class="ic press" onclick={mute} aria-label={st.muted ? "Unmute" : "Mute"}><Icon name={st.muted ? "mute" : "volume"} size={18} /></button>
          </div>
        {:else if menu === "tracks"}
          <div class="tracks">
            <div class="col">
              <h4 class="headline">Audio</h4>
              {#each audio as t (t.id)}
                <button class="opt" class:sel={t.selected} onclick={() => cmd("aid", { value: t.id })}>{trackName(t)}</button>
              {/each}
            </div>
            <div class="col alt">
              <h4 class="headline">Subtitles</h4>
              <button class="opt" class:sel={!subsOn} onclick={() => cmd("sid", { value: "no" })}>Off</button>
              {#each subs as t (t.id)}
                <button class="opt" class:sel={t.selected} onclick={() => cmd("sid", { value: t.id })}>{trackName(t)}{#if t.external}<span class="faint"> · addon</span>{/if}</button>
              {/each}
              {#if subsOn}
                <div class="delay">
                  <span class="foot dim">timing {st.subDelay.toFixed(1)} s</span>
                  <button class="btn tinted small press" onclick={() => cmd("sub-delay", { value: st.subDelay - 0.1 })}>−0.1</button>
                  <button class="btn tinted small press" onclick={() => cmd("sub-delay", { value: st.subDelay + 0.1 })}>+0.1</button>
                </div>
              {/if}
            </div>
          </div>
        {:else if menu === "episodes"}
          <EpisodesPopover {seasons} bind:season videos={seasonVideos} currentId={pb.videoId} {watched} progress={epProgress} onplay={pickEpisode} />
        {:else if menu === "more"}
          <div class="more-pop">
            <h4 class="foot faint">Speed</h4>
            <div class="seg-row">
              {#each [0.5, 0.75, 1, 1.25, 1.5, 2] as s}
                <button class="chip press" class:sel={st.speed === s} onclick={() => (native ? cmd("speed", { value: s }) : video && (video.playbackRate = s))}>{s}×</button>
              {/each}
            </div>
            <h4 class="foot faint">Picture</h4>
            <div class="seg-row">
              {#each [["fit", "Fit"], ["fill", "Fill screen"], ["stretch", "Stretch"]] as [v, l]}
                <button class="chip press" onclick={() => cmd("aspect", { value: v })}>{l}</button>
              {/each}
            </div>
            {#if native}
              <h4 class="foot faint">3D</h4>
              <div class="seg-row">
                {#each [["auto", pb.stream?._couch?.threeD ? "Auto (3D)" : "Auto"], ["off", "Off"], ["sbs", "SBS"], ["ou", "OU"]] as [v, l]}
                  <button class="chip press" class:sel={mode3d === v} onclick={() => (mode3d = v as typeof mode3d)}>{l}</button>
                {/each}
              </div>
            {/if}
            <p class="foot faint info">{st.video.width}×{st.video.height} · {st.video.codec} · {st.video.hwdec && st.video.hwdec !== "no" ? `hardware (${st.video.hwdec})` : "software"} · {st.video.fps?.toFixed(2)} fps</p>
          </div>
        {/if}
      </div>
    {/if}
  </footer>
</div>

<style>
  .eye-hint {
    position: fixed;
    z-index: 115;
    left: 50%;
    top: 104px;
    translate: -50% 0;
    width: min(560px, calc(100vw - 40px));
    display: flex;
    gap: 20px;
    align-items: center;
    padding: 18px 20px;
    border-radius: 26px;
    background: rgb(18 18 22 / 0.86);
    box-shadow:
      inset 0 0 0 1px rgb(var(--hi) / 0.14),
      0 30px 60px -20px rgb(var(--lo) / 0.8);
    animation: hint-in 600ms var(--spring) both;
  }
  @keyframes hint-in {
    from {
      opacity: 0;
      transform: translateY(-16px) scale(0.96);
    }
  }
  .hint-txt {
    display: flex;
    flex-direction: column;
    gap: 6px;
    align-items: flex-start;
  }
  .hint-txt .foot {
    color: rgb(var(--hi) / 0.78);
    line-height: 1.4;
  }
  .hint-txt b {
    color: var(--white);
  }
  .hint-txt .btn {
    margin-top: 6px;
  }
  /* a tiny screen: the gaze dot drifts to the corner, bubbles rise in a "(" */
  .demo {
    position: relative;
    flex: none;
    width: 128px;
    height: 84px;
  }
  .screen {
    position: absolute;
    inset: 0;
    border-radius: 10px;
    background: linear-gradient(135deg, #2a2f3a, #14161c);
    box-shadow: inset 0 0 0 1.5px rgb(var(--hi) / 0.2);
  }
  .dot {
    position: absolute;
    width: 10px;
    height: 10px;
    margin: -5px;
    border-radius: 50%;
    background: #ffc22e;
    box-shadow: 0 0 10px 3px rgb(255 194 46 / 0.6);
    animation: look 3.6s ease-in-out infinite;
  }
  @keyframes look {
    0%,
    10% {
      left: 64%;
      top: 40%;
    }
    35%,
    45% {
      left: 9%;
      top: 88%;
    }
    65%,
    100% {
      left: 14%;
      top: 50%;
    }
  }
  .mini {
    position: absolute;
    width: 14px;
    height: 14px;
    margin: -7px;
    border-radius: 50%;
    background: radial-gradient(circle at 32% 28%, rgb(var(--hi) / 0.8), rgb(var(--hi) / 0.15) 50%, rgb(var(--hi) / 0.3));
    box-shadow: inset 0 0 0 1px rgb(var(--hi) / 0.5);
    top: calc(18% + var(--i) * 21%);
    left: calc(12% + (var(--i) - 1.5) * (var(--i) - 1.5) * 3%);
    animation: mini 3.6s infinite;
    animation-delay: calc(var(--i) * 60ms);
  }
  @keyframes mini {
    0%,
    45% {
      opacity: 0;
      transform: translateY(18px) scale(0.3);
    }
    58% {
      opacity: 1;
      transform: translateY(-2px) scale(1.15);
    }
    66%,
    92% {
      opacity: 1;
      transform: none;
    }
    100% {
      opacity: 0;
    }
  }

  /* Video is always dark, whatever the theme: the player's chrome uses its
     own dark material so it stays readable over any picture. */
  .player {
    position: fixed;
    inset: 0;
    z-index: 100;
    background: var(--black);
    color: var(--white);
    color-scheme: dark;
    --label: var(--white);
    --label-2: rgb(var(--hi) / 0.72);
    --label-3: rgb(var(--hi) / 0.45);
    --text: var(--white);
    --text-dim: rgb(var(--hi) / 0.72);
    --text-faint: rgb(var(--hi) / 0.45);
    --fill-1: rgb(var(--hi) / 0.28);
    --fill-2: rgb(var(--hi) / 0.2);
    --fill-3: rgb(var(--hi) / 0.14);
    --fill-4: rgb(var(--hi) / 0.08);
    --separator: rgb(var(--hi) / 0.14);
    /* black leather chrome */
    --glass-tint: rgb(26 22 20 / 0.6);
    --glass-tint-strong: rgb(22 18 17 / 0.9);
    --glass-spec: rgb(var(--hi) / 0.32);
    --glass-rim: rgb(var(--hi) / 0.12);
    --glass-edge: rgb(var(--lo) / 0.3);
    --glass-shadow: 0 12px 36px -12px rgb(var(--lo) / 0.6);
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
    background: var(--black);
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
    background: var(--black);
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
    filter: drop-shadow(0 4px 20px rgb(var(--lo) / 0.5));
  }
  /* the couch wobbling while the stream warms up */
  .couch-load {
    width: 150px;
    height: auto;
    opacity: 0.9;
  }
  .mini-ring {
    width: 38px;
    height: 38px;
    border-radius: 50%;
    border: 3px solid rgb(var(--hi) / 0.18);
    border-top-color: var(--white);
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
    background: rgb(var(--lo) / var(--bg-o));
    backdrop-filter: blur(var(--blur)) saturate(140%);
    -webkit-box-decoration-break: clone;
  }
  .cue[data-bg="box"] .line {
    background: rgb(var(--lo) / var(--bg-o));
  }
  .cue[data-bg="shadow"] {
    text-shadow: 0 2px 6px rgb(var(--lo) / 0.95), 0 0 2px rgb(var(--lo) / 0.95);
  }
  .cue[data-bg="outline"] {
    -webkit-text-stroke: 0.12em rgb(var(--lo) / 0.85);
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
    --c: 244 234 240;
    position: absolute;
    right: var(--gutter);
    bottom: 128px;
    display: flex;
    align-items: center;
    gap: 9px;
    height: 46px;
    padding: 0 20px 0 16px;
    border-radius: 14px;
    font-weight: 650;
    color: rgb(var(--c));
    background: color-mix(in srgb, rgb(var(--c)) 16%, rgb(8 10 20 / 0.82));
    box-shadow: inset 0 0 0 1px rgb(var(--c) / 0.35), 0 10px 30px -12px rgb(var(--c) / 0.6);
    backdrop-filter: blur(14px);
    -webkit-backdrop-filter: blur(14px);
    animation: pop var(--t-slow) var(--spring) both;
  }
  .skip-intro {
    --c: var(--seg-intro);
  }
  .skip-recap {
    --c: var(--seg-recap);
  }
  .skip-credits {
    --c: var(--seg-credits);
  }
  .skip-preview {
    --c: var(--seg-preview);
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
    background: linear-gradient(180deg, rgb(var(--lo) / 0.55), transparent);
  }
  :global(html.winctl) .top {
    padding-top: 36px;
    padding-right: calc(var(--gutter) + 10px);
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
    background: rgb(var(--lo) / 0.42);
    color: var(--white);
    backdrop-filter: blur(12px);
    box-shadow: inset 0 0 0 0.5px rgb(var(--hi) / 0.18);
  }
  .bottom {
    position: absolute;
    left: var(--gutter);
    right: var(--gutter);
    bottom: 20px;
    display: flex;
    flex-direction: column;
    gap: 10px;
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
    height: 4px;
    border-radius: 0;
    background: rgb(var(--hi) / 0.2);
    overflow: hidden;
    transition: height var(--t-fast) var(--spring);
  }
  .scrub:hover .track {
    height: 7px;
  }
  .buf {
    position: absolute;
    inset: 0 auto 0 0;
    background: rgb(var(--hi) / 0.25);
  }
  /* intro blue · recap orange · credits purple · preview cyan */
  .player {
    --seg-intro: 59 130 246;
    --seg-recap: 249 115 22;
    --seg-credits: 168 85 247;
    --seg-preview: 20 184 166;
  }
  .seg {
    position: absolute;
    z-index: 1;
    top: 0;
    bottom: 0;
    background: rgb(var(--c));
  }
  .seg-intro {
    --c: var(--seg-intro);
  }
  .seg-recap {
    --c: var(--seg-recap);
  }
  .seg-credits {
    --c: var(--seg-credits);
  }
  .seg-preview {
    --c: var(--seg-preview);
  }
  .done {
    position: absolute;
    inset: 0 auto 0 0;
    background: var(--text);
    border-radius: inherit;
  }
  .scrub:hover .done,
  .scrub:focus-within .done {
    background: var(--lamp);
    box-shadow: 0 0 12px rgb(var(--lamp-rgb) / 0.6);
  }
  .load-line {
    margin: 4px 0 0;
    font-size: 15px;
    color: var(--text-dim);
  }
  .badges .chip {
    background: rgb(var(--neon-rgb) / 0.1);
    color: var(--neon-soft);
    box-shadow: inset 0 0 0 1px rgb(var(--neon-rgb) / 0.45), 0 0 14px -4px rgb(var(--neon-rgb) / 0.6);
    font-size: 12px;
    letter-spacing: 0.03em;
  }
  .couchmode.on {
    color: var(--lamp);
    filter: drop-shadow(0 0 8px rgb(var(--lamp-rgb) / 0.7));
  }
  .un-play {
    gap: 8px;
  }
  .lamp-ring {
    width: 18px;
    height: 18px;
    transform: rotate(-90deg);
  }
  .lamp-ring circle {
    fill: none;
    stroke: rgb(var(--lo) / 0.25);
    stroke-width: 2.6;
  }
  .lamp-ring .fg {
    stroke: var(--lamp);
    stroke-linecap: round;
    stroke-dasharray: 50.3;
    transition: stroke-dashoffset 1s linear;
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
  /* --------------------------------------------- control bar (image 3) */
  .scrub-row {
    display: flex;
    align-items: center;
    gap: 14px;
  }
  .scrub-row .scrub {
    flex: 1;
  }
  .done {
    background: var(--scene, var(--lamp)) !important;
    transition: background-color 900ms ease, width 120ms linear;
  }
  .scrub:hover .done,
  .scrub:focus-within .done {
    box-shadow: 0 0 14px var(--scene, rgb(var(--lamp-rgb) / 0.6)) !important;
  }
  .remain {
    font-size: 14px;
    font-weight: 600;
    min-width: 54px;
    text-align: right;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 6px 10px;
    border-radius: var(--r-lg);
  }
  .ic {
    position: relative;
    width: 42px;
    height: 42px;
    flex: none;
    border-radius: 12px;
    display: grid;
    place-items: center;
    color: rgb(var(--hi) / 0.85);
  }
  .ic:hover,
  .ic.on {
    color: var(--text);
    background: rgb(var(--hi) / 0.1);
  }
  .seekic span,
  .p-seek span {
    position: absolute;
    font-size: 8.5px;
    font-weight: 800;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -35%);
  }
  .now {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: baseline;
    justify-content: center;
    gap: 10px;
    padding: 0 12px;
    white-space: nowrap;
    overflow: hidden;
  }
  .now-show {
    font-weight: 700;
    font-size: 14px;
  }
  .now-ep {
    font-size: 14px;
    color: rgb(var(--hi) / 0.55);
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* popovers, each pointing at its button */
  .pop {
    position: fixed;
    bottom: 96px;
    left: clamp(12px, calc(var(--x) - 140px), calc(100vw - 292px));
    min-width: 280px;
    padding: 10px;
    border-radius: var(--r-lg);
    animation: menu-in var(--t-med) var(--spring) both;
    transform-origin: bottom center;
    z-index: 5;
  }
  .pop.wide {
    left: clamp(12px, calc(var(--x) - 260px), calc(100vw - 532px));
    min-width: 0;
  }
  @keyframes menu-in {
    from {
      opacity: 0;
      transform: translateY(8px) scale(0.96);
    }
  }
  .vol-pop {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 4px 6px;
  }
  .vol-pop input {
    flex: 1;
    -webkit-appearance: none;
    appearance: none;
    height: 24px;
    background: transparent;
  }
  .vol-pop input::-webkit-slider-runnable-track {
    height: 5px;
    border-radius: 5px;
    background: linear-gradient(90deg, var(--lamp) var(--p), rgb(var(--hi) / 0.22) var(--p));
  }
  .vol-pop input::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 16px;
    height: 16px;
    margin-top: -5.5px;
    border-radius: 50%;
    background: var(--text);
  }
  .tracks {
    display: grid;
    grid-template-columns: 1fr 1fr;
    width: min(520px, calc(100vw - 40px));
    max-height: 50vh;
    border-radius: var(--r-md);
    overflow: hidden;
  }
  .col {
    padding: 10px 6px;
    overflow-y: auto;
    max-height: 50vh;
  }
  .col.alt {
    background: rgb(var(--hi) / 0.04);
  }
  .col h4 {
    margin: 4px 12px 10px;
  }
  .opt {
    position: relative;
    display: block;
    width: 100%;
    text-align: left;
    padding: 8px 12px 8px 26px;
    border-radius: 10px;
    font-size: 14px;
    color: rgb(var(--hi) / 0.7);
  }
  .opt:hover {
    background: rgb(var(--hi) / 0.07);
    color: var(--text);
  }
  .opt.sel {
    color: var(--text);
    font-weight: 650;
  }
  .opt.sel::before {
    content: "";
    position: absolute;
    left: 11px;
    top: 50%;
    width: 6px;
    height: 6px;
    margin-top: -3px;
    border-radius: 50%;
    background: var(--lamp);
    box-shadow: 0 0 8px var(--lamp);
  }
  .delay {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 10px 10px 2px;
  }
  .delay span {
    margin-right: auto;
  }
  .more-pop h4 {
    margin: 6px 4px 8px;
    font-weight: 600;
  }
  .seg-row {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-bottom: 8px;
  }
  .info {
    margin: 8px 4px 2px;
  }
  .season-pick {
    position: relative;
    display: inline-flex;
    align-items: center;
  }
  .season-pick select {
    -webkit-appearance: none;
    appearance: none;
    height: 32px;
    padding: 0 30px 0 14px;
    border-radius: 999px;
    border: 0;
    font-weight: 600;
    font-size: 13px;
    color: var(--text);
    background: rgb(var(--lo) / 0.5);
    box-shadow: inset 0 0 0 1px rgb(var(--hi) / 0.25);
    backdrop-filter: blur(12px);
    cursor: pointer;
  }
  .season-pick :global(svg) {
    position: absolute;
    right: 11px;
    pointer-events: none;
  }

  /* --------------------------------------- paused (image 1) */
  .paused {
    position: absolute;
    inset: 0;
    pointer-events: none;
    animation: fade-in 380ms var(--ease-out) both;
  }
  .p-center,
  .paused :global(.carousel) {
    pointer-events: auto;
  }
  @keyframes fade-in {
    from {
      opacity: 0;
    }
  }
  .p-shade {
    position: absolute;
    inset: 0;
    pointer-events: none;
    background:
      linear-gradient(90deg, rgb(var(--lo) / 0.7), rgb(var(--lo) / 0.25) 45%, rgb(var(--lo) / 0.1) 65%, rgb(var(--lo) / 0.55)),
      linear-gradient(0deg, rgb(var(--lo) / 0.75), transparent 45%);
  }
  .p-center {
    position: absolute;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    display: flex;
    align-items: center;
    gap: 44px;
  }
  .paused:has(.carousel) .p-center {
    left: 42%;
  }
  .p-play {
    width: 92px;
    height: 92px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    padding-left: 5px;
    color: var(--text);
    border: 2.5px solid rgb(var(--hi) / 0.9);
    background: rgb(var(--lo) / 0.25);
    backdrop-filter: blur(8px);
    transition: transform var(--t-med) var(--spring), box-shadow var(--t-med);
  }
  .p-play:hover {
    transform: scale(1.06);
    box-shadow: 0 0 40px -6px rgb(var(--neon-rgb) / 0.6);
  }
  .p-seek {
    position: relative;
    width: 56px;
    height: 56px;
    display: grid;
    place-items: center;
    color: var(--text);
    border-radius: 50%;
  }
  .p-seek span {
    font-size: 11px;
  }
  .p-seek:hover {
    background: rgb(var(--hi) / 0.1);
  }
  .p-info {
    position: absolute;
    left: var(--gutter);
    bottom: 150px;
    max-width: min(560px, 46vw);
    display: flex;
    flex-direction: column;
    gap: 6px;
    pointer-events: none;
  }
  .p-show {
    font-weight: 600;
    font-size: 15px;
    color: rgb(var(--hi) / 0.85);
  }
  .p-facts {
    font-size: 13px;
    color: rgb(var(--hi) / 0.6);
  }
  .p-title {
    margin: 4px 0 2px;
    font-family: var(--font-display);
    font-weight: 800;
    font-size: clamp(32px, 3.6vw, 52px);
    line-height: 1.02;
    letter-spacing: -0.03em;
  }
  .p-desc {
    margin: 0;
    font-size: 15px;
    line-height: 1.45;
    color: rgb(var(--hi) / 0.82);
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  /* --------------------------------------- resting (image 4) */
  .resting {
    position: absolute;
    inset: 0;
    cursor: none;
    background:
      linear-gradient(90deg, rgb(var(--lo) / 0.82), rgb(var(--lo) / 0.4) 45%, transparent 75%),
      linear-gradient(0deg, rgb(var(--lo) / 0.6), transparent 50%);
    animation: fade-in 1.2s var(--ease-out) both;
  }
  .r-copy {
    position: absolute;
    left: calc(var(--gutter) + 8px);
    bottom: 18vh;
    max-width: min(560px, 50vw);
    display: flex;
    flex-direction: column;
    gap: 6px;
    animation: rise-in 1.4s var(--ease-out) 0.2s both;
  }
  @keyframes rise-in {
    from {
      opacity: 0;
      transform: translateY(14px);
    }
  }
  .r-eyebrow {
    font-family: var(--font-bump);
    font-size: 13px;
    color: rgb(var(--hi) / 0.55);
  }
  .r-title {
    margin: 0;
    font-family: var(--font-display);
    font-weight: 800;
    font-size: clamp(36px, 4.2vw, 64px);
    letter-spacing: -0.035em;
    line-height: 1;
  }
  .r-ep {
    font-size: 16px;
    font-weight: 600;
    color: rgb(var(--hi) / 0.85);
  }
  .r-name {
    margin-top: 12px;
    font-size: 18px;
    font-weight: 700;
  }
  .r-desc {
    margin: 0;
    font-size: 15px;
    line-height: 1.45;
    color: rgb(var(--hi) / 0.6);
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  /* spec labels: plain white, sharp, like the back of a Blu-ray box */
  .spec {
    display: inline-flex;
    align-items: baseline;
    gap: 5px;
    height: 24px;
    padding: 0 8px;
    align-items: center;
    border: 1.5px solid rgb(255 255 255 / 0.85);
    border-radius: 3px;
    color: #fff;
    font: 800 12px/1 "Inter", system-ui, sans-serif;
    letter-spacing: 0.04em;
    background: rgb(0 0 0 / 0.25);
    backdrop-filter: blur(8px);
  }
  .spec.dolby b {
    font-weight: 900;
    letter-spacing: 0.08em;
  }
  .spec.dolby i {
    font-style: normal;
    font-weight: 400;
    letter-spacing: 0.14em;
  }
  .track .done,
  .track .buf,
  .track .seg {
    border-radius: 0 !important;
  }
  .bar.gone {
    opacity: 0;
    transform: translateY(10px);
    pointer-events: none;
  }
  .bar {
    transition: opacity var(--t-med) ease, transform var(--t-med) var(--glide);
  }
</style>
