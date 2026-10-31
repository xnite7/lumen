// Browser-only stand-in for the Rust backend, so the UI can be developed and
// screenshotted without Tauri. Artwork is generated SVG.

import { DEFAULT_SETTINGS } from "./defaults";

const titles = [
  ["The Long Summer", "movie", ["#FFC22E", "#E4736A"]],
  ["Glass Harbor", "series", ["#77B9E4", "#1d2b4a"]],
  ["Sunflower Static", "movie", ["#FFE9A8", "#7DBF3F"]],
  ["Night Swim", "series", ["#2a3d66", "#77B9E4"]],
  ["Boards of Light", "movie", ["#E4736A", "#3a1f2b"]],
  ["Field Recording", "movie", ["#7DBF3F", "#20331b"]],
  ["Paper Moons", "series", ["#c9b8ff", "#2b2348"]],
  ["Afterglow", "movie", ["#FF9A62", "#4a1d2e"]],
  ["Low Tide", "series", ["#5fc6c0", "#173a3f"]],
  ["The Quiet Year", "movie", ["#d8d2c4", "#3b3a36"]],
  ["Radio Silence", "series", ["#ff6b6b", "#271b2e"]],
  ["Prelinger Road", "movie", ["#e0b45a", "#2b2414"]],
] as const;

function art(name: string, c: readonly string[], w: number, h: number) {
  const svg = `<svg xmlns='http://www.w3.org/2000/svg' width='${w}' height='${h}' viewBox='0 0 ${w} ${h}'>
  <defs><linearGradient id='g' x1='0' y1='0' x2='1' y2='1'><stop offset='0' stop-color='${c[0]}'/><stop offset='1' stop-color='${c[1]}'/></linearGradient>
  <radialGradient id='s' cx='.7' cy='.3' r='.6'><stop offset='0' stop-color='#fff' stop-opacity='.55'/><stop offset='1' stop-color='#fff' stop-opacity='0'/></radialGradient></defs>
  <rect width='100%' height='100%' fill='url(#g)'/><circle cx='${w * 0.7}' cy='${h * 0.32}' r='${Math.min(w, h) * 0.22}' fill='url(#s)'/>
  <text x='${w * 0.08}' y='${h * 0.88}' font-family='Segoe UI, system-ui' font-weight='700' font-size='${Math.round(w / 9)}' fill='rgba(255,255,255,.92)'>${name}</text></svg>`;
  return "data:image/svg+xml;utf8," + encodeURIComponent(svg);
}

function logo(name: string) {
  const svg = `<svg xmlns='http://www.w3.org/2000/svg' width='600' height='160'><text x='0' y='120' font-family='Georgia, serif' font-style='italic' font-weight='700' font-size='104' fill='white'>${name}</text></svg>`;
  return "data:image/svg+xml;utf8," + encodeURIComponent(svg);
}

const metas = titles.map(([name, type, c], i) => ({
  id: `tt00${1000 + i}`,
  type,
  name,
  poster: art(name, c, 342, 513),
  background: art("", c, 1920, 820),
  logo: logo(name),
  description:
    "A warm, overexposed summer where everything is a little too bright. Two kids find an old television in a sunflower field that only plays tomorrow's weather.",
  releaseInfo: String(2018 + (i % 8)),
  imdbRating: (6.4 + (i % 5) * 0.6).toFixed(1),
  genres: ["Drama", "Mystery", "Sci-Fi"].slice(0, 2 + (i % 2)),
  runtime: type === "movie" ? `${96 + i * 4} min` : "48 min",
  videos:
    type === "series"
      ? Array.from({ length: 16 }, (_, k) => ({
          id: `tt00${1000 + i}:${k < 8 ? 1 : 2}:${(k % 8) + 1}`,
          title: `Episode ${(k % 8) + 1}`,
          season: k < 8 ? 1 : 2,
          episode: (k % 8) + 1,
          released: "2024-06-01T00:00:00.000Z",
          thumbnail: art("", c, 480, 270),
          overview: "Somebody leaves the porch light on all night, and the moths start spelling things.",
        }))
      : undefined,
}));

const catalogs = [
  { key: "a/movie/top", addonId: "a", addonName: "Cinemeta", transportUrl: "x", type: "movie", id: "top", name: "Popular", extra: [], adult: false, builtin: false },
  { key: "b/series/top", addonId: "b", addonName: "Cinemeta", transportUrl: "x", type: "series", id: "top", name: "Popular", extra: [], adult: false, builtin: false },
  { key: "c/movie/motn-top", addonId: "c", addonName: "Lumen", transportUrl: "x", type: "movie", id: "motn-top", name: "Top 10 Today", extra: [], adult: false, builtin: true },
  { key: "d/movie/simkl-recs", addonId: "d", addonName: "Lumen", transportUrl: "x", type: "movie", id: "simkl-recs", name: "Because You Watched", extra: [], adult: false, builtin: true },
];

