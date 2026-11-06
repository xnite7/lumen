// The current playback session as the UI sees it. Rust owns the player;
// this mirrors its state and holds the UI-only bits (overlay subtitle cue,
// skip segments, up-next).

import { api, listen, type Meta, type PlayerState, type Stream, type Track, type Video } from "../api";
import { app } from "./app.svelte";
import { setAmbient } from "./ambient.svelte";
import { loadLibrary } from "./library.svelte";
import { toast, errorText } from "./toast.svelte";
import { parseCue, type Cue } from "../subs";
import { isTauri } from "../api";
import { nativePlay, onNative, haptic } from "../native";

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
  onNative<{ pos: number; duration: number; paused: boolean }>("progress", (p) => api.report(p.pos, p.duration, p.paused));
  onNative<{ pos: number; duration: number }>("closed", async (p) => {
    api.report(p.pos, p.duration, true);
    pb.ios = false;
    await api.stop().catch(() => {});
    loadLibrary().catch(() => {});
  });
  onNative("ended", () => {
    if (app.settings.playback.autoplayNext && nextVideo()) playNext();
  });
  onNative<{ url: string }>("failed", ({ url }) => {
    pb.ios = false;
    if (tryNext()) return;
    pb.handoff = url || pb.url;
  });
}

async function startIOS(url: string, startAt?: number, headers?: Record<string, string>, startPct?: number) {
  pb.url = url;
  pb.ios = true;
  haptic("medium");
  const m = pb.meta;
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
    engine: app.settings.playback.iosPlayer,
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

export async function close() {
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
  // Don't offer episodes that haven't aired yet.
  if (n.released && new Date(n.released).getTime() > Date.now()) return null;
  return n;
}

export async function playNext() {
  const n = nextVideo();
  if (!n || !pb.meta || !pb.stream) return;
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
  if (pb.mode !== "native") return Promise.resolve();
  return api.cmd(name, args).catch((e) => toast(errorText(e), "error"));
}
