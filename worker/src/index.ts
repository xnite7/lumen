// midnite™ couch crew server: presence + watch together (ask / answer / room).
//
// One Durable Object holds who's on the couch right now, in memory only
// (anyone who stops sending heartbeats is forgotten after two minutes).
// The lasting part, kept in its storage: favourites, the crew's reviews and
// ratings, and the activity feed (Simkl's API has none of these).
//
// Share links: /t/<type>/<id> opens the title in the app (couch://title/...).
//
// Who may use it: members of the midnite Discord server. The app sends its
// Discord access token to /v1/auth once; this checks the server list with
// Discord itself and hands back a signed session (HMAC with CREW_CODE). Every
// other call carries that session, and your id IS your Discord id, so nobody
// can act as someone else.
//
//   PUT    /v1/presence        { id, name, watching, paused, pos, duration }
//   DELETE /v1/presence/:id
//   GET    /v1/presence     -> { members: [...] }
//   GET    /logo.png           the midnite mark (Discord's small image)

import logo from "./logo.png";
import { TERMS_HTML } from "./terms";

export interface Env {
  CREW: DurableObjectNamespace;
  /** signing key for sessions */
  CREW_CODE: string;
  /** the Discord server couch is for */
  GUILD_ID?: string;
  /** who gets in: role ids (server-subscription roles + any you pick),
   *  comma-separated. Unset = everyone in the server. */
  ALLOWED_ROLES?: string;
  /** couch's bot, sitting in the server: reads members' roles, so nobody has
   *  to give couch an extra permission or sign in again */
  DISCORD_BOT_TOKEN?: string;
  /** ClipCafe PRO key: scene clips for billboard / preview (instead of trailers) */
  CLIPCAFE_KEY?: string;
  /** read-only GitHub token for the app's releases (updates) */
  GH_TOKEN?: string;
  /** owner/repo holding the releases */
  GH_REPO?: string;
  /** Discord client secret: only for sign-ins through the Discord app */
  DISCORD_SECRET?: string;
}

const DISCORD_APP = "1568062298721886290";

/** Sign-in through the Discord desktop app: its code needs the client
 *  secret to become a token, and the secret lives here, not in the app. */
async function discordToken(req: Request, env: Env, kind: "exchange" | "refresh"): Promise<Response> {
  if (!env.DISCORD_SECRET) return json({ error: "desktop sign-in isn't set up" }, 501);
  let b: any;
  try {
    b = await req.json();
  } catch {
    return json({ error: "bad json" }, 400);
  }
  const form = new URLSearchParams({ client_id: DISCORD_APP, client_secret: env.DISCORD_SECRET });
  if (kind === "exchange") {
    form.set("grant_type", "authorization_code");
    form.set("code", String(b?.code ?? ""));
    form.set("redirect_uri", String(b?.redirect_uri ?? ""));
  } else {
    form.set("grant_type", "refresh_token");
    form.set("refresh_token", String(b?.refresh_token ?? ""));
  }
  const r = await fetch("https://discord.com/api/oauth2/token", { method: "POST", body: form, headers: { "content-type": "application/x-www-form-urlencoded" } });
  return new Response(r.body, { status: r.status, headers: { "content-type": "application/json", "cache-control": "no-store" } });
}

// ------------------------------------------------------------- updates
// The app asks /v1/update/<target>/<arch>/<current>; this reads the newest
// GitHub release's latest.json and answers with it when it's newer (204
// otherwise). Installers stream through /v1/update/file/<name>, so the repo
// stays private and only server members get builds.

const newer = (a: string, b: string) => {
  const pa = a.split(".").map(Number), pb = b.split(".").map(Number);
  for (let i = 0; i < 3; i++) if ((pa[i] ?? 0) !== (pb[i] ?? 0)) return (pa[i] ?? 0) > (pb[i] ?? 0);
  return false;
};

async function gh(env: Env, path: string, accept = "application/vnd.github+json") {
  return fetch(`https://api.github.com/repos/${env.GH_REPO || "xnite7/lumen"}${path}`, {
    headers: { authorization: `Bearer ${env.GH_TOKEN}`, accept, "user-agent": "midnite-couch" },
    redirect: "manual",
  });
}

/** an asset's bytes: GitHub answers with a redirect to signed storage */
async function asset(env: Env, id: number) {
  const r = await gh(env, `/releases/assets/${id}`, "application/octet-stream");
  const to = r.headers.get("location");
  return to ? fetch(to) : r;
}

async function updates(url: URL, env: Env): Promise<Response> {
  if (!env.GH_TOKEN) return new Response(null, { status: 204 });
  const rel = await gh(env, "/releases/latest");
  if (!rel.ok) return new Response(null, { status: 204 });
  const r: any = await rel.json();
  const assets: any[] = r.assets ?? [];
  const file = url.pathname.match(/^\/v1\/update\/file\/([^/]+)$/);
  if (file) {
    const a = assets.find((x) => x.name === decodeURIComponent(file[1]));
    if (!a) return json({ error: "not here" }, 404);
    const bin = await asset(env, a.id);
    return new Response(bin.body, { headers: { "content-type": "application/octet-stream", "content-length": String(a.size) } });
  }
  const m = url.pathname.match(/^\/v1\/update\/([^/]+)\/([^/]+)\/([^/]+)$/);
  if (!m) return json({ error: "not here" }, 404);
  const manifest = assets.find((x) => x.name === "latest.json");
  if (!manifest) return new Response(null, { status: 204 });
  const latest: any = await (await asset(env, manifest.id)).json();
  if (!newer(String(latest.version ?? "0"), m[3])) return new Response(null, { status: 204 });
  return json(latest);
}

// ----------------------------------------------------- the new app (beta)
// The GPUI build: prereleases tagged gpui-<n>, each with couch-gpui.zip.
// /v1/gpui/latest -> { version, size } ; /v1/gpui/file -> the zip.
async function gpui(url: URL, env: Env): Promise<Response> {
  if (!env.GH_TOKEN) return json({ error: "no builds yet" }, 404);
  const rels = await gh(env, "/releases?per_page=20");
  if (!rels.ok) return json({ error: "no builds yet" }, 404);
  const list: any[] = await rels.json();
  const rel = list.find((r) => String(r.tag_name).startsWith("gpui-"));
  const zip = rel?.assets?.find((a: any) => a.name === "couch-gpui.zip");
  if (!rel || !zip) return json({ error: "no builds yet" }, 404);
  if (url.pathname === "/v1/gpui/file") {
    const bin = await asset(env, zip.id);
    return new Response(bin.body, { headers: { "content-type": "application/zip", "content-length": String(zip.size) } });
  }
  return json({ version: String(rel.tag_name).slice(5), size: zip.size, notes: rel.body ?? "" });
}

