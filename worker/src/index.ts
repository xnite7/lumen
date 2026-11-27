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
import { PRIVACY_HTML } from "./privacy";
import { appStorePro } from "./appstore";
import { Game } from "./game";
import { DOWNLOAD_HTML } from "./download";
import { INSTALL_HTML } from "./install";
import { LANDING_HTML, INSTALL_PAGE, LANDING_FILES } from "./landing/landing";

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
  /** access code for App Review's demo login (no Discord needed). Unset = off */
  REVIEW_CODE?: string;
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

// ------------------------------------------------------------- demo source
const DEMO: Record<string, { name: string; url: string; size: number }> = {
  tt1254207: { name: "Big Buck Bunny", url: "https://archive.org/download/BigBuckBunny_124/Content/big_buck_bunny_720p_surround.mp4", size: 61878609 },
  tt1727587: { name: "Sintel", url: "https://archive.org/download/Sintel/sintel-2048-surround.mp4", size: 310674005 },
};

function demoSource(url: URL): Response {
  const cors = { "access-control-allow-origin": "*", "cache-control": "public, max-age=3600" };
  const path = url.pathname.replace(/^\/demo/, "");
  if (path === "" || path === "/" || path === "/manifest.json") {
    return json(
      {
        id: "app.midnite.couch.demo",
        version: "1.0.0",
        name: "couch demo",
        description: "Open movies from the Blender Foundation (Creative Commons): Big Buck Bunny and Sintel.",
        logo: `${url.origin}/couch.png`,
        types: ["movie"],
        idPrefixes: ["tt"],
        resources: ["stream"],
        catalogs: [],
      },
      200,
      cors,
    );
  }
  const m = path.match(/^\/stream\/movie\/(tt\d+)\.json$/);
  if (m) {
    const d = DEMO[m[1]];
    return json({ streams: d ? [{ name: "couch demo\n720p", title: `${d.name} (Blender Foundation, CC BY)`, url: d.url, behaviorHints: { videoSize: d.size, filename: d.url.split("/").pop() } }] : [] }, 200, cors);
  }
  return json({ streams: [] }, 200, cors);
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
  const rels = await gh(env, "/releases?per_page=100");
  if (!rels.ok) return json({ error: "no builds yet" }, 404);
  const list: any[] = await rels.json();
  // GitHub lists tags as text (gpui-0.9.99 before gpui-0.9.106) and drafts
  // first: pick the highest published version that has its zip
  const ver = (t: string) => t.slice(5).split(".").map((n) => parseInt(n, 10) || 0);
  const newer = (a: number[], b: number[]) => {
    for (let i = 0; i < Math.max(a.length, b.length); i++) {
      if ((a[i] ?? 0) !== (b[i] ?? 0)) return (a[i] ?? 0) > (b[i] ?? 0);
    }
    return false;
  };
  let rel: any = null;
  for (const r of list) {
    const tag = String(r.tag_name);
    if (r.draft || !tag.startsWith("gpui-") || !r.assets?.some((a: any) => a.name === "couch-gpui.zip")) continue;
    if (!rel || newer(ver(tag), ver(String(rel.tag_name)))) rel = r;
  }
  const zip = rel?.assets?.find((a: any) => a.name === "couch-gpui.zip");
  if (!rel || !zip) return json({ error: "no builds yet" }, 404);
  if (url.pathname === "/v1/gpui/file") {
    const bin = await asset(env, zip.id);
    return new Response(bin.body, { headers: { "content-type": "application/zip", "content-length": String(zip.size) } });
  }
  return json({ version: String(rel.tag_name).slice(5), size: zip.size, notes: rel.body ?? "" });
}

/** pro: clip previews etc. crew: the midnite social side. Missing = true
 *  (sessions from the old gate: everyone there had the role). */
