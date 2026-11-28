// couch accounts and friends: for people without Discord (Sign in with
// Apple), and alongside the Discord crew for everyone else.
//
// An account is { id, name, handle }. Discord sign-ins get one the first
// time they pick a handle; Apple sign-ins get one when they sign in, with a
// long-lived token of couch's own (Apple's sign-in token can't be renewed
// quietly; ours can be thrown away by signing out or deleting the account).
//
// Friends are asked, accepted, removed, blocked. A friend sees what you're
// watching (if you let them); nobody else does.

type Storage = DurableObjectStorage;
export type Account = { id: string; name: string; handle?: string; avatar?: string; created: number; tok?: string };
type Edges = { friends: string[]; in: string[]; out: string[]; blocked: string[] };
export type Watching = { watching: Record<string, unknown> | null; paused: boolean; pos: number; duration: number; at: number };

const j = (v: unknown, status = 200) => new Response(JSON.stringify(v), { status, headers: { "content-type": "application/json", "cache-control": "no-store" } });
const hex = (b: ArrayBuffer) => [...new Uint8Array(b)].map((x) => x.toString(16).padStart(2, "0")).join("");
const sha = async (s: string) => hex(await crypto.subtle.digest("SHA-256", new TextEncoder().encode(s)));
const b64 = (s: string) => Uint8Array.from(atob(s.replace(/-/g, "+").replace(/_/g, "/")), (c) => c.charCodeAt(0));

const HANDLE = /^[a-z0-9_.]{3,20}$/;
const RESERVED = new Set(["admin", "couch", "midnite", "apple", "support", "help", "official", "staff", "mod", "moderator", "system", "everyone", "here", "null", "undefined"]);
const MAX_FRIENDS = 500;
const MAX_PENDING = 100;

/** a display name: trimmed, one line, no control characters */
export function cleanName(v: unknown): string | null {
  if (typeof v !== "string") return null;
  const s = v.replace(/[\u0000-\u001f\u007f\u200b-\u200f\u2028-\u202e]/g, "").replace(/\s+/g, " ").trim().slice(0, 40);
  return s.length >= 1 ? s : null;
}

// ---- Sign in with Apple: the identity token is a JWT signed by Apple (RS256)
let appleKeys: { at: number; keys: any[] } | null = null;
async function keys(force = false): Promise<any[]> {
  if (!force && appleKeys && Date.now() - appleKeys.at < 3600_000) return appleKeys.keys;
  const r = await fetch("https://appleid.apple.com/auth/keys");
  if (!r.ok) return appleKeys?.keys ?? [];
  const v: any = await r.json();
  appleKeys = { at: Date.now(), keys: Array.isArray(v?.keys) ? v.keys : [] };
  return appleKeys.keys;
}

/** Apple's stable id for this person in this app, or null if the token isn't good. */
export async function appleSubject(token: string, audiences: string[]): Promise<string | null> {
  try {
    const [h, p, s] = token.split(".");
    if (!h || !p || !s) return null;
    const head = JSON.parse(new TextDecoder().decode(b64(h)));
    if (head.alg !== "RS256" || typeof head.kid !== "string") return null;
    let jwk = (await keys()).find((k) => k.kid === head.kid);
    if (!jwk) jwk = (await keys(true)).find((k) => k.kid === head.kid); // Apple rotates them
    if (!jwk) return null;
    const key = await crypto.subtle.importKey("jwk", jwk, { name: "RSASSA-PKCS1-v1_5", hash: "SHA-256" }, false, ["verify"]);
    if (!(await crypto.subtle.verify("RSASSA-PKCS1-v1_5", key, b64(s), new TextEncoder().encode(`${h}.${p}`)))) return null;
    const c = JSON.parse(new TextDecoder().decode(b64(p)));
    const now = Date.now() / 1000;
    if (c.iss !== "https://appleid.apple.com" || !audiences.includes(c.aud)) return null;
    if (typeof c.exp !== "number" || c.exp < now - 30 || (typeof c.iat === "number" && c.iat > now + 300)) return null;
    return typeof c.sub === "string" && c.sub.length > 5 ? c.sub : null;
  } catch {
    return null;
  }
}

