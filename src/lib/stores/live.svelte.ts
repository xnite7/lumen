// Keeps couch fresh without you ever refreshing: a tick that moves when your
// lists change (Simkl sync, watching something) and every few minutes while
// couch is on screen. Rows and lists that show "your" stuff follow it.
import { listen } from "../api";

export const live = $state({ tick: 0 });

let started = false;
export function startLive() {
  if (started) return;
  started = true;
  const bump = () => live.tick++;
  listen("library-changed", bump);
  setInterval(() => document.visibilityState === "visible" && bump(), 3 * 60_000);
  document.addEventListener("visibilitychange", () => document.visibilityState === "visible" && bump());
}
