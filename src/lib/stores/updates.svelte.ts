// New versions of couch. Auto-update (Settings → About) installs them by
// itself, but never in the middle of something you're watching.

import { api, isTauri, listen } from "../api";
import { app } from "./app.svelte";
import { pb } from "./playback.svelte";
import { toast, errorText } from "./toast.svelte";

export const updates = $state({
  available: null as string | null,
  checking: false,
  installing: false,
  progress: 0,
  error: "",
  /** the "update now / when I close couch" sheet is up */
  ask: false,
  /** downloaded, goes in when couch closes */
  onExit: false,
});

const asked = new Set<string>(); // don't ask twice about the same version

const desktop = () => isTauri && (app.platform === "windows" || app.platform === "linux");

export async function checkForUpdates(manual = false) {
  if (!desktop() || updates.checking || updates.installing) return;
  updates.checking = true;
  updates.error = "";
  try {
    const r = await api.updateCheck();
    updates.available = r.available;
    if (manual && !r.available) toast("you're on the newest couch", "ok");
    if (r.available && !updates.onExit && (manual || !asked.has(r.available))) askWhenFree();
  } catch (e) {
    updates.error = errorText(e);
    if (manual) toast(updates.error, "warn");
  }
  updates.checking = false;
}

let waiting = false;
// never pops up over a movie: waits until the player's closed
function askWhenFree() {
  const ask = () => {
    if (updates.available) asked.add(updates.available);
    updates.ask = true;
  };
  if (!pb.open) return ask();
  if (waiting) return;
  waiting = true;
  const t = setInterval(() => {
    if (pb.open) return;
    clearInterval(t);
    waiting = false;
    ask();
  }, 5000);
}

/** "when I close couch": fetch it now, it goes in on the way out */
export async function updateOnExit() {
  updates.ask = false;
  if (updates.onExit) return;
  try {
    await api.updateOnExit();
    updates.onExit = true;
    toast(`couch ${updates.available ?? ""} will install when you close the app`.replace("  ", " "), "ok");
  } catch (e) {
    toast(errorText(e), "error");
  }
}

export async function installUpdate() {
  updates.ask = false;
  if (updates.installing) return;
  updates.installing = true;
  updates.progress = 0;
  toast(`updating couch to ${updates.available ?? "the new version"}…`, "info");
  try {
    await api.updateInstall(); // restarts on success
  } catch (e) {
    updates.installing = false;
    toast(errorText(e), "error");
  }
}

let started = false;
export function startUpdates() {
  if (started || !desktop()) return;
  started = true;
  let total = 0, got = 0;
  listen<{ got: number; total?: number }>("update-progress", (p) => {
    got += p.got;
    if (p.total) total = p.total;
    updates.progress = total ? got / total : 0;
  });
  setTimeout(() => checkForUpdates(), 8000);
  // keep looking: every 10 minutes, and whenever you come back to couch
  setInterval(() => checkForUpdates(), 10 * 60 * 1000);
  let lastLook = Date.now();
  window.addEventListener("focus", () => {
    if (Date.now() - lastLook < 2 * 60 * 1000) return;
    lastLook = Date.now();
    checkForUpdates();
  });
}