type Who = { id: string; name: string; avatar?: string; pro?: boolean; crew?: boolean };
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
    return { id: String(v.id), name: String(v.name ?? "someone"), avatar: v.avatar || undefined, pro: v.pro, crew: v.crew };
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
  // App Review's demo login: an access code instead of Discord
  if (typeof b?.code === "string") {
    const want = env.REVIEW_CODE ?? "";
    const got = b.code.trim();
    const enc = new TextEncoder();
    const [x, y] = await Promise.all([crypto.subtle.digest("SHA-256", enc.encode(got)), crypto.subtle.digest("SHA-256", enc.encode(want))]);
    const same = new Uint8Array(x).every((v, i) => v === new Uint8Array(y)[i]);
    if (!want || !same) return json({ error: "that code didn't work" }, 403);
    const who: Who = { id: "app-review", name: "App Review" };
    return json({ session: await sign(env, who), exp: Math.floor(Date.now() / 1000) + SESSION_TTL, id: who.id, name: who.name });
  }
  // no account: a guest session for this device (plays the scenes game;
  // not on the leaderboards, no Pro, no crew)
  if (typeof b?.guest === "string" && /^[A-Za-z0-9]{12,40}$/.test(b.guest)) {
    const who: Who = { id: `g-${b.guest}`, name: "guest", pro: false, crew: false };
    return json({ session: await sign(env, who), exp: Math.floor(Date.now() / 1000) + SESSION_TTL, id: who.id, pro: false, crew: false });
  }
  // couch Pro bought in the App Store: Apple's signed transaction
  if (typeof b?.appstore === "string") {
    const t = await appStorePro(b.appstore);
    if (!t) return json({ error: "couldn't check that purchase" }, 403);
    const who: Who = { id: `as-${t}`, name: "couch pro", pro: true, crew: false };
    return json({ session: await sign(env, who), exp: Math.floor(Date.now() / 1000) + SESSION_TTL, id: who.id, pro: true, crew: false });
  }
  const token = typeof b?.token === "string" ? b.token : "";
  if (!token) return json({ error: "no token" }, 400);
  // the App Store app: anyone can sign in; the server role decides the tier
  // instead of locking the door
  const open = b?.open === true;
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
      if (br.status === 404) return deny(open, "not in the server");
      if (!br.ok) return json({ error: "couldn't check your roles" }, 503);
      const mem: any = await br.json();
      if (!(mem?.roles ?? []).some((r: string) => roles.includes(r))) return deny(open, "no subscription");
    } else {
      const g = await fetch("https://discord.com/api/users/@me/guilds", { headers: h });
      if (!g.ok) return json({ error: "couldn't check your servers" }, 503);
      const list: any = await g.json();
      if (!Array.isArray(list)) return json({ error: "couldn't check your servers" }, 503);
      if (!list.some((x: any) => x?.id === env.GUILD_ID)) return deny(open, "not in the server");
    }
    if (roles.length && !env.DISCORD_BOT_TOKEN) {
      const mr = await fetch(`https://discord.com/api/users/@me/guilds/${env.GUILD_ID}/member`, { headers: h });
      // no permission to see roles = an old sign-in (or an old app): no way in
      // until they sign in again; new apps show "one more knock", old ones lock
      if (mr.status === 401 || mr.status === 403) return open ? welcome(false) : json({ error: "sign in again (couch needs to see your server roles)", reauth: true, notMember: true }, 403);
      if (!mr.ok) return json({ error: "couldn't check your roles" }, 503);
      const mem: any = await mr.json();
      const has = (mem?.roles ?? []).some((r: string) => roles.includes(r));
      if (!has) return deny(open, "no subscription");
    }
  }
  return welcome(true);

  // signed in, with or without the role (open mode only for the latter)
  async function welcome(coucher: boolean) {
    const who: Who = {
      id: String(u.id),
      name: String(u.global_name || u.username || "someone").slice(0, 40),
      avatar: u.avatar ? `https://cdn.discordapp.com/avatars/${u.id}/${u.avatar}.png?size=128` : undefined,
    };
    if (open) {
      who.pro = coucher;
      who.crew = coucher;
    }
    return json({ session: await sign(env, who), exp: Math.floor(Date.now() / 1000) + SESSION_TTL, id: who.id, name: who.name, avatar: who.avatar, pro: coucher, crew: coucher, coucher });
  }
  async function deny(open: boolean, why: string) {
    if (open) return welcome(false);
    return json({ error: why, notMember: true }, 403);
  }
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
type Ask = { id: string; from: string; fromName: string; fromAvatar?: string; to: string; at: number; status: "pending" | "accepted" | "declined" | "ended"; metaId?: string; title?: string };
/** a host's playback, followed by the guests they let in */
type Room = { host: string; metaId: string; type: string; videoId: string; title?: string; sk?: string; pos: number; paused: boolean; at: number; guests: string[]; reacts?: React[]; shared?: boolean; cmds?: Cmd[]; stalled?: boolean; wsAt?: number; remote?: string[]; buf?: Record<string, number>; du?: number };
/** a guest steering the host's player (only when the host shares control) */
type Cmd = { from: string; name: string; action: "pause" | "play" | "seek"; pos?: number; at: number };
type React = { n: number; who: string; name: string; avatar?: string; emoji: string; text?: string; at: number };
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
/** one emoji (people pick their own): a short run of emoji characters, nothing else */
const isEmoji = (e: unknown): e is string =>
  typeof e === "string" && e.length > 0 && e.length <= 16 && /^(?:\p{Extended_Pictographic}|\p{Emoji_Component}|\u200D|\uFE0F|\p{Regional_Indicator})+$/u.test(e) && /\p{Extended_Pictographic}|\p{Regional_Indicator}/u.test(e);
