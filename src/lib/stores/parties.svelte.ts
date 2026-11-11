// Watch parties: pick a title and a time, invite the crew. An invite lands
// as an envelope that slides onto the screen with a ticket inside; accept or
// pass right under it. When it's time, the host presses play and everyone
// who said yes walks straight onto the couch (no knocking).
import { api, type Party, type Meta } from "../api";
import { loadMe } from "./social.svelte";
import { toast, errorText } from "./toast.svelte";
import { crew } from "./crew.svelte";
import { pb } from "./playback.svelte";
import { go } from "../router.svelte";

export const parties = $state({
  list: [] as Party[],
  /** invites waiting to be opened, oldest first */
  invites: [] as Party[],
  me: "",
  /** the "throw a watch party" sheet */
  plan: null as null | { meta: Meta; videoId?: string },
});

const SEEN = "couch.partySeen";
const seen = (): Set<string> => {
  try {
    return new Set(JSON.parse(localStorage.getItem(SEEN) ?? "[]"));
  } catch {
    return new Set();
  }
};
function markSeen(id: string) {
  const s = seen();
  s.add(id);
  try {
    localStorage.setItem(SEEN, JSON.stringify([...s].slice(-200)));
  } catch {}
}

export async function loadParties() {
  if (!crew.setup) return;
  const me = (await loadMe())?.id ?? "";
  parties.me = me;
  const r = await api.social<{ parties: Party[] }>("GET", "/v1/parties").catch(() => null);
  if (!r) return;
  parties.list = r.parties ?? [];
  const s = seen();
  // new invites: not mine, not answered, not opened before, not over
  const fresh = parties.list.filter((p) => p.host !== me && !p.rsvp[me] && !s.has(p.id) && p.at > Date.now() - 30 * 60_000);
  for (const p of fresh) if (!parties.invites.some((x) => x.id === p.id)) parties.invites.push(p);
}

export async function rsvp(p: Party, yes: boolean) {
  markSeen(p.id);
  parties.invites = parties.invites.filter((x) => x.id !== p.id);
  try {
    const r = await api.social<{ party: Party }>("POST", `/v1/party/${p.id}/rsvp`, { yes });
    parties.list = parties.list.map((x) => (x.id === p.id ? r.party : x));
  } catch (e) {
    toast(errorText(e), "error");
  }
}
/** closed the envelope without answering: it waits on the Couch tab */
export function later(p: Party) {
  markSeen(p.id);
  parties.invites = parties.invites.filter((x) => x.id !== p.id);
}

export async function throwParty(meta: Meta, at: number, invite: string[] | "all", note: string, videoId?: string) {
  try {
    const title = { id: meta.id, type: meta.type, name: meta.name, poster: meta.poster, background: meta.background, logo: meta.logo, year: meta.releaseInfo ? String(meta.releaseInfo).slice(0, 4) : undefined, runtime: meta.runtime ? String(meta.runtime) : undefined };
    const r = await api.social<{ party: Party }>("POST", "/v1/party", { title, at, invite, note, videoId });
    parties.list = [...parties.list, r.party].sort((a, b) => a.at - b.at);
    parties.plan = null;
    toast(`invites sent. ${new Date(at).toLocaleString(undefined, { weekday: "short", hour: "numeric", minute: "2-digit" }).toLowerCase()} it is`, "ok");
  } catch (e) {
    toast(errorText(e), "error");
  }
}

export async function callOff(p: Party) {
  try {
    await api.social("DELETE", `/v1/party/${p.id}`);
    parties.list = parties.list.filter((x) => x.id !== p.id);
    toast("party called off", "info");
  } catch (e) {
    toast(errorText(e), "error");
  }
}

/** a party on right now (from 10 min before to 3 h after) */
export const liveNow = (p: Party) => Date.now() > p.at - 10 * 60_000 && Date.now() < p.at + 3 * 3600_000;
/** someone who said yes to a party of mine that's on now: let them straight in */
export function guestOfMyParty(from: string) {
  return parties.list.some((p) => p.host === parties.me && liveNow(p) && p.rsvp[from]?.yes);
}

// a nudge when it's time
const nudged = new Set<string>();
function remind() {
  for (const p of parties.list) {
    const going = p.host === parties.me || p.rsvp[parties.me]?.yes;
    if (!going || nudged.has(p.id) || pb.open) continue;
    const left = p.at - Date.now();
    if (left < 2 * 60_000 && left > -20 * 60_000) {
      nudged.add(p.id);
      const host = p.host === parties.me;
      toast(host ? `your watch party for ${p.title.name} is starting` : `${p.hostName}'s watch party for ${p.title.name} is starting`, "info", { label: host ? "start" : "open", run: () => go({ name: "couch" }) }, 15_000);
    }
  }
}

let started = false;
export function startParties() {
  if (started) return;
  started = true;
  setTimeout(loadParties, 4000);
  setInterval(() => document.visibilityState === "visible" && loadParties(), 60_000);
  setInterval(remind, 20_000);
  window.addEventListener("focus", () => loadParties());
}
