// "scenes": couch's guess-the-movie game, played with ClipCafe's most-watched
// scene of each film.
//
// A round shows a blink of the scene (0.1 s). You can guess right away for the
// most points, or reveal more (1 s, 3 s, 5 s, then the whole scene): every
// reveal costs points. One guess per round, four titles to pick from.
//
//   Daily     5 scenes, the same for everyone, easy first then nicher. One go
//             a day (UTC). Keeps a streak. Its own leaderboard each day.
//   Endless   couch Pro. Play whenever. Starts easy and gets nicher; three wrong guesses
//             end the run. Leaderboard of best runs.
//
// The server decides everything (which scene, what you've revealed, the
// score); the apps only show it. A scene's video is asked for by game and
// round number, never by the film's id, so the address doesn't give it away.

export const STAGES = [0.1, 1, 3, 5, 0]; // seconds shown; 0 = the whole scene
export const POINTS = [1000, 800, 600, 400, 200];
const DAILY_TIERS = [0, 0, 1, 1, 2];
const LIVES = 3;
const ROUND_SECONDS = 150; // longer than this on one scene: it counts as a miss
const BOARD = 100;

// IMDb ids only: the names come from Cinemeta, so a title always matches its
// id. Three shelves: everyone knows it / film fans know it / deep cuts.
const POOL: string[][] = [
  `tt0111161 tt0068646 tt0468569 tt0110912 tt0109830 tt0137523 tt1375666 tt0133093 tt0120737 tt0167260 tt0076759 tt0080684
   tt0120338 tt0107290 tt0114709 tt0110357 tt0088763 tt0499549 tt0848228 tt4154796 tt4154756 tt0241527 tt0325980 tt0172495
   tt0816692 tt0103064 tt0099785 tt0126029 tt0266543 tt0371746 tt0145487 tt7286456 tt0993846 tt0102926 tt0082971 tt0073195
   tt0083866 tt2294629 tt0317705 tt0361748 tt0095016 tt0087332 tt1431045 tt2015381 tt1745960 tt2911666 tt15398776 tt1517268
   tt0167261 tt0198781 tt0435761 tt1049413 tt0910970 tt0382932 tt1119646 tt0377092 tt0075148 tt0088247 tt0116629 tt1345836`,
  `tt0114369 tt0099685 tt0120815 tt0108052 tt0407887 tt0482571 tt1853728 tt0209144 tt0120689 tt0253474 tt0086190 tt0078748
   tt0090605 tt0081505 tt0075314 tt0086250 tt0119217 tt0120586 tt0169547 tt0180093 tt0246578 tt0268978 tt0338013 tt0372784
   tt0434409 tt0477348 tt1130884 tt2582802 tt1392190 tt3498820 tt1825683 tt6751668 tt1160419 tt9362722 tt4633694 tt0096874
   tt0091042 tt0093058 tt0118715 tt0112573 tt0112384 tt0114814 tt0117951 tt0120382 tt0167404 tt0208092 tt0245429 tt0264464
   tt0266697 tt0332280 tt0367594 tt1010048 tt0829482 tt0365748 tt0357413 tt0988045 tt1285016 tt1504320 tt2267998 tt2278388
   tt0892769 tt2096673 tt3783958 tt5052448 tt7131622 tt8579674 tt1877830 tt6710474 tt0092099 tt0071562 tt0083658 tt0093779
   tt0097576 tt0101414 tt0107048 tt0116282 tt0381061 tt0401792 tt0947798 tt1201607 tt1856101 tt2380307 tt3315342 tt0105236
   tt0110413 tt0113277 tt0112641 tt0062622 tt0054215 tt0095953`,
  `tt0050083 tt0060196 tt0073486 tt0047478 tt0034583 tt0038650 tt0064116 tt0078788 tt0082096 tt0086879 tt0087843 tt0095327
   tt0095765 tt0119698 tt0120735 tt0211915 tt0317248 tt0364569 tt0405094 tt0457430 tt0469494 tt0986264 tt1187043 tt1255953
   tt1832382 tt2106476 tt5311514 tt5074352 tt0056172 tt0057012 tt0066921 tt0071315 tt0077416 tt0080678 tt0081398 tt0084787
   tt0091763 tt0105695 tt0118849 tt0120601 tt0166924 tt0206634 tt0353969 tt0363163 tt0395169 tt0758758 tt1028532 tt1205489
   tt1291584 tt1305806 tt1454029 tt1979320 tt2024544 tt2119532 tt3170832 tt4729430 tt5027774 tt10272386 tt0190332 tt0119488
   tt1675434 tt0118799`,
].map((s) => s.trim().split(/\s+/));

export const POOL_IDS = new Set(POOL.flat());

