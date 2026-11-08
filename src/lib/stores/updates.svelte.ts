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
});

const desktop = () => isTauri && (app.platform === "windows" || app.platform === "linux");

export async function checkForUpdates(manual = false) {
  if (!desktop() || updates.checking || updates.installing) return;
  updates.checking = true;
  updates.error = "";
  try {
    const r = await api.updateCheck();
    updates.available = r.available;
    if (manual && !r.available) toast("you're on the newest couch", "ok");
    if (r.available && app.settings.general.autoUpdate) installWhenFree();
  } catch (e) {
    updates.error = errorText(e);
    if (manual) toast(updates.error, "warn");
  }
  updates.checking = false;
}

let waiting = false;
function installWhenFree() {
  if (!pb.open) return void installUpdate();
  if (waiting) return;
  waiting = true;
  const t = setInterval(() => {
    if (pb.open) return;
    clearInterval(t);
    waiting = false;
    installUpdate();
  }, 5000);
}

export async function installUpdate() {
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
