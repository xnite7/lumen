// How alike two people's taste is, from what they've watched and how they
// scored it. Watching the same things counts; agreeing on scores counts more.
import type { Taste, ProfileItem } from "./api";

export type Match = {
  pct: number;
  /** titles you've both watched */
  both: number;
  /** titles you've both scored */
  rated: number;
  /** both scored 8+ */
  bothLoved: ProfileItem[];
  /** they loved it, you haven't seen it */
  forYou: ProfileItem[];
};

export function tasteMatch(me: Taste | undefined, them: Taste | undefined): Match | null {
  if (!me?.ids?.length || !them?.ids?.length) return null;
  const a = new Set(me.ids);
  const b = new Set(them.ids);
  let both = 0;
  for (const id of a) if (b.has(id)) both++;
  const union = a.size + b.size - both;
  // overlap, gently stretched: two people rarely share more than a third
  const overlap = Math.min(1, Math.sqrt(both / Math.max(1, union)) * 1.7);

  let diff = 0, rated = 0;
  for (const [id, r] of Object.entries(me.ratings ?? {})) {
    const o = them.ratings?.[id];
    if (o) {
      diff += Math.abs(r - o);
      rated++;
    }
  }
  const agree = rated ? 1 - diff / rated / 9 : 0;
  const score = rated >= 3 ? 0.55 * agree + 0.45 * overlap : overlap;
  const pct = Math.max(1, Math.min(99, Math.round(score * 100)));

  const mineLoved = new Set(Object.entries(me.ratings ?? {}).filter(([, r]) => r >= 8).map(([id]) => id));
  const bothLoved = (them.loved ?? []).filter((i) => mineLoved.has(i.id)).slice(0, 20);
  const forYou = (them.loved ?? []).filter((i) => !a.has(i.id)).slice(0, 20);
  return { pct, both, rated, bothLoved, forYou };
}

export const matchWord = (p: number) => (p >= 85 ? "taste twins" : p >= 70 ? "same couch energy" : p >= 50 ? "pretty in sync" : p >= 30 ? "some overlap" : "opposites attract");
