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
  // sized to the name, so long titles aren't clipped
  const w = Math.round(name.length * 58 + 20);
  const svg = `<svg xmlns='http://www.w3.org/2000/svg' width='${w}' height='160' viewBox='0 0 ${w} 160'><text x='0' y='120' font-family='Georgia, serif' font-style='italic' font-weight='700' font-size='104' fill='white' textLength='${w - 20}' lengthAdjust='spacingAndGlyphs'>${name}</text></svg>`;
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
  { key: "c/movie/motn-top", addonId: "c", addonName: "couch", transportUrl: "x", type: "movie", id: "motn-top", name: "Top 10 Today", extra: [], adult: false, builtin: true },
  { key: "e/movie/tmdb-popular", addonId: "e", addonName: "couch", transportUrl: "x", type: "movie", id: "tmdb-popular", name: "Popular", extra: [{ name: "genre", options: ["Action", "Adventure", "Animation", "Comedy", "Crime", "Documentary", "Drama", "Family", "Fantasy", "History", "Horror", "Mystery", "Romance", "Science Fiction", "Thriller"] }], adult: false, builtin: true },
  { key: "e/movie/tmdb-top", addonId: "e", addonName: "couch", transportUrl: "x", type: "movie", id: "tmdb-top", name: "Top Rated", extra: [{ name: "genre", options: ["Drama"] }], adult: false, builtin: true },
  { key: "e/series/tmdb-popular", addonId: "e", addonName: "couch", transportUrl: "x", type: "series", id: "tmdb-popular", name: "Popular", extra: [{ name: "genre", options: ["Action & Adventure", "Animation", "Comedy", "Crime", "Documentary", "Drama", "Family", "Kids", "Mystery", "Reality", "Sci-Fi & Fantasy", "War & Politics", "Western"] }], adult: false, builtin: true },
  { key: "e/series/tmdb-trending", addonId: "e", addonName: "couch", transportUrl: "x", type: "series", id: "tmdb-trending", name: "Trending This Week", extra: [{ name: "genre", options: ["Drama"] }], adult: false, builtin: true },
  { key: "d/movie/simkl-recs", addonId: "d", addonName: "couch", transportUrl: "x", type: "movie", id: "simkl-recs", name: "Because You Watched", extra: [], adult: false, builtin: true },
];

const streams = [
  { name: "[RD+] Torrentio\n4K DV", title: "The.Long.Summer.2024.2160p.BluRay.REMUX.DV.HDR10.HEVC.TrueHD.Atmos.7.1-FGT\n👤 512 💾 61.2 GB", _couch: { res: "4K", resRank: 4, hdr: ["DV", "HDR10"], codec: "HEVC", audio: ["Atmos", "TrueHD", "7.1"], source: "REMUX", sizeBytes: 61.2e9, seeders: 512, cached: true, debrid: "RD", kind: "debrid", cam: false, threeD: false, languages: [], group: "FGT", label: "The.Long.Summer.2024.2160p.BluRay.REMUX.DV.HDR10", score: 99000 } },
  { name: "[RD+] Torrentio\n1080p", title: "The.Long.Summer.2024.1080p.WEB-DL.DDP5.1.H.264-NTb\n👤 1203 💾 6.1 GB", _couch: { res: "1080p", resRank: 3, hdr: [], codec: "AVC", audio: ["DD+", "5.1"], source: "WEB-DL", sizeBytes: 6.1e9, seeders: 1203, cached: true, debrid: "RD", kind: "debrid", cam: false, threeD: false, languages: [], group: "NTb", label: "The.Long.Summer.2024.1080p.WEB-DL.DDP5.1", score: 88000 } },
  { name: "Torrentio\n720p", title: "The.Long.Summer.2024.720p.WEBRip.x265\n👤 88 💾 1.4 GB", _couch: { res: "720p", resRank: 2, hdr: [], codec: "HEVC", audio: [], source: "WEBRip", sizeBytes: 1.4e9, seeders: 88, kind: "torrent", cam: false, threeD: false, languages: [], label: "The.Long.Summer.2024.720p.WEBRip.x265", score: 21000 } },
];

let emit: ((e: string, p: unknown) => void) | null = null;
let polls = 0;
let askPolls = 0;
let knocked = false;
const bootAt = Date.now();
const q = typeof location !== "undefined" ? new URLSearchParams(location.search) : new URLSearchParams();
// the hosted live preview (npm run demo): signed in, set up, phone layout on phones
if (import.meta.env.VITE_DEMO) {
  q.set("onboarded", "");
  q.set("signed", "");
  q.set("crew", "");
  if (!q.has("platform") && /iPhone|iPad|Android/i.test(navigator.userAgent)) q.set("platform", "ios");
}