const ASK_TTL = 120_000;
const ROOM_TTL = 90_000;

const json = (v: unknown, status = 200, headers: Record<string, string> = {}) =>
  new Response(JSON.stringify(v), { status, headers: { "content-type": "application/json", "cache-control": "no-store", ...headers } });

function safeEqual(a: string, b: string) {
  if (!a || !b || a.length !== b.length) return false;
  let d = 0;
  for (let i = 0; i < a.length; i++) d |= a.charCodeAt(i) ^ b.charCodeAt(i);
  return d === 0;
}

const str = (v: unknown, max: number) => (typeof v === "string" ? v.slice(0, max) : undefined);
const num = (v: unknown) => (typeof v === "number" && isFinite(v) ? v : 0);

let INVITE: { url: string; at: number } | null = null;
/** A permanent invite to the crew's server, made once by the bot and reused. */
async function serverInvite(env: Env): Promise<Response> {
  if (INVITE && Date.now() - INVITE.at < 6 * 3600_000) return json({ url: INVITE.url });
  if (!env.GUILD_ID || !env.DISCORD_BOT_TOKEN) return json({ url: null });
  const h = { authorization: `Bot ${env.DISCORD_BOT_TOKEN}`, "content-type": "application/json" };
  try {
    const g: any = await (await fetch(`https://discord.com/api/guilds/${env.GUILD_ID}`, { headers: h })).json();
    let ch: string | undefined = g?.rules_channel_id || g?.system_channel_id;
    if (!ch) {
      const list: any = await (await fetch(`https://discord.com/api/guilds/${env.GUILD_ID}/channels`, { headers: h })).json();
      ch = (Array.isArray(list) ? list : []).filter((c: any) => c?.type === 0).sort((a: any, b: any) => (a.position ?? 0) - (b.position ?? 0))[0]?.id;
    }
    if (!ch) return json({ url: null });
    // unique:false hands back the same permanent invite every time
    const r = await fetch(`https://discord.com/api/channels/${ch}/invites`, { method: "POST", headers: h, body: JSON.stringify({ max_age: 0, max_uses: 0, unique: false }) });
    const inv: any = await r.json();
    if (!r.ok || !inv?.code) return json({ url: null, error: inv?.message ?? "the bot can't make invites" });
    INVITE = { url: `https://discord.gg/${inv.code}`, at: Date.now() };
    return json({ url: INVITE.url });
  } catch {
    return json({ url: null });
  }
}