type Who = { id: string; name: string; avatar?: string };
// short on purpose: a session is re-earned from Discord every couple of
// minutes, so someone kicked from the server is out right away
const SESSION_TTL = 120;
const enc = new TextEncoder();
const b64u = (b: ArrayBuffer | Uint8Array) => btoa(String.fromCharCode(...new Uint8Array(b))).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
// the token is UTF-8 JSON: decode its bytes as UTF-8, not as Latin-1, or "✿" turns into "â¿"
const unb64u = (s: string) => new TextDecoder().decode(Uint8Array.from(atob(s.replace(/-/g, "+").replace(/_/g, "/")), (c) => c.charCodeAt(0)));

async function hmac(key: string, data: string) {
  const k = await crypto.subtle.importKey("raw", enc.encode(key), { name: "HMAC", hash: "SHA-256" }, false, ["sign"]);
  return b64u(await crypto.subtle.sign("HMAC", k, enc.encode(data)));
}

async function sign(env: Env, who: Who) {
  const body = b64u(enc.encode(JSON.stringify({ ...who, exp: Math.floor(Date.now() / 1000) + SESSION_TTL })));
  return `${body}.${await hmac(env.CREW_CODE, body)}`;
}

async function verify(env: Env, header: string | null): Promise<Who | null> {
  const t = header?.replace(/^Bearer\s+/i, "") ?? "";
  const [body, mac] = t.split(".");
  if (!body || !mac || !safeEqual(mac, await hmac(env.CREW_CODE, body))) return null;
  try {
    const v = JSON.parse(unb64u(body));
    if (typeof v.exp !== "number" || v.exp < Date.now() / 1000) return null;
    return { id: String(v.id), name: String(v.name ?? "someone"), avatar: v.avatar || undefined };
  } catch {
    return null;
  }
}

/** Discord says who you are and which servers you're in. */
async function auth(req: Request, env: Env): Promise<Response> {
  let b: any;
  try {
    b = await req.json();
  } catch {
    return json({ error: "bad json" }, 400);
  }
  const token = typeof b?.token === "string" ? b.token : "";
  if (!token) return json({ error: "no token" }, 400);
  const h = { authorization: `Bearer ${token}` };
  const me = await fetch("https://discord.com/api/users/@me", { headers: h });
  if (me.status === 429 || me.status >= 500) return json({ error: "discord is busy, try again" }, 503);
  if (!me.ok) return json({ error: "discord didn't recognise that sign-in" }, 401);
  const u: any = await me.json();
  if (env.GUILD_ID) {
    const roles = (env.ALLOWED_ROLES ?? "").split(/[\s,]+/).filter(Boolean);
    if (roles.length && env.DISCORD_BOT_TOKEN) {
      // the bot sees membership AND roles in one call; the user's own server
      // list isn't needed (and is heavily rate-limited by Discord)
      const br = await fetch(`https://discord.com/api/guilds/${env.GUILD_ID}/members/${u.id}`, { headers: { authorization: `Bot ${env.DISCORD_BOT_TOKEN}` } });
      if (br.status === 404) return json({ error: "not in the server", notMember: true }, 403);
      if (!br.ok) return json({ error: "couldn't check your roles" }, 503);
      const mem: any = await br.json();
      if (!(mem?.roles ?? []).some((r: string) => roles.includes(r))) return json({ error: "no subscription", notMember: true }, 403);
    } else {
      const g = await fetch("https://discord.com/api/users/@me/guilds", { headers: h });
      if (!g.ok) return json({ error: "couldn't check your servers" }, 503);
      const list: any = await g.json();
      if (!Array.isArray(list)) return json({ error: "couldn't check your servers" }, 503);
      if (!list.some((x: any) => x?.id === env.GUILD_ID)) return json({ error: "not in the server", notMember: true }, 403);
    }
    if (roles.length && !env.DISCORD_BOT_TOKEN) {
      const mr = await fetch(`https://discord.com/api/users/@me/guilds/${env.GUILD_ID}/member`, { headers: h });
      // no permission to see roles = an old sign-in (or an old app): no way in
      // until they sign in again; new apps show "one more knock", old ones lock
      if (mr.status === 401 || mr.status === 403) return json({ error: "sign in again (couch needs to see your server roles)", reauth: true, notMember: true }, 403);
      if (!mr.ok) return json({ error: "couldn't check your roles" }, 503);
      const mem: any = await mr.json();
      const has = (mem?.roles ?? []).some((r: string) => roles.includes(r));
      if (!has) return json({ error: "no subscription", notMember: true }, 403);
    }
  }
  const who: Who = {
    id: String(u.id),
    name: String(u.global_name || u.username || "someone").slice(0, 40),
    avatar: u.avatar ? `https://cdn.discordapp.com/avatars/${u.id}/${u.avatar}.png?size=128` : undefined,
  };
  return json({ session: await sign(env, who), exp: Math.floor(Date.now() / 1000) + SESSION_TTL, id: who.id });
}

const STALE_MS = 120_000;
const MAX_MEMBERS = 64;

type Member = {
  id: string;
  name: string;
  avatar?: string;
  watching: Record<string, unknown> | null;
  paused: boolean;
  pos: number;
  duration: number;
  at: number;
};

/** "can I join your couch?" */
type Ask = { id: string; from: string; fromName: string; fromAvatar?: string; to: string; at: number; status: "pending" | "accepted" | "declined" | "ended" };
/** a host's playback, followed by the guests they let in */
type Room = { host: string; metaId: string; type: string; videoId: string; title?: string; sk?: string; pos: number; paused: boolean; at: number; guests: string[]; reacts?: React[]; shared?: boolean; cmds?: Cmd[]; stalled?: boolean; wsAt?: number };
/** a guest steering the host's player (only when the host shares control) */
type Cmd = { from: string; name: string; action: "pause" | "play" | "seek"; pos?: number; at: number };
type React = { n: number; who: string; name: string; avatar?: string; emoji: string; at: number };
/** one line of the crew's activity: a review, a rating, or "finished X" */
type Act = { id: string; who: string; name: string; avatar?: string; kind: "review" | "rating" | "finished"; title: { id: string; type: string; name: string; poster?: string; background?: string; logo?: string }; rating?: number; text?: string; at: number };
type Fav = { id: string; type: string; name: string; poster?: string; at: number };
/** one title on someone's profile */
type PItem = { id: string; type: string; name: string; poster?: string; background?: string; logo?: string; rating?: number; at?: number; eps?: [number, number] };
/** what someone's couch publishes about them (from their Simkl, or their couch library) */
type Follow = { kind: "person" | "studio"; id: number; name: string; img?: string };
type Taste = { ids: string[]; ratings: Record<string, number>; loved: PItem[] };
type Profile = { watching: PItem[]; recent: PItem[]; plan: PItem[]; follows?: Follow[]; taste?: Taste; stats: Record<string, number>; source: "simkl" | "couch"; at: number };
/** the bits of their page they pick themselves */
type Style = { accent?: string; glow?: boolean };
/** a watch party: a title, a time, who's invited and who's coming */
type Party = {
  id: string;
  host: string;
  hostName: string;
  hostAvatar?: string;
  title: { id: string; type: string; name: string; poster?: string; background?: string; logo?: string; year?: string; runtime?: string };
  videoId?: string;
  at: number;
  note?: string;
  invited: string[] | "all";
  rsvp: Record<string, { yes: boolean; name: string; avatar?: string }>;
  created: number;
};
type Person = { id: string; name: string; avatar?: string; accent?: string; at: number };
const FEED_MAX = 150;
const REACTIONS = ["😂", "😭", "😱", "🔥", "❤️", "👀", "💀", "🍿", "👏", "🤯"];
const ASK_TTL = 120_000;
const ROOM_TTL = 90_000;

