// The current playback session as the UI sees it. Rust owns the player;
// this mirrors its state and holds the UI-only bits (overlay subtitle cue,
// skip segments, up-next).

import { api, listen, type Meta, type PlayerState, type Stream, type Track, type Video } from "../api";
import { app } from "./app.svelte";
import { setAmbient } from "./ambient.svelte";
import { loadLibrary } from "./library.svelte";
import { toast, errorText } from "./toast.svelte";
import { parseCue, type Cue } from "../subs";

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
});

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
    pb.loading = false;
    loadSegments();
  });
  listen<{ reason: string }>("player://end", ({ reason }) => {
    if (/error/i.test(reason)) {
      pb.error = "Playback stopped: the stream failed. Try another source.";
    }
  });
  listen<{ message: string }>("player://error", ({ message }) => {
    if (pb.open) pb.error = message;
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

export async function play(meta: Meta, stream: Stream, videoId: string, opts: { startAt?: number; episodeTitle?: string } = {}) {
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
  } catch (e) {
    pb.loading = false;
    toast(errorText(e), "error");
  }
}

export function cmd(name: string, args?: Record<string, unknown>) {
  if (pb.mode !== "native") return Promise.resolve();
  return api.cmd(name, args).catch((e) => toast(errorText(e), "error"));
}
