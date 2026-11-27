// A source for couch (a Stremio-style addon): the actual episode or film on
// YouTube, for titles that are officially up there (Fishtank and the like).
//   /yt/manifest.json
//   /yt/stream/<movie|series>/<tt…[:season:episode]>.json
// It searches YouTube for the title, then keeps only what looks like the
// real thing: long enough, the right episode, not a trailer / recap / review.
// Streams carry a ytId; couch on Windows and Mac plays those in its own
// player (through yt-dlp).

const UA = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0 Safari/537.36";
const JUNK = /\b(trailer|teaser|reaction|reacts?|review|recap|explained|explanation|clip|scene|scenes|behind the scenes|bts|interview|promo|preview|sneak peek|highlights?|best of|moments|compilation|ost|soundtrack|theme song|breakdown|ending|analysis|theory|theories|ranked|ranking|podcast|commentary|summary|in \d+ minutes|tribute|edit|amv|shorts?|bloopers|deleted|featurette|making of|first look|opening|intro|credits)\b/i;

type Vid = { id: string; title: string; channel: string; secs: number; views: number };
type Found = Vid & { score: number };

const norm = (s: string) =>
  s
    .toLowerCase()
    .normalize("NFKD")
    .replace(/[̀-ͯ]/g, "")
    .replace(/&/g, " and ")
    .replace(/[^a-z0-9]+/g, " ")
    .trim();
const words = (s: string) => norm(s).split(" ").filter((w) => w.length > 1 && !["the", "of", "and", "an"].includes(w));

function secsOf(t: string): number {
  const p = t.split(":").map(Number);
  return p.length === 3 ? p[0] * 3600 + p[1] * 60 + p[2] : p.length === 2 ? p[0] * 60 + p[1] : 0;
}
function viewsOf(t: string): number {
  const m = /([\d.,]+)\s*([KMB])?/i.exec(t.replace(/,/g, ""));
  if (!m) return 0;
  return Number(m[1]) * ({ K: 1e3, M: 1e6, B: 1e9 }[(m[2] ?? "").toUpperCase()] ?? 1);
}

/** YouTube's search page (videos only), read from the data it embeds. */
async function search(q: string, long: boolean): Promise<Vid[]> {
  // sp: videos only; with `long`, over 20 minutes
  const sp = long ? "EgQQARgC" : "EgIQAQ%3D%3D";
  const r = await fetch(`https://www.youtube.com/results?search_query=${encodeURIComponent(q)}&sp=${sp}&hl=en&gl=US`, {
    headers: { "user-agent": UA, "accept-language": "en-US,en;q=0.9", cookie: "SOCS=CAI; CONSENT=YES+1" },
  });
  if (!r.ok) throw new Error(`youtube ${r.status}`);
  const html = await r.text();
  const m = /var ytInitialData\s*=\s*(\{[\s\S]*?\});\s*<\/script>/.exec(html);
  if (!m) throw new Error("youtube: no results data");
  const data = JSON.parse(m[1]);
  const out: Vid[] = [];
  const walk = (o: any) => {
    if (!o || typeof o !== "object") return;
    if (o.videoRenderer?.videoId) {
      const v = o.videoRenderer;
      const title = (v.title?.runs ?? []).map((x: any) => x.text).join("") || v.title?.simpleText || "";
      out.push({
        id: v.videoId,
        title,
        channel: v.ownerText?.runs?.[0]?.text ?? v.longBylineText?.runs?.[0]?.text ?? "",
        secs: secsOf(v.lengthText?.simpleText ?? ""),
        views: viewsOf(v.viewCountText?.simpleText ?? ""),
      });
      return;
    }
    for (const k in o) walk(o[k]);
  };
  walk(data?.contents);
  return out;
}

type Want = { show: string; year: string; runtime: number; season?: number; episode?: number; epTitle?: string; movie: boolean };

export function score(v: Vid, w: Want): number | null {
  const t = norm(v.title);
  if (!v.secs) return null; // live / upcoming
  if (JUNK.test(v.title)) return null;
  // every real word of the title has to be there
  const need = words(w.show);
  if (!need.length || !need.every((x) => t.split(" ").includes(x))) return null;
  const mins = v.secs / 60;
  let s = 0;
  if (w.movie) {
    const min = Math.max(40, (w.runtime || 80) * 0.7);
    if (mins < min || (w.runtime && mins > w.runtime * 1.7)) return null;
    if (/\bfull (movie|film)\b/i.test(v.title)) s += 3;
    if (w.year && t.includes(w.year)) s += 1;
    // a sequel's number / another year is another film
    const other = /\b(19|20)\d\d\b/.exec(v.title);
    if (other && w.year && other[0] !== w.year) return null;
  } else {
    const min = Math.max(8, (w.runtime || 0) * 0.55);
    if (mins < min) return null;
    const S = w.season!, E = w.episode!;
    const sxe = new RegExp(`\\bs0*${S}\\s*[.x-]?\\s*e(p(isode)?)?\\s*0*${E}\\b|\\b${S}x0*${E}\\b`, "i");
    const season = new RegExp(`\\b(season|series)\\s*0*${S}\\b|\\bs0*${S}\\b`, "i");
    const anySeason = /\b(season|series)\s*0*(\d+)\b|\bs0*(\d+)\s*[.x-]?\s*e/i.exec(v.title);
    const ep = new RegExp(`\\b(episode|ep|e|part|pt|day|week|chapter)\\.?\\s*#?0*${E}\\b|#0*${E}\\b`, "i");
    const named = w.epTitle && norm(w.epTitle).length >= 5 && !/^episode \d+$/i.test(w.epTitle) && t.includes(norm(w.epTitle));
    // another season said out loud: not this one
    if (anySeason && Number(anySeason[2] ?? anySeason[3]) !== S) return null;
    if (sxe.test(v.title)) s += 6;
    else if (ep.test(v.title) && (season.test(v.title) || S === 1)) s += season.test(v.title) ? 5 : 3;
    else if (named) s += 4;
    else return null;
    if (named) s += 2;
    if (/\bfull episode\b/i.test(v.title)) s += 2;
  }
  // the show's own channel beats a re-upload
  const ch = norm(v.channel);
  if (need.every((x) => ch.includes(x))) s += 3;
  if (/official/i.test(v.channel) || /official/i.test(v.title)) s += 1;
  s += Math.min(2, Math.log10(Math.max(1, v.views)) / 4);
  return s;
}