export default {
  async fetch(req: Request, env: Env): Promise<Response> {
    const url = new URL(req.url);
    // the couch mark: the favicon and every logo on the site. /logo.png is the old name
    // (and Discord's small image); /couch.png is the one the pages use, so browsers
    // that cached an older mark under /logo.png still get the right one.
    if (url.pathname === "/couch.png" || url.pathname === "/logo.png" || url.pathname === "/favicon.ico" || url.pathname === "/apple-touch-icon.png") {
      return new Response(logo, { headers: { "content-type": "image/png", "cache-control": "public, max-age=3600" } });
    }
    if (url.pathname === "/" || url.pathname === "/download" || url.pathname === "/couch") {
      return new Response(LANDING_HTML, { headers: { "content-type": "text/html; charset=utf-8", "cache-control": "public, max-age=600" } });
    }
    const lf = url.pathname.match(/^\/l\/([\w.-]+)$/);
    if (lf && LANDING_FILES[lf[1]]) {
      const [data, type] = LANDING_FILES[lf[1]];
      return new Response(data, { headers: { "content-type": type, "cache-control": "public, max-age=604800, immutable" } });
    }
    if (url.pathname === "/old") {
      return new Response(DOWNLOAD_HTML, { headers: { "content-type": "text/html; charset=utf-8", "cache-control": "public, max-age=600" } });
    }
    if (url.pathname === "/install" || url.pathname === "/help" || url.pathname === "/ios") {
      return new Response(INSTALL_PAGE, { headers: { "content-type": "text/html; charset=utf-8", "cache-control": "public, max-age=600" } });
    }
    if (url.pathname === "/join") return joinPage(url);
    // a demo source for the App Store app: open movies anyone may stream
    // (Blender Foundation, Creative Commons). Shows how "Add a source" works.
    if (url.pathname.startsWith("/demo")) return demoSource(url);
    if (url.pathname === "/privacy") {
      return new Response(PRIVACY_HTML, { headers: { "content-type": "text/html; charset=utf-8", "cache-control": "public, max-age=600" } });
    }
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
      if (w.pro === false) return new Response("couch pro", { status: 402 });
      return clipStream(env, clipFile[1], req, url.searchParams.get("title"), url.searchParams.get("year"));
    }
    // a game round's scene: asked for by game and round, so the address
    // doesn't say which film it is
    const gameClip = url.pathname.match(/^\/game\/clip\/([a-f0-9]{20})\/(\d{1,3})$/);
    if (gameClip) {
      const w = await verify(env, `Bearer ${url.searchParams.get("t") ?? ""}`);
      if (!w) return new Response("sign in again", { status: 401 });
      const stub = env.CREW.get(env.CREW.idFromName("midnite"));
      const r = await stub.fetch(new Request(`https://couch.internal/internal/game-clip?g=${gameClip[1]}&n=${gameClip[2]}&u=${encodeURIComponent(w.id)}`));
      if (!r.ok) return new Response("that game is over", { status: 404 });
      const { id } = (await r.json()) as { id: string };
      const out = await clipStream(env, id, req);
      const h = new Headers(out.headers);
      h.set("cache-control", "private, no-store"); // the address is per game
      return new Response(out.body, { status: out.status, headers: h });
    }
    const clipCheck = url.pathname.match(/^\/clip-status\/(tt\d{5,10})$/);
    if (clipCheck) return clipStatus(env, clipCheck[1], false, req); // download check verified; off so nobody burns the monthly downloads
    // Discord pictures: a title's animated "on the big screen" card, made by
    // couch and kept here so Discord can show it (public, like the logo)
    const artFile = url.pathname.match(/^\/art\/([A-Za-z0-9_.-]{1,80})\.gif$/);
    if (artFile) {
      // kept in the Durable Object (the Cache API does nothing on workers.dev)
      const fwd = new Request(req);
      fwd.headers.delete("x-who");
      return env.CREW.get(env.CREW.idFromName("midnite")).fetch(fwd);
    }
    // which country the app is in (for "Top 10 in UAE"); just the 2-letter code
    // the server invite (permanent, made by couch's bot): for people who
    // aren't in the server yet, so it's public
    if (url.pathname === "/v1/invite") return serverInvite(env);
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
    if (clipAsk) {
      if (who.pro === false) return json({ clip: false, pro: true }, 402);
      // already fetched once: it plays from the cache, no request needed
      if (await caches.default.match(new Request(`https://couch.clips/file2/${clipAsk[1]}.mp4`))) return json({ clip: true });
      // out of ClipCafe requests today: no clip (the app plays a trailer)
      if (await clipsOut()) return json({ clip: false, out: true });
      return json({ clip: !!(await clipPick(env, clipAsk[1], url.searchParams.get("title"), url.searchParams.get("year"))) });
    }
    if (url.pathname.startsWith("/v1/update/")) return updates(url, env);
    if (url.pathname === "/v1/gpui/latest" || url.pathname === "/v1/gpui/file") return gpui(url, env);
    // your own devices work for anyone signed in; the crew (presence, rooms,
    // reviews…) is for the server's couchers
    if (who.crew === false && !url.pathname.startsWith("/v1/me/") && !url.pathname.startsWith("/v1/game/")) return json({ error: "for couchers", crew: false }, 403);
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
  const error = d?.error ? String(d.error).slice(0, 200) : undefined;
  // out of requests for today (the free plan's 100): say so until midnight UTC,
  // so the apps go straight to a trailer instead of waiting on a clip
  if (r.status === 429 || r.status === 402 || r.status === 403 || /limit|quota|exceed/i.test(error ?? "")) await clipsOutToday();
  return { status: r.status, error, clips };
}

