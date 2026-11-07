// The midnite crew: who's on the couch right now. Polled gently while the
// app is in front; nothing happens at all until a crew link is set up.

import { api, type CrewMember } from "../api";
import { startTogether } from "./together.svelte";

export const crew = $state({ setup: false, members: [] as CrewMember[], error: "", loaded: false });

let timer = 0;
/** stills of what each person's watching, by member id */
export const frames = $state<Record<string, { at: number; img: string }>>({});
async function pullFrames(members: CrewMember[]) {
  for (const m of members) {
    if (!m.frameAt || frames[m.id]?.at === m.frameAt) continue;
    const at = m.frameAt;
    api
      .social<{ img: string | null }>("GET", `/v1/frame/${m.id}`)
      .then((r) => r.img && (frames[m.id] = { at, img: r.img }))
      .catch(() => {});
  }
}

let busy = false;
export async function refreshCrew() {
  if (busy) return;
  busy = true;
  try {
    const r = await api.crewList();
    crew.setup = r.setup;
    crew.members = r.members ?? [];
    crew.error = "";
    pullFrames(crew.members);
  } catch (e) {
    crew.error = String(e);
  }
  crew.loaded = true;
  busy = false;
}

export function startCrew() {
  if (timer) return;
  startTogether();
  refreshCrew();
  // every few seconds while couch is in front, and right away when you
  // come back to it: no tab-hopping to see who just sat down
  timer = window.setInterval(() => document.visibilityState === "visible" && refreshCrew(), 6_000);
  window.addEventListener("focus", () => refreshCrew());
  document.addEventListener("visibilitychange", () => document.visibilityState === "visible" && refreshCrew());
}

/** couch://crew?u=<server>&c=<code>, or the https://<server>/join?c=<code> page */
export function parseInvite(link: string): { url: string; code: string } | null {
  try {
    const raw = link.trim();
    if (raw.startsWith("couch://")) {
      const u = new URL(raw.replace(/^couch:\/\//, "https://x/"));
      const url = u.searchParams.get("u");
      const code = u.searchParams.get("c");
      return url && code ? { url, code } : null;
    }
    const u = new URL(raw);
    const code = u.searchParams.get("c") ?? new URLSearchParams(u.hash.slice(1)).get("c");
    return u.protocol === "https:" && code ? { url: u.origin, code } : null;
  } catch {
    return null;
  }
}

export async function joinCrew(link: string) {
  const p = parseInvite(link);
  if (!p) throw new Error("that crew link doesn't look right");
  const r = await api.crewJoin(p.url, p.code);
  crew.setup = r.setup;
  crew.members = r.members ?? [];
  crew.error = "";
  crew.loaded = true;
}
