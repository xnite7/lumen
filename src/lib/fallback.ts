// Pictures that don't load: try IMDb's art (via metahub, the same source
// Stremio's Cinemeta uses) before giving up; then the spot shows couch's
// placeholder (`.img-ph` in base.css) instead of a broken-image icon.

const imdb = (id?: string) => (id ?? "").split(":")[0].match(/^tt\d+$/)?.[0] ?? null;

/** IMDb artwork for a title: poster, background or logo. */
export function imdbArt(id: string | undefined, kind: "poster" | "background" | "logo"): string | null {
  const tt = imdb(id);
  return tt ? `https://images.metahub.space/${kind}/medium/${tt}/img` : null;
}

/** IMDb's still for one episode ("tt123:1:4" or id + season + episode). */
export function imdbEpisode(id: string | undefined, season?: number, episode?: number): string | null {
  const p = (id ?? "").split(":");
  const tt = imdb(p[0]);
  const s = season ?? Number(p[1]), e = episode ?? Number(p[2]);
  return tt && s >= 0 && e > 0 ? `https://episodes.metahub.space/${tt}/${s}/${e}/w780.jpg` : null;
}

type Opts = { alts?: (string | null | undefined)[]; onfail?: () => void; minWidth?: number } | (string | null | undefined)[];

/** use:fallback={[alt1, alt2]} or use:fallback={{ alts, onfail }} on an <img>. */
export function fallback(node: HTMLImageElement, opts: Opts) {
  let o = Array.isArray(opts) ? { alts: opts } : opts;
  let queue = (o.alts ?? []).filter(Boolean) as string[];
  const fail = () => {
    const next = queue.shift();
    if (next && next !== node.src) {
      node.src = next;
      return;
    }
    node.style.visibility = "hidden";
    node.parentElement?.classList.add("img-ph");
    o.onfail?.();
  };
  node.addEventListener("error", fail);
  // some hosts answer "no picture" with a tiny grey stand-in (YouTube's 120 px one)
  const loaded = () => {
    const min = Array.isArray(o) ? 0 : (o.minWidth ?? 0);
    if (min && node.naturalWidth > 0 && node.naturalWidth <= min) fail();
  };
  node.addEventListener("load", loaded);
  // already broken before we got here
  if (!node.getAttribute("src") || (node.complete && node.naturalWidth === 0)) fail();
  return {
    update(n: Opts) {
      o = Array.isArray(n) ? { alts: n } : n;
      queue = (o.alts ?? []).filter(Boolean) as string[];
      node.style.visibility = "";
      node.parentElement?.classList.remove("img-ph");
    },
    destroy() {
      node.removeEventListener("error", fail);
      node.removeEventListener("load", loaded);
    },
  };
}
