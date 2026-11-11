// The current playback session as the UI sees it. Rust owns the player;
// this mirrors its state and holds the UI-only bits (overlay subtitle cue,
// skip segments, up-next).

import { api, listen, type Meta, type PlayerState, type Stream, type Track, type Video } from "../api";
import { askAfterFinish } from "./social.svelte";
import { app } from "./app.svelte";
import { setAmbient } from "./ambient.svelte";
import { loadLibrary, progressOf, watchedIds } from "./library.svelte";
import { toast, errorText } from "./toast.svelte";
import { parseCue, type Cue } from "../subs";
import { isTauri } from "../api";
import { nativePlay, nativeControl, onNative, haptic } from "../native";

export interface Segment {
  start: number;
  end: number | null;
}

const emptyState: PlayerState = {
  pos: 0, duration: 0, paused: true, buffering: true, cacheAhead: 0, bufferingPct: 0, volume: 100, muted: false, speed: 1,
  aid: "", sid: "", ended: false, idle: false, subOverlay: true, subDelay: 0,
  video: { width: 0, height: 0, hdr: false, gamma: "", codec: "", hwdec: "", fps: 0 },
};

export const pb = $state({
  open: false,
  mode: "native" as "native" | "web" | "youtube" | "external",
  url: "" as string,
  ytId: "" as string,
  meta: null as Meta | null,
  stream: null as Stream | null,
  videoId: "",
  episodeTitle: "",
  state: { ...emptyState } as PlayerState,
  tracks: [] as Track[],
  cue: null as Cue | null,
  segments: {} as Record<string, Segment[]>,
  addonSubs: [] as any[],
  loading: false,
  error: "",
  torrent: null as any,
  /** playing in the corner while you browse couch */
  mini: false,
  /** iPhone: AVPlayer has the video (pb.open stays false) */
  ios: false,
  /** a file AVPlayer can't open: offer VLC / Infuse */
  handoff: "" as string,
});

const iosNative = () => isTauri && app.platform === "ios";

let wired = false;

export function initPlayback() {
  if (wired) return;
  wired = true;
  listen<PlayerState>("player://state", (s) => {
    pb.state = s;
    if (s.buffering === false && pb.loading && s.duration > 0) pb.loading = false;
  });
  listen<Track[]>("player://tracks", (t) => (pb.tracks = t));
  listen<{ text: string }>("player://sub", ({ text }) => {
    const sub = app.settings.subtitles;
    pb.cue = parseCue(text, { hideSdh: sub.hideSdh });
  });
  listen<any[]>("player://subtitles", (s) => (pb.addonSubs = s));
  listen("player://loaded", async () => {
    clearTimeout(startTimer);
    pb.loading = false;
    loadSegments();
  });
  listen<{ reason: string }>("player://end", ({ reason }) => {
    if (/error/i.test(reason)) {
      if (pb.state.pos < 5 && tryNext()) return;
      pb.error = "the tv got scared. that stream stopped.";
    }
  });
  listen<{ message: string }>("player://error", ({ message }) => {
    if (!pb.open) return;
    if (pb.state.pos < 5 && tryNext()) return;
    pb.error = message;
  });

  // iPhone: the native player reports back here.
  onNative<{ pos: number; duration: number; paused: boolean }>("progress", (p) => {
    // the iPhone player's state, so watch together can follow along
    pb.state.pos = p.pos;
    pb.state.duration = p.duration;
    pb.state.paused = p.paused;
    if (pb.ios) pb.loading = false;
    api.report(p.pos, p.duration, p.paused);
  });
  onNative<{ pos: number; duration: number }>("closed", async (p) => {
    api.report(p.pos, p.duration, true);
    checkFinished(p.pos, p.duration);
    pb.ios = false;
    await api.stop().catch(() => {});
    loadLibrary().catch(() => {});
  });
  onNative<{ idle?: number }>("ended", (p) => {
    if (app.settings.playback.autoplayNext) autoNext(p?.idle != null ? p.idle * 1000 : undefined);
  });
  // "are u still on the couch?" answered on the iPhone player ("i'm done"
  // closes the player itself)
  onNative<{ stay: boolean }>("still", ({ stay }) => {
    still.ask = false;
    if (stay) keepWatching();
  });
  // the player's own buttons: rating, the Episodes list, Next Episode
  onNative<{ value: number }>("rate", async ({ value }) => {
    const m = pb.meta;
    if (!m) return;
    const n = value === 2 ? 10 : value === 1 ? 8 : value === -1 ? 4 : null;
    simklRating = { id: m.id, value };
    try {
      await api.simklRate(m.type, m.id, n);
    } catch (e) {
      toast(errorText(e), "error");
    }
  });
  onNative<{ id: string }>("episode", ({ id }) => {
    markActive();
    const v = pb.meta ? orderedVideos(pb.meta).find((x) => x.id === id) : null;
    if (v) playVideo(v);
  });
  onNative("next", () => {
    markActive();
    playNext();
  });
  onNative<{ url: string }>("failed", ({ url }) => {
    pb.ios = false;
    if (tryNext()) return;
    pb.handoff = url || pb.url;
  });
}