const streams = [
  { name: "[RD+] Torrentio\n4K DV", title: "The.Long.Summer.2024.2160p.BluRay.REMUX.DV.HDR10.HEVC.TrueHD.Atmos.7.1-FGT\n👤 512 💾 61.2 GB", _lumen: { res: "4K", resRank: 4, hdr: ["DV", "HDR10"], codec: "HEVC", audio: ["Atmos", "TrueHD", "7.1"], source: "REMUX", sizeBytes: 61.2e9, seeders: 512, cached: true, debrid: "RD", kind: "debrid", cam: false, threeD: false, languages: [], group: "FGT", label: "The.Long.Summer.2024.2160p.BluRay.REMUX.DV.HDR10", score: 99000 } },
  { name: "[RD+] Torrentio\n1080p", title: "The.Long.Summer.2024.1080p.WEB-DL.DDP5.1.H.264-NTb\n👤 1203 💾 6.1 GB", _lumen: { res: "1080p", resRank: 3, hdr: [], codec: "AVC", audio: ["DD+", "5.1"], source: "WEB-DL", sizeBytes: 6.1e9, seeders: 1203, cached: true, debrid: "RD", kind: "debrid", cam: false, threeD: false, languages: [], group: "NTb", label: "The.Long.Summer.2024.1080p.WEB-DL.DDP5.1", score: 88000 } },
  { name: "Torrentio\n720p", title: "The.Long.Summer.2024.720p.WEBRip.x265\n👤 88 💾 1.4 GB", _lumen: { res: "720p", resRank: 2, hdr: [], codec: "HEVC", audio: [], source: "WEBRip", sizeBytes: 1.4e9, seeders: 88, kind: "torrent", cam: false, threeD: false, languages: [], label: "The.Long.Summer.2024.720p.WEBRip.x265", score: 21000 } },
];

let emit: ((e: string, p: unknown) => void) | null = null;
let polls = 0;
const q = typeof location !== "undefined" ? new URLSearchParams(location.search) : new URLSearchParams();