export class Social {
  /** what non-crew people are watching (the crew's own presence lives with the crew) */
  on = new Map<string, Watching>();

  constructor(
    private storage: Storage,
    /** the crew's presence, for friends who are couchers */
    private crewWatching: (id: string) => Watching | undefined,
  ) {}

  private acct = (id: string) => this.storage.get<Account>(`acct:${id}`);
  private async edges(id: string): Promise<Edges> {
    return (await this.storage.get<Edges>(`fr:${id}`)) ?? { friends: [], in: [], out: [], blocked: [] };
  }
  private card = (a: Account) => ({ id: a.id, name: a.name, handle: a.handle, avatar: a.avatar });

  // ---- from the worker's sign-in (never reachable from outside)

  /** Sign in with Apple: find or make the account, hand back a fresh couch token. */
  async appleSignIn(sub: string, name: string | null): Promise<{ account: Account; token: string }> {
    const id = `ap-${(await sha(`couch-apple:${sub}`)).slice(0, 24)}`;
    let a = await this.acct(id);
    if (!a) a = { id, name: name ?? "someone", created: Date.now() };
    else if (name && a.name === "someone") a.name = name;
    // a new token each sign-in; the old one stops working
    const token = hex(crypto.getRandomValues(new Uint8Array(32)).buffer);
    if (a.tok) await this.storage.delete(`tok:${a.tok}`);
    a.tok = await sha(token);
    await this.storage.put({ [`acct:${id}`]: a, [`tok:${a.tok}`]: id });
    return { account: a, token };
  }

  /** a couch token → whose it is */
  async byToken(token: string): Promise<Account | null> {
    if (!/^[0-9a-f]{64}$/.test(token)) return null;
    const id = await this.storage.get<string>(`tok:${await sha(token)}`);
    return id ? ((await this.acct(id)) ?? null) : null;
  }

  // ---- /v1/me/account, /v1/me/friends…, /v1/me/presence, /v1/me/report

