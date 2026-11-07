// Is it out to watch yet? (no digital / disc / TV release = "Unreleased").
// One TMDB lookup per title, remembered; Play buttons dim while it isn't.
import { api, type Meta } from "./api";

export const release = $state<Record<string, { unreleased: boolean; home?: string | null }>>({});
const asked = new Set<string>();

export function checkRelease(m: Meta | null | undefined) {
  if (!m?.id || asked.has(m.id) || (m.type !== "movie" && m.type !== "series")) return;
  asked.add(m.id);
  api
    .enrich(m.type, m.id)
    .then((e) => (release[m.id] = { unreleased: !!e?.unreleased, home: e?.homeRelease ?? null }))
    .catch(() => {});
}

export const isUnreleased = (id?: string) => !!(id && release[id]?.unreleased);

/** "out on digital Mar 4" when the date is known */
export function releaseNote(id?: string) {
  const h = id ? release[id]?.home : null;
  if (!h) return "not out to watch yet";
  return "on digital " + new Date(h).toLocaleDateString(undefined, { month: "short", day: "numeric" });
}