const json = (v: unknown, status = 200) =>
  new Response(JSON.stringify(v), { status, headers: { "content-type": "application/json", "cache-control": "no-store" } });

function safeEqual(a: string, b: string) {
  if (!a || !b || a.length !== b.length) return false;
  let d = 0;
  for (let i = 0; i < a.length; i++) d |= a.charCodeAt(i) ^ b.charCodeAt(i);
  return d === 0;
}

const str = (v: unknown, max: number) => (typeof v === "string" ? v.slice(0, max) : undefined);
const num = (v: unknown) => (typeof v === "number" && isFinite(v) ? v : 0);

export default {
  async fetch(req: Request, env: Env): Promise<Response> {
    const url = new URL(req.url);
    if (url.pathname === "/logo.png") {
      return new Response(logo, { headers: { "content-type": "image/png", "cache-control": "public, max-age=86400" } });
    }
    if (url.pathname === "/") return new Response("on the couch.\n");
    if (url.pathname === "/join") return joinPage(url);
    if (url.pathname === "/terms" || url.pathname === "/tos") {
      return new Response(TERMS_HTML, { headers: { "content-type": "text/html; charset=utf-8", "cache-control": "public, max-age=600" } });
    }
    const share = url.pathname.match(/^\/t\/(movie|series)\/([A-Za-z0-9:._-]{1,100})$/);
    if (share) return titlePage(share[1], share[2], url.searchParams.get("n") ?? "");
    // (these two live outside /v1/, so they come before the /v1/ gate)
    // a scene clip as video (the <video> can't send headers: session in the URL)
    const clipFile = url.pathname.match(/^\/clip\/(tt\d{5,10})$/);
    if (clipFile) {
      const w = await verify(env, `Bearer ${url.searchParams.get("t") ?? ""}`);
      if (!w) return new Response("sign in again", { status: 401 });
      return clipStream(env, clipFile[1], req, url.searchParams.get("title"), url.searchParams.get("year"));
    }
    const clipCheck = url.pathname.match(/^\/clip-status\/(tt\d{5,10})$/);
    if (clipCheck) return clipStatus(env, clipCheck[1], false, req); // download check verified; off so nobody burns the monthly downloads
    // which country the app is in (for "Top 10 in UAE"); just the 2-letter code
    if (url.pathname === "/geo") return json({ country: String((req as any).cf?.country ?? "").toLowerCase() });
    if (!url.pathname.startsWith("/v1/")) return json({ error: "not here" }, 404);
    if (req.method === "POST" && url.pathname === "/v1/auth") return auth(req, env);
    if (req.method === "POST" && url.pathname === "/v1/discord/exchange") return discordToken(req, env, "exchange");
    if (req.method === "POST" && url.pathname === "/v1/discord/refresh") return discordToken(req, env, "refresh");
    // watch together's live line: a WebSocket can't send headers, so the
    // session rides in the URL (like the clip video)
    const isWs = /^\/v1\/room\/[A-Za-z0-9_-]{1,40}\/ws$/.test(url.pathname) && req.headers.get("upgrade") === "websocket";
    const who = await verify(env, isWs ? `Bearer ${url.searchParams.get("t") ?? ""}` : req.headers.get("authorization"));
    if (!who) return json({ error: "sign in again" }, 401);
    const clipAsk = url.pathname.match(/^\/v1\/clip\/(tt\d{5,10})$/);
    if (clipAsk) return json({ clip: !!(await clipPick(env, clipAsk[1], url.searchParams.get("title"), url.searchParams.get("year"))) });
    if (url.pathname.startsWith("/v1/update/")) return updates(url, env);
    if (url.pathname === "/v1/gpui/latest" || url.pathname === "/v1/gpui/file") return gpui(url, env);
    // the Durable Object trusts this header; set it here, never from the client
    const fwd = new Request(req);
    fwd.headers.set("x-who", JSON.stringify(who));
    const stub = env.CREW.get(env.CREW.idFromName("midnite"));
    return stub.fetch(fwd);
  },
};

// ------------------------------------------------------------ clips
// The most watched ClipCafe scene for a title that runs at least 20 s (else
// 15, else 10): what the billboard and previews play instead of a trailer.
// The pick and the video are kept in Cloudflare's cache, so each clip costs
// one ClipCafe download however many people watch it.

type Clip = { slug: string; download: string; duration: number } | null;
const CLIP_DAYS = 30;

// ClipCafe: needs the CLIPCAFE_KEY secret. Previews play the most-viewed
// clip, 20s+ then 15s+ then 10s+. A search hands out per-clip download keys
// that die after 5 minutes, so only the clip's slug is remembered and a
// fresh key is fetched right before the (one-time, then cached) download.
async function clipSearch(env: Env, q: string): Promise<{ status: number; error?: string; clips: any[] }> {
  const r = await fetch(`https://api.clip.cafe/?api_key=${encodeURIComponent(env.CLIPCAFE_KEY ?? "")}&${q}`);
  const d: any = await r.json().catch(() => null);
  const clips = (d?.hits?.hits ?? []).map((h: any) => h?._source).filter((c: any) => c?.slug);
  return { status: r.status, error: d?.error ? String(d.error).slice(0, 200) : undefined, clips };
}