const CLIPS_OUT = "https://couch.clips/out";
async function clipsOutToday() {
  const now = new Date();
  const midnight = Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), now.getUTCDate() + 1);
  const secs = Math.max(60, Math.floor((midnight - now.getTime()) / 1000));
  await caches.default.put(new Request(CLIPS_OUT), new Response("1", { headers: { "cache-control": `public, max-age=${secs}` } }));
}
async function clipsOut(): Promise<boolean> {
  return !!(await caches.default.match(new Request(CLIPS_OUT)));
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
<div><img src="/couch.png" alt=""><h1 style="font:800 26px system-ui,sans-serif;letter-spacing:-.03em;margin:22px 0 0">you're invited to the couch.</h1>
<a href="${esc(deep)}">open couch</a><p>no couch yet? ask in the server for the app.</p></div>
<script>setTimeout(function(){location.href=${JSON.stringify(deep)}},400)</script>`;
  return new Response(html, { headers: { "content-type": "text/html; charset=utf-8", "cache-control": "no-store", "referrer-policy": "no-referrer" } });
}

/** one of your own devices (couch on a PC, a Mac, an iPhone), Spotify-style */
type Dev = {
  id: string;
  name: string;
  kind: string;
  playing: any;
  at: number;
  /** what another of your devices asked it to do, waiting for it to collect */
  cmds: { cmd: string; pos?: number; from?: string; at: number }[];
};

export class Crew {
  members = new Map<string, Member>();
  /** each person's devices, by person then device id (in memory, like presence) */
  devices = new Map<string, Map<string, Dev>>();
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
    const base = { k: "s", v: r.videoId, m: r.metaId, ty: r.type, ti: r.title, sk: r.sk, p: this.roomNow(r), pa: r.paused, st: !!r.stalled, du: r.du ?? 0 };
    for (const ws of only ? [only] : [...s.guests.keys()]) {
      const g = s.guests.get(ws) ?? "";
      // the remote: everyone (shared) or just the guests the host handed it to
      const msg = JSON.stringify({ ...base, sh: !!r.shared || !!r.remote?.includes(g) });
      try {
        ws.send(msg);
      } catch {
        s.guests.delete(ws);
      }
    }
  }
  /** one message to the host's player */
  toHost(hostId: string, d: unknown) {
    const s = this.socks.get(hostId);
    try {
      s?.host?.send(JSON.stringify(d));
    } catch {}
  }
  /** one message to every guest (or one guest, by id) */
  toGuests(hostId: string, d: unknown, only?: string) {
    const s = this.socks.get(hostId);
    if (!s) return;
    const msg = JSON.stringify(d);
    for (const [ws, g] of s.guests) {
      if (only && g !== only) continue;
      try {
        ws.send(msg);
      } catch {}
    }
  }
  /** the host's list of who's on the couch: connected, buffering, remote */
  roster(hostId: string) {
    const r = this.rooms.get(hostId);
    const s = this.socks.get(hostId);
    if (!r || !s?.host) return;
    const live = new Set(s.guests.values());
    const now = Date.now();
    const list = r.guests.map((id) => {
      const m = this.members.get(id);
      return { id, name: m?.name ?? "someone", avatar: m?.avatar, live: live.has(id), remote: !!r.shared || !!r.remote?.includes(id), buf: (r.buf?.[id] ?? 0) > now - 15_000 };
    });
    this.toHost(hostId, { k: "guests", list });
  }
  /** a guest is off the couch: the host hears it, their line closes */
  dropGuest(hostId: string, g: string, kicked: boolean) {
    const r = this.rooms.get(hostId);
    if (!r) return;
    const name = this.members.get(g)?.name ?? "someone";
    r.guests = r.guests.filter((x) => x !== g);
    r.remote = (r.remote ?? []).filter((x) => x !== g);
    if (r.buf) delete r.buf[g];
    if (kicked) this.toGuests(hostId, { k: "kicked" }, g);
    const s = this.socks.get(hostId);
    for (const [ws, id] of s?.guests ?? []) {
      if (id !== g) continue;
      s!.guests.delete(ws);
      try {
        ws.close(1000, kicked ? "kicked" : "left");
      } catch {}
    }
    if (!kicked) this.toHost(hostId, { k: "left", g, name });
    this.roster(hostId);
  }
  /** the host is done: guests hold where they are and choose */
  endRoom(hostId: string) {
    const name = this.members.get(hostId)?.name ?? "the host";
    this.toGuests(hostId, { k: "hostleft", name });
    this.rooms.delete(hostId);
    for (const a of this.asks.values()) if (a.to === hostId && a.status === "pending") a.status = "ended";
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
      this.roster(hostId);
    } else {
      s.guests.set(server, who.id);
      this.pushRoom(hostId, server);
      this.roster(hostId);
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
          // the length of the host's file: guests check theirs is the same film
          du: num(d.du) || room.du,
          // the host's own one-way delay, so "pos" lines up with the server clock
          at: Date.now() - Math.max(0, Math.min(1500, num(d.lag))),
          wsAt: Date.now(),
        });
        this.pushRoom(hostId);
        return;
      }
      if (isHost && (d?.k === "kick" || d?.k === "remote")) {
        const room = this.rooms.get(hostId);
        const g = typeof d.g === "string" ? d.g : "";
        if (!room || !room.guests.includes(g)) return;
        if (d.k === "kick") this.dropGuest(hostId, g, true);
        else {
          room.remote = (room.remote ?? []).filter((x) => x !== g);
          if (d.on) room.remote.push(g);
          this.pushRoom(hostId);
          this.roster(hostId);
        }
        return;
      }
      // the host's couch panel asks who's here
      if (isHost && d?.k === "roster") {
        this.roster(hostId);
        return;
      }
      // the host closed the player: guests stop on this frame and choose
      if (isHost && d?.k === "bye") {
        this.endRoom(hostId);
        return;
      }
      // a guest's player is loading or buffering: the host waits for them
      if (!isHost && d?.k === "buf") {
        const room = this.rooms.get(hostId);
        if (!room) return;
        room.buf = room.buf ?? {};
        if (d.on) room.buf[who.id] = Date.now();
        else delete room.buf[who.id];
        this.roster(hostId);
        return;
      }
      // a guest with the remote: straight to the host's player
      if (!isHost && d?.k === "cmd") {
        const room = this.rooms.get(hostId);
        const action = d.a === "pause" || d.a === "play" || d.a === "seek" ? d.a : null;
        if (!room || !(room.shared || room.remote?.includes(who.id)) || !action) return;
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
      } else {
        s.guests.delete(server);
        this.roster(hostId);
      }
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
  env: Env;
  game: Game;
  constructor(state: DurableObjectState, env: Env) {
    this.storage = state.storage;
    this.env = env;
    this.game = new Game(state.storage, async (id, title, year) => !!(await clipPick(env, id, title, year)));
  }
  /** Discord's own look for someone: banner and display-name style (an hour) */
  looks = new Map<string, { v: any; at: number }>();
  async discordLook(id: string): Promise<any> {
    const c = this.looks.get(id);
    if (c && Date.now() - c.at < 3600_000) return c.v;
    if (!this.env.DISCORD_BOT_TOKEN) return null;
    let v: any = null;
    try {
      const r = await fetch(`https://discord.com/api/users/${id}`, { headers: { authorization: `Bot ${this.env.DISCORD_BOT_TOKEN}` } });
      if (r.ok) {
        const u: any = await r.json();
        const st = u?.display_name_styles;
        v = {
          banner: u?.banner ? `https://cdn.discordapp.com/banners/${id}/${u.banner}.png?size=1024` : undefined,
          bannerColor: typeof u?.accent_color === "number" ? "#" + u.accent_color.toString(16).padStart(6, "0") : undefined,
          nameStyle: st ? { font: num(st.font_id), effect: num(st.effect_id), colors: (Array.isArray(st.colors) ? st.colors : []).slice(0, 2).map((c: number) => "#" + num(c).toString(16).padStart(6, "0")) } : undefined,
        };
      }
    } catch {}
    this.looks.set(id, { v, at: Date.now() });
    return v;
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
    // display-name styles for a few people at once (names drawn in their font)
    if (p === "/v1/looks" && req.method === "GET") {
      const ids = (url.searchParams.get("ids") ?? "").split(",").filter((x) => /^[0-9]{5,25}$/.test(x)).slice(0, 40);
      const looks: Record<string, any> = {};
      await Promise.all(ids.map(async (id) => { looks[id] = (await this.discordLook(id))?.nameStyle ?? null; }));
      return json({ looks });
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
      const look = await this.discordLook(id);
      return json({
        id,
        look,
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
      // the ask is for what they're on right now: a different title later can't take it
      this.asks.set(id, { id, from, fromName: me.name, fromAvatar: me.avatar, to, at: Date.now(), status: "pending", metaId: str(them.watching.metaId, 100), title: str(them.watching.name, 200) ?? str(them.watching.title, 200) });
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
        if (a.status !== "pending") return json({ error: "that ask ran out", gone: true }, 409);
        // the host has moved to another title since: this ask was for the old one
        const nowOn = str(b?.metaId, 100) ?? str(this.members.get(a.to)?.watching?.metaId, 100);
        if (b?.accept && a.metaId && nowOn && nowOn !== a.metaId) {
          a.status = "ended";
          return json({ error: "they asked while you were on something else", moved: true }, 409);
        }
        a.status = b?.accept ? "accepted" : "declined";
        a.at = Date.now();
        if (a.status === "accepted") {
          const host = this.members.get(a.to);
          const r = this.rooms.get(a.to) ?? { host: a.to, metaId: str(host?.watching?.metaId, 100) ?? "", type: str(host?.watching?.type, 20) ?? "", videoId: "", pos: host?.pos ?? 0, paused: !!host?.paused, at: Date.now(), guests: [] };
          if (!r.guests.includes(a.from)) r.guests.push(a.from);
          this.rooms.set(a.to, r);
          this.roster(a.to);
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
      if (!(r.shared || r.remote?.includes(who.id))) return json({ error: "the host has the remote" }, 403);
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
        // couch chat: a line of text rides the same way as a reaction
        const text = typeof b?.text === "string" ? b.text.replace(/\s+/g, " ").trim().slice(0, 200) : "";
        const emoji = text ? "💬" : REACTIONS.includes(b?.emoji) || isEmoji(b?.emoji) ? b.emoji : null;
        if (!emoji) return json({ error: "bad reaction" }, 400);
        const mine = r.reacts.filter((x) => x.who === who.id && now - x.at < 3000).length;
        if (mine >= 20) return json({ ok: true }); // easy there
        r.reacts.push({ n: ++this.reactN, who: who.id, name: who.name, avatar: who.avatar, emoji, text: text || undefined, at: now });
        return json({ ok: true });
      }
      const since = Number(url.searchParams.get("since") ?? 0) || 0;
      // who's on this couch (so everyone can see who they're watching with)
      const people = [r.host, ...r.guests].map((id) => {
        const mm = this.members.get(id);
        return { id, name: mm?.name ?? "someone", avatar: mm?.avatar, host: id === r.host };
      });
      return json({ reactions: r.reacts.filter((x) => x.n > since), last: this.reactN, people });
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
        if (r && m[2] && m[2] === who.id) this.dropGuest(m[1], who.id, false);
        else if (m[1] === who.id && r) this.endRoom(m[1]);
        return json({ ok: true });
      }
    }
    return null;
  }

  async fetch(req: Request): Promise<Response> {
    const url = new URL(req.url);
    this.sweep();
    // a Discord card (public): stored in pieces (storage values are small)
    const artGet = url.pathname.match(/^\/art\/([A-Za-z0-9_.-]{1,80})\.gif$/);
    if (artGet) {
      const meta = await this.storage.get<{ n: number; at: number }>(`art:${artGet[1]}`);
      if (!meta) return new Response("no card", { status: 404 });
      const parts = await this.storage.get<ArrayBuffer>(Array.from({ length: meta.n }, (_, i) => `art:${artGet[1]}:${i}`));
      const out = new Uint8Array(Array.from(parts.values()).reduce((n, b) => n + b.byteLength, 0));
      let o = 0;
      for (let i = 0; i < meta.n; i++) {
        const b = new Uint8Array(parts.get(`art:${artGet[1]}:${i}`) ?? new ArrayBuffer(0));
        out.set(b, o);
        o += b.byteLength;
      }
      return new Response(out, { headers: { "content-type": "image/gif", "cache-control": "public, max-age=604800" } });
    }
    // from the worker only (it never forwards /internal/ from outside)
    if (url.pathname === "/internal/game-clip") {
      const c = this.game.clipOf(url.searchParams.get("g") ?? "", Number(url.searchParams.get("n")), url.searchParams.get("u") ?? "");
      return c ? json(c) : json({ error: "gone" }, 404);
    }
    let who: Who;
    try {
      who = JSON.parse(req.headers.get("x-who") ?? "");
    } catch {
      return json({ error: "sign in again" }, 401);
    }
    // the scenes game; only Discord sign-ins go on the leaderboards
    if (url.pathname.startsWith("/v1/game/")) return this.game.handle(req, url, who, /^\d+$/.test(who.id));

    const line = url.pathname.match(/^\/v1\/room\/([A-Za-z0-9_-]{1,40})\/ws$/);
    if (line && req.headers.get("upgrade") === "websocket") return this.openLine(line[1], who);

    // couch hands in a title's Discord card (a GIF, kept 30 days)
    const artPut = url.pathname.match(/^\/v1\/art\/([A-Za-z0-9_.-]{1,80})$/);
    if (artPut && req.method === "PUT") {
      const b = await req.arrayBuffer();
      const head = String.fromCharCode(...new Uint8Array(b.slice(0, 6)));
      if (b.byteLength > 1_500_000 || head !== "GIF89a") return json({ error: "not a card" }, 400);
      const key = artPut[1];
      const size = 100_000;
      const n = Math.ceil(b.byteLength / size);
      const put: Record<string, unknown> = { [`art:${key}`]: { n, at: Date.now() } };
      for (let i = 0; i < n; i++) put[`art:${key}:${i}`] = b.slice(i * size, (i + 1) * size);
      await this.storage.put(put);
      // forget cards nobody has shown in 30 days
      const old = await this.storage.list<{ n: number; at: number }>({ prefix: "art:", limit: 400 });
      for (const [k, v] of old) {
        if (k.split(":").length === 2 && v && Date.now() - v.at > 30 * 86400_000) {
          await this.storage.delete([k, ...Array.from({ length: v.n }, (_, i) => `${k}:${i}`)]);
        }
      }
      return json({ url: `${url.origin}/art/${key}.gif` });
    }

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

    // your devices: each says what it's playing every few seconds and collects
    // what your other devices asked it to do (pause, seek, stop, hand over)
    if (req.method === "PUT" && url.pathname === "/v1/me/device") {
      let b: any;
      try {
        b = await req.json();
      } catch {
        return json({ error: "bad json" }, 400);
      }
      const id = str(b?.id, 64);
      if (!id || !/^[A-Za-z0-9_-]+$/.test(id)) return json({ error: "bad device" }, 400);
      const mine = this.devices.get(who.id) ?? new Map<string, Dev>();
      this.devices.set(who.id, mine);
      const pl = b.playing && typeof b.playing === "object" ? b.playing : null;
      let stream: any = undefined;
      if (pl?.stream && typeof pl.stream === "object") {
        const raw = JSON.stringify(pl.stream);
        if (raw.length <= 6000) stream = pl.stream;
      }
      const prev = mine.get(id);
      const d: Dev = {
        id,
        name: str(b.name, 60) ?? "couch",
        kind: str(b.kind, 20) ?? "pc",
        playing: pl && {
          metaId: str(pl.metaId, 100),
          type: str(pl.type, 20),
          title: str(pl.title, 200),
          episode: str(pl.episode, 200),
          videoId: str(pl.videoId, 120),
          poster: str(pl.poster, 500),
          background: str(pl.background, 500),
          logo: str(pl.logo, 500),
          pos: num(pl.pos),
          duration: num(pl.duration),
          paused: !!pl.paused,
          stream,
        },
        at: Date.now(),
        cmds: [],
      };
      const cmds = prev?.cmds ?? [];
      mine.set(id, d);
      // gone quiet for a minute: not online anymore
      for (const [k, v] of mine) if (Date.now() - v.at > 60_000) mine.delete(k);
      const devices = [...mine.values()].map((v) => ({ id: v.id, name: v.name, kind: v.kind, playing: v.playing, at: v.at, me: v.id === id }));
      return json({ devices, cmds: cmds.filter((c) => Date.now() - c.at < 30_000) });
    }
    const dc = url.pathname.match(/^\/v1\/me\/device\/([A-Za-z0-9_-]{1,64})\/cmd$/);
    if (req.method === "POST" && dc) {
      let b: any;
      try {
        b = await req.json();
      } catch {
        return json({ error: "bad json" }, 400);
      }
      const cmd = str(b?.cmd, 20);
      if (!cmd || !["pause", "play", "seek", "stop", "next"].includes(cmd)) return json({ error: "bad command" }, 400);
      const d = this.devices.get(who.id)?.get(dc[1]);
      if (!d) return json({ error: "that device isn't online" }, 404);
      d.cmds.push({ cmd, pos: typeof b.pos === "number" ? b.pos : undefined, from: str(b.from, 64), at: Date.now() });
      if (d.cmds.length > 20) d.cmds.splice(0, d.cmds.length - 20);
      return json({ ok: true });
    }
    const dd = url.pathname.match(/^\/v1\/me\/device\/([A-Za-z0-9_-]{1,64})$/);
    if (req.method === "DELETE" && dd) {
      this.devices.get(who.id)?.delete(dd[1]);
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