  async handle(req: Request, path: string, who: { id: string; name: string; avatar?: string }): Promise<Response | null> {
    // guests and bare purchases aren't people here
    const person = /^\d+$/.test(who.id) || who.id.startsWith("ap-");
    if (!path.startsWith("/account") && !path.startsWith("/friends") && path !== "/presence" && path !== "/report") return null;
    if (!person) return j({ error: "sign in first" }, 403);
    let b: any = {};
    if (req.method !== "GET") {
      try {
        b = await req.json();
      } catch {
        b = {};
      }
    }
    const me = async (): Promise<Account> => (await this.acct(who.id)) ?? { id: who.id, name: who.name, avatar: who.avatar, created: Date.now() };

    if (path === "/account" && req.method === "GET") return j({ account: this.card(await me()) });

    if (path === "/account" && req.method === "PUT") {
      const a = await me();
      if (b.name !== undefined) {
        const n = cleanName(b.name);
        if (!n) return j({ error: "that name won't work" }, 400);
        a.name = n;
      }
      if (b.handle !== undefined) {
        const h = String(b.handle).trim().toLowerCase().replace(/^@/, "");
        if (!HANDLE.test(h) || RESERVED.has(h)) return j({ error: "3 to 20 letters, numbers, _ or ." }, 400);
        if (h !== a.handle) {
          const taken = await this.storage.get<string>(`h:${h}`);
          if (taken && taken !== a.id) return j({ error: "that name is taken" }, 409);
          if (a.handle) await this.storage.delete(`h:${a.handle}`);
          a.handle = h;
          await this.storage.put(`h:${h}`, a.id);
        }
      }
      if (/^\d+$/.test(who.id)) a.avatar = who.avatar; // Discord's picture, kept current
      await this.storage.put(`acct:${a.id}`, a);
      return j({ account: this.card(a) });
    }

    // sign out everywhere (Apple accounts): the couch token stops working
    if (path === "/account/signout" && req.method === "POST") {
      const a = await this.acct(who.id);
      if (a?.tok) {
        await this.storage.delete(`tok:${a.tok}`);
        a.tok = undefined;
        await this.storage.put(`acct:${a.id}`, a);
      }
      return j({ ok: true });
    }

    // delete the account: the name, the handle, every friendship, the token
    if (path === "/account" && req.method === "DELETE") {
      const a = await this.acct(who.id);
      const e = await this.edges(who.id);
      for (const other of new Set([...e.friends, ...e.in, ...e.out])) {
        const o = await this.edges(other);
        const drop = (l: string[]) => l.filter((x) => x !== who.id);
        await this.storage.put(`fr:${other}`, { friends: drop(o.friends), in: drop(o.in), out: drop(o.out), blocked: o.blocked });
      }
      const gone = [`acct:${who.id}`, `fr:${who.id}`, `gs:${who.id}`];
      if (a?.handle) gone.push(`h:${a.handle}`);
      if (a?.tok) gone.push(`tok:${a.tok}`);
      await this.storage.delete(gone);
      this.on.delete(who.id);
      return j({ ok: true });
    }

    if (path === "/presence" && req.method === "PUT") {
      const w = b.watching && typeof b.watching === "object" ? b.watching : null;
      const s = (v: unknown, n: number) => (typeof v === "string" ? v.slice(0, n) : undefined);
      const n = (v: unknown) => (typeof v === "number" && isFinite(v) ? v : 0);
      if (!w) this.on.delete(who.id);
      else {
        if (!this.on.has(who.id) && this.on.size >= 5000) return j({ ok: false });
        this.on.set(who.id, {
          watching: { title: s(w.title, 200), year: typeof w.year === "number" ? w.year : undefined, poster: s(w.poster, 500), background: s(w.background, 500), episode: s(w.episode, 200), metaId: s(w.metaId, 100), type: s(w.type, 20) },
          paused: !!b.paused,
          pos: n(b.pos),
          duration: n(b.duration),
          at: Date.now(),
        });
      }
      return j({ ok: true });
    }

    if (path === "/friends" && req.method === "GET") {
      const e = await this.edges(who.id);
      const cards = async (ids: string[], live: boolean) => {
        const out: any[] = [];
        for (const id of ids.slice(0, MAX_FRIENDS)) {
          const a = await this.acct(id);
          if (!a) continue;
          const c: any = this.card(a);
          if (live) {
            const p = this.crewWatching(id) ?? this.on.get(id);
            if (p?.watching && Date.now() - p.at < 90_000) Object.assign(c, { watching: p.watching, paused: p.paused, pos: p.pos, duration: p.duration });
          }
          out.push(c);
        }
        return out;
      };
      const acct = await me();
      return j({ me: this.card(acct), friends: await cards(e.friends, true), incoming: await cards(e.in, false), outgoing: await cards(e.out, false) });
    }

    const target = async (): Promise<Account | null> => {
      if (typeof b.id === "string" && /^[A-Za-z0-9-]{3,40}$/.test(b.id)) return (await this.acct(b.id)) ?? null;
      if (typeof b.handle === "string") {
        const id = await this.storage.get<string>(`h:${b.handle.trim().toLowerCase().replace(/^@/, "")}`);
        return id ? ((await this.acct(id)) ?? null) : null;
      }
      return null;
    };

    if (path === "/friends/add" && req.method === "POST") {
      const mine = await me();
      if (!mine.handle) return j({ error: "pick your own name first" }, 400);
      const t = await target();
      // (the same answer whether they exist or blocked you: nothing to probe)
      if (!t || t.id === who.id) return j({ error: "nobody by that name" }, 404);
      const e = await this.edges(who.id);
      const o = await this.edges(t.id);
      if (o.blocked.includes(who.id)) return j({ error: "nobody by that name" }, 404);
      if (e.friends.includes(t.id)) return j({ ok: true, friends: true });
      e.blocked = e.blocked.filter((x) => x !== t.id);
      // they'd already asked you: that's a yes from both
      if (e.in.includes(t.id)) {
        if (e.friends.length >= MAX_FRIENDS || o.friends.length >= MAX_FRIENDS) return j({ error: "too many friends" }, 400);
        e.in = e.in.filter((x) => x !== t.id);
        o.out = o.out.filter((x) => x !== who.id);
        e.friends.push(t.id);
        o.friends.push(who.id);
        await this.storage.put({ [`fr:${who.id}`]: e, [`fr:${t.id}`]: o, [`acct:${who.id}`]: mine });
        return j({ ok: true, friends: true });
      }
      if (e.out.includes(t.id)) return j({ ok: true });
      if (e.out.length >= MAX_PENDING) return j({ error: "too many requests waiting" }, 429);
      if (o.in.length >= MAX_PENDING) return j({ ok: true }); // quietly: their inbox is full
      e.out.push(t.id);
      o.in.push(who.id);
      await this.storage.put({ [`fr:${who.id}`]: e, [`fr:${t.id}`]: o, [`acct:${who.id}`]: mine });
      return j({ ok: true });
    }

    if (path === "/friends/accept" && req.method === "POST") {
      const t = await target();
      if (!t) return j({ error: "they're gone" }, 404);
      const e = await this.edges(who.id);
      if (!e.in.includes(t.id)) return j({ error: "no request from them" }, 404);
      const o = await this.edges(t.id);
      if (e.friends.length >= MAX_FRIENDS || o.friends.length >= MAX_FRIENDS) return j({ error: "too many friends" }, 400);
      e.in = e.in.filter((x) => x !== t.id);
      o.out = o.out.filter((x) => x !== who.id);
      if (!e.friends.includes(t.id)) e.friends.push(t.id);
      if (!o.friends.includes(who.id)) o.friends.push(who.id);
      await this.storage.put({ [`fr:${who.id}`]: e, [`fr:${t.id}`]: o, [`acct:${who.id}`]: await me() });
      return j({ ok: true });
    }

    // unfriend, turn a request down, take one back; with block: also never again
    if ((path === "/friends/remove" || path === "/friends/block") && req.method === "POST") {
      const id = typeof b.id === "string" && /^[A-Za-z0-9-]{3,40}$/.test(b.id) ? b.id : null;
      if (!id || id === who.id) return j({ error: "who?" }, 400);
      const e = await this.edges(who.id);
      const o = await this.edges(id);
      const drop = (l: string[], x: string) => l.filter((y) => y !== x);
      e.friends = drop(e.friends, id);
      e.in = drop(e.in, id);
      e.out = drop(e.out, id);
      o.friends = drop(o.friends, who.id);
      o.in = drop(o.in, who.id);
      o.out = drop(o.out, who.id);
      if (path === "/friends/block" && !e.blocked.includes(id)) e.blocked = [...e.blocked.slice(-499), id];
      await this.storage.put({ [`fr:${who.id}`]: e, [`fr:${id}`]: o });
      return j({ ok: true });
    }

    // report someone: kept for the moderators (newest 500)
    if (path === "/report" && req.method === "POST") {
      const id = typeof b.id === "string" ? b.id.slice(0, 40) : "";
      if (!id) return j({ error: "who?" }, 400);
      const list = (await this.storage.get<any[]>("reports")) ?? [];
      list.push({ by: who.id, about: id, what: typeof b.what === "string" ? b.what.slice(0, 60) : "", why: typeof b.why === "string" ? b.why.slice(0, 500) : "", at: Date.now() });
      await this.storage.put("reports", list.slice(-500));
      return j({ ok: true });
    }
    return null;
  }
}