async function clipPick(env: Env, imdb: string, title?: string | null, year?: string | null): Promise<Clip> {
  if (!env.CLIPCAFE_KEY) return null;
  const cache = caches.default;
  const key = new Request(`https://couch.clips/pick4/${imdb}`);
  const hit = await cache.match(key);
  if (hit) return (await hit.json()) as Clip;
  let pick: Clip = null;
  let failed = false;
  // ClipCafe sometimes files a film under a different IMDb id (Zootopia 2):
  // nothing for the id -> the same title (and year), checked word for word
  const norm = (x: unknown) => String(x ?? "").toLowerCase().replace(/[^a-z0-9]+/g, " ").trim();
  let by = `imdb=${imdb}`;
  if (title) {
    const probe = await clipSearch(env, `imdb=${imdb}&size=1`);
    if (probe.status === 200 && !probe.clips.length) {
      const y = year && /^\d{4}$/.test(year) ? `&movie_year=${year}` : "";
      const t = await clipSearch(env, `movie_title=${encodeURIComponent(title)}${y}&size=5`);
      const hit = t.clips.find((c: any) => norm(c.movie_title) === norm(title));
      if (hit?.imdb) by = `imdb=${encodeURIComponent(String(hit.imdb))}`;
      else if (hit) by = `movie_title=${encodeURIComponent(title)}${y}`;
    }
  }
  for (const min of [20, 15, 10]) {
    const r = await clipSearch(env, `${by}&duration=${min}-&size=25`);
    if (r.status !== 200) {
      failed = true; // key/limit trouble: don't remember "no clip"
      break;
    }
    // a scene, not a whole sequence: big, long clips (the most-viewed ones for
    // The Matrix, Pulp Fiction, Money Heist) were too heavy to fetch and cache
    const fit = r.clips.filter((c: any) => (Number(c.duration) || 0) <= 75);
    if (fit.length) {
      fit.sort((a: any, b: any) => (Number(b.views) || 0) - (Number(a.views) || 0));
      const c = fit[0];
      pick = { slug: String(c.slug), download: String(c.download ?? ""), duration: Number(c.duration) || 0 };
      break;
    }
  }
  if (!failed) await cache.put(key, new Response(JSON.stringify(pick), { headers: { "content-type": "application/json", "cache-control": `public, max-age=${pick ? CLIP_DAYS * 86400 : 86400}` } }));
  return pick;
}

/** the download address for a clip, with a key that's fresh right now */
async function clipDownloadUrl(env: Env, pick: NonNullable<Clip>): Promise<string | null> {
  const r = await clipSearch(env, `slug=${encodeURIComponent(pick.slug)}&size=1`);
  const c = r.clips.find((x: any) => x.slug === pick.slug) ?? r.clips[0];
  const dl = String(c?.download ?? pick.download ?? "");
  if (!dl) return null;
  const k = encodeURIComponent(env.CLIPCAFE_KEY ?? "");
  if (/^https?:/.test(dl)) return dl.includes("api_key=") ? dl : `${dl}${dl.includes("?") ? "&" : "?"}api_key=${k}`;
  // a bare download key
  return `https://api.clip.cafe/?api_key=${k}&slug=${encodeURIComponent(pick.slug)}&key=${encodeURIComponent(dl)}`;
}

async function clipStream(env: Env, imdb: string, req: Request, title?: string | null, year?: string | null): Promise<Response> {
  const cache = caches.default;
  const fileKey = `https://couch.clips/file2/${imdb}.mp4`;
  // ranges (seeking) are answered straight from the cache
  const cached = await cache.match(new Request(fileKey, { headers: req.headers.get("range") ? { range: req.headers.get("range")! } : {} }));
  if (cached) return cached;
  const pick = await clipPick(env, imdb, title, year);
  if (!pick) return new Response("no clip", { status: 404 });
  const dl = await clipDownloadUrl(env, pick);
  if (!dl) return new Response("clip unavailable", { status: 502 });
  const r = await fetch(dl);
  const type = r.headers.get("content-type") ?? "";
  if (!r.ok || !r.body || type.includes("json") || type.includes("html")) return new Response("clip unavailable", { status: 502 });
  // straight through to the player while a copy goes into the cache (no
  // holding the whole file in memory first)
  const [a, b] = r.body.tee();
  const headers = { "content-type": "video/mp4", "cache-control": `public, max-age=${CLIP_DAYS * 86400}`, "accept-ranges": "bytes" };
  const len = r.headers.get("content-length");
  const h2: Record<string, string> = len ? { ...headers, "content-length": len } : headers;
  cache.put(new Request(fileKey), new Response(b, { headers: h2 })).catch(() => {});
  return new Response(a, { headers: h2 });
}

/** Is ClipCafe answering couch's key? Never shows the key or a download link. */
async function clipStatus(env: Env, imdb: string, tryDownload = false, req?: Request): Promise<Response> {
  if (!env.CLIPCAFE_KEY) return json({ key: "missing" });
  const title = req && new URL(req.url).searchParams.get("title");
  const r = await clipSearch(env, title ? `movie_title=${encodeURIComponent(title)}&size=3` : `imdb=${imdb}&size=3`);
  const c = r.clips[0];
  let download: any = undefined;
  if (tryDownload && c) {
    const u = await clipDownloadUrl(env, { slug: String(c.slug), download: String(c.download ?? ""), duration: 0 });
    if (u) {
      const d = await fetch(u);
      download = { status: d.status, type: d.headers.get("content-type"), bytes: d.headers.get("content-length") };
      await d.body?.cancel();
    }
  }
  return json({
    key: "set",
    keyLength: env.CLIPCAFE_KEY.length,
    status: r.status,
    error: r.error,
    clips: r.clips.length,
    fields: c ? Object.keys(c).sort() : [],
    downloadLooksLike: c?.download ? (/^https?:/.test(String(c.download)) ? "url" : "key") : "none",
    sample: c ? { slug: c.slug, duration: c.duration, views: c.views, imdb: c.imdb, movie: c.movie_title, year: c.movie_year } : null,
    download,
  });
}

// A shared title: opens couch right on its page (or says how to get couch).
function titlePage(type: string, id: string, name: string) {
  const deep = `couch://title/${type}/${encodeURIComponent(id)}`;
  const esc = (s: string) => s.replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);
  const imdb = id.match(/^tt\d+/)?.[0];
  const art = imdb ? `https://images.metahub.space/background/medium/${imdb}/img` : "";
  const logoArt = imdb ? `https://images.metahub.space/logo/medium/${imdb}/img` : "";
  const nm = name.slice(0, 120);
  const html = `<!doctype html><meta charset=utf-8><meta name=viewport content="width=device-width,initial-scale=1">
<title>${esc(nm || "midnite™ couch")}</title>
<meta property="og:title" content="${esc(nm ? nm + " · on the couch" : "midnite™ couch")}">
${art ? `<meta property="og:image" content="${esc(art)}"><meta name="twitter:card" content="summary_large_image">` : ""}
<meta name="theme-color" content="#E0309A">
<style>
body{margin:0;min-height:100vh;display:grid;place-items:center;background:#000 center/cover no-repeat;color:#F4EAF0;font:16px ui-monospace,SFMono-Regular,Consolas,monospace;text-align:center}
body::before{content:"";position:fixed;inset:0;background:linear-gradient(transparent,#000 75%)}
div{position:relative;padding:24px}
.l{max-width:min(420px,80vw);max-height:140px;object-fit:contain}
h1{font:800 34px system-ui,sans-serif;letter-spacing:-.03em;margin:0}
a{display:inline-block;margin-top:26px;padding:13px 30px;border-radius:999px;background:#F4EAF0;color:#000;text-decoration:none;font:700 15px system-ui,sans-serif}
p{color:#9C8A96;margin:18px 0 0}
</style>
<body style="${art ? `background-image:url('${esc(art)}')` : ""}">
<div>${logoArt ? `<img class=l src="${esc(logoArt)}" alt="${esc(nm)}" onerror="this.outerHTML='<h1>${esc(esc(nm))}</h1>'">` : `<h1>${esc(nm)}</h1>`}
<br><a href="${esc(deep)}">open in couch</a><p>no couch yet? ask in the midnite server.</p></div>
<script>setTimeout(function(){location.href=${JSON.stringify(deep)}},350)</script>`;
  return new Response(html, { headers: { "content-type": "text/html; charset=utf-8", "cache-control": "public, max-age=300", "referrer-policy": "no-referrer" } });
}

