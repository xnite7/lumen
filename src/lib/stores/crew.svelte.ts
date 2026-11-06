// The midnite crew: who's on the couch right now. Polled gently while the
// app is in front; nothing happens at all until a crew link is set up.

import { api, type CrewMember } from "../api";

export const crew = $state({ setup: false, members: [] as CrewMember[], error: "", loaded: false });

let timer = 0;
export async function refreshCrew() {
  try {
    const r = await api.crewList();
    crew.setup = r.setup;
    crew.members = r.members ?? [];
    crew.error = "";
  } catch (e) {
    crew.error = String(e);
  }
  crew.loaded = true;
}

export function startCrew() {
  if (timer) return;
  refreshCrew();
  timer = window.setInterval(() => document.visibilityState === "visible" && crew.setup && refreshCrew(), 45_000);
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