export async function mock(cmd: string, args: any): Promise<any> {
  if (!emit) emit = (await import("./api")).emitLocal;
  await new Promise((r) => setTimeout(r, 60 + Math.random() * 160));
  switch (cmd) {
    case "app_info":
      return { version: "0.2.0", native: false, platform: q.get("platform") ?? "browser", dataDir: "" };
    case "settings_get": {
      const st = structuredClone(DEFAULT_SETTINGS);
      st.general.onboarded = q.has("onboarded");
      if (q.get("theme")) st.general.theme = q.get("theme")!;
      return st;
    }
    case "secrets_status":
      return {
        tmdb: false, simkl_client_id: true, simkl_token: true, anthropic: true, simklUser: { user: { name: "xnite" } },
        ...(q.has("signed") ? { stremio_auth: true, stremioUser: { email: "xnite7@icloud.com", fullname: "Sultan" } } : {}),
      };
    case "stremio_link_start":
      return { code: "U1DA", link: "https://link.stremio.com/U1DA", qrcode: "https://link.stremio.com/qr?data=https%3A%2F%2Flink.stremio.com%2FU1DA" };
    case "stremio_link_poll":
      return ++polls > 3;
    case "settings_set":
    case "secret_set":
      return null;
    case "catalogs_list":
      return catalogs;
    case "catalog_fetch": {
      const t = args.catalog.type;
      const list = metas.filter((m) => m.type === t || args.catalog.id === "search");
      return [...list, ...list].map((m, i) => ({ ...m, id: m.id + (i >= list.length ? "b" : "") }));
    }
    case "hero_items":
      return metas.slice(0, 6);
    case "continue_watching":
      return metas.slice(1, 6).map((m, i) => ({
        _id: m.id, name: m.name, type: m.type, poster: m.poster, background: m.background, logo: m.logo, removed: false, temp: true, _mtime: "",
        state: { timeOffset: 1000 * (600 + i * 900), duration: 1000 * 5400, video_id: m.type === "series" ? `${m.id}:1:${i + 2}` : m.id, flaggedWatched: 0, timesWatched: 0, season: 1, episode: i + 2 },
      }));
    case "library_list":
      return metas.slice(0, 9).map((m) => ({ _id: m.id, name: m.name, type: m.type, poster: m.poster, removed: false, temp: false, _mtime: "", state: { timeOffset: 0, duration: 0, flaggedWatched: 0, timesWatched: 0 } }));
    case "meta_get":
      return metas.find((m) => m.id === args.id) ?? metas[0];
    case "meta_enrich":
      return {
        tagline: "Too much light.",
        certification: "PG-13",
        cast: Array.from({ length: 10 }, (_, i) => ({ id: i, name: ["Ada Moss", "Lior Kane", "June Park", "Omar Haddad", "Ivy Lund"][i % 5], character: ["Wren", "The Neighbor", "Dad", "Weatherman", "Ghost"][i % 5], profile: null })),
        directors: ["Mira Solberg"],
        recommendations: metas.slice(3, 11),
        videos: [{ key: "x", name: "Official Trailer", type: "Trailer" }, { key: "y", name: "Behind the field", type: "Featurette" }],
        keywords: ["summer", "nostalgia", "television", "sunflowers"],
        studios: ["Prelinger Pictures"],
        rating: 7.8,
      };
    case "addons_list":
      return [
        { transportUrl: "lumen://builtin/manifest.json", transportName: "builtin", manifest: { id: "app.lumen.builtin", name: "Lumen", version: "0.1.0", description: "Simkl lists, TMDB browse rows, Top 10 and richer artwork — built in.", types: ["movie", "series"], catalogs: [{}, {}, {}], resources: ["catalog", "meta"] }, flags: { official: true, protected: true } },
        { transportUrl: "https://v3-cinemeta.strem.io/manifest.json", transportName: "http", manifest: { id: "com.linvo.cinemeta", name: "Cinemeta", version: "3.0.13", description: "The official addon for movie and series catalogs", types: ["movie", "series"], catalogs: [{}, {}], resources: ["catalog", "meta"] }, flags: { official: true, protected: false } },
        { transportUrl: "https://torrentio.strem.fun/manifest.json", transportName: "http", manifest: { id: "com.stremio.torrentio.addon", name: "Torrentio RD", version: "0.0.15", description: "Provides torrent streams from scraped torrent providers.", types: ["movie", "series"], catalogs: [], resources: ["stream"], behaviorHints: { configurable: true } }, flags: { official: false, protected: false } },
        { transportUrl: "https://opensubtitles-v3.strem.io/manifest.json", transportName: "http", manifest: { id: "org.stremio.opensubtitlesv3", name: "OpenSubtitles v3", version: "1.0.0", description: "OpenSubtitles v3 Addon for Stremio", types: ["movie", "series"], catalogs: [], resources: ["subtitles"] }, flags: { official: true, protected: false } },
      ];
    case "streams_fetch":
      setTimeout(() => emit!("streams", { reqId: args.reqId, addonId: "t", addonName: "Torrentio RD", streams, ms: 640, finished: false }), 400);
      setTimeout(() => emit!("streams", { reqId: args.reqId, finished: true, streams: [] }), 700);
      return null;
    case "search_start":
      setTimeout(() => emit!("search", { reqId: args.reqId, catalog: catalogs[0], metas: metas.filter((m) => m.name.toLowerCase().includes(String(args.query).toLowerCase().slice(0, 2))), finished: false }), 300);
      setTimeout(() => emit!("search", { reqId: args.reqId, finished: true, metas: [], catalog: catalogs[0] }), 500);
      return null;
    case "search_suggest":
      return metas.slice(0, 5).map((m) => ({ id: m.id, name: m.name, year: m.releaseInfo, type: m.type, poster: m.poster }));
    case "recents_get":
      return ["dune", "aftersun", "boards of canada"];
    case "vibe_prompts":
    case "ai_prompts":
      return { prompts: [{ label: "cry all night tonight", q: "cry all night tonight" }, { label: "slow, gorgeous, nothing happens", q: "slow, gorgeous" }, { label: "saltburn + ethel cain had a baby", q: "saltburn + ethel cain" }] };
    case "palette_get":
      return { colors: [[255, 194, 46], [119, 185, 228], [228, 115, 106]], average: [80, 70, 60], dark: true };
    case "art_get":
      return { backdrop: null, poster: null };
    case "player_open":
      setTimeout(() => emit!("player://state", { pos: 1234, duration: 5400, paused: false, buffering: false, cacheAhead: 300, bufferingPct: 100, volume: 80, muted: false, speed: 1, aid: "1", sid: "2", ended: false, idle: false, subOverlay: true, subDelay: 0, video: { width: 3840, height: 2160, hdr: true, gamma: "pq", codec: "hevc", hwdec: "d3d11va", fps: 23.976 } }), 300);
      setTimeout(() => emit!("player://sub", { text: "(soft static)\nThe porch light's been on all summer." }), 400);
      setTimeout(() => emit!("player://loaded", {}), 350);
      if (q.has("playerError")) setTimeout(() => emit!("player://error", { message: "This stream wouldn't open. The link may have expired, or the server isn't answering. Try another stream." }), 600);
      return { mode: "native", url: "" };
    case "skip_segments":
      return { intro: [{ start: 1200, end: 1290 }], credits: [{ start: 5200, end: null }] };
    case "tobii_status":
      return { status: "off" };
    case "themes_user":
      return [];
    case "health":
    case "addons_health":
      return [];
    default:
      return null;
  }
}