// The invite link friends tap in Discord: it hands the crew to the app.
function joinPage(url: URL) {
  const code = url.searchParams.get("c") ?? "";
  const deep = `couch://crew?u=${encodeURIComponent(url.origin)}&c=${encodeURIComponent(code)}`;
  const esc = (s: string) => s.replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);
  const html = `<!doctype html><meta charset=utf-8><meta name=viewport content="width=device-width,initial-scale=1">
<title>midnite™ couch</title>
<style>
body{margin:0;min-height:100vh;display:grid;place-items:center;background:#0C070B;color:#F4EAF0;font:16px ui-monospace,SFMono-Regular,Consolas,monospace;text-align:center}
img{width:120px;height:120px;border-radius:28px;box-shadow:0 0 60px -10px #E0309A}
a{display:inline-block;margin-top:22px;padding:12px 26px;border-radius:999px;background:#F4EAF0;color:#0C070B;text-decoration:none;font:600 15px system-ui,sans-serif}
p{color:#9C8A96;margin:18px 24px 0}
</style>
<div><img src="/logo.png" alt=""><h1 style="font:800 26px system-ui,sans-serif;letter-spacing:-.03em;margin:22px 0 0">you're invited to the couch.</h1>
<a href="${esc(deep)}">open couch</a><p>no couch yet? ask in the server for the app.</p></div>
<script>setTimeout(function(){location.href=${JSON.stringify(deep)}},400)</script>`;
  return new Response(html, { headers: { "content-type": "text/html; charset=utf-8", "cache-control": "no-store", "referrer-policy": "no-referrer" } });
}

export class Crew {
  members = new Map<string, Member>();
  asks = new Map<string, Ask>();
  rooms = new Map<string, Room>();
  /** live lines per room: the host's socket and each guest's */
  socks = new Map<string, { host: WebSocket | null; guests: Map<WebSocket, string> }>();

  /** where the host is right now, by the server's clock */
  roomNow(r: Room) {
    return r.paused || r.stalled ? r.pos : r.pos + (Date.now() - r.at) / 1000;
  }
  /** tell every guest on this couch where the host is, at once */
  pushRoom(hostId: string, only?: WebSocket) {
    const r = this.rooms.get(hostId);
    const s = this.socks.get(hostId);
    if (!r || !s) return;
    const msg = JSON.stringify({ k: "s", v: r.videoId, m: r.metaId, ty: r.type, ti: r.title, sk: r.sk, p: this.roomNow(r), pa: r.paused, st: !!r.stalled, sh: !!r.shared });
    for (const ws of only ? [only] : s.guests.keys()) {
      try {
        ws.send(msg);
      } catch {
        s.guests.delete(ws);
      }
    }
  }
  /** a socket for watch together: host sends state, guests get it pushed */
  openLine(hostId: string, who: Who): Response {
    const r = this.rooms.get(hostId);
    const isHost = hostId === who.id;
    if (!isHost && !(r && r.guests.includes(who.id))) return json({ error: "not in this room" }, 403);
    const pair = new WebSocketPair();
    const [client, server] = [pair[0], pair[1]];
    server.accept();
    const s = this.socks.get(hostId) ?? { host: null, guests: new Map<WebSocket, string>() };
    this.socks.set(hostId, s);
    if (isHost) {
      try {
        s.host?.close(1000, "replaced");
      } catch {}
      s.host = server;
    } else {
      s.guests.set(server, who.id);
      this.pushRoom(hostId, server);
    }
    server.addEventListener("message", (ev: MessageEvent) => {
      let d: any;
      try {
        d = JSON.parse(typeof ev.data === "string" ? ev.data : "");
      } catch {
        return;
      }
      // clock check: the guest times the round trip to aim at the right moment
      if (d?.k === "ping") {
        try {
          server.send(JSON.stringify({ k: "pong", c: num(d.c) }));
        } catch {}
        return;
      }
      if (isHost && d?.k === "s") {
        const room = this.rooms.get(hostId);
        if (!room) return;
        Object.assign(room, {
          videoId: str(d.v, 200) ?? room.videoId,
          metaId: str(d.m, 100) ?? room.metaId,
          type: str(d.ty, 20) ?? room.type,
          title: str(d.ti, 200) ?? room.title,
          // which stream the host plays, so guests can play the same file
          sk: str(d.sk, 700) ?? room.sk,
          pos: num(d.p),
          paused: !!d.pa,
          stalled: !!d.st,
          shared: !!d.sh,
          // the host's own one-way delay, so "pos" lines up with the server clock
          at: Date.now() - Math.max(0, Math.min(1500, num(d.lag))),
          wsAt: Date.now(),
        });
        this.pushRoom(hostId);
        return;
      }
      // a guest with the remote: straight to the host's player
      if (!isHost && d?.k === "cmd") {
        const room = this.rooms.get(hostId);
        const action = d.a === "pause" || d.a === "play" || d.a === "seek" ? d.a : null;
        if (!room || !room.shared || !action) return;
        const c: Cmd = { from: who.id, name: who.name, action, pos: action === "seek" ? num(d.p) : undefined, at: Date.now() };
        if (s.host) {
          try {
            s.host.send(JSON.stringify({ k: "cmd", ...c }));
            return;
          } catch {}
        }
        room.cmds = [...(room.cmds ?? []).slice(-10), c];
      }
    });
    const gone = () => {
      if (isHost) {
        if (s.host === server) s.host = null;
      } else s.guests.delete(server);
      if (!s.host && !s.guests.size) this.socks.delete(hostId);
    };
    server.addEventListener("close", gone);
    server.addEventListener("error", gone);
    return new Response(null, { status: 101, webSocket: client });
  }
  /** a still of what each person's watching right now (JPEG data URL) */
  frames = new Map<string, { img: string; at: number }>();
  storage: DurableObjectStorage;
  reactN = 0;
  lastSeenWrite = new Map<string, number>();
  constructor(state: DurableObjectState, _env: Env) {
    this.storage = state.storage;
  }

  /** keep the people index (who has a page) current. accent "" = clear it */
  async touchPerson(who: Who, accent?: string) {
    const people = (await this.storage.get<Record<string, Person>>("people")) ?? {};
    const prev = people[who.id];
    people[who.id] = { id: who.id, name: who.name, avatar: who.avatar, accent: accent === undefined ? prev?.accent : accent || undefined, at: Date.now() };
    await this.storage.put("people", people);
  }

