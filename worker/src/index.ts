// midnite™ couch crew server: presence + watch together (ask / answer / room).
//
// One Durable Object holds who's on the couch right now, in memory only:
// nothing is written to disk, and anyone who stops sending heartbeats is
// forgotten after two minutes.
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

export interface Env {
  CREW: DurableObjectNamespace;
  /** signing key for sessions */
  CREW_CODE: string;
  /** the Discord server couch is for */
  GUILD_ID?: string;
  /** read-only GitHub token for the app's releases (updates) */
  GH_TOKEN?: string;
  /** owner/repo holding the releases */
  GH_REPO?: string;
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

type Who = { id: string; name: string; avatar?: string };
const SESSION_TTL = 7 * 86400;
const enc = new TextEncoder();
const b64u = (b: ArrayBuffer | Uint8Array) => btoa(String.fromCharCode(...new Uint8Array(b))).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
const unb64u = (s: string) => atob(s.replace(/-/g, "+").replace(/_/g, "/"));

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
  if (!me.ok) return json({ error: "discord didn't recognise that sign-in" }, 401);
  const u: any = await me.json();
  if (env.GUILD_ID) {
    const g = await fetch("https://discord.com/api/users/@me/guilds", { headers: h });
    if (!g.ok) return json({ error: "couldn't check your servers" }, 401);
    const list: any[] = await g.json();
    if (!list.some((x) => x?.id === env.GUILD_ID)) return json({ error: "not in the server", notMember: true }, 403);
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
type Ask = { id: string; from: string; fromName: string; fromAvatar?: string; to: string; at: number; status: "pending" | "accepted" | "declined" };
/** a host's playback, followed by the guests they let in */
type Room = { host: string; metaId: string; type: string; videoId: string; title?: string; pos: number; paused: boolean; at: number; guests: string[] };
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
    if (!url.pathname.startsWith("/v1/")) return json({ error: "not here" }, 404);
    if (req.method === "POST" && url.pathname === "/v1/auth") return auth(req, env);
    const who = await verify(env, req.headers.get("authorization"));
    if (!who) return json({ error: "sign in again" }, 401);
    if (url.pathname.startsWith("/v1/update/")) return updates(url, env);
    // the Durable Object trusts this header; set it here, never from the client
    const fwd = new Request(req);
    fwd.headers.set("x-who", JSON.stringify(who));
    const stub = env.CREW.get(env.CREW.idFromName("midnite"));
    return stub.fetch(fwd);
  },
};

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
  constructor(_state: DurableObjectState, _env: Env) {}

  sweep() {
    const now = Date.now();
    for (const [k, m] of this.members) if (now - m.at > STALE_MS) this.members.delete(k);
    for (const [k, a] of this.asks) if (now - a.at > ASK_TTL) this.asks.delete(k);
    for (const [k, r] of this.rooms) if (now - r.at > ROOM_TTL) this.rooms.delete(k);
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
      const me = this.members.get(from), them = this.members.get(to);
      if (!me) return json({ error: "you're not on the couch" }, 400);
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
      Object.assign(r, {
        metaId: str(b.metaId, 100) ?? r.metaId,
        type: str(b.type, 20) ?? r.type,
        videoId: str(b.videoId, 200) ?? r.videoId,
        title: str(b.title, 200),
        pos: num(b.pos),
        paused: !!b.paused,
        at: Date.now(),
      });
      return json({ guests: r.guests.map((g) => this.members.get(g)?.name ?? "someone") });
    }
    m = p.match(/^\/v1\/room\/([A-Za-z0-9_-]{1,40})(?:\/([A-Za-z0-9_-]{1,40}))?$/);
    if (m) {
      const r = this.rooms.get(m[1]);
      if (req.method === "GET") {
        if (!r || !r.videoId) return json({ room: null });
        // where the host is right now, by the server's clock
        const now = r.paused ? r.pos : r.pos + (Date.now() - r.at) / 1000;
        return json({ room: { ...r, now } });
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

    if (req.method === "GET" && url.pathname === "/v1/presence") {
      const members = [...this.members.values()].sort((a, b) => b.at - a.at);
      return json({ members });
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

    return json({ error: "not here" }, 404);
  }
}