/** Your Simkl rating for what's playing, as the player's three buttons. */
let simklRating: { id: string; value: number } | null = null;
async function ratingFor(m: Meta | null): Promise<number> {
  if (!m || !app.secrets.simkl_token) return 0;
  if (simklRating?.id === m.id) return simklRating.value;
  let r: number | null = null;
  try {
    r = (await api.simklStatus(m.type, m.id))?.rating ?? null;
  } catch {
    /* not on Simkl */
  }
  const value = !r ? 0 : r >= 10 ? 2 : r >= 6 ? 1 : -1;
  simklRating = { id: m.id, value };
  return value;
}

/** The season of the episode playing, for the player's Episodes list. */
function seasonList(m: Meta | null) {
  if (!m || m.type !== "series") return undefined;
  const all = orderedVideos(m);
  const now = all.find((v) => v.id === pb.videoId);
  if (!now) return undefined;
  const watched = watchedIds(m.id);
  const today = Date.now();
  return JSON.stringify(
    all
      .filter((v) => (v.season ?? 0) === (now.season ?? 0) && (!v.released || new Date(v.released).getTime() <= today))
      .map((v) => ({
        id: v.id,
        episode: v.episode ?? v.number ?? 0,
        title: v.title || v.name || "",
        overview: v.overview ?? v.description ?? "",
        thumbnail: v.thumbnail ?? "",
        progress: watched.has(v.id) ? 1 : progressOf(m.id, v.id),
        current: v.id === now.id,
      })),
  );
}