const fmt = (secs: number) => {
  const h = Math.floor(secs / 3600), m = Math.floor((secs % 3600) / 60);
  return h ? `${h}h ${m}m` : `${m}m`;
};
const views = (n: number) => (n >= 1e6 ? `${(n / 1e6).toFixed(1)}M` : n >= 1e3 ? `${Math.round(n / 1e3)}K` : String(n)) + " views";

async function find(type: string, id: string): Promise<{ streams: any[]; debug: any }> {
  const [imdb, s, e] = id.split(":");
  const movie = type === "movie";
  const mr = await fetch(`https://v3-cinemeta.strem.io/meta/${movie ? "movie" : "series"}/${imdb}.json`);
  const meta: any = ((await mr.json().catch(() => null)) as any)?.meta;
  if (!meta?.name) return { streams: [], debug: { error: "no meta" } };
  const w: Want = { show: meta.name, year: String(meta.releaseInfo ?? meta.year ?? "").slice(0, 4), runtime: parseInt(String(meta.runtime ?? "")) || 0, movie };
  const qs: string[] = [];
  if (movie) {
    qs.push(`${w.show} ${w.year} full movie`);
  } else {
    w.season = Number(s) || 1;
    w.episode = Number(e) || 1;
    const ep = (meta.videos ?? []).find((v: any) => v.season === w.season && (v.episode ?? v.number) === w.episode);
    w.epTitle = ep?.name ?? ep?.title;
    qs.push(`${w.show} season ${w.season} episode ${w.episode}`);
    if (w.epTitle && !/^episode \d+$/i.test(w.epTitle)) qs.push(`${w.show} ${w.epTitle}`);
  }
  const seen = new Map<string, Found>();
  const errors: string[] = [];
  let raw = 0;
  for (const q of qs) {
    for (const long of [true, false]) {
      // the long-videos filter first; the plain search only if that found nothing
      if (!long && seen.size) break;
      try {
        const list = await search(q, long);
        raw += list.length;
        for (const v of list) {
          const sc = score(v, w);
          if (sc !== null && (!seen.has(v.id) || seen.get(v.id)!.score < sc)) seen.set(v.id, { ...v, score: sc });
        }
      } catch (x) {
        errors.push(String(x));
      }
    }
  }
  const all = [...seen.values()].sort((a, b) => b.score - a.score);
  // only what's about as sure as the best one (a clear match pushes out the maybes)
  const best = all.filter((v) => v.score >= all[0].score - 4).slice(0, 4);
  const streams = best.map((v) => ({
    name: "YouTube",
    title: `${v.title}\n${v.channel} · ${fmt(v.secs)} · ${views(v.views)}`,
    ytId: v.id,
    behaviorHints: { bingeGroup: `youtube-${norm(v.channel).replace(/ /g, "-")}` },
  }));
  return { streams, debug: { want: w, queries: qs, raw, kept: best.map((v) => ({ t: v.title, c: v.channel, m: Math.round(v.secs / 60), s: +v.score.toFixed(1) })), errors } };
}

export async function youtubeSource(url: URL, ctx: ExecutionContext): Promise<Response> {
  const cors = { "access-control-allow-origin": "*", "content-type": "application/json" };
  const path = url.pathname.replace(/^\/yt/, "");
  if (path === "" || path === "/" || path === "/manifest.json") {
    return new Response(
      JSON.stringify({
        id: "app.midnite.couch.youtube",
        version: "1.0.0",
        name: "couch YouTube",
        description: "Full episodes and films that are on YouTube (not trailers or clips), matched to the title you opened.",
        logo: `${url.origin}/couch.png`,
        types: ["movie", "series"],
        idPrefixes: ["tt"],
        resources: ["stream"],
        catalogs: [],
        behaviorHints: { configurable: false },
      }),
      { headers: { ...cors, "cache-control": "public, max-age=3600" } },
    );
  }
  const m = /^\/stream\/(movie|series)\/(tt\d+(?::\d+:\d+)?)\.json$/.exec(decodeURIComponent(path));
  if (!m) return new Response(JSON.stringify({ streams: [] }), { headers: cors });
  const debug = url.searchParams.has("debug");
  const key = new Request(`https://couch.yt/v1/${m[1]}/${m[2]}`);
  if (!debug) {
    const hit = await caches.default.match(key);
    if (hit) return new Response(hit.body, { headers: { ...cors, "cache-control": "public, max-age=1800" } });
  }
  const r = await find(m[1], m[2]);
  const body = JSON.stringify(debug ? r : { streams: r.streams });
  // found: kept six hours; nothing: asked again in half an hour (it may get uploaded)
  if (!debug && !r.debug.errors?.length) ctx.waitUntil(caches.default.put(key, new Response(body, { headers: { "content-type": "application/json", "cache-control": `public, max-age=${r.streams.length ? 21600 : 1800}` } })));
  return new Response(body, { headers: { ...cors, "cache-control": debug ? "no-store" : "public, max-age=1800" } });
}
