// crew server: presence + watch together (ask / answer / room)
// midnite™ couch crew server.
//
// One Durable Object holds who's on the couch right now, in memory only:
// nothing is written to disk, and anyone who stops sending heartbeats is
// forgotten after two minutes. Every request needs the crew code (a shared
// secret set with `wrangler secret put CREW_CODE`).
//
//   PUT    /v1/presence        { id, name, watching, paused, pos, duration }
//   DELETE /v1/presence/:id
//   GET    /v1/presence     -> { members: [...] }
//   GET    /logo.png           the midnite mark (Discord's small image)

import logo from "./logo.png";

export interface Env {
  CREW: DurableObjectNamespace;
  CREW_CODE: string;
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
    if (!safeEqual(req.headers.get("x-crew-code") ?? "", env.CREW_CODE ?? "")) return json({ error: "wrong crew code" }, 401);
    const stub = env.CREW.get(env.CREW.idFromName("midnite"));
    return stub.fetch(req);
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
  together(req: Request, url: URL, b: any): Response | null {
    const okId = (v: unknown) => {
      const id = str(v, 40);
      return id && /^[A-Za-z0-9_-]+$/.test(id) ? id : null;
    };
    const p = url.pathname;
    // ask to join someone's couch
    if (req.method === "POST" && p === "/v1/ask") {
      const from = okId(b?.from), to = okId(b?.to);
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
      const asks = [...this.asks.values()].filter((a) => a.to === m![1] && a.status === "pending");
      return json({ asks });
    }
    m = p.match(/^\/v1\/ask\/([0-9a-f-]{36})$/);
    if (m) {
      const a = this.asks.get(m[1]);
      if (!a) return json({ status: "gone" });
      if (req.method === "GET") return json({ status: a.status });
      if (req.method === "POST") {
        if (b?.me !== a.to) return json({ error: "not yours" }, 403);
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
      const host = okId(b?.host);
      const r = host && this.rooms.get(host);
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
        if (r && m[2]) r.guests = r.guests.filter((g) => g !== m![2]);
        else this.rooms.delete(m[1]);
        return json({ ok: true });
      }
    }
    return null;
  }

  async fetch(req: Request): Promise<Response> {
    const url = new URL(req.url);
    this.sweep();

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
      const id = str(b?.id, 40);
      if (!id || !/^[A-Za-z0-9_-]+$/.test(id)) return json({ error: "bad id" }, 400);
      if (!this.members.has(id) && this.members.size >= MAX_MEMBERS) return json({ error: "couch is full" }, 429);
      const w = b.watching && typeof b.watching === "object" ? b.watching : null;
      this.members.set(id, {
        id,
        name: str(b.name, 40) || "someone",
        avatar: /^https:\/\/cdn\.discordapp\.com\//.test(str(b.avatar, 300) ?? "") ? str(b.avatar, 300) : undefined,
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
    const t = this.together(req, url, body);
    if (t) return t;

    return json({ error: "not here" }, 404);
  }
}
