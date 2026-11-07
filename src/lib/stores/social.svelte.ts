// The social side of couch, kept on the crew server (Simkl's API has no
// favourites or reviews): your favourites, the crew's reviews and ratings,
// the activity feed on Home, and reactions in watch together.

import { api, type Meta } from "../api";
import { app } from "./app.svelte";

export type TitleRef = { id: string; type: string; name: string; poster?: string; background?: string; logo?: string };
export type Activity = {
  id: string;
  who: string;
  name: string;
  avatar?: string;
  kind: "review" | "rating" | "finished";
  title: TitleRef;
  rating?: number;
  text?: string;
  at: number;
};
export type Fav = TitleRef & { at: number };

const FAVS = "couch.favs";
const read = <T>(k: string, d: T): T => {
  try {
    return JSON.parse(localStorage.getItem(k) ?? "") as T;
  } catch {
    return d;
  }
};

export const social = $state({
  me: null as { id: string; name: string; avatar?: string } | null,
  favs: read<Fav[]>(FAVS, []),
  feed: [] as Activity[],
  feedLoaded: false,
});

export function ref(m: Meta | TitleRef): TitleRef {
  const id = (m.id ?? "").split(":")[0];
  return { id, type: m.type, name: m.name, poster: m.poster, background: (m as any).background, logo: (m as any).logo };
}

export async function loadMe() {
  if (!social.me) social.me = await api.discordMe().catch(() => null);
  return social.me;
}

// ------------------------------------------------------------- favourites

let favsPulled = false;
export async function loadFavs() {
  if (favsPulled) return social.favs;
  try {
    const r = await api.social<{ favs: Fav[] }>("GET", "/v1/favs");
    favsPulled = true;
    // first time on the server: hand it what this device had
    if (!r.favs.length && social.favs.length) await pushFavs();
    else setFavs(r.favs);
  } catch {
    /* offline: the local copy stands */
  }
  return social.favs;
}
function setFavs(f: Fav[]) {
  social.favs = f;
  try {
    localStorage.setItem(FAVS, JSON.stringify(f));
  } catch {}
}
async function pushFavs() {
  try {
    await api.social("PUT", "/v1/favs", { favs: social.favs });
  } catch {}
}
export const isFav = (id: string) => social.favs.some((f) => f.id === id.split(":")[0]);
export function toggleFav(m: Meta | TitleRef) {
  const r = ref(m);
  const on = !isFav(r.id);
  setFavs(on ? [{ ...r, at: Date.now() }, ...social.favs] : social.favs.filter((f) => f.id !== r.id));
  pushFavs();
  return on;
}

// ------------------------------------------------------ reviews + activity

/** Tell the crew: a review (text, maybe a score), a rating, or "finished". */
export async function postActivity(kind: Activity["kind"], m: Meta | TitleRef, rating?: number | null, text?: string) {
  // gatekeeper mode: what you watch stays yours
  if (kind === "finished" && app.settings.integrations.crewGatekeeper) return;
  try {
    await api.social("POST", "/v1/activity", { kind, title: ref(m), rating: rating ?? undefined, text: text?.trim() || undefined });
    loadFeed(true);
  } catch {}
}

export async function deleteReview(id: string) {
  await api.social("DELETE", `/v1/reviews/${encodeURIComponent(id.split(":")[0])}`);
  loadFeed(true);
}

export async function reviewsOf(id: string): Promise<Activity[]> {
  try {
    return (await api.social<{ reviews: Activity[] }>("GET", `/v1/reviews/${encodeURIComponent(id.split(":")[0])}`)).reviews;
  } catch {
    return [];
  }
}

let feedAt = 0;
export async function loadFeed(force = false) {
  if (!force && Date.now() - feedAt < 60_000) return social.feed;
  feedAt = Date.now();
  try {
    social.feed = (await api.social<{ feed: Activity[] }>("GET", "/v1/activity")).feed;
  } catch {}
  social.feedLoaded = true;
  return social.feed;
}

export function ago(t: number) {
  const s = Math.max(1, Math.round((Date.now() - t) / 1000));
  if (s < 60) return "just now";
  const m = Math.round(s / 60);
  if (m < 60) return `${m}m`;
  const h = Math.round(m / 60);
  if (h < 24) return `${h}h`;
  const d = Math.round(h / 24);
  if (d < 7) return `${d}d`;
  return new Date(t).toLocaleDateString(undefined, { month: "short", day: "numeric" });
}

// --------------------------------------------------------- finish prompt

/** After something ends: rate / favourite / review it. */
export const finish = $state({ meta: null as TitleRef | null });
const asked = new Set<string>();
export function askAfterFinish(m: Meta | TitleRef) {
  const r = ref(m);
  if (!r.id || asked.has(r.id)) return;
  asked.add(r.id);
  postActivity("finished", r);
  finish.meta = r;
}
