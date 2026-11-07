// Can it actually be watched yet? Not from official release dates (leaks
// show up before any digital release): couch looks for streams. Only titles
// that might be too new get checked (TMDB says no home release yet / not
// aired), so old films cost nothing. No streams found = "Unreleased".
import { api, type Meta } from "./api";
import { gather } from "./quickplay";

export const release = $state<Record<string, { unreleased: boolean; home?: string | null }>>({});
const asked = new Set<string>();

export function checkRelease(m: Meta | null | undefined) {
  if (!m?.id || asked.has(m.id) || (m.type !== "movie" && m.type !== "series")) return;
  asked.add(m.id);
  (async () => {
    const e = await api.enrich(m.type, m.id).catch(() => null);
    if (!e?.unreleased) return; // out at home for ages: playable, no check
    const id = m.type === "series" ? `${m.id.split(":")[0]}:1:1` : m.id;
    const streams = await gather(m.type, id, 12000);
    release[m.id] = { unreleased: streams.length === 0, home: e?.homeRelease ?? null };
  })();
}

export const isUnreleased = (id?: string) => !!(id && release[id]?.unreleased);

/** "on digital Mar 4" when the date is known */
export function releaseNote(id?: string) {
  const h = id ? release[id]?.home : null;
  if (!h || new Date(h).getTime() < Date.now()) return "no streams yet";
  return "on digital " + new Date(h).toLocaleDateString(undefined, { month: "short", day: "numeric" });
}
