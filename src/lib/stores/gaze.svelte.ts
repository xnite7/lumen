// Tobii events from Rust, plus a Svelte action that turns "you looked at
// this for a while" into a callback.

import { listen } from "../api";
import { app } from "./app.svelte";

export const gaze = $state({
  x: -1,
  y: -1,
  inside: false,
  present: true,
  head: { x: 0, y: 0, z: 600 },
  active: false,
  status: "off",
});

let started = false;
const watchers = new Set<() => void>();

export function initGaze() {
  if (started) return;
  started = true;
  listen<{ x: number; y: number; inside: boolean }>("tobii://gaze", (p) => {
    gaze.x = p.x;
    gaze.y = p.y;
    gaze.inside = p.inside;
    gaze.active = true;
    for (const w of watchers) w();
  });
  listen<{ present: boolean }>("tobii://presence", (p) => (gaze.present = p.present));
  listen<{ x: number; y: number; z: number }>("tobii://head", (p) => (gaze.head = p));
  listen<{ status: string }>("tobii://status", (p) => (gaze.status = p.status));
}

/** use:dwell={() => ...}: fires once when gaze rests on the element. */
export function dwell(node: HTMLElement, cb: () => void) {
  let since = 0;
  let fired = false;
  const check = () => {
    if (!app.settings.tobii.enabled || !app.settings.tobii.lookToPreview || !gaze.inside) return;
    const r = node.getBoundingClientRect();
    const hit = gaze.x >= r.left && gaze.x <= r.right && gaze.y >= r.top && gaze.y <= r.bottom;
    const now = performance.now();
    if (!hit) {
      since = 0;
      fired = false;
      return;
    }
    if (!since) since = now;
    if (!fired && now - since >= app.settings.tobii.dwellMs) {
      fired = true;
      cb();
    }
  };
  watchers.add(check);
  return {
    update(next: () => void) {
      cb = next;
    },
    destroy() {
      watchers.delete(check);
    },
  };
}