async function startIOS(url: string, startAt?: number, headers?: Record<string, string>, startPct?: number) {
  pb.url = url;
  pb.ios = true;
  haptic("medium");
  const m = pb.meta;
  const ep = m && m.type === "series" ? orderedVideos(m).find((v) => v.id === pb.videoId) : undefined;
  await nativePlay({
    url,
    title: m?.name ?? "couch",
    subtitle: pb.episodeTitle || (m?.releaseInfo ? String(m.releaseInfo) : ""),
    // the plain https address: the native side loads it itself
    artwork: (m?.background ?? m?.poster ?? "").replace(/\/t\/p\/original\//, "/t/p/w780/"),
    startAt,
    startPct,
    tint: getComputedStyle(document.documentElement).getPropertyValue("--ambient-1").trim(),
    headers,
    engine: "couch",
    episodeLabel: ep ? `S${ep.season ?? 0}:E${ep.episode ?? ep.number ?? 0}` : undefined,
    episodesJson: seasonList(m),
    rating: await ratingFor(m),
    hasNext: !!nextVideo(),
    // mpv takes ISO 639-2 codes, same as the settings store
    subLangs: app.settings.subtitles.enabledByDefault ? app.settings.subtitles.languages : [],
    audioLangs: app.settings.playback.audioLanguages,
  });
}

async function loadSegments() {
  pb.segments = {};
  if (!app.settings.integrations.tidbEnabled || !pb.videoId) return;
  try {
    const dur = pb.state.duration ? Math.round(pb.state.duration * 1000) : undefined;
    const s = await api.skip(pb.videoId, dur);
    if (s) pb.segments = s;
  } catch {
    /* no segments is the normal case */
  }
}

export function previewOf(m: Meta): Partial<Meta> {
  return { id: m.id, type: m.type, name: m.name, poster: m.poster, background: m.background, logo: m.logo, posterShape: m.posterShape };
}

// ------------------------------------------------------------ fallback
// When a stream won't start, the next one in line plays by itself.
let queue: Stream[] = [];
let lastPlay: { meta: Meta; videoId: string; opts: { startAt?: number; episodeTitle?: string } } | null = null;
let startTimer: ReturnType<typeof setTimeout> | undefined;

/** Try the next stream in line. Returns false when there's none left. */
export function tryNext(why = ""): boolean {
  const next = queue.shift();
  if (!next || !lastPlay) return false;
  toast(why ? `${why} trying another stream…` : "that one didn't start. trying another…", "info");
  const { meta, videoId, opts } = lastPlay;
  play(meta, next, videoId, { ...opts, queue: [...queue] });
  return true;
}

export async function play(meta: Meta, stream: Stream, videoId: string, opts: { startAt?: number; episodeTitle?: string; queue?: Stream[] } = {}) {
  queue = opts.queue ? [...opts.queue] : [];
  lastPlay = { meta, videoId, opts: { startAt: opts.startAt, episodeTitle: opts.episodeTitle } };
  clearTimeout(startTimer);
  // nothing playing after 25 s counts as a failure
  startTimer = setTimeout(() => {
    if (pb.open && pb.loading && pb.state.pos < 1 && queue.length) tryNext("this one is taking forever.");
  }, 25_000);
  pb.open = true;
  pb.mini = false;
  pb.loading = true;
  pb.error = "";
  pb.meta = meta;
  pb.stream = stream;
  pb.videoId = videoId;
  pb.episodeTitle = opts.episodeTitle ?? "";
  pb.cue = null;
  pb.tracks = [];
  pb.segments = {};
  pb.state = { ...emptyState };
  setAmbient(meta.background ?? meta.poster);
  try {
    const r = await api.open({ stream, meta: previewOf(meta), videoId, startAt: opts.startAt, episodeTitle: opts.episodeTitle });
    if (r.mode === "web" && iosNative() && r.url) {
      pb.open = false;
      pb.loading = false;
      pb.mode = "web";
      await startIOS(r.url, r.startAt ?? undefined, r.headers as any, r.startPct ?? undefined);
      return;
    }
    pb.mode = r.mode;
    pb.url = r.url ?? "";
    pb.ytId = r.ytId ?? "";
    if (r.mode === "external") {
      await api.openExternal(r.url!);
      close();
      return;
    }
    if (r.mode === "native") {
      app.playing = true;
      document.documentElement.classList.add("playing");
    }
  } catch (e) {
    if (tryNext()) return;
    pb.loading = false;
    pb.error = errorText(e);
  }
}

/** Got to the end of a film, or of a show's last episode: ask to rate it. */
function checkFinished(pos = pb.state.pos, dur = pb.state.duration) {
  const m = pb.meta;
  if (!m || !dur || pos / dur < 0.9) return;
  if (m.type === "series") {
    const eps = orderedVideos(m).filter((v) => (v.season ?? 0) > 0);
    if (!eps.length || eps[eps.length - 1].id !== pb.videoId) return;
  }
  askAfterFinish(m);
}

/** shrink the player into the corner and keep browsing */
export function minimize() {
  if (pb.open && !pb.error) pb.mini = true;
}
export function restore() {
  pb.mini = false;
}

// ---------------------------------------------------------- sleep timer
/** "episode": stop when this one ends. "time": stop at `at` (ms). */
export const sleep = $state({ mode: "" as "" | "episode" | "time", at: 0 });
export function setSleep(mode: "" | "episode" | number) {
  if (mode === "") sleep.mode = "";
  else if (mode === "episode") sleep.mode = "episode";
  else {
    sleep.mode = "time";
    sleep.at = Date.now() + mode * 60_000;
  }
}
/** time's up: pause where we are (the spot is saved) */
export function sleepNow() {
  sleep.mode = "";
  cmd("pause", { value: true });
  toast("sleep timer: paused. goodnight", "info", undefined, 8000);
}

export async function close() {
  pb.mini = false;
  sleep.mode = "";
  still.ask = false;
  autoStreak = 0;
  checkFinished();
  pb.open = false;
  app.playing = false;
  document.documentElement.classList.remove("playing");
  if (app.pip) {
    app.pip = false;
    api.pip(false).catch(() => {});
  }
  try {
    await api.stop();
  } catch {
    /* already stopped */
  }
  loadLibrary().catch(() => {});
}

/** Episodes in airing order, specials last. */
export function orderedVideos(meta: Meta | null): Video[] {
  const v = (meta?.videos ?? []).slice();
  v.sort((a, b) => {
    const sa = a.season || 9999, sb = b.season || 9999;
    if (sa !== sb) return sa - sb;
    return (a.episode ?? a.number ?? 0) - (b.episode ?? b.number ?? 0);
  });
  return v;
}

export function nextVideo(): Video | null {
  if (!pb.meta || pb.meta.type !== "series") return null;
  const list = orderedVideos(pb.meta);
  const i = list.findIndex((v) => v.id === pb.videoId);
  if (i < 0 || i + 1 >= list.length) return null;
  const n = list[i + 1];
  // the season finale isn't followed by specials / behind-the-scenes extras
  if ((n.season ?? 0) === 0 && (list[i].season ?? 0) !== 0) return null;
  // Don't offer episodes that haven't aired yet.
  if (n.released && new Date(n.released).getTime() > Date.now()) return null;
  return n;
}

// ------------------------------------------- "are u still on the couch?"
// Netflix-style: after 3 episodes in a row that started by themselves with
// nobody touching anything (or 90 min of no input), ask before the next one
// instead of playing to an empty room.
const STILL_STREAK = 2; // two auto-started already → asking before the third
const STILL_IDLE_MS = 90 * 60_000;
let autoStreak = 0;
let lastAuto = 0;
let lastActive = Date.now();
export const still = $state({ ask: false });

/** someone's there: any tap, click, key or scroll in the player */
export function markActive() {
  lastActive = Date.now();
}

/** the episode ended (or the up-next countdown ran out): next one, or ask */
export function autoNext(idleMs?: number) {
  if (still.ask || !nextVideo()) return;
  if (sleep.mode === "episode") return sleepNow();
  // Apple's own player (the AVPlayer path) can't ask or report touches
  if (pb.ios && idleMs == null) return void playNext();
  const now = Date.now();
  const idle = idleMs ?? now - lastActive;
  if (now - idle > lastAuto) autoStreak = 0; // touched since the last auto-advance
  if (autoStreak >= STILL_STREAK || idle >= STILL_IDLE_MS) {
    still.ask = true;
    if (pb.ios) nativeControl("still").catch(() => {});
    else cmd("pause", { value: true });
    return;
  }
  autoStreak++;
  lastAuto = now;
  playNext();
}

/** "still here" */
export function keepWatching() {
  still.ask = false;
  autoStreak = 0;
  lastActive = lastAuto = Date.now();
  playNext();
}

export async function playNext() {
  const n = nextVideo();
  if (n) await playVideo(n);
}

/** Jump to any episode of the show that's playing (same addon & quality). */
export async function playVideo(n: Video) {
  if (!pb.meta || !pb.stream || n.id === pb.videoId) return;
  pb.loading = true;
  pb.cue = null;
  const title = n.title || n.name || `Episode ${n.episode ?? n.number}`;
  try {
    const r = await api.next(previewOf(pb.meta), n.id, pb.stream, title);
    pb.stream = r.stream;
    pb.videoId = n.id;
    pb.episodeTitle = title;
    pb.segments = {};
    pb.mode = r.open.mode;
    pb.url = r.open.url ?? "";
    if (r.open.mode === "web" && iosNative() && r.open.url) {
      pb.loading = false;
      await startIOS(r.open.url, undefined, r.open.headers);
    }
  } catch (e) {
    pb.loading = false;
    toast(errorText(e), "error");
  }
}

export function cmd(name: string, args?: Record<string, unknown>) {
  // iPhone: couch's own player, driven through the native bridge
  if (pb.ios) {
    if (name === "pause") return nativeControl("pause", typeof args?.value === "boolean" ? { flag: args.value } : {});
    if (name === "seek") return nativeControl("seek", { value: Number(args?.to ?? 0) });
    return Promise.resolve();
  }
  if (pb.mode !== "native") return Promise.resolve();
  return api.cmd(name, args).catch((e) => toast(errorText(e), "error"));
}