type Title = { n: string; y: string; p?: string };
type Round = { id: string; options: string[]; stage: number; at: number; done: boolean };
type Run = {
  user: string;
  name: string;
  avatar?: string;
  ranked: boolean;
  mode: "daily" | "endless";
  day: string;
  rounds: Round[];
  score: number;
  lives: number;
  right: number;
  results: number[]; // per round: the stage it was got at, -1 = missed
  used: string[];
  at: number;
};
type Row = { id: string; name: string; avatar?: string; score: number };
type Stats = { streak: number; best: number; last: string; run: number; played: number };

export type Who = { id: string; name: string; avatar?: string };
type Storage = DurableObjectStorage;
/** null = couldn't tell (ClipCafe didn't answer): not held against the film */
type HasClip = (id: string, title: string, year: string) => Promise<boolean | null>;

const j = (v: unknown, status = 200) => new Response(JSON.stringify(v), { status, headers: { "content-type": "application/json", "cache-control": "no-store" } });
const today = () => new Date().toISOString().slice(0, 10);
const dayBefore = (d: string) => new Date(Date.parse(d + "T00:00:00Z") - 86400_000).toISOString().slice(0, 10);

/** small seeded generator (mulberry32) so the daily five are the same for everyone */
function rng(seed: string) {
  let h = 1779033703;
  for (let i = 0; i < seed.length; i++) h = Math.imul(h ^ seed.charCodeAt(i), 3432918353), (h = (h << 13) | (h >>> 19));
  return () => {
    h = (h + 0x6d2b79f5) | 0;
    let t = Math.imul(h ^ (h >>> 15), 1 | h);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
function shuffled<T>(a: T[], r: () => number): T[] {
  const o = a.slice();
  for (let i = o.length - 1; i > 0; i--) {
    const k = Math.floor(r() * (i + 1));
    [o[i], o[k]] = [o[k], o[i]];
  }
  return o;
}

export class Game {
  runs = new Map<string, Run>();
  constructor(
    private storage: Storage,
    private hasClip: HasClip,
  ) {}

  // ---- runs: in memory while this object is awake, and in storage so a
  // game survives it going to sleep (it does after a minute or two of quiet)
  private async run(gid: string): Promise<Run | undefined> {
    if (!/^[a-f0-9]{20}$/.test(gid)) return undefined;
    let r = this.runs.get(gid);
    if (!r) {
      r = await this.storage.get<Run>(`gr:${gid}`);
      if (r) this.runs.set(gid, r);
    }
    return r;
  }
  private async keep(gid: string, run: Run) {
    this.runs.set(gid, run);
    await this.storage.put(`gr:${gid}`, run);
    await this.storage.put(`gu:${run.user}:${run.mode}`, gid);
  }
  private async drop(gid: string, run: Run) {
    this.runs.delete(gid);
    await this.storage.delete(`gr:${gid}`);
    if ((await this.storage.get<string>(`gu:${run.user}:${run.mode}`)) === gid) await this.storage.delete(`gu:${run.user}:${run.mode}`);
  }

  // ---- titles (names from Cinemeta, kept)
  private async title(id: string): Promise<Title | null> {
    const k = `gt:${id}`;
    const have = await this.storage.get<Title | false>(k);
    if (have !== undefined) return have || null;
    try {
      const r = await fetch(`https://v3-cinemeta.strem.io/meta/movie/${id}.json`);
      const m = ((await r.json()) as any)?.meta;
      if (!m?.name) throw 0;
      const t: Title = { n: String(m.name), y: String(m.releaseInfo ?? m.year ?? "").slice(0, 4), p: m.poster };
      await this.storage.put(k, t);
      return t;
    } catch {
      return null; // Cinemeta hiccup: not remembered, tried again next time
    }
  }

  /** a film on this shelf that has a scene; ones without are remembered */
  private async pick(order: string[], used: string[]): Promise<string | null> {
    for (const id of order) {
      if (used.includes(id)) continue;
      if ((await this.storage.get(`gx2:${id}`)) === true) continue;
      const t = await this.title(id);
      if (!t) continue;
      const has = await this.hasClip(id, t.n, t.y);
      if (has) return id;
      if (has === false) await this.storage.put(`gx2:${id}`, true);
    }
    return null;
  }

  private async daily(day: string): Promise<string[]> {
    const k = `gday:${day}`;
    const have = await this.storage.get<string[]>(k);
    if (have) return have;
    const r = rng(day);
    const out: string[] = [];
    for (const tier of DAILY_TIERS) {
      const id = await this.pick(shuffled(POOL[tier], r), out);
      if (id) out.push(id);
    }
    if (out.length === DAILY_TIERS.length) await this.storage.put(k, out);
    return out;
  }

  /** four titles: the answer and three others from the same shelf */
  private async options(id: string, r: () => number): Promise<string[]> {
    const tier = Math.max(0, POOL.findIndex((p) => p.includes(id)));
    const names = new Set<string>();
    const out: string[] = [];
    const add = async (x: string) => {
      const t = await this.title(x);
      if (!t || names.has(t.n)) return;
      names.add(t.n);
      out.push(x);
    };
    await add(id);
    for (const x of shuffled(POOL[tier], r)) {
      if (out.length >= 4) break;
      if (x !== id) await add(x);
    }
    return shuffled(out, r);
  }

  private async round(run: Run, id: string): Promise<Round> {
    const r: Round = { id, options: await this.options(id, Math.random), stage: 0, at: Date.now(), done: false };
    run.rounds.push(r);
    run.used.push(id);
    return r;
  }

  private async view(gid: string, run: Run) {
    const r = run.rounds[run.rounds.length - 1];
    const options = [];
    for (const id of r.options) {
      const t = await this.title(id);
      options.push({ title: t?.n ?? "?", year: t?.y ?? "" });
    }
    const n = run.rounds.length;
    return {
      game: gid,
      mode: run.mode,
      n,
      total: run.mode === "daily" ? DAILY_TIERS.length : 0,
      options,
      stage: r.stage,
      stages: STAGES,
      points: POINTS,
      clip: `/game/clip/${gid}/${n}`,
      score: run.score,
      lives: run.mode === "endless" ? run.lives : null,
      level: run.mode === "endless" ? Math.min(2, Math.floor((n - 1) / 5)) + 1 : DAILY_TIERS[n - 1] + 1,
    };
  }

  // ---- boards
  private async board(key: string): Promise<Row[]> {
    return (await this.storage.get<Row[]>(key)) ?? [];
  }
  private async post(key: string, row: Row, keepBest: boolean) {
    const b = await this.board(key);
    const i = b.findIndex((x) => x.id === row.id);
    if (i >= 0) {
      if (keepBest && b[i].score >= row.score) {
        b[i] = { ...b[i], name: row.name, avatar: row.avatar };
      } else b[i] = row;
    } else b.push(row);
    b.sort((a, z) => z.score - a.score);
    await this.storage.put(key, b.slice(0, BOARD));
  }
  private async stats(user: string): Promise<Stats> {
    return (await this.storage.get<Stats>(`gs:${user}`)) ?? { streak: 0, best: 0, last: "", run: 0, played: 0 };
  }

  private async home(who: Who, ranked: boolean) {
    const day = today();
    const s = await this.stats(who.id);
    const mine = await this.storage.get<{ score: number; results: number[] }>(`gd:${day}:${who.id}`);
    // a streak that wasn't kept up yesterday or today is over
    const alive = s.last === day || s.last === dayBefore(day);
    const [daily, runs] = await Promise.all([this.board(`gl:${day}`), this.board("gl:run")]);
    const rank = (b: Row[]) => {
      const i = b.findIndex((x) => x.id === who.id);
      return i < 0 ? null : i + 1;
    };
    const next = Date.parse(day + "T00:00:00Z") + 86400_000;
    return {
      day,
      nextIn: Math.max(0, Math.floor((next - Date.now()) / 1000)),
      daily: mine ? { played: true, score: mine.score, results: mine.results } : { played: false },
      streak: alive ? s.streak : 0,
      bestStreak: s.best,
      bestRun: s.run,
      ranked,
      boards: { daily: daily.slice(0, 25), runs: runs.slice(0, 25) },
      ranks: { daily: rank(daily), runs: rank(runs) },
      stages: STAGES,
      points: POINTS,
    };
  }

  private async finish(gid: string, run: Run) {
    await this.drop(gid, run);
    const s = await this.stats(run.user);
    const row: Row = { id: run.user, name: run.name, avatar: run.avatar, score: run.score };
    s.played++;
    if (run.mode === "daily") {
      await this.storage.put(`gd:${run.day}:${run.user}`, { score: run.score, results: run.results });
      if (s.last !== run.day) {
        s.streak = s.last === dayBefore(run.day) ? s.streak + 1 : 1;
        s.last = run.day;
        s.best = Math.max(s.best, s.streak);
      }
      if (run.ranked) await this.post(`gl:${run.day}`, row, true);
    } else {
      s.run = Math.max(s.run, run.score);
      if (run.ranked && run.score > 0) await this.post("gl:run", row, true);
    }
    await this.storage.put(`gs:${run.user}`, s);
    const b = await this.board(run.mode === "daily" ? `gl:${run.day}` : "gl:run");
    const i = b.findIndex((x) => x.id === run.user);
    return { mode: run.mode, score: run.score, right: run.right, rounds: run.rounds.length, results: run.results, streak: s.streak, bestRun: s.run, rank: i < 0 ? null : i + 1 };
  }

  /** which film a round's video is (only the worker asks, for the owner of the game) */
  async clipOf(gid: string, n: number, user: string): Promise<{ id: string } | null> {
    const run = await this.run(gid);
    if (!run || run.user !== user) return null;
    const r = run.rounds[n - 1];
    return r ? { id: r.id } : null;
  }

  async handle(req: Request, url: URL, who: Who, ranked: boolean, pro: boolean, paused = false): Promise<Response> {
    const path = url.pathname.replace(/^\/v1\/game/, "");
    if (req.method === "GET" && path === "/home") return j({ ...(await this.home(who, ranked)), pro, paused });
    if (req.method !== "POST") return j({ error: "not here" }, 404);
    let b: any = {};
    try {
      b = await req.json();
    } catch {}

    if (paused) return j({ error: "scenes is taking a break while we sort out where the clips come from. back soon.", paused: true }, 503);
    if (path === "/start") {
      const mode = b?.mode === "daily" ? "daily" : "endless";
      const day = today();
      // the daily five are for everyone; endless runs come with couch Pro
      if (mode === "endless" && !pro) return j({ error: "Endless is part of couch Pro", pro: true }, 402);
      if (mode === "daily" && (await this.storage.get(`gd:${day}:${who.id}`))) return j({ error: "you've played today's scenes", played: true }, 409);
      // one run per person per mode: today's daily picks up where it was;
      // anything else left over is dropped
      const old = await this.storage.get<string>(`gu:${who.id}:${mode}`);
      const was = old ? await this.run(old) : undefined;
      if (old && was) {
        if (mode === "daily" && was.day === day) return j(await this.view(old, was));
        await this.drop(old, was);
      }
      const run: Run = { user: who.id, name: who.name, avatar: who.avatar, ranked, mode, day, rounds: [], score: 0, lives: LIVES, right: 0, results: [], used: [], at: Date.now() };
      const first = mode === "daily" ? (await this.daily(day))[0] : await this.pick(shuffled(POOL[0], Math.random), []);
      if (!first) return j({ error: "no scenes right now, try again in a bit" }, 503);
      const gid = crypto.randomUUID().replace(/-/g, "").slice(0, 20);
      await this.round(run, first);
      await this.keep(gid, run);
      return j(await this.view(gid, run));
    }

    const gid = String(b?.game ?? "");
    const run = await this.run(gid);
    if (!run || run.user !== who.id) return j({ error: "that game is over", gone: true }, 410);
    const r = run.rounds[run.rounds.length - 1];
    run.at = Date.now();

    if (path === "/reveal") {
      if (!r.done && r.stage < STAGES.length - 1) r.stage++;
      await this.keep(gid, run);
      return j({ stage: r.stage });
    }

    if (path === "/answer") {
      if (r.done) return j({ error: "already answered" }, 409);
      r.done = true;
      const pick = typeof b?.pick === "number" ? r.options[b.pick] : undefined;
      const late = Date.now() - r.at > ROUND_SECONDS * 1000;
      const correct = !late && pick === r.id;
      const gained = correct ? POINTS[r.stage] : 0;
      run.score += gained;
      run.results.push(correct ? r.stage : -1);
      if (correct) run.right++;
      else run.lives--;
      const t = await this.title(r.id);
      const answer = { index: r.options.indexOf(r.id), title: t?.n ?? "", year: t?.y ?? "", poster: t?.p ?? null, id: r.id };
      const base = { correct, late, gained, answer, score: run.score, lives: run.mode === "endless" ? run.lives : null };

      let nextId: string | null = null;
      if (run.mode === "daily") {
        nextId = (await this.daily(run.day))[run.rounds.length] ?? null;
      } else if (run.lives > 0) {
        // five scenes a shelf, then the next one (deep cuts from scene 11 on)
        const tier = Math.min(2, Math.floor(run.rounds.length / 5));
        nextId = (await this.pick(shuffled(POOL[tier], Math.random), run.used)) ?? (await this.pick(shuffled(POOL.flat(), Math.random), run.used));
      }
      if (!nextId) return j({ ...base, next: null, done: await this.finish(gid, run) });
      await this.round(run, nextId);
      await this.keep(gid, run);
      return j({ ...base, next: await this.view(gid, run), done: null });
    }

    if (path === "/quit") {
      // leaving a run counts what you had (a daily left early is still your go)
      return j({ done: await this.finish(gid, run) });
    }
    return j({ error: "not here" }, 404);
  }
}