  /** the lasting part (kept in the Durable Object's storage): favourites,
   *  reviews and ratings, and the crew's activity feed */
  async social(req: Request, url: URL, b: any, who: Who): Promise<Response | null> {
    const p = url.pathname;
    const title = (t: any) => {
      const id = str(t?.id, 100);
      if (!id || !/^[A-Za-z0-9:._-]+$/.test(id)) return null;
      return { id, type: str(t?.type, 20) ?? "movie", name: str(t?.name, 200) ?? id, poster: str(t?.poster, 500), background: str(t?.background, 500), logo: str(t?.logo, 500) };
    };
    if (p === "/v1/favs") {
      const key = `favs:${who.id}`;
      if (req.method === "GET") return json({ favs: (await this.storage.get<Fav[]>(key)) ?? [] });
      if (req.method === "PUT") {
        const favs: Fav[] = [];
        for (const f of Array.isArray(b?.favs) ? b.favs.slice(0, 500) : []) {
          const t = title(f);
          if (t) favs.push({ id: t.id, type: t.type, name: t.name, poster: t.poster, at: num(f.at) || Date.now() });
        }
        await this.storage.put(key, favs);
        return json({ ok: true });
      }
    }
    // ---- watch parties
    if (p === "/v1/parties" && req.method === "GET") {
      const now = Date.now();
      const all = ((await this.storage.get<Party[]>("parties")) ?? []).filter((x) => x.at > now - 4 * 3600_000);
      const mine = all.filter((x) => x.host === who.id || x.invited === "all" || x.invited.includes(who.id));
      return json({ parties: mine.sort((a, b) => a.at - b.at) });
    }
    if (p === "/v1/party" && req.method === "POST") {
      const t = title(b?.title);
      const at = num(b?.at);
      if (!t || at < Date.now() - 5 * 60_000 || at > Date.now() + 90 * 86400_000) return json({ error: "pick a title and a time" }, 400);
      const invited = b?.invite === "all" ? "all" : (Array.isArray(b?.invite) ? b.invite : []).filter((x: unknown) => typeof x === "string" && /^[0-9]{5,25}$/.test(x)).slice(0, 50);
      const party: Party = {
        id: Math.random().toString(36).slice(2, 10),
        host: who.id,
        hostName: who.name,
        hostAvatar: who.avatar,
        title: { ...t, year: str(b?.title?.year, 10), runtime: str(b?.title?.runtime, 20) },
        videoId: str(b?.videoId, 200),
        at,
        note: str(b?.note, 140),
        invited,
        rsvp: { [who.id]: { yes: true, name: who.name, avatar: who.avatar } },
        created: Date.now(),
      };
      const now = Date.now();
      const list = ((await this.storage.get<Party[]>("parties")) ?? []).filter((x) => x.at > now - 4 * 3600_000).slice(-99);
      list.push(party);
      await this.storage.put("parties", list);
      return json({ party });
    }
    const pm2 = p.match(/^\/v1\/party\/([a-z0-9]{4,16})(\/rsvp)?$/);
    if (pm2) {
      const list = (await this.storage.get<Party[]>("parties")) ?? [];
      const party = list.find((x) => x.id === pm2[1]);
      if (!party) return json({ error: "that party's over" }, 404);
      if (pm2[2] && req.method === "POST") {
        if (party.invited !== "all" && !party.invited.includes(who.id) && party.host !== who.id) return json({ error: "not invited" }, 403);
        party.rsvp[who.id] = { yes: !!b?.yes, name: who.name, avatar: who.avatar };
        await this.storage.put("parties", list);
        return json({ party });
      }
      if (!pm2[2] && req.method === "DELETE") {
        if (party.host !== who.id) return json({ error: "only the host can call it off" }, 403);
        await this.storage.put("parties", list.filter((x) => x.id !== party.id));
        return json({ ok: true });
      }
    }

    // ---- profiles: everyone's page on the couch
    if (p === "/v1/profile" && req.method === "PUT") {
      const items = (a: unknown, max: number): PItem[] => {
        const out: PItem[] = [];
        for (const x of Array.isArray(a) ? a.slice(0, max) : []) {
          const t = title(x);
          if (!t) continue;
          const r = num((x as any).rating);
          const e = Array.isArray((x as any).eps) ? [num((x as any).eps[0]), num((x as any).eps[1])] as [number, number] : undefined;
          out.push({ ...t, rating: r >= 1 && r <= 10 ? Math.round(r) : undefined, at: num((x as any).at) || undefined, eps: e });
        }
        return out;
      };
      const stats: Record<string, number> = {};
      for (const k of ["movies", "shows", "episodes", "watching", "plan"]) stats[k] = Math.max(0, Math.round(num(b?.stats?.[k])));
      const follows: Follow[] = [];
      for (const f of Array.isArray(b?.follows) ? b.follows.slice(0, 60) : []) {
        const kind = f?.kind === "studio" ? "studio" : f?.kind === "person" ? "person" : null;
        const fid = num(f?.id);
        if (kind && fid > 0) follows.push({ kind, id: Math.round(fid), name: str(f?.name, 120) ?? "", img: str(f?.img, 500) });
      }
      const t = b?.taste;
      const tIds = (Array.isArray(t?.ids) ? t.ids : []).filter((x: unknown) => typeof x === "string" && /^tt\d{5,10}$/.test(x)).slice(0, 1000) as string[];
      const tRatings: Record<string, number> = {};
      for (const [k, v] of Object.entries(t?.ratings && typeof t.ratings === "object" ? t.ratings : {}).slice(0, 1000)) {
        const n = num(v);
        if (/^tt\d{5,10}$/.test(k) && n >= 1 && n <= 10) tRatings[k] = Math.round(n);
      }
      const taste: Taste = { ids: tIds, ratings: tRatings, loved: items(t?.loved, 60) };
      const prof: Profile = { watching: items(b?.watching, 30), recent: items(b?.recent, 30), plan: items(b?.plan, 30), follows, taste, stats, source: b?.source === "simkl" ? "simkl" : "couch", at: Date.now() };
      await this.storage.put(`profile:${who.id}`, prof);
      await this.touchPerson(who);
      return json({ ok: true });
    }
    if (p === "/v1/profile/style" && req.method === "PUT") {
      const accent = typeof b?.accent === "string" && /^#[0-9a-fA-F]{6}$/.test(b.accent) ? b.accent.toLowerCase() : undefined;
      const style: Style = { accent, glow: b?.glow !== false };
      await this.storage.put(`style:${who.id}`, style);
      await this.touchPerson(who, accent ?? "");
      return json({ ok: true, style });
    }
    if (p === "/v1/people" && req.method === "GET") {
      const people = Object.values((await this.storage.get<Record<string, Person>>("people")) ?? {}).sort((a, b) => b.at - a.at);
      return json({ people });
    }
    const pm = p.match(/^\/v1\/profile\/([0-9]{5,25})$/);
    if (pm && req.method === "GET") {
      const id = pm[1];
      const [prof, style, favs, people, feed] = await Promise.all([
        this.storage.get<Profile>(`profile:${id}`),
        this.storage.get<Style>(`style:${id}`),
        this.storage.get<Fav[]>(`favs:${id}`),
        this.storage.get<Record<string, Person>>("people"),
        this.storage.get<Act[]>("feed"),
      ]);
      const person = people?.[id];
      return json({
        id,
        name: person?.name,
        avatar: person?.avatar,
        profile: prof ?? null,
        style: style ?? {},
        favs: (favs ?? []).slice(0, 30),
        activity: (feed ?? []).filter((a) => a.who === id).slice(0, 20),
      });
    }

    if (p === "/v1/activity") {
      if (req.method === "GET") return json({ feed: ((await this.storage.get<Act[]>("feed")) ?? []).slice(0, 60) });
      if (req.method === "POST") {
        const t = title(b?.title);
        const kind = b?.kind === "review" || b?.kind === "rating" || b?.kind === "finished" ? b.kind : null;
        if (!t || !kind) return json({ error: "bad activity" }, 400);
        const rating = Math.max(0, Math.min(10, Math.round(num(b?.rating)))) || undefined;
        const text = str(b?.text, 1200)?.trim() || undefined;
        if (kind === "review" && !text && !rating) return json({ error: "empty review" }, 400);
        const a: Act = { id: crypto.randomUUID(), who: who.id, name: who.name, avatar: who.avatar, kind: text ? "review" : kind, title: t, rating, text, at: Date.now() };
        let feed = (await this.storage.get<Act[]>("feed")) ?? [];
        // one line per person per title: a newer word replaces the older one
        // (a bare "finished" doesn't replace a review)
        const old = feed.find((x) => x.who === who.id && x.title.id === t.id);
        if (old && kind === "finished" && old.kind !== "finished") return json({ ok: true });
        feed = feed.filter((x) => !(x.who === who.id && x.title.id === t.id));
        if (kind === "finished" && old?.rating) a.rating = old.rating;
        feed.unshift(a);
        await this.storage.put("feed", feed.slice(0, FEED_MAX));
        if (a.kind !== "finished") {
          const rk = `rev:${t.id}`;
          const revs = ((await this.storage.get<Act[]>(rk)) ?? []).filter((x) => x.who !== who.id);
          revs.unshift(a);
          await this.storage.put(rk, revs.slice(0, 200));
        }
        return json({ ok: true, id: a.id });
      }
    }
    const m = p.match(/^\/v1\/reviews\/([A-Za-z0-9:._-]{1,100})$/);
    if (m) {
      const rk = `rev:${m[1]}`;
      if (req.method === "GET") return json({ reviews: (await this.storage.get<Act[]>(rk)) ?? [] });
      if (req.method === "DELETE") {
        await this.storage.put(rk, ((await this.storage.get<Act[]>(rk)) ?? []).filter((x) => x.who !== who.id));
        const feed = (await this.storage.get<Act[]>("feed")) ?? [];
        await this.storage.put("feed", feed.filter((x) => !(x.who === who.id && x.title.id === m[1] && x.kind !== "finished")));
        return json({ ok: true });
      }
    }
    return null;
  }

