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
export type Account = { id: string; name: string; handle?: string; avatar?: string; created: number; tok?: string; /** an Apple account joined to a Discord one: that one is who you are */ linked?: string };
type Edges = { friends: string[]; in: string[]; out: string[]; blocked: string[] };
export type Watching = { watching: Record<string, unknown> | null; paused: boolean; pos: number; duration: number; at: number };

const j = (v: unknown, status = 200) => new Response(JSON.stringify(v), { status, headers: { "content-type": "application/json", "cache-control": "no-store" } });
const hex = (b: ArrayBuffer) => [...new Uint8Array(b)].map((x) => x.toString(16).padStart(2, "0")).join("");
const sha = async (s: string) => hex(await crypto.subtle.digest("SHA-256", new TextEncoder().encode(s)));
const b64 = (s: string) => Uint8Array.from(atob(s.replace(/-/g, "+").replace(/_/g, "/")), (c) => c.charCodeAt(0));

/** a review, as kept per title (`rev:<title id>`; the crew's reviews live in the same list) */
type Review = { id: string; who: string; name: string; handle?: string; avatar?: string; kind: string; title: any; rating?: number; text?: string; at: number; likes?: string[] };

// words that don't get shown (starred out): a plain filter, the report button does the rest
const ROUGH = /\b(n[i1]gg(?:er|a)s?|f[a@]gg?(?:ot)?s?|r[e3]tard(?:ed|s)?|k[i1]kes?|ch[i1]nks?|sp[i1]cs?|tr[a@]nn(?:y|ies)|c[u*]nts?|wh[o0]res?|sl[u*]ts?|rap(?:e|ist)s?|kys|kill yourself)\b/gi;
export const soften = (t: string) => t.replace(ROUGH, (w) => w[0] + "*".repeat(Math.max(1, w.length - 1)));

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
    /** may this person read the reports? */
    private isAdmin: (id: string) => Promise<boolean> = async () => false,
  ) {}

  private acct = (id: string) => this.storage.get<Account>(`acct:${id}`);
  private async edges(id: string): Promise<Edges> {
    return (await this.storage.get<Edges>(`fr:${id}`)) ?? { friends: [], in: [], out: [], blocked: [] };
  }
  private card = (a: Account) => ({ id: a.id, name: a.name, handle: a.handle, avatar: a.avatar });

  // ---- reviews, for everyone: read by anyone, written / liked by people with an account

  private async reviews(req: Request, title: string, sub: string, who: { id: string; name: string; avatar?: string }, person: boolean): Promise<Response> {
    const key = `rev:${title}`;
    const list = (await this.storage.get<Review[]>(key)) ?? [];
    let b: any = {};
    if (req.method !== "GET") b = await req.json().catch(() => ({}));

    if (!sub && req.method === "GET") {
      const blocked = person ? (await this.edges(who.id)).blocked : [];
      const out = list
        .filter((r) => !blocked.includes(r.who) && (r.text || r.rating))
        .map((r) => ({ id: r.id, who: r.who, name: r.name, handle: r.handle, avatar: r.avatar, rating: r.rating, text: r.text ? soften(r.text) : undefined, at: r.at, likes: r.likes?.length ?? 0, liked: !!r.likes?.includes(who.id), mine: r.who === who.id }))
        // the most liked first, then the newest
        .sort((x, y) => y.likes - x.likes || y.at - x.at)
        .slice(0, 100);
      const rated = list.filter((r) => typeof r.rating === "number" && r.rating > 0);
      return j({ reviews: out, count: rated.length, average: rated.length ? Math.round((rated.reduce((n, r) => n + (r.rating ?? 0), 0) / rated.length) * 10) / 10 : null, admin: await this.isAdmin(who.id) });
    }
    if (!person) return j({ error: "sign in to do that" }, 403);

    if (!sub && req.method === "POST") {
      const rating = Math.max(0, Math.min(10, Math.round(typeof b.rating === "number" ? b.rating : 0))) || undefined;
      const text = typeof b.text === "string" ? b.text.replace(/[\u0000-\u0008\u000b-\u001f\u007f]/g, "").trim().slice(0, 1200) || undefined : undefined;
      if (!text && !rating) return j({ error: "say something, or give it a score" }, 400);
      const a = await this.acct(who.id);
      const t = b.title && typeof b.title === "object" ? b.title : {};
      const s = (v: unknown, n: number) => (typeof v === "string" ? v.slice(0, n) : undefined);
      const old = list.find((r) => r.who === who.id);
      const r: Review = { id: old?.id ?? crypto.randomUUID(), who: who.id, name: a?.name ?? who.name, handle: a?.handle, avatar: a?.avatar ?? who.avatar, kind: text ? "review" : "rating", title: { id: title, type: s(t.type, 20) ?? "movie", name: s(t.name, 200) ?? title, poster: s(t.poster, 500) }, rating, text, at: Date.now(), likes: old && old.text === text ? old.likes : [] };
      await this.storage.put(key, [r, ...list.filter((x) => x.who !== who.id)].slice(0, 300));
      return j({ ok: true, id: r.id });
    }
    if (!sub && req.method === "DELETE") {
      await this.storage.put(key, list.filter((r) => r.who !== who.id));
      return j({ ok: true });
    }
    if (sub === "/like" && req.method === "POST") {
      const r = list.find((x) => x.id === b.id);
      if (!r) return j({ error: "that review is gone" }, 404);
      const likes = (r.likes ?? []).filter((x) => x !== who.id);
      if (b.on !== false && r.who !== who.id) likes.push(who.id);
      r.likes = likes.slice(-5000);
      await this.storage.put(key, list);
      return j({ ok: true, likes: r.likes.length, liked: r.likes.includes(who.id) });
    }
    // whoever runs the place takes a review down
    if (sub === "/remove" && req.method === "POST") {
      if (!(await this.isAdmin(who.id))) return j({ error: "not yours to remove" }, 403);
      await this.storage.put(key, list.filter((r) => r.id !== b.id));
      return j({ ok: true });
    }
    return j({ error: "not found" }, 404);
  }

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

  /** a couch token → the Apple account it belongs to (not following a link) */
  private async rawByToken(token: string): Promise<Account | null> {
    if (!/^[0-9a-f]{64}$/.test(token)) return null;
    const id = await this.storage.get<string>(`tok:${await sha(token)}`);
    return id ? ((await this.acct(id)) ?? null) : null;
  }

  /** who an account really is: the Discord one it's joined to, if any */
  async resolve(a: Account): Promise<Account> {
    if (!a.linked) return a;
    return (await this.acct(a.linked)) ?? { id: a.linked, name: a.name, created: a.created };
  }

  /** a couch token → whose it is */
  async byToken(token: string): Promise<Account | null> {
    const a = await this.rawByToken(token);
    return a ? this.resolve(a) : null;
  }

  /** Join an Apple account (by its token) to the Discord person asking: one
   *  person from then on, with the friends and the couch name of both. */
  private async link(who: { id: string; name: string; avatar?: string }, token: string): Promise<Response> {
    if (!/^\d+$/.test(who.id)) return j({ error: "sign in with Discord to link" }, 400);
    const a = await this.rawByToken(token);
    if (!a || !a.id.startsWith("ap-")) return j({ error: "sign in with Apple again" }, 401);
    if (a.linked === who.id) return j({ ok: true });
    if (a.linked) return j({ error: "that Apple sign-in is linked to another Discord" }, 409);
    const other = await this.storage.get<string>(`link:${who.id}`);
    if (other && other !== a.id) return j({ error: "this Discord is linked to another Apple sign-in" }, 409);
    const d: Account = (await this.acct(who.id)) ?? { id: who.id, name: who.name, avatar: who.avatar, created: Date.now() };
    const puts: Record<string, unknown> = {};
    const gone: string[] = [`fr:${a.id}`];
    // the couch name: Discord's if it has one, else the Apple account's comes over
    if (a.handle) {
      if (d.handle) gone.push(`h:${a.handle}`);
      else {
        d.handle = a.handle;
        puts[`h:${a.handle}`] = d.id;
      }
    }
    // friendships: everything the Apple account had now belongs to the Discord one
    const ea = await this.edges(a.id);
    const ed = await this.edges(d.id);
    const uniq = (l: string[]) => [...new Set(l)].filter((x) => x !== d.id && x !== a.id);
    ed.friends = uniq([...ed.friends, ...ea.friends]);
    ed.blocked = uniq([...ed.blocked, ...ea.blocked]);
    ed.in = uniq([...ed.in, ...ea.in]).filter((x) => !ed.friends.includes(x));
    ed.out = uniq([...ed.out, ...ea.out]).filter((x) => !ed.friends.includes(x));
    for (const o of new Set([...ea.friends, ...ea.in, ...ea.out])) {
      if (o === d.id) continue;
      const eo = await this.edges(o);
      const swap = (l: string[]) => [...new Set(l.map((x) => (x === a.id ? d.id : x)))];
      puts[`fr:${o}`] = { friends: swap(eo.friends), in: swap(eo.in).filter((x) => !swap(eo.friends).includes(x)), out: swap(eo.out).filter((x) => !swap(eo.friends).includes(x)), blocked: eo.blocked };
    }
    a.linked = d.id;
    a.handle = undefined;
    puts[`acct:${a.id}`] = a;
    puts[`acct:${d.id}`] = d;
    puts[`fr:${d.id}`] = ed;
    puts[`link:${d.id}`] = a.id;
    await this.storage.delete(gone);
    await this.storage.put(puts);
    const was = this.on.get(a.id);
    if (was) this.on.delete(a.id);
    return j({ ok: true, account: this.card(d) });
  }

  // ---- /v1/me/account, /v1/me/friends…, /v1/me/presence, /v1/me/report

  async handle(req: Request, path: string, who: { id: string; name: string; avatar?: string }): Promise<Response | null> {
    // guests and bare purchases aren't people here
    const person = /^\d+$/.test(who.id) || who.id.startsWith("ap-");
    const rv = path.match(/^\/reviews\/([A-Za-z0-9:._-]{1,100})(\/like|\/remove)?$/);
    if (rv) return this.reviews(req, rv[1], rv[2] ?? "", who, person);
    if (!path.startsWith("/account") && !path.startsWith("/friends") && path !== "/presence" && !path.startsWith("/report")) return null;
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

    if (path === "/account" && req.method === "GET") return j({ account: this.card(await me()), linked: !!(await this.storage.get<string>(`link:${who.id}`)) });

    if (path === "/account/link" && req.method === "POST") return this.link(who, typeof b.account === "string" ? b.account : "");

    // apart again: the Apple sign-in becomes its own (empty) account; friends stay with Discord
    if (path === "/account/unlink" && req.method === "POST") {
      const ap = await this.storage.get<string>(`link:${who.id}`);
      if (ap) {
        const a = await this.acct(ap);
        if (a) {
          a.linked = undefined;
          await this.storage.put(`acct:${ap}`, a);
        }
        await this.storage.delete(`link:${who.id}`);
      }
      return j({ ok: true });
    }

    // the reports, for whoever runs the place
    if (path === "/reports" && req.method === "GET") {
      if (!(await this.isAdmin(who.id))) return j({ error: "not yours to read" }, 403);
      const list = ((await this.storage.get<any[]>("reports")) ?? []).slice().reverse();
      const out: any[] = [];
      for (const r of list.slice(0, 200)) {
        const [by, about] = await Promise.all([this.acct(r.by), this.acct(r.about)]);
        out.push({ ...r, byName: by?.name, byHandle: by?.handle, aboutName: about?.name, aboutHandle: about?.handle });
      }
      return j({ reports: out });
    }
    if (path === "/reports/done" && req.method === "POST") {
      if (!(await this.isAdmin(who.id))) return j({ error: "not yours" }, 403);
      const list = (await this.storage.get<any[]>("reports")) ?? [];
      await this.storage.put("reports", list.filter((r) => !(r.at === b.at && r.by === b.by)));
      return j({ ok: true });
    }

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
      const ap = await this.storage.get<string>(`link:${who.id}`);
      const a = await this.acct(ap ?? who.id);
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
      // (a joined Apple sign-in goes with it)
      const ap = await this.storage.get<string>(`link:${who.id}`);
      if (ap) {
        const x = await this.acct(ap);
        gone.push(`acct:${ap}`, `link:${who.id}`);
        if (x?.tok) gone.push(`tok:${x.tok}`);
      }
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
      return j({ me: { ...this.card(acct), admin: await this.isAdmin(who.id) }, friends: await cards(e.friends, true), incoming: await cards(e.in, false), outgoing: await cards(e.out, false) });
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
      const rep: any = { by: who.id, about: id, what: typeof b.what === "string" ? b.what.slice(0, 60) : "", why: typeof b.why === "string" ? b.why.slice(0, 500) : "", at: Date.now() };
      // a reported review: which one, and what it said at the time
      if (typeof b.title === "string" && /^[A-Za-z0-9:._-]{1,100}$/.test(b.title) && typeof b.ref === "string") {
        const r = ((await this.storage.get<Review[]>(`rev:${b.title}`)) ?? []).find((x) => x.id === b.ref);
        if (r) Object.assign(rep, { title: b.title, ref: r.id, text: (r.text ?? "").slice(0, 600), titleName: r.title?.name });
      }
      list.push(rep);
      await this.storage.put("reports", list.slice(-500));
      return j({ ok: true });
    }
    return null;
  }
}
