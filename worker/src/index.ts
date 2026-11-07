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
  constructor(_state: DurableObjectState, _env: Env) {}

  sweep() {
    const now = Date.now();
    for (const [k, m] of this.members) if (now - m.at > STALE_MS) this.members.delete(k);
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
        avatar: /^https:\/\/cdn\.discordapp\.com\//.test(str(b.avatar, 300)) ? str(b.avatar, 300) : undefined,
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
      return json({ ok: true });
    }

    return json({ error: "not here" }, 404);
  }
}