  sweep() {
    const now = Date.now();
    for (const [k, m] of this.members) if (now - m.at > STALE_MS) this.members.delete(k);
    for (const [k, a] of this.asks) if (now - a.at > ASK_TTL) this.asks.delete(k);
    for (const [k, r] of this.rooms) if (now - r.at > ROOM_TTL) this.rooms.delete(k);
    for (const k of this.frames.keys()) if (!this.members.get(k)?.watching) this.frames.delete(k);
  }

  /** watch together: asking, answering, and the host's playback for guests */
  together(req: Request, url: URL, b: any, who: Who): Response | null {
    const okId = (v: unknown) => {
      const id = str(v, 40);
      return id && /^[A-Za-z0-9_-]+$/.test(id) ? id : null;
    };
    const p = url.pathname;
    // ask to join someone's couch
    if (req.method === "POST" && p === "/v1/ask") {
      const from = who.id, to = okId(b?.to);
      if (!from || !to || from === to) return json({ error: "bad ask" }, 400);
      // you don't have to be watching anything yourself to ask
      const me = this.members.get(from) ?? { name: who.name, avatar: who.avatar };
      const them = this.members.get(to);
      if (!them?.watching) return json({ error: "they're not watching anything right now" }, 409);
      for (const a of this.asks.values()) if (a.from === from && a.to === to && a.status === "pending") return json({ id: a.id });
      const id = crypto.randomUUID();
      this.asks.set(id, { id, from, fromName: me.name, fromAvatar: me.avatar, to, at: Date.now(), status: "pending" });
      return json({ id });
    }
    let m = p.match(/^\/v1\/inbox\/([A-Za-z0-9_-]{1,40})$/);
    if (req.method === "GET" && m) {
      const asks = [...this.asks.values()].filter((a) => a.to === who.id && a.status === "pending");
      return json({ asks });
    }
    m = p.match(/^\/v1\/ask\/([0-9a-f-]{36})$/);
    if (m) {
      const a = this.asks.get(m[1]);
      if (!a) return json({ status: "gone" });
      if (req.method === "GET") return json({ status: a.status });
      // the asker changed their mind: it's gone from the host's screen too
      if (req.method === "DELETE") {
        if (who.id !== a.from) return json({ error: "not yours" }, 403);
        this.asks.delete(a.id);
        return json({ ok: true });
      }
      if (req.method === "POST") {
        if (who.id !== a.to) return json({ error: "not yours" }, 403);
        a.status = b?.accept ? "accepted" : "declined";
        a.at = Date.now();
        if (a.status === "accepted") {
          const host = this.members.get(a.to);
          const r = this.rooms.get(a.to) ?? { host: a.to, metaId: str(host?.watching?.metaId, 100) ?? "", type: str(host?.watching?.type, 20) ?? "", videoId: "", pos: host?.pos ?? 0, paused: !!host?.paused, at: Date.now(), guests: [] };
          if (!r.guests.includes(a.from)) r.guests.push(a.from);
          this.rooms.set(a.to, r);
        }
        return json({ ok: true });
      }
    }
    // the host's player reports here about once a second while it has guests
    if (req.method === "PUT" && p === "/v1/room") {
      const r = this.rooms.get(who.id);
      if (!r || !r.guests.length) return json({ error: "no room" }, 404);
      // the live line is newer and exact: the 1/s report only fills in when it's quiet
      const live = r.wsAt && Date.now() - r.wsAt < 4000;
      if (!live) Object.assign(r, {
        metaId: str(b.metaId, 100) ?? r.metaId,
        type: str(b.type, 20) ?? r.type,
        videoId: str(b.videoId, 200) ?? r.videoId,
        title: str(b.title, 200),
        pos: num(b.pos),
        paused: !!b.paused,
        shared: !!b.shared,
        stalled: !!b.stalled,
        at: Date.now() - Math.max(0, Math.min(1500, num(b.lag))),
      });
      if (!live) this.pushRoom(who.id);
      // hand the host whatever the guests asked for since last time
      const cmds = r.cmds ?? [];
      r.cmds = [];
      return json({ guests: r.guests.map((g) => this.members.get(g)?.name ?? "someone"), cmds });
    }
    // a guest steering, when the host lets everyone control
    m = p.match(/^\/v1\/room\/([A-Za-z0-9_-]{1,40})\/cmd$/);
    if (m && req.method === "POST") {
      const r = this.rooms.get(m[1]);
      if (!r || !r.guests.includes(who.id)) return json({ error: "not in this room" }, 403);
      if (!r.shared) return json({ error: "the host has the remote" }, 403);
      const action = b?.action === "pause" || b?.action === "play" || b?.action === "seek" ? b.action : null;
      if (!action) return json({ error: "bad command" }, 400);
      r.cmds = [...(r.cmds ?? []).slice(-10), { from: who.id, name: who.name, action, pos: action === "seek" ? num(b.pos) : undefined, at: Date.now() }];
      return json({ ok: true });
    }
    // reactions in watch together: anyone in the room sends, everyone polls
    m = p.match(/^\/v1\/react\/([A-Za-z0-9_-]{1,40})$/);
    if (m) {
      const r = this.rooms.get(m[1]);
      const inIt = r && (r.host === who.id || r.guests.includes(who.id));
      if (!r || !inIt) return json({ reactions: [] });
      const now = Date.now();
      r.reacts = (r.reacts ?? []).filter((x) => now - x.at < 20_000);
      if (req.method === "POST") {
        const emoji = REACTIONS.includes(b?.emoji) ? b.emoji : null;
        if (!emoji) return json({ error: "bad reaction" }, 400);
        const mine = r.reacts.filter((x) => x.who === who.id && now - x.at < 3000).length;
        if (mine >= 20) return json({ ok: true }); // easy there
        r.reacts.push({ n: ++this.reactN, who: who.id, name: who.name, avatar: who.avatar, emoji, at: now });
        return json({ ok: true });
      }
      const since = Number(url.searchParams.get("since") ?? 0) || 0;
      return json({ reactions: r.reacts.filter((x) => x.n > since), last: this.reactN });
    }
    m = p.match(/^\/v1\/room\/([A-Za-z0-9_-]{1,40})(?:\/([A-Za-z0-9_-]{1,40}))?$/);
    if (m) {
      const r = this.rooms.get(m[1]);
      if (req.method === "GET") {
        if (!r || !r.videoId) return json({ room: null });
        // where the host is right now, by the server's clock
        return json({ room: { ...r, now: this.roomNow(r) } });
      }
      if (req.method === "DELETE") {
        // a guest leaves, or the host ends it
        if (r && m[2] && m[2] === who.id) r.guests = r.guests.filter((g) => g !== who.id);
        else if (m[1] === who.id) this.rooms.delete(m[1]);
        return json({ ok: true });
      }
    }
    return null;
  }

