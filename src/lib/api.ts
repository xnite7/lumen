// Typed bridge to the Rust side. In a plain browser (vite dev without
// Tauri, or the screenshot harness) calls fall through to a small mock so
// every screen still renders.

import { invoke as tauriInvoke, convertFileSrc } from "@tauri-apps/api/core";
import { listen as tauriListen, type UnlistenFn } from "@tauri-apps/api/event";
import { mock } from "./mock";

export const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export async function invoke<T = any>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri) return mock(cmd, args) as Promise<T>;
  return tauriInvoke<T>(cmd, args);
}

type Handler<T> = (payload: T) => void;
const localBus = new Map<string, Set<Handler<any>>>();

export async function listen<T = any>(event: string, cb: Handler<T>): Promise<UnlistenFn> {
  if (!isTauri) {
    if (!localBus.has(event)) localBus.set(event, new Set());
    localBus.get(event)!.add(cb);
    return () => localBus.get(event)?.delete(cb);
  }
  return tauriListen<T>(event, (e) => cb(e.payload));
}

/** Used by the mock to fake backend events. */
export function emitLocal(event: string, payload: unknown) {
  localBus.get(event)?.forEach((cb) => cb(payload));
}

// ------------------------------------------------------------------ images

function b64url(s: string) {
  const bytes = new TextEncoder().encode(s);
  let bin = "";
  for (const b of bytes) bin += String.fromCharCode(b);
  return btoa(bin).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

const metahubSize: Record<string, string> = { small: "small", medium: "medium", large: "large" };

/**
 * Every image goes through the Rust `img://` handler: disk cache, DoH, and
 * the blocked-host proxy fallback, with zero DOM error handling.
 * `size` picks a smaller TMDB/metahub variant when the original is huge.
 */
export function img(url?: string | null, size?: "small" | "medium" | "large" | "w342" | "w500" | "w780" | "w1280" | "original"): string | undefined {
  if (!url || typeof url !== "string") return undefined;
  let u = url;
  if (size) {
    if (u.includes("image.tmdb.org/t/p/") && size.startsWith("w")) u = u.replace(/\/t\/p\/[^/]+\//, `/t/p/${size}/`);
    else if (u.includes("images.metahub.space") && metahubSize[size]) u = u.replace(/\/(small|medium|large)\//, `/${size}/`);
  }
  if (u.startsWith("data:") || u.startsWith("blob:")) return u;
  if (!isTauri) return u;
  return convertFileSrc(b64url(u), "img");
}

// ------------------------------------------------------------------- types

export interface ExtraProp {
  name: string;
  isRequired?: boolean;
  options?: string[];
}

export interface CatalogRef {
  key: string;
  addonId: string;
  addonName: string;
  transportUrl: string;
  type: string;
  id: string;
  name: string;
  extra: ExtraProp[];
  adult: boolean;
  builtin: boolean;
}

export interface Meta {
  id: string;
  type: string;
  name: string;
  poster?: string;
  posterShape?: "poster" | "landscape" | "square";
  background?: string;
  logo?: string;
  description?: string;
  releaseInfo?: string;
  released?: string;
  imdbRating?: string;
  genres?: string[];
  runtime?: string;
  cast?: string[];
  director?: string[];
  links?: { name: string; category: string; url: string }[];
  trailers?: { source: string; type: string }[];
  trailerStreams?: { ytId: string; title?: string }[];
  videos?: Video[];
  adult?: boolean;
  awards?: string;
  tagline?: string;
  _addon?: string;
  tmdbId?: number;
  tmdbKind?: string;
  character?: string;
  [k: string]: any;
}

export interface Video {
  id: string;
  title?: string;
  name?: string;
  season?: number;
  episode?: number;
  number?: number;
  released?: string;
  thumbnail?: string;
  overview?: string;
  description?: string;
}

export interface ParsedStream {
  res: string;
  resRank: number;
  hdr: string[];
  codec?: string;
  audio: string[];
  source?: string;
  sizeBytes?: number;
  seeders?: number;
  cached?: boolean;
  debrid?: string;
  kind: "debrid" | "torrent" | "http" | "youtube" | "external";
  cam: boolean;
  threeD: boolean;
  languages: string[];
  group?: string;
  label: string;
  score: number;
}

export interface Stream {
  url?: string;
  infoHash?: string;
  fileIdx?: number;
  ytId?: string;
  externalUrl?: string;
  name?: string;
  title?: string;
  description?: string;
  behaviorHints?: Record<string, any>;
  _lumen: ParsedStream;
  _addonId: string;
  _addonName: string;
  [k: string]: any;
}

export interface AddonDescriptor {
  transportUrl: string;
  transportName: string;
  manifest: {
    id: string;
    name: string;
    version: string;
    description?: string;
    logo?: string;
    background?: string;
    types: string[];
    catalogs: any[];
    resources: any[];
    behaviorHints?: Record<string, any>;
  };
  flags: { official: boolean; protected: boolean };
}

export interface LibraryItem {
  _id: string;
  name: string;
  type: string;
  poster?: string;
  background?: string;
  logo?: string;
  removed: boolean;
  temp: boolean;
  _mtime: string;
  state: {
    lastWatched?: string;
    timeOffset: number;
    duration: number;
    video_id?: string;
    flaggedWatched: number;
    timesWatched: number;
    season?: number;
    episode?: number;
    watchedIds?: string[];
  };
}

export interface PlayerState {
  pos: number;
  duration: number;
  paused: boolean;
  buffering: boolean;
  cacheAhead: number;
  bufferingPct: number;
  volume: number;
  muted: boolean;
  speed: number;
  aid: string;
  sid: string;
  ended: boolean;
  idle: boolean;
  subOverlay: boolean;
  subDelay: number;
  video: { width: number; height: number; hdr: boolean; gamma: string; codec: string; hwdec: string; fps: number };
}

export interface Track {
  id: number;
  type: "video" | "audio" | "sub";
  lang?: string;
  title?: string;
  codec?: string;
  selected: boolean;
  external: boolean;
  default: boolean;
  forced: boolean;
  channels?: number;
}

export const api = {
  appInfo: () => invoke<{ version: string; native: boolean; platform: string; dataDir: string }>("app_info"),
  catalogs: () => invoke<CatalogRef[]>("catalogs_list"),
  catalog: (catalog: CatalogRef, extra: [string, string][] = []) => invoke<Meta[]>("catalog_fetch", { catalog, extra }),
  meta: (kind: string, id: string, hint?: string) => invoke<Meta>("meta_get", { kind, id, hint }),
  enrich: (kind: string, id: string) => invoke<any>("meta_enrich", { kind, id }),
  resolve: (kind: string, id: string) => invoke<{ type: string; id: string }>("tmdb_resolve", { kind, id }),
  trailer: (kind: string, id: string) => invoke<string | null>("trailer_get", { kind, id }),
  person: (name: string, tmdbId?: number, adult = false) => invoke<any>("person_get", { name, tmdbId, adult }),
  hero: () => invoke<Meta[]>("hero_items"),
  art: (imdb: string) => invoke<{ backdrop?: string; poster?: string }>("art_get", { imdb }),
  palette: (url: string) => invoke<{ colors: [number, number, number][]; average: [number, number, number]; dark: boolean }>("palette_get", { url }),
  streams: (kind: string, id: string, reqId: string) => invoke<void>("streams_fetch", { kind, id, reqId }),
  search: (query: string, reqId: string) => invoke<void>("search_start", { query, reqId }),
  suggest: (query: string) => invoke<any[]>("search_suggest", { query }),
  recents: () => invoke<string[]>("recents_get"),
  recentsClear: () => invoke<void>("recents_clear"),
  skip: (videoId: string, durationMs?: number) => invoke<any>("skip_segments", { videoId, durationMs }),
  open: (req: { stream: Stream; meta: Partial<Meta>; videoId: string; startAt?: number; episodeTitle?: string }) =>
    invoke<{ mode: "native" | "web" | "youtube" | "external"; url?: string; ytId?: string; startAt?: number | null; startPct?: number | null; headers?: Record<string, string> | null }>("player_open", { req }),
  cmd: (cmd: string, args?: Record<string, unknown>) => invoke<any>("player_cmd", { cmd, args }),
  stop: () => invoke<void>("player_stop"),
  playerState: () => invoke<PlayerState>("player_state"),
  tracks: () => invoke<Track[]>("player_tracks"),
  report: (pos: number, duration: number, paused: boolean) => invoke<void>("player_report", { pos, duration, paused }),
  next: (meta: Partial<Meta>, videoId: string, previous: Stream, episodeTitle?: string) =>
    invoke<{ open: any; stream: Stream }>("player_next", { meta, videoId, previous, episodeTitle }),
  torrentStats: () => invoke<any>("torrent_stats"),
  library: () => invoke<LibraryItem[]>("library_list"),
  libraryItem: (id: string) => invoke<LibraryItem | null>("library_item", { id }),
  libraryAdd: (meta: Partial<Meta>) => invoke<LibraryItem>("library_add", { meta }),
  libraryRemove: (id: string, kind: string) => invoke<void>("library_remove", { id, kind }),
  setWatched: (meta: Partial<Meta>, videoId: string | null, watched: boolean) => invoke<LibraryItem>("library_set_watched", { meta, videoId, watched }),
  continueWatching: () => invoke<LibraryItem[]>("continue_watching"),
  dismiss: (id: string) => invoke<void>("cw_dismiss", { id }),
  addons: () => invoke<AddonDescriptor[]>("addons_list"),
  addonPreview: (url: string) => invoke<AddonDescriptor>("addon_preview", { url }),
  install: (url: string) => invoke<AddonDescriptor>("addons_install", { url }),
  uninstall: (transportUrl: string) => invoke<void>("addons_uninstall", { transportUrl }),
  reorder: (order: string[]) => invoke<void>("addons_reorder", { order }),
  refreshAddons: () => invoke<void>("addons_refresh"),
  health: () => invoke<{ transportUrl: string; status: string; ms: number; detail?: string }[]>("addons_health"),
  exportAddons: () => invoke<any>("addons_export"),
  importAddons: (urls: string[]) => invoke<number>("addons_import", { urls }),
  settings: () => invoke<any>("settings_get"),
  saveSettings: (settings: any) => invoke<void>("settings_set", { settings }),
  setSecret: (name: string, value: string | null) => invoke<void>("secret_set", { name, value }),
  secrets: () => invoke<Record<string, any>>("secrets_status"),
  openExternal: (url: string) => invoke<void>("open_external", { url }),
  stremioLogin: (email: string, password: string) => invoke<any>("stremio_login", { email, password }),
  stremioLinkStart: () => invoke<{ code: string; link: string; qrcode: string }>("stremio_link_start"),
  stremioLinkPoll: (code: string) => invoke<boolean>("stremio_link_poll", { code }),
  stremioLogout: () => invoke<void>("stremio_logout"),
  stremioSync: (pullAddons: boolean) => invoke<{ pulled: number; pushed: number }>("stremio_sync", { pullAddons }),
  simklPinStart: () => invoke<any>("simkl_pin_start"),
  simklPinPoll: (code: string) => invoke<boolean>("simkl_pin_poll", { code }),
  simklLogout: () => invoke<void>("simkl_logout"),
  simklSync: () => invoke<boolean>("simkl_sync"),
  simklOAuthStart: () => invoke<void>("simkl_oauth_start"),
  simklOAuthFinish: (url: string) => invoke<void>("simkl_oauth_finish", { url }),
  simklDeviceStart: () => invoke<{ user_code: string; verification_url: string; interval: number; expires_in: number }>("simkl_device_start"),
  simklDevicePoll: () => invoke<boolean>("simkl_device_poll"),
  simklRate: (kind: string, id: string, rating: number | null) => invoke<void>("simkl_rate", { kind, id, rating }),
  simklStatus: (kind: string, id: string) => invoke<any>("simkl_item_status", { kind, id }),
  vibe: (query: string) => invoke<{ read: string; picks: any[] }>("ai_vibe", { query }),
  vibePrompts: () => invoke<{ prompts: { label: string; q: string }[] }>("ai_prompts"),
  hueDiscover: () => invoke<any[]>("hue_discover"),
  huePair: (ip: string) => invoke<any>("hue_pair", { ip }),
  hueOverview: () => invoke<any>("hue_overview"),
  hueIdentify: () => invoke<void>("hue_identify"),
  hueTest: (on: boolean) => invoke<void>("hue_test", { on }),
  hueMood: (url: string) => invoke<void>("hue_mood", { url }),
  tobiiStatus: () => invoke<{ status: string; dll?: string }>("tobii_status"),
  tobiiStart: () => invoke<void>("tobii_start"),
  tobiiStop: () => invoke<void>("tobii_stop"),
  window: (action: string) => invoke<void>("window_action", { action }),
  pip: (on: boolean) => invoke<void>("window_pip", { on }),
  themesUser: () => invoke<any[]>("themes_user"),
  themeSave: (theme: any) => invoke<void>("theme_save", { theme }),
  themesOpenDir: () => invoke<void>("themes_open_dir"),
  cacheInfo: () => invoke<{ images: number }>("cache_info"),
  cacheClear: () => invoke<void>("cache_clear"),
  importEnhanced: () => invoke<Record<string, boolean>>("import_stremio_enhanced"),
};

export function uid() {
  return Math.random().toString(36).slice(2, 10) + Date.now().toString(36);
}