// ------------------------------------------------------------ fake player
const mp = { pos: 1234, duration: 2700, paused: false, volume: 80, muted: false, speed: 1, aid: "1", sid: "3" };
let mpTimer: ReturnType<typeof setInterval> | undefined;
function pushMock() {
  emit?.("player://state", { ...mp, buffering: false, cacheAhead: 300, bufferingPct: 100, ended: false, idle: false, subOverlay: true, subDelay: 0, video: { width: 3840, height: 2160, hdr: true, gamma: "pq", codec: "hevc", hwdec: "d3d11va", fps: 23.976 } });
}
function startMockPlayer() {
  clearInterval(mpTimer);
  mpTimer = setInterval(() => {
    if (!mp.paused) mp.pos = Math.min(mp.duration, mp.pos + mp.speed);
    pushMock();
  }, 1000);
  setTimeout(pushMock, 300);
}

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
      if (q.has("tobii")) {
        st.tobii.enabled = true;
        st.general.experimental = true;
      }
      return st;
    }
    case "secrets_status":
      return {
        tmdb: false, simkl_client_id: true, simkl_token: true, smartBar: true, simklUser: { user: { name: "xnite" } },
        ...(q.has("signed") ? { discordUser: { id: "1", name: "xnite", avatar: "" } } : {}),
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
      return metas.slice(0, 6).map((m, i) => ({ ...m, trendRank: i + 1, watchingNow: [25613, 4120, 980, 310, 77, 12][i] }));
    case "home_rows": {
      const all = [
        { id: "couch-hits", type: "series", name: "Hit Shows You Might Have Missed" },
        { id: "couch-hits", type: "movie", name: "Hit Films You Might Have Missed" },
        { id: "couch-season", type: "movie", name: "Cozy Nights In" },
        { id: "couch-emmy", type: "series", name: "Emmy-Winning Series" },
        { id: "couch-oscar", type: "movie", name: "Best Picture Winners" },
        { id: "couch-morelike", type: "series", name: "More Like Night Swim", extra: [["seed", "tt1"]] },
      ];
      return all.filter((r) => args.scope === "all" || r.type === args.scope);
    }
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
        { transportUrl: "couch://builtin/manifest.json", transportName: "builtin", manifest: { id: "com.midnite.couch.builtin", name: "couch", version: "0.1.0", description: "Simkl lists, TMDB browse rows, Top 10 and richer artwork — built in.", types: ["movie", "series"], catalogs: [{}, {}, {}], resources: ["catalog", "meta"] }, flags: { official: true, protected: true } },
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
    case "ai_vibe":
      return new Promise((r) => setTimeout(() => r({ read: "You want to feel it in your chest: quiet grief, gorgeous light, nobody explaining anything.", picks: metas.slice(0, 3).map((m) => ({ ...m, year: m.releaseInfo, why: "Slow, sunlit and devastating in the way your top-rated dramas are." })) }), 700));
    case "vibe_prompts":
    case "ai_prompts":
      return { prompts: [{ label: "cry all night tonight", q: "cry all night tonight" }, { label: "slow, gorgeous, nothing happens", q: "slow, gorgeous" }, { label: "grown-up superhero, nothing cringe", q: "a superhero series that is not cringe or for kids" }] };
    case "palette_get":
      return { colors: [[255, 194, 46], [119, 185, 228], [228, 115, 106]], average: [80, 70, 60], dark: true };
    case "art_get":
      return { backdrop: null, poster: null };
    case "player_open":
    case "player_next": {
      // a pretend player that answers play/pause/seek, so the preview behaves
      mp.pos = cmd === "player_next" ? 0 : 1234;
      mp.paused = false;
      startMockPlayer();
      setTimeout(() => emit!("player://tracks", [
        { id: "1", type: "audio", lang: "eng", title: "English", channels: 6, codec: "eac3", selected: true },
        { id: "2", type: "audio", lang: "jpn", title: "Japanese", channels: 2, codec: "aac", selected: false },
        { id: "3", type: "sub", lang: "eng", title: "English", codec: "subrip", selected: true },
        { id: "4", type: "sub", lang: "ara", title: "Arabic", codec: "subrip", selected: false },
        { id: "5", type: "sub", lang: "spa", title: "Spanish", codec: "subrip", selected: false },
      ]), 300);
      setTimeout(() => emit!("player://sub", { text: "(soft static)\nThe porch light's been on all summer." }), 400);
      setTimeout(() => emit!("player://loaded", {}), 350);
      if (q.has("playerError")) setTimeout(() => emit!("player://error", { message: "This stream wouldn't open. The link may have expired, or the server isn't answering. Try another stream." }), 600);
      const open = { mode: "native", url: "" };
      return cmd === "player_next" ? { stream: args.previous, open } : open;
    }
    case "player_cmd": {
      const a = args.args ?? {};
      if (args.cmd === "pause") mp.paused = a.value ?? !mp.paused;
      else if (args.cmd === "seek") mp.pos = Math.max(0, Math.min(mp.duration, a.relative ? mp.pos + a.to : a.to));
      else if (args.cmd === "volume") mp.volume = a.value;
      else if (args.cmd === "mute") mp.muted = a.value ?? !mp.muted;
      else if (args.cmd === "speed") mp.speed = a.value;
      else if (args.cmd === "sid") mp.sid = String(a.value);
      else if (args.cmd === "aid") mp.aid = String(a.value);
      pushMock();
      return null;
    }
    case "player_stop":
      clearInterval(mpTimer);
      return null;
    case "skip_segments":
      return { recap: [{ start: 0, end: 70 }], intro: [{ start: 380, end: 470 }], credits: [{ start: 4600, end: 5300 }], preview: [{ start: 5300, end: null }] };
    case "tobii_status":
      return { status: "off" };
    case "crew_list":
    case "crew_join":
      return q.has("crew")
        ? {
            setup: true,
            members: [
              { id: "a", name: "mari", watching: { title: "Night Swim", episode: "S1 · E4 · The Deep End", metaId: metas[3].id, type: "series", background: metas[3].background }, paused: false, pos: 900, duration: 2700, at: Date.now() },
              { id: "b", name: "hazza", watching: { title: "The Long Summer", year: 2018, metaId: metas[0].id, type: "movie", background: metas[0].background }, paused: true, pos: 3000, duration: 5760, at: Date.now() },
            ],
          }
        : { setup: false, members: [] };
    // watch together (preview): asks get accepted after a few seconds, and
    // hazza knocks once, ten seconds in
    case "crew_ask":
      askPolls = 0;
      return { id: "ask-1" };
    case "crew_ask_status":
      return { status: ++askPolls >= 2 ? "accepted" : "pending" };
    case "crew_inbox":
      return { asks: !knocked && Date.now() - bootAt > 10_000 ? [{ id: "ask-2", from: "b", fromName: "hazza", at: Date.now() }] : [] };
    case "crew_answer":
      knocked = true;
      return null;
    case "crew_room":
      return { room: { host: args.host, metaId: metas[3].id, type: "series", videoId: metas[3].id + ":1:4", title: "Night Swim", pos: 900, now: 900, paused: false, guests: ["me"] } };
    case "crew_leave":
      return null;
    case "crew_invite":
      return q.has("crew") ? "https://midnite-couch.example.workers.dev/join?c=xxxx" : null;
    case "health":
    case "addons_health":
      return [];
    case "streaming_services":
      return ["Netflix", "Max", "Disney+", "Prime Video", "Apple TV+", "Hulu", "Paramount+", "Peacock", "Crunchyroll"].map((name, i) => ({ id: i + 1, name, logo: null }));
    case "discord_me":
      return { id: "me", name: "sultan", avatar: undefined };
    case "calendar_get": {
      const day = (k: number) => { const d = new Date(); d.setDate(d.getDate() + k); return d.toISOString().slice(0, 10); };
      const L = ["S2 E5", "in cinemas", "season 3 premiere", "at home", "S1 E8", "from Glasshouse", "S4 E2", "in cinemas"];
      const S = ["yours", "hit", "yours", "yours", "hit", "follow", "yours", "hit"];
      return metas.slice(0, 16).map((m: any, k: number) => ({ ...m, date: day([-1, 0, 0, 1, 1, 2, 4, 4, 6, 9, 9, 12, 15, 18, 18, 20][k]), label: L[k % 8], episode: k % 2 ? null : "The Long Way Round", source: S[k % 8] }))
        .filter((x: any) => args.hits || x.source !== "hit");
    }
    case "wrapped_data": {
      const G = [["Drama", "Mystery"], ["Comedy"], ["Drama", "Thriller"], ["Sci-Fi", "Drama"], ["Animation", "Comedy"], ["Thriller"]];
      return metas.slice(0, 14).map((m: any, k: number) => ({
        id: m.id, type: m.type, name: m.name, poster: m.poster, background: m.background,
        genres: G[k % G.length], at: new Date(Date.UTC(args.year, (k * 5) % 12, 3 + k, (k * 7) % 24)).toISOString(),
        status: "completed", userRating: k % 3 === 0 ? 10 : k % 3 === 1 ? 8 : null, eps: m.type === "series" ? 8 + k * 3 : null, minutes: m.type === "series" ? 45 : 118,
      }));
    }
    case "local_ratings":
      return {};
    case "share_link":
      return `https://midnite-couch.xnite7.workers.dev/t/${args.kind}/${args.id}`;
    case "simkl_episode_info":
      return { "1:6": { runtime: 52, finale: 1 }, "1:10": { runtime: 61, finale: 2 } };
    case "my_lists":
      return { simkl: false };
    case "simkl_custom_lists":
      return { paid: false, lists: [] };
    case "simkl_list_items":
      return [];
    case "crew_social": {
      const p: string = args.path;
      if (p === "/v1/favs") return { favs: [] };
      if (p.startsWith("/v1/reviews/")) return { reviews: [] };
      if (p.startsWith("/v1/react/")) return { reactions: [] };
      if (p === "/v1/people") {
        const now = Date.now();
        return { people: [
          { id: "100000000000000001", name: "nora", accent: "#4cc9f0", at: now - 6e4 },
          { id: "100000000000000002", name: "dex", accent: "#ffb25c", at: now - 36e5 },
          { id: "100000000000000003", name: "mika", accent: "#7ee081", at: now - 9e6 },
        ] };
      }
      if (/^\/v1\/profile\/\d+$/.test(p) && args.method === "GET") {
        const now = Date.now();
        const it = (i: number, extra: any = {}) => ({ id: metas[i % metas.length].id, type: metas[i % metas.length].type, name: metas[i % metas.length].name, poster: metas[i % metas.length].poster, background: metas[i % metas.length].background, logo: (metas[i % metas.length] as any).logo, ...extra });
        const series = metas.map((m, i) => [m, i] as const).filter(([m]) => m.type === "series").map(([, i]) => i);
        const movies = metas.map((m, i) => [m, i] as const).filter(([m]) => m.type !== "series").map(([, i]) => i);
        return {
          id: p.split("/").pop(),
          name: "nora",
          profile: {
            source: "simkl",
            at: now - 6e5,
            stats: { movies: 412, shows: 57, episodes: 3184, watching: 6, plan: 88 },
            watching: [...series.slice(0, 4).map((i, k) => it(i, { eps: [3 + k * 4, 10 + k * 6] })), ...movies.slice(0, 2).map((i) => it(i))],
            recent: movies.slice(2, 12).map((i, k) => it(i, { rating: k % 3 ? 7 + (k % 4) : undefined, at: now - k * 864e5 })),
            plan: metas.slice(0, 10).map((_, i) => it(i + 3)),
          },
          style: { accent: "#4cc9f0", glow: true },
          favs: movies.slice(0, 6).map((i) => it(i, { at: now })),
          activity: [{ id: "x1", kind: "review", title: it(1), rating: 9, text: "the last twenty minutes had me holding my breath. go in blind.", at: now - 42 * 6e4 }],
        };
      }
      if (p === "/v1/activity" && args.method === "GET") {
        const now = Date.now();
        const t = (i: number) => ({ id: metas[i].id, type: metas[i].type, name: metas[i].name, poster: metas[i].poster, background: metas[i].background, logo: (metas[i] as any).logo });
        return {
          feed: [
            { id: "a1", who: "u1", name: "nora", kind: "review", title: t(1), rating: 9, text: "the last twenty minutes had me holding my breath. go in blind.", at: now - 1000 * 60 * 42 },
            { id: "a2", who: "u2", name: "dex", kind: "rating", title: t(2), rating: 7, at: now - 1000 * 60 * 60 * 3 },
            { id: "a3", who: "u3", name: "mika", kind: "finished", title: t(3), at: now - 1000 * 60 * 60 * 9 },
            { id: "a4", who: "u1", name: "nora", kind: "review", title: t(4), rating: 6, text: "pretty to look at, forgot it by morning.", at: now - 1000 * 60 * 60 * 30 },
          ],
        };
      }
      return { ok: true };
    }
    default:
      return null;
  }
}