  async fetch(req: Request): Promise<Response> {
    const url = new URL(req.url);
    this.sweep();
    let who: Who;
    try {
      who = JSON.parse(req.headers.get("x-who") ?? "");
    } catch {
      return json({ error: "sign in again" }, 401);
    }

    const line = url.pathname.match(/^\/v1\/room\/([A-Za-z0-9_-]{1,40})\/ws$/);
    if (line && req.headers.get("upgrade") === "websocket") return this.openLine(line[1], who);

    if (req.method === "GET" && url.pathname === "/v1/presence") {
      const members = [...this.members.values()].sort((a, b) => b.at - a.at).map((m) => ({ ...m, frameAt: this.frames.get(m.id)?.at }));
      return json({ members });
    }
    const fm = url.pathname.match(/^\/v1\/frame(?:\/([A-Za-z0-9_-]{1,40}))?$/);
    if (fm && req.method === "GET" && fm[1]) {
      return json({ img: this.frames.get(fm[1])?.img ?? null });
    }
    if (fm && req.method === "PUT" && !fm[1]) {
      let b: any;
      try {
        b = await req.json();
      } catch {
        return json({ error: "bad json" }, 400);
      }
      const img = str(b?.img, 200_000);
      if (!img || !img.startsWith("data:image/jpeg;base64,")) return json({ error: "bad frame" }, 400);
      if (!this.members.get(who.id)?.watching) return json({ ok: false });
      this.frames.set(who.id, { img, at: Date.now() });
      return json({ ok: true });
    }

    if (req.method === "PUT" && url.pathname === "/v1/presence") {
      let b: any;
      try {
        b = await req.json();
      } catch {
        return json({ error: "bad json" }, 400);
      }
      const id = who.id;
      if (!this.members.has(id) && this.members.size >= MAX_MEMBERS) return json({ error: "couch is full" }, 429);
      // anyone who's been on the couch stays in the crew list, online or not
      if ((this.lastSeenWrite.get(id) ?? 0) < Date.now() - 3_600_000) {
        this.lastSeenWrite.set(id, Date.now());
        await this.touchPerson(who);
      }
      const w = b.watching && typeof b.watching === "object" ? b.watching : null;
      this.members.set(id, {
        id,
        name: who.name,
        avatar: who.avatar,
        watching: w && {
          title: str(w.title, 200),
          year: typeof w.year === "number" ? w.year : undefined,
          poster: str(w.poster, 500),
          background: str(w.background, 500),
          episode: str(w.episode, 200),
          link: str(w.link, 500),
          metaId: str(w.metaId, 100),
          type: str(w.type, 20),
        },
        paused: !!b.paused,
        pos: num(b.pos),
        duration: num(b.duration),
        at: Date.now(),
      });
      return json({ ok: true });
    }

    const m = url.pathname.match(/^\/v1\/presence\/([A-Za-z0-9_-]{1,40})$/);
    if (req.method === "DELETE" && m) {
      if (m[1] !== who.id) return json({ error: "not yours" }, 403);
      this.members.delete(m[1]);
      this.rooms.delete(m[1]);
      this.frames.delete(m[1]);
      // stopped watching with people still asking: that's a no
      for (const a of this.asks.values()) if (a.to === m[1] && a.status === "pending") {
        a.status = "ended";
        a.at = Date.now();
      }
      return json({ ok: true });
    }

    let body: any = null;
    if (req.method === "POST" || req.method === "PUT") {
      try {
        body = await req.json();
      } catch {
        return json({ error: "bad json" }, 400);
      }
    }
    const t = this.together(req, url, body, who);
    if (t) return t;
    const s2 = await this.social(req, url, body, who);
    if (s2) return s2;

    return json({ error: "not here" }, 404);
  }
}
